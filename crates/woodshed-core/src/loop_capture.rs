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
mod tests;
