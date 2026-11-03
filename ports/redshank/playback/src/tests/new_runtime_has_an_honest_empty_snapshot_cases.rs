// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn new_runtime_has_an_honest_empty_snapshot() {
    assert_eq!(
        PlaybackRuntime::start().snapshot().state,
        PlaybackState::Empty
    );
}

#[test]
fn failed_remote_load_publishes_its_token_and_controller_error() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 8,
            source: MediaSource::Local {
                path: "redshank-test://previous-item".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(8) && snapshot.state == PlaybackState::Paused
    });
    runtime
        .command(PlaybackCommand::Load {
            token: 9,
            source: MediaSource::Enclosure {
                url: "redshank-test://missing/episode.mp3".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(9) && matches!(snapshot.state, PlaybackState::Unavailable(_))
    });
    assert!(
        matches!(snapshot.state, PlaybackState::Unavailable(ref error) if error.contains("could not reach HTTP audio source")),
        "unexpected remote failure: {:?}",
        snapshot.state
    );
    assert_eq!(snapshot.position_ms, 0);
    assert_eq!(snapshot.duration_ms, None);
    assert_eq!(snapshot.source, None);
    assert_eq!(snapshot.representation, None);
}

#[test]
fn cached_source_receipt_requires_the_published_bytes() {
    let mut backend = Backend::new(own_fetch());
    backend
        .load(&servo_media_player::controller::MediaSource::Local {
            path: "redshank-test://cached-integrity".into(),
        })
        .unwrap();
    backend
        .admit_cached_representation(&RepresentationReceipt::default())
        .unwrap_err();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cached.wav");
    synthetic_source(&path, 1);
    let bytes = fs::read(&path).unwrap();
    backend
        .load(&servo_media_player::controller::MediaSource::Local {
            path: path.display().to_string(),
        })
        .unwrap();
    let missing = backend
        .admit_cached_representation(&RepresentationReceipt::default())
        .unwrap_err();
    assert!(missing.contains("failed integrity validation"));
    let mismatch = backend
        .admit_cached_representation(&RepresentationReceipt {
            byte_length: Some(1),
            complete_digest: Some("blake3:different".into()),
            ..Default::default()
        })
        .unwrap_err();
    assert!(mismatch.contains("failed integrity validation"));

    let expected = RepresentationReceipt {
        requested_url: Some("https://example.invalid/episode.wav".into()),
        byte_length: Some(bytes.len() as u64),
        complete_digest: Some(format!("blake3:{}", blake3::hash(&bytes).to_hex())),
        ..Default::default()
    };
    backend.admit_cached_representation(&expected).unwrap();
    assert_eq!(backend.facts().0, Some(expected));
}

#[test]
fn worker_cached_integrity_refusal_keeps_its_diagnostic_and_accepts_a_later_load() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("changed-cache.wav");
    synthetic_source(&path, 1);
    let original = fs::read(&path).unwrap();
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 31,
            source: MediaSource::Cached {
                path: path.display().to_string(),
                origin_url: "https://example.invalid/changed-cache.wav".into(),
                representation: Box::new(RepresentationReceipt {
                    byte_length: Some(original.len() as u64),
                    complete_digest: Some("blake3:changed".into()),
                    ..Default::default()
                }),
            },
            resume_ms: 0,
        })
        .unwrap();
    let refused = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(31) && matches!(snapshot.state, PlaybackState::Unavailable(_))
    });
    assert!(
        matches!(refused.state, PlaybackState::Unavailable(ref reason)
                if reason.contains("cached audio failed integrity validation")
                    && !reason.contains("internal audio failure")),
        "integrity refusal must survive the worker path: {:?}",
        refused.state
    );
    assert!(refused.source.is_none() && refused.representation.is_none());
    assert_eq!(fs::read(&path).unwrap(), original);
    runtime
        .command(PlaybackCommand::Load {
            token: 32,
            source: MediaSource::Local {
                path: "redshank-test://after-cache-refusal".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.load_token == Some(32)
            && snapshot.state == PlaybackState::Paused)
        .state,
        PlaybackState::Paused
    );
}

