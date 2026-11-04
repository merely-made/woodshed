//! Fretboard mapping: scale and chord positions on any tuning.
//!
//! A [`Fretboard`] is a [`Tuning`] plus a fret count. It answers:
//!
//! - **Where does a scale appear?** [`Fretboard::positions_for_scale`]
//!   returns every (string, fret) where a scale degree sounds, with
//!   the scale's preferred spelling.
//! - **Where do chord tones appear?** [`Fretboard::positions_for_chord`]
//!   does the same for chord tones.
//! - **What chord shapes are playable?** [`Fretboard::find_chord_voicings`]
//!   enumerates voicings within a fret window — combinations of one
//!   position per string (or muted) where all chord tones appear and
//!   the root is in the bass.
//!
//! The first two are pure data layout. The third is a search problem
//! that scales with `(options_per_string)^string_count` — bounded but
//! non-trivial. Voicings are returned in discovery order; callers can
//! sort by `fret_span()`, lowest-fret, etc.
//!
//! # Example
//!
//! ```
//! use woodshedding::pitch::{NoteName, Pitch};
//! use woodshedding::tuning::{Instrument, Tuning};
//! use woodshedding::scale::catalog as scale_catalog;
//! use woodshedding::fretboard::Fretboard;
//!
//! let std_guitar = Tuning::find_for("Standard", Instrument::Guitar).unwrap();
//! let board = Fretboard::new(std_guitar, 22);
//! let major = scale_catalog().iter().find(|s| s.name == "Major").unwrap();
//! let positions = board.positions_for_scale(major, Pitch::natural(NoteName::C, 4)).unwrap();
//! assert!(!positions.is_empty());
//! ```

use crate::chord::ChordFormula;
use crate::interval::Interval;
use crate::pitch::{Accidental, NoteName, Pitch, TranspositionError};
use crate::scale::ScaleFormula;
use crate::tuning::Tuning;

/// A fret position that produces a particular pitch.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub string_index: usize,
    pub fret: u8,
    pub pitch: Pitch,
    /// If derived from a scale or chord, the interval from the named root.
    pub interval_from_root: Option<Interval>,
}

/// One string's contribution to a chord voicing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StringPlay {
    Muted,
    /// `fret == 0` means open string. `interval_from_root` is `None`
    /// when the played pitch is a slash-chord bass that is not a
    /// chord tone (e.g. the F# in Cmaj7/F#).
    Played {
        fret: u8,
        pitch: Pitch,
        interval_from_root: Option<Interval>,
    },
}

impl StringPlay {
    pub fn is_played(self) -> bool {
        matches!(self, StringPlay::Played { .. })
    }
    pub fn fret(self) -> Option<u8> {
        match self {
            StringPlay::Played { fret, .. } => Some(fret),
            StringPlay::Muted => None,
        }
    }
    pub fn pitch(self) -> Option<Pitch> {
        match self {
            StringPlay::Played { pitch, .. } => Some(pitch),
            StringPlay::Muted => None,
        }
    }
}

/// A chord voicing: one [`StringPlay`] per string in tuning order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordVoicing {
    pub strings: Vec<StringPlay>,
}

/// Result of a bounded voicing search. `complete` is false when the caller's
/// work or result budget was reached, so an interactive picker never presents
/// an incomplete prefix as its full candidate set.
#[derive(Clone, Debug)]
pub struct BoundedVoicingSearch {
    pub voicings: Vec<ChordVoicing>,
    pub complete: bool,
}

impl ChordVoicing {
    /// Distance between the lowest and highest fretted strings (open
    /// strings excluded). Returns 0 if no strings are fretted.
    pub fn fret_span(&self) -> u8 {
        let frets: Vec<u8> = self
            .strings
            .iter()
            .filter_map(|s| match s {
                StringPlay::Played { fret, .. } if *fret > 0 => Some(*fret),
                _ => None,
            })
            .collect();
        if frets.is_empty() {
            return 0;
        }
        frets.iter().max().copied().unwrap() - frets.iter().min().copied().unwrap()
    }

