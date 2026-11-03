// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn retiming_delivers_the_inverse_of_the_rate_in_source_time() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rate.wav");
    let source_frames = synthetic_source(&path, 2);
    for percent in [80_u16, 120] {
        let (mut decoder, _) = open_local(&path).unwrap();
        let mut stage = RateStage::new(48_000, 2, percent);
        let mut output = Vec::new();
        loop {
            match decode_packet(&mut decoder).unwrap() {
                Decoded::Frames(samples) => stage.push(&samples, &mut output),
                Decoded::Skipped => {},
                Decoded::EndOfFile => break,
            }
        }
        stage.flush(&mut output);

        // The sink is handed 100/percent of the source frames.
        let output_frames = (output.len() / 2) as u64;
        let expected = source_frames as f64 * 100.0 / f64::from(percent);
        assert!(
            (output_frames as f64 - expected).abs() / expected < 0.02,
            "{percent}%: {output_frames} output frames against {expected}"
        );

        // Reported position stays in source time end to end.
        assert_eq!(stage.source_frames_at(0), 0);
        assert_eq!(stage.source_frames_at(output_frames), source_frames);
        let middle = stage.source_frames_at(output_frames / 2) as f64;
        assert!(
            (middle - source_frames as f64 / 2.0).abs() / (source_frames as f64) < 0.02,
            "{percent}%: midpoint mapped to source frame {middle}"
        );

        // Output still queued is source time not yet heard, at the rate.
        let queued = 0.5_f64;
        let presented = output_frames.saturating_sub((queued * 48_000.0) as u64);
        let behind = (source_frames - stage.source_frames_at(presented)) as f64;
        let expected_behind = queued * 48_000.0 * f64::from(percent) / 100.0;
        assert!(
            (behind - expected_behind).abs() / expected_behind < 0.05,
            "{percent}%: {behind} source frames behind, expected {expected_behind}"
        );
    }
}

#[test]
fn unity_rate_leaves_the_decoded_bytes_alone() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unity.wav");
    let source_frames = synthetic_source(&path, 1);
    let (mut decoder, _) = open_local(&path).unwrap();
    let mut stage = RateStage::new(48_000, 2, 100);
    let mut decoded = Vec::new();
    let mut output = Vec::new();
    loop {
        match decode_packet(&mut decoder).unwrap() {
            Decoded::Frames(samples) => {
                decoded.extend_from_slice(&samples);
                stage.push(&samples, &mut output);
            },
            Decoded::Skipped => {},
            Decoded::EndOfFile => break,
        }
    }
    stage.flush(&mut output);
    assert_eq!(output, decoded);
    assert_eq!(
        stage.source_frames_at((output.len() / 2) as u64),
        source_frames
    );
}

#[test]
fn worker_controller_sequence_needs_no_output_device() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 7,
            source: MediaSource::Local {
                path: "redshank-test://controller-sequence".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(7) && snapshot.state == PlaybackState::Paused
        })
        .state,
        PlaybackState::Paused
    );
    runtime.command(PlaybackCommand::Pause).unwrap();
    runtime.command(PlaybackCommand::Seek(250)).unwrap();
    let seeked = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(7) && snapshot.position_ms == 250
    });
    assert!(matches!(
        seeked.state,
        PlaybackState::Loading | PlaybackState::Paused
    ));
    runtime.command(PlaybackCommand::Play).unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.state == PlaybackState::Ended).state,
        PlaybackState::Ended
    );
}

#[test]
fn worker_note_seek_acknowledges_only_the_matching_request() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 18,
            source: MediaSource::Local {
                path: "redshank-test://note-seek".into(),
            },
            resume_ms: 125,
        })
        .unwrap();
    let loaded = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(18) && snapshot.state == PlaybackState::Paused
    });
    assert_eq!(loaded.state, PlaybackState::Paused);
    assert_eq!(loaded.position_ms, 125);
    // Resuming and ordinary transport seeks do not acknowledge note seeks.
    assert_eq!(loaded.completed_note_seek, None);
    runtime.command(PlaybackCommand::Seek(175)).unwrap();
    let ordinary = wait_for(&runtime, |snapshot| snapshot.position_ms == 175);
    assert_eq!(ordinary.position_ms, 175);
    assert_eq!(ordinary.completed_note_seek, None);

    runtime
        .command(PlaybackCommand::SeekNote {
            token: 18,
            request_id: 41,
            position_ms: 250,
        })
        .unwrap();
    let seeked = wait_for(&runtime, |snapshot| {
        snapshot.completed_note_seek == Some(41)
    });
    assert_eq!(seeked.completed_note_seek, Some(41));
    assert_eq!(seeked.load_token, Some(18));
    assert_eq!(seeked.position_ms, 250);

    for (request_id, position_ms) in [(42, 300), (43, 350)] {
        runtime
            .command(PlaybackCommand::SeekNote {
                token: 18,
                request_id,
                position_ms,
            })
            .unwrap();
    }
    let latest = wait_for(&runtime, |snapshot| {
        snapshot.completed_note_seek == Some(43)
    });
    assert_eq!(latest.completed_note_seek, Some(43));
    assert_eq!(latest.position_ms, 350);

    runtime
        .command(PlaybackCommand::SeekNote {
            token: 17,
            request_id: 44,
            position_ms: 400,
        })
        .unwrap();
    // This later command provides a barrier after the stale request.
    runtime.command(PlaybackCommand::SetVolume(37)).unwrap();
    let stale = wait_for(&runtime, |snapshot| snapshot.volume_percent == 37);
    assert_eq!(stale.volume_percent, 37);
    assert_eq!(stale.load_token, Some(18));
    assert_eq!(stale.completed_note_seek, Some(43));
    assert_eq!(stale.position_ms, 350);

    let directory = tempfile::tempdir().unwrap();
    runtime
        .command(PlaybackCommand::Load {
            token: 19,
            source: MediaSource::Local {
                path: directory.path().join("missing.wav").display().to_string(),
            },
            resume_ms: 500,
        })
        .unwrap();
    let failed = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(19) && matches!(snapshot.state, PlaybackState::Unavailable(_))
    });
    assert!(matches!(failed.state, PlaybackState::Unavailable(_)));
    assert_eq!(failed.load_token, Some(19));
    assert_eq!(failed.completed_note_seek, Some(43));
}

