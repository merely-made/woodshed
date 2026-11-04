use super::*;

fn seconds_reference(bytes: &[u8], rate: u32, frames: usize, seconds: f64) -> LoopCaptureRef {
    let mut take = reference(bytes, frames);
    take.sample_rate = rate;
    take.bars = 0;
    take.clock = Some(SECONDS_CLOCK.into());
    take.seconds = Some(seconds);
    take.codec = LoopCaptureCodec::RawF32V1;
    take
}

#[test]
fn versions_one_through_three_keep_their_wire_and_exact_pcm() {
    let samples = [0.0, -0.0, 0.25, -1.5, f32::from_bits(1)];
    let bytes = encode_pcm(48_000, &samples).unwrap();
    for (version, explicit_clock) in [(1, false), (2, false), (3, false), (3, true)] {
        let mut take = reference(&bytes, samples.len());
        if version >= 2 {
            take.codec = LoopCaptureCodec::RawF32V1;
        }
        if explicit_clock {
            take.clock = Some(PER_BEAT_CLOCK.into());
        }
        let manifest = LoopCaptureManifest {
            version,
            namespace: Some(NAMESPACE.into()),
            takes: vec![take],
        };
        let wire = serde_json::to_value(&manifest).unwrap();
        assert!(wire["takes"][0].get("seconds").is_none());
        let reopened: LoopCaptureManifest = serde_json::from_value(wire.clone()).unwrap();
        reopened.validate().unwrap();
        assert_eq!(serde_json::to_value(&reopened).unwrap(), wire);
        assert_eq!(reopened.takes[0].legacy_clock(), !explicit_clock);
        let decoded = decode_capture(&bytes, &reopened.takes[0]).unwrap();
        assert!(
            decoded
                .iter()
                .zip(samples)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );

        let empty = LoopCaptureManifest {
            version,
            ..Default::default()
        };
        empty.validate().unwrap();
        assert!(
            empty.is_empty(),
            "old empty manifests must remain omittable"
        );
    }
}

#[test]
fn seconds_clock_requires_version_four_and_complete_matching_metadata() {
    let take = seconds_reference(&[], 1024, 512, 0.5);
    let mut manifest = LoopCaptureManifest {
        version: 4,
        namespace: Some(NAMESPACE.into()),
        takes: vec![take.clone()],
    };
    manifest.validate().unwrap();
    assert!(!take.legacy_clock());
    for version in 1..=3 {
        manifest.version = version;
        assert!(manifest.validate().unwrap_err().contains("version 4"));
    }
    manifest.version = 4;
    for (clock, seconds, bars) in [
        (None, Some(0.5), 0),
        (None, Some(0.5), 1),
        (Some(PER_BEAT_CLOCK), Some(0.5), 0),
        (Some(PER_BEAT_CLOCK), Some(0.5), 1),
        (Some(SECONDS_CLOCK), None, 0),
        (Some(SECONDS_CLOCK), None, 1),
        (Some(SECONDS_CLOCK), Some(0.5), 1),
        (None, None, 0),
        (Some(PER_BEAT_CLOCK), None, 0),
    ] {
        let mut bad = take.clone();
        bad.clock = clock.map(str::to_owned);
        bad.seconds = seconds;
        bad.bars = bars;
        assert!(
            bad.validate().is_err(),
            "forged clock/seconds/bars: {bad:?}"
        );
        manifest.takes = vec![bad];
        assert!(manifest.validate().is_err());
    }
    for seconds in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        0.0,
        0.4999,
        f64::MAX,
    ] {
        let mut bad = take.clone();
        bad.seconds = Some(seconds);
        assert!(bad.validate().is_err());
    }
    for bpm in [f32::NAN, f32::INFINITY, 0.5] {
        let mut bad = take.clone();
        bad.bpm = bpm;
        assert!(
            bad.validate().is_err(),
            "Seconds still requires valid click tempo"
        );
    }
    for beats in [0, 17] {
        let mut bad = take.clone();
        bad.beats = beats;
        assert!(
            bad.validate().is_err(),
            "Seconds still requires valid click meter"
        );
    }
}

