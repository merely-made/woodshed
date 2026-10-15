use super::*;
use crate::capture_feed::CaptureFeedConsumer;
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
    click_step: AtomicUsize,
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
        self.click_step
            .store(snapshot.click_step, Ordering::Relaxed);
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
                click_step: self.click_step.load(Ordering::Relaxed),
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
        for bar in &song.bars {
            let duration = bar.duration_samples(rate as f32).max(1) as usize;
            let chord_len = if bar.chord_ref.is_some() {
                (bar.time_signature.numerator.max(1) as f32 * 60.0 / bar.bpm.max(1.0))
                    .max(0.5)
                    .clamp(0.3, 2.0)
                    * rate as f32
            } else {
                0.0
            };
            resource_frames = resource_frames
                .checked_add(duration)
                .and_then(|n| n.checked_add(chord_len.ceil() as usize))
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
            let duration = bar.duration_samples(rate as f32).max(1);
            let beats = u64::from(bar.time_signature.numerator.max(1));
            let beat = ((60.0 / bar.bpm.max(1.0)) * rate as f32) as u64;
            let measure = beat.max(1).saturating_mul(beats).max(1);
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
                let duration_seconds = beats as f32 * 60.0 / bar.bpm.max(1.0);
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
            bars.push(PreparedBar {
                duration,
                beat: beat.max(1),
                measure,
                lead: (beats as f32 * (60.0 / bar.bpm.max(1.0)) * rate as f32) as u64,
                beats,
                chord,
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
        })
    }
}

pub(super) struct RecordBar {
    pub pcm: SampleBuffer,
    pub touched: bool,
    retired: Option<SampleBuffer>,
}
pub(super) struct Recording {
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
            let len = bar.duration_samples(rate as f32).max(1) as usize;
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
    returns: RefCell<Option<rtrb::Producer<OwnedGc<Recording>>>>,
}
impl OutputNode {
    pub fn new(shared: Arc<Shared>, returns: rtrb::Producer<OwnedGc<Recording>>) -> Self {
        Self {
            shared,
            returns: RefCell::new(Some(returns)),
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
            pending: std::array::from_fn(|_| None),
            pending_len: 0,
            rate: cx.stream_info.sample_rate.get(),
            blocks: 0,
            handled_commands: 0,
            invalid: false,
        })
    }
}
struct Processor {
    shared: Arc<Shared>,
    returns: rtrb::Producer<OwnedGc<Recording>>,
    backing: Option<OwnedGc<PreparedBacking>>,
    click: Option<OwnedGc<PreparedClick>>,
    feed: Option<OwnedGc<CaptureFeedConsumer>>,
    take: Option<OwnedGc<Recording>>,
    completed: Option<OwnedGc<Recording>>,
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
    click_position: f64,
    last_click: Option<u64>,
    song_last_beat: Option<u64>,
    song_last_measure: Option<u64>,
    click_voices: [Option<Voice>; 8],
    song_voices: [Option<Voice>; 8],
    preview_voices: [Option<Voice>; 16],
    pending: [Option<Boundary>; COMMAND_BOUND],
    pending_len: usize,
    rate: u32,
    blocks: u64,
    handled_commands: usize,
    invalid: bool,
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
                    for (idx, bar) in take.bars.iter_mut().enumerate() {
                        if bar.touched {
                            bar.retired = backing.bars[idx].loop_pcm.replace(bar.pcm.clone());
                        }
                    }
                }
            }
            self.completed = Some(take);
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
    fn push_boundary(&mut self, op: Boundary) {
        if self.pending_len < COMMAND_BOUND {
            self.pending[self.pending_len] = Some(op);
            self.pending_len += 1;
        } else {
            self.invalid = true;
            self.shared.boundaries.fetch_sub(1, Ordering::Release);
        } // Retired resources use deferred GC.
    }
    fn boundary(&mut self) {
        self.song_last_beat = None;
        self.song_last_measure = None;
        for idx in 0..self.pending_len {
            if let Some(op) = self.pending[idx].take() {
                self.shared.boundaries.fetch_sub(1, Ordering::Release);
                match op {
                    Boundary::Change(PendingChange::SeekTo { bar_idx }) => {
                        if self
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
                    Boundary::Change(PendingChange::StopRecording) => self.finish_take(),
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
        self.pending_len = 0;
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
    fn frame(&mut self) -> f32 {
        if self.take.is_none() {
            if let Some(feed) = self.feed.as_mut() {
                let _ = feed.pop_sample();
            }
        }
        if self.click_playing {
            if let Some(click) = self.click.as_ref() {
                let absolute_step = self.click_position as u64;
                let index = (absolute_step % click.steps.len() as u64) as usize;
                if self.last_click != Some(absolute_step) {
                    if let Some(pcm) = &click.steps[index] {
                        trigger(&mut self.click_voices, pcm.clone());
                    }
                    self.last_click = Some(absolute_step);
                }
                self.click_position +=
                    self.bpm as f64 * click.divisions as f64 / (60.0 * self.rate as f64);
            }
        }
        let metro = mix(&mut self.click_voices).clamp(-1.0, 1.0);
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
                if let Some(take) = &self.take {
                    let data = &take.bars[self.cursor.bar_idx].pcm.data;
                    song_sample += data[self.cursor.sample_in_bar as usize % data.len()];
                } else if let Some(pcm) = &bar.loop_pcm {
                    if !pcm.is_empty() {
                        song_sample += pcm.data[self.cursor.sample_in_bar as usize % pcm.len()];
                    }
                }
                if let Some(take) = self.take.as_mut() {
                    let sample = self.feed.as_mut().and_then(|f| f.pop_sample());
                    if let Some(value) = sample {
                        let bar = &mut take.bars[self.cursor.bar_idx];
                        let data = Arc::get_mut(&mut bar.pcm.data)
                            .expect("active recording storage has exclusive ownership");
                        let idx = self.cursor.sample_in_bar as usize % data.len();
                        data[idx] = if self.replace {
                            value.clamp(-1.0, 1.0)
                        } else {
                            (data[idx] + value).clamp(-1.0, 1.0)
                        };
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
                    if self.cursor.bar_idx + 1 == backing.bars.len() {
                        if backing.one_shot {
                            self.playing = false;
                            self.clear_voices();
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
        (metro + (song_sample * self.gain).clamp(-1.0, 1.0)).clamp(-1.0, 1.0)
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
        for sample in &mut buffers.outputs[0][..info.frames] {
            *sample = self.frame();
        }
        self.flush_return();
        self.blocks += 1;
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
            click_playing: self.click_playing,
            click_step: self.click.as_ref().map_or(0, |c| {
                (self.last_click.unwrap_or(0) % c.steps.len() as u64) as usize
            }),
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
