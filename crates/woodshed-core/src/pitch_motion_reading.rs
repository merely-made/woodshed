//! Anchored pitch-motion reading for the Stage catalog context.
//!
//! The anchor is a captured keyed catalog subject. Fixed world bounds, identity
//! slots and distance rings keep browsing separate from layout recentering.

use std::collections::BTreeSet;

#[cfg(test)]
use woodshedding::pitch::PitchClass;

use crate::harmony::{KeyedCatalogRef, compare_pitch_motion};
use crate::settings::StageGraphReading;
use crate::stage_candidates::ranked_candidates;
use crate::stage_context::{
    StageContextGraph, StageContextNode, StageContextRelation, StageContextRelationKind,
    StageNodeId, StageNodeKind,
};

/// Build a bounded anchored pitch-motion neighborhood.
///
/// Retained identities are emitted first, then exact-motion candidates. The
/// focus remains present when it is not foreground material, so a background
/// click can keep its target visible while the UI changes focus.
pub fn context(
    focus: &KeyedCatalogRef,
    node_limit: usize,
    omit: &BTreeSet<KeyedCatalogRef>,
    retained: &BTreeSet<KeyedCatalogRef>,
) -> StageContextGraph {
    if node_limit == 0 {
        return StageContextGraph::default();
    }

    let mut keys = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |keyed: &KeyedCatalogRef| {
        if !omit.contains(keyed) && keyed.to_material().is_some() && seen.insert(keyed.clone()) {
            keys.push(keyed.clone());
        }
    };
    push(focus);
    for keyed in retained {
        push(keyed);
    }
    for candidate in ranked_candidates(StageGraphReading::PitchMotion, focus, omit) {
        push(&candidate.keyed);
    }
    let truncated = keys.len() > node_limit;
    keys.truncate(node_limit);

    let nodes = keys
        .iter()
        .map(|keyed| StageContextNode {
            id: StageNodeId::Catalog(keyed.clone()),
            label: keyed.label().unwrap_or_else(|| keyed.wire_key()),
            kind: kind_for(keyed),
            relation_distance: compare_pitch_motion(focus, keyed)
                .map(|motion| motion.total_semitones.min(u16::from(u8::MAX)) as u8)
                .unwrap_or(u8::MAX),
            keyed: keyed.clone(),
        })
        .collect::<Vec<_>>();

    let mut relations = Vec::new();
    for (index, left) in keys.iter().enumerate() {
        for right in keys.iter().skip(index + 1) {
            let Some(shared) = shared_tones(left, right) else {
                continue;
            };
            relations.push(StageContextRelation {
                from: StageNodeId::Catalog(left.clone()),
                to: StageNodeId::Catalog(right.clone()),
                kind: StageContextRelationKind::SharedTones,
                shared_tones: shared,
            });
        }
    }
    StageContextGraph {
        nodes,
        relations,
        truncated,
    }
}

pub const CENTER: (f32, f32) = (-180.0, 0.0);
pub const ZERO_RADIUS: f32 = 70.0;

/// Scale against the complete supported catalog, independent of visible nodes.
pub fn semitone_unit(anchor: &KeyedCatalogRef) -> Option<f32> {
    Some(620.0 / f32::from(maximum_motion(anchor)?.max(1)))
}

pub fn guides(anchor: &KeyedCatalogRef) -> Vec<(u16, f32)> {
    let Some(maximum) = maximum_motion(anchor) else {
        return Vec::new();
    };
    let step = maximum.div_ceil(6).max(1);
    (0..=maximum)
        .step_by(usize::from(step))
        .map(|cost| {
            (
                cost,
                ZERO_RADIUS + f32::from(cost) * 620.0 / f32::from(maximum.max(1)),
            )
        })
        .collect()
}

fn maximum_motion(anchor: &KeyedCatalogRef) -> Option<u16> {
    type Cache = std::sync::Mutex<Vec<(KeyedCatalogRef, u16)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Some((_, value)) = cache.lock().unwrap().iter().find(|(key, _)| key == anchor) {
        return Some(*value);
    }
    let tones = crate::harmony::keyed_pitch_classes(&anchor.to_material()?)?;
    if tones.is_empty() {
        return None;
    }
    let maximum = formula_slots()
        .iter()
        .map(String::as_str)
        .chain(["chord:Major", "chord:Minor"])
        .flat_map(|formula| {
            (0..12).map(move |root| KeyedCatalogRef {
                formula_id: formula.into(),
                root: woodshedding::pitch::PitchClass::new(root),
            })
        })
        .filter_map(|key| compare_pitch_motion(anchor, &key).map(|motion| motion.total_semitones))
        .max()
        .unwrap_or(0);
    let mut entries = cache.lock().unwrap();
    if entries.len() >= 4 {
        entries.remove(0);
    }
    entries.push((anchor.clone(), maximum));
    Some(maximum)
}

fn formula_slots() -> &'static Vec<String> {
    static FORMULAS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    FORMULAS.get_or_init(|| {
        let mut formulas = woodshedding::chord::catalog()
            .iter()
            .map(|item| format!("chord:{}", item.name))
            .chain(
                woodshedding::scale::catalog()
                    .iter()
                    .map(|item| format!("scale:{}", item.name)),
            )
            .filter(|id| id != "chord:Major" && id != "chord:Minor")
            .collect::<Vec<_>>();
        formulas.sort();
        formulas.dedup();
        formulas
    })
}

