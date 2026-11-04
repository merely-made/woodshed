use super::*;

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

pub(super) fn set_uses_catalog(set: &Set) -> bool {
    set.cards.iter().any(|card| match &card.material {
        woodshedding::rehearsal::Material::Riff { name } => woodshedding::exercise::catalog()
            .iter()
            .any(|exercise| exercise.name == name),
        material => crate::harmony::KeyedCatalogRef::from_material(material)
            .and_then(|reference| reference.to_material())
            .is_some(),
    })
}
