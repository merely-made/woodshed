use super::*;

impl UiState {
    pub fn relationship_reading(&self) -> Result<Option<RelationshipReading>, String> {
        self.relationship_reading_json
            .as_deref()
            .map(RelationshipReading::decode)
            .transpose()
    }
    pub(super) fn install_relationship(
        &mut self,
        reading: RelationshipReading,
    ) -> Result<(), String> {
        source_return::PreparedReading::prepare(&reading)?.install(self);
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
        let set = source_return::current_set(
            self,
            source,
            "Source instructions changed. Explicitly rebind before returning to a Card.",
        )?;
        let destination = source_return::destination(&reading, source, set)?;
        source_return::navigate(self, destination)
    }
    pub fn relationship_result(&mut self, result: Result<(), String>) {
        self.relationship_notice = Some(match result {
            Ok(()) => "Reading updated in this session.".into(),
            Err(e) => e,
        });
    }
}
