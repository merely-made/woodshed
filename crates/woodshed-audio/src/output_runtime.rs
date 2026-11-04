//! One output device, one Firewheel graph clock, and control-owned projections.
//!
//! Native input remains independently lazy. Capture presently requires equal
//! nominal input/output rates; this does not imply synchronized hardware clocks.
//! Commands are bounded to 64 outstanding operations, preview envelopes to 16,
//! and prepared backing PCM to 16,777,216 frames across at most 256 segments.
//! Allocation, synthesis, resource collection and completed-take publication
//! run on the control thread. The processor owns its mutable recording storage.

use crate::{
    SampleBuffer, SequencerPattern, Sound,
    chord_audio::{ChordRender, render_chord},
    song::{PendingChange, Song, SongCursor},
};
use audioadapter_buffers::direct::InterleavedSlice;
use firewheel::{
    ActivateInfo, FirewheelConfig, FirewheelContext,
    backend::BackendProcessInfo,
    channel_config::ChannelCount,
    collector::{ArcGc, GlobalRtGc, OwnedGc},
    cpal::{CpalConfig, CpalStream},
    event::NodeEventType,
    node::{NodeID, StreamStatus},
    processor::FirewheelProcessor,
};
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

mod clock;
mod preview;
mod processor;
#[cfg(test)]
mod tests;
pub use clock::{
    BEAT_TICKS, PROGRAM_TICKS_PER_BEAT, PreparedProgram, ProgramCard, ProgramDuration, ProgramEnd,
    ProgramEndReason, ProgramEpoch, ProgramEvent, ProgramLoop, ProgramPlan, ProgramPosition,
    ProgramSnapshot, ProgramStatus, ProgramToken, ProgramTransition, ProgramTransitionKind,
};
use preview::PreviewWorker;
use processor::{
    ChordCache, Command, CompletedRecording, OutputNode, PreparedBacking, PreparedClick, Recording,
    RecordingScope, Shared,
};

pub(crate) const COMMAND_BOUND: usize = 64;
pub(crate) const MAX_BARS: usize = 256;
pub(crate) const MAX_PCM_FRAMES: usize = 16_777_216;
pub(crate) const MAX_PROGRAM_CARDS: usize = 256;
pub(crate) const MAX_PROGRAM_EVENTS: usize = 4096;
pub(crate) const MAX_PROGRAM_PCM_FRAMES: usize = 16_777_216;
pub(crate) const MAX_PROGRAM_VOICES: usize = 256;
pub(crate) const PROGRAM_TRANSITION_BOUND: usize = 256;
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;
static RUNTIME_OBSERVATION_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputError(pub String);
impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for OutputError {}
fn err(message: impl Into<String>) -> OutputError {
    OutputError(message.into())
}

/// Coherent processor publication, measured in graph/device frames.
/// `processing_blocks` counts node processing blocks, not CPAL callbacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputSnapshot {
    pub device_frames: u64,
    pub processing_blocks: u64,
    pub cursor: SongCursor,
    pub playing: bool,
    pub recording: bool,
    pub capture_armed: bool,
    pub capture_invalid: bool,
    pub preview_voices: usize,
    /// PCM previews admitted by the processor, including ones already ended.
    /// This is software admission evidence, not physical audibility.
    pub preview_starts: u64,
    pub click_playing: bool,
    pub click_step: usize,
    pub program: ProgramSnapshot,
}

pub struct OutputRuntime {
    observation_id: u64,
    // Streams/processors must return before context destruction.
    stream: Option<CpalStream>,
    offline_processor: Option<FirewheelProcessor>,
    cx: FirewheelContext,
    node: NodeID,
    shared: Arc<Shared>,
    returns: rtrb::Consumer<CompletedRecording>,
    program_transitions: rtrb::Consumer<ProgramTransition>,
    preview: PreviewWorker,
    song: Song,
    chord_cache: Vec<ChordCache>,
    metronome: (f32, bool),
    rate: u32,
    capture_attached: bool,
    capture_requested: bool,
    selected_capture_requested: bool,
    native: bool,
    device_errors: u64,
    last_error: Option<OutputError>,
    next_program_epoch: u64,
    active_program_cards: Option<usize>,
    requested_program_revision: u64,
}

