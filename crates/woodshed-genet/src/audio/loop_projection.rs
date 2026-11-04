//! Device-free projection and capture custody used by the desktop audio seam.

use std::collections::{BTreeMap, BTreeSet};

use woodshed_audio::{
    BackingEvent, BackingPhrase, Bar, ChordRef, PendingChange, SampleBuffer, Song, Subdivision,
    TimeSignature,
};
use woodshed_core::loop_plan::{LoopPlan, LoopSegment, LoopSegmentId};

/// Completed Set/Card PCM and the exact clock under which it was recorded.
/// Cloning this snapshot shares the immutable samples; legacy saved-song
/// compatibility buffers never enter this custody boundary.
#[derive(Clone)]
pub(crate) struct CaptureSnapshot {
    pub id: LoopSegmentId,
    pub bpm: f32,
    pub beats: u8,
    pub bars: u8,
    pub seconds: Option<f64>,
    pub buffer: SampleBuffer,
    pub legacy_clock: bool,
}

impl CaptureSnapshot {
    pub(crate) fn clock_field(&self) -> Option<String> {
        if self.seconds.is_some() {
            Some(woodshed_core::loop_capture::SECONDS_CLOCK.to_string())
        } else {
            (!self.legacy_clock).then(|| woodshed_core::loop_capture::PER_BEAT_CLOCK.to_string())
        }
    }
}

const MAX_STORED_CAPTURES: usize = 256;
const MAX_STORED_FRAMES: usize = 16_777_216;

#[derive(Clone)]
struct Capture {
    buffer: SampleBuffer,
    legacy_clock: bool,
    /// Preserve the clock under which the PCM was captured. A Card clock
    /// change requires an explicit clear rather than rewriting that take.
    bpm: f32,
    beats: u8,
    bars: u8,
    seconds: Option<f64>,
}

impl Capture {
    fn clock_matches(&self, segment: &LoopSegment) -> bool {
        self.bpm == segment.bpm
            && self.beats == segment.beats
            && self.bars == segment.bars
            && self.seconds == segment.seconds
    }
}

#[derive(Default)]
pub(super) struct LoopCaptures {
    pub(super) plan: Option<LoopPlan>,
    cards: BTreeMap<LoopSegmentId, Capture>,
    /// Saved backing retains its existing index-based compatibility semantics.
    pub(super) saved_song: Vec<Option<SampleBuffer>>,
}

impl LoopCaptures {
    pub(super) fn snapshot(&self) -> Vec<CaptureSnapshot> {
        self.cards
            .iter()
            .map(|(&id, capture)| CaptureSnapshot {
                legacy_clock: capture.legacy_clock,
                id,
                bpm: capture.bpm,
                beats: capture.beats,
                bars: capture.bars,
                seconds: capture.seconds,
                buffer: capture.buffer.clone(),
            })
            .collect()
    }

