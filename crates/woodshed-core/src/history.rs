//! Portable practice-engagement history, over chartulary's lineage model.
//!
//! **Adopted stemma 2026-07-26.** This was a hand-rolled event log until the
//! strength model came up and the investigation found woodshed already depended
//! on a better one: `chartulary::stemma` maintains per-subject
//! `first_seen_at_ms` / `last_seen_at_ms` / `visit_count`, and aggregates
//! per-pair traversals with their own recency — exactly the inputs a decayed,
//! recency-weighted strength model needs, and exactly what the local log did not
//! keep. Building the model on the local log first would have been the throwaway
//! path.
//!
//! The split follows the same line as the typed relations: **woodshed owns the
//! musical judgment, the substrate owns the structure.** Stemma holds the dated,
//! branching lineage; woodshed names the engagement kinds and decides what they
//! weigh. So the vocabulary here is unchanged and every consumer still speaks
//! [`PracticeHistory`] — only the store beneath it moved.
//!
//! Two deliberate mappings:
//!
//! - **The kind lives in the visit's `context`, not in stemma's
//!   `TransitionKind`.** Those variants answer "how did you get here"
//!   (`LinkClick`, `Back`, `Reload`) and map one-to-one onto a browsing trace;
//!   previewed / staged / completed answer "what did you do with it". Forcing
//!   one into the other would be a costume, so the app's own semantics ride the
//!   payload slot built for them.
//! - **Datedness rides the context too.** `visit_entry` takes a `u64`, not an
//!   `Option`, so an undated engagement (a host with no clock, or a session
//!   written before woodshed kept time) records `at_ms = 0` with
//!   `dated: false`. A reader must never read that as 1970.

use chartulary::stemma::{EntryPrivacy, Stemma, TransitionKind};
use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use woodshedding::rehearsal::{Card, CardId, Material, Touch};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngagementKind {
    Previewed,
    Staged,
    Rehearsed,
    Completed,
    Looped,
    Recorded,
}

impl EngagementKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Previewed => "Previewed",
            Self::Staged => "Staged",
            Self::Rehearsed => "Rehearsed",
            Self::Completed => "Completed",
            Self::Looped => "Looped",
            Self::Recorded => "Recorded",
        }
    }

    /// Whether this kind is evidence of *practice* rather than of interest.
    ///
    /// The plan's own line: preview and staging are evidence of interest;
    /// completed rehearsal time is evidence of practice. Ranking leans on the
    /// distinction, so it lives on the kind rather than in each query's filter.
    pub fn is_practice(self) -> bool {
        !matches!(self, Self::Previewed | Self::Staged)
    }
}

/// Event-time instruction provenance. Missing provenance on older observations
/// stays missing: it must never be reconstructed from today's edited Set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationProvenance {
    /// Assigned Set identity; UNASSIGNED qualifies a catalog-only preview.
    pub occurrence_id: CardId,
    /// Owning working Set when observed. Missing on legacy observations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_set_id: Option<crate::working_sets::WorkingSetId>,
    /// Exact serialized Card, including material, setup, touch, timing and recipe.
    /// This is an instruction snapshot, not proof of the player's performance.
    /// Unspecified inherited settings remain unspecified unless the caller also
    /// supplies their resolved event-time context.
    pub card_snapshot: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Actual MIDI pitches presented by the runner, if observed. `None` means
    /// unknown; an empty vector means an observed silent event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presented_midi: Option<Vec<i32>>,
}

impl ObservationProvenance {
    pub fn capture_in_set(
        card: &Card,
        owner: crate::working_sets::WorkingSetId,
    ) -> Result<Self, serde_json::Error> {
        let mut provenance = Self::capture(card)?;
        provenance.working_set_id = Some(owner);
        Ok(provenance)
    }

    pub fn capture(card: &Card) -> Result<Self, serde_json::Error> {
        Ok(Self {
            occurrence_id: card.id,
            working_set_id: None,
            card_snapshot: serde_json::to_value(card)?,
            run_id: None,
            presented_midi: None,
        })
    }
}

