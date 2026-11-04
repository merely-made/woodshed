use super::*;

/// Cheap deterministic pseudo-random noise sample, in [-1, 1].
/// Pure function of an index — lets warmup and the test signal
/// share an identical noise floor so the detector's median is
/// well-calibrated when the impulse arrives.
fn noise(i: usize) -> f32 {
    ((i as f32 * 12.9898).sin() * 43_758.547).fract() - 0.5
}

/// Generate a synthetic signal of `len` samples: low-amplitude
/// noise floor with sharp impulse "hits" + short decay tails at
/// the given sample indices.
fn synthetic_signal(len: usize, hit_at: &[usize]) -> Vec<f32> {
    let mut out: Vec<f32> = (0..len).map(|i| noise(i) * 0.01).collect();
    for &h in hit_at {
        if h < len {
            out[h] += 0.8;
        }
        // Short exponential decay tail (one frame's worth) so
        // each impulse spans a measurable energy bump.
        for j in 1..256 {
            if h + j < len {
                out[h + j] += 0.8 * (-0.02 * j as f32).exp();
            }
        }
    }
    out
}

/// Feed enough background noise to populate the energy-history
/// median. Uses the same noise generator as the test signal so the
/// floor is calibrated when the test impulse arrives.
fn warmup_detector(detector: &mut OnsetDetector, sample_rate: f32) {
    let n = (sample_rate * 0.3) as usize;
    let quiet: Vec<f32> = (0..n).map(|i| noise(i) * 0.01).collect();
    let _ = detector.feed(&quiet);
}

#[test]
fn detector_warmup_returns_no_onsets() {
    let mut det = OnsetDetector::new(48_000.0);
    let quiet = vec![0.001_f32; 48_000];
    let onsets = det.feed(&quiet);
    // Quiet floor → no false positives.
    assert!(onsets.is_empty(), "got false-positive onsets: {onsets:?}");
}

#[test]
fn detector_finds_isolated_impulse() {
    let sample_rate = 48_000.0;
    let mut det = OnsetDetector::new(sample_rate);
    warmup_detector(&mut det, sample_rate);
    let baseline = det.samples_consumed();

    // 1 second window with a hit at 0.5s.
    let signal = synthetic_signal(48_000, &[24_000]);
    let onsets = det.feed(&signal);

    assert_eq!(
        onsets.len(),
        1,
        "expected 1 onset, got {}: {onsets:?}",
        onsets.len()
    );

    // Onset should land somewhere near sample 24000 (offset from
    // baseline), within one frame of the actual hit.
    let hit_global = baseline + 24_000;
    let detected = onsets[0];
    let delta = detected.abs_diff(hit_global);
    assert!(
        delta <= DEFAULT_FRAME_SIZE as u64 * 2,
        "detected onset {detected} too far from hit {hit_global}; delta = {delta}"
    );
}

#[test]
fn detector_finds_multiple_well_spaced_impulses() {
    let sample_rate = 48_000.0;
    let mut det = OnsetDetector::new(sample_rate);
    warmup_detector(&mut det, sample_rate);

    // 4 hits, 250ms apart = 120 BPM quarter notes.
    let hits = [12_000, 24_000, 36_000, 48_000];
    let signal = synthetic_signal(60_000, &hits);
    let onsets = det.feed(&signal);

    assert_eq!(
        onsets.len(),
        4,
        "expected 4 onsets, got {}: {onsets:?}",
        onsets.len()
    );
}

#[test]
fn detector_debounces_too_close_hits() {
    let sample_rate = 48_000.0;
    // Aggressive debounce: 200ms between accepted hits.
    let mut det = OnsetDetector::new(sample_rate).with_min_spacing_ms(200.0);
    warmup_detector(&mut det, sample_rate);

    // Two hits 100ms apart (4800 samples) — second should be
    // suppressed by the debounce.
    let signal = synthetic_signal(48_000, &[10_000, 14_800]);
    let onsets = det.feed(&signal);

    assert_eq!(
        onsets.len(),
        1,
        "debounce should reject second hit: {onsets:?}"
    );
}

#[test]
fn detector_reset_clears_history() {
    let sample_rate = 48_000.0;
    let mut det = OnsetDetector::new(sample_rate);
    warmup_detector(&mut det, sample_rate);
    let signal = synthetic_signal(24_000, &[12_000]);
    det.feed(&signal);

    det.reset();
    assert_eq!(det.samples_consumed(), 0);
    // After reset, the warmup needs to happen again before we
    // trust onsets. Just verify the next quiet feed yields zero
    // onsets (i.e. no stale state firing).
    let onsets = det.feed(&vec![0.001_f32; 48_000]);
    assert!(
        onsets.is_empty(),
        "post-reset quiet feed yielded onsets: {onsets:?}"
    );
}

#[test]
fn detector_handles_split_buffer_seam() {
    // The same impulse should be detected whether fed as one
    // batch or split across a buffer boundary that lands mid-frame.
    let sample_rate = 48_000.0;
    let signal = synthetic_signal(48_000, &[24_000]);

    let mut one_shot = OnsetDetector::new(sample_rate);
    warmup_detector(&mut one_shot, sample_rate);
    let single = one_shot.feed(&signal);

    let mut split = OnsetDetector::new(sample_rate);
    warmup_detector(&mut split, sample_rate);
    // Split mid-frame on purpose (DEFAULT_FRAME_SIZE = 256).
    let split_at = 24_137; // not a multiple of 256
    let first = split.feed(&signal[..split_at]);
    let second = split.feed(&signal[split_at..]);
    let combined: Vec<u64> = first.into_iter().chain(second).collect();

    assert_eq!(single.len(), 1);
    assert_eq!(combined.len(), 1);
    // Within one frame either way.
    let delta = single[0].abs_diff(combined[0]);
    assert!(
        delta <= DEFAULT_FRAME_SIZE as u64,
        "single={single:?} split={combined:?}"
    );
}

