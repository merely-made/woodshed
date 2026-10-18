//! Onset-detection **analyzer** — wires the pure detector into the
//! shared [`crate::InputEngine`].
//!
//! The pure DSP — [`OnsetDetector`], [`estimate_bpm`], and the
//! energy/median helpers — now lives in [`audio_primitives::onset`] and
//! is shared with Strophe. This module is the thread-safe, engine-wired
//! layer on top: an [`Analyzer`](crate::input::Analyzer) that publishes
//! wall-clock-stamped onsets behind an `Arc<Mutex<…>>` readable from any
//! thread via [`OnsetHandle`].
//!
//! The pure cores are re-exported here so existing
//! `woodshed_audio::{OnsetDetector, estimate_bpm}` paths keep working.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::input::Analyzer;

// Re-export the pure DSP layer so downstream `crate::onset::*` /
// `woodshed_audio::*` paths resolve unchanged after the extraction.
pub use audio_primitives::onset::{
    estimate_bpm, OnsetDetector, DEFAULT_FRAME_SIZE, DEFAULT_MEDIAN_WINDOW, DEFAULT_MIN_SPACING_MS,
    DEFAULT_THRESHOLD_MULTIPLIER,
};

// =================================================================
// Analyzer layer — wires the pure detector into the shared InputEngine
// =================================================================
//
// [`OnsetAnalyzer`] implements [`crate::input::Analyzer`] so it can be
// registered alongside the pitch analyzer on a single
// [`crate::InputEngine`]. State is published behind an `Arc<Mutex<...>>`
// readable from any thread via [`OnsetHandle`].

/// A single detected onset, timestamped both in sample-clock and
/// wall-clock. Wall-clock makes it easy to correlate against the
/// sequencer's `Instant`-based play time.
#[derive(Copy, Clone, Debug)]
pub struct DetectedOnset {
    /// Detector-clock sample index (since the engine's input stream
    /// started). Useful for inter-onset interval analysis.
    pub sample_index: u64,
    /// Instant the onset frame completed processing on the audio
    /// thread. Useful for syncing against a wall-clock metronome.
    pub at: Instant,
}

/// Snapshot of recent onsets, snapshotted by the UI thread.
#[derive(Clone, Debug, Default)]
pub struct OnsetSnapshot {
    /// Most recent onsets, oldest first. Bounded — see
    /// [`ONSET_SNAPSHOT_CAPACITY`].
    pub recent: Vec<DetectedOnset>,
    /// Rolling input level (mean-square of the last analysis frame).
    /// Useful for a "listening" indicator in the UI.
    pub input_level: f32,
}

/// How many recent onsets to retain in the snapshot history. 32 is
/// plenty for tap-tempo estimation (you want at most ~8 hits) and for
/// a "last few hits" UI display.
pub const ONSET_SNAPSHOT_CAPACITY: usize = 32;

struct OnsetInternals {
    detector: OnsetDetector,
    snapshot: OnsetSnapshot,
    /// Most recent onsets, used both for the snapshot and for BPM
    /// estimation. Trimmed to [`ONSET_SNAPSHOT_CAPACITY`] from the front.
    history: VecDeque<DetectedOnset>,
    /// If false, [`OnsetAnalyzer::process`] is a no-op. Lets the UI
    /// turn onset detection off until a timing-feedback or loop-record
    /// session needs it.
    enabled: bool,
}

/// Thread-safe handle into an [`OnsetAnalyzer`]'s shared state.
#[derive(Clone)]
pub struct OnsetHandle {
    inner: Arc<Mutex<OnsetInternals>>,
}

impl OnsetHandle {
    /// Read a snapshot of recent activity. Cheap clone.
    pub fn snapshot(&self) -> OnsetSnapshot {
        self.inner.lock().unwrap().snapshot.clone()
    }

    /// Drop all stored history. Use when starting a fresh listening
    /// session (e.g. tap-tempo session begins, calibration starts).
    pub fn reset(&self) {
        let mut s = self.inner.lock().unwrap();
        s.detector.reset();
        s.history.clear();
        s.snapshot = OnsetSnapshot::default();
    }

    /// Estimate BPM from the currently stored history, if there are
    /// enough onsets. See [`estimate_bpm`].
    pub fn current_bpm(&self) -> Option<f32> {
        let s = self.inner.lock().unwrap();
        let samples: Vec<u64> = s.history.iter().map(|o| o.sample_index).collect();
        estimate_bpm(&samples, s.detector.sample_rate_hz())
    }

    /// Overwrite the detector's threshold multiplier on the fly.
    pub fn set_threshold(&self, multiplier: f32) {
        let mut s = self.inner.lock().unwrap();
        s.detector.set_threshold_multiplier(multiplier);
    }

    pub fn threshold(&self) -> f32 {
        self.inner.lock().unwrap().detector.threshold_multiplier()
    }

    /// Enable or disable onset detection. When disabled the analyzer
    /// is a near-no-op (only does enabled-flag check, no DSP).
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.lock().unwrap().enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.lock().unwrap().enabled
    }
}

