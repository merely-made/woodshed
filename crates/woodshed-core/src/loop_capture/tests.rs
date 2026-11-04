use super::*;
use crate::working_sets::WorkingSetId;
use woodshedding::rehearsal::CardId;
const NAMESPACE: &str = "0123456789abcdef0123456789abcdef";

fn reference(bytes: &[u8], frames: usize) -> LoopCaptureRef {
    LoopCaptureRef {
        clock: None,
        seconds: None,
        id: LoopSegmentId {
            set: WorkingSetId(3),
            card: CardId(7),
        },
        bpm: 83.75,
        beats: 7,
        bars: 3,
        sample_rate: 48_000,
        frames,
        digest: pcm_digest(bytes),
        codec: LoopCaptureCodec::LegacyRawF32V1,
    }
}

#[test]
fn version_one_wire_and_exact_sample_bits_are_stable() {
    let samples = [0.0, -0.0, 0.25, -1.5, f32::from_bits(1)];
    let bytes = encode_pcm(48_000, &samples).unwrap();
    assert_eq!(
        &bytes[..PCM_HEADER_BYTES],
        b"WSHPCM\0\0\x01\0\0\0\x80\xbb\0\0\x05\0\0\0\0\0\0\0"
    );
    assert_eq!(
        &bytes[PCM_HEADER_BYTES..],
        &[
            0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 128, 62, 0, 0, 192, 191, 1, 0, 0, 0
        ]
    );
    let decoded = decode_pcm(&bytes, &reference(&bytes, samples.len())).unwrap();
    assert_eq!(
        decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        samples.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
}

fn compressed_reference(bytes: &[u8], rate: u32, frames: usize) -> LoopCaptureRef {
    let mut take = reference(bytes, frames);
    take.sample_rate = rate;
    take.codec = LoopCaptureCodec::WavpackF32V1;
    take
}

fn compressed_blob(rate: u32, samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(WAVPACK_MAGIC);
    bytes.extend_from_slice(&WAVPACK_VERSION.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(samples.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&wavicle::encode_float(1, rate, samples).unwrap());
    bytes
}

fn block_size(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize + 8
}

fn metadata_range(bytes: &[u8], block_offset: usize, id: u8) -> (usize, std::ops::Range<usize>) {
    use wavicle::format::meta;
    let mut offset = block_offset + wavicle::block::HEADER_LEN;
    let end = block_offset + block_size(bytes, block_offset);
    while offset < end {
        let raw_id = bytes[offset];
        let header = if raw_id & meta::LARGE == 0 { 2 } else { 4 };
        let words = if header == 2 {
            usize::from(bytes[offset + 1])
        } else {
            usize::from(bytes[offset + 1])
                | usize::from(bytes[offset + 2]) << 8
                | usize::from(bytes[offset + 3]) << 16
        };
        let stored_end = offset + header + words * 2;
        let logical_end = stored_end - usize::from(raw_id & meta::ODD_SIZE != 0);
        if raw_id & meta::ID_MASK == id {
            return (offset, offset + header..logical_end);
        }
        offset = stored_end;
    }
    panic!("test fixture lacks metadata {id}");
}

fn assert_rehashed_refused(bytes: &[u8], rate: u32, frames: usize) {
    let take = compressed_reference(bytes, rate, frames);
    assert!(decode_capture(bytes, &take).is_err());
}

#[test]
fn compressed_multiblock_custom_rate_preserves_signed_zero_and_subnormals() {
    let pattern = [0.0, -0.0, 0.25, -1.5, f32::from_bits(1), -f32::from_bits(1)];
    let samples: Vec<_> = pattern.into_iter().cycle().take(70_000).collect();
    let encoded = encode_capture(12_345, &samples).unwrap();
    assert_eq!(encoded.codec, LoopCaptureCodec::WavpackF32V1);
    assert!(encoded.bytes.len() < PCM_HEADER_BYTES + samples.len() * 4);
    assert_eq!(&encoded.bytes[..8], b"WSHWVC\0\0");
    assert_eq!(
        u32::from_le_bytes(encoded.bytes[8..12].try_into().unwrap()),
        1
    );
    assert_eq!(
        wavicle::Blocks::new(&encoded.bytes[PCM_HEADER_BYTES..]).count(),
        3
    );
    let decoded = decode_capture(
        &encoded.bytes,
        &compressed_reference(&encoded.bytes, 12_345, samples.len()),
    )
    .unwrap();
    assert!(
        decoded
            .iter()
            .zip(&samples)
            .all(|(decoded, source)| decoded.to_bits() == source.to_bits())
    );
}

#[test]
fn incompressible_and_small_capture_fallbacks_keep_exact_raw_v1_bytes() {
    let mut state = 0x1234_5678u32;
    let noise: Vec<_> = (0..8_192)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let mut bits = state;
            if bits & 0x7f80_0000 == 0x7f80_0000 {
                bits &= !0x0080_0000;
            }
            f32::from_bits(bits)
        })
        .collect();
    for samples in [&noise[..], &[-0.0][..]] {
        let encoded = encode_capture(48_000, samples).unwrap();
        assert_eq!(encoded.codec, LoopCaptureCodec::RawF32V1);
        assert_eq!(encoded.bytes, encode_pcm(48_000, samples).unwrap());
        let mut take = reference(&encoded.bytes, samples.len());
        take.codec = encoded.codec;
        let decoded = decode_capture(&encoded.bytes, &take).unwrap();
        assert!(
            decoded
                .iter()
                .zip(samples)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
    }
}

#[test]
fn legacy_manifest_wire_and_unknown_codecs_survive_roundtrip() {
    let bytes = encode_pcm(48_000, &[0.25]).unwrap();
    let old = LoopCaptureManifest {
        version: 1,
        namespace: Some(NAMESPACE.into()),
        takes: vec![reference(&bytes, 1)],
    };
    let wire = serde_json::to_value(&old).unwrap();
    assert!(wire["takes"][0].get("codec").is_none());
    let mut opened: LoopCaptureManifest = serde_json::from_value(wire.clone()).unwrap();
    opened.validate().unwrap();
    assert_eq!(serde_json::to_value(&opened).unwrap(), wire);
    assert_eq!(opened.takes[0].codec, LoopCaptureCodec::LegacyRawF32V1);
    assert_eq!(decode_capture(&bytes, &opened.takes[0]).unwrap(), [0.25]);
    opened.takes[0].codec = LoopCaptureCodec::RawF32V1;
    assert!(opened.validate().is_err());
    opened.version = CAPTURE_VERSION;
    opened.validate().unwrap();
    assert_eq!(
        serde_json::to_value(&opened).unwrap()["takes"][0]["codec"],
        "raw-f32-v1"
    );
    opened.takes[0].codec = LoopCaptureCodec::WavpackF32V1;
    opened.validate().unwrap();
    opened.version = 1;
    assert!(opened.validate().is_err());
    opened.version = CAPTURE_VERSION;
    opened.takes[0].codec = LoopCaptureCodec::Unsupported("future-lossless-v3".into());
    let wire = serde_json::to_value(&opened).unwrap();
    let reopened: LoopCaptureManifest = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(), wire);
    reopened.validate().unwrap();
    assert!(
        decode_capture(&bytes, &reopened.takes[0])
            .unwrap_err()
            .contains("Unsupported")
    );
}