#[test]
fn irregular_chunks_count_each_sample_once_and_preserve_onset_times() {
    let sample_rate = 48_000.0;
    let signal = synthetic_signal(48_000 + 73, &[12_000, 24_000, 36_000]);

    let mut one_shot = OnsetDetector::new(sample_rate);
    warmup_detector(&mut one_shot, sample_rate);
    let warmup_samples = one_shot.samples_consumed();
    let one_shot_onsets = one_shot.feed(&signal);
    assert!(
        !one_shot_onsets.is_empty(),
        "fixture must produce onsets before comparing chunking"
    );
    assert_eq!(
        one_shot.samples_consumed(),
        warmup_samples + signal.len() as u64
    );
    assert!(
        one_shot_onsets
            .iter()
            .all(|sample| { sample % DEFAULT_FRAME_SIZE as u64 == 0 })
    );

    let mut chunked = OnsetDetector::new(sample_rate);
    warmup_detector(&mut chunked, sample_rate);
    let mut chunked_onsets = Vec::new();
    let mut cursor = 0;
    let chunk_sizes = [1, 37, 256, 19, 513, 7, 1024, 63];
    let mut chunk_index = 0;
    while cursor < signal.len() {
        // Empty feeds must not move the sample clock or disturb a
        // partial frame.
        assert!(chunked.feed(&[]).is_empty());
        let end = (cursor + chunk_sizes[chunk_index % chunk_sizes.len()]).min(signal.len());
        chunked_onsets.extend(chunked.feed(&signal[cursor..end]));
        cursor = end;
        chunk_index += 1;
    }

    assert_eq!(
        chunked.samples_consumed(),
        warmup_samples + signal.len() as u64
    );
    assert_eq!(chunked_onsets, one_shot_onsets);
    assert_eq!(
        chunked.samples_consumed() - warmup_samples,
        signal.len() as u64
    );
    assert_eq!(
        signal.len() % DEFAULT_FRAME_SIZE,
        201,
        "fixture retains a partial tail"
    );

    let mut samplewise = OnsetDetector::new(sample_rate);
    warmup_detector(&mut samplewise, sample_rate);
    let mut samplewise_onsets = Vec::new();
    for sample in &signal {
        samplewise_onsets.extend(samplewise.feed(std::slice::from_ref(sample)));
    }
    assert_eq!(
        samplewise.samples_consumed(),
        warmup_samples + signal.len() as u64
    );
    assert_eq!(samplewise_onsets, one_shot_onsets);
}

// === Tempo estimation ===

#[test]
fn estimate_bpm_returns_none_below_two_onsets() {
    assert!(estimate_bpm(&[], 48_000.0).is_none());
    assert!(estimate_bpm(&[1000], 48_000.0).is_none());
}

#[test]
fn estimate_bpm_120bpm_from_quarter_intervals() {
    // 120 BPM = 0.5s = 24000 samples at 48kHz between hits.
    let sample_rate = 48_000.0;
    let onsets: Vec<u64> = (0..8).map(|i| i as u64 * 24_000).collect();
    let bpm = estimate_bpm(&onsets, sample_rate).unwrap();
    assert!((bpm - 120.0).abs() < 0.5, "got {bpm}");
}

#[test]
fn estimate_bpm_60bpm_from_one_second_intervals() {
    let sample_rate = 48_000.0;
    let onsets: Vec<u64> = (0..5).map(|i| i as u64 * 48_000).collect();
    let bpm = estimate_bpm(&onsets, sample_rate).unwrap();
    assert!((bpm - 60.0).abs() < 0.5, "got {bpm}");
}

#[test]
fn estimate_bpm_clamps_excessive_tempo() {
    // 10ms between hits = 6000 BPM. Should clamp to 240.
    let sample_rate = 48_000.0;
    let onsets: Vec<u64> = (0..5).map(|i| i as u64 * 480).collect();
    let bpm = estimate_bpm(&onsets, sample_rate).unwrap();
    assert_eq!(bpm, 240.0);
}

#[test]
fn estimate_bpm_clamps_too_slow_tempo() {
    // 3 seconds between hits = 20 BPM. Should clamp to 40.
    let sample_rate = 48_000.0;
    let onsets: Vec<u64> = (0..5).map(|i| i as u64 * 144_000).collect();
    let bpm = estimate_bpm(&onsets, sample_rate).unwrap();
    assert_eq!(bpm, 40.0);
}

#[test]
fn estimate_bpm_median_rejects_outlier() {
    // Four onsets at 24000-sample intervals (120 BPM) plus one
    // gappy outlier in the middle. Median should still pick 120.
    let sample_rate = 48_000.0;
    let onsets: Vec<u64> = vec![
        0,
        24_000,
        48_000,
        48_000 + 96_000, // huge gap — simulates a missed hit
        48_000 + 96_000 + 24_000,
        48_000 + 96_000 + 48_000,
    ];
    let bpm = estimate_bpm(&onsets, sample_rate).unwrap();
    assert!((bpm - 120.0).abs() < 1.0, "got {bpm}");
}
