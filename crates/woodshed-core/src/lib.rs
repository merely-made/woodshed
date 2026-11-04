//! Woodshed's portable state/logic core.
//!
//! Pure state over the `woodshedding` theory crate — no UI, no host, no
//! audio dependency, so the same core feeds the desktop host, the web host,
//! and tests. This is the genet-host plan's W1.1 split.
//!
//! S2 ships the Stage slice ([`StageState`]) with the lens model: tuning +
//! root + active lens, with the Scales and Chords lenses resolving to
//! fretboard dots. Arpeggios / Progressions / Exercises are placeholders
//! until their engines migrate from woodshed-xilem's `AppState` (S4).

pub mod arpeggio;
pub mod arrangement;
pub mod audio;
pub mod card_shapes;
pub mod card_timing;
pub use card_timing::{CardPreviewResult, CardTimingError, resolve_card_timing};
pub mod captured_arpeggio;
pub mod catalog_explorations;
pub mod chord_approach;
pub mod comparison_disclosure;
pub mod connected_catalog;
pub mod connected_scales;
pub mod harmony;
pub mod history;
pub mod loop_capture;
pub mod loop_plan;
pub mod mere;
pub mod midi;
pub mod music;
pub mod musical_context;
pub mod personal_relations;
pub mod pitch_motion_reading;
pub mod practice_clock;
pub use practice_clock::{
    LiveSequenceKind, PracticeCard, PracticeEvent, PracticeHold, PracticeProgram,
};
#[cfg(feature = "portable-scenes")]
pub mod arrangement_document;
#[cfg(feature = "portable-scenes")]
pub mod arrangement_scene;
#[cfg(feature = "portable-scenes")]
pub mod fretboard_scene;
#[cfg(feature = "portable-scenes")]
pub mod progression_scene;
pub mod retained_sets;
pub mod saved_arrangements;
pub mod scale_pattern_discovery;
pub mod scale_realization;
pub mod sealed_backend;
pub mod search;
pub mod session_overview;
pub mod settings;
pub mod shape_movement;
pub mod song;
pub mod stage_candidates;
pub mod stage_context;
pub mod stage_scene;
pub mod storage;
pub mod tonnetz;
pub mod working_sets;

pub use card_shapes::{
    CARD_SHAPE_PROFILE, CardShapeGeometry, CardShapeStatus, CardShapeUnavailable, ResolvedCardShape,
};

use arpeggio::{ArpeggioDirection, ArpeggioRun, generate_shapes};
use std::cell::RefCell;
use woodshedding::chord::{ChordFormula, catalog as chord_catalog};
use woodshedding::exercise::{Exercise, ExerciseParams, catalog as exercise_catalog};
use woodshedding::fretboard::{Fretboard, Position, StringPlay};
use woodshedding::interval::Interval;
use woodshedding::pitch::PitchClass;
use woodshedding::pitch::{Pitch, Spelling};
use woodshedding::practice::{PracticeItem, PracticeSet};
use woodshedding::progression::{
    ChordRole, Progression, RoleQuality, catalog as progression_catalog,
};
use woodshedding::rehearsal::{
    Card, CardId, FretWindow, LoopMode, MarkMode, Material, Recipe, Set, Setting, Timing, Touch,
};
use woodshedding::scale::{ScaleFormula, catalog as scale_catalog};
use woodshedding::tuning::{Tuning, TuningSpec, catalog as tuning_catalog};

/// The fretboard lens strip (redesign-plan vocabulary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Lens {
    #[default]
    Scales,
    Chords,
    Arpeggios,
    Progressions,
    Exercises,
}

impl Lens {
    pub const ALL: [Lens; 5] = [
        Lens::Scales,
        Lens::Chords,
        Lens::Arpeggios,
        Lens::Progressions,
        Lens::Exercises,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Lens::Scales => "Scale",
            Lens::Chords => "Chord",
            Lens::Arpeggios => "Arpeggio",
            Lens::Progressions => "Progression",
            Lens::Exercises => "Exercise",
        }
    }

    /// True when the lens resolves dots on the board today. All five do
    /// as of S4 slice 3; kept for the S4 tab placeholders' sake.
    pub fn implemented(self) -> bool {
        true
    }
}

