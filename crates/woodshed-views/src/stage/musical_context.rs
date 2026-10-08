//! Explicit owner actions for musical material kept in the session Mere.
use super::{UiState, WorkspacePanel};
use woodshed_core::audio::AudioRequest;
use woodshed_core::captured_arpeggio::CapturedArpeggio;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::history::{EngagementKind, ObservationProvenance, catalog_id_for_card};
use woodshed_core::musical_context::{ContextItemId, is_supported_subject};
use woodshed_core::session_overview::{OverviewNodeId, SessionArtifactId};
use woodshed_core::working_sets::WorkingSetId;
use woodshedding::rehearsal::{Card, CardId};

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

    fn kept_arpeggio() -> (UiState, ContextItemId) {
        let mut ui = UiState::new();
        assert!(ui.focus_context_catalog(major(0)));
        let mut keyed = major(0);
        keyed.formula_id = "arpeggio:Major".into();
        ui.set.push(keyed.to_card().unwrap());
        ui.set.cards[0].timing.bpm = None;
        ui.transport.bpm = 83.0;
        assert!(ui.keep_current_arpeggio_nearby());
        let id = ui.musical_context.items()[0].id;
        (ui, id)
    }

    #[test]
    fn captured_recipe_reopens_and_survives_source_removal_without_rebinding() {
        let (ui, id) = kept_arpeggio();
        let source_owner = ui.working_sets.active_id;
        let recipe = ui
            .musical_context
            .get(id)
            .unwrap()
            .captured_recipe
            .as_ref()
            .unwrap();
        let expected = recipe.preview().unwrap();
        let expected_card = recipe.card().unwrap();
        let source_id = recipe.source_card;
        let session =
            serde_json::from_str(&serde_json::to_string(&ui.to_persisted()).unwrap()).unwrap();
        let mut restored = UiState::new();
        restored.apply_persisted(&session, AppSettings::default());
        restored.set.cards.clear();
        restored.stage.set_root(9);
        restored.transport.bpm = 140.0;
        restored.arpeggio_source = Some(CardId(998));
        let stage = restored.stage.catalog_id();
        let root = restored.stage.root_idx;
        assert!(restored.hear_musical_context(id));
        match restored.audio_requests.last().unwrap() {
            AudioRequest::PreviewPitches {
                pitches,
                duration_s,
                strum_s,
            } => {
                assert_eq!((pitches.clone(), *duration_s, *strum_s), expected);
            },
            other => panic!("unexpected audio request: {other:?}"),
        }
        let preview = &restored.practice_history.recent(1)[0];
        assert_eq!(
            preview.provenance.as_ref().unwrap().working_set_id,
            Some(source_owner)
        );
        assert_eq!(
            preview.provenance.as_ref().unwrap().occurrence_id,
            source_id
        );
        assert!(restored.add_musical_context_to_set(id));
        let mut actual = restored.set.cards[0].clone();
        actual.id = expected_card.id;
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected_card).unwrap()
        );
        assert_eq!(restored.stage.catalog_id(), stage);
        assert_eq!(restored.stage.root_idx, root);
        assert_eq!(restored.arpeggio_source, Some(CardId(998)));
    }

    #[test]
    fn captured_recipe_add_uses_new_active_owner_and_new_occurrences() {
        let (mut ui, id) = kept_arpeggio();
        let source_owner = ui.working_sets.active_id;
        let original = serde_json::to_value(&ui.set).unwrap();
        let target = ui.create_working_set();
        ui.arpeggio_source = Some(CardId(999));
        let stage = ui.stage.catalog_id();
        assert!(ui.add_musical_context_to_set(id));
        assert!(ui.add_musical_context_to_set(id));
        assert_ne!(ui.set.cards[0].id, ui.set.cards[1].id);
        assert_eq!(ui.set.cursor, 0);
        assert_eq!(ui.stage.catalog_id(), stage);
        assert_eq!(ui.arpeggio_source, Some(CardId(999)));
        assert_eq!(
            serde_json::to_value(ui.working_sets.get(source_owner, &ui.set).unwrap()).unwrap(),
            original
        );
        for observation in ui.practice_history.recent(2) {
            assert_eq!(observation.kind, EngagementKind::Staged);
            assert_eq!(
                observation.provenance.as_ref().unwrap().working_set_id,
                Some(target)
            );
        }
        assert!(ui.open_musical_context(id));
        assert_eq!(ui.context_focus, Some(major(0)));
        assert_eq!(ui.set.cards.len(), 2);
    }

    #[test]
    fn malformed_or_mismatched_captured_recipe_refuses_actions_without_fallback() {
        let (ui, id) = kept_arpeggio();
        let original = serde_json::to_value(&ui.musical_context).unwrap();
        for corruption in ["instruction", "subject", "stale", "shape"] {
            let mut value = original.clone();
            match corruption {
                "instruction" => {
                    value["entries"][0]["captured_recipe"]["instruction"] = serde_json::json!({})
                },
                "subject" => value["entries"][0]["subject"]["root"] = serde_json::json!(7),
                "stale" => {
                    let json = serde_json::to_string(&value)
                        .unwrap()
                        .replace("Major", "Missing Formula");
                    value = serde_json::from_str(&json).unwrap();
                },
                "shape" => {
                    value["entries"][0]["captured_recipe"]["instruction"]["setting"]["voicing_idx"] =
                        serde_json::json!(usize::MAX)
                },
                _ => unreachable!(),
            }
            let mut corrupted = UiState::new();
            corrupted.musical_context = serde_json::from_value(value).unwrap();
            let stage = corrupted.stage.catalog_id();
            assert!(!corrupted.hear_musical_context(id), "{corruption}");
            assert!(!corrupted.add_musical_context_to_set(id), "{corruption}");
            assert!(!corrupted.open_musical_context(id), "{corruption}");
            assert!(corrupted.audio_requests.is_empty());
            assert!(corrupted.set.cards.is_empty());
            assert_eq!(corrupted.stage.catalog_id(), stage);
            assert!(corrupted.remove_musical_context(id));
        }
    }

    #[test]
    fn copied_selected_shape_freezes_inherited_window_and_keeps_source_unchanged() {
        let (mut ui, previous) = kept_arpeggio();
        assert!(ui.remove_musical_context(previous));
        ui.set.cards[0].setting.voicing_idx = Some(0);
        ui.set.cards[0].setting.fret_window = None;
        let source = serde_json::to_value(&ui.set.cards[0]).unwrap();
        assert!(ui.keep_current_arpeggio_nearby());
        let id = ui.musical_context.items()[0].id;
        assert_eq!(serde_json::to_value(&ui.set.cards[0]).unwrap(), source);
        let recipe = ui
            .musical_context
            .get(id)
            .unwrap()
            .captured_recipe
            .clone()
            .unwrap();
        let saved = recipe.card().unwrap();
        assert_eq!(saved.setting.voicing_idx, Some(0));
        assert!(saved.setting.fret_window.is_some());
        let expected = recipe.preview().unwrap();
        ui.stage.set_tuning(1);
        ui.stage.fret_start = 10;
        ui.stage.fret_count = 14;
        assert!(ui.hear_musical_context(id));
        assert!(
            matches!(ui.audio_requests.last(), Some(AudioRequest::PreviewPitches { pitches, duration_s, strum_s }) if (pitches.clone(), *duration_s, *strum_s) == expected)
        );
        assert!(ui.add_musical_context_to_set(id));
        let mut added = ui.set.cards.last().unwrap().clone();
        added.id = saved.id;
        assert_eq!(
            serde_json::to_value(added).unwrap(),
            serde_json::to_value(saved).unwrap()
        );
    }
}

