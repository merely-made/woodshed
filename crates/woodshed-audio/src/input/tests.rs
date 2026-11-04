use super::pitch::{compute_rms, compute_stable, is_harmonic_of};
use super::*;

#[test]
fn stream_failure_latches_across_observers_and_wakes_without_analyzer_polling() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let builder = InputEngineBuilder::new();
    let health = builder.health_handle();
    let observer = health.clone();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    health.set_wake(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    assert!(observer.failure().is_none());
    // This is the same report path invoked by CPAL's error callback.
    health.report_failure("native input disconnected");
    health.report_failure("later duplicate status");
    assert_eq!(
        observer.failure().as_deref(),
        Some("native input disconnected")
    );
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert!(
        InputEngineBuilder::new()
            .health_handle()
            .failure()
            .is_none(),
        "a fresh stream owns a fresh latch; old observers never reset"
    );
}

#[test]
fn capture_consumer_is_transferred_once_and_rate_precedes_callbacks() {
    let mut analyzer = LooperCaptureAnalyzer::new();
    let handle = analyzer.handle();
    let mut consumer = handle.take_consumer().unwrap();
    assert!(handle.clone().take_consumer().is_none());
    assert_eq!(handle.source_rate(), 0);
    analyzer.prepare(44_100);
    assert_eq!(handle.source_rate(), 44_100);
    assert_eq!(consumer.source_rate(), 44_100);
    assert!(consumer.validate_output_rate(48_000).is_err());
    handle.set_enabled(true);
    analyzer.process(&[0.4, -0.2], 44_100.0, Instant::now());
    assert_eq!(consumer.pop_sample(), Some(0.4));
    assert_eq!(consumer.pop_sample(), Some(-0.2));
    handle.clear();
    assert!(handle.is_enabled());
    handle.set_enabled(false);
    assert!(!handle.is_enabled());
}

#[test]
fn builder_prepares_capture_from_explicit_source_configuration() {
    let (mut builder, handle) = InputEngineBuilder::new().with_looper_capture();
    assert_eq!(handle.source_rate(), 0);
    builder.prepare_analyzers(48_000);
    assert_eq!(handle.source_rate(), 48_000);
    assert!(
        handle
            .take_consumer()
            .unwrap()
            .validate_output_rate(48_000)
            .is_ok()
    );
    // Physical build supplies its own configuration, never retaining a
    // harness/provisional rate; repeated preparation updates the handle.
    builder.prepare_analyzers(44_100);
    assert_eq!(handle.source_rate(), 44_100);
}

#[test]
fn rms_of_silence_is_zero() {
    let signal = vec![0.0_f64; 1024];
    assert_eq!(compute_rms(&signal), 0.0);
}

#[test]
fn rms_of_constant_one_is_one() {
    let signal = vec![1.0_f64; 1024];
    assert!((compute_rms(&signal) - 1.0).abs() < 1e-9);
}

#[test]
fn rms_of_pure_sine_is_one_over_sqrt_two() {
    let n = 4096;
    let signal: Vec<f64> = (0..n)
        .map(|i| (i as f64 * std::f64::consts::TAU / n as f64).sin())
        .collect();
    let rms = compute_rms(&signal);
    let expected = 1.0 / 2.0_f64.sqrt();
    assert!(
        (rms - expected).abs() < 1e-3,
        "rms = {rms}, expected ≈ {expected}"
    );
}

#[test]
fn silence_threshold_gates_very_quiet_signal() {
    let n = 4096;
    let amplitude = 0.0005;
    let signal: Vec<f64> = (0..n)
        .map(|i| amplitude * (i as f64 * std::f64::consts::TAU / n as f64).sin())
        .collect();
    let rms = compute_rms(&signal);
    assert!(
        rms < DEFAULT_SILENCE_RMS_THRESHOLD,
        "rms {rms} should be below threshold {DEFAULT_SILENCE_RMS_THRESHOLD}"
    );
}

fn note(name: PdNoteName, octave: i32) -> DetectedNote {
    DetectedNote {
        name,
        octave,
        note_freq_hz: 0.0,
        actual_freq_hz: 0.0,
        cents_offset: 0.0,
        in_tune: true,
    }
}

fn note_with_freq(name: PdNoteName, octave: i32, freq: f64) -> DetectedNote {
    DetectedNote {
        name,
        octave,
        note_freq_hz: freq,
        actual_freq_hz: freq,
        cents_offset: 0.0,
        in_tune: true,
    }
}

#[test]
fn smoothing_returns_none_when_history_too_short() {
    let mut h = VecDeque::new();
    h.push_back(note(PdNoteName::A, 4));
    assert!(compute_stable(&h).is_none());
}

#[test]
fn smoothing_returns_consensus_when_majority_agrees() {
    let mut h = VecDeque::new();
    h.push_back(note(PdNoteName::A, 4));
    h.push_back(note(PdNoteName::A, 4));
    h.push_back(note(PdNoteName::B, 4));
    let stable = compute_stable(&h).expect("majority should win");
    assert_eq!(stable.name, PdNoteName::A);
    assert_eq!(stable.octave, 4);
}

#[test]
fn smoothing_returns_none_when_no_majority() {
    let mut h = VecDeque::new();
    h.push_back(note(PdNoteName::A, 4));
    h.push_back(note(PdNoteName::B, 4));
    h.push_back(note(PdNoteName::C, 5));
    assert!(compute_stable(&h).is_none());
}

