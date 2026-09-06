// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Emit the bounded musical comparison used by the projection proof.
//!
//! This is a product-owned disclosure example. It derives keyed pitch sets
//! from Woodshed's real `Set` cards and chord catalog, while the JSON shape is
//! deliberately independent of Woodshed crates at the consuming edge.
//!
//! The method is intentionally narrow: it reports set intersection and, only
//! when each side has exactly one differing pitch class, names that singleton
//! as a movement. It does not define a general voice-leading assignment.

use std::collections::BTreeMap;

use sceno::SourceRef;
use serde::{Deserialize, Serialize};
use woodshed_core::stage_scene::{StageGraphSnapshot, StageSceneOptions, stage_scene};
use woodshedding::chord::catalog as chord_catalog;
use woodshedding::pitch::{Pitch, PitchClass, Spelling};
use woodshedding::rehearsal::{
    Card, CardId, Hold, LoopMode, Material, Set, Setting, Timing, Touch,
};

const AUTHORITY: &str = "woodshed";
const DOMAIN: &str = "music.practice";
const RESOURCE: &str = "set:musical-comparison";
const REVISION: &str = "musical-comparison-fixture-v1";
const METHOD_ID: &str = "keyed-pitch-set-singleton";
const METHOD_VERSION: u32 = 1;
const RESULT_VERSION: u32 = 1;

