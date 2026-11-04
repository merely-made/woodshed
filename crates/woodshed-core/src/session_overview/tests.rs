use super::*;

fn admitted_plan(owner: crate::working_sets::WorkingSetId) -> LoopPlan {
    LoopPlan {
        owner,
        name: "Loaded backing".into(),
        one_shot: false,
        click: true,
        segments: vec![crate::loop_plan::LoopSegment {
            seconds: None,
            id: crate::loop_plan::LoopSegmentId {
                set: owner,
                card: woodshedding::rehearsal::CardId(7),
            },
            label: "Accepted Card".into(),
            bpm: 97.0,
            beats: 3,
            bars: 2,
            pitches_hz: vec![220.0, 277.182_62, 329.627_56],
            phrase: None,
        }],
    }
}

fn backing_overview(admitted_loop_plan: Option<&LoopPlan>) -> OverviewSnapshot {
    use crate::{
        catalog_explorations::{CatalogExplorationState, CatalogExplorations},
        song::SongBar,
        working_sets::WorkingSets,
    };
    let mut set = Set::default();
    let mut sets = WorkingSets::default();
    let editing = sets.create(&mut set, "Requested backing", Set::default());
    let song = SongDoc {
        name: "Authored legacy form".into(),
        bars: vec![SongBar {
            label: "Exact old material".into(),
            chord_label: "Custom pitches".into(),
            formula_name: "Custom formula".into(),
            silent: false,
            root_pc: 9,
            root_freq_hz: 217.3,
            pitches_hz: vec![217.3, 281.6, 336.2],
            bpm: 113.5,
            beats: 5,
            length: 3,
        }],
        one_shot: true,
        click: false,
    };
    let retained = RetainedSets::default();
    let history = PracticeHistory::default();
    let explorations = CatalogExplorations::default();
    let state = CatalogExplorationState::default();
    let views = [
        SessionView {
            id: "looper".into(),
            label: "Actual Looper view".into(),
            domain: SessionViewDomain::Looper,
        },
        SessionView {
            id: "rehearsal".into(),
            label: "Actual rehearsal view".into(),
            domain: SessionViewDomain::Rehearsal,
        },
    ];
    let payload = serde_json::to_value(&song).unwrap();
    let result = configured_session_overview(&ConfiguredSessionOverviewInput {
        session: SessionOverviewInput {
            working_set: &set,
            retained_sets: &retained,
            song: &song,
            history: &history,
            views: &views,
            activity: SessionActivity {
                rehearsal: true,
                looper: true,
                tuner: false,
            },
        },
        working_sets: &sets,
        explorations: &explorations,
        active_exploration: &state,
        runner_owner: Some(editing),
        admitted_loop_plan,
        loop_captures: &LoopCaptureManifest::default(),
    });
    assert_eq!(serde_json::to_value(&song).unwrap(), payload);
    assert_eq!(
        result
            .nodes
            .iter()
            .filter(|node| matches!(node.id, OverviewNodeId::View(_)))
            .count(),
        views.len()
    );
    assert!(
        result
            .relations
            .iter()
            .all(|edge| result.nodes.iter().any(|node| node.id == edge.from)
                && result.nodes.iter().any(|node| node.id == edge.to))
    );
    result
}

#[test]
fn admitted_backing_owner_is_independent_of_editing_and_rehearsal_owners() {
    use crate::working_sets::WorkingSetId;
    let graph = backing_overview(Some(&admitted_plan(WorkingSetId(1))));
    let owner =
        |id| OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(WorkingSetId(id)));
    for (from, to, kind) in [
        (
            OverviewNodeId::Process(OverviewProcess::Looper),
            owner(1),
            OverviewRelationKind::ActsOn,
        ),
        (
            OverviewNodeId::View("looper".into()),
            owner(1),
            OverviewRelationKind::Presents,
        ),
        (
            OverviewNodeId::Process(OverviewProcess::Rehearsal),
            owner(2),
            OverviewRelationKind::ActsOn,
        ),
    ] {
        let targets: Vec<_> = graph
            .relations
            .iter()
            .filter(|edge| edge.from == from && edge.kind == kind)
            .map(|edge| &edge.to)
            .collect();
        assert_eq!(targets, vec![&to]);
    }
    let song = graph
        .nodes
        .iter()
        .find(|node| node.id == OverviewNodeId::Artifact(SessionArtifactId::Song))
        .unwrap();
    assert_eq!(song.id.wire_key(), "artifact:song");
    assert_eq!(
        song.label,
        "Saved compatibility backing · Authored legacy form"
    );
    assert!(song.detail.starts_with("1 retained bars"));
}