/// Root pitch-class names, indexed by semitones above A.
pub const ROOT_NAMES: [&str; 12] = [
    "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
];

/// Midi of root index 0 (A3) — the octave is irrelevant to position
/// resolution (pitch-class matching) but midi space needs one.
const ROOT_BASE_MIDI: i32 = 57;

/// Scale-degree label for an interval measured in semitones from the root
/// ("1", "b3", "5"). Empty for anything outside an octave slot.
pub fn degree_label(semitones: i32) -> &'static str {
    match semitones.rem_euclid(12) {
        0 => "1",
        1 => "b2",
        2 => "2",
        3 => "b3",
        4 => "3",
        5 => "4",
        6 => "b5",
        7 => "5",
        8 => "b6",
        9 => "6",
        10 => "b7",
        11 => "7",
        _ => "",
    }
}

/// Interval name from the root ("Root", "Minor 3rd") for a semitone distance.
pub fn interval_name(semitones: i32) -> &'static str {
    match semitones.rem_euclid(12) {
        0 => "Root",
        1 => "Minor 2nd",
        2 => "Major 2nd",
        3 => "Minor 3rd",
        4 => "Major 3rd",
        5 => "Perfect 4th",
        6 => "Tritone",
        7 => "Perfect 5th",
        8 => "Minor 6th",
        9 => "Major 6th",
        10 => "Minor 7th",
        11 => "Major 7th",
        _ => "",
    }
}

/// One rendered fretboard dot: a position resolved against the current
/// tuning, ready for a view layer to place.
#[derive(Clone, Debug)]
pub struct FretDot {
    pub string_index: usize,
    pub fret: u8,
    /// True when this position is the root (unison from the named root).
    pub is_root: bool,
    /// Note name without octave ("A", "C#") — the dot label.
    pub label: String,
    /// Pitch octave, for the detail card.
    pub octave: i8,
    /// Scale degree relative to the root ("1", "b3"), empty if unknown.
    pub degree: String,
    /// Interval name from the root ("Root", "Minor 3rd"), empty if unknown.
    pub interval_name: String,
    /// The note's frequency in Hz, for the card's play button (0.0 if unknown).
    pub frequency: f32,
}

impl FretDot {
    fn from_position(p: Position) -> Self {
        let semis = p.interval_from_root.map(|i| i.semitones());
        Self {
            string_index: p.string_index,
            fret: p.fret,
            is_root: semis == Some(0),
            label: format!("{}{}", p.pitch.name, p.pitch.accidental),
            octave: p.pitch.octave,
            degree: semis.map(degree_label).unwrap_or("").to_string(),
            interval_name: semis.map(interval_name).unwrap_or("").to_string(),
            frequency: p.pitch.frequency() as f32,
        }
    }
}

/// A spoken label for a fretboard marker, for assistive tech (and rich enough
/// for a semantic driver to resolve): the note with its octave, its role in the
/// material, and where it sits. E.g. "A2, root, string 6, fret 5" or
/// "C#3, Major 3rd, string 2, open". The marker overlay carries this as an
/// `aria-label`, so the neck is navigable note-by-note even though the strings
/// and frets themselves are painted (and thus invisible to the DOM).
pub fn marker_a11y_label(dot: &FretDot, string_count: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    // A resolved pitch carries an octave; an exercise finger (no pitch) does not.
    let note = if dot.frequency > 0.0 {
        format!("{}{}", dot.label, dot.octave)
    } else {
        dot.label.clone()
    };
    if !note.is_empty() {
        parts.push(note);
    }
    if dot.is_root {
        parts.push("root".to_string());
    } else if !dot.interval_name.is_empty() {
        parts.push(dot.interval_name.clone());
    }
    // Guitar-style numbering: string 1 is the highest (largest index).
    let string_no = string_count.saturating_sub(dot.string_index);
    parts.push(if dot.fret == 0 {
        format!("string {string_no}, open")
    } else {
        format!("string {string_no}, fret {}", dot.fret)
    });
    parts.join(", ")
}

