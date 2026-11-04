use super::*;

fn sample(seconds: u64, frames: u64) -> Observation {
    Observation {
        monotonic_ns: seconds * 1_000_000_000,
        unix_ms: 1_800_000_000_000 + seconds * 1_000,
        output_id: 17,
        output_frames: frames,
        output_rate: 48_000,
        input_rate: Some(48_000),
        input_admission_pending: false,
        native_streams: 1,
        playing: true,
        recording: true,
        capture_busy: true,
        recording_bar: Some(1),
        cursor_bar: 1,
        capture_target: Some(1),
        capture_epoch: Some(3),
        capture_invalid: false,
    }
}

#[test]
fn wall_and_device_frame_bounds_are_both_required() {
    let mut wall_only = State::new(Mode::Capture, 1);
    assert!(wall_only.observe(sample(0, 100)));
    assert!(wall_only.observe(sample(15, 100 + 48_000 * 14)));
    assert!(wall_only.result.is_none());
    assert!(!wall_only.observe(sample(16, 100 + 48_000 * 15)));
    assert!(matches!(wall_only.result, Some(Ok(()))));

    let mut frames_only = State::new(Mode::Capture, 1);
    assert!(frames_only.observe(sample(0, 100)));
    assert!(frames_only.observe(sample(1, 100 + 48_000 * 15)));
    assert!(frames_only.result.is_none());
    assert!(!frames_only.observe(sample(15, 100 + 48_000 * 15)));
    assert!(matches!(frames_only.result, Some(Ok(()))));
}

#[test]
fn armed_and_count_in_seconds_never_count_as_active_capture() {
    let mut state = State::new(Mode::Capture, 1);
    let mut waiting = sample(0, 0);
    waiting.recording = false;
    assert!(state.observe(waiting.clone()));
    waiting.monotonic_ns = 20_000_000_000;
    waiting.output_frames = 48_000 * 20;
    assert!(state.observe(waiting));
    assert!(state.baseline.is_none());
    assert!(state.observe(sample(21, 48_000 * 21)));
    assert_eq!(
        state.baseline.as_ref().unwrap().monotonic_ns,
        21_000_000_000
    );
    assert!(state.observe(sample(35, 48_000 * 35)));
    assert!(!state.observe(sample(36, 48_000 * 36)));
    assert!(matches!(state.result, Some(Ok(()))));
}

#[test]
fn explicit_input_opening_is_allowed_but_only_active_admitted_time_and_frames_count() {
    let mut state = State::new(Mode::Capture, 1);
    let opening = |seconds| {
        let mut now = sample(seconds, 48_000 * seconds);
        now.recording = false;
        now.recording_bar = None;
        now.input_rate = None;
        now.input_admission_pending = true;
        now
    };
    assert!(state.observe(opening(0)));
    assert!(state.observe(opening(8)));
    assert!(state.baseline.is_none());
    let mut armed = sample(9, 48_000 * 9);
    armed.recording = false;
    armed.recording_bar = None;
    assert!(state.observe(armed));
    assert!(state.baseline.is_none());
    assert!(state.observe(sample(10, 48_000 * 10)));
    assert!(
        state.observe(sample(24, 48_000 * 25)),
        "frames alone cannot pass"
    );
    assert!(!state.observe(sample(25, 48_000 * 25)));
    assert!(matches!(state.result, Some(Ok(()))));
    assert_eq!(
        state.baseline.as_ref().unwrap().monotonic_ns,
        10_000_000_000
    );
}

#[test]
fn pending_input_timeout_missing_pending_and_lost_admitted_rate_are_failures() {
    for defect in 0..4 {
        let mut state = State::new(Mode::Capture, 1);
        let mut now = sample(0, 0);
        now.recording = false;
        now.recording_bar = None;
        now.input_rate = None;
        now.input_admission_pending = true;
        assert!(state.observe(now.clone()));
        match defect {
            0 => {
                now.monotonic_ns = 30_000_000_000;
            },
            1 => {
                now.input_admission_pending = false;
            },
            _ => {
                now.input_rate = Some(48_000);
                now.input_admission_pending = false;
                assert!(state.observe(now.clone()));
                now.input_rate = if defect == 2 { None } else { Some(44_100) };
                now.input_admission_pending = true; // cannot excuse loss after admission
            },
        }
        assert!(!state.observe(now));
        assert!(matches!(state.result, Some(Err(_))));
    }
}

