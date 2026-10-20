//! Owner-scoped source-return regressions over real catalog material.
use super::*;
use serde_json::{Value, json};
use woodshed_core::harmony::KeyedCatalogRef;
use woodshedding::pitch::PitchClass;

fn fixture(version: u32) -> UiState {
    let mut ui = UiState::new();
    for (formula, tonic) in [("chord:Major", 0), ("chord:Major", 0), ("chord:Minor", 9)] {
        ui.set.push(
            KeyedCatalogRef {
                formula_id: formula.into(),
                root: PitchClass::new(tonic),
            }
            .to_card()
            .unwrap(),
        );
    }
    ui.open_relationship_recipe();
    // Keep an unselected Card so currency covers more than the chosen pair.
    ui.relationship_cards.truncate(2);
    ui.bind_relationship_recipe(false).unwrap();
    let mut reading = ui.relationship_reading().unwrap().unwrap();
    let source = reading.source.as_ref().unwrap();
    let facts = comparison_disclosure::disclose_comparison(
        source.owner,
        &source.revision,
        &ui.set,
        &source.cards,
        source.pair,
    )
    .unwrap();
    reading.version = version;
    reading.dataset = serde_json::from_value(match version {
        0 | 1 => comparison_disclosure::projection_json(&facts),
        2 => comparison_disclosure::comparison_projection_json(&facts),
        _ => comparison_disclosure::passage_projection_json(
            source.owner,
            &source.revision,
            &ui.set,
            &source.cards,
        )
        .unwrap(),
    })
    .unwrap();
    reading.snapshot.selected_occurrence = Some(comparison_disclosure::occurrence_id(
        source.owner,
        CardId(2),
    ));
    reading.snapshot.selected_relationship = None;
    ui.install_relationship(reading).unwrap();
    ui.relationship_label = cambium::TextInput::new("Pending label, not installed");
    ui
}

fn member_target(ui: &UiState) -> cambium::LinkMemberTarget {
    let reading = ui.relationship_reading().unwrap().unwrap();
    let key = relationship_link_keys(&reading).into_iter().next().unwrap();
    let pair = relationship_pair_reading(&reading, &key).unwrap();
    let bucket = &pair.links[0].buckets[0];
    cambium::LinkMemberTarget {
        source: pair.source.clone(),
        link: key,
        from: bucket.source_from.id.clone(),
        to: bucket.source_to.id.clone(),
        assertion_id: bucket.assertions[0].id.clone(),
    }
}
fn invoke(
    ui: &mut UiState,
    target: Option<&cambium::LinkMemberTarget>,
    action: &str,
) -> Result<(), String> {
    match target {
        Some(target) => open_relationship_member(ui, target, action),
        None => ui.open_relationship_source(),
    }
}
fn replace_raw(ui: &mut UiState, edit: impl FnOnce(&mut Value)) {
    let mut raw: Value =
        serde_json::from_str(ui.relationship_reading_json.as_deref().unwrap()).unwrap();
    edit(&mut raw);
    ui.relationship_reading_json = Some(serde_json::to_string(&raw).unwrap());
}
fn snapshot(ui: &UiState) -> Value {
    let selection = ui.relationship_reading().ok().flatten().map(|r| json!({
        "occurrence":r.snapshot.selected_occurrence,"relationship":r.snapshot.selected_relationship}));
    json!({"reading_json":ui.relationship_reading_json,"selection":selection,
        "relationship_label":ui.relationship_label.text(),"relationship_notice":ui.relationship_notice,
        "relationship_open":ui.relationship_open,"expanded":ui.set_graph_card_expanded,
        "active_set":ui.set,"working_sets":ui.working_sets,
        "choice_owner":ui.relationship_choice_owner,"choice_cards":ui.relationship_cards,
        "runner_owner":ui.rehearsal_owner,"runner_running":ui.rehearsal_running,
        "runner_pending":ui.rehearsal_pending,"runner_paused":ui.rehearsal_paused,
        "runner_epoch":ui.rehearsal_epoch,"runner_observed_midi":ui.rehearsal_observed_midi,
        "loop_captures":ui.loop_captures})
}
fn inactive_source(ui: &mut UiState) -> WorkingSetId {
    let copy = ui.set.clone();
    let other = ui
        .working_sets
        .create(&mut ui.set, "Other owner with same Card IDs", copy);
    ui.set.cards[1].label = "Other owner's second Card".into();
    ui.rehearsal_owner = Some(other);
    ui.rehearsal_running = true;
    other
}

