//! Onset detection and tempo estimation (pure DSP).
//!
//! [`OnsetDetector`] consumes a stream of audio samples and emits the
//! sample-time of each transient onset (pick attack, drum hit, etc.).
//! [`estimate_bpm`] turns a handful of onset timestamps into a tempo.
//!
//! Both are framework-agnostic: feed plain `&[f32]`, get plain
//! timestamps. The thread-safe, engine-wired layer (an `Analyzer` that
//! publishes onsets behind a `Mutex` handle, wall-clock-stamped) lives
//! in the consuming crate — see `woodshed-audio`'s `OnsetAnalyzer`.
//!
//! # Algorithm
//!
//! Time-domain energy envelope with an adaptive threshold:
//!
//! 1. Slice the input into non-overlapping frames of `frame_size`
//!    samples (default ≈ 5ms at 48 kHz). Most pick / drum attacks
//!    are sharper than this; the frame is small enough to localize
//!    them within ±5ms.
//! 2. Compute each frame's energy (mean square).
//! 3. Maintain a rolling **median** of the last
//!    `median_window_frames` frame energies (default ≈ 200ms). The
//!    median is the noise floor / local background level. Median
//!    (not mean) is robust to the impulse you're trying to detect.
//! 4. Declare an **onset** when:
//!    - current frame energy > `noise_floor × threshold_multiplier`,
//!    - current frame is greater than the previous frame (rising
//!      edge — rules out the *peak* of a sustained sound),
//!    - at least `min_spacing_samples` have passed since the last
//!      onset (debounce against single attacks splattering into
//!      multiple onsets).
//!
//! ## Why not spectral flux?
//!
//! Spectral flux is the standard in MIR papers and handles pitched
//! attacks well. But for our use cases (single-line picking, drum
//! hits, finger taps), energy envelope is simpler, allocation-free,
//! and works well — and it doesn't drag in an FFT dependency.
//!
//! If a future use case demands pitched-attack robustness (legato runs
//! where energy alone won't trigger), we can swap in a spectral-flux
//! detector behind the same `OnsetDetector` API.

use std::collections::VecDeque;

/// Default detection frame in samples. 256 samples ≈ 5.3 ms at
/// 48 kHz. Small enough to localize onsets; large enough to smooth
/// the per-sample noise.
pub const DEFAULT_FRAME_SIZE: usize = 256;

/// Default rolling-median window in frames. 40 frames ≈ 213 ms at
/// 48 kHz / 256-sample frames. Long enough to span a beat at fast
/// tempos without absorbing the onset itself.
pub const DEFAULT_MEDIAN_WINDOW: usize = 40;

/// Default threshold multiplier above the noise floor. 3.0 = onset
/// must be 3× the median frame energy. Tuned for typical mic + guitar
/// input; user-adjustable.
pub const DEFAULT_THRESHOLD_MULTIPLIER: f32 = 3.0;

/// Default debounce between onsets, in milliseconds. 50 ms = 1200 BPM
/// ceiling on detected hits; well above any musical tempo, low enough
/// not to merge consecutive sixteenth-notes at 240 BPM (62.5 ms apart).
pub const DEFAULT_MIN_SPACING_MS: f32 = 50.0;

/// Streaming onset detector. Hand it audio samples; it tells you when
/// transient onsets occurred. Sample-rate agnostic at construction
/// time so the same instance can be reused if the engine reconfigures.
#[derive(Clone, Debug)]
pub struct OnsetDetector {
    sample_rate_hz: f32,
    frame_size: usize,
    threshold_multiplier: f32,
    min_spacing_samples: u64,
    /// Rolling history of the last N frame energies, used to compute
    /// the noise-floor median. Sized at `median_window_frames`.
    energy_history: VecDeque<f32>,
    median_window_frames: usize,
    /// Carry-over samples between `feed` calls that didn't fill a full
    /// frame. Kept small (< frame_size).
    pending: Vec<f32>,
    /// Energy of the most recently completed frame, used for the
    /// rising-edge check on the next frame.
    last_frame_energy: f32,
    /// Total samples ever fed. Used to assign global timestamps to
    /// onsets so consumers can correlate against an external clock.
    samples_consumed: u64,
    /// Timestamp of the last-emitted onset, in samples. `None` = no
    /// onsets yet.
    last_onset_sample: Option<u64>,
}

