//! Explicit owner actions for musical material kept in the session Mere.
use super::{UiState, WorkspacePanel};
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::musical_context::{ContextItemId, is_supported_subject};
use woodshed_core::session_overview::{OverviewNodeId, SessionArtifactId};

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::settings::AppSettings;
    use woodshedding::pitch::PitchClass;

    fn major(root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: "chord:Major".into(),
            root: PitchClass::new(root),
        }
    }

    fn kept_card() -> (UiState, ContextItemId) {
        let mut ui = UiState::new();
        assert!(ui.focus_context_catalog(major(0)));
        ui.stage_current(None);
        assert!(ui.keep_current_card_nearby());
        let id = ui.musical_context.items().first().unwrap().id;
        (ui, id)
    }

    #[test]
    fn keeping_and_exploring_preserve_set_and_authored_root() {
        let (mut ui, id) = kept_card();
        let before = serde_json::to_value(&ui.set).unwrap();
        ui.overview_background.remove(&node_id(id));
        assert!(ui.keep_current_card_nearby());
        assert_eq!(ui.musical_context.items().len(), 1);
        assert!(!ui.overview_background.contains(&node_id(id)));
        assert!(ui.focus_context_catalog(major(9)));
        assert!(ui.keep_context_focus_nearby());
        assert_eq!(ui.musical_context.items().len(), 2);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        assert!(ui.open_musical_context(id));
        assert_eq!(ui.context_focus, Some(major(0)));
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Practice));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        assert!(ui.hear_musical_context(id));
        assert!(!ui.audio_requests.is_empty());
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
    }

    #[test]
    fn explicit_add_targets_active_set_without_changing_originating_owner() {
        let (mut ui, id) = kept_card();
        let owner = ui.working_sets.active_id;
        let before = serde_json::to_value(&ui.set).unwrap();
        let target = ui.create_working_set_from("New passage", Default::default());
        assert_ne!(owner, target);
        assert!(ui.add_musical_context_to_set(id));
        assert_eq!(ui.set.cards.len(), 1);
        let first_id = ui.set.cards[0].id;
        assert!(ui.add_musical_context_to_set(id));
        assert_eq!(ui.set.cards.len(), 2);
        assert_ne!(ui.set.cards[1].id, first_id);
        assert_eq!(ui.musical_context.get(id).unwrap().owner, owner);
        assert_eq!(
            serde_json::to_value(ui.working_sets.get(owner, &ui.set).unwrap()).unwrap(),
            before
        );
        let active_before = serde_json::to_value(&ui.set).unwrap();
        assert!(ui.remove_musical_context(id));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), active_before);
    }

    #[test]
    fn retained_context_restores_before_its_scene_presentation() {
        let (mut ui, id) = kept_card();
        let node = node_id(id);
        ui.overview_positions.insert(node.clone(), (24.0, 86.0));
        ui.overview_roles.insert(node.clone(), "pinned".into());
        let json = serde_json::to_string(&ui.to_persisted()).unwrap();
        let session = serde_json::from_str(&json).unwrap();
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, AppSettings::default());
        assert_eq!(reopened.musical_context.get(id).unwrap().subject, major(0));
        assert_eq!(reopened.overview_positions.get(&node), Some(&(24.0, 86.0)));
        assert_eq!(
            reopened.overview_roles.get(&node).map(String::as_str),
            Some("pinned")
        );
        assert!(reopened.overview_background.contains(&node));
        assert_eq!(reopened.overview_focus.as_ref(), Some(&node));
        assert!(reopened.remove_musical_context(id));
        assert!(!reopened.overview_positions.contains_key(&node));
        assert!(!reopened.overview_roles.contains_key(&node));
        assert!(!reopened.overview_background.contains(&node));
        assert!(reopened.overview_focus.is_none());
    }

    #[test]
    fn stale_context_refuses_owner_actions_but_remains_removable() {
        let (mut ui, id) = kept_card();
        let json = serde_json::to_string(&ui.musical_context)
            .unwrap()
            .replace("chord:Major", "chord:Missing Formula");
        ui.musical_context = serde_json::from_str(&json).unwrap();
        let before = serde_json::to_value(&ui.set).unwrap();
        let stage = ui.stage.catalog_id();
        assert!(!ui.open_musical_context(id));
        assert!(!ui.hear_musical_context(id));
        assert!(!ui.add_musical_context_to_set(id));
        assert_eq!(ui.stage.catalog_id(), stage);
        assert!(ui.audio_requests.is_empty());
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        assert!(ui.musical_context.get(id).is_some());
        assert!(ui.remove_musical_context(id));
    }

    #[test]
    fn unsupported_recipe_and_removed_identity_do_not_fall_back_to_current_material() {
        let mut ui = UiState::new();
        assert!(!ui.keep_current_card_nearby());
        ui.context_focus = Some(KeyedCatalogRef {
            formula_id: "arpeggio:Major".into(),
            root: PitchClass::new(0),
        });
        assert!(!ui.keep_context_focus_nearby());
        assert!(ui.musical_context.items().is_empty());
        let (mut ui, id) = kept_card();
        assert!(ui.remove_musical_context(id));
        let before = serde_json::to_value(&ui.set).unwrap();
        assert!(!ui.open_musical_context(id));
        assert!(!ui.hear_musical_context(id));
        assert!(!ui.add_musical_context_to_set(id));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        assert!(ui.audio_requests.is_empty());
    }
}

