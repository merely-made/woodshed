// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! The adapter's tests, over its own types: attribute maps in, attribute maps out.
use super::*;
use crate::rehearsal::arrangement::TimeSignatureChange;

fn attrs() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("class".into(), "progression".into()),
        (
            SOURCE.into(),
            serde_json::from_str::<Value>(EXAMPLE_SOURCE)
                .unwrap()
                .to_string(),
        ),
        ("custom".into(), "keep me".into()),
    ])
}
#[test]
fn write_notation_is_explicit_atomic_and_preserves_source_fields() {
    let mut a = attrs();
    let mut raw: Value = serde_json::from_str(&a[SOURCE]).unwrap();
    raw["set"]["future"] = serde_json::json!({"retained": true});
    a.insert(SOURCE.into(), raw.to_string());
    let before = a.clone();
    assert!(music(&a).unwrap().is_none());
    let written = write_music(&a, 108).unwrap();
    // The input map is untouched: the write is a new map, never an edit in place.
    assert_eq!(a, before);
    let score = music(&written).unwrap().unwrap();
    assert_eq!(score.tempo, 108);
    assert_eq!(score.measures.len(), 4);
    assert!(score.measures[1].events[1].rest);
    assert_eq!(
        score.measures[1].events[0].value,
        crate::notation::NoteValue::Half
    );
    // Only the `woodshed` attribute changed, and within it only `set.music` was added.
    let changed: Vec<_> = written
        .iter()
        .filter(|(k, v)| a.get(*k) != Some(*v))
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(changed, [SOURCE]);
    assert_eq!(written.len(), a.len());
    let mut next: Value = serde_json::from_str(&written[SOURCE]).unwrap();
    next["set"].as_object_mut().unwrap().remove("music");
    assert_eq!(next, raw);
    assert_eq!(a["custom"], written["custom"]);
    // A second write refuses and the saved notation stays as it was.
    assert!(write_music(&written, 96).is_err());
    assert_eq!(music(&written).unwrap(), Some(score));
}

#[test]
fn write_notation_accepts_full_written_tempo_range() {
    let a = attrs();
    let before = a.clone();
    for bpm in [1, 19, 20, 400] {
        let written = write_music(&a, bpm).unwrap();
        assert_eq!(music(&written).unwrap().unwrap().tempo, bpm);
    }
    for bpm in [0, 401, u16::MAX] {
        assert!(
            write_music(&a, bpm)
                .unwrap_err()
                .contains("whole number from 1 to 400 BPM")
        );
    }
    assert_eq!(a, before);
}

#[test]
fn unsupported_material_or_corrupt_music_refuses_without_replacing_source() {
    let a = attrs();
    let mut raw: Value = serde_json::from_str(&a[SOURCE]).unwrap();
    raw["set"]["cards"][0]["setting"]["voicing_fingerprint"] = Value::String("changed".into());
    let mut bad = a.clone();
    bad.insert(SOURCE.into(), raw.to_string());
    let before = bad.clone();
    assert!(write_music(&bad, 96).unwrap_err().contains("fret pattern"));
    assert_eq!(bad, before);
    raw["set"]["music"] = serde_json::json!({"musicxml":"<broken>"});
    bad.insert(SOURCE.into(), raw.to_string());
    assert!(music(&bad).is_err());
    assert!(write_music(&bad, 96).is_err());
    assert!(write_music(&a, 0).is_err());
}

#[test]
fn saved_music_reading_does_not_require_legacy_card_or_arrangement_metadata() {
    use crate::notation::{MusicDocument, Score};
    let score = Score::rhythm_example();
    let raw = serde_json::json!({
        "version": 1,
        "set": {
            "music": MusicDocument::from_score(&score).unwrap(),
            "cards": "unrecognized historical Card metadata",
            "progression_arrangement": {"version": 999},
            "future": {"keep": true}
        }
    });
    let attrs = BTreeMap::from([(SOURCE.into(), raw.to_string())]);
    let before = attrs.clone();
    assert_eq!(music(&attrs).unwrap(), Some(score));
    assert!(rows(&attrs).is_err());
    assert!(write_music(&attrs, 96).is_err());
    assert_eq!(attrs, before);
    // A block with no music at all reads as none, not as an error.
    let mut bare: Value = raw.clone();
    bare["set"].as_object_mut().unwrap().remove("music");
    let bare = BTreeMap::from([(SOURCE.into(), bare.to_string())]);
    assert_eq!(music(&bare).unwrap(), None);
}