/// The Stage state: what the fretboard is showing right now.
pub struct StageState {
    pub lens: Lens,
    /// Index into [`tunings`].
    pub tuning_idx: usize,
    /// Index into [`ROOT_NAMES`] (semitones above A).
    pub root_idx: usize,
    /// Index into the scale catalog (Scales lens).
    pub scale_idx: usize,
    /// Index into the chord catalog (Chords lens; chord-tone view — the
    /// voicing browser migrates in S4).
    pub chord_idx: usize,
    /// Highest fret shown (inclusive, from the nut at 0).
    /// The neck window's last fret. The board shows `fret_start ..= fret_count`;
    /// [`Self::apply_neck`] sets both from the settings and the instrument.
    pub fret_count: u8,
    /// The neck window's first fret. 0 includes the open strings and the nut;
    /// a higher start is a window onto the middle of the neck (8-16, 2-22).
    pub fret_start: u8,

    // === Arpeggio lens (S4 slice 1) ===
    /// Index into the *chord* catalog — the arpeggio's quality.
    pub arpeggio_idx: usize,
    /// Which generated position shape is active.
    pub arpeggio_position_idx: usize,
    /// Transport cursor through the walk.
    pub arpeggio_step_idx: usize,
    /// True while the step-through transport is auto-advancing.
    pub arpeggio_playing: bool,
    /// Scale-run transport (Scale/Chord lens): step through the board's notes at
    /// tempo, sounding and highlighting each. The keystone of touch-as-behavior;
    /// transient (not persisted).
    pub scale_run_playing: bool,
    pub scale_run_step: usize,
    /// The note sounding right now during a run, for the board highlight.
    pub scale_run_active: Option<(usize, u8)>,
    /// Whether to draw the touch's path as a trail over the markers (the touch
    /// editor: show the treatment, not just name it). Transient.
    pub path_shown: bool,
    /// Draw mode: clicking a marker appends it to `authored_path` instead of
    /// pinning its detail card, so the player draws the touch's path by hand.
    /// Transient.
    pub draw_mode: bool,
    /// The hand-drawn path, an ordered visit list of `(string, fret)`. When
    /// non-empty it *is* the run's path (the trail and the stepping run follow
    /// it), overriding the derived pitch-order. Transient (persisting it as a
    /// saved exercise is the next step). Duplicates allowed — a path may revisit
    /// a note.
    pub authored_path: Vec<(usize, u8)>,
    pub arpeggio_direction: ArpeggioDirection,
    /// Inversion: which chord tone the run starts on (0 = root),
    /// clamped to the tone count.
    pub arpeggio_inversion: u8,

    // === Progression lens (S4 slice 2) ===
    /// Index into the progression catalog; `None` = nothing selected
    /// (cold-start state, prompts the user to pick from the list).
    pub progression_idx: Option<usize>,
    /// Which chord card is expanded onto the board.
    pub progression_expanded: usize,

    // === Exercise lens (S4 slice 3) ===
    pub exercise_idx: usize,
    /// Lowest fret of the four-fret hand position.
    pub exercise_starting_fret: u8,
    /// Transport cursor through the step sequence.
    pub exercise_step_idx: usize,
    /// True while auto-advancing.
    pub exercise_playing: bool,
    /// Two-entry transient cache for selected-shape comparison, audition, and
    /// controls in the same frame. It is derived from the card + live setup and
    /// never participates in persistence.
    card_shape_cache: RefCell<Vec<card_shapes::CardShapeCache>>,
}

/// How many trailing steps the exercise board keeps visible behind the
/// current one (the fading-motion presentation from woodshed-xilem).
pub const EXERCISE_TRAIL: usize = 3;