#[test]
fn unavailable_admitted_owner_is_visible_without_reassignment_or_dangling_edges() {
    use crate::working_sets::WorkingSetId;
    let graph = backing_overview(Some(&admitted_plan(WorkingSetId(999))));
    for id in [
        OverviewNodeId::View("looper".into()),
        OverviewNodeId::Process(OverviewProcess::Looper),
    ] {
        assert!(!graph.relations.iter().any(|edge| edge.from == id
            && matches!(
                edge.kind,
                OverviewRelationKind::ActsOn | OverviewRelationKind::Presents
            )));
        let node = graph.nodes.iter().find(|node| node.id == id).unwrap();
        assert!(node.detail.contains("unavailable working Set 999"));
    }
    assert!(!graph.nodes.iter().any(|node| node.id
        == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(WorkingSetId(999)))));
    assert!(graph.relations.contains(&OverviewRelation {
        from: OverviewNodeId::Process(OverviewProcess::Rehearsal),
        to: OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(WorkingSetId(2))),
        kind: OverviewRelationKind::ActsOn,
    }));
}

#[test]
fn saved_backing_fallback_keeps_the_song_artifact_and_loaded_relations() {
    let graph = backing_overview(None);
    let song = OverviewNodeId::Artifact(SessionArtifactId::Song);
    for (from, kind) in [
        (
            OverviewNodeId::Process(OverviewProcess::Looper),
            OverviewRelationKind::ActsOn,
        ),
        (
            OverviewNodeId::View("looper".into()),
            OverviewRelationKind::Presents,
        ),
    ] {
        let targets: Vec<_> = graph
            .relations
            .iter()
            .filter(|edge| edge.from == from && edge.kind == kind)
            .map(|edge| &edge.to)
            .collect();
        assert_eq!(targets, vec![&song]);
    }
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == song)
            .unwrap()
            .label,
        "Saved compatibility backing · Authored legacy form"
    );
}

#[test]
fn recipe_overview_discloses_copied_setup_without_formula_pitch_edges() {
    use crate::{
        captured_arpeggio::CapturedArpeggio, harmony::KeyedCatalogRef,
        musical_context::MusicalContext, working_sets::WorkingSetId,
    };
    use woodshedding::{pitch::PitchClass, rehearsal::CardId};
    let subject = KeyedCatalogRef {
        formula_id: "scale-pattern:thirds:Major".into(),
        root: PitchClass::new(0),
    };
    let mut card = subject.to_card().unwrap();
    card.id = CardId(8);
    let recipe = CapturedArpeggio::capture(&card, &crate::StageState::new(), 94.0).unwrap();
    let mut context = MusicalContext::default();
    let id = context.keep_recipe(WorkingSetId(1), recipe).unwrap();
    context
        .keep(
            WorkingSetId(1),
            KeyedCatalogRef {
                formula_id: "chord:Major".into(),
                root: PitchClass::new(0),
            },
        )
        .unwrap();
    let mut snapshot = OverviewSnapshot {
        nodes: vec![OverviewNode {
            id: OverviewNodeId::Catalog("woodshed-catalog".into()),
            label: "Catalog".into(),
            detail: String::new(),
        }],
        relations: vec![],
    };
    append_musical_context(&mut snapshot, &context);
    let node = OverviewNodeId::Artifact(SessionArtifactId::ContextItem(id));
    let item = snapshot.nodes.iter().find(|item| item.id == node).unwrap();
    assert!(item.label.starts_with("Recipe · "));
    assert!(item.detail.contains("Copied from Card 8"));
    assert!(item.detail.contains("94 BPM"));
    assert!(
        snapshot
            .relations
            .iter()
            .any(|edge| edge.from == node && edge.kind == OverviewRelationKind::UsesCatalog)
    );
    assert!(
        !snapshot
            .relations
            .iter()
            .any(|edge| (edge.from == node || edge.to == node)
                && matches!(
                    edge.kind,
                    OverviewRelationKind::EqualTones
                        | OverviewRelationKind::ContainsTones
                        | OverviewRelationKind::SharesTones
                ))
    );
}

