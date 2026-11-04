use super::*;
use woodshedding::pitch::PitchClass;
use woodshedding::rehearsal::{LoopMode, Material};

fn fixture() -> Set {
    Set::from_cards(
        [
            ("C Major", "Major", 0),
            ("C Major again", "Major", 0),
            ("A Minor", "Minor", 9),
        ]
        .map(|(label, name, root)| {
            let mut card = KeyedCatalogRef {
                formula_id: format!("chord:{name}"),
                root: PitchClass::new(root),
            }
            .to_card()
            .unwrap();
            card.label = label.into();
            card
        }),
        LoopMode::Off,
    )
}

fn read(set: &Set, owner: WorkingSetId) -> Result<SelectedMaterial, DisclosureError> {
    let ids = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
    disclose(owner, "fixture-v1", set, &ids, (ids[0], ids[2]))
}

#[test]
fn passage_compares_selected_authored_neighbors_and_preserves_repeated_occurrences() {
    let set = fixture();
    let ids = set.cards.iter().map(|c| c.id).collect::<Vec<_>>();
    let value =
        passage_projection_json(WorkingSetId(1), "v1", &set, &[ids[2], ids[0], ids[1]]).unwrap();
    let relations = value["relationships"].as_array().unwrap();
    assert_eq!(relations.len(), 4);
    assert!(relations.iter().any(
        |r| r["from_occurrence"] == "set:1:card:1" && r["to_occurrence"] == "set:1:card:2"
    ));
    assert!(relations.iter().any(
        |r| r["from_occurrence"] == "set:1:card:2" && r["to_occurrence"] == "set:1:card:3"
    ));
    assert!(!relations.iter().any(
        |r| r["from_occurrence"] == "set:1:card:1" && r["to_occurrence"] == "set:1:card:3"
    ));
    let sparse = passage_projection_json(WorkingSetId(1), "v1", &set, &[ids[2], ids[0]]).unwrap();
    assert_eq!(
        sparse["dataset"]["occurrences"][1]["values"]["order"]["value"],
        3
    );
    assert_eq!(
        sparse["relationships"][0]["from_occurrence"],
        "set:1:card:1"
    );
    assert_eq!(sparse["relationships"][0]["to_occurrence"], "set:1:card:3");
    assert_eq!(
        passage_projection_json(WorkingSetId(1), "v1", &set, &vec![ids[0]; 65]),
        Err(DisclosureError::SelectionTooLarge)
    );
}

#[test]
fn seventh_chord_comparison_discloses_shared_and_unique_tones_without_voice_assignment() {
    let mut set = fixture();
    set.cards[0].material = Material::Chord {
        name: "Major 7".into(),
        root: PitchClass::new(0),
    };
    set.cards[2].material = Material::Chord {
        name: "Minor 7".into(),
        root: PitchClass::new(9),
    };
    let ids = [set.cards[0].id, set.cards[2].id];
    let facts = disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
    let value = comparison_projection_json(&facts);
    assert_eq!(facts.relationship.shared_pitch_classes, [0, 4, 7]);
    let difference = &value["relationships"][1];
    assert_eq!(difference["kind"], TONE_DIFFERENCES);
    let explanation = difference["explanation"].as_str().unwrap();
    assert!(explanation.contains("Shared: C, E, G."));
    assert!(explanation.contains("Only in C Major: B."));
    assert!(explanation.contains("Only in A Minor: A."));
    assert_eq!(value["relationships"].as_array().unwrap().len(), 2);
}

#[test]
fn scale_contains_chord_with_directed_endpoints_and_exact_extra_tones() {
    let mut set = fixture();
    set.cards[2].material = Material::Scale {
        name: "Major".into(),
        root: PitchClass::new(0),
    };
    set.cards[2].label = "C Major scale".into();
    let ids = [set.cards[0].id, set.cards[2].id];
    let facts = disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
    let value = comparison_projection_json(&facts);
    let contained = &value["relationships"][2];
    assert_eq!(contained["kind"], PITCH_CONTAINMENT);
    assert_eq!(
        contained["from_occurrence"],
        occurrence_id(WorkingSetId(1), ids[1])
    );
    assert_eq!(
        contained["to_occurrence"],
        occurrence_id(WorkingSetId(1), ids[0])
    );
    assert!(
        contained["explanation"]
            .as_str()
            .unwrap()
            .contains("D, F, A, B")
    );
}

#[test]
fn disjoint_comparison_has_differences_but_no_false_overlap_or_containment() {
    let mut set = fixture();
    set.cards[2].material = Material::Chord {
        name: "Major".into(),
        root: PitchClass::new(1),
    };
    let ids = [set.cards[0].id, set.cards[2].id];
    let facts = disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
    let value = comparison_projection_json(&facts);
    assert_eq!(value["relationships"].as_array().unwrap().len(), 1);
    assert_eq!(value["relationships"][0]["kind"], TONE_DIFFERENCES);
    assert!(
        value["relationships"][0]["explanation"]
            .as_str()
            .unwrap()
            .contains("Shared: none.")
    );
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])),
        Err(DisclosureError::NoSharedPitchClasses)
    );
}