#[test]
fn refused_stopped_overflowed_replaced_or_offline_capture_cannot_pass() {
    for defect in 0..11 {
        let mut state = State::new(Mode::Capture, 1);
        assert!(state.observe(sample(0, 1_000)));
        let mut bad = sample(15, 1_000 + 48_000 * 15);
        match defect {
            0 => bad.capture_invalid = true,
            1 => bad.input_rate = None,
            2 => bad.recording = false,
            3 => bad.capture_busy = false,
            4 => bad.capture_target = Some(0),
            5 => bad.capture_epoch = Some(4),
            6 => bad.output_id = 18,
            7 => bad.output_rate = 44_100,
            8 => bad.native_streams = 0,
            9 => bad.output_frames = 999,
            _ => bad.playing = false,
        }
        assert!(!state.observe(bad));
        assert!(matches!(state.result, Some(Err(_))), "defect {defect}");
    }
}

#[test]
fn moved_recording_bar_or_cursor_fails_despite_unchanged_accepted_target() {
    for at_assert in [false, true] {
        for move_recording_bar in [false, true] {
            let mut state = State::new(Mode::Capture, 1);
            assert!(state.observe(sample(0, 100)));
            if at_assert {
                assert!(!state.observe(sample(15, 720_100)));
                assert!(matches!(state.result, Some(Ok(()))));
                // Production assertion resamples after provisional success.
                state.result = None;
            }
            let mut moved = sample(if at_assert { 16 } else { 1 }, 768_100);
            if move_recording_bar {
                moved.recording_bar = Some(2);
            } else {
                moved.cursor_bar = 2;
            }
            assert_eq!(moved.capture_target, Some(1));
            assert!(!state.observe(moved));
            assert!(matches!(state.result, Some(Err(_))));
        }
    }
}

#[test]
fn waiting_or_device_progress_timeout_is_a_terminal_failure() {
    let mut waiting = State::new(Mode::Capture, 1);
    let mut observation = sample(30, 48_000 * 30);
    observation.recording = false;
    assert!(!waiting.observe(observation));
    assert!(matches!(waiting.result, Some(Err(_))));
    let mut stalled = State::new(Mode::Capture, 1);
    assert!(stalled.observe(sample(0, 100)));
    assert!(!stalled.observe(sample(60, 101)));
    assert!(matches!(stalled.result, Some(Err(_))));
}

#[test]
fn backing_dwell_needs_five_real_seconds_and_assert_rechecks_capture_health() {
    let mut backing = State::new(Mode::Backing, 1);
    let mut start = sample(0, 100);
    start.recording = false;
    start.capture_busy = false;
    start.input_rate = None;
    assert!(backing.observe(start.clone()));
    start.monotonic_ns = 5_000_000_000;
    start.output_frames += 48_000 * 5;
    assert!(!backing.observe(start));
    assert!(matches!(backing.result, Some(Ok(()))));
    let mut capture = State::new(Mode::Capture, 1);
    capture.observe(sample(0, 100));
    capture.observe(sample(15, 720_100));
    assert!(matches!(capture.result, Some(Ok(()))));
    // The production assert clears only a provisional success and samples
    // fresh health before admitting the receipt.
    capture.result = None;
    let mut invalidated = sample(16, 768_100);
    invalidated.capture_invalid = true;
    assert!(!capture.observe(invalidated));
    assert!(matches!(capture.result, Some(Err(_))));
}

#[test]
fn json_receipt_records_observed_active_interval_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("physical-capture-dwell.json");
    let mut state = State::new(Mode::Capture, 1);
    state.observe(sample(3, 10_000));
    state.observe(sample(18, 730_000));
    let dwell = Dwell {
        started: Instant::now(),
        requested_unix_ms: 1_800_000_000_000,
        session: Arc::new(()),
        path: path.clone(),
        state,
    };
    dwell.write().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["success"], true);
    assert_eq!(value["active_elapsed_ns"], 15_000_000_000_u64);
    assert_eq!(value["output_frame_advance"], 720_000);
    assert_eq!(value["active_start"]["recording"], true);
    assert_eq!(value["end"]["input_rate"], 48_000);
    assert_eq!(value["end"]["capture_invalid"], false);
    assert!(dwell.write().is_err());
}
