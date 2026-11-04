use super::*;

/// One exercise-board dot: the current step or one of its trail.
#[derive(Clone, Debug)]
pub struct ExerciseDot {
    pub string_index: usize,
    pub fret: u8,
    /// 0 = the current step; 1..=EXERCISE_TRAIL = steps behind it.
    pub recency: usize,
    /// Suggested fingering label ("1"-"4", empty when unspecified).
    pub label: String,
}

/// Everything the view needs to draw the Exercise lens.
#[derive(Clone, Debug)]
pub struct ExerciseBoard {
    pub dots: Vec<ExerciseDot>,
    pub step: usize,
    pub total: usize,
    pub starting_fret: u8,
    pub name: &'static str,
    pub description: &'static str,
}

/// One chord card of a materialized progression.
#[derive(Clone, Debug)]
pub struct ProgressionCard {
    /// Roman-numeral role ("I", "ii", "V7", "♭VII").
    pub numeral: String,
    /// Concrete chord in the current key ("A", "Dm7", "E7").
    pub chord_label: String,
    pub is_expanded: bool,
}

/// Everything the view needs to draw the Progression lens.
#[derive(Clone, Debug)]
pub struct ProgressionBoard {
    pub name: &'static str,
    pub description: &'static str,
    pub cards: Vec<ProgressionCard>,
    /// Chord-tone dots for the expanded card's chord.
    pub dots: Vec<FretDot>,
    /// The expanded chord's label (caption use).
    pub expanded_label: String,
}

/// A painted-leaf marker for a transport lens's board (Arpeggio / Exercise /
/// Progression), uniform so the host feeds one leaf and the view draws one
/// overlay from the same list — the way Scale / Chord already share
/// [`StageState::dots`]. `is_current` is the transport's highlighted step.
#[derive(Clone, Debug)]
pub struct LensMarker {
    pub string_index: usize,
    pub fret: u8,
    pub is_root: bool,
    pub is_current: bool,
    /// A note just behind the current step (the Exercise's fading trail): drawn
    /// faint so the eye stays on the current step. Never set with `is_current`.
    pub is_trail: bool,
    pub label: String,
}

/// One arpeggio-board dot: a shape position with the transport highlight.
#[derive(Clone, Debug)]
pub struct ArpDot {
    pub string_index: usize,
    pub fret: u8,
    pub is_root: bool,
    /// Under the transport cursor right now.
    pub is_current: bool,
    pub label: String,
}

/// Everything the view needs to draw the Arpeggio lens.
#[derive(Clone, Debug)]
pub struct ArpeggioBoard {
    pub dots: Vec<ArpDot>,
    pub shape_count: usize,
    pub position_idx: usize,
    pub start_fret: u8,
    pub walk_len: usize,
    pub step: usize,
    pub direction: ArpeggioDirection,
    pub inversion_label: String,
}
