//! Independently configured instances share one renderer and one explicit audio runner.
use super::*;
use woodshed_core::catalog_explorations::{CatalogExplorationId, CatalogExplorationState};
use woodshed_core::working_sets::WorkingSetId;

#[derive(Default)]
pub struct SetPresentation {
    pub viewport: GraphViewport,
    pub positions: BTreeMap<CardId, (f32, f32)>,
    pub context_positions: BTreeMap<String, (f32, f32)>,
}

impl UiState {
    pub fn capture_exploration(&self) -> CatalogExplorationState {
        let mut state = CatalogExplorationState::capture(&self.stage, &self.app_settings);
        state.search_query = self.search.text().to_string();
        state.context_focus = self.context_focus.clone();
        state
    }
    fn apply_exploration(&mut self, state: &CatalogExplorationState) {
        state.apply(&mut self.stage, &mut self.app_settings);
        self.tuning_dd = SelectState::new(self.stage.tuning_idx);
        self.root_dd = SelectState::new(self.stage.root_idx);
        self.set_arrangement_dd = SelectState::new(self.app_settings.stage.set_arrangement.index())
            .with_label("Set arrangement");
        self.search = TextInput::new(state.search_query.clone());
        self.context_focus = state.context_focus.clone();
        self.pinned_markers.clear();
        self.hover_peek = None;
        self.related_hover = None;
    }
    pub fn create_exploration(&mut self) -> CatalogExplorationId {
        let mut current = self.capture_exploration();
        let fresh = CatalogExplorationState::capture(&StageState::new(), &self.app_settings);
        let id = self
            .catalog_explorations
            .create(&mut current, "", fresh.clone());
        self.clear_occurrence_inspectors();
        self.apply_exploration(&fresh);
        self.activate_workspace_panel(WorkspacePanel::Practice);
        id
    }
    pub fn duplicate_exploration(&mut self) -> CatalogExplorationId {
        let mut current = self.capture_exploration();
        let copy = current.clone();
        let name = format!("{} copy", self.catalog_explorations.active_name);
        let id = self
            .catalog_explorations
            .create(&mut current, name, copy.clone());
        self.clear_occurrence_inspectors();
        self.apply_exploration(&copy);
        self.activate_workspace_panel(WorkspacePanel::Practice);
        id
    }
    pub fn activate_exploration(&mut self, id: CatalogExplorationId) -> bool {
        let mut current = self.capture_exploration();
        if !self.catalog_explorations.activate(id, &mut current) {
            return false;
        }
        self.clear_occurrence_inspectors();
        self.apply_exploration(&current);
        self.activate_workspace_panel(WorkspacePanel::Practice);
        true
    }
    fn park_set_presentation(&mut self) {
        self.set_presentations.insert(
            self.working_sets.active_id,
            SetPresentation {
                viewport: self.set_graph_viewport,
                positions: std::mem::take(&mut self.set_graph_positions),
                context_positions: std::mem::take(&mut self.context_positions),
            },
        );
    }
    fn restore_set_presentation(&mut self) {
        let state = self
            .set_presentations
            .remove(&self.working_sets.active_id)
            .unwrap_or_default();
        self.set_graph_viewport = state.viewport;
        self.set_graph_positions = state.positions;
        self.context_positions = state.context_positions;
        self.clear_occurrence_inspectors();
    }
    pub(super) fn clear_occurrence_inspectors(&mut self) {
        self.card_rename_for = None;
        self.card_shape_notice = None;
        self.set_graph_card_expanded = false;
        self.set_graph_drag_active = false;
        self.set_graph_relation = None;
        self.set_graph_hidden_relations.clear();
        self.context_disclosed.clear();
        self.pitch_motion_anchor = None;
        self.arpeggio_source = None;
        self.discovery_notice = None;
        self.scale_source = None;
        self.scale_subject = None;
        self.scale_notice = None;
        self.scale_limit = 4;
        self.pattern_source = None;
        self.scale_pattern = None;
        self.pattern_subject = None;
        self.pattern_notice = None;
        self.approach_source = None;
        self.approach_target = None;
        self.approach_direction = None;
        self.approach_subject = None;
        self.approach_notice = None;
        self.pinned_markers.clear();
        self.hover_peek = None;
    }
    pub fn create_working_set(&mut self) -> WorkingSetId {
        self.create_working_set_from("", Set::default())
    }
    /// Open supplied instructions under a new owner, leaving the previous Set
    /// and any background runner intact.
    pub fn create_working_set_from(&mut self, name: impl Into<String>, set: Set) -> WorkingSetId {
        self.sync_card_rename();
        self.park_set_presentation();
        let id = self.working_sets.create(&mut self.set, name, set);
        self.restore_set_presentation();
        self.activate_workspace_panel(WorkspacePanel::Set);
        id
    }
    pub fn clear_working_set(&mut self) {
        if self.is_current_set_rehearsing() {
            self.stop_rehearsal();
        }
        self.set.ensure_card_ids();
        self.set.cards.clear();
        self.set.cursor = 0;
        self.set.loop_mode = woodshedding::rehearsal::LoopMode::Off;
        self.set_graph_positions.clear();
        self.context_positions.clear();
        self.clear_occurrence_inspectors();
    }
    pub fn duplicate_working_set(&mut self) -> WorkingSetId {
        self.sync_card_rename();
        self.park_set_presentation();
        let source = self.working_sets.active_id;
        let name = format!("{} copy", self.working_sets.active_name);
        let id = self
            .working_sets
            .duplicate(source, &mut self.set, name)
            .expect("active owner exists");
        self.restore_set_presentation();
        self.activate_workspace_panel(WorkspacePanel::Set);
        id
    }
    pub fn activate_working_set(&mut self, id: WorkingSetId) -> bool {
        if self.working_sets.get(id, &self.set).is_none() {
            return false;
        }
        if id != self.working_sets.active_id {
            self.sync_card_rename();
            self.park_set_presentation();
            self.working_sets.activate(id, &mut self.set);
            self.restore_set_presentation();
        }
        self.activate_workspace_panel(WorkspacePanel::Set);
        true
    }
    pub fn rehearsal_set(&self) -> &Set {
        self.rehearsal_owner
            .and_then(|owner| self.working_sets.get(owner, &self.set))
            .unwrap_or(&self.set)
    }
    pub fn rehearsal_set_mut(&mut self) -> &mut Set {
        let owner = self.rehearsal_owner.unwrap_or(self.working_sets.active_id);
        self.working_sets
            .get_mut(owner, &mut self.set)
            .expect("runner owner remains in working bank")
    }
    pub fn rehearsal_card(&self) -> Option<&Card> {
        let set = self.rehearsal_set();
        set.cards
            .get(set.cursor.min(set.cards.len().saturating_sub(1)))
    }
    pub fn is_current_set_rehearsing(&self) -> bool {
        self.rehearsal_running
            && self.rehearsal_owner.unwrap_or(self.working_sets.active_id)
                == self.working_sets.active_id
    }
    pub fn rehearsal_sounding_pitches(&self) -> Option<(Vec<f32>, f32, f32)> {
        let card = self.rehearsal_card()?;
        Some(
            self.rehearsal_stage
                .as_ref()
                .unwrap_or(&self.stage)
                .card_sounding_pitches_at_tempo(card, self.transport.bpm),
        )
    }
    pub fn stop_rehearsal(&mut self) {
        self.finish_rehearsal_observation(EngagementKind::Rehearsed);
        self.rehearsal_running = false;
        self.rehearsal_owner = None;
        self.rehearsal_stage = None;
        self.rehearsal_observed_midi = None;
    }
    pub fn advance_rehearsal_cursor(&mut self) -> bool {
        if !self.rehearsal_running {
            return false;
        }
        self.complete_rehearsal_cursor();
        let advanced = woodshed_core::step_set(self.rehearsal_set_mut(), 1);
        if advanced {
            self.record_rehearsal_cursor();
        } else {
            self.rehearsal_running = false;
            self.rehearsal_owner = None;
            self.rehearsal_stage = None;
        }
        advanced
    }
    pub(super) fn capture_runner_stage(&mut self) {
        let config = self.capture_exploration();
        let mut state = StageState::new();
        let mut settings = self.app_settings.clone();
        config.apply(&mut state, &mut settings);
        // The runner inherits the rendered neck at Run, including direct host
        // adjustments that have not yet been copied into scoped preferences.
        state.apply_neck(self.stage.fret_start, Some(self.stage.fret_count));
        self.rehearsal_stage = Some(state);
    }
}

