use super::*;

#[test]
fn takes_are_owner_qualified_navigation_nodes_and_orphans_stay_unassigned() {
    use crate::working_sets::{WorkingSetId, WorkingSets};
    let mut set = Set::default();
    let mut sets = WorkingSets::default();
    let first = sets.active_id;
    set.push(riff("Verse"));
    let verse = set.cards[0].id;
    let mut other = Set::default();
    other.push(riff("Bridge"));
    let bridge = other.cards[0].id;
    let second = sets.create(&mut set, "Second", other);
    // Card IDs are local to a Set: both owners hold the same local Card id.
    assert_eq!(verse, bridge);
    let missing_set = WorkingSetId(999);
    let mut manifest = LoopCaptureManifest {
        namespace: Some("0123456789abcdef0123456789abcdef".into()),
        ..Default::default()
    };
    manifest.takes = vec![
        take(first, verse.0),
        take(second, bridge.0),
        take(first, 77),
        take(missing_set, verse.0),
    ];
    manifest.validate().unwrap();
    let before = serde_json::to_value(&manifest).unwrap();
    let sets_before = serde_json::to_value(&sets).unwrap();
    let graph = take_overview(&set, &sets, None, &manifest);
    assert_eq!(serde_json::to_value(&manifest).unwrap(), before);
    assert_eq!(serde_json::to_value(&sets).unwrap(), sets_before);
    let takes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.id,
                OverviewNodeId::Artifact(SessionArtifactId::LoopTake(_))
            )
        })
        .collect();
    assert_eq!(takes.len(), 4);
    let id = |set, card| {
        OverviewNodeId::Artifact(SessionArtifactId::LoopTake(LoopSegmentId {
            set,
            card: woodshedding::rehearsal::CardId(card),
        }))
    };
    assert_eq!(
        id(second, bridge.0).wire_key(),
        format!("artifact:loop-take:{}:{}", second.0, bridge.0)
    );
    assert_ne!(
        id(first, verse.0).wire_key(),
        id(second, bridge.0).wire_key()
    );
    let node = |key: &OverviewNodeId| graph.nodes.iter().find(|node| &node.id == key).unwrap();
    assert_eq!(node(&id(first, verse.0)).label, "Take · Verse");
    assert_eq!(node(&id(second, bridge.0)).label, "Take · Bridge");
    assert!(
        node(&id(second, bridge.0))
            .detail
            .contains("once that backing is admitted")
    );
    for orphan in [id(first, 77), id(missing_set, verse.0)] {
        assert!(node(&orphan).label.starts_with("Take · Set "));
        assert!(node(&orphan).detail.contains("not reassigned"));
        assert!(
            !graph
                .relations
                .iter()
                .any(|edge| edge.from == orphan || edge.to == orphan)
        );
    }
    for (take, owner) in [(id(first, verse.0), first), (id(second, bridge.0), second)] {
        assert_eq!(
            graph
                .relations
                .iter()
                .filter(|edge| edge.from == take)
                .cloned()
                .collect::<Vec<_>>(),
            vec![OverviewRelation {
                from: take.clone(),
                to: OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(owner)),
                kind: OverviewRelationKind::CapturedFor,
            }]
        );
    }
    // Admission only changes lane disclosure; it never adds or moves takes.
    let mut plan = admitted_plan(second);
    plan.segments[0].id.card = bridge;
    let admitted = take_overview(&set, &sets, Some(&plan), &manifest);
    let admitted_node = admitted
        .nodes
        .iter()
        .find(|node| node.id == id(second, bridge.0))
        .unwrap();
    assert!(admitted_node.detail.contains("recorder lane is loaded"));
    assert!(
        admitted
            .nodes
            .iter()
            .find(|node| node.id == id(first, verse.0))
            .unwrap()
            .detail
            .contains("once that backing is admitted")
    );
}

#[test]
fn empty_manifest_projects_no_takes() {
    use crate::working_sets::WorkingSets;
    let mut set = Set::default();
    set.push(riff("Verse"));
    let sets = WorkingSets::default();
    let graph = take_overview(&set, &sets, None, &LoopCaptureManifest::default());
    assert!(!graph.nodes.iter().any(|node| matches!(
        node.id,
        OverviewNodeId::Artifact(SessionArtifactId::LoopTake(_))
    )));
    assert!(
        !graph
            .relations
            .iter()
            .any(|edge| edge.kind == OverviewRelationKind::CapturedFor)
    );
}

#[test]
fn refused_manifest_keeps_references_visible_and_explained() {
    use crate::working_sets::WorkingSets;
    let mut set = Set::default();
    set.push(riff("Verse"));
    let sets = WorkingSets::default();
    let card = set.cards[0].id.0;
    let manifest = LoopCaptureManifest {
        version: 99,
        namespace: Some("0123456789abcdef0123456789abcdef".into()),
        takes: vec![take(sets.active_id, card), take(sets.active_id, card)],
    };
    assert!(manifest.validate().is_err());
    let graph = take_overview(&set, &sets, None, &manifest);
    let takes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.id,
                OverviewNodeId::Artifact(SessionArtifactId::LoopTake(_))
            )
        })
        .collect();
    // Duplicate identities never mint ambiguous navigation targets.
    assert_eq!(takes.len(), 1);
    assert!(takes[0].detail.contains("manifest is refused"));
}

#[test]
fn take_edits_change_the_epoch_but_not_node_identity() {
    use crate::working_sets::WorkingSets;
    let mut set = Set::default();
    set.push(riff("Verse"));
    set.push(riff("Chorus"));
    let sets = WorkingSets::default();
    let owner = sets.active_id;
    let mut manifest = LoopCaptureManifest {
        namespace: Some("0123456789abcdef0123456789abcdef".into()),
        ..Default::default()
    };
    manifest.takes.push(take(owner, set.cards[1].id.0));
    let before = take_overview(&set, &sets, None, &manifest);
    manifest.takes.insert(0, take(owner, set.cards[0].id.0));
    let after = take_overview(&set, &sets, None, &manifest);
    assert_ne!(overview_scene(&before).epoch, overview_scene(&after).epoch);
    let chorus = OverviewNodeId::Artifact(SessionArtifactId::LoopTake(manifest.takes[1].id));
    let source = |graph: &OverviewSnapshot| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == chorus)
            .map(|node| node.id.source_ref())
    };
    assert_eq!(source(&before), source(&after));
    assert!(source(&before).is_some());
}
