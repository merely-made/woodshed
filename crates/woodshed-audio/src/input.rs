//! Audio input capture and analysis fan-out.
//!
//! [`InputEngine`] owns one cpal input stream. Through
//! [`InputEngineBuilder`], callers register one or more [`Analyzer`]
//! implementations before the stream starts. Each cpal callback then
//! downmixes the captured samples to mono once and hands them to every
//! registered analyzer.
//!
//! Replaces the older "one engine per analysis kind" pattern (separate
//! cpal streams for the tuner and the onset detector), which fought
//! over the input device on some platforms and doubled the audio-thread
//! overhead.
//!
//! # Shipped analyzers
//!
//! - [`PitchAnalyzer`] — drives the tuner. Publishes via [`TunerHandle`].
//! - [`crate::onset::OnsetAnalyzer`] — drives onset detection and
//!   audio-tap-tempo. Publishes via [`crate::onset::OnsetHandle`].
//!
//! Each analyzer carries its own enable/disable flag — when an
//! analyzer is "off" its `process()` is a near-no-op, so it's cheap to
//! leave every analyzer registered for the whole session and toggle
//! visibility from the UI.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use pitch_detection::detector::PitchDetector as McLeodTrait;
use pitch_detection::detector::mcleod::McLeodDetector;
use pitch_detector::core::NoteName as PdNoteName;
use pitch_detector::note::detect_note_in_range;
use pitch_detector::note::hinted::HintedNoteDetector;
use pitch_detector::pitch::{HannedFftDetector, PowerCepstrum};

use std::cell::RefCell;

thread_local! {
    /// McLeod detector lives in thread-local storage because its
    /// internal buffer pool uses `Rc<RefCell<...>>` (not `Send`),
    /// and the `Analyzer` trait requires `Send`. The audio thread
    /// is the only thread that ever calls `Analyzer::process`, so
    /// thread-local is correct and lazy-initialized.
    static MCLEOD: RefCell<McLeodDetector<f64>> = RefCell::new(
        McLeodDetector::new(ANALYSIS_WINDOW, ANALYSIS_WINDOW / 2),
    );
}

use crate::AudioError;
use crate::capture_feed::{self, CaptureFeedConsumer, CaptureFeedControl, CaptureFeedProducer};

// =================================================================
// Analyzer trait — the contract every consumer of input audio meets.
// =================================================================

/// A consumer of input audio samples.
///
/// Implementations receive the latest mono `f32` sample batch at the
/// configured sample rate plus the wall-clock instant the batch was
/// captured. Analyzers run on the audio thread, so they must:
///
/// - return quickly (no I/O, no allocation in steady state),
/// - publish results through shared state (typically `Arc<Mutex<...>>`)
///   that UI / control threads read non-blockingly.
pub trait Analyzer: Send {
    /// Prepare from the actual stream configuration before callbacks begin.
    /// Runs on the control thread, so analyzers may allocate rate-specific state.
    fn prepare(&mut self, _sample_rate: u32) {}

    /// Process one batch of mono input samples. May be empty.
    fn process(&mut self, samples: &[f32], sample_rate_hz: f32, at: Instant);
}

// =================================================================
// Pitch detection — types, helpers, and the PitchAnalyzer.
// =================================================================

/// Analysis window size. 8192 samples at 48 kHz ≈ 170 ms gives a
/// frequency resolution of ~6 Hz per FFT bin, fine enough to separate
/// close low-register pitches (E2 = 82 Hz vs B1 = 62 Hz are 20 Hz apart).
const ANALYSIS_WINDOW: usize = 8192;

/// Hop size between detections. Window slides forward by `HOP_SIZE`
/// samples for each detection, so successive analyses overlap by
/// `ANALYSIS_WINDOW - HOP_SIZE`. 4096 gives ~12 detections/sec at 48 kHz.
const HOP_SIZE: usize = 4096;

/// Default RMS amplitude below which detection is suppressed. Live-
/// adjustable via [`TunerHandle::set_threshold`].
pub const DEFAULT_SILENCE_RMS_THRESHOLD: f64 = 0.001;

/// Frequency search range for pitch detection. Covers low bass B0
/// (≈31 Hz) up to the high register, encompassing every stringed
/// instrument we ship.
const MIN_DETECT_FREQ: f64 = 30.0;
const MAX_DETECT_FREQ: f64 = 2100.0;

/// Number of recent detections kept for majority smoothing.
const HISTORY_LEN: usize = 3;

/// McLeod power gate — minimum signal energy for the detector to
/// even attempt pitch estimation. Stricter than our RMS gate (we
/// pre-gate before calling), but the crate insists on its own.
const MCLEOD_POWER_THRESHOLD: f64 = 0.5;

/// McLeod clarity gate — minimum confidence in the detected pitch
/// before the result is returned. The crate's `Pitch::clarity`
/// field ranges 0..1 with values >0.7 typically indicating a
/// well-defined tone; 0.6 lets through brief transients without
/// flooding the snapshot with high-confidence-only blanks.
const MCLEOD_CLARITY_THRESHOLD: f64 = 0.6;