fn node_id(id: ContextItemId) -> OverviewNodeId {
    OverviewNodeId::Artifact(SessionArtifactId::ContextItem(id))
}

impl UiState {
    fn keep_musical_context(&mut self, subject: Option<KeyedCatalogRef>) -> bool {
        let Some(subject) = subject.filter(is_supported_subject) else {
            self.overview_notice = Some("Keep a catalog chord or scale nearby. This material is unavailable or needs a playable recipe.".into());
            return false;
        };
        let label = subject.label().unwrap_or_else(|| subject.wire_key());
        let already_kept = self
            .musical_context
            .items()
            .iter()
            .any(|item| item.owner == self.working_sets.active_id && item.subject == subject);
        match self
            .musical_context
            .keep(self.working_sets.active_id, subject)
        {
            Ok(id) => {
                let node = node_id(id);
                if !already_kept {
                    self.overview_background.insert(node.clone());
                }
                self.overview_focus = Some(node);
                self.overview_notice = Some(format!(
                    "Kept {label} nearby for {}.",
                    self.working_sets.active_name
                ));
                true
            },
            Err(error) => {
                self.overview_notice = Some(format!("Could not keep this material: {error}"));
                false
            },
        }
    }

    pub fn keep_current_card_nearby(&mut self) -> bool {
        let subject = self.current_card().and_then(KeyedCatalogRef::from_card);
        self.keep_musical_context(subject)
    }

    pub fn keep_context_focus_nearby(&mut self) -> bool {
        self.keep_musical_context(self.context_focus.clone())
    }

    fn retained_context_subject(&mut self, id: ContextItemId) -> Option<KeyedCatalogRef> {
        let subject = self
            .musical_context
            .get(id)
            .map(|item| item.subject.clone());
        match subject {
            Some(subject) if is_supported_subject(&subject) => Some(subject),
            Some(_) => {
                self.overview_notice = Some("This kept material is unavailable in the current catalog. You can remove it from the Mere.".into());
                None
            },
            None => {
                self.overview_notice = Some("This kept material is no longer in the Mere.".into());
                None
            },
        }
    }

    pub fn open_musical_context(&mut self, id: ContextItemId) -> bool {
        let Some(subject) = self.retained_context_subject(id) else {
            return false;
        };
        if !self.focus_context_catalog(subject) {
            return false;
        }
        self.activate_workspace_panel(WorkspacePanel::Practice);
        true
    }

    pub fn hear_musical_context(&mut self, id: ContextItemId) -> bool {
        let Some(subject) = self.retained_context_subject(id) else {
            return false;
        };
        if !self.focus_context_catalog(subject) {
            return false;
        }
        self.audition_context_focus();
        true
    }

    pub fn add_musical_context_to_set(&mut self, id: ContextItemId) -> bool {
        let Some(subject) = self.retained_context_subject(id) else {
            return false;
        };
        let label = subject.label().unwrap_or_else(|| subject.wire_key());
        if !self.focus_context_catalog(subject) {
            return false;
        }
        let before = self.set.cards.len();
        self.add_context_focus_to_set();
        let added = self.set.cards.len() == before + 1;
        if added {
            self.overview_notice = Some(format!(
                "Added {label} to {}.",
                self.working_sets.active_name
            ));
        }
        added
    }

    pub fn remove_musical_context(&mut self, id: ContextItemId) -> bool {
        if self.musical_context.get(id).is_none() {
            return false;
        }
        self.musical_context.remove(id);
        let node = node_id(id);
        if self.overview_focus.as_ref() == Some(&node) {
            self.overview_focus = None;
        }
        self.overview_positions.remove(&node);
        self.overview_roles.remove(&node);
        self.overview_background.remove(&node);
        self.overview_notice = Some("Removed kept material from the Mere.".into());
        true
    }
}
