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
    RelationshipReading,
    ContextItem(crate::musical_context::ContextItemId),
    WorkingSetInstance(crate::working_sets::WorkingSetId),
    Exploration(crate::catalog_explorations::CatalogExplorationId),
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
            Self::Artifact(SessionArtifactId::ContextItem(id)) => {
                format!("artifact:musical-context:{}", id.0)
            },
            Self::Artifact(SessionArtifactId::RelationshipReading) => {
                "artifact:relationship-reading".into()
            },
            Self::Artifact(SessionArtifactId::WorkingSetInstance(id)) => {
                format!("artifact:working-set:{}", id.0)
            },
            Self::Artifact(SessionArtifactId::Exploration(id)) => {
                format!("artifact:catalog-exploration:{}", id.0)
            },
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
    CapturedReadingOf,
    Presents,
    ActsOn,
    Records,
    UsesCatalog,
    ContextFor,
    EqualTones,
    ContainsTones,
    SharesTones,
}

impl OverviewRelationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::HistoricalSnapshotOf => "saved from working Set",
            Self::CapturedReadingOf => "captured from working Set",
            Self::Presents => "presents",
            Self::ActsOn => "acts on",
            Self::Records => "records catalog engagement",
            Self::UsesCatalog => "uses catalog material",
            Self::ContextFor => "kept nearby for",
            Self::EqualTones => "same pitch classes",
            Self::ContainsTones => "contains all pitch classes of",
            Self::SharesTones => "shares pitch classes with",
        }
    }
    pub fn stable_id(self) -> &'static str {
        match self {
            Self::HistoricalSnapshotOf => "woodshed:historical-set-snapshot",
            Self::CapturedReadingOf => "woodshed:captured-reading",
            Self::Presents => "woodshed:presents",
            Self::ActsOn => "woodshed:acts-on",
            Self::Records => "woodshed:records-engagement",
            Self::UsesCatalog => "woodshed:uses-catalog",
            Self::ContextFor => "woodshed:context-for",
            Self::EqualTones => "woodshed:equal-pitch-classes",
            Self::ContainsTones => "woodshed:contains-pitch-classes",
            Self::SharesTones => "woodshed:shares-pitch-classes",
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

/// Append explicit authored context and exact pitch-class membership facts.
/// Relations describe sounding sets, never harmonic function or recommendation.
/// Imported over-capacity payloads remain retained, but disclosure is bounded.
pub fn append_musical_context(
    snapshot: &mut OverviewSnapshot,
    context: &crate::musical_context::MusicalContext,
) {
    use crate::musical_context::MAX_CONTEXT_ITEMS;
    use OverviewNodeId::{Artifact, Catalog};
    use OverviewRelationKind::{ContainsTones, ContextFor, EqualTones, SharesTones, UsesCatalog};
    use woodshedding::pitch::{Pitch, Spelling};
    let catalog = Catalog("woodshed-catalog".into());
    let mut valid = Vec::new();
    for item in context.items().iter().take(MAX_CONTEXT_ITEMS) {
        // Duplicate imported IDs must not mint ambiguous actionable targets.
        if context.get(item.id).is_none() {
            continue;
        }
        let id = Artifact(SessionArtifactId::ContextItem(item.id));
        if snapshot.nodes.iter().any(|node| node.id == id) {
            continue;
        }
        let mut label = item
            .subject
            .label()
            .filter(|_| item.available())
            .unwrap_or_else(|| format!("Unavailable {}", item.subject.wire_key()));
        if item.captured_recipe.is_some() {
            label = format!("Recipe · {label}");
        }
        let pitches = (item.captured_recipe.is_none() && item.available())
            .then(|| item.subject.pitch_classes())
            .flatten();
        let tone_names = pitches.as_ref().map(|tones| {
            tones
                .iter()
                .map(|pc| {
                    let pitch = Pitch::from_midi(60 + i32::from(pc.value()), Spelling::Sharps);
                    format!("{}{}", pitch.name, pitch.accidental)
                })
                .collect::<Vec<_>>()
                .join(", ")
        });
        let owner = Artifact(SessionArtifactId::WorkingSetInstance(item.owner));
        let owner_description = snapshot
            .nodes
            .iter()
            .find(|node| node.id == owner)
            .map(|node| node.label.clone())
            .unwrap_or_else(|| {
                format!(
                    "working Set {} (association target unavailable)",
                    item.owner.0
                )
            });
        let mut detail = match tone_names {
            Some(tones) => format!(
                "Kept nearby for {}. Pitch classes: {}. This association does not add a Card or imply harmonic function.",
                owner_description, tones
            ),
            None => format!(
                "Kept nearby for {}. Catalog material is unavailable; retained identity is preserved. Open, Hear and Add are unavailable.",
                owner_description
            ),
        };
        if let Some(recipe) = &item.captured_recipe {
            detail = format!("Kept nearby for {}. {}", owner_description, recipe.detail());
        }
        snapshot.nodes.push(OverviewNode {
            id: id.clone(),
            label,
            detail,
        });
        if item.available() && snapshot.nodes.iter().any(|node| node.id == catalog) {
            snapshot.relations.push(OverviewRelation {
                from: id.clone(),
                to: catalog.clone(),
                kind: UsesCatalog,
            });
        }
        if snapshot.nodes.iter().any(|node| node.id == owner) {
            snapshot.relations.push(OverviewRelation {
                from: id.clone(),
                to: owner,
                kind: ContextFor,
            });
        }
        if let Some(tones) = pitches {
            valid.push((id, tones));
        }
    }
    for (index, (left, left_tones)) in valid.iter().enumerate() {
        for (right, right_tones) in valid.iter().skip(index + 1) {
            let (from, to, kind) = if left_tones == right_tones {
                (left, right, EqualTones)
            } else if left_tones.is_superset(right_tones) {
                (left, right, ContainsTones)
            } else if right_tones.is_superset(left_tones) {
                (right, left, ContainsTones)
            } else if !left_tones.is_disjoint(right_tones) {
                (left, right, SharesTones)
            } else {
                continue;
            };
            snapshot.relations.push(OverviewRelation {
                from: from.clone(),
                to: to.clone(),
                kind,
            });
        }
    }
}

fn set_uses_catalog(set: &Set) -> bool {
    set.cards.iter().any(|card| match &card.material {
        woodshedding::rehearsal::Material::Riff { name } => woodshedding::exercise::catalog()
            .iter()
            .any(|exercise| exercise.name == name),
        material => crate::harmony::KeyedCatalogRef::from_material(material)
            .and_then(|reference| reference.to_material())
            .is_some(),
    })
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
    if set_uses_catalog(input.working_set) {
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

pub struct ConfiguredSessionOverviewInput<'a> {
    pub session: SessionOverviewInput<'a>,
    pub working_sets: &'a crate::working_sets::WorkingSets,
    pub explorations: &'a crate::catalog_explorations::CatalogExplorations,
    pub active_exploration: &'a crate::catalog_explorations::CatalogExplorationState,
    /// The actual runner owner, even when it is parked in the bank.
    pub runner_owner: Option<crate::working_sets::WorkingSetId>,
}

/// Overview of multiple real owners. No legacy active-Set alias is emitted.
pub fn configured_session_overview(input: &ConfiguredSessionOverviewInput<'_>) -> OverviewSnapshot {
    use OverviewNodeId::{Artifact, Catalog, Process, View};
    use SessionArtifactId::{Exploration, WorkingSet, WorkingSetInstance};
    let mut result = session_overview(&input.session);
    result.nodes.retain(|node| node.id != Artifact(WorkingSet));
    // Legacy snapshots have no instance-qualified source. Do not assign them
    // whichever Set happens to be visible now.
    result
        .relations
        .retain(|edge| edge.kind != OverviewRelationKind::HistoricalSnapshotOf);
    for edge in &mut result.relations {
        if edge.from == Artifact(WorkingSet) {
            edge.from = Artifact(WorkingSetInstance(input.working_sets.active_id));
        }
        if edge.to == Artifact(WorkingSet) {
            edge.to = Artifact(WorkingSetInstance(input.working_sets.active_id));
        }
    }
    for summary in input.working_sets.summaries(input.session.working_set) {
        if input
            .working_sets
            .get(summary.id, input.session.working_set)
            .is_some_and(set_uses_catalog)
        {
            let edge = OverviewRelation {
                from: Artifact(WorkingSetInstance(summary.id)),
                to: Catalog("woodshed-catalog".into()),
                kind: OverviewRelationKind::UsesCatalog,
            };
            if !result.relations.contains(&edge) {
                result.relations.push(edge);
            }
        }
        result.nodes.push(OverviewNode {
            id: Artifact(WorkingSetInstance(summary.id)),
            label: summary.name,
            detail: format!(
                "{} ordered Cards. {}",
                summary.card_count,
                if summary.active {
                    "Currently open for editing."
                } else {
                    "Working Set available to open; its edits are retained."
                }
            ),
        });
    }
    for saved in &input.session.retained_sets.entries {
        if let Some(owner) = saved.source_working_set_id {
            if input
                .working_sets
                .get(owner, input.session.working_set)
                .is_some()
            {
                result.relations.push(OverviewRelation {
                    from: Artifact(SessionArtifactId::SavedSet(saved.id)),
                    to: Artifact(WorkingSetInstance(owner)),
                    kind: OverviewRelationKind::HistoricalSnapshotOf,
                });
            }
        }
    }
    for summary in input.explorations.summaries() {
        let Some(state) = input.explorations.get(summary.id, input.active_exploration) else {
            continue;
        };
        let id = Artifact(Exploration(summary.id));
        result.nodes.push(OverviewNode {
            id: id.clone(),
            label: summary.name,
            detail: format!(
                "Configured {} exploration. Selection, tuning, fretboard and catalog presentation are retained independently.",
                state.lens.label()
            ),
        });
        result.relations.push(OverviewRelation {
            from: id,
            to: Catalog("woodshed-catalog".into()),
            kind: OverviewRelationKind::UsesCatalog,
        });
    }
    // Catalog-bearing views present the current configured exploration, not
    // an invented duplicate catalog inventory.
    for view in input.session.views {
        if matches!(
            view.domain,
            SessionViewDomain::Stage | SessionViewDomain::Catalog
        ) {
            result.relations.retain(|edge| {
                !(edge.from == View(view.id.clone())
                    && edge.kind == OverviewRelationKind::Presents
                    && matches!(edge.to, Catalog(_)))
            });
            result.relations.push(OverviewRelation {
                from: View(view.id.clone()),
                to: Artifact(Exploration(input.explorations.active_id)),
                kind: OverviewRelationKind::Presents,
            });
        }
    }
    result.relations.retain(|edge| {
        !(edge.from == Process(OverviewProcess::Rehearsal)
            && edge.kind == OverviewRelationKind::ActsOn)
    });
    if input.session.activity.rehearsal {
        if let Some(owner) = input.runner_owner.filter(|owner| {
            input
                .working_sets
                .get(*owner, input.session.working_set)
                .is_some()
        }) {
            result.relations.push(OverviewRelation {
                from: Process(OverviewProcess::Rehearsal),
                to: Artifact(WorkingSetInstance(owner)),
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
        let owner =
            OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(WorkingSetId(1)));
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
            && edge.to
                == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(background))
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
