use super::*;

#[test]
fn chromatic_approach_partners_are_bounded_and_legacy_chord_wire_is_unchanged() {
    assert_eq!(ApproachDirection::Below.partner_fret(2, 2, 8), None);
    assert_eq!(ApproachDirection::Above.partner_fret(8, 2, 8), None);
    assert_eq!(ApproachDirection::Below.partner_fret(5, 2, 8), Some(4));
    assert_eq!(ApproachDirection::Above.partner_fret(5, 2, 8), Some(6));
    assert_eq!(ApproachDirection::Below.partner_fret(9, 2, 8), None);
    assert_eq!(ApproachDirection::Above.partner_fret(1, 2, 8), None);
    assert_eq!(ApproachDirection::Below.partner_fret(255, 0, 254), None);
    let legacy = r#"{"Chord":{"name":"Major","root":0}}"#;
    let chord: Material = serde_json::from_str(legacy).unwrap();
    assert_eq!(serde_json::to_string(&chord).unwrap(), legacy);
    let recipe = Material::ChordApproach {
        name: "Major".into(),
        root: PitchClass::new(0),
        direction: ApproachDirection::Above,
    };
    let reopened: Material =
        serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
    assert!(matches!(
        reopened,
        Material::ChordApproach {
            direction: ApproachDirection::Above,
            ..
        }
    ));
}

fn card(label: &str) -> Card {
    Card {
        id: CardId::UNASSIGNED,
        label: label.into(),
        material: Material::Riff { name: label.into() },
        setting: Setting::default(),
        touch: Touch::default(),
        timing: Timing::default(),
        from: None,
    }
}

fn set_of(labels: &[&str]) -> Set {
    Set::from_cards(labels.iter().map(|l| card(l)), LoopMode::Off)
}

#[test]
fn set_graph_numbers_occurrences_and_exposes_sequence_edges() {
    let set = set_of(&["Shape", "Shape", "Turnaround"]);

    let graph = set.graph();
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.nodes[0].number, 1);
    assert_eq!(graph.nodes[1].number, 2);
    assert_eq!(graph.nodes[0].label, graph.nodes[1].label);
    assert_ne!(
        graph.nodes[0].id, graph.nodes[1].id,
        "the same material staged twice is two occurrences"
    );
    assert_eq!(
        graph.edges,
        vec![
            SetGraphEdge {
                from: graph.nodes[0].id,
                to: graph.nodes[1].id,
                kind: SetGraphEdgeKind::Next,
            },
            SetGraphEdge {
                from: graph.nodes[1].id,
                to: graph.nodes[2].id,
                kind: SetGraphEdgeKind::Next,
            },
        ]
    );
}

#[test]
fn reorder_changes_numbering_without_changing_identity() {
    let mut set = set_of(&["One", "Two", "Three"]);
    let before: Vec<CardId> = set.cards.iter().map(|c| c.id).collect();
    set.cursor = 0;
    let staged = set.cursor_id().unwrap();

    set.move_card(0, 1);

    let graph = set.graph();
    assert_eq!(
        graph.node(staged).unwrap().number,
        2,
        "number follows order"
    );
    assert_eq!(set.cursor_id(), Some(staged), "cursor follows the card");
    let after: Vec<CardId> = set.cards.iter().map(|c| c.id).collect();
    assert_eq!(after, vec![before[1], before[0], before[2]]);
}

#[test]
fn duplicating_mints_a_new_occurrence_and_editing_keeps_one() {
    let mut set = set_of(&["Riff"]);
    let original = set.cards[0].id;

    set.duplicate(0);
    assert_eq!(set.cards.len(), 2);
    assert_eq!(set.cards[0].id, original, "the original is untouched");
    assert_ne!(set.cards[1].id, original, "the copy is its own occurrence");

    set.card_mut(original).unwrap().label = "Riff, slower".into();
    assert_eq!(set.cards[0].id, original, "editing does not re-identify");
}

#[test]
fn removal_drops_only_incident_edges_and_never_reuses_the_id() {
    let mut set = set_of(&["One", "Two", "Three"]);
    let [first, second, third] = [set.cards[0].id, set.cards[1].id, set.cards[2].id];

    set.remove(1);

    let graph = set.graph();
    assert!(graph.node(second).is_none());
    assert_eq!(
        graph.edges,
        vec![SetGraphEdge {
            from: first,
            to: third,
            kind: SetGraphEdgeKind::Next,
        }],
        "the survivors re-derive one Next edge; nothing dangles"
    );

    set.push(card("Four"));
    let minted = set.cards.last().unwrap().id;
    assert_ne!(minted, second, "a removed id is never handed out again");
}