#[test]
fn direct_and_member_return_to_exact_inactive_owner_without_disturbing_runner() {
    for version in 0..=3 {
        for member in [false, true] {
            let mut ui = fixture(version);
            let owner = ui.working_sets.active_id;
            let target = member_target(&ui);
            let expected = if member {
                target.from.clone()
            } else {
                comparison_disclosure::occurrence_id(owner, CardId(2))
            };
            let original_cards = serde_json::to_value(&ui.set.cards).unwrap();
            let other = inactive_source(&mut ui);
            let background = serde_json::to_value(&ui.set).unwrap();
            let captures = serde_json::to_value(&ui.loop_captures).unwrap();
            invoke(&mut ui, member.then_some(&target), "open-source-card").unwrap();
            assert_eq!(ui.working_sets.active_id, owner);
            assert_eq!(
                comparison_disclosure::occurrence_id(owner, ui.set.cursor_id().unwrap()),
                expected
            );
            assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), original_cards);
            assert_eq!(
                serde_json::to_value(ui.working_sets.get(other, &ui.set).unwrap()).unwrap(),
                background
            );
            assert_eq!(ui.rehearsal_owner, Some(other));
            assert!(ui.rehearsal_running);
            assert_eq!(serde_json::to_value(&ui.loop_captures).unwrap(), captures);
            assert!(ui.set_graph_card_expanded);
            assert!(!ui.relationship_open);
        }
    }
}

#[test]
fn unselected_instruction_changes_refuse_early_but_cursor_navigation_is_current() {
    for member in [false, true] {
        let mut ui = fixture(3);
        let target = member_target(&ui);
        ui.set.cards[2].timing.bpm = Some(103.0);
        let before = snapshot(&ui);
        let error = invoke(&mut ui, member.then_some(&target), "open-source-card").unwrap_err();
        assert!(error.starts_with("Source instructions changed."));
        assert_eq!(snapshot(&ui), before);
        let mut ui = fixture(3);
        let target = member_target(&ui);
        ui.set.cursor = 2;
        invoke(&mut ui, member.then_some(&target), "open-source-card").unwrap();
        assert_ne!(ui.set.cursor, 2);
    }
}

#[test]
fn member_installs_exact_selection_before_late_dataset_refusal() {
    let mut ui = fixture(3);
    replace_raw(&mut ui, |r| {
        r["dataset"]["relationships"][0]["explanation"] = json!("Caller altered disclosure")
    });
    let target = member_target(&ui);
    let before = snapshot(&ui);
    let error = open_relationship_member(&mut ui, &target, "open-source-card").unwrap_err();
    assert_eq!(error, "Retained disclosure does not match the source Set.");
    let after = snapshot(&ui);
    assert_ne!(after["reading_json"], before["reading_json"]);
    assert_eq!(after["selection"]["occurrence"], target.from);
    assert_eq!(after["selection"]["relationship"], target.assertion_id);
    assert_eq!(
        after["relationship_label"],
        "Selected material relationships"
    );
    for key in [
        "active_set",
        "working_sets",
        "runner_owner",
        "runner_running",
        "loop_captures",
        "relationship_open",
        "expanded",
    ] {
        assert_eq!(after[key], before[key], "{key}");
    }
}

#[test]
fn invalid_retained_bytes_and_wrong_member_action_are_nonwriting() {
    for bytes in [
        "{invalid json".to_owned(),
        " ".repeat(MAX_READING_BYTES + 1),
    ] {
        let mut ui = fixture(3);
        ui.relationship_reading_json = Some(bytes.clone());
        let before = snapshot(&ui);
        assert!(ui.open_relationship_source().is_err());
        assert_eq!(snapshot(&ui), before);
        assert_eq!(
            ui.relationship_reading_json.as_deref(),
            Some(bytes.as_str())
        );
    }
    let mut ui = fixture(3);
    let target = member_target(&ui);
    let before = snapshot(&ui);
    assert_eq!(
        open_relationship_member(&mut ui, &target, "unavailable").unwrap_err(),
        "This relationship member action is not available."
    );
    assert_eq!(snapshot(&ui), before);
}
