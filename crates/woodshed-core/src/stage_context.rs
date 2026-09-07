//! Derived keyed context for the Stage graph.
//!
//! This is a read model. It names catalog realizations around a focus but does
//! not add cards, reorder a Set, or turn a formula relation into keyed truth.

use std::collections::BTreeSet;

use woodshedding::pitch::PitchClass;
use woodshedding::rehearsal::CardId;

use crate::harmony::{KeyedCatalogRef, compare_pitch_sets};

/// A durable subject identity rendered by the Stage scene.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StageNodeId {
    /// One authored occurrence in the current Set.
    Card(CardId),
    /// A keyed catalog realization shown as derived context.
    Catalog(KeyedCatalogRef),
}

impl StageNodeId {
    pub fn wire_key(&self) -> String {
        match self {
            Self::Card(id) => format!("card:{}", id.0),
            Self::Catalog(reference) => reference.wire_key(),
        }
    }
}

/// The family of a node's material. It is product meaning exposed to views;
/// Sceno continues to carry only a generic item representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StageNodeKind {
    Card,
    Chord,
    Scale,
}

/// One quiet catalog realization in the Circle-of-Fifths reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageContextNode {
    pub id: StageNodeId,
    pub keyed: KeyedCatalogRef,
    pub label: String,
    pub kind: StageNodeKind,
    /// Distance in the selected reading: fifth steps or P/L/R transformations.
    pub relation_distance: u8,
}

/// A relation the keyed context can state without borrowing formula-graph
/// semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StageContextRelationKind {
    SharedTones,
    Diatonic,
    Fifth,
}

impl StageContextRelationKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::SharedTones => "woodshed:keyed-shared-tones",
            Self::Diatonic => "woodshed:keyed-diatonic",
            Self::Fifth => "woodshed:circle-of-fifths",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SharedTones => "shares tones",
            Self::Diatonic => "diatonic in",
            Self::Fifth => "a fifth apart",
        }
    }
}

/// One real relation within the bounded context, including context-to-context
/// links. `shared_tones` is meaningful only for the corresponding kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageContextRelation {
    pub from: StageNodeId,
    pub to: StageNodeId,
    pub kind: StageContextRelationKind,
    pub shared_tones: usize,
}

/// Request for the Circle-of-Fifths context around a keyed focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageContextOptions {
    /// The musical reading owns candidate selection, placement, and relations.
    pub reading: crate::settings::StageGraphReading,
    /// The view-local focus. A `Card` is resolved to its keyed material by the
    /// scene adapter; catalog focus can be used directly after a background
    /// click. `None` means the adapter chooses the Set cursor.
    pub focus: Option<StageNodeId>,
    /// Maximum number of quiet catalog nodes, excluding foreground Set cards.
    pub node_limit: usize,
    /// Already disclosed keyed nodes. They are retained before newly discovered
    /// neighbors so changing focus expands the reading instead of replacing it.
    pub retained: BTreeSet<KeyedCatalogRef>,
}

impl Default for StageContextOptions {
    fn default() -> Self {
        Self {
            reading: crate::settings::StageGraphReading::CircleOfFifths,
            focus: None,
            node_limit: 24,
            retained: BTreeSet::new(),
        }
    }
}

/// The derived background and its induced relations. Coordinates are assigned
/// by the scene adapter, allowing the view to retain user-adjusted positions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StageContextGraph {
    pub nodes: Vec<StageContextNode>,
    pub relations: Vec<StageContextRelation>,
    /// More eligible material existed than the requested context budget.
    pub truncated: bool,
}

