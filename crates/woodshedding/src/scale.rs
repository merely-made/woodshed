//! Scale formulas.
//!
//! A [`ScaleFormula`] is an ordered list of intervals from the root,
//! including the unison and excluding the upper octave. Apply a formula
//! to any [`Pitch`] to generate the scale's pitches with correct
//! enharmonic spelling.
//!
//! The catalog covers diatonic modes, pentatonic, blues, harmonic minor
//! and modes, melodic minor and modes, symmetric scales, exotic scales,
//! and bebop. Non-tertiary scales are a future expansion.
//!
//! # Example
//!
//! ```
//! use woodshedding::pitch::{NoteName, Pitch};
//! use woodshedding::scale::{catalog, ScaleFormula};
//!
//! let major = catalog().iter().find(|s| s.name == "Major").unwrap();
//! let c_major = major.apply_to(Pitch::natural(NoteName::C, 4)).unwrap();
//! // c_major == [C4, D4, E4, F4, G4, A4, B4]
//! ```

use core::fmt;

use crate::interval::Interval;
use crate::pitch::{Pitch, TranspositionError};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScaleCategory {
    Diatonic,
    Pentatonic,
    Blues,
    Bebop,
    Symmetric,
    /// Harmonic minor, melodic minor, and their modes.
    AlteredMinor,
    /// World, gypsy, neapolitan, byzantine, etc.
    Exotic,
    /// Built from non-third intervals — placeholder for future entries.
    NonTertiary,
}

impl fmt::Display for ScaleCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ScaleCategory::Diatonic => "Diatonic",
            ScaleCategory::Pentatonic => "Pentatonic",
            ScaleCategory::Blues => "Blues",
            ScaleCategory::Bebop => "Bebop",
            ScaleCategory::Symmetric => "Symmetric",
            ScaleCategory::AlteredMinor => "Altered Minor",
            ScaleCategory::Exotic => "Exotic",
            ScaleCategory::NonTertiary => "Non-Tertiary",
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ScaleFormula {
    pub name: &'static str,
    /// Intervals from the root, including unison, excluding the octave.
    pub intervals: &'static [Interval],
    pub category: ScaleCategory,
}

impl ScaleFormula {
    /// Apply this formula to a root pitch, producing the scale pitches
    /// in ascending order. Includes the root, excludes the upper octave.
    pub fn apply_to(&self, root: Pitch) -> Result<Vec<Pitch>, TranspositionError> {
        self.intervals
            .iter()
            .map(|iv| root.transposed_by(*iv))
            .collect()
    }

    /// Number of unique scale degrees (length of `intervals`).
    pub fn degree_count(&self) -> usize {
        self.intervals.len()
    }
}

/// Return all scale formulas in the catalog.
pub fn catalog() -> &'static [ScaleFormula] {
    CATALOG
}

/// Iterate the catalog filtered by category.
pub fn catalog_in_category(category: ScaleCategory) -> impl Iterator<Item = &'static ScaleFormula> {
    catalog().iter().filter(move |f| f.category == category)
}

/// Recognize which scale formula(s) match a sequence of ascending pitches,
/// taking the first pitch as the root. Comparison is chromatic
/// (pitch-class), so spelling differences don't affect the match.
/// Multiple formulas may match for ambiguous patterns (rare).
pub fn recognize(pitches: &[Pitch]) -> Vec<&'static ScaleFormula> {
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
//
// Convention: each formula starts with PERFECT_UNISON. Comments give
// the conventional scale-degree shorthand (1, b3, #5, etc.).

use crate::interval::Interval as I;

#[cfg(test)]
mod tests;

mod formulas;

use formulas::CATALOG;
