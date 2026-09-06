//! Exact common and differing tones of two concrete pitch-class sets.
//!
//! This comparison preserves cardinality differences. It does not pair voices,
//! infer harmonic function, or assign a fingering or movement cost.

use std::collections::BTreeSet;

use crate::pitch::PitchClass;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchSetComparison {
    pub shared: BTreeSet<PitchClass>,
    pub left_only: BTreeSet<PitchClass>,
    pub right_only: BTreeSet<PitchClass>,
}

pub fn compare(left: &BTreeSet<PitchClass>, right: &BTreeSet<PitchClass>) -> PitchSetComparison {
    PitchSetComparison {
        shared: left.intersection(right).copied().collect(),
        left_only: left.difference(right).copied().collect(),
        right_only: right.difference(left).copied().collect(),
    }
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
}