/// The tuning catalog (all instruments; instrument filtering arrives with
/// the instrument picker in S4).
pub fn tunings() -> &'static [TuningSpec] {
    tuning_catalog()
}

/// Roman-numeral label for a progression role ("I", "ii", "V7", "♭VII").
/// Ported from woodshed-xilem `format_role`, plus the degree-alteration
/// prefix (the theory crate supplies the symbol; the old app dropped it).
pub fn format_role(role: &ChordRole) -> String {
    let lowercase = matches!(
        role.quality,
        RoleQuality::Minor
            | RoleQuality::Diminished
            | RoleQuality::Minor7
            | RoleQuality::HalfDiminished7
            | RoleQuality::Diminished7
            | RoleQuality::Minor6
            | RoleQuality::MinorMajor7
            | RoleQuality::Minor9
    );
    let numeral = match (role.degree, lowercase) {
        (1, false) => "I",
        (1, true) => "i",
        (2, false) => "II",
        (2, true) => "ii",
        (3, false) => "III",
        (3, true) => "iii",
        (4, false) => "IV",
        (4, true) => "iv",
        (5, false) => "V",
        (5, true) => "v",
        (6, false) => "VI",
        (6, true) => "vi",
        (7, false) => "VII",
        (7, true) => "vii",
        _ => "?",
    };
    let suffix = match role.quality {
        RoleQuality::Major | RoleQuality::Minor => "",
        RoleQuality::Diminished => "°",
        RoleQuality::Augmented => "+",
        RoleQuality::Dominant7 => "7",
        RoleQuality::Major7 => "M7",
        RoleQuality::Minor7 => "m7",
        RoleQuality::HalfDiminished7 => "ø7",
        RoleQuality::Diminished7 => "°7",
        RoleQuality::Sus2 => "sus2",
        RoleQuality::Sus4 => "sus4",
        RoleQuality::Major6 => "6",
        RoleQuality::Minor6 => "m6",
        RoleQuality::MinorMajor7 => "mM7",
        RoleQuality::Major9 => "M9",
        RoleQuality::Minor9 => "m9",
        RoleQuality::Dominant9 => "9",
    };
    format!("{}{numeral}{suffix}", role.alteration.symbol())
}

/// Voicing shape for `count` tones as `(duration secs, strum offset ms)`.
/// A scale becomes an ascending cascade (a wide per-note offset, so you
/// hear it climb); a chord strums tight (all tones near-together).
fn voicing_shape(count: usize, scale_like: bool) -> (f32, f32) {
    if scale_like {
        ((count as f32 * 0.13 + 0.4).min(3.0), 130.0)
    } else {
        (1.4, 18.0)
    }
}