/// Build a bounded, deterministic circle-of-fifths neighborhood.
///
/// Major chords provide landmarks across all twelve tonics, followed by nearby
/// relative minor chords and scales before chord extensions;
/// `Major 7`, `Dominant 7`, and `Minor 7` expand the chord reading as room
/// permits. Existing foreground material is excluded by `omit` so it is never
/// duplicated as a quiet background card.
pub fn circle_of_fifths_context(
    focus: &KeyedCatalogRef,
    node_limit: usize,
    omit: &BTreeSet<KeyedCatalogRef>,
    retained: &BTreeSet<KeyedCatalogRef>,
) -> StageContextGraph {
    let mut retained_candidates = Vec::new();
    let mut fresh_candidates = Vec::new();
    let mut seen = BTreeSet::new();
    // Reducing breadth must keep the material being inspected visible.
    if !omit.contains(focus) && focus.to_material().is_some() {
        seen.insert(focus.clone());
        retained_candidates.push((0, kind_for(focus), focus.clone()));
    }
    for keyed in retained {
        if !omit.contains(keyed) && keyed.to_material().is_some() && seen.insert(keyed.clone()) {
            retained_candidates.push((
                fifth_distance(key_sector(focus), key_sector(keyed)),
                kind_for(keyed),
                keyed.clone(),
            ));
        }
    }
    let focus_sector = key_sector(focus);
    for (distance, major_root) in fifth_roots(focus_sector) {
        let minor_root = PitchClass::new(major_root.value() + 9);
        for (kind, formula_id, root) in [
            (StageNodeKind::Chord, "chord:Major", major_root),
            (StageNodeKind::Scale, "scale:Major", major_root),
            (StageNodeKind::Chord, "chord:Minor", minor_root),
            (StageNodeKind::Scale, "scale:Minor", minor_root),
            (StageNodeKind::Chord, "chord:Major 7", major_root),
            (StageNodeKind::Chord, "chord:Dominant 7", major_root),
            (StageNodeKind::Chord, "chord:Minor 7", minor_root),
        ] {
            let keyed = KeyedCatalogRef {
                formula_id: formula_id.into(),
                root,
            };
            if omit.contains(&keyed) || keyed.to_material().is_none() || !seen.insert(keyed.clone())
            {
                continue;
            }
            fresh_candidates.push((distance, kind, keyed));
        }
    }
    retained_candidates.sort_by(|(_, _, a), (_, _, b)| (a != focus, a).cmp(&(b != focus, b)));
    fresh_candidates.sort_by(|(a_distance, a_kind, a), (b_distance, b_kind, b)| {
        let extension = |keyed: &KeyedCatalogRef| {
            matches!(
                keyed.formula_id.as_str(),
                "chord:Major 7" | "chord:Dominant 7" | "chord:Minor 7"
            )
        };
        // Preserve the whole circle, then disclose basic material near focus
        // before extensions fill the remaining budget.
        let layer = |keyed: &KeyedCatalogRef| {
            if keyed.formula_id == "chord:Major" {
                0
            } else if extension(keyed) {
                2
            } else {
                1
            }
        };
        (layer(a), a_distance, a_kind, a).cmp(&(layer(b), b_distance, b_kind, b))
    });
    retained_candidates.extend(fresh_candidates);
    let mut candidates = retained_candidates;
    let truncated = candidates.len() > node_limit;
    candidates.truncate(node_limit);

    let nodes = candidates
        .into_iter()
        .filter_map(|(fifth_distance, kind, keyed)| {
            Some(StageContextNode {
                id: StageNodeId::Catalog(keyed.clone()),
                label: keyed.label()?,
                keyed,
                kind,
                relation_distance: fifth_distance,
            })
        })
        .collect::<Vec<_>>();
    let relations = context_relations(focus, &nodes);
    StageContextGraph {
        nodes,
        relations,
        truncated,
    }
}

fn kind_for(keyed: &KeyedCatalogRef) -> StageNodeKind {
    if keyed.formula_id.starts_with("scale:") {
        StageNodeKind::Scale
    } else {
        StageNodeKind::Chord
    }
}

fn fifth_roots(focus: PitchClass) -> Vec<(u8, PitchClass)> {
    // Alternate clockwise and anticlockwise fifths so a bounded disclosure
    // grows around its focus rather than filling one side of the circle first.
    let mut roots = vec![(0, focus)];
    for distance in 1..=6_u8 {
        roots.push((distance, PitchClass::new(focus.value() + 7 * distance)));
        let reverse = PitchClass::new(focus.value() + 12 - (7 * distance) % 12);
        if reverse != roots.last().map(|(_, root)| *root).unwrap_or(focus) {
            roots.push((distance, reverse));
        }
    }
    roots
}

fn fifth_distance(from: PitchClass, to: PitchClass) -> u8 {
    let clockwise = ((i16::from(to.value()) - i16::from(from.value())) * 7).rem_euclid(12) as u8;
    clockwise.min(12 - clockwise)
}

fn key_sector(keyed: &KeyedCatalogRef) -> PitchClass {
    if keyed.formula_id.contains("Minor") {
        PitchClass::new(keyed.root.value() + 3)
    } else {
        keyed.root
    }
}