/// Default tolerance for "in tune" — cents within ±this count
/// register as correct. Matches `pitch-detector`'s internal
/// threshold so the McLeod path agrees with the FFT path.
const IN_TUNE_CENTS_TOLERANCE: f64 = 5.0;

/// Resolve a raw frequency (Hz) to a `pitch_detector::note::NoteDetectionResult`-
/// shaped tuple — the rest of the analyzer expects that shape.
/// Mirrors pitch-detector's note-resolution math: nearest 12-TET
/// note at A=440, cents offset signed in [-50, +50], `in_tune` if
/// |cents| < [`IN_TUNE_CENTS_TOLERANCE`].
fn freq_to_note_detection_result(freq_hz: f64) -> pitch_detector::note::NoteDetectionResult {
    // Semitones above A4 (440 Hz). MIDI note number = round(s) + 69.
    let semis = 12.0 * (freq_hz / 440.0).log2();
    let midi = (semis.round() as i32) + 69;
    // Reference frequency of the nearest 12-TET note.
    let nearest_hz = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
    // Cents offset of the actual freq from the nearest note.
    let cents = 1200.0 * (freq_hz / nearest_hz).log2();
    // MIDI → (name, octave). C4 = MIDI 60, so pitch-class = midi % 12,
    // octave = midi / 12 - 1.
    let pc = midi.rem_euclid(12) as usize;
    let octave = (midi / 12) - 1;
    let note_name = match pc {
        0 => PdNoteName::C,
        1 => PdNoteName::CSharp,
        2 => PdNoteName::D,
        3 => PdNoteName::DSharp,
        4 => PdNoteName::E,
        5 => PdNoteName::F,
        6 => PdNoteName::FSharp,
        7 => PdNoteName::G,
        8 => PdNoteName::GSharp,
        9 => PdNoteName::A,
        10 => PdNoteName::ASharp,
        _ => PdNoteName::B,
    };
    // Adjacent note names — used by pitch-detector's hint logic; we
    // fill them in by stepping pc ±1 mod 12.
    let prev_pc = (pc + 11) % 12;
    let next_pc = (pc + 1) % 12;
    let to_pd = |pc: usize| match pc {
        0 => PdNoteName::C,
        1 => PdNoteName::CSharp,
        2 => PdNoteName::D,
        3 => PdNoteName::DSharp,
        4 => PdNoteName::E,
        5 => PdNoteName::F,
        6 => PdNoteName::FSharp,
        7 => PdNoteName::G,
        8 => PdNoteName::GSharp,
        9 => PdNoteName::A,
        10 => PdNoteName::ASharp,
        _ => PdNoteName::B,
    };
    pitch_detector::note::NoteDetectionResult {
        actual_freq: freq_hz,
        note_name,
        octave: octave as i32,
        cents_offset: cents,
        note_freq: nearest_hz,
        in_tune: cents.abs() < IN_TUNE_CENTS_TOLERANCE,
        previous_note_name: to_pd(prev_pc),
        next_note_name: to_pd(next_pc),
    }
}

/// Consecutive harmonic-suspect detections tolerated before we accept
/// the new pitch as a real change.
const HARMONIC_SUPPRESS_GRACE: usize = 3;

#[derive(Clone, Debug)]
pub struct DetectedNote {
    /// Nearest 12-TET note name (sharps spelling — pitch-detector's
    /// convention).
    pub name: PdNoteName,
    pub octave: i32,
    pub note_freq_hz: f64,
    pub actual_freq_hz: f64,
    /// Distance in cents from the nearest 12-TET note. Range roughly
    /// [-50, +50].
    pub cents_offset: f64,
    pub in_tune: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TunerSnapshot {
    pub note: Option<DetectedNote>,
    /// RMS amplitude of the most recent analysis window, in [0, 1].
    pub input_level: f32,
}

/// Which pitch-detection algorithm to use.
///
/// Three families:
/// - **FFT** (frequency domain) — fast, picks the strongest spectral
///   peak. Prone to octave errors when a harmonic dominates the
///   fundamental (classic problem on guitar low-E).
/// - **Cepstrum** (frequency-domain refinement) — IFFT of the log
///   magnitude. Periodicity collapses to a single peak which is
///   more robust to harmonic-dominant signals than raw FFT, at
///   slightly higher cost.
/// - **McLeod (NSDF)** (time domain) — Tartini's algorithm, a
///   refined autocorrelation using the normalized square difference
///   function. Different domain than the others, so makes different
///   mistakes — particularly less prone to octave errors on
///   sustained tones like guitar strings. Sourced from the
///   `pitch-detection` crate (distinct from `pitch-detector` which
///   only exposes FFT + Cepstrum). Slowest of the three but still
///   real-time on our analysis windows.
///
/// Detector choice belongs in the UI; the audio engine just runs
/// whichever is set.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DetectorKind {
    Fft,
    Cepstrum,
    McLeod,
}