    /// Lowest fret used by any fretted string (open strings excluded).
    /// Returns 0 if no fretted notes (e.g. all open or muted).
    pub fn lowest_fretted_position(&self) -> u8 {
        self.strings
            .iter()
            .filter_map(|s| match s {
                StringPlay::Played { fret, .. } if *fret > 0 => Some(*fret),
                _ => None,
            })
            .min()
            .unwrap_or(0)
    }

    pub fn played_string_count(&self) -> usize {
        self.strings.iter().filter(|s| s.is_played()).count()
    }

    /// Convenience for tests: `[Some(0), None, Some(2), ...]` per string.
    pub fn fret_pattern(&self) -> Vec<Option<u8>> {
        self.strings.iter().map(|s| s.fret()).collect()
    }
}

/// What note is allowed in the bass (lowest sounding string) of a chord
/// voicing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BassConstraint {
    /// Bass must be the chord root. The traditional "root position" voicing.
    Root,
    /// Bass must be a chord tone — any inversion (1st = 3rd in bass,
    /// 2nd = 5th in bass, 3rd = 7th in bass for sevenths).
    AnyChordTone,
    /// Bass must have this specific pitch class. Used for slash chords
    /// like C/G (pitch class 7) or Cmaj7/F# (pitch class 6, not a
    /// chord tone). The bass pitch class need not be a chord tone.
    Pitch(u8),
}

#[derive(Clone, Debug)]
pub struct Fretboard {
    pub tuning: Tuning,
    pub fret_count: u8,
}

impl Fretboard {
    pub fn new(tuning: Tuning, fret_count: u8) -> Self {
        Self { tuning, fret_count }
    }

    /// Pitch produced by stopping `string_index` at `fret`. Spelling
    /// uses sharps for chromatic positions; for spelling derived from
    /// a specific scale or chord, use [`Self::positions_for_scale`] or
    /// [`Self::positions_for_chord`].
    pub fn pitch_at(&self, string_index: usize, fret: u8) -> Pitch {
        let open = self.tuning.strings[string_index];
        let target_midi = open.midi() + fret as i32;
        Pitch::from_midi(target_midi, crate::pitch::Spelling::Sharps)
    }

    /// All positions on the fretboard whose pitch class matches a
    /// degree of the scale rooted at `root`. Spelling is taken from
    /// the scale's pitches (so D major scale positions display F# not Gb).
    pub fn positions_for_scale(
        &self,
        scale: &ScaleFormula,
        root: Pitch,
    ) -> Result<Vec<Position>, TranspositionError> {
        let pitches = scale.apply_to(root)?;
        Ok(self.positions_for_pitch_set(&pitches, scale.intervals))
    }

    /// All positions whose pitch class matches a chord tone of `chord`
    /// rooted at `root`. Spelling is taken from the chord's pitches.
    pub fn positions_for_chord(
        &self,
        chord: &ChordFormula,
        root: Pitch,
    ) -> Result<Vec<Position>, TranspositionError> {
        let pitches = chord.apply_to(root)?;
        Ok(self.positions_for_pitch_set(&pitches, chord.intervals))
    }

    fn positions_for_pitch_set(&self, pitches: &[Pitch], intervals: &[Interval]) -> Vec<Position> {
        let mut result = Vec::new();
        for (string_index, &open) in self.tuning.strings.iter().enumerate() {
            for fret in 0..=self.fret_count {
                let target_midi = open.midi() + fret as i32;
                let target_pc = target_midi.rem_euclid(12) as u8;
                if let Some(degree_idx) = pitches.iter().position(|p| p.pitch_class() == target_pc)
                {
                    let canonical = pitches[degree_idx];
                    let pitch = respell_at_midi(canonical.name, canonical.accidental, target_midi);
                    result.push(Position {
                        string_index,
                        fret,
                        pitch,
                        interval_from_root: Some(intervals[degree_idx]),
                    });
                }
            }
        }
        result
    }

