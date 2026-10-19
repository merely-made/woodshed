//! Focused behavioral regressions for exact arrangement observations.
use super::*;
fn fixture() -> UiState {
    let mut ui = UiState::new();
    ui.stage.lens = woodshed_core::Lens::Progressions;
    ui.stage.set_root(
        woodshed_core::ROOT_NAMES
            .iter()
            .position(|s| *s == "C")
            .unwrap(),
    );
    ui.root_dd.selected = ui.stage.root_idx;
    ui.stage.select_progression(
        ui.stage
            .progressions()
            .iter()
            .position(|p| p.name == "ii-V-I (Jazz)")
            .unwrap(),
    );
    for i in 0..3 {
        ui.stage.progression_expand(i);
        let mut c = ui.stage.card_from_lens().unwrap();
        ui.stage.select_card_shape_index(&mut c, 0).unwrap();
        ui.set.push(c);
    }
    ui.set.duplicate(2);
    ui.stage
        .select_card_shape_index(&mut ui.set.cards[3], 1)
        .unwrap();
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    ui.start_arrangement();
    ui
}
fn edit(ui: &mut UiState, key: &str, value: &str) {
    let owner = ui.working_sets.active_id;
    *ui.arrangement_editor.field_mut(owner, key).unwrap() = TextInput::new(value);
    ui.sync_arrangement_scene();
}
fn value(v: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}
fn card_span(ui: &UiState, key: &str) -> Span {
    ui.arrangement_editor.drafts[&ui.working_sets.active_id]
        .occurrence_span(key)
        .unwrap()
}
fn gesture_point(ui: &UiState, key: &str, action: SpanAction) -> ((f32, f32), f32) {
    let surfaces = ui.arrangement_editor.surfaces.borrow();
    let surface = surfaces.primary.as_ref().unwrap();
    let r = surface.occurrence_rect(key, 580, 300).unwrap();
    let x = match action {
        SpanAction::Move => r.x + r.width / 2.0,
        SpanAction::Start => r.x + 1.0,
        SpanAction::End => r.x + r.width - 1.0,
    };
    let a = surface
        .viewport_point(sceno::Vec2::new(48.0, 50.0), 580, 300)
        .unwrap();
    let b = surface
        .viewport_point(sceno::Vec2::new(95.0, 50.0), 580, 300)
        .unwrap();
    ((x, r.y + r.height / 2.0), b.x - a.x)
}
fn pointer(ui: &mut UiState, phase: cambium::PointerPhase, point: (f32, f32)) {
    ui.arrangement_pointer(cambium::PointerEvent::new(phase, point, (580.0, 300.0)));
}

fn saved_fixture() -> UiState {
    let mut ui = fixture();
    ui.place_arrangement_cards_in_order();
    ui.save_arrangement();
    assert!(!ui.arrangement_editor.dirty());
    ui
}
fn document(ui: &UiState) -> SceneDocument {
    ui.arrangement_editor
        .surfaces
        .borrow()
        .document
        .clone()
        .unwrap()
}
fn begin_move(ui: &mut UiState) -> (f32, f32) {
    let (at, beat) = gesture_point(ui, "card-2", SpanAction::Move);
    let target = (at.0 + beat, at.1);
    pointer(ui, cambium::PointerPhase::Down, at);
    pointer(ui, cambium::PointerPhase::Move, target);
    assert_eq!(
        ui.arrangement_editor.gesture.as_ref().unwrap().proposed,
        Span {
            start: 20,
            length: 16
        }
    );
    target
}