fn node_id(id: ContextItemId) -> OverviewNodeId {
    OverviewNodeId::Artifact(SessionArtifactId::ContextItem(id))
}

impl UiState {
    /// Capture an authored arpeggio occurrence with its resolved playable setup.
    pub fn keep_current_arpeggio_nearby(&mut self) -> bool {
        let Some(card) = self.current_card() else {
            self.overview_notice = Some("Select an authored arpeggio Card to keep nearby.".into());
            return false;
        };
        let recipe =
            match CapturedArpeggio::capture(card, self.current_card_stage(), self.transport.bpm) {
                Ok(recipe) => recipe,
                Err(error) => {
                    self.overview_notice = Some(error);
                    return false;
                },
            };
        let existing: Vec<_> = self
            .musical_context
            .items()
            .iter()
            .map(|item| item.id)
            .collect();
        match self
            .musical_context
            .keep_recipe(self.working_sets.active_id, recipe)
        {
            Ok(id) => {
                let node = node_id(id);
                if !existing.contains(&id) {
                    self.overview_background.insert(node.clone());
                }
                self.overview_focus = Some(node);
                self.overview_notice = Some(format!(
                    "Kept this arpeggio recipe nearby for {}.",
                    self.working_sets.active_name
                ));
                true
            },
            Err(error) => {
                self.overview_notice = Some(error);
                false
            },
        }
    }