impl DetectorKind {
    pub const ALL: [Self; 3] = [Self::Fft, Self::Cepstrum, Self::McLeod];

    pub fn label(self) -> &'static str {
        match self {
            Self::Fft => "FFT",
            Self::Cepstrum => "Cepstrum",
            Self::McLeod => "McLeod",
        }
    }
}

impl core::fmt::Display for DetectorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.label())
    }
}

/// Shared state behind a [`TunerHandle`].
struct PitchInternals {
    snapshot: TunerSnapshot,
    threshold_rms: f64,
    detector_kind: DetectorKind,
    target_hint: Option<PdNoteName>,
    /// If false, [`PitchAnalyzer::process`] is a no-op. Lets the UI
    /// turn pitch off when not visible (FFT is expensive).
    enabled: bool,
}

/// Cloneable handle. `snapshot()` reads the latest detection result;
/// other methods reconfigure the analyzer live.
#[derive(Clone)]
pub struct TunerHandle {
    inner: Arc<Mutex<PitchInternals>>,
}

impl TunerHandle {
    pub fn snapshot(&self) -> TunerSnapshot {
        self.inner.lock().unwrap().snapshot.clone()
    }

    /// Set the RMS amplitude threshold below which detection is
    /// suppressed.
    pub fn set_threshold(&self, threshold_rms: f64) {
        self.inner.lock().unwrap().threshold_rms = threshold_rms;
    }

    pub fn threshold(&self) -> f64 {
        self.inner.lock().unwrap().threshold_rms
    }

    pub fn set_detector_kind(&self, kind: DetectorKind) {
        self.inner.lock().unwrap().detector_kind = kind;
    }

    pub fn detector_kind(&self) -> DetectorKind {
        self.inner.lock().unwrap().detector_kind
    }

    pub fn set_target_hint(&self, hint: Option<DetectedNoteName>) {
        self.inner.lock().unwrap().target_hint = hint;
    }

    pub fn target_hint(&self) -> Option<DetectedNoteName> {
        self.inner.lock().unwrap().target_hint.clone()
    }

    /// Enable or disable pitch detection. When disabled the FFT
    /// pipeline is skipped entirely — useful when the user is on a
    /// non-tuner tab.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.lock().unwrap().enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.lock().unwrap().enabled
    }
}

/// Pitch (tuner) analyzer.
pub struct PitchAnalyzer {
    state: Arc<Mutex<PitchInternals>>,
    /// Working buffer for accumulating audio thread samples until we
    /// have a full analysis window.
    buffer: Vec<f64>,
    /// All detectors are kept warm so switching kinds is instant.
    fft_detector: HannedFftDetector,
    cepstrum_detector: PowerCepstrum,
    history: VecDeque<DetectedNote>,
    harmonic_suppress_streak: usize,
}

impl Default for PitchAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PitchAnalyzer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PitchInternals {
                snapshot: TunerSnapshot::default(),
                threshold_rms: DEFAULT_SILENCE_RMS_THRESHOLD,
                detector_kind: DetectorKind::Fft,
                target_hint: None,
                // Enabled by default so a tuner-only app "just works"
                // without an explicit handshake. UIs that want to gate
                // by tab visibility should call `handle.set_enabled(false)`
                // after construction.
                enabled: true,
            })),
            buffer: Vec::with_capacity(ANALYSIS_WINDOW * 2),
            fft_detector: HannedFftDetector::default(),
            cepstrum_detector: PowerCepstrum::default(),
            history: VecDeque::with_capacity(HISTORY_LEN),
            harmonic_suppress_streak: 0,
        }
    }

    /// Clone-able handle into this analyzer's shared state.
    pub fn handle(&self) -> TunerHandle {
        TunerHandle {
            inner: Arc::clone(&self.state),
        }
    }
}