#[test]
fn chord_and_arpeggio_equality_keeps_articulation_identity() {
    let mut set = fixture();
    set.cards[1].touch = woodshedding::rehearsal::Touch::Arpeggiate {
        direction: woodshedding::rehearsal::ArpeggioDirection::Up,
        inversion: 0,
    };
    let ids = [set.cards[0].id, set.cards[1].id];
    let facts = disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
    let value = comparison_projection_json(&facts);
    assert_ne!(
        facts.occurrences[0].material.wire_key(),
        facts.occurrences[1].material.wire_key()
    );
    assert_eq!(value["relationships"][1]["kind"], PITCH_EQUALITY);
    assert_eq!(value["relationships"].as_array().unwrap().len(), 2);
}

#[test]
fn repeats_share_material_but_not_occurrence_or_owner_identity() {
    let set = fixture();
    let first = read(&set, WorkingSetId(1)).unwrap();
    let other = read(&set, WorkingSetId(2)).unwrap();
    assert_eq!(first.occurrences[0].material, first.occurrences[1].material);
    assert_ne!(
        first.occurrences[0].occurrence_id,
        first.occurrences[1].occurrence_id
    );
    assert_ne!(
        first.occurrences[0].occurrence_id,
        other.occurrences[0].occurrence_id
    );
    assert_ne!(first.relationship.id, other.relationship.id);
}

#[test]
fn authored_order_survives_reversed_and_sparse_selection() {
    let set = fixture();
    let (a, b) = (set.cards[0].id, set.cards[2].id);
    let read = disclose(WorkingSetId(1), "v1", &set, &[b, a], (a, b)).unwrap();
    assert_eq!(
        read.occurrences
            .iter()
            .map(|item| item.order)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(read.occurrences[0].card, a);
}

#[test]
fn comparison_is_exact_and_does_not_mutate_the_set() {
    let set = fixture();
    let before = serde_json::to_vec(&set).unwrap();
    let read = read(&set, WorkingSetId(1)).unwrap();
    assert_eq!(read.relationship.shared_pitch_classes, [0, 4]);
    assert!(read.relationship.explanation.contains("C, E"));
    assert_eq!(read.relationship.method, COMPARISON_METHOD);
    assert_eq!(serde_json::to_vec(&set).unwrap(), before);
    assert_eq!(read.relationship.from, read.occurrences[0].occurrence_id);
    assert_eq!(read.relationship.to, read.occurrences[2].occurrence_id);
}

#[test]
fn missing_and_unselected_endpoints_are_refused() {
    let set = fixture();
    let ids = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
    assert_eq!(
        disclose(
            WorkingSetId(1),
            "v1",
            &set,
            &[CardId(999)],
            (ids[0], ids[2])
        ),
        Err(DisclosureError::UnknownCard(CardId(999)))
    );
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &[ids[0]], (ids[0], ids[2])),
        Err(DisclosureError::EndpointNotSelected(ids[2]))
    );
    assert_eq!(
        disclose(
            WorkingSetId(1),
            "v1",
            &set,
            &[ids[0], ids[0]],
            (ids[0], ids[0])
        ),
        Err(DisclosureError::DuplicateSelection(ids[0]))
    );
}

#[test]
fn unknown_material_and_missing_revision_are_refused() {
    let mut set = fixture();
    let bad = set.cards[1].id;
    set.cards[1].material = Material::Chord {
        name: "uninstalled formula".into(),
        root: PitchClass::new(0),
    };
    assert_eq!(
        read(&set, WorkingSetId(1)),
        Err(DisclosureError::UnsupportedMaterial(bad))
    );
    set = fixture();
    assert_eq!(
        read(&set, WorkingSetId(0)),
        Err(DisclosureError::InvalidOwner)
    );
    assert_eq!(
        disclose(WorkingSetId(1), " ", &set, &[], (CardId(1), CardId(2))),
        Err(DisclosureError::MissingRevision)
    );
}

#[test]
fn different_roots_have_different_material_identity() {
    let mut set = fixture();
    set.cards[1].material = Material::Chord {
        name: "Major".into(),
        root: PitchClass::new(2),
    };
    let read = read(&set, WorkingSetId(1)).unwrap();
    assert_ne!(
        read.occurrences[0].material.wire_key(),
        read.occurrences[1].material.wire_key()
    );
}

#[test]
fn malformed_occurrence_identity_is_not_disclosed() {
    let mut set = fixture();
    set.cards[1].id = set.cards[0].id;
    let id = set.cards[0].id;
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &[id], (id, id)),
        Err(DisclosureError::InvalidCardIdentity(id))
    );
    set.cards[0].id = CardId::UNASSIGNED;
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &[CardId::UNASSIGNED], (id, id)),
        Err(DisclosureError::InvalidCardIdentity(CardId::UNASSIGNED))
    );
}

#[test]
fn empty_or_false_relationships_are_not_invented() {
    let mut set = fixture();
    let (a, b) = (set.cards[0].id, set.cards[2].id);
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &[], (a, b)),
        Err(DisclosureError::EmptySelection)
    );
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &[a], (a, a)),
        Err(DisclosureError::SameOccurrence)
    );
    set.cards[2].material = Material::Chord {
        name: "Major".into(),
        root: PitchClass::new(1),
    };
    assert_eq!(
        disclose(WorkingSetId(1), "v1", &set, &[a, b], (a, b)),
        Err(DisclosureError::NoSharedPitchClasses)
    );
}
