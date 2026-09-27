//! Conservative, bounded audio-anchor matching. Feed decoded PCM before any
//! speed or gain effects. Positions are estimates on a 100 ms grid, not proof
//! of an exact edit boundary. Version 1 is validated on synthetic fixtures;
//! speech, lossy codecs and real dynamic advertisements need separate receipts.

use std::{collections::VecDeque, error::Error, fmt};

pub const FINGERPRINT_VERSION: u16 = 1;
pub const FRAME_MS: u16 = 100;
pub const BANDS: usize = 16;
pub const MAX_FRAMES: usize = 600;
const RATE: u32 = 8_000;
const FRAME_SAMPLES: usize = 800;
pub type FeatureFrame = [u8; BANDS];

#[derive(Clone, Debug, PartialEq)]
pub struct Fingerprint {
    pub version: u16,
    pub frame_ms: u16,
    /// Offset from the first complete feature frame to the original anchor.
    /// Includes at most one unfinished frame, whose audio is not fingerprinted.
    pub anchor_offset_ms: u32,
    pub frames: Vec<FeatureFrame>,
}

impl Fingerprint {
    pub fn covered_ms(&self) -> u32 {
        self.frames.len() as u32 * u32::from(self.frame_ms)
    }

    pub fn validate(&self) -> Result<(), AlignmentError> {
        if self.version != FINGERPRINT_VERSION || self.frame_ms != FRAME_MS {
            return Err(AlignmentError::UnsupportedVersion);
        }
        if !(30..=MAX_FRAMES).contains(&self.frames.len()) {
            return Err(AlignmentError::InsufficientMaterial);
        }
        if self.anchor_offset_ms < self.covered_ms()
            || self.anchor_offset_ms >= self.covered_ms() + u32::from(FRAME_MS)
        {
            return Err(AlignmentError::InvalidFingerprint);
        }
        usable(&self.frames)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FingerprintConfig {
    pub window_ms: u32,
    pub history_ms: u32,
    pub min_material_ms: u32,
    pub silence_rms: f32,
}

impl Default for FingerprintConfig {
    fn default() -> Self {
        Self {
            window_ms: 20_000,
            history_ms: 65_000,
            min_material_ms: 3_000,
            silence_rms: 0.0001,
        }
    }
}

impl FingerprintConfig {
    fn validate(self) -> Result<Self, AlignmentError> {
        if !(3_000..=60_000).contains(&self.window_ms)
            || !self.window_ms.is_multiple_of(u32::from(FRAME_MS))
            || !(self.window_ms..=90_000).contains(&self.history_ms)
            || !self.history_ms.is_multiple_of(u32::from(FRAME_MS))
            || !(3_000..=self.window_ms).contains(&self.min_material_ms)
            || !self.silence_rms.is_finite()
            || !(0.0..=0.1).contains(&self.silence_rms)
        {
            return Err(AlignmentError::InvalidConfig);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AlignmentConfig {
    pub min_score: f32,
    pub min_tail_score: f32,
    pub min_tail_frame_score: f32,
    pub min_separation: f32,
    pub tail_ms: u32,
    /// Matches within this distance are one timing neighbourhood.
    pub ambiguity_distance_ms: u32,
    pub silence_rms: f32,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            min_score: 0.94,
            min_tail_score: 0.97,
            min_tail_frame_score: 0.85,
            min_separation: 0.04,
            tail_ms: 2_000,
            ambiguity_distance_ms: 500,
            silence_rms: 0.0001,
        }
    }
}

impl AlignmentConfig {
    fn validate(self) -> Result<Self, AlignmentError> {
        if [
            self.min_score,
            self.min_tail_score,
            self.min_tail_frame_score,
            self.min_separation,
            self.silence_rms,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || !(0.8..=1.0).contains(&self.min_score)
            || !(0.8..=1.0).contains(&self.min_tail_score)
            || !(0.5..=1.0).contains(&self.min_tail_frame_score)
            || !(0.01..=0.5).contains(&self.min_separation)
            || !(500..=3_000).contains(&self.tail_ms)
            || !self.tail_ms.is_multiple_of(u32::from(FRAME_MS))
            || !(100..=1_000).contains(&self.ambiguity_distance_ms)
            || !(0.0..=0.1).contains(&self.silence_rms)
        {
            return Err(AlignmentError::InvalidConfig);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AlignmentError {
    InvalidConfig,
    InvalidPcm,
    InvalidFingerprint,
    UnsupportedVersion,
    InsufficientMaterial,
    LowInformation,
    NoMatch,
    Ambiguous,
    AnchorDiscontinuity,
}

impl fmt::Display for AlignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfig => "invalid audio alignment configuration",
            Self::InvalidPcm => "PCM must have complete interleaved frames and finite samples",
            Self::InvalidFingerprint => "invalid fingerprint anchor interval",
            Self::UnsupportedVersion => "unsupported audio fingerprint version",
            Self::InsufficientMaterial => "insufficient audio near this anchor",
            Self::LowInformation => "audio has too little distinctive material",
            Self::NoMatch => "no sufficiently similar audio was found",
            Self::Ambiguous => "multiple plausible audio matches were found",
            Self::AnchorDiscontinuity => "matching audio does not establish the anchor boundary",
        })
    }
}
impl Error for AlignmentError {}

struct Extractor {
    sample_rate: u32,
    channels: usize,
    phase: u32,
    sum: f64,
    count: u32,
    samples: Vec<f32>,
    silence_rms: f32,
}

impl Extractor {
    fn new(sample_rate: u32, channels: usize, silence_rms: f32) -> Result<Self, AlignmentError> {
        if !(RATE..=192_000).contains(&sample_rate) || !(1..=8).contains(&channels) {
            return Err(AlignmentError::InvalidConfig);
        }
        Ok(Self {
            sample_rate,
            channels,
            phase: 0,
            sum: 0.0,
            count: 0,
            samples: Vec::with_capacity(FRAME_SAMPLES),
            silence_rms,
        })
    }

    fn validate_pcm(&self, pcm: &[f32]) -> Result<(), AlignmentError> {
        if !pcm.len().is_multiple_of(self.channels)
            || pcm.iter().any(|s| !s.is_finite() || s.abs() > 8.0)
        {
            return Err(AlignmentError::InvalidPcm);
        }
        Ok(())
    }

    fn push_frame(&mut self, samples: &[f32]) -> Option<FeatureFrame> {
        self.sum += samples.iter().map(|v| f64::from(*v)).sum::<f64>() / self.channels as f64;
        self.count += 1;
        self.phase += RATE;
        if self.phase < self.sample_rate {
            return None;
        }
        self.phase -= self.sample_rate;
        self.samples.push((self.sum / f64::from(self.count)) as f32);
        self.sum = 0.0;
        self.count = 0;
        if self.samples.len() != FRAME_SAMPLES {
            return None;
        }
        let feature = features(&self.samples, self.silence_rms);
        self.samples.clear();
        Some(feature)
    }
}

// A small Goertzel bank, independently implemented here. L2-normalized band
// amplitudes remove overall gain; a Hann window reduces phase sensitivity.
fn features(samples: &[f32], silence_rms: f32) -> FeatureFrame {
    let power = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    if power.sqrt() <= silence_rms {
        return [0; BANDS];
    }
    let windowed: [f32; FRAME_SAMPLES] = std::array::from_fn(|index| {
        samples[index]
            * (0.5
                - 0.5 * (std::f32::consts::TAU * index as f32 / (FRAME_SAMPLES - 1) as f32).cos())
    });
    let mut bands = [0.0_f32; BANDS];
    for (band, value) in bands.iter_mut().enumerate() {
        let frequency = 110.0 * (3_500.0_f32 / 110.0).powf(band as f32 / (BANDS - 1) as f32);
        let coefficient = 2.0 * (std::f32::consts::TAU * frequency / RATE as f32).cos();
        let (mut previous, mut older) = (0.0_f32, 0.0_f32);
        for sample in &windowed {
            let next = sample + coefficient * previous - older;
            older = previous;
            previous = next;
        }
        *value = (previous * previous + older * older - coefficient * previous * older)
            .max(0.0)
            .sqrt();
    }
    let norm = bands.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm < 1e-8 {
        return [0; BANDS];
    }
    std::array::from_fn(|i| (bands[i] / norm * 255.0).round() as u8)
}

fn similarity(a: &FeatureFrame, b: &FeatureFrame) -> f32 {
    let mut dot = 0_u32;
    let mut aa = 0_u32;
    let mut bb = 0_u32;
    for (&a, &b) in a.iter().zip(b) {
        dot += u32::from(a) * u32::from(b);
        aa += u32::from(a).pow(2);
        bb += u32::from(b).pow(2);
    }
    if aa == 0 || bb == 0 {
        return 0.0;
    }
    (dot as f32 / (aa as f32 * bb as f32).sqrt()).min(1.0)
}

fn usable(frames: &[FeatureFrame]) -> Result<(), AlignmentError> {
    let active = frames.iter().filter(|f| f.iter().any(|v| *v != 0)).count();
    let variation = frames
        .windows(2)
        .map(|f| 1.0 - similarity(&f[0], &f[1]))
        .sum::<f32>();
    if active * 4 < frames.len() * 3 || variation < frames.len() as f32 * 0.015 {
        return Err(AlignmentError::LowInformation);
    }
    // A silent tail cannot establish the anchor even when earlier audio matches.
    if frames
        .iter()
        .rev()
        .take(5)
        .any(|f| f.iter().all(|v| *v == 0))
    {
        return Err(AlignmentError::LowInformation);
    }
    Ok(())
}

/// Retains only the configured preceding audio window. Reset by constructing a
/// new builder after a seek, discontinuity or PCM format change.
pub struct FingerprintBuilder {
    extractor: Extractor,
    config: FingerprintConfig,
    frames: VecDeque<FeatureFrame>,
    frames_seen: u64,
}

impl FingerprintBuilder {
    pub fn new(
        sample_rate: u32,
        channels: usize,
        config: FingerprintConfig,
    ) -> Result<Self, AlignmentError> {
        let config = config.validate()?;
        Ok(Self {
            extractor: Extractor::new(sample_rate, channels, config.silence_rms)?,
            config,
            frames: VecDeque::new(),
            frames_seen: 0,
        })
    }

    pub fn push(&mut self, pcm: &[f32]) -> Result<(), AlignmentError> {
        self.extractor.validate_pcm(pcm)?;
        for samples in pcm.chunks_exact(self.extractor.channels) {
            if let Some(frame) = self.extractor.push_frame(samples) {
                self.frames_seen += 1;
                self.frames.push_back(frame);
                if self.frames.len() > (self.config.history_ms / u32::from(FRAME_MS)) as usize {
                    self.frames.pop_front();
                }
            }
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> Result<Fingerprint, AlignmentError> {
        let anchor = self.frames_seen * u64::from(FRAME_MS)
            + self.extractor.samples.len() as u64 * 1_000 / u64::from(RATE);
        self.fingerprint_at(anchor, self.config.window_ms)
    }

    /// Select preceding context for a presented source anchor, measured from
    /// this builder's creation/reset. This keeps decoded lookahead out of the
    /// note. The caller owns absolute source offsets and seek/reset detection.
    pub fn fingerprint_at(
        &self,
        anchor_relative_ms: u64,
        window_ms: u32,
    ) -> Result<Fingerprint, AlignmentError> {
        if !(3_000..=self.config.window_ms).contains(&window_ms)
            || !window_ms.is_multiple_of(u32::from(FRAME_MS))
        {
            return Err(AlignmentError::InvalidConfig);
        }
        let available_end = self.frames_seen * u64::from(FRAME_MS)
            + self.extractor.samples.len() as u64 * 1_000 / u64::from(RATE);
        if anchor_relative_ms > available_end {
            return Err(AlignmentError::InsufficientMaterial);
        }
        let end = anchor_relative_ms / u64::from(FRAME_MS);
        let history_start = self.frames_seen - self.frames.len() as u64;
        if end < history_start || end > self.frames_seen {
            return Err(AlignmentError::InsufficientMaterial);
        }
        let start = end
            .saturating_sub(u64::from(window_ms) / u64::from(FRAME_MS))
            .max(history_start);
        let covered = (end - start) as u32 * u32::from(FRAME_MS);
        if covered < self.config.min_material_ms {
            return Err(AlignmentError::InsufficientMaterial);
        }
        let fingerprint = Fingerprint {
            version: FINGERPRINT_VERSION,
            frame_ms: FRAME_MS,
            anchor_offset_ms: (anchor_relative_ms - start * u64::from(FRAME_MS)) as u32,
            frames: self
                .frames
                .iter()
                .skip((start - history_start) as usize)
                .take((end - start) as usize)
                .copied()
                .collect(),
        };
        fingerprint.validate()?;
        Ok(fingerprint)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignmentMatch {
    pub position_ms: u64,
    /// Similarity, not a calibrated probability of correctness.
    pub confidence: f32,
    pub runner_up_score: f32,
}

#[derive(Clone, Copy)]
struct Candidate {
    start_ms: u64,
    score: f32,
    tail: f32,
    tail_min: f32,
}

/// Streams a complete candidate copy from its beginning. Memory is bounded by
/// the reference window and 64 separated candidates. The host must bind results
/// to the digest of the immutable bytes decoded, and persist algorithm version.
pub struct AlignmentSearcher {
    extractor: Extractor,
    reference: Fingerprint,
    config: AlignmentConfig,
    window: VecDeque<FeatureFrame>,
    frames_seen: u64,
    candidates: Vec<Candidate>,
}

impl AlignmentSearcher {
    pub fn new(
        reference: &Fingerprint,
        sample_rate: u32,
        channels: usize,
        config: AlignmentConfig,
    ) -> Result<Self, AlignmentError> {
        reference.validate()?;
        let config = config.validate()?;
        Ok(Self {
            extractor: Extractor::new(sample_rate, channels, config.silence_rms)?,
            reference: reference.clone(),
            config,
            window: VecDeque::new(),
            frames_seen: 0,
            candidates: Vec::new(),
        })
    }

    pub fn push(&mut self, pcm: &[f32]) -> Result<(), AlignmentError> {
        self.extractor.validate_pcm(pcm)?;
        for samples in pcm.chunks_exact(self.extractor.channels) {
            if let Some(frame) = self.extractor.push_frame(samples) {
                self.push_feature(frame);
            }
        }
        Ok(())
    }

    fn push_feature(&mut self, frame: FeatureFrame) {
        self.frames_seen += 1;
        self.window.push_back(frame);
        let length = self.reference.frames.len();
        if self.window.len() > length {
            self.window.pop_front();
        }
        if self.window.len() < length {
            return;
        }
        let tail_count = (self.config.tail_ms / u32::from(FRAME_MS)) as usize;
        let mut total = 0.0;
        let mut tail = 0.0;
        let mut tail_min = 1.0_f32;
        for (i, (a, b)) in self.reference.frames.iter().zip(&self.window).enumerate() {
            let score = similarity(a, b);
            total += score;
            if i >= length - tail_count {
                tail += score;
                tail_min = tail_min.min(score);
            }
        }
        let candidate = Candidate {
            start_ms: (self.frames_seen - length as u64) * u64::from(FRAME_MS),
            score: total / length as f32,
            tail: tail / tail_count as f32,
            tail_min,
        };
        // Keep a representative for each nearby peak. Separate identical
        // passages remain independent candidates and therefore refuse.
        if let Some(existing) = self.candidates.iter_mut().find(|c| {
            c.start_ms.abs_diff(candidate.start_ms) <= u64::from(self.config.ambiguity_distance_ms)
        }) {
            if candidate.score > existing.score {
                *existing = candidate;
            }
        } else {
            self.candidates.push(candidate);
        }
        self.candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
        self.candidates.truncate(64);
    }

    pub fn finish(self) -> Result<AlignmentMatch, AlignmentError> {
        let Some(best) = self.candidates.first() else {
            return Err(AlignmentError::InsufficientMaterial);
        };
        if best.score < self.config.min_score {
            return Err(AlignmentError::NoMatch);
        }
        let runner_up_score = self
            .candidates
            .iter()
            .skip(1)
            .filter(|c| {
                c.start_ms.abs_diff(best.start_ms) > u64::from(self.config.ambiguity_distance_ms)
            })
            .map(|c| c.score)
            .fold(0.0_f32, f32::max);
        if best.score - runner_up_score < self.config.min_separation {
            return Err(AlignmentError::Ambiguous);
        }
        if best.tail < self.config.min_tail_score
            || best.tail_min < self.config.min_tail_frame_score
        {
            return Err(AlignmentError::AnchorDiscontinuity);
        }
        let decoded_end_ms = self.frames_seen * u64::from(FRAME_MS)
            + self.extractor.samples.len() as u64 * 1_000 / u64::from(RATE);
        if best.start_ms + u64::from(self.reference.anchor_offset_ms) > decoded_end_ms {
            return Err(AlignmentError::AnchorDiscontinuity);
        }
        Ok(AlignmentMatch {
            position_ms: best.start_ms + u64::from(self.reference.anchor_offset_ms),
            confidence: best.score,
            runner_up_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A reproducible sequence of changing tone mixtures, not donor audio and
    // not a substitute for spoken-word or codec acceptance fixtures.
    fn track(seconds: usize, sample_rate: usize, seed: u32) -> Vec<f32> {
        (0..seconds * sample_rate)
            .map(|sample| {
                let step = sample / (sample_rate / 5);
                let hash = (step as u32).wrapping_add(seed).wrapping_mul(747_796_405);
                let band = ((hash ^ (hash >> 16)) % BANDS as u32) as usize;
                let frequency =
                    110.0 * (3_500.0_f32 / 110.0).powf(band as f32 / (BANDS - 1) as f32);
                (std::f32::consts::TAU * frequency * sample as f32 / sample_rate as f32).sin() * 0.4
            })
            .collect()
    }

    fn reference(pcm: &[f32]) -> Fingerprint {
        let mut builder = FingerprintBuilder::new(
            8_000,
            1,
            FingerprintConfig {
                window_ms: 5_000,
                ..Default::default()
            },
        )
        .unwrap();
        // Deliberately split away from feature boundaries.
        for chunk in pcm.chunks(317) {
            builder.push(chunk).unwrap();
        }
        builder.fingerprint().unwrap()
    }

    fn search(
        reference: &Fingerprint,
        pcm: &[f32],
        rate: u32,
    ) -> Result<AlignmentMatch, AlignmentError> {
        let mut searcher =
            AlignmentSearcher::new(reference, rate, 1, AlignmentConfig::default()).unwrap();
        for chunk in pcm.chunks(977) {
            searcher.push(chunk).unwrap();
        }
        searcher.finish()
    }

    #[test]
    fn finds_thirty_second_insert_and_longer_preroll() {
        let base = track(16, 8_000, 21);
        let reference = reference(&base);
        let mut inserted = base[..8 * 8_000].to_vec();
        inserted.extend(track(30, 8_000, 999));
        inserted.extend_from_slice(&base[8 * 8_000..]);
        let found = search(&reference, &inserted, 8_000).unwrap();
        assert_eq!(found.position_ms, 46_000);
        assert!(found.confidence > 0.99);

        let mut preroll = track(7, 8_000, 405);
        preroll.extend_from_slice(&base);
        assert_eq!(
            search(&reference, &preroll, 8_000).unwrap().position_ms,
            23_000
        );
    }

    #[test]
    fn refuses_unrelated_repeated_deleted_and_discontinuous_audio() {
        let base = track(16, 8_000, 21);
        let reference = reference(&base);
        assert!(search(&reference, &track(16, 8_000, 987), 8_000).is_err());
        let mut repeated = base.clone();
        repeated.extend_from_slice(&base);
        assert_eq!(
            search(&reference, &repeated, 8_000),
            Err(AlignmentError::Ambiguous)
        );
        assert!(search(&reference, &base[..15 * 8_000], 8_000).is_err());
        let mut interrupted = base[..15 * 8_000 + 6_400].to_vec();
        interrupted.extend(track(1, 8_000, 987));
        interrupted.extend_from_slice(&base[15 * 8_000 + 6_400..]);
        assert!(search(&reference, &interrupted, 8_000).is_err());
    }

    #[test]
    fn tolerates_gain_noise_and_different_pcm_sample_rate() {
        let base = track(16, 8_000, 21);
        let reference = reference(&base);
        let noisy: Vec<_> = base
            .iter()
            .enumerate()
            .map(|(i, sample)| sample * 0.3 + ((i * 17 % 101) as f32 / 101.0 - 0.5) * 0.0001)
            .collect();
        assert_eq!(
            search(&reference, &noisy, 8_000).unwrap().position_ms,
            16_000
        );
        assert_eq!(
            search(&reference, &track(16, 16_000, 21), 16_000)
                .unwrap()
                .position_ms,
            16_000
        );
    }

    #[test]
    fn strong_earlier_match_cannot_override_a_different_tail() {
        let base = track(16, 8_000, 21);
        let reference = reference(&base);
        let mut changed = base;
        changed[16 * 8_000 - 800..].fill(0.0);
        assert_eq!(
            search(&reference, &changed, 8_000),
            Err(AlignmentError::AnchorDiscontinuity)
        );
    }

    #[test]
    fn silence_stationary_and_short_material_refuse() {
        let mut builder = FingerprintBuilder::new(8_000, 1, FingerprintConfig::default()).unwrap();
        builder.push(&[0.0; 800]).unwrap();
        assert_eq!(
            builder.fingerprint(),
            Err(AlignmentError::InsufficientMaterial)
        );
        builder.push(&vec![0.0; 40_000]).unwrap();
        assert_eq!(builder.fingerprint(), Err(AlignmentError::LowInformation));
        let constant_tone: Vec<_> = (0..40_000)
            .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / 8_000.0).sin())
            .collect();
        let mut builder = FingerprintBuilder::new(8_000, 1, FingerprintConfig::default()).unwrap();
        builder.push(&constant_tone).unwrap();
        assert_eq!(builder.fingerprint(), Err(AlignmentError::LowInformation));
    }

    #[test]
    fn presented_anchor_excludes_lookahead_and_tracks_eviction() {
        let base = track(16, 8_000, 21);
        let mut builder = FingerprintBuilder::new(
            8_000,
            1,
            FingerprintConfig {
                window_ms: 5_000,
                history_ms: 10_000,
                ..Default::default()
            },
        )
        .unwrap();
        builder.push(&base[..10 * 8_000]).unwrap();
        let presented = builder.fingerprint_at(8_037, 5_000).unwrap();
        let expected = reference(&base[..8 * 8_000]);
        assert_eq!(presented.frames, expected.frames);
        assert_eq!(presented.anchor_offset_ms, 5_037);
        assert_eq!(
            builder.fingerprint_at(11_000, 5_000),
            Err(AlignmentError::InsufficientMaterial)
        );
        builder.push(&base[10 * 8_000..]).unwrap();
        assert_eq!(
            builder.fingerprint_at(5_000, 5_000),
            Err(AlignmentError::InsufficientMaterial)
        );
        assert_eq!(
            builder.fingerprint_at(10_000, 5_000).unwrap().covered_ms(),
            4_000
        );
        assert!(builder.frames.len() <= 100);
    }

    #[test]
    fn validates_bounds_versions_and_pcm_without_partial_mutation() {
        assert!(
            FingerprintBuilder::new(
                8_000,
                1,
                FingerprintConfig {
                    history_ms: 90_100,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let mut builder = FingerprintBuilder::new(8_000, 2, FingerprintConfig::default()).unwrap();
        assert_eq!(builder.push(&[0.0]), Err(AlignmentError::InvalidPcm));
        assert_eq!(
            builder.push(&[0.0, f32::NAN]),
            Err(AlignmentError::InvalidPcm)
        );
        assert_eq!(builder.frames_seen, 0);
        let mut fingerprint = reference(&track(5, 8_000, 21));
        fingerprint.version += 1;
        assert_eq!(
            fingerprint.validate(),
            Err(AlignmentError::UnsupportedVersion)
        );
        fingerprint.version = FINGERPRINT_VERSION;
        fingerprint.anchor_offset_ms += 100;
        assert_eq!(
            fingerprint.validate(),
            Err(AlignmentError::InvalidFingerprint)
        );
        assert!(
            AlignmentSearcher::new(
                &reference(&track(5, 8_000, 21)),
                8_000,
                1,
                AlignmentConfig {
                    min_score: f32::NAN,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
