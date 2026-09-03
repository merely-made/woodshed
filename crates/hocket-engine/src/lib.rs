// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket audio engine, built on [Firewheel](https://github.com/BillyDM/Firewheel) 0.10.
//!
//! ## Architecture
//!
//! - `hocket-model` is the *authority* for session truth (tracks,
//!   layers, history, phrase pool).
//! - This crate is the *runtime*: a Firewheel audio graph that
//!   plays/captures/mixes the currently projected session state.
//! - The engine has no opinion about `PlaybackMode`. The host (UI /
//!   demo) translates model state into a series of `play_layer` /
//!   `stop_layer` calls based on what the model says is audible.
//!   This keeps the model authoritative and the engine a dumb-pipe
//!   playback substrate.
//!
//! ## FT3b.1 graph
//!
//! ```text
//!  graph_in ──► stream_reader     (mono capture tap)
//!  graph_in ──► silent_monitor ──► graph_out  (keeps input path live)
//!
//!  click_sampler ────┐
//!  voice[0..N]   ────┼──► peak_meter ──► graph_out (stereo)
//! ```
//!
//! - `click_sampler` plays the pre-rendered click loop endlessly.
//! - Voice pool: `N` `SamplerNode`s, each capable of playing one
//!   layer at a time. Voices are addressed by [`LayerKey`] via an
//!   internal map. See [`Engine::play_layer`].
//! - `stream_reader` taps mic input and exposes samples to the non-RT
//!   thread via [`Engine::drain_input`]. The host wires those into the
//!   `Capture` state machine when ready.
//! - `peak_meter` measures the post-mix output level;
//!   [`Engine::peak_db`] exposes it for UI animation.

pub mod capture;
pub mod click;
pub mod export;
pub mod handoff;
pub mod media;
pub mod project_store;
pub mod waveform;

use std::collections::BTreeMap;
use std::sync::Arc;

use firewheel::core::node::NodeID;
use firewheel::nodes::stream::ResamplingChannelConfig;
use firewheel::{
    FirewheelConfig, FirewheelContext,
    channel_config::{ChannelCount, NonZeroChannelCount},
    collector::ArcGc,
    cpal::{CpalConfig, CpalEnumerator, CpalInputConfig, CpalOutputConfig},
    diff::Notify,
    dsp::volume::Volume,
    nodes::{
        peak_meter::{PeakMeterStereoNode, PeakMeterStereoState},
        sampler::{PlayFrom, RepeatMode, SamplerNode, SamplerState},
        stream::reader::{StreamReaderConfig, StreamReaderNode, StreamReaderState},
        volume::{VolumeNode, VolumeNodeConfig},
    },
    sample_resource::SampleResource,
};
use hocket_model::TrackId;

use audio_primitives::{OnsetDetector, estimate_bpm};

// Re-export model types that appear in this crate's public API so
// downstream consumers don't need a separate hocket-model dep just
// to construct a LayerKey.
pub use hocket_model::TrackId as ModelTrackId;

/// A selectable native audio device. `id` is a CPAL-stable identifier for
/// host-local settings; it is not project/session state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Available devices for the current system default audio host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioDevices {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
}

/// Host-local device preference. `None` follows the system default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioDeviceSelection {
    pub input_id: Option<String>,
    pub output_id: Option<String>,
}

/// Enumerate input and output devices from Firewheel's CPAL backend.
pub fn available_audio_devices() -> AudioDevices {
    let host = CpalEnumerator {}.default_host();
    let map = |device: firewheel::cpal::DeviceInfo| AudioDevice {
        id: device.id.to_string(),
        name: device.name.unwrap_or_else(|| "unnamed device".to_string()),
        is_default: device.is_default,
    };
    AudioDevices {
        inputs: host.input_devices().into_iter().map(map).collect(),
        outputs: host.output_devices().into_iter().map(map).collect(),
    }
}

/// Soft cap on simultaneously playing layer voices. Each voice is a
/// `SamplerNode` dynamically added to the graph on `play_layer` and
/// removed on `stop_layer`. The cap exists so a runaway host can't
/// allocate unbounded nodes.
///
/// Deeler profile maxes at 10 tracks × 1 active = 10 voices; looper
/// profile is bounded by the user's captured layer count.
pub const VOICE_POOL_SIZE: usize = 32;

/// How many recent input onsets the engine retains for tap-tempo
/// estimation. A handful of hits is plenty; 32 matches Woodshed's
/// onset history bound.
const ONSET_HISTORY: usize = 32;

/// Engine-side identifier for a model `Layer`. Hocket-model uses
/// `(TrackId, layer_index)` to address layers; the engine maps each
/// such key to one voice in the pool.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerKey {
    pub track_id: TrackId,
    pub layer_index: u16,
}

impl LayerKey {
    pub fn new(track_id: TrackId, layer_index: u16) -> Self {
        Self {
            track_id,
            layer_index,
        }
    }
}