    fn captured_context(
        &self,
        id: ContextItemId,
    ) -> Result<(Card, Vec<f32>, f32, f32, WorkingSetId), String> {
        let item = self
            .musical_context
            .get(id)
            .ok_or("This kept material is no longer in the Mere.")?;
        let recipe = item
            .captured_recipe
            .as_ref()
            .ok_or("This kept material has no captured recipe.")?;
        let mut card = recipe.card()?;
        if KeyedCatalogRef::from_card(&card).as_ref() != Some(&item.subject) {
            return Err("The captured recipe does not match its retained musical identity.".into());
        }
        let (pitches, duration_s, strum_s) = recipe.preview()?;
        card.id = recipe.source_card;
        Ok((card, pitches, duration_s, strum_s, item.owner))
    }

    fn has_captured_context(&self, id: ContextItemId) -> bool {
        self.musical_context
            .get(id)
            .is_some_and(|item| item.captured_recipe.is_some())
    }

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
        if self.has_captured_context(id) {
            if let Err(error) = self.captured_context(id) {
                self.overview_notice = Some(error);
                return false;
            }
            let mut subject = self.musical_context.get(id).unwrap().subject.clone();
            subject.formula_id = subject.formula_id.replacen("arpeggio:", "chord:", 1);
            if !self.focus_context_catalog(subject) {
                return false;
            }
            self.activate_workspace_panel(WorkspacePanel::Practice);
            return true;
        }
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
        if self.has_captured_context(id) {
            let (card, pitches, duration_s, strum_s, owner) = match self.captured_context(id) {
                Ok(captured) => captured,
                Err(error) => {
                    self.overview_notice = Some(error);
                    return false;
                },
            };
            self.refresh_event_time();
            if let Some(catalog) = catalog_id_for_card(&card) {
                self.practice_history.record_observation(
                    self.now_ms,
                    catalog,
                    EngagementKind::Previewed,
                    None,
                    None,
                    ObservationProvenance::capture_in_set(&card, owner).ok(),
                );
            }
            self.request(AudioRequest::PreviewPitches {
                pitches,
                duration_s,
                strum_s,
            });
            return true;
        }
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
        if self.has_captured_context(id) {
            let (mut card, _, _, _, _) = match self.captured_context(id) {
                Ok(captured) => captured,
                Err(error) => {
                    self.overview_notice = Some(error);
                    return false;
                },
            };
            card.id = CardId::UNASSIGNED;
            self.refresh_event_time();
            self.set.push(card);
            if let Some(card) = self.set.cards.last() {
                if let Some(catalog) = catalog_id_for_card(card) {
                    self.practice_history.record_observation(
                        self.now_ms,
                        catalog,
                        EngagementKind::Staged,
                        None,
                        None,
                        ObservationProvenance::capture_in_set(card, self.working_sets.active_id)
                            .ok(),
                    );
                }
            }
            self.overview_notice = Some(format!(
                "Added the captured arpeggio recipe to {}.",
                self.working_sets.active_name
            ));
            return true;
        }
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