#[test]
fn exact_text_including_invalid_input_is_dirty_and_restorable() {
    let mut ui = saved_fixture();
    let owner = ui.working_sets.active_id;
    let saved = value(&ui.set);
    let original = ui.arrangement_editor.drafts[&owner].snapshot();
    let extent = ui.arrangement_editor.drafts[&owner]
        .value("extent")
        .to_owned();
    for input in [
        format!(" {extent}"),
        format!("0{extent}"),
        "-".into(),
        "".into(),
    ] {
        edit(&mut ui, "extent", &input);
        let draft = &ui.arrangement_editor.drafts[&owner];
        assert_eq!(draft.value("extent"), input);
        assert!(!original.matches(draft));
        assert!(draft.dirty());
        assert_eq!(value(&ui.set), saved);
        edit(&mut ui, "extent", &extent);
        assert!(original.matches(&ui.arrangement_editor.drafts[&owner]));
        assert!(!ui.arrangement_editor.dirty());
    }
}

#[test]
fn structural_and_captured_context_changes_are_observed_without_text_edits() {
    for cause in ["card-order", "placed", "rest-order", "context"] {
        let mut ui = saved_fixture();
        ui.add_arrangement_rest();
        ui.add_arrangement_rest();
        ui.save_arrangement();
        let owner = ui.working_sets.active_id;
        let saved = value(&ui.set);
        let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
        let before = draft.snapshot();
        let text: Vec<_> = draft
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), v.text().to_owned()))
            .collect();
        match cause {
            "card-order" => draft.cards.swap(0, 1),
            "placed" => {
                draft.placed.remove(&draft.cards[0]);
            },
            "rest-order" => draft.rests.swap(0, 1),
            "context" => draft.context.root_pc = (draft.context.root_pc + 1) % 12,
            _ => unreachable!(),
        }
        assert_eq!(
            text,
            draft
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.text().to_owned()))
                .collect::<Vec<_>>()
        );
        assert!(!before.matches(draft), "{cause}");
        assert!(draft.dirty(), "{cause}");
        assert_eq!(value(&ui.set), saved, "{cause}");
    }
}

#[test]
fn noop_mutable_field_borrow_preserves_clean_state_and_active_capture() {
    let mut ui = saved_fixture();
    let owner = ui.working_sets.active_id;
    let target = begin_move(&mut ui);
    let preview = document(&ui);
    let revision = ui.arrangement_editor.surfaces.borrow().revision;
    let saved = value(&ui.set);
    assert_eq!(
        ui.arrangement_editor
            .field_mut(owner, "extent")
            .unwrap()
            .text(),
        "64"
    );
    ui.sync_arrangement_scene();
    assert!(ui.arrangement_editor.gesture.is_some());
    assert!(!ui.arrangement_editor.drafts[&owner].dirty());
    assert_eq!(document(&ui), preview);
    assert_eq!(ui.arrangement_editor.surfaces.borrow().revision, revision);
    pointer(&mut ui, cambium::PointerPhase::Up, target);
    assert_eq!(
        card_span(&ui, "card-2"),
        Span {
            start: 20,
            length: 16
        }
    );
    assert_eq!(value(&ui.set), saved);
}

#[test]
fn full_set_label_and_order_changes_reject_capture_even_without_scene_sync() {
    for cause in ["label", "order"] {
        for terminal in ["release", "save"] {
            let mut ui = saved_fixture();
            let owner = ui.working_sets.active_id;
            let target = begin_move(&mut ui);
            let draft = ui.arrangement_editor.drafts[&owner].snapshot();
            match cause {
                "label" => ui.set.cards[0].label.push_str(" externally relabelled"),
                "order" => ui.set.cards.swap(0, 1),
                _ => unreachable!(),
            }
            let external = value(&ui.set);
            if terminal == "release" {
                pointer(&mut ui, cambium::PointerPhase::Up, target);
            } else {
                ui.save_arrangement();
            }
            assert!(
                ui.arrangement_editor.gesture.is_none(),
                "{cause}/{terminal}"
            );
            assert_eq!(
                card_span(&ui, "card-2"),
                Span {
                    start: 16,
                    length: 16
                }
            );
            assert_eq!(ui.arrangement_editor.drafts[&owner].snapshot(), draft);
            assert_eq!(value(&ui.set), external, "{cause}/{terminal}");
            assert!(!ui.arrangement_editor.dirty());
        }
    }
}