#[test]
fn hiding_a_relation_family_changes_only_the_view() {
    let set = set_of(&["One", "Two"]);
    let hidden = set.graph().with_relations(&[]);
    assert_eq!(hidden.nodes.len(), 2, "nodes remain");
    assert!(hidden.edges.is_empty());
    assert_eq!(set.graph().edges.len(), 1, "Set truth is unchanged");
}

#[test]
fn a_round_trip_preserves_occurrence_identity_and_the_selected_card() {
    let mut set = set_of(&["One", "Two", "Three"]);
    set.cursor = 2;
    let selected = set.cursor_id().unwrap();

    let json = serde_json::to_string(&set).unwrap();
    let back: Set = serde_json::from_str(&json).unwrap();

    assert_eq!(
        back.cards.iter().map(|c| c.id).collect::<Vec<_>>(),
        set.cards.iter().map(|c| c.id).collect::<Vec<_>>()
    );
    assert_eq!(back.cursor_id(), Some(selected));
}

#[test]
fn card_identity_admission_rejects_duplicates_without_mutation_or_copy() {
    let mut source = set_of(&["One", "Two", "Legacy"]);
    source.cards[1].id = source.cards[0].id;
    source.cards[2].id = CardId::UNASSIGNED;
    let before = serde_json::to_value(&source).unwrap();
    assert!(
        source
            .admit_card_ids()
            .unwrap_err()
            .contains("Duplicate Card identity")
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    let mut destination = set_of(&["Existing"]);
    let destination_before = serde_json::to_value(&destination).unwrap();
    assert!(destination.copy_instructions_from(&source).is_err());
    assert_eq!(
        serde_json::to_value(destination).unwrap(),
        destination_before
    );
}

#[test]
fn card_identity_admission_migrates_only_zeros_and_keeps_mint_and_cursor() {
    let mut set = set_of(&["Assigned", "Legacy", "Also legacy"]);
    set.cards[0].id = CardId(40);
    set.cards[1].id = CardId::UNASSIGNED;
    set.cards[2].id = CardId::UNASSIGNED;
    set.last_card_id = 50;
    set.cursor = 2;
    set.admit_card_ids().unwrap();
    assert_eq!(
        set.cards.iter().map(|card| card.id.0).collect::<Vec<_>>(),
        [40, 51, 52]
    );
    assert_eq!(set.cursor_id(), Some(CardId(52)));
    let wire = serde_json::to_value(&set).unwrap();
    set.admit_card_ids().unwrap();
    assert_eq!(serde_json::to_value(&set).unwrap(), wire);
    let mut back: Set = serde_json::from_value(wire).unwrap();
    back.admit_card_ids().unwrap();
    back.push(card("Next"));
    assert_eq!(back.cards.last().unwrap().id, CardId(53));
}

#[test]
fn exhausted_card_mints_are_refused_even_without_legacy_cards() {
    for (mint, ids) in [
        (u64::MAX, vec![]),
        (u64::MAX, vec![1]),
        (0, vec![u64::MAX]),
        (u64::MAX - 1, vec![0]),
        (u64::MAX - 2, vec![0, 0]),
    ] {
        let mut set = Set::default();
        set.last_card_id = mint;
        set.cards = ids
            .into_iter()
            .map(|id| Card {
                id: CardId(id),
                ..card("Input")
            })
            .collect();
        let before = serde_json::to_value(&set).unwrap();
        assert!(set.admit_card_ids().is_err());
        assert_eq!(serde_json::to_value(&set).unwrap(), before);
        set.ensure_card_ids();
        assert_eq!(serde_json::to_value(&set).unwrap(), before);
    }
}

#[test]
fn admitted_near_exhaustion_sets_never_wrap_on_push_duplicate_or_copy() {
    let mut set = set_of(&["Existing"]);
    set.last_card_id = u64::MAX - 2;
    set.admit_card_ids().unwrap();
    set.ensure_card_ids();
    set.push(card("Last available"));
    assert_eq!(set.cards[1].id, CardId(u64::MAX - 1));
    let before = serde_json::to_value(&set).unwrap();
    set.push(card("Refused"));
    set.duplicate(0);
    set.ensure_card_ids();
    assert_eq!(serde_json::to_value(&set).unwrap(), before);
    set.admit_card_ids().unwrap();
    let mut reopened: Set = serde_json::from_value(before).unwrap();
    reopened.admit_card_ids().unwrap();

    let mut legacy = set_of(&["Legacy"]);
    legacy.last_card_id = u64::MAX - 2;
    legacy.cards[0].id = CardId::UNASSIGNED;
    legacy.admit_card_ids().unwrap();
    assert_eq!(legacy.cards[0].id, CardId(u64::MAX - 1));
    legacy.ensure_card_ids();
    legacy.push(card("Refused"));
    assert_eq!(legacy.cards.len(), 1);

    let mut destination = set_of(&["Destination"]);
    destination.last_card_id = u64::MAX - 2;
    destination.admit_card_ids().unwrap();
    let before = serde_json::to_value(&destination).unwrap();
    assert!(
        destination
            .copy_instructions_from(&set_of(&["One", "Two"]))
            .is_err()
    );
    assert_eq!(serde_json::to_value(&destination).unwrap(), before);
    destination.admit_card_ids().unwrap();
}

#[test]
fn exhausted_arrangement_reference_mint_refuses_copy_atomically() {
    let mut source = set_of(&["One"]);
    source.progression_arrangement = Some(arrangement::SavedProgressionArrangement {
        version: 1,
        progression: "Retained".into(),
        roles: vec![],
        root_pc: 0,
        instrument: "guitar".into(),
        tuning: "standard".into(),
        open_midi: vec![],
        fret_start: 0,
        fret_end: 12,
        alignment: arrangement::ProgressionArrangement {
            name: "Retained".into(),
            extent: 1,
            role_spans: vec![],
            cards: vec![arrangement::CardAlignment {
                card: CardId(999),
                placement: None,
                role: None,
            }],
            rests: vec![],
            time_signatures: vec![],
            bar_groups: vec![],
        },
    });
    source.admit_card_ids().unwrap();
    let mut destination = set_of(&["Destination"]);
    destination.last_card_id = u64::MAX - 2;
    destination.admit_card_ids().unwrap();
    let before = serde_json::to_value(&destination).unwrap();
    // The Card fits; the retained missing-reference reservation does not.
    assert!(destination.copy_instructions_from(&source).is_err());
    assert_eq!(serde_json::to_value(&destination).unwrap(), before);
}

#[test]
fn exhausted_legacy_card_migration_is_an_error_without_mutation() {
    let mut set = set_of(&["Assigned", "Legacy"]);
    set.cards[0].id = CardId(u64::MAX);
    set.cards[1].id = CardId::UNASSIGNED;
    let before = serde_json::to_value(&set).unwrap();
    assert!(set.admit_card_ids().is_err());
    assert_eq!(serde_json::to_value(&set).unwrap(), before);
}

#[test]
fn a_legacy_set_gains_ids_at_load_without_colliding() {
    // A session written before occurrence identity existed: no ids, no
    // mint state. Serde fills both with their defaults.
    let legacy = r#"{"cards":[
            {"label":"One","material":{"Riff":{"name":"One"}},"setting":{"instrument":"guitar"},"touch":"Block","timing":{},"from":null},
            {"label":"Two","material":{"Riff":{"name":"Two"}},"setting":{"instrument":"guitar"},"touch":"Block","timing":{},"from":null}
        ],"cursor":1}"#;
    let mut set: Set = serde_json::from_str(legacy).unwrap();
    assert!(set.cards.iter().all(|c| !c.id.is_assigned()));

    set.ensure_card_ids();

    let ids: Vec<CardId> = set.cards.iter().map(|c| c.id).collect();
    assert!(ids.iter().all(|id| id.is_assigned()));
    assert_ne!(ids[0], ids[1]);
    assert_eq!(
        set.cursor_id(),
        Some(ids[1]),
        "the cursor still points at Two"
    );

    set.push(card("Three"));
    assert!(
        !ids.contains(&set.cards[2].id),
        "the mint continues past the migrated ids"
    );

    let before = set.cards.iter().map(|c| c.id).collect::<Vec<_>>();
    set.ensure_card_ids();
    assert_eq!(
        set.cards.iter().map(|c| c.id).collect::<Vec<_>>(),
        before,
        "migration is idempotent"
    );
}
