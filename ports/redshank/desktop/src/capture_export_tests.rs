//! Native-host ownership exercised with a device-free sample feed and real I/O.
use super::*;
use std::fs;
use std::path::Path;

fn anchor(item: &str) -> CaptureAnchor {
    CaptureAnchor {
        fingerprint: None,
        item_id: ItemId(item.into()),
        offset_ms: 1_234,
        end_offset_ms: None,
        pressed_offset_ms: None,
        representation: Default::default(),
    }
}

fn capture(desktop: &mut Desktop, item: &str, max_frames: usize) {
    desktop.voice_capture.begin_test_capture(10, max_frames);
    desktop.voice_session = Some(VoiceSession {
        anchor: anchor(item),
        resume_playback: false,
        note_id: None,
    });
}

fn poll_until(
    desktop: &mut Desktop,
    state: &mut RedshankSurfaceState,
    done: impl Fn(&Desktop) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        desktop.poll(state);
        if done(desktop) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "product work did not settle: {:?}",
            state.notice
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn block_store(desktop: &mut Desktop, root: &Path) -> PathBuf {
    let blocked = root.join("blocked-model-store");
    fs::write(&blocked, b"blocked model directory").unwrap();
    desktop.persistence = Persistence::start(JsonDirectoryStore::new(&blocked));
    blocked
}

#[test]
fn capture_limit_auto_saves_exact_accepted_prefix_and_reopens_original_target() {
    let directory = tempfile::tempdir().unwrap();
    let mut desktop = tests::desktop(directory.path());
    let mut state = RedshankSurfaceState::default();
    capture(&mut desktop, "a", 20);
    desktop.voice_capture.test_input(&[0.5; 10], 1);
    assert_eq!(desktop.host_facts().recording.unwrap().elapsed_ms, 1_000);
    desktop.voice_capture.test_input(&[0.5; 20], 1);
    desktop.poll(&mut state);
    assert!(!state.compact.voice_capture_active);
    assert_eq!(state.compact.recording.as_ref().unwrap().elapsed_ms, 2_000);
    assert!(desktop.voice_capture.progress().unwrap().stopped);
    assert!(state.notice.as_deref().unwrap().contains("limit"));
    let id = desktop
        .voice_session
        .as_ref()
        .unwrap()
        .note_id
        .clone()
        .unwrap();
    desktop.finish_voice_note(&mut state).unwrap();
    assert_eq!(
        desktop.session.model.annotations.len(),
        1,
        "release/retry must not duplicate admission"
    );
    assert_eq!(
        desktop.voice_session.as_ref().unwrap().note_id,
        Some(id.clone())
    );
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.voice_session.is_none()
    });
    assert!(desktop.voice_capture.progress().is_none());
    assert!(state.notice.as_deref().unwrap().contains("limit"));
    let saved_notice = state.notice.clone();
    desktop.poll(&mut state);
    assert_eq!(
        state.notice, saved_notice,
        "limit outcome survives ordinary projection"
    );
    desktop.finish_voice_note(&mut state).unwrap();
    assert_eq!(
        desktop.session.model.annotations.len(),
        1,
        "late release is idempotent after durable acknowledgement"
    );
    let reopened = JsonDirectoryStore::new(directory.path())
        .load()
        .unwrap()
        .unwrap();
    let note = &reopened.annotations[&id];
    assert_eq!(note.target, anchor("a"));
    let NoteBody::Audio {
        blob_id,
        duration_ms,
        ..
    } = &note.body
    else {
        panic!("audio note")
    };
    assert_eq!(*duration_ms, 2_000);
    let mut wav =
        hound::WavReader::open(LocalVoiceCapture::blob_path(directory.path(), blob_id).unwrap())
            .unwrap();
    assert_eq!(wav.spec().sample_rate, 10);
    assert_eq!(wav.duration(), 20);
    assert_eq!(
        wav.samples::<i16>().map(Result::unwrap).collect::<Vec<_>>(),
        vec![16384; 20]
    );
}

