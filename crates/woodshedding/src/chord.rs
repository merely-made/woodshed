//! Chord formulas.
//!
//! A [`ChordFormula`] is an ordered list of intervals from the root.
//! Apply a formula to any [`Pitch`] to generate the chord's pitches with
//! correct enharmonic spelling.
//!
//! Catalog covers triads, suspended, sevenths, sixths, add chords,
//! extended (9/11/13), altered dominants, and non-tertiary (quartal,
//! quintal, cluster).
//!
//! # Example
//!
//! ```
//! use woodshedding::pitch::{NoteName, Pitch};
//! use woodshedding::chord::catalog;
//!
//! let cmaj7 = catalog().iter().find(|c| c.name == "Major 7").unwrap();
//! let pitches = cmaj7.apply_to(Pitch::natural(NoteName::C, 4)).unwrap();
//! // pitches == [C4, E4, G4, B4]
//! ```

use core::fmt;

use crate::interval::Interval;
use crate::pitch::{Pitch, TranspositionError};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChordCategory {
    Triad,
    Suspended,
    Seventh,
    Sixth,
    Add,
    Extended,
    Altered,
    Quartal,
    Quintal,
    Cluster,
}

impl fmt::Display for ChordCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ChordCategory::Triad => "Triad",
            ChordCategory::Suspended => "Suspended",
            ChordCategory::Seventh => "Seventh",
            ChordCategory::Sixth => "Sixth",
            ChordCategory::Add => "Add",
            ChordCategory::Extended => "Extended",
            ChordCategory::Altered => "Altered",
            ChordCategory::Quartal => "Quartal",
            ChordCategory::Quintal => "Quintal",
            ChordCategory::Cluster => "Cluster",
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ChordFormula {
    pub name: &'static str,
    /// Common chord-symbol suffix (e.g. "m7", "maj7", "7b9"). Empty for
    /// plain major triad.
    pub symbol: &'static str,
    /// Intervals from the root, ascending.
    pub intervals: &'static [Interval],
    pub category: ChordCategory,
}

impl ChordFormula {
    /// Apply this formula to a root pitch, producing chord pitches in
    /// ascending order.
    pub fn apply_to(&self, root: Pitch) -> Result<Vec<Pitch>, TranspositionError> {
        self.intervals
            .iter()
            .map(|iv| root.transposed_by(*iv))
            .collect()
    }

    pub fn note_count(&self) -> usize {
        self.intervals.len()
    }
}

/// Return all chord formulas in the catalog.
pub fn catalog() -> &'static [ChordFormula] {
    CATALOG
}

/// Iterate the catalog filtered by category.
pub fn catalog_in_category(category: ChordCategory) -> impl Iterator<Item = &'static ChordFormula> {
    catalog().iter().filter(move |f| f.category == category)
}

/// Recognize chord formulas matching a sequence of ascending pitches.
/// Lowest pitch is treated as the root. Comparison is chromatic
/// (pitch-class), so spelling differences don't affect matches.
pub fn recognize(pitches: &[Pitch]) -> Vec<&'static ChordFormula> {
    if pitches.is_empty() {
        return vec![];
    }
    let root_midi = pitches[0].midi();
    let observed: Vec<i32> = pitches
        .iter()
        .map(|p| (p.midi() - root_midi).rem_euclid(12))
        .collect();
    catalog()
        .iter()
        .filter(|f| {
            if f.intervals.len() != observed.len() {
                return false;
            }
            f.intervals
                .iter()
                .zip(observed.iter())
                .all(|(iv, &s)| iv.semitones().rem_euclid(12) == s)
        })
        .collect()
}

// === Catalog ===

use crate::interval::Interval as I;

