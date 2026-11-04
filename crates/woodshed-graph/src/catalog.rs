use super::*;

/// The node id for a scale.
pub fn scale_id(name: &str) -> String {
    format!("scale:{name}")
}

/// Stable formula identity for a structured scale-degree pattern.
pub fn scale_pattern_id(name: &str, pattern: woodshedding::rehearsal::ScalePattern) -> String {
    format!("scale-pattern:{}:{name}", pattern.slug())
}

/// The exercise subject used by observations of a structured scale pattern.
pub fn scale_pattern_exercise_id(
    name: &str,
    pattern: woodshedding::rehearsal::ScalePattern,
) -> String {
    exercise_id(&format!("{}:{name}", pattern.stable_id()))
}

pub fn chord_approach_id(
    name: &str,
    direction: woodshedding::rehearsal::ApproachDirection,
) -> String {
    format!("chord-approach:{}:{name}", direction.slug())
}

pub fn chord_approach_exercise_id(
    name: &str,
    direction: woodshedding::rehearsal::ApproachDirection,
) -> String {
    exercise_id(&format!("{}:{name}", direction.stable_id()))
}

/// The node id for a chord.
pub fn chord_id(name: &str) -> String {
    format!("chord:{name}")
}

/// The node id for an arpeggio realization of a chord formula.
pub fn arpeggio_id(name: &str) -> String {
    format!("arpeggio:{name}")
}

/// The node id for a progression.
pub fn progression_id(name: &str) -> String {
    format!("progression:{name}")
}

/// The node id for an exercise.
pub fn exercise_id(name: &str) -> String {
    format!("exercise:{name}")
}

pub(super) fn relation(kind: u16) -> Relation {
    Relation::new(RelationClass::app(FAMILY, kind))
}

/// The pitch classes (semitones mod 12) an interval list touches.
pub(super) fn pitch_classes<'a>(
    intervals: impl IntoIterator<Item = &'a woodshedding::interval::Interval>,
) -> BTreeSet<i32> {
    intervals
        .into_iter()
        .map(|interval| interval.semitones().rem_euclid(12))
        .collect()
}

/// Build the theory catalog as a chartulary graph, log-backed under `log_id` so a
/// user's fork descends from it with provenance.
pub fn build_catalog_graph(log_id: &str) -> GraphLog<Container, Relation> {
    let mut graph = GraphLog::with_id(LogId::new(log_id));
    // chartulary edits are attributed now; the built-in theory catalog is
    // authored by the app itself, so every node and edge carries "catalog".
    let author = Author::new("catalog");

    // Nodes, one per catalog object. Insert all before connecting.
    for s in scale::catalog() {
        graph.insert_node(
            &author,
            Container::new(scale_id(s.name))
                .with_title(s.name)
                .with_tag("scale")
                .with_tag(format!("{:?}", s.category)),
        );
    }
    for c in chord::catalog() {
        graph.insert_node(
            &author,
            Container::new(chord_id(c.name))
                .with_title(c.name)
                .with_tag("chord")
                .with_tag(format!("{:?}", c.category)),
        );
        graph.insert_node(
            &author,
            Container::new(arpeggio_id(c.name))
                .with_title(c.name)
                .with_tag("arpeggio")
                .with_tag(format!("{:?}", c.category)),
        );
    }
    for p in progression::catalog() {
        graph.insert_node(
            &author,
            Container::new(progression_id(p.name))
                .with_title(p.name)
                .with_tag("progression")
                .with_tag(format!("{:?}", p.category)),
        );
    }
    for e in exercise::catalog() {
        graph.insert_node(
            &author,
            Container::new(exercise_id(e.name))
                .with_title(e.name)
                .with_tag("exercise")
                .with_tag(format!("{:?}", e.category)),
        );
    }

    // Edges: a progression contains each distinct chord its roles call for.
    for p in progression::catalog() {
        let mut seen = BTreeSet::new();
        for role in p.roles {
            let chord_name = role.quality.chord_formula_name();
            if seen.insert(chord_name) {
                graph.connect(
                    &author,
                    &progression_id(p.name),
                    &chord_id(chord_name),
                    relation(CONTAINS),
                );
            }
        }
    }

    // Edges: a chord fits a scale when its pitch classes are a subset of the
    // scale's. Computed from the real interval formulas.
    for c in chord::catalog() {
        let chord_pcs = pitch_classes(c.intervals);
        for s in scale::catalog() {
            let scale_pcs = pitch_classes(s.intervals);
            if chord_pcs.is_subset(&scale_pcs) {
                graph.connect(
                    &author,
                    &chord_id(c.name),
                    &scale_id(s.name),
                    relation(FITS_IN_SCALE),
                );
                graph.connect(
                    &author,
                    &arpeggio_id(c.name),
                    &scale_id(s.name),
                    relation(FITS_IN_SCALE),
                );
            }
        }
        graph.connect(
            &author,
            &arpeggio_id(c.name),
            &chord_id(c.name),
            relation(REALIZES),
        );
    }

    graph
}
