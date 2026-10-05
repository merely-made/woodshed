//! Owner-local facts for the bounded cross-domain comparison recipe.
//!
//! This is a musical disclosure, not a scene or authoring model. The shared
//! compiler receives an adapter's projection of these facts. Neither reading
//! the facts nor compiling them authors Cards or starts a practice runner.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use woodshedding::pitch::{Pitch, Spelling};
use woodshedding::rehearsal::{CardId, Set};

use crate::harmony::{KeyedCatalogRef, compare_pitch_sets};
use crate::working_sets::WorkingSetId;

pub const SHARED_PITCH_CLASSES: &str = "music.shared_pitch_classes";
pub const COMPARISON_METHOD: &str = "woodshed.keyed_pitch_set_intersection.v1";
pub const DISCLOSURE_ADAPTER: &str = "woodshed.selected-material";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedMaterial {
    pub owner: WorkingSetId,
    /// Supplied by the owning host, not inferred from a layout or clock.
    pub revision: String,
    pub occurrences: Vec<SelectedOccurrence>,
    pub relationship: SharedPitchRelationship,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedOccurrence {
    pub card: CardId,
    pub occurrence_id: String,
    pub material: KeyedCatalogRef,
    pub label: String,
    /// One-based position in the owner's authored Set, not selection order.
    pub order: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedPitchRelationship {
    pub id: String,
    pub from: String,
    pub to: String,
    pub shared_pitch_classes: Vec<u8>,
    pub explanation: String,
    pub method: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisclosureError {
    InvalidOwner,
    MissingRevision,
    EmptySelection,
    DuplicateSelection(CardId),
    UnknownCard(CardId),
    InvalidCardIdentity(CardId),
    UnsupportedMaterial(CardId),
    EndpointNotSelected(CardId),
    SameOccurrence,
    NoSharedPitchClasses,
}

pub fn occurrence_id(owner: WorkingSetId, card: CardId) -> String {
    format!("set:{}:card:{}", owner.0, card.0)
}

/// Serialize into the shared relationship dataset's disclosure boundary.
///
/// JSON keeps this owner on its existing shared dependency revision while the
/// receiving compiler owns its typed schema. No projection grammar is copied
/// here. The original Stage fixture format is unaffected.
pub fn projection_json(material: &SelectedMaterial) -> serde_json::Value {
    use serde_json::json;
    let source = json!({
        "authority": "woodshed",
        "domain": "music.practice",
        "resource": format!("working-set:{}", material.owner.0),
    });
    let occurrences = material
        .occurrences
        .iter()
        .map(|item| {
            json!({
                "occurrence_id": item.occurrence_id,
                "source": {"adapter": DISCLOSURE_ADAPTER, "id": item.material.wire_key()},
                "values": {
                    "occurrence_id": {"kind": "text", "value": item.occurrence_id},
                    "label": {"kind": "text", "value": item.label},
                    "order": {"kind": "number", "value": item.order},
                },
            })
        })
        .collect::<Vec<_>>();
    let evidence = material
        .occurrences
        .iter()
        .filter(|item| {
            item.occurrence_id == material.relationship.from
                || item.occurrence_id == material.relationship.to
        })
        .map(|item| json!({"adapter": DISCLOSURE_ADAPTER, "id": item.material.wire_key()}))
        .collect::<Vec<_>>();
    json!({
        "dataset": {
            "source": source,
            "revision": material.revision,
            "fields": {"occurrence_id": "text", "label": "text", "order": "number"},
            "occurrences": occurrences,
        },
        "facets": ["authored_order", "occurrence_labels", "explained_relationships"],
        "relationships": [{
            "id": material.relationship.id,
            "from_occurrence": material.relationship.from,
            "to_occurrence": material.relationship.to,
            "kind": SHARED_PITCH_CLASSES,
            "label": "Shared pitch classes",
            "explanation": material.relationship.explanation,
            "provenance": {
                "source": source,
                "source_revision": material.revision,
                "method": material.relationship.method,
                "method_version": 1,
                "provider": "woodshed",
                "evidence": evidence,
            },
        }],
    })
}

/// Disclose selected catalog-backed material and one exact tone relationship.
/// The caller selects the owner and pair; unavailable facts are refused rather
/// than replaced by a similarly typed field or an inferred harmonic reading.
pub fn disclose(
    owner: WorkingSetId,
    revision: &str,
    set: &Set,
    selected: &[CardId],
    pair: (CardId, CardId),
) -> Result<SelectedMaterial, DisclosureError> {
    if owner.0 == 0 {
        return Err(DisclosureError::InvalidOwner);
    }
    if revision.trim().is_empty() {
        return Err(DisclosureError::MissingRevision);
    }
    if selected.is_empty() {
        return Err(DisclosureError::EmptySelection);
    }
    let mut included = BTreeSet::new();
    for &id in selected {
        if !id.is_assigned() || set.cards.iter().filter(|card| card.id == id).count() > 1 {
            return Err(DisclosureError::InvalidCardIdentity(id));
        }
        if !included.insert(id) {
            return Err(DisclosureError::DuplicateSelection(id));
        }
        if set.card(id).is_none() {
            return Err(DisclosureError::UnknownCard(id));
        }
    }
    for endpoint in [pair.0, pair.1] {
        if !included.contains(&endpoint) {
            return Err(DisclosureError::EndpointNotSelected(endpoint));
        }
    }
    if pair.0 == pair.1 {
        return Err(DisclosureError::SameOccurrence);
    }
    let occurrences = set
        .cards
        .iter()
        .enumerate()
        .filter(|(_, card)| included.contains(&card.id))
        .map(|(index, card)| {
            let material = KeyedCatalogRef::from_card(card)
                .ok_or(DisclosureError::UnsupportedMaterial(card.id))?;
            Ok(SelectedOccurrence {
                card: card.id,
                occurrence_id: occurrence_id(owner, card.id),
                material,
                label: card.label.clone(),
                order: index + 1,
            })
        })
        .collect::<Result<Vec<_>, DisclosureError>>()?;
    let left = occurrences
        .iter()
        .find(|entry| entry.card == pair.0)
        .unwrap();
    let right = occurrences
        .iter()
        .find(|entry| entry.card == pair.1)
        .unwrap();
    let comparison = compare_pitch_sets(&left.material, &right.material)
        .ok_or(DisclosureError::UnsupportedMaterial(pair.0))?;
    if comparison.shared.is_empty() {
        return Err(DisclosureError::NoSharedPitchClasses);
    }
    let labels = comparison
        .shared
        .iter()
        .map(|pc| {
            let pitch = Pitch::from_midi(60 + i32::from(pc.value()), Spelling::Sharps);
            format!("{}{}", pitch.name, pitch.accidental)
        })
        .collect::<Vec<_>>();
    let relationship = SharedPitchRelationship {
        id: format!(
            "set:{}:shared-pitch-classes:{}:{}",
            owner.0, pair.0.0, pair.1.0
        ),
        from: left.occurrence_id.clone(),
        to: right.occurrence_id.clone(),
        shared_pitch_classes: comparison.shared.iter().map(|pc| pc.value()).collect(),
        explanation: format!(
            "{} and {} share {} by exact pitch-class intersection; register, fingering, and harmonic function are not inferred.",
            left.label,
            right.label,
            labels.join(", ")
        ),
        method: COMPARISON_METHOD.into(),
    };
    Ok(SelectedMaterial {
        owner,
        revision: revision.into(),
        occurrences,
        relationship,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::{LoopMode, Material};

    fn fixture() -> Set {
        Set::from_cards(
            [
                ("C Major", "Major", 0),
                ("C Major again", "Major", 0),
                ("A Minor", "Minor", 9),
            ]
            .map(|(label, name, root)| {
                let mut card = KeyedCatalogRef {
                    formula_id: format!("chord:{name}"),
                    root: PitchClass::new(root),
                }
                .to_card()
                .unwrap();
                card.label = label.into();
                card
            }),
            LoopMode::Off,
        )
    }

    fn read(set: &Set, owner: WorkingSetId) -> Result<SelectedMaterial, DisclosureError> {
        let ids = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        disclose(owner, "fixture-v1", set, &ids, (ids[0], ids[2]))
    }

    #[test]
    fn repeats_share_material_but_not_occurrence_or_owner_identity() {
        let set = fixture();
        let first = read(&set, WorkingSetId(1)).unwrap();
        let other = read(&set, WorkingSetId(2)).unwrap();
        assert_eq!(first.occurrences[0].material, first.occurrences[1].material);
        assert_ne!(
            first.occurrences[0].occurrence_id,
            first.occurrences[1].occurrence_id
        );
        assert_ne!(
            first.occurrences[0].occurrence_id,
            other.occurrences[0].occurrence_id
        );
        assert_ne!(first.relationship.id, other.relationship.id);
    }

    #[test]
    fn authored_order_survives_reversed_and_sparse_selection() {
        let set = fixture();
        let (a, b) = (set.cards[0].id, set.cards[2].id);
        let read = disclose(WorkingSetId(1), "v1", &set, &[b, a], (a, b)).unwrap();
        assert_eq!(
            read.occurrences
                .iter()
                .map(|item| item.order)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(read.occurrences[0].card, a);
    }

    #[test]
    fn comparison_is_exact_and_does_not_mutate_the_set() {
        let set = fixture();
        let before = serde_json::to_vec(&set).unwrap();
        let read = read(&set, WorkingSetId(1)).unwrap();
        assert_eq!(read.relationship.shared_pitch_classes, [0, 4]);
        assert!(read.relationship.explanation.contains("C, E"));
        assert_eq!(read.relationship.method, COMPARISON_METHOD);
        assert_eq!(serde_json::to_vec(&set).unwrap(), before);
        assert_eq!(read.relationship.from, read.occurrences[0].occurrence_id);
        assert_eq!(read.relationship.to, read.occurrences[2].occurrence_id);
    }

    #[test]
    fn missing_and_unselected_endpoints_are_refused() {
        let set = fixture();
        let ids = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        assert_eq!(
            disclose(
                WorkingSetId(1),
                "v1",
                &set,
                &[CardId(999)],
                (ids[0], ids[2])
            ),
            Err(DisclosureError::UnknownCard(CardId(999)))
        );
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &[ids[0]], (ids[0], ids[2])),
            Err(DisclosureError::EndpointNotSelected(ids[2]))
        );
        assert_eq!(
            disclose(
                WorkingSetId(1),
                "v1",
                &set,
                &[ids[0], ids[0]],
                (ids[0], ids[0])
            ),
            Err(DisclosureError::DuplicateSelection(ids[0]))
        );
    }

    #[test]
    fn unknown_material_and_missing_revision_are_refused() {
        let mut set = fixture();
        let bad = set.cards[1].id;
        set.cards[1].material = Material::Chord {
            name: "uninstalled formula".into(),
            root: PitchClass::new(0),
        };
        assert_eq!(
            read(&set, WorkingSetId(1)),
            Err(DisclosureError::UnsupportedMaterial(bad))
        );
        set = fixture();
        assert_eq!(
            read(&set, WorkingSetId(0)),
            Err(DisclosureError::InvalidOwner)
        );
        assert_eq!(
            disclose(WorkingSetId(1), " ", &set, &[], (CardId(1), CardId(2))),
            Err(DisclosureError::MissingRevision)
        );
    }

    #[test]
    fn different_roots_have_different_material_identity() {
        let mut set = fixture();
        set.cards[1].material = Material::Chord {
            name: "Major".into(),
            root: PitchClass::new(2),
        };
        let read = read(&set, WorkingSetId(1)).unwrap();
        assert_ne!(
            read.occurrences[0].material.wire_key(),
            read.occurrences[1].material.wire_key()
        );
    }

    #[test]
    fn malformed_occurrence_identity_is_not_disclosed() {
        let mut set = fixture();
        set.cards[1].id = set.cards[0].id;
        let id = set.cards[0].id;
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &[id], (id, id)),
            Err(DisclosureError::InvalidCardIdentity(id))
        );
        set.cards[0].id = CardId::UNASSIGNED;
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &[CardId::UNASSIGNED], (id, id)),
            Err(DisclosureError::InvalidCardIdentity(CardId::UNASSIGNED))
        );
    }

    #[test]
    fn empty_or_false_relationships_are_not_invented() {
        let mut set = fixture();
        let (a, b) = (set.cards[0].id, set.cards[2].id);
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &[], (a, b)),
            Err(DisclosureError::EmptySelection)
        );
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &[a], (a, a)),
            Err(DisclosureError::SameOccurrence)
        );
        set.cards[2].material = Material::Chord {
            name: "Major".into(),
            root: PitchClass::new(1),
        };
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &[a, b], (a, b)),
            Err(DisclosureError::NoSharedPitchClasses)
        );
    }
}
