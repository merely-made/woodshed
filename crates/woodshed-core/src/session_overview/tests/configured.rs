use super::*;

#[test]
fn configured_overview_uses_real_banks_and_background_runner_owner() {
    use crate::{
        catalog_explorations::{CatalogExplorationState, CatalogExplorations},
        working_sets::WorkingSets,
    };
    let mut set = Set::default();
    let mut sets = WorkingSets::default();
    let background = sets.active_id;
    let visible = sets.create(&mut set, "Visible", Set::default());
    let mut retained = RetainedSets::default();
    let legacy_saved = retained.save_snapshot(&set, "Legacy copy");
    let owned_saved = retained.save_snapshot_from(&set, "Owned copy", background);
    let mut exploration_state = CatalogExplorationState::default();
    let mut explorations = CatalogExplorations::default();
    explorations.create(
        &mut exploration_state,
        "Alternative",
        CatalogExplorationState::default(),
    );
    let song = SongDoc::default();
    let history = PracticeHistory::default();
    let views = [SessionView {
        id: "stage".into(),
        label: "Stage".into(),
        domain: SessionViewDomain::Stage,
    }];
    let input = ConfiguredSessionOverviewInput {
        session: SessionOverviewInput {
            working_set: &set,
            retained_sets: &retained,
            song: &song,
            history: &history,
            views: &views,
            activity: SessionActivity {
                rehearsal: true,
                ..Default::default()
            },
        },
        working_sets: &sets,
        explorations: &explorations,
        active_exploration: &exploration_state,
        runner_owner: Some(background),
        admitted_loop_plan: None,
        loop_captures: &LoopCaptureManifest::default(),
    };
    let graph = configured_session_overview(&input);
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.id == OverviewNodeId::Artifact(SessionArtifactId::WorkingSet))
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| matches!(
                node.id,
                OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(_))
            ))
            .count(),
        2
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| matches!(
                node.id,
                OverviewNodeId::Artifact(SessionArtifactId::Exploration(_))
            ))
            .count(),
        2
    );
    let runner = graph
        .relations
        .iter()
        .find(|edge| {
            edge.from == OverviewNodeId::Process(OverviewProcess::Rehearsal)
                && edge.kind == OverviewRelationKind::ActsOn
        })
        .unwrap();
    assert_eq!(
        runner.to,
        OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(background))
    );
    assert_ne!(background, visible);
    assert!(!graph.relations.iter().any(|edge| edge.from
        == OverviewNodeId::Artifact(SessionArtifactId::SavedSet(legacy_saved))
        && edge.kind == OverviewRelationKind::HistoricalSnapshotOf));
    assert!(graph.relations.iter().any(|edge| edge.from
        == OverviewNodeId::Artifact(SessionArtifactId::SavedSet(owned_saved))
        && edge.to == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(background))
        && edge.kind == OverviewRelationKind::HistoricalSnapshotOf));
    assert_eq!(overview_scene(&graph).tables.items.len(), graph.nodes.len());
    let unknown = configured_session_overview(&ConfiguredSessionOverviewInput {
        runner_owner: Some(crate::working_sets::WorkingSetId(999)),
        ..input
    });
    assert!(!unknown.relations.iter().any(|edge| edge.from
        == OverviewNodeId::Process(OverviewProcess::Rehearsal)
        && edge.kind == OverviewRelationKind::ActsOn));
}

