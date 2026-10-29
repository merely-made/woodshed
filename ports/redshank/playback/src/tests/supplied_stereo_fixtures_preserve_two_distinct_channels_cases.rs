// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
#[ignore = "requires REDSHANK_PHASE4_FIXTURES with generated stereo fixtures"]
fn supplied_stereo_fixtures_preserve_two_distinct_channels() {
    for name in ["stereo.mp3", "stereo.m4a"] {
        let (mut decoder, _) = open_local(&fixture(name)).unwrap();
        let mut distinct = false;
        while let Ok(packet) = decoder.format.next_packet() {
            if packet.track_id() != decoder.track_id {
                continue;
            }
            let decoded = decoder.decoder.decode(&packet).unwrap();
            let spec = *decoded.spec();
            assert_eq!(spec.channels.count(), 2);
            let mut samples =
                symphonia::core::audio::SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            samples.copy_interleaved_ref(decoded);
            distinct |= samples
                .samples()
                .as_chunks::<2>()
                .0
                .iter()
                .any(|[left, right]| (left - right).abs() > 0.01);
            if distinct {
                break;
            }
        }
        assert!(distinct, "{name} lost stereo separation");
    }
}

#[test]
#[ignore = "requires REDSHANK_PHASE4_FIXTURES with generated stereo fixtures"]
fn supplied_fixture_resumes_through_the_controller() {
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 42,
            source: MediaSource::Local {
                path: fixture("stereo.mp3").display().to_string(),
            },
            resume_ms: 1_000,
        })
        .unwrap();
    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(42) && matches!(snapshot.state, PlaybackState::Paused)
    });
    assert!((800..=1_200).contains(&snapshot.position_ms));
    assert!(
        snapshot
            .representation
            .and_then(|receipt| receipt.complete_digest)
            .is_some()
    );
}

#[test]
#[ignore = "requires REDSHANK_HTTP_FIXTURE_URL served with byte ranges"]
fn supplied_http_fixture_decodes_progressively() {
    let url = std::env::var("REDSHANK_HTTP_FIXTURE_URL")
        .expect("set REDSHANK_HTTP_FIXTURE_URL to a range-served MP3 fixture");
    let (mut decoder, receipt) = open_http(own_fetch(), &url).unwrap();
    assert_eq!(receipt.requested_url.as_deref(), Some(url.as_str()));
    assert!(receipt.byte_length.is_some());
    loop {
        let packet = decoder.format.next_packet().unwrap();
        if packet.track_id() != decoder.track_id {
            continue;
        }
        let decoded = decoder.decoder.decode(&packet).unwrap();
        assert!(decoded.frames() > 0);
        break;
    }
}

#[test]
#[ignore = "requires REDSHANK_HTTP_FIXTURE_URL and a default output device"]
fn supplied_http_fixture_reaches_controller_ready() {
    let url = std::env::var("REDSHANK_HTTP_FIXTURE_URL")
        .expect("set REDSHANK_HTTP_FIXTURE_URL to a range-served MP3 fixture");
    let runtime = PlaybackRuntime::start();
    runtime
        .command(PlaybackCommand::Load {
            token: 51,
            source: MediaSource::Enclosure { url },
            resume_ms: 400,
        })
        .unwrap();
    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot.load_token == Some(51) && snapshot.state != PlaybackState::Loading
    });
    assert_eq!(snapshot.state, PlaybackState::Paused, "{snapshot:?}");
    assert!((300..=500).contains(&snapshot.position_ms));
    assert!(snapshot.representation.is_some());
}
