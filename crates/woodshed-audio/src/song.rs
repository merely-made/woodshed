//! Song-mode arrangement: a multi-bar sequence with per-bar settings.
//!
//! A [`Song`] is an ordered list of [`Bar`]s. Each bar carries its own
//! BPM, time signature, optional recorded audio, and optional chord
//! reference. Songs are the upgrade path from the single-bar
//! [`crate::Looper`]: a one-bar song with a recorded audio buffer *is*
//! a simple looper.
//!
//! # What this module is, what it isn't
//!
//! **Is**: a pure-data model with operations (add, remove, copy/paste,
//! seek, advance) and tests. Output rendering and UI gestures live elsewhere.
//! The single output runtime lowers this control-side document into prepared
//! segments; it does not make this document a second product authority.
//!
//! **Isn't**: a multi-track DAW. Each bar holds at most one audio
//! buffer (the practice-recording layer). Want chords too? Use the
//! `chord_ref` field — the engine resolves it via
//! [`crate::chord_audio`] and mixes alongside the audio buffer.
//!
//! # Pending-change model (SR-16 pattern)
//!
//! Mid-song settings changes are queued via [`Song::queue`] and
//! applied at the next bar boundary by [`Song::on_bar_boundary`].
//! This matches how the Alesis SR-16 drum machine handles tempo and
//! pattern changes — you press the button now, the change lands when
//! musically appropriate.
//!
//! # Serde
//!
//! [`Song`], [`Bar`], and [`ChordRef`] all serialize. Audio buffers
//! follow the same convention as [`crate::Sound::Sample`]: serialized
//! songs carry buffer metadata (sample rate, length) but the PCM
//! itself is `#[serde(skip)]` and rehydrated by a runtime asset bank
//! or — for songs saved as standalone files — embedded in a separate
//! companion `.wav` and re-attached on load.

use serde::{Deserialize, Serialize};

use crate::sequencer::{Subdivision, TimeSignature};
use crate::sound::SampleBuffer;

/// Reference to a chord by formula name + root frequency. Audio
/// crates don't depend on the woodshedding theory crate, so callers
/// translate (chord formula, root pitch) → (formula_name string,
/// root frequency in Hz) before stashing in a [`Bar`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChordRef {
    /// Catalog key for the chord formula (e.g. `"Major"`, `"Dominant 7"`).
    pub formula_name: String,
    /// Root pitch as a frequency in Hz. The renderer adds intervals
    /// on top to produce the chord's pitches.
    pub root_freq_hz: f32,
    /// Pre-computed chord-tone frequencies, oldest-to-highest. Stored
    /// so the audio layer doesn't need theory dependencies. Empty =
    /// only the root sounds.
    pub pitches_hz: Vec<f32>,
    /// User-facing label for display ("Em7", "Cmaj9"). Cosmetic; the
    /// engine doesn't read this.
    pub label: String,
}

/// One bar in a [`Song`]. Holds the bar's own tempo + time sig (so
/// per-bar tempo changes are first-class), an optional recorded audio
/// buffer, and an optional chord reference for chord-card display +
/// audio.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bar {
    /// Tempo for this bar, in BPM.
    pub bpm: f32,
    pub time_signature: TimeSignature,
    pub subdivision: Subdivision,
    /// Optional one-shot chord that plays at the start of this bar.
    /// `None` = silent (just the click + any recorded audio).
    pub chord_ref: Option<ChordRef>,
    /// Ordered backing events for this bar. Transient host realization; saved
    /// chord references remain the durable backing representation.
    #[serde(skip)]
    pub backing_phrase: Option<crate::backing_phrase::BackingPhrase>,
    /// Recorded audio for this bar (the practice-loop layer). Skipped
    /// from serde — see module docs.
    #[serde(skip)]
    pub audio_buffer: Option<SampleBuffer>,
    /// Cosmetic label ("Verse 1", "Bridge"). Optional. Doubles as the
    /// section marker in the timeline UI: a non-empty label opens a
    /// section band that runs until the next labeled bar.
    pub label: String,
    /// How many measures this bar spans (≥ 1). A length-4 bar is one
    /// timeline cell holding four measures of the same tempo / meter /
    /// chord — the looper-friendly "this block repeats for N bars"
    /// unit. Playback walks all `length` measures before advancing to
    /// the next [`Bar`].
    #[serde(default = "default_bar_length")]
    pub length: u8,
    /// Compatibility playback for an existing take captured before per-beat
    /// rounding. New songs/captures are canonical; only capture restoration
    /// selects this policy, and Clear removes it without changing old PCM.
    #[serde(default)]
    pub legacy_clock: bool,
    /// Transient absolute segment duration. None retains the exact saved-Song
    /// and Bars clocks. Explicit seconds cannot use legacy capture timing.
    #[serde(skip)]
    pub duration_seconds: Option<f64>,
}

