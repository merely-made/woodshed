use super::*;
use std::{marker::PhantomData, rc::Rc, sync::atomic::AtomicUsize, thread::ThreadId};

struct StreamOwner {
    constructed: ThreadId,
    dropped: mpsc::Sender<(ThreadId, ThreadId)>,
    _not_send: PhantomData<Rc<()>>,
}
impl Drop for StreamOwner {
    fn drop(&mut self) {
        let _ = self.dropped.send((self.constructed, std::thread::current().id()));
    }
}

struct Trial {
    audio: CpalBackend,
    entered: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
    opened: Arc<AtomicUsize>,
    dropped: mpsc::Receiver<(ThreadId, ThreadId)>,
}
fn plan(owner: u64) -> LoopPlan {
    let set = woodshed_core::working_sets::WorkingSetId(owner);
    LoopPlan { owner: set, name: "Worker input trial".into(), one_shot: true, click: false,
        segments: (0..3).map(|index| woodshed_core::loop_plan::LoopSegment {
            id: LoopSegmentId { set, card: woodshedding::rehearsal::CardId(index + 7) },
            label: format!("Card {index}"), bpm: 6000.0, beats: 1, bars: 1, seconds: None,
            pitches_hz: vec![220.0], phrase: None,
        }).collect() }
}
fn trial(rate: u32) -> Trial {
    let mut audio = CpalBackend::offline_for_test();
    audio.input_factory = None; // Exercise the real worker, never legacy synchronous injection.
    let (entered, began) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (retired, dropped) = mpsc::channel();
    let opened = Arc::new(AtomicUsize::new(0));
    let count = opened.clone();
    audio.input_admission.opener = Some(Box::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        entered.send(()).unwrap();
        gate.recv().map_err(|e| e.to_string())?;
        InputSession::open_with(|mut builder| {
            builder.prepare_analyzers(rate);
            Ok(InputStream::TestOwner { _owner: Box::new(StreamOwner {
                constructed: std::thread::current().id(), dropped: retired.clone(), _not_send: PhantomData,
            }) })
        })
    }));
    audio.set_loop_plan(&plan(1)).unwrap();
    audio.process_for_test(1);
    Trial { audio, entered: began, release, opened, dropped }
}
fn settle_worker(audio: &mut CpalBackend) {
    let limit = Instant::now() + Duration::from_secs(2);
    while audio.input_admission.in_flight {
        audio.service();
        assert!(Instant::now() < limit, "fake worker completion not serviced");
        std::thread::yield_now();
    }
}

