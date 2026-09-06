// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Export a small, deterministic Stage dataset for a host with no Woodshed
//! dependency.
//!
//! The cards and their positions come from Woodshed's real `Set` and
//! `stage_scene` seams. The wire types in this example are deliberately
//! product-neutral: a consumer only needs the source binding, a typed field
//! map, and occurrences that carry both the shared source and their own
//! occurrence identity.

use std::collections::BTreeMap;

use sceno::SourceRef;
use serde::{Deserialize, Serialize};
use woodshed_core::stage_scene::{StageSceneOptions, stage_scene};
use woodshedding::pitch::PitchClass;
use woodshedding::rehearsal::{
    Card, CardId, Hold, LoopMode, Material, Set, Setting, Timing, Touch,
};

const AUTHORITY: &str = "woodshed";
const DOMAIN: &str = "music.practice";
const RESOURCE: &str = "stage-set:catalog-fixture";
const REVISION: &str = "stage-fixture-v1";

/// The source binding and revision selected by the producer of this dataset.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectionDataset {
    pub source: SourceBinding,
    pub revision: String,
    pub fields: BTreeMap<String, ProjectionFieldType>,
    pub occurrences: Vec<ProjectionOccurrence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceBinding {
    pub authority: String,
    pub domain: String,
    pub resource: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionFieldType {
    Text,
    Number,
    Boolean,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectionOccurrence {
    pub occurrence_id: String,
    pub source: SourceRef,
    pub values: BTreeMap<String, ProjectionValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ProjectionValue {
    Text(String),
    Number(f64),
    Boolean(bool),
}

/// Build the actual Set used by the export receipt.
///
/// The first two cards are the same catalog material. `Set::from_cards` still
/// assigns distinct CardIds, which makes the repeated-source/unique-instance
/// distinction visible to a downstream projection.
pub fn fixture_set() -> Set {
    Set::from_cards(
        [
            card("C Major", "Major", PitchClass::new(0), 96.0, 2),
            card("C Major", "Major", PitchClass::new(0), 104.0, 1),
            card("G7", "Dominant 7", PitchClass::new(7), 112.0, 2),
        ],
        LoopMode::Off,
    )
}

fn card(label: &str, name: &str, root: PitchClass, bpm: f32, bars: u8) -> Card {
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
            bpm: Some(bpm),
            hold: Hold::Bars(bars),
        },
        from: None,
    }
}

/// Project the fixture Set into a host-readable dataset.
pub fn fixture_dataset() -> ProjectionDataset {
    let set = fixture_set();
    let stage = stage_scene(&set, &StageSceneOptions::default());
    let mut fields = BTreeMap::new();
    fields.insert("catalog_id".into(), ProjectionFieldType::Text);
    fields.insert("label".into(), ProjectionFieldType::Text);
    fields.insert("material_kind".into(), ProjectionFieldType::Text);
    fields.insert("occurrence_id".into(), ProjectionFieldType::Text);
    fields.insert("order".into(), ProjectionFieldType::Number);
    fields.insert("root_pitch_class".into(), ProjectionFieldType::Number);
    fields.insert("tempo_bpm".into(), ProjectionFieldType::Number);
    fields.insert("x".into(), ProjectionFieldType::Number);
    fields.insert("y".into(), ProjectionFieldType::Number);

    let occurrences = stage
        .items()
        .into_iter()
        .map(|(instance, item)| {
            let card_id = stage
                .card_of_ref(instance)
                .expect("every Stage item maps to a Card occurrence");
            let card = set.card(card_id).expect("Stage Card exists in source Set");
            let source = stage
                .snapshot
                .tables
                .sources
                .get(item.source.0 as usize)
                .and_then(Option::as_ref)
                .cloned()
                .expect("Stage item has a live SourceRef");
            let occurrence_id = occurrence_id(card_id);
            let (catalog_id, root_pitch_class) = match &card.material {
                Material::Chord { name, root } => (format!("chord:{name}"), root.value()),
                Material::Scale { name, root } => (format!("scale:{name}"), root.value()),
                Material::Path { root, .. } => (format!("path:{}", card_id.0), root.value()),
                Material::Riff { name } => (format!("exercise:{name}"), 0),
            };
            let bpm = card
                .timing
                .bpm
                .expect("fixture cards carry their actual tempo");
            let mut values = BTreeMap::new();
            values.insert("catalog_id".into(), ProjectionValue::Text(catalog_id));
            values.insert("label".into(), ProjectionValue::Text(card.label.clone()));
            values.insert(
                "material_kind".into(),
                ProjectionValue::Text(card.material.tag().to_owned()),
            );
            values.insert(
                "occurrence_id".into(),
                ProjectionValue::Text(occurrence_id.clone()),
            );
            values.insert(
                "order".into(),
                ProjectionValue::Number(
                    (set.index_of(card_id).expect("Card is ordered") + 1) as f64,
                ),
            );
            values.insert(
                "root_pitch_class".into(),
                ProjectionValue::Number(f64::from(root_pitch_class)),
            );
            values.insert("tempo_bpm".into(), ProjectionValue::Number(f64::from(bpm)));
            values.insert(
                "x".into(),
                ProjectionValue::Number(f64::from(item.transform.translate.x)),
            );
            values.insert(
                "y".into(),
                ProjectionValue::Number(f64::from(item.transform.translate.y)),
            );
            ProjectionOccurrence {
                occurrence_id,
                source,
                values,
            }
        })
        .collect();

    ProjectionDataset {
        source: SourceBinding {
            authority: AUTHORITY.into(),
            domain: DOMAIN.into(),
            resource: RESOURCE.into(),
        },
        revision: REVISION.into(),
        fields,
        occurrences,
    }
}

fn occurrence_id(id: CardId) -> String {
    format!("card:{}", id.0)
}

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&fixture_dataset()).expect("fixture serializes")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_distinct_occurrences_for_one_catalog_source() {
        let dataset = fixture_dataset();
        assert_eq!(dataset.occurrences.len(), 3);
        assert_eq!(dataset.occurrences[0].source, dataset.occurrences[1].source);
        assert_ne!(
            dataset.occurrences[0].occurrence_id,
            dataset.occurrences[1].occurrence_id
        );
        assert_eq!(
            dataset.occurrences[0].values["occurrence_id"],
            ProjectionValue::Text(dataset.occurrences[0].occurrence_id.clone())
        );
        assert_eq!(
            dataset.occurrences[1].values["occurrence_id"],
            ProjectionValue::Text(dataset.occurrences[1].occurrence_id.clone())
        );
    }

    #[test]
    fn fixture_uses_stage_coordinates_and_real_card_timing() {
        let dataset = fixture_dataset();
        let first = &dataset.occurrences[0];
        assert_eq!(
            first.values["catalog_id"],
            ProjectionValue::Text("chord:Major".into())
        );
        assert_eq!(first.values["order"], ProjectionValue::Number(1.0));
        assert_eq!(first.values["tempo_bpm"], ProjectionValue::Number(96.0));
        assert_ne!(first.values["x"], ProjectionValue::Number(0.0));
        assert!(matches!(first.values["y"], ProjectionValue::Number(_)));
        assert_eq!(
            dataset.occurrences[2].values["catalog_id"],
            ProjectionValue::Text("chord:Dominant 7".into())
        );
    }

    #[test]
    fn wire_round_trip_keeps_source_and_typed_values() {
        let dataset = fixture_dataset();
        let wire = serde_json::to_string(&dataset).expect("serialize fixture");
        let decoded: ProjectionDataset = serde_json::from_str(&wire).expect("decode fixture");
        assert_eq!(decoded, dataset);
    }
}