/// What woodshed hangs on one stemma visit: the engagement's kind, the practice
/// span where there is one, whether its timestamp is real, and the subject the
/// player chose it *from*.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Engagement {
    pub kind: EngagementKind,
    /// The subject this engagement was chosen from, as the player's stated
    /// reason — staging a suggestion names where it came from.
    ///
    /// Deliberately not left to the lineage's own edge. Stemma parents a visit
    /// to the path actually walked, which is a different fact: "what I looked at
    /// before" is not "what I chose this from", and a first engagement has no
    /// parent at all yet can still name its source. Both facts are worth
    /// keeping, so the walked path stays stemma's and the stated reason rides
    /// here — the same division that puts the kind here rather than in
    /// `TransitionKind`.
    #[serde(default)]
    pub from_id: Option<String>,
    /// Elapsed practice, milliseconds, where the event has a span to measure —
    /// a completed rehearsal card. `None` for the instantaneous kinds.
    #[serde(default)]
    pub practiced_ms: Option<u64>,
    /// Whether the visit's `created_at_ms` is an observation or a placeholder.
    #[serde(default)]
    pub dated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ObservationProvenance>,
}

/// The practice lineage: catalog ids as entry keys, no entry payload (the
/// catalog owns names; history must not copy them), one owner, and woodshed's
/// [`Engagement`] as the per-visit context.
type Lineage = Stemma<String, (), String, Engagement>;

/// The single owner of the practice lineage. Woodshed has one practitioner, so
/// one cursor: the visit path is the practice path. Per-session owners would
/// branch it, and nothing asks for that yet.
const PRACTITIONER: &str = "practitioner";

/// One engagement, read back out. A view over the lineage rather than the
/// stored shape, so consumers keep the flat vocabulary they had.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PracticeEvent {
    pub subject_id: String,
    pub kind: EngagementKind,
    /// The explicitly stated source of this choice, independently of the walked path.
    #[serde(default)]
    pub from_id: Option<String>,
    /// Unix epoch milliseconds; `None` when the engagement is undated.
    #[serde(default)]
    pub at_ms: Option<u64>,
    #[serde(default)]
    pub practiced_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ObservationProvenance>,
}

/// One aggregated movement in the practice path. Catalog and history share
/// the same ids, so this can join the material graph without copying nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeTransition {
    pub from_id: String,
    pub to_id: String,
    pub traversal_count: u64,
    pub latest_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct PracticeHistory {
    lineage: Lineage,
}

impl PracticeHistory {
    /// Record an engagement at `at_ms` (Unix epoch milliseconds from the host;
    /// `None` where no clock is available), with `practiced_ms` where the
    /// engagement has a measured span.
    ///
    /// `from_id` is the player's stated reason and is stored as such; the
    /// lineage separately records the path actually walked. Keeping both is why
    /// a single "staged Minor 7 from Dorian" still carries its provenance even
    /// though it is the first engagement and has no parent visit.
    pub fn record(
        &mut self,
        at_ms: Option<u64>,
        subject_id: impl Into<String>,
        kind: EngagementKind,
        from_id: Option<String>,
        practiced_ms: Option<u64>,
    ) {
        self.record_observation(at_ms, subject_id, kind, from_id, practiced_ms, None);
    }

    /// Record an observation with explicitly captured event-time provenance.
    /// This records runner activity, never an assessment of successful playing.
    pub fn record_observation(
        &mut self,
        at_ms: Option<u64>,
        subject_id: impl Into<String>,
        kind: EngagementKind,
        from_id: Option<String>,
        practiced_ms: Option<u64>,
        provenance: Option<ObservationProvenance>,
    ) {
        let stamp = at_ms.unwrap_or_default();
        let owner = self.lineage.ensure_owner(PRACTITIONER.to_string(), None);
        let entry = self.lineage.resolve_or_create_entry(
            subject_id.into(),
            (),
            stamp,
            EntryPrivacy::LocalOnly,
        );
        // The lineage is local practice, and the transition vocabulary is
        // navigation's; `Unknown` is the honest value for a movement woodshed
        // does not describe in those terms. The kind rides the context.
        let _ = self.lineage.visit_entry(
            owner,
            entry,
            Engagement {
                kind,
                from_id,
                practiced_ms,
                dated: at_ms.is_some(),
                provenance,
            },
            TransitionKind::Unknown,
            stamp,
        );
    }