#[test]
fn kept_context_projects_exact_pitch_relations_and_only_existing_owners() {
    use crate::{
        harmony::KeyedCatalogRef, musical_context::MusicalContext, working_sets::WorkingSetId,
    };
    use woodshedding::pitch::PitchClass;
    let mut context = MusicalContext::default();
    let mut keep = |owner, formula: &str, root| {
        context
            .keep(
                WorkingSetId(owner),
                KeyedCatalogRef {
                    formula_id: formula.into(),
                    root: PitchClass::new(root),
                },
            )
            .unwrap()
    };
    let major = keep(1, "chord:Major", 0);
    let duplicate_tones = keep(2, "chord:Major", 0);
    let scale = keep(1, "scale:Major", 0);
    let minor = keep(1, "chord:Minor", 9);
    let disjoint = keep(1, "chord:Major", 1);
    let owner = OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(WorkingSetId(1)));
    let mut snapshot = OverviewSnapshot {
        nodes: vec![
            OverviewNode {
                id: owner.clone(),
                label: "Set".into(),
                detail: String::new(),
            },
            OverviewNode {
                id: OverviewNodeId::Catalog("woodshed-catalog".into()),
                label: "Catalog".into(),
                detail: String::new(),
            },
        ],
        relations: vec![],
    };
    let retained = serde_json::to_value(&context).unwrap();
    append_musical_context(&mut snapshot, &context);
    let node = |id| OverviewNodeId::Artifact(SessionArtifactId::ContextItem(id));
    let has = |from, to, kind| {
        snapshot
            .relations
            .contains(&OverviewRelation { from, to, kind })
    };
    assert!(has(
        node(major),
        node(duplicate_tones),
        OverviewRelationKind::EqualTones
    ));
    assert!(has(
        node(scale),
        node(major),
        OverviewRelationKind::ContainsTones
    ));
    assert!(has(
        node(major),
        node(minor),
        OverviewRelationKind::SharesTones
    ));
    assert!(
        !snapshot
            .relations
            .iter()
            .any(|edge| edge.from == node(major) && edge.to == node(disjoint))
    );
    assert!(has(node(major), owner, OverviewRelationKind::ContextFor));
    assert!(
        !snapshot
            .relations
            .iter()
            .any(|edge| edge.from == node(duplicate_tones)
                && edge.kind == OverviewRelationKind::ContextFor)
    );
    assert!(
        snapshot
            .nodes
            .iter()
            .find(|entry| entry.id == node(major))
            .unwrap()
            .detail
            .contains("C, E, G")
    );
    assert_eq!(serde_json::to_value(&context).unwrap(), retained);
    assert_eq!(
        overview_scene(&snapshot).tables.items.len(),
        snapshot.nodes.len()
    );
    let before = snapshot.clone();
    append_musical_context(&mut snapshot, &context);
    assert_eq!(snapshot, before);
}

#[test]
fn unavailable_context_retains_identity_without_claiming_pitch_facts() {
    use crate::musical_context::{ContextItemId, MusicalContext};
    let context: MusicalContext = serde_json::from_value(serde_json::json!({"entries":[{"id":7,"owner":1,"subject":{"formula_id":"chord:Deleted", "root":0}}]})).unwrap();
    let mut snapshot = OverviewSnapshot::default();
    append_musical_context(&mut snapshot, &context);
    assert_eq!(
        snapshot.nodes[0].id,
        OverviewNodeId::Artifact(SessionArtifactId::ContextItem(ContextItemId(7)))
    );
    assert!(
        snapshot.nodes[0]
            .detail
            .contains("association target unavailable")
    );
    assert!(snapshot.nodes[0].label.contains("@pc:0"));
    assert!(snapshot.relations.is_empty());
}

