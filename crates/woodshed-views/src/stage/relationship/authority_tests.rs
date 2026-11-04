use super::*;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshedding::pitch::PitchClass;
#[test]
fn pending_choices_cannot_bind_another_owner_with_the_same_card_ids() {
    let mut ui = UiState::new();
    for _ in 0..2 {
        ui.set.push(
            KeyedCatalogRef {
                formula_id: "chord:Major".into(),
                root: PitchClass::new(0),
            }
            .to_card()
            .unwrap(),
        );
    }
    ui.open_relationship_recipe();
    let copy = ui.set.clone();
    ui.working_sets.create(&mut ui.set, "Other", copy);
    let before = serde_json::to_value(&ui.set).unwrap();
    assert!(ui.bind_relationship_recipe(false).is_err());
    assert!(ui.relationship_reading_json.is_none());
    assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
    ui.open_relationship_recipe();
    ui.bind_relationship_recipe(false).unwrap();
    assert_eq!(
        ui.relationship_reading()
            .unwrap()
            .unwrap()
            .source
            .unwrap()
            .owner,
        ui.working_sets.active_id
    );
}
#[test]
fn source_action_disambiguates_identical_card_numbers_in_two_owners() {
    let mut ui = UiState::new();
    for _ in 0..2 {
        ui.set.push(
            KeyedCatalogRef {
                formula_id: "chord:Major".into(),
                root: PitchClass::new(0),
            }
            .to_card()
            .unwrap(),
        );
    }
    ui.open_relationship_recipe();
    ui.bind_relationship_recipe(false).unwrap();
    let owner = ui.working_sets.active_id;
    ui.select_relationship(
        Some(comparison_disclosure::occurrence_id(owner, CardId(2))),
        None,
    )
    .unwrap();
    let copy = ui.set.clone();
    let other = ui.working_sets.create(&mut ui.set, "Other", copy);
    ui.set.cards[1].label = "Other owner's Card 2".into();
    ui.rehearsal_owner = Some(other);
    ui.rehearsal_running = true;
    ui.open_relationship_source().unwrap();
    assert_eq!(ui.working_sets.active_id, owner);
    assert_eq!(ui.set.cursor_id(), Some(CardId(2)));
    assert_ne!(ui.set.cards[1].label, "Other owner's Card 2");
    assert_eq!(
        ui.working_sets.get(other, &ui.set).unwrap().cards[1].label,
        "Other owner's Card 2"
    );
    assert_eq!(ui.rehearsal_owner, Some(other));
    assert!(ui.rehearsal_running);
}
#[test]
fn source_envelope_and_unknown_versions_refuse_but_preserve_bytes() {
    let mut ui = UiState::new();
    for _ in 0..2 {
        ui.set.push(
            KeyedCatalogRef {
                formula_id: "chord:Major".into(),
                root: PitchClass::new(0),
            }
            .to_card()
            .unwrap(),
        );
    }
    ui.open_relationship_recipe();
    ui.bind_relationship_recipe(false).unwrap();
    let valid = ui.relationship_reading().unwrap().unwrap();
    for change in 0..3 {
        let mut reading = valid.clone();
        match change {
            0 => reading.version = 4,
            1 => reading.source.as_mut().unwrap().cards.push(CardId(1)),
            _ => reading.source.as_mut().unwrap().owner = WorkingSetId(0),
        };
        let bytes = serde_json::to_string(&reading).unwrap();
        ui.relationship_reading_json = Some(bytes.clone());
        assert!(ui.relationship_reading().is_err());
        assert_eq!(ui.to_persisted().relationship_reading_json, Some(bytes));
    }
}
