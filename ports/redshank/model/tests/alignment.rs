use redshank_model::{
    AnnotationId, AudioFingerprint, DerivedPosition, ItemId, LibraryItem, ListenerSettings,
    MediaSource, ModelError, RedshankModel, RepresentationReceipt, TimedTarget,
};

fn fingerprint() -> AudioFingerprint {
    AudioFingerprint {
        version: 1,
        frame_ms: 100,
        anchor_offset_ms: 20_045,
        frames: vec![[17; 16]; 200],
    }
}

fn add_note(model: &mut RedshankModel, item: &str, note: &str) -> AnnotationId {
    let item_id = ItemId(item.into());
    model
        .add_item(LibraryItem::LocalAudio {
            id: item_id.clone(),
            title: item.into(),
            source: MediaSource::Local {
                path: format!("{item}.wav"),
            },
        })
        .unwrap();
    let id = AnnotationId(note.into());
    model
        .add_text_annotation(
            id.clone(),
            TimedTarget {
                item_id,
                offset_ms: 80_045,
                end_offset_ms: Some(90_000),
                pressed_offset_ms: None,
                representation: RepresentationReceipt::default(),
                fingerprint: Some(fingerprint()),
            },
            "original words".into(),
            1,
        )
        .unwrap();
    id
}

fn result(model: &RedshankModel, id: &AnnotationId) -> DerivedPosition {
    DerivedPosition {
        original_target: model.annotations[id].target.clone(),
        destination: RepresentationReceipt {
            complete_digest: Some(format!("blake3:{}", "a".repeat(64))),
            byte_length: Some(100_000),
            ..RepresentationReceipt::default()
        },
        offset_ms: 110_045,
        algorithm_version: 1,
        confidence_per_mille: 980,
        runner_up_per_mille: 700,
        reference_duration_ms: 20_000,
    }
}

#[test]
fn old_notes_and_settings_load_without_alignment_fields() {
    let mut model = RedshankModel::default();
    let id = add_note(&mut model, "one", "n1");
    let mut old = serde_json::to_value(model).unwrap();
    old.as_object_mut().unwrap().remove("derived_positions");
    old["annotations"][&id.0]["target"]
        .as_object_mut()
        .unwrap()
        .remove("fingerprint");
    for field in [
        "alignment_window_ms",
        "alignment_min_confidence_per_mille",
        "alignment_max_search_ms",
    ] {
        old["settings"].as_object_mut().unwrap().remove(field);
    }
    let reopened: RedshankModel = serde_json::from_value(old).unwrap();
    assert!(reopened.annotations[&id].target.fingerprint.is_none());
    assert!(reopened.derived_positions.is_empty());
    assert_eq!(reopened.settings.alignment_window_ms, 20_000);
    assert_eq!(reopened.settings.alignment_min_confidence_per_mille, 940);
    assert_eq!(reopened.settings.alignment_max_search_ms, 21_600_000);
    let serialized = serde_json::to_value(reopened).unwrap();
    assert!(
        serialized["annotations"][&id.0]["target"]
            .get("fingerprint")
            .is_none()
    );
    assert!(serialized.get("derived_positions").is_none());
}

#[test]
fn derived_point_round_trips_without_changing_original_span_or_fingerprint() {
    let mut model = RedshankModel::default();
    let id = add_note(&mut model, "one", "n1");
    let derived = result(&model, &id);
    let original = model.annotations[&id].target.clone();
    model.store_derived_position(&id, derived.clone()).unwrap();
    model
        .update_text_annotation(&id, "edited words".into())
        .unwrap();
    assert_eq!(model.annotations[&id].target, original);
    assert_eq!(model.derived_positions[&id], derived);
    let reopened: RedshankModel =
        serde_json::from_str(&serde_json::to_string(&model).unwrap()).unwrap();
    assert_eq!(reopened, model);
    assert_eq!(reopened.annotations[&id].target.end_offset_ms, Some(90_000));
    assert_eq!(reopened.derived_positions[&id].offset_ms, 110_045);
}

#[test]
fn stale_and_missing_note_results_are_refused() {
    let mut model = RedshankModel::default();
    let id = add_note(&mut model, "one", "n1");
    let derived = result(&model, &id);
    model.annotations.get_mut(&id).unwrap().target.offset_ms += 1;
    assert_eq!(
        model.store_derived_position(&id, derived.clone()),
        Err(ModelError::StaleAlignment(id.clone()))
    );
    model.delete_annotation(&id).unwrap();
    assert_eq!(
        model.store_derived_position(&id, derived),
        Err(ModelError::MissingAnnotation(id))
    );
    assert!(model.derived_positions.is_empty());
}