impl OnsetDetector {
    /// Build a detector with sensible defaults for the given sample
    /// rate.
    pub fn new(sample_rate_hz: f32) -> Self {
        let min_spacing_samples = (sample_rate_hz * DEFAULT_MIN_SPACING_MS / 1000.0) as u64;
        Self {
            sample_rate_hz,
            frame_size: DEFAULT_FRAME_SIZE,
            threshold_multiplier: DEFAULT_THRESHOLD_MULTIPLIER,
            min_spacing_samples,
            energy_history: VecDeque::with_capacity(DEFAULT_MEDIAN_WINDOW),
            median_window_frames: DEFAULT_MEDIAN_WINDOW,
            pending: Vec::with_capacity(DEFAULT_FRAME_SIZE),
            last_frame_energy: 0.0,
            samples_consumed: 0,
            last_onset_sample: None,
        }
    }

    /// Override the detection threshold. Higher = more conservative
    /// (fewer false positives, more missed quiet hits).
    pub fn with_threshold(mut self, multiplier: f32) -> Self {
        self.threshold_multiplier = multiplier.max(1.0);
        self
    }

    /// Override the debounce interval in milliseconds.
    pub fn with_min_spacing_ms(mut self, ms: f32) -> Self {
        let safe_ms = ms.max(1.0);
        self.min_spacing_samples = (self.sample_rate_hz * safe_ms / 1000.0) as u64;
        self
    }

    /// Override the frame size in samples. Smaller = finer onset
    /// localization, more CPU per-second of audio.
    pub fn with_frame_size(mut self, frame_size: usize) -> Self {
        self.frame_size = frame_size.max(1);
        self.pending.reserve(self.frame_size);
        self
    }

    /// Set the detection threshold multiplier in place (clamped to
    /// `>= 1.0`). The `&mut self` counterpart to [`Self::with_threshold`]
    /// for handles that tune the detector live.
    pub fn set_threshold_multiplier(&mut self, multiplier: f32) {
        self.threshold_multiplier = multiplier.max(1.0);
    }

    /// Current detection threshold multiplier.
    pub fn threshold_multiplier(&self) -> f32 {
        self.threshold_multiplier
    }

    /// Reset all per-stream state. Call when starting fresh capture
    /// (e.g. user pressed Stop and then Play). Configuration is
    /// preserved.
    pub fn reset(&mut self) {
        self.energy_history.clear();
        self.pending.clear();
        self.last_frame_energy = 0.0;
        self.samples_consumed = 0;
        self.last_onset_sample = None;
    }

    /// Sample-rate the detector was configured for.
    pub fn sample_rate_hz(&self) -> f32 {
        self.sample_rate_hz
    }

    /// Total samples consumed across the lifetime of this detector.
    /// Useful as a clock reference for downstream consumers.
    pub fn samples_consumed(&self) -> u64 {
        self.samples_consumed
    }

    /// Feed a batch of samples. Returns the sample-indexed timestamps
    /// of any onsets detected within the batch. The timestamps are
    /// in the detector's own clock — see [`Self::samples_consumed`].
    pub fn feed(&mut self, samples: &[f32]) -> Vec<u64> {
        let mut onsets = Vec::new();

        // Pull a full frame whenever pending + new gives us one.
        let mut cursor = 0;
        loop {
            let needed = self.frame_size.saturating_sub(self.pending.len());
            if cursor + needed > samples.len() {
                // Not enough samples this call to complete a frame —
                // stash what we have and return.
                self.pending.extend_from_slice(&samples[cursor..]);
                self.samples_consumed += (samples.len() - cursor) as u64;
                return onsets;
            }
            // Form a frame from pending + a slice of new samples.
            let new_slice = &samples[cursor..cursor + needed];
            let energy = if self.pending.is_empty() {
                frame_energy(new_slice)
            } else {
                let combined_energy = frame_energy_two(&self.pending, new_slice);
                self.pending.clear();
                combined_energy
            };

            cursor += needed;

            // Count only samples supplied in this call. Any pending
            // prefix was counted when it was first fed. Advance before
            // deciding on an onset so timestamps land at the end of
            // the completed frame.
            self.samples_consumed += needed as u64;

            if let Some(onset_sample) = self.evaluate_frame(energy) {
                // Record the onset's timestamp so the debounce window
                // suppresses follow-up frames during the attack tail.
                self.last_onset_sample = Some(onset_sample);
                onsets.push(onset_sample);
            }

            self.update_history(energy);
            self.last_frame_energy = energy;
        }
    }