static CATALOG: &[ChordFormula] = &[
    // ── Triads ──
    ChordFormula {
        name: "Major",
        symbol: "",
        intervals: &[I::PERFECT_UNISON, I::MAJOR_THIRD, I::PERFECT_FIFTH],
        category: ChordCategory::Triad,
    },
    ChordFormula {
        name: "Minor",
        symbol: "m",
        intervals: &[I::PERFECT_UNISON, I::MINOR_THIRD, I::PERFECT_FIFTH],
        category: ChordCategory::Triad,
    },
    ChordFormula {
        name: "Augmented",
        symbol: "aug",
        intervals: &[I::PERFECT_UNISON, I::MAJOR_THIRD, I::AUGMENTED_FIFTH],
        category: ChordCategory::Triad,
    },
    ChordFormula {
        name: "Diminished",
        symbol: "dim",
        intervals: &[I::PERFECT_UNISON, I::MINOR_THIRD, I::DIMINISHED_FIFTH],
        category: ChordCategory::Triad,
    },
    // ── Suspended ──
    ChordFormula {
        name: "Suspended 2nd",
        symbol: "sus2",
        intervals: &[I::PERFECT_UNISON, I::MAJOR_SECOND, I::PERFECT_FIFTH],
        category: ChordCategory::Suspended,
    },
    ChordFormula {
        name: "Suspended 4th",
        symbol: "sus4",
        intervals: &[I::PERFECT_UNISON, I::PERFECT_FOURTH, I::PERFECT_FIFTH],
        category: ChordCategory::Suspended,
    },
    // ── Sevenths ──
    ChordFormula {
        name: "Major 7",
        symbol: "maj7",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    ChordFormula {
        name: "Dominant 7",
        symbol: "7",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    ChordFormula {
        name: "Minor 7",
        symbol: "m7",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    ChordFormula {
        name: "Minor-Major 7",
        symbol: "m(maj7)",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    ChordFormula {
        name: "Half-Diminished 7",
        symbol: "m7b5",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::DIMINISHED_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    ChordFormula {
        name: "Diminished 7",
        symbol: "dim7",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::DIMINISHED_FIFTH,
            I::DIMINISHED_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    ChordFormula {
        name: "Augmented Major 7",
        symbol: "maj7#5",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::AUGMENTED_FIFTH,
            I::MAJOR_SEVENTH,
        ],
        category: ChordCategory::Seventh,
    },
    // ── Sixths ──
    ChordFormula {
        name: "Major 6",
        symbol: "6",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
        ],
        category: ChordCategory::Sixth,
    },
    ChordFormula {
        name: "Minor 6",
        symbol: "m6",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SIXTH,
        ],
        category: ChordCategory::Sixth,
    },
    // ── Add ──
    ChordFormula {
        name: "Add 9",
        symbol: "add9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_NINTH,
        ],
        category: ChordCategory::Add,
    },
    ChordFormula {
        name: "Minor Add 9",
        symbol: "m(add9)",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_NINTH,
        ],
        category: ChordCategory::Add,
    },
    ChordFormula {
        name: "Add 11",
        symbol: "add11",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::PERFECT_ELEVENTH,
        ],
        category: ChordCategory::Add,
    },
    // ── Extended (9th) ──
    ChordFormula {
        name: "Major 9",
        symbol: "maj9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SEVENTH,
            I::MAJOR_NINTH,
        ],
        category: ChordCategory::Extended,
    },
    ChordFormula {
        name: "Dominant 9",
        symbol: "9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
        ],
        category: ChordCategory::Extended,
    },
    ChordFormula {
        name: "Minor 9",
        symbol: "m9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
        ],
        category: ChordCategory::Extended,
    },
    // ── Extended (11th) ──
    ChordFormula {
        name: "Major 11",
        symbol: "maj11",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SEVENTH,
            I::MAJOR_NINTH,
            I::PERFECT_ELEVENTH,
        ],
        category: ChordCategory::Extended,
    },
    ChordFormula {
        name: "Dominant 11",
        symbol: "11",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
            I::PERFECT_ELEVENTH,
        ],
        category: ChordCategory::Extended,
    },
    ChordFormula {
        name: "Minor 11",
        symbol: "m11",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
            I::PERFECT_ELEVENTH,
        ],
        category: ChordCategory::Extended,
    },
    // ── Extended (13th) ──
    ChordFormula {
        name: "Major 13",
        symbol: "maj13",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MAJOR_SEVENTH,
            I::MAJOR_NINTH,
            I::PERFECT_ELEVENTH,
            I::MAJOR_THIRTEENTH,
        ],
        category: ChordCategory::Extended,
    },
    ChordFormula {
        name: "Dominant 13",
        symbol: "13",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
            I::PERFECT_ELEVENTH,
            I::MAJOR_THIRTEENTH,
        ],
        category: ChordCategory::Extended,
    },
    ChordFormula {
        name: "Minor 13",
        symbol: "m13",
        intervals: &[
            I::PERFECT_UNISON,
            I::MINOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
            I::PERFECT_ELEVENTH,
            I::MAJOR_THIRTEENTH,
        ],
        category: ChordCategory::Extended,
    },
    // ── Altered (dominants) ──
    ChordFormula {
        name: "Dominant 7 b5",
        symbol: "7b5",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::DIMINISHED_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ChordCategory::Altered,
    },
    ChordFormula {
        name: "Dominant 7 #5",
        symbol: "7#5",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::AUGMENTED_FIFTH,
            I::MINOR_SEVENTH,
        ],
        category: ChordCategory::Altered,
    },
    ChordFormula {
        name: "Dominant 7 b9",
        symbol: "7b9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MINOR_NINTH,
        ],
        category: ChordCategory::Altered,
    },
    ChordFormula {
        name: "Dominant 7 #9",
        symbol: "7#9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::AUGMENTED_NINTH,
        ],
        category: ChordCategory::Altered,
    },
    ChordFormula {
        name: "Dominant 7 b5 b9",
        symbol: "7b5b9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::DIMINISHED_FIFTH,
            I::MINOR_SEVENTH,
            I::MINOR_NINTH,
        ],
        category: ChordCategory::Altered,
    },
    ChordFormula {
        name: "Dominant 7 #5 #9",
        symbol: "7#5#9",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::AUGMENTED_FIFTH,
            I::MINOR_SEVENTH,
            I::AUGMENTED_NINTH,
        ],
        category: ChordCategory::Altered,
    },
    ChordFormula {
        name: "Dominant 7 #11",
        symbol: "7#11",
        intervals: &[
            I::PERFECT_UNISON,
            I::MAJOR_THIRD,
            I::PERFECT_FIFTH,
            I::MINOR_SEVENTH,
            I::MAJOR_NINTH,
            I::AUGMENTED_ELEVENTH,
        ],
        category: ChordCategory::Altered,
    },
    // ── Quartal ──
    ChordFormula {
        // Two stacked perfect fourths: 1 4 b7
        name: "Quartal Triad",
        symbol: "Q",
        intervals: &[I::PERFECT_UNISON, I::PERFECT_FOURTH, I::MINOR_SEVENTH],
        category: ChordCategory::Quartal,
    },
    ChordFormula {
        // Three stacked perfect fourths: 1 4 b7 b3 (compound)
        name: "Quartal Tetrad",
        symbol: "Q4",
        intervals: &[
            I::PERFECT_UNISON,
            I::PERFECT_FOURTH,
            I::MINOR_SEVENTH,
            I::MINOR_THIRD,
        ],
        category: ChordCategory::Quartal,
    },
    // ── Quintal ──
    ChordFormula {
        // Two stacked perfect fifths: 1 5 9
        name: "Quintal Triad",
        symbol: "Q5",
        intervals: &[I::PERFECT_UNISON, I::PERFECT_FIFTH, I::MAJOR_NINTH],
        category: ChordCategory::Quintal,
    },
    // ── Cluster ──
    ChordFormula {
        // Whole-tone cluster: 1 2 3
        name: "Whole-Tone Cluster",
        symbol: "cluster(W)",
        intervals: &[I::PERFECT_UNISON, I::MAJOR_SECOND, I::MAJOR_THIRD],
        category: ChordCategory::Cluster,
    },
    ChordFormula {
        // Half-step cluster: 1 b2 2
        name: "Chromatic Cluster",
        symbol: "cluster(H)",
        intervals: &[I::PERFECT_UNISON, I::MINOR_SECOND, I::MAJOR_SECOND],
        category: ChordCategory::Cluster,
    },
];

#[cfg(test)]
mod tests;
