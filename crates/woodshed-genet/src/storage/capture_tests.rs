use super::*;
use woodshed_core::loop_capture::{LoopCaptureCodec, LoopCaptureRef, encode_pcm, pcm_digest};
use woodshed_core::storage::PersistedSession;
const NAMESPACE: &str = "0123456789abcdef0123456789abcdef";
const OTHER_NAMESPACE: &str = "fedcba9876543210fedcba9876543210";

fn backend(directory: &Path) -> FsBackend {
    FsBackend::at(directory)
}

fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("local filesystem backend unexpectedly awaited external work")
        },
    }
}

fn reference(digest: String, frames: usize) -> LoopCaptureRef {
    LoopCaptureRef {
        seconds: None,
        clock: None,
        id: woodshed_core::loop_plan::LoopSegmentId {
            set: woodshed_core::working_sets::WorkingSetId(3),
            card: woodshedding::rehearsal::CardId(7),
        },
        bpm: 83.75,
        beats: 7,
        bars: 3,
        sample_rate: 48_000,
        frames,
        codec: LoopCaptureCodec::LegacyRawF32V1,
        digest,
    }
}

#[test]
fn filesystem_companion_reopens_exact_pcm_after_session_reference_commit() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::new(backend(directory.path()));
    let samples = [0.0, -0.0, 0.25, -1.5];
    let (digest, frames) = store
        .try_write_capture(NAMESPACE, 48_000, &samples)
        .unwrap();
    let mut session = PersistedSession::default();
    session
        .loop_captures
        .takes
        .push(reference(digest.clone(), frames));
    session.loop_captures.namespace = Some(NAMESPACE.into());
    store
        .try_save(&serde_json::to_string(&session).unwrap())
        .unwrap();
    let reopened = SessionStore::new(backend(directory.path()));
    let persisted = woodshed_core::storage::decode_session(&reopened.load().unwrap())
        .unwrap()
        .session;
    assert_eq!(persisted.loop_captures, session.loop_captures);
    let decoded = reopened
        .try_read_capture(NAMESPACE, &persisted.loop_captures.takes[0])
        .unwrap();
    assert_eq!(
        decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        samples.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(
        std::fs::read(
            directory
                .path()
                .join("loop-pcm")
                .join(NAMESPACE)
                .join(format!("{digest}.bin"))
        )
        .unwrap(),
        encode_pcm(48_000, &samples).unwrap()
    );
    assert_eq!(
        ready(reopened.backend().list("loop-pcm/")).unwrap(),
        vec![pcm_key(NAMESPACE, &digest).unwrap()]
    );
    assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
    reopened.save_settings("first");
    reopened.save_settings("second");
    assert_eq!(reopened.load_settings().as_deref(), Some("second"));
}

#[test]
fn filesystem_rejects_unknown_and_path_shaped_keys_without_touching_session() {
    let directory = tempfile::tempdir().unwrap();
    let backend = backend(directory.path());
    ready(backend.put("session", b"original")).unwrap();
    for key in [
        "unknown".into(),
        "loop-pcm/../practice.json".into(),
        format!("loop-pcm/{}", "A".repeat(64)),
        format!("loop-pcm/{}.bin", "a".repeat(64)),
        format!("loop-pcm/{NAMESPACE}/{}.bin", "a".repeat(64)),
        format!("loop-pcm/{NAMESPACE}/{}/../session", "a".repeat(64)),
    ] {
        assert!(ready(backend.get(&key)).is_err());
        assert!(ready(backend.put(&key, b"overwrite")).is_err());
        assert!(ready(backend.delete(&key)).is_err());
    }
    assert_eq!(ready(backend.get("session")).unwrap().unwrap(), b"original");
}