    /// How many times the player chose `subject_id` *from* `from_id`, counting
    /// only the kinds that are evidence of practice.
    ///
    /// The stated reason, not the walked path — this is what ranks a suggestion
    /// the player has taken before. For the path, see [`Self::traversals`].
    pub fn related_transition_count(&self, from_id: &str, subject_id: &str) -> usize {
        self.visits_of(subject_id)
            .filter(|context| context.kind.is_practice())
            .filter(|context| context.from_id.as_deref() == Some(from_id))
            .count()
    }

    /// Adjacent practice observations from one subject to another. Exploration
    /// interrupts adjacency; it is never silently skipped to invent a transition.
    /// Raw exploration and rehearsal visits are retained together in the lineage.
    pub fn traversals(&self, from_id: &str, subject_id: &str) -> (u64, Option<u64>) {
        self.transitions()
            .into_iter()
            .find(|edge| edge.from_id == from_id && edge.to_id == subject_id)
            .map(|edge| (edge.traversal_count, edge.latest_at_ms))
            .unwrap_or((0, None))
    }

    /// The most recent engagements, newest first, along the practice path.
    pub fn recent(&self, limit: usize) -> Vec<PracticeEvent> {
        let Some(owner) = self.lineage.owner_id_by_identity(&PRACTITIONER.to_string()) else {
            return Vec::new();
        };
        let path = self
            .lineage
            .linear_history_visits_of_owner(owner)
            .unwrap_or_default();
        let mut out = Vec::new();
        for visit_id in path.iter().rev().take(limit) {
            let Some(visit) = self.lineage.visit(*visit_id) else {
                continue;
            };
            let Some(entry) = self.lineage.entry(visit.entry) else {
                continue;
            };
            out.push(PracticeEvent {
                subject_id: entry.key.clone(),
                kind: visit.context.kind,
                from_id: visit.context.from_id.clone(),
                at_ms: visit.context.dated.then_some(visit.created_at_ms),
                practiced_ms: visit.context.practiced_ms,
                provenance: visit.context.provenance.clone(),
            });
        }
        out
    }

    /// Aggregate adjacent practice observations for the Overview's PracticedAfter
    /// layer. Both endpoints must be practice observations. Staging and previews
    /// remain in the raw lineage and engagement queries, but cannot produce a
    /// "practiced next" claim. Lifecycle observations of the same assigned
    /// occurrence do not form a transition. Legacy visits without occurrence
    /// provenance retain their qualified catalog-level adjacency.
    pub fn transitions(&self) -> Vec<PracticeTransition> {
        let events = self.recent(self.len());
        let mut edges = std::collections::BTreeMap::<(String, String), PracticeTransition>::new();
        for pair in events.windows(2) {
            let (to, from) = (&pair[0], &pair[1]);
            if !from.kind.is_practice() || !to.kind.is_practice() {
                continue;
            }
            if let (Some(from_provenance), Some(to_provenance)) = (&from.provenance, &to.provenance)
            {
                if from_provenance.occurrence_id.is_assigned()
                    && from_provenance.occurrence_id == to_provenance.occurrence_id
                    && from_provenance.working_set_id == to_provenance.working_set_id
                {
                    // Opening, pausing and completing one occurrence are
                    // observations of that occurrence, not movement to another.
                    // Loop evidence needs an explicit runner observation rather
                    // than inferring it from these lifecycle events.
                    continue;
                }
            }
            let key = (from.subject_id.clone(), to.subject_id.clone());
            let edge = edges.entry(key).or_insert_with(|| PracticeTransition {
                from_id: from.subject_id.clone(),
                to_id: to.subject_id.clone(),
                traversal_count: 0,
                latest_at_ms: None,
            });
            edge.traversal_count += 1;
            if let Some(at) = to.at_ms {
                edge.latest_at_ms = Some(edge.latest_at_ms.map_or(at, |previous| previous.max(at)));
            }
        }
        edges.into_values().collect()
    }

