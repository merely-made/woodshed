//! Song-mode audio engine.
//!
//! [`SongEngine`] owns a cpal output stream and renders a [`Song`]
//! according to its current cursor:
//!
//! - **Click** — a [`Sound::Click`] voice fires on every beat boundary
//!   using the current bar's BPM + time signature.
//! - **Chord** — at each bar boundary, if the bar carries a
//!   [`ChordRef`], its pre-rendered chord buffer is triggered as a
//!   [`Sound::Sample`] voice.
//! - **Loop** — if the bar has an `audio_buffer`, it's played back
//!   sample-by-sample at the bar's cursor position. If `recording`
//!   is on, input samples drained from the [`crate::LooperCaptureHandle`]
//!   ring are *also* mixed into the buffer at the cursor.
//!
//! Parallel to [`crate::SequencerEngine`] — the two engines own
//! separate output streams. The app picks which one is active by
//! starting / stopping each independently.
//!
//! # Why a separate engine
//!
//! The existing SequencerEngine's mixer is locked to a single
//! [`SequencerPattern`]. Song mode has per-bar tempo, per-bar chord,
//! per-bar recorded loops — adding all that as a third mode in the
//! sequencer would balloon `process_buffer`'s branching. Splitting
//! keeps each mixer focused.

use std::collections::VecDeque;
use std::sync::{
    Arc, Condvar, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};

use crate::chord_audio::{ChordRender, render_chord, render_chord_cancellable};
use crate::engine::{AudioError, Voice};
use crate::input::LooperCaptureHandle;
use crate::song::{Bar, PendingChange, Song};
use crate::sound::{SampleBuffer, Sound};

// =================================================================
// Shared state
// =================================================================

/// Identity of the inputs actually used by the fixed strum renderer. Labels,
/// bar length and recorded loops do not change this per-measure PCM.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ChordKey {
    pitches: Vec<u32>,
    duration: u32,
    sample_rate: u32,
}

impl ChordKey {
    fn for_bar(bar: &Bar, sample_rate: u32) -> Option<Self> {
        let chord = bar.chord_ref.as_ref()?;
        let pitches = if chord.pitches_hz.is_empty() {
            vec![chord.root_freq_hz.to_bits()]
        } else {
            chord.pitches_hz.iter().map(|p| p.to_bits()).collect()
        };
        Some(Self {
            pitches,
            duration: chord_duration(bar).to_bits(),
            sample_rate,
        })
    }

    /// Callback validation performs no key allocation, even during an edit.
    fn matches(&self, bar: &Bar, sample_rate: u32) -> bool {
        let Some(chord) = &bar.chord_ref else {
            return false;
        };
        self.sample_rate == sample_rate
            && self.duration == chord_duration(bar).to_bits()
            && if chord.pitches_hz.is_empty() {
                self.pitches.as_slice() == [chord.root_freq_hz.to_bits()]
            } else {
                self.pitches
                    .iter()
                    .copied()
                    .eq(chord.pitches_hz.iter().map(|p| p.to_bits()))
            }
    }

    fn params(&self) -> ChordRender {
        ChordRender::strum(
            self.pitches.iter().map(|p| f32::from_bits(*p)).collect(),
            f32::from_bits(self.duration),
        )
    }
}

fn chord_duration(bar: &Bar) -> f32 {
    ((bar.time_signature.numerator.max(1) as f32) * (60.0 / bar.bpm.max(1.0)))
        .max(0.5)
        .clamp(0.3, 2.0)
}

#[derive(Clone)]
struct PreparedChord {
    key: ChordKey,
    buffer: SampleBuffer,
}

/// Runs before a stream exists or outside its mixer mutex.
fn prepare_song_chords(
    song: &Song,
    sample_rate: u32,
    mut render: impl FnMut(&ChordRender, u32) -> SampleBuffer,
) -> Vec<Option<PreparedChord>> {
    song.bars
        .iter()
        .map(|bar| {
            ChordKey::for_bar(bar, sample_rate).map(|key| {
                let buffer = render(&key.params(), sample_rate);
                PreparedChord { key, buffer }
            })
        })
        .collect()
}

struct SongEngineInternals {
    song: Song,
    /// Latest replacement request; slow older preparations cannot overwrite it.
    replacement_revision: u64,
    sample_rate_hz: f32,
    /// Voices currently sounding (clicks, chord triggers).
    voices: Vec<Voice>,
    /// Absolute samples since the stream started — used as voice
    /// timing reference. Independent of `song.cursor` which uses the
    /// bar-relative clock.
    voice_clock: u64,
    /// Last beat-within-bar we fired a click for. -1 = next beat
    /// crossing will fire.
    last_click_beat: i64,
    /// Last bar index we fired a chord for. -1 = chord on first
    /// landed bar.
    last_chord_bar: i64,
    /// Last measure-within-block index we fired a chord for. Lets a
    /// multi-measure bar re-strike its chord each measure (a "C ×4"
    /// block) rather than sounding once and going silent. -1 = fire
    /// on the next measure boundary.
    last_chord_measure: i64,
    /// Prepared per-measure buffers, validated against their audible inputs.
    /// The callback only consumes ready buffers; it never synthesizes a miss.
    chord_cache: Vec<Option<PreparedChord>>,
    /// Optional input ring drained when `song.recording` is true.
    capture_ring: Option<Arc<Mutex<VecDeque<f32>>>>,
    /// Output channel count for mono → stereo fan-out.
    channels: u16,
    /// User-set master output gain in [0, 1]. The chord + click + loop
    /// mix gets scaled by this before clipping.
    output_gain: f32,
    /// Samples remaining in the one-bar count-in click lead. 0 = not
    /// counting in. Set by `play()` (when the click is enabled); while
    /// non-zero, only the count-in click sounds and the song cursor
    /// holds still.
    count_in_remaining: u64,
    /// Samples elapsed within the current count-in, for beat detection.
    count_in_pos: u64,
    /// One-shot voices played on demand via [`SongEngineHandle::play_note_now`]
    /// — e.g. the arpeggio/exercise step-through sonification. Mixed even
    /// when the song isn't playing (that's the whole point), on their own
    /// always-advancing clock.
    oneshot_voices: Vec<Voice>,
    /// Sample clock for `oneshot_voices`; advances whenever the song is
    /// stopped (when the song plays, one-shots are cleared instead).
    oneshot_clock: u64,
}