#[test]
fn worker_exact_duration_resume_reaches_ended_without_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("exact-end.wav");
    synthetic_source(&path, 1);
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 33,
            source: MediaSource::Local {
                path: path.display().to_string(),
            },
            resume_ms: 1_000,
        })
        .unwrap();
    let ended = wait_for(&runtime, |snapshot| snapshot.state == PlaybackState::Ended);
    assert_eq!(ended.load_token, Some(33));
    assert_eq!(ended.state, PlaybackState::Ended);
    assert_eq!(ended.position_ms, 1_000);
    assert_eq!(ended.duration_ms, Some(1_000));
    runtime.command(PlaybackCommand::Seek(1_000)).unwrap();
    // A worker barrier prevents the already-ended snapshot from satisfying
    // the next assertion before the new seek has actually been processed.
    inspect_controls(&runtime);
    let sought = wait_for(&runtime, |snapshot| snapshot.state == PlaybackState::Ended);
    assert_eq!(sought.state, PlaybackState::Ended);
    assert_eq!(sought.position_ms, 1_000);
    assert_eq!(sought.duration_ms, Some(1_000));
}

#[test]
fn worker_defers_unloaded_rate_until_controller_admission_and_reapplies_it_on_load() {
    let runtime = PlaybackRuntime::start();
    runtime.command(PlaybackCommand::SetRate(175)).unwrap();
    runtime.command(PlaybackCommand::SetVolume(37)).unwrap();
    assert_eq!(inspect_controls(&runtime), (100, None, 37));
    assert_eq!(runtime.snapshot().state, PlaybackState::Empty);
    assert_eq!(
        runtime.snapshot().rate_percent,
        100,
        "no source admitted this preference"
    );

    for (token, expected_rate) in [(41, 175), (42, 200)] {
        if token == 42 {
            runtime.command(PlaybackCommand::SetRate(400)).unwrap();
            assert_eq!(inspect_controls(&runtime), (200, Some(200), 37));
        }
        runtime
            .command(PlaybackCommand::Load {
                token,
                source: MediaSource::Local {
                    path: "redshank-test://admitted-rate".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        let loaded = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(token) && snapshot.state == PlaybackState::Paused
        });
        assert_eq!(loaded.state, PlaybackState::Paused);
        assert_eq!(loaded.rate_percent, expected_rate);
        assert_eq!(
            inspect_controls(&runtime),
            (expected_rate, Some(expected_rate), 37)
        );
    }

    let directory = tempfile::tempdir().unwrap();
    runtime
        .command(PlaybackCommand::Load {
            token: 43,
            source: MediaSource::Local {
                path: directory.path().join("missing.wav").display().to_string(),
            },
            resume_ms: 0,
        })
        .unwrap();
    let failed = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(43) && matches!(snapshot.state, PlaybackState::Unavailable(_))
    });
    assert!(matches!(failed.state, PlaybackState::Unavailable(_)));
    runtime.command(PlaybackCommand::SetRate(125)).unwrap();
    inspect_controls(&runtime);
    assert_eq!(
        runtime.snapshot().rate_percent,
        200,
        "failed load cannot admit pending rate"
    );
    runtime
        .command(PlaybackCommand::Load {
            token: 44,
            source: MediaSource::Local {
                path: "redshank-test://later-rate-admission".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    let loaded = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(44) && snapshot.state == PlaybackState::Paused
    });
    assert_eq!(loaded.state, PlaybackState::Paused);
    assert_eq!(loaded.rate_percent, 125);
    assert_eq!(inspect_controls(&runtime), (125, Some(125), 37));
}

#[test]
fn worker_recovery_retains_actual_rate_volume_and_capture_duck_until_unduck() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 51,
            source: MediaSource::Local {
                path: "redshank-test://controls-before-recovery".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    assert_eq!(
        wait_for(&runtime, |snapshot| snapshot.load_token == Some(51)
            && snapshot.state == PlaybackState::Paused)
        .state,
        PlaybackState::Paused
    );
    runtime.command(PlaybackCommand::SetRate(150)).unwrap();
    runtime.command(PlaybackCommand::SetVolume(40)).unwrap();
    runtime.command(PlaybackCommand::Duck(25)).unwrap();
    assert_eq!(inspect_controls(&runtime), (150, Some(150), 10));
    runtime.command(PlaybackCommand::CrashWorker).unwrap();
    let recovered = wait_for(
        &runtime,
        |snapshot| matches!(snapshot.state, PlaybackState::Unavailable(ref reason) if reason.contains("recovered")),
    );
    assert!(matches!(recovered.state, PlaybackState::Unavailable(_)));
    assert_eq!(
        inspect_controls(&runtime),
        (100, None, 10),
        "gain restored before source/output admission"
    );
    runtime
        .command(PlaybackCommand::Load {
            token: 52,
            source: MediaSource::Local {
                path: "redshank-test://controls-after-recovery".into(),
            },
            resume_ms: 0,
        })
        .unwrap();
    let loaded = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(52) && snapshot.state == PlaybackState::Paused
    });
    assert_eq!(loaded.state, PlaybackState::Paused);
    assert_eq!(
        (
            loaded.rate_percent,
            loaded.volume_percent,
            loaded.duck_percent
        ),
        (150, 40, Some(25))
    );
    assert_eq!(
        inspect_controls(&runtime),
        (150, Some(150), 10),
        "backend, controller and snapshot must agree"
    );
    runtime.command(PlaybackCommand::Unduck).unwrap();
    assert_eq!(inspect_controls(&runtime), (150, Some(150), 40));
    assert_eq!(runtime.snapshot().duck_percent, None);
}