#[test]
fn third_harmonic_is_recognized() {
    assert!(is_harmonic_of(247.94, 82.41));
}

#[test]
fn second_harmonic_is_recognized() {
    assert!(is_harmonic_of(440.0, 220.0));
}

#[test]
fn sub_octave_is_recognized() {
    assert!(is_harmonic_of(220.0, 440.0));
}

#[test]
fn unrelated_pitches_are_not_harmonics() {
    assert!(!is_harmonic_of(110.0, 82.0));
}

#[test]
fn same_pitch_is_not_a_harmonic() {
    assert!(!is_harmonic_of(440.0, 440.0));
}

#[test]
fn smoothing_uses_most_recent_matching_detection() {
    let mut h = VecDeque::new();
    let mut a1 = note(PdNoteName::A, 4);
    a1.cents_offset = -10.0;
    let mut a2 = note(PdNoteName::A, 4);
    a2.cents_offset = 5.0;
    h.push_back(a1);
    h.push_back(note(PdNoteName::B, 4));
    h.push_back(a2);
    let stable = compute_stable(&h).expect("A4 has majority");
    assert_eq!(stable.cents_offset, 5.0);
}

#[test]
fn fundamental_bias_promotes_lower_entry_when_consensus_is_3rd_harmonic() {
    let mut h = VecDeque::new();
    h.push_back(note_with_freq(PdNoteName::B, 3, 247.94));
    h.push_back(note_with_freq(PdNoteName::E, 2, 82.41));
    h.push_back(note_with_freq(PdNoteName::B, 3, 247.94));
    let stable = compute_stable(&h).expect("majority B with E sub-multiple");
    assert_eq!(stable.name, PdNoteName::E);
    assert_eq!(stable.octave, 2);
}

#[test]
fn fundamental_bias_does_not_promote_when_no_sub_multiple_present() {
    let mut h = VecDeque::new();
    h.push_back(note_with_freq(PdNoteName::A, 4, 440.0));
    h.push_back(note_with_freq(PdNoteName::B, 4, 493.88));
    h.push_back(note_with_freq(PdNoteName::A, 4, 440.0));
    let stable = compute_stable(&h).expect("A majority");
    assert_eq!(stable.name, PdNoteName::A);
    assert_eq!(stable.octave, 4);
}

#[test]
fn fundamental_bias_does_not_demote_when_consensus_is_already_lowest() {
    let mut h = VecDeque::new();
    h.push_back(note_with_freq(PdNoteName::E, 2, 82.41));
    h.push_back(note_with_freq(PdNoteName::E, 2, 82.41));
    h.push_back(note_with_freq(PdNoteName::B, 3, 247.94));
    let stable = compute_stable(&h).expect("E2 majority");
    assert_eq!(stable.name, PdNoteName::E);
    assert_eq!(stable.octave, 2);
}

#[test]
fn quiet_playing_passes_threshold() {
    let n = 4096;
    let amplitude = 0.05;
    let signal: Vec<f64> = (0..n)
        .map(|i| amplitude * (i as f64 * std::f64::consts::TAU / n as f64).sin())
        .collect();
    let rms = compute_rms(&signal);
    assert!(
        rms >= DEFAULT_SILENCE_RMS_THRESHOLD,
        "rms {rms} should pass threshold {DEFAULT_SILENCE_RMS_THRESHOLD}"
    );
}

// === New tests for the analyzer pipeline ===

#[test]
fn pitch_analyzer_handle_starts_with_empty_snapshot() {
    let a = PitchAnalyzer::new();
    let h = a.handle();
    let snap = h.snapshot();
    assert!(snap.note.is_none());
    assert_eq!(snap.input_level, 0.0);
}

#[test]
fn pitch_analyzer_set_enabled_round_trips() {
    let a = PitchAnalyzer::new();
    let h = a.handle();
    assert!(h.is_enabled());
    h.set_enabled(false);
    assert!(!h.is_enabled());
    h.set_enabled(true);
    assert!(h.is_enabled());
}

#[test]
fn pitch_analyzer_disabled_does_not_publish_a_detection() {
    // Feed a strong A4 sine to a disabled analyzer; snapshot should
    // stay empty.
    let mut a = PitchAnalyzer::new();
    let h = a.handle();
    h.set_enabled(false);

    let sr = 48_000.0_f32;
    let samples: Vec<f32> = (0..ANALYSIS_WINDOW)
        .map(|i| {
            let t = i as f32 / sr;
            (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
        })
        .collect();
    a.process(&samples, sr, Instant::now());
    let snap = h.snapshot();
    assert!(snap.note.is_none());
}

#[test]
fn pitch_analyzer_enabled_detects_a4_from_sine() {
    let mut a = PitchAnalyzer::new();
    let h = a.handle();
    let sr = 48_000.0_f32;
    // Feed enough samples to fill the analysis window + smoothing
    // history. ANALYSIS_WINDOW * 4 gives ~3 detection passes which
    // is enough for compute_stable to publish a result.
    let samples: Vec<f32> = (0..ANALYSIS_WINDOW * 4)
        .map(|i| {
            let t = i as f32 / sr;
            (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
        })
        .collect();
    // Feed in small batches like cpal would deliver.
    for chunk in samples.chunks(512) {
        a.process(chunk, sr, Instant::now());
    }
    let snap = h.snapshot();
    let note = snap.note.expect("should have detected A4");
    assert_eq!(note.name, PdNoteName::A);
    assert_eq!(note.octave, 4);
}