impl SongEngineInternals {
    fn new(song: Song, sample_rate_hz: f32, channels: u16) -> Self {
        let chord_cache = prepare_song_chords(&song, sample_rate_hz as u32, render_chord);
        Self {
            song,
            replacement_revision: 0,
            sample_rate_hz,
            voices: Vec::new(),
            voice_clock: 0,
            last_click_beat: -1,
            last_chord_bar: -1,
            last_chord_measure: -1,
            chord_cache,
            capture_ring: None,
            channels,
            output_gain: 0.8,
            count_in_remaining: 0,
            count_in_pos: 0,
            oneshot_voices: Vec::new(),
            oneshot_clock: 0,
        }
    }

    /// Preserve matching PCM across read-only/cosmetic edits. Changed slots
    /// become unavailable until prepared off-lock; stale PCM is never played.
    fn resync_chord_cache(&mut self) {
        self.chord_cache.resize(self.song.bars.len(), None);
        for (bar, slot) in self.song.bars.iter().zip(&mut self.chord_cache) {
            if slot
                .as_ref()
                .is_some_and(|chord| !chord.key.matches(bar, self.sample_rate_hz as u32))
            {
                *slot = None;
            }
        }
    }
}

// =================================================================
// Public handle
// =================================================================

// One renderer, one in-flight request, one replaceable pending request. The
// worker holds only a Weak mixer reference, so cloned handles cannot form a cycle.
struct PreviewRequest {
    params: ChordRender,
    id: &'static str,
    rate: u32,
    revision: u64,
}
#[derive(Default)]
struct PreviewQueue {
    pending: Option<PreviewRequest>,
    running: bool,
    closed: bool,
}
#[derive(Default)]
struct PreviewMailbox {
    queue: Mutex<PreviewQueue>,
    changed: Condvar,
    revision: AtomicU64,
}
struct PreviewWorker {
    mailbox: Arc<PreviewMailbox>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl PreviewWorker {
    fn new(
        mixer: Weak<Mutex<SongEngineInternals>>,
        mut render: impl FnMut(&ChordRender, u32, &dyn Fn() -> bool) -> Option<SampleBuffer>
        + Send
        + 'static,
    ) -> Self {
        let mailbox = Arc::new(PreviewMailbox::default());
        let worker = mailbox.clone();
        let thread = std::thread::Builder::new()
            .name("woodshed-preview".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut q = worker.queue.lock().unwrap();
                        while q.pending.is_none() && !q.closed {
                            q = worker.changed.wait(q).unwrap();
                        }
                        if q.closed {
                            break;
                        }
                        q.running = true;
                        q.pending.take().unwrap()
                    };
                    let cancelled = || worker.revision.load(Ordering::Acquire) != request.revision;
                    if let Some(buffer) = render(&request.params, request.rate, &cancelled) {
                        if let Some(mixer) = mixer.upgrade() {
                            let mut s = mixer.lock().unwrap();
                            if !cancelled() && !s.song.playing {
                                push_oneshot(&mut s, buffer, request.id);
                            }
                        }
                    }
                    let mut q = worker.queue.lock().unwrap();
                    q.running = false;
                    worker.changed.notify_all();
                }
            })
            .expect("start preview worker");
        Self {
            mailbox,
            thread: Mutex::new(Some(thread)),
        }
    }

    // Call with the mixer locked. Publication validates under the same lock;
    // an invalidated render cannot race a stop or a newer request into output.
    fn cancel(&self) {
        self.mailbox.revision.fetch_add(1, Ordering::AcqRel);
        self.mailbox.queue.lock().unwrap().pending = None;
    }

    fn shutdown(&self) {
        {
            let mut q = self.mailbox.queue.lock().unwrap();
            q.closed = true;
            q.pending = None;
            self.mailbox.revision.fetch_add(1, Ordering::AcqRel);
            self.mailbox.changed.notify_all();
        }
        if let Some(thread) = self.thread.lock().unwrap().take() {
            thread.join().expect("preview worker panicked");
        }
    }
}
impl Drop for PreviewWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Thread-safe handle to control / inspect the running [`SongEngine`].
#[derive(Clone)]
pub struct SongEngineHandle {
    inner: Arc<Mutex<SongEngineInternals>>,
    preview: Arc<PreviewWorker>,
}

impl SongEngineHandle {
    fn new(inner: Arc<Mutex<SongEngineInternals>>) -> Self {
        Self::with_preview_renderer(inner, render_chord_cancellable)
    }

    fn with_preview_renderer(
        inner: Arc<Mutex<SongEngineInternals>>,
        render: impl FnMut(&ChordRender, u32, &dyn Fn() -> bool) -> Option<SampleBuffer>
        + Send
        + 'static,
    ) -> Self {
        let preview = Arc::new(PreviewWorker::new(Arc::downgrade(&inner), render));
        Self { inner, preview }
    }

    /// Whether preview synthesis is queued or in progress (not voice playback).
    pub fn preview_busy(&self) -> bool {
        let q = self.preview.mailbox.queue.lock().unwrap();
        q.running || q.pending.is_some()
    }

    /// Number of sounding preview envelopes, for read-only diagnostics.
    pub fn preview_voice_count(&self) -> usize {
        self.inner.lock().unwrap().oneshot_voices.len()
    }

    /// Cancel queued/in-flight synthesis and stop sounding preview envelopes.
    pub fn cancel_preview(&self) {
        let mut s = self.inner.lock().unwrap();
        self.cancel_preview_locked(&mut s);
    }

    fn cancel_preview_locked(&self, s: &mut SongEngineInternals) {
        self.preview.cancel();
        s.oneshot_voices.clear();
    }

    /// Replace the active song. Resets transport, clears cached
    /// audio. Call when switching arrangements or after editing a
    /// bar's chord.
    pub fn set_song(&self, song: Song) {
        self.replace_song(song, render_chord);
    }

    fn replace_song(&self, song: Song, render: impl FnMut(&ChordRender, u32) -> SampleBuffer) {
        let (rate, revision) = {
            let mut s = self.inner.lock().unwrap();
            self.cancel_preview_locked(&mut s);
            s.replacement_revision = s.replacement_revision.wrapping_add(1);
            (s.sample_rate_hz as u32, s.replacement_revision)
        };
        let chord_cache = prepare_song_chords(&song, rate, render);
        let mut s = self.inner.lock().unwrap();
        if s.replacement_revision != revision {
            return;
        }
        self.cancel_preview_locked(&mut s);
        s.song = song;
        s.song.cursor = Default::default();
        s.song.playing = false;
        s.song.recording = false;
        s.song.clear_pending();
        s.voices.clear();
        s.last_click_beat = -1;
        s.last_chord_bar = -1;
        s.chord_cache = chord_cache;
        s.count_in_remaining = 0;
    }