/// The Hocket audio engine. Owns the Firewheel context (which in
/// turn owns the cpal stream after `start_stream`).
///
/// Not `Send` (cpal streams aren't `Send` on most platforms).
pub struct Engine {
    cx: FirewheelContext,
    meter_id: NodeID,
    click_id: NodeID,
    /// Tempo + meter for bar-phase math. Set at construction; future
    /// API will allow runtime changes wired through to the click
    /// loop and the bar-aligned scheduling state.
    bpm: f32,
    beats_per_bar: u8,
    beat_unit: u8,
    /// Map from layer key to the dynamically-added sampler node
    /// playing that layer. Nodes are added by `play_layer` and
    /// removed by `stop_layer`.
    ///
    /// **Why dynamic add/remove instead of a pre-allocated pool**:
    /// Firewheel 0.10.0's `SamplerNode` has documented state-machine
    /// bugs around post-construction state transitions (BillyDM's
    /// own TODO at the top of `sampler.rs`). Post-hoc
    /// `sync_*_event` calls to mutate an empty sampler into a
    /// playing one don't audibly take effect. The pattern that
    /// works is the one the click uses: construct the sampler
    /// fully populated and hand it to `cx.add_node`. So each
    /// `play_layer` adds a fresh node; each `stop_layer` removes it.
    voice_nodes: BTreeMap<LayerKey, NodeID>,
    /// Bar-aligned capture state machine. Set by
    /// `arm_bar_aligned_capture`; advanced inside `tick`. See
    /// [`PendingCapture`].
    pending_capture: PendingCapture,
    /// Scratch buffer reused each `tick` to drain mic input into,
    /// before feeding the bar-aligned capture. Also exposed via
    /// [`Engine::recent_input`] for VU metering.
    input_scratch: Vec<f32>,
    /// Bar-aligned replay queue. Set by `play_layer_at_next_bar`;
    /// each tick checks if the click playhead has crossed the
    /// scheduled boundary and fires the underlying `play_layer`.
    pending_layers: Vec<PendingLayer>,
    reader_state: StreamReaderState,
    meter_state: PeakMeterStereoState,
    sample_rate: u32,
    /// Onset detection over captured mic input, backed by the shared
    /// [`audio_primitives::OnsetDetector`]. Disabled by default; the
    /// host enables it for audio tap-tempo (and, later, FT3c latency
    /// calibration). When disabled, `tick` skips the per-frame DSP.
    onset_detector: OnsetDetector,
    onset_enabled: bool,
    /// Recent onset timestamps (detector-clock samples), bounded to
    /// [`ONSET_HISTORY`]. Feeds [`Engine::detected_bpm`].
    recent_onsets: std::collections::VecDeque<u64>,
}

/// Internal: bar-aligned capture state machine.
///
/// `SamplerState::playhead_frames` returns the position *within* the
/// current loop iteration (wraps every bar), not a monotonic counter.
/// So we can't say "start at playhead X" — we have to detect bar
/// crossings (playhead wrap-arounds) and decrement a bar countdown.
enum PendingCapture {
    Idle,
    /// Waiting for `bars_until_start` more bar boundaries to pass.
    /// `last_in_bar_phase` is the most recent in-bar position we
    /// observed, used to detect wrap-around (when the new position
    /// is smaller than the last, we crossed a boundary).
    Waiting {
        bars_until_start: u8,
        target_samples: usize,
        last_in_bar_phase: usize,
    },
    /// Currently accumulating real input samples.
    Recording(capture::Capture),
    /// Free (unclocked) capture — accumulates every drained sample with
    /// no target length, until `stop_free_capture`. The master-clock-off
    /// looper mode (variable-length loops).
    FreeRecording(Vec<f32>),
    /// Capture completed; buffer ready for `take_bar_aligned_capture`.
    Complete(Vec<f32>),
}

/// Internal: a layer playback queued to start at a bar boundary.
/// Same wrap-detection technique as `PendingCapture::Waiting`.
struct PendingLayer {
    bars_until_start: u8,
    last_in_bar_phase: usize,
    key: LayerKey,
    samples: Vec<f32>,
    sample_rate: u32,
    gain: f32,
    looping: bool,
}

impl Engine {
    /// Construct the engine. Opens default I/O devices, builds the
    /// FT3b.1 graph, and starts the click loop playing.
    pub fn new() -> Result<Self, EngineError> {
        Self::new_with_audio_devices(&AudioDeviceSelection::default())
    }