/// Serde default for [`Bar::length`] — old saves predate the field and
/// must read back as single-measure bars, not zero-length ones.
fn default_bar_length() -> u8 {
    1
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            time_signature: TimeSignature::default(),
            subdivision: Subdivision::QUARTER,
            chord_ref: None,
            backing_phrase: None,
            audio_buffer: None,
            label: String::new(),
            length: 1,
            legacy_clock: false,
            duration_seconds: None,
        }
    }
}

impl Bar {
    /// Musical measure duration. PCM clocks use `duration_samples` instead.
    pub fn measure_secs(&self) -> f32 {
        let quarter_seconds = 60.0 / self.bpm.max(1.0);
        let beat_seconds = if self.legacy_clock {
            quarter_seconds
        } else {
            quarter_seconds * 4.0 / self.time_signature.denominator.max(1) as f32
        };
        self.time_signature.numerator.max(1) as f32 * beat_seconds
    }

    /// Preserve the legacy chord-envelope calculation's operation order too.
    pub(crate) fn chord_measure_secs(&self) -> f32 {
        if self.legacy_clock {
            self.time_signature.numerator.max(1) as f32 * 60.0 / self.bpm.max(1.0)
        } else {
            self.measure_secs()
        }
    }

    pub fn duration_secs(&self) -> f32 {
        if let Some(seconds) = self.duration_seconds {
            return seconds as f32;
        }
        self.measure_secs() * self.length.max(1) as f32
    }

    /// Checked admission shared by backing, capture preparation and export.
    /// Absolute durations must be at least 0.5 seconds; event units must match.
    /// The click grid remains independent of an absolute segment endpoint.
    pub fn checked_duration_frames(&self, rate: u32) -> Result<u64, &'static str> {
        let duration = if let Some(seconds) = self.duration_seconds {
            if seconds < 0.5 {
                return Err("backing duration seconds must be at least 0.5");
            }
            if self.legacy_clock {
                return Err("absolute seconds cannot use the legacy capture clock");
            }
            audio_primitives::timing::seconds_to_frames(seconds, rate)?
        } else {
            self.duration_samples(rate as f32).max(1)
        };
        if let Some(phrase) = &self.backing_phrase {
            for event in &phrase.events {
                let seconds_event =
                    matches!(event.time, audio_primitives::timing::EventTime::Seconds(_));
                if seconds_event != self.duration_seconds.is_some() {
                    return Err("backing event timebase must match its segment duration");
                }
            }
        }
        Ok(duration)
    }

    /// Frames per beat: canonical clocks round once per denominator-aware
    /// beat; restored legacy captures keep the original truncation.
    pub fn beat_frames(&self, rate: u32) -> u64 {
        if self.legacy_clock {
            ((60.0 / self.bpm.max(1.0)) * rate as f32) as u64
        } else {
            audio_primitives::frames_per_beat(rate, self.bpm, self.time_signature.denominator)
                as u64
        }
        .max(1)
    }

    pub fn measure_frames(&self, rate: u32) -> u64 {
        self.beat_frames(rate)
            .saturating_mul(self.time_signature.numerator.max(1) as u64)
    }

    pub fn count_in_frames(&self, rate: u32) -> u64 {
        if self.legacy_clock {
            (self.time_signature.numerator.max(1) as f32 * (60.0 / self.bpm.max(1.0)) * rate as f32)
                as u64
        } else {
            self.measure_frames(rate)
        }
    }

    /// Canonical measures contain an integer number of rounded beats. Existing
    /// capture playback retains the exact pre-change whole-block expression.
    pub fn duration_samples(&self, sample_rate_hz: f32) -> u64 {
        if let Some(seconds) = self.duration_seconds {
            // Infallible document queries return zero for invalid conversion;
            // admission uses checked_duration_frames for the product minimum,
            // legacy policy and event units before allocation.
            return audio_primitives::timing::seconds_to_frames(seconds, sample_rate_hz as u32)
                .unwrap_or(0);
        }
        if self.legacy_clock {
            (self.duration_secs() * sample_rate_hz).round() as u64
        } else {
            (audio_primitives::frames_per_bar(
                sample_rate_hz as u32,
                self.bpm,
                self.time_signature.numerator.max(1),
                self.time_signature.denominator,
            ) as u64)
                .saturating_mul(self.length.max(1) as u64)
        }
    }

    /// Construct with just BPM + time-signature (the common case for
    /// "add a new bar matching the current song settings").
    pub fn with_tempo(bpm: f32, time_signature: TimeSignature) -> Self {
        Self {
            bpm,
            time_signature,
            ..Self::default()
        }
    }

    /// Convenience: produce a deep copy. `clone()` already does this
    /// (SampleBuffer's `data` is Arc-shared so the audio is shared, not
    /// duplicated); kept as a named operation so app code can spell
    /// out copy/paste semantics.
    pub fn duplicate(&self) -> Self {
        self.clone()
    }
}

