use super::*;
use std::collections::BTreeSet;
use woodshed_core::personal_relations::{
    AssertionBasis, PersonalRelationSource, PersonalRelationSourceId, Relation,
};

#[test]
fn personal_custom_predicates_reach_shared_reader_with_exact_history_and_basis() {
    let from = "chord:Major".to_string();
    let to = "chord:Minor".to_string();
    let actor = Author::new("personae:p6-reader");
    let source_id = PersonalRelationSourceId::from_random_uuid_bytes([0x66; 16]);
    let mut source = PersonalRelationSource::empty(source_id.clone());
    let relation =
        || Relation::new(RelationClass::app("portable-test", 47)).with_label("custom predicate");
    let first = source
        .assert_relation(
            &actor,
            0,
            &from,
            &to,
            relation(),
            Some(100),
            AssertionBasis {
                note: Some("first reason".into()),
                evidence: vec!["urn:evidence:first".into()],
                ..Default::default()
            },
        )
        .unwrap();
    let second = source
        .assert_relation(
            &actor,
            source.runtime_revision(),
            &from,
            &to,
            relation(),
            Some(110),
            AssertionBasis {
                note: Some("second reason".into()),
                evidence: vec!["urn:evidence:second".into()],
                ..Default::default()
            },
        )
        .unwrap();

    let mut ui = UiState::new();
    ui.personal_relations = Some(source);
    ui.personal_link_pair = Some((from.clone(), to.clone()));
    let held = claims_link_reading(&ui).expect("the personal pair has a reading");
    let held_records = held.links[0]
        .buckets
        .iter()
        .flat_map(|bucket| &bucket.assertions)
        .filter(|assertion| assertion.id.starts_with("personal:"))
        .collect::<Vec<_>>();
    assert_eq!(held_records.len(), 2);
    assert_ne!(held_records[0].id, held_records[1].id);
    assert!(held_records.iter().all(|assertion| {
        assertion.predicate == "app:portable-test:47"
            && assertion.recognized_kind.is_none()
            && assertion.scope == LinkGraphScope::Custom(source_id.to_string())
            && assertion.asserter.as_deref() == Some("personae:p6-reader")
            && assertion.withdrawn.is_none()
    }));
    assert_eq!(
        held_records
            .iter()
            .map(|assertion| assertion.basis.note.as_deref())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([Some("first reason"), Some("second reason")])
    );
    assert_eq!(
        held_records
            .iter()
            .flat_map(|assertion| assertion.basis.evidence.iter().map(String::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["urn:evidence:first", "urn:evidence:second"])
    );

    let layout = OpenedLinkLayout::default_for(&held);
    let compiled = scenomise::compile_opened_link(
        &held,
        &layout,
        scenograph::link::OpenedLinkOptions { now_ms: Some(120) },
    )
    .expect("the shared reader compiles the portable records");
    let resolved_ids = compiled
        .entries
        .iter()
        .filter_map(|entry| match compiled.resolve(entry) {
            Some(scenograph::link::OpenedLinkMember::Assertion(assertion))
                if assertion.id.starts_with("personal:") =>
            {
                Some(assertion.id.as_str())
            },
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let first_id = format!("personal:{}", serde_json::to_string(&first).unwrap());
    let second_id = format!("personal:{}", serde_json::to_string(&second).unwrap());
    assert_eq!(
        resolved_ids,
        BTreeSet::from([first_id.as_str(), second_id.as_str()])
    );
    assert_eq!(
        compiled.reading, held,
        "compilation retains the source reading"
    );
    let LinkSource::Available { binding, revision } = &compiled.reading.source else {
        panic!("the personal relation source is disclosed");
    };
    assert!(binding.resource.contains(&source_id.to_string()));
    assert!(matches!(revision, LinkSourceRevision::Unavailable { .. }));

    let source = ui.personal_relations.as_mut().unwrap();
    let first_id_typed = first;
    source
        .withdraw_exact(
            &actor,
            source.runtime_revision(),
            first_id_typed,
            &from,
            &to,
            Some(115),
        )
        .unwrap();
    ui.personal_link_history = true;
    let history = claims_link_reading(&ui).expect("explicit history includes the tombstone");
    let history_records = history.links[0]
        .buckets
        .iter()
        .flat_map(|bucket| &bucket.assertions)
        .filter(|assertion| assertion.id.starts_with("personal:"))
        .collect::<Vec<_>>();
    assert_eq!(history_records.len(), 2);
    let withdrawn = history_records
        .iter()
        .find(|assertion| assertion.id == first_id)
        .unwrap();
    assert_eq!(withdrawn.withdrawn.as_ref().unwrap().at_ms, Some(115));
    assert_eq!(
        withdrawn.withdrawn.as_ref().unwrap().by.as_deref(),
        Some("personae:p6-reader")
    );
    assert_eq!(withdrawn.basis.note.as_deref(), Some("first reason"));
    assert_eq!(withdrawn.basis.evidence, vec!["urn:evidence:first"]);
    let held_after_withdrawal = history_records
        .iter()
        .find(|assertion| assertion.id == second_id)
        .unwrap();
    assert!(held_after_withdrawal.withdrawn.is_none());
    assert_eq!(
        held_after_withdrawal.basis.note.as_deref(),
        Some("second reason")
    );
    assert_eq!(
        held_after_withdrawal.basis.evidence,
        vec!["urn:evidence:second"]
    );
    assert_eq!(history.source, held.source);
}

#[test]
fn exact_assertion_audition_queues_frozen_before_then_after_without_touching_set() {
    let from = "chord:Major".to_string();
    let to = "chord:Minor".to_string();
    let actor = Author::new("personae:alice");
    let mut source =
        PersonalRelationSource::empty(PersonalRelationSourceId::from_random_uuid_bytes([5; 16]));
    let id = source
        .assert_relation(
            &actor,
            0,
            &from,
            &to,
            Relation::new(RelationClass::app("woodshed", 3)).with_label("related"),
            Some(100),
            AssertionBasis::default(),
        )
        .unwrap();
    let revision = source.runtime_revision();
    let mut ui = UiState::new();
    ui.personal_relations = Some(source);
    ui.personal_link_pair = Some((from.clone(), to.clone()));
    ui.seal = Some(crate::persona::PracticeSeal::Sealed {
        persona: actor.as_str().to_string(),
        protection: "test".into(),
    });
    let set_before = serde_json::to_value(&ui.set).unwrap();
    let stage_material_before = ui.stage.catalog_id();
    let reading = claims_link_reading(&ui).unwrap();
    let expected_source = reading.source.clone();
    let key = LinkKey::new(LinkStratum::Surface, from.clone(), to.clone());
    let target = cambium::LinkMemberTarget {
        source: expected_source,
        link: key,
        from: from.clone(),
        to: to.clone(),
        assertion_id: format!("personal:{}", serde_json::to_string(&id).unwrap()),
    };

    queue_relation_action(
        &mut ui,
        OpenedLinkEvent::Member {
            target,
            action: "audition".into(),
        },
        revision,
    );

    let before_expected = endpoint_preview(&ui, &from).unwrap().0;
    let after_expected = endpoint_preview(&ui, &to).unwrap().0;
    assert!(matches!(
        ui.audio_requests.as_slice(),
        [woodshed_core::audio::AudioRequest::PreviewPhrase { before, after, .. }]
            if before == &before_expected && after == &after_expected
    ));
    assert_eq!(serde_json::to_value(&ui.set).unwrap(), set_before);
    assert_eq!(ui.stage.catalog_id(), stage_material_before);
    assert!(ui.personal_relation_intents.is_empty());
}

#[test]
fn generated_inverse_alias_auditions_original_assertion_direction() {
    let mut ui = UiState::new();
    ui.app_settings.stage.related.graph_scope = woodshed_core::settings::RelatedGraphScope::Mere;
    let snapshot = related_mere_snapshot(&ui);
    let generated = snapshot
        .relations
        .iter()
        .find(|relation| {
            snapshot.relations.iter().any(|candidate| {
                candidate.claim_id == relation.claim_id
                    && candidate.source == relation.target
                    && candidate.target == relation.source
            })
        })
        .unwrap()
        .clone();
    let original_from = generated.provenance.original_from.clone();
    let original_to = generated.provenance.original_to.clone();
    let reverse_display = snapshot
        .relations
        .iter()
        .find(|candidate| {
            candidate.claim_id == generated.claim_id
                && candidate.source == original_to
                && candidate.target == original_from
        })
        .unwrap();
    assert_ne!(reverse_display.source, original_from);
    ui.personal_link_pair = Some((
        reverse_display.source.clone(),
        reverse_display.target.clone(),
    ));
    let reading = claims_link_reading(&ui).unwrap();
    let target = cambium::LinkMemberTarget {
        source: reading.source,
        link: LinkKey::new(
            LinkStratum::Surface,
            reverse_display.source.clone(),
            reverse_display.target.clone(),
        ),
        from: original_from.clone(),
        to: original_to.clone(),
        assertion_id: format!("generated:{}", generated.claim_id.qualified_key()),
    };

    queue_relation_action(
        &mut ui,
        OpenedLinkEvent::Member {
            target,
            action: "audition".into(),
        },
        0,
    );

    let before = endpoint_preview(&ui, &original_from).unwrap().0;
    let after = endpoint_preview(&ui, &original_to).unwrap().0;
    assert!(matches!(
        ui.audio_requests.as_slice(),
        [woodshed_core::audio::AudioRequest::PreviewPhrase { before: queued_before, after: queued_after, .. }]
            if queued_before == &before && queued_after == &after
    ));
    assert!(ui.personal_relation_intents.is_empty());
}

#[test]
fn selecting_related_inverse_claim_opens_original_directed_pair() {
    let mut ui = UiState::new();
    ui.app_settings.stage.related.graph_scope = woodshed_core::settings::RelatedGraphScope::Mere;
    let snapshot = related_mere_snapshot(&ui);
    let inverse = snapshot
        .relations
        .iter()
        .find(|relation| {
            relation.source != relation.provenance.original_from
                && relation.target == relation.provenance.original_from
                && snapshot.relations.iter().any(|candidate| {
                    candidate.claim_id == relation.claim_id
                        && candidate.source == relation.provenance.original_from
                        && candidate.target == relation.provenance.original_to
                })
        })
        .expect("the related catalog exposes an inverse display alias")
        .clone();
    let expected = (
        inverse.provenance.original_from.clone(),
        inverse.provenance.original_to.clone(),
    );
    let key = LinkKey::new(LinkStratum::Surface, expected.0.clone(), expected.1.clone());
    let set_before = serde_json::to_value(&ui.set).unwrap();

    activate_related_relation(&mut ui, key.stable_id());

    assert_eq!(ui.personal_link_pair, Some(expected));
    assert_eq!(
        ui.related_relation.as_deref(),
        Some(key.stable_id().as_str())
    );
    assert_eq!(serde_json::to_value(&ui.set).unwrap(), set_before);
}

#[path = "tests/claims.rs"]
mod claims;
