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

// Re-export pitch-detector's NoteName so app code can convert it
// without taking pitch-detector as a direct dep.
pub use pitch_detector::core::NoteName as DetectedNoteName;

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
mod tests;

mod pitch;
