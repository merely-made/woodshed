//! Synchronous source-return preparation. No retained cache or authority handle.
use super::{
    MAX_READING_BYTES, ReadingSource, RelationshipDataset, RelationshipReading, UiState, owner_set,
    revision,
};
use woodshed_core::{comparison_disclosure, working_sets::WorkingSetId};
use woodshedding::rehearsal::{CardId, Set};

pub(super) struct PreparedReading {
    json: String,
    label: String,
}
impl PreparedReading {
    pub(super) fn prepare(reading: &RelationshipReading) -> Result<Self, String> {
        reading.compile()?;
        let json = serde_json::to_string(reading).map_err(|e| e.to_string())?;
        if json.len() > MAX_READING_BYTES {
            return Err("Reading exceeds the 2 MiB limit.".into());
        }
        Ok(Self {
            json,
            label: reading.snapshot.recipe.definition.label.clone(),
        })
    }
    pub(super) fn install(self, ui: &mut UiState) {
        #[cfg(test)]
        super::source_return_stats::record(super::source_return_stats::Event::Install);
        ui.relationship_label = cambium::TextInput::new(self.label);
        ui.relationship_reading_json = Some(self.json);
    }
}

pub(super) fn current_set<'a>(
    ui: &'a UiState,
    source: &ReadingSource,
    stale: &str,
) -> Result<&'a Set, String> {
    let set = owner_set(ui, source.owner)?;
    if revision(source.owner, set) != source.revision {
        return Err(stale.into());
    }
    Ok(set)
}

pub(super) struct Destination {
    owner: WorkingSetId,
    card: CardId,
}
/// Pure preparation against the already resolved current Set. A member action
/// retains this Result until after its original reading-installation boundary.
pub(super) fn destination(
    reading: &RelationshipReading,
    source: &ReadingSource,
    set: &Set,
) -> Result<Destination, String> {
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
    let current = reconstruct(reading.version, source, set)?;
    if serde_json::to_value(current).map_err(|e| e.to_string())?
        != serde_json::to_value(&reading.dataset).map_err(|e| e.to_string())?
    {
        return Err("Retained disclosure does not match the source Set.".into());
    }
    Ok(Destination {
        owner: source.owner,
        card,
    })
}
fn reconstruct(
    version: u32,
    source: &ReadingSource,
    set: &Set,
) -> Result<RelationshipDataset, String> {
    #[cfg(test)]
    super::source_return_stats::record(super::source_return_stats::Event::Reconstruct);
    let value = if version >= 3 {
        comparison_disclosure::comparison_passage_projection_json(
            source.owner,
            &source.revision,
            set,
            &source.cards,
            source.pair,
        )
        .map_err(|e| format!("Source refused: {e:?}"))?
    } else {
        let disclose = if version >= 2 {
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
        if version >= 2 {
            comparison_disclosure::comparison_projection_json(&facts)
        } else {
            comparison_disclosure::projection_json(&facts)
        }
    };
    serde_json::from_value(value).map_err(|e| e.to_string())
}
pub(super) fn navigate(ui: &mut UiState, destination: Destination) -> Result<(), String> {
    #[cfg(test)]
    super::source_return_stats::record(super::source_return_stats::Event::Navigate);
    ui.activate_working_set(destination.owner);
    ui.set.select_id(destination.card);
    ui.set_graph_card_expanded = true;
    ui.relationship_open = false;
    Ok(())
}
