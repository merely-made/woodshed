//! Diatonic degree-pair recipes over a bounded available pitch set.
//! Missing physical pitches never close gaps in the scale's degree grammar.

use std::collections::BTreeSet;

use crate::rehearsal::ScalePattern;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalePatternError {
    UnsupportedDegrees,
    NoCompletePairs,
}

impl std::fmt::Display for ScalePatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedDegrees => f.write_str("Thirds and Fourths require seven distinct ascending scale degrees within one octave, starting at the root"),
            Self::NoCompletePairs => f.write_str("no complete scale-degree pairs are available in this fret window"),
        }
    }
}
impl std::error::Error for ScalePatternError {}

/// Complete pairs in ascending starting-pitch order. Each pair is visited in
/// order, including repeated pitches between successive pairs. `root_midi`
/// establishes octave-relative degrees; `degrees` must be seven ascending
/// semitone offsets 0..11. Available notes are exact pitches, not indices into
/// a clipped fretboard's surviving notes.
pub fn available_pairs(
    pattern: ScalePattern,
    root_midi: i32,
    degrees: &[i32],
    available_midi: &[i32],
) -> Result<Vec<(i32, i32)>, ScalePatternError> {
    if degrees.len() != 7
        || degrees.first() != Some(&0)
        || degrees.iter().any(|degree| !(0..12).contains(degree))
        || !degrees.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(ScalePatternError::UnsupportedDegrees);
    }
    let available = available_midi.iter().copied().collect::<BTreeSet<_>>();
    let mut pairs = Vec::new();
    for &start in &available {
        let semitone = (start - root_midi).rem_euclid(12);
        let Some(degree) = degrees.iter().position(|offset| *offset == semitone) else {
            continue;
        };
        let target_degree = degree + pattern.degree_distance();
        let offset = degrees[target_degree % 7] + (target_degree / 7) as i32 * 12 - semitone;
        let target = start + offset;
        if available.contains(&target) {
            pairs.push((start, target));
        }
    }
    if pairs.is_empty() {
        return Err(ScalePatternError::NoCompletePairs);
    }
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

    #[test]
    fn thirds_and_fourths_follow_degrees_and_cross_octaves() {
        let available = [60, 62, 64, 65, 67, 69, 71, 72, 74, 76];
        assert_eq!(
            available_pairs(ScalePattern::Thirds, 60, &MAJOR, &available).unwrap(),
            vec![
                (60, 64),
                (62, 65),
                (64, 67),
                (65, 69),
                (67, 71),
                (69, 72),
                (71, 74),
                (72, 76)
            ]
        );
        assert_eq!(
            available_pairs(ScalePattern::Fourths, 60, &MAJOR, &available).unwrap(),
            vec![
                (60, 65),
                (62, 67),
                (64, 69),
                (65, 71),
                (67, 72),
                (69, 74),
                (71, 76)
            ]
        );
    }

    #[test]
    fn missing_degrees_do_not_change_the_interval_recipe() {
        assert_eq!(
            available_pairs(ScalePattern::Thirds, 60, &MAJOR, &[60, 64, 67]).unwrap(),
            vec![(60, 64), (64, 67)]
        );
        assert_eq!(
            available_pairs(ScalePattern::Fourths, 60, &MAJOR, &[60, 64, 67]),
            Err(ScalePatternError::NoCompletePairs)
        );
        assert_eq!(
            available_pairs(ScalePattern::Thirds, 60, &[0, 2, 4, 7, 9], &[60, 64, 67]),
            Err(ScalePatternError::UnsupportedDegrees)
        );
    }

    #[test]
    fn legacy_scale_wire_and_explicit_pattern_wire_remain_distinct() {
        use crate::{pitch::PitchClass, rehearsal::Material};
        let legacy = r#"{"Scale":{"name":"Major","root":0}}"#;
        let material: Material = serde_json::from_str(legacy).unwrap();
        assert_eq!(serde_json::to_string(&material).unwrap(), legacy);
        let pattern = Material::ScalePattern {
            name: "Major".into(),
            root: PitchClass::new(0),
            pattern: ScalePattern::Thirds,
        };
        let reopened: Material =
            serde_json::from_str(&serde_json::to_string(&pattern).unwrap()).unwrap();
        assert!(matches!(
            reopened,
            Material::ScalePattern {
                pattern: ScalePattern::Thirds,
                ..
            }
        ));
    }
}