    /// Construct the engine for a host-local input/output preference. Missing
    /// or stale IDs intentionally fall back to the system default.
    pub fn new_with_audio_devices(selection: &AudioDeviceSelection) -> Result<Self, EngineError> {
        let mut cx = FirewheelContext::new(FirewheelConfig {
            num_graph_inputs: ChannelCount::new(1).expect("mono input channel count is valid"),
            ..Default::default()
        });

        cx.start_stream(cpal_config(selection))
            .map_err(|e| EngineError::CpalInit(format!("{e:?}")))?;

        let stream_info = cx
            .stream_info()
            .ok_or_else(|| EngineError::CpalInit("no stream_info after start".into()))?;
        let sample_rate = stream_info.sample_rate.get();
        let sr_nz = stream_info.sample_rate;

        // --- Click sampler (fully populated at add_node time;
        //     `Notify<bool>::patch` rejects post-hoc Bool events) ---
        let click_buf = click::render_click_loop(sample_rate, 120.0, 4, 4);
        let click_resource = make_mono_resource(click_buf);
        let click_node = SamplerNode {
            sample: Some(click_resource),
            repeat_mode: RepeatMode::RepeatEndlessly,
            play: Notify::new(true),
            play_from: PlayFrom::BEGINNING,
            ..SamplerNode::default()
        };
        let click_id = cx.add_node(click_node, None);

        let reader_id = cx.add_node(
            StreamReaderNode,
            Some(StreamReaderConfig {
                channels: NonZeroChannelCount::MONO,
            }),
        );
        let meter_id = cx.add_node(PeakMeterStereoNode { enabled: true }, None);

        // Silent monitor: graph_in → VolumeNode(SILENT) → graph_out.
        // Keeps the input path live so the reader actually receives
        // samples (Firewheel prunes input processing when no path
        // reaches output).
        let monitor_id = cx.add_node(
            VolumeNode {
                volume: Volume::SILENT,
                ..VolumeNode::default()
            },
            Some(VolumeNodeConfig {
                channels: NonZeroChannelCount::MONO,
            }),
        );

        let in_id = cx.graph_in_node_id();
        let out_id = cx.graph_out_node_id();

        // --- Wire graph ---
        cx.connect(in_id, reader_id, &[(0, 0)], false)
            .map_err(|e| EngineError::Graph(format!("in→reader: {e:?}")))?;
        cx.connect(in_id, monitor_id, &[(0, 0)], false)
            .map_err(|e| EngineError::Graph(format!("in→monitor: {e:?}")))?;
        cx.connect(monitor_id, out_id, &[(0, 0), (0, 1)], false)
            .map_err(|e| EngineError::Graph(format!("monitor→out: {e:?}")))?;

        // Click feeds the meter. Voice nodes are added dynamically
        // in `play_layer` and wired to the meter at that time.
        cx.connect(click_id, meter_id, &[(0, 0), (1, 1)], false)
            .map_err(|e| EngineError::Graph(format!("click→meter: {e:?}")))?;
        cx.connect(meter_id, out_id, &[(0, 0), (1, 1)], false)
            .map_err(|e| EngineError::Graph(format!("meter→out: {e:?}")))?;

        // Tell click's shared state it's playing so SamplerState
        // exposes the right status from t=0.
        if let Some(state) = cx.node_state::<SamplerState>(click_id) {
            state.mark_playing();
        }

        // --- Reader stream (non-RT consumer; autocorrect disabled) ---
        let mut reader_state = cx
            .node_state::<StreamReaderState>(reader_id)
            .ok_or_else(|| EngineError::Graph("reader state missing".into()))?
            .clone();
        let channel_config = ResamplingChannelConfig {
            underflow_autocorrect_percent_threshold: None,
            overflow_autocorrect_percent_threshold: None,
            ..ResamplingChannelConfig::default()
        };
        let new_stream_event = reader_state
            .start_stream(sr_nz, sr_nz, channel_config)
            .map_err(|_| EngineError::Graph("reader start_stream failed".into()))?;
        cx.queue_event_for(reader_id, new_stream_event.into());

        let meter_state = cx
            .node_state::<PeakMeterStereoState>(meter_id)
            .ok_or_else(|| EngineError::Graph("meter state missing".into()))?
            .clone();

        Ok(Self {
            cx,
            meter_id,
            click_id,
            bpm: 120.0,
            beats_per_bar: 4,
            beat_unit: 4,
            voice_nodes: BTreeMap::new(),
            pending_capture: PendingCapture::Idle,
            input_scratch: Vec::with_capacity(8192),
            pending_layers: Vec::new(),
            reader_state,
            meter_state,
            sample_rate,
            onset_detector: OnsetDetector::new(sample_rate as f32),
            onset_enabled: false,
            recent_onsets: std::collections::VecDeque::with_capacity(ONSET_HISTORY),
        })
    }

    /// Drive everything: drain mic input + advance the bar-aligned
    /// capture, advance queued bar-aligned layer playback, and flush
    /// the Firewheel event queue. Call regularly (~every 15 ms).
    ///
    /// This is the single "advance the engine" call. The host does
    /// not need to drain input separately; capture progresses purely
    /// from `tick`.
    pub fn tick(&mut self) -> Result<(), EngineError> {
        self.drain_and_advance_capture();
        self.advance_pending_layers();
        self.cx
            .update()
            .map_err(|e| EngineError::Tick(format!("{e:?}")))
    }

    /// The mic samples drained on the most recent `tick`. Empty if
    /// the reader had nothing. Useful for input VU metering; the
    /// buffer is overwritten each tick.
    pub fn recent_input(&self) -> &[f32] {
        &self.input_scratch
    }

    /// Enable or disable onset detection over captured mic input.
    /// Disabled by default — when off, `tick` skips the detector's
    /// per-frame DSP and the onset history stays empty. Turning it off
    /// also clears any accumulated onsets.
    ///
    /// First Hocket consumer of the shared
    /// [`audio_primitives::OnsetDetector`]; the substrate for audio
    /// tap-tempo and FT3c latency calibration.
    pub fn set_onset_detection(&mut self, enabled: bool) {
        self.onset_enabled = enabled;
        if !enabled {
            self.reset_onsets();
        }
    }

