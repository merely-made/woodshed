//! Disposable, pure lowering of a retained Set or live Stage sequence into
//! occurrence-addressed practice events. Hosts may schedule this program, but
//! it does not author or mutate the Set and is not persisted.

use woodshedding::{
    exercise::ExerciseParams,
    fretboard::Fretboard,
    pitch::PitchClass,
    rehearsal::{Card, CardId, Hold, MarkMode, Material, Set, Setting, Timing, Touch},
};

use crate::{ArpeggioRun, StageState, arpeggio::generate_shapes, card_shapes::CardShapeStatus};

/// The temporal unit for a card's held phrase. Events use the matching unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PracticeHold {
    /// Authored bars/repetitions resolve against quarter-note beats.
    Beats(f64),
    /// Authored seconds remain absolute across tempo edits.
    Seconds(f64),
    /// One phrase is played; the Set waits for explicit advancement.
    Manual {
        phrase_beats: f64,
        tail_seconds: f64,
    },
}

/// One audible occurrence. Ordinal is identity within the derived program;
/// repeated pitches and repeated physical contacts remain distinct events.
#[derive(Clone, Debug, PartialEq)]
pub struct PracticeEvent {
    pub ordinal: u32,
    /// Relative onset, in beats for Beats/Manual or seconds for Seconds.
    pub at: f64,
    /// Internal chord-tone onset spread for a grouped event, preserving the
    /// existing block-chord strum articulation. Sequential events use zero.
    pub strum_seconds: f64,
    /// A block/chord may contain several pitches; a walk contains one.
    pub pitches: Vec<f32>,
    /// Physical `(string_index, nut-relative fret)` contacts for this event.
    pub contacts: Vec<(usize, u8)>,
}

#[derive(Clone, Copy, Debug)]
struct NoteVisit {
    pitch: f32,
    contact: (usize, u8),
}

/// One retained Card and its derived playback. Unavailable material stays in
/// place with an explanation instead of disappearing from the program.
#[derive(Clone, Debug)]
pub struct PracticeCard {
    pub card: Card,
    pub bpm: f32,
    pub hold: PracticeHold,
    pub events: Vec<PracticeEvent>,
    pub unavailable: Option<String>,
}

/// A snapshot of a Set's current derived sequence. It is not durable truth.
#[derive(Clone, Debug, Default)]
pub struct PracticeProgram {
    pub cards: Vec<PracticeCard>,
}

/// The three transient one-note Stage clocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveSequenceKind {
    Arpeggio,
    Exercise,
    ScaleRun,
}

fn live_setting(stage: &StageState) -> Setting {
    let tuning = stage.tuning();
    Setting {
        instrument: tuning.instrument.to_string(),
        tuning: Some(tuning.name.clone()),
        ..Setting::default()
    }
}

fn authored_hold(card: &Card) -> PracticeHold {
    match &card.timing.hold {
        Hold::Manual => PracticeHold::Manual {
            phrase_beats: 0.0,
            tail_seconds: 0.0,
        },
        Hold::Bars(bars) => PracticeHold::Beats(f64::from((*bars).max(1)) * 4.0),
        Hold::Reps(reps) => PracticeHold::Beats(f64::from((*reps).max(1)) * 4.0),
        Hold::Seconds(seconds) => PracticeHold::Seconds(f64::from((*seconds).max(0.5))),
    }
}

fn set_even_onsets(events: &mut [PracticeEvent], total: f64) {
    if events.is_empty() {
        return;
    }
    let step = total / events.len() as f64;
    for event in events {
        event.at = f64::from(event.ordinal) * step;
    }
}

fn same_pitch(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.02
}

fn same_pitches(actual: &[f32], expected: &[f32]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| same_pitch(*actual, *expected))
}

#[cfg(test)]
mod tests;

mod program;
