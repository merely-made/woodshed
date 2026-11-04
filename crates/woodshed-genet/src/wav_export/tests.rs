use super::*;
use cambium_genet_winit_host::{Harness, HostHooks, Init};
use std::cell::Cell;
use std::rc::Rc;
use taproot::Selector;

use woodshed_audio::{Bar, SampleBuffer};
use woodshed_core::audio::AudioBackend;

#[test]
#[ignore = "native receipt helper; requires an isolated sealed profile and owned reference directory"]
fn sealed_profile_export_reference() {
    let directory = PathBuf::from(
        std::env::var_os("WOODSHED_EXPORT_REFERENCE_DIR").expect("owned reference directory"),
    );
    assert!(
        std::env::var_os("WOODSHED_DATA_DIR").is_some(),
        "isolated state override required"
    );
    let opened = crate::storage::open_store_as(None);
    let storage = opened.storage;
    let seal = opened.seal;
    let mut ui = UiState::new();
    crate::session::restore(&storage, &mut ui);
    let shared = Shared::with_store(Some(storage), Some(seal), None);
    let mut shared = shared.borrow_mut();
    shared.backend = Some(crate::audio::CpalBackend::offline_for_test());
    crate::capture_persistence::begin_session(&mut shared, &ui).unwrap();
    let plan = ui.requested_loop_plan().unwrap();
    assert!(
        !ui.loop_captures.is_empty(),
        "reference profile requires a completed take"
    );
    shared
        .capture_persistence
        .permit_export(Some(&plan))
        .unwrap();
    let backend = shared.backend.as_mut().unwrap();
    backend.set_loop_plan(&plan).unwrap();
    let (mut frozen, rate) = backend.export_backing().unwrap();
    fs::create_dir_all(&directory).unwrap();
    let full = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("loop-reference.wav"))
        .unwrap();
    let report = export_song_wav_to(&frozen, rate, full).unwrap();
    for bar in &mut frozen.bars {
        bar.audio_buffer = None;
    }
    let backing = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("backing-reference.wav"))
        .unwrap();
    assert_eq!(export_song_wav_to(&frozen, rate, backing).unwrap(), report);
    fs::write(directory.join("reference.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "frames":report.frames,"rate":rate,"channels":report.channels,"click":plan.click,"segments":plan.segments.len(),"captured_takes":ui.loop_captures.takes.len(),
        "capture_manifest":ui.loop_captures
    })).unwrap()).unwrap();
    assert!(backend.capture_source_rate().is_none());
}

fn song(value: f32) -> Song {
    Song {
        bars: vec![Bar {
            bpm: 120.0,
            audio_buffer: Some(SampleBuffer::new(vec![value; 96_000], 48_000)),
            ..Default::default()
        }],
        click_enabled: false,
        ..Default::default()
    }
}

#[test]
fn cancelled_worker_cannot_publish_new_or_replacement_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let gate = Mutex::new(true);
    assert!(
        render(
            None,
            song(0.1),
            48_000,
            path.clone(),
            Protection::default(),
            &gate
        )
        .is_err()
    );
    assert!(!path.exists());
    fs::write(&path, b"keep").unwrap();
    let (artifact, hash) = collision(
        render(
            None,
            song(0.1),
            48_000,
            path.clone(),
            Protection::default(),
            &Mutex::new(false),
        )
        .unwrap(),
    );
    let session = Arc::new(());
    let (started, ready) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let mut exporter = WavExport::default();
    exporter
        .spawn(session, move |cancelled| {
            started.send(()).unwrap();
            blocked.recv().unwrap();
            publish(artifact, Some(hash), &cancelled)
        })
        .unwrap();
    ready.recv().unwrap();
    let mut ui = UiState::new();
    assert!(exporter.take_completion(&ui.audio_session).is_none());
    release.send(()).unwrap();
    let job = exporter.job.take().unwrap();
    assert!(
        job.receiver
            .recv()
            .unwrap()
            .err()
            .unwrap()
            .contains("cancelled")
    );
    assert!(ui.loop_export_notice.take().is_none());
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

fn collision(outcome: Outcome) -> (Artifact, Hash) {
    match outcome {
        Outcome::Collision(artifact, hash) => (artifact, hash),
        _ => panic!("expected collision"),
    }
}

