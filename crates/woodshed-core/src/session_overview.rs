//! Read-only overview of this session's real artifacts, views and active work.
//!
//! The caller supplies actual workspace view instances and live process facts.
//! Catalog nodes are references to their existing authority, not copied catalog
//! documents. Saved-copy relations describe historical derivation only.

use crate::{
    history::PracticeHistory,
    loop_capture::{LoopCaptureManifest, MAX_CAPTURE_TAKES},
    loop_plan::{LoopPlan, LoopSegmentId},
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
    /// A retained Looper take, qualified by its owning Set and Card. The
    /// overview references the capture manifest entry; it never owns the take.
    LoopTake(LoopSegmentId),
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
            Self::Artifact(SessionArtifactId::LoopTake(id)) => {
                format!("artifact:loop-take:{}:{}", id.set.0, id.card.0)
            },
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
    CapturedFor,
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
            Self::CapturedFor => "take captured for a Card of",
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
            Self::CapturedFor => "woodshed:loop-take-of",
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
        OverviewNode { id: Artifact(Song), label: if input.song.name.trim().is_empty() { "Saved compatibility backing".into() } else { format!("Saved compatibility backing · {}", input.song.name) }, detail: format!("{} retained bars in the saved compatibility backing. Its authored material stays independent of working Sets.", input.song.bars.len()) },
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
    /// The host-accepted Set backing, retained when a requested replacement
    /// fails. None means the saved compatibility backing remains loaded.
    pub admitted_loop_plan: Option<&'a LoopPlan>,
    /// The persisted capture manifest. Takes are referenced, never written.
    pub loop_captures: &'a LoopCaptureManifest,
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
    if let Some(plan) = input.admitted_loop_plan {
        let owner = input
            .working_sets
            .get(plan.owner, input.session.working_set)
            .map(|_| Artifact(WorkingSetInstance(plan.owner)));
        // A loaded snapshot can outlive its source. Keep that absence visible
        // rather than retargeting it to the edited Set or the saved backing.
        let unavailable = format!(
            "Loaded Set backing belongs to unavailable working Set {}. Its owner is not reassigned.",
            plan.owner.0
        );
        for view in input.session.views {
            if view.domain != SessionViewDomain::Looper {
                continue;
            }
            let id = View(view.id.clone());
            result
                .relations
                .retain(|edge| !(edge.from == id && edge.kind == OverviewRelationKind::Presents));
            if let Some(owner) = &owner {
                result.relations.push(OverviewRelation {
                    from: id,
                    to: owner.clone(),
                    kind: OverviewRelationKind::Presents,
                });
            } else if let Some(node) = result.nodes.iter_mut().find(|node| node.id == id) {
                node.detail = unavailable.clone();
            }
        }
        let process = Process(OverviewProcess::Looper);
        result
            .relations
            .retain(|edge| !(edge.from == process && edge.kind == OverviewRelationKind::ActsOn));
        if input.session.activity.looper {
            if let Some(owner) = owner {
                result.relations.push(OverviewRelation {
                    from: process,
                    to: owner,
                    kind: OverviewRelationKind::ActsOn,
                });
            } else if let Some(node) = result.nodes.iter_mut().find(|node| node.id == process) {
                node.detail = unavailable;
            }
        }
    }
    append_loop_takes(&mut result, input);
    result
}

/// Retained takes from the persisted capture manifest. Saved-song bars are
/// positional and have no persisted PCM, so they never appear here. A take
/// whose Set or Card is gone keeps its identity and gains no relation; it is
/// not reassigned to whichever Set is open.
fn append_loop_takes(result: &mut OverviewSnapshot, input: &ConfiguredSessionOverviewInput<'_>) {
    use OverviewNodeId::Artifact;
    use SessionArtifactId::{LoopTake, WorkingSetInstance};
    let manifest = input.loop_captures;
    let refused = manifest.validate().err();
    for take in manifest.takes.iter().take(MAX_CAPTURE_TAKES) {
        let id = Artifact(LoopTake(take.id));
        if result.nodes.iter().any(|node| node.id == id) {
            continue;
        }
        let owner = input
            .working_sets
            .get(take.id.set, input.session.working_set);
        let set_name = owner.and_then(|_| {
            result
                .nodes
                .iter()
                .find(|node| node.id == Artifact(WorkingSetInstance(take.id.set)))
                .map(|node| node.label.clone())
        });
        let card = owner.and_then(|set| set.card(take.id.card));
        let seconds = if take.sample_rate > 0 {
            take.frames as f64 / f64::from(take.sample_rate)
        } else {
            0.0
        };
        let clock = format!(
            "{} × {}/4 at {} bpm, {:.1} s",
            take.bars, take.beats, take.bpm, seconds
        );
        let related = card.is_some() && set_name.is_some();
        let (label, mut detail) = match (card, set_name) {
            (Some(card), Some(set_name)) => {
                let lane = match input.admitted_loop_plan {
                    Some(plan) if plan.index_of(take.id).is_some() => {
                        "Its recorder lane is loaded in the Looper."
                    },
                    Some(plan) if plan.owner == take.id.set => {
                        "Its Card is not in the loaded backing; the take is kept."
                    },
                    _ => {
                        "Opening loads its Set in the Looper; the recorder lane appears once that backing is admitted."
                    },
                };
                (
                    format!("Take · {}", card.label),
                    format!(
                        "Looper take ({clock}) for {} in {set_name}. {lane}",
                        card.label
                    ),
                )
            },
            _ => (
                format!("Take · Set {} Card {}", take.id.set.0, take.id.card.0),
                format!(
                    "Looper take ({clock}) for Card {} of working Set {}, which is no longer available. The take is kept and not reassigned; clear it from the Looper's stored loops if it is no longer wanted.",
                    take.id.card.0, take.id.set.0
                ),
            ),
        };
        if let Some(reason) = &refused {
            detail = format!(
                "{detail} The capture manifest is refused ({reason}); references are kept unchanged."
            );
        }
        result.nodes.push(OverviewNode {
            id: id.clone(),
            label,
            detail,
        });
        if related {
            result.relations.push(OverviewRelation {
                from: id,
                to: Artifact(WorkingSetInstance(take.id.set)),
                kind: OverviewRelationKind::CapturedFor,
            });
        }
    }
}

#[cfg(test)]
mod tests;

mod context;

pub use context::append_musical_context;
use context::set_uses_catalog;

mod scene;

pub use scene::overview_scene;