#[test]
fn seconds_frame_validation_rounds_once_and_refuses_mismatch_or_overflow() {
    // Exactly representable binary fractions exercise both sides of a
    // half-frame boundary without depending on decimal float rounding.
    for (seconds, frames) in [(0.5, 512), (0.500244140625, 512), (0.50048828125, 513)] {
        let take = seconds_reference(&[], 1024, frames, seconds);
        take.validate().unwrap();
        let mut bad = take;
        bad.frames += 1;
        assert!(bad.validate().is_err());
    }
    for (rate, frames) in [
        (1000, 500),
        (44_100, 22_050),
        (48_000, 24_000),
        (384_000, 192_000),
    ] {
        seconds_reference(&[], rate, frames, 0.5)
            .validate()
            .unwrap();
    }
    let mut take = seconds_reference(&[], 1024, MAX_CAPTURE_FRAMES, 16_384.0);
    take.validate().unwrap();
    for seconds in [16_384.5, u64::MAX as f64, f64::MAX] {
        take.seconds = Some(seconds);
        assert!(
            take.validate().is_err(),
            "oversized clocks must not saturate into a valid frame count"
        );
    }
}

#[test]
fn seconds_v4_raw_and_compressed_reopen_preserve_exact_pcm_and_refuse_corruption() {
    let pattern = [0.0, -0.0, 0.25, -1.5, f32::from_bits(1), -f32::from_bits(1)];
    let samples: Vec<_> = pattern.into_iter().cycle().take(24_000).collect();
    let compressed = encode_capture(48_000, &samples).unwrap();
    assert_eq!(compressed.codec, LoopCaptureCodec::WavpackF32V1);
    for encoded in [
        EncodedCapture {
            codec: LoopCaptureCodec::RawF32V1,
            bytes: encode_pcm(48_000, &samples).unwrap(),
        },
        compressed,
    ] {
        let mut take = seconds_reference(&encoded.bytes, 48_000, samples.len(), 0.5);
        take.codec = encoded.codec;
        let manifest = LoopCaptureManifest {
            version: 4,
            namespace: Some(NAMESPACE.into()),
            takes: vec![take],
        };
        let wire = serde_json::to_value(&manifest).unwrap();
        let reopened: LoopCaptureManifest = serde_json::from_value(wire.clone()).unwrap();
        reopened.validate().unwrap();
        assert_eq!(serde_json::to_value(&reopened).unwrap(), wire);
        let take = &reopened.takes[0];
        let decoded = decode_capture(&encoded.bytes, take).unwrap();
        assert_eq!(decoded.len(), samples.len());
        assert!(
            decoded
                .iter()
                .zip(&samples)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );

        let mut corrupt = encoded.bytes.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(decode_capture(&corrupt, take).is_err());
        let mut wrong_shape = take.clone();
        wrong_shape.frames += 1;
        assert!(decode_capture(&encoded.bytes, &wrong_shape).is_err());
        let mut wrong_seconds = take.clone();
        wrong_seconds.seconds = Some(1.0);
        assert!(decode_capture(&encoded.bytes, &wrong_seconds).is_err());

        for field in ["seconds", "clock"] {
            let mut missing = wire.clone();
            missing["takes"][0].as_object_mut().unwrap().remove(field);
            let retained: LoopCaptureManifest = serde_json::from_value(missing.clone()).unwrap();
            assert!(retained.validate().is_err());
            assert_eq!(serde_json::to_value(&retained).unwrap(), missing);
            assert!(!retained.is_empty());
            assert!(!retained.takes[0].legacy_clock());
        }
    }
}

#[test]
fn seconds_unknown_clock_and_codec_are_retained_without_admission() {
    for unknown_clock in [false, true] {
        let mut take = seconds_reference(&[], 1024, 512, 0.5);
        if unknown_clock {
            take.clock = Some("future-seconds-clock".into());
        } else {
            take.codec = LoopCaptureCodec::Unsupported("future-seconds-codec".into());
        }
        let wire = serde_json::to_value(&take).unwrap();
        let retained: LoopCaptureRef = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&retained).unwrap(), wire);
        if unknown_clock {
            assert!(retained.validate().is_err());
        } else {
            // Existing codec policy retains unknown names in a valid
            // manifest, but the decoder refuses them before reading PCM.
            retained.validate().unwrap();
        }
        assert!(decode_capture(&[], &retained).is_err());
    }
}