impl UiState {
    pub fn select_staging_target(&mut self, id: WorkingSetId) -> bool {
        let section = self.section;
        let panel = self.workspace.active_panel();
        if !self.activate_working_set(id) {
            return false;
        }
        if let Some(panel) = panel {
            self.activate_workspace_panel(panel);
        }
        self.section = section;
        true
    }
}

pub(super) fn staging_targets(ui: &UiState) -> UiChild {
    let choices: Vec<UiChild> = ui
        .working_sets
        .summaries(&ui.set)
        .into_iter()
        .map(|entry| {
            let id = entry.id;
            Box::new(clickable(
                el("div", text(entry.name)).attr(
                    "class",
                    if entry.active {
                        "t-btn staging-target staging-target-active"
                    } else {
                        "t-btn staging-target"
                    },
                ),
                move |ui: &mut UiState, _| {
                    ui.select_staging_target(id);
                },
            )) as UiChild
        })
        .collect();
    Box::new(
        el(
            "div",
            (
                el("div", text(ui.catalog_explorations.active_name.clone()))
                    .attr("class", "exploration-instance-name"),
                el("div", text("Stage into:")).attr("class", "staging-target-label"),
                el("div", choices).attr("class", "staging-target-choices"),
            ),
        )
        .attr("class", "instance-binding"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::catalog_explorations::CatalogExplorationId;
    use woodshed_core::session_overview::{
        OverviewNodeId, OverviewProcess, OverviewRelationKind, SessionArtifactId,
    };

    fn two_cards() -> UiState {
        let mut ui = UiState::new();
        ui.stage_current(None);
        ui.stage_current(None);
        ui
    }
    #[test]
    fn background_runner_keeps_owner_cursor_sound_and_event_snapshot_across_focus() {
        let mut ui = two_cards();
        let owner = ui.working_sets.active_id;
        ui.now_ms = Some(100);
        ui.toggle_rehearsal();
        let sound = ui.rehearsal_sounding_pitches().unwrap();
        ui.set_graph_viewport.pan = (0.2, 0.3);
        let other = ui.create_working_set();
        ui.stage.set_tuning(1);
        ui.stage.set_root(6);
        ui.stage_current(None);
        ui.create_exploration();
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        let history_len = ui.practice_history.len();
        let observed = serde_json::to_value(&ui.rehearsal_observed_card).unwrap();
        ui.now_ms = Some(700);
        ui.set.cards[0].label = "Foreign foreground edit".into();
        ui.sync();
        assert_eq!(ui.card_started_ms, Some(100));
        assert_eq!(ui.practice_history.len(), history_len);
        assert_eq!(
            serde_json::to_value(&ui.rehearsal_observed_card).unwrap(),
            observed
        );
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert_eq!(ui.rehearsal_sounding_pitches().unwrap(), sound);
        assert!(!ui.is_current_set_rehearsing());
        let foreground = serde_json::to_value(&ui.set).unwrap();
        ui.now_ms = Some(1100);
        assert!(ui.advance_rehearsal_cursor());
        assert_eq!(ui.rehearsal_set().cursor, 1);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), foreground);
        let completed = ui
            .practice_history
            .recent(20)
            .into_iter()
            .find(|event| event.kind == EngagementKind::Completed)
            .unwrap();
        assert_eq!(completed.practiced_ms, Some(1000));
        assert_eq!(completed.provenance.unwrap().working_set_id, Some(owner));
        let overview = super::super::overview_snapshot(&ui);
        assert!(overview.relations.iter().any(|edge| edge.from
            == OverviewNodeId::Process(OverviewProcess::Rehearsal)
            && edge.to == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(owner))
            && edge.kind == OverviewRelationKind::ActsOn));
        assert!(ui.activate_working_set(owner));
        assert_eq!(ui.set_graph_viewport.pan, (0.2, 0.3));
        assert!(ui.activate_working_set(other));
        ui.now_ms = Some(1600);
        ui.toggle_rehearsal();
        assert_eq!(ui.rehearsal_owner, Some(other));
        assert!(ui.is_current_set_rehearsing());
        ui.stop_rehearsal();
        assert_eq!(ui.rehearsal_owner, None);
    }
    #[test]
    fn explorers_restore_scoped_configuration_query_and_focus_without_shared_theme() {
        let mut ui = two_cards();
        let first = ui.catalog_explorations.active_id;
        ui.stage.set_root(3);
        ui.root_dd = SelectState::new(3);
        ui.app_settings.fretboard.neck_start = 4;
        ui.app_settings.fretboard.neck_end = Some(14);
        ui.stage.apply_neck(4, Some(14));
        ui.search = TextInput::new("Dorian");
        let focus = KeyedCatalogRef::from_card(&ui.set.cards[0]);
        ui.context_focus = focus.clone();
        let second = ui.create_exploration();
        ui.stage.set_root(6);
        ui.root_dd = SelectState::new(6);
        ui.search = TextInput::new("Major");
        ui.app_settings.appearance.theme = "shared preference".into();
        assert!(ui.activate_exploration(first));
        assert_eq!(ui.stage.root_idx, 3);
        assert_eq!(ui.stage.fret_start, 4);
        assert_eq!(ui.search.text(), "Dorian");
        assert_eq!(ui.context_focus, focus);
        assert_eq!(ui.app_settings.appearance.theme, "shared preference");
        let before = serde_json::to_value(ui.capture_exploration()).unwrap();
        assert!(!ui.activate_exploration(CatalogExplorationId(u64::MAX)));
        assert_eq!(
            serde_json::to_value(ui.capture_exploration()).unwrap(),
            before
        );
        assert!(ui.activate_exploration(second));
        assert_eq!(ui.search.text(), "Major");
        let saved = ui.to_persisted();
        let mut restored = UiState::new();
        restored.apply_persisted(&saved, ui.app_settings.clone());
        assert_eq!(restored.catalog_explorations.active_id, second);
        assert_eq!(restored.search.text(), "Major");
        assert!(restored.activate_exploration(first));
        assert_eq!(restored.search.text(), "Dorian");
        assert!(!restored.rehearsal_running);
        assert_eq!(restored.rehearsal_owner, None);
    }
    #[test]
    fn old_scene_cannot_select_identical_occurrences_in_another_owner() {
        let mut ui = two_cards();
        let old = super::super::set_graph_snapshot(&ui);
        let selected = old.items()[1].0;
        let clone = ui.set.clone();
        ui.working_sets.create(&mut ui.set, "Other", clone);
        ui.set.cursor = 0;
        ui.handle_set_graph_event(&old, GraphCanvasEvent::Activate(selected));
        assert_eq!(ui.set.cursor, 0);
        let current = super::super::set_graph_snapshot(&ui);
        assert_ne!(current.epoch(), old.epoch());
    }
    #[test]
    fn rename_buffer_follows_occurrence_owner_after_removal_and_same_index_replacement() {
        let mut ui = two_cards();
        ui.set.cards[0].label = "First source".into();
        ui.set.cards[1].label = "Surviving target".into();
        ui.sync_card_rename();
        ui.set.remove(0);
        ui.sync_card_rename();
        assert_eq!(ui.set.cards[0].label, "Surviving target");
        assert_eq!(ui.card_rename.text(), "Surviving target");
        ui.card_rename = TextInput::new("Authored rename");
        ui.sync_card_rename();
        assert_eq!(ui.set.cards[0].label, "Authored rename");
        let mut other = ui.set.clone();
        other.cards[0].label = "Another owner's label".into();
        ui.working_sets.create(&mut ui.set, "Other", other);
        ui.sync_card_rename();
        assert_eq!(ui.set.cards[0].label, "Another owner's label");
        assert_eq!(ui.card_rename.text(), "Another owner's label");
    }
    #[test]
    fn opening_recipe_keeps_previous_named_owner_and_background_observation() {
        let mut ui = two_cards();
        let old = ui.working_sets.active_id;
        ui.working_sets.active_name = "Authored passage".into();
        let original = serde_json::to_value(&ui.set).unwrap();
        ui.now_ms = Some(100);
        ui.toggle_rehearsal();
        let history = ui.practice_history.len();
        let recipe = woodshedding::practice::catalog()
            .into_iter()
            .next()
            .unwrap();
        let expected = woodshed_core::set_from_practice(&recipe);
        let name = recipe.name.clone();
        ui.apply_search_hit(SearchHit::Recipe(0));
        let new = ui.working_sets.active_id;
        ui.now_ms = Some(900);
        ui.sync();
        assert_ne!(new, old);
        assert_eq!(ui.working_sets.active_name, name);
        assert_eq!(
            serde_json::to_value(&ui.set).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(
            serde_json::to_value(ui.working_sets.get(old, &ui.set).unwrap()).unwrap(),
            original
        );
        assert_eq!(
            ui.working_sets
                .inactive
                .iter()
                .find(|entry| entry.id == old)
                .unwrap()
                .name,
            "Authored passage"
        );
        assert_eq!(ui.rehearsal_owner, Some(old));
        assert!(ui.rehearsal_running);
        assert_eq!(ui.card_started_ms, Some(100));
        assert_eq!(ui.practice_history.len(), history);
    }
    #[test]
    fn clear_preserves_allocator_and_only_closes_the_active_owner_runner() {
        let mut ui = two_cards();
        let first = ui.working_sets.active_id;
        ui.now_ms = Some(10);
        ui.toggle_rehearsal();
        let second = ui.duplicate_working_set();
        let cleared = ui.set.cards.last().unwrap().id;
        ui.sync_card_rename();
        ui.arpeggio_source = Some(cleared);
        ui.clear_working_set();
        assert!(ui.set.cards.is_empty());
        assert_eq!(ui.working_sets.active_id, second);
        assert_eq!(ui.rehearsal_owner, Some(first));
        assert!(ui.rehearsal_running);
        assert_eq!(ui.arpeggio_source, None);
        assert_eq!(ui.card_rename_for, None);
        ui.stage_current(None);
        ui.sync_card_rename();
        assert!(ui.set.cards[0].id.0 > cleared.0);
        assert_eq!(ui.card_rename.text(), ui.set.cards[0].label);
        ui.now_ms = Some(20);
        ui.toggle_rehearsal();
        assert_eq!(ui.rehearsal_owner, Some(second));
        ui.now_ms = Some(30);
        ui.clear_working_set();
        assert!(!ui.rehearsal_running);
        assert_eq!(ui.rehearsal_owner, None);
        assert_eq!(ui.working_sets.get(first, &ui.set).unwrap().cards.len(), 2);
    }
}
