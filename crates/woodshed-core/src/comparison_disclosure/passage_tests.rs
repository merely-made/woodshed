//! Public-boundary regressions captured before selection/projection consolidation.
use super::*;
use serde_json::{Value, json};
use woodshedding::pitch::PitchClass;
use woodshedding::rehearsal::{LoopMode, Material};

fn fixture(count: usize) -> Set {
    Set::from_cards(
        (0..count).map(|index| {
            let mut card = KeyedCatalogRef {
                formula_id: "chord:Major".into(),
                root: PitchClass::new(0),
            }
            .to_card()
            .expect("real C Major catalog fixture");
            card.label = format!("C Major occurrence {}", index + 1);
            card
        }),
        LoopMode::Off,
    )
}

fn ids(set: &Set) -> Vec<CardId> {
    set.cards.iter().map(|card| card.id).collect()
}

#[test]
fn unsupported_selected_tail_refuses_the_entire_passage_without_source_changes() {
    let mut set = fixture(4);
    let selected = ids(&set);
    set.cards[3].material = Material::Chord {
        name: "not-an-installed-formula".into(),
        root: PitchClass::new(0),
    };
    let before = serde_json::to_vec(&set).unwrap();
    assert_eq!(
        passage_projection_json(WorkingSetId(7), "captured", &set, &selected),
        Err(DisclosureError::UnsupportedMaterial(selected[3]))
    );
    // A malformed unselected Card does not invalidate disclosed selected material.
    assert!(passage_projection_json(WorkingSetId(7), "captured", &set, &selected[..3]).is_ok());
    assert_eq!(serde_json::to_vec(&set).unwrap(), before);
}

#[test]
fn legacy_overlap_refuses_disjoint_pair_while_rich_pair_and_passage_disclose_differences() {
    let mut set = fixture(2);
    set.cards[1].material = Material::Chord {
        name: "Major".into(),
        root: PitchClass::new(1),
    };
    let selected = ids(&set);
    let pair = (selected[0], selected[1]);
    let before = serde_json::to_vec(&set).unwrap();
    assert_eq!(
        disclose(WorkingSetId(7), "captured", &set, &selected, pair),
        Err(DisclosureError::NoSharedPitchClasses)
    );
    let facts = disclose_comparison(WorkingSetId(7), "captured", &set, &selected, pair).unwrap();
    let rich = comparison_projection_json(&facts);
    let passage = passage_projection_json(WorkingSetId(7), "captured", &set, &selected).unwrap();
    for value in [&rich, &passage] {
        let relations = value["relationships"].as_array().unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0]["kind"], TONE_DIFFERENCES);
        assert!(
            relations[0]["explanation"]
                .as_str()
                .unwrap()
                .contains("Shared: none.")
        );
    }
    assert_eq!(rich["dataset"], passage["dataset"]);
    assert_eq!(serde_json::to_vec(&set).unwrap(), before);
}

#[test]
fn passage_identity_errors_preserve_the_existing_validation_precedence() {
    let set = fixture(3);
    let selected = ids(&set);
    let cases = [
        (
            vec![selected[0], selected[1], selected[0]],
            DisclosureError::DuplicateSelection(selected[0]),
        ),
        (
            vec![selected[0], selected[1], CardId(999)],
            DisclosureError::UnknownCard(CardId(999)),
        ),
        (
            vec![selected[0], selected[1], CardId::UNASSIGNED],
            DisclosureError::InvalidCardIdentity(CardId::UNASSIGNED),
        ),
        // Passage needs two existing occurrences before full identity validation.
        (
            vec![selected[0], CardId(999)],
            DisclosureError::EmptySelection,
        ),
        (
            vec![CardId(998), CardId(999)],
            DisclosureError::EmptySelection,
        ),
        (
            vec![selected[0], selected[0]],
            DisclosureError::EmptySelection,
        ),
    ];
    for (selection, expected) in cases {
        assert_eq!(
            passage_projection_json(WorkingSetId(7), "captured", &set, &selection),
            Err(expected),
            "selection={selection:?}"
        );
    }
    let mut duplicate = set.clone();
    duplicate.cards[2].id = selected[0];
    assert_eq!(
        passage_projection_json(WorkingSetId(7), "captured", &duplicate, &selected[..2]),
        Err(DisclosureError::InvalidCardIdentity(selected[0]))
    );
}

#[test]
fn passage_bound_precedes_owner_revision_and_identity_but_pair_api_has_no_passage_cap() {
    let mut set = fixture(65);
    let selected = ids(&set);
    set.cards[64].material = Material::Chord {
        name: "not-an-installed-formula".into(),
        root: PitchClass::new(0),
    };
    assert_eq!(
        passage_projection_json(WorkingSetId(0), " ", &set, &selected),
        Err(DisclosureError::SelectionTooLarge)
    );
    assert_eq!(
        passage_projection_json(WorkingSetId(0), " ", &set, &[]),
        Err(DisclosureError::EmptySelection)
    );
    assert_eq!(
        passage_projection_json(WorkingSetId(0), " ", &set, &selected[..2]),
        Err(DisclosureError::InvalidOwner)
    );
    assert_eq!(
        passage_projection_json(
            WorkingSetId(7),
            " ",
            &set,
            &[selected[0], selected[1], selected[0]]
        ),
        Err(DisclosureError::MissingRevision)
    );
    let accepted =
        passage_projection_json(WorkingSetId(7), "captured", &set, &selected[..64]).unwrap();
    assert_eq!(
        accepted["dataset"]["occurrences"].as_array().unwrap().len(),
        64
    );
    assert_eq!(accepted["relationships"].as_array().unwrap().len(), 126);
    // The public pair API accepts >64 selected Cards; keep the passage cap local.
    set.cards[64].material = set.cards[0].material.clone();
    assert!(
        disclose_comparison(
            WorkingSetId(7),
            "captured",
            &set,
            &selected,
            (selected[0], selected[64])
        )
        .is_ok()
    );
}