/// Onset-detection analyzer. Plug into [`crate::InputEngine`] via
/// [`crate::InputEngineBuilder::with_analyzer`].
///
/// Disabled by default — the UI flips the enable flag through
/// [`OnsetHandle::set_enabled`] when timing feedback or loop record
/// goes live. This keeps the audio thread cheap when the feature is
/// not in use.
pub struct OnsetAnalyzer {
    state: Arc<Mutex<OnsetInternals>>,
}

impl Default for OnsetAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl OnsetAnalyzer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OnsetInternals {
                // Sample rate is provisional; OnsetDetector reconfigures
                // through its `sample_rate_hz` accessor when the first
                // `process()` call delivers the real rate.
                detector: OnsetDetector::new(48_000.0),
                snapshot: OnsetSnapshot::default(),
                history: VecDeque::with_capacity(ONSET_SNAPSHOT_CAPACITY),
                enabled: false,
            })),
        }
    }

    /// Clone-able handle into this analyzer's shared state.
    pub fn handle(&self) -> OnsetHandle {
        OnsetHandle {
            inner: Arc::clone(&self.state),
        }
    }
}

impl Analyzer for OnsetAnalyzer {
    fn process(&mut self, samples: &[f32], sample_rate_hz: f32, at: Instant) {
        let mut s = self.state.lock().unwrap();
        if !s.enabled {
            return;
        }

        // Rebuild the detector if the sample rate has shifted (e.g.
        // device hot-swap). Preserves no state — calibration / tempo
        // sessions need to restart on rate change anyway.
        if (s.detector.sample_rate_hz() - sample_rate_hz).abs() > 0.5 {
            s.detector = OnsetDetector::new(sample_rate_hz);
        }

        // RMS for level display.
        let level = if samples.is_empty() {
            0.0
        } else {
            let sum_sq: f32 = samples.iter().map(|x| x * x).sum();
            (sum_sq / samples.len() as f32).sqrt()
        };

        let new_onsets = s.detector.feed(samples);
        for sample_index in new_onsets {
            let detected = DetectedOnset { sample_index, at };
            if s.history.len() >= ONSET_SNAPSHOT_CAPACITY {
                s.history.pop_front();
            }
            s.history.push_back(detected);
        }
        s.snapshot.input_level = level;
        s.snapshot.recent = s.history.iter().copied().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzer_disabled_by_default_emits_nothing() {
        let mut analyzer = OnsetAnalyzer::new();
        let handle = analyzer.handle();
        assert!(!handle.is_enabled());
        // A loud impulse fed while disabled produces no history.
        analyzer.process(&[0.9_f32; 4096], 48_000.0, Instant::now());
        assert!(handle.snapshot().recent.is_empty());
    }

    #[test]
    fn threshold_round_trips_through_handle() {
        let analyzer = OnsetAnalyzer::new();
        let handle = analyzer.handle();
        handle.set_threshold(4.5);
        assert!((handle.threshold() - 4.5).abs() < 1e-6);
        // Clamped to >= 1.0.
        handle.set_threshold(0.2);
        assert!((handle.threshold() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn analyzer_preserves_sample_indices_across_irregular_chunks() {
        let warmup = vec![0.0_f32; 14_400];
        let mut signal = vec![0.0_f32; 48_073];
        for hit in [12_000, 24_000, 36_000] {
            for (offset, sample) in signal[hit..hit + 256].iter_mut().enumerate() {
                *sample = 0.8 * (-0.02 * offset as f32).exp();
            }
        }

        let mut whole_analyzer = OnsetAnalyzer::new();
        let whole_handle = whole_analyzer.handle();
        whole_handle.set_enabled(true);
        whole_analyzer.process(&warmup, 48_000.0, Instant::now());
        whole_analyzer.process(&signal, 48_000.0, Instant::now());
        let whole_indices: Vec<u64> = whole_handle
            .snapshot()
            .recent
            .into_iter()
            .map(|onset| onset.sample_index)
            .collect();
        assert!(!whole_indices.is_empty(), "fixture must produce onsets");

        let mut chunked_analyzer = OnsetAnalyzer::new();
        let chunked_handle = chunked_analyzer.handle();
        chunked_handle.set_enabled(true);
        chunked_analyzer.process(&warmup, 48_000.0, Instant::now());
        let chunk_sizes = [1, 37, 256, 19, 513, 7, 1024, 63];
        let mut cursor = 0;
        let mut chunk_index = 0;
        while cursor < signal.len() {
            // `at` intentionally varies between callbacks. This checks
            // sample-clock parity only, not wall-clock calibration.
            chunked_analyzer.process(&[], 48_000.0, Instant::now());
            let end = (cursor + chunk_sizes[chunk_index % chunk_sizes.len()]).min(signal.len());
            chunked_analyzer.process(&signal[cursor..end], 48_000.0, Instant::now());
            cursor = end;
            chunk_index += 1;
        }
        let chunked_indices: Vec<u64> = chunked_handle
            .snapshot()
            .recent
            .into_iter()
            .map(|onset| onset.sample_index)
            .collect();

        assert_eq!(chunked_indices, whole_indices);
    }
}
