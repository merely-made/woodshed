use super::*;

#[test]
fn related_transitions_ignore_exploration_events() {
    let mut history = PracticeHistory::default();
    history.record(
        Some(1_000),
        "scale:Dorian",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    history.record(
        Some(2_000),
        "chord:Minor",
        EngagementKind::Previewed,
        Some("scale:Dorian".into()),
        None,
    );
    assert_eq!(
        history.related_transition_count("scale:Dorian", "chord:Minor"),
        0,
        "an interested glance is not a practised transition"
    );
    history.record(
        Some(3_000),
        "scale:Dorian",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    history.record(
        Some(4_000),
        "chord:Minor",
        EngagementKind::Staged,
        Some("scale:Dorian".into()),
        None,
    );
    assert_eq!(
        history.related_transition_count("scale:Dorian", "chord:Minor"),
        0,
        "staging is still interest rather than practice"
    );
    history.record(
        Some(5_000),
        "chord:Minor",
        EngagementKind::Rehearsed,
        Some("scale:Dorian".into()),
        None,
    );
    assert_eq!(
        history.related_transition_count("scale:Dorian", "chord:Minor"),
        1
    );
}

#[test]
fn staging_only_and_mixed_paths_do_not_claim_practiced_transitions() {
    let mut history = PracticeHistory::default();
    history.record(
        Some(1),
        "chord:Major 7",
        EngagementKind::Staged,
        None,
        Some(999),
    );
    history.record(
        Some(2),
        "arpeggio:Major 7",
        EngagementKind::Staged,
        Some("chord:Major 7".into()),
        None,
    );
    assert!(history.transitions().is_empty());
    assert_eq!(history.total_practiced_ms("chord:Major 7"), 0);
    assert_eq!(history.engagement_count("chord:Major 7"), 1);
    history.record(
        Some(3),
        "chord:Major 7",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    history.record(
        Some(4),
        "scale:Major",
        EngagementKind::Previewed,
        None,
        None,
    );
    history.record(
        Some(5),
        "arpeggio:Major 7",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    assert!(
        history.transitions().is_empty(),
        "an intervening exploration is not a practice move"
    );
    history.record(
        Some(0),
        "chord:Major 7",
        EngagementKind::Completed,
        None,
        Some(500),
    );
    assert_eq!(
        history.traversals("arpeggio:Major 7", "chord:Major 7"),
        (1, Some(0)),
        "a genuine epoch timestamp stays dated"
    );
    let back: PracticeHistory =
        serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
    assert_eq!(back.recent(back.len()), history.recent(history.len()));
    assert_eq!(back.transitions(), history.transitions());
}

#[test]
fn occurrence_lifecycle_is_not_a_move_but_repeated_material_occurrences_are() {
    use woodshedding::rehearsal::{Setting, Timing};
    let mut card = Card {
        id: CardId(1),
        label: "Cmaj7".into(),
        material: Material::Chord {
            name: "Major 7".into(),
            root: woodshedding::pitch::PitchClass::new(0),
        },
        setting: Setting::default(),
        touch: Touch::Block,
        timing: Timing::default(),
        from: None,
    };
    let mut history = PracticeHistory::default();
    let first = ObservationProvenance::capture(&card).unwrap();
    history.record_observation(
        Some(1),
        "chord:Major 7",
        EngagementKind::Rehearsed,
        None,
        None,
        Some(first.clone()),
    );
    history.record_observation(
        Some(2),
        "chord:Major 7",
        EngagementKind::Completed,
        None,
        Some(1_000),
        Some(first),
    );
    assert!(
        history.transitions().is_empty(),
        "completion is not a move to another occurrence"
    );
    card.id = CardId(2);
    let second = ObservationProvenance::capture(&card).unwrap();
    history.record_observation(
        Some(3),
        "chord:Major 7",
        EngagementKind::Rehearsed,
        None,
        None,
        Some(second.clone()),
    );
    history.record_observation(
        Some(4),
        "chord:Major 7",
        EngagementKind::Completed,
        None,
        Some(1_000),
        Some(second),
    );
    assert_eq!(
        history.traversals("chord:Major 7", "chord:Major 7"),
        (1, Some(3)),
        "a second occurrence of the same formula is a distinct next instruction"
    );
    assert_eq!(history.len(), 4, "all raw lifecycle observations remain");
    assert_eq!(history.total_practiced_ms("chord:Major 7"), 2_000);
    let back: PracticeHistory =
        serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
    assert_eq!(back.transitions(), history.transitions());
}

#[test]
fn equal_local_occurrences_in_different_sets_are_a_real_transition() {
    use crate::working_sets::WorkingSetId;
    let mut set = woodshedding::rehearsal::Set::default();
    set.push(crate::StageState::new().card_from_lens().unwrap());
    let card = &set.cards[0];
    let mut history = PracticeHistory::default();
    for owner in [WorkingSetId(1), WorkingSetId(2)] {
        history.record_observation(
            Some(owner.0),
            "scale:Major",
            EngagementKind::Completed,
            None,
            Some(100),
            Some(ObservationProvenance::capture_in_set(card, owner).unwrap()),
        );
    }
    assert_eq!(history.transitions().len(), 1);
    assert_eq!(history.transitions()[0].traversal_count, 1);
    let mut legacy =
        serde_json::to_value(ObservationProvenance::capture_in_set(card, WorkingSetId(1)).unwrap())
            .unwrap();
    legacy.as_object_mut().unwrap().remove("working_set_id");
    let legacy: ObservationProvenance = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.working_set_id, None);
    let mut same_owner = PracticeHistory::default();
    for _ in 0..2 {
        same_owner.record_observation(
            None,
            "scale:Major",
            EngagementKind::Completed,
            None,
            None,
            Some(legacy.clone()),
        );
    }
    assert!(same_owner.transitions().is_empty());
}

#[test]
fn event_time_occurrences_and_instruction_snapshots_survive_edits_and_reopen() {
    use woodshedding::rehearsal::{ArpeggioDirection, Setting, Timing};
    let mut card = Card {
        id: CardId(1),
        label: "Cmaj7".into(),
        material: Material::Chord {
            name: "Major 7".into(),
            root: woodshedding::pitch::PitchClass::new(0),
        },
        setting: Setting::default(),
        touch: Touch::Block,
        timing: Timing::default(),
        from: None,
    };
    let mut history = PracticeHistory::default();
    let mut first = ObservationProvenance::capture(&card).unwrap();
    first.run_id = Some("run-a".into());
    first.presented_midi = Some(vec![60, 64, 67, 71]);
    history.record_observation(
        Some(1),
        "chord:Major 7",
        EngagementKind::Rehearsed,
        None,
        None,
        Some(first.clone()),
    );
    card.id = CardId(2);
    card.touch = Touch::Arpeggiate {
        direction: ArpeggioDirection::Down,
        inversion: 0,
    };
    let second = ObservationProvenance::capture(&card).unwrap();
    history.record_observation(
        Some(2),
        "arpeggio:Major 7",
        EngagementKind::Completed,
        None,
        Some(400),
        Some(second.clone()),
    );
    card.label = "edited after observation".into();
    let back: PracticeHistory =
        serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
    let recent = back.recent(2);
    assert_eq!(recent[0].provenance.as_ref(), Some(&second));
    assert_eq!(recent[1].provenance.as_ref(), Some(&first));
    assert_eq!(
        recent[1].provenance.as_ref().unwrap().card_snapshot["label"],
        "Cmaj7"
    );
    assert_eq!(
        recent[1].provenance.as_ref().unwrap().presented_midi,
        Some(vec![60, 64, 67, 71])
    );
}

#[test]
fn an_engagement_keeps_the_time_the_host_supplied() {
    let mut history = PracticeHistory::default();
    history.record(
        Some(1_700_000_000_000),
        "scale:Major",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    assert_eq!(history.last_seen_ms("scale:Major"), Some(1_700_000_000_000));
    assert!(history.has_times());
    assert_eq!(history.engagement_count("scale:Major"), 1);
}

#[test]
fn measured_practice_outweighs_a_pile_of_previews() {
    let mut history = PracticeHistory::default();
    history.record(
        Some(10_000),
        "scale:Dorian",
        EngagementKind::Completed,
        None,
        Some(540_000),
    );
    for at in [1_u64, 2, 3, 4, 5] {
        history.record(
            Some(at),
            "scale:Lydian",
            EngagementKind::Previewed,
            None,
            None,
        );
    }
    assert_eq!(history.total_practiced_ms("scale:Dorian"), 540_000);
    assert_eq!(
        history.total_practiced_ms("scale:Lydian"),
        0,
        "an interested glance is not practice time"
    );
    assert_eq!(
        history.engagement_count("scale:Lydian"),
        5,
        "frequency is still counted, and the lineage keeps it"
    );
}

#[test]
fn practice_spans_accumulate_across_sessions() {
    let mut history = PracticeHistory::default();
    history.record(
        Some(10),
        "chord:Major 7",
        EngagementKind::Completed,
        None,
        Some(60_000),
    );
    history.record(
        Some(20),
        "chord:Major 7",
        EngagementKind::Completed,
        None,
        Some(90_000),
    );
    assert_eq!(history.total_practiced_ms("chord:Major 7"), 150_000);
    assert_eq!(history.last_seen_ms("chord:Major 7"), Some(20));
    assert_eq!(history.engagement_count("chord:Major 7"), 2);
}

#[test]
fn recent_reads_newest_first_and_reports_the_stated_reason() {
    let mut history = PracticeHistory::default();
    history.record(
        Some(1),
        "scale:Major",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    history.record(
        Some(2),
        "chord:Major 7",
        EngagementKind::Staged,
        Some("scale:Major".into()),
        None,
    );
    let recent = history.recent(2);
    assert_eq!(recent[0].subject_id, "chord:Major 7");
    assert_eq!(recent[0].from_id.as_deref(), Some("scale:Major"));
    assert_eq!(recent[1].subject_id, "scale:Major");
    assert_eq!(recent[1].from_id, None, "nothing was named for the first");
}

#[test]
fn the_stated_reason_and_the_walked_path_are_separate_facts() {
    let mut history = PracticeHistory::default();
    // Practice moved Major -> Minor 7 along the path, but the player named
    // Dorian as the reason for the second one (a suggestion taken from a
    // frontier while the focus sat elsewhere).
    history.record(
        Some(1),
        "scale:Major",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    history.record(
        Some(2),
        "chord:Minor 7",
        EngagementKind::Rehearsed,
        Some("scale:Dorian".into()),
        None,
    );

    // The stated reason ranks the suggestion...
    assert_eq!(
        history.related_transition_count("scale:Dorian", "chord:Minor 7"),
        1
    );
    // ...and is not confused with a traversal that never happened.
    assert_eq!(
        history.traversals("scale:Dorian", "chord:Minor 7"),
        (0, None)
    );
    // The walked path is its own fact, with its own recency.
    assert_eq!(
        history.traversals("scale:Major", "chord:Minor 7"),
        (1, Some(2))
    );
    assert_eq!(
        history.related_transition_count("scale:Major", "chord:Minor 7"),
        0,
        "the player did not name Major as the reason"
    );
}

#[path = "tests/wire.rs"]
mod wire;
