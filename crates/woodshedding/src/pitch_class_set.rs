//! Exact common and differing tones of two concrete pitch-class sets.
//!
//! Exact comparison preserves cardinality differences. Minimum motion pairs
//! equal-sized pitch-class sets without assigning register, harmonic function,
//! or fingering.

use std::collections::BTreeSet;

use crate::pitch::PitchClass;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchSetComparison {
    pub shared: BTreeSet<PitchClass>,
    pub left_only: BTreeSet<PitchClass>,
    pub right_only: BTreeSet<PitchClass>,
}

/// One octave-free pitch-class movement in a minimum assignment.
///
/// This is a set comparison, not a registered voice-leading, fingering, or
/// octave assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchClassMove {
    pub from: PitchClass,
    pub to: PitchClass,
    pub semitones: u8,
}

/// Exact held tones and a minimum-cost one-to-one assignment for equal-sized
/// nonempty pitch-class sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchClassMotion {
    pub held: BTreeSet<PitchClass>,
    pub moves: Vec<PitchClassMove>,
    pub total_semitones: u16,
}

pub fn compare(left: &BTreeSet<PitchClass>, right: &BTreeSet<PitchClass>) -> PitchSetComparison {
    PitchSetComparison {
        shared: left.intersection(right).copied().collect(),
        left_only: left.difference(right).copied().collect(),
        right_only: right.difference(left).copied().collect(),
    }
}

/// Find the exact minimum octave-free pitch-class assignment.
///
/// Exact shared tones remain held. The remaining tones use a bounded subset
/// dynamic program (`2^n`, where `n <= 12`) and circular undirected chromatic
/// distance `0..=6`. Equal-cost assignments choose the lexicographically first
/// sequence of target pitch classes for ascending source pitch classes.
pub fn minimum_motion(
    left: &BTreeSet<PitchClass>,
    right: &BTreeSet<PitchClass>,
) -> Option<PitchClassMotion> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let held: BTreeSet<_> = left.intersection(right).copied().collect();
    let sources = left.difference(right).copied().collect::<Vec<_>>();
    let targets = right.difference(left).copied().collect::<Vec<_>>();
    if sources.is_empty() {
        return Some(PitchClassMotion {
            held,
            moves: Vec::new(),
            total_semitones: 0,
        });
    }
    let count = sources.len();
    let mut states = vec![None::<(u16, Vec<usize>)>; 1 << count];
    states[0] = Some((0, Vec::new()));
    // The complete mask is a terminal state, with no remaining source.
    for mask in 0..((1 << count) - 1) {
        let Some((cost, assignment)) = states[mask].clone() else {
            continue;
        };
        let source = sources[assignment.len()];
        for target_index in 0..count {
            if mask & (1 << target_index) != 0 {
                continue;
            }
            let next_mask = mask | (1 << target_index);
            let mut candidate = assignment.clone();
            candidate.push(target_index);
            let candidate_cost = cost + u16::from(circular_distance(source, targets[target_index]));
            let replace = match &states[next_mask] {
                None => true,
                Some((current_cost, current)) => {
                    candidate_cost < *current_cost
                        || (candidate_cost == *current_cost && candidate < *current)
                },
            };
            if replace {
                states[next_mask] = Some((candidate_cost, candidate));
            }
        }
    }
    let (total_semitones, assignment) = states[(1 << count) - 1].clone()?;
    let moves = sources
        .into_iter()
        .zip(assignment)
        .map(|(from, index)| PitchClassMove {
            semitones: circular_distance(from, targets[index]),
            from,
            to: targets[index],
        })
        .collect();
    Some(PitchClassMotion {
        held,
        moves,
        total_semitones,
    })
}