impl OutputRuntime {
    pub fn new() -> Result<Self, OutputError> {
        let mut cx = context();
        let stream = CpalStream::new(&mut cx, CpalConfig::default())
            .map_err(|e| err(format!("output device: {e:?}")))?;
        let rate = cx
            .stream_info()
            .ok_or_else(|| err("output has no stream info"))?
            .sample_rate
            .get();
        Self::build(cx, Some(stream), None, rate)
    }

    /// Production graph/control without device enumeration or a native stream.
    pub fn offline(rate: u32) -> Result<Self, OutputError> {
        valid_rate(rate)?;
        let mut cx = context();
        let processor = cx
            .activate(ActivateInfo {
                sample_rate: NonZeroU32::new(rate).unwrap(),
                max_block_frames: NonZeroU32::new(8192).unwrap(),
                num_stream_in_channels: 0,
                num_stream_out_channels: 2,
                input_to_output_latency_seconds: 0.0,
            })
            .map_err(|e| err(format!("offline graph: {e:?}")))?;
        Self::build(cx, None, Some(processor), rate)
    }

    fn build(
        mut cx: FirewheelContext,
        stream: Option<CpalStream>,
        offline_processor: Option<FirewheelProcessor>,
        rate: u32,
    ) -> Result<Self, OutputError> {
        valid_rate(rate)?;
        let shared = Arc::new(Shared::default());
        let (return_tx, returns) = rtrb::RingBuffer::new(COMMAND_BOUND);
        let (program_transition_tx, program_transitions) =
            rtrb::RingBuffer::new(PROGRAM_TRANSITION_BOUND);
        let node = cx
            .add_node(
                OutputNode::new(shared.clone(), return_tx, program_transition_tx),
                None,
            )
            .map_err(|e| err(format!("output node: {e:?}")))?;
        cx.connect(node, cx.graph_out_node_id(), &[(0, 0), (0, 1)], false)
            .map_err(|e| err(format!("output wiring: {e:?}")))?;
        let native = stream.is_some();
        let mut result = Self {
            observation_id: RUNTIME_OBSERVATION_IDS.fetch_add(1, Ordering::Relaxed),
            stream,
            offline_processor,
            cx,
            node,
            shared,
            returns,
            program_transitions,
            preview: PreviewWorker::new(),
            song: Song::new(),
            chord_cache: Vec::new(),
            metronome: (120.0, false),
            rate,
            capture_attached: false,
            capture_requested: false,
            selected_capture_requested: false,
            native,
            device_errors: 0,
            last_error: None,
            next_program_epoch: 0,
            active_program_cards: None,
            requested_program_revision: 0,
        };
        result.set_click_pattern(SequencerPattern::metronome(
            120.0,
            Default::default(),
            crate::Subdivision::QUARTER,
        ))?;
        result.set_song(Song::new())?;
        result.service()?;
        Ok(result)
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }
    /// Process-local diagnostic scope. Never a retained song or Card identity.
    pub fn observation_id(&self) -> u64 {
        self.observation_id
    }
    pub fn native_stream_count(&self) -> usize {
        usize::from(self.native && self.stream.is_some())
    }
    /// CPAL status messages observed by control-side service, including messages
    /// that did not stop the device. This is independent of capture-loss errors.
    pub fn device_error_count(&self) -> u64 {
        self.device_errors
    }
    pub fn snapshot(&self) -> OutputSnapshot {
        self.shared.snapshot()
    }
    pub fn last_error(&self) -> Option<&OutputError> {
        self.last_error.as_ref()
    }
    pub fn set_wake(&mut self, wake: Wake) {
        self.preview.set_wake(wake);
    }
    pub fn preview_busy(&self) -> bool {
        self.preview.busy()
    }
    pub fn needs_service(&self) -> bool {
        let s = self.snapshot();
        self.preview.busy()
            || self.shared.commands.load(Ordering::Acquire) != 0
            || s.playing
            || matches!(
                s.program.status,
                ProgramStatus::Playing
                    | ProgramStatus::Paused
                    | ProgramStatus::WaitingManual
                    | ProgramStatus::Refused
                    | ProgramStatus::WaitingObserver
            )
            || s.click_playing
            || s.recording
            || s.capture_armed
            || s.preview_voices != 0
    }