#[test]
fn levels_report_the_effective_rate_and_the_honest_buffered_extent() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 21,
            source: MediaSource::Local {
                path: "redshank-test://levels".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    let loaded = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(21) && snapshot.state == PlaybackState::Paused
    });
    assert_eq!(loaded.buffered_percent, 100);
    runtime.command(PlaybackCommand::SetVolume(40)).unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.volume_percent == 40).volume_percent,
        40
    );
    // A duck scales the gain for a capture and leaves the listener's
    // volume where it was; unducking clears it.
    runtime.command(PlaybackCommand::Duck(10)).unwrap();
    let ducked = wait_for(&runtime, |snapshot| snapshot.duck_percent == Some(10));
    assert_eq!(ducked.volume_percent, 40);
    runtime.command(PlaybackCommand::Unduck).unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.duck_percent.is_none()).volume_percent,
        40
    );
    // The retiming stage applies the rate, so the snapshot reports the
    // chosen rate rather than a silent 100.
    runtime.command(PlaybackCommand::SetRate(150)).unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.rate_percent == 150).rate_percent,
        150
    );
    // Outside the range the stage can hold honestly, the request clamps.
    runtime.command(PlaybackCommand::SetRate(400)).unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.rate_percent != 150).rate_percent,
        200
    );
}

#[test]
fn voice_preview_preserves_the_loaded_episode_snapshot() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 17,
            source: MediaSource::Local {
                path: "redshank-test://episode".into(),
            },
            resume_ms: 250,
        })
        .unwrap();
    let episode = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(17) && snapshot.state == PlaybackState::Paused
    });
    runtime
        .command(PlaybackCommand::StartPreview {
            id: "voice-note".into(),
            source: MediaSource::Local {
                path: "redshank-test://voice-note".into(),
            },
        })
        .unwrap();
    let previewed = wait_for(&runtime, |snapshot| {
        snapshot
            .preview
            .as_ref()
            .is_some_and(|preview| preview.state == PreviewState::Ended)
    });
    assert_eq!(previewed.load_token, episode.load_token);
    assert_eq!(previewed.representation, episode.representation);
    assert_eq!(previewed.source, episode.source);
    assert_eq!(previewed.position_ms, episode.position_ms);
    assert_eq!(previewed.state, PlaybackState::Paused);
    assert_eq!(previewed.preview.unwrap().id, "voice-note");
}

#[test]
fn worker_recovers_after_an_internal_failure() {
    let runtime = PlaybackRuntime::start();
    runtime.command(PlaybackCommand::CrashWorker).unwrap();
    let recovered = wait_for(
        &runtime,
        |snapshot| matches!(snapshot.state, PlaybackState::Unavailable(ref message) if message.contains("recovered")),
    );
    assert!(matches!(recovered.state, PlaybackState::Unavailable(_)));

    runtime
        .command(PlaybackCommand::Load {
            token: 12,
            source: MediaSource::Local {
                path: "redshank-test://post-recovery".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(12) && snapshot.state == PlaybackState::Paused
        })
        .state,
        PlaybackState::Paused
    );
}