impl Analyzer for PitchAnalyzer {
    fn process(&mut self, samples: &[f32], sample_rate_hz: f32, _at: Instant) {
        // Read the live config and the enable flag in one lock.
        let (enabled, threshold, kind, hint) = {
            let s = self.state.lock().unwrap();
            (
                s.enabled,
                s.threshold_rms,
                s.detector_kind,
                s.target_hint.clone(),
            )
        };
        if !enabled {
            // Keep the buffer drained-ish so when we re-enable we
            // don't analyze stale audio.
            if self.buffer.len() > ANALYSIS_WINDOW {
                let to_drop = self.buffer.len() - ANALYSIS_WINDOW;
                self.buffer.drain(..to_drop);
            }
            return;
        }

        for &s in samples {
            self.buffer.push(s as f64);
        }
        if self.buffer.len() < ANALYSIS_WINDOW {
            return;
        }
        let start = self.buffer.len() - ANALYSIS_WINDOW;
        let signal = &self.buffer[start..];
        let rms = compute_rms(signal);
        let sample_rate = sample_rate_hz as f64;

        let raw = if rms >= threshold {
            let range = MIN_DETECT_FREQ..MAX_DETECT_FREQ;
            // pitch-detector v0.3 only implements HintedNoteDetector for
            // HannedFftDetector, so when a target hint is set we route
            // through FFT regardless of selected kind.
            let result = match (kind, hint) {
                (_, Some(name)) => self.fft_detector.detect_note_with_hint_and_range(
                    name,
                    signal,
                    sample_rate,
                    Some(range),
                ),
                (DetectorKind::Fft, None) => {
                    detect_note_in_range(signal, &mut self.fft_detector, sample_rate, range)
                },
                (DetectorKind::Cepstrum, None) => {
                    detect_note_in_range(signal, &mut self.cepstrum_detector, sample_rate, range)
                },
                // McLeod returns a raw frequency + clarity; we
                // resolve to note name / cents locally since
                // pitch-detector's NoteDetectionResult builder
                // isn't on this code path. `freq_to_note` mirrors
                // pitch-detector's internal math: nearest 12-TET
                // note at A=440, cents offset signed (-50..+50),
                // `in_tune` if |cents| < tolerance.
                (DetectorKind::McLeod, None) => MCLEOD.with(|cell| {
                    cell.borrow_mut()
                        .get_pitch(
                            signal,
                            sample_rate as usize,
                            MCLEOD_POWER_THRESHOLD,
                            MCLEOD_CLARITY_THRESHOLD,
                        )
                        .and_then(|p| {
                            let f = p.frequency;
                            if f < MIN_DETECT_FREQ || f > MAX_DETECT_FREQ {
                                None
                            } else {
                                Some(freq_to_note_detection_result(f))
                            }
                        })
                }),
            };
            result.map(|n| DetectedNote {
                name: n.note_name,
                octave: n.octave,
                note_freq_hz: n.note_freq,
                actual_freq_hz: n.actual_freq,
                cents_offset: n.cents_offset,
                in_tune: n.in_tune,
            })
        } else {
            None
        };

        // Octave-error correction: if the new raw detection is a
        // small-integer multiple/fraction of the prior stable freq,
        // suspect harmonic confusion and suppress unless it persists.
        let prior_stable = compute_stable(&self.history);
        let suspect = matches!(
            (&raw, &prior_stable),
            (Some(note), Some(stable))
                if is_harmonic_of(note.actual_freq_hz, stable.actual_freq_hz)
        );

        if suspect {
            self.harmonic_suppress_streak += 1;
            if self.harmonic_suppress_streak >= HARMONIC_SUPPRESS_GRACE {
                self.history.clear();
                if let Some(note) = raw {
                    self.history.push_back(note);
                }
                self.harmonic_suppress_streak = 0;
            }
        } else {
            self.harmonic_suppress_streak = 0;
            match raw {
                Some(note) => {
                    if self.history.len() >= HISTORY_LEN {
                        self.history.pop_front();
                    }
                    self.history.push_back(note);
                },
                None => self.history.clear(),
            }
        }

        let stable = compute_stable(&self.history);

        let mut s = self.state.lock().unwrap();
        s.snapshot.input_level = rms as f32;
        s.snapshot.note = stable;
        drop(s);

        // Drain by HOP_SIZE so successive analyses overlap.
        let to_drop = HOP_SIZE.min(self.buffer.len());
        self.buffer.drain(..to_drop);
    }
}

// Re-export pitch-detector's NoteName so app code can convert it
// without taking pitch-detector as a direct dep.
pub use pitch_detector::core::NoteName as DetectedNoteName;

fn compute_rms(signal: &[f64]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = signal.iter().map(|s| s * s).sum();
    (sum_sq / signal.len() as f64).sqrt()
}

/// True if `raw` looks like a harmonic-confusion artifact of `reference`:
/// approximately a 2×, 3×, or 4× multiple, or a 1/2, 1/3, 1/4 fraction.
fn is_harmonic_of(raw_freq: f64, reference_freq: f64) -> bool {
    if reference_freq < 1.0 || raw_freq < 1.0 {
        return false;
    }
    let ratio = raw_freq / reference_freq;
    let near = |target: f64| (ratio - target).abs() / target < 0.03;
    near(2.0) || near(3.0) || near(4.0) || near(0.5) || near(1.0 / 3.0) || near(0.25)
}