#[test]
fn views_present_their_real_domain_subjects() {
    let set = Set::default();
    let retained = RetainedSets::default();
    let song = SongDoc::default();
    let history = PracticeHistory::default();
    let views = [
        SessionView {
            id: "related".into(),
            label: "Related".into(),
            domain: SessionViewDomain::Catalog,
        },
        SessionView {
            id: "practice".into(),
            label: "Practice".into(),
            domain: SessionViewDomain::Stage,
        },
        SessionView {
            id: "rehearsal".into(),
            label: "Rehearsal".into(),
            domain: SessionViewDomain::Rehearsal,
        },
    ];
    let graph = session_overview(&SessionOverviewInput {
        working_set: &set,
        retained_sets: &retained,
        song: &song,
        history: &history,
        views: &views,
        activity: SessionActivity::default(),
    });
    let subjects = |id: &str| {
        graph
            .relations
            .iter()
            .filter(|edge| {
                edge.from == OverviewNodeId::View(id.into())
                    && edge.kind == OverviewRelationKind::Presents
            })
            .map(|edge| edge.to.clone())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let working = OverviewNodeId::Artifact(SessionArtifactId::WorkingSet);
    let catalog = OverviewNodeId::Catalog("woodshed-catalog".into());
    assert_eq!(
        subjects("related"),
        std::collections::BTreeSet::from([catalog.clone()])
    );
    assert_eq!(
        subjects("practice"),
        std::collections::BTreeSet::from([working.clone(), catalog])
    );
    assert_eq!(
        subjects("rehearsal"),
        std::collections::BTreeSet::from([working])
    );
}

#[test]
fn only_known_material_claims_catalog_usage() {
    use woodshedding::{pitch::PitchClass, rehearsal::Material};
    let mut set = Set::default();
    let mut card = crate::StageState::new().card_from_lens().unwrap();
    card.material = Material::Chord {
        name: "Unknown manual formula".into(),
        root: PitchClass::new(0),
    };
    set.push(card);
    let retained = RetainedSets::default();
    let song = SongDoc::default();
    let history = PracticeHistory::default();
    let input = |working_set| SessionOverviewInput {
        working_set,
        retained_sets: &retained,
        song: &song,
        history: &history,
        views: &[],
        activity: SessionActivity::default(),
    };
    assert!(
        !session_overview(&input(&set))
            .relations
            .iter()
            .any(|edge| edge.kind == OverviewRelationKind::UsesCatalog)
    );
    let mut known_set = set.clone();
    known_set.cards[0].material = Material::Chord {
        name: "Major".into(),
        root: PitchClass::new(0),
    };
    assert!(
        session_overview(&input(&known_set))
            .relations
            .iter()
            .any(|edge| edge.kind == OverviewRelationKind::UsesCatalog)
    );
}

#[test]
fn real_instances_and_activity_are_separate_and_projection_is_read_only() {
    let set = Set::default();
    let song = SongDoc::default();
    let history = PracticeHistory::default();
    let mut retained = RetainedSets::default();
    let saved_id = retained.save_snapshot(&set, "Study");
    let before = serde_json::to_value(&retained).unwrap();
    let views = vec![SessionView {
        id: "tile-7".into(),
        label: "Stage view".into(),
        domain: SessionViewDomain::Stage,
    }];
    let input = SessionOverviewInput {
        working_set: &set,
        retained_sets: &retained,
        song: &song,
        history: &history,
        views: &views,
        activity: SessionActivity::default(),
    };
    let graph = session_overview(&input);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.id == OverviewNodeId::Artifact(SessionArtifactId::SavedSet(saved_id)))
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| matches!(node.id, OverviewNodeId::View(_)))
            .count(),
        1
    );
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| matches!(node.id, OverviewNodeId::Process(_)))
    );
    assert_eq!(serde_json::to_value(&retained).unwrap(), before);
    let active = session_overview(&SessionOverviewInput {
        activity: SessionActivity {
            rehearsal: true,
            looper: false,
            tuner: true,
        },
        ..input
    });
    assert_eq!(
        active
            .nodes
            .iter()
            .filter(|node| matches!(node.id, OverviewNodeId::Process(_)))
            .count(),
        2
    );
    assert_eq!(
        active
            .relations
            .iter()
            .filter(|edge| edge.kind == OverviewRelationKind::ActsOn)
            .count(),
        1
    );
    let scene = overview_scene(&active);
    assert_ne!(
        OverviewNodeId::Artifact(SessionArtifactId::WorkingSet)
            .source_ref()
            .adapter,
        OverviewNodeId::Catalog("woodshed-catalog".into())
            .source_ref()
            .adapter
    );
    assert_ne!(
        OverviewNodeId::View("tile-7".into()).source_ref().adapter,
        OverviewNodeId::Process(OverviewProcess::Rehearsal)
            .source_ref()
            .adapter
    );
    assert_eq!(scene.tables.items.len(), active.nodes.len());
    assert_eq!(scene.tables.relations.len(), active.relations.len());
    assert_eq!(overview_scene(&active).epoch, scene.epoch);
    assert_ne!(overview_scene(&graph).epoch, scene.epoch);
}
