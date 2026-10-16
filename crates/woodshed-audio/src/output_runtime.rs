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
    sync::{Arc, atomic::Ordering},
};

mod preview;
mod processor;
#[cfg(test)]
mod tests;
use preview::PreviewWorker;
use processor::{
    ChordCache, Command, OutputNode, PreparedBacking, PreparedClick, Recording, Shared,
};

pub(crate) const COMMAND_BOUND: usize = 64;
pub(crate) const MAX_BARS: usize = 256;
pub(crate) const MAX_PCM_FRAMES: usize = 16_777_216;
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;

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
    pub click_playing: bool,
    pub click_step: usize,
}

pub struct OutputRuntime {
    // Streams/processors must return before context destruction.
    stream: Option<CpalStream>,
    offline_processor: Option<FirewheelProcessor>,
    cx: FirewheelContext,
    node: NodeID,
    shared: Arc<Shared>,
    returns: rtrb::Consumer<OwnedGc<Recording>>,
    preview: PreviewWorker,
    song: Song,
    chord_cache: Vec<ChordCache>,
    metronome: (f32, bool),
    rate: u32,
    capture_attached: bool,
    capture_requested: bool,
    native: bool,
    device_errors: u64,
    last_error: Option<OutputError>,
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
        let node = cx
            .add_node(OutputNode::new(shared.clone(), return_tx), None)
            .map_err(|e| err(format!("output node: {e:?}")))?;
        cx.connect(node, cx.graph_out_node_id(), &[(0, 0), (0, 1)], false)
            .map_err(|e| err(format!("output wiring: {e:?}")))?;
        let native = stream.is_some();
        let mut result = Self {
            stream,
            offline_processor,
            cx,
            node,
            shared,
            returns,
            preview: PreviewWorker::new(),
            song: Song::new(),
            chord_cache: Vec::new(),
            metronome: (120.0, false),
            rate,
            capture_attached: false,
            capture_requested: false,
            native,
            device_errors: 0,
            last_error: None,
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
            if take.valid && take.generation == self.shared.generation.load(Ordering::Acquire) {
                for (idx, bar) in take.bars.iter_mut().enumerate() {
                    if bar.touched {
                        self.song.bars[idx].audio_buffer = Some(bar.pcm.clone());
                    }
                }
            } else if !take.valid
                && take.generation == self.shared.generation.load(Ordering::Acquire)
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
    pub fn attach_capture_handle(
        &mut self,
        handle: &crate::LooperCaptureHandle,
    ) -> Result<(), OutputError> {
        if self.capture_busy() {
            return Err(err("cannot replace a busy input feed"));
        }
        if handle.source_rate() == 0 || handle.source_rate() != self.rate {
            return Err(err("capture requires equal nonzero input/output rates"));
        }
        if self.native && self.stream.is_none() {
            return Err(err("output stream is inactive"));
        }
        self.shared
            .commands
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < COMMAND_BOUND).then_some(n + 1)
            })
            .map_err(|_| err("output command queue is full"))?;
        let Some(feed) = handle.take_consumer() else {
            self.shared.commands.fetch_sub(1, Ordering::Release);
            return Err(err("input consumer is already attached"));
        };
        self.cx.queue_event_for(
            self.node,
            NodeEventType::custom(Command::Capture(Some(OwnedGc::new(feed)))),
        );
        self.capture_attached = true;
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
    /// Read one processor publication plus the control-side reservation. A UI
    /// projection must not combine different blocks' capture phase and cursor.
    pub fn capture_observation(&self) -> (OutputSnapshot, bool) {
        let snapshot = self.snapshot();
        let busy = self.capture_requested || snapshot.recording || snapshot.capture_armed;
        (snapshot, busy)
    }
    pub fn capture_busy(&self) -> bool {
        self.capture_observation().1
    }
    pub fn song(&self) -> Song {
        let mut song = self.song.clone();
        let s = self.snapshot();
        song.cursor = s.cursor;
        if self.shared.commands.load(Ordering::Acquire) == 0 {
            song.playing = s.playing;
        }
        song.recording = s.recording;
        if !self.capture_busy() {
            song.pending.clear();
        }
        song
    }
    pub fn set_song(&mut self, mut song: Song) -> Result<(), OutputError> {
        if self.capture_busy() {
            return Err(err(
                "cannot replace backing while capture is armed or active",
            ));
        }
        let prepared = PreparedBacking::prepare(&song, self.rate, &mut self.chord_cache)?;
        let generation = self
            .shared
            .generation
            .load(Ordering::Acquire)
            .wrapping_add(1);
        self.send(Command::Backing {
            data: Some(OwnedGc::new(prepared)),
            reset: true,
            clear: false,
            playing: song.playing,
            generation,
        })?;
        self.cancel_worker();
        self.shared.generation.store(generation, Ordering::Release);
        song.cursor = Default::default();
        song.recording = false;
        song.pending.clear();
        self.song = song;
        Ok(())
    }
    pub fn clear_recording(&mut self, bar_idx: usize) -> Result<(), OutputError> {
        if self.capture_busy() {
            return Err(err("cannot clear a busy capture"));
        }
        let mut song = self.song.clone();
        song.bars
            .get_mut(bar_idx)
            .ok_or_else(|| err("recording bar out of range"))?
            .audio_buffer = None;
        let data = PreparedBacking::prepare(&song, self.rate, &mut self.chord_cache)?;
        self.send(Command::Backing {
            data: Some(OwnedGc::new(data)),
            reset: false,
            clear: false,
            playing: self.song().playing,
            generation: self.shared.generation.load(Ordering::Acquire),
        })?;
        self.song = song;
        Ok(())
    }
    pub fn set_metronome(&mut self, bpm: f32, playing: bool) -> Result<(), OutputError> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(err("metronome tempo must be finite and positive"));
        }
        if self.metronome == (bpm, playing) {
            return Ok(());
        }
        let transition = playing != self.metronome.1;
        self.send(Command::Metronome { bpm, playing })?;
        if transition {
            self.cancel_worker();
        }
        self.metronome = (bpm, playing);
        Ok(())
    }
    pub fn set_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), OutputError> {
        let bpm = pattern.bpm;
        let data = PreparedClick::prepare(&pattern, self.rate)?;
        self.send(Command::ClickPattern {
            data: Some(OwnedGc::new(data)),
            bpm,
            playing: None,
        })?;
        self.metronome.0 = bpm;
        Ok(())
    }
    /// Prepare and admit the pattern and play state as one bounded operation.
    pub fn set_click_pattern_playing(
        &mut self,
        pattern: SequencerPattern,
        playing: bool,
    ) -> Result<(), OutputError> {
        let bpm = pattern.bpm;
        let data = PreparedClick::prepare(&pattern, self.rate)?;
        self.send(Command::ClickPattern {
            data: Some(OwnedGc::new(data)),
            bpm,
            playing: Some(playing),
        })?;
        self.metronome = (bpm, playing);
        Ok(())
    }
    pub fn start_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), OutputError> {
        self.set_click_pattern_playing(pattern, true)
    }
    pub fn play_click(&mut self) -> Result<(), OutputError> {
        self.send(Command::ClickPlaying(true))?;
        self.metronome.1 = true;
        Ok(())
    }
    pub fn stop_click(&mut self) -> Result<(), OutputError> {
        self.send(Command::ClickPlaying(false))?;
        self.metronome.1 = false;
        Ok(())
    }
    pub fn play(&mut self) -> Result<(), OutputError> {
        self.cancel_worker();
        self.send(Command::Playing(true))?;
        self.song.playing = true;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), OutputError> {
        self.cancel_worker();
        self.send(Command::Playing(false))?;
        self.song.playing = false;
        Ok(())
    }
    pub fn rewind(&mut self) -> Result<(), OutputError> {
        self.cancel_worker();
        self.send(Command::Rewind)
    }
    pub fn set_output_gain(&mut self, gain: f32) -> Result<(), OutputError> {
        if !gain.is_finite() {
            return Err(err("output gain must be finite"));
        }
        self.send(Command::Gain(gain.clamp(0.0, 1.0)))
    }
    pub fn set_record_replace(&mut self, replace: bool) -> Result<(), OutputError> {
        self.send(Command::RecordReplace(replace))?;
        self.song.record_replace = replace;
        Ok(())
    }
    pub fn arm_capture(&mut self, bar_idx: usize, seek: bool) -> Result<(), OutputError> {
        if !self.capture_attached {
            return Err(err("recording input is not attached"));
        }
        if self.capture_busy() {
            return Err(err("capture already armed or active"));
        }
        if bar_idx >= self.song.bars.len() {
            return Err(err("recording bar out of range"));
        }
        let take = Recording::prepare(
            &self.song,
            self.rate,
            self.shared.generation.load(Ordering::Acquire),
        )?;
        self.send_boundary(Command::Arm {
            bar_idx,
            seek,
            take: Some(OwnedGc::new(take)),
        })?;
        self.capture_requested = true;
        self.song
            .pending
            .push(PendingChange::StartRecording { bar_idx });
        Ok(())
    }
    pub fn stop_capture_now(&mut self) -> Result<(), OutputError> {
        self.send(Command::StopCapture)
    }
    pub fn queue(&mut self, change: PendingChange) -> Result<(), OutputError> {
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
    fn cancel_worker(&mut self) {
        let revision = self.preview.cancel();
        self.shared
            .preview_revision
            .store(revision, Ordering::Release);
    }
    pub fn cancel_preview(&mut self) -> Result<(), OutputError> {
        self.cancel_worker();
        self.send(Command::CancelPreview)
    }
    pub fn preview_note(&mut self, frequency: f32, duration: f32) -> Result<(), OutputError> {
        self.preview_pitches_inner(&[frequency], duration.max(0.05), 0.0, false)
    }
    pub fn preview_pitches(
        &mut self,
        pitches: &[f32],
        duration: f32,
        strum: f32,
    ) -> Result<(), OutputError> {
        self.preview_pitches_inner(pitches, duration.max(0.1), strum.max(0.0), true)
    }
    fn preview_pitches_inner(
        &mut self,
        pitches: &[f32],
        duration: f32,
        strum: f32,
        replace: bool,
    ) -> Result<(), OutputError> {
        if self.snapshot().playing {
            return Ok(());
        }
        if !duration.is_finite()
            || !strum.is_finite()
            || duration * self.rate as f32 > MAX_PCM_FRAMES as f32
            || pitches.len() > 256
        {
            return Err(err("preview exceeds supported resource bounds"));
        }
        let pitches: Vec<_> = pitches
            .iter()
            .copied()
            .filter(|f| f.is_finite() && *f > 0.0)
            .collect();
        if pitches.is_empty() {
            return Ok(());
        }
        if replace {
            self.send(Command::CancelPreview)?;
        }
        let revision = self.preview.request(
            ChordRender {
                pitches_hz: pitches,
                duration_seconds: duration,
                strum_offset_ms: strum,
                ..Default::default()
            },
            self.rate,
        );
        self.shared
            .preview_revision
            .store(revision, Ordering::Release);
        Ok(())
    }

    /// Queue two independently voiced chord groups as one ordered preview.
    /// The worker renders both groups and the pause into one bounded buffer,
    /// so the output processor plays before then after without a later request
    /// cancelling the first chord.
    pub fn preview_phrase(
        &mut self,
        before_hz: &[f32],
        after_hz: &[f32],
        chord_duration_secs: f32,
        gap_secs: f32,
        strum_ms: f32,
    ) -> Result<(), OutputError> {
        if self.snapshot().playing {
            return Ok(());
        }
        if !chord_duration_secs.is_finite()
            || chord_duration_secs <= 0.0
            || !gap_secs.is_finite()
            || gap_secs < 0.0
            || !strum_ms.is_finite()
            || before_hz.len() > 256
            || after_hz.len() > 256
            || (2.0 * chord_duration_secs + gap_secs) * self.rate as f32 > MAX_PCM_FRAMES as f32
        {
            return Err(err("ordered preview exceeds supported resource bounds"));
        }
        let valid = |pitches: &[f32]| {
            pitches
                .iter()
                .copied()
                .filter(|frequency| frequency.is_finite() && *frequency > 0.0)
                .collect::<Vec<_>>()
        };
        let before = valid(before_hz);
        let after = valid(after_hz);
        if before.is_empty() || after.is_empty() {
            return Ok(());
        }
        let group = |pitches_hz| ChordRender {
            pitches_hz,
            duration_seconds: chord_duration_secs,
            strum_offset_ms: strum_ms.max(0.0),
            ..Default::default()
        };
        // Cancel any currently audible preview before replacing its worker
        // request. Cancelling the worker alone cannot retire a voice already
        // handed to the processor.
        self.send(Command::CancelPreview)?;
        let revision =
            self.preview
                .request_phrase(vec![group(before), group(after)], gap_secs, self.rate);
        self.shared
            .preview_revision
            .store(revision, Ordering::Release);
        Ok(())
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