fn context_relations(
    focus: &KeyedCatalogRef,
    nodes: &[StageContextNode],
) -> Vec<StageContextRelation> {
    let mut relations = Vec::new();
    let focus_id = StageNodeId::Catalog(focus.clone());
    for node in nodes {
        if node.keyed == *focus {
            continue;
        }
        if let Some(comparison) = compare_pitch_sets(focus, &node.keyed) {
            if !comparison.shared.is_empty() {
                relations.push(StageContextRelation {
                    from: focus_id.clone(),
                    to: node.id.clone(),
                    kind: StageContextRelationKind::SharedTones,
                    shared_tones: comparison.shared.len(),
                });
            }
            if focus.formula_id.starts_with("chord:")
                && node.kind == StageNodeKind::Scale
                && comparison.left_only.is_empty()
            {
                relations.push(StageContextRelation {
                    from: focus_id.clone(),
                    to: node.id.clone(),
                    kind: StageContextRelationKind::Diatonic,
                    shared_tones: comparison.shared.len(),
                });
            }
        }
    }
    for (index, left) in nodes.iter().enumerate() {
        for right in nodes.iter().skip(index + 1) {
            let tonic_delta = (left.keyed.root.value() + 12 - right.keyed.root.value()) % 12;
            if tonic_delta == 5 || tonic_delta == 7 {
                relations.push(StageContextRelation {
                    from: left.id.clone(),
                    to: right.id.clone(),
                    kind: StageContextRelationKind::Fifth,
                    shared_tones: 0,
                });
            }
            if let Some(comparison) = compare_pitch_sets(&left.keyed, &right.keyed) {
                if !comparison.shared.is_empty() {
                    relations.push(StageContextRelation {
                        from: left.id.clone(),
                        to: right.id.clone(),
                        kind: StageContextRelationKind::SharedTones,
                        shared_tones: comparison.shared.len(),
                    });
                }
                let (chord, scale) = match (left.kind, right.kind) {
                    (StageNodeKind::Chord, StageNodeKind::Scale) => (left, right),
                    (StageNodeKind::Scale, StageNodeKind::Chord) => (right, left),
                    _ => continue,
                };
                if let Some(diatonic) = compare_pitch_sets(&chord.keyed, &scale.keyed) {
                    if diatonic.left_only.is_empty() {
                        relations.push(StageContextRelation {
                            from: chord.id.clone(),
                            to: scale.id.clone(),
                            kind: StageContextRelationKind::Diatonic,
                            shared_tones: diatonic.shared.len(),
                        });
                    }
                }
            }
        }
    }
    relations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn major(root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: "chord:Major".into(),
            root: PitchClass::new(root),
        }
    }

    #[test]
    fn circle_context_is_bounded_and_excludes_foreground_identity() {
        let focus = major(0);
        let context = circle_of_fifths_context(
            &focus,
            8,
            &BTreeSet::from([focus.clone()]),
            &BTreeSet::new(),
        );
        assert_eq!(context.nodes.len(), 8);
        assert!(context.nodes.iter().all(|node| node.keyed != focus));
        assert!(context.nodes.iter().all(|node| node.label.contains(' ')));
    }

    #[test]
    fn context_keeps_neighbor_to_neighbor_fifth_relations() {
        let context = circle_of_fifths_context(&major(0), 16, &BTreeSet::new(), &BTreeSet::new());
        assert!(context.relations.iter().any(|relation| {
            relation.kind == StageContextRelationKind::Fifth
                && matches!(relation.from, StageNodeId::Catalog(_))
                && matches!(relation.to, StageNodeId::Catalog(_))
        }));
    }

    #[test]
    fn initial_c_major_context_includes_its_relative_minor_and_adjacent_keys() {
        let context = circle_of_fifths_context(&major(0), 16, &BTreeSet::new(), &BTreeSet::new());
        let ids = context
            .nodes
            .iter()
            .map(|node| node.keyed.wire_key())
            .collect::<BTreeSet<_>>();
        for expected in [
            "chord:Minor@pc:9",
            "scale:Minor@pc:9",
            "chord:Major@pc:7",
            "chord:Major@pc:5",
            "scale:Major@pc:0",
        ] {
            assert!(ids.contains(expected), "missing {expected}: {ids:?}");
        }
    }

    #[test]
    fn retained_distant_node_survives_focus_expansion_before_new_neighbors() {
        let retained = BTreeSet::from([KeyedCatalogRef {
            formula_id: "scale:Minor".into(),
            root: PitchClass::new(1),
        }]);
        let context =
            circle_of_fifths_context(&major(0), 1, &BTreeSet::from([major(0)]), &retained);
        assert_eq!(context.nodes.len(), 1);
        assert_eq!(context.nodes[0].keyed, retained.into_iter().next().unwrap());
        assert!(context.truncated);
    }

    #[test]
    fn reducing_breadth_keeps_the_focused_material_visible() {
        let focus = KeyedCatalogRef {
            formula_id: "scale:Minor".into(),
            root: PitchClass::new(9),
        };
        let retained = circle_of_fifths_context(&major(0), 36, &BTreeSet::new(), &BTreeSet::new())
            .nodes
            .into_iter()
            .map(|node| node.keyed)
            .collect();
        let context = circle_of_fifths_context(&focus, 12, &BTreeSet::new(), &retained);
        assert_eq!(context.nodes.len(), 12);
        assert_eq!(context.nodes[0].keyed, focus);
    }
}
