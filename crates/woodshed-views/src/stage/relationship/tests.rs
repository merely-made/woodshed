use super::*;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshedding::pitch::PitchClass;
fn fixture() -> UiState {
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
    ui.bind_relationship_recipe(false).unwrap();
    ui
}

#[test]
fn relationship_canvas_has_one_rest_stroke_and_opens_all_directed_members() {
    let mut ui = fixture();
    ui.relationship_cards = vec![ui.set.cards[0].id, ui.set.cards[1].id];
    ui.bind_relationship_recipe(true).unwrap();
    let reading = ui.relationship_reading().unwrap().unwrap();
    let compiled = reading.compile().unwrap();
    let keys = relationship_link_keys(&reading);
    assert!(!keys.is_empty());

    let swatch = swatch_from(&reading, &compiled, ui.viewport_width, None);
    assert_eq!(swatch.relations.len(), keys.len());
    for key in &keys {
        let visual = swatch
            .relations
            .iter()
            .find(|relation| relation.link_key.as_ref() == Some(key))
            .expect("each canonical pair has one resting stroke");
        let pair = relationship_pair_reading(&reading, key).unwrap();
        let link = pair.links.iter().find(|link| link.key == *key).unwrap();
        let summary = link.rest_summary(&key.first, &key.second);
        assert_eq!(visual.id, key.stable_id());
        assert_eq!(visual.marks, summary.marks);
        assert_eq!(visual.strength, summary.strength);

        let projected = reading
            .dataset
            .relationships
            .iter()
            .filter(|relation| {
                LinkKey::new(
                    LinkStratum::Surface,
                    relation.from_occurrence.clone(),
                    relation.to_occurrence.clone(),
                ) == *key
            })
            .map(|relation| {
                (
                    relation.from_occurrence.clone(),
                    relation.to_occurrence.clone(),
                    relation.id.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        let disclosed = link
            .buckets
            .iter()
            .flat_map(|bucket| {
                bucket.assertions.iter().map(|assertion| {
                    (
                        bucket.source_from.id.clone(),
                        bucket.source_to.id.clone(),
                        assertion.id.clone(),
                    )
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(disclosed, projected);
        assert!(link.buckets.iter().all(|bucket| {
            bucket.assertions.iter().all(|assertion| {
                assertion.scope == LinkGraphScope::Source
                    && assertion.asserter.is_none()
                    && assertion.first_asserted_ms.is_none()
                    && assertion.asserted_at_ms.is_none()
            })
        }));
    }

    let key = keys.iter().next().unwrap().clone();
    open_relationship_link(&mut ui, &key.stable_id()).unwrap();
    assert_eq!(ui.relationship_opened_link.as_ref(), Some(&key));
    let pair = relationship_pair_reading(&reading, &key).unwrap();
    let bucket = pair.links[0].buckets.first().unwrap();
    let assertion = bucket.assertions.first().unwrap();
    let target = cambium::LinkMemberTarget {
        source: pair.source.clone(),
        link: key,
        from: bucket.source_from.id.clone(),
        to: bucket.source_to.id.clone(),
        assertion_id: assertion.id.clone(),
    };
    let source = reading.source.as_ref().unwrap();
    let expected_card = source
        .cards
        .iter()
        .copied()
        .find(|card| comparison_disclosure::occurrence_id(source.owner, *card) == target.from)
        .unwrap();
    let cards_before = serde_json::to_value(&ui.set.cards).unwrap();
    open_relationship_member(&mut ui, &target, "open-source-card").unwrap();
    assert_eq!(ui.set.cursor_id(), Some(expected_card));
    assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), cards_before);
}

#[test]
fn legacy_reading_preserves_evidence_until_explicit_richer_rebind() {
    let mut ui = fixture();
    let mut reading = ui.relationship_reading().unwrap().unwrap();
    let source = reading.source.as_ref().unwrap();
    let facts = comparison_disclosure::disclose(
        source.owner,
        &source.revision,
        &ui.set,
        &source.cards,
        source.pair,
    )
    .unwrap();
    reading.version = 1;
    reading.dataset =
        serde_json::from_value(comparison_disclosure::projection_json(&facts)).unwrap();
    reading.snapshot.selected_occurrence = Some(comparison_disclosure::occurrence_id(
        source.owner,
        source.cards[0],
    ));
    ui.install_relationship(reading).unwrap();
    let retained = ui.relationship_reading_json.clone();
    let mut restored = UiState::new();
    restored.apply_persisted(&ui.to_persisted(), Default::default());
    restored.open_relationship_recipe();
    assert_eq!(restored.relationship_reading_json, retained);
    assert_eq!(
        restored
            .relationship_reading()
            .unwrap()
            .unwrap()
            .dataset
            .relationships
            .len(),
        1
    );
    restored.open_relationship_source().unwrap();
    restored.open_relationship_recipe();
    restored.bind_relationship_recipe(true).unwrap();
    let rebound = restored.relationship_reading().unwrap().unwrap();
    assert_eq!(rebound.version, 3);
    assert_eq!(rebound.dataset.relationships.len(), 4);
}

#[test]
fn version_two_pair_evidence_stays_unchanged_until_passage_rebind() {
    let mut ui = fixture();
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
    reading.version = 2;
    reading.dataset =
        serde_json::from_value(comparison_disclosure::comparison_projection_json(&facts)).unwrap();
    reading.snapshot.selected_occurrence = Some("set:1:card:2".into());
    ui.install_relationship(reading).unwrap();
    let captured = ui.relationship_reading_json.clone();
    ui.open_relationship_recipe();
    assert_eq!(ui.relationship_reading_json, captured);
    ui.open_relationship_source().unwrap();
    assert_eq!(ui.set.cursor_id(), Some(CardId(2)));
    ui.open_relationship_recipe();
    ui.bind_relationship_recipe(true).unwrap();
    let reading = ui.relationship_reading().unwrap().unwrap();
    assert_eq!(reading.version, 3);
    assert_eq!(reading.compile().unwrap().relationships.len(), 4);
    assert_eq!(
        reading.snapshot.selected_occurrence.as_deref(),
        Some("set:1:card:2")
    );
}

#[test]
fn disjoint_reading_compiles_and_returns_to_exact_source() {
    let mut ui = fixture();
    ui.set.cards[1].material = woodshedding::rehearsal::Material::Chord {
        name: "Major".into(),
        root: woodshedding::pitch::PitchClass::new(1),
    };
    ui.open_relationship_recipe();
    ui.bind_relationship_recipe(true).unwrap();
    let reading = ui.relationship_reading().unwrap().unwrap();
    assert_eq!(reading.compile().unwrap().relationships.len(), 2);
    assert_eq!(
        reading.dataset.relationships[0].kind,
        comparison_disclosure::TONE_DIFFERENCES
    );
    let id = reading.dataset.dataset.occurrences[1].occurrence_id.clone();
    ui.select_relationship(Some(id), None).unwrap();
    ui.open_relationship_source().unwrap();
    assert_eq!(ui.set.cursor_id(), Some(ui.set.cards[1].id));
}

#[test]
fn shared_edits_selection_and_explanation_survive_session_restore() {
    let mut ui = fixture();
    let before = serde_json::to_value(&ui.set).unwrap();
    ui.edit_relationship_recipe(Some("My comparison".into()), Some(48))
        .unwrap();
    let reading = ui.relationship_reading().unwrap().unwrap();
    let occurrence = reading.dataset.dataset.occurrences[1].occurrence_id.clone();
    let relation = reading.dataset.relationships[0].id.clone();
    ui.select_relationship(Some(occurrence.clone()), Some(relation.clone()))
        .unwrap();
    assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
    let mut restored = UiState::new();
    restored.apply_persisted(&ui.to_persisted(), Default::default());
    let reading = restored.relationship_reading().unwrap().unwrap();
    assert_eq!(reading.snapshot.recipe.definition.label, "My comparison");
    assert_eq!(reading.snapshot.recipe.definition.arrangement.spacing, 48);
    assert_eq!(reading.snapshot.selected_occurrence, Some(occurrence));
    assert_eq!(reading.snapshot.selected_relationship, Some(relation));
    let compiled = reading.compile().unwrap();
    assert_eq!(compiled.relationships.len(), 4);
    assert_eq!(compiled.projection.scene.relations.len(), 4);
    assert_eq!(
        compiled
            .relationships
            .iter()
            .find(|r| r.disclosure.id == reading.dataset.relationships[0].id)
            .unwrap()
            .disclosure
            .explanation,
        reading.dataset.relationships[0].explanation
    );
    assert!(!restored.rehearsal_running);
}
#[test]
fn exact_owner_action_preserves_background_runner_and_cursor_navigation_is_current() {
    let mut ui = fixture();
    let owner = ui.working_sets.active_id;
    let id = ui.set.cards[1].id;
    ui.select_relationship(Some(comparison_disclosure::occurrence_id(owner, id)), None)
        .unwrap();
    ui.set.cursor = 2;
    ui.duplicate_working_set();
    let other = ui.working_sets.active_id;
    ui.rehearsal_owner = Some(other);
    ui.rehearsal_running = true;
    let other_set = serde_json::to_value(&ui.set).unwrap();
    ui.open_relationship_source().unwrap();
    assert_eq!(ui.working_sets.active_id, owner);
    assert_eq!(ui.set.cursor_id(), Some(id));
    assert_eq!(ui.rehearsal_owner, Some(other));
    assert!(ui.rehearsal_running);
    assert_eq!(
        serde_json::to_value(ui.working_sets.get(other, &ui.set).unwrap()).unwrap(),
        other_set
    );
}
#[test]
fn full_instruction_changes_and_missing_or_ambiguous_owners_refuse_without_navigation() {
    for change in 0..5 {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let id = ui.set.cards[0].id;
        ui.select_relationship(Some(comparison_disclosure::occurrence_id(owner, id)), None)
            .unwrap();
        match change {
            0 => ui.set.cards[0].setting.capo = Some(2),
            1 => ui.set.cards[0].timing.bpm = Some(101.0),
            2 => ui.set.cards.swap(0, 1),
            3 => {
                ui.set.cards.remove(0);
            },
            _ => ui.set.cards[0].label.push_str(" changed"),
        };
        let before = serde_json::to_value(&ui.set).unwrap();
        assert!(ui.open_relationship_source().is_err());
        assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
    }
    let mut ui = fixture();
    let owner = ui.working_sets.active_id;
    let id = ui.set.cards[0].id;
    ui.select_relationship(Some(comparison_disclosure::occurrence_id(owner, id)), None)
        .unwrap();
    ui.working_sets
        .inactive
        .push(woodshed_core::working_sets::WorkingSet {
            id: owner,
            name: "Ambiguous".into(),
            set: ui.set.clone(),
        });
    assert!(ui.open_relationship_source().is_err());
}
#[test]
fn explicit_rebind_preserves_recipe_and_occurrences_do_not_change_cards() {
    let mut ui = fixture();
    ui.edit_relationship_recipe(Some("Reusable".into()), Some(48))
        .unwrap();
    let first = ui.relationship_reading().unwrap().unwrap();
    ui.duplicate_working_set();
    ui.open_relationship_recipe();
    let before = serde_json::to_value(&ui.set).unwrap();
    ui.bind_relationship_recipe(true).unwrap();
    let reading = ui.relationship_reading().unwrap().unwrap();
    assert_eq!(reading.snapshot.recipe.definition.label, "Reusable");
    assert_eq!(reading.snapshot.recipe.definition.arrangement.spacing, 48);
    assert_ne!(first.dataset.dataset.source, reading.dataset.dataset.source);
    assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
    assert_ne!(
        reading.dataset.dataset.occurrences[0].occurrence_id,
        reading.dataset.dataset.occurrences[1].occurrence_id
    );
    assert_eq!(
        reading.dataset.dataset.occurrences[0].source,
        reading.dataset.dataset.occurrences[1].source
    );
}
#[test]
fn invalid_optional_payload_is_preserved_and_never_discards_sets() {
    for invalid in [
        "{".to_string(),
        "x".repeat(MAX_READING_BYTES + 1),
        "{\"version\":99}".to_string(),
    ] {
        let mut ui = fixture();
        ui.relationship_reading_json = Some(invalid.clone());
        let persisted = ui.to_persisted();
        let mut restored = UiState::new();
        restored.apply_persisted(&persisted, Default::default());
        assert_eq!(restored.set.cards.len(), 3);
        assert!(restored.relationship_reading().is_err());
        assert_eq!(
            restored.to_persisted().relationship_reading_json,
            Some(invalid)
        );
    }
}
#[test]
fn incompatible_facet_and_stale_provenance_refuse_without_replacing_payload() {
    let mut ui = fixture();
    let valid = ui.relationship_reading_json.clone();
    let mut reading = ui.relationship_reading().unwrap().unwrap();
    reading.dataset.facets.remove("authored_order");
    assert!(ui.install_relationship(reading).is_err());
    assert_eq!(ui.relationship_reading_json, valid);
    let mut reading = ui.relationship_reading().unwrap().unwrap();
    reading.dataset.relationships[0].provenance.source_revision =
        PublicSourceRevision::new("stale");
    assert!(ui.install_relationship(reading).is_err());
    assert_eq!(ui.relationship_reading_json, valid);
}