/// A product-neutral disclosure for one explicit comparison contribution.
///
/// The host may persist this value when the player explicitly contributes it.
/// Constructing or printing the example does not write persistence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MusicalComparisonDisclosure {
    pub source: SourceBinding,
    pub revision: String,
    pub method: ComparisonMethod,
    pub left: ComparisonOccurrence,
    pub right: ComparisonOccurrence,
    pub result: ComparisonResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceBinding {
    pub authority: String,
    pub domain: String,
    pub resource: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ComparisonMethod {
    pub id: String,
    pub version: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ComparisonOccurrence {
    pub occurrence_id: String,
    pub source: SourceRef,
    pub label: String,
    pub root_pitch_class: u8,
    pub pitch_set: Vec<PitchFact>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PitchFact {
    pub pitch_class: u8,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ComparisonResult {
    pub version: u32,
    pub status: ComparisonStatus,
    pub shared: Vec<PitchFact>,
    pub left_only: Vec<PitchFact>,
    pub right_only: Vec<PitchFact>,
    pub singleton_motion: Option<SingletonMotion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Applicable,
    Inapplicable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SingletonMotion {
    pub from: PitchFact,
    pub to: PitchFact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ComparisonError {
    UnsupportedMaterial,
    UnknownCatalogFormula,
}

/// The real Set used for the checked-in comparison receipt.
pub fn fixture_set() -> Set {
    Set::from_cards(
        [
            card("C Major", "Major", PitchClass::new(0)),
            card("A Minor", "Minor", PitchClass::new(9)),
        ],
        LoopMode::Off,
    )
}

fn card(label: &str, name: &str, root: PitchClass) -> Card {
    Card {
        id: CardId::UNASSIGNED,
        label: label.to_owned(),
        material: Material::Chord {
            name: name.to_owned(),
            root,
        },
        setting: Setting {
            instrument: "guitar".to_owned(),
            tuning: Some("Standard".to_owned()),
            ..Setting::default()
        },
        touch: Touch::Block,
        timing: Timing {
            bpm: Some(96.0),
            hold: Hold::Bars(2),
        },
        from: None,
    }
}

/// Construct the disclosure that a caller can explicitly contribute.
pub fn explicit_contribution() -> MusicalComparisonDisclosure {
    let set = fixture_set();
    let left_id = set.cards[0].id;
    let right_id = set.cards[1].id;
    disclosure_for(&set, left_id, right_id).expect("the fixture cards are comparable chords")
}

fn disclosure_for(
    set: &Set,
    left_id: CardId,
    right_id: CardId,
) -> Result<MusicalComparisonDisclosure, ComparisonError> {
    let stage = stage_scene(set, &StageSceneOptions::default());
    let left = occurrence_for(set, &stage, left_id)?;
    let right = occurrence_for(set, &stage, right_id)?;
    let result = compare_pitch_sets(&left, &right);
    Ok(MusicalComparisonDisclosure {
        source: SourceBinding {
            authority: AUTHORITY.into(),
            domain: DOMAIN.into(),
            resource: RESOURCE.into(),
        },
        revision: REVISION.into(),
        method: ComparisonMethod {
            id: METHOD_ID.into(),
            version: METHOD_VERSION,
        },
        left,
        right,
        result,
    })
}

fn occurrence_for(
    set: &Set,
    stage: &StageGraphSnapshot,
    card_id: CardId,
) -> Result<ComparisonOccurrence, ComparisonError> {
    let card = set
        .card(card_id)
        .ok_or(ComparisonError::UnsupportedMaterial)?;
    let (name, root) = match &card.material {
        Material::Chord { name, root } => (name.as_str(), *root),
        _ => return Err(ComparisonError::UnsupportedMaterial),
    };
    let formula = chord_catalog()
        .iter()
        .find(|formula| formula.name == name)
        .ok_or(ComparisonError::UnknownCatalogFormula)?;
    let root_pitch = Pitch::from_midi(60 + i32::from(root.value()), Spelling::Sharps);
    let pitch_set = formula
        .apply_to(root_pitch)
        .map_err(|_| ComparisonError::UnknownCatalogFormula)?
        .into_iter()
        .map(|pitch| PitchFact {
            pitch_class: pitch.pitch_class(),
            label: format!("{}{}", pitch.name, pitch.accidental),
        })
        .collect();
    let source = source_for(stage, card_id).ok_or(ComparisonError::UnsupportedMaterial)?;
    Ok(ComparisonOccurrence {
        occurrence_id: occurrence_id(card_id),
        source,
        label: card.label.clone(),
        root_pitch_class: root.value(),
        pitch_set,
    })
}

fn source_for(stage: &StageGraphSnapshot, card_id: CardId) -> Option<SourceRef> {
    let (_, item) = stage
        .items()
        .into_iter()
        .find(|(instance, _)| stage.card_of_ref(*instance) == Some(card_id))?;
    stage
        .snapshot
        .tables
        .sources
        .get(item.source.0 as usize)
        .and_then(Option::as_ref)
        .cloned()
}

fn occurrence_id(id: CardId) -> String {
    format!("card:{}", id.0)
}

/// Compare keyed sets without inventing a reusable assignment metric.
fn compare_pitch_sets(
    left: &ComparisonOccurrence,
    right: &ComparisonOccurrence,
) -> ComparisonResult {
    let left_by_pc: BTreeMap<u8, &PitchFact> = left
        .pitch_set
        .iter()
        .map(|fact| (fact.pitch_class, fact))
        .collect();
    let right_by_pc: BTreeMap<u8, &PitchFact> = right
        .pitch_set
        .iter()
        .map(|fact| (fact.pitch_class, fact))
        .collect();

    let shared = left_by_pc
        .keys()
        .filter_map(|pc| right_by_pc.get(pc).copied())
        .cloned()
        .collect();
    let left_only: Vec<PitchFact> = left_by_pc
        .iter()
        .filter(|(pc, _)| !right_by_pc.contains_key(pc))
        .map(|(_, fact)| (*fact).clone())
        .collect();
    let right_only: Vec<PitchFact> = right_by_pc
        .iter()
        .filter(|(pc, _)| !left_by_pc.contains_key(pc))
        .map(|(_, fact)| (*fact).clone())
        .collect();
    let singleton_motion =
        (left_only.len() == 1 && right_only.len() == 1).then(|| SingletonMotion {
            from: left_only[0].clone(),
            to: right_only[0].clone(),
        });
    let status = if singleton_motion.is_some() {
        ComparisonStatus::Applicable
    } else {
        ComparisonStatus::Inapplicable
    };
    ComparisonResult {
        version: RESULT_VERSION,
        status,
        shared,
        left_only,
        right_only,
        singleton_motion,
    }
}

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&explicit_contribution()).expect("fixture serializes")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occurrence(label: &str, pitches: &[(u8, &str)]) -> ComparisonOccurrence {
        ComparisonOccurrence {
            occurrence_id: label.into(),
            source: SourceRef::new("woodshed.stage", format!("test:{label}")),
            label: label.into(),
            root_pitch_class: 0,
            pitch_set: pitches
                .iter()
                .map(|(pitch_class, label)| PitchFact {
                    pitch_class: *pitch_class,
                    label: (*label).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn c_major_and_a_minor_disclose_shared_c_e_and_g_to_a() {
        let disclosure = explicit_contribution();
        assert_eq!(disclosure.left.occurrence_id, "card:1");
        assert_eq!(disclosure.right.occurrence_id, "card:2");
        assert_ne!(disclosure.left.source, disclosure.right.source);
        assert_eq!(
            disclosure
                .left
                .pitch_set
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["C", "E", "G"]
        );
        assert_eq!(
            disclosure
                .right
                .pitch_set
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["A", "C", "E"]
        );
        assert_eq!(disclosure.result.status, ComparisonStatus::Applicable);
        assert_eq!(
            disclosure
                .result
                .shared
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["C", "E"]
        );
        assert_eq!(
            disclosure
                .result
                .left_only
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["G"]
        );
        assert_eq!(
            disclosure
                .result
                .right_only
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
        assert_eq!(
            disclosure
                .result
                .singleton_motion
                .as_ref()
                .unwrap()
                .from
                .label,
            "G"
        );
        assert_eq!(
            disclosure
                .result
                .singleton_motion
                .as_ref()
                .unwrap()
                .to
                .label,
            "A"
        );
    }

    #[test]
    fn repeated_catalog_material_keeps_distinct_occurrence_ids_and_one_source() {
        let set = Set::from_cards(
            [
                card("C Major one", "Major", PitchClass::new(0)),
                card("C Major two", "Major", PitchClass::new(0)),
            ],
            LoopMode::Off,
        );
        let first = set.cards[0].id;
        let second = set.cards[1].id;
        let stage = stage_scene(&set, &StageSceneOptions::default());
        assert_ne!(first, second);
        assert_eq!(source_for(&stage, first), source_for(&stage, second));
        assert_ne!(occurrence_id(first), occurrence_id(second));
    }

    #[test]
    fn transposed_d_major_and_b_minor_use_the_same_bounded_rule() {
        let left = occurrence("d-major", &[(2, "D"), (6, "F#"), (9, "A")]);
        let right = occurrence("b-minor", &[(11, "B"), (2, "D"), (6, "F#")]);
        let result = compare_pitch_sets(&left, &right);
        assert_eq!(result.status, ComparisonStatus::Applicable);
        assert_eq!(
            result
                .shared
                .iter()
                .map(|p| p.label.as_str())
                .collect::<Vec<_>>(),
            ["D", "F#"]
        );
        assert_eq!(result.singleton_motion.as_ref().unwrap().from.label, "A");
        assert_eq!(result.singleton_motion.as_ref().unwrap().to.label, "B");
    }

    #[test]
    fn equal_sets_are_inapplicable_without_a_singleton_delta() {
        let left = occurrence("left", &[(0, "C"), (4, "E"), (7, "G")]);
        let right = occurrence("right", &[(0, "C"), (4, "E"), (7, "G")]);
        let result = compare_pitch_sets(&left, &right);
        assert_eq!(result.status, ComparisonStatus::Inapplicable);
        assert!(result.singleton_motion.is_none());
        assert!(result.left_only.is_empty());
        assert!(result.right_only.is_empty());
    }

    #[test]
    fn a_four_tone_input_is_inapplicable_when_it_has_two_singletons() {
        let left = occurrence("left", &[(0, "C"), (4, "E"), (7, "G"), (11, "B")]);
        let right = occurrence("right", &[(9, "A"), (0, "C"), (4, "E"), (2, "D")]);
        let result = compare_pitch_sets(&left, &right);
        assert_eq!(result.status, ComparisonStatus::Inapplicable);
        assert_eq!(result.left_only.len(), 2);
        assert_eq!(result.right_only.len(), 2);
    }

    #[test]
    fn non_chord_material_is_rejected_before_disclosure() {
        let set = Set::from_cards(
            [Card {
                id: CardId::UNASSIGNED,
                label: "drawn path".into(),
                material: Material::Path {
                    root: PitchClass::new(0),
                    positions: vec![(0, 0)],
                },
                setting: Setting::default(),
                touch: Touch::Block,
                timing: Timing::default(),
                from: None,
            }],
            LoopMode::Off,
        );
        let stage = stage_scene(&set, &StageSceneOptions::default());
        let error = occurrence_for(&set, &stage, set.cards[0].id).unwrap_err();
        assert_eq!(error, ComparisonError::UnsupportedMaterial);
    }

    #[test]
    fn disclosure_round_trip_preserves_provenance_and_result() {
        let disclosure = explicit_contribution();
        let wire = serde_json::to_string(&disclosure).expect("serialize disclosure");
        let decoded: MusicalComparisonDisclosure =
            serde_json::from_str(&wire).expect("decode disclosure");
        assert_eq!(decoded, disclosure);
        assert_eq!(decoded.revision, REVISION);
        assert_eq!(decoded.method.version, METHOD_VERSION);
        assert_eq!(decoded.result.version, RESULT_VERSION);
    }
}