#[test]
fn publication_collision_replace_and_changed_destination_preserve_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    fs::write(&path, b"original").unwrap();
    let (artifact, hash) = collision(
        render(
            None,
            song(0.25),
            48_000,
            path.clone(),
            Protection::default(),
            &Mutex::new(false),
        )
        .unwrap(),
    );
    assert_eq!(fs::read(&path).unwrap(), b"original");
    let rendered = fs::read(artifact.file.path()).unwrap();
    // A later backing edit does not redirect the rendered replacement.
    let changed_backing = song(0.9);
    assert_ne!(
        changed_backing.bars[0].audio_buffer.as_ref().unwrap().data[0],
        0.25
    );
    assert!(matches!(
        publish(artifact, Some(hash), &Mutex::new(false)).unwrap(),
        Outcome::Saved(_, SongExportReport { frames: 96_000, .. })
    ));
    assert_eq!(fs::read(&path).unwrap(), rendered);

    let (artifact, hash) = collision(
        render(
            None,
            song(0.5),
            48_000,
            path.clone(),
            Protection::default(),
            &Mutex::new(false),
        )
        .unwrap(),
    );
    fs::write(&path, b"concurrent edit").unwrap();
    assert!(
        publish(artifact, Some(hash), &Mutex::new(false))
            .err()
            .unwrap()
            .contains("changed")
    );
    assert_eq!(fs::read(&path).unwrap(), b"concurrent edit");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn collision_cancellation_and_invalid_render_leave_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cancel.wav");
    fs::write(&path, b"keep").unwrap();
    let (artifact, _) = collision(
        render(
            None,
            song(0.1),
            48_000,
            path.clone(),
            Protection::default(),
            &Mutex::new(false),
        )
        .unwrap(),
    );
    drop(artifact);
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    let mut bad = song(0.1);
    bad.bars[0].audio_buffer = Some(SampleBuffer::new(vec![f32::NAN; 96_000], 48_000));
    assert!(
        render(
            None,
            bad,
            48_000,
            path.clone(),
            Protection::default(),
            &Mutex::new(false)
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    assert!(
        render(
            None,
            song(0.1),
            48_000,
            dir.path().join("absent").join("take.wav"),
            Protection::default(),
            &Mutex::new(false)
        )
        .is_err()
    );
}

#[test]
fn protects_existing_missing_storage_and_future_capture_destinations() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state.wav");
    let settings = dir.path().join("settings.wav");
    let capture = dir.path().join("state.loop-pcm");
    fs::write(&state, b"sealed session").unwrap();
    let protection = Protection {
        paths: vec![state.clone(), settings.clone()],
        directories: vec![capture.clone()],
    };
    for path in [
        state.clone(),
        settings,
        capture.join("namespace").join("future.wav"),
        dir.path().join("SETTINGS.WAV"),
        dir.path().join("STATE.LOOP-PCM").join("future.wav"),
        dir.path().join("sub").join("..").join("state.wav"),
    ] {
        assert!(protection.check(&path).is_err(), "{}", path.display());
    }
    assert!(protection.check(&dir.path().join("external.wav")).is_ok());
    #[cfg(unix)]
    {
        let alias = dir.path().join("alias.wav");
        std::os::unix::fs::symlink(&state, &alias).unwrap();
        assert!(protection.check(&alias).is_err());
        fs::remove_file(&alias).unwrap();
        fs::hard_link(&state, &alias).unwrap();
        assert!(protection.check(&alias).is_err());
        std::os::unix::fs::symlink(dir.path(), dir.path().join("folder")).unwrap();
        assert!(
            protection
                .check(&dir.path().join("folder").join("state.wav"))
                .is_err()
        );
    }
    assert_eq!(fs::read(&state).unwrap(), b"sealed session");
}

#[test]
fn prior_session_completion_and_replacement_do_not_cross_personas() {
    let mut ui = UiState::new();
    let session = ui.audio_session.clone();
    let (sender, receiver) = mpsc::channel();
    let cancelled = Arc::new(Mutex::new(false));
    let mut exporter = WavExport {
        job: Some(Job {
            session,
            receiver,
            cancelled,
        }),
        ..Default::default()
    };
    let persisted = ui.to_persisted();
    ui.apply_persisted(&persisted, ui.app_settings.clone());
    sender.send(Err("old session failure".into())).unwrap();
    if let Some(result) = exporter.take_completion(&ui.audio_session) {
        exporter.complete(&mut ui, result);
    }
    assert!(ui.loop_export_notice.is_none());
    assert!(!ui.loop_export_busy);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.wav");
    fs::write(&path, b"keep").unwrap();
    let (artifact, fingerprint) = collision(
        render(
            None,
            song(0.1),
            48_000,
            path.clone(),
            Protection::default(),
            &Mutex::new(false),
        )
        .unwrap(),
    );
    exporter.pending = Some((
        Arc::new(()),
        Collision {
            artifact,
            fingerprint,
            token: 1,
        },
    ));
    if let Some(result) = exporter.take_completion(&ui.audio_session) {
        exporter.complete(&mut ui, result);
    }
    assert!(exporter.pending.is_none());
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn snapshot_requires_current_admitted_backing_and_keeps_complete_pcm() {
    let shared = Shared::with_store(None, None, None);
    let mut shared = shared.borrow_mut();
    let mut ui = UiState::new();
    ui.song.bars.push(woodshed_core::song::SongBar::default());
    let mut backend = crate::audio::CpalBackend::offline_for_test();
    backend.try_set_song(&ui.song).unwrap();
    shared.backend = Some(backend);
    shared.last_song = Some(ui.song.clone());
    assert!(snapshot(&mut shared, &ui).unwrap_err().contains("restore"));
    shared.last_audio_session = Some(ui.audio_session.clone());
    assert!(snapshot(&mut shared, &ui).is_ok());
    ui.song.bars[0].bpm += 1.0;
    assert!(snapshot(&mut shared, &ui).unwrap_err().contains("admitted"));
    ui.song = shared.last_song.clone().unwrap();
    for _ in 0..64 {
        shared.backend.as_mut().unwrap().song_rewind();
    }
    shared.backend.as_mut().unwrap().clear_loop_session();
    assert!(snapshot(&mut shared, &ui).is_err());
}

#[path = "tests/deferral.rs"]
mod deferral;
