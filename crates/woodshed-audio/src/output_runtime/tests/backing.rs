use super::*;

#[test]
fn prepared_chord_matches_legacy_strum_and_warm_cache_reuses_pcm() {
    let mut s = song(1);
    s.bars[0].bpm = 120.0;
    s.bars[0].chord_ref = Some(ChordRef {
        formula_name: "exact".into(),
        root_freq_hz: 220.0,
        pitches_hz: vec![220.0, 333.0],
        label: "x".into(),
    });
    let mut cache = Vec::new();
    let a = PreparedBacking::prepare(&s, 1000, &mut cache).unwrap();
    let pcm_a = cache[0].pcm.data.clone();
    let expected = render_chord(&ChordRender::strum(vec![220.0, 333.0], 0.5), 1000);
    assert_eq!(*pcm_a, *expected.data);
    s.click_enabled = true;
    let b = PreparedBacking::prepare(&s, 1000, &mut cache).unwrap();
    assert!(Arc::ptr_eq(&pcm_a, &cache[0].pcm.data));
    drop((a, b));
}

#[test]
fn invalid_saved_audio_is_rejected_without_replacing_backing_or_chord_cache() {
    let mut accepted = song(1);
    accepted.playing = true;
    accepted.bars[0].chord_ref = Some(ChordRef {
        formula_name: "accepted".into(),
        root_freq_hz: 220.0,
        pitches_hz: vec![220.0, 330.0],
        label: "accepted".into(),
    });
    let mut live = runtime();
    live.set_song(accepted.clone()).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    render(&mut live, 3);
    let before = live.snapshot();
    let accepted_pcm = live.chord_cache[0].pcm.data.clone();

    for invalid_sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut invalid = accepted.clone();
        invalid.bars[0].audio_buffer = Some(SampleBuffer::new(vec![invalid_sample; 10], 1000));
        let exact_rejected_pcm = invalid.bars[0].audio_buffer.as_ref().unwrap().data.clone();
        assert_eq!(exact_rejected_pcm.len(), 10);
        assert_eq!(
            invalid.bars[0]
                .audio_buffer
                .as_ref()
                .unwrap()
                .sample_rate_hz,
            1000
        );
        let error = live.set_song(invalid).unwrap_err();
        assert!(error.to_string().contains("nonfinite samples"), "{error}");
        assert_eq!(live.snapshot(), before);
        assert!(live.song().bars[0].audio_buffer.is_none());
        assert_eq!(live.chord_cache.len(), 1);
        assert!(Arc::ptr_eq(&accepted_pcm, &live.chord_cache[0].pcm.data));
        assert_eq!(
            exact_rejected_pcm
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            vec![invalid_sample.to_bits(); 10],
            "rejected source PCM is not modified"
        );
    }

    let mut pathological = accepted.clone();
    pathological.bars[0].chord_ref.as_mut().unwrap().pitches_hz = vec![f32::MAX];
    assert!(live.set_song(pathological).is_err());
    assert_eq!(live.snapshot(), before);
    assert_eq!(
        live.song().bars[0].chord_ref.as_ref().unwrap().pitches_hz,
        [220.0, 330.0]
    );
    assert!(Arc::ptr_eq(&accepted_pcm, &live.chord_cache[0].pcm.data));

    live.chord_cache[0].pcm = ArcGc::new(SampleBuffer::new(vec![f32::NAN], 1000));
    let error = live.set_song(accepted.clone()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("prepared chord contains nonfinite PCM")
    );
    assert_eq!(live.snapshot(), before);
    assert_eq!(
        live.song().bars[0].chord_ref.as_ref().unwrap().pitches_hz,
        [220.0, 330.0]
    );
}

#[test]
fn phrase_mixes_with_capture_and_click_across_loop_boundaries_without_callback_allocation() {
    let mut s = song(1);
    s.playing = true;
    s.click_enabled = false;
    s.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![
            BackingEvent {
                time: audio_primitives::timing::EventTime::Beats(0.0),
                pitches_hz: vec![173.3, 261.6],
                strum_seconds: 0.002,
            },
            BackingEvent {
                time: audio_primitives::timing::EventTime::Beats(0.5),
                pitches_hz: vec![349.2],
                strum_seconds: 0.0,
            },
        ],
    });
    s.bars[0].audio_buffer = Some(crate::SampleBuffer::new(vec![0.125; 10], 1000));
    let mut live = runtime();
    live.set_song(s.clone()).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    let repeated = render(&mut live, 20);
    assert_ne!(&repeated[..10], &repeated[10..20]);
    assert_eq!(&repeated[..20], &repeated[20..]);

    let mut with_click = s;
    with_click.click_enabled = true;
    let mut click_runtime = runtime();
    click_runtime.set_song(with_click).unwrap();
    click_runtime.rewind().unwrap();
    click_runtime.service().unwrap();
    assert_ne!(&repeated[..4], &render(&mut click_runtime, 2)[..4]);
}