#[test]
fn cap_blob_failure_and_close_keep_prefix_and_target_until_retry_succeeds() {
    let directory = tempfile::tempdir().unwrap();
    let voice_root = directory.path().join("voice");
    fs::write(&voice_root, b"blocked voice directory").unwrap();
    let mut desktop = tests::desktop(directory.path());
    let mut state = RedshankSurfaceState::default();
    capture(&mut desktop, "a", 20);
    desktop.voice_capture.test_input(&[0.25; 30], 1);
    desktop.poll(&mut state);
    assert!(desktop.voice_capture.progress().unwrap().stopped);
    assert_eq!(desktop.voice_capture.progress().unwrap().frames, 20);
    assert_eq!(desktop.voice_session.as_ref().unwrap().anchor, anchor("a"));
    assert!(desktop.session.model.annotations.is_empty());
    assert!(state.notice.as_deref().unwrap().contains("limit"));
    assert!(state.notice.as_deref().unwrap().contains("retained"));
    assert!(desktop.close(&mut state).is_err());
    assert!(!desktop.ready_to_close());
    assert_eq!(desktop.voice_capture.progress().unwrap().frames, 20);
    fs::remove_file(voice_root).unwrap();
    desktop.finish_voice_note(&mut state).unwrap();
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.voice_session.is_none()
    });
    assert_eq!(desktop.session.model.annotations.len(), 1);
    assert_eq!(
        desktop
            .session
            .model
            .annotations
            .values()
            .next()
            .unwrap()
            .target,
        anchor("a")
    );
}

#[test]
fn model_admission_failure_keeps_audio_and_frozen_target_for_explicit_retry() {
    let directory = tempfile::tempdir().unwrap();
    let mut desktop = tests::desktop(directory.path());
    let mut state = RedshankSurfaceState::default();
    capture(&mut desktop, "missing", 20);
    desktop.voice_capture.test_input(&[0.25; 10], 1);
    assert!(
        desktop
            .finish_voice_note(&mut state)
            .unwrap_err()
            .contains("retained")
    );
    assert_eq!(desktop.voice_capture.progress().unwrap().frames, 10);
    assert_eq!(
        desktop.voice_session.as_ref().unwrap().anchor,
        anchor("missing")
    );
    assert!(desktop.session.model.annotations.is_empty());
    desktop
        .session
        .model
        .add_item(LibraryItem::LocalAudio {
            id: ItemId("missing".into()),
            title: "Restored target".into(),
            source: MediaSource::Local {
                path: "restored.wav".into(),
            },
        })
        .unwrap();
    desktop.finish_voice_note(&mut state).unwrap();
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.voice_session.is_none()
    });
    assert_eq!(desktop.session.model.annotations.len(), 1);
    assert_eq!(
        desktop
            .session
            .model
            .annotations
            .values()
            .next()
            .unwrap()
            .target,
        anchor("missing")
    );
}

#[test]
fn failed_durable_save_retains_same_note_and_pcm_across_close_and_retry() {
    let directory = tempfile::tempdir().unwrap();
    let mut desktop = tests::desktop(directory.path());
    let blocked = block_store(&mut desktop, directory.path());
    let mut state = RedshankSurfaceState::default();
    capture(&mut desktop, "a", 20);
    desktop.voice_capture.test_input(&[0.25; 30], 1);
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.persistence.failed
    });
    let id = desktop
        .voice_session
        .as_ref()
        .unwrap()
        .note_id
        .clone()
        .unwrap();
    assert_eq!(desktop.voice_capture.progress().unwrap().frames, 20);
    assert!(state.notice.as_deref().unwrap().contains("limit"));
    assert!(state.notice.as_deref().unwrap().contains("retained"));
    desktop.close(&mut state).unwrap();
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.persistence.failed
    });
    assert!(!desktop.ready_to_close());
    assert_eq!(desktop.session.model.annotations.len(), 1);
    assert_eq!(
        desktop.voice_session.as_ref().unwrap().note_id,
        Some(id.clone())
    );
    fs::remove_file(&blocked).unwrap();
    desktop.finish_voice_note(&mut state).unwrap();
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.voice_session.is_none()
    });
    assert_eq!(desktop.session.model.annotations.len(), 1);
    assert!(
        JsonDirectoryStore::new(blocked)
            .load()
            .unwrap()
            .unwrap()
            .annotations
            .contains_key(&id)
    );
    assert!(desktop.voice_capture.progress().is_none());
}

#[test]
fn explicit_cancel_discards_retained_capture_and_publishes_pending_note_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let mut desktop = tests::desktop(directory.path());
    let blocked = block_store(&mut desktop, directory.path());
    let mut state = RedshankSurfaceState::default();
    capture(&mut desktop, "a", 20);
    desktop.voice_capture.test_input(&[0.25; 30], 1);
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.persistence.failed
    });
    assert_eq!(desktop.session.model.annotations.len(), 1);
    fs::remove_file(&blocked).unwrap();
    desktop.cancel_voice_note(&mut state).unwrap();
    assert!(desktop.voice_session.is_none() && desktop.voice_capture.progress().is_none());
    assert!(desktop.session.model.annotations.is_empty());
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.persistence.durable == desktop.persistence.revision
            && desktop.voice_removals.is_empty()
            && desktop.voice_removals_in_flight == 0
    });
    assert!(
        JsonDirectoryStore::new(blocked)
            .load()
            .unwrap()
            .unwrap()
            .annotations
            .is_empty()
    );
}