/// Majority-stable detection across recent history, with promotion of
/// a sub-multiple fundamental when the consensus is its harmonic.
fn compute_stable(history: &VecDeque<DetectedNote>) -> Option<DetectedNote> {
    if history.len() < 2 {
        return None;
    }
    let mut best_count = 0;
    let mut best: Option<&DetectedNote> = None;
    for candidate in history {
        let count = history
            .iter()
            .filter(|n| n.name == candidate.name && n.octave == candidate.octave)
            .count();
        if count > best_count {
            best_count = count;
            best = Some(candidate);
        }
    }
    let required = history.len().div_ceil(2);
    if best_count < required {
        return None;
    }
    let consensus = best?;
    let consensus_freq = consensus.actual_freq_hz;

    // Fundamental-bias promotion.
    if consensus_freq > 1.0 {
        for entry in history {
            let f = entry.actual_freq_hz;
            if f < 1.0 || f >= consensus_freq * 0.95 {
                continue;
            }
            if is_harmonic_of(consensus_freq, f) {
                return Some(entry.clone());
            }
        }
    }

    history
        .iter()
        .rev()
        .find(|n| n.name == consensus.name && n.octave == consensus.octave)
        .cloned()
}

// =================================================================
// Looper-capture analyzer — bounded SPSC input feed.
// =================================================================

/// Three seconds at 48 kHz. The queue never grows; rejected samples
/// increment an overflow count that invalidates an armed/recording take.
pub const LOOPER_CAPTURE_RING_LIMIT: usize = 48_000 * 3;

/// Input-side owner of the capture producer. Disabled by default and
/// registered alongside pitch/onset on the one lazy input stream.
pub struct LooperCaptureAnalyzer {
    producer: CaptureFeedProducer,
    handle: LooperCaptureHandle,
}

impl Default for LooperCaptureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LooperCaptureAnalyzer {
    pub fn new() -> Self {
        let (producer, consumer, control) = capture_feed::channel(LOOPER_CAPTURE_RING_LIMIT);
        Self {
            producer,
            handle: LooperCaptureHandle {
                control,
                consumer: Arc::new(Mutex::new(Some(consumer))),
            },
        }
    }

    pub fn handle(&self) -> LooperCaptureHandle {
        self.handle.clone()
    }
}

impl Analyzer for LooperCaptureAnalyzer {
    fn prepare(&mut self, sample_rate: u32) {
        self.producer.prepare(sample_rate);
    }

    fn process(&mut self, samples: &[f32], _sample_rate_hz: f32, _at: Instant) {
        self.producer.push_samples(samples);
    }
}

/// Cloneable control handle. The mutex protects only the one-time transfer
/// of consumer ownership on a control thread; callbacks never acquire it.
#[derive(Clone)]
pub struct LooperCaptureHandle {
    control: CaptureFeedControl,
    consumer: Arc<Mutex<Option<CaptureFeedConsumer>>>,
}

impl LooperCaptureHandle {
    /// Change capture generation on enable/disable edges. Earlier samples
    /// remain bounded in the queue but cannot enter a later take.
    pub fn set_enabled(&self, on: bool) {
        self.control.set_enabled(on);
    }

    pub fn is_enabled(&self) -> bool {
        self.control.is_enabled()
    }

    /// Move the sole reader into the output processor. Calling through any
    /// clone again returns `None`; extraction never happens on a callback.
    pub fn take_consumer(&self) -> Option<CaptureFeedConsumer> {
        self.consumer.lock().unwrap().take()
    }

    /// Reset the capture generation without changing its enabled state.
    pub fn clear(&self) {
        self.control.clear();
    }

    /// Actual configured input rate; zero before stream preparation.
    pub fn source_rate(&self) -> u32 {
        self.control.source_rate()
    }
}

// =================================================================
// InputEngine + Builder
// =================================================================

/// Shared native-input health. A callback failure is permanent for this stream;
/// recovery creates a fresh stream and health handle. Observers never clear it.
#[derive(Clone, Default)]
pub struct InputHealth {
    state: Arc<Mutex<InputHealthState>>,
}

#[derive(Default)]
struct InputHealthState {
    failure: Option<String>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl InputHealth {
    pub fn failure(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .failure
            .clone()
    }

    /// The native error callback owns this report. Explicit input adapters may
    /// report an equivalent stream failure without creating another runtime.
    pub fn report_failure(&self, reason: impl Into<String>) {
        let wake = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.failure.is_some() {
                return;
            }
            state.failure = Some(reason.into());
            state.wake.clone()
        };
        if let Some(wake) = wake {
            wake();
        }
    }

    pub fn set_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        let failed = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.wake = Some(wake.clone());
            state.failure.is_some()
        };
        if failed {
            wake();
        }
    }
}

/// Build phase for an [`InputEngine`]. Register analyzers, then call
/// [`Self::build`] to open the cpal stream.
pub struct InputEngineBuilder {
    analyzers: Vec<Box<dyn Analyzer + Send>>,
    health: InputHealth,
}

