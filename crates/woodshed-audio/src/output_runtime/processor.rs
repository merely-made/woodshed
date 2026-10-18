use super::clock::{
    PreparedProgram, PreparedTone, ProgramDuration, ProgramEnd, ProgramEndReason, ProgramEpoch,
    ProgramLoop, ProgramPosition, ProgramSnapshot, ProgramStatus, ProgramTransition,
    ProgramTransitionKind,
};
use super::*;
use crate::capture_feed::CaptureFeedConsumer;
use audio_primitives::capture::CompletedTake;
use firewheel::{
    channel_config::{ChannelConfig, ChannelCount},
    collector::{ArcGc, OwnedGc},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        NodeError, ProcBuffers, ProcExtra, ProcInfo, ProcessStatus,
    },
};
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

#[derive(Default)]
pub(super) struct Shared {
    pub commands: AtomicUsize,
    pub generation: AtomicU64,
    pub preview_revision: AtomicU64,
    pub boundaries: AtomicUsize,
    sequence: AtomicU64,
    pub frames: AtomicU64,
    blocks: AtomicU64,
    bar: AtomicUsize,
    in_bar: AtomicU64,
    flags: AtomicUsize,
    previews: AtomicUsize,
    preview_starts: AtomicU64,
    click_step: AtomicUsize,
    program_epoch: AtomicU64,
    program_card: AtomicUsize,
    program_token: AtomicU64,
    program_event: AtomicUsize,
    program_event_token: AtomicU64,
    program_ticks: AtomicU64,
    program_bpm_bits: AtomicU64,
    program_click_subdivision_ticks: AtomicU64,
    program_frames: AtomicU64,
    program_active_frames: AtomicU64,
    program_status: AtomicUsize,
}
impl Shared {
    fn publish(&self, snapshot: OutputSnapshot) {
        self.sequence.fetch_add(1, Ordering::AcqRel);
        self.frames.store(snapshot.device_frames, Ordering::Relaxed);
        self.blocks
            .store(snapshot.processing_blocks, Ordering::Relaxed);
        self.bar.store(snapshot.cursor.bar_idx, Ordering::Relaxed);
        self.in_bar
            .store(snapshot.cursor.sample_in_bar, Ordering::Relaxed);
        self.flags.store(
            usize::from(snapshot.playing)
                | usize::from(snapshot.recording) << 1
                | usize::from(snapshot.capture_armed) << 2
                | usize::from(snapshot.capture_invalid) << 3
                | usize::from(snapshot.click_playing) << 4,
            Ordering::Relaxed,
        );
        self.previews
            .store(snapshot.preview_voices, Ordering::Relaxed);
        self.preview_starts
            .store(snapshot.preview_starts, Ordering::Relaxed);
        self.click_step
            .store(snapshot.click_step, Ordering::Relaxed);
        self.program_epoch
            .store(snapshot.program.epoch.0, Ordering::Relaxed);
        self.program_card
            .store(snapshot.program.card_index, Ordering::Relaxed);
        self.program_token.store(
            snapshot.program.card_token.unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.program_event
            .store(snapshot.program.event_index, Ordering::Relaxed);
        self.program_event_token.store(
            snapshot.program.event_token.unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.program_ticks
            .store(snapshot.program.beat_ticks, Ordering::Relaxed);
        self.program_bpm_bits
            .store(snapshot.program.bpm_bits, Ordering::Relaxed);
        self.program_click_subdivision_ticks
            .store(snapshot.program.click_subdivision_ticks, Ordering::Relaxed);
        self.program_frames
            .store(snapshot.program.card_frames, Ordering::Relaxed);
        self.program_active_frames
            .store(snapshot.program.active_frames, Ordering::Relaxed);
        self.program_status
            .store(snapshot.program.status as usize, Ordering::Relaxed);
        self.sequence.fetch_add(1, Ordering::Release);
    }
    pub fn snapshot(&self) -> OutputSnapshot {
        loop {
            let before = self.sequence.load(Ordering::Acquire);
            if before & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let flags = self.flags.load(Ordering::Relaxed);
            let s = OutputSnapshot {
                device_frames: self.frames.load(Ordering::Relaxed),
                processing_blocks: self.blocks.load(Ordering::Relaxed),
                cursor: SongCursor {
                    bar_idx: self.bar.load(Ordering::Relaxed),
                    sample_in_bar: self.in_bar.load(Ordering::Relaxed),
                },
                playing: flags & 1 != 0,
                recording: flags & 2 != 0,
                capture_armed: flags & 4 != 0,
                capture_invalid: flags & 8 != 0,
                click_playing: flags & 16 != 0,
                preview_voices: self.previews.load(Ordering::Relaxed),
                preview_starts: self.preview_starts.load(Ordering::Relaxed),
                click_step: self.click_step.load(Ordering::Relaxed),
                program: ProgramSnapshot {
                    epoch: ProgramEpoch(self.program_epoch.load(Ordering::Relaxed)),
                    device_frame: self.frames.load(Ordering::Relaxed),
                    card_index: self.program_card.load(Ordering::Relaxed),
                    card_token: match self.program_token.load(Ordering::Relaxed) {
                        u64::MAX => None,
                        token => Some(token),
                    },
                    event_index: self.program_event.load(Ordering::Relaxed),
                    event_token: match self.program_event_token.load(Ordering::Relaxed) {
                        u64::MAX => None,
                        token => Some(token),
                    },
                    beat_ticks: self.program_ticks.load(Ordering::Relaxed),
                    bpm_bits: self.program_bpm_bits.load(Ordering::Relaxed),
                    click_subdivision_ticks: self
                        .program_click_subdivision_ticks
                        .load(Ordering::Relaxed),
                    card_frames: self.program_frames.load(Ordering::Relaxed),
                    active_frames: self.program_active_frames.load(Ordering::Relaxed),
                    status: match self.program_status.load(Ordering::Relaxed) {
                        1 => ProgramStatus::Playing,
                        2 => ProgramStatus::Paused,
                        3 => ProgramStatus::WaitingManual,
                        4 => ProgramStatus::Refused,
                        5 => ProgramStatus::Complete,
                        6 => ProgramStatus::WaitingObserver,
                        _ => ProgramStatus::Stopped,
                    },
                },
            };
            std::sync::atomic::fence(Ordering::Acquire);
            if before == self.sequence.load(Ordering::Relaxed) {
                return s;
            }
        }
    }
}

pub(super) struct PreparedBar {
    duration: u64,
    beat: u64,
    measure: u64,
    lead: u64,
    beats: u64,
    chord: Option<ArcGc<SampleBuffer>>,
    phrase_pcm: Option<SampleBuffer>,
    loop_pcm: Option<SampleBuffer>,
}
pub(super) struct ChordCache {
    pitches: Vec<u32>,
    duration: u32,
    rate: u32,
    pub(super) pcm: ArcGc<SampleBuffer>,
}
pub(super) struct PreparedBacking {
    bars: Vec<PreparedBar>,
    one_shot: bool,
    click: bool,
    replace: bool,
}
impl PreparedBacking {
    pub fn prepare(
        song: &Song,
        rate: u32,
        cache: &mut Vec<ChordCache>,
    ) -> Result<Self, OutputError> {
        if song.bars.is_empty() || song.bars.len() > MAX_BARS {
            return Err(err("backing needs 1..=256 segments"));
        }
        let mut resource_frames = 0usize;
        let mut render_work = 0u64;
        for bar in &song.bars {
            if !bar.bpm.is_finite() || bar.bpm <= 0.0 {
                return Err(err("backing tempo must be finite and positive"));
            }
            let duration = usize::try_from(bar.checked_duration_frames(rate).map_err(err)?)
                .map_err(|_| err("backing duration is too large"))?;
            if bar.chord_ref.is_some() && bar.backing_phrase.is_some() {
                return Err(err("backing chord and phrase are mutually exclusive"));
            }
            if let Some(phrase) = &bar.backing_phrase {
                let beats = u64::from(bar.time_signature.numerator.max(1))
                    .saturating_mul(u64::from(bar.length.max(1)));
                let layout = crate::backing_phrase::preflight(
                    phrase,
                    duration,
                    bar.beat_frames(rate),
                    beats,
                    rate,
                )
                .map_err(err)?;
                render_work = render_work
                    .checked_add(layout.work_frames)
                    .ok_or_else(|| err("aggregate phrase render work overflow"))?;
                if render_work > crate::backing_phrase::MAX_RENDER_WORK {
                    return Err(err("aggregate backing phrase exceeds render-work bound"));
                }
            }
            let chord_len = if bar.chord_ref.is_some() {
                bar.chord_measure_secs().max(0.5).clamp(0.3, 2.0) * rate as f32
            } else {
                0.0
            };
            resource_frames = resource_frames
                .checked_add(duration)
                .and_then(|n| n.checked_add(chord_len.ceil() as usize))
                .and_then(|n| {
                    n.checked_add(if bar.backing_phrase.is_some() {
                        duration
                    } else {
                        0
                    })
                })
                .ok_or_else(|| err("backing PCM exceeds resource bound"))?;
            if resource_frames > MAX_PCM_FRAMES {
                return Err(err("aggregate backing PCM exceeds 16777216 frames"));
            }
        }
        let mut next_cache = Vec::new();
        let mut total = 0usize;
        let mut bars = Vec::with_capacity(song.bars.len());
        for bar in &song.bars {
            if !bar.bpm.is_finite() || bar.bpm <= 0.0 {
                return Err(err("backing tempo must be finite and positive"));
            }
            let duration = bar.checked_duration_frames(rate).map_err(err)?;
            let beats = u64::from(bar.time_signature.numerator.max(1));
            let phrase_beats = beats.saturating_mul(u64::from(bar.length.max(1)));
            let beat = bar.beat_frames(rate);
            let measure = bar.measure_frames(rate);
            total = total
                .checked_add(
                    usize::try_from(duration).map_err(|_| err("backing duration is too large"))?,
                )
                .ok_or_else(|| err("backing duration is too large"))?;
            if total > MAX_PCM_FRAMES {
                return Err(err("backing exceeds 16777216 prepared frames"));
            }
            if let Some(pcm) = &bar.audio_buffer {
                if pcm.sample_rate_hz != rate {
                    return Err(err("backing PCM rate differs from output rate"));
                }
                if pcm.len() != duration as usize {
                    return Err(err("backing PCM must fit its segment duration"));
                }
            }
            let chord = if let Some(chord) = &bar.chord_ref {
                if chord.pitches_hz.len() > 256 {
                    return Err(err("backing chord exceeds 256 pitches"));
                }
                let pitches = if chord.pitches_hz.is_empty() {
                    vec![chord.root_freq_hz]
                } else {
                    chord.pitches_hz.clone()
                };
                if pitches.iter().any(|f| !f.is_finite() || *f <= 0.0) {
                    return Err(err("backing chord pitches must be finite and positive"));
                }
                // The existing backing strikes once per measure, even for long blocks.
                let duration_seconds = bar.chord_measure_secs();
                let duration_seconds = duration_seconds.max(0.5).clamp(0.3, 2.0);
                let bits: Vec<_> = pitches.iter().map(|f| f.to_bits()).collect();
                let pcm = if let Some(hit) = cache.iter().find(|c| {
                    c.pitches == bits && c.duration == duration_seconds.to_bits() && c.rate == rate
                }) {
                    hit.pcm.clone()
                } else {
                    ArcGc::new(render_chord(
                        &ChordRender::strum(pitches, duration_seconds),
                        rate,
                    ))
                };
                next_cache.push(ChordCache {
                    pitches: bits,
                    duration: duration_seconds.to_bits(),
                    rate,
                    pcm: pcm.clone(),
                });
                Some(pcm)
            } else {
                None
            };
            let phrase_pcm = if let Some(phrase) = &bar.backing_phrase {
                Some(
                    crate::backing_phrase::render(
                        phrase,
                        duration as usize,
                        beat,
                        phrase_beats,
                        rate,
                    )
                    .map_err(err)?,
                )
            } else {
                None
            };
            bars.push(PreparedBar {
                duration,
                beat: beat.max(1),
                measure,
                lead: bar.count_in_frames(rate),
                beats,
                chord,
                phrase_pcm,
                loop_pcm: bar.audio_buffer.clone(),
            });
        }
        *cache = next_cache;
        Ok(Self {
            bars,
            one_shot: song.one_shot,
            click: song.click_enabled,
            replace: song.record_replace,
        })
    }
}

pub(super) struct PreparedClick {
    steps: Vec<Option<ArcGc<SampleBuffer>>>,
    divisions: u32,
    beat_unit: u8,
}
impl PreparedClick {
    pub fn prepare(pattern: &SequencerPattern, rate: u32) -> Result<Self, OutputError> {
        if !pattern.bpm.is_finite()
            || pattern.bpm <= 0.0
            || pattern.steps_per_bar() == 0
            || pattern.steps_per_bar() > 1024
            || pattern.tracks.len() > 64
        {
            return Err(err("click pattern exceeds supported bounds"));
        }
        let mut lengths = Vec::with_capacity(pattern.steps_per_bar());
        let mut total = 0usize;
        for idx in 0..pattern.steps_per_bar() {
            let mut longest = 0usize;
            for track in &pattern.tracks {
                if track.muted || !matches!(track.steps.get(idx), Some(crate::Step::Active { .. }))
                {
                    continue;
                }
                let length = match &track.sound {
                    Sound::Click {
                        duration_seconds, ..
                    } => {
                        if !duration_seconds.is_finite() || *duration_seconds < 0.0 {
                            return Err(err("invalid click duration"));
                        }
                        (*duration_seconds * rate as f32).ceil() as usize
                    },
                    Sound::Sample {
                        buffer,
                        pitch_semitones,
                        ..
                    } => {
                        if !pitch_semitones.is_finite() {
                            return Err(err("invalid sample pitch"));
                        }
                        (buffer.len() as f64 * rate as f64
                            / buffer.sample_rate_hz.max(1) as f64
                            / 2f64.powf(*pitch_semitones as f64 / 12.0))
                        .ceil() as usize
                    },
                }
                .checked_add(1)
                .ok_or_else(|| err("click PCM exceeds resource bound"))?;
                longest = longest.max(length);
            }
            total = total
                .checked_add(longest)
                .ok_or_else(|| err("click PCM exceeds resource bound"))?;
            if total > MAX_PCM_FRAMES {
                return Err(err("aggregate click PCM exceeds 16777216 frames"));
            }
            lengths.push(longest);
        }
        let mut steps = Vec::with_capacity(pattern.steps_per_bar());
        for (idx, &length) in lengths.iter().enumerate() {
            let mut data: Vec<f32> = vec![0.0; length];
            for track in &pattern.tracks {
                if track.muted {
                    continue;
                }
                let Some(step) = track.steps.get(idx) else {
                    continue;
                };
                let crate::Step::Active { accent } = step else {
                    continue;
                };
                let limit = match &track.sound {
                    Sound::Click {
                        duration_seconds, ..
                    } => (*duration_seconds * rate as f32).ceil() as usize + 1,
                    Sound::Sample {
                        buffer,
                        pitch_semitones,
                        ..
                    } => {
                        (buffer.len() as f64 * rate as f64
                            / buffer.sample_rate_hz.max(1) as f64
                            / 2f64.powf(*pitch_semitones as f64 / 12.0))
                        .ceil() as usize
                            + 1
                    },
                };
                if limit > MAX_PCM_FRAMES
                    || !track
                        .sound
                        .render_sample(0, rate as f32, *accent)
                        .unwrap_or(0.0)
                        .is_finite()
                {
                    return Err(err("click voice exceeds supported resource bounds"));
                }
                for (local, sample) in data.iter_mut().enumerate().take(limit) {
                    if let Some(value) =
                        track
                            .sound
                            .render_sample(local as u32, rate as f32, *accent)
                    {
                        *sample += value;
                    }
                }
            }
            steps.push((!data.is_empty()).then(|| ArcGc::new(SampleBuffer::new(data, rate))));
        }
        Ok(Self {
            steps,
            divisions: pattern.subdivision.divisions_per_beat.max(1) as u32,
            beat_unit: pattern.time_signature.denominator,
        })
    }
}

pub(super) struct RecordBar {
    pub pcm: SampleBuffer,
    pub touched: bool,
    retired: Option<SampleBuffer>,
}
pub(super) struct CompletionStamp {
    pub generation: u64,
    pub valid: bool,
}
pub(super) type CompletedRecording = CompletedTake<OwnedGc<Recording>, CompletionStamp>;

/// Recording ownership is independent of how the initial arm reaches a bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecordingScope {
    Follow,
    Selected(usize),
}
impl RecordingScope {
    pub fn includes(self, bar: usize) -> bool {
        match self {
            Self::Follow => true,
            Self::Selected(target) => bar == target,
        }
    }
    fn selected(self) -> Option<usize> {
        match self {
            Self::Follow => None,
            Self::Selected(target) => Some(target),
        }
    }
}

pub(super) struct Recording {
    pub scope: RecordingScope,
    pub generation: u64,
    pub bars: Vec<RecordBar>,
    pub valid: bool,
    overflow: u64,
}
impl Recording {
    pub fn prepare(song: &Song, rate: u32, generation: u64) -> Result<Self, OutputError> {
        let mut bars = Vec::with_capacity(song.bars.len());
        let mut total = 0usize;
        for bar in &song.bars {
            let len = usize::try_from(bar.checked_duration_frames(rate).map_err(err)?)
                .map_err(|_| err("capture duration is too large"))?;
            total = total
                .checked_add(len)
                .ok_or_else(|| err("capture storage exceeds bound"))?;
            if total > MAX_PCM_FRAMES {
                return Err(err("capture storage exceeds 16777216 frames"));
            }
            let data = if let Some(pcm) = &bar.audio_buffer {
                if pcm.sample_rate_hz != rate || pcm.len() != len {
                    return Err(err("captured backing clock differs from current segment"));
                }
                pcm.data.as_ref().clone()
            } else {
                vec![0.0; len]
            };
            bars.push(RecordBar {
                pcm: SampleBuffer::new(data, rate),
                touched: false,
                retired: None,
            });
        }
        Ok(Self {
            scope: RecordingScope::Follow,
            generation,
            bars,
            valid: true,
            overflow: 0,
        })
    }
}

pub(super) enum Command {
    Backing {
        data: Option<OwnedGc<PreparedBacking>>,
        reset: bool,
        clear: bool,
        playing: bool,
        generation: u64,
    },
    ClickPattern {
        data: Option<OwnedGc<PreparedClick>>,
        bpm: f32,
        playing: Option<bool>,
    },
    Metronome {
        bpm: f32,
        playing: bool,
    },
    ClickPlaying(bool),
    Playing(bool),
    Rewind,
    Gain(f32),
    RecordReplace(bool),
    Preview {
        revision: u64,
        buffer: Option<ArcGc<SampleBuffer>>,
    },
    CancelPreview,
    Capture(Option<OwnedGc<CaptureFeedConsumer>>),
    Arm {
        bar_idx: usize,
        seek: bool,
        take: Option<OwnedGc<Recording>>,
    },
    Boundary(PendingChange),
    StopCapture,
    ProgramStart {
        data: Option<OwnedGc<PreparedProgram>>,
        epoch: ProgramEpoch,
    },
    ProgramPause {
        epoch: ProgramEpoch,
        paused: bool,
    },
    ProgramTempo {
        epoch: ProgramEpoch,
        card: usize,
        bpm: f64,
    },
    ProgramLoopMode {
        epoch: ProgramEpoch,
        mode: ProgramLoop,
    },
    ProgramAdvance {
        epoch: ProgramEpoch,
    },
    ProgramSeek {
        epoch: ProgramEpoch,
        card: usize,
    },
    ProgramStop {
        epoch: ProgramEpoch,
    },
}
enum Boundary {
    Change(PendingChange),
    Arm {
        bar: usize,
        seek: bool,
        take: OwnedGc<Recording>,
    },
}
struct Voice {
    pcm: ArcGc<SampleBuffer>,
    position: usize,
}
fn mix(voices: &mut [Option<Voice>]) -> f32 {
    let mut result = 0.0;
    for voice in voices {
        if let Some(v) = voice {
            if let Some(sample) = v.pcm.data.get(v.position) {
                result += *sample;
                v.position += 1;
            } else {
                *voice = None;
            }
        }
    }
    result
}
fn trigger(voices: &mut [Option<Voice>], pcm: ArcGc<SampleBuffer>) {
    if let Some(slot) = voices.iter_mut().find(|v| v.is_none()) {
        *slot = Some(Voice { pcm, position: 0 });
    } else {
        // Replace the oldest envelope; voice bounds never grow.
        let idx = voices
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| v.as_ref().map_or(0, |v| v.position))
            .unwrap()
            .0;
        voices[idx] = Some(Voice { pcm, position: 0 });
    }
}

