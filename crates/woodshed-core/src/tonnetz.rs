//! A finite, deterministic Tonnetz reading for the 24 major and minor triads.
//!
//! The lattice is deliberately a 4 by 3 fundamental patch, not an infinite
//! plane or a solver. Its `+1` pitch offset places C major at the interior
//! cell `(1, 1)`, so its P/L/R edges are literal in the initial reading.
//! Boundary pitch vertices repeat geometrically; adjacency wraps through triad
//! identity, while positions stay in their canonical patch.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use woodshedding::pitch::PitchClass;

use crate::harmony::KeyedCatalogRef;
use crate::stage_context::{StageContextGraph, StageContextNode, StageNodeId, StageNodeKind};

const PARALLEL: &str = "woodshed:tonnetz-parallel";
const RELATIVE: &str = "woodshed:tonnetz-relative";
const LEADING_TONE: &str = "woodshed:tonnetz-leading-tone";

/// The centroid of the triad's canonical triangle in world coordinates.
pub fn position(keyed: &KeyedCatalogRef) -> Option<(f32, f32)> {
    let triangle = triangle(keyed)?;
    Some((
        (triangle[0].0 + triangle[1].0 + triangle[2].0) / 3.0,
        (triangle[0].1 + triangle[1].1 + triangle[2].1) / 3.0,
    ))
}

/// The canonical triangle vertices, each carrying its sounding pitch class.
pub fn triangle(keyed: &KeyedCatalogRef) -> Option<[(f32, f32, PitchClass); 3]> {
    let (major, root) = triad_kind(keyed)?;
    let base = if major {
        root
    } else {
        PitchClass::new(root.value() + 8)
    };
    let (x, y) = canonical_base(base)?;
    let points = if major {
        [(x, y), (x + 1, y), (x, y + 1)]
    } else {
        [(x + 1, y), (x, y + 1), (x + 1, y + 1)]
    };
    Some(points.map(|(x, y)| {
        let (world_x, world_y) = lattice_world(x, y);
        (world_x, world_y, lattice_pitch(x, y))
    }))
}

/// The classical P/L/R neighbors, returned as stable relation slugs.
pub fn neighbors(keyed: &KeyedCatalogRef) -> Vec<(KeyedCatalogRef, &'static str)> {
    let Some((major, root)) = triad_kind(keyed) else {
        return Vec::new();
    };
    let chord = |name, root| KeyedCatalogRef {
        formula_id: format!("chord:{name}"),
        root,
    };
    if major {
        vec![
            (chord("Minor", root), PARALLEL),
            (
                chord("Minor", PitchClass::new(root.value() + 4)),
                LEADING_TONE,
            ),
            (chord("Minor", PitchClass::new(root.value() + 9)), RELATIVE),
        ]
    } else {
        vec![
            (chord("Major", root), PARALLEL),
            (
                chord("Major", PitchClass::new(root.value() + 8)),
                LEADING_TONE,
            ),
            (chord("Major", PitchClass::new(root.value() + 3)), RELATIVE),
        ]
    }
}

/// Bounded P/L/R discovery for a context adapter.
///
/// Unsupported focus falls back to C major. The shared context record's
/// relation distance carries real P/L/R BFS depth.
/// Relations intentionally remain empty: the scene adapter derives P/L/R
/// routes over the final displayed set so omitted staged focus cards still
/// participate and each route has one canonical owner.
pub fn context(
    focus: &KeyedCatalogRef,
    node_limit: usize,
    omit: &BTreeSet<KeyedCatalogRef>,
    retained: &BTreeSet<KeyedCatalogRef>,
) -> StageContextGraph {
    let seed = triad_kind(focus)
        .map(|_| focus.clone())
        .unwrap_or_else(c_major);
    let mut distances = BTreeMap::from([(seed.clone(), 0_u8)]);
    let mut queue = VecDeque::from([(seed, 0_u8)]);
    while let Some((current, depth)) = queue.pop_front() {
        for (next, _) in neighbors(&current) {
            if !distances.contains_key(&next) {
                distances.insert(next.clone(), depth.saturating_add(1));
                queue.push_back((next, depth.saturating_add(1)));
            }
        }
        if distances.len() >= 24 {
            break;
        }
    }
    let mut ordered = Vec::new();
    let mut emitted = BTreeSet::new();
    let mut add = |keyed: KeyedCatalogRef| {
        if !omit.contains(&keyed) {
            if let Some(depth) = distances.get(&keyed).copied() {
                if emitted.insert(keyed.clone()) {
                    ordered.push((keyed, depth));
                }
            }
        }
    };
    // The actual seed is first when it is supported; C major is the fallback.
    let seed = distances
        .iter()
        .find_map(|(keyed, depth)| (*depth == 0).then_some(keyed.clone()))
        .unwrap_or_else(c_major);
    add(seed);
    for keyed in retained {
        add(keyed.clone());
    }
    let mut fresh = distances
        .iter()
        .map(|(keyed, depth)| (keyed.clone(), *depth))
        .collect::<Vec<_>>();
    fresh.sort_by(|(a, a_depth), (b, b_depth)| (a_depth, a).cmp(&(b_depth, b)));
    for (keyed, _) in fresh {
        add(keyed);
    }
    let truncated = ordered.len() > node_limit;
    ordered.truncate(node_limit);
    let nodes = ordered
        .into_iter()
        .filter_map(|(keyed, depth)| {
            Some(StageContextNode {
                id: StageNodeId::Catalog(keyed.clone()),
                label: keyed.label()?,
                keyed,
                kind: StageNodeKind::Chord,
                relation_distance: depth,
            })
        })
        .collect();
    StageContextGraph {
        nodes,
        relations: Vec::new(),
        truncated,
    }
}

