//! JSON schema construction over already resolved musical facts.
use super::{
    DISCLOSURE_ADAPTER, FULL_COMPARISON_METHOD, PITCH_CONTAINMENT, PITCH_EQUALITY,
    SHARED_PITCH_CLASSES, SelectedOccurrence, SharedPitchRelationship, TONE_DIFFERENCES,
    WorkingSetId,
};
use crate::harmony::PitchSetComparison;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use woodshedding::pitch::{Pitch, Spelling};

pub(super) fn dataset(
    owner: WorkingSetId,
    revision: &str,
    occurrences: &[SelectedOccurrence],
) -> Value {
    #[cfg(test)]
    super::construction_stats::dataset();
    let occurrences = occurrences
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
    json!({
        "dataset": {
            "source": source(owner), "revision": revision,
            "fields": {"occurrence_id": "text", "label": "text", "order": "number"},
            "occurrences": occurrences,
        },
        "facets": ["authored_order", "occurrence_labels", "explained_relationships"],
        "relationships": [],
    })
}
fn source(owner: WorkingSetId) -> Value {
    json!({"authority": "woodshed", "domain": "music.practice",
        "resource": format!("working-set:{}", owner.0)})
}
pub(super) fn overlap(
    owner: WorkingSetId,
    revision: &str,
    occurrences: &[SelectedOccurrence],
    relationship: &SharedPitchRelationship,
) -> Value {
    let evidence = occurrences
        .iter()
        .filter(|item| {
            item.occurrence_id == relationship.from || item.occurrence_id == relationship.to
        })
        .map(|item| json!({"adapter": DISCLOSURE_ADAPTER, "id": item.material.wire_key()}))
        .collect::<Vec<_>>();
    json!({
        "id": relationship.id, "from_occurrence": relationship.from,
        "to_occurrence": relationship.to, "kind": SHARED_PITCH_CLASSES,
        "label": "Shared pitch classes", "explanation": relationship.explanation,
        "provenance": {"source": source(owner), "source_revision": revision,
            "method": relationship.method, "method_version": 1, "provider": "woodshed", "evidence": evidence},
    })
}

pub(super) fn comparison(
    owner: WorkingSetId,
    left: &SelectedOccurrence,
    right: &SelectedOccurrence,
    comparison: &PitchSetComparison,
    overlap: Value,
) -> Vec<Value> {
    #[cfg(test)]
    super::construction_stats::pair();
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
    let mut provenance = overlap["provenance"].clone();
    provenance["method"] = json!(FULL_COMPARISON_METHOD);
    let relation = |kind: &str, label: &str, from: &str, to: &str, explanation: String| {
        json!({
            "id": format!("set:{}:{}:{}:{}", owner.0, kind, left.card.0, right.card.0),
            "from_occurrence": from, "to_occurrence": to, "kind": kind,
            "label": label,
            "explanation": format!("{explanation} Exact catalog pitch classes; register, fingering, voice leading, key and harmonic function are not inferred."),
            "provenance": provenance,
        })
    };
    let mut relationships = if comparison.shared.is_empty() {
        Vec::new()
    } else {
        vec![overlap]
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
    relationships
}