#[test]
fn rehashed_compressed_header_frame_channel_and_order_mismatches_are_refused() {
    use wavicle::format::Flags;
    let samples = vec![0.0; 40_000];
    let bytes = compressed_blob(48_000, &samples);
    let start = PCM_HEADER_BYTES;
    let second = start + block_size(&bytes, start);
    for cut in [0, 7, 23, bytes.len() - 1] {
        assert_rehashed_refused(&bytes[..cut], 48_000, samples.len());
    }
    for size in [u32::MAX, u32::MAX - 7, 23, wavicle::format::MAX_BLOCK_SIZE] {
        let mut bad = bytes.clone();
        bad[start + 4..start + 8].copy_from_slice(&size.to_le_bytes());
        assert_rehashed_refused(&bad, 48_000, samples.len());
    }
    for (offset, value) in [
        (start + 12, 39_999u32), // wrong declared total
        (start + 16, 1),         // first frame must be zero
        (start + 20, 0),         // metadata-only blocks cannot hide content
        (start + 20, 32_769),    // decoder allocation bound is 32768
        (second + 16, 0),        // repeated/reordered block
        (second + 20, 8_000),    // aggregate exceeds reference
        (second + 20, 1),        // aggregate shorter than declared
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert_rehashed_refused(&bad, 48_000, samples.len());
    }
    let flags = u32::from_le_bytes(bytes[start + 24..start + 28].try_into().unwrap());
    for flags in [
        flags & !(1 << 2),
        flags | Flags::FALSE_STEREO,
        flags & !Flags::FLOAT_DATA,
        flags & !3,
        flags & !Flags::INITIAL_BLOCK,
        flags & !Flags::FINAL_BLOCK,
        (flags & !(0xf << 23)) | (9 << 23), // 44100 rather than 48000
    ] {
        let mut bad = bytes.clone();
        bad[start + 24..start + 28].copy_from_slice(&flags.to_le_bytes());
        assert_rehashed_refused(&bad, 48_000, samples.len());
    }
    let mut changed_version = bytes.clone();
    changed_version[second + 8..second + 10].copy_from_slice(&0x40fu16.to_le_bytes());
    assert_rehashed_refused(&changed_version, 48_000, samples.len());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_rehashed_refused(&trailing, 48_000, samples.len());
    let mut repeated = bytes.clone();
    repeated.extend_from_slice(&bytes[start..second]);
    assert_rehashed_refused(&repeated, 48_000, samples.len());
    let mut outer = bytes.clone();
    outer[8] = 2;
    assert_rehashed_refused(&outer, 48_000, samples.len());
    outer = bytes.clone();
    outer[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_rehashed_refused(&outer, 48_000, samples.len());
}

#[test]
fn rehashed_compressed_metadata_rate_crc_and_nonfinite_samples_are_refused() {
    use wavicle::format::meta;
    let frames = 4_096;
    let bytes = compressed_blob(12_345, &vec![0.25; frames]);
    let (rate_header, rate) = metadata_range(&bytes, PCM_HEADER_BYTES, meta::SAMPLE_RATE);
    let (float_header, _) = metadata_range(&bytes, PCM_HEADER_BYTES, meta::FLOAT_INFO);
    let mut wrong_rate = bytes.clone();
    wrong_rate[rate.start] ^= 1;
    assert_rehashed_refused(&wrong_rate, 12_345, frames);
    let mut malformed = bytes.clone();
    malformed[rate_header + 1] = 255;
    assert_rehashed_refused(&malformed, 12_345, frames);
    let mut unknown_required = bytes.clone();
    unknown_required[float_header] = 0x1f;
    assert_rehashed_refused(&unknown_required, 12_345, frames);
    let mut missing_float = bytes.clone();
    missing_float[float_header] = meta::RIFF_HEADER;
    assert_rehashed_refused(&missing_float, 12_345, frames);
    let mut corrupt_crc = bytes.clone();
    corrupt_crc[PCM_HEADER_BYTES + 28] ^= 1;
    assert_rehashed_refused(&corrupt_crc, 12_345, frames);
    for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(encode_capture(48_000, &[sample]).is_err());
        let bytes = compressed_blob(48_000, &vec![sample; frames]);
        let err =
            decode_capture(&bytes, &compressed_reference(&bytes, 48_000, frames)).unwrap_err();
        assert!(err.contains("nonfinite"), "{err}");
    }
}