/// Pitch class (0..=11) of an equal-tempered frequency, via its nearest MIDI
/// note. Used to fold board positions and voicing pitches onto pitch classes
/// for the Mute mode (drop a note by its class, at any octave).
fn pc_from_hz(hz: f32) -> u8 {
    if hz <= 0.0 {
        return 0;
    }
    let midi = (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32;
    midi.rem_euclid(12) as u8
}

impl Default for StageState {
    fn default() -> Self {
        Self::new()
    }
}

impl StageState {
    pub fn new() -> Self {
        // Default: the catalog's first tuning (standard 6-string guitar),
        // A root, minor pentatonic — the guitarist's hello-world.
        let scale_idx = scale_catalog()
            .iter()
            .position(|s| s.name == "Minor Pentatonic")
            .unwrap_or(0);
        Self {
            lens: Lens::Scales,
            tuning_idx: 0,
            root_idx: 0,
            scale_idx,
            chord_idx: 0,
            // Overwritten by apply_neck from the settings + instrument; the
            // literal only covers a StageState built before that runs.
            fret_count: 22,
            fret_start: 0,
            arpeggio_idx: 0,
            arpeggio_position_idx: 0,
            arpeggio_step_idx: 0,
            arpeggio_playing: false,
            scale_run_playing: false,
            scale_run_step: 0,
            scale_run_active: None,
            path_shown: false,
            draw_mode: false,
            authored_path: Vec::new(),
            arpeggio_direction: ArpeggioDirection::default(),
            arpeggio_inversion: 0,
            progression_idx: None,
            progression_expanded: 0,
            exercise_idx: 0,
            exercise_starting_fret: 1,
            exercise_step_idx: 0,
            exercise_playing: false,
            card_shape_cache: RefCell::new(Vec::new()),
        }
    }

    pub fn tuning(&self) -> Tuning {
        let specs = tunings();
        Tuning::from_spec(&specs[self.tuning_idx.min(specs.len() - 1)])
    }

    pub fn root(&self) -> Pitch {
        Pitch::from_midi(ROOT_BASE_MIDI + self.root_idx as i32, Spelling::Sharps)
    }

    pub fn root_name(&self) -> &'static str {
        ROOT_NAMES[self.root_idx.min(ROOT_NAMES.len() - 1)]
    }

    pub fn scales(&self) -> &'static [ScaleFormula] {
        scale_catalog()
    }

    pub fn chords(&self) -> &'static [ChordFormula] {
        chord_catalog()
    }

    pub fn scale(&self) -> &'static ScaleFormula {
        &scale_catalog()[self.scale_idx.min(scale_catalog().len() - 1)]
    }

    pub fn chord(&self) -> &'static ChordFormula {
        &chord_catalog()[self.chord_idx.min(chord_catalog().len() - 1)]
    }

    pub fn set_lens(&mut self, lens: Lens) {
        self.lens = lens;
    }

    pub fn select_scale(&mut self, idx: usize) {
        if idx < scale_catalog().len() {
            self.scale_idx = idx;
        }
    }

    pub fn select_chord(&mut self, idx: usize) {
        if idx < chord_catalog().len() {
            self.chord_idx = idx;
        }
    }

    pub fn set_tuning(&mut self, idx: usize) {
        if idx < tunings().len() {
            self.tuning_idx = idx;
        }
    }

    /// Set the neck window shown on the board from the settings. `end` of `None`
    /// means the current instrument's full standard neck. The window is
    /// `fret_start ..= fret_count` (inclusive); a start of 0 includes the open
    /// strings. Restores an old woodshed capability: pick the fret range
    /// (0-12, 8-16, 2-22). Callers re-apply this on a tuning change too, so the
    /// full-neck default tracks the instrument.
    pub fn apply_neck(&mut self, start: u8, end: Option<u8>) {
        let specs = tunings();
        let standard = specs[self.tuning_idx.min(specs.len() - 1)]
            .instrument
            .standard_fret_count();
        // Cap generously (24-fret basses, 24-position bowed necks) but bound the
        // painted board.
        let end = end.unwrap_or(standard).clamp(1, 30);
        self.fret_start = start.min(end);
        self.fret_count = end;
    }

    pub fn set_root(&mut self, idx: usize) {
        if idx < ROOT_NAMES.len() {
            self.root_idx = idx;
        }
    }

    pub fn string_count(&self) -> usize {
        self.tuning().string_count()
    }

    // === Voicing (S4 slice 12: "hear the theory") ===
}

#[cfg(test)]
mod tests;

mod stage_neighbors;

mod stage_cards;

mod stage_paths;

mod stage_pitches;

mod stage_boards;

mod related_types;

use related_types::diversify;
pub use related_types::{NeighborhoodNode, NeighborhoodSnapshot, RelatedSuggestion, RelatedTarget};

mod board_types;

pub use board_types::{
    ArpDot, ArpeggioBoard, ExerciseBoard, ExerciseDot, LensMarker, ProgressionBoard,
    ProgressionCard,
};

mod practice_sets;

pub use practice_sets::{card_dwell, card_from_practice_item, set_from_practice, step_set};
