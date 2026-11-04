//! A retained reading belongs to Woodshed; authored grammar and layout are shared.
use super::{UiChild, UiState, WorkspacePanel};
use cambium::{
    GraphCanvasEvent, GraphCanvasInput, GraphCanvasNode, GraphCanvasRelation, GraphCanvasSubgraph,
    GraphCanvasSwatch, LinkMemberAction, OpenedLinkEvent, clickable, el, graph_canvas_with_wheel,
    map_state, text, text_field,
};
use scenograph::link::{
    LinkAssertion, LinkAssertionBasis, LinkAssertionMethod, LinkBucket, LinkCoverage, LinkEndpoint,
    LinkGraphScope, LinkKey, LinkPairReading, LinkReading, LinkRestSummary, LinkSource,
    LinkSourceRevision, LinkStratum, OpenedLinkLayout,
};
use scenograph::relationship::{
    RecipeEdit, RelationshipRecipeDraft, RelationshipSnapshot, relationship_recipe,
};
use scenograph::swatch::{Scope, Swatch, SwatchMode};
use scenograph::{ProjectionInputBinding, PublicSourceRevision, RevisionEvidence};
use scenomise::projection::{
    CompiledRelationshipProjection, ItemSizes, ProjectionCompiler, RelationshipDataset,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use woodshed_core::{comparison_disclosure, working_sets::WorkingSetId};
use woodshedding::rehearsal::{CardId, Set};

mod source_return;
#[cfg(test)]
mod source_return_stats;

pub const RELATIONSHIP_GRAPH_LEAF_KEY: u64 = 0x5753_5243;
pub const MAX_READING_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadingSource {
    pub owner: WorkingSetId,
    pub revision: String,
    pub cards: Vec<CardId>,
    pub pair: (CardId, CardId),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipReading {
    #[serde(default)]
    pub version: u32,
    pub snapshot: RelationshipSnapshot,
    pub dataset: RelationshipDataset,
    #[serde(default)]
    pub source: Option<ReadingSource>,
}
impl RelationshipReading {
    pub fn decode(json: &str) -> Result<Self, String> {
        #[cfg(test)]
        source_return_stats::record(source_return_stats::Event::Decode);
        if json.len() > MAX_READING_BYTES {
            return Err("Retained reading exceeds the 2 MiB limit.".into());
        }
        let reading: Self = serde_json::from_str(json)
            .map_err(|e| format!("Retained reading cannot be decoded: {e}"))?;
        if reading.version > 3 {
            return Err("Retained reading uses an unsupported version.".into());
        }
        if let Some(source) = &reading.source {
            let unique: std::collections::BTreeSet<_> = source.cards.iter().copied().collect();
            if source.owner.0 == 0
                || source.revision.trim().is_empty()
                || !(2..=64).contains(&source.cards.len())
                || unique.len() != source.cards.len()
                || unique.iter().any(|id| id.0 == 0)
                || source.pair.0 == source.pair.1
                || !unique.contains(&source.pair.0)
                || !unique.contains(&source.pair.1)
            {
                return Err("Retained source identities are invalid.".into());
            }
        }
        reading.compile()?;
        Ok(reading)
    }
    pub fn compile(&self) -> Result<CompiledRelationshipProjection, String> {
        #[cfg(test)]
        source_return_stats::record(source_return_stats::Event::Compile);
        // This host retains the established card footprint when reopening readings.
        // Scenomise now requires its consumer to supply representation sizes.
        ProjectionCompiler::new(ItemSizes {
            card: sceno::Size2::new(164.0, 68.0),
        })
        .compile_relationship_snapshot(&self.snapshot, &self.dataset)
        .map_err(|e| format!("Reading refused: {e:?}"))
    }
}
// Cursor navigation is presentation. Every ordered Card instruction is content.
fn revision(owner: WorkingSetId, set: &Set) -> String {
    #[cfg(test)]
    source_return_stats::record(source_return_stats::Event::Revision);
    let bytes = serde_json::to_vec(&(owner, &set.cards)).expect("Set serializes");
    blake3::hash(&bytes).to_hex().to_string()
}
fn finish(
    mut draft: RelationshipRecipeDraft,
) -> Result<scenograph::relationship::RelationshipRecipe, String> {
    let mut provenance = draft.recipe().definition.provenance.clone();
    provenance.source_revision = None;
    draft.apply(RecipeEdit::SetProvenance(provenance.clone()));
    provenance.source_revision = Some(PublicSourceRevision::new(
        blake3::hash(&serde_json::to_vec(draft.recipe()).map_err(|e| e.to_string())?)
            .to_hex()
            .to_string(),
    ));
    draft.apply(RecipeEdit::SetProvenance(provenance));
    draft
        .to_recipe()
        .map_err(|e| format!("Recipe edit refused: {e:?}"))
}
fn owner_set(ui: &UiState, owner: WorkingSetId) -> Result<&Set, String> {
    #[cfg(test)]
    source_return_stats::record(source_return_stats::Event::Owner);
    let count = usize::from(ui.working_sets.active_id == owner)
        + ui.working_sets
            .inactive
            .iter()
            .filter(|entry| entry.id == owner)
            .count();
    if count != 1 {
        return Err("The source Set is unavailable or ambiguous.".into());
    }
    ui.working_sets
        .get(owner, &ui.set)
        .ok_or_else(|| "The source Set is unavailable.".into())
}

fn relationship_link_source(reading: &RelationshipReading) -> LinkSource {
    LinkSource::Available {
        binding: reading.dataset.dataset.source.clone(),
        revision: LinkSourceRevision::Available {
            revision: reading.dataset.dataset.revision.clone(),
        },
    }
}

fn relationship_evidence_token(source: &sceno::SourceRef) -> String {
    format!(
        "urn:woodshed:source-ref:v1:{}:{}{}:{}",
        source.adapter.len(),
        source.adapter,
        source.id.len(),
        source.id
    )
}

fn relationship_pair_reading(
    reading: &RelationshipReading,
    key: &LinkKey,
) -> Option<LinkPairReading> {
    if key.stratum != LinkStratum::Surface {
        return None;
    }
    let mut grouped = BTreeMap::<(String, String), Vec<LinkAssertion>>::new();
    for relation in &reading.dataset.relationships {
        let relation_key = LinkKey::new(
            LinkStratum::Surface,
            relation.from_occurrence.clone(),
            relation.to_occurrence.clone(),
        );
        if relation_key != *key {
            continue;
        }
        grouped
            .entry((
                relation.from_occurrence.clone(),
                relation.to_occurrence.clone(),
            ))
            .or_default()
            .push(LinkAssertion {
                // This remains the source disclosure's identity. It is not a
                // Chartulary EdgeId or a generated pair entity.
                id: relation.id.clone(),
                predicate: format!("urn:woodshed:comparison:{}", relation.kind),
                recognized_kind: Some(relation.kind.clone()),
                label: Some(format!("{} — {}", relation.label, relation.explanation)),
                scope: LinkGraphScope::Source,
                asserter: None,
                first_asserted_ms: None,
                asserted_at_ms: None,
                withdrawn: None,
                basis: LinkAssertionBasis {
                    note: None,
                    method: Some(LinkAssertionMethod {
                        iri: format!("urn:woodshed:method:{}", relation.provenance.method),
                        version: Some(relation.provenance.method_version.to_string()),
                    }),
                    evidence: relation
                        .provenance
                        .evidence
                        .iter()
                        .map(relationship_evidence_token)
                        .collect(),
                },
            });
    }
    if grouped.is_empty() {
        return None;
    }
    let mut buckets = grouped
        .into_iter()
        .map(|((from, to), mut assertions)| {
            assertions.sort_by(|left, right| left.id.cmp(&right.id));
            LinkBucket {
                displayed_from: from.clone(),
                displayed_to: to.clone(),
                source_from: LinkEndpoint {
                    id: from.clone(),
                    address: Some(from),
                },
                source_to: LinkEndpoint {
                    id: to.clone(),
                    address: Some(to),
                },
                assertions,
                legacy_members: Vec::new(),
                legacy_summaries: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        (&left.source_from.id, &left.source_to.id)
            .cmp(&(&right.source_from.id, &right.source_to.id))
    });
    Some(LinkPairReading {
        source: relationship_link_source(reading),
        links: vec![LinkReading {
            key: key.clone(),
            buckets,
        }],
        coverage: LinkCoverage::default(),
    })
}

fn relationship_link_keys(reading: &RelationshipReading) -> BTreeSet<LinkKey> {
    reading
        .dataset
        .relationships
        .iter()
        .map(|relation| {
            LinkKey::new(
                LinkStratum::Surface,
                relation.from_occurrence.clone(),
                relation.to_occurrence.clone(),
            )
        })
        .collect()
}

fn relationship_link_summary(
    reading: &RelationshipReading,
    key: &LinkKey,
) -> Option<LinkRestSummary> {
    let pair = relationship_pair_reading(reading, key)?;
    pair.links
        .iter()
        .find(|link| link.key == *key)
        .map(|link| link.rest_summary(&key.first, &key.second))
}

fn relationship_source_is_current(ui: &UiState, reading: &RelationshipReading) -> bool {
    let Some(source) = &reading.source else {
        return false;
    };
    owner_set(ui, source.owner).is_ok_and(|set| revision(source.owner, set) == source.revision)
}

fn handle_relationship_link_event(ui: &mut UiState, event: OpenedLinkEvent) {
    match event {
        OpenedLinkEvent::Layout(layout) => ui.relationship_opened_link_layout = Some(layout),
        OpenedLinkEvent::Member { target, action } => {
            let result = open_relationship_member(ui, &target, &action);
            ui.relationship_result(result);
        },
    }
}

fn open_relationship_link(ui: &mut UiState, id: &str) -> Result<(), String> {
    let reading = ui.relationship_reading()?.ok_or("Choose material first.")?;
    let key = relationship_link_keys(&reading)
        .into_iter()
        .find(|key| key.stable_id() == id)
        .ok_or("The selected relationship link is unavailable.")?;
    ui.relationship_opened_link = Some(key);
    ui.relationship_opened_link_layout = None;
    ui.relationship_notice = None;
    Ok(())
}

fn open_relationship_member(
    ui: &mut UiState,
    target: &cambium::LinkMemberTarget,
    action: &str,
) -> Result<(), String> {
    if action != "open-source-card" {
        return Err("This relationship member action is not available.".into());
    }
    if LinkKey::new(LinkStratum::Surface, target.from.clone(), target.to.clone()) != target.link {
        return Err("The selected member direction does not belong to this link.".into());
    }
    let mut reading = ui.relationship_reading()?.ok_or("Choose material first.")?;
    let pair = relationship_pair_reading(&reading, &target.link)
        .ok_or("The selected link is no longer in the retained reading.")?;
    if target.source != pair.source {
        return Err("The selected member belongs to a different source reading.".into());
    }
    let member = pair
        .links
        .iter()
        .flat_map(|link| &link.buckets)
        .find(|bucket| bucket.source_from.id == target.from && bucket.source_to.id == target.to)
        .and_then(|bucket| {
            bucket
                .assertions
                .iter()
                .find(|assertion| assertion.id == target.assertion_id)
        })
        .ok_or("The exact relationship assertion is no longer available.")?;
    let Some(source) = reading.source.as_ref() else {
        return Err("This retained relationship has no current owner action.".into());
    };
    let set = source_return::current_set(
        ui,
        source,
        "Source instructions changed. Explicitly rebind before opening a source Card.",
    )?;
    reading.snapshot.selected_occurrence = Some(target.from.clone());
    reading.snapshot.selected_relationship = Some(member.id.clone());
    // Prepare in the old installation order. Keep a late source refusal until
    // after installation, preserving the selected reading even on that refusal.
    let installation = source_return::PreparedReading::prepare(&reading)?;
    let destination = source_return::destination(&reading, source, set);
    installation.install(ui);
    source_return::navigate(ui, destination?)
}

pub fn relationship_swatch(ui: &UiState) -> Option<GraphCanvasSwatch<String, &'static str>> {
    let reading = ui.relationship_reading().ok()??;
    let compiled = reading.compile().ok()?;
    let mut swatch = swatch_from(
        &reading,
        &compiled,
        ui.viewport_width,
        ui.relationship_opened_link.as_ref(),
    );
    swatch.viewport = ui.relationship_viewport;
    Some(swatch)
}
fn swatch_from(
    reading: &RelationshipReading,
    compiled: &CompiledRelationshipProjection,
    viewport_width: f32,
    selected_link: Option<&LinkKey>,
) -> GraphCanvasSwatch<String, &'static str> {
    let scene = &compiled.projection.scene;
    let bounds = scene.bounds;
    let width = (bounds.size.w + 64.0).max(280.0) as u32;
    // The scene owns arrangement coordinates; the overview fits that scene
    // into its pane. Use the same fitted box for paint and native targets.
    let canvas_width = width.min((viewport_width.min(1100.0) - 64.0).max(280.0) as u32);
    let height = 220;
    let normalize = |x: f32, y: f32| {
        (
            (x - bounds.origin.x + 32.0) / width as f32,
            (y - bounds.origin.y + 96.0) / height as f32,
        )
    };
    let nodes = compiled
        .projection
        .instance_by_occurrence
        .iter()
        .map(|(id, instance)| {
            let item = &scene.items[instance.0 as usize];
            GraphCanvasNode {
                id: id.clone(),
                kind: "occurrence",
                position: normalize(item.transform.translate.x, item.transform.translate.y),
                label: compiled
                    .projection
                    .labels
                    .get(instance)
                    .cloned()
                    .unwrap_or_else(|| id.clone()),
                key: Some(id.clone()),
            }
        })
        .collect();
    let relations = relationship_link_keys(reading)
        .into_iter()
        .filter_map(|key| {
            let summary = relationship_link_summary(reading, &key)?;
            let from_instance = compiled.projection.instance_by_occurrence.get(&key.first)?;
            let to_instance = compiled
                .projection
                .instance_by_occurrence
                .get(&key.second)?;
            let from = &scene.items[from_instance.0 as usize];
            let to = &scene.items[to_instance.0 as usize];
            Some(GraphCanvasRelation {
                id: key.stable_id(),
                link_key: Some(key.clone()),
                marks: summary.marks,
                strength: summary.strength,
                from: key.first.clone(),
                to: key.second.clone(),
                kind: "surface-link".into(),
                label: format!(
                    "{} held assertions · {} legacy members",
                    summary.held_assertions, summary.legacy_members
                ),
                route: vec![
                    normalize(from.transform.translate.x, from.transform.translate.y),
                    normalize(to.transform.translate.x, to.transform.translate.y),
                ],
                visible: true,
                emphasized: selected_link == Some(&key),
            })
        })
        .collect();
    let mut swatch = GraphCanvasSwatch::new(
        RELATIONSHIP_GRAPH_LEAF_KEY,
        GraphCanvasSubgraph {
            nodes,
            edges: vec![],
        },
    )
    .with_relations(relations)
    .with_size(canvas_width, height)
    .with_node_labels(true)
    .with_expand(false)
    .with_label("Selected material relationship graph");
    swatch.selected = reading.snapshot.selected_occurrence.clone();
    swatch
}
fn action(
    label: impl Into<String>,
    class: &'static str,
    f: impl Fn(&mut UiState) + 'static,
) -> UiChild {
    Box::new(clickable(
        el("div", text(label.into())).attr("class", class),
        move |ui: &mut UiState, _| f(ui),
    ))
}

#[cfg(test)]
mod source_return_tests;

#[cfg(test)]
#[path = "relationship/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "relationship/authority_tests.rs"]
mod authority_tests;

#[path = "relationship/operations.rs"]
mod operations;

#[path = "relationship/render.rs"]
mod render;

pub(super) use render::screen;
