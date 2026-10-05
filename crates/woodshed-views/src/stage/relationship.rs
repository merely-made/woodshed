//! A retained reading belongs to Woodshed; authored grammar and layout are shared.
use super::{UiChild, UiState, WorkspacePanel};
use cambium::{
    GraphCanvasEvent, GraphCanvasNode, GraphCanvasRelation, GraphCanvasSubgraph, GraphCanvasSwatch,
    clickable, el, graph_canvas, map_state, text, text_field,
};
use scenograph::relationship::{
    RecipeEdit, RelationshipRecipeDraft, RelationshipSnapshot, relationship_recipe,
};
use scenograph::{ProjectionInputBinding, PublicSourceRevision, RevisionEvidence};
use scenomise::projection::{
    CompiledRelationshipProjection, RelationshipDataset, compile_relationship_snapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
        if reading.version > 1 {
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
        compile_relationship_snapshot(&self.snapshot, &self.dataset)
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
            return Err("Choose between two and 64 Cards; the first two selected occurrences supply the comparison.".into());
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
        let facts = comparison_disclosure::disclose(owner, &revision, set, &cards, pair)
            .map_err(|e| format!("Comparison unavailable: {e:?}"))?;
        let dataset: RelationshipDataset =
            serde_json::from_value(comparison_disclosure::projection_json(&facts))
                .map_err(|e| e.to_string())?;
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
            version: 1,
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
        let facts = comparison_disclosure::disclose(
            source.owner,
            &source.revision,
            set,
            &source.cards,
            source.pair,
        )
        .map_err(|e| format!("Source refused: {e:?}"))?;
        let current: RelationshipDataset =
            serde_json::from_value(comparison_disclosure::projection_json(&facts))
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
    Some(swatch_from(&reading, &compiled))
}
fn swatch_from(
    reading: &RelationshipReading,
    compiled: &CompiledRelationshipProjection,
) -> GraphCanvasSwatch<String, &'static str> {
    let scene = &compiled.projection.scene;
    let bounds = scene.bounds;
    let width = (bounds.size.w + 64.0).max(280.0) as u32;
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
    let relations = compiled
        .relationships
        .iter()
        .map(|r| {
            let from = &scene.items[r.from.0 as usize];
            let to = &scene.items[r.to.0 as usize];
            GraphCanvasRelation {
                id: r.disclosure.id.clone(),
                from: r.disclosure.from_occurrence.clone(),
                to: r.disclosure.to_occurrence.clone(),
                kind: r.disclosure.kind.clone(),
                label: r.disclosure.label.clone(),
                route: vec![
                    normalize(from.transform.translate.x, from.transform.translate.y),
                    normalize(to.transform.translate.x, to.transform.translate.y),
                ],
                visible: true,
                emphasized: reading.snapshot.selected_relationship.as_ref()
                    == Some(&r.disclosure.id),
            }
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
    .with_size(width, height)
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
                "Choose occurrences from the current Set. The first two selected Cards in authored order supply an exact shared pitch-class relationship. A retained reading keeps its captured evidence until you explicitly rebind.",
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
                let swatch = swatch_from(&reading, &compiled);
                children.push(Box::new(
                    el(
                        "div",
                        el(
                            "div",
                            graph_canvas(&swatch, |ui: &mut UiState, event| {
                                let r = match event {
                                    GraphCanvasEvent::Activate(id) => {
                                        ui.select_relationship(Some(id), None)
                                    },
                                    GraphCanvasEvent::RelationActivate(id) => {
                                        let left =
                                            ui.relationship_reading().ok().flatten().and_then(
                                                |r| {
                                                    r.dataset
                                                        .relationships
                                                        .iter()
                                                        .find(|r| r.id == id)
                                                        .map(|r| r.from_occurrence.clone())
                                                },
                                            );
                                        ui.select_relationship(left, Some(id))
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
        assert_eq!(compiled.relationships.len(), 1);
        assert_eq!(compiled.projection.scene.relations.len(), 1);
        assert_eq!(
            compiled.relationships[0].disclosure.explanation,
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
                0 => reading.version = 2,
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