pub(super) struct OutputNode {
    shared: Arc<Shared>,
    returns: RefCell<Option<rtrb::Producer<CompletedRecording>>>,
    program_transitions: RefCell<Option<rtrb::Producer<ProgramTransition>>>,
}
impl OutputNode {
    pub fn new(
        shared: Arc<Shared>,
        returns: rtrb::Producer<CompletedRecording>,
        program_transitions: rtrb::Producer<ProgramTransition>,
    ) -> Self {
        Self {
            shared,
            returns: RefCell::new(Some(returns)),
            program_transitions: RefCell::new(Some(program_transitions)),
        }
    }
}
impl AudioNode for OutputNode {
    type Configuration = EmptyConfig;
    fn info(&self, _: &EmptyConfig) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("woodshed_output")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::MONO,
            }))
    }
    fn construct_processor(
        &self,
        _: &EmptyConfig,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        Ok(Processor {
            shared: self.shared.clone(),
            returns: self
                .returns
                .borrow_mut()
                .take()
                .expect("output processor constructed once"),
            program_transitions: self
                .program_transitions
                .borrow_mut()
                .take()
                .expect("program transition processor constructed once"),
            backing: None,
            click: None,
            feed: None,
            take: None,
            completed: None,
            generation: 0,
            cursor: Default::default(),
            playing: false,
            count_in: 0,
            count_in_pos: 0,
            backing_click: default_click(cx.stream_info.sample_rate.get()),
            gain: 0.8,
            replace: false,
            bpm: 120.0,
            click_playing: false,
            click_position: 0.0,
            last_click: None,
            song_last_beat: None,
            song_last_measure: None,
            click_voices: std::array::from_fn(|_| None),
            song_voices: std::array::from_fn(|_| None),
            preview_voices: std::array::from_fn(|_| None),
            preview_starts: 0,
            pending: std::array::from_fn(|_| None),
            pending_len: 0,
            rate: cx.stream_info.sample_rate.get(),
            blocks: 0,
            handled_commands: 0,
            invalid: false,
            program: None,
            program_status: ProgramStatus::Stopped,
            program_epoch: ProgramEpoch::default(),
            program_card_index: 0,
            program_event_index: 0,
            program_tick_phase_q32: 0,
            program_card_frames: 0,
            program_tail_remaining: 0,
            program_advance: false,
            program_transition_pending: None,
            program_stop_pending: false,
            program_resume_after_observer: ProgramStatus::Playing,
            program_active_frames: 0,
            program_click_last_beat: None,
            program_voices: std::array::from_fn(|_| None),
            program_event_token: None,
            program_card_started: false,
            program_pending_action: ProgramPendingAction::None,
            program_deferred_controls: std::array::from_fn(|_| None),
            program_deferred_len: 0,
            current_frame: 0,
            program_tail_started: false,
            program_click_last_step: None,
            program_tick_step_q32: 0,
            program_event_due_frame: None,
            program_event_group_base: None,
            program_manual_body_done: false,
            program_resume_after_pause: ProgramStatus::Playing,
            program_loop_mode: ProgramLoop::Once,
        })
    }
}
struct Processor {
    shared: Arc<Shared>,
    returns: rtrb::Producer<CompletedRecording>,
    program_transitions: rtrb::Producer<ProgramTransition>,
    backing: Option<OwnedGc<PreparedBacking>>,
    click: Option<OwnedGc<PreparedClick>>,
    feed: Option<OwnedGc<CaptureFeedConsumer>>,
    take: Option<OwnedGc<Recording>>,
    completed: Option<CompletedRecording>,
    generation: u64,
    cursor: SongCursor,
    playing: bool,
    count_in: u64,
    count_in_pos: u64,
    backing_click: [ArcGc<SampleBuffer>; 2],
    gain: f32,
    replace: bool,
    bpm: f32,
    click_playing: bool,
    // Integer frame numerator (increments by subdivisions); divide by the
    // rounded beat length only when observing a step. Avoid per-frame
    // fractional addition, which can put a click one sample after a boundary.
    click_position: f64,
    last_click: Option<u64>,
    song_last_beat: Option<u64>,
    song_last_measure: Option<u64>,
    click_voices: [Option<Voice>; 8],
    song_voices: [Option<Voice>; 8],
    preview_voices: [Option<Voice>; 16],
    preview_starts: u64,
    pending: [Option<Boundary>; COMMAND_BOUND],
    pending_len: usize,
    rate: u32,
    blocks: u64,
    handled_commands: usize,
    invalid: bool,
    program: Option<OwnedGc<PreparedProgram>>,
    program_status: ProgramStatus,
    program_epoch: ProgramEpoch,
    program_card_index: usize,
    program_event_index: usize,
    program_tick_phase_q32: u128,
    program_card_frames: u64,
    program_tail_remaining: u64,
    program_advance: bool,
    program_transition_pending: Option<ProgramTransition>,
    program_stop_pending: bool,
    program_resume_after_observer: ProgramStatus,
    program_active_frames: u64,
    program_click_last_beat: Option<u64>,
    program_voices: [Option<ProgramVoice>; MAX_PROGRAM_VOICES],
    program_event_token: Option<u64>,
    program_card_started: bool,
    program_pending_action: ProgramPendingAction,
    program_deferred_controls: [Option<DeferredProgramControl>; COMMAND_BOUND],
    program_deferred_len: usize,
    current_frame: u64,
    program_tail_started: bool,
    program_click_last_step: Option<u64>,
    program_tick_step_q32: u64,
    program_event_due_frame: Option<(usize, u64)>,
    program_event_group_base: Option<(ProgramPosition, u64)>,
    program_manual_body_done: bool,
    program_resume_after_pause: ProgramStatus,
    program_loop_mode: ProgramLoop,
}

