use super::*;

#[test]
fn claims_reading_keeps_generated_and_personal_reasons_with_distinct_owners() {
    let mut ui = UiState::new();
    ui.app_settings.stage.related.graph_scope = woodshed_core::settings::RelatedGraphScope::Mere;
    let generated = related_mere_snapshot(&ui).relations[0].clone();
    let from = generated.provenance.original_from.clone();
    let to = generated.provenance.original_to.clone();
    let actor = Author::new("personae:alice");
    let mut source =
        PersonalRelationSource::empty(PersonalRelationSourceId::from_random_uuid_bytes([9; 16]));
    let personal_id = source
        .assert_relation(
            &actor,
            0,
            &from,
            &to,
            Relation::new(RelationClass::app("woodshed", 3)).with_label("related"),
            Some(10),
            AssertionBasis {
                note: Some("personal interpretation".into()),
                ..AssertionBasis::default()
            },
        )
        .unwrap();
    ui.personal_relations = Some(source);
    ui.personal_link_pair = Some((from.clone(), to.clone()));

    let reading = claims_link_reading(&ui).unwrap();
    let assertions = reading
        .links
        .iter()
        .flat_map(|link| link.buckets.iter())
        .flat_map(|bucket| bucket.assertions.iter())
        .collect::<Vec<_>>();
    let generated_claim = assertions
        .iter()
        .find(|assertion| {
            assertion.id == format!("generated:{}", generated.claim_id.qualified_key())
        })
        .expect("generated reason is part of the pair reading");
    assert_eq!(
        generated_claim.scope,
        LinkGraphScope::Custom(format!(
            "urn:woodshed:generated-claim:{}",
            generated.claim_id.owner.as_str()
        ))
    );
    assert_eq!(generated_claim.asserter, None);
    assert_eq!(generated_claim.asserted_at_ms, None);
    assert_eq!(generated_claim.withdrawn, None);
    assert_eq!(
        generated_claim.predicate,
        generated.provenance.original_kind.predicate_iri()
    );
    assert_eq!(
        generated_claim.label.as_deref(),
        Some(generated.provenance.original_explanation.as_str())
    );
    assert_eq!(
        generated_claim
            .basis
            .method
            .as_ref()
            .map(|method| method.iri.as_str()),
        Some(generated.provenance.method.iri)
    );
    assert_eq!(
        generated_claim.basis.evidence,
        generated
            .provenance
            .evidence
            .iter()
            .map(generated_evidence_token)
            .collect::<Vec<_>>()
    );

    let personal = assertions
        .iter()
        .find(|assertion| {
            assertion.id == format!("personal:{}", serde_json::to_string(&personal_id).unwrap())
        })
        .expect("personal reason remains independently addressable");
    assert_eq!(
        personal.basis.note.as_deref(),
        Some("personal interpretation")
    );
    assert_eq!(personal.asserter.as_deref(), Some("personae:alice"));
    assert_eq!(
        personal.predicate,
        woodshed_core::mere::RELATED_PREDICATE_IRI
    );
    assert_ne!(generated_claim.id, personal.id);
}

#[test]
fn claims_source_discloses_personal_owner_unavailable_while_generated_reading_remains_current() {
    let mut ui = UiState::new();
    ui.personal_link_pair = Some(("chord:Major".into(), "chord:Major 7".into()));

    let reading = claims_link_reading(&ui).unwrap();
    let LinkSource::Available { binding, revision } = &reading.source else {
        panic!("the generated catalog projection is still available");
    };
    assert!(binding.resource.contains("personal-source:unavailable"));
    assert!(
        binding
            .domain
            .contains("identified separately when present")
    );
    assert!(matches!(
        revision,
        LinkSourceRevision::Unavailable { reason }
            if reason.contains("no shared immutable public revision")
    ));
}

#[test]
fn opened_pair_claims_ignore_unrelated_selection_canvas_scope() {
    let mut ui = UiState::new();
    ui.app_settings.stage.related.graph_scope =
        woodshed_core::settings::RelatedGraphScope::Selection;
    ui.app_settings.stage.related.relation_depth = 1;
    ui.stage.set_lens(woodshed_core::Lens::Progressions);
    ui.personal_link_pair = Some(("chord:Major".into(), "chord:Major 7".into()));

    assert!(
        super::super::super::related_mere_snapshot(&ui)
            .relations
            .is_empty(),
        "the current progression selection has no selected catalog center"
    );
    let (from, to) = ui.personal_link_pair.as_ref().unwrap();
    let whole = woodshed_core::mere::woodshed_mere(
        &ui.practice_history,
        woodshed_core::mere::MereScope::Whole,
    );
    let expected_ids = whole
        .relations
        .iter()
        .filter(|relation| {
            relation.source.as_str() == from.as_str() && relation.target.as_str() == to.as_str()
        })
        .map(|relation| relation.claim_id.qualified_key())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(!expected_ids.is_empty());

    let reading = claims_link_reading(&ui).expect("opened pair has a reading");
    let actual_ids = reading
        .links
        .iter()
        .flat_map(|link| link.buckets.iter())
        .flat_map(|bucket| bucket.assertions.iter())
        .filter_map(|assertion| assertion.id.strip_prefix("generated:"))
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_ids, expected_ids);

    let target = cambium::LinkMemberTarget {
        source: reading.source,
        link: LinkKey::new(LinkStratum::Surface, from.clone(), to.clone()),
        from: from.clone(),
        to: to.clone(),
        assertion_id: format!("generated:{}", expected_ids.first().unwrap()),
    };
    queue_relation_action(
        &mut ui,
        OpenedLinkEvent::Member {
            target,
            action: "audition".into(),
        },
        0,
    );
    assert!(matches!(
        ui.audio_requests.as_slice(),
        [woodshed_core::audio::AudioRequest::PreviewPhrase { .. }]
    ));
    assert!(
        ui.personal_relation_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("Audition queued in assertion direction"))
    );
}