#[test]
fn restored_alignment_evidence_is_revalidated_without_mutating_the_model() {
    let mut model = RedshankModel::default();
    let id = add_note(&mut model, "one", "n1");
    let valid = result(&model, &id);
    model.store_derived_position(&id, valid).unwrap();
    let before = model.clone();
    assert!(
        model
            .validate_derived_position(&id, &model.derived_positions[&id])
            .is_ok()
    );
    assert_eq!(model, before);
    model
        .derived_positions
        .get_mut(&id)
        .unwrap()
        .algorithm_version = 2;
    let restored: RedshankModel =
        serde_json::from_value(serde_json::to_value(&model).unwrap()).unwrap();
    assert!(matches!(
        restored.validate_derived_position(&id, &restored.derived_positions[&id]),
        Err(ModelError::InvalidAlignment(_))
    ));
    assert_eq!(restored.derived_positions[&id].algorithm_version, 2);
}

#[test]
fn invalid_evidence_does_not_replace_an_existing_position() {
    let mut model = RedshankModel::default();
    let id = add_note(&mut model, "one", "n1");
    let valid = result(&model, &id);
    model.store_derived_position(&id, valid.clone()).unwrap();
    let mut invalids = Vec::new();
    for digest in [
        None,
        Some("blake3:short".into()),
        Some(format!("sha256:{}", "a".repeat(64))),
        Some(format!("blake3:{}", "z".repeat(64))),
    ] {
        let mut invalid = valid.clone();
        invalid.destination.complete_digest = digest;
        invalids.push(invalid);
    }
    for (winner, runner_up) in [
        (1_001, 700),
        (980, 1_001),
        (939, 700),
        (980, 941),
        (980, 990),
    ] {
        let mut invalid = valid.clone();
        invalid.confidence_per_mille = winner;
        invalid.runner_up_per_mille = runner_up;
        invalids.push(invalid);
    }
    let mut invalid = valid.clone();
    invalid.algorithm_version = 2;
    invalids.push(invalid);
    let mut invalid = valid.clone();
    invalid.reference_duration_ms = 19_999;
    invalids.push(invalid);
    for offset in [20_044, model.settings.alignment_max_search_ms + 1] {
        let mut invalid = valid.clone();
        invalid.offset_ms = offset;
        invalids.push(invalid);
    }
    for invalid in invalids {
        assert!(matches!(
            model.store_derived_position(&id, invalid),
            Err(ModelError::InvalidAlignment(_))
        ));
        assert_eq!(model.derived_positions[&id], valid);
    }
}

#[test]
fn old_note_without_fingerprint_cannot_acquire_an_alignment_claim() {
    let mut model = RedshankModel::default();
    let id = add_note(&mut model, "one", "n1");
    model.annotations.get_mut(&id).unwrap().target.fingerprint = None;
    let derived = result(&model, &id);
    assert!(matches!(
        model.store_derived_position(&id, derived),
        Err(ModelError::InvalidAlignment(_))
    ));
}

#[test]
fn deleting_notes_and_items_removes_only_their_derived_positions() {
    let mut model = RedshankModel::default();
    let a = add_note(&mut model, "one", "n1");
    let b = add_note(&mut model, "two", "n2");
    let c = add_note(&mut model, "three", "n3");
    for id in [&a, &b, &c] {
        model
            .store_derived_position(id, result(&model, id))
            .unwrap();
    }
    model.delete_annotation(&a).unwrap();
    model.remove_item(&ItemId("two".into())).unwrap();
    assert_eq!(model.derived_positions.keys().collect::<Vec<_>>(), vec![&c]);
}

#[test]
fn fingerprint_evidence_requires_known_bounded_frames_reaching_the_anchor() {
    let valid = fingerprint();
    assert!(valid.validate().is_ok());
    let mut invalids = Vec::new();
    let mut invalid = valid.clone();
    invalid.version = 2;
    invalids.push(invalid);
    let mut invalid = valid.clone();
    invalid.frame_ms = 200;
    invalids.push(invalid);
    for count in [0, 29, 601] {
        let mut invalid = valid.clone();
        invalid.frames.resize(count, [0; 16]);
        invalids.push(invalid);
    }
    for anchor in [19_999, 20_100] {
        let mut invalid = valid.clone();
        invalid.anchor_offset_ms = anchor;
        invalids.push(invalid);
    }
    for invalid in invalids {
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn alignment_settings_keep_explicit_disable_and_validate_resource_bounds() {
    let mut settings = ListenerSettings::default();
    for window in [0, 5_000, 60_000] {
        settings.alignment_window_ms = window;
        assert!(settings.validate_alignment_settings().is_ok());
    }
    settings.alignment_window_ms = 0;
    let reopened: ListenerSettings =
        serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
    assert_eq!(reopened.alignment_window_ms, 0);
    for window in [1, 4_999, 60_001] {
        settings.alignment_window_ms = window;
        assert!(settings.validate_alignment_settings().is_err());
    }
    settings = ListenerSettings::default();
    for confidence in [799, 991, u16::MAX] {
        settings.alignment_min_confidence_per_mille = confidence;
        assert!(settings.validate_alignment_settings().is_err());
    }
    settings = ListenerSettings::default();
    for duration in [59_999, 86_400_001] {
        settings.alignment_max_search_ms = duration;
        assert!(settings.validate_alignment_settings().is_err());
    }
}