struct ProgramVoice {
    tone: ArcGc<PreparedTone>,
    position: usize,
    age: u64,
    gate: ProgramDuration,
    gate_end: u64,
    amplitude: f32,
    released: bool,
    release_age: u64,
    card_index: usize,
}

#[derive(Clone, Copy)]
enum ProgramPendingAction {
    None,
    CardStarted,
    NextCard,
    AdvanceCard,
    Seek(usize),
    Stop,
}
#[derive(Clone, Copy)]
enum DeferredProgramControl {
    Seek(usize),
    Stop,
}
impl Processor {
    fn clear_voices(&mut self) {
        for v in &mut self.song_voices {
            *v = None;
        }
        for v in &mut self.preview_voices {
            *v = None;
        }
    }
    fn start_playing(&mut self, playing: bool) {
        self.playing = playing;
        self.clear_voices();
        self.song_last_beat = None;
        self.song_last_measure = None;
        self.count_in_pos = 0;
        self.count_in = if playing {
            self.backing
                .as_ref()
                .filter(|b| b.click)
                .map_or(0, |b| b.bars[self.cursor.bar_idx].lead)
        } else {
            0
        };
    }
    fn finish_take(&mut self) {
        if let Some(mut take) = self.take.take() {
            if self
                .feed
                .as_ref()
                .is_some_and(|f| f.overflow_count() != take.overflow)
            {
                take.valid = false;
                self.invalid = true;
            }
            if take.valid && take.generation == self.generation {
                if let Some(backing) = self.backing.as_mut() {
                    let scope = take.scope;
                    for (idx, bar) in take.bars.iter_mut().enumerate() {
                        if bar.touched && scope.includes(idx) {
                            bar.retired = backing.bars[idx].loop_pcm.replace(bar.pcm.clone());
                        }
                    }
                }
            }
            let stamp = CompletionStamp {
                generation: take.generation,
                valid: take.valid,
            };
            self.completed = Some(CompletedTake {
                storage: take,
                stamp,
            });
        }
    }
    fn flush_return(&mut self) {
        if let Some(take) = self.completed.take() {
            if let Err(rtrb::PushError::Full(take)) = self.returns.push(take) {
                self.completed = Some(take);
            }
        }
    }
    fn stop_capture(&mut self) {
        self.finish_take();
        for op in &mut self.pending {
            if matches!(op, Some(Boundary::Arm { .. })) {
                *op = None;
                self.shared.boundaries.fetch_sub(1, Ordering::Release);
            }
        }
        let mut write = 0;
        for read in 0..self.pending_len {
            if self.pending[read].is_some() {
                self.pending.swap(read, write);
                write += 1;
            }
        }
        self.pending_len = write;
    }
    fn discard_unreachable_arms(&mut self) {
        let mut write = 0;
        for read in 0..self.pending_len {
            match self.pending[read].take() {
                Some(Boundary::Arm { .. }) => {
                    self.invalid = true;
                    self.shared.boundaries.fetch_sub(1, Ordering::Release);
                },
                Some(op) => {
                    self.pending[write] = Some(op);
                    write += 1;
                },
                None => {},
            }
        }
        self.pending_len = write;
    }
    fn push_boundary(&mut self, op: Boundary) {
        if self.pending_len < COMMAND_BOUND {
            self.pending[self.pending_len] = Some(op);
            self.pending_len += 1;
        } else {
            self.invalid = true;
            self.shared.boundaries.fetch_sub(1, Ordering::Release);
        } // Retired resources use deferred GC.
    }
    /// Includes queued selected arms so later controls cannot redirect an
    /// accepted reservation while it waits for its first boundary/count-in.
    fn selected_capture_bar(&self) -> Option<usize> {
        self.take
            .as_ref()
            .and_then(|take| take.scope.selected())
            .or_else(|| {
                self.pending[..self.pending_len]
                    .iter()
                    .find_map(|op| match op {
                        Some(Boundary::Arm { take, .. }) => take.scope.selected(),
                        _ => None,
                    })
            })
    }

