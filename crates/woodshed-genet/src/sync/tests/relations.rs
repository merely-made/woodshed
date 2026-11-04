use super::*;

#[test]
fn rejected_personal_relation_save_does_not_publish_candidate_or_advance_identity() {
    let bytes = Arc::new(Mutex::new(BTreeMap::new()));
    let mut storage = Some(SessionStore::new(
        Box::new(RejectingSessionBackend(bytes.clone())) as crate::storage::HostBackend,
    ));
    let mut ui = ready_ui();
    ui.practice_saved = true;
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Sealed {
        persona: "personae:alice".into(),
        protection: "test".into(),
    });
    ui.personal_relations = Some(PersonalRelationSource::empty(
        PersonalRelationSourceId::from_random_uuid_bytes([9; 16]),
    ));
    ui.personal_relation_intents
        .push(woodshed_views::stage::PersonalRelationIntent::Assert {
            expected_source_id: PersonalRelationSourceId::from_random_uuid_bytes([9; 16]),
            expected_revision: 0,
            from: "chord:Major".into(),
            to: "chord:Minor".into(),
            note: Some("one reason".into()),
            at_ms: Some(10),
        });
    let original_revision = ui.personal_relations.as_ref().unwrap().runtime_revision();

    commit_personal_relation_intents(&mut ui, &mut storage);

    let source = ui.personal_relations.as_ref().unwrap();
    assert_eq!(source.runtime_revision(), original_revision);
    assert_eq!(source.retained_assertions().count(), 0);
    assert!(
        ui.personal_relation_notice
            .as_deref()
            .unwrap()
            .contains("not confirmed")
    );
    assert!(bytes.lock().unwrap().is_empty());
}

#[test]
fn foreign_source_personal_intent_is_rejected_before_candidate_or_store_write() {
    let mut storage = Some(SessionStore::new(
        Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend,
    ));
    let mut ui = ready_ui();
    ui.practice_saved = true;
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Sealed {
        persona: "personae:alice".into(),
        protection: "test".into(),
    });
    ui.personal_relations = Some(PersonalRelationSource::empty(
        PersonalRelationSourceId::from_random_uuid_bytes([12; 16]),
    ));
    let original_revision = ui.personal_relations.as_ref().unwrap().runtime_revision();
    ui.personal_relation_intents
        .push(woodshed_views::stage::PersonalRelationIntent::Assert {
            expected_source_id: PersonalRelationSourceId::from_random_uuid_bytes([13; 16]),
            expected_revision: 0,
            from: "chord:Major".into(),
            to: "chord:Minor".into(),
            note: Some("foreign source action".into()),
            at_ms: Some(10),
        });

    commit_personal_relation_intents(&mut ui, &mut storage);

    let source = ui.personal_relations.as_ref().unwrap();
    assert_eq!(source.runtime_revision(), original_revision);
    assert_eq!(source.retained_assertions().count(), 0);
    assert!(storage.as_ref().unwrap().load().is_none());
    assert!(
        ui.personal_relation_notice
            .as_deref()
            .unwrap()
            .contains("different saved source")
    );
}

#[test]
fn uncertain_personal_relation_write_closes_storage_before_any_later_autosave() {
    let bytes = Arc::new(Mutex::new(BTreeMap::new()));
    let writes = Arc::new(AtomicUsize::new(0));
    let backend = WriteThenReadFailureBackend {
        bytes: bytes.clone(),
        writes: writes.clone(),
    };
    let mut storage = Some(SessionStore::new(
        Box::new(backend) as crate::storage::HostBackend
    ));
    let mut ui = ready_ui();
    ui.practice_saved = true;
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Sealed {
        persona: "personae:alice".into(),
        protection: "test".into(),
    });
    ui.personal_relations = Some(PersonalRelationSource::empty(
        PersonalRelationSourceId::from_random_uuid_bytes([11; 16]),
    ));
    ui.personal_relation_intents
        .push(woodshed_views::stage::PersonalRelationIntent::Assert {
            expected_source_id: PersonalRelationSourceId::from_random_uuid_bytes([11; 16]),
            expected_revision: 0,
            from: "chord:Major".into(),
            to: "chord:Minor".into(),
            note: Some("candidate may be on disk".into()),
            at_ms: Some(10),
        });
    let live_revision = ui.personal_relations.as_ref().unwrap().runtime_revision();

    commit_personal_relation_intents(&mut ui, &mut storage);

    assert!(storage.is_none(), "uncertain session storage is revoked");
    assert!(!ui.practice_saved, "the current session is read-only");
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        ui.personal_relations.as_ref().unwrap().runtime_revision(),
        live_revision,
        "the uncertain candidate was not published into live state"
    );
    assert_eq!(
        ui.personal_relations
            .as_ref()
            .unwrap()
            .retained_assertions()
            .count(),
        0
    );
    assert!(
        ui.personal_relation_notice
            .as_deref()
            .unwrap()
            .contains("reopen or switch persona")
    );

    ui.personal_relation_intents
        .push(woodshed_views::stage::PersonalRelationIntent::Assert {
            expected_source_id: PersonalRelationSourceId::from_random_uuid_bytes([11; 16]),
            expected_revision: 0,
            from: "chord:Major".into(),
            to: "chord:Minor".into(),
            note: Some("must not overwrite uncertain slot".into()),
            at_ms: Some(11),
        });
    commit_personal_relation_intents(&mut ui, &mut storage);

    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        ui.personal_relations
            .as_ref()
            .unwrap()
            .retained_assertions()
            .count(),
        0
    );
    let stored = bytes.lock().unwrap();
    let saved = stored
        .get("session")
        .expect("put succeeded before readback failed");
    let candidate =
        woodshed_core::storage::decode_session(std::str::from_utf8(saved).unwrap()).unwrap();
    let persisted = PersonalRelationSource::restore(candidate.session.personal_relations.unwrap());
    assert_eq!(persisted.retained_assertions().count(), 1);
}

#[test]
fn personal_relation_is_published_only_after_session_store_acknowledges_it() {
    let mut storage = Some(SessionStore::new(
        Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend,
    ));
    let mut ui = ready_ui();
    ui.practice_saved = true;
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Sealed {
        persona: "personae:alice".into(),
        protection: "test".into(),
    });
    ui.personal_relations = Some(PersonalRelationSource::empty(
        PersonalRelationSourceId::from_random_uuid_bytes([10; 16]),
    ));
    ui.personal_relation_intents
        .push(woodshed_views::stage::PersonalRelationIntent::Assert {
            expected_source_id: PersonalRelationSourceId::from_random_uuid_bytes([10; 16]),
            expected_revision: 0,
            from: "chord:Major".into(),
            to: "chord:Minor".into(),
            note: Some("one reason".into()),
            at_ms: Some(10),
        });

    commit_personal_relation_intents(&mut ui, &mut storage);

    assert_eq!(
        ui.personal_relations
            .as_ref()
            .unwrap()
            .retained_assertions()
            .count(),
        1
    );
    let json = storage
        .as_ref()
        .unwrap()
        .load()
        .expect("acknowledged session exists");
    let loaded = woodshed_core::storage::decode_session(&json).unwrap();
    let reopened = PersonalRelationSource::restore(
        loaded
            .session
            .personal_relations
            .expect("journal is durable"),
    );
    let rows = reopened.retained_assertions().collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].basis.note.as_deref(), Some("one reason"));
    assert_eq!(rows[0].span.first_asserted_ms, Some(10));
    assert_eq!(rows[0].from, "chord:Major");
    assert_eq!(rows[0].to, "chord:Minor");
}