#[test]
fn preview_rebuilds_for_theme_and_valid_candidate_without_saving() {
    let mut ui = saved_fixture();
    let saved = value(&ui.set);
    let original = document(&ui);
    let ground = ui.arrangement_editor.surfaces.borrow().theme.ground;
    // Pick a different built-in regardless of the default theme.
    for theme in ["Slate", "Ember"] {
        ui.app_settings.appearance.theme = theme.into();
        ui.app_settings.appearance.theme_choice = None;
        ui.sync_arrangement_scene();
        if ui.arrangement_editor.surfaces.borrow().theme.ground != ground {
            break;
        }
    }
    assert_ne!(ui.arrangement_editor.surfaces.borrow().theme.ground, ground);
    assert_eq!(document(&ui), original);
    assert!(!ui.arrangement_editor.dirty());
    edit(&mut ui, "name", "Exact candidate title");
    assert!(
        document(&ui)
            .primary
            .title
            .contains("Exact candidate title")
    );
    assert_ne!(document(&ui), original);
    assert_eq!(value(&ui.set), saved);
    ui.reload_arrangement();
    assert_eq!(document(&ui), original);
    assert!(!ui.arrangement_editor.dirty());
}

#[test]
fn explicit_save_refuses_external_attachment_conflict_and_reload_adopts_it() {
    let mut ui = saved_fixture();
    let owner = ui.working_sets.active_id;
    edit(&mut ui, "name", "Unsaved local title");
    let target = begin_move(&mut ui);
    let local = ui.arrangement_editor.drafts[&owner].snapshot();
    ui.set
        .progression_arrangement
        .as_mut()
        .unwrap()
        .alignment
        .name = "External saved title".into();
    let external = value(&ui.set);
    // Save itself must detect the conflict, without a preceding refresh.
    ui.save_arrangement();
    assert_eq!(value(&ui.set), external);
    assert_eq!(ui.arrangement_editor.drafts[&owner].snapshot(), local);
    assert_eq!(
        ui.arrangement_editor.drafts[&owner].value("name"),
        "Unsaved local title"
    );
    assert!(
        ui.arrangement_editor
            .notice
            .as_ref()
            .unwrap()
            .contains("Reload")
    );
    assert!(ui.arrangement_editor.dirty());
    assert!(ui.arrangement_editor.gesture.is_none());
    ui.reload_arrangement();
    pointer(&mut ui, cambium::PointerPhase::Up, target);
    assert_eq!(value(&ui.set), external);
    assert_eq!(
        ui.arrangement_editor.drafts[&owner].value("name"),
        "External saved title"
    );
    assert_eq!(
        card_span(&ui, "card-2"),
        Span {
            start: 16,
            length: 16
        }
    );
    assert!(document(&ui).primary.title.contains("External saved title"));
    assert!(!ui.arrangement_editor.dirty());
}

fn selection(ui: &UiState) -> Option<String> {
    ui.arrangement_editor
        .surfaces
        .borrow()
        .primary
        .as_ref()
        .and_then(|s| s.selected())
        .map(str::to_owned)
}

