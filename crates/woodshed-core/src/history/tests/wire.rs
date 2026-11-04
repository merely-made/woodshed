use super::*;

#[test]
fn malformed_history_shapes_never_fall_back_to_empty_legacy() {
    for value in [
        serde_json::json!({"entries": [], "visits": "damaged", "owners": []}),
        serde_json::json!({"entries": [], "events": []}),
        serde_json::json!({"visits": null, "events": []}),
        serde_json::json!({"owners": [], "events": []}),
        serde_json::json!({}),
        serde_json::json!({"next_sequence": 2}),
        serde_json::json!({"events": null}),
        serde_json::json!({"events": [{}]}),
        serde_json::json!(null),
        serde_json::json!([]),
    ] {
        assert!(
            serde_json::from_value::<PracticeHistory>(value.clone()).is_err(),
            "{value}"
        );
    }
    for value in [
        serde_json::json!({"events": [], "next_sequence": 0}),
        serde_json::json!({"entries": [], "visits": [], "owners": []}),
    ] {
        assert_eq!(
            serde_json::from_value::<PracticeHistory>(value)
                .unwrap()
                .len(),
            0
        );
    }
}

fn two_visit_history_wire() -> serde_json::Value {
    let mut history = PracticeHistory::default();
    history.record(
        Some(10),
        "scale:Major",
        EngagementKind::Rehearsed,
        None,
        None,
    );
    history.record(
        Some(20),
        "chord:Major",
        EngagementKind::Completed,
        None,
        Some(50),
    );
    serde_json::to_value(history).unwrap()
}

#[test]
fn invalid_snapshot_references_and_consistency_are_decode_errors() {
    // Exercise every index consumed by the locked reconstruction, plus
    // ambiguous keys, repeated vector identities, and reciprocal links.
    for (pointer, replacement) in [
        ("/visits/0/entry", serde_json::json!(99)),
        ("/visits/1/parent", serde_json::json!(99)),
        ("/visits/0/children", serde_json::json!([99])),
        ("/visits/0/bindings/0/owner", serde_json::json!(99)),
        ("/visits/0/bindings/0/forward_child", serde_json::json!(99)),
        ("/owners/0/origin", serde_json::json!(99)),
        ("/owners/0/current", serde_json::json!(99)),
        ("/owners/0/creator", serde_json::json!(99)),
        ("/owners/0/pending_origin_parent", serde_json::json!(99)),
        ("/owners/0/owned_visits", serde_json::json!([99])),
        ("/visits/0/children", serde_json::json!([1, 1])),
        ("/visits/0/children", serde_json::json!([])),
        ("/visits/1/parent", serde_json::json!(null)),
        ("/visits/0/bindings/0/forward_child", serde_json::json!(0)),
        ("/owners/0/owned_visits", serde_json::json!([0, 0, 1])),
        ("/owners/0/owned_visits", serde_json::json!([])),
        ("/visits/1/bindings", serde_json::json!([])),
        ("/entries/0/visit_count", serde_json::json!(999)),
    ] {
        let mut wire = two_visit_history_wire();
        *wire.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            serde_json::from_value::<PracticeHistory>(wire).is_err(),
            "{pointer}"
        );
    }
    for collection in ["entries", "owners"] {
        let mut wire = two_visit_history_wire();
        let rows = wire[collection].as_array_mut().unwrap();
        rows.push(rows[0].clone());
        assert!(
            serde_json::from_value::<PracticeHistory>(wire).is_err(),
            "{collection}"
        );
    }
    let mut wire = two_visit_history_wire();
    let bindings = wire["visits"][0]["bindings"].as_array_mut().unwrap();
    bindings.push(bindings[0].clone());
    assert!(serde_json::from_value::<PracticeHistory>(wire).is_err());
}

#[test]
fn cyclic_snapshot_ancestry_is_a_decode_error() {
    // Reciprocal links alone cannot detect a disconnected cycle.
    let mut wire = two_visit_history_wire();
    wire["visits"][0]["parent"] = serde_json::json!(1);
    wire["visits"][1]["children"] = serde_json::json!([0]);
    assert!(serde_json::from_value::<PracticeHistory>(wire).is_err());

    let mut wire = two_visit_history_wire();
    wire["owners"][0]["creator"] = serde_json::json!(0);
    assert!(serde_json::from_value::<PracticeHistory>(wire).is_err());
}