#[test]
fn meter_patch_keeps_other_source_fields_and_refuses_invalid_boundaries() {
    let a = attrs();
    assert_eq!(
        meter_attributes(&a, &time_signatures(&a).unwrap(), &[]).unwrap(),
        a
    );
    let map = [
        TimeSignatureChange {
            start: 0,
            numerator: 4,
            denominator: 4,
        },
        TimeSignatureChange {
            start: 32,
            numerator: 3,
            denominator: 4,
        },
    ];
    let next = meter_attributes(&a, &map, &[]).unwrap();
    let original: Value = serde_json::from_str(&a[SOURCE]).unwrap();
    let mut raw: Value = serde_json::from_str(&next[SOURCE]).unwrap();
    raw["set"]["progression_arrangement"]["alignment"]
        .as_object_mut()
        .unwrap()
        .remove("time_signatures");
    raw["set"]["progression_arrangement"]["alignment"]
        .as_object_mut()
        .unwrap()
        .remove("bar_groups");
    let mut expected = original.clone();
    expected["set"]["progression_arrangement"]["alignment"]
        .as_object_mut()
        .unwrap()
        .remove("time_signatures");
    assert_eq!(raw, expected);
    assert_eq!(next["custom"], a["custom"]);
    let mut bad = map;
    bad[1].start = 24;
    assert!(meter_attributes(&a, &bad, &[]).is_err());
}

#[test]
fn grouping_edits_only_group_metadata_and_changes_only_the_source_attribute() {
    let mut attrs = attrs();
    let mut raw: Value = serde_json::from_str(&attrs[SOURCE]).unwrap();
    raw["set"]["future_field"] = serde_json::json!({"keep":"unknown content"});
    attrs.insert(SOURCE.into(), raw.to_string());
    let before = attrs.clone();
    let selected = [0, 1, 2, 3];
    let changes = choices(&attrs, &selected, "2 + 2").unwrap();
    assert!(!changes[0].conflict);
    let updated = attributes(&attrs, &selected, &changes).unwrap();
    assert_eq!(attrs, before);
    // Compare the whole attribute map: exactly one key differs, and it is `woodshed`.
    assert_eq!(
        updated.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>()
    );
    let differing: Vec<_> = updated
        .iter()
        .filter(|(k, v)| before[*k] != **v)
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(differing, [SOURCE]);
    assert_eq!(updated["custom"], "keep me");
    assert_eq!(updated["class"], "progression");
    // Inside that source only `bar_groups` differs.
    let mut new: Value = serde_json::from_str(&updated[SOURCE]).unwrap();
    let groups = new
        .pointer_mut("/set/progression_arrangement/alignment")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("bar_groups")
        .unwrap();
    assert_eq!(
        groups,
        serde_json::json!([{"first":0,"count":2},{"first":2,"count":2}])
    );
    assert_eq!(new, raw);
    // The same decision applied again is a no-op that keeps the exact bytes.
    let again = choices(&updated, &selected, "2 + 2").unwrap();
    assert_eq!(attributes(&updated, &selected, &again).unwrap(), updated);
}

#[test]
fn conflicting_groups_keep_local_until_accepted_and_noop_keeps_exact_bytes() {
    let a = attrs();
    let selected = [0, 1, 2, 3];
    let a = attributes(&a, &selected, &choices(&a, &selected, "2 2").unwrap()).unwrap();
    let mut changes = choices(&a, &selected, "1 3").unwrap();
    assert!(changes[0].conflict);
    assert!(!changes[0].use_update);
    assert_eq!(attributes(&a, &selected, &changes).unwrap(), a);
    changes[0].use_update = true;
    let b = attributes(&a, &selected, &changes).unwrap();
    assert_ne!(b, a);
    assert!(choices(&b, &[0, 1], "2").is_err());
}
#[test]
fn plaintext_and_rows_use_actual_set_with_explicit_rest_and_meter() {
    let a = attrs();
    assert_eq!(rows(&a).unwrap().len(), 4);
    let text = plaintext(&a).unwrap();
    assert!(text.contains("4/4 |"));
    assert!(text.contains(" r |"));
    assert_eq!(text.matches('|').count(), 5);
    let mut raw: Value = serde_json::from_str(&a[SOURCE]).unwrap();
    raw["set"]["progression_arrangement"]["alignment"]["time_signatures"] = serde_json::json!([]);
    let mut missing = a;
    missing.insert(SOURCE.into(), raw.to_string());
    assert!(rows(&missing).unwrap().is_empty());
    assert!(plaintext(&missing).is_err());
    assert!(choices(&missing, &[], "2 2").is_err());
}