fn angular_slot(keyed: &KeyedCatalogRef) -> Option<f32> {
    let offset = match keyed.formula_id.as_str() {
        "chord:Major" => 0.0,
        "chord:Minor" => 1.0,
        _ => {
            (formula_slots().binary_search(&keyed.formula_id).ok()? + 1) as f32
                / (formula_slots().len() + 1) as f32
        },
    };
    Some(f32::from(keyed.root.value()) * 2.0 + offset)
}

fn identity_fraction(identity: &str) -> f32 {
    let hash = identity.bytes().fold(0x811c9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
    });
    hash as f32 / u32::MAX as f32
}

/// A separate two-dimensional shelf, never a fabricated distance value.
pub fn unscored_position(identity: &str) -> (f32, f32) {
    let x = identity_fraction(identity);
    let y = identity_fraction(&format!("row:{identity}"));
    (590.0 + x * 220.0, -650.0 + y * 1300.0)
}

/// Every scored subject is on the zero ring plus its exact summed distance.
/// Catalog angles use full formula/root identity. Occurrences use their durable
/// CardId within that angular slot, preserving positions across reorder/removal.
pub fn subject_position(
    keyed: Option<&KeyedCatalogRef>,
    anchor: Option<&KeyedCatalogRef>,
    occurrence: Option<u64>,
) -> (f32, f32) {
    let identity = occurrence
        .map(|id| format!("card:{id}"))
        .unwrap_or_else(|| {
            keyed
                .map(KeyedCatalogRef::wire_key)
                .unwrap_or_else(|| "unknown".into())
        });
    let Some((keyed, anchor)) = keyed.zip(anchor) else {
        return unscored_position(&identity);
    };
    let Some(motion) = compare_pitch_motion(anchor, keyed) else {
        return unscored_position(&identity);
    };
    let Some((slot, unit)) = angular_slot(keyed).zip(semitone_unit(anchor)) else {
        return unscored_position(&identity);
    };
    // A narrow jitter would leave repeated zero-cost Cards on the same pointer
    // target. Spread occurrences around their cost ring with a stable angle.
    let offset = occurrence
        .map(|id| ((id as f64 * 0.6180339887498949).fract() * 24.0) as f32)
        .unwrap_or(0.0);
    let angle = (slot + offset) / 24.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let radius = ZERO_RADIUS + f32::from(motion.total_semitones) * unit;
    (
        CENTER.0 + radius * angle.cos(),
        CENTER.1 + radius * angle.sin(),
    )
}

pub fn position(keyed: &KeyedCatalogRef, anchor: &KeyedCatalogRef) -> Option<(f32, f32)> {
    Some(subject_position(Some(keyed), Some(anchor), None))
}

fn kind_for(keyed: &KeyedCatalogRef) -> StageNodeKind {
    if keyed.formula_id.starts_with("scale:") {
        StageNodeKind::Scale
    } else {
        StageNodeKind::Chord
    }
}

fn shared_tones(left: &KeyedCatalogRef, right: &KeyedCatalogRef) -> Option<usize> {
    let shared = crate::harmony::compare_pitch_sets(left, right)?
        .shared
        .len();
    (shared > 0).then_some(shared)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keyed(formula: &str, root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: formula.into(),
            root: PitchClass::new(root),
        }
    }
    #[test]
    fn equal_pitch_sets_and_repeated_occurrences_have_distinct_zero_ring_targets() {
        let anchor = keyed("chord:Augmented", 0);
        let other = keyed("chord:Augmented", 4);
        assert_eq!(
            compare_pitch_motion(&anchor, &other)
                .unwrap()
                .total_semitones,
            0
        );
        let points = [
            subject_position(Some(&anchor), Some(&anchor), Some(1)),
            subject_position(Some(&anchor), Some(&anchor), Some(2)),
            subject_position(Some(&other), Some(&anchor), None),
        ];
        assert!(points.windows(2).all(|pair| pair[0] != pair[1]));
        for (x, y) in points {
            assert!(((x - CENTER.0).hypot(y - CENTER.1) - ZERO_RADIUS).abs() < 0.001);
        }
    }
    #[test]
    fn radius_encodes_exact_motion_and_unscored_material_has_its_own_area() {
        let anchor = keyed("chord:Major", 0);
        let target = keyed("chord:Minor", 9);
        let (x, y) = subject_position(Some(&target), Some(&anchor), None);
        assert!(
            ((x - CENTER.0).hypot(y - CENTER.1)
                - ZERO_RADIUS
                - 2.0 * semitone_unit(&anchor).unwrap())
            .abs()
                < 0.001
        );
        let scale = keyed("scale:Major", 0);
        assert!(compare_pitch_motion(&anchor, &scale).is_none());
        assert!(subject_position(Some(&scale), Some(&anchor), None).0 >= 590.0);
        assert!(subject_position(None, Some(&anchor), Some(5)).0 >= 590.0);
    }
    #[test]
    fn all_initial_triad_slots_are_distinct() {
        let mut slots = BTreeSet::new();
        for formula in ["chord:Major", "chord:Minor"] {
            for root in 0..12 {
                assert!(slots.insert(angular_slot(&keyed(formula, root)).unwrap().to_bits()));
            }
        }
    }
}