#[test]
fn parked_catalog_set_keeps_its_relation_while_manual_unknown_set_is_focused() {
    use crate::{
        catalog_explorations::{CatalogExplorationState, CatalogExplorations},
        working_sets::WorkingSets,
    };
    fn project(bank: &WorkingSets, current: &Set) -> OverviewSnapshot {
        let retained = RetainedSets::default();
        let song = SongDoc::default();
        let history = PracticeHistory::default();
        let explorations = CatalogExplorations::default();
        let state = CatalogExplorationState::default();
        configured_session_overview(&ConfiguredSessionOverviewInput {
            session: SessionOverviewInput {
                working_set: current,
                retained_sets: &retained,
                song: &song,
                history: &history,
                views: &[],
                activity: SessionActivity::default(),
            },
            working_sets: bank,
            explorations: &explorations,
            active_exploration: &state,
            runner_owner: None,
            admitted_loop_plan: None,
            loop_captures: &LoopCaptureManifest::default(),
        })
    }
    let mut current = Set::default();
    current.push(crate::StageState::new().card_from_lens().unwrap());
    let mut bank = WorkingSets::default();
    let known = bank.active_id;
    let mut manual = Set::default();
    let mut card = crate::StageState::new().card_from_lens().unwrap();
    card.material = woodshedding::rehearsal::Material::Chord {
        name: "Unknown hand-written formula".into(),
        root: woodshedding::pitch::PitchClass::new(0),
    };
    manual.push(card);
    let unknown = bank.create(&mut current, "Manual", manual);
    let edge = OverviewRelation {
        from: OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(known)),
        to: OverviewNodeId::Catalog("woodshed-catalog".into()),
        kind: OverviewRelationKind::UsesCatalog,
    };
    let parked = project(&bank, &current);
    assert_eq!(
        parked
            .relations
            .iter()
            .filter(|relation| **relation == edge)
            .count(),
        1
    );
    assert!(!parked.relations.iter().any(|relation| relation.from
        == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(unknown))
        && relation.kind == OverviewRelationKind::UsesCatalog));
    assert!(bank.activate(known, &mut current));
    let focused = project(&bank, &current);
    assert_eq!(
        focused
            .relations
            .iter()
            .filter(|relation| **relation == edge)
            .count(),
        1
    );
    assert!(!focused.relations.iter().any(|relation| relation.from
        == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(unknown))
        && relation.kind == OverviewRelationKind::UsesCatalog));
}

fn take(set: crate::working_sets::WorkingSetId, card: u64) -> crate::loop_capture::LoopCaptureRef {
    crate::loop_capture::LoopCaptureRef {
        seconds: None,
        clock: None,
        id: LoopSegmentId {
            set,
            card: woodshedding::rehearsal::CardId(card),
        },
        bpm: 120.0,
        beats: 4,
        bars: 2,
        sample_rate: 48_000,
        frames: 192_000,
        digest: "ab".repeat(32),
        codec: Default::default(),
    }
}

fn take_overview(
    set: &Set,
    sets: &crate::working_sets::WorkingSets,
    admitted: Option<&LoopPlan>,
    manifest: &LoopCaptureManifest,
) -> OverviewSnapshot {
    use crate::catalog_explorations::{CatalogExplorationState, CatalogExplorations};
    configured_session_overview(&ConfiguredSessionOverviewInput {
        session: SessionOverviewInput {
            working_set: set,
            retained_sets: &RetainedSets::default(),
            song: &SongDoc::default(),
            history: &PracticeHistory::default(),
            views: &[],
            activity: SessionActivity::default(),
        },
        working_sets: sets,
        explorations: &CatalogExplorations::default(),
        active_exploration: &CatalogExplorationState::default(),
        runner_owner: None,
        admitted_loop_plan: admitted,
        loop_captures: manifest,
    })
}

fn riff(label: &str) -> woodshedding::rehearsal::Card {
    woodshedding::rehearsal::Card {
        id: woodshedding::rehearsal::CardId::UNASSIGNED,
        label: label.into(),
        material: woodshedding::rehearsal::Material::Riff {
            name: "Unlisted riff".into(),
        },
        setting: Default::default(),
        touch: Default::default(),
        timing: Default::default(),
        from: None,
    }
}

#[path = "tests/takes.rs"]
mod takes;

#[path = "tests/configured.rs"]
mod configured;