impl Default for InputEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl InputEngineBuilder {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
            health: InputHealth::default(),
        }
    }

    pub fn health_handle(&self) -> InputHealth {
        self.health.clone()
    }

    /// Register an arbitrary analyzer. The caller is responsible for
    /// holding a handle into its shared state (typically returned by
    /// the analyzer type's `handle()` method).
    pub fn with_analyzer<A>(mut self, analyzer: A) -> Self
    where
        A: Analyzer + 'static,
    {
        self.analyzers.push(Box::new(analyzer));
        self
    }

    /// Register a [`PitchAnalyzer`] and return its handle. Convenience
    /// for the very common tuner case.
    pub fn with_pitch(mut self) -> (Self, TunerHandle) {
        let analyzer = PitchAnalyzer::new();
        let handle = analyzer.handle();
        self.analyzers.push(Box::new(analyzer));
        (self, handle)
    }

    /// Register a [`LooperCaptureAnalyzer`] for song-mode loop recording.
    pub fn with_looper_capture(mut self) -> (Self, LooperCaptureHandle) {
        let analyzer = LooperCaptureAnalyzer::new();
        let handle = analyzer.handle();
        self.analyzers.push(Box::new(analyzer));
        (self, handle)
    }

    /// Prepare registered analyzers for an explicitly configured source.
    /// Useful for device-free input harnesses. `build` always repeats this
    /// with the actual CPAL configuration; there is no default capture rate.
    pub fn prepare_analyzers(&mut self, source_rate: u32) {
        for analyzer in &mut self.analyzers {
            analyzer.prepare(source_rate);
        }
    }

    /// Open the cpal input stream and start running the analyzers.
    pub fn build(mut self) -> Result<InputEngine, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;
        let supported = device
            .default_input_config()
            .map_err(AudioError::StreamConfig)?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let channels = config.channels as usize;
        let source_rate = config.sample_rate;
        let sample_rate_hz = source_rate as f32;

        self.prepare_analyzers(source_rate);
        let health = self.health;
        let callback_health = health.clone();
        let mut analyzers = self.analyzers;
        // Reusable scratch buffer for the per-callback mono downmix.
        // Grows on first callback then stays sized.
        let mut mono: Vec<f32> = Vec::with_capacity(2048);

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device
                .build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        mono.clear();
                        mono.reserve(data.len() / channels.max(1));
                        for frame in data.chunks(channels) {
                            let mean = frame.iter().copied().sum::<f32>() / channels as f32;
                            mono.push(mean);
                        }
                        let now = Instant::now();
                        for analyzer in analyzers.iter_mut() {
                            analyzer.process(&mono, sample_rate_hz, now);
                        }
                    },
                    move |err| callback_health.report_failure(format!("audio input stream: {err}")),
                    None,
                )
                .map_err(AudioError::StreamBuild)?,
            other => return Err(AudioError::UnsupportedSampleFormat(other)),
        };
        stream.play().map_err(AudioError::StreamPlay)?;

        Ok(InputEngine {
            _stream: stream,
            source_rate,
            health,
        })
    }
}

/// Running input engine. Holding this alive keeps the cpal stream
/// running. Drop it to stop capture.
pub struct InputEngine {
    _stream: Stream,
    source_rate: u32,
    health: InputHealth,
}

impl InputEngine {
    pub fn health_handle(&self) -> InputHealth {
        self.health.clone()
    }

    pub fn sample_rate_hz(&self) -> f32 {
        self.source_rate as f32
    }

    /// Actual configured input rate, without a provisional default or cast.
    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }
}

// =================================================================
// Backward-compat: TunerEngine wraps an InputEngine with only pitch
// =================================================================

/// Convenience wrapper for the common "I just want a tuner" case.
/// Internally constructs an [`InputEngine`] with only a [`PitchAnalyzer`]
/// registered.
pub struct TunerEngine {
    _engine: InputEngine,
    handle: TunerHandle,
}

impl TunerEngine {
    pub fn new() -> Result<Self, AudioError> {
        let (builder, handle) = InputEngineBuilder::new().with_pitch();
        let engine = builder.build()?;
        Ok(Self {
            _engine: engine,
            handle,
        })
    }

    pub fn handle(&self) -> TunerHandle {
        self.handle.clone()
    }
}

