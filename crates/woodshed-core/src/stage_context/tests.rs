use super::*;

#[test]
fn chromatic_approaches_disclose_bounded_target_relations_and_respect_pins() {
    let focus = KeyedCatalogRef {
        formula_id: "chord:Major 7".into(),
        root: PitchClass::new(0),
    };
    let mut graph = circle_of_fifths_context(
        &focus,
        5,
        &BTreeSet::from([focus.clone()]),
        &BTreeSet::new(),
    );
    let retained_above = KeyedCatalogRef {
        formula_id: "chord-approach:above:Major 7".into(),
        root: focus.root,
    };
    graph.nodes.push(StageContextNode {
        id: StageNodeId::Catalog(retained_above.clone()),
        label: retained_above.label().unwrap(),
        keyed: retained_above.clone(),
        kind: StageNodeKind::ChordApproach,
        relation_distance: 0,
    });
    disclose_chord_approaches(
        &mut graph,
        &focus,
        6,
        &BTreeSet::new(),
        &BTreeSet::from([retained_above.clone()]),
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.keyed == retained_above)
            .unwrap()
            .label,
        "Cmaj7 · above"
    );
    assert_eq!(
        retained_above.label().unwrap(),
        "C Major 7 approach from above"
    );
    assert_eq!(graph.nodes.len(), 6);
    let approach_labels = graph
        .nodes
        .iter()
        .filter(|node| node.kind == StageNodeKind::ChordApproach)
        .map(|node| node.label.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        approach_labels,
        BTreeSet::from(["Cmaj7 · below", "Cmaj7 · above"])
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == StageNodeKind::ChordApproach)
            .count(),
        2
    );
    assert_eq!(
        graph
            .relations
            .iter()
            .filter(|relation| relation.kind == StageContextRelationKind::TargetApproach)
            .count(),
        2
    );
    let pinned = graph
        .nodes
        .iter()
        .map(|node| node.keyed.clone())
        .collect::<BTreeSet<_>>();
    let original = graph.nodes.clone();
    disclose_chord_approaches(
        &mut graph,
        &KeyedCatalogRef {
            formula_id: "chord:Minor".into(),
            root: PitchClass::new(2),
        },
        6,
        &BTreeSet::new(),
        &pinned,
    );
    assert_eq!(graph.nodes, original);
}

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
    let context = circle_of_fifths_context(&major(0), 1, &BTreeSet::from([major(0)]), &retained);
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

#[test]
fn arpeggio_disclosure_is_rooted_bounded_and_explains_the_relation() {
    let focus = KeyedCatalogRef {
        formula_id: "chord:Major 7".into(),
        root: PitchClass::new(2),
    };
    let omit = BTreeSet::from([focus.clone()]);
    let mut graph = circle_of_fifths_context(&focus, 4, &omit, &BTreeSet::new());
    disclose_chord_arpeggio(&mut graph, &focus, 4, &omit, &BTreeSet::new());
    assert_eq!(graph.nodes.len(), 4);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.kind == StageNodeKind::Arpeggio
                && node.keyed.wire_key() == "arpeggio:Major 7@pc:2")
    );
    assert!(graph.relations.iter().any(|relation| relation.kind
        == StageContextRelationKind::Arpeggiation
        && relation.shared_tones == 4));
    assert!(graph.relations.iter().all(|relation| relation.from
        == StageNodeId::Catalog(focus.clone())
        || graph.nodes.iter().any(|node| node.id == relation.from)));
}

#[test]
fn arpeggio_disclosure_preserves_pins_and_rejects_stale_formulas() {
    let focus = major(0);
    let mut graph = circle_of_fifths_context(
        &focus,
        1,
        &BTreeSet::from([focus.clone()]),
        &BTreeSet::new(),
    );
    let pinned = graph.nodes.iter().map(|node| node.keyed.clone()).collect();
    let original = graph.clone();
    disclose_chord_arpeggio(&mut graph, &focus, 1, &BTreeSet::new(), &pinned);
    assert_eq!(graph.nodes, original.nodes);
    let invalid = KeyedCatalogRef {
        formula_id: "chord:Missing".into(),
        root: PitchClass::new(0),
    };
    let original = graph.clone();
    disclose_chord_arpeggio(&mut graph, &invalid, 2, &BTreeSet::new(), &BTreeSet::new());
    assert_eq!(graph, original);
}

#[test]
fn degree_patterns_disclose_two_bounded_typed_subjects_without_editing_scale() {
    let focus = KeyedCatalogRef {
        formula_id: "scale:Major".into(),
        root: PitchClass::new(2),
    };
    let mut graph = circle_of_fifths_context(
        &focus,
        4,
        &BTreeSet::from([focus.clone()]),
        &BTreeSet::new(),
    );
    disclose_scale_patterns(
        &mut graph,
        &focus,
        4,
        &BTreeSet::from([focus.clone()]),
        &BTreeSet::new(),
    );
    assert_eq!(graph.nodes.len(), 4);
    for slug in ["thirds", "fourths"] {
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.kind == StageNodeKind::ScalePattern
                    && node.keyed.wire_key() == format!("scale-pattern:{slug}:Major@pc:2"))
        );
    }
    assert_eq!(
        graph
            .relations
            .iter()
            .filter(|relation| relation.kind == StageContextRelationKind::DegreePattern)
            .count(),
        2
    );
    assert!(graph.truncated);
    let mut empty = StageContextGraph::default();
    disclose_scale_patterns(&mut empty, &focus, 0, &BTreeSet::new(), &BTreeSet::new());
    assert!(empty.nodes.is_empty() && empty.truncated);
    let unsupported = KeyedCatalogRef {
        formula_id: "scale:Major Pentatonic".into(),
        root: PitchClass::new(0),
    };
    disclose_scale_patterns(
        &mut empty,
        &unsupported,
        2,
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    assert!(empty.nodes.is_empty());
}