    /// Read the current song state (cursor, playing, recording, etc.).
    pub fn song(&self) -> Song {
        self.inner.lock().unwrap().song.clone()
    }

    /// The playback cursor's current bar index. Read-only — unlike
    /// [`Self::with_song`] it doesn't resync the chord cache, so it's
    /// cheap to poll every frame (timeline follow).
    pub fn cursor_bar(&self) -> usize {
        self.inner.lock().unwrap().song.cursor.bar_idx
    }

    /// Whether the engine is currently capturing input into a bar
    /// (read-only, no resync).
    pub fn is_recording(&self) -> bool {
        self.inner.lock().unwrap().song.recording
    }

    /// Per-bar recorded-loop presence (read-only, no resync).
    pub fn loop_flags(&self) -> Vec<bool> {
        self.inner
            .lock()
            .unwrap()
            .song
            .bars
            .iter()
            .map(|b| b.audio_buffer.is_some())
            .collect()
    }

    /// Output sample rate the engine is running at. Lets the UI turn
    /// `cursor.sample_in_bar` into a within-bar fraction for the
    /// playhead.
    pub fn sample_rate(&self) -> f32 {
        self.inner.lock().unwrap().sample_rate_hz
    }

    /// Apply a closure to the active song under the lock. Use for
    /// edits the engine must see atomically (e.g. paste-bar).
    pub fn with_song<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Song) -> R,
    {
        let mut s = self.inner.lock().unwrap();
        let playing = s.song.playing;
        let result = f(&mut s.song);
        if playing != s.song.playing {
            self.cancel_preview_locked(&mut s);
        }
        s.resync_chord_cache();
        drop(s);
        self.prepare_chords(render_chord);
        result
    }

    /// Snapshot misses briefly, synthesize on the caller without the mixer
    /// mutex, then publish only if the current bar still has those inputs.
    /// A concurrent edit/replacement cannot receive an obsolete render.
    fn prepare_chords(&self, mut render: impl FnMut(&ChordRender, u32) -> SampleBuffer) {
        let requests: Vec<_> = {
            let s = self.inner.lock().unwrap();
            s.song
                .bars
                .iter()
                .enumerate()
                .filter_map(|(idx, bar)| {
                    if s.chord_cache
                        .get(idx)
                        .and_then(Option::as_ref)
                        .is_some_and(|cached| cached.key.matches(bar, s.sample_rate_hz as u32))
                    {
                        None
                    } else {
                        ChordKey::for_bar(bar, s.sample_rate_hz as u32).map(|key| (idx, key))
                    }
                })
                .collect()
        };
        for (idx, key) in requests {
            let buffer = render(&key.params(), key.sample_rate);
            let mut s = self.inner.lock().unwrap();
            if s.song
                .bars
                .get(idx)
                .is_some_and(|bar| key.matches(bar, s.sample_rate_hz as u32))
            {
                s.chord_cache[idx] = Some(PreparedChord { key, buffer });
            }
        }
    }

    /// Begin playback from the cursor, after a one-bar count-in click
    /// lead (only when the click is enabled — a muted click skips the
    /// count-in rather than stalling silently).
    pub fn play(&self) {
        self.cancel_preview();
        self.prepare_chords(render_chord);
        let mut s = self.inner.lock().unwrap();
        self.cancel_preview_locked(&mut s);
        s.song.playing = true;
        s.voice_clock = 0;
        s.voices.clear();
        // Drop any pending step-through one-shots — song playback owns the
        // output now (and the one-shot clock won't advance while playing).
        s.oneshot_voices.clear();
        // Force chord/click triggers to fire on the next eligible
        // sample so the user hears the bar immediately.
        s.last_click_beat = -1;
        s.last_chord_bar = -1;
        s.count_in_pos = 0;
        s.count_in_remaining = if s.song.click_enabled {
            let idx = s
                .song
                .cursor
                .bar_idx
                .min(s.song.bars.len().saturating_sub(1));
            let bar = &s.song.bars[idx];
            let num = bar.time_signature.numerator.max(1) as f32;
            let secs_per_beat = 60.0 / bar.bpm.max(1.0);
            (num * secs_per_beat * s.sample_rate_hz) as u64
        } else {
            0
        };
    }

    /// Stop playback. Cursor stays where it is.
    pub fn stop(&self) {
        let mut s = self.inner.lock().unwrap();
        self.cancel_preview_locked(&mut s);
        s.song.playing = false;
        s.voices.clear();
        s.count_in_remaining = 0;
    }

    /// Rewind to bar 0, sample 0.
    pub fn rewind(&self) {
        let mut s = self.inner.lock().unwrap();
        self.cancel_preview_locked(&mut s);
        s.song.cursor = Default::default();
        s.last_click_beat = -1;
        s.last_chord_bar = -1;
        s.count_in_remaining = 0;
        s.voices.clear();
    }

    /// Queue a pending change to apply at the next bar boundary.
    pub fn queue(&self, change: PendingChange) {
        let mut s = self.inner.lock().unwrap();
        if matches!(
            change,
            PendingChange::SetPlaying(_) | PendingChange::SeekTo { .. }
        ) {
            self.cancel_preview_locked(&mut s);
        }
        s.song.queue(change);
    }

    /// Wire the input capture ring. Must be called before song-mode
    /// recording will pick up anything.
    pub fn set_capture(&self, handle: &LooperCaptureHandle) {
        self.inner.lock().unwrap().capture_ring = Some(handle.ring_arc());
    }

    pub fn set_output_gain(&self, gain: f32) {
        self.inner.lock().unwrap().output_gain = gain.clamp(0.0, 1.0);
    }

    /// Queue a single note while the song is stopped. Synthesis runs on the
    /// preview worker; the UI returns immediately. Already sounding step notes
    /// keep their envelopes, while unfinished older requests are superseded.
    pub fn play_note_now(&self, freq_hz: f32, duration_secs: f32) {
        if freq_hz <= 0.0 {
            return;
        }
        let params = ChordRender::block(vec![freq_hz], duration_secs.max(0.05));
        self.queue_preview(params, "step-note", false);
    }

    /// Queue a strummed one-shot while the song is stopped. A new Hear
    /// replaces queued/in-flight previews and sounding preview envelopes. `strum_ms`
    /// staggers note onsets: 0 = block chord, ~18 = a gentle strum,
    /// larger = an arpeggiated cascade (a scale run). Zero-or-negative
    /// pitches are dropped; an all-empty set is a no-op. Mixed on the
    /// one-shot clock like [`Self::play_note_now`], so it sounds while
    /// the song is stopped.
    pub fn play_chord_now(&self, pitches_hz: &[f32], duration_secs: f32, strum_ms: f32) {
        let pitches: Vec<f32> = pitches_hz.iter().copied().filter(|f| *f > 0.0).collect();
        if pitches.is_empty() {
            return;
        }
        let params = ChordRender {
            pitches_hz: pitches,
            duration_seconds: duration_secs.max(0.1),
            strum_offset_ms: strum_ms.max(0.0),
            ..ChordRender::default()
        };
        self.queue_preview(params, "preview-chord", true);
    }

    fn queue_preview(&self, params: ChordRender, id: &'static str, replace: bool) {
        let mut s = self.inner.lock().unwrap();
        let mut q = self.preview.mailbox.queue.lock().unwrap();
        if q.closed || s.song.playing {
            return;
        }
        let revision = self
            .preview
            .mailbox
            .revision
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        if replace {
            s.oneshot_voices.clear();
        }
        q.pending = Some(PreviewRequest {
            params,
            id,
            rate: s.sample_rate_hz as u32,
            revision,
        });
        self.preview.mailbox.changed.notify_one();
    }

    #[cfg(test)]
    fn wait_preview(&self) {
        let mut q = self.preview.mailbox.queue.lock().unwrap();
        while q.running || q.pending.is_some() {
            let (next, timeout) = self
                .preview
                .mailbox
                .changed
                .wait_timeout(q, std::time::Duration::from_secs(5))
                .unwrap();
            q = next;
            assert!(!timeout.timed_out(), "preview worker did not finish");
        }
    }
}

