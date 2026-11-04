/// A catalog selection surfaced by the related-material projection. It is
/// intentionally small and copyable so views can route it through a click
/// without carrying graph implementation types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelatedTarget {
    Scale(usize),
    Chord(usize),
    Arpeggio(usize),
    Progression(usize),
    Exercise(usize),
}

/// Take `limit` suggestions without letting one relation family fill the panel.
///
/// A display policy, not a change to relation truth: nothing is dropped that a
/// longer list would not also drop, and no record inside a suggestion is
/// touched. It exists because the raw ranking is honest but unhelpful at six
/// rows — `Major` appears in nine catalog progressions, all scoring 96, so the
/// harmonic neighbours that make the frontier worth reading never appeared.
/// Round-robin by the primary relation kind keeps the strongest of each family
/// first, in score order within a family.
pub(super) fn diversify(
    suggestions: Vec<RelatedSuggestion>,
    limit: usize,
) -> Vec<RelatedSuggestion> {
    use std::collections::BTreeMap;

    if suggestions.len() <= limit {
        return suggestions;
    }
    // Preserve rank inside each family; families keep the order in which their
    // strongest member appeared, so the overall best suggestion is still first.
    let mut families: Vec<Vec<RelatedSuggestion>> = Vec::new();
    let mut seen: BTreeMap<woodshed_graph::RelationKind, usize> = BTreeMap::new();
    for suggestion in suggestions {
        let kind = suggestion
            .relations
            .first()
            .map(|relation| relation.kind)
            .unwrap_or(woodshed_graph::RelationKind::SharesTones);
        match seen.get(&kind) {
            Some(&index) => families[index].push(suggestion),
            None => {
                seen.insert(kind, families.len());
                families.push(vec![suggestion]);
            },
        }
    }
    let mut out = Vec::with_capacity(limit);
    let mut round = 0;
    while out.len() < limit {
        let mut placed = false;
        for family in &mut families {
            if round < family.len() {
                out.push(family[round].clone());
                placed = true;
                if out.len() == limit {
                    break;
                }
            }
        }
        if !placed {
            break;
        }
        round += 1;
    }
    out
}

/// One ranked neighbor of the focused material, carrying **every** relation
/// that connects them rather than the one that ranked highest. Practice
/// evidence joins that list as another record; it no longer overwrites the
/// theory relation that was there first.
#[derive(Clone, Debug, PartialEq)]
pub struct RelatedSuggestion {
    pub title: String,
    pub kind: &'static str,
    /// Strongest first, always at least one.
    pub relations: Vec<woodshed_graph::MaterialRelation>,
    pub score: u16,
    pub target: RelatedTarget,
}

impl RelatedSuggestion {
    /// The explanation a one-line row shows.
    pub fn reason(&self) -> &str {
        self.relations
            .first()
            .map(|relation| relation.explanation.as_str())
            .unwrap_or_default()
    }

    /// Every explanation, which is what an expanded row or a selected edge
    /// should show: all applicable reasons, each with its own authority.
    pub fn reasons(&self) -> Vec<&str> {
        self.relations
            .iter()
            .map(|relation| relation.explanation.as_str())
            .collect()
    }

    /// How many relations connect this pair. `1` is the common case; more is
    /// what the old flattened boundary used to discard.
    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    /// Whether observed practice is among the reasons.
    pub fn has_evidence(&self) -> bool {
        self.relations
            .iter()
            .any(|relation| relation.authority == woodshed_graph::RelationAuthority::Evidence)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeighborhoodNode {
    pub id: String,
    pub title: String,
    pub kind: &'static str,
    pub score: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NeighborhoodSnapshot {
    pub nodes: Vec<NeighborhoodNode>,
    pub edges: Vec<(u16, u16)>,
}
