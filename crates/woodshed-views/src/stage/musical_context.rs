//! Explicit owner actions for musical material kept in the session Mere.
use super::{UiState, WorkspacePanel};
use woodshed_core::captured_arpeggio::CapturedRecipe;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::history::{EngagementKind, ObservationProvenance, catalog_id_for_card};
use woodshed_core::musical_context::{ContextItemId, is_supported_subject};
use woodshed_core::session_overview::{OverviewNodeId, SessionArtifactId};
use woodshed_core::working_sets::WorkingSetId;
use woodshedding::rehearsal::{Card, CardId, Material, Touch};

#[cfg(test)]
#[path = "musical_context/tests.rs"]
mod tests;

fn node_id(id: ContextItemId) -> OverviewNodeId {
    OverviewNodeId::Artifact(SessionArtifactId::ContextItem(id))
}

impl UiState {
    /// Capture an authored arpeggio occurrence with its resolved playable setup.
    pub fn keep_current_arpeggio_nearby(&mut self) -> bool {
        self.keep_current_recipe_nearby(false)
    }

    /// Capture an authored scale-pattern occurrence without changing discovery state.
    pub fn keep_current_scale_pattern_nearby(&mut self) -> bool {
        self.keep_current_recipe_nearby(true)
    }

    fn keep_current_recipe_nearby(&mut self, scale_pattern: bool) -> bool {
        let kind = if scale_pattern {
            "scale pattern"
        } else {
            "arpeggio"
        };
        let Some(card) = self.current_card().filter(|card| {
            if scale_pattern {
                matches!(card.material, Material::ScalePattern { .. })
            } else {
                matches!(card.material, Material::Chord { .. })
                    && matches!(card.touch, Touch::Arpeggiate { .. })
            }
        }) else {
            self.overview_notice = Some(format!("Select an authored {kind} Card to keep nearby."));
            return false;
        };
        let recipe =
            match CapturedRecipe::capture(card, self.current_card_stage(), self.transport.bpm) {
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
                    "Kept this {kind} recipe nearby for {}.",
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
            let (card, _, _, _, _) = match self.captured_context(id) {
                Ok(captured) => captured,
                Err(error) => {
                    self.overview_notice = Some(error);
                    return false;
                },
            };
            let material = match card.material {
                Material::ScalePattern { name, root, .. } => Material::Scale { name, root },
                material => material,
            };
            let Some(subject) = KeyedCatalogRef::from_material(&material) else {
                return false;
            };
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
            self.request_observed_preview(
                (pitches, duration_s, strum_s),
                catalog_id_for_card(&card),
                None,
                ObservationProvenance::capture_in_set(&card, owner).ok(),
            );
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
            let kind = if matches!(card.material, Material::ScalePattern { .. }) {
                "scale pattern"
            } else {
                "arpeggio"
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
                "Added the captured {kind} recipe to {}.",
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