#[test]
#[ignore = "requires REDSHANK_LOCAL_SEQUENCE or REDSHANK_PHASE4_FIXTURES and a default output device"]
fn supplied_local_sources_can_be_replaced() {
    let runtime = PlaybackRuntime::start();
    let paths = std::env::var_os("REDSHANK_LOCAL_SEQUENCE")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![fixture("stereo.mp3"), fixture("stereo.m4a")]);
    assert_eq!(paths.len(), 2, "supply exactly two local audio paths");
    for (token, path) in [(60, &paths[0]), (61, &paths[1]), (62, &paths[0])] {
        runtime
            .command(PlaybackCommand::Load {
                token,
                source: MediaSource::Local {
                    path: path.display().to_string(),
                },
                resume_ms: 0,
            })
            .unwrap();
        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(token) && snapshot.state != PlaybackState::Loading
        });
        assert_eq!(
            snapshot.state,
            PlaybackState::Paused,
            "{}: {snapshot:?}",
            path.display()
        );
        runtime.command(PlaybackCommand::Play).unwrap();
        let started = Instant::now();
        let playing = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(token) && snapshot.position_ms >= 300
        });
        assert_eq!(
            playing.state,
            PlaybackState::Playing,
            "{}: {playing:?}",
            path.display()
        );
        assert!(
            started.elapsed() >= Duration::from_millis(200),
            "{} advanced 300 ms in {:?}",
            path.display(),
            started.elapsed()
        );
        runtime.command(PlaybackCommand::Pause).unwrap();
        let paused = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(token) && snapshot.state == PlaybackState::Paused
        });
        assert_eq!(paused.state, PlaybackState::Paused);
    }
}

#[test]
#[ignore = "requires REDSHANK_PHASE4_FIXTURES and a default output device"]
fn supplied_fixture_plays_each_dock_rate_at_its_wall_clock() {
    // The timed span of source, measured between two reported source
    // positions so the output queue's one-off start-up latency drops out.
    const FROM_MS: u64 = 200;
    const TO_MS: u64 = 1_800;
    for (token, percent) in [(70, 80_u16), (71, 100), (72, 120), (73, 150)] {
        let runtime = PlaybackRuntime::start();
        runtime.command(PlaybackCommand::SetRate(percent)).unwrap();
        runtime
            .command(PlaybackCommand::Load {
                token,
                source: MediaSource::Local {
                    path: fixture("stereo.mp3").display().to_string(),
                },
                resume_ms: 0,
            })
            .unwrap();
        let loaded = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(token) && snapshot.state != PlaybackState::Loading
        });
        assert_eq!(loaded.state, PlaybackState::Paused, "{loaded:?}");
        assert_eq!(loaded.rate_percent, percent);
        assert_eq!(loaded.duration_ms.map(|ms| ms / 100), Some(20));

        runtime.command(PlaybackCommand::Play).unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        let until = |target: u64| loop {
            let snapshot = runtime.snapshot();
            if snapshot.position_ms >= target || Instant::now() >= deadline {
                return (snapshot, Instant::now());
            }
            thread::sleep(Duration::from_millis(1));
        };
        let (opened, from) = until(FROM_MS);
        let (reached, to) = until(TO_MS);
        runtime.command(PlaybackCommand::Pause).unwrap();
        assert_eq!(reached.state, PlaybackState::Playing, "{reached:?}");
        assert!(reached.position_ms >= TO_MS, "{reached:?}");
        assert_eq!(reached.rate_percent, percent);
        // Source span divided by the rate is the wall clock it takes.
        let span = (reached.position_ms - opened.position_ms) as f64;
        let expected = span * 100.0 / f64::from(percent);
        let measured = (to - from).as_secs_f64() * 1000.0;
        println!(
            "{percent}%: {measured:.0} ms of wall clock for {span:.0} ms of source (expected {expected:.0} ms)"
        );
        // The floor here is the snapshot publication period, not the
        // retiming: supplied_fixtures_keep_their_pitch_at_every_dock_rate
        // checks the same ratio frame-exactly without a device.
        assert!(
            (measured - expected).abs() < expected * 0.03,
            "{percent}%: {measured:.0} ms against {expected:.0} ms"
        );
    }
}

#[test]
#[ignore = "requires REDSHANK_PHASE4_FIXTURES with generated stereo fixtures"]
fn supplied_fixtures_keep_their_pitch_at_every_dock_rate() {
    for name in ["stereo.mp3", "stereo.m4a"] {
        for percent in [80_u16, 100, 120, 150] {
            let (mut decoder, _) = open_local(&fixture(name)).unwrap();
            let mut stage = RateStage::new(48_000, 2, percent);
            let mut output = Vec::new();
            let mut source_frames = 0_u64;
            loop {
                match decode_packet(&mut decoder).unwrap() {
                    Decoded::Frames(samples) => {
                        source_frames += (samples.len() / 2) as u64;
                        stage.push(&samples, &mut output);
                    },
                    Decoded::Skipped => {},
                    Decoded::EndOfFile => break,
                }
            }
            stage.flush(&mut output);
            let frames = (output.len() / 2) as f64;
            let expected = source_frames as f64 * 100.0 / f64::from(percent);
            assert!(
                (frames - expected).abs() / expected < 0.02,
                "{name} at {percent}%: {frames} frames against {expected}"
            );
            // Skip the codec's leading silence and the flush tail.
            let measured = &output[4_800 * 2..output.len() - 4_800 * 2];
            let left = fundamental(measured, 2, 0);
            let right = fundamental(measured, 2, 1);
            println!("{name} at {percent}%: {frames} frames, {left:.1} Hz / {right:.1} Hz");
            assert!(
                (left - 440.0).abs() / 440.0 < 0.01,
                "{name} {percent}%: {left} Hz"
            );
            assert!(
                (right - 660.0).abs() / 660.0 < 0.01,
                "{name} {percent}%: {right} Hz"
            );
        }
    }
}