    /// Validate the complete incoming bank before replacing any completed
    /// take. Output admission still checks its actual device rate separately.
    pub(super) fn restore(&mut self, takes: Vec<CaptureSnapshot>) -> Result<(), String> {
        if takes.len() > MAX_STORED_CAPTURES {
            return Err("Stored Set captures exceed the 256-take bound.".into());
        }
        let mut frames = 0usize;
        let mut validated = BTreeMap::new();
        for take in takes {
            if take.id.set.0 == 0
                || take.id.card.0 == 0
                || validated.contains_key(&take.id)
                || !take.bpm.is_finite()
                || take.bpm < 1.0
                || !(1..=16).contains(&take.beats)
                || !valid_hold(take.bars, take.seconds)
                || (take.seconds.is_some() && take.legacy_clock)
                || !(1000..=384_000).contains(&take.buffer.sample_rate_hz)
            {
                return Err("Stored Set capture has an invalid or duplicate identity, clock or sample rate.".into());
            }
            frames = frames
                .checked_add(take.buffer.len())
                .filter(|&frames| frames <= MAX_STORED_FRAMES)
                .ok_or("Stored Set captures exceed the 16777216-frame bound.")?;
            let expected = Bar {
                legacy_clock: take.legacy_clock,
                bpm: take.bpm,
                time_signature: TimeSignature::new(take.beats, 4),
                length: if take.seconds.is_some() { 1 } else { take.bars },
                duration_seconds: take.seconds,
                ..Bar::default()
            }
            .checked_duration_frames(take.buffer.sample_rate_hz)
            .map_err(|error| error.to_string())?;
            if expected == 0 || expected != take.buffer.len() as u64 {
                return Err(
                    "Stored Set capture PCM length does not match its recorded clock.".into(),
                );
            }
            if take.buffer.data.iter().any(|sample| !sample.is_finite()) {
                return Err("Stored Set capture contains nonfinite PCM samples.".into());
            }
            validated.insert(
                take.id,
                Capture {
                    legacy_clock: take.legacy_clock,
                    buffer: take.buffer,
                    bpm: take.bpm,
                    beats: take.beats,
                    bars: take.bars,
                    seconds: take.seconds,
                },
            );
        }
        self.cards = validated;
        Ok(())
    }

    /// A take may continue through every admitted bar. Reserve those slots
    /// together with inactive custody before input is opened or recording is
    /// armed; existing current-plan IDs are replaced rather than counted twice.
    pub(super) fn check_capture_quota(&self, song: &Song, output_rate: u32) -> Result<(), String> {
        if song.bars.iter().any(|bar| bar.legacy_clock) {
            return Err("Clear the legacy-clock loops before recording. Existing takes keep their original playback clocks.".into());
        }
        let Some(plan) = &self.plan else {
            // Legacy saved-song recording has separate custody and the
            // runtime's own allocation bounds; it cannot grow the Set bank.
            return Ok(());
        };
        if plan.segments.len() != song.bars.len() || output_rate == 0 {
            return Err("Set capture quota requires the current admitted backing clock.".into());
        }
        let current: BTreeSet<_> = plan.segments.iter().map(|segment| segment.id).collect();
        if current.len() != plan.segments.len() {
            return Err("Set capture quota contains duplicate admitted Card identities.".into());
        }
        let mut count = current.len();
        let mut frames = 0u64;
        for (id, capture) in &self.cards {
            if !current.contains(id) {
                count += 1;
                frames = frames
                    .checked_add(capture.buffer.len() as u64)
                    .filter(|&frames| frames <= MAX_STORED_FRAMES as u64)
                    .ok_or("Set capture quota exceeds the stored frame bound.")?;
            }
        }
        if count > MAX_STORED_CAPTURES {
            return Err("Set capture quota would exceed 256 stored takes. Clear stored Card loops before recording this Set.".into());
        }
        for bar in &song.bars {
            // Match Recording::prepare's exact per-bar storage reservation.
            frames = frames
                .checked_add(bar.checked_duration_frames(output_rate).map_err(|error| error.to_string())?)
                .filter(|&frames| frames <= MAX_STORED_FRAMES as u64)
                .ok_or("Set capture quota would exceed 16777216 stored frames. Clear stored Card loops before recording this Set.")?;
        }
        Ok(())
    }

    pub(super) fn prepare_replacement(
        &mut self,
        current: &Song,
        plan: &LoopPlan,
    ) -> Result<Song, String> {
        if capture_busy(current) {
            return Err("Stop recording / armed capture before updating the loaded backing. The currently loaded Cards remain active.".into());
        }
        self.remember(current);
        self.prepare(plan)
    }