    /// Enumerate chord voicings playable in the fret window
    /// `[lowest_fret, lowest_fret + max_fret_span]` with the bass note
    /// being the chord root. Convenience wrapper around
    /// [`Self::find_chord_voicings_for_bass`] with `BassConstraint::Root`.
    pub fn find_chord_voicings(
        &self,
        chord: &ChordFormula,
        root: Pitch,
        lowest_fret: u8,
        max_fret_span: u8,
    ) -> Result<Vec<ChordVoicing>, TranspositionError> {
        self.find_chord_voicings_for_bass(
            chord,
            root,
            lowest_fret,
            max_fret_span,
            BassConstraint::Root,
        )
    }

    /// Like [`Self::find_chord_voicings`], but explicitly bounds enumeration
    /// work and returned candidates for interactive consumers. A non-complete
    /// result is not a valid complete picker inventory.
    pub fn find_chord_voicings_bounded(
        &self,
        chord: &ChordFormula,
        root: Pitch,
        lowest_fret: u8,
        max_fret_span: u8,
        max_combinations: usize,
        max_results: usize,
    ) -> Result<BoundedVoicingSearch, TranspositionError> {
        self.find_chord_voicings_for_bass_bounded(
            chord,
            root,
            lowest_fret,
            max_fret_span,
            BassConstraint::Root,
            max_combinations,
            max_results,
        )
    }

    /// Enumerate chord voicings with a specific bass requirement.
    ///
    /// - [`BassConstraint::Root`] — root in bass (default voicings)
    /// - [`BassConstraint::AnyChordTone`] — any inversion (1st, 2nd,
    ///   3rd inversion all allowed)
    /// - [`BassConstraint::Pitch(pc)`] — slash chord with that pitch
    ///   class as bass; the slash bass need not be a chord tone
    ///   (e.g. Cmaj7/F# uses pitch class 6)
    ///
    /// Constraints:
    /// - All chord pitch classes appear somewhere in the voicing
    /// - Lowest sounding string satisfies the [`BassConstraint`]
    /// - Each fretted note is in `[lowest_fret, lowest_fret + max_fret_span]`
    /// - Fret span among fretted notes ≤ `max_fret_span`
    /// - At least 3 strings played
    pub fn find_chord_voicings_for_bass(
        &self,
        chord: &ChordFormula,
        root: Pitch,
        lowest_fret: u8,
        max_fret_span: u8,
        bass: BassConstraint,
    ) -> Result<Vec<ChordVoicing>, TranspositionError> {
        Ok(self
            .find_chord_voicings_for_bass_bounded(
                chord,
                root,
                lowest_fret,
                max_fret_span,
                bass,
                usize::MAX,
                usize::MAX,
            )?
            .voicings)
    }