/// A queued mid-song change waiting to land at the next bar boundary.
/// Matches the SR-16 transport model: press the button now, the
/// change applies when musically appropriate.
#[derive(Clone, Debug, PartialEq)]
pub enum PendingChange {
    /// Start capturing input audio into the buffer at this bar index
    /// when playback reaches it.
    StartRecording { bar_idx: usize },
    /// Stop capturing input audio.
    StopRecording,
    /// Jump playback to a specific bar.
    SeekTo { bar_idx: usize },
    /// Toggle play state at the boundary.
    SetPlaying(bool),
}

/// Where playback currently is. `bar_idx` indexes into [`Song::bars`];
/// `sample_in_bar` advances every output frame and wraps when it
/// reaches the bar's duration (at which point `bar_idx` advances).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongCursor {
    pub bar_idx: usize,
    pub sample_in_bar: u64,
}

/// Errors from bar-manipulation operations.
#[derive(Debug, PartialEq)]
pub enum SongError {
    BarIndexOutOfRange {
        idx: usize,
        len: usize,
    },
    /// Tried to remove the last remaining bar — songs must have at
    /// least one bar.
    CannotRemoveLastBar,
}

impl std::fmt::Display for SongError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BarIndexOutOfRange { idx, len } => {
                write!(f, "bar index {idx} out of range (len {len})")
            },
            Self::CannotRemoveLastBar => {
                f.write_str("cannot remove the last bar; songs must have ≥1 bar")
            },
        }
    }
}

impl std::error::Error for SongError {}

/// A multi-bar arrangement with playback cursor and queued changes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub bars: Vec<Bar>,
    /// When true, playback stops at the end of the last bar instead
    /// of wrapping. Default false (= loop the whole song).
    pub one_shot: bool,
    pub cursor: SongCursor,
    /// Currently playing? Stored on the song so the engine and UI
    /// agree on transport state.
    pub playing: bool,
    /// Currently recording into the bar at `cursor.bar_idx`?
    pub recording: bool,
    /// Changes waiting on a bar boundary.
    #[serde(skip)]
    pub pending: Vec<PendingChange>,
    /// User-supplied song name for the song list / file picker.
    pub name: String,
    /// Whether the metronome click sounds during playback. Default
    /// true; toggled from the transport row. Legacy saves predate the
    /// field and read back as `true`.
    #[serde(default = "default_true")]
    pub click_enabled: bool,
    /// Recording mode: `false` (default) overdubs onto the bar's
    /// existing audio (sum + soft-clamp); `true` replaces it
    /// per-sample as the cursor passes, so a new take overwrites the
    /// old one across a full loop pass.
    #[serde(default)]
    pub record_replace: bool,
}

/// Serde default for boolean fields that should be `true` when absent
/// from an older save.
fn default_true() -> bool {
    true
}