#[test]
fn local_snapshot_survives_in_place_writes_and_truncation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.bin");
    let original = vec![0x5a; 192_000];
    fs::write(&path, &original).unwrap();
    // Keep a writer open before snapshotting: changing only the reader's
    // handle or pathname does not protect against this writer.
    let mut writer = OpenOptions::new().write(true).open(&path).unwrap();
    let (mut snapshot, receipt) = backend::snapshot_local(&path).unwrap();
    writer.write_all(&vec![0xa5; original.len()]).unwrap();
    let mut actual = Vec::new();
    snapshot.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, original);

    writer.set_len(0).unwrap();
    snapshot.seek(SeekFrom::Start(0)).unwrap();
    actual.clear();
    snapshot.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, original);
    assert_eq!(receipt.byte_length, Some(actual.len() as u64));
    assert_eq!(
        receipt.complete_digest,
        Some(format!("blake3:{}", blake3::hash(&actual).to_hex()))
    );
}

#[test]
fn local_snapshot_survives_pathname_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.bin");
    let original = vec![0x5a; 192_000];
    fs::write(&path, &original).unwrap();
    let (mut snapshot, receipt) = backend::snapshot_local(&path).unwrap();
    fs::rename(&path, directory.path().join("retired.bin")).unwrap();
    fs::write(&path, b"replacement").unwrap();

    let mut actual = Vec::new();
    snapshot.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, original);
    assert_eq!(receipt.byte_length, Some(actual.len() as u64));
    assert_eq!(
        receipt.complete_digest,
        Some(format!("blake3:{}", blake3::hash(&actual).to_hex()))
    );
}

#[test]
fn local_decoder_plays_the_receipted_audio_after_source_mutation() {
    fn decode_all(decoder: &mut backend::Decoder) -> Vec<f32> {
        let mut audio = Vec::new();
        loop {
            match decode_packet(decoder).unwrap() {
                Decoded::Frames(samples) => audio.extend_from_slice(&samples),
                Decoded::Skipped => {},
                Decoded::EndOfFile => return audio,
            }
        }
    }

    for replace_path in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("episode.wav");
        let frames = synthetic_source(&path, 2);
        let original = fs::read(&path).unwrap();
        let (mut baseline, _) = open_local(&path).unwrap();
        let expected_audio = decode_all(&mut baseline);
        assert_eq!(expected_audio.len() as u64, frames * 2);

        let (mut decoder, receipt) = open_local(&path).unwrap();
        if replace_path {
            fs::rename(&path, directory.path().join("retired.wav")).unwrap();
        }
        // A much shorter invalid source tests future reads beyond any
        // bytes the decoder buffered while opening the WAV header.
        fs::write(&path, b"not the receipted audio").unwrap();
        assert_eq!(decode_all(&mut decoder), expected_audio);
        assert_eq!(receipt.byte_length, Some(original.len() as u64));
        assert_eq!(
            receipt.complete_digest,
            Some(format!("blake3:{}", blake3::hash(&original).to_hex()))
        );
    }
}

#[test]
fn captured_pcm_wav_is_decodable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("voice-note.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for sample in 0..4_800 {
        writer
            .write_sample(if sample % 2 == 0 { i16::MAX } else { i16::MIN })
            .unwrap();
    }
    writer.finalize().unwrap();

    let (mut decoder, receipt) = open_local(&path).unwrap();
    let packet = decoder.format.next_packet().unwrap();
    let decoded = decoder.decoder.decode(&packet).unwrap();
    assert!(decoded.frames() > 0);
    assert_eq!(decoded.spec().channels.count(), 1);
    assert_eq!(decoded.spec().rate, 48_000);
    assert_eq!(receipt.byte_length, Some(9_644));
}

#[test]
fn sink_clock_helpers_preserve_frame_boundaries() {
    let mut pcm = vec![0.0; 10];
    consume_frames(&mut pcm, 3, 2);
    assert_eq!(pcm.len(), 4);
    assert!(!drained(true, 4, true));
    assert!(!drained(false, 0, true));
    assert!(!drained(true, 0, false));
    assert!(drained(true, 0, true));
}
