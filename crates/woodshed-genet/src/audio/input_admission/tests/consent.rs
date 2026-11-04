use super::*;

#[test]
fn explicit_consent_wait_excludes_more_than_ten_seconds_and_never_opens_hal_before_grant() {
    let (mut t, prompted, answer) = permission_trial();
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    prompted.recv_timeout(Duration::from_secs(2)).unwrap();
    // Virtual elapsed time, no sleep or native prompt. The queued waiting event
    // was published before the initial deadline, even though UI polling is late.
    assert!(
        t.audio
            .input_admission
            .poll(Instant::now() + Duration::from_secs(60))
            .is_none()
    );
    assert!(t.audio.input_admission.awaiting_permission());
    assert!(t.audio.input_admission.deadline.is_none());
    assert!(t.audio.loop_capture_busy() && t.audio.needs_service());
    assert_eq!(t.opened.load(Ordering::SeqCst), 0);
    assert!(matches!(
        t.entered.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    let pending = t.audio.recorder_observation(None);
    assert!(pending.input_permission_pending);
    let granted_after = Instant::now();
    answer.send(Ok(())).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let hardware_started_before = Instant::now();
    t.audio.service();
    assert!(!t.audio.input_admission.awaiting_permission());
    let deadline = t.audio.input_admission.deadline.unwrap();
    assert!(deadline >= granted_after + OPEN_TIMEOUT);
    assert!(deadline <= hardware_started_before + OPEN_TIMEOUT);
    assert!(
        t.audio.loop_capture_busy(),
        "grant alone does not finish hardware opening"
    );
    let opening = t.audio.recorder_observation(None);
    assert!(!opening.input_permission_pending);
    assert_eq!(opening.capture_epoch, pending.capture_epoch);
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert_eq!(t.audio.recorder_observation(None).capture_target, Some(1));
    assert_eq!(
        t.audio.recorder_observation(None).capture_epoch,
        pending.capture_epoch
    );
    assert!(t.audio.input.as_ref().unwrap().capture.is_enabled());
    assert_eq!(t.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn denied_or_restricted_authorization_clears_intent_without_native_open() {
    for refusal in [
        "Microphone permission denied",
        "Microphone access restricted",
    ] {
        let (mut t, prompted, answer) = permission_trial();
        t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
        prompted.recv_timeout(Duration::from_secs(2)).unwrap();
        t.audio.calibration_start();
        answer.send(Err(refusal.into())).unwrap();
        settle_worker(&mut t.audio);
        assert!(!t.audio.loop_capture_busy());
        assert!(!t.audio.pending_calibration);
        assert!(!t.audio.input_admission.awaiting_permission());
        assert!(t.audio.input.is_none());
        assert!(t.audio.error().unwrap().contains(refusal));
        assert_eq!(t.opened.load(Ordering::SeqCst), 0);
        assert_eq!(t.authorized.load(Ordering::SeqCst), 1);
        assert!(matches!(
            t.entered.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }
}

#[test]
fn cancelled_record_cannot_resurrect_after_late_permission_grant() {
    let (mut t, prompted, answer) = permission_trial();
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    prompted.recv_timeout(Duration::from_secs(2)).unwrap();
    t.audio.service();
    assert!(t.audio.input_admission.awaiting_permission());
    let epoch = t.audio.recorder_capture_epoch;
    t.audio.recorder_stop_capture(1, epoch).unwrap();
    t.audio.process_for_test(1);
    assert!(!t.audio.loop_capture_busy());
    assert!(t.audio.pending_capture.is_none());
    let cancelled = t.audio.recorder_observation(None);
    assert!(
        cancelled.input_permission_pending,
        "OS consent remains pending after Record cancellation"
    );
    assert!(!cancelled.capture_busy && !cancelled.capture_armed);
    assert!(cancelled.capture_target.is_none() && cancelled.capture_epoch.is_none());
    answer.send(Ok(())).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    t.release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert!(!t.audio.loop_capture_busy());
    assert!(t.audio.recorder_observation(None).capture_target.is_none());
    let output = t.audio.output_snapshot().unwrap();
    assert!(!output.recording && !output.capture_armed);
    assert!(!t.audio.input.as_ref().unwrap().capture.is_enabled());
    assert!(!t.audio.capture_attached);
}

#[test]
fn checking_authorization_without_explicit_consent_wait_remains_bounded() {
    let mut t = trial(48_000);
    let (checking, checked) = mpsc::channel();
    let (release, released) = mpsc::channel();
    t.audio.input_admission.authorizer = Some(Box::new(move |_| {
        checking.send(()).unwrap();
        released.recv().unwrap();
        Err("Authorization check failed".into())
    }));
    t.audio.loop_arm_record(plan(1).segments[1].id).unwrap();
    checked.recv_timeout(Duration::from_secs(2)).unwrap();
    t.audio.input_admission.deadline = Some(Instant::now());
    t.audio.service();
    assert!(!t.audio.loop_capture_busy());
    assert!(!t.audio.input_admission.awaiting_permission());
    assert!(t.audio.error().unwrap().contains("timed out"));
    release.send(()).unwrap();
    settle_worker(&mut t.audio);
    assert_eq!(t.opened.load(Ordering::SeqCst), 0);
    assert!(t.audio.input.is_none());
}

#[test]
fn blocked_native_opener_does_not_block_service_and_reserves_original_card() {
    let mut t = trial(48_000);
    t.audio.service();
    assert_eq!(
        t.opened.load(Ordering::SeqCst),
        0,
        "no opening before explicit request"
    );
    assert_eq!(
        t.authorized.load(Ordering::SeqCst),
        0,
        "no permission request before explicit input request"
    );
    let selected = plan(1).segments[1].id;
    t.audio.loop_arm_record(selected).unwrap();
    t.entered.recv_timeout(Duration::from_secs(2)).unwrap();
    // The opener remains blocked until this test releases it. All these UI
    // operations complete before release; no timing heuristic proves safety.
    for _ in 0..10 {
        t.audio.service();
    }
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
    t.audio
        .output
        .as_mut()
        .unwrap()
        .queue(woodshed_audio::PendingChange::SeekTo { bar_idx: 2 })
        .unwrap();
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