#[test]
fn blocked_native_opener_does_not_block_service_and_reserves_original_card() {
    let mut t = trial(48_000);
    t.audio.service();
    assert_eq!(t.opened.load(Ordering::SeqCst), 0, "no opening before explicit request");
    let selected = plan(1).segments[1].id;
    t.audio.loop_arm_record(selected).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    // The opener remains blocked until this test releases it. All these UI
    // operations complete before release; no timing heuristic proves safety.
    for _ in 0..10 { t.audio.service(); }
    assert!(t.audio.needs_service());
    let pending = t.audio.recorder_observation(None);
    assert!(pending.capture_busy && pending.capture_armed);
    assert_eq!(pending.capture_target, Some(1));
    assert!(pending.capture_epoch.is_some());
    assert!(pending.input_rate.is_none() && pending.recording_bar.is_none());
    assert!(t.audio.export_backing().is_err());
    assert!(t.audio.loop_clear(selected).is_err());
    assert!(t.audio.set_loop_plan(&plan(2)).is_err());
    assert!(t.audio.try_set_song(&SongDoc::default()).is_err());
    // Inspection/transport can move before input admission. The reserved Card
    // must still arm explicitly rather than reading the later output cursor.
    t.audio.output.as_mut().unwrap().queue(woodshed_audio::PendingChange::SeekTo { bar_idx: 2 }).unwrap();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    let admitted = t.audio.recorder_observation(None);
    assert!(admitted.capture_busy);
    assert_eq!(admitted.capture_target, Some(1));
    assert_eq!(admitted.capture_epoch, pending.capture_epoch);
    assert_eq!(admitted.input_rate, Some(48_000));
    t.audio.set_song_transport(true);
    t.audio.process_for_test(481);
    assert_eq!(t.audio.recorder_observation(None).recording_bar, Some(1));
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn stop_epoch_cancels_pending_record_and_old_stop_cannot_cancel_new_pending_card() {
    let mut t = trial(48_000);
    let p = plan(1);
    t.audio.loop_arm_record(p.segments[1].id).unwrap();
    let old_epoch = t.audio.recorder_capture_epoch;
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.audio.recorder_stop_capture(1, old_epoch).unwrap();
    t.audio.process_for_test(1);
    assert!(!t.audio.loop_capture_busy());
    t.audio.loop_arm_record(p.segments[2].id).unwrap();
    let new_epoch = t.audio.recorder_capture_epoch;
    assert_ne!(new_epoch, old_epoch);
    assert!(t.audio.recorder_stop_capture(1, old_epoch).is_err());
    assert!(t.audio.recorder_stop_capture(2, old_epoch).is_err());
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    let observed = t.audio.recorder_observation(None);
    assert_eq!(observed.capture_target, Some(2));
    assert_eq!(observed.capture_epoch, Some(new_epoch));
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn reset_and_replaced_source_cannot_resurrect_late_record_and_input_is_reused() {
    let mut t = trial(48_000);
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.audio.clear_loop_session();
    t.audio.process_for_test(1);
    t.audio.set_loop_plan(&plan(2)).unwrap();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert!(!t.audio.loop_capture_busy());
    assert!(!t.audio.output_snapshot().unwrap().recording);
    assert!(!t.audio.input.as_ref().unwrap().capture.is_enabled());
    t.audio.loop_arm_record(plan(2).segments[0].id).unwrap();
    assert_eq!(t.audio.recorder_observation(None).capture_target, Some(0));
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn completion_revalidates_source_backing_revision_and_output_identity() {
    for changed in 0..3 {
        let mut t = trial(48_000);
        t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
        t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
        // Public replacement is guarded; inject changes to prove the completion
        // seam independently refuses stale identity rather than relying on it.
        match changed {
            0 => t.audio.captures.plan = Some(plan(2)),
            1 => t.audio.backing_revision += 1,
            _ => t.audio.output = Some(OutputRuntime::offline(48_000).unwrap()),
        }
        t.release.send(()).unwrap();
        settle_worker(&mut t.audio);
        assert!(!t.audio.loop_capture_busy());
        assert!(t.audio.error().unwrap().contains("changed"));
        assert!(!t.audio.input.as_ref().unwrap().capture.is_enabled());
    }
}

#[test]
fn timeout_is_visible_bounded_and_late_success_never_arms_or_spawns_a_second_worker() {
    let mut t = trial(48_000);
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.audio.calibration_start();
    t.audio.input_admission.deadline = Some(Instant::now());
    t.audio.service();
    assert!(!t.audio.loop_capture_busy());
    assert!(!t.audio.pending_calibration);
    assert!(t.audio.error().unwrap().contains("timed out"));
    assert!(t.audio.loop_arm_record(plan(1).segments[2].id).is_err());
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert!(t.audio.input.is_none(), "expired completion is retired, not admitted");
    assert!(!t.audio.loop_capture_busy());
    assert!(!t.audio.output_snapshot().unwrap().click_playing);
    // Explicit retry retrieves the worker's retained session, without reopening.
    t.audio.loop_arm_record(plan(1).segments[2].id).unwrap();
    settle_worker(&mut t.audio);
    assert_eq!(t.audio.recorder_observation(None).capture_target, Some(2));
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn worker_rate_refusal_keeps_backing_and_output_error() {
    let mut t = trial(44_100);
    t.audio.initial_error = Some("audio output: prior failure".into());
    let original = t.audio.captures.plan.clone();
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert!(!t.audio.loop_capture_busy());
    assert_eq!(t.audio.captures.plan, original);
    let error = t.audio.error().unwrap();
    assert!(error.contains("prior failure") && error.contains("44100"));
    assert!(!t.audio.input.as_ref().unwrap().capture.is_enabled());
}

#[test]
fn late_ready_rechecks_native_output_without_treating_historical_error_as_device_loss() {
    for lost_output in [false, true] {
        let mut t = trial(48_000);
        let original_plan = t.audio.captures.plan.clone();
        let output_id = t.audio.output_observation_id();
        t.audio.initial_error = Some("audio output: historical diagnostic".into());
        t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
        t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
        // Offline runtime has zero native streams. Marking the backend native
        // models device loss during opening without changing runtime identity.
        t.audio.native_output = lost_output;
        assert_eq!(t.audio.output_stream_count(), 0);
        t.release.send(()).unwrap();
        settle_worker(&mut t.audio);
        assert_eq!(t.audio.output_observation_id(), output_id);
        assert_eq!(t.audio.captures.plan, original_plan);
        assert_eq!(t.audio.initial_error.as_deref(), Some("audio output: historical diagnostic"));
        let input = t.audio.input.as_ref().unwrap();
        if lost_output {
            assert!(!t.audio.loop_capture_busy());
            assert!(!t.audio.capture_attached, "no input feed attachment after output loss");
            assert!(!input.capture.is_enabled(), "no capture producer start after output loss");
            let snapshot = t.audio.output_snapshot().unwrap();
            assert!(!snapshot.capture_armed && !snapshot.recording);
            let error = t.audio.error().unwrap();
            assert!(error.contains("historical diagnostic") && error.contains("became unavailable"));
        } else {
            assert!(t.audio.loop_capture_busy(), "historical error alone must not refuse healthy output");
            assert!(t.audio.capture_attached && input.capture.is_enabled());
            assert_eq!(t.audio.recorder_observation(None).capture_target, Some(1));
        }
    }
}

#[test]
fn worker_completion_rechecks_capture_quota_before_enabling_input() {
    let mut t = trial(48_000);
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    // The normal restore seam refuses busy replacement. Inject a full inactive
    // bank to exercise completion's quota check independently of dispatch guards.
    t.audio.captures.restore((1..=256).map(|card| CaptureSnapshot {
        id: LoopSegmentId {
            set: woodshed_core::working_sets::WorkingSetId(9),
            card: woodshedding::rehearsal::CardId(card),
        },
        bpm: 6000.0, beats: 1, bars: 1, seconds: None, legacy_clock: false,
        buffer: woodshed_audio::SampleBuffer::new(vec![0.25; 480], 48_000),
    }).collect()).unwrap();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert!(!t.audio.loop_capture_busy());
    assert!(t.audio.error().unwrap().contains("quota"));
    assert!(!t.audio.input.as_ref().unwrap().capture.is_enabled());
}

#[test]
fn legacy_saved_clear_refuses_pending_input_without_mutating_pcm_or_reservation() {
    let mut t = trial(48_000);
    let mut doc = SongDoc::default();
    doc.click = false;
    doc.bars = vec![Default::default(), Default::default()];
    for bar in &mut doc.bars { bar.bpm = 6000.0; bar.beats = 1; bar.length = 1; }
    t.audio.try_set_song(&doc).unwrap();
    let mut song = t.audio.output.as_ref().unwrap().song();
    let original_pcm = woodshed_audio::SampleBuffer::new(vec![0.25; 480], 48_000);
    song.bars[0].audio_buffer = Some(original_pcm.clone());
    t.audio.output.as_mut().unwrap().set_song(song).unwrap();
    t.audio.process_for_test(1);
    t.audio.recorder_arm_saved(1).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let before = t.audio.recorder_observation(Some(&doc));
    let revision = t.audio.backing_revision;
    t.audio.initial_error = Some("audio output: retained diagnostic".into());
    t.audio.song_clear_loop(0);
    t.audio.service();
    let after = t.audio.recorder_observation(Some(&doc));
    assert!(after.capture_busy && after.capture_armed);
    assert_eq!(after.capture_target, before.capture_target);
    assert_eq!(after.capture_epoch, before.capture_epoch);
    assert!(after.recording_bar.is_none() && after.input_rate.is_none());
    assert_eq!(t.audio.backing_revision, revision);
    let retained = t.audio.output.as_ref().unwrap().song().bars[0].audio_buffer.clone().unwrap();
    assert!(Arc::ptr_eq(&retained.data, &original_pcm.data));
    assert!(retained.data.iter().all(|sample| sample.to_bits() == 0.25_f32.to_bits()));
    let error = t.audio.error().unwrap();
    assert!(error.contains("retained diagnostic") && error.contains("before clearing a saved bar"));
    // The clear refusal did not cancel or retarget the accepted Record.
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    let admitted = t.audio.recorder_observation(Some(&doc));
    assert!(admitted.capture_busy);
    assert_eq!(admitted.capture_target, before.capture_target);
    assert_eq!(admitted.capture_epoch, before.capture_epoch);
    assert!(t.audio.input.as_ref().unwrap().capture.is_enabled());
    let retained = t.audio.output.as_ref().unwrap().song().bars[0].audio_buffer.clone().unwrap();
    assert!(Arc::ptr_eq(&retained.data, &original_pcm.data));
}

#[test]
fn worker_admitted_saved_bar_keeps_follow_traversal_and_reuses_input() {
    let mut t = trial(48_000);
    let mut doc = SongDoc::default();
    doc.click = false;
    doc.one_shot = false;
    doc.bars = vec![Default::default(), Default::default(), Default::default()];
    for bar in &mut doc.bars { bar.bpm = 6000.0; bar.beats = 1; bar.length = 1; }
    t.audio.try_set_song(&doc).unwrap();
    t.audio.process_for_test(1);
    t.audio.recorder_arm_saved(1).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    t.audio.set_song_transport(true);
    t.audio.process_for_test(480 * 2 + 1);
    assert_eq!(t.audio.recorder_observation(Some(&doc)).recording_bar, Some(2));
    t.audio.loop_stop_record();
    t.audio.process_for_test(1);
    t.audio.set_tuner_enabled(true);
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn tuner_latest_desire_and_calibration_cancel_survive_delayed_input_admission() {
    let mut t = trial(48_000);
    t.audio.set_tuner_enabled(true);
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.audio.calibration_start();
    assert!(t.audio.pending_calibration);
    t.audio.set_tuner_enabled(false);
    t.audio.calibration_cancel();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    let input = t.audio.input.as_ref().unwrap();
    assert!(!input.tuner.is_enabled());
    assert!(!input.onset.is_enabled());
    assert!(!t.audio.output_snapshot().unwrap().click_playing);
    t.audio.set_tuner_enabled(true);
    assert!(t.audio.input.as_ref().unwrap().tuner.is_enabled());
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn native_owner_constructs_and_drops_only_on_worker_without_send_or_ui_join() {
    let mut t = trial(48_000);
    let ui_thread = std::thread::current().id();
    t.audio.set_tuner_enabled(true);
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(t.audio); // Must return while the opener remains blocked.
    t.release.send(()).unwrap();
    let (constructed, dropped) = t.dropped.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_ne!(constructed, ui_thread);
    assert_eq!(constructed, dropped);
}

#[test]
fn admitted_handles_wake_service_without_another_user_command() {
    let mut t = trial(48_000);
    let (wake, woke) = mpsc::channel();
    t.audio.input_admission.set_wake(Arc::new(move || { let _ = wake.send(()); }));
    t.audio.set_tuner_enabled(true);
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(t.audio.needs_service());
    t.release.send(()).unwrap();
    woke.recv_timeout(Duration::from_secs(2)).unwrap();
    // The worker's publication wakes the host; that service call admits the
    // latest tuner desire without requiring a second enable/Record action.
    t.audio.service();
    assert!(t.audio.input.as_ref().unwrap().tuner.is_enabled());
    assert!(!t.audio.input_admission_pending());
}
