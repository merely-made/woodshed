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
    let owner = source.owner;
    let source_revision = source.revision.clone();
    let set = owner_set(ui, owner)?;
    if revision(owner, set) != source_revision {
        return Err(
            "Source instructions changed. Explicitly rebind before opening a source Card.".into(),
        );
    }
    reading.snapshot.selected_occurrence = Some(target.from.clone());
    reading.snapshot.selected_relationship = Some(member.id.clone());
    ui.install_relationship(reading)?;
    ui.open_relationship_source()
}

impl UiState {
    pub fn relationship_reading(&self) -> Result<Option<RelationshipReading>, String> {
        self.relationship_reading_json
            .as_deref()
            .map(RelationshipReading::decode)
            .transpose()
    }
    fn install_relationship(&mut self, reading: RelationshipReading) -> Result<(), String> {
        reading.compile()?;
        let json = serde_json::to_string(&reading).map_err(|e| e.to_string())?;
        if json.len() > MAX_READING_BYTES {
            return Err("Reading exceeds the 2 MiB limit.".into());
        }
        self.relationship_label =
            cambium::TextInput::new(reading.snapshot.recipe.definition.label.clone());
        self.relationship_reading_json = Some(json);
        Ok(())
    }
    pub fn open_relationship_recipe(&mut self) {
        self.activate_workspace_panel(WorkspacePanel::Overview);
        self.relationship_open = true;
        self.relationship_choice_owner = Some(self.working_sets.active_id);
        self.relationship_cards = self.set.cards.iter().take(64).map(|card| card.id).collect();
        if let Ok(Some(reading)) = self.relationship_reading() {
            self.relationship_label =
                cambium::TextInput::new(reading.snapshot.recipe.definition.label);
        }
    }
    pub fn bind_relationship_recipe(&mut self, rebind: bool) -> Result<(), String> {
        let owner = self.working_sets.active_id;
        if self.relationship_choice_owner != Some(owner) {
            return Err("The working Set changed. Reopen the recipe to choose material from its current owner.".into());
        }
        let set = owner_set(self, owner)?;
        if self.relationship_cards.len() < 2 || self.relationship_cards.len() > 64 {
            return Err("Choose between two and 64 Cards; consecutive selected occurrences supply the comparisons.".into());
        }
        let cards: Vec<_> = set
            .cards
            .iter()
            .filter(|card| self.relationship_cards.contains(&card.id))
            .map(|card| card.id)
            .collect();
        if cards.len() != self.relationship_cards.len() {
            return Err("Selected Card identities changed. Choose the material again.".into());
        }
        let pair = (cards[0], cards[1]);
        let revision = revision(owner, set);
        let value = comparison_disclosure::passage_projection_json(owner, &revision, set, &cards)
            .map_err(|e| format!("Comparison unavailable: {e:?}"))?;
        let dataset: RelationshipDataset =
            serde_json::from_value(value).map_err(|e| e.to_string())?;
        let binding = ProjectionInputBinding {
            source: dataset.dataset.source.clone(),
            expects_generation: Some(dataset.dataset.revision.clone()),
            revision_evidence: RevisionEvidence::PublicGeneration,
        };
        let mut snapshot = if rebind {
            self.relationship_reading()?
                .ok_or("There is no retained recipe to rebind.")?
                .snapshot
        } else {
            if self.relationship_reading_json.is_some() {
                return Err("A retained reading already exists. Use explicit rebind to preserve its authored recipe.".into());
            }
            RelationshipSnapshot {
                recipe: relationship_recipe(
                    "woodshed-relationships",
                    "Selected material relationships",
                    "Woodshed",
                    "1",
                    BTreeMap::from([("woodshed".into(), binding.clone())]),
                ),
                source_name: "woodshed".into(),
                selected_occurrence: None,
                selected_relationship: None,
            }
        };
        let mut draft = RelationshipRecipeDraft::new(snapshot.recipe);
        draft.apply(RecipeEdit::BindInput {
            name: snapshot.source_name.clone(),
            binding,
        });
        snapshot.recipe = finish(draft)?;
        // Preserve selections only when the exact disclosed identities survive.
        snapshot.selected_occurrence = snapshot.selected_occurrence.filter(|id| {
            dataset
                .dataset
                .occurrences
                .iter()
                .any(|o| &o.occurrence_id == id)
        });
        snapshot.selected_relationship = snapshot
            .selected_relationship
            .filter(|id| dataset.relationships.iter().any(|r| &r.id == id));
        self.install_relationship(RelationshipReading {
            version: 3,
            snapshot,
            dataset,
            source: Some(ReadingSource {
                owner,
                revision,
                cards,
                pair,
            }),
        })
    }
    pub fn edit_relationship_recipe(
        &mut self,
        label: Option<String>,
        spacing: Option<u32>,
    ) -> Result<(), String> {
        let mut reading = self
            .relationship_reading()?
            .ok_or("Choose material first.")?;
        let mut draft = RelationshipRecipeDraft::new(reading.snapshot.recipe);
        if let Some(label) = label {
            draft.apply(RecipeEdit::SetLabel(label));
        }
        if let Some(spacing) = spacing {
            if !(8..=256).contains(&spacing) {
                return Err("Spacing must be between 8 and 256.".into());
            }
            let mut arrangement = draft.recipe().definition.arrangement.clone();
            arrangement.spacing = spacing;
            draft.apply(RecipeEdit::SetArrangement(arrangement));
        }
        reading.snapshot.recipe = finish(draft)?;
        self.install_relationship(reading)
    }
    pub fn select_relationship(
        &mut self,
        occurrence: Option<String>,
        relation: Option<String>,
    ) -> Result<(), String> {
        let mut reading = self
            .relationship_reading()?
            .ok_or("Choose material first.")?;
        reading.snapshot.selected_occurrence = occurrence;
        reading.snapshot.selected_relationship = relation;
        self.install_relationship(reading)
    }
    pub fn open_relationship_source(&mut self) -> Result<(), String> {
        let reading = self
            .relationship_reading()?
            .ok_or("Choose material first.")?;
        let source = reading.source.as_ref().ok_or(
            "This retained disclosure has no owner action. Explicitly rebind to a Woodshed Set.",
        )?;
        let set = owner_set(self, source.owner)?;
        if revision(source.owner, set) != source.revision {
            return Err(
                "Source instructions changed. Explicitly rebind before returning to a Card.".into(),
            );
        }
        let id = reading
            .snapshot
            .selected_occurrence
            .as_ref()
            .ok_or("Select a recipe occurrence first.")?;
        let matches: Vec<_> = set
            .cards
            .iter()
            .filter(|card| {
                comparison_disclosure::occurrence_id(source.owner, card.id) == *id
                    && source.cards.contains(&card.id)
            })
            .collect();
        if matches.len() != 1 {
            return Err("The selected source occurrence is unavailable or ambiguous.".into());
        }
        let card = matches[0].id;
        // Validate the retained disclosure itself against current owner-owned facts.
        let disclose = if reading.version >= 2 {
            comparison_disclosure::disclose_comparison
        } else {
            comparison_disclosure::disclose
        };
        let facts = disclose(
            source.owner,
            &source.revision,
            set,
            &source.cards,
            source.pair,
        )
        .map_err(|e| format!("Source refused: {e:?}"))?;
        let current: RelationshipDataset = serde_json::from_value(if reading.version >= 3 {
            comparison_disclosure::passage_projection_json(
                source.owner,
                &source.revision,
                set,
                &source.cards,
            )
            .map_err(|e| format!("Source refused: {e:?}"))?
        } else if reading.version >= 2 {
            comparison_disclosure::comparison_projection_json(&facts)
        } else {
            comparison_disclosure::projection_json(&facts)
        })
        .map_err(|e| e.to_string())?;
        if serde_json::to_value(current).map_err(|e| e.to_string())?
            != serde_json::to_value(&reading.dataset).map_err(|e| e.to_string())?
        {
            return Err("Retained disclosure does not match the source Set.".into());
        }
        self.activate_working_set(source.owner);
        self.set.select_id(card);
        self.set_graph_card_expanded = true;
        self.relationship_open = false;
        Ok(())
    }
    pub fn relationship_result(&mut self, result: Result<(), String>) {
        self.relationship_notice = Some(match result {
            Ok(()) => "Reading updated in this session.".into(),
            Err(e) => e,
        });
    }
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
pub(super) fn screen(ui: &UiState) -> UiChild {
    let mut children: Vec<UiChild> = vec![
        Box::new(el("h3", text("Relationship recipe"))),
        action("Back to Mere", "t-btn relationship-back", |ui| {
            ui.relationship_open = false
        }),
        Box::new(el(
            "p",
            text(
                "Choose occurrences from the current Set. Consecutive selected Cards in authored order supply exact shared tones, differences, equality or directed containment. Sparse choices compare the selected sequence, not every intervening Card. A retained reading keeps its captured evidence until you explicitly rebind.",
            ),
        )),
    ];
    let choices: Vec<UiChild> = ui
        .set
        .cards
        .iter()
        .take(64)
        .enumerate()
        .map(|(index, card)| {
            let id = card.id;
            let selected = ui.relationship_cards.contains(&id);
            action(
                format!(
                    "{} {} · {}",
                    if selected { "Selected" } else { "Choose" },
                    index + 1,
                    card.label
                ),
                "t-btn relationship-material",
                move |ui| {
                    if ui.relationship_cards.contains(&id) {
                        ui.relationship_cards.retain(|c| *c != id)
                    } else {
                        ui.relationship_cards.push(id)
                    }
                },
            )
        })
        .collect();
    children.push(Box::new(
        el("div", choices).attr("class", "relationship-choices"),
    ));
    children.push(action(
        if ui.relationship_reading_json.is_some() {
            "Rebind recipe to selected Cards"
        } else {
            "Use selected Card relationships"
        },
        "t-btn relationship-bind",
        |ui| {
            let result = ui.bind_relationship_recipe(ui.relationship_reading_json.is_some());
            ui.relationship_result(result)
        },
    ));
    if let Some(notice) = &ui.relationship_notice {
        children.push(Box::new(
            el("p", text(notice.clone())).attr("class", "relationship-notice"),
        ));
    }
    match ui.relationship_reading() {
        Ok(Some(reading)) => {
            let compiled = reading.compile().expect("decoded reading validated");
            if reading.version >= 3 {
                let count = reading.dataset.dataset.occurrences.len();
                children.push(Box::new(el(
                    "p",
                    text(format!(
                        "{count} selected occurrences · {} consecutive comparisons",
                        count.saturating_sub(1)
                    )),
                )));
            }
            children.push(Box::new(el(
                "h4",
                text(reading.snapshot.recipe.definition.label.clone()),
            )));
            children.push(Box::new(
                el(
                    "div",
                    map_state(text_field(&ui.relationship_label), |ui: &mut UiState| {
                        &mut ui.relationship_label
                    }),
                )
                .attr("class", "relationship-label"),
            ));
            children.push(action(
                "Apply recipe label",
                "t-btn relationship-apply-label",
                |ui| {
                    let r = ui.edit_relationship_recipe(
                        Some(ui.relationship_label.text().to_string()),
                        None,
                    );
                    ui.relationship_result(r)
                },
            ));
            let spacing = reading.snapshot.recipe.definition.arrangement.spacing;
            children.push(Box::new(el("p", text(format!("Grid spacing: {spacing}")))));
            children.push(action(
                "Increase recipe spacing",
                "t-btn relationship-spacing",
                move |ui| {
                    let r = ui.edit_relationship_recipe(None, Some(spacing.saturating_add(8)));
                    ui.relationship_result(r)
                },
            ));
            for compiled_relation in &compiled.relationships {
                let relation = &compiled_relation.disclosure;
                let id = relation.id.clone();
                let left = relation.from_occurrence.clone();
                children.push(action(
                    format!("Explain {}", relation.label),
                    "t-btn relationship-explain",
                    move |ui| {
                        let r = ui.select_relationship(Some(left.clone()), Some(id.clone()));
                        ui.relationship_result(r)
                    },
                ));
                if reading.snapshot.selected_relationship.as_ref() == Some(&relation.id) {
                    children.push(Box::new(
                        el(
                            "p",
                            text(format!(
                                "{} Method: {} v{} · {}.",
                                relation.explanation,
                                relation.provenance.method,
                                relation.provenance.method_version,
                                relation.provenance.provider
                            )),
                        )
                        .attr("class", "relationship-explanation"),
                    ));
                }
            }
            {
                let mut swatch = swatch_from(
                    &reading,
                    &compiled,
                    ui.viewport_width,
                    ui.relationship_opened_link.as_ref(),
                );
                swatch.viewport = ui.relationship_viewport;
                children.push(Box::new(
                    el(
                        "div",
                        el(
                            "div",
                            graph_canvas_with_wheel(&swatch, |ui: &mut UiState, input| {
                                let event = match input {
                                    GraphCanvasInput::Wheel(event) => {
                                        super::canvas_camera::apply_wheel(
                                            ui,
                                            super::canvas_camera::CanvasSurface::Relationship,
                                            &event,
                                        );
                                        return;
                                    },
                                    GraphCanvasInput::Event(event) => event,
                                };
                                let r = match event {
                                    GraphCanvasEvent::Activate(id) => {
                                        ui.relationship_opened_link = None;
                                        ui.relationship_opened_link_layout = None;
                                        ui.select_relationship(Some(id), None)
                                    },
                                    GraphCanvasEvent::RelationActivate(id) => {
                                        open_relationship_link(ui, &id)
                                    },
                                    GraphCanvasEvent::Pan { delta } => {
                                        ui.relationship_viewport.pan.0 += delta.0;
                                        ui.relationship_viewport.pan.1 += delta.1;
                                        return;
                                    },
                                    GraphCanvasEvent::Zoom { factor } => {
                                        let command = super::canvas_camera::zoom_by(
                                            ui,
                                            super::canvas_camera::CanvasSurface::Relationship,
                                            factor,
                                        );
                                        super::canvas_camera::apply_canvas_command(
                                            ui,
                                            super::canvas_camera::CanvasSurface::Relationship,
                                            command,
                                        );
                                        return;
                                    },
                                    _ => return,
                                };
                                ui.relationship_result(r)
                            }),
                        )
                        .attr(
                            "style",
                            format!("width:{}px;height:{}px;", swatch.width, swatch.height),
                        ),
                    )
                    .attr("class", "relationship-graph"),
                ));
                children.push(Box::new(
                    el(
                        "div",
                        super::canvas_camera::camera_controls(
                            super::canvas_camera::CanvasSurface::Relationship,
                            "relationship-camera",
                        ),
                    )
                    .attr("class", "relationship-camera-controls"),
                ));
            }
            if let Some(key) = ui.relationship_opened_link.as_ref() {
                if let Some(pair) = relationship_pair_reading(&reading, key) {
                    let swatch = Swatch {
                        id: format!("woodshed-relationship-link-{}", key.stable_id()),
                        scope: Scope::Link(key.clone()),
                        mode: SwatchMode::Projection {
                            definition_id: "opened-link".into(),
                            variant_id: None,
                        },
                    };
                    let layout = ui
                        .relationship_opened_link_layout
                        .clone()
                        .unwrap_or_else(|| OpenedLinkLayout::default_for(&pair));
                    let can_open_source = relationship_source_is_current(ui, &reading);
                    children.push(Box::new(cambium::opened_link_swatch(
                        &swatch,
                        &pair,
                        &layout,
                        scenograph::link::OpenedLinkOptions { now_ms: ui.now_ms },
                        false,
                        move |_source, _key, bucket, _assertion| {
                            if can_open_source {
                                vec![LinkMemberAction {
                                    id: "open-source-card".into(),
                                    label: format!(
                                        "Open source Card for {} → {}",
                                        bucket.displayed_from, bucket.displayed_to
                                    ),
                                }]
                            } else {
                                Vec::new()
                            }
                        },
                        handle_relationship_link_event,
                    )));
                }
            }
            for occurrence in &reading.dataset.dataset.occurrences {
                let id = occurrence.occurrence_id.clone();
                let label = compiled
                    .projection
                    .instance_by_occurrence
                    .get(&id)
                    .and_then(|i| compiled.projection.labels.get(i))
                    .cloned()
                    .unwrap_or_else(|| id.clone());
                children.push(action(
                    format!("Inspect {label} · {id}"),
                    "t-btn relationship-occurrence",
                    move |ui| {
                        let r = ui.select_relationship(Some(id.clone()), None);
                        ui.relationship_result(r)
                    },
                ));
            }

            children.push(action(
                "Open selected source Card",
                "t-btn relationship-source",
                |ui| {
                    let r = ui.open_relationship_source();
                    ui.relationship_result(r)
                },
            ));
            children.push(Box::new(el("p",text("This captured reading is included in session persistence. Reopening preserves its evidence and selections; opening a source Card validates the current owner and instructions."))));
        },
        Err(error) => children.push(Box::new(
            el(
                "p",
                text(format!(
                    "{error} The retained payload is preserved; your Sets remain available."
                )),
            )
            .attr("class", "relationship-refusal"),
        )),
        Ok(None) => {},
    }
    Box::new(el("div", children).attr("class", "relationship-screen"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    fn fixture() -> UiState {
        let mut ui = UiState::new();
        for (formula, tonic) in [("chord:Major", 0), ("chord:Major", 0), ("chord:Minor", 9)] {
            ui.set.push(
                KeyedCatalogRef {
                    formula_id: formula.into(),
                    root: PitchClass::new(tonic),
                }
                .to_card()
                .unwrap(),
            );
        }
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(false).unwrap();
        ui
    }

    #[test]
    fn relationship_canvas_has_one_rest_stroke_and_opens_all_directed_members() {
        let mut ui = fixture();
        ui.relationship_cards = vec![ui.set.cards[0].id, ui.set.cards[1].id];
        ui.bind_relationship_recipe(true).unwrap();
        let reading = ui.relationship_reading().unwrap().unwrap();
        let compiled = reading.compile().unwrap();
        let keys = relationship_link_keys(&reading);
        assert!(!keys.is_empty());

        let swatch = swatch_from(&reading, &compiled, ui.viewport_width, None);
        assert_eq!(swatch.relations.len(), keys.len());
        for key in &keys {
            let visual = swatch
                .relations
                .iter()
                .find(|relation| relation.link_key.as_ref() == Some(key))
                .expect("each canonical pair has one resting stroke");
            let pair = relationship_pair_reading(&reading, key).unwrap();
            let link = pair.links.iter().find(|link| link.key == *key).unwrap();
            let summary = link.rest_summary(&key.first, &key.second);
            assert_eq!(visual.id, key.stable_id());
            assert_eq!(visual.marks, summary.marks);
            assert_eq!(visual.strength, summary.strength);

            let projected = reading
                .dataset
                .relationships
                .iter()
                .filter(|relation| {
                    LinkKey::new(
                        LinkStratum::Surface,
                        relation.from_occurrence.clone(),
                        relation.to_occurrence.clone(),
                    ) == *key
                })
                .map(|relation| {
                    (
                        relation.from_occurrence.clone(),
                        relation.to_occurrence.clone(),
                        relation.id.clone(),
                    )
                })
                .collect::<BTreeSet<_>>();
            let disclosed = link
                .buckets
                .iter()
                .flat_map(|bucket| {
                    bucket.assertions.iter().map(|assertion| {
                        (
                            bucket.source_from.id.clone(),
                            bucket.source_to.id.clone(),
                            assertion.id.clone(),
                        )
                    })
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(disclosed, projected);
            assert!(link.buckets.iter().all(|bucket| {
                bucket.assertions.iter().all(|assertion| {
                    assertion.scope == LinkGraphScope::Source
                        && assertion.asserter.is_none()
                        && assertion.first_asserted_ms.is_none()
                        && assertion.asserted_at_ms.is_none()
                })
            }));
        }

        let key = keys.iter().next().unwrap().clone();
        open_relationship_link(&mut ui, &key.stable_id()).unwrap();
        assert_eq!(ui.relationship_opened_link.as_ref(), Some(&key));
        let pair = relationship_pair_reading(&reading, &key).unwrap();
        let bucket = pair.links[0].buckets.first().unwrap();
        let assertion = bucket.assertions.first().unwrap();
        let target = cambium::LinkMemberTarget {
            source: pair.source.clone(),
            link: key,
            from: bucket.source_from.id.clone(),
            to: bucket.source_to.id.clone(),
            assertion_id: assertion.id.clone(),
        };
        let source = reading.source.as_ref().unwrap();
        let expected_card = source
            .cards
            .iter()
            .copied()
            .find(|card| comparison_disclosure::occurrence_id(source.owner, *card) == target.from)
            .unwrap();
        let cards_before = serde_json::to_value(&ui.set.cards).unwrap();
        open_relationship_member(&mut ui, &target, "open-source-card").unwrap();
        assert_eq!(ui.set.cursor_id(), Some(expected_card));
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), cards_before);
    }

    #[test]
    fn legacy_reading_preserves_evidence_until_explicit_richer_rebind() {
        let mut ui = fixture();
        let mut reading = ui.relationship_reading().unwrap().unwrap();
        let source = reading.source.as_ref().unwrap();
        let facts = comparison_disclosure::disclose(
            source.owner,
            &source.revision,
            &ui.set,
            &source.cards,
            source.pair,
        )
        .unwrap();
        reading.version = 1;
        reading.dataset =
            serde_json::from_value(comparison_disclosure::projection_json(&facts)).unwrap();
        reading.snapshot.selected_occurrence = Some(comparison_disclosure::occurrence_id(
            source.owner,
            source.cards[0],
        ));
        ui.install_relationship(reading).unwrap();
        let retained = ui.relationship_reading_json.clone();
        let mut restored = UiState::new();
        restored.apply_persisted(&ui.to_persisted(), Default::default());
        restored.open_relationship_recipe();
        assert_eq!(restored.relationship_reading_json, retained);
        assert_eq!(
            restored
                .relationship_reading()
                .unwrap()
                .unwrap()
                .dataset
                .relationships
                .len(),
            1
        );
        restored.open_relationship_source().unwrap();
        restored.open_relationship_recipe();
        restored.bind_relationship_recipe(true).unwrap();
        let rebound = restored.relationship_reading().unwrap().unwrap();
        assert_eq!(rebound.version, 3);
        assert_eq!(rebound.dataset.relationships.len(), 4);
    }

    #[test]
    fn version_two_pair_evidence_stays_unchanged_until_passage_rebind() {
        let mut ui = fixture();
        let mut reading = ui.relationship_reading().unwrap().unwrap();
        let source = reading.source.as_ref().unwrap();
        let facts = comparison_disclosure::disclose_comparison(
            source.owner,
            &source.revision,
            &ui.set,
            &source.cards,
            source.pair,
        )
        .unwrap();
        reading.version = 2;
        reading.dataset =
            serde_json::from_value(comparison_disclosure::comparison_projection_json(&facts))
                .unwrap();
        reading.snapshot.selected_occurrence = Some("set:1:card:2".into());
        ui.install_relationship(reading).unwrap();
        let captured = ui.relationship_reading_json.clone();
        ui.open_relationship_recipe();
        assert_eq!(ui.relationship_reading_json, captured);
        ui.open_relationship_source().unwrap();
        assert_eq!(ui.set.cursor_id(), Some(CardId(2)));
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(true).unwrap();
        let reading = ui.relationship_reading().unwrap().unwrap();
        assert_eq!(reading.version, 3);
        assert_eq!(reading.compile().unwrap().relationships.len(), 4);
        assert_eq!(
            reading.snapshot.selected_occurrence.as_deref(),
            Some("set:1:card:2")
        );
    }

    #[test]
    fn disjoint_reading_compiles_and_returns_to_exact_source() {
        let mut ui = fixture();
        ui.set.cards[1].material = woodshedding::rehearsal::Material::Chord {
            name: "Major".into(),
            root: woodshedding::pitch::PitchClass::new(1),
        };
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(true).unwrap();
        let reading = ui.relationship_reading().unwrap().unwrap();
        assert_eq!(reading.compile().unwrap().relationships.len(), 2);
        assert_eq!(
            reading.dataset.relationships[0].kind,
            comparison_disclosure::TONE_DIFFERENCES
        );
        let id = reading.dataset.dataset.occurrences[1].occurrence_id.clone();
        ui.select_relationship(Some(id), None).unwrap();
        ui.open_relationship_source().unwrap();
        assert_eq!(ui.set.cursor_id(), Some(ui.set.cards[1].id));
    }

    #[test]
    fn shared_edits_selection_and_explanation_survive_session_restore() {
        let mut ui = fixture();
        let before = serde_json::to_value(&ui.set).unwrap();
        ui.edit_relationship_recipe(Some("My comparison".into()), Some(48))
            .unwrap();
        let reading = ui.relationship_reading().unwrap().unwrap();
        let occurrence = reading.dataset.dataset.occurrences[1].occurrence_id.clone();
        let relation = reading.dataset.relationships[0].id.clone();
        ui.select_relationship(Some(occurrence.clone()), Some(relation.clone()))
            .unwrap();
        assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
        let mut restored = UiState::new();
        restored.apply_persisted(&ui.to_persisted(), Default::default());
        let reading = restored.relationship_reading().unwrap().unwrap();
        assert_eq!(reading.snapshot.recipe.definition.label, "My comparison");
        assert_eq!(reading.snapshot.recipe.definition.arrangement.spacing, 48);
        assert_eq!(reading.snapshot.selected_occurrence, Some(occurrence));
        assert_eq!(reading.snapshot.selected_relationship, Some(relation));
        let compiled = reading.compile().unwrap();
        assert_eq!(compiled.relationships.len(), 4);
        assert_eq!(compiled.projection.scene.relations.len(), 4);
        assert_eq!(
            compiled
                .relationships
                .iter()
                .find(|r| r.disclosure.id == reading.dataset.relationships[0].id)
                .unwrap()
                .disclosure
                .explanation,
            reading.dataset.relationships[0].explanation
        );
        assert!(!restored.rehearsal_running);
    }
    #[test]
    fn exact_owner_action_preserves_background_runner_and_cursor_navigation_is_current() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let id = ui.set.cards[1].id;
        ui.select_relationship(Some(comparison_disclosure::occurrence_id(owner, id)), None)
            .unwrap();
        ui.set.cursor = 2;
        ui.duplicate_working_set();
        let other = ui.working_sets.active_id;
        ui.rehearsal_owner = Some(other);
        ui.rehearsal_running = true;
        let other_set = serde_json::to_value(&ui.set).unwrap();
        ui.open_relationship_source().unwrap();
        assert_eq!(ui.working_sets.active_id, owner);
        assert_eq!(ui.set.cursor_id(), Some(id));
        assert_eq!(ui.rehearsal_owner, Some(other));
        assert!(ui.rehearsal_running);
        assert_eq!(
            serde_json::to_value(ui.working_sets.get(other, &ui.set).unwrap()).unwrap(),
            other_set
        );
    }
    #[test]
    fn full_instruction_changes_and_missing_or_ambiguous_owners_refuse_without_navigation() {
        for change in 0..5 {
            let mut ui = fixture();
            let owner = ui.working_sets.active_id;
            let id = ui.set.cards[0].id;
            ui.select_relationship(Some(comparison_disclosure::occurrence_id(owner, id)), None)
                .unwrap();
            match change {
                0 => ui.set.cards[0].setting.capo = Some(2),
                1 => ui.set.cards[0].timing.bpm = Some(101.0),
                2 => ui.set.cards.swap(0, 1),
                3 => {
                    ui.set.cards.remove(0);
                },
                _ => ui.set.cards[0].label.push_str(" changed"),
            };
            let before = serde_json::to_value(&ui.set).unwrap();
            assert!(ui.open_relationship_source().is_err());
            assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
        }
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let id = ui.set.cards[0].id;
        ui.select_relationship(Some(comparison_disclosure::occurrence_id(owner, id)), None)
            .unwrap();
        ui.working_sets
            .inactive
            .push(woodshed_core::working_sets::WorkingSet {
                id: owner,
                name: "Ambiguous".into(),
                set: ui.set.clone(),
            });
        assert!(ui.open_relationship_source().is_err());
    }
    #[test]
    fn explicit_rebind_preserves_recipe_and_occurrences_do_not_change_cards() {
        let mut ui = fixture();
        ui.edit_relationship_recipe(Some("Reusable".into()), Some(48))
            .unwrap();
        let first = ui.relationship_reading().unwrap().unwrap();
        ui.duplicate_working_set();
        ui.open_relationship_recipe();
        let before = serde_json::to_value(&ui.set).unwrap();
        ui.bind_relationship_recipe(true).unwrap();
        let reading = ui.relationship_reading().unwrap().unwrap();
        assert_eq!(reading.snapshot.recipe.definition.label, "Reusable");
        assert_eq!(reading.snapshot.recipe.definition.arrangement.spacing, 48);
        assert_ne!(first.dataset.dataset.source, reading.dataset.dataset.source);
        assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
        assert_ne!(
            reading.dataset.dataset.occurrences[0].occurrence_id,
            reading.dataset.dataset.occurrences[1].occurrence_id
        );
        assert_eq!(
            reading.dataset.dataset.occurrences[0].source,
            reading.dataset.dataset.occurrences[1].source
        );
    }
    #[test]
    fn invalid_optional_payload_is_preserved_and_never_discards_sets() {
        for invalid in [
            "{".to_string(),
            "x".repeat(MAX_READING_BYTES + 1),
            "{\"version\":99}".to_string(),
        ] {
            let mut ui = fixture();
            ui.relationship_reading_json = Some(invalid.clone());
            let persisted = ui.to_persisted();
            let mut restored = UiState::new();
            restored.apply_persisted(&persisted, Default::default());
            assert_eq!(restored.set.cards.len(), 3);
            assert!(restored.relationship_reading().is_err());
            assert_eq!(
                restored.to_persisted().relationship_reading_json,
                Some(invalid)
            );
        }
    }
    #[test]
    fn incompatible_facet_and_stale_provenance_refuse_without_replacing_payload() {
        let mut ui = fixture();
        let valid = ui.relationship_reading_json.clone();
        let mut reading = ui.relationship_reading().unwrap().unwrap();
        reading.dataset.facets.remove("authored_order");
        assert!(ui.install_relationship(reading).is_err());
        assert_eq!(ui.relationship_reading_json, valid);
        let mut reading = ui.relationship_reading().unwrap().unwrap();
        reading.dataset.relationships[0].provenance.source_revision =
            PublicSourceRevision::new("stale");
        assert!(ui.install_relationship(reading).is_err());
        assert_eq!(ui.relationship_reading_json, valid);
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    #[test]
    fn pending_choices_cannot_bind_another_owner_with_the_same_card_ids() {
        let mut ui = UiState::new();
        for _ in 0..2 {
            ui.set.push(
                KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(0),
                }
                .to_card()
                .unwrap(),
            );
        }
        ui.open_relationship_recipe();
        let copy = ui.set.clone();
        ui.working_sets.create(&mut ui.set, "Other", copy);
        let before = serde_json::to_value(&ui.set).unwrap();
        assert!(ui.bind_relationship_recipe(false).is_err());
        assert!(ui.relationship_reading_json.is_none());
        assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(false).unwrap();
        assert_eq!(
            ui.relationship_reading()
                .unwrap()
                .unwrap()
                .source
                .unwrap()
                .owner,
            ui.working_sets.active_id
        );
    }
    #[test]
    fn source_action_disambiguates_identical_card_numbers_in_two_owners() {
        let mut ui = UiState::new();
        for _ in 0..2 {
            ui.set.push(
                KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(0),
                }
                .to_card()
                .unwrap(),
            );
        }
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(false).unwrap();
        let owner = ui.working_sets.active_id;
        ui.select_relationship(
            Some(comparison_disclosure::occurrence_id(owner, CardId(2))),
            None,
        )
        .unwrap();
        let copy = ui.set.clone();
        let other = ui.working_sets.create(&mut ui.set, "Other", copy);
        ui.set.cards[1].label = "Other owner's Card 2".into();
        ui.rehearsal_owner = Some(other);
        ui.rehearsal_running = true;
        ui.open_relationship_source().unwrap();
        assert_eq!(ui.working_sets.active_id, owner);
        assert_eq!(ui.set.cursor_id(), Some(CardId(2)));
        assert_ne!(ui.set.cards[1].label, "Other owner's Card 2");
        assert_eq!(
            ui.working_sets.get(other, &ui.set).unwrap().cards[1].label,
            "Other owner's Card 2"
        );
        assert_eq!(ui.rehearsal_owner, Some(other));
        assert!(ui.rehearsal_running);
    }
    #[test]
    fn source_envelope_and_unknown_versions_refuse_but_preserve_bytes() {
        let mut ui = UiState::new();
        for _ in 0..2 {
            ui.set.push(
                KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(0),
                }
                .to_card()
                .unwrap(),
            );
        }
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(false).unwrap();
        let valid = ui.relationship_reading().unwrap().unwrap();
        for change in 0..3 {
            let mut reading = valid.clone();
            match change {
                0 => reading.version = 4,
                1 => reading.source.as_mut().unwrap().cards.push(CardId(1)),
                _ => reading.source.as_mut().unwrap().owner = WorkingSetId(0),
            };
            let bytes = serde_json::to_string(&reading).unwrap();
            ui.relationship_reading_json = Some(bytes.clone());
            assert!(ui.relationship_reading().is_err());
            assert_eq!(ui.to_persisted().relationship_reading_json, Some(bytes));
        }
    }
}