impl Default for Song {
    fn default() -> Self {
        Self::new()
    }
}

impl Song {
    /// A fresh song with one empty bar at 120 BPM 4/4.
    pub fn new() -> Self {
        Self {
            bars: vec![Bar::default()],
            one_shot: false,
            cursor: SongCursor::default(),
            playing: false,
            recording: false,
            pending: Vec::new(),
            name: String::from("Untitled Song"),
            click_enabled: true,
            record_replace: false,
        }
    }

    /// Number of bars in the song. Always ≥ 1.
    pub fn len(&self) -> usize {
        self.bars.len()
    }

    pub fn is_empty(&self) -> bool {
        // The song itself always has at least one bar (enforced by
        // remove guards). `is_empty` mirrors the standard collection
        // method for API consistency but will always return false on
        // a well-formed song.
        self.bars.is_empty()
    }

    /// Borrow the bar at the cursor.
    pub fn current_bar(&self) -> &Bar {
        &self.bars[self.cursor.bar_idx]
    }

    /// Mutable borrow of the bar at the cursor.
    pub fn current_bar_mut(&mut self) -> &mut Bar {
        &mut self.bars[self.cursor.bar_idx]
    }

    /// Borrow a specific bar.
    pub fn bar(&self, idx: usize) -> Result<&Bar, SongError> {
        self.bars.get(idx).ok_or(SongError::BarIndexOutOfRange {
            idx,
            len: self.bars.len(),
        })
    }

    pub fn bar_mut(&mut self, idx: usize) -> Result<&mut Bar, SongError> {
        let len = self.bars.len();
        self.bars
            .get_mut(idx)
            .ok_or(SongError::BarIndexOutOfRange { idx, len })
    }

    /// Append a new bar, returning its index. Defaults match the
    /// current cursor-bar's tempo + time signature so adding a bar
    /// in the middle of working doesn't snap settings back to 120/4.
    pub fn add_bar(&mut self) -> usize {
        let template = self.current_bar().clone();
        let mut new = template;
        // Don't carry forward the recorded audio when adding a new
        // bar — that would be surprising. Same for any pending state.
        new.audio_buffer = None;
        new.legacy_clock = false;
        new.label.clear();
        self.bars.push(new);
        self.bars.len() - 1
    }

    /// Insert `bar` at position `idx`, shifting existing bars right.
    /// Cursor stays on its current bar (if its index was ≥ idx, it
    /// shifts right too).
    pub fn insert_bar(&mut self, idx: usize, bar: Bar) -> Result<(), SongError> {
        if idx > self.bars.len() {
            return Err(SongError::BarIndexOutOfRange {
                idx,
                len: self.bars.len(),
            });
        }
        self.bars.insert(idx, bar);
        if self.cursor.bar_idx >= idx {
            self.cursor.bar_idx += 1;
        }
        Ok(())
    }

    /// Remove the bar at `idx`. The cursor adjusts: if it was on the
    /// removed bar (or after), it shifts left by one (clamped to the
    /// new length).
    pub fn remove_bar(&mut self, idx: usize) -> Result<Bar, SongError> {
        if self.bars.len() == 1 {
            return Err(SongError::CannotRemoveLastBar);
        }
        if idx >= self.bars.len() {
            return Err(SongError::BarIndexOutOfRange {
                idx,
                len: self.bars.len(),
            });
        }
        let removed = self.bars.remove(idx);
        if self.cursor.bar_idx >= idx && self.cursor.bar_idx > 0 {
            self.cursor.bar_idx -= 1;
        }
        if self.cursor.bar_idx >= self.bars.len() {
            self.cursor.bar_idx = self.bars.len() - 1;
        }
        Ok(removed)
    }

    /// Return a clone of the bar at `idx` (your clipboard).
    pub fn copy_bar(&self, idx: usize) -> Result<Bar, SongError> {
        self.bar(idx).map(|b| b.duplicate())
    }

    /// Paste `bar` into position `idx`, replacing whatever was there.
    /// The bar's audio buffer Arc is shared with the source — paste
    /// is cheap regardless of buffer size.
    pub fn paste_bar(&mut self, idx: usize, bar: Bar) -> Result<(), SongError> {
        let len = self.bars.len();
        let slot = self
            .bars
            .get_mut(idx)
            .ok_or(SongError::BarIndexOutOfRange { idx, len })?;
        *slot = bar;
        Ok(())
    }