    /// Whether onset detection is currently running.
    pub fn onset_detection_enabled(&self) -> bool {
        self.onset_enabled
    }

    /// Estimate BPM from recently detected input onsets — audio
    /// tap-tempo. `None` until at least two onsets have landed. Backed
    /// by the shared [`audio_primitives::estimate_bpm`] (median
    /// inter-onset interval, clamped to 40–240 BPM).
    pub fn detected_bpm(&self) -> Option<f32> {
        let onsets: Vec<u64> = self.recent_onsets.iter().copied().collect();
        estimate_bpm(&onsets, self.sample_rate as f32)
    }

    /// How many onsets are currently in the detection history.
    pub fn detected_onset_count(&self) -> usize {
        self.recent_onsets.len()
    }

    /// Clear the onset detector + history. Use when beginning a fresh
    /// tap-tempo capture so prior playing doesn't bias the estimate.
    pub fn reset_onsets(&mut self) {
        self.onset_detector.reset();
        self.recent_onsets.clear();
    }

    /// Drain the reader into the scratch buffer and feed the
    /// bar-aligned capture state machine. Called from `tick`.
    fn drain_and_advance_capture(&mut self) {
        let available = self.reader_state.available_frames();
        if available == 0 {
            self.input_scratch.clear();
            self.advance_pending_capture_wait_only();
            return;
        }
        // Reuse the scratch allocation across ticks.
        let mut scratch = std::mem::take(&mut self.input_scratch);
        scratch.clear();
        scratch.resize(available, 0.0);
        let _ = self.reader_state.read_interleaved(&mut scratch);
        self.advance_pending_capture(&scratch);
        if self.onset_enabled {
            for ts in self.onset_detector.feed(&scratch) {
                if self.recent_onsets.len() >= ONSET_HISTORY {
                    self.recent_onsets.pop_front();
                }
                self.recent_onsets.push_back(ts);
            }
        }
        self.input_scratch = scratch;
    }

    // --- Bar-phase math ---

    /// Samples in one bar at the engine's current BPM / time
    /// signature. Pure function of `bpm`, `beats_per_bar`, `beat_unit`, and
    /// `sample_rate`.
    pub fn samples_per_bar(&self) -> usize {
        click::frames_per_bar(
            self.sample_rate,
            self.bpm,
            self.beats_per_bar,
            self.beat_unit,
        )
    }

    /// Click playhead position *within the current bar*, in samples.
    /// Returns `None` if the click sampler state is unreachable.
    ///
    /// **Note:** Firewheel's `SamplerState::playhead_frames` returns
    /// the position within the current loop iteration of the click
    /// sample (one bar long). It wraps to 0 on each bar boundary —
    /// it is *not* a monotonic since-start counter. Bar-aligned
    /// scheduling in this engine uses wrap-detection rather than
    /// absolute sample positions.
    pub fn click_in_bar_phase(&self) -> Option<usize> {
        self.cx
            .node_state::<SamplerState>(self.click_id)
            .map(|s| s.playhead_frames().0.max(0) as usize)
    }

    /// Samples remaining until the next click bar boundary. Returns
    /// `None` if the click state is unreachable.
    pub fn samples_to_next_bar(&self) -> Option<usize> {
        let in_bar = self.click_in_bar_phase()?;
        Some(self.samples_per_bar() - in_bar)
    }

    /// Audio backend sample rate, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    // --- Bar-aligned capture ---

    /// Arm a bar-aligned capture. After this call, the engine waits
    /// for the next bar boundary (plus `count_in_bars` extra bars
    /// of click), then starts accumulating real input samples into
    /// an internal `Capture` state machine. Length is
    /// `bars * samples_per_bar`.
    ///
    /// Call [`Engine::take_bar_aligned_capture`] to retrieve the
    /// completed buffer once `Capture::Complete` is reached.
    ///
    /// Returns `Err` if a capture is already in progress.
    pub fn arm_bar_aligned_capture(
        &mut self,
        bars: u8,
        count_in_bars: u8,
    ) -> Result<(), EngineError> {
        if !matches!(self.pending_capture, PendingCapture::Idle) {
            return Err(EngineError::Graph("capture already in progress".into()));
        }
        let last_in_bar_phase = self
            .click_in_bar_phase()
            .ok_or_else(|| EngineError::Graph("click bar-phase unavailable".into()))?;
        let target_samples = bars as usize * self.samples_per_bar();
        // Semantic: `count_in_bars` is full bars of click between "we
        // hit the next bar boundary" and "recording begins." The
        // initial wait to the *next* boundary doesn't count as
        // count-in — that's just "phase-align to the grid."
        // Examples (count_in_bars = 1):
        //   - Armed mid-bar: ~1 partial bar wait + 1 full bar
        //     count-in + start recording
        //   - Armed right at a boundary: ~0 wait + 1 full bar
        //     count-in + start recording
        //   - Armed just before a boundary: ~0 wait + 1 full bar
        //     count-in + start recording
        // (So count-in is *at least* `count_in_bars` full bars in
        // every case.)
        self.pending_capture = PendingCapture::Waiting {
            bars_until_start: 1 + count_in_bars,
            target_samples,
            last_in_bar_phase,
        };
        Ok(())
    }