    fn boundary(&mut self) {
        self.song_last_beat = None;
        self.song_last_measure = None;
        let pending_len = self.pending_len;
        let mut retained = 0;
        for idx in 0..pending_len {
            if let Some(op) = self.pending[idx].take() {
                if matches!(&op, Boundary::Arm { seek: false, bar, .. } if self.cursor.bar_idx != *bar)
                {
                    self.pending[retained] = Some(op);
                    retained += 1;
                    continue;
                }
                self.shared.boundaries.fetch_sub(1, Ordering::Release);
                match op {
                    Boundary::Change(PendingChange::SeekTo { bar_idx }) => {
                        if self.selected_capture_bar().is_none()
                            && self
                                .backing
                                .as_ref()
                                .is_some_and(|b| bar_idx < b.bars.len())
                        {
                            self.cursor = SongCursor {
                                bar_idx,
                                sample_in_bar: 0,
                            };
                        }
                    },
                    Boundary::Change(PendingChange::StopRecording) => {
                        self.finish_take();
                        // A boundary stop cancels earlier queued arms as well
                        // as an active take. Later arms retain FIFO semantics.
                        for slot in &mut self.pending[..retained] {
                            if matches!(slot, Some(Boundary::Arm { .. })) {
                                *slot = None;
                                retained -= 1;
                                self.shared.boundaries.fetch_sub(1, Ordering::Release);
                            }
                        }
                    },
                    Boundary::Change(PendingChange::SetPlaying(playing)) => {
                        self.playing = playing;
                        if !playing {
                            self.clear_voices();
                            self.finish_take();
                        }
                    },
                    Boundary::Change(PendingChange::StartRecording { .. }) => {},
                    Boundary::Arm {
                        bar,
                        seek,
                        mut take,
                    } => {
                        if seek {
                            self.cursor = SongCursor {
                                bar_idx: bar,
                                sample_in_bar: 0,
                            };
                        }
                        if self.cursor.bar_idx == bar && self.completed.is_none() {
                            if let Some(feed) = self.feed.as_mut() {
                                take.overflow = feed.overflow_count();
                                self.invalid = false;
                                self.take = Some(take);
                            } else {
                                self.invalid = true;
                            }
                        }
                    },
                }
            }
        }
        self.pending_len = retained;
    }
    fn click_trigger(&mut self, accent: bool, song: bool) {
        // Backing always uses the same pre-rendered quarter click as the
        // root metronome's default; calibration patterns affect only its lane.
        let pcm = self.backing_click[usize::from(!accent)].clone();
        if song {
            trigger(&mut self.song_voices, pcm);
        } else {
            trigger(&mut self.click_voices, pcm);
        }
    }
    fn program_transition(
        &mut self,
        kind: ProgramTransitionKind,
        action: ProgramPendingAction,
    ) -> bool {
        let Some(card) = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index))
        else {
            return false;
        };
        let transition = ProgramTransition {
            epoch: self.program_epoch,
            card_token: card.token,
            device_frame: self.current_frame,
            active_frames: self.program_active_frames,
            kind,
        };
        match self.program_transitions.push(transition) {
            Ok(()) => {
                self.apply_program_action(action);
                true
            },
            Err(rtrb::PushError::Full(transition)) => {
                self.program_transition_pending = Some(transition);
                self.program_pending_action = action;
                self.program_resume_after_observer = self.program_status;
                self.program_status = ProgramStatus::WaitingObserver;
                false
            },
        }
    }

    fn apply_program_action(&mut self, action: ProgramPendingAction) {
        match action {
            ProgramPendingAction::None => {},
            ProgramPendingAction::CardStarted => {
                self.program_card_started = true;
                self.program_status = ProgramStatus::Playing;
            },
            ProgramPendingAction::NextCard => self.move_program_card(None),
            ProgramPendingAction::AdvanceCard => self.advance_program_card(),
            ProgramPendingAction::Seek(card) => self.move_program_card(Some(card)),
            ProgramPendingAction::Stop => {
                for voice in &mut self.program_voices {
                    *voice = None;
                }
                self.program = None;
                self.program_status = ProgramStatus::Stopped;
                self.program_card_started = false;
                self.program_event_token = None;
                self.program_deferred_controls = std::array::from_fn(|_| None);
                self.program_deferred_len = 0;
                self.program_stop_pending = false;
            },
        }
    }

    fn apply_program_control(&mut self, control: DeferredProgramControl) {
        let was_paused = self.program_status == ProgramStatus::Paused;
        match control {
            DeferredProgramControl::Seek(card) if self.program_card_started => {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Seek),
                    ProgramPendingAction::Seek(card),
                );
            },
            DeferredProgramControl::Seek(card) => self.move_program_card(Some(card)),
            DeferredProgramControl::Stop if self.program_card_started => {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Stop),
                    ProgramPendingAction::Stop,
                );
            },
            DeferredProgramControl::Stop => self.apply_program_action(ProgramPendingAction::Stop),
        }
        if was_paused
            && !matches!(
                self.program_status,
                ProgramStatus::Stopped | ProgramStatus::Complete | ProgramStatus::WaitingObserver
            )
        {
            self.program_resume_after_pause = self.program_status;
            self.program_status = ProgramStatus::Paused;
        }
    }

    fn defer_program_control(&mut self, control: DeferredProgramControl) {
        if self.program_transition_pending.is_some() {
            match control {
                // Stop is a dedicated bounded pending bit and always wins
                // over queued seeks; observer pressure cannot lose it.
                DeferredProgramControl::Stop => self.program_stop_pending = true,
                DeferredProgramControl::Seek(card) => {
                    if self.program_deferred_len < self.program_deferred_controls.len() {
                        self.program_deferred_controls[self.program_deferred_len] =
                            Some(DeferredProgramControl::Seek(card));
                        self.program_deferred_len += 1;
                    } else if self.program_deferred_len > 0 {
                        // Under pressure, accepted seeks coalesce to the
                        // newest destination. Their order is not meaningful;
                        // Stop remains lossless and takes precedence.
                        self.program_deferred_controls[self.program_deferred_len - 1] =
                            Some(DeferredProgramControl::Seek(card));
                    }
                },
            }
            return;
        }
        self.apply_program_control(control);
    }

    fn dispatch_deferred_program_control(&mut self) {
        if self.program_transition_pending.is_some() {
            return;
        }
        if self.program_stop_pending {
            self.program_stop_pending = false;
            self.program_deferred_controls = std::array::from_fn(|_| None);
            self.program_deferred_len = 0;
            if self.program.is_some() {
                self.apply_program_control(DeferredProgramControl::Stop);
            }
            return;
        }
        if self.program_deferred_len == 0 {
            return;
        }
        let control = self.program_deferred_controls[0]
            .take()
            .expect("deferred program control slot is occupied");
        for index in 1..self.program_deferred_len {
            self.program_deferred_controls[index - 1] =
                self.program_deferred_controls[index].take();
        }
        self.program_deferred_len -= 1;
        self.apply_program_control(control);
    }

    fn flush_program_transition(&mut self) -> bool {
        let Some(transition) = self.program_transition_pending.take() else {
            return true;
        };
        match self.program_transitions.push(transition) {
            Ok(()) => {
                let action =
                    std::mem::replace(&mut self.program_pending_action, ProgramPendingAction::None);
                let was_paused = self.program_resume_after_observer == ProgramStatus::Paused;
                self.program_status = if was_paused {
                    self.program_resume_after_pause
                } else {
                    self.program_resume_after_observer
                };
                self.apply_program_action(action);
                if was_paused
                    && !matches!(
                        self.program_status,
                        ProgramStatus::Stopped | ProgramStatus::Complete
                    )
                {
                    self.program_resume_after_pause = self.program_status;
                    self.program_status = ProgramStatus::Paused;
                }
                self.dispatch_deferred_program_control();
                true
            },
            Err(rtrb::PushError::Full(transition)) => {
                self.program_transition_pending = Some(transition);
                self.program_status = ProgramStatus::WaitingObserver;
                false
            },
        }
    }

    /// Explicit Next leaves a focused Card repeat. Automatic completion alone
    /// consults that repeat mode; Set-wide looping still wraps at the boundary.
    fn advance_program_card(&mut self) {
        if self.program_loop_mode == ProgramLoop::Card {
            self.move_program_card(Some(self.program_card_index.saturating_add(1)));
        } else {
            self.move_program_card(None);
        }
    }

    fn move_program_card(&mut self, requested: Option<usize>) {
        let Some(program) = self.program.as_ref() else {
            return;
        };
        let next = if let Some(card) = requested {
            Some(card)
        } else {
            match self.program_loop_mode {
                ProgramLoop::Card => Some(self.program_card_index),
                ProgramLoop::Set if self.program_card_index + 1 >= program.cards.len() => Some(0),
                ProgramLoop::Set => Some(self.program_card_index + 1),
                ProgramLoop::Once if self.program_card_index + 1 < program.cards.len() => {
                    Some(self.program_card_index + 1)
                },
                ProgramLoop::Once => None,
            }
        };
        match next {
            Some(index) if index < program.cards.len() => {
                self.program_card_index = index;
                self.program_event_index = 0;
                self.program_tick_phase_q32 = 0;
                self.program_card_frames = 0;
                self.program_tail_remaining = 0;
                self.program_tail_started = false;
                self.program_active_frames = 0;
                self.program_card_started = false;
                self.program_click_last_beat = None;
                self.program_click_last_step = None;
                self.program_event_due_frame = None;
                self.program_event_group_base = None;
                self.program_event_token = None;
                self.program_manual_body_done = false;
                self.program_status = ProgramStatus::Playing;
            },
            _ => {
                self.program_status = ProgramStatus::Complete;
                self.program_card_started = false;
            },
        }
    }

    fn enter_program_card(&mut self) {
        let Some(program) = self.program.as_ref() else {
            return;
        };
        let Some(card) = program.cards.get(self.program_card_index) else {
            self.program_status = ProgramStatus::Complete;
            return;
        };
        match card.end {
            ProgramEnd::Refusal(code) => {
                self.program_status = ProgramStatus::Refused;
                let _ = self.program_transition(
                    ProgramTransitionKind::Refused(code),
                    ProgramPendingAction::None,
                );
            },
            _ => {
                let bpm = card.bpm;
                self.program_tick_step_q32 =
                    (bpm * super::clock::PROGRAM_TICKS_PER_BEAT as f64 * (u32::MAX as f64 + 1.0)
                        / (60.0 * self.rate as f64))
                        .round() as u64;
                self.program_status = ProgramStatus::Playing;
                let _ = self.program_transition(
                    ProgramTransitionKind::Started,
                    ProgramPendingAction::CardStarted,
                );
            },
        }
    }

    fn trigger_program_events(&mut self) {
        let Some(program) = self.program.as_ref() else {
            return;
        };
        let Some(card) = program.cards.get(self.program_card_index) else {
            return;
        };
        while let Some(event) = card.events.get(self.program_event_index) {
            let onset_reached = match event.onset {
                ProgramPosition::BeatTicks(ticks) => {
                    (self.program_tick_phase_q32 >> 32) as u64 >= ticks
                },
                ProgramPosition::DeviceFrames(frames) => self.program_card_frames >= frames,
            };
            if !onset_reached {
                break;
            }
            let group_base = match self.program_event_group_base {
                Some((onset, base)) if onset == event.onset => base,
                _ => {
                    let base = self.program_card_frames;
                    self.program_event_group_base = Some((event.onset, base));
                    base
                },
            };
            let due_frame = group_base.saturating_add(event.onset_offset_frames);
            self.program_event_due_frame = Some((self.program_event_index, due_frame));
            if self.program_card_frames < due_frame {
                break;
            }
            let tone = program.tones[event.tone].clone();
            let gate_end = match (event.onset, event.gate) {
                (ProgramPosition::BeatTicks(at), ProgramDuration::BeatTicks(len)) => {
                    at.saturating_add(len)
                },
                (ProgramPosition::DeviceFrames(at), ProgramDuration::DeviceFrames(len)) => {
                    at.saturating_add(len)
                },
                (_, ProgramDuration::UntilCardEnd) => u64::MAX,
                _ => u64::MAX,
            };
            let slot_index = self
                .program_voices
                .iter()
                .position(Option::is_none)
                .or_else(|| {
                    // A previous card may still be inside its bounded release
                    // tail. Reclaim its oldest released voice before sacrificing
                    // an onset from the current card.
                    self.program_voices
                        .iter()
                        .enumerate()
                        .filter_map(|(index, voice)| {
                            voice
                                .as_ref()
                                .filter(|voice| voice.released)
                                .map(|voice| (index, voice.release_age))
                        })
                        .max_by_key(|(_, age)| *age)
                        .map(|(index, _)| index)
                });
            let Some(slot_index) = slot_index else {
                // Validation bounds same-card overlap. If a malformed or
                // otherwise unexpected schedule exhausts the fixed pool,
                // leave the event pending for a later frame instead of
                // silently consuming it.
                break;
            };
            self.program_voices[slot_index] = Some(ProgramVoice {
                tone,
                position: 0,
                age: 0,
                gate: event.gate,
                gate_end: match event.gate {
                    ProgramDuration::BeatTicks(length) => {
                        ((self.program_tick_phase_q32 >> 32) as u64).saturating_add(length)
                    },
                    ProgramDuration::DeviceFrames(length) => {
                        self.program_card_frames.saturating_add(length)
                    },
                    ProgramDuration::UntilCardEnd => gate_end,
                },
                amplitude: event.amplitude,
                released: false,
                release_age: 0,
                card_index: self.program_card_index,
            });
            self.program_event_token = Some(event.token);
            self.program_event_index += 1;
            self.program_event_due_frame = None;
        }
    }

    fn program_frame_start(&mut self) -> bool {
        if !self.flush_program_transition() {
            return false;
        }
        if self.program_advance {
            self.program_advance = false;
            let was_paused = self.program_status == ProgramStatus::Paused;
            if self.program_card_started {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Completed),
                    ProgramPendingAction::AdvanceCard,
                );
                if self.program_status == ProgramStatus::WaitingObserver {
                    return false;
                }
            } else {
                self.advance_program_card();
            }
            if was_paused
                && !matches!(
                    self.program_status,
                    ProgramStatus::Stopped | ProgramStatus::Complete
                )
            {
                self.program_resume_after_pause = self.program_status;
                self.program_status = ProgramStatus::Paused;
            }
        }
        if self.program_status != ProgramStatus::Playing || self.program.is_none() {
            return false;
        }
        if !self.program_card_started {
            self.enter_program_card();
            if !self.program_card_started {
                return false;
            }
        }
        self.trigger_program_events();
        if self.click_playing {
            if let Some(click) = self.click.as_ref() {
                let ticks_per_step =
                    (super::clock::PROGRAM_TICKS_PER_BEAT / click.divisions.max(1) as u64).max(1);
                let beat_ticks = (self.program_tick_phase_q32 >> 32) as u64;
                let step = beat_ticks / ticks_per_step;
                if self.program_click_last_step != Some(step) {
                    let index = (step % click.steps.len() as u64) as usize;
                    if let Some(pcm) = &click.steps[index] {
                        trigger(&mut self.click_voices, pcm.clone());
                    }
                    self.program_click_last_step = Some(step);
                }
            }
        }
        true
    }

    fn mix_program_voices(&mut self) -> f32 {
        if self.program_status != ProgramStatus::Playing {
            return 0.0;
        }
        let beat_ticks = (self.program_tick_phase_q32 >> 32) as u64;
        let attack = (self.rate as f64 * 0.015).round().max(1.0) as u64;
        let release = (self.rate as f64 * 0.250).round().max(1.0) as u64;
        let current_card = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index));
        let current_card_end = current_card.map(|card| card.end);
        let current_card_bpm = current_card.map_or(120.0, |card| card.bpm);
        let mut sum = 0.0;
        for slot in &mut self.program_voices {
            let Some(voice) = slot else {
                continue;
            };
            let expired = voice.card_index != self.program_card_index
                || match voice.gate {
                    ProgramDuration::BeatTicks(_) => beat_ticks >= voice.gate_end,
                    ProgramDuration::DeviceFrames(_) => self.program_card_frames >= voice.gate_end,
                    ProgramDuration::UntilCardEnd => match current_card_end {
                        Some(ProgramEnd::BeatTicks(end)) => {
                            let release_ticks = (current_card_bpm
                                * 0.250
                                * super::clock::PROGRAM_TICKS_PER_BEAT as f64
                                / 60.0)
                                .ceil() as u64;
                            beat_ticks >= end.saturating_sub(release_ticks)
                        },
                        Some(ProgramEnd::DeviceFrames(end)) => {
                            self.program_card_frames >= end.saturating_sub(release)
                        },
                        Some(ProgramEnd::Manual { tail_frames, .. }) => {
                            self.program_tail_started
                                && self.program_tail_remaining <= release.min(tail_frames)
                        },
                        _ => false,
                    },
                };
            if expired && !voice.released {
                voice.released = true;
                voice.release_age = 0;
            }
            if voice.released && voice.release_age >= release {
                *slot = None;
                continue;
            }
            let sample = voice.tone.pcm.data[voice.position];
            voice.position += 1;
            if voice.position == voice.tone.pcm.len() {
                voice.position = 0;
            }
            let attack_gain = (voice.age as f32 / attack as f32).min(1.0);
            let release_gain = if voice.released {
                (1.0 - voice.release_age as f32 / release as f32).max(0.0)
            } else {
                1.0
            };
            sum += sample * voice.amplitude * attack_gain * release_gain;
            voice.age = voice.age.saturating_add(1);
            if voice.released {
                voice.release_age = voice.release_age.saturating_add(1);
            }
        }
        sum
    }

    fn program_frame_end(&mut self) {
        if self.program_status == ProgramStatus::WaitingManual {
            self.program_active_frames = self.program_active_frames.saturating_add(1);
            return;
        }
        if self.program_status != ProgramStatus::Playing || !self.program_card_started {
            return;
        }
        let Some(end) = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index))
            .map(|card| card.end)
        else {
            return;
        };
        self.program_active_frames = self.program_active_frames.saturating_add(1);
        self.program_card_frames = self.program_card_frames.saturating_add(1);
        self.program_tick_phase_q32 = self
            .program_tick_phase_q32
            .saturating_add(u128::from(self.program_tick_step_q32));
        let completed = match end {
            ProgramEnd::BeatTicks(ticks) => {
                (self.program_tick_phase_q32 >> 32) >= u128::from(ticks)
            },
            ProgramEnd::DeviceFrames(frames) => self.program_card_frames >= frames,
            ProgramEnd::Manual {
                preview_ticks,
                tail_frames,
            } => {
                if !self.program_tail_started
                    && (self.program_tick_phase_q32 >> 32) >= u128::from(preview_ticks)
                {
                    self.program_tail_started = true;
                    self.program_tail_remaining = tail_frames;
                    false
                } else if self.program_tail_started {
                    self.program_tail_remaining = self.program_tail_remaining.saturating_sub(1);
                    self.program_tail_remaining == 0
                } else {
                    false
                }
            },
            ProgramEnd::Refusal(_) => false,
        };
        if completed {
            for voice in self.program_voices.iter_mut().flatten() {
                if !voice.released {
                    voice.released = true;
                    voice.release_age = 0;
                }
            }
            if matches!(end, ProgramEnd::Manual { .. }) {
                for voice in &mut self.program_voices {
                    *voice = None;
                }
                self.program_manual_body_done = true;
                self.program_status = ProgramStatus::WaitingManual;
            } else {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Completed),
                    ProgramPendingAction::NextCard,
                );
            }
        }
    }

    fn program_snapshot(&self) -> ProgramSnapshot {
        let current_card = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index));
        let card_token = current_card.map(|card| card.token);
        let click_subdivision_ticks = self.click.as_ref().map_or(0, |click| {
            (super::clock::PROGRAM_TICKS_PER_BEAT / click.divisions.max(1) as u64).max(1)
        });
        ProgramSnapshot {
            epoch: self.program_epoch,
            device_frame: self.current_frame,
            card_index: self.program_card_index,
            card_token,
            event_index: self.program_event_index,
            event_token: self.program_event_token,
            beat_ticks: (self.program_tick_phase_q32 >> 32).min(u128::from(u64::MAX)) as u64,
            bpm_bits: current_card.map_or(0, |card| card.bpm.to_bits()),
            click_subdivision_ticks,
            card_frames: self.program_card_frames,
            active_frames: self.program_active_frames,
            status: self.program_status,
        }
    }

    fn frame(&mut self) -> f32 {
        let program_running = self.program_frame_start();
        if self.take.is_none() {
            if let Some(feed) = self.feed.as_mut() {
                let _ = feed.pop_sample();
            }
        }
        if self.click_playing && self.program.is_none() {
            if let Some(click) = self.click.as_ref() {
                let beat_frames =
                    audio_primitives::frames_per_beat(self.rate, self.bpm, click.beat_unit).max(1);
                let absolute_step = (self.click_position / beat_frames as f64) as u64;
                let index = (absolute_step % click.steps.len() as u64) as usize;
                if self.last_click != Some(absolute_step) {
                    if let Some(pcm) = &click.steps[index] {
                        trigger(&mut self.click_voices, pcm.clone());
                    }
                    self.last_click = Some(absolute_step);
                }
                self.click_position += click.divisions as f64;
            }
        }
        let metro = if self.program.is_some() && self.program_status != ProgramStatus::Playing {
            0.0
        } else {
            mix(&mut self.click_voices).clamp(-1.0, 1.0)
        };
        let mut song_sample = 0.0;
        if self.playing && self.backing.is_some() {
            if self.count_in > 0 {
                let beat = self.count_in_pos
                    / self.backing.as_ref().unwrap().bars[self.cursor.bar_idx].beat;
                if self.song_last_beat != Some(beat) {
                    self.click_trigger(beat == 0, true);
                    self.song_last_beat = Some(beat);
                }
                self.count_in -= 1;
                self.count_in_pos += 1;
                if self.count_in == 0 {
                    self.song_last_beat = None;
                }
                song_sample += mix(&mut self.song_voices);
            } else {
                let bar = &self.backing.as_ref().unwrap().bars[self.cursor.bar_idx];
                let beat = self.cursor.sample_in_bar / bar.beat;
                let measure = self.cursor.sample_in_bar / bar.measure;
                if self.backing.as_ref().unwrap().click && self.song_last_beat != Some(beat) {
                    let accent = beat.is_multiple_of(bar.beats);
                    self.click_trigger(accent, true);
                    self.song_last_beat = Some(beat);
                }
                let bar = &self.backing.as_ref().unwrap().bars[self.cursor.bar_idx];
                if self.song_last_measure != Some(measure) {
                    if let Some(pcm) = &bar.chord {
                        trigger(&mut self.song_voices, pcm.clone());
                    }
                    self.song_last_measure = Some(measure);
                }
                if let Some(pcm) = &bar.phrase_pcm {
                    song_sample += pcm.data[self.cursor.sample_in_bar as usize];
                }
                if let Some(take) = &self.take {
                    let target = take.scope.selected().unwrap_or(self.cursor.bar_idx);
                    let data = &take.bars[target].pcm.data;
                    song_sample += data[self.cursor.sample_in_bar as usize % data.len()];
                } else if let Some(pcm) = &bar.loop_pcm {
                    if !pcm.is_empty() {
                        song_sample += pcm.data[self.cursor.sample_in_bar as usize % pcm.len()];
                    }
                }
                if let Some(take) = self.take.as_mut() {
                    let sample = self.feed.as_mut().and_then(|feed| {
                        audio_primitives::capture::InputFeed::pop_sample(&mut **feed)
                    });
                    if let Some(value) = sample {
                        let target = take.scope.selected().unwrap_or(self.cursor.bar_idx);
                        let bar = &mut take.bars[target];
                        let data = Arc::get_mut(&mut bar.pcm.data)
                            .expect("active recording storage has exclusive ownership");
                        let idx = self.cursor.sample_in_bar as usize % data.len();
                        let mode = if self.replace {
                            audio_primitives::capture::WriteMode::IndexedReplace
                        } else {
                            audio_primitives::capture::WriteMode::IndexedOverdubClamped
                        };
                        let written = audio_primitives::capture::TakeAccumulator::default()
                            .write(data, mode, idx, value);
                        debug_assert!(written);
                        bar.touched = true;
                    } else {
                        take.valid = false;
                        self.invalid = true;
                    }
                    if self
                        .feed
                        .as_ref()
                        .is_some_and(|f| f.overflow_count() != take.overflow)
                    {
                        take.valid = false;
                        self.invalid = true;
                    }
                }
                song_sample += mix(&mut self.song_voices);
                self.cursor.sample_in_bar += 1;
                let backing = self.backing.as_ref().unwrap();
                if self.cursor.sample_in_bar >= backing.bars[self.cursor.bar_idx].duration {
                    self.cursor.sample_in_bar = 0;
                    if let Some(target) = self.selected_capture_bar() {
                        self.cursor.bar_idx = target;
                    } else if self.cursor.bar_idx + 1 == backing.bars.len() {
                        if backing.one_shot {
                            self.playing = false;
                            self.clear_voices();
                            self.discard_unreachable_arms();
                            self.stop_capture();
                            for op in &mut self.pending {
                                if op.take().is_some() {
                                    self.shared.boundaries.fetch_sub(1, Ordering::Release);
                                }
                            }
                            self.pending_len = 0;
                        } else {
                            self.cursor.bar_idx = 0;
                        }
                    } else {
                        self.cursor.bar_idx += 1;
                    }
                    self.boundary();
                }
            }
        } else {
            song_sample += mix(&mut self.preview_voices);
        }
        let program_sample = if program_running {
            self.mix_program_voices()
        } else {
            0.0
        };
        self.program_frame_end();
        (metro + (song_sample * self.gain).clamp(-1.0, 1.0) + program_sample).clamp(-1.0, 1.0)
    }
}
impl AudioNodeProcessor for Processor {
    fn events(&mut self, _: &ProcInfo, events: &mut ProcEvents, _: &mut ProcExtra) {
        for mut event in events.drain() {
            if let Some(command) = event.downcast_mut::<Command>() {
                match command {
                    Command::Backing {
                        data,
                        reset,
                        clear,
                        playing,
                        generation,
                    } => {
                        if *clear {
                            if self.program.is_some() {
                                // Session replacement silences the old
                                // program at this callback boundary. If an
                                // earlier transition awaits observation,
                                // retain a dedicated stop request so it
                                // cannot be displaced by ordinary controls.
                                for voice in &mut self.program_voices {
                                    *voice = None;
                                }
                                for voice in &mut self.click_voices {
                                    *voice = None;
                                }
                                if self.program_transition_pending.is_some() {
                                    self.program_stop_pending = true;
                                } else {
                                    self.program_deferred_controls = std::array::from_fn(|_| None);
                                    self.program_deferred_len = 0;
                                    self.apply_program_control(DeferredProgramControl::Stop);
                                }
                            }
                            if let Some(take) = self.take.as_mut() {
                                take.valid = false;
                            }
                            self.stop_capture();
                            for op in &mut self.pending {
                                if op.take().is_some() {
                                    self.shared.boundaries.fetch_sub(1, Ordering::Release);
                                }
                            }
                            self.pending_len = 0;
                            self.invalid = false;
                        }
                        std::mem::swap(data, &mut self.backing);
                        self.generation = *generation;
                        self.replace = self.backing.as_ref().is_some_and(|b| b.replace);
                        if *reset {
                            self.cursor = Default::default();
                            self.start_playing(*playing);
                        }
                    },
                    Command::ClickPattern { data, bpm, playing } => {
                        std::mem::swap(data, &mut self.click);
                        self.bpm = *bpm;
                        if let Some(playing) = playing {
                            self.click_playing = *playing;
                        }
                        self.click_position = 0.0;
                        self.last_click = None;
                        for v in &mut self.click_voices {
                            *v = None;
                        }
                    },
                    Command::Metronome { bpm, playing } => {
                        let unit = self.click.as_ref().map_or(4, |click| click.beat_unit);
                        let before =
                            audio_primitives::frames_per_beat(self.rate, self.bpm, unit).max(1);
                        let after = audio_primitives::frames_per_beat(self.rate, *bpm, unit).max(1);
                        self.click_position = self.click_position / before as f64 * after as f64;
                        self.bpm = *bpm;
                        if *playing != self.click_playing {
                            for voice in &mut self.preview_voices {
                                *voice = None;
                            }
                            self.click_playing = *playing;
                            self.click_position = 0.0;
                            self.last_click = None;
                            if !*playing {
                                for v in &mut self.click_voices {
                                    *v = None;
                                }
                            }
                        }
                    },
                    Command::ClickPlaying(playing) => {
                        self.click_playing = *playing;
                        self.click_position = 0.0;
                        self.last_click = None;
                        if !*playing {
                            for v in &mut self.click_voices {
                                *v = None;
                            }
                        }
                    },
                    Command::Playing(playing) => {
                        self.start_playing(*playing);
                        if !*playing {
                            self.finish_take();
                        }
                    },
                    Command::Rewind => {
                        // A stale queued rewind cannot redirect a selected take.
                        if self.selected_capture_bar().is_some() {
                            self.handled_commands += 1;
                            continue;
                        }
                        self.cursor = Default::default();
                        self.count_in = 0;
                        self.clear_voices();
                        self.song_last_beat = None;
                        self.song_last_measure = None;
                    },
                    Command::Gain(gain) => self.gain = *gain,
                    Command::RecordReplace(replace) => self.replace = *replace,
                    Command::Preview { revision, buffer } => {
                        if !self.playing
                            && *revision == self.shared.preview_revision.load(Ordering::Acquire)
                        {
                            if let Some(pcm) = buffer.take() {
                                trigger(&mut self.preview_voices, pcm);
                                self.preview_starts = self.preview_starts.saturating_add(1);
                            }
                        }
                    },
                    Command::CancelPreview => {
                        for v in &mut self.preview_voices {
                            *v = None;
                        }
                    },
                    Command::Capture(feed) => std::mem::swap(feed, &mut self.feed),
                    Command::Arm {
                        bar_idx,
                        seek,
                        take,
                    } => {
                        if let Some(take) = take.take() {
                            self.push_boundary(Boundary::Arm {
                                bar: *bar_idx,
                                seek: *seek,
                                take,
                            });
                        }
                    },
                    Command::Boundary(change) => {
                        if matches!(
                            change,
                            PendingChange::SeekTo { .. } | PendingChange::SetPlaying(_)
                        ) {
                            for voice in &mut self.preview_voices {
                                *voice = None;
                            }
                        }
                        self.push_boundary(Boundary::Change(change.clone()));
                    },
                    Command::StopCapture => self.stop_capture(),
                    Command::ProgramStart { data, epoch } => {
                        if !self.playing && self.take.is_none() {
                            self.program = data.take();
                            self.program_epoch = *epoch;
                            self.program_loop_mode = self
                                .program
                                .as_ref()
                                .map_or(ProgramLoop::Once, |program| program.loop_mode);
                            self.program_card_index = self
                                .program
                                .as_ref()
                                .map_or(0, |program| program.start_index);
                            self.program_event_index = 0;
                            self.program_tick_phase_q32 = 0;
                            self.program_card_frames = 0;
                            self.program_tail_remaining = 0;
                            self.program_active_frames = 0;
                            self.program_status = ProgramStatus::Playing;
                            self.program_card_started = false;
                            self.program_advance = false;
                            self.program_transition_pending = None;
                            self.program_stop_pending = false;
                            self.program_pending_action = ProgramPendingAction::None;
                            self.program_deferred_controls = std::array::from_fn(|_| None);
                            self.program_deferred_len = 0;
                            self.program_event_token = None;
                            self.program_tail_started = false;
                            self.program_click_last_step = None;
                            self.program_event_due_frame = None;
                            self.program_click_last_beat = None;
                        }
                    },
                    Command::ProgramPause { epoch, paused } if *epoch == self.program_epoch => {
                        if self.program_status == ProgramStatus::WaitingObserver {
                            if *paused {
                                if self.program_resume_after_observer != ProgramStatus::Paused {
                                    self.program_resume_after_pause =
                                        self.program_resume_after_observer;
                                }
                                self.program_resume_after_observer = ProgramStatus::Paused;
                            } else if self.program_resume_after_observer == ProgramStatus::Paused {
                                self.program_resume_after_observer =
                                    self.program_resume_after_pause;
                            }
                        } else if *paused
                            && self.program_status != ProgramStatus::Paused
                            && self.program_status != ProgramStatus::Stopped
                            && self.program_status != ProgramStatus::Complete
                        {
                            self.program_resume_after_pause = self.program_status;
                            self.program_status = ProgramStatus::Paused;
                        } else if !*paused && self.program_status == ProgramStatus::Paused {
                            self.program_status = self.program_resume_after_pause;
                        }
                    },
                    Command::ProgramPause { .. } => {},
                    Command::ProgramTempo { epoch, card, bpm } if *epoch == self.program_epoch => {
                        if let Some(program) = self.program.as_mut() {
                            if let Some(target) = program.cards.get_mut(*card) {
                                let step = *bpm
                                    * super::clock::PROGRAM_TICKS_PER_BEAT as f64
                                    * (u32::MAX as f64 + 1.0)
                                    / (60.0 * self.rate as f64);
                                if step.is_finite() && step >= 1.0 && step <= u64::MAX as f64 {
                                    target.bpm = *bpm;
                                    if *card == self.program_card_index {
                                        self.program_tick_step_q32 = step.round() as u64;
                                    }
                                }
                            }
                        }
                    },
                    Command::ProgramTempo { .. } => {},
                    Command::ProgramLoopMode { epoch, mode } if *epoch == self.program_epoch => {
                        self.program_loop_mode = *mode;
                    },
                    Command::ProgramLoopMode { .. } => {},
                    Command::ProgramAdvance { epoch } if *epoch == self.program_epoch => {
                        if matches!(
                            self.program_status,
                            ProgramStatus::Playing
                                | ProgramStatus::Paused
                                | ProgramStatus::WaitingManual
                                | ProgramStatus::Refused
                        ) {
                            self.program_advance = true;
                        }
                    },
                    Command::ProgramAdvance { .. } => {},
                    Command::ProgramSeek { epoch, card } if *epoch == self.program_epoch => {
                        self.defer_program_control(DeferredProgramControl::Seek(*card));
                    },
                    Command::ProgramSeek { .. } => {},
                    Command::ProgramStop { epoch } if *epoch == self.program_epoch => {
                        self.defer_program_control(DeferredProgramControl::Stop);
                    },
                    Command::ProgramStop { .. } => {},
                }
                self.handled_commands += 1;
            }
        }
    }
    fn process(
        &mut self,
        info: &ProcInfo,
        buffers: ProcBuffers,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        self.flush_return();
        // Waiting/count-in input is not part of a take. It must not fill the
        // bounded feed and poison the take that begins at a future boundary.
        if self.take.is_none() && !self.playing {
            if let Some(feed) = self.feed.as_mut() {
                feed.discard_pending();
            }
        }
        let first_frame = info.clock_samples.0.max(0) as u64;
        for (offset, sample) in buffers.outputs[0][..info.frames].iter_mut().enumerate() {
            self.current_frame = first_frame.saturating_add(offset as u64);
            *sample = self.frame();
        }
        self.current_frame = first_frame.saturating_add(info.frames as u64);
        self.flush_return();
        self.blocks += 1;
        let program = self.program_snapshot();
        let click_step = if self.program.is_some()
            && matches!(
                self.program_status,
                ProgramStatus::Playing
                    | ProgramStatus::Paused
                    | ProgramStatus::WaitingManual
                    | ProgramStatus::WaitingObserver
                    | ProgramStatus::Refused
            ) {
            self.click.as_ref().map_or(0, |click| {
                let step_ticks =
                    (super::clock::PROGRAM_TICKS_PER_BEAT / click.divisions.max(1) as u64).max(1);
                ((program.beat_ticks / step_ticks) % click.steps.len().max(1) as u64) as usize
            })
        } else {
            self.click.as_ref().map_or(0, |c| {
                (self.last_click.unwrap_or(0) % c.steps.len() as u64) as usize
            })
        };
        self.shared.publish(OutputSnapshot {
            device_frames: info.clock_samples.0.max(0) as u64 + info.frames as u64,
            processing_blocks: self.blocks,
            cursor: self.cursor,
            playing: self.playing,
            recording: self.take.is_some(),
            capture_armed: self
                .pending
                .iter()
                .any(|op| matches!(op, Some(Boundary::Arm { .. }))),
            capture_invalid: self.invalid,
            preview_voices: self.preview_voices.iter().filter(|v| v.is_some()).count(),
            preview_starts: self.preview_starts,
            click_playing: self.click_playing,
            click_step,
            program,
        });
        if self.handled_commands != 0 {
            self.shared
                .commands
                .fetch_sub(self.handled_commands, Ordering::Release);
            self.handled_commands = 0;
        }
        ProcessStatus::OutputsModified
    }
}

fn default_click(rate: u32) -> [ArcGc<SampleBuffer>; 2] {
    std::array::from_fn(|idx| {
        let sound = Sound::click();
        let mut data = Vec::new();
        for local in 0..=(rate / 20) {
            if let Some(v) = sound.render_sample(local, rate as f32, idx == 0) {
                data.push(v);
            }
        }
        ArcGc::new(SampleBuffer::new(data, rate))
    })
}