#[test]
fn pair_identity_endpoint_and_material_failures_keep_their_order() {
    let mut set = fixture(3);
    let selected = ids(&set);
    set.cards[2].material = Material::Chord {
        name: "not-an-installed-formula".into(),
        root: PitchClass::new(0),
    };
    for (selection, pair, expected) in [
        (
            vec![selected[0], selected[0]],
            (CardId(999), CardId(999)),
            DisclosureError::DuplicateSelection(selected[0]),
        ),
        (
            vec![selected[0], CardId(999)],
            (selected[0], selected[0]),
            DisclosureError::UnknownCard(CardId(999)),
        ),
        (
            selected.clone(),
            (CardId(999), selected[0]),
            DisclosureError::EndpointNotSelected(CardId(999)),
        ),
        (
            selected.clone(),
            (selected[0], selected[0]),
            DisclosureError::SameOccurrence,
        ),
        (
            selected.clone(),
            (selected[0], selected[1]),
            DisclosureError::UnsupportedMaterial(selected[2]),
        ),
    ] {
        assert_eq!(
            disclose_comparison(WorkingSetId(7), "captured", &set, &selection, pair),
            Err(expected)
        );
    }
}

#[test]
fn sparse_reversed_selection_keeps_authored_order_repeated_identity_and_source_bytes() {
    let mut set = fixture(4);
    set.cursor = 2;
    let selected = ids(&set);
    let before = serde_json::to_vec(&set).unwrap();
    let value = passage_projection_json(
        WorkingSetId(7),
        "captured",
        &set,
        &[selected[3], selected[0], selected[2]],
    )
    .unwrap();
    let occurrences = value["dataset"]["occurrences"].as_array().unwrap();
    assert_eq!(
        occurrences
            .iter()
            .map(|o| o["occurrence_id"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("set:7:card:1"),
            json!("set:7:card:3"),
            json!("set:7:card:4")
        ]
    );
    assert_eq!(
        occurrences
            .iter()
            .map(|o| o["values"]["order"]["value"].clone())
            .collect::<Vec<_>>(),
        vec![json!(1), json!(3), json!(4)]
    );
    assert!(
        occurrences
            .iter()
            .all(|o| o["source"] == occurrences[0]["source"])
    );
    let relations = value["relationships"].as_array().unwrap();
    assert_eq!(relations.len(), 4);
    for (index, (from, to)) in [
        ("set:7:card:1", "set:7:card:3"),
        ("set:7:card:3", "set:7:card:4"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(relations[index * 2]["kind"], SHARED_PITCH_CLASSES);
        assert_eq!(relations[index * 2 + 1]["kind"], PITCH_EQUALITY);
        for relation in &relations[index * 2..index * 2 + 2] {
            assert_eq!(relation["from_occurrence"], from);
            assert_eq!(relation["to_occurrence"], to);
        }
    }
    assert_eq!(serde_json::to_vec(&set).unwrap(), before);
}

#[test]
fn public_projection_wrappers_preserve_edited_overlap_fields_and_authored_evidence_order() {
    let mut set = fixture(3);
    set.cards[2].material = Material::Chord {
        name: "Minor".into(),
        root: PitchClass::new(9),
    };
    let selected = ids(&set);
    // Reverse endpoints while occurrence/evidence order remains authored order.
    let mut facts = disclose_comparison(
        WorkingSetId(7),
        "captured",
        &set,
        &selected,
        (selected[2], selected[0]),
    )
    .unwrap();
    facts.relationship.id = "edited-overlap-id".into();
    facts.relationship.explanation = "Caller-supplied retained explanation".into();
    facts.relationship.method = "caller.overlap.method".into();
    facts.relationship.shared_pitch_classes = vec![11];
    let before = facts.clone();
    let legacy = projection_json(&facts);
    let rich = comparison_projection_json(&facts);
    assert_eq!(facts, before);
    assert_eq!(rich["dataset"], legacy["dataset"]);
    assert_eq!(rich["relationships"][0], legacy["relationships"][0]);
    let overlap = &legacy["relationships"][0];
    assert_eq!(overlap["id"], "edited-overlap-id");
    assert_eq!(
        overlap["explanation"],
        "Caller-supplied retained explanation"
    );
    assert_eq!(overlap["provenance"]["method"], "caller.overlap.method");
    assert_eq!(overlap["from_occurrence"], "set:7:card:3");
    assert_eq!(overlap["to_occurrence"], "set:7:card:1");
    let evidence = json!([
        {"adapter": DISCLOSURE_ADAPTER, "id": facts.occurrences[0].material.wire_key()},
        {"adapter": DISCLOSURE_ADAPTER, "id": facts.occurrences[2].material.wire_key()},
    ]);
    assert_eq!(overlap["provenance"]["evidence"], evidence);
    let difference = &rich["relationships"][1];
    assert_eq!(difference["kind"], TONE_DIFFERENCES);
    assert_eq!(difference["provenance"]["method"], FULL_COMPARISON_METHOD);
    assert_eq!(difference["provenance"]["evidence"], evidence);
    assert!(
        difference["explanation"]
            .as_str()
            .unwrap()
            .contains("Shared: C, E.")
    );
    // Unknown caller-only metadata is not synthesized into the schema.
    assert_eq!(
        legacy["dataset"]["revision"],
        Value::String("captured".into())
    );
}