    /// Start a **free (unclocked) capture** immediately — no bar wait,
    /// no count-in. Records every drained input sample until
    /// [`Self::stop_free_capture`]. This is the master-clock-off looper
    /// mode (variable-length loops). Returns `Err` if a capture is
    /// already in progress.
    pub fn start_free_capture(&mut self) -> Result<(), EngineError> {
        if !matches!(self.pending_capture, PendingCapture::Idle) {
            return Err(EngineError::Graph("capture already in progress".into()));
        }
        self.pending_capture = PendingCapture::FreeRecording(Vec::new());
        Ok(())
    }

    /// Stop a free capture and mark its buffer ready for
    /// [`Self::take_bar_aligned_capture`]. No-op unless a free capture
    /// is running.
    pub fn stop_free_capture(&mut self) {
        if matches!(self.pending_capture, PendingCapture::FreeRecording(_)) {
            let taken = std::mem::replace(&mut self.pending_capture, PendingCapture::Idle);
            if let PendingCapture::FreeRecording(buf) = taken {
                self.pending_capture = PendingCapture::Complete(buf);
            }
        }
    }

    /// If a bar-aligned capture has completed, drain its buffer and
    /// reset state to Idle. Returns `None` while waiting or recording.
    pub fn take_bar_aligned_capture(&mut self) -> Option<Vec<f32>> {
        if !matches!(self.pending_capture, PendingCapture::Complete(_)) {
            return None;
        }
        let taken = std::mem::replace(&mut self.pending_capture, PendingCapture::Idle);
        match taken {
            PendingCapture::Complete(buf) => Some(buf),
            _ => unreachable!(),
        }
    }

    /// Current state of the bar-aligned capture, for UI / demo
    /// progress display.
    pub fn pending_capture_progress(&self) -> CapturePhase {
        match &self.pending_capture {
            PendingCapture::Idle => CapturePhase::Idle,
            PendingCapture::Waiting {
                bars_until_start, ..
            } => CapturePhase::Waiting {
                bars_remaining: *bars_until_start,
                samples_until_next_bar: self.samples_to_next_bar().unwrap_or(0),
            },
            PendingCapture::Recording(c) => CapturePhase::Recording {
                progress: c.progress(),
            },
            PendingCapture::FreeRecording(buf) => CapturePhase::FreeRecording {
                samples_done: buf.len(),
            },
            PendingCapture::Complete(_) => CapturePhase::Complete,
        }
    }

    // --- Bar-aligned layer playback ---

    /// Queue a layer to start playing at the next click bar boundary.
    /// The play_layer call fires from the next `tick` after the
    /// playhead crosses the boundary, giving ~tick-resolution
    /// (~15 ms) precision in bar phase alignment. Future versions
    /// will use Firewheel's scheduled-events for sample-accurate
    /// scheduling.
    pub fn play_layer_at_next_bar(
        &mut self,
        key: LayerKey,
        samples: Vec<f32>,
        gain: f32,
        looping: bool,
    ) -> Result<(), EngineError> {
        self.play_layer_at_next_bar_at_sample_rate(key, samples, self.sample_rate, gain, looping)
    }

    /// Queue a layer for the next bar, preserving its source sample rate.
    pub fn play_layer_at_next_bar_at_sample_rate(
        &mut self,
        key: LayerKey,
        samples: Vec<f32>,
        sample_rate: u32,
        gain: f32,
        looping: bool,
    ) -> Result<(), EngineError> {
        let last_in_bar_phase = self
            .click_in_bar_phase()
            .ok_or_else(|| EngineError::Graph("click bar-phase unavailable".into()))?;
        self.pending_layers.push(PendingLayer {
            bars_until_start: 1,
            last_in_bar_phase,
            key,
            samples,
            sample_rate,
            gain,
            looping,
        });
        Ok(())
    }

    // --- Pending-state advancement (called from tick / drain_input) ---

    /// True if `cur` is on the other side of a bar boundary from
    /// `last`. The click playhead wraps from `bar_samples - 1` to 0
    /// on each bar boundary, so we look for `cur` to be far less
    /// than `last` (more than half a bar) — that's an unambiguous
    /// wrap.
    ///
    /// Why the "half a bar" gate: `SamplerState::playhead_frames` is
    /// read across the audio-thread/UI-thread boundary and can
    /// momentarily return a slightly stale (smaller) value due to
    /// internal buffering. A bare `cur < last` check fires on
    /// every such jitter, decrementing the bar counter spuriously.
    /// Requiring a half-bar gap eliminates jitter false positives
    /// while still catching every real wrap (a real wrap goes from
    /// near `bar_samples` to near 0 — a gap of nearly a full bar).
    fn crossed_bar_boundary(last: usize, cur: usize, bar_samples: usize) -> bool {
        cur < last && (last - cur) > bar_samples / 2
    }