fn circular_distance(from: PitchClass, to: PitchClass) -> u8 {
    let difference = from.value().abs_diff(to.value());
    difference.min(12 - difference)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tones(values: &[u8]) -> BTreeSet<PitchClass> {
        values.iter().copied().map(PitchClass::new).collect()
    }

    #[test]
    fn unequal_cardinality_and_reversal_preserve_every_tone() {
        let major = tones(&[0, 4, 7]);
        let major_seventh = tones(&[0, 4, 7, 11]);
        let forward = compare(&major, &major_seventh);
        assert_eq!(forward.shared, major);
        assert!(forward.left_only.is_empty());
        assert_eq!(forward.right_only, tones(&[11]));
        let reverse = compare(&major_seventh, &major);
        assert_eq!(reverse.shared, forward.shared);
        assert_eq!(reverse.left_only, forward.right_only);
        assert_eq!(reverse.right_only, forward.left_only);
    }

    #[test]
    fn disjoint_and_empty_material_do_not_lose_differences() {
        let left = tones(&[0, 4, 7]);
        let right = tones(&[6, 9, 1]);
        assert_eq!(
            compare(&left, &right),
            PitchSetComparison {
                shared: BTreeSet::new(),
                left_only: left.clone(),
                right_only: right,
            }
        );
        let empty = compare(&left, &BTreeSet::new());
        assert_eq!(empty.left_only, left);
        assert!(empty.shared.is_empty());
    }

    #[test]
    fn minimum_motion_holds_common_tones_and_handles_known_triads() {
        let c_major = tones(&[0, 4, 7]);
        let e_minor = tones(&[4, 7, 11]);
        let c_to_e = minimum_motion(&c_major, &e_minor).unwrap();
        assert_eq!(c_to_e.held, tones(&[4, 7]));
        assert_eq!(
            c_to_e.moves,
            vec![PitchClassMove {
                from: PitchClass::new(0),
                to: PitchClass::new(11),
                semitones: 1
            }]
        );
        let a_minor = tones(&[9, 0, 4]);
        let c_to_a = minimum_motion(&c_major, &a_minor).unwrap();
        assert_eq!(c_to_a.total_semitones, 2);
        assert_eq!(c_to_a.moves[0].from, PitchClass::new(7));
        assert_eq!(c_to_a.moves[0].to, PitchClass::new(9));
    }

    #[test]
    fn minimum_motion_is_bijective_deterministic_and_symmetric_in_cost() {
        let left = tones(&[0, 2, 6]);
        let right = tones(&[1, 5, 7]);
        let first = minimum_motion(&left, &right).unwrap();
        let second = minimum_motion(&left, &right).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first
                .moves
                .iter()
                .map(|item| item.to)
                .collect::<BTreeSet<_>>()
                .len(),
            first.moves.len()
        );
        assert_eq!(
            first.total_semitones,
            minimum_motion(&right, &left).unwrap().total_semitones
        );
    }

    #[test]
    fn subset_assignment_beats_a_nearest_available_greedy_choice() {
        let left = tones(&[0, 1]);
        let right = tones(&[2, 7]);
        let motion = minimum_motion(&left, &right).unwrap();
        assert!(motion.held.is_empty());
        assert_eq!(motion.total_semitones, 6);
        assert_eq!(
            motion.moves,
            vec![
                PitchClassMove {
                    from: PitchClass::new(0),
                    to: PitchClass::new(7),
                    semitones: 5
                },
                PitchClassMove {
                    from: PitchClass::new(1),
                    to: PitchClass::new(2),
                    semitones: 1
                },
            ]
        );
    }

    #[test]
    fn c_major_to_f_sharp_minor_has_exact_circular_cost_five() {
        let c_major = tones(&[0, 4, 7]);
        let f_sharp_minor = tones(&[6, 9, 1]);
        let motion = minimum_motion(&c_major, &f_sharp_minor).unwrap();
        assert_eq!(motion.total_semitones, 5);
    }

    #[test]
    fn assignment_matches_independent_exhaustive_three_tone_oracle() {
        let mut sets = Vec::new();
        for a in 0..12 {
            for b in (a + 1)..12 {
                for c in (b + 1)..12 {
                    sets.push([a, b, c]);
                }
            }
        }
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for left in &sets {
            for right in &sets {
                let expected = permutations
                    .iter()
                    .map(|order| {
                        (0..3)
                            .map(|i| {
                                let difference = (i16::from(left[i]) - i16::from(right[order[i]]))
                                    .unsigned_abs();
                                difference.min(12 - difference)
                            })
                            .sum::<u16>()
                    })
                    .min()
                    .unwrap();
                assert_eq!(
                    minimum_motion(&tones(left), &tones(right))
                        .unwrap()
                        .total_semitones,
                    expected,
                    "{left:?} -> {right:?}"
                );
            }
        }
        let tied = minimum_motion(&tones(&[0, 6]), &tones(&[3, 9])).unwrap();
        assert_eq!(
            tied.moves.iter().map(|m| m.to.value()).collect::<Vec<_>>(),
            vec![3, 9]
        );
        let identical = minimum_motion(&tones(&[0, 4, 7]), &tones(&[0, 4, 7])).unwrap();
        assert_eq!(identical.total_semitones, 0);
        assert!(identical.moves.is_empty());
        assert_eq!(identical.held, tones(&[0, 4, 7]));
    }

    #[test]
    fn minimum_motion_rejects_unequal_or_empty_sets() {
        assert!(minimum_motion(&tones(&[]), &tones(&[])).is_none());
        assert!(minimum_motion(&tones(&[0]), &tones(&[0, 4])).is_none());
    }
}
