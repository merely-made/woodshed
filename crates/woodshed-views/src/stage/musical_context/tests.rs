use super::*;
use woodshed_core::audio::AudioRequest;
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
    let request = match restored.audio_requests.last().unwrap() {
        AudioRequest::Audition(request) => request.clone(),
        other => panic!("unexpected audio request: {other:?}"),
    };
    assert_eq!(request.preview, expected);
    assert!(
        restored
            .practice_history
            .recent(20)
            .iter()
            .all(|event| { event.kind != EngagementKind::Previewed })
    );
    restored.finish_audition_admission(&request, Ok(()));
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

fn kept_scale_pattern(pattern: woodshedding::rehearsal::ScalePattern) -> (UiState, ContextItemId) {
    let mut ui = UiState::new();
    let subject = KeyedCatalogRef::from_material(&Material::ScalePattern {
        name: "Major".into(),
        root: PitchClass::new(2),
        pattern,
    })
    .unwrap();
    ui.set.push(subject.to_card().unwrap());
    ui.set.cards[0].timing.bpm = None;
    ui.set.cards[0].setting.instrument.clear();
    ui.set.cards[0].setting.tuning = None;
    ui.set.cards[0].setting.fret_window = None;
    ui.transport.bpm = 79.0;
    let before = serde_json::to_value(&ui.set).unwrap();
    assert!(ui.keep_current_scale_pattern_nearby());
    assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
    let id = ui.musical_context.items()[0].id;
    (ui, id)
}

#[test]
fn captured_scale_pattern_reopens_after_source_deletion_and_ignores_live_discovery() {
    use woodshedding::rehearsal::ScalePattern;
    for pattern in ScalePattern::ALL {
        let (ui, id) = kept_scale_pattern(pattern);
        let recipe = ui
            .musical_context
            .get(id)
            .unwrap()
            .captured_recipe
            .as_ref()
            .unwrap();
        let expected = recipe.preview().unwrap();
        let saved_card = recipe.card().unwrap();
        let source_id = recipe.source_card;
        let source_owner = ui.working_sets.active_id;
        let session =
            serde_json::from_str(&serde_json::to_string(&ui.to_persisted()).unwrap()).unwrap();
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, AppSettings::default());
        reopened.set.cards.clear();
        reopened.stage.set_root(9);
        reopened.stage.set_tuning(1);
        reopened.stage.fret_start = 10;
        reopened.stage.fret_count = 14;
        reopened.transport.bpm = 143.0;
        reopened.pattern_source = Some(CardId(997));
        reopened.scale_pattern = Some(ScalePattern::Fourths);
        reopened.pattern_subject = Some(major(6));
        reopened.pattern_notice = Some("Existing inspection".into());
        let stage = reopened.stage.catalog_id();
        assert!(reopened.hear_musical_context(id));
        let request = match reopened.audio_requests.last().unwrap() {
            AudioRequest::Audition(request) => request.clone(),
            other => panic!("unexpected audio request: {other:?}"),
        };
        assert_eq!(request.preview, expected);
        assert!(
            reopened
                .practice_history
                .recent(20)
                .iter()
                .all(|event| { event.kind != EngagementKind::Previewed })
        );
        reopened.finish_audition_admission(&request, Ok(()));
        let observation = &reopened.practice_history.recent(1)[0];
        assert_eq!(
            observation.provenance.as_ref().unwrap().working_set_id,
            Some(source_owner)
        );
        assert_eq!(
            observation.provenance.as_ref().unwrap().occurrence_id,
            source_id
        );
        assert!(reopened.add_musical_context_to_set(id));
        let mut added = reopened.set.cards.last().unwrap().clone();
        added.id = saved_card.id;
        assert_eq!(
            serde_json::to_value(added).unwrap(),
            serde_json::to_value(saved_card).unwrap()
        );
        assert_eq!(reopened.stage.catalog_id(), stage);
        assert_eq!(reopened.stage.root_idx, 9);
        assert_eq!(reopened.stage.fret_start, 10);
        assert_eq!(reopened.transport.bpm, 143.0);
        assert_eq!(reopened.pattern_source, Some(CardId(997)));
        assert_eq!(reopened.scale_pattern, Some(ScalePattern::Fourths));
        assert_eq!(reopened.pattern_subject, Some(major(6)));
        assert_eq!(
            reopened.pattern_notice.as_deref(),
            Some("Existing inspection")
        );
    }
}

#[test]
fn captured_scale_pattern_add_uses_active_owner_and_explores_base_scale() {
    use woodshedding::rehearsal::ScalePattern;
    let (mut ui, id) = kept_scale_pattern(ScalePattern::Thirds);
    let source_owner = ui.working_sets.active_id;
    let original = serde_json::to_value(&ui.set).unwrap();
    let target = ui.create_working_set();
    assert!(ui.add_musical_context_to_set(id));
    assert!(ui.add_musical_context_to_set(id));
    assert_ne!(ui.set.cards[0].id, ui.set.cards[1].id);
    assert_eq!(ui.musical_context.get(id).unwrap().owner, source_owner);
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
    let before = serde_json::to_value(&ui.set).unwrap();
    assert!(ui.open_musical_context(id));
    assert_eq!(
        ui.context_focus,
        Some(KeyedCatalogRef {
            formula_id: "scale:Major".into(),
            root: PitchClass::new(2)
        })
    );
    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Practice));
    assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
}

#[test]
fn stale_captured_scale_pattern_refuses_actions_and_remains_removable() {
    use woodshedding::rehearsal::ScalePattern;
    let (mut ui, id) = kept_scale_pattern(ScalePattern::Fourths);
    let json = serde_json::to_string(&ui.musical_context)
        .unwrap()
        .replace("Major", "Missing Formula");
    ui.musical_context = serde_json::from_str(&json).unwrap();
    let before = serde_json::to_value(&ui.set).unwrap();
    let stage = ui.stage.catalog_id();
    assert!(!ui.hear_musical_context(id));
    assert!(!ui.add_musical_context_to_set(id));
    assert!(!ui.open_musical_context(id));
    assert!(ui.audio_requests.is_empty());
    assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
    assert_eq!(ui.stage.catalog_id(), stage);
    assert!(ui.remove_musical_context(id));
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
    let request = match ui.audio_requests.last().unwrap() {
        AudioRequest::Audition(request) => request.clone(),
        other => panic!("unexpected audio request: {other:?}"),
    };
    assert_eq!(request.preview, expected);
    ui.finish_audition_admission(&request, Ok(()));
    assert!(ui.add_musical_context_to_set(id));
    let mut added = ui.set.cards.last().unwrap().clone();
    added.id = saved.id;
    assert_eq!(
        serde_json::to_value(added).unwrap(),
        serde_json::to_value(saved).unwrap()
    );
}