#[test]
fn opened_minor_major_pair_keeps_all_catalog_adjacency_ids_outside_canvas_filter() {
    let mut ui = UiState::new();
    ui.app_settings.stage.related.graph_scope =
        woodshed_core::settings::RelatedGraphScope::Selection;
    ui.app_settings.stage.related.relation_depth = 1;
    ui.stage.set_lens(woodshed_core::Lens::Progressions);
    let from = "chord:Minor".to_owned();
    let to = "chord:Major".to_owned();
    let predicate = "urn:woodshed:relation:used-together";
    ui.personal_link_pair = Some((from.clone(), to.clone()));

    assert!(
        super::super::super::related_mere_snapshot(&ui)
            .relations
            .is_empty(),
        "the selected canvas lens has no active progression center"
    );

    // The core selection seam forwards the raw graph's catalog reasons;
    // it does not pass through woodshed_mere, the projection under test.
    let mut raw = std::collections::BTreeMap::new();
    for (source, target) in [(&from, &to), (&to, &from)] {
        let mut source_selection = woodshed_core::StageState::new();
        assert!(source_selection.select_catalog_id(source));
        for neighbor in source_selection.related_material(usize::MAX) {
            for relation in neighbor.relations.into_iter().filter(|relation| {
                relation.target == *target
                    && relation.provenance.original_kind.predicate_iri() == predicate
            }) {
                raw.insert(
                    relation.claim_id.qualified_key(),
                    (
                        relation.provenance.original_from,
                        relation.provenance.original_to,
                    ),
                );
            }
        }
    }
    assert_eq!(
        raw.len(),
        11,
        "the catalog fixture has eleven source claims"
    );
    assert!(
        raw.values()
            .filter(|(source, target)| source == &from && target == &to)
            .count()
            > 1,
        "parallel adjacency reasons in the selected direction keep separate IDs"
    );

    let pair_snapshot = super::super::super::opened_claims_mere_snapshot(&ui, &from);
    let projected = pair_snapshot
        .relations
        .iter()
        .filter(|relation| {
            relation.provenance.original_kind.predicate_iri() == predicate
                && ((relation.provenance.original_from == from
                    && relation.provenance.original_to == to)
                    || (relation.provenance.original_from == to
                        && relation.provenance.original_to == from))
        })
        .map(|relation| {
            (
                relation.claim_id.qualified_key(),
                (
                    relation.provenance.original_from.clone(),
                    relation.provenance.original_to.clone(),
                ),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(projected, raw);

    let reading = claims_link_reading(&ui).expect("opened pair has a reading");
    let actual = reading
        .links
        .iter()
        .flat_map(|link| link.buckets.iter())
        .flat_map(|bucket| {
            bucket
                .assertions
                .iter()
                .filter(|assertion| assertion.predicate == predicate)
                .filter_map(|assertion| {
                    assertion.id.strip_prefix("generated:").map(|id| {
                        (
                            id.to_owned(),
                            (bucket.source_from.id.clone(), bucket.source_to.id.clone()),
                        )
                    })
                })
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        actual, raw,
        "both directions keep their exact source buckets"
    );

    let key = LinkKey::new(LinkStratum::Surface, from.clone(), to.clone());
    let pair_only = woodshed_core::mere::WoodshedMereSnapshot {
        nodes: pair_snapshot
            .nodes
            .iter()
            .filter(|node| node.id == from || node.id == to)
            .cloned()
            .collect(),
        relations: pair_snapshot
            .relations
            .iter()
            .filter(|relation| {
                LinkKey::new(
                    LinkStratum::Surface,
                    relation.provenance.original_from.clone(),
                    relation.provenance.original_to.clone(),
                ) == key
            })
            .cloned()
            .collect(),
    };
    let swatch = super::super::super::related_swatch_from_snapshot(&pair_only, &ui, true);
    assert_eq!(swatch.relations.len(), 1);
    assert_eq!(swatch.relations[0].link_key.as_ref(), Some(&key));
    assert_eq!(swatch.relations[0].id, key.stable_id());
}