    fn advance_pending_capture(&mut self, new_samples: &[f32]) {
        let bar_samples = self.samples_per_bar();
        let cur_in_bar = self.click_in_bar_phase().unwrap_or(0);

        // Take ownership transiently so we can match-and-replace.
        let current = std::mem::replace(&mut self.pending_capture, PendingCapture::Idle);
        self.pending_capture = match current {
            PendingCapture::Idle => PendingCapture::Idle,
            PendingCapture::Complete(buf) => PendingCapture::Complete(buf),
            PendingCapture::FreeRecording(mut buf) => {
                buf.extend_from_slice(new_samples);
                PendingCapture::FreeRecording(buf)
            }
            PendingCapture::Waiting {
                bars_until_start,
                target_samples,
                last_in_bar_phase,
            } => {
                let crossed =
                    Self::crossed_bar_boundary(last_in_bar_phase, cur_in_bar, bar_samples);
                let bars_remaining = if crossed {
                    bars_until_start.saturating_sub(1)
                } else {
                    bars_until_start
                };
                if bars_remaining == 0 {
                    // Boundary just crossed and no more count-in.
                    // Recording starts now. We accept up to one
                    // drain-interval (~15 ms = ~720 samples at 48 kHz)
                    // of phase imprecision because we don't trim the
                    // overshoot in new_samples.
                    let mut c = capture::Capture::new(target_samples);
                    c.arm();
                    c.feed_slice(new_samples);
                    if matches!(c.state(), capture::CaptureState::Complete) {
                        PendingCapture::Complete(c.take_completed().expect("complete"))
                    } else {
                        PendingCapture::Recording(c)
                    }
                } else {
                    PendingCapture::Waiting {
                        bars_until_start: bars_remaining,
                        target_samples,
                        last_in_bar_phase: cur_in_bar,
                    }
                }
            }
            PendingCapture::Recording(mut c) => {
                c.feed_slice(new_samples);
                if matches!(c.state(), capture::CaptureState::Complete) {
                    PendingCapture::Complete(c.take_completed().expect("complete"))
                } else {
                    PendingCapture::Recording(c)
                }
            }
        };
    }

    /// Advance only the "waiting" state of pending capture (called
    /// when drain_input returned zero — the wait counter still
    /// advances based on click playhead even if no new input arrived).
    fn advance_pending_capture_wait_only(&mut self) {
        let bar_samples = self.samples_per_bar();
        let cur_in_bar = match self.click_in_bar_phase() {
            Some(p) => p,
            None => return,
        };
        if let PendingCapture::Waiting {
            bars_until_start,
            target_samples,
            last_in_bar_phase,
        } = self.pending_capture
        {
            if Self::crossed_bar_boundary(last_in_bar_phase, cur_in_bar, bar_samples) {
                let bars_remaining = bars_until_start.saturating_sub(1);
                if bars_remaining == 0 {
                    let mut c = capture::Capture::new(target_samples);
                    c.arm();
                    self.pending_capture = PendingCapture::Recording(c);
                } else {
                    self.pending_capture = PendingCapture::Waiting {
                        bars_until_start: bars_remaining,
                        target_samples,
                        last_in_bar_phase: cur_in_bar,
                    };
                }
            } else {
                // Update last_in_bar_phase so next call sees a fresh
                // baseline for boundary detection.
                self.pending_capture = PendingCapture::Waiting {
                    bars_until_start,
                    target_samples,
                    last_in_bar_phase: cur_in_bar,
                };
            }
        }
    }

    fn advance_pending_layers(&mut self) {
        let bar_samples = self.samples_per_bar();
        let cur_in_bar = match self.click_in_bar_phase() {
            Some(p) => p,
            None => return,
        };
        let mut ready: Vec<PendingLayer> = Vec::new();
        let mut still_pending: Vec<PendingLayer> = Vec::new();
        for mut layer in std::mem::take(&mut self.pending_layers) {
            if Self::crossed_bar_boundary(layer.last_in_bar_phase, cur_in_bar, bar_samples) {
                layer.bars_until_start = layer.bars_until_start.saturating_sub(1);
            }
            layer.last_in_bar_phase = cur_in_bar;
            if layer.bars_until_start == 0 {
                ready.push(layer);
            } else {
                still_pending.push(layer);
            }
        }
        self.pending_layers = still_pending;
        for layer in ready {
            let _ = self.play_layer_at_sample_rate(
                layer.key,
                layer.samples,
                layer.sample_rate,
                layer.gain,
                layer.looping,
            );
        }
    }

    /// Latest output meter peaks (left, right) in decibels. Returns
    /// `f32::NEG_INFINITY` for channels under the -60 dB epsilon.
    pub fn peak_db(&self) -> [f32; 2] {
        self.meter_state.peak_gain_db(-60.0)
    }

