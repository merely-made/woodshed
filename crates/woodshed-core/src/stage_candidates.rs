//! Bounded nearby catalog candidates for the musical Stage readings.
//!
//! Candidate ranking is a read model. It never changes Set membership or the
//! scene's placement policy, and keeps the reading distance separate from the
//! optional exact pitch-motion comparison.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use woodshedding::pitch::PitchClass;

use crate::harmony::{KeyedCatalogRef, compare_pitch_motion};
use crate::settings::StageGraphReading;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageCandidateReason {
    CircleOfFifths { distance: u8 },
    Tonnetz { depth: u8 },
    PitchMotion { total_semitones: u16 },
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageCandidate {
    pub keyed: KeyedCatalogRef,
    pub reason: StageCandidateReason,
    pub fifth_distance: Option<u8>,
    pub tonnetz_depth: Option<u8>,
    pub pitch_motion: Option<crate::harmony::PitchClassMotion>,
}

/// Rank the 24 major/minor triads for the selected musical reading.
///
/// `omit` represents authored foreground material. The focused realization is
/// also omitted from suggestions. Exact pitch motion is retained as a separate
/// metric and becomes unavailable when the focus is not an equal-cardinality
/// triad; unavailable is never encoded as zero.
pub fn ranked_candidates(
    reading: StageGraphReading,
    focus: &KeyedCatalogRef,
    omit: &BTreeSet<KeyedCatalogRef>,
) -> Vec<StageCandidate> {
    if reading == StageGraphReading::Set {
        return Vec::new();
    }
    let depths = tonnetz_depths(focus);
    let focus_sector = fifth_sector(focus);
    let mut candidates = Vec::new();
    for name in ["Major", "Minor"] {
        for root in 0..12 {
            let keyed = KeyedCatalogRef {
                formula_id: format!("chord:{name}"),
                root: PitchClass::new(root),
            };
            if keyed == *focus || omit.contains(&keyed) {
                continue;
            }
            let fifth_distance = Some(circular_distance(focus_sector, fifth_sector(&keyed)));
            let tonnetz_depth = depths.get(&keyed).copied();
            let pitch_motion = compare_pitch_motion(focus, &keyed);
            let reason = match reading {
                StageGraphReading::Tonnetz => tonnetz_depth
                    .map(|depth| StageCandidateReason::Tonnetz { depth })
                    .unwrap_or(StageCandidateReason::Unavailable),
                StageGraphReading::PitchMotion => pitch_motion
                    .as_ref()
                    .map(|motion| StageCandidateReason::PitchMotion {
                        total_semitones: motion.total_semitones,
                    })
                    .unwrap_or(StageCandidateReason::Unavailable),
                _ => StageCandidateReason::CircleOfFifths {
                    distance: fifth_distance.unwrap_or_default(),
                },
            };
            candidates.push(StageCandidate {
                keyed,
                reason,
                fifth_distance,
                tonnetz_depth,
                pitch_motion,
            });
        }
    }
    candidates.sort_by(|a, b| {
        let primary = match reading {
            StageGraphReading::Tonnetz => metric_cmp(a.tonnetz_depth, b.tonnetz_depth),
            StageGraphReading::PitchMotion => metric_cmp(
                a.pitch_motion.as_ref().map(|motion| motion.total_semitones),
                b.pitch_motion.as_ref().map(|motion| motion.total_semitones),
            ),
            _ => metric_cmp(a.fifth_distance, b.fifth_distance),
        };
        primary
            .then_with(|| {
                metric_cmp(
                    a.pitch_motion.as_ref().map(|motion| motion.total_semitones),
                    b.pitch_motion.as_ref().map(|motion| motion.total_semitones),
                )
            })
            .then_with(|| a.keyed.cmp(&b.keyed))
    });
    candidates
}

fn metric_cmp<T: Ord>(left: Option<T>, right: Option<T>) -> std::cmp::Ordering {
    left.is_none()
        .cmp(&right.is_none())
        .then_with(|| left.cmp(&right))
}

fn fifth_sector(keyed: &KeyedCatalogRef) -> u8 {
    // Multiplication by seven walks clockwise through the fifths; 7 is its
    // own inverse modulo 12, so this also maps a pitch class to its sector.
    let root = if keyed.formula_id.contains("Minor") {
        PitchClass::new(keyed.root.value() + 3)
    } else {
        keyed.root
    };
    (root.value() * 7) % 12
}

fn circular_distance(left: u8, right: u8) -> u8 {
    let distance = left.abs_diff(right);
    distance.min(12 - distance)
}

fn tonnetz_depths(focus: &KeyedCatalogRef) -> BTreeMap<KeyedCatalogRef, u8> {
    let mut depths = BTreeMap::new();
    let Some(_) = crate::tonnetz::triangle(focus) else {
        return depths;
    };
    depths.insert(focus.clone(), 0);
    let mut queue = VecDeque::from([focus.clone()]);
    while let Some(current) = queue.pop_front() {
        let depth = depths[&current];
        for (next, _) in crate::tonnetz::neighbors(&current) {
            if depths.contains_key(&next) {
                continue;
            }
            depths.insert(next.clone(), depth.saturating_add(1));
            queue.push_back(next);
        }
    }
    depths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(name: &str, root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: format!("chord:{name}"),
            root: PitchClass::new(root),
        }
    }

    #[test]
    fn circle_ranking_has_distinct_distance_and_motion_metrics() {
        let candidates = ranked_candidates(
            StageGraphReading::CircleOfFifths,
            &chord("Major", 0),
            &BTreeSet::new(),
        );
        assert_eq!(candidates.len(), 23);
        assert_eq!(candidates[0].fifth_distance, Some(0));
        assert_eq!(candidates[0].keyed, chord("Minor", 9));
        assert_eq!(
            candidates[0].pitch_motion.as_ref().unwrap().total_semitones,
            2
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.pitch_motion.is_some())
        );
    }

    #[test]
    fn tonnetz_ranking_reports_unavailable_for_nontriad_focus() {
        let focus = KeyedCatalogRef {
            formula_id: "scale:Major".into(),
            root: PitchClass::new(0),
        };
        let candidates = ranked_candidates(StageGraphReading::Tonnetz, &focus, &BTreeSet::new());
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.pitch_motion.is_none())
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.tonnetz_depth.is_none())
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.reason == StageCandidateReason::Unavailable)
        );
    }
}