#[test]
fn invalid_phrase_replacement_preserves_the_accepted_runtime() {
    let mut accepted = song(1);
    accepted.playing = true;
    accepted.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![BackingEvent {
            time: audio_primitives::timing::EventTime::Beats(0.0),
            pitches_hz: vec![220.0],
            strum_seconds: 0.0,
        }],
    });
    let mut live = runtime();
    live.set_song(accepted.clone()).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    render(&mut live, 3);
    let before = live.snapshot();
    let mut invalid = accepted;
    invalid.bars[0].chord_ref = Some(ChordRef {
        formula_name: "conflict".into(),
        root_freq_hz: 220.0,
        pitches_hz: vec![220.0],
        label: String::new(),
    });
    assert!(live.set_song(invalid).is_err());
    assert_eq!(live.snapshot(), before);
    assert!(live.song().bars[0].chord_ref.is_none());
    assert!(live.song().bars[0].backing_phrase.is_some());
    let after_rejection = render(&mut live, 5);
    let mut control = runtime();
    let mut control_song = live.song();
    control_song.playing = true;
    control.set_song(control_song).unwrap();
    control.rewind().unwrap();
    control.service().unwrap();
    render(&mut control, 3);
    assert_eq!(after_rejection, render(&mut control, 5));
}

#[test]
fn backing_phrase_render_work_is_bounded_across_the_whole_snapshot() {
    let phrase = || BackingPhrase {
        events: (0..256)
            .map(|index| BackingEvent {
                time: audio_primitives::timing::EventTime::Beats((300 + index) as f64),
                pitches_hz: vec![220.0; 4],
                strum_seconds: 0.0,
            })
            .collect(),
    };
    let mut s = Song::new();
    s.bars = (0..2)
        .map(|_| Bar {
            bpm: 600.0,
            time_signature: TimeSignature::new(4, 4),
            length: 250,
            backing_phrase: Some(phrase()),
            ..Default::default()
        })
        .collect();
    assert!(PreparedBacking::prepare(&s, 1000, &mut Vec::new()).is_err());
}

#[test]
fn absolute_backing_duration_and_phrase_are_block_size_invariant_without_callback_allocation() {
    use audio_primitives::timing::EventTime;
    let mut s = song(1);
    s.playing = true;
    s.one_shot = true;
    s.bars[0].bpm = 137.0;
    s.bars[0].duration_seconds = Some(0.537);
    s.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![BackingEvent {
            time: EventTime::Seconds(0.031),
            pitches_hz: vec![173.3, 261.6],
            strum_seconds: 0.002,
        }],
    });
    let mut expected = None;
    for chunk in [1, 7, 64, 137, 537, 1024] {
        let mut r = runtime();
        r.set_song(s.clone()).unwrap();
        r.rewind().unwrap();
        r.service().unwrap();
        let mut pcm = Vec::new();
        let mut remaining = 537;
        while remaining > 0 {
            let n = remaining.min(chunk);
            pcm.extend(render(&mut r, n));
            remaining -= n;
            r.service().unwrap();
        }
        assert!(!r.snapshot().playing);
        assert!(pcm[..62].iter().all(|s| *s == 0.0));
        assert!(pcm[62..].iter().any(|s| *s != 0.0));
        let bits: Vec<_> = pcm.iter().map(|s| s.to_bits()).collect();
        if let Some(expected) = &expected {
            assert_eq!(&bits, expected);
        } else {
            expected = Some(bits);
        }
    }
}