#[test]
fn rehashed_entropy_escape_overflow_is_an_error_without_supplier_panic() {
    use wavicle::format::meta;
    let frames = 4_096;
    let mut bytes = compressed_blob(48_000, &vec![0.0; frames]);
    let (header, payload) = metadata_range(&bytes, PCM_HEADER_BYTES, meta::WV_BITSTREAM);
    // First skip the zero-run branch, then encode a 16-one escape whose
    // Elias value is u32::MAX. Adding LIMIT_ONES must refuse the stream.
    let mut bits = vec![0u8];
    bits.extend([1; 16]);
    bits.push(0);
    bits.extend([1; 32]);
    bits.push(0);
    bits.extend([1; 31]);
    let mut entropy = vec![0xffu8; 12];
    for (i, bit) in bits.into_iter().enumerate() {
        let mask = 1 << (i % 8);
        entropy[i / 8] = (entropy[i / 8] & !mask) | bit << (i % 8);
    }
    let mut sub = vec![meta::WV_BITSTREAM | meta::LARGE, 6, 0, 0];
    sub.extend_from_slice(&entropy);
    let stored_end = payload.end + usize::from(bytes[header] & meta::ODD_SIZE != 0);
    bytes.splice(header..stored_end, sub);
    let size = (bytes.len() - PCM_HEADER_BYTES - 8) as u32;
    bytes[PCM_HEADER_BYTES + 4..PCM_HEADER_BYTES + 8].copy_from_slice(&size.to_le_bytes());
    let err = decode_capture(&bytes, &compressed_reference(&bytes, 48_000, frames)).unwrap_err();
    // A caught panic has different text; this malformed stream must reach
    // the supplier's checked refusal even when panic unwinding is disabled.
    assert!(err.contains("decode failed: truncated"), "{err}");
}

