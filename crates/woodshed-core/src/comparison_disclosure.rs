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
pub const TONE_DIFFERENCES: &str = "music.pitch_class_differences";
pub const PITCH_CONTAINMENT: &str = "music.pitch_class_containment";
pub const PITCH_EQUALITY: &str = "music.same_pitch_classes";
pub const FULL_COMPARISON_METHOD: &str = "woodshed.keyed_pitch_set_comparison.v1";
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
    SelectionTooLarge,
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

/// Richer owner-disclosed relationships over the same exact selected pair.
/// Containment points from the container to the contained occurrence. Equality
/// is distinct from material identity: a chord and its arpeggio can share tones.
pub fn comparison_projection_json(material: &SelectedMaterial) -> serde_json::Value {
    use serde_json::json;
    let mut value = projection_json(material);
    let left = material
        .occurrences
        .iter()
        .find(|o| o.occurrence_id == material.relationship.from)
        .expect("disclosed endpoint");
    let right = material
        .occurrences
        .iter()
        .find(|o| o.occurrence_id == material.relationship.to)
        .expect("disclosed endpoint");
    let comparison =
        compare_pitch_sets(&left.material, &right.material).expect("validated catalog material");
    let names = |tones: &BTreeSet<woodshedding::pitch::PitchClass>| {
        let labels = tones
            .iter()
            .map(|pc| {
                let p = Pitch::from_midi(60 + i32::from(pc.value()), Spelling::Sharps);
                format!("{}{}", p.name, p.accidental)
            })
            .collect::<Vec<_>>();
        if labels.is_empty() {
            "none".into()
        } else {
            labels.join(", ")
        }
    };
    let mut provenance = value["relationships"][0]["provenance"].clone();
    provenance["method"] = json!(FULL_COMPARISON_METHOD);
    let relation = |kind: &str, label: &str, from: &str, to: &str, explanation: String| {
        json!({
            "id": format!("set:{}:{}:{}:{}", material.owner.0, kind, left.card.0, right.card.0),
            "from_occurrence": from, "to_occurrence": to, "kind": kind,
            "label": label,
            "explanation": format!("{explanation} Exact catalog pitch classes; register, fingering, voice leading, key and harmonic function are not inferred."),
            "provenance": provenance,
        })
    };
    let mut relationships = if comparison.shared.is_empty() {
        Vec::new()
    } else {
        vec![value["relationships"][0].clone()]
    };
    if comparison.left_only.is_empty() && comparison.right_only.is_empty() {
        relationships.push(relation(PITCH_EQUALITY, "Same pitch classes", &left.occurrence_id, &right.occurrence_id,
            format!("{} and {} have the same pitch classes: {}. Their occurrences and articulation remain distinct.", left.label, right.label, names(&comparison.shared))));
    } else {
        relationships.push(relation(TONE_DIFFERENCES, "Pitch-class differences", &left.occurrence_id, &right.occurrence_id,
            format!("Shared: {}. Only in {}: {}. Only in {}: {}. These are set differences, not paired voice movements.", names(&comparison.shared), left.label, names(&comparison.left_only), right.label, names(&comparison.right_only))));
        let container = if comparison.left_only.is_empty() {
            Some((right, left, &comparison.right_only))
        } else if comparison.right_only.is_empty() {
            Some((left, right, &comparison.left_only))
        } else {
            None
        };
        if let Some((outer, inner, extras)) = container {
            relationships.push(relation(
                PITCH_CONTAINMENT,
                "Pitch-class containment",
                &outer.occurrence_id,
                &inner.occurrence_id,
                format!(
                    "{} contains every pitch class of {}. Additional tones in {}: {}.",
                    outer.label,
                    inner.label,
                    outer.label,
                    names(extras)
                ),
            ));
        }
    }
    value["relationships"] = json!(relationships);
    value
}

/// Compare adjacent selected occurrences in authored order, bounded by the host.
/// A sparse selection is a selected-material sequence, not a claim of adjacency
/// in the complete Set. Repeated materials keep their distinct occurrence IDs.
pub fn passage_projection_json(
    owner: WorkingSetId,
    revision: &str,
    set: &Set,
    selected: &[CardId],
) -> Result<serde_json::Value, DisclosureError> {
    if selected.len() > 64 {
        return Err(DisclosureError::SelectionTooLarge);
    }
    let ordered = set
        .cards
        .iter()
        .filter(|c| selected.contains(&c.id))
        .map(|c| c.id)
        .collect::<Vec<_>>();
    let pair = ordered
        .first()
        .zip(ordered.get(1))
        .map(|(a, b)| (*a, *b))
        .ok_or(DisclosureError::EmptySelection)?;
    // Validate all selected identities/material before disclosing any pair.
    let facts = disclose_comparison(owner, revision, set, selected, pair)?;
    let mut value = comparison_projection_json(&facts);
    let mut relationships = Vec::new();
    for pair in ordered.windows(2) {
        let facts = disclose_comparison(owner, revision, set, selected, (pair[0], pair[1]))?;
        let pair_value = comparison_projection_json(&facts);
        for mut relation in pair_value["relationships"]
            .as_array()
            .expect("owner schema")
            .iter()
            .cloned()
        {
            let label = |field: &str| {
                facts
                    .occurrences
                    .iter()
                    .find(|o| o.occurrence_id == relation[field].as_str().unwrap())
                    .unwrap()
                    .label
                    .clone()
            };
            relation["label"] = serde_json::json!(format!(
                "{} · {} / {}",
                relation["label"].as_str().unwrap(),
                label("from_occurrence"),
                label("to_occurrence")
            ));
            relationships.push(relation);
        }
    }
    value["relationships"] = serde_json::json!(relationships);
    Ok(value)
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
    disclose_pair(owner, revision, set, selected, pair, true)
}

