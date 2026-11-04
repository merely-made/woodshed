use super::*;

#[test]
fn legacy_capture_defaults_and_unavailable_references_survive_session_roundtrip() {
    let mut session = decode_session("{}").unwrap().session;
    assert!(session.loop_captures.is_empty());
    assert!(
        serde_json::to_value(&session)
            .unwrap()
            .get("loop_captures")
            .is_none()
    );
    session
        .loop_captures
        .takes
        .push(capture_ref("a".repeat(64), 3));
    session.loop_captures.namespace = Some(NAMESPACE.into());
    let loaded = decode_session(&serde_json::to_string(&session).unwrap())
        .unwrap()
        .session;
    assert_eq!(loaded.loop_captures, session.loop_captures);
    let store = SessionStore::new(muniment::MemoryBackend::default());
    assert!(
        store
            .try_read_capture(NAMESPACE, &loaded.loop_captures.takes[0])
            .is_err()
    );
    assert_eq!(loaded.loop_captures, session.loop_captures);
    session.loop_captures.version = 99;
    session.loop_captures.takes.clear();
    assert!(
        serde_json::to_value(&session)
            .unwrap()
            .get("loop_captures")
            .is_some()
    );
}

#[test]
fn sealed_companion_roundtrips_exact_pcm_and_wrong_persona_refuses_it() {
    let inner = muniment::MemoryBackend::default();
    let store = SessionStore::new(crate::sealed_backend::SealedBackend::new(
        inner.clone(),
        [1; 32],
    ));
    let samples = [0.0, -0.0, 0.25, -1.5];
    let (digest, frames) = store
        .try_write_capture(NAMESPACE, 48_000, &samples)
        .unwrap();
    let reference = capture_ref(digest.clone(), frames);
    let decoded = store.try_read_capture(NAMESPACE, &reference).unwrap();
    assert_eq!(
        decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        samples.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    let raw = pollster::block_on(inner.get(&pcm_key(NAMESPACE, &digest).unwrap()))
        .unwrap()
        .unwrap();
    assert_ne!(raw, encode_pcm(48_000, &samples).unwrap());
    assert!(decode_pcm(&raw, &reference).is_err());
    store.try_save("{\"take\":\"sealed\"}").unwrap();
    let other = SessionStore::new(crate::sealed_backend::SealedBackend::new(inner, [2; 32]));
    assert!(
        other
            .try_read_capture(NAMESPACE, &reference)
            .unwrap_err()
            .contains("unsealed")
    );
}

#[test]
fn compressed_companion_is_sealed_and_reopens_with_exact_bits() {
    let inner = muniment::MemoryBackend::default();
    let store = SessionStore::new(crate::sealed_backend::SealedBackend::new(
        inner.clone(),
        [19; 32],
    ));
    let samples: Vec<f32> = (0..96_000)
        .map(|index| [0.125, -0.25, -0.0, f32::from_bits(1)][index % 4])
        .collect();
    let (codec, digest, frames) = store
        .try_write_capture_compressed(NAMESPACE, 48_000, &samples)
        .unwrap();
    assert_eq!(codec, LoopCaptureCodec::WavpackF32V1);
    let mut reference = capture_ref(digest.clone(), frames);
    reference.codec = codec;
    let key = pcm_key(NAMESPACE, &digest).unwrap();
    let sealed = pollster::block_on(inner.get(&key)).unwrap().unwrap();
    assert!(sealed.len() < encode_pcm(48_000, &samples).unwrap().len());
    let reopened = SessionStore::new(crate::sealed_backend::SealedBackend::new(
        inner.clone(),
        [19; 32],
    ));
    let decoded = reopened.try_read_capture(NAMESPACE, &reference).unwrap();
    assert!(
        decoded
            .iter()
            .zip(&samples)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    assert!(decode_capture(&sealed, &reference).is_err());
    let other = SessionStore::new(crate::sealed_backend::SealedBackend::new(inner, [20; 32]));
    assert!(other.try_read_capture(NAMESPACE, &reference).is_err());
    assert!(
        reopened
            .try_read_capture("fedcba9876543210fedcba9876543210", &reference)
            .is_err()
    );
    let mut relabelled = reference;
    relabelled.codec = LoopCaptureCodec::RawF32V1;
    assert!(reopened.try_read_capture(NAMESPACE, &relabelled).is_err());
}

#[test]
fn prepared_capture_must_match_its_source_before_writing() {
    let inner = muniment::MemoryBackend::default();
    let store = SessionStore::new(inner.clone());
    let encoded = encode_capture(48_000, &vec![0.25; 96_000]).unwrap();
    assert!(
        store
            .try_write_encoded_capture(NAMESPACE, 44_100, 96_000, &encoded)
            .is_err()
    );
    assert!(
        store
            .try_write_encoded_capture(NAMESPACE, 48_000, 48_000, &encoded)
            .is_err()
    );
    assert!(
        pollster::block_on(inner.list("loop-pcm/"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn settings_failure_survives_successful_session_until_same_slot_verifies_on_a_clone() {
    let dropping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let store = SessionStore::new(FaultBackend {
        inner: muniment::MemoryBackend::default(),
        fault: Fault::DropSettings(dropping.clone()),
    });
    let sibling = store.clone();
    assert!(store.try_save_settings("authored settings").is_err());
    sibling.try_save("authored session").unwrap();
    assert!(store.save_problem().unwrap().contains("settings"));
    assert!(sibling.save_problem().is_some());
    dropping.store(false, std::sync::atomic::Ordering::Relaxed);
    sibling.try_save_settings("authored settings").unwrap();
    assert!(store.save_problem().is_none());
    assert!(sibling.save_problem().is_none());
    assert_eq!(store.load().as_deref(), Some("authored session"));
    assert_eq!(store.load_settings().as_deref(), Some("authored settings"));
}

#[test]
fn dropped_and_corrupt_writes_cannot_claim_durability() {
    for fault in [Fault::Drop, Fault::CorruptCapture] {
        let store = SessionStore::new(FaultBackend {
            inner: muniment::MemoryBackend::default(),
            fault,
        });
        assert!(store.try_write_capture(NAMESPACE, 48_000, &[0.25]).is_err());
        assert!(
            store
                .try_write_capture_compressed(NAMESPACE, 48_000, &vec![0.25; 48_000])
                .is_err()
        );
    }
    let store = SessionStore::new(FaultBackend {
        inner: muniment::MemoryBackend::default(),
        fault: Fault::Drop,
    });
    assert!(store.try_save("changed session").is_err());
}

#[test]
fn refused_session_replacement_retains_previous_reference_and_orphan_blob() {
    let inner = muniment::MemoryBackend::default();
    pollster::block_on(inner.put(SESSION_SLOT, b"previous authored session")).unwrap();
    let store = SessionStore::new(FaultBackend {
        inner: inner.clone(),
        fault: Fault::RefuseSession,
    });
    let (digest, frames) = store.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap();
    assert!(store.try_save("replacement manifest").is_err());
    assert_eq!(store.load().as_deref(), Some("previous authored session"));
    assert_eq!(
        store
            .try_read_capture(NAMESPACE, &capture_ref(digest.clone(), frames))
            .unwrap(),
        vec![0.25]
    );
    assert_eq!(
        pollster::block_on(inner.list("loop-pcm/")).unwrap(),
        vec![pcm_key(NAMESPACE, &digest).unwrap()]
    );
}