    /// Evaluate whether the just-computed frame energy constitutes an
    /// onset. Returns the sample-time of the onset if so.
    fn evaluate_frame(&self, energy: f32) -> Option<u64> {
        if self.energy_history.len() < self.median_window_frames / 4 {
            // Not enough warmup data to trust the noise floor.
            return None;
        }
        let noise_floor = median(&self.energy_history);
        let threshold = noise_floor * self.threshold_multiplier;
        if energy <= threshold {
            return None;
        }
        if energy <= self.last_frame_energy {
            // Not a rising edge — we're past the attack peak.
            return None;
        }
        // Onset timestamp = end of the current frame (we already
        // incremented samples_consumed for this frame).
        let onset_sample = self.samples_consumed;
        if let Some(last) = self.last_onset_sample {
            if onset_sample - last < self.min_spacing_samples {
                return None;
            }
        }
        Some(onset_sample)
    }

    fn update_history(&mut self, energy: f32) {
        if self.energy_history.len() >= self.median_window_frames {
            self.energy_history.pop_front();
        }
        self.energy_history.push_back(energy);
    }

    /// Manually record an onset (e.g. from `evaluate_frame`'s return
    /// value) so the debounce tracker knows about it. `feed` does this
    /// internally; exposed mostly for testing.
    #[doc(hidden)]
    pub fn record_onset(&mut self, sample: u64) {
        self.last_onset_sample = Some(sample);
    }
}

/// Estimate BPM from a list of sample-indexed onset timestamps. Uses
/// the **median** inter-onset interval for robustness against missed
/// or extra hits.
///
/// Returns `None` if fewer than two onsets are provided or the median
/// interval is degenerate (zero / near-zero).
///
/// # Range
///
/// The result is clamped to `[40, 240]` BPM — the musically useful
/// window. Out-of-range estimates almost always mean the detector saw
/// double-hits or missed every other beat; clamping prevents the UI
/// from displaying garbage.
pub fn estimate_bpm(onsets: &[u64], sample_rate_hz: f32) -> Option<f32> {
    if onsets.len() < 2 {
        return None;
    }
    let mut intervals_secs: Vec<f32> = onsets
        .windows(2)
        .map(|w| (w[1].saturating_sub(w[0])) as f32 / sample_rate_hz)
        .collect();
    intervals_secs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_secs = intervals_secs[intervals_secs.len() / 2];
    if median_secs < 0.001 {
        return None;
    }
    Some((60.0 / median_secs).clamp(40.0, 240.0))
}

/// Mean-square energy of a single frame.
fn frame_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    sum_sq / samples.len() as f32
}

/// Mean-square energy spanning two slices (pending + new). Saves
/// allocating a combined buffer.
fn frame_energy_two(a: &[f32], b: &[f32]) -> f32 {
    let total = a.len() + b.len();
    if total == 0 {
        return 0.0;
    }
    let sum_sq: f32 = a.iter().map(|&s| s * s).sum::<f32>() + b.iter().map(|&s| s * s).sum::<f32>();
    sum_sq / total as f32
}

/// Median of a `VecDeque<f32>`. Allocates a small temp vec; called
/// once per frame which is fine for the workload.
fn median(values: &VecDeque<f32>) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f32> = values.iter().copied().collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) * 0.5
    }
}

#[cfg(test)]
mod tests;