    /// Total measured practice on `subject_id`, milliseconds. An hour of
    /// rehearsal outweighs fifty previews rather than tying with them on count.
    pub fn total_practiced_ms(&self, subject_id: &str) -> u64 {
        self.visits_of(subject_id)
            .filter(|context| context.kind.is_practice())
            .filter_map(|context| context.practiced_ms)
            .sum()
    }

    /// When `subject_id` was last engaged, if any of its engagements are dated.
    /// The basis for recency: strength has to decay, and decay needs an age.
    pub fn last_seen_ms(&self, subject_id: &str) -> Option<u64> {
        let entry = self.lineage.entry_id_by_key(&subject_id.to_string())?;
        self.lineage
            .visits()
            .filter(|(_, visit)| visit.entry == entry && visit.context.dated)
            .map(|(_, visit)| visit.created_at_ms)
            .max()
    }

    /// How many times `subject_id` has been engaged at all, of any kind.
    /// Maintained by the lineage rather than counted here.
    pub fn engagement_count(&self, subject_id: &str) -> u64 {
        self.lineage
            .entry_id_by_key(&subject_id.to_string())
            .and_then(|id| self.lineage.entry(id))
            .map(|entry| entry.visit_count)
            .unwrap_or_default()
    }

    /// Whether any engagement is dated. A history restored from a session
    /// written before woodshed kept time answers false, which a
    /// recency-weighted reader must know before it ranks by age.
    pub fn has_times(&self) -> bool {
        self.lineage.visits().any(|(_, visit)| visit.context.dated)
    }

    /// Total engagements recorded, all subjects.
    pub fn len(&self) -> usize {
        self.lineage.visit_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn visits_of<'a>(&'a self, subject_id: &str) -> impl Iterator<Item = &'a Engagement> + 'a {
        let entry = self.lineage.entry_id_by_key(&subject_id.to_string());
        self.lineage
            .visits()
            .filter(move |(_, visit)| Some(visit.entry) == entry)
            .map(|(_, visit)| &visit.context)
    }
}

type HistorySnapshot = chartulary::stemma::StemmaSnapshot<String, (), String, Engagement>;

