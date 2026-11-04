use super::*;

#[test]
fn absolute_seconds_and_bars_mix_with_exact_wav_live_parity_at_both_rates() {
    use audio_primitives::timing::{EventTime, seconds_to_frames};
    for rate in [44_100, 48_000] {
        let mut ordinary = Bar {
            bpm: 6000.0,
            time_signature: TimeSignature::new(1, 4),
            length: 2,
            ..Default::default()
        };
        let ordinary_frames = ordinary.duration_samples(rate as f32) as usize;
        ordinary.audio_buffer = Some(SampleBuffer::new(vec![0.125; ordinary_frames], rate));
        let seconds = 0.53731;
        let frames = seconds_to_frames(seconds, rate).unwrap();
        let mut absolute = Bar {
            bpm: 137.0,
            duration_seconds: Some(seconds),
            backing_phrase: Some(BackingPhrase {
                events: vec![
                    BackingEvent {
                        time: EventTime::Seconds(0.0137),
                        pitches_hz: vec![173.3, 261.6],
                        strum_seconds: 0.018,
                    },
                    BackingEvent {
                        time: EventTime::Seconds(0.40361),
                        pitches_hz: vec![349.2],
                        strum_seconds: 0.0,
                    },
                ],
            }),
            ..Default::default()
        };
        absolute.audio_buffer = Some(SampleBuffer::new(
            (0..frames)
                .map(|i| (i % 17) as f32 * 0.00625 - 0.05)
                .collect(),
            rate,
        ));
        let mut song = Song::new();
        song.click_enabled = true;
        song.bars = vec![ordinary, absolute];
        let total = ordinary_frames as u64 + frames;
        let (report, pcm, _) = export(&song, rate);
        assert_eq!(report.frames, total);
        for chunk in [7, 113, 4096] {
            let mut r = OutputRuntime::offline(rate).unwrap();
            let mut live = song.clone();
            live.playing = true;
            live.one_shot = true;
            r.set_song(live).unwrap();
            r.rewind().unwrap();
            r.service().unwrap();
            let mut expected = Vec::new();
            let mut remaining = total as usize;
            while remaining > 0 {
                let n = remaining.min(chunk);
                let mut scratch = vec![0.0; n * 2];
                r.render_offline(&mut scratch).unwrap();
                expected.extend(scratch.chunks_exact(2).map(|s| s[0]));
                r.service().unwrap();
                remaining -= n;
            }
            assert_eq!(bits(&pcm), bits(&expected), "{rate} Hz, {chunk} frames");
            assert!(!r.snapshot().playing);
        }
        // Removing click makes the seconds segment PCM independent of BPM.
        song.click_enabled = false;
        let original = export(&song, rate).1;
        song.bars[1].bpm = 20.0;
        assert_eq!(bits(&original), bits(&export(&song, rate).1));
        song.bars[1].bpm = 400.0;
        assert_eq!(bits(&original), bits(&export(&song, rate).1));
    }
}

#[test]
fn absolute_clock_and_phrase_failures_never_touch_the_export_writer() {
    use audio_primitives::timing::EventTime;
    for case in [
        "zero",
        "negative",
        "nan",
        "infinite",
        "below minimum",
        "overflow",
        "legacy",
        "beats",
        "seconds in bars",
        "collision",
        "outside",
        "strum",
        "pcm budget",
        "work budget",
    ] {
        let mut song = Song::new();
        song.bars[0].duration_seconds = Some(0.537);
        song.bars[0].backing_phrase = Some(BackingPhrase {
            events: vec![BackingEvent {
                time: EventTime::Seconds(0.0),
                pitches_hz: vec![220.0],
                strum_seconds: 0.0,
            }],
        });
        match case {
            "zero" => song.bars[0].duration_seconds = Some(0.0),
            "negative" => song.bars[0].duration_seconds = Some(-1.0),
            "nan" => song.bars[0].duration_seconds = Some(f64::NAN),
            "infinite" => song.bars[0].duration_seconds = Some(f64::INFINITY),
            "below minimum" => song.bars[0].duration_seconds = Some(0.499),
            "overflow" => song.bars[0].duration_seconds = Some(f64::MAX),
            "legacy" => song.bars[0].legacy_clock = true,
            "beats" => {
                song.bars[0].backing_phrase.as_mut().unwrap().events[0].time = EventTime::Beats(0.0)
            },
            "seconds in bars" => song.bars[0].duration_seconds = None,
            "collision" => {
                let phrase = song.bars[0].backing_phrase.as_mut().unwrap();
                let mut duplicate = phrase.events[0].clone();
                duplicate.time = EventTime::Seconds(0.00001);
                phrase.events.push(duplicate);
            },
            "outside" => {
                song.bars[0].backing_phrase.as_mut().unwrap().events[0].time =
                    EventTime::Seconds(0.537)
            },
            "strum" => {
                let event = &mut song.bars[0].backing_phrase.as_mut().unwrap().events[0];
                event.pitches_hz.push(330.0);
                event.strum_seconds = 0.537;
            },
            "pcm budget" => song.bars[0].duration_seconds = Some(20_000.0),
            "work budget" => {
                song.bars[0].duration_seconds = Some(100.0);
                song.bars[0].backing_phrase = Some(BackingPhrase {
                    events: (0..256)
                        .map(|i| BackingEvent {
                            time: EventTime::Seconds((300 + i) as f64 / 10.0),
                            pitches_hz: vec![220.0; 4],
                            strum_seconds: 0.0,
                        })
                        .collect(),
                });
                song.bars.push(song.bars[0].clone());
            },
            _ => unreachable!(),
        }
        let mut writer = UntouchedWriter::default();
        assert!(
            export_song_wav_to(&song, 1_000, &mut writer).is_err(),
            "{case}"
        );
        assert_eq!((writer.writes, writer.seeks), (0, 0), "{case}");
    }
}
