//! Portable Set capture references and exact, content-addressed mono PCM.
//!
//! The session owns identities and clocks; companion blobs contain only a
//! versioned sample rate/frame header and finite little-endian f32 samples.
//! Validation is explicit so a host can retain a refused reference and explain
//! it without discarding the rest of an authored session.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::loop_plan::LoopSegmentId;

pub const CAPTURE_VERSION: u32 = 1;
pub const MAX_CAPTURE_TAKES: usize = 256;
pub const MAX_CAPTURE_FRAMES: usize = 16_777_216;
pub const PCM_HEADER_BYTES: usize = 24;
pub const MAX_PCM_BYTES: usize = PCM_HEADER_BYTES + MAX_CAPTURE_FRAMES * 4;
const PCM_MAGIC: &[u8; 8] = b"WSHPCM\0\0";

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
        self.version == CAPTURE_VERSION && self.namespace.is_none() && self.takes.is_empty()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != CAPTURE_VERSION {
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
}

impl LoopCaptureRef {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.set.0 == 0 || self.id.card.0 == 0 {
            return Err("Loop capture requires a nonzero Set/Card identity.".into());
        }
        if !self.bpm.is_finite()
            || self.bpm < 1.0
            || !(1..=16).contains(&self.beats)
            || self.bars == 0
        {
            return Err("Loop capture has an invalid tempo, meter or hold.".into());
        }
        validate_pcm_shape(self.sample_rate, self.frames)?;
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
    bytes.extend_from_slice(&CAPTURE_VERSION.to_le_bytes());
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
    if bytes.len() < PCM_HEADER_BYTES || bytes.len() > MAX_PCM_BYTES || &bytes[..8] != PCM_MAGIC {
        return Err("Loop PCM has an invalid or truncated header.".into());
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let rate = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let frames = usize::try_from(u64::from_le_bytes(bytes[16..24].try_into().unwrap()))
        .map_err(|_| "Loop PCM frame count is not supported on this platform.")?;
    validate_pcm_shape(rate, frames)?;
    if version != CAPTURE_VERSION || rate != reference.sample_rate || frames != reference.frames {
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
            version: 2,
            ..Default::default()
        };
        assert!(!manifest.is_empty());
        assert!(manifest.validate().is_err());
    }
}