    /// Start (or restart) playback of a layer.
    ///
    /// Adds a fresh, fully-populated `SamplerNode` to the graph and
    /// wires it to the meter. If `key` already has a node, the old
    /// one is removed first (effectively a restart with new state).
    /// Returns `Err(NoFreeVoices)` if the cap would be exceeded.
    ///
    /// `samples` is treated as mono at the engine's sample rate.
    /// `gain` is a linear multiplier (1.0 = unity). When `looping`,
    /// the buffer repeats endlessly until `stop_layer`; otherwise it
    /// plays once. (Note: one-shot voices currently aren't auto-
    /// cleaned-up when their buffer ends; the host should call
    /// `stop_layer` when done. Auto-cleanup via `SamplerState::stopped()`
    /// polling is a FT3b-proper enhancement.)
    pub fn play_layer(
        &mut self,
        key: LayerKey,
        samples: Vec<f32>,
        gain: f32,
        looping: bool,
    ) -> Result<(), EngineError> {
        self.play_layer_at_sample_rate(key, samples, self.sample_rate, gain, looping)
    }

    /// Start a layer at its source sample rate. Firewheel adjusts sampler speed
    /// so captures keep their original pitch and duration across device changes.
    pub fn play_layer_at_sample_rate(
        &mut self,
        key: LayerKey,
        samples: Vec<f32>,
        sample_rate: u32,
        gain: f32,
        looping: bool,
    ) -> Result<(), EngineError> {
        // Restart? Remove the existing node first.
        if let Some(old_id) = self.voice_nodes.remove(&key) {
            let _ = self.cx.remove_node(old_id);
        } else if self.voice_nodes.len() >= VOICE_POOL_SIZE {
            return Err(EngineError::NoFreeVoices);
        }

        let resource = make_mono_resource(samples);
        let repeat_mode = if looping {
            RepeatMode::RepeatEndlessly
        } else {
            RepeatMode::PlayOnce
        };
        let node = SamplerNode {
            sample: Some(resource),
            volume: Volume::Linear(gain),
            speed: playback_speed(sample_rate, self.sample_rate),
            repeat_mode,
            play: Notify::new(true),
            play_from: PlayFrom::BEGINNING,
            ..SamplerNode::default()
        };
        let sampler_id = self.cx.add_node(node, None);

        // Wire to meter (stereo since SamplerNode does mono→stereo).
        self.cx
            .connect(sampler_id, self.meter_id, &[(0, 0), (1, 1)], false)
            .map_err(|e| EngineError::Graph(format!("voice→meter: {e:?}")))?;

        // Tell shared state we're playing so SamplerState reports it.
        if let Some(state) = self.cx.node_state::<SamplerState>(sampler_id) {
            state.mark_playing();
        }

        self.voice_nodes.insert(key, sampler_id);
        Ok(())
    }

    /// Stop the layer's voice and remove its node from the graph.
    /// No-op if the key has no voice assigned.
    pub fn stop_layer(&mut self, key: LayerKey) {
        if let Some(sampler_id) = self.voice_nodes.remove(&key) {
            let _ = self.cx.remove_node(sampler_id);
        }
    }

    /// Update the gain on a playing layer. No-op if the key has no
    /// voice assigned.
    ///
    /// Uses `sync_volume_event` which produces a `ParamData::Volume`
    /// event. `Volume`'s patch path accepts this variant (unlike
    /// `Notify<bool>::patch` which rejects `ParamData::Bool`), so
    /// this works post-hoc on an active sampler.
    pub fn set_layer_gain(&mut self, key: LayerKey, gain: f32) {
        if let Some(&sampler_id) = self.voice_nodes.get(&key) {
            let node = SamplerNode {
                volume: Volume::Linear(gain),
                ..SamplerNode::default()
            };
            self.cx
                .queue_event_for(sampler_id, node.sync_volume_event());
        }
    }

    /// Re-render the click loop at a new tempo / bar length and swap it
    /// into the graph. Post-hoc sample swaps on `SamplerNode` are
    /// unreliable, so this *rebuilds* the click node (remove + add) —
    /// the same add-on-demand pattern the voices use. The click playhead
    /// restarts at bar 0, and the engine's bar-phase reference (the full tempo
    /// and time signature, read by [`Self::samples_per_bar`]) updates to
    /// match, so capture/replay stay aligned to the new grid.
    ///
    /// Already-playing layers are fixed-length buffers captured at the
    /// old tempo — they are *not* time-stretched; tempo is meant to be
    /// set before recording (or accepted as a deliberate re-pitch). The
    /// new click node starts at unity volume; callers that keep the
    /// master clock muted should re-apply [`Self::set_click_enabled`]
    /// afterwards.
    pub fn set_tempo(
        &mut self,
        bpm: f32,
        beats_per_bar: u8,
        beat_unit: u8,
    ) -> Result<(), EngineError> {
        let bpm = bpm.max(1.0);
        let beats_per_bar = beats_per_bar.max(1);
        let beat_unit = beat_unit.max(1);
        let click_buf = click::render_click_loop(self.sample_rate, bpm, beats_per_bar, beat_unit);
        let resource = make_mono_resource(click_buf);
        let node = SamplerNode {
            sample: Some(resource),
            repeat_mode: RepeatMode::RepeatEndlessly,
            play: Notify::new(true),
            play_from: PlayFrom::BEGINNING,
            ..SamplerNode::default()
        };
        let _ = self.cx.remove_node(self.click_id);
        let new_id = self.cx.add_node(node, None);
        self.cx
            .connect(new_id, self.meter_id, &[(0, 0), (1, 1)], false)
            .map_err(|e| EngineError::Graph(format!("click→meter: {e:?}")))?;
        if let Some(state) = self.cx.node_state::<SamplerState>(new_id) {
            state.mark_playing();
        }
        self.click_id = new_id;
        self.bpm = bpm;
        self.beats_per_bar = beats_per_bar;
        self.beat_unit = beat_unit;
        Ok(())
    }