#[test]
fn branched_current_history_preserves_retained_visits_and_owner_cursors() {
    let mut lineage = Lineage::default();
    let owner = lineage.ensure_owner(PRACTITIONER.into(), None);
    let entry =
        lineage.resolve_or_create_entry("scale:Major".into(), (), 1, EntryPrivacy::LocalOnly);
    let context = Engagement {
        kind: EngagementKind::Rehearsed,
        from_id: None,
        practiced_ms: None,
        dated: true,
        provenance: None,
    };
    lineage
        .visit_entry(owner, entry, context.clone(), TransitionKind::Unknown, 1)
        .unwrap();
    lineage
        .visit_entry(owner, entry, context.clone(), TransitionKind::Unknown, 2)
        .unwrap();
    lineage.back(owner, 1, 3).unwrap();
    lineage
        .visit_entry(owner, entry, context.clone(), TransitionKind::Unknown, 4)
        .unwrap();
    let child_owner = lineage.ensure_owner("second owner".into(), Some(owner));
    lineage
        .visit_entry(child_owner, entry, context, TransitionKind::Unknown, 5)
        .unwrap();
    // The spawned owner's binding on the parent is not an owned visit.
    let before = lineage.to_snapshot();
    let wire = serde_json::to_string(&PracticeHistory { lineage }).unwrap();
    let back: PracticeHistory = serde_json::from_str(&wire).unwrap();
    let after = back.lineage.to_snapshot();
    // Hash-backed collections have no wire order contract.
    assert_eq!(after.entries, before.entries);
    assert_eq!(after.visits.len(), 4);
    for (old, new) in before.visits.iter().zip(&after.visits) {
        assert_eq!(old.entry, new.entry);
        assert_eq!(old.parent, new.parent);
        assert_eq!(old.children, new.children);
        assert_eq!(old.context, new.context);
        assert_eq!(old.inbound, new.inbound);
        assert_eq!(old.created_at_ms, new.created_at_ms);
        for binding in &old.bindings {
            assert!(new.bindings.contains(binding));
        }
        assert_eq!(old.bindings.len(), new.bindings.len());
    }
    for (old, new) in before.owners.iter().zip(&after.owners) {
        assert_eq!(old.identity, new.identity);
        assert_eq!(old.origin, new.origin);
        assert_eq!(old.current, new.current);
        assert_eq!(old.creator, new.creator);
        assert_eq!(old.pending_origin_parent, new.pending_origin_parent);
        let mut old_owned = old.owned_visits.clone();
        let mut new_owned = new.owned_visits.clone();
        old_owned.sort_unstable();
        new_owned.sort_unstable();
        assert_eq!(old_owned, new_owned);
    }
}

#[test]
fn a_lineage_round_trips_through_the_session_wire() {
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
        EngagementKind::Completed,
        Some("scale:Major".into()),
        Some(30_000),
    );

    let json = serde_json::to_string(&history).unwrap();
    let back: PracticeHistory = serde_json::from_str(&json).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back.total_practiced_ms("chord:Major 7"), 30_000);
    assert_eq!(back.last_seen_ms("chord:Major 7"), Some(2));
    assert_eq!(
        back.related_transition_count("scale:Major", "chord:Major 7"),
        1,
        "the traversal survives the round trip"
    );
}

#[test]
fn the_flat_log_woodshed_used_to_persist_replays_into_the_lineage() {
    // A session written by the previous store, including the pre-clock
    // shape: no `at_ms`, no `practiced_ms`, and a `next_sequence` field the
    // lineage has no use for.
    let flat = r#"{"events":[
            {"subject_id":"scale:Major","kind":"Rehearsed","from_id":null,"sequence":0},
            {"subject_id":"chord:Minor 7","kind":"Staged","from_id":"scale:Major","sequence":1,
             "at_ms":9000,"practiced_ms":null}
        ],"next_sequence":2}"#;
    let history: PracticeHistory = serde_json::from_str(flat).unwrap();

    assert_eq!(history.len(), 2, "both engagements replayed");
    assert!(
        history
            .recent(2)
            .iter()
            .all(|event| event.provenance.is_none()),
        "legacy observations do not invent current occurrence provenance"
    );
    assert!(
        history.transitions().is_empty(),
        "staging cannot become practice through migration"
    );
    assert_eq!(
        history.related_transition_count("scale:Major", "chord:Minor 7"),
        0,
        "historical staging remains interest after migration"
    );
    // The undated one stays unknown rather than becoming 1970.
    assert_eq!(history.last_seen_ms("scale:Major"), None);
    assert_eq!(history.last_seen_ms("chord:Minor 7"), Some(9_000));
    assert!(
        history.has_times(),
        "one dated engagement is enough to rank by age"
    );

    // And it persists as a lineage from here on: no flat log is written back.
    let json = serde_json::to_string(&history).unwrap();
    assert!(!json.contains("next_sequence"));
    assert!(json.contains("visits"));
    let migrated: PracticeHistory = serde_json::from_str(&json).unwrap();
    assert_eq!(migrated.recent(2), history.recent(2));
}