#[test]
fn corruption_truncation_trailing_bytes_and_ref_disagreement_are_refused() {
    let bytes = encode_pcm(48_000, &[0.25, -0.5]).unwrap();
    let take = reference(&bytes, 2);
    for cut in [0, 7, 23, bytes.len() - 1] {
        assert!(decode_pcm(&bytes[..cut], &take).is_err());
    }
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(
        decode_pcm(&corrupt, &take)
            .unwrap_err()
            .contains("integrity")
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_pcm(&trailing, &take).is_err());
    let mut wrong = take.clone();
    wrong.sample_rate = 44_100;
    assert!(decode_pcm(&bytes, &wrong).is_err());
    wrong = take.clone();
    wrong.frames = 1;
    assert!(decode_pcm(&bytes, &wrong).is_err());
    let mut future = bytes.clone();
    future[8] = 2;
    assert!(decode_pcm(&future, &reference(&future, 2)).is_err());
    let mut huge = bytes.clone();
    huge[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(decode_pcm(&huge, &reference(&huge, 2)).is_err());
}

#[test]
fn nonfinite_samples_invalid_rates_and_path_shaped_digests_are_refused() {
    for rate in [0, 999, 384_001] {
        assert!(encode_pcm(rate, &[0.0]).is_err());
    }
    assert!(encode_pcm(48_000, &[]).is_err());
    for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(encode_pcm(48_000, &[sample]).is_err());
        let mut bytes = encode_pcm(48_000, &[0.0]).unwrap();
        bytes[PCM_HEADER_BYTES..].copy_from_slice(&sample.to_le_bytes());
        assert!(decode_pcm(&bytes, &reference(&bytes, 1)).is_err());
    }
    for digest in [
        "../session".to_string(),
        "A".repeat(64),
        "a".repeat(63),
        "g".repeat(64),
        format!("{}x", "a".repeat(64)),
    ] {
        assert!(pcm_key(NAMESPACE, &digest).is_err());
    }
    for namespace in [
        "../session".to_string(),
        "A".repeat(32),
        "a".repeat(31),
        "g".repeat(32),
    ] {
        assert!(pcm_key(&namespace, &"a".repeat(64)).is_err());
    }
}