// =================================================================
// Engine
// =================================================================

pub struct SongEngine {
    handle: SongEngineHandle,
    _stream: Stream,
}

impl SongEngine {
    pub fn new(initial_song: Song) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;
        let supported = device
            .default_output_config()
            .map_err(AudioError::StreamConfig)?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let channels = config.channels;
        let sample_rate = config.sample_rate as f32;

        let internals = Arc::new(Mutex::new(SongEngineInternals::new(
            initial_song,
            sample_rate,
            channels,
        )));
        let internals_for_callback = Arc::clone(&internals);

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut s = internals_for_callback.lock().unwrap();
                        process_song_buffer(&mut s, data);
                    },
                    |err| eprintln!("song engine stream error: {err}"),
                    None,
                )
                .map_err(AudioError::StreamBuild)?,
            other => return Err(AudioError::UnsupportedSampleFormat(other)),
        };
        stream.play().map_err(AudioError::StreamPlay)?;

        Ok(Self {
            handle: SongEngineHandle::new(internals),
            _stream: stream,
        })
    }

    pub fn handle(&self) -> SongEngineHandle {
        self.handle.clone()
    }
}

impl Drop for SongEngine {
    fn drop(&mut self) {
        self.handle.cancel_preview();
        self.handle.preview.shutdown();
    }
}

// =================================================================
// Mixer — runs in the cpal callback
// =================================================================