#[test]
fn same_owner_save_and_reload_preserve_distinct_voicing_selection() {
    let mut ui = saved_fixture();
    ui.arrangement_select(BackdropSceneEvent::Select("card-3".into()));
    let third = document(&ui)
        .for_occurrence(Some("card-3"))
        .unwrap()
        .scene
        .clone();
    ui.arrangement_select(BackdropSceneEvent::Select("card-4".into()));
    let fourth = document(&ui)
        .for_occurrence(Some("card-4"))
        .unwrap()
        .scene
        .clone();
    assert_ne!(third, fourth);
    edit(&mut ui, "name", "Selected fourth tonic");
    ui.save_arrangement();
    for reload in [false, true] {
        if reload {
            ui.reload_arrangement();
        }
        let surfaces = ui.arrangement_editor.surfaces.borrow();
        assert_eq!(
            surfaces.primary.as_ref().unwrap().selected(),
            Some("card-4")
        );
        assert!(surfaces.related.is_some());
        assert_eq!(
            surfaces
                .document
                .as_ref()
                .unwrap()
                .for_occurrence(Some("card-4"))
                .unwrap()
                .scene,
            fourth
        );
        assert!(!ui.arrangement_editor.dirty());
    }
    // Equal data in another working owner must not inherit presentation state.
    let other = ui.set.clone();
    ui.working_sets
        .create(&mut ui.set, "Same material, other owner", other);
    ui.sync_arrangement_scene();
    assert_eq!(selection(&ui), None);
    assert!(ui.arrangement_editor.surfaces.borrow().related.is_none());
}

#[test]
fn removed_occurrence_is_not_retained_as_selection_after_save_or_reload() {
    let mut ui = saved_fixture();
    ui.add_arrangement_rest();
    ui.save_arrangement();
    ui.arrangement_select(BackdropSceneEvent::Select("rest-rest-1".into()));
    assert_eq!(selection(&ui).as_deref(), Some("rest-rest-1"));
    let owner = ui.working_sets.active_id;
    ui.arrangement_editor
        .drafts
        .get_mut(&owner)
        .unwrap()
        .rests
        .clear();
    ui.save_arrangement();
    assert_eq!(selection(&ui), None);
    assert!(ui.arrangement_editor.surfaces.borrow().related.is_none());
    assert!(document(&ui).for_occurrence(Some("rest-rest-1")).is_none());
    ui.reload_arrangement();
    assert_eq!(selection(&ui), None);
    assert!(ui.arrangement_editor.surfaces.borrow().related.is_none());
}

#[test]
fn value_restoration_preserves_capture_only_when_intermediate_change_was_unobserved() {
    for observed in [false, true] {
        let mut ui = saved_fixture();
        let saved = value(&ui.set);
        let target = begin_move(&mut ui);
        let label = ui.set.cards[0].label.clone();
        ui.set.cards[0].label.push_str(" intermediate B");
        if observed {
            ui.sync_arrangement_scene();
            assert!(ui.arrangement_editor.gesture.is_none());
        }
        ui.set.cards[0].label = label;
        ui.sync_arrangement_scene();
        assert_eq!(ui.arrangement_editor.gesture.is_some(), !observed);
        pointer(&mut ui, cambium::PointerPhase::Up, target);
        assert_eq!(
            card_span(&ui, "card-2"),
            Span {
                start: if observed { 16 } else { 20 },
                length: 16
            }
        );
        assert_eq!(ui.arrangement_editor.dirty(), !observed);
        assert_eq!(value(&ui.set), saved);
    }
}

#[test]
fn identical_set_in_different_owner_cannot_accept_previous_capture() {
    let mut ui = saved_fixture();
    let first = ui.working_sets.active_id;
    let target = begin_move(&mut ui);
    let original = ui.arrangement_editor.drafts[&first].snapshot();
    let saved = value(&ui.set);
    let same = ui.set.clone();
    let second = ui
        .working_sets
        .create(&mut ui.set, "Identical second Set", same);
    assert_ne!(first, second);
    assert_eq!(value(&ui.set), saved);
    // No intervening sync: terminal delivery must check owner itself.
    pointer(&mut ui, cambium::PointerPhase::Up, target);
    assert!(ui.arrangement_editor.gesture.is_none());
    assert_eq!(ui.arrangement_editor.drafts[&first].snapshot(), original);
    assert_eq!(value(&ui.set), saved);
    assert_eq!(
        card_span(&ui, "card-2"),
        Span {
            start: 16,
            length: 16
        }
    );
    assert!(!ui.arrangement_editor.dirty());
}
