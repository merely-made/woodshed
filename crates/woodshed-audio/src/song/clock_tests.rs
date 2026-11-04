use super::*;

#[test]
fn absolute_duration_is_exact_and_transient_with_independent_beat_grid() {
    for rate in [44_100, 48_000] {
        for bpm in [20.0, 113.5, 400.0] {
            let bar = Bar {
                bpm,
                time_signature: TimeSignature::new(7, 8),
                length: 3,
                duration_seconds: Some(2.10003),
                ..Default::default()
            };
            let frames = (2.10003 * rate as f64).round() as u64;
            assert_eq!(bar.checked_duration_frames(rate), Ok(frames));
            assert_eq!(bar.duration_samples(rate as f32), frames);
            assert_eq!(
                bar.duration_secs().to_bits(),
                (2.10003_f64 as f32).to_bits()
            );
            let mut ordinary = bar.clone();
            ordinary.duration_seconds = None;
            assert_eq!(bar.beat_frames(rate), ordinary.beat_frames(rate));
            assert_eq!(bar.count_in_frames(rate), ordinary.count_in_frames(rate));
            assert_eq!(
                serde_json::to_value(&bar).unwrap(),
                serde_json::to_value(&ordinary).unwrap()
            );
            let restored: Bar =
                serde_json::from_value(serde_json::to_value(&bar).unwrap()).unwrap();
            assert_eq!(restored.duration_seconds, None);
        }
    }
}

#[test]
fn bars_and_legacy_none_keep_their_exact_arithmetic() {
    for rate in [44_100, 48_000] {
        for bpm in [83.75_f32, 113.5, 137.0] {
            for legacy in [false, true] {
                let bar = Bar {
                    bpm,
                    legacy_clock: legacy,
                    time_signature: TimeSignature::new(7, 8),
                    length: 3,
                    ..Default::default()
                };
                let quarter = 60.0 / bpm.max(1.0);
                let expected_secs = if legacy {
                    7.0 * quarter * 3.0
                } else {
                    7.0 * (quarter * 4.0 / 8.0) * 3.0
                };
                let frames = if legacy {
                    (expected_secs * rate as f32).round() as u64
                } else {
                    audio_primitives::frames_per_bar(rate, bpm, 7, 8) as u64 * 3
                };
                assert_eq!(bar.duration_secs().to_bits(), expected_secs.to_bits());
                assert_eq!(bar.duration_samples(rate as f32), frames);
                assert_eq!(bar.checked_duration_frames(rate), Ok(frames));
            }
        }
    }
}

#[test]
fn canonical_clock_honors_meter_and_rounds_per_beat_before_measures() {
    for (rate, bpm, beats, unit, measures, expected) in [
        (48_000, 120.0, 3, 8, 1, 36_000),
        (48_000, 113.5, 5, 4, 2, 253_740),
        (48_000, 137.0, 7, 4, 3, 441_462),
        (48_000, 120.0, 4, 4, 1, 96_000),
    ] {
        let bar = Bar {
            bpm,
            time_signature: TimeSignature::new(beats, unit),
            length: measures,
            ..Default::default()
        };
        assert_eq!(bar.duration_samples(rate as f32), expected);
        assert_eq!(
            bar.duration_samples(rate as f32),
            bar.measure_frames(rate) * measures as u64
        );
        assert_eq!(bar.count_in_frames(rate), bar.measure_frames(rate));
    }
    let eighths = Bar::with_tempo(120.0, TimeSignature::new(3, 8));
    assert_eq!(eighths.duration_secs(), 0.75);
}

#[test]
fn legacy_playback_keeps_whole_block_rounding_and_old_beat_and_lead() {
    let legacy = Bar {
        bpm: 113.5,
        time_signature: TimeSignature::new(5, 4),
        length: 2,
        legacy_clock: true,
        ..Default::default()
    };
    assert_eq!(legacy.duration_samples(48_000.0), 253_744);
    assert_eq!(legacy.beat_frames(48_000), 25_374);
    assert_eq!(legacy.measure_frames(48_000), 126_870);
    assert_eq!(legacy.count_in_frames(48_000), 126_872);
    let mut eighths = Bar::with_tempo(120.0, TimeSignature::new(3, 8));
    eighths.legacy_clock = true;
    assert_eq!(eighths.duration_samples(48_000.0), 72_000);
}
#[test]
fn new_or_cleared_bars_do_not_inherit_legacy_capture_timing() {
    let mut song = Song::new();
    song.bars[0].legacy_clock = true;
    song.bars[0].audio_buffer = Some(SampleBuffer::new(vec![0.25; 10], 1000));
    let new = song.add_bar();
    assert!(!song.bars[new].legacy_clock);
    assert!(song.bars[0].legacy_clock);
    let original = song.detach_buffer(0).unwrap().unwrap();
    assert_eq!(original.data[0], 0.25);
    assert!(!song.bars[0].legacy_clock);
}
