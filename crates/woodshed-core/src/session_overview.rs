//! Read-only overview of this session's real artifacts, views and active work.
//!
//! The caller supplies actual workspace view instances and live process facts.
//! Catalog nodes are references to their existing authority, not copied catalog
//! documents. Saved-copy relations describe historical derivation only.

use crate::{
    history::PracticeHistory,
    retained_sets::{RetainedSets, SavedSetId},
    song::SongDoc,
};
use sceno::{
    Footprint, InstanceId, ProjectedItem, Rect, Representation, RoutedRelation, Scene, Size2,
    SourceRef, Transform2, Vec2,
};
use scenotime::{Revision, SceneEpoch, SceneSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use woodshedding::rehearsal::Set;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SessionArtifactId {
    WorkingSet,
    SavedSet(SavedSetId),
    Song,
    History,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OverviewProcess {
    Rehearsal,
    Looper,
    Tuner,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OverviewNodeId {
    Artifact(SessionArtifactId),
    View(String),
    Process(OverviewProcess),
    Catalog(String),
}

impl OverviewNodeId {
    /// The source authority stays distinct from the workspace occurrence.
    pub fn source_ref(&self) -> SourceRef {
        let authority = match self {
            Self::Artifact(SessionArtifactId::History) => "woodshed.history",
            Self::Artifact(_) => "woodshed.session",
            Self::View(_) => "woodshed.workspace",
            Self::Process(_) => "woodshed.runtime",
            Self::Catalog(_) => "woodshed.catalog",
        };
        SourceRef::new(authority, self.wire_key())
    }

    pub fn wire_key(&self) -> String {
        match self {
            Self::Artifact(SessionArtifactId::WorkingSet) => "artifact:working-set".into(),
            Self::Artifact(SessionArtifactId::SavedSet(id)) => {
                format!("artifact:saved-set:{}", id.0)
            },
            Self::Artifact(SessionArtifactId::Song) => "artifact:song".into(),
            Self::Artifact(SessionArtifactId::History) => "artifact:practice-history".into(),
            Self::View(id) => format!("view:{id}"),
            Self::Process(process) => format!(
                "process:{}",
                match process {
                    OverviewProcess::Rehearsal => "rehearsal",
                    OverviewProcess::Looper => "looper",
                    OverviewProcess::Tuner => "tuner",
                }
            ),
            Self::Catalog(id) => format!("catalog:{id}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionViewDomain {
    Stage,
    Catalog,
    Rehearsal,
    Looper,
    Tools,
    History,
    Overview,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionView {
    /// The actual workspace tile identity, encoded by the host/view layer.
    pub id: String,
    pub label: String,
    pub domain: SessionViewDomain,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SessionActivity {
    pub rehearsal: bool,
    pub looper: bool,
    pub tuner: bool,
}

pub struct SessionOverviewInput<'a> {
    pub working_set: &'a Set,
    pub retained_sets: &'a RetainedSets,
    pub song: &'a SongDoc,
    pub history: &'a PracticeHistory,
    pub views: &'a [SessionView],
    pub activity: SessionActivity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewNode {
    pub id: OverviewNodeId,
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OverviewRelationKind {
    HistoricalSnapshotOf,
    Presents,
    ActsOn,
    Records,
    UsesCatalog,
}

impl OverviewRelationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::HistoricalSnapshotOf => "saved from working Set",
            Self::Presents => "presents",
            Self::ActsOn => "acts on",
            Self::Records => "records catalog engagement",
            Self::UsesCatalog => "uses catalog material",
        }
    }
    pub fn stable_id(self) -> &'static str {
        match self {
            Self::HistoricalSnapshotOf => "woodshed:historical-set-snapshot",
            Self::Presents => "woodshed:presents",
            Self::ActsOn => "woodshed:acts-on",
            Self::Records => "woodshed:records-engagement",
            Self::UsesCatalog => "woodshed:uses-catalog",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewRelation {
    pub from: OverviewNodeId,
    pub to: OverviewNodeId,
    pub kind: OverviewRelationKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewSnapshot {
    pub nodes: Vec<OverviewNode>,
    pub relations: Vec<OverviewRelation>,
}

pub fn session_overview(input: &SessionOverviewInput<'_>) -> OverviewSnapshot {
    use OverviewNodeId::{Artifact, Catalog, Process, View};
    use SessionArtifactId::{History, SavedSet, Song, WorkingSet};
    let mut result = OverviewSnapshot::default();
    let catalog = Catalog("woodshed-catalog".into());
    result.nodes.extend([
        OverviewNode { id: Artifact(WorkingSet), label: "Working Set".into(), detail: format!("{} ordered Cards in the Set you are editing.", input.working_set.cards.len()) },
        OverviewNode { id: Artifact(Song), label: if input.song.name.trim().is_empty() { "Looper form".into() } else { input.song.name.clone() }, detail: format!("{} bars in the current Looper form.", input.song.bars.len()) },
        OverviewNode { id: Artifact(History), label: "Practice history".into(), detail: format!("{} recorded engagements. Observations retain their own event-time provenance.", input.history.len()) },
        OverviewNode { id: catalog.clone(), label: "Catalog".into(), detail: "Reference to Woodshed's theory and exercise catalogs. Catalog contents remain owned by their catalog authority.".into() },
    ]);
    if input
        .working_set
        .cards
        .iter()
        .any(|card| match &card.material {
            woodshedding::rehearsal::Material::Riff { name } => woodshedding::exercise::catalog()
                .iter()
                .any(|exercise| exercise.name == name),
            material => crate::harmony::KeyedCatalogRef::from_material(material)
                .and_then(|reference| reference.to_material())
                .is_some(),
        })
    {
        result.relations.push(OverviewRelation {
            from: Artifact(WorkingSet),
            to: catalog.clone(),
            kind: OverviewRelationKind::UsesCatalog,
        });
    }
    if !input.history.is_empty() {
        result.relations.push(OverviewRelation {
            from: Artifact(History),
            to: catalog.clone(),
            kind: OverviewRelationKind::Records,
        });
    }
    for saved in &input.retained_sets.entries {
        let id = Artifact(SavedSet(saved.id));
        if result.nodes.iter().any(|node| node.id == id) {
            continue;
        }
        result.nodes.push(OverviewNode {
            id: id.clone(),
            label: saved.name.clone(),
            detail: format!(
                "Saved copy of {} Cards. Later edits to the working Set leave this copy unchanged.",
                saved.set.cards.len()
            ),
        });
        result.relations.push(OverviewRelation {
            from: id,
            to: Artifact(WorkingSet),
            kind: OverviewRelationKind::HistoricalSnapshotOf,
        });
    }
    for view in input.views {
        let id = View(view.id.clone());
        if result.nodes.iter().any(|node| node.id == id) {
            continue;
        }
        result.nodes.push(OverviewNode { id: id.clone(), label: view.label.clone(), detail: format!("{} presents one workspace view. Its presentation state is separate from the underlying instructions.", view.label) });
        let subjects = match view.domain {
            SessionViewDomain::Stage => vec![Artifact(WorkingSet), catalog.clone()],
            SessionViewDomain::Catalog => vec![catalog.clone()],
            SessionViewDomain::Rehearsal => vec![Artifact(WorkingSet)],
            SessionViewDomain::Looper => vec![Artifact(Song)],
            SessionViewDomain::History => vec![Artifact(History)],
            SessionViewDomain::Tools | SessionViewDomain::Overview => Vec::new(),
        };
        for subject in subjects {
            result.relations.push(OverviewRelation {
                from: id.clone(),
                to: subject,
                kind: OverviewRelationKind::Presents,
            });
        }
    }
    for (active, process, label, artifact) in [
        (
            input.activity.rehearsal,
            OverviewProcess::Rehearsal,
            "Rehearsal running",
            Some(WorkingSet),
        ),
        (
            input.activity.looper,
            OverviewProcess::Looper,
            "Looper playing",
            Some(Song),
        ),
        (
            input.activity.tuner,
            OverviewProcess::Tuner,
            "Tuner listening",
            None,
        ),
    ] {
        if !active {
            continue;
        }
        let id = Process(process);
        result.nodes.push(OverviewNode { id:id.clone(), label:label.into(), detail:"Active process reported by the current host. Activity is not inferred from saved instructions and is not persisted here.".into() });
        if let Some(artifact) = artifact {
            result.relations.push(OverviewRelation {
                from: id,
                to: Artifact(artifact),
                kind: OverviewRelationKind::ActsOn,
            });
        }
    }
    result
}

/// Lower the typed read model into the shared portable scene contract. Identity
/// stays with the DTO; placement is presentation only and may be replaced.
pub fn overview_scene(overview: &OverviewSnapshot) -> SceneSnapshot {
    let mut scene = Scene::new();
    let mut indices = BTreeMap::new();
    let mut row_counts = [0usize; 4];
    let mut points = Vec::new();
    for node in &overview.nodes {
        let row = match node.id {
            OverviewNodeId::Artifact(_) => 0,
            OverviewNodeId::View(_) => 1,
            OverviewNodeId::Process(_) => 2,
            OverviewNodeId::Catalog(_) => 3,
        };
        let point = Vec2::new(row_counts[row] as f32 * 220.0, row as f32 * 180.0);
        row_counts[row] += 1;
        let instance = InstanceId(scene.items.len() as u32);
        indices.insert(node.id.clone(), instance);
        points.push(point);
        let source = scene.intern_source(node.id.source_ref());
        scene.items.push(ProjectedItem {
            source,
            space: Scene::WORLD,
            transform: Transform2::translation(point.x, point.y),
            footprint: Footprint::Rect {
                size: Size2::new(180.0, 100.0),
            },
            representation: Representation::Card,
            layer: 0,
            visible: true,
            hit: None,
            channels: Vec::new(),
        });
    }
    for relation in &overview.relations {
        let (Some(&from), Some(&to)) = (indices.get(&relation.from), indices.get(&relation.to))
        else {
            continue;
        };
        scene.relations.push(RoutedRelation {
            from,
            to,
            space: Scene::WORLD,
            points: vec![points[from.0 as usize], points[to.0 as usize]],
            kind: Some(relation.kind.stable_id().into()),
            weight: None,
        });
    }
    scene.bounds = Rect::new(
        Vec2::new(-120.0, -80.0),
        Size2::new(
            row_counts.into_iter().max().unwrap_or(1).max(1) as f32 * 220.0 + 20.0,
            720.0,
        ),
    );
    // Dense identities are projection-local. Changed read-model contents use a
    // new epoch so a retained consumer cannot interpret old instance references
    // against a freshly ordered overview.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for node in &overview.nodes {
        for value in [node.id.wire_key(), node.label.clone(), node.detail.clone()] {
            for byte in (value.len() as u64)
                .to_le_bytes()
                .into_iter()
                .chain(value.bytes())
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    for relation in &overview.relations {
        for value in [
            relation.from.wire_key(),
            relation.to.wire_key(),
            relation.kind.stable_id().into(),
        ] {
            for byte in (value.len() as u64)
                .to_le_bytes()
                .into_iter()
                .chain(value.bytes())
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    SceneSnapshot::from_dense(SceneEpoch(hash), Revision(0), scene).expect("valid overview scene")
}

#[cfg(test)]
mod tests {
    use super::*;
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
                .any(|node| node.id
                    == OverviewNodeId::Artifact(SessionArtifactId::SavedSet(saved_id)))
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
}