#[test]
fn input_failure_stops_acquisition_but_retains_prefix_for_finish_and_unwinds_controls_once() {
    let directory = tempfile::tempdir().unwrap();
    let mut desktop = tests::desktop(directory.path());
    let mut state = RedshankSurfaceState::default();
    capture(&mut desktop, "a", 20);
    desktop.voice_session.as_mut().unwrap().resume_playback = true;
    desktop.session.selected = Some(ItemId("a".into()));
    desktop.duck().unwrap();
    desktop.voice_capture.test_input(&[0.25; 10], 1);
    desktop
        .voice_capture
        .test_error("fixture input disconnected");
    desktop.poll(&mut state);
    assert!(desktop.voice_capture.progress().unwrap().stopped);
    assert_eq!(desktop.host_facts().recording.unwrap().elapsed_ms, 1_000);
    assert!(!state.compact.voice_capture_active && !desktop.ducked);
    assert!(!desktop.voice_session.as_ref().unwrap().resume_playback);
    assert!(desktop.session.model.annotations.is_empty());
    assert!(state.notice.as_deref().unwrap().contains("Input stopped"));
    desktop.poll(&mut state);
    assert!(!desktop.voice_session.as_ref().unwrap().resume_playback);
    desktop.finish_voice_note(&mut state).unwrap();
    poll_until(&mut desktop, &mut state, |desktop| {
        desktop.voice_session.is_none()
    });
    assert_eq!(desktop.session.model.annotations.len(), 1);
}

#[test]
fn annotation_export_worker_keeps_colliding_item_json_separate_and_atomically_replaces_same_item() {
    let directory = tempfile::tempdir().unwrap();
    let mut desktop = tests::desktop(directory.path());
    let mut state = RedshankSurfaceState::default();
    let first = ItemId("local:/takes/a/b.wav".into());
    let second = ItemId("local:/takes/a-b.wav".into());
    for (index, id) in [&first, &second].into_iter().enumerate() {
        desktop
            .session
            .model
            .add_item(LibraryItem::LocalAudio {
                id: id.clone(),
                title: format!("Export {index}"),
                source: MediaSource::Local { path: id.0.clone() },
            })
            .unwrap();
        desktop
            .session
            .model
            .add_text_annotation(
                AnnotationId(format!("note-{index}")),
                anchor(&id.0),
                format!("Independent note {index}"),
                1,
            )
            .unwrap();
        desktop
            .session
            .model
            .set_annotation_privacy(
                &AnnotationId(format!("note-{index}")),
                redshank_model::NotePrivacy::Shareable,
            )
            .unwrap();
        assert_eq!(
            desktop.session.model.export_annotations(id)["items"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "fixture notes must be explicitly shareable before exercising export I/O"
        );
        desktop.session.selected = Some(id.clone());
        desktop.export_annotations(&mut state).unwrap();
        poll_until(&mut desktop, &mut state, |desktop| !desktop.export_pending);
    }
    let first_path = directory
        .path()
        .join("exports")
        .join(format!("{}.json", export_stem(&first)));
    let second_path = directory
        .path()
        .join("exports")
        .join(format!("{}.json", export_stem(&second)));
    let second_bytes = fs::read(&second_path).unwrap();
    assert_ne!(first_path, second_path);
    assert_eq!(
        fs::read(&first_path).unwrap(),
        desktop
            .session
            .model
            .export_annotations_json(&first)
            .as_bytes()
    );
    assert_eq!(
        second_bytes,
        desktop
            .session
            .model
            .export_annotations_json(&second)
            .as_bytes()
    );
    assert_ne!(fs::read(&first_path).unwrap(), second_bytes);
    desktop
        .session
        .model
        .update_text_annotation(&AnnotationId("note-0".into()), "Revised first note".into())
        .unwrap();
    desktop.session.selected = Some(first.clone());
    desktop.export_annotations(&mut state).unwrap();
    poll_until(&mut desktop, &mut state, |desktop| !desktop.export_pending);
    assert_eq!(
        fs::read(first_path).unwrap(),
        desktop
            .session
            .model
            .export_annotations_json(&first)
            .as_bytes()
    );
    assert_eq!(fs::read(second_path).unwrap(), second_bytes);
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        2
    );
}
