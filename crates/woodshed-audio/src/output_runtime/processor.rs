use super::clock::{
    FALLBACK_PHASE_FRACTION_BITS, PreparedProgram, PreparedTone, ProgramDuration, ProgramEnd,
    ProgramEndReason, ProgramEpoch, ProgramLoop, ProgramPosition, ProgramSnapshot, ProgramStatus,
    ProgramTransition, ProgramTransitionKind,
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
    program_revision: AtomicU64,
    program_card_count: AtomicUsize,
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

pub(super) struct PreparedBar {
    duration: u64,
    beat: u64,
    beat_unit: u8,
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

pub(super) struct PreparedClick {
    steps: Vec<Option<ArcGc<SampleBuffer>>>,
    divisions: u32,
    beat_unit: u8,
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

pub(super) struct Recording {
    pub scope: RecordingScope,
    pub generation: u64,
    pub bars: Vec<RecordBar>,
    pub valid: bool,
    overflow: u64,
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
        backing_rhythm: bool,
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
    ProgramReplace {
        epoch: ProgramEpoch,
        revision: u64,
        data: Option<OwnedGc<PreparedProgram>>,
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
    ProgramTempoAll {
        epoch: ProgramEpoch,
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
struct Processor {
    shared: Arc<Shared>,
    returns: rtrb::Producer<CompletedRecording>,
    program_transitions: rtrb::Producer<ProgramTransition>,
    backing: Option<OwnedGc<PreparedBacking>>,
    click: Option<OwnedGc<PreparedClick>>,
    backing_rhythm: bool,
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
    program_replacement: Option<(u64, OwnedGc<PreparedProgram>)>,
    program_revision: u64,
    program_requested_revision: u64,
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
    phase_q64: u64,
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
impl Processor {}

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

mod shared;

mod callback;

mod program_control;

mod program_render;

mod node;

mod preparation;