    /// Move a bar from one position to another. Useful for drag-to-
    /// reorder gestures.
    pub fn move_bar(&mut self, from: usize, to: usize) -> Result<(), SongError> {
        let len = self.bars.len();
        if from >= len {
            return Err(SongError::BarIndexOutOfRange { idx: from, len });
        }
        if to >= len {
            return Err(SongError::BarIndexOutOfRange { idx: to, len });
        }
        if from == to {
            return Ok(());
        }
        let bar = self.bars.remove(from);
        self.bars.insert(to, bar);
        Ok(())
    }

    /// Queue a pending change to apply at the next bar boundary.
    pub fn queue(&mut self, change: PendingChange) {
        self.pending.push(change);
    }

    /// Cancel all queued changes without applying them.
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }

    /// Apply queued changes; called when the playback cursor reaches
    /// a bar boundary. Returns the number of changes applied.
    ///
    /// This data-model helper consumes the pending vector on the control side.
    /// The output processor uses its own bounded boundary commands rather than
    /// moving or destroying this document's allocations on the audio thread.
    pub fn on_bar_boundary(&mut self) -> usize {
        let n = self.pending.len();
        let changes = std::mem::take(&mut self.pending);
        for change in changes {
            match change {
                PendingChange::StartRecording { bar_idx } => {
                    self.recording = self.cursor.bar_idx == bar_idx;
                },
                PendingChange::StopRecording => {
                    self.recording = false;
                },
                PendingChange::SeekTo { bar_idx } => {
                    if bar_idx < self.bars.len() {
                        self.cursor.bar_idx = bar_idx;
                        self.cursor.sample_in_bar = 0;
                    }
                },
                PendingChange::SetPlaying(p) => {
                    self.playing = p;
                },
            }
        }
        n
    }

    /// Advance the cursor by `frames` samples at `sample_rate_hz`.
    /// If the cursor crosses a bar boundary, applies queued changes
    /// and (for the song's `one_shot` mode) stops at the final bar.
    /// Returns true iff at least one bar boundary was crossed.
    pub fn advance(&mut self, frames: u64, sample_rate_hz: f32) -> bool {
        if !self.playing || frames == 0 {
            return false;
        }
        let mut remaining = frames;
        let mut crossed = false;
        while remaining > 0 {
            let bar_len = self.current_bar().duration_samples(sample_rate_hz);
            if bar_len == 0 {
                break;
            }
            let until_boundary = bar_len.saturating_sub(self.cursor.sample_in_bar);
            if remaining < until_boundary {
                self.cursor.sample_in_bar += remaining;
                break;
            }
            // Cross the boundary.
            remaining -= until_boundary;
            crossed = true;
            self.cursor.sample_in_bar = 0;
            let next_idx = self.cursor.bar_idx + 1;
            if next_idx >= self.bars.len() {
                if self.one_shot {
                    self.playing = false;
                    break;
                }
                self.cursor.bar_idx = 0;
            } else {
                self.cursor.bar_idx = next_idx;
            }
            self.on_bar_boundary();
        }
        crossed
    }

    /// Total song duration in seconds. Sum of all bar durations.
    pub fn duration_secs(&self) -> f32 {
        self.bars.iter().map(|b| b.duration_secs()).sum()
    }

    /// Attach a freshly-rendered audio buffer to the bar at `idx`.
    /// Replaces any existing buffer there.
    pub fn attach_buffer(&mut self, idx: usize, buffer: SampleBuffer) -> Result<(), SongError> {
        let bar = self.bar_mut(idx)?;
        bar.audio_buffer = Some(buffer);
        bar.legacy_clock = false;
        Ok(())
    }

    /// Detach (and return) the audio buffer on bar `idx`, leaving the
    /// bar audio-empty. Useful for "clear loop" gestures.
    pub fn detach_buffer(&mut self, idx: usize) -> Result<Option<SampleBuffer>, SongError> {
        let bar = self.bar_mut(idx)?;
        bar.legacy_clock = false;
        Ok(bar.audio_buffer.take())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod clock_tests;