/// Complete exact comparison, including pairs with no shared pitch classes.
/// The legacy overlap-only disclosure remains unchanged for captured readings.
pub fn disclose_comparison(
    owner: WorkingSetId,
    revision: &str,
    set: &Set,
    selected: &[CardId],
    pair: (CardId, CardId),
) -> Result<SelectedMaterial, DisclosureError> {
    disclose_pair(owner, revision, set, selected, pair, false)
}

fn disclose_pair(
    owner: WorkingSetId,
    revision: &str,
    set: &Set,
    selected: &[CardId],
    pair: (CardId, CardId),
    require_overlap: bool,
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
    if require_overlap && comparison.shared.is_empty() {
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
    fn passage_compares_selected_authored_neighbors_and_preserves_repeated_occurrences() {
        let set = fixture();
        let ids = set.cards.iter().map(|c| c.id).collect::<Vec<_>>();
        let value = passage_projection_json(WorkingSetId(1), "v1", &set, &[ids[2], ids[0], ids[1]])
            .unwrap();
        let relations = value["relationships"].as_array().unwrap();
        assert_eq!(relations.len(), 4);
        assert!(relations.iter().any(
            |r| r["from_occurrence"] == "set:1:card:1" && r["to_occurrence"] == "set:1:card:2"
        ));
        assert!(relations.iter().any(
            |r| r["from_occurrence"] == "set:1:card:2" && r["to_occurrence"] == "set:1:card:3"
        ));
        assert!(!relations.iter().any(
            |r| r["from_occurrence"] == "set:1:card:1" && r["to_occurrence"] == "set:1:card:3"
        ));
        let sparse =
            passage_projection_json(WorkingSetId(1), "v1", &set, &[ids[2], ids[0]]).unwrap();
        assert_eq!(
            sparse["dataset"]["occurrences"][1]["values"]["order"]["value"],
            3
        );
        assert_eq!(
            sparse["relationships"][0]["from_occurrence"],
            "set:1:card:1"
        );
        assert_eq!(sparse["relationships"][0]["to_occurrence"], "set:1:card:3");
        assert_eq!(
            passage_projection_json(WorkingSetId(1), "v1", &set, &vec![ids[0]; 65]),
            Err(DisclosureError::SelectionTooLarge)
        );
    }

    #[test]
    fn seventh_chord_comparison_discloses_shared_and_unique_tones_without_voice_assignment() {
        let mut set = fixture();
        set.cards[0].material = Material::Chord {
            name: "Major 7".into(),
            root: PitchClass::new(0),
        };
        set.cards[2].material = Material::Chord {
            name: "Minor 7".into(),
            root: PitchClass::new(9),
        };
        let ids = [set.cards[0].id, set.cards[2].id];
        let facts =
            disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
        let value = comparison_projection_json(&facts);
        assert_eq!(facts.relationship.shared_pitch_classes, [0, 4, 7]);
        let difference = &value["relationships"][1];
        assert_eq!(difference["kind"], TONE_DIFFERENCES);
        let explanation = difference["explanation"].as_str().unwrap();
        assert!(explanation.contains("Shared: C, E, G."));
        assert!(explanation.contains("Only in C Major: B."));
        assert!(explanation.contains("Only in A Minor: A."));
        assert_eq!(value["relationships"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn scale_contains_chord_with_directed_endpoints_and_exact_extra_tones() {
        let mut set = fixture();
        set.cards[2].material = Material::Scale {
            name: "Major".into(),
            root: PitchClass::new(0),
        };
        set.cards[2].label = "C Major scale".into();
        let ids = [set.cards[0].id, set.cards[2].id];
        let facts =
            disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
        let value = comparison_projection_json(&facts);
        let contained = &value["relationships"][2];
        assert_eq!(contained["kind"], PITCH_CONTAINMENT);
        assert_eq!(
            contained["from_occurrence"],
            occurrence_id(WorkingSetId(1), ids[1])
        );
        assert_eq!(
            contained["to_occurrence"],
            occurrence_id(WorkingSetId(1), ids[0])
        );
        assert!(
            contained["explanation"]
                .as_str()
                .unwrap()
                .contains("D, F, A, B")
        );
    }

    #[test]
    fn disjoint_comparison_has_differences_but_no_false_overlap_or_containment() {
        let mut set = fixture();
        set.cards[2].material = Material::Chord {
            name: "Major".into(),
            root: PitchClass::new(1),
        };
        let ids = [set.cards[0].id, set.cards[2].id];
        let facts =
            disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
        let value = comparison_projection_json(&facts);
        assert_eq!(value["relationships"].as_array().unwrap().len(), 1);
        assert_eq!(value["relationships"][0]["kind"], TONE_DIFFERENCES);
        assert!(
            value["relationships"][0]["explanation"]
                .as_str()
                .unwrap()
                .contains("Shared: none.")
        );
        assert_eq!(
            disclose(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])),
            Err(DisclosureError::NoSharedPitchClasses)
        );
    }

    #[test]
    fn chord_and_arpeggio_equality_keeps_articulation_identity() {
        let mut set = fixture();
        set.cards[1].touch = woodshedding::rehearsal::Touch::Arpeggiate {
            direction: woodshedding::rehearsal::ArpeggioDirection::Up,
            inversion: 0,
        };
        let ids = [set.cards[0].id, set.cards[1].id];
        let facts =
            disclose_comparison(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])).unwrap();
        let value = comparison_projection_json(&facts);
        assert_ne!(
            facts.occurrences[0].material.wire_key(),
            facts.occurrences[1].material.wire_key()
        );
        assert_eq!(value["relationships"][1]["kind"], PITCH_EQUALITY);
        assert_eq!(value["relationships"].as_array().unwrap().len(), 2);
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