#[test]
fn filesystem_bounds_reads_and_reports_corrupt_or_truncated_companions() {
    let directory = tempfile::tempdir().unwrap();
    let backend = backend(directory.path());
    let bytes = encode_pcm(48_000, &[0.25]).unwrap();
    let reference = reference(pcm_digest(&bytes), 1);
    let path = backend
        .path(&pcm_key(NAMESPACE, &reference.digest).unwrap())
        .unwrap()
        .unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_CAPTURE_FILE_BYTES as u64 + 1).unwrap();
    let store = SessionStore::new(backend);
    assert!(
        store
            .try_read_capture(NAMESPACE, &reference)
            .unwrap_err()
            .contains("byte budget")
    );
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(store.try_read_capture(NAMESPACE, &reference).is_err());
    let mut corrupt = bytes;
    *corrupt.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &corrupt).unwrap();
    assert!(
        store
            .try_read_capture(NAMESPACE, &reference)
            .unwrap_err()
            .contains("integrity")
    );
    assert!(store.try_write_capture(NAMESPACE, 48_000, &[0.25]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    let (digest, frames) = store
        .try_write_capture(OTHER_NAMESPACE, 48_000, &[0.25])
        .unwrap();
    assert_eq!(
        store
            .try_read_capture(OTHER_NAMESPACE, &self::reference(digest, frames))
            .unwrap(),
        vec![0.25]
    );
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}

#[test]
fn failed_companion_write_retains_previous_session_and_failed_rename_cleans_temp() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::new(backend(directory.path()));
    store.try_save("previous session").unwrap();
    std::fs::write(directory.path().join("loop-pcm"), b"directory collision").unwrap();
    assert!(store.try_write_capture(NAMESPACE, 48_000, &[0.25]).is_err());
    assert_eq!(store.load().as_deref(), Some("previous session"));
    let occupied = directory.path().join("occupied.json");
    std::fs::create_dir(&occupied).unwrap();
    std::fs::write(occupied.join("previous"), b"retained").unwrap();
    assert!(atomic_write(&occupied, b"replacement", false).is_err());
    assert_eq!(
        std::fs::read(occupied.join("previous")).unwrap(),
        b"retained"
    );
    assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}

#[test]
fn sealed_filesystem_blobs_and_pending_write_drop_keep_custody_semantics() {
    let directory = tempfile::tempdir().unwrap();
    let store = SessionStore::new(SealedBackend::new(backend(directory.path()), [3; 32]));
    let (digest, frames) = store.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap();
    assert_eq!(
        store
            .try_read_capture(NAMESPACE, &reference(digest.clone(), frames))
            .unwrap(),
        vec![0.25]
    );
    let raw = std::fs::read(
        directory
            .path()
            .join("loop-pcm")
            .join(NAMESPACE)
            .join(format!("{digest}.bin")),
    )
    .unwrap();
    assert_eq!(raw.len(), encode_pcm(48_000, &[0.25]).unwrap().len() + 40);
    assert_ne!(&raw[..8], b"WSHPCM\0\0");
    let pending = SessionStore::new(PendingBackend::new());
    assert!(
        pending
            .try_write_capture(NAMESPACE, 48_000, &[0.5])
            .is_err()
    );
    assert!(pending.try_save("must remain unsaved").is_err());
    assert!(!directory.path().join("session").exists());
    assert_eq!(
        ready(pending.backend().list("loop-pcm/")).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn two_personas_share_pcm_only_through_distinct_artifacts_and_cannot_clobber_ciphertext() {
    let directory = tempfile::tempdir().unwrap();
    let first = SessionStore::new(SealedBackend::new(backend(directory.path()), [1; 32]));
    let second = SessionStore::new(SealedBackend::new(backend(directory.path()), [2; 32]));
    let (digest, frames) = first.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap();
    let path = directory
        .path()
        .join("loop-pcm")
        .join(NAMESPACE)
        .join(format!("{digest}.bin"));
    let original = std::fs::read(&path).unwrap();
    assert_eq!(
        first.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap(),
        (digest.clone(), frames)
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "identical readable PCM reuses its existing sealed blob"
    );
    assert!(
        second
            .try_write_capture(NAMESPACE, 48_000, &[0.25])
            .is_err()
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "inaccessible ciphertext is never overwritten"
    );
    let other = second
        .try_write_capture(OTHER_NAMESPACE, 48_000, &[0.25])
        .unwrap();
    assert_eq!(other, (digest.clone(), frames));
    let reference = reference(digest.clone(), frames);
    assert_eq!(
        first.try_read_capture(NAMESPACE, &reference).unwrap(),
        vec![0.25]
    );
    assert_eq!(
        second
            .try_read_capture(OTHER_NAMESPACE, &reference)
            .unwrap(),
        vec![0.25]
    );
    assert!(second.try_read_capture(NAMESPACE, &reference).is_err());
    assert!(first.try_read_capture(OTHER_NAMESPACE, &reference).is_err());
    // Even a directly addressed write must preserve the immutable blob.
    assert!(
        ready(backend(directory.path()).put(
            &pcm_key(NAMESPACE, &digest).unwrap(),
            b"corrupt replacement"
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
