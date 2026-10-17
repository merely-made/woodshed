//! Portable Set capture references and exact, content-addressed mono PCM.
//!
//! The session owns identities and clocks; companion blobs contain only a
//! versioned sample rate/frame header and finite, exact f32 samples. New blobs
//! use lossless WavPack only when smaller than the original little-endian PCM.
//! Validation is explicit so a host can retain a refused reference and explain
//! it without discarding the rest of an authored session.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::loop_plan::LoopSegmentId;

pub const CAPTURE_VERSION: u32 = 4;
/// Absent clock fields retain whole-block clocks from manifests V1/V2.
pub const PER_BEAT_CLOCK: &str = "per-beat-v1";
/// Explicit active seconds, rounded once at the recorded sample rate.
pub const SECONDS_CLOCK: &str = "seconds-v1";
pub const PCM_VERSION: u32 = 1;
pub const MAX_CAPTURE_TAKES: usize = 256;
pub const MAX_CAPTURE_FRAMES: usize = 16_777_216;
pub const PCM_HEADER_BYTES: usize = 24;
pub const MAX_PCM_BYTES: usize = PCM_HEADER_BYTES + MAX_CAPTURE_FRAMES * 4;
const PCM_MAGIC: &[u8; 8] = b"WSHPCM\0\0";
const WAVPACK_MAGIC: &[u8; 8] = b"WSHWVC\0\0";
const WAVPACK_VERSION: u32 = 1;
const MAX_WAVPACK_BLOCK_FRAMES: u32 = 32_768;

/// Missing codec fields describe old raw blobs, even inside a V2 manifest.
/// Explicit raw marks a current-policy fallback that need not be recompressed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LoopCaptureCodec {
    #[default]
    LegacyRawF32V1,
    RawF32V1,
    WavpackF32V1,
    /// Preserve a future codec name as a refused reference, not a lost session.
    Unsupported(String),
}

impl LoopCaptureCodec {
    fn is_legacy(&self) -> bool {
        matches!(self, Self::LegacyRawF32V1)
    }
}

impl Serialize for LoopCaptureCodec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::LegacyRawF32V1 => "legacy-raw-f32-v1",
            Self::RawF32V1 => "raw-f32-v1",
            Self::WavpackF32V1 => "wavpack-f32-v1",
            Self::Unsupported(name) => name,
        })
    }
}