// =================================================================
// Tests — DSP helpers and PitchAnalyzer behavior
// =================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_failure_latches_across_observers_and_wakes_without_analyzer_polling() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let builder = InputEngineBuilder::new();
        let health = builder.health_handle();
        let observer = health.clone();
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        health.set_wake(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(observer.failure().is_none());
        // This is the same report path invoked by CPAL's error callback.
        health.report_failure("native input disconnected");
        health.report_failure("later duplicate status");
        assert_eq!(
            observer.failure().as_deref(),
            Some("native input disconnected")
        );
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        assert!(
            InputEngineBuilder::new()
                .health_handle()
                .failure()
                .is_none(),
            "a fresh stream owns a fresh latch; old observers never reset"
        );
    }

    #[test]
    fn capture_consumer_is_transferred_once_and_rate_precedes_callbacks() {
        let mut analyzer = LooperCaptureAnalyzer::new();
        let handle = analyzer.handle();
        let mut consumer = handle.take_consumer().unwrap();
        assert!(handle.clone().take_consumer().is_none());
        assert_eq!(handle.source_rate(), 0);
        analyzer.prepare(44_100);
        assert_eq!(handle.source_rate(), 44_100);
        assert_eq!(consumer.source_rate(), 44_100);
        assert!(consumer.validate_output_rate(48_000).is_err());
        handle.set_enabled(true);
        analyzer.process(&[0.4, -0.2], 44_100.0, Instant::now());
        assert_eq!(consumer.pop_sample(), Some(0.4));
        assert_eq!(consumer.pop_sample(), Some(-0.2));
        handle.clear();
        assert!(handle.is_enabled());
        handle.set_enabled(false);
        assert!(!handle.is_enabled());
    }

    #[test]
    fn builder_prepares_capture_from_explicit_source_configuration() {
        let (mut builder, handle) = InputEngineBuilder::new().with_looper_capture();
        assert_eq!(handle.source_rate(), 0);
        builder.prepare_analyzers(48_000);
        assert_eq!(handle.source_rate(), 48_000);
        assert!(
            handle
                .take_consumer()
                .unwrap()
                .validate_output_rate(48_000)
                .is_ok()
        );
        // Physical build supplies its own configuration, never retaining a
        // harness/provisional rate; repeated preparation updates the handle.
        builder.prepare_analyzers(44_100);
        assert_eq!(handle.source_rate(), 44_100);
    }

    #[test]
    fn rms_of_silence_is_zero() {
        let signal = vec![0.0_f64; 1024];
        assert_eq!(compute_rms(&signal), 0.0);
    }

    #[test]
    fn rms_of_constant_one_is_one() {
        let signal = vec![1.0_f64; 1024];
        assert!((compute_rms(&signal) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rms_of_pure_sine_is_one_over_sqrt_two() {
        let n = 4096;
        let signal: Vec<f64> = (0..n)
            .map(|i| (i as f64 * std::f64::consts::TAU / n as f64).sin())
            .collect();
        let rms = compute_rms(&signal);
        let expected = 1.0 / 2.0_f64.sqrt();
        assert!(
            (rms - expected).abs() < 1e-3,
            "rms = {rms}, expected ≈ {expected}"
        );
    }

    #[test]
    fn silence_threshold_gates_very_quiet_signal() {
        let n = 4096;
        let amplitude = 0.0005;
        let signal: Vec<f64> = (0..n)
            .map(|i| amplitude * (i as f64 * std::f64::consts::TAU / n as f64).sin())
            .collect();
        let rms = compute_rms(&signal);
        assert!(
            rms < DEFAULT_SILENCE_RMS_THRESHOLD,
            "rms {rms} should be below threshold {DEFAULT_SILENCE_RMS_THRESHOLD}"
        );
    }

    fn note(name: PdNoteName, octave: i32) -> DetectedNote {
        DetectedNote {
            name,
            octave,
            note_freq_hz: 0.0,
            actual_freq_hz: 0.0,
            cents_offset: 0.0,
            in_tune: true,
        }
    }

    fn note_with_freq(name: PdNoteName, octave: i32, freq: f64) -> DetectedNote {
        DetectedNote {
            name,
            octave,
            note_freq_hz: freq,
            actual_freq_hz: freq,
            cents_offset: 0.0,
            in_tune: true,
        }
    }

    #[test]
    fn smoothing_returns_none_when_history_too_short() {
        let mut h = VecDeque::new();
        h.push_back(note(PdNoteName::A, 4));
        assert!(compute_stable(&h).is_none());
    }

    #[test]
    fn smoothing_returns_consensus_when_majority_agrees() {
        let mut h = VecDeque::new();
        h.push_back(note(PdNoteName::A, 4));
        h.push_back(note(PdNoteName::A, 4));
        h.push_back(note(PdNoteName::B, 4));
        let stable = compute_stable(&h).expect("majority should win");
        assert_eq!(stable.name, PdNoteName::A);
        assert_eq!(stable.octave, 4);
    }

    #[test]
    fn smoothing_returns_none_when_no_majority() {
        let mut h = VecDeque::new();
        h.push_back(note(PdNoteName::A, 4));
        h.push_back(note(PdNoteName::B, 4));
        h.push_back(note(PdNoteName::C, 5));
        assert!(compute_stable(&h).is_none());
    }

    #[test]
    fn third_harmonic_is_recognized() {
        assert!(is_harmonic_of(247.94, 82.41));
    }

    #[test]
    fn second_harmonic_is_recognized() {
        assert!(is_harmonic_of(440.0, 220.0));
    }

    #[test]
    fn sub_octave_is_recognized() {
        assert!(is_harmonic_of(220.0, 440.0));
    }

    #[test]
    fn unrelated_pitches_are_not_harmonics() {
        assert!(!is_harmonic_of(110.0, 82.0));
    }

    #[test]
    fn same_pitch_is_not_a_harmonic() {
        assert!(!is_harmonic_of(440.0, 440.0));
    }

    #[test]
    fn smoothing_uses_most_recent_matching_detection() {
        let mut h = VecDeque::new();
        let mut a1 = note(PdNoteName::A, 4);
        a1.cents_offset = -10.0;
        let mut a2 = note(PdNoteName::A, 4);
        a2.cents_offset = 5.0;
        h.push_back(a1);
        h.push_back(note(PdNoteName::B, 4));
        h.push_back(a2);
        let stable = compute_stable(&h).expect("A4 has majority");
        assert_eq!(stable.cents_offset, 5.0);
    }

    #[test]
    fn fundamental_bias_promotes_lower_entry_when_consensus_is_3rd_harmonic() {
        let mut h = VecDeque::new();
        h.push_back(note_with_freq(PdNoteName::B, 3, 247.94));
        h.push_back(note_with_freq(PdNoteName::E, 2, 82.41));
        h.push_back(note_with_freq(PdNoteName::B, 3, 247.94));
        let stable = compute_stable(&h).expect("majority B with E sub-multiple");
        assert_eq!(stable.name, PdNoteName::E);
        assert_eq!(stable.octave, 2);
    }

    #[test]
    fn fundamental_bias_does_not_promote_when_no_sub_multiple_present() {
        let mut h = VecDeque::new();
        h.push_back(note_with_freq(PdNoteName::A, 4, 440.0));
        h.push_back(note_with_freq(PdNoteName::B, 4, 493.88));
        h.push_back(note_with_freq(PdNoteName::A, 4, 440.0));
        let stable = compute_stable(&h).expect("A majority");
        assert_eq!(stable.name, PdNoteName::A);
        assert_eq!(stable.octave, 4);
    }

    #[test]
    fn fundamental_bias_does_not_demote_when_consensus_is_already_lowest() {
        let mut h = VecDeque::new();
        h.push_back(note_with_freq(PdNoteName::E, 2, 82.41));
        h.push_back(note_with_freq(PdNoteName::E, 2, 82.41));
        h.push_back(note_with_freq(PdNoteName::B, 3, 247.94));
        let stable = compute_stable(&h).expect("E2 majority");
        assert_eq!(stable.name, PdNoteName::E);
        assert_eq!(stable.octave, 2);
    }

    #[test]
    fn quiet_playing_passes_threshold() {
        let n = 4096;
        let amplitude = 0.05;
        let signal: Vec<f64> = (0..n)
            .map(|i| amplitude * (i as f64 * std::f64::consts::TAU / n as f64).sin())
            .collect();
        let rms = compute_rms(&signal);
        assert!(
            rms >= DEFAULT_SILENCE_RMS_THRESHOLD,
            "rms {rms} should pass threshold {DEFAULT_SILENCE_RMS_THRESHOLD}"
        );
    }

    // === New tests for the analyzer pipeline ===

    #[test]
    fn pitch_analyzer_handle_starts_with_empty_snapshot() {
        let a = PitchAnalyzer::new();
        let h = a.handle();
        let snap = h.snapshot();
        assert!(snap.note.is_none());
        assert_eq!(snap.input_level, 0.0);
    }

    #[test]
    fn pitch_analyzer_set_enabled_round_trips() {
        let a = PitchAnalyzer::new();
        let h = a.handle();
        assert!(h.is_enabled());
        h.set_enabled(false);
        assert!(!h.is_enabled());
        h.set_enabled(true);
        assert!(h.is_enabled());
    }

    #[test]
    fn pitch_analyzer_disabled_does_not_publish_a_detection() {
        // Feed a strong A4 sine to a disabled analyzer; snapshot should
        // stay empty.
        let mut a = PitchAnalyzer::new();
        let h = a.handle();
        h.set_enabled(false);

        let sr = 48_000.0_f32;
        let samples: Vec<f32> = (0..ANALYSIS_WINDOW)
            .map(|i| {
                let t = i as f32 / sr;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect();
        a.process(&samples, sr, Instant::now());
        let snap = h.snapshot();
        assert!(snap.note.is_none());
    }

    #[test]
    fn pitch_analyzer_enabled_detects_a4_from_sine() {
        let mut a = PitchAnalyzer::new();
        let h = a.handle();
        let sr = 48_000.0_f32;
        // Feed enough samples to fill the analysis window + smoothing
        // history. ANALYSIS_WINDOW * 4 gives ~3 detection passes which
        // is enough for compute_stable to publish a result.
        let samples: Vec<f32> = (0..ANALYSIS_WINDOW * 4)
            .map(|i| {
                let t = i as f32 / sr;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect();
        // Feed in small batches like cpal would deliver.
        for chunk in samples.chunks(512) {
            a.process(chunk, sr, Instant::now());
        }
        let snap = h.snapshot();
        let note = snap.note.expect("should have detected A4");
        assert_eq!(note.name, PdNoteName::A);
        assert_eq!(note.octave, 4);
    }
}