/// Durable-input guard for the locked supplier's trusted, infallible
/// `from_snapshot` (Eidetic 738ef159). That revision has no checked reconstruction
/// API. Keep reconstruction in the supplier; replace this guard when it gains
/// a fallible seam. Do not normalize malformed references or discard branches.
fn validate_history_snapshot(snapshot: &HistorySnapshot) -> Result<(), String> {
    use std::collections::HashSet;

    let fail = |reason: &str| Err(format!("Invalid practice history: {reason}"));
    let mut keys = HashSet::new();
    if snapshot
        .entries
        .iter()
        .any(|entry| !keys.insert(&entry.key))
    {
        return fail("duplicate entry key");
    }
    let mut identities = HashSet::new();
    if snapshot
        .owners
        .iter()
        .any(|owner| !identities.insert(&owner.identity))
    {
        return fail("duplicate owner identity");
    }
    let visits = snapshot.visits.len();
    let owners = snapshot.owners.len();
    let mut counts = vec![0u64; snapshot.entries.len()];
    // Bounds precede every cross-reference below and the supplier call.
    for visit in &snapshot.visits {
        if visit.entry >= counts.len()
            || visit.parent.is_some_and(|index| index >= visits)
            || visit.children.iter().any(|&index| index >= visits)
            || visit.bindings.iter().any(|binding| {
                binding.owner >= owners
                    || binding.forward_child.is_some_and(|index| index >= visits)
            })
        {
            return fail("visit reference out of bounds");
        }
        counts[visit.entry] += 1;
    }
    for owner in &snapshot.owners {
        if [owner.origin, owner.current, owner.pending_origin_parent]
            .into_iter()
            .flatten()
            .any(|index| index >= visits)
            || owner.creator.is_some_and(|index| index >= owners)
            || owner.owned_visits.iter().any(|&index| index >= visits)
        {
            return fail("owner reference out of bounds");
        }
    }
    let mut linked_children = HashSet::new();
    let mut bound_visits = HashSet::new();
    for (index, visit) in snapshot.visits.iter().enumerate() {
        let mut children = HashSet::new();
        if visit.children.iter().any(|&child| {
            !children.insert(child)
                || !linked_children.insert(child)
                || snapshot.visits[child].parent != Some(index)
        }) {
            return fail("inconsistent parent/child links");
        }
        let mut bindings = HashSet::new();
        if visit.bindings.iter().any(|binding| {
            bound_visits.insert((binding.owner, index));
            !bindings.insert(binding.owner)
                || binding
                    .forward_child
                    .is_some_and(|child| !children.contains(&child))
        }) {
            return fail("duplicate owner binding or non-child forward reference");
        }
    }
    if snapshot
        .visits
        .iter()
        .enumerate()
        .any(|(index, visit)| visit.parent.is_some() && !linked_children.contains(&index))
    {
        return fail("inconsistent parent/child links");
    }
    // Iterative coloring bounds work and stack usage even on hostile deep or
    // cyclic input. Check both visit ancestry and owner-creation ancestry.
    fn acyclic(parents: impl Iterator<Item = Option<usize>>) -> bool {
        let parents: Vec<_> = parents.collect();
        let mut colors = vec![0u8; parents.len()];
        for start in 0..parents.len() {
            let mut path = Vec::new();
            let mut cursor = Some(start);
            while let Some(index) = cursor {
                match colors[index] {
                    1 => return false,
                    2 => break,
                    _ => {},
                }
                colors[index] = 1;
                path.push(index);
                cursor = parents[index];
            }
            for index in path {
                colors[index] = 2;
            }
        }
        true
    }
    if !acyclic(snapshot.visits.iter().map(|visit| visit.parent))
        || !acyclic(snapshot.owners.iter().map(|owner| owner.creator))
    {
        return fail("cyclic ancestry");
    }
    for (index, owner) in snapshot.owners.iter().enumerate() {
        let mut owned = HashSet::new();
        if owner
            .owned_visits
            .iter()
            .any(|&visit| !owned.insert(visit) || !bound_visits.contains(&(index, visit)))
            || [owner.origin, owner.current]
                .into_iter()
                .flatten()
                .any(|visit| !owned.contains(&visit))
        {
            return fail("inconsistent owner origin/current/owned visits");
        }
    }
    if snapshot
        .entries
        .iter()
        .zip(counts)
        .any(|(entry, count)| entry.visit_count != count)
    {
        return fail("entry visit count disagrees with retained visits");
    }
    Ok(())
}

/// The catalog subject a card practises, for practice history and the Related
/// graph. `None` when the card has no catalog identity — a hand-drawn path is
/// its own content, not a catalog entry, so it stays out of both.
pub fn catalog_id_for_card(card: &Card) -> Option<String> {
    Some(match &card.material {
        Material::Scale { name, .. } => woodshed_graph::scale_id(name),
        Material::ScalePattern { name, pattern, .. } => {
            woodshed_graph::scale_pattern_exercise_id(name, *pattern)
        },
        Material::ChordApproach {
            name, direction, ..
        } => woodshed_graph::chord_approach_exercise_id(name, *direction),
        Material::Chord { name, .. } if matches!(card.touch, Touch::Arpeggiate { .. }) => {
            woodshed_graph::arpeggio_id(name)
        },
        Material::Chord { name, .. } => woodshed_graph::chord_id(name),
        Material::Riff { name } => woodshed_graph::exercise_id(name),
        Material::Path { .. } => return None,
    })
}

#[cfg(test)]
mod tests;

mod wire;