    pub(super) fn remember(&mut self, song: &Song) {
        if let Some(plan) = &self.plan {
            for (segment, bar) in plan.segments.iter().zip(&song.bars) {
                if let Some(buffer) = &bar.audio_buffer {
                    self.cards.insert(
                        segment.id,
                        Capture {
                            legacy_clock: bar.legacy_clock,
                            buffer: buffer.clone(),
                            bpm: segment.bpm,
                            beats: segment.beats,
                            bars: segment.bars,
                            seconds: segment.seconds,
                        },
                    );
                } else {
                    self.cards.remove(&segment.id);
                }
            }
        } else {
            self.saved_song = song
                .bars
                .iter()
                .map(|bar| bar.audio_buffer.clone())
                .collect();
        }
    }

    pub(super) fn prepare(&self, plan: &LoopPlan) -> Result<Song, String> {
        if plan.segments.is_empty() || plan.owner.0 == 0 {
            return Err("Set backing requires a nonempty, explicitly owned plan.".into());
        }
        let mut seen = BTreeSet::new();
        let mut song = Song::new();
        song.name = plan.name.clone();
        song.one_shot = plan.one_shot;
        song.click_enabled = plan.click;
        song.bars.clear();
        for segment in &plan.segments {
            if segment.id.set != plan.owner
                || segment.id.card.0 == 0
                || !seen.insert(segment.id)
                || !segment.bpm.is_finite()
                || segment.bpm < 1.0
                || !(1..=16).contains(&segment.beats)
                || !valid_hold(segment.bars, segment.seconds)
                || (segment.seconds.is_some() && segment.phrase.is_none())
                || (segment.phrase.is_some() && !segment.pitches_hz.is_empty())
                || segment
                    .pitches_hz
                    .iter()
                    .any(|pitch| !pitch.is_finite() || *pitch <= 0.0)
            {
                return Err("Set backing contains an invalid identity, clock or pitch.".into());
            }
            let capture = self.cards.get(&segment.id);
            if capture.is_some_and(|capture| !capture.clock_matches(segment)) {
                return Err(format!(
                    "Clear the captured loop for '{}' before changing its tempo, meter or hold. The loaded backing keeps its original clock.",
                    segment.label
                ));
            }
            song.bars.push(Bar {
                legacy_clock: capture.is_some_and(|capture| capture.legacy_clock),
                bpm: segment.bpm,
                time_signature: TimeSignature::new(segment.beats, 4),
                subdivision: Subdivision::QUARTER,
                chord_ref: segment.pitches_hz.first().map(|root| ChordRef {
                    formula_name: "Set Card".into(),
                    root_freq_hz: *root,
                    pitches_hz: segment.pitches_hz.clone(),
                    label: segment.label.clone(),
                }),
                backing_phrase: segment.phrase.as_ref().map(|phrase| BackingPhrase {
                    events: phrase
                        .events
                        .iter()
                        .map(|event| BackingEvent {
                            time: event.time,
                            pitches_hz: event.pitches_hz.clone(),
                            strum_seconds: event.strum_seconds,
                        })
                        .collect(),
                }),
                audio_buffer: capture.map(|capture| capture.buffer.clone()),
                label: segment.label.clone(),
                length: if segment.seconds.is_some() {
                    1
                } else {
                    segment.bars
                },
                duration_seconds: segment.seconds,
            });
        }
        Ok(song)
    }

    pub(super) fn index_of(&self, id: LoopSegmentId) -> Result<usize, String> {
        self.plan
            .as_ref()
            .and_then(|plan| plan.index_of(id))
            .ok_or_else(|| "That Card is not in the currently loaded Set backing.".into())
    }

    pub(super) fn clear(&mut self, id: LoopSegmentId) {
        self.cards.remove(&id);
    }
}

fn valid_hold(bars: u8, seconds: Option<f64>) -> bool {
    match seconds {
        Some(seconds) => bars == 0 && seconds.is_finite() && seconds >= 0.5,
        None => bars > 0,
    }
}

pub(super) fn capture_busy(song: &Song) -> bool {
    song.recording
        || song
            .pending
            .iter()
            .any(|change| matches!(change, PendingChange::StartRecording { .. }))
}

#[cfg(test)]
#[path = "loop_projection/tests.rs"]
mod tests;