impl<'de> Deserialize<'de> for LoopCaptureCodec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Ok(match name.as_str() {
            "legacy-raw-f32-v1" => Self::LegacyRawF32V1,
            "raw-f32-v1" => Self::RawF32V1,
            "wavpack-f32-v1" => Self::WavpackF32V1,
            _ => Self::Unsupported(name),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedCapture {
    pub codec: LoopCaptureCodec,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoopCaptureManifest {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub takes: Vec<LoopCaptureRef>,
}

impl Default for LoopCaptureManifest {
    fn default() -> Self {
        Self {
            version: CAPTURE_VERSION,
            namespace: None,
            takes: Vec::new(),
        }
    }
}

impl LoopCaptureManifest {
    pub fn is_empty(&self) -> bool {
        matches!(self.version, 1 | 2 | 3 | CAPTURE_VERSION)
            && self.namespace.is_none()
            && self.takes.is_empty()
    }

    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.version, 1 | 2 | 3 | CAPTURE_VERSION) {
            return Err("Unsupported loop capture manifest version.".into());
        }
        if self.takes.len() > MAX_CAPTURE_TAKES {
            return Err("Loop capture manifest exceeds 256 takes.".into());
        }
        match &self.namespace {
            Some(namespace) => validate_namespace(namespace)?,
            None if !self.takes.is_empty() => {
                return Err("Loop capture takes require an artifact namespace.".into());
            },
            None => {},
        }
        let mut seen = BTreeSet::new();
        let mut frames = 0usize;
        for take in &self.takes {
            take.validate()?;
            if self.version < 4 && take.seconds.is_some() {
                return Err("Seconds capture clocks require manifest version 4.".into());
            }
            if self.version < 3 && take.clock.is_some() {
                return Err("Explicit capture clock policies require manifest version 3.".into());
            }
            if self.version == 1 && take.codec != LoopCaptureCodec::LegacyRawF32V1 {
                return Err("Version-one loop capture manifests require legacy raw PCM.".into());
            }
            if !seen.insert(take.id) {
                return Err("Loop capture manifest repeats a Set/Card identity.".into());
            }
            frames = frames
                .checked_add(take.frames)
                .filter(|total| *total <= MAX_CAPTURE_FRAMES)
                .ok_or("Loop capture manifest exceeds its total frame budget.")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoopCaptureRef {
    pub id: LoopSegmentId,
    pub bpm: f32,
    pub beats: u8,
    pub bars: u8,
    pub sample_rate: u32,
    pub frames: usize,
    pub digest: String,
    #[serde(default, skip_serializing_if = "LoopCaptureCodec::is_legacy")]
    pub codec: LoopCaptureCodec,
    /// Missing means the original whole-block clock; new captures explicitly
    /// name per-beat rounding or active seconds. Unknown policies survive decoding but refuse
    /// admission, retaining their reference and immutable companion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<String>,
    /// Effective active duration required under `seconds-v1`, absent for Bars.
    /// Never infer Seconds from PCM length or the current Card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
}

impl LoopCaptureRef {
    pub fn legacy_clock(&self) -> bool {
        self.clock.is_none() && self.seconds.is_none()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.set.0 == 0 || self.id.card.0 == 0 {
            return Err("Loop capture requires a nonzero Set/Card identity.".into());
        }
        if !self.bpm.is_finite() || self.bpm < 1.0 || !(1..=16).contains(&self.beats) {
            return Err("Loop capture has an invalid tempo, meter or hold.".into());
        }
        validate_pcm_shape(self.sample_rate, self.frames)?;
        match (self.clock.as_deref(), self.seconds) {
            (Some(SECONDS_CLOCK), Some(seconds)) => {
                if self.bars != 0 || !seconds.is_finite() || seconds < 0.5 {
                    return Err("Seconds capture requires a finite duration of at least 0.5 seconds and zero bars.".into());
                }
                let frames = audio_primitives::timing::seconds_to_frames(seconds, self.sample_rate)
                    .map_err(|reason| format!("Seconds capture clock is invalid: {reason}."))?;
                // Shared timing owns rounding and the checked integer boundary.
                // Apply the capture budget in u64 before narrowing to usize.
                if frames > MAX_CAPTURE_FRAMES as u64 || frames as usize != self.frames {
                    return Err("Seconds capture frame count differs from its recorded clock or exceeds its bounded budget.".into());
                }
            },
            (None | Some(PER_BEAT_CLOCK), None) if self.bars > 0 => {},
            (Some(SECONDS_CLOCK), None) => {
                return Err("Seconds capture clock requires explicit seconds metadata.".into());
            },
            (None | Some(PER_BEAT_CLOCK), Some(_)) => {
                return Err("Seconds metadata requires the seconds-v1 capture clock.".into());
            },
            (None | Some(PER_BEAT_CLOCK), None) => {
                return Err("Bars capture requires a positive bar count.".into());
            },
            _ => return Err("Unsupported loop capture clock policy.".into()),
        }
        validate_hex(&self.digest, 64, "digest")?;
        Ok(())
    }
}

fn validate_pcm_shape(rate: u32, frames: usize) -> Result<(), String> {
    if !(1_000..=384_000).contains(&rate) {
        return Err("Loop PCM sample rate must be within 1000..=384000 Hz.".into());
    }
    if frames == 0 || frames > MAX_CAPTURE_FRAMES {
        return Err("Loop PCM frame count is empty or exceeds its bounded budget.".into());
    }
    Ok(())
}

/// Encode exact mono f32 bit patterns; no conversion, normalization or clipping.
pub fn encode_pcm(rate: u32, samples: &[f32]) -> Result<Vec<u8>, String> {
    validate_pcm_shape(rate, samples.len())?;
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err("Loop PCM contains a nonfinite sample.".into());
    }
    let mut bytes = Vec::with_capacity(PCM_HEADER_BYTES + samples.len() * 4);
    bytes.extend_from_slice(PCM_MAGIC);
    bytes.extend_from_slice(&PCM_VERSION.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(samples.len() as u64).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}

/// Validate identity/clock metadata, exact header shape and the full blob digest
/// before allocating decoded samples. Missing or corrupt PCM is never silence.
pub fn decode_pcm(bytes: &[u8], reference: &LoopCaptureRef) -> Result<Vec<f32>, String> {
    reference.validate()?;
    if let LoopCaptureCodec::Unsupported(name) = &reference.codec {
        return Err(format!("Unsupported loop capture codec: {name}."));
    }
    if reference.codec == LoopCaptureCodec::WavpackF32V1 {
        return Err("Loop PCM reference requires a compressed capture decoder.".into());
    }
    if bytes.len() < PCM_HEADER_BYTES || bytes.len() > MAX_PCM_BYTES || &bytes[..8] != PCM_MAGIC {
        return Err("Loop PCM has an invalid or truncated header.".into());
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let rate = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let frames = usize::try_from(u64::from_le_bytes(bytes[16..24].try_into().unwrap()))
        .map_err(|_| "Loop PCM frame count is not supported on this platform.")?;
    validate_pcm_shape(rate, frames)?;
    if version != PCM_VERSION || rate != reference.sample_rate || frames != reference.frames {
        return Err(
            "Loop PCM version, sample rate or frame count differs from its reference.".into(),
        );
    }
    if bytes.len() != PCM_HEADER_BYTES + frames * 4 {
        return Err("Loop PCM payload length differs from its frame header.".into());
    }
    if pcm_digest(bytes) != reference.digest {
        return Err("Loop PCM integrity check failed.".into());
    }
    let sample = |chunk: &[u8]| f32::from_le_bytes(chunk.try_into().unwrap());
    if bytes[PCM_HEADER_BYTES..]
        .chunks_exact(4)
        .any(|chunk| !sample(chunk).is_finite())
    {
        return Err("Loop PCM contains a nonfinite sample.".into());
    }
    Ok(bytes[PCM_HEADER_BYTES..]
        .chunks_exact(4)
        .map(sample)
        .collect())
}

/// Encode current-policy mono capture storage, retaining exact raw V1 bytes
/// whenever compression is refused or a complete versioned compressed blob
/// would be equally large or larger. Raw fallback marks the policy as evaluated.
pub fn encode_capture(rate: u32, samples: &[f32]) -> Result<EncodedCapture, String> {
    let raw = encode_pcm(rate, samples)?;
    let compressed = match wavicle::encode_float(1, rate, samples) {
        Ok(bytes) if PCM_HEADER_BYTES + bytes.len() < raw.len() => bytes,
        _ => {
            return Ok(EncodedCapture {
                codec: LoopCaptureCodec::RawF32V1,
                bytes: raw,
            });
        },
    };
    let mut bytes = Vec::with_capacity(PCM_HEADER_BYTES + compressed.len());
    bytes.extend_from_slice(WAVPACK_MAGIC);
    bytes.extend_from_slice(&WAVPACK_VERSION.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(samples.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&compressed);
    Ok(EncodedCapture {
        codec: LoopCaptureCodec::WavpackF32V1,
        bytes,
    })
}

/// Check the complete blob and every block's resource/format contract before
/// invoking Wavicle, which allocates decoded sample storage. A malformed blob
/// is refused; it never becomes replacement silence or truncated material.
pub fn decode_capture(bytes: &[u8], reference: &LoopCaptureRef) -> Result<Vec<f32>, String> {
    if reference.codec != LoopCaptureCodec::WavpackF32V1 {
        return decode_pcm(bytes, reference);
    }
    reference.validate()?;
    if bytes.len() < PCM_HEADER_BYTES
        || bytes.len() > MAX_PCM_BYTES
        || bytes.len() >= PCM_HEADER_BYTES + reference.frames * 4
        || &bytes[..8] != WAVPACK_MAGIC
    {
        return Err("Loop compressed PCM has an invalid header or storage bound.".into());
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let rate = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let frames = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
    if version != WAVPACK_VERSION
        || rate != reference.sample_rate
        || frames != reference.frames as u64
    {
        return Err("Loop compressed PCM header differs from its reference.".into());
    }
    if pcm_digest(bytes) != reference.digest {
        return Err("Loop PCM integrity check failed.".into());
    }
    let stream = &bytes[PCM_HEADER_BYTES..];
    preflight_wavpack(stream, reference)?;
    // This is an additional boundary for unexpected supplier panics in unwind
    // builds; the framing and quotas above do not depend on catching a panic.
    let decoded = std::panic::catch_unwind(|| wavicle::decode_stream(stream))
        .map_err(|_| "Loop compressed PCM decoder refused malformed content.")?
        .map_err(|error| format!("Loop compressed PCM decode failed: {error}"))?;
    if decoded.channels != 1
        || decoded.sample_rate != reference.sample_rate
        || decoded.bits_per_sample != 32
        || !decoded.is_float
        || decoded.samples.len() != reference.frames
    {
        return Err("Loop compressed PCM decoded format differs from its reference.".into());
    }
    if decoded
        .samples
        .iter()
        .any(|bits| !f32::from_bits(*bits as u32).is_finite())
    {
        return Err("Loop PCM contains a nonfinite sample.".into());
    }
    Ok(decoded
        .samples
        .into_iter()
        .map(|bits| f32::from_bits(bits as u32))
        .collect())
}

fn preflight_wavpack(mut stream: &[u8], reference: &LoopCaptureRef) -> Result<(), String> {
    use wavicle::format::meta;

    let mut expected_index = 0u64;
    let mut stream_version = None;
    while !stream.is_empty() {
        if stream.len() < wavicle::block::HEADER_LEN {
            return Err("Loop compressed PCM has a truncated block header.".into());
        }
        // Guard untrusted u32 size arithmetic before any public Block parser
        // or iterator sees the bytes. The exact slice also refuses truncation.
        let size = u32::from_le_bytes(stream[4..8].try_into().unwrap())
            .checked_add(8)
            .filter(|size| {
                *size >= wavicle::block::HEADER_LEN as u32
                    && *size <= wavicle::format::MAX_BLOCK_SIZE
            })
            .ok_or("Loop compressed PCM has an invalid block size.")? as usize;
        if size > stream.len() {
            return Err("Loop compressed PCM has a truncated block.".into());
        }
        let header = wavicle::BlockHeader::parse(&stream[..size])
            .map_err(|error| format!("Loop compressed PCM block refused: {error}"))?;
        if header.block_samples == 0 || header.block_samples > MAX_WAVPACK_BLOCK_FRAMES {
            return Err("Loop compressed PCM has an empty or oversized audio block.".into());
        }
        if !header.flags.mono_stored()
            || header.flags.output_channels() != 1
            || !header.flags.is_float()
            || header.flags.bytes_per_sample() != 4
            || !header.flags.initial_block()
            || !header.flags.final_block()
        {
            return Err("Loop compressed PCM requires complete mono float32 blocks.".into());
        }
        if header.block_index != expected_index
            || header.total_samples != Some(reference.frames as u64)
            || stream_version.is_some_and(|version| version != header.version)
        {
            return Err("Loop compressed PCM block order or declared length differs.".into());
        }
        stream_version = Some(header.version);
        expected_index = expected_index
            .checked_add(u64::from(header.block_samples))
            .filter(|frames| *frames <= reference.frames as u64)
            .ok_or("Loop compressed PCM exceeds its referenced frame budget.")?;
        let block = wavicle::Block {
            header,
            metadata: &stream[wavicle::block::HEADER_LEN..size],
        };
        let mut seen = BTreeSet::new();
        let mut custom_rate = None;
        for sub in block.sub_blocks() {
            let sub =
                sub.map_err(|error| format!("Loop compressed PCM framing refused: {error}"))?;
            wavicle::metadata::check_scope(sub)
                .map_err(|error| format!("Loop compressed PCM metadata refused: {error}"))?;
            if !seen.insert(sub.id) {
                return Err("Loop compressed PCM repeats block metadata.".into());
            }
            match sub.id {
                meta::SAMPLE_RATE => {
                    if sub.data.len() != 3 {
                        return Err("Loop compressed PCM has an invalid custom rate.".into());
                    }
                    let rate = u32::from(sub.data[0])
                        | u32::from(sub.data[1]) << 8
                        | u32::from(sub.data[2]) << 16;
                    if rate != reference.sample_rate {
                        return Err(
                            "Loop compressed PCM custom rate differs from its reference.".into(),
                        );
                    }
                    custom_rate = Some(rate);
                },
                meta::CHANNEL_INFO if sub.data.first() != Some(&1) => {
                    return Err("Loop compressed PCM metadata requires mono audio.".into());
                },
                meta::FLOAT_INFO if sub.data.len() != 4 => {
                    return Err("Loop compressed PCM has invalid float metadata.".into());
                },
                meta::ENTROPY_VARS if sub.data.len() != 6 => {
                    return Err("Loop compressed PCM has invalid mono entropy metadata.".into());
                },
                meta::WV_BITSTREAM if sub.data.is_empty() => {
                    return Err("Loop compressed PCM has an empty audio bitstream.".into());
                },
                _ => {},
            }
        }
        if header.flags.sample_rate().or(custom_rate) != Some(reference.sample_rate) {
            return Err("Loop compressed PCM block rate differs from its reference.".into());
        }
        if [
            meta::DECORR_TERMS,
            meta::ENTROPY_VARS,
            meta::FLOAT_INFO,
            meta::WV_BITSTREAM,
        ]
        .iter()
        .any(|id| !seen.contains(id))
        {
            return Err("Loop compressed PCM block lacks required audio metadata.".into());
        }
        stream = &stream[size..];
    }
    if expected_index != reference.frames as u64 {
        return Err("Loop compressed PCM actual length differs from its reference.".into());
    }
    Ok(())
}

/// The address covers the complete encoded blob, including rate and frame count.
pub fn pcm_digest(bytes: &[u8]) -> String {
    muniment::blob::Hash::of(bytes).to_hex()
}

pub fn validate_namespace(namespace: &str) -> Result<(), String> {
    validate_hex(namespace, 32, "artifact namespace")
}

fn validate_hex(value: &str, length: usize, label: &str) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "Loop PCM {label} must be exactly {length} lowercase hexadecimal characters."
        ));
    }
    Ok(())
}

pub fn pcm_key(namespace: &str, digest: &str) -> Result<String, String> {
    validate_namespace(namespace)?;
    validate_hex(digest, 64, "digest")?;
    Ok(format!("loop-pcm/{namespace}/{digest}"))
}

#[cfg(test)]
mod tests {
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

    fn metadata_range(
        bytes: &[u8],
        block_offset: usize,
        id: u8,
    ) -> (usize, std::ops::Range<usize>) {
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
        let err =
            decode_capture(&bytes, &compressed_reference(&bytes, 48_000, frames)).unwrap_err();
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
                decoded.iter().zip(samples).all(|(a, b)| a.to_bits() == b.to_bits())
            );

            let empty = LoopCaptureManifest {
                version,
                ..Default::default()
            };
            empty.validate().unwrap();
            assert!(empty.is_empty(), "old empty manifests must remain omittable");
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
            assert!(bad.validate().is_err(), "forged clock/seconds/bars: {bad:?}");
            manifest.takes = vec![bad];
            assert!(manifest.validate().is_err());
        }
        for seconds in [
            f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0, 0.4999, f64::MAX,
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
        for (rate, frames) in [(1000, 500), (44_100, 22_050), (48_000, 24_000), (384_000, 192_000)] {
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
            assert!(decoded.iter().zip(&samples).all(|(a, b)| a.to_bits() == b.to_bits()));

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
}