#[test]
fn manifest_retains_owner_clock_and_enforces_unique_bounded_references() {
    let bytes = encode_pcm(48_000, &[0.25]).unwrap();
    let take = reference(&bytes, 1);
    let mut manifest = LoopCaptureManifest {
        namespace: Some(NAMESPACE.into()),
        takes: vec![take.clone()],
        ..Default::default()
    };
    manifest.validate().unwrap();
    assert_eq!(
        serde_json::from_str::<LoopCaptureManifest>(&serde_json::to_string(&manifest).unwrap())
            .unwrap(),
        manifest
    );
    assert_eq!(
        serde_json::from_str::<LoopCaptureManifest>("{}").unwrap(),
        LoopCaptureManifest::default()
    );
    manifest.takes.push(take.clone());
    assert!(manifest.validate().is_err());
    manifest.takes[1].id.set = WorkingSetId(4);
    manifest.validate().unwrap();
    manifest.takes[0].frames = MAX_CAPTURE_FRAMES;
    assert!(manifest.validate().is_err());
    manifest.takes[0].frames = usize::MAX;
    assert!(manifest.validate().is_err());
    manifest.takes = (1..=MAX_CAPTURE_TAKES + 1)
        .map(|id| {
            let mut x = take.clone();
            x.id.card = CardId(id as u64);
            x
        })
        .collect();
    assert!(manifest.validate().is_err());
    let mut bad = take.clone();
    bad.id.set = WorkingSetId(0);
    assert!(bad.validate().is_err());
    bad = take.clone();
    bad.id.card = CardId(0);
    assert!(bad.validate().is_err());
    for bpm in [f32::NAN, f32::INFINITY, 0.5] {
        let mut bad = take.clone();
        bad.bpm = bpm;
        assert!(bad.validate().is_err());
    }
    for beats in [0, 17] {
        let mut bad = take.clone();
        bad.beats = beats;
        assert!(bad.validate().is_err());
    }
    let mut bad = take.clone();
    bad.bars = 0;
    assert!(bad.validate().is_err());
    manifest.takes = vec![take];
    manifest.namespace = None;
    assert!(manifest.validate().unwrap_err().contains("namespace"));
    manifest = LoopCaptureManifest {
        namespace: Some("invalid".into()),
        ..Default::default()
    };
    assert!(manifest.validate().is_err());
    manifest = LoopCaptureManifest {
        version: CAPTURE_VERSION + 1,
        ..Default::default()
    };
    assert!(!manifest.is_empty());
    assert!(manifest.validate().is_err());
}
#[test]
fn missing_clock_stays_legacy_and_explicit_policies_require_v3() {
    let take = reference(&encode_pcm(48_000, &[0.25]).unwrap(), 1);
    let old = serde_json::to_value(&take).unwrap();
    assert!(old.get("clock").is_none());
    let mut decoded: LoopCaptureRef = serde_json::from_value(old.clone()).unwrap();
    assert!(decoded.legacy_clock());
    assert_eq!(serde_json::to_value(&decoded).unwrap(), old);
    decoded.clock = Some(PER_BEAT_CLOCK.into());
    let mut manifest = LoopCaptureManifest {
        namespace: Some(NAMESPACE.into()),
        takes: vec![decoded],
        ..Default::default()
    };
    manifest.validate().unwrap();
    manifest.version = 2;
    assert!(manifest.validate().unwrap_err().contains("version 3"));
    manifest.version = CAPTURE_VERSION;
    manifest.takes[0].clock = Some("future-clock-v4".into());
    let wire = serde_json::to_value(&manifest).unwrap();
    let reopened: LoopCaptureManifest = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(), wire);
    assert!(reopened.validate().unwrap_err().contains("clock policy"));
}

#[path = "tests/seconds.rs"]
mod seconds;