    /// Mute or unmute the click loop. Used when the session's master
    /// clock is toggled off/on. Patches the click sampler's volume the
    /// same way [`Self::set_layer_gain`] does (`sync_volume_event` is
    /// accepted post-hoc, unlike a `play` toggle), so the click keeps
    /// running on its grid but goes silent — bar-phase math that reads
    /// the click's playhead is unaffected.
    pub fn set_click_enabled(&mut self, enabled: bool) {
        let node = SamplerNode {
            volume: if enabled {
                Volume::Linear(1.0)
            } else {
                Volume::SILENT
            },
            ..SamplerNode::default()
        };
        self.cx
            .queue_event_for(self.click_id, node.sync_volume_event());
    }

    /// Is `key` currently assigned to a voice (i.e. nominally
    /// playing)? Note: one-shot voices may still report assigned
    /// after their buffer has naturally ended; the host should
    /// `stop_layer` to clean up explicitly.
    pub fn is_layer_assigned(&self, key: LayerKey) -> bool {
        self.voice_nodes.contains_key(&key)
    }

    /// Number of voices currently playing.
    pub fn voice_count(&self) -> usize {
        self.voice_nodes.len()
    }

    /// Stop the audio stream. Also runs on drop.
    pub fn stop(&mut self) {
        self.cx.stop_stream();
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.cx.stop_stream();
    }
}

fn cpal_config(selection: &AudioDeviceSelection) -> CpalConfig {
    let parse_device_id = |id: Option<&String>| id.and_then(|id| id.parse().ok());
    CpalConfig {
        output: CpalOutputConfig {
            device_id: parse_device_id(selection.output_id.as_ref()),
            ..Default::default()
        },
        input: Some(CpalInputConfig {
            device_id: parse_device_id(selection.input_id.as_ref()),
            ..Default::default()
        }),
    }
}

fn playback_speed(source_sample_rate: u32, engine_sample_rate: u32) -> f64 {
    source_sample_rate.max(1) as f64 / engine_sample_rate.max(1) as f64
}

/// Wrap a mono `Vec<f32>` into an `ArcGc<dyn SampleResource>` that the
/// SamplerNode can play. Uses the bundled `impl SampleResource for
/// Vec<Vec<f32>>` (de-interleaved channels = outer Vec, samples = inner
/// Vec) — we have one channel.
fn make_mono_resource(samples: Vec<f32>) -> ArcGc<dyn SampleResource> {
    let buffer: Vec<Vec<f32>> = vec![samples];
    ArcGc::new_unsized(|| {
        let arc: Arc<dyn SampleResource> = Arc::new(buffer);
        arc
    })
}

/// Snapshot of bar-aligned capture progress, for UI display.
#[derive(Debug, Clone, PartialEq)]
pub enum CapturePhase {
    Idle,
    Waiting {
        bars_remaining: u8,
        samples_until_next_bar: usize,
    },
    Recording {
        progress: f32,
    },
    /// Free (unclocked) capture in progress — no fixed length. Records
    /// until stopped; `samples_done` is how much is captured so far.
    FreeRecording {
        samples_done: usize,
    },
    Complete,
}

/// Errors raised by the engine.
#[derive(Debug)]
pub enum EngineError {
    CpalInit(String),
    Graph(String),
    Tick(String),
    /// Voice pool is full — no idle voice could be allocated for a
    /// new `play_layer` call. Either `stop_layer` an existing one or
    /// raise `VOICE_POOL_SIZE`.
    NoFreeVoices,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CpalInit(m) => write!(f, "audio backend init failed: {m}"),
            Self::Graph(m) => write!(f, "audio graph error: {m}"),
            Self::Tick(m) => write!(f, "engine tick failed: {m}"),
            Self::NoFreeVoices => write!(
                f,
                "voice pool exhausted (max {VOICE_POOL_SIZE} simultaneous layers)"
            ),
        }
    }
}

impl std::error::Error for EngineError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_speed_preserves_source_duration_across_device_rates() {
        assert!((playback_speed(44_100, 48_000) - 0.918_75).abs() < f64::EPSILON);
        assert!((playback_speed(48_000, 44_100) - 1.088_435_374).abs() < 1e-9);
    }

    #[test]
    fn invalid_device_ids_fall_back_to_system_defaults() {
        let config = cpal_config(&AudioDeviceSelection {
            input_id: Some("not a CPAL device id".to_string()),
            output_id: Some("also not a device id".to_string()),
        });
        assert!(config.output.device_id.is_none());
        assert!(config.input.unwrap().device_id.is_none());
    }
}