fn c_major() -> KeyedCatalogRef {
    KeyedCatalogRef {
        formula_id: "chord:Major".into(),
        root: PitchClass::new(0),
    }
}

fn triad_kind(keyed: &KeyedCatalogRef) -> Option<(bool, PitchClass)> {
    match keyed.formula_id.as_str() {
        "chord:Major" => Some((true, keyed.root)),
        "chord:Minor" => Some((false, keyed.root)),
        _ => None,
    }
}

fn canonical_base(pitch: PitchClass) -> Option<(i32, i32)> {
    for y in 0..=2 {
        for x in 0..=3 {
            if lattice_pitch(x, y) == pitch {
                return Some((x, y));
            }
        }
    }
    None
}

fn lattice_pitch(x: i32, y: i32) -> PitchClass {
    PitchClass::new((7 * x + 4 * y + 1).rem_euclid(12) as u8)
}

fn lattice_world(x: i32, y: i32) -> (f32, f32) {
    (
        x as f32 * 240.0 + y as f32 * 120.0 - 660.0,
        y as f32 * 208.0 - 312.0,
    )
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
    fn all_twenty_four_triads_have_unique_centroids() {
        let points = ["Major", "Minor"]
            .into_iter()
            .flat_map(|name| (0..12).map(move |root| position(&chord(name, root)).unwrap()))
            .map(|(x, y)| (x.to_bits(), y.to_bits()))
            .collect::<BTreeSet<_>>();
        assert_eq!(points.len(), 24);
    }

    #[test]
    fn every_triangle_matches_its_catalog_pitch_set() {
        for name in ["Major", "Minor"] {
            for root in 0..12 {
                let keyed = chord(name, root);
                let from_triangle = triangle(&keyed)
                    .unwrap()
                    .map(|point| point.2)
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    from_triangle,
                    keyed.pitch_classes().unwrap(),
                    "{name} {root}"
                );
            }
        }
    }

    #[test]
    fn c_major_has_expected_plr_neighbors_and_each_is_an_involution() {
        let c = chord("Major", 0);
        let found = neighbors(&c).into_iter().collect::<BTreeSet<_>>();
        assert!(found.contains(&(chord("Minor", 0), PARALLEL)));
        assert!(found.contains(&(chord("Minor", 4), LEADING_TONE)));
        assert!(found.contains(&(chord("Minor", 9), RELATIVE)));
        for name in ["Major", "Minor"] {
            for root in 0..12 {
                let source = chord(name, root);
                let pitches = source.pitch_classes().unwrap();
                for (neighbor, slug) in neighbors(&source) {
                    assert_eq!(
                        pitches
                            .intersection(&neighbor.pitch_classes().unwrap())
                            .count(),
                        2
                    );
                    assert!(
                        neighbors(&neighbor)
                            .into_iter()
                            .any(|(back, back_slug)| back == source && back_slug == slug)
                    );
                }
            }
        }
    }

    #[test]
    fn c_major_plr_neighbors_share_literal_triangle_edges_while_boundary_wraps() {
        let c = chord("Major", 0);
        let vertices = |keyed: &KeyedCatalogRef| {
            triangle(keyed)
                .unwrap()
                .map(|(x, y, _)| (x.to_bits(), y.to_bits()))
                .into_iter()
                .collect::<BTreeSet<_>>()
        };
        let c_vertices = vertices(&c);
        for (neighbor, _) in neighbors(&c) {
            assert_eq!(c_vertices.intersection(&vertices(&neighbor)).count(), 2);
        }
        let wraps = ["Major", "Minor"]
            .into_iter()
            .flat_map(|name| {
                (0..12).flat_map(move |root| {
                    let keyed = chord(name, root);
                    let source = vertices(&keyed);
                    neighbors(&keyed).into_iter().map(move |(neighbor, _)| {
                        source.intersection(&vertices(&neighbor)).count() < 2
                    })
                })
            })
            .any(|wrapped| wrapped);
        assert!(
            wraps,
            "at least one canonical boundary edge wraps by identity"
        );
    }

    #[test]
    fn context_retains_focus_before_fresh_neighbors_and_respects_bound() {
        let focus = chord("Major", 0);
        let retained = BTreeSet::from([chord("Minor", 1)]);
        let context = context(&focus, 2, &BTreeSet::new(), &retained);
        assert_eq!(context.nodes.len(), 2);
        assert_eq!(context.nodes[0].keyed, focus);
        assert_eq!(context.nodes[1].keyed, chord("Minor", 1));
        assert!(context.truncated);
    }

    #[test]
    fn retained_immediate_neighbors_do_not_stop_full_lattice_discovery() {
        let focus = chord("Major", 0);
        let retained = neighbors(&focus)
            .into_iter()
            .map(|(keyed, _)| keyed)
            .collect::<BTreeSet<_>>();
        let context = context(&focus, 24, &BTreeSet::new(), &retained);
        assert_eq!(context.nodes.len(), 24);
        assert!(context.nodes.iter().any(|node| node.relation_distance > 1));
    }

    #[test]
    fn unsupported_inputs_have_no_geometry_or_neighbors_and_context_falls_back() {
        let unsupported = KeyedCatalogRef {
            formula_id: "scale:Major".into(),
            root: PitchClass::new(0),
        };
        assert!(position(&unsupported).is_none());
        assert!(triangle(&unsupported).is_none());
        assert!(neighbors(&unsupported).is_empty());
        assert_eq!(
            context(&unsupported, 1, &BTreeSet::new(), &BTreeSet::new()).nodes[0].keyed,
            c_major()
        );
    }
}