    /// Flush worker completions/events, reclaim resources, and observe device health.
    /// Hosts also service periodically while stopped to observe device failure.
    pub fn service(&mut self) -> Result<(), OutputError> {
        if let Some(stream) = self.stream.as_mut() {
            for problem in stream.poll_status() {
                self.device_errors = self.device_errors.saturating_add(1);
                self.last_error = Some(err(format!("output stream: {problem:?}")));
            }
            if !stream.all_streams_ok() {
                self.stream = None;
                let problem = err("output stream stopped; create a fresh runtime to retry");
                self.last_error = Some(problem.clone());
                return Err(problem);
            }
        } else if self.native {
            return Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| err("output stream is inactive")));
        }
        while let Ok(mut take) = self.returns.pop() {
            if take.stamp.valid
                && take.stamp.generation == self.shared.generation.load(Ordering::Acquire)
            {
                let scope = take.storage.scope;
                for (idx, bar) in take.storage.bars.iter_mut().enumerate() {
                    if bar.touched && scope.includes(idx) {
                        self.song.bars[idx].audio_buffer = Some(bar.pcm.clone());
                    }
                }
            } else if !take.stamp.valid
                && take.stamp.generation == self.shared.generation.load(Ordering::Acquire)
            {
                self.last_error = Some(err(
                    "recording invalidated by input loss; incomplete take discarded",
                ));
            }
        }
        if let Some(completion) = (self.shared.commands.load(Ordering::Acquire) < COMMAND_BOUND)
            .then(|| self.preview.take_completion())
            .flatten()
        {
            if completion.revision == self.shared.preview_revision.load(Ordering::Acquire) {
                self.send(Command::Preview {
                    revision: completion.revision,
                    buffer: Some(ArcGc::new(completion.buffer)),
                })?;
            }
        }
        self.cx
            .update()
            .map_err(|e| err(format!("output service: {e:?}")))?;
        GlobalRtGc::collect();
        let s = self.snapshot();
        if self.shared.commands.load(Ordering::Acquire) == 0 && !s.capture_armed && !s.recording {
            self.capture_requested = false;
            self.selected_capture_requested = false;
            self.song.pending.clear();
        }
        Ok(())
    }

    pub fn render_offline(&mut self, output: &mut [f32]) -> Result<(), OutputError> {
        if !output.len().is_multiple_of(2) {
            return Err(err("offline output must have complete stereo frames"));
        }
        let frames = output.len() / 2;
        let processor = self
            .offline_processor
            .as_mut()
            .ok_or_else(|| err("runtime is not offline"))?;
        let input = InterleavedSlice::new(&[], 0, frames).unwrap();
        let mut adapter = InterleavedSlice::new_mut(output, 2, frames).unwrap();
        processor.process(
            &input,
            &mut adapter,
            BackendProcessInfo {
                frames,
                process_timestamp: None,
                duration_since_stream_start: std::time::Duration::from_secs_f64(
                    self.shared.frames.load(Ordering::Acquire) as f64 / self.rate as f64,
                ),
                input_stream_status: StreamStatus::empty(),
                output_stream_status: StreamStatus::empty(),
                dropped_frames: 0,
                process_to_playback_delay: None,
            },
        );
        Ok(())
    }

    fn send_boundary(&mut self, command: Command) -> Result<(), OutputError> {
        self.shared
            .boundaries
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < COMMAND_BOUND).then_some(n + 1)
            })
            .map_err(|_| err("pending boundary operation queue is full"))?;
        if let Err(error) = self.send(command) {
            self.shared.boundaries.fetch_sub(1, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }
    pub fn clear_session(&mut self) -> Result<(), OutputError> {
        let song = Song::new();
        let data = PreparedBacking::prepare(&song, self.rate, &mut self.chord_cache)?;
        let generation = self
            .shared
            .generation
            .load(Ordering::Acquire)
            .wrapping_add(1);
        self.send(Command::Backing {
            data: Some(OwnedGc::new(data)),
            reset: true,
            clear: true,
            playing: false,
            generation,
        })?;
        self.cancel_worker();
        self.shared.generation.store(generation, Ordering::Release);
        self.song = song;
        Ok(())
    }
    fn send(&mut self, command: Command) -> Result<(), OutputError> {
        if self.native && self.stream.is_none() {
            return Err(err("output stream is inactive"));
        }
        self.shared
            .commands
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < COMMAND_BOUND).then_some(n + 1)
            })
            .map_err(|_| err("output command queue is full"))?;
        self.cx
            .queue_event_for(self.node, NodeEventType::custom(command));
        Ok(())
    }
    fn program_owns_output(&self) -> bool {
        let status = self.snapshot().program.status;
        (self.active_program_cards.is_some()
            && !matches!(status, ProgramStatus::Stopped | ProgramStatus::Complete))
            || matches!(
                status,
                ProgramStatus::Playing
                    | ProgramStatus::Paused
                    | ProgramStatus::WaitingManual
                    | ProgramStatus::Refused
                    | ProgramStatus::WaitingObserver
            )
    }

    pub fn queue(&mut self, change: PendingChange) -> Result<(), OutputError> {
        if self.selected_capture_requested && matches!(change, PendingChange::SeekTo { .. }) {
            return Err(err(
                "stop selected capture before selecting another backing segment",
            ));
        }
        if self.program_owns_output()
            && matches!(
                change,
                PendingChange::SetPlaying(true) | PendingChange::StartRecording { .. }
            )
        {
            return Err(err(
                "prepared program owns the output lane; stop it before backing playback or capture",
            ));
        }
        match change {
            PendingChange::StartRecording { bar_idx } => self.arm_capture(bar_idx, false),
            other => {
                self.send_boundary(Command::Boundary(other.clone()))?;
                if matches!(
                    other,
                    PendingChange::SeekTo { .. } | PendingChange::SetPlaying(_)
                ) {
                    self.cancel_worker();
                }
                self.song.pending.push(other);
                Ok(())
            },
        }
    }
}

impl crate::calibration::ClickControl for OutputRuntime {
    type Error = OutputError;
    fn start_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), Self::Error> {
        OutputRuntime::start_click_pattern(self, pattern)
    }
    fn set_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), Self::Error> {
        OutputRuntime::set_click_pattern(self, pattern)
    }
    fn play_click(&mut self) -> Result<(), Self::Error> {
        OutputRuntime::play_click(self)
    }
    fn stop_click(&mut self) -> Result<(), Self::Error> {
        OutputRuntime::stop_click(self)
    }
}

fn context() -> FirewheelContext {
    FirewheelContext::new(FirewheelConfig {
        num_graph_inputs: ChannelCount::ZERO,
        num_graph_outputs: ChannelCount::STEREO,
        initial_node_capacity: 4,
        initial_edge_capacity: 4,
        event_queue_capacity: COMMAND_BOUND,
        immediate_event_capacity: COMMAND_BOUND,
        ..Default::default()
    })
}
fn valid_rate(rate: u32) -> Result<(), OutputError> {
    if (1000..=384_000).contains(&rate) {
        Ok(())
    } else {
        Err(err("output rate must be 1000..=384000 Hz"))
    }
}

mod program_control;

mod preview_control;

mod capture_control;

mod transport;