    /// Bounded form of [`Self::find_chord_voicings_for_bass`].
    pub fn find_chord_voicings_for_bass_bounded(
        &self,
        chord: &ChordFormula,
        root: Pitch,
        lowest_fret: u8,
        max_fret_span: u8,
        bass: BassConstraint,
        max_combinations: usize,
        max_results: usize,
    ) -> Result<BoundedVoicingSearch, TranspositionError> {
        let chord_pitches = chord.apply_to(root)?;
        let required_pcs: Vec<u8> = {
            let mut s: Vec<u8> = chord_pitches.iter().map(|p| p.pitch_class()).collect();
            s.sort_unstable();
            s.dedup();
            s
        };
        // Pitch classes positions can use: chord tones, plus the slash
        // bass if specified and not already a chord tone.
        let mut allowed_pcs = required_pcs.clone();
        if let BassConstraint::Pitch(pc) = bass {
            if !allowed_pcs.contains(&pc) {
                allowed_pcs.push(pc);
            }
        }

        let upper = lowest_fret
            .saturating_add(max_fret_span)
            .min(self.fret_count);

        let per_string: Vec<Vec<StringPlay>> = self
            .tuning
            .strings
            .iter()
            .map(|&open| {
                let mut opts = vec![StringPlay::Muted];
                let lowest_in_window = if lowest_fret == 0 { 0 } else { lowest_fret };
                for fret in 0..=upper {
                    if fret > 0 && fret < lowest_in_window {
                        continue;
                    }
                    let target_midi = open.midi() + fret as i32;
                    let target_pc = target_midi.rem_euclid(12) as u8;
                    if !allowed_pcs.contains(&target_pc) {
                        continue;
                    }
                    // Use the chord's spelling if this is a chord tone;
                    // otherwise (slash bass non-chord-tone) fall back
                    // to the root's accidental preference for spelling.
                    let (pitch, interval_from_root) = match chord_pitches
                        .iter()
                        .position(|p| p.pitch_class() == target_pc)
                    {
                        Some(idx) => {
                            let canonical = chord_pitches[idx];
                            (
                                respell_at_midi(canonical.name, canonical.accidental, target_midi),
                                Some(chord.intervals[idx]),
                            )
                        },
                        None => {
                            // Slash bass non-chord-tone. Spell with sharps by default.
                            (
                                Pitch::from_midi(target_midi, crate::pitch::Spelling::Sharps),
                                None,
                            )
                        },
                    };
                    opts.push(StringPlay::Played {
                        fret,
                        pitch,
                        interval_from_root,
                    });
                }
                opts
            })
            .collect();

        let counts: Vec<usize> = per_string.iter().map(|v| v.len()).collect();
        let total = counts
            .iter()
            .try_fold(1usize, |product, count| product.checked_mul(*count));
        let work = total.unwrap_or(usize::MAX).min(max_combinations);
        let mut complete = total == Some(work);
        if max_results == 0 {
            return Ok(BoundedVoicingSearch {
                voicings: Vec::new(),
                complete: false,
            });
        }
        let root_pc = root.pitch_class();
        let mut result = Vec::new();
        for i in 0..work {
            let mut idx = i;
            let combo: Vec<StringPlay> = per_string
                .iter()
                .zip(counts.iter())
                .map(|(opts, &count)| {
                    let pick = idx % count;
                    idx /= count;
                    opts[pick]
                })
                .collect();
            if let Some(v) =
                self.validate_voicing(combo, &required_pcs, root_pc, max_fret_span, bass)
            {
                result.push(v);
                if result.len() >= max_results {
                    complete = false;
                    break;
                }
            }
        }
        Ok(BoundedVoicingSearch {
            voicings: result,
            complete,
        })
    }

    fn validate_voicing(
        &self,
        combo: Vec<StringPlay>,
        required_pcs: &[u8],
        root_pc: u8,
        max_fret_span: u8,
        bass: BassConstraint,
    ) -> Option<ChordVoicing> {
        let played_count = combo.iter().filter(|s| s.is_played()).count();
        if played_count < 3 {
            return None;
        }
        // Written string order is not necessarily low-to-high (high-G ukulele
        // and five-string banjo are re-entrant). Bass means the lowest sounding
        // MIDI pitch, never simply the first written string.
        let lowest_played = combo
            .iter()
            .filter_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(*pitch),
                _ => None,
            })
            .min_by_key(|pitch| pitch.midi())?
            .pitch_class();
        let bass_ok = match bass {
            BassConstraint::Root => lowest_played == root_pc,
            BassConstraint::AnyChordTone => required_pcs.contains(&lowest_played),
            BassConstraint::Pitch(pc) => lowest_played == pc,
        };
        if !bass_ok {
            return None;
        }
        let mut present: Vec<u8> = combo
            .iter()
            .filter_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .collect();
        present.sort_unstable();
        present.dedup();
        for pc in required_pcs {
            if !present.contains(pc) {
                return None;
            }
        }
        let voicing = ChordVoicing { strings: combo };
        if voicing.fret_span() > max_fret_span {
            return None;
        }
        Some(voicing)
    }
}

/// Construct a [`Pitch`] with the given letter+accidental that sounds at
/// `target_midi`. The octave is chosen so the resulting pitch's MIDI
/// number equals `target_midi`.
fn respell_at_midi(name: NoteName, accidental: Accidental, target_midi: i32) -> Pitch {
    let semitone_in_octave = name.semitone_offset() + accidental.semitone_offset();
    let octave = (target_midi - semitone_in_octave).div_euclid(12) - 1;
    Pitch::new(name, accidental, octave as i8)
}

#[cfg(test)]
mod tests;