#[test]
fn invalid_absolute_timing_preserves_admitted_backing_and_capture_refuses_it() {
    use audio_primitives::timing::EventTime;
    let mut accepted = song(1);
    accepted.playing = true;
    accepted.bars[0].duration_seconds = Some(0.537);
    accepted.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![BackingEvent {
            time: EventTime::Seconds(0.0),
            pitches_hz: vec![220.0],
            strum_seconds: 0.0,
        }],
    });
    let mut r = runtime();
    r.set_song(accepted.clone()).unwrap();
    r.rewind().unwrap();
    r.service().unwrap();
    render(&mut r, 3);
    let before = r.snapshot();
    for case in [
        "zero",
        "negative",
        "nan",
        "infinite",
        "subframe",
        "below minimum",
        "overflow",
        "legacy",
        "beats",
        "too large",
    ] {
        let mut invalid = accepted.clone();
        match case {
            "zero" => invalid.bars[0].duration_seconds = Some(0.0),
            "negative" => invalid.bars[0].duration_seconds = Some(-1.0),
            "nan" => invalid.bars[0].duration_seconds = Some(f64::NAN),
            "infinite" => invalid.bars[0].duration_seconds = Some(f64::INFINITY),
            "subframe" => invalid.bars[0].duration_seconds = Some(0.000001),
            "below minimum" => invalid.bars[0].duration_seconds = Some(0.499),
            "overflow" => invalid.bars[0].duration_seconds = Some(f64::MAX),
            "legacy" => invalid.bars[0].legacy_clock = true,
            "beats" => {
                invalid.bars[0].backing_phrase.as_mut().unwrap().events[0].time =
                    EventTime::Beats(0.0)
            },
            "too large" => invalid.bars[0].duration_seconds = Some(20_000.0),
            _ => unreachable!(),
        }
        assert!(r.set_song(invalid.clone()).is_err(), "{case}");
        assert_eq!(r.snapshot(), before, "{case}");
        assert_eq!(r.song().bars[0].duration_seconds, Some(0.537));
        assert!(Recording::prepare(&invalid, 1_000, 1).is_err(), "{case}");
    }
    let mut wrong_domain = accepted;
    wrong_domain.bars[0].duration_seconds = None;
    assert!(r.set_song(wrong_domain).is_err());
    assert_eq!(r.snapshot(), before);
}

#[test]
fn seconds_phrase_work_is_bounded_across_segments() {
    use audio_primitives::timing::EventTime;
    let mut s = Song::new();
    s.bars = (0..2)
        .map(|_| Bar {
            duration_seconds: Some(100.0),
            backing_phrase: Some(BackingPhrase {
                events: (0..256)
                    .map(|index| BackingEvent {
                        time: EventTime::Seconds((300 + index) as f64 / 10.0),
                        pitches_hz: vec![220.0; 4],
                        strum_seconds: 0.0,
                    })
                    .collect(),
            }),
            ..Default::default()
        })
        .collect();
    assert!(PreparedBacking::prepare(&s, 1_000, &mut Vec::new()).is_err());
}

#[test]
fn backing_click_is_independent_of_calibration_pattern() {
    let mut r = runtime();
    let mut s = song(1);
    s.click_enabled = true;
    s.playing = true;
    r.set_song(s.clone()).unwrap();
    r.service().unwrap();
    let first = render(&mut r, 8);
    let mut custom =
        SequencerPattern::metronome(30.0, Default::default(), crate::Subdivision::QUARTER);
    custom.tracks[0].sound = Sound::Click {
        frequency_hz: 77.0,
        accent_frequency_hz: 77.0,
        duration_seconds: 0.001,
        amplitude: 0.01,
    };
    r.set_click_pattern(custom).unwrap();
    r.set_song(s).unwrap();
    r.service().unwrap();
    assert_eq!(first, render(&mut r, 8));
}

#[test]
fn prepared_pack_swap_and_backing_steps_render_without_callback_allocation() {
    let selection = audio_packs::files::load(std::path::Path::new("bundled"), None).unwrap();
    let mut r = OutputRuntime::offline(48_000).unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    r.set_pack_pattern_playing(selection.hydrated_pattern().unwrap(), true)
        .unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 120_000).iter().any(|v| v.abs() > 0.05));
    let changed = selection
        .edit(
            &selection.revision(),
            audio_packs::PackEdit::MoveHit {
                track: 0,
                from: 0,
                to: 1,
            },
        )
        .unwrap();
    r.set_pack_pattern_playing(changed.hydrated_pattern().unwrap(), false)
        .unwrap();
    r.service().unwrap();
    let mut backing = Song::new();
    backing.click_enabled = true;
    backing.playing = true;
    backing.bars = vec![Bar {
        bpm: 90.0,
        time_signature: TimeSignature::new(4, 4),
        ..Default::default()
    }];
    r.set_song(backing).unwrap();
    r.rewind().unwrap();
    r.service().unwrap();
    let samples = render(&mut r, 256_000);
    assert!(samples.iter().any(|v| v.abs() > 0.05));
    assert!(samples.iter().all(|v| v.is_finite()));
}