fn process_song_buffer(s: &mut SongEngineInternals, output: &mut [f32]) {
    let chs = s.channels.max(1) as usize;
    let frames = output.len() / chs;
    let sample_rate = s.sample_rate_hz;

    if !s.song.playing {
        // Song stopped — but still mix any on-demand one-shot voices
        // (arpeggio/exercise step notes) on their own clock so the
        // step-through sonification works without song playback.
        for frame_idx in 0..frames {
            let mut sample_value = 0.0_f32;
            s.oneshot_voices.retain(|v| {
                let local = s.oneshot_clock.saturating_sub(v.start_sample) as u32;
                match v.sound.render_sample(local, sample_rate, false) {
                    Some(value) => {
                        sample_value += value;
                        true
                    },
                    None => false,
                }
            });
            let mixed = (sample_value * s.output_gain).clamp(-1.0, 1.0);
            for ch in 0..chs {
                output[frame_idx * chs + ch] = mixed;
            }
            s.oneshot_clock += 1;
        }
        return;
    }

    // Drain the input ring if recording is on. We read into a local
    // buffer first so we hold the input lock for as little as possible.
    let recorded_input: Vec<f32> = if s.song.recording {
        if let Some(ring) = &s.capture_ring {
            let mut ring = ring.lock().unwrap();
            let take = frames.min(ring.len());
            let mut out = Vec::with_capacity(take);
            for _ in 0..take {
                if let Some(v) = ring.pop_front() {
                    out.push(v);
                }
            }
            out
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    for frame_idx in 0..frames {
        // Count-in: a one-bar click lead before the song proper. The
        // cursor holds still; only the count-in click sounds.
        if s.count_in_remaining > 0 {
            let idx = s.song.cursor.bar_idx;
            let bpm = s.song.bars[idx].bpm.max(1.0);
            let samples_per_beat = ((60.0 / bpm) * sample_rate) as u64;
            let beat = if samples_per_beat == 0 {
                0
            } else {
                (s.count_in_pos / samples_per_beat) as i64
            };
            if beat != s.last_click_beat {
                let downbeat = beat == 0;
                s.voices
                    .push(Voice::new(Sound::click(), downbeat, s.voice_clock));
                s.last_click_beat = beat;
            }
            let mut sample_value = 0.0_f32;
            s.voices.retain(|v| {
                let local = s.voice_clock.saturating_sub(v.start_sample) as u32;
                match v.sound.render_sample(local, sample_rate, v.accent) {
                    Some(value) => {
                        sample_value += value;
                        true
                    },
                    None => false,
                }
            });
            let mixed = (sample_value * s.output_gain).clamp(-1.0, 1.0);
            for ch in 0..chs {
                output[frame_idx * chs + ch] = mixed;
            }
            s.voice_clock += 1;
            s.count_in_pos += 1;
            s.count_in_remaining -= 1;
            if s.count_in_remaining == 0 {
                // Hand off to the song: fire bar 1's click + chord fresh.
                s.last_click_beat = -1;
                s.last_chord_bar = -1;
            }
            continue;
        }

        let sample_in_bar = s.song.cursor.sample_in_bar;
        let bar_idx = s.song.cursor.bar_idx;
        let bar = &s.song.bars[bar_idx];
        let secs_per_beat = 60.0 / bar.bpm.max(1.0);
        let samples_per_beat = (secs_per_beat * sample_rate) as u64;
        // A "measure" is one numerator's worth of beats. A bar may
        // span several (length > 1); the click downbeat + chord
        // re-strike track measure boundaries within the block so a
        // length-4 C bar plays "C ×4", not one C held for four bars.
        let num_beats = bar.time_signature.numerator.max(1) as u64;
        let measure_samples = (num_beats * samples_per_beat).max(1);

        // Beat-boundary click trigger. Clicks fire every beat; the
        // accent lands on beat 1 of each measure.
        let beat_in_bar = if samples_per_beat == 0 {
            0
        } else {
            (sample_in_bar / samples_per_beat) as i64
        };
        if beat_in_bar != s.last_click_beat {
            if s.song.click_enabled {
                let downbeat = (sample_in_bar % measure_samples) / samples_per_beat.max(1) == 0;
                let click = Sound::click();
                s.voices.push(Voice::new(click, downbeat, s.voice_clock));
            }
            s.last_click_beat = beat_in_bar;
        }

        // Measure-boundary chord trigger. Re-strikes at the start of
        // every measure in the block (the chord is rendered one
        // measure long, so it sounds then leaves a break before the
        // next strike).
        let measure_idx = (sample_in_bar / measure_samples) as i64;
        let new_block = (bar_idx as i64) != s.last_chord_bar;
        let new_measure = measure_idx != s.last_chord_measure;
        if (new_block || new_measure) && (sample_in_bar % measure_samples) < samples_per_beat {
            // During an in-flight live edit, a missing/new buffer skips this
            // strike rather than blocking or playing stale PCM. Publication
            // never creates a late strike; the next measure uses ready audio.
            if let Some(cached) = s
                .chord_cache
                .get(bar_idx)
                .and_then(Option::as_ref)
                .filter(|cached| cached.key.matches(bar, sample_rate as u32))
            {
                let chord_sound =
                    Sound::sample_with_buffer(format!("song-bar-{bar_idx}"), cached.buffer.clone());
                s.voices.push(Voice::new(chord_sound, false, s.voice_clock));
            }
            s.last_chord_bar = bar_idx as i64;
            s.last_chord_measure = measure_idx;
        }

        // Mix active voices into this frame.
        let mut sample_value = 0.0_f32;
        s.voices.retain(|v| {
            let local = s.voice_clock.saturating_sub(v.start_sample) as u32;
            match v.sound.render_sample(local, sample_rate, v.accent) {
                Some(value) => {
                    sample_value += value;
                    true
                },
                None => false,
            }
        });

        // Mix the bar's recorded audio loop (read at the bar cursor).
        if let Some(buf) = bar.audio_buffer.as_ref() {
            if !buf.is_empty() {
                let idx = (sample_in_bar as usize) % buf.len();
                sample_value += buf.data[idx];
            }
        }

        // Write input audio into the recorded buffer if recording is
        // on — overdub (sum) or replace (overwrite) per `record_replace`.
        if s.song.recording {
            let replace = s.song.record_replace;
            if let Some(in_sample) = recorded_input.get(frame_idx) {
                let bar = &mut s.song.bars[bar_idx];
                let bar_len_samples = bar.duration_samples(sample_rate) as usize;
                let buf = bar.audio_buffer.get_or_insert_with(|| {
                    SampleBuffer::new(vec![0.0; bar_len_samples], sample_rate as u32)
                });
                // Resize if the bar's tempo changed since the last
                // record (changing buffer length on the fly is rare
                // but it'd otherwise read/write past the end).
                if buf.len() != bar_len_samples {
                    *buf = SampleBuffer::new(vec![0.0; bar_len_samples], sample_rate as u32);
                }
                // `Arc::make_mut` clones the underlying Vec if any
                // other Arc still references it (e.g. the UI holds a
                // snapshot). Cheap fast-path when the engine is the
                // sole owner.
                let data_mut = std::sync::Arc::make_mut(&mut buf.data);
                let len = data_mut.len().max(1);
                let idx = (sample_in_bar as usize) % len;
                data_mut[idx] = if replace {
                    (*in_sample).clamp(-1.0, 1.0)
                } else {
                    (data_mut[idx] + in_sample).clamp(-1.0, 1.0)
                };
            }
        }

        let mixed = (sample_value * s.output_gain).clamp(-1.0, 1.0);
        for ch in 0..chs {
            output[frame_idx * chs + ch] = mixed;
        }

        // Advance the cursor by one sample, handling bar boundaries.
        s.voice_clock += 1;
        s.song.cursor.sample_in_bar += 1;
        let bar_len = s.song.bars[bar_idx].duration_samples(sample_rate);
        if s.song.cursor.sample_in_bar >= bar_len {
            s.song.cursor.sample_in_bar = 0;
            s.last_click_beat = -1;
            let next_idx = s.song.cursor.bar_idx + 1;
            if next_idx >= s.song.bars.len() {
                if s.song.one_shot {
                    s.song.playing = false;
                    s.voices.clear();
                    break;
                }
                s.song.cursor.bar_idx = 0;
            } else {
                s.song.cursor.bar_idx = next_idx;
            }
            s.last_chord_bar = -1;
            s.song.on_bar_boundary();
        }
    }
}

/// Push a rendered buffer as a one-shot preview voice, trimming the
/// backlog so rapid triggers don't pile up. Shared by
/// [`SongEngineHandle::play_note_now`] and [`SongEngineHandle::play_chord_now`].
fn push_oneshot(s: &mut SongEngineInternals, buf: SampleBuffer, id: &str) {
    let start = s.oneshot_clock;
    s.oneshot_voices
        .push(Voice::new(Sound::sample_with_buffer(id, buf), false, start));
    // Don't let stale voices accumulate if something pushes faster than
    // they decay.
    if s.oneshot_voices.len() > 16 {
        let drop_n = s.oneshot_voices.len() - 16;
        s.oneshot_voices.drain(0..drop_n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(song: Song) -> SongEngineInternals {
        SongEngineInternals::new(song, 48_000.0, 1)
    }

    fn chord_song() -> Song {
        let mut song = Song::new();
        song.click_enabled = false;
        song.bars[0].bpm = 300.0;
        song.bars[0].chord_ref = Some(crate::song::ChordRef {
            formula_name: "Major".into(),
            root_freq_hz: 261.63,
            pitches_hz: vec![261.63, 329.63, 392.0],
            label: "C".into(),
        });
        song
    }

    #[test]
    fn callback_miss_is_silent_and_does_not_synthesize_or_strike_late() {
        let mut state = make_state(chord_song());
        state.chord_cache[0] = None;
        state.song.playing = true;
        let mut out = [1.0; 512];
        process_song_buffer(&mut state, &mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(state.song.cursor.sample_in_bar, 512);
        assert!(state.chord_cache[0].is_none());
        // Ready PCM arriving after the boundary must wait for the next measure.
        state.chord_cache = prepare_song_chords(&state.song, 48_000, render_chord);
        process_song_buffer(&mut state, &mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(state.voices.is_empty());
        let to_boundary = 38_400 - state.song.cursor.sample_in_bar;
        process_song_buffer(&mut state, &mut vec![0.0; to_boundary as usize + 512]);
        assert!(!state.voices.is_empty());
        assert_eq!(state.voices[0].start_sample, 38_400);
    }

    #[test]
    fn chord_preparation_keeps_callback_running_and_rejects_obsolete_pcm() {
        use std::sync::mpsc;
        let mut state = make_state(chord_song());
        state.chord_cache[0] = None;
        state.song.playing = true;
        let internals = Arc::new(Mutex::new(state));
        let handle = SongEngineHandle::new(internals.clone());
        let background = handle.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            background.prepare_chords(|params, rate| {
                started_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
                render_chord(params, rate)
            });
        });
        started_rx.recv().unwrap();
        let progressed = match internals.try_lock() {
            Ok(mut state) => {
                process_song_buffer(&mut state, &mut [0.0; 512]);
                assert_eq!(state.song.cursor.sample_in_bar, 512);
                true
            },
            Err(_) => false,
        };
        if !progressed {
            resume_tx.send(()).unwrap();
            worker.join().unwrap();
            panic!("chord synthesis held the callback mutex");
        }
        // Replacement publishes a different ready chord while the old render
        // is still paused. Completing that render must not overwrite it.
        let mut replacement = chord_song();
        replacement.bars[0].chord_ref.as_mut().unwrap().pitches_hz = vec![220.0, 261.63, 329.63];
        handle.set_song(replacement);
        let current = internals.lock().unwrap().chord_cache[0]
            .as_ref()
            .unwrap()
            .buffer
            .clone();
        resume_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(progressed, "chord synthesis held the callback mutex");
        let state = internals.lock().unwrap();
        assert!(Arc::ptr_eq(
            &current.data,
            &state.chord_cache[0].as_ref().unwrap().buffer.data
        ));
    }

    #[test]
    fn slow_song_replacement_cannot_overwrite_a_newer_request() {
        use std::sync::mpsc;
        let internals = Arc::new(Mutex::new(make_state(Song::new())));
        let handle = SongEngineHandle::new(internals.clone());
        let background = handle.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            background.replace_song(chord_song(), |params, rate| {
                started_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
                render_chord(params, rate)
            })
        });
        started_rx.recv().unwrap();
        let mut latest = chord_song();
        latest.name = "Latest request".into();
        latest.bars[0].chord_ref.as_mut().unwrap().pitches_hz = vec![220.0];
        handle.set_song(latest);
        resume_tx.send(()).unwrap();
        worker.join().unwrap();
        let state = internals.lock().unwrap();
        assert_eq!(state.song.name, "Latest request");
        assert!(
            state.chord_cache[0]
                .as_ref()
                .unwrap()
                .key
                .matches(&state.song.bars[0], 48_000)
        );
    }

    #[test]
    fn read_and_cosmetic_edits_reuse_pcm_but_pitch_tempo_and_meter_edits_refresh_it() {
        let internals = Arc::new(Mutex::new(make_state(chord_song())));
        let handle = SongEngineHandle::new(internals.clone());
        let before = internals.lock().unwrap().chord_cache[0]
            .as_ref()
            .unwrap()
            .buffer
            .clone();
        handle.with_song(|song| {
            song.bars[0].label = "Verse".into();
            song.bars[0].length = 4;
            song.bars[0].chord_ref.as_mut().unwrap().label = "CMaj".into();
        });
        handle.with_song(|song| song.playing);
        let unchanged = internals.lock().unwrap().chord_cache[0]
            .as_ref()
            .unwrap()
            .buffer
            .clone();
        assert!(Arc::ptr_eq(&before.data, &unchanged.data));
        handle.with_song(|song| song.bars[0].bpm = 400.0);
        let tempo = internals.lock().unwrap().chord_cache[0]
            .as_ref()
            .unwrap()
            .buffer
            .clone();
        assert_eq!(tempo.len(), 28_800);
        assert!(!Arc::ptr_eq(&before.data, &tempo.data));
        handle.with_song(|song| song.bars[0].time_signature.numerator = 3);
        let meter = internals.lock().unwrap().chord_cache[0]
            .as_ref()
            .unwrap()
            .buffer
            .clone();
        assert_eq!(meter.len(), 24_000); // existing 0.5 second minimum
        handle.with_song(|song| song.bars[0].chord_ref.as_mut().unwrap().pitches_hz[1] = 311.13);
        let state = internals.lock().unwrap();
        let cached = state.chord_cache[0].as_ref().unwrap();
        let expected = render_chord(
            &ChordRender::strum(vec![261.63, 311.13, 392.0], 0.5),
            48_000,
        );
        assert_eq!(cached.buffer.data, expected.data);
        assert!(!Arc::ptr_eq(&meter.data, &cached.buffer.data));
    }

    #[test]
    fn fallback_pitch_and_sample_rate_are_part_of_cache_identity() {
        let mut song = chord_song();
        song.bars[0].chord_ref.as_mut().unwrap().pitches_hz.clear();
        let key = ChordKey::for_bar(&song.bars[0], 48_000).unwrap();
        assert!(key.matches(&song.bars[0], 48_000));
        assert!(!key.matches(&song.bars[0], 44_100));
        song.bars[0].chord_ref.as_mut().unwrap().root_freq_hz = 220.0;
        assert!(!key.matches(&song.bars[0], 48_000));
        song.bars[0].chord_ref = None;
        assert!(!key.matches(&song.bars[0], 48_000));
    }

    #[test]
    fn preparation_preserves_count_in_and_multi_measure_chord_boundaries() {
        let mut song = chord_song();
        song.click_enabled = true;
        song.bars[0].bpm = 600.0;
        song.bars[0].time_signature.numerator = 2;
        song.bars[0].length = 2;
        let internals = Arc::new(Mutex::new(SongEngineInternals::new(song, 1_000.0, 1)));
        let handle = SongEngineHandle::new(internals.clone());
        handle.play();
        let mut state = internals.lock().unwrap();
        assert_eq!(state.count_in_remaining, 200);
        process_song_buffer(&mut state, &mut [0.0; 200]);
        assert_eq!(state.song.cursor.sample_in_bar, 0);
        assert_eq!(state.count_in_remaining, 0);
        process_song_buffer(&mut state, &mut [0.0; 1]);
        assert!(
            state
                .voices
                .iter()
                .any(|v| v.start_sample == 200 && matches!(v.sound, Sound::Sample { .. }))
        );
        process_song_buffer(&mut state, &mut [0.0; 200]);
        assert!(
            state
                .voices
                .iter()
                .any(|v| v.start_sample == 400 && matches!(v.sound, Sound::Sample { .. }))
        );
        assert_eq!(state.last_chord_measure, 1);
    }

    #[test]
    fn silent_output_when_not_playing() {
        let mut state = make_state(Song::new());
        let mut buf = vec![1.0_f32; 256];
        process_song_buffer(&mut state, &mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn click_fires_on_first_frame_when_playing() {
        let mut state = make_state(Song::new());
        state.song.playing = true;
        let mut buf = vec![0.0_f32; 2 * 48_000]; // 1 second mono
        process_song_buffer(&mut state, &mut buf);
        let max_abs = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(max_abs > 0.0, "expected audible click");
    }

    #[test]
    fn cursor_advances_with_processed_frames() {
        let mut state = make_state(Song::new());
        state.song.playing = true;
        let frame_count = 24_000;
        let mut buf = vec![0.0_f32; frame_count];
        process_song_buffer(&mut state, &mut buf);
        // 24000 samples at 48 kHz = 0.5 s. 120 BPM 4/4 = 0.5 s/beat,
        // so we should still be in bar 0 with cursor advanced ~halfway
        // through the first beat segment (bar duration = 2s).
        assert_eq!(state.song.cursor.bar_idx, 0);
        assert!(
            state.song.cursor.sample_in_bar >= 24_000,
            "cursor: {}",
            state.song.cursor.sample_in_bar
        );
    }

    #[test]
    fn chord_bar_produces_extra_audio() {
        // Compare a click-only bar to a click + chord bar in the same
        // time window. The chord version should have a higher peak.
        let mut click_only = make_state(Song::new());
        click_only.song.playing = true;

        let mut with_chord = make_state({
            let mut s = Song::new();
            s.bar_mut(0).unwrap().chord_ref = Some(crate::song::ChordRef {
                formula_name: "Major".into(),
                root_freq_hz: 261.63,
                pitches_hz: vec![261.63, 329.63, 392.00],
                label: "C".into(),
            });
            s
        });
        with_chord.song.playing = true;

        let mut click_buf = vec![0.0_f32; 4800];
        let mut chord_buf = vec![0.0_f32; 4800];
        process_song_buffer(&mut click_only, &mut click_buf);
        process_song_buffer(&mut with_chord, &mut chord_buf);

        let click_peak = click_buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        let chord_peak = chord_buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            chord_peak > click_peak,
            "chord peak {chord_peak} should exceed click peak {click_peak}"
        );
    }

    #[test]
    fn set_song_replaces_arrangement_cleanly() {
        let internals = Arc::new(Mutex::new(SongEngineInternals::new(
            Song::new(),
            48_000.0,
            1,
        )));
        let handle = SongEngineHandle::new(Arc::clone(&internals));

        let mut new_song = Song::new();
        new_song.add_bar();
        new_song.add_bar();
        new_song.bars[1].label = "Bridge".into();
        handle.set_song(new_song);

        let s = internals.lock().unwrap();
        assert_eq!(s.song.len(), 3);
        assert_eq!(s.song.bars[1].label, "Bridge");
        assert!(!s.song.playing);
        assert_eq!(s.chord_cache.len(), 3);
    }

    #[test]
    fn with_song_lets_caller_edit_under_lock() {
        let internals = Arc::new(Mutex::new(SongEngineInternals::new(
            Song::new(),
            48_000.0,
            1,
        )));
        let handle = SongEngineHandle::new(Arc::clone(&internals));
        handle.with_song(|s| {
            s.add_bar();
            s.bar_mut(1).unwrap().bpm = 90.0;
        });
        assert_eq!(internals.lock().unwrap().song.len(), 2);
        assert_eq!(internals.lock().unwrap().song.bars[1].bpm, 90.0);
        // Resync should have grown the cache.
        assert_eq!(internals.lock().unwrap().chord_cache.len(), 2);
    }

    #[test]
    fn one_shot_song_stops_at_end() {
        let mut song = Song::new();
        song.add_bar(); // 2 bars
        song.one_shot = true;
        let mut state = make_state(song);
        state.song.playing = true;
        // Each bar at 120 BPM 4/4 = 2s = 96 000 samples. Total = 192 000.
        let total_samples = (state.song.bars[0].duration_samples(48_000.0)
            + state.song.bars[1].duration_samples(48_000.0)) as usize;
        let mut buf = vec![0.0_f32; total_samples + 1000];
        process_song_buffer(&mut state, &mut buf);
        assert!(!state.song.playing, "one-shot should stop at end");
    }

    #[test]
    fn preview_synthesis_does_not_block_the_callback_and_queues_at_current_clock() {
        use std::sync::mpsc;
        let internals = Arc::new(Mutex::new(SongEngineInternals::new(
            Song::new(),
            48_000.0,
            1,
        )));
        let (started_tx, started_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let handle =
            SongEngineHandle::with_preview_renderer(internals.clone(), move |params, rate, _| {
                started_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
                Some(render_chord(params, rate))
            });
        // Returns even though the worker cannot finish until we release it.
        handle.play_note_now(261.63, 0.1);
        started_rx.recv().unwrap();
        // The renderer is deliberately still running. The mixer must acquire
        // state and advance audio immediately, without timing-based assertions.
        let progressed = match internals.try_lock() {
            Ok(mut state) => {
                let mut out = [0.0; 512];
                process_song_buffer(&mut state, &mut out);
                assert_eq!(state.oneshot_clock, 512);
                assert!(state.oneshot_voices.is_empty());
                true
            },
            Err(_) => false,
        };
        resume_tx.send(()).unwrap();
        handle.wait_preview();
        assert!(progressed, "synthesis held the callback's mutex");
        let mut state = internals.lock().unwrap();
        assert_eq!(state.oneshot_voices[0].start_sample, 512);
        let mut out = [0.0; 512];
        process_song_buffer(&mut state, &mut out);
        assert!(out.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    fn preview_burst_keeps_only_latest_pending_and_rejects_stale_completion() {
        use std::sync::mpsc;
        let internals = Arc::new(Mutex::new(make_state(Song::new())));
        let (started_tx, started_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let calls = seen.clone();
        let mut first = true;
        let handle =
            SongEngineHandle::with_preview_renderer(internals.clone(), move |p, rate, _| {
                calls.lock().unwrap().push(p.pitches_hz[0]);
                if first {
                    first = false;
                    started_tx.send(()).unwrap();
                    resume_rx.recv().unwrap();
                }
                // Deliberately ignore cancellation: publication must still reject it.
                Some(render_chord(p, rate))
            });
        handle.play_note_now(220.0, 0.1);
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        for freq in [330.0, 440.0, 550.0] {
            handle.play_chord_now(&[freq], 0.1, 0.0);
        }
        let pending = handle
            .preview
            .mailbox
            .queue
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .params
            .pitches_hz
            .clone();
        resume_tx.send(()).unwrap();
        handle.wait_preview();
        assert_eq!(pending, [550.0]);
        assert_eq!(*seen.lock().unwrap(), [220.0, 550.0]);
        let mut state = internals.lock().unwrap();
        assert_eq!(state.oneshot_voices.len(), 1);
        let expected = render_chord(&ChordRender::block(vec![550.0], 0.1), 48_000);
        let mut output = [0.0; 512];
        process_song_buffer(&mut state, &mut output);
        for (idx, sample) in output.iter().enumerate() {
            assert_eq!(*sample, expected.data[idx] * state.output_gain);
        }
    }

    #[test]
    fn transport_actions_cancel_running_and_pending_preview() {
        use std::sync::mpsc;
        for action in 0..7 {
            let internals = Arc::new(Mutex::new(make_state(Song::new())));
            let (started_tx, started_rx) = mpsc::channel();
            let (resume_tx, resume_rx) = mpsc::channel();
            let handle =
                SongEngineHandle::with_preview_renderer(internals.clone(), move |p, rate, _| {
                    started_tx.send(()).unwrap();
                    resume_rx.recv().unwrap();
                    Some(render_chord(p, rate))
                });
            handle.play_note_now(220.0, 0.1);
            started_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            handle.play_note_now(330.0, 0.1);
            match action {
                0 => handle.stop(),
                1 => handle.rewind(),
                2 => handle.play(),
                3 => handle.set_song(Song::new()),
                4 => handle.queue(PendingChange::SetPlaying(true)),
                5 => handle.queue(PendingChange::SeekTo { bar_idx: 0 }),
                _ => handle.cancel_preview(),
            }
            let cleared = handle
                .preview
                .mailbox
                .queue
                .lock()
                .unwrap()
                .pending
                .is_none();
            resume_tx.send(()).unwrap();
            handle.wait_preview();
            assert!(cleared, "action {action} retained a pending preview");
            assert!(
                internals.lock().unwrap().oneshot_voices.is_empty(),
                "action {action} published stale PCM"
            );
        }
    }

    #[test]
    fn step_notes_keep_envelopes_but_hear_replaces_and_playing_rejects_requests() {
        let internals = Arc::new(Mutex::new(make_state(Song::new())));
        let handle = SongEngineHandle::new(internals.clone());
        handle.play_note_now(220.0, 0.1);
        handle.wait_preview();
        handle.play_note_now(330.0, 0.1);
        handle.wait_preview();
        assert_eq!(internals.lock().unwrap().oneshot_voices.len(), 2);
        handle.play_chord_now(&[440.0], 0.1, 0.0);
        handle.wait_preview();
        assert_eq!(internals.lock().unwrap().oneshot_voices.len(), 1);
        handle.play();
        handle.play_note_now(550.0, 0.1);
        handle.play_chord_now(&[660.0], 0.1, 0.0);
        assert!(!handle.preview_busy());
        handle.stop();
        assert!(internals.lock().unwrap().oneshot_voices.is_empty());
    }

    #[test]
    fn shutdown_cancels_worker_and_retained_handle_cannot_restart_it() {
        use std::sync::mpsc;
        let internals = Arc::new(Mutex::new(make_state(Song::new())));
        let (started_tx, started_rx) = mpsc::channel();
        let (aborted_tx, aborted_rx) = mpsc::channel();
        let handle =
            SongEngineHandle::with_preview_renderer(internals.clone(), move |_, _, cancelled| {
                started_tx.send(()).unwrap();
                while !cancelled() {
                    std::thread::yield_now();
                }
                aborted_tx.send(()).unwrap();
                None
            });
        handle.play_note_now(220.0, 0.1);
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        handle.preview.shutdown();
        aborted_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        handle.play_note_now(330.0, 0.1);
        assert!(!handle.preview_busy());
        assert!(internals.lock().unwrap().oneshot_voices.is_empty());
    }

    #[test]
    fn play_chord_now_sounds_while_stopped() {
        let internals = Arc::new(Mutex::new(SongEngineInternals::new(
            Song::new(),
            48_000.0,
            1,
        )));
        let handle = SongEngineHandle::new(Arc::clone(&internals));
        // A C-major triad preview.
        handle.play_chord_now(&[261.63, 329.63, 392.00], 0.5, 18.0);
        handle.wait_preview();
        assert!(
            !internals.lock().unwrap().oneshot_voices.is_empty(),
            "preview should queue a one-shot voice"
        );
        // The song is stopped, so the one-shot mixes on its own clock.
        let mut s = internals.lock().unwrap();
        let mut buf = vec![0.0_f32; 2048];
        process_song_buffer(&mut s, &mut buf);
        let peak = buf.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        assert!(peak > 0.0, "preview chord should sound while stopped");
    }

    #[test]
    fn play_chord_now_ignores_empty_and_silent_pitches() {
        let internals = Arc::new(Mutex::new(SongEngineInternals::new(
            Song::new(),
            48_000.0,
            1,
        )));
        let handle = SongEngineHandle::new(Arc::clone(&internals));
        handle.play_chord_now(&[], 0.5, 0.0);
        handle.play_chord_now(&[0.0, -20.0], 0.5, 0.0);
        assert!(
            internals.lock().unwrap().oneshot_voices.is_empty(),
            "no voiceable pitches → no voice"
        );
    }

    #[test]
    fn loop_wraps_to_first_bar() {
        let mut song = Song::new();
        song.add_bar(); // 2 bars
        let mut state = make_state(song);
        state.song.playing = true;
        let total_samples = (state.song.bars[0].duration_samples(48_000.0)
            + state.song.bars[1].duration_samples(48_000.0)) as usize;
        let mut buf = vec![0.0_f32; total_samples + 1000];
        process_song_buffer(&mut state, &mut buf);
        assert!(state.song.playing);
        assert_eq!(state.song.cursor.bar_idx, 0);
        assert!(state.song.cursor.sample_in_bar < 5000);
    }
}
