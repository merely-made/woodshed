// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! Block-adapter logic for a Djot `progression` block, as plain Rust over
//! `woodshedding` types. Feature `blocks`.
//!
//! A host sends a block's attribute map; these functions read the `woodshed`
//! source attribute (JSON) and return proposals or a complete new attribute map.
//! Nothing here edits a document, and nothing depends on a host: musical
//! coordinates and bar validation stay owned by this crate.
mod grouping;
mod plaintext;
#[cfg(test)]
mod tests;

pub use grouping::{attributes, choices, rows};
pub use plaintext::plaintext;

use crate::rehearsal::{
    Set,
    arrangement::{BarGroup, ProgressionArrangement, TimeSignatureChange, bars},
};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

/// The attribute that carries the structured source.
pub const SOURCE: &str = "woodshed";
/// The example progression source (a ii–V–I with an authored rest and meter), as JSON.
pub const EXAMPLE_SOURCE: &str = include_str!("example.json");
/// The most source bytes one block may carry (one MiB).
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// One proposed edit, in the value shape a host reviews before applying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockChange {
    pub index: usize,
    pub label: String,
    pub before: Option<String>,
    pub after: String,
    pub conflict: bool,
    pub use_update: bool,
}

/// Read the carrier without requiring metadata used by other commands.
fn raw_source(attrs: &BTreeMap<String, String>) -> Result<Value, String> {
    let json = attrs
        .get(SOURCE)
        .ok_or("This progression has no structured Woodshed source")?;
    if json.len() > MAX_SOURCE_BYTES {
        return Err("Woodshed source exceeds one MiB".into());
    }
    let raw: Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid Woodshed source: {e}"))?;
    if raw.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("Unsupported Woodshed source version; source preserved".into());
    }
    if !raw.get("set").is_some_and(Value::is_object) {
        return Err("Woodshed source needs its owning Set".into());
    }
    Ok(raw)
}

/// Keep the original JSON tree alongside the typed source. Applying a group
/// patches one owner field, retaining source fields this host does not know.
pub(super) fn source(attrs: &BTreeMap<String, String>) -> Result<(Value, Set), String> {
    let raw = raw_source(attrs)?;
    let set: Set = serde_json::from_value(
        raw.get("set")
            .cloned()
            .ok_or("Woodshed source needs its owning Set")?,
    )
    .map_err(|e| format!("Invalid Woodshed Set: {e}"))?;
    let arr = arrangement(&set)?;
    if arr.cards.len() > 512
        || arr.rests.len() > 512
        || set.cards.len() > 512
        || arr.role_spans.len() > 512
    {
        return Err("Arrangement exceeds preview limits".into());
    }
    bars::validate_groups(arr)?;
    let mut ids = HashSet::new();
    for card in &set.cards {
        if !card.id.is_assigned() || !ids.insert(card.id) {
            return Err("Cards need unique persistent identities".into());
        }
    }
    let mut bound = HashSet::new();
    for card in &arr.cards {
        if !ids.contains(&card.card) || !bound.insert(card.card) {
            return Err("Alignment must name an existing Card once".into());
        }
    }
    let mut rests = HashSet::new();
    if arr
        .rests
        .iter()
        .any(|r| r.key.trim().is_empty() || !rests.insert(&r.key))
    {
        return Err("Rests need unique source identities".into());
    }
    Ok((raw, set))
}
/// The validated owning Set of a block's source, for callers that build their
/// own projections (a scene, a view) over it.
pub fn set(attrs: &BTreeMap<String, String>) -> Result<Set, String> {
    source(attrs).map(|(_, set)| set)
}

/// Read the saved musical authority independently of Card/arrangement metadata.
/// Readings never derive or author notation.
pub fn music(attrs: &BTreeMap<String, String>) -> Result<Option<crate::notation::Score>, String> {
    let raw = raw_source(attrs)?;
    raw.pointer("/set/music")
        .filter(|value| !value.is_null())
        .map(|value| {
            serde_json::from_value::<crate::notation::MusicDocument>(value.clone())
                .map_err(|e| format!("Invalid saved MusicXML document: {e}"))?
                .score()
        })
        .transpose()
}

/// Explicitly write notation once, preserving unknown source fields. All
/// conversion and XML validation completes before a new declaration is returned.
pub fn write_music(
    attrs: &BTreeMap<String, String>,
    bpm: u16,
) -> Result<BTreeMap<String, String>, String> {
    let (mut raw, set) = source(attrs)?;
    if set.music.is_some() {
        return Err("This Set already has MusicXML; its saved notation was preserved".into());
    }
    if !(1..=400).contains(&bpm) {
        return Err("Notation tempo must be a whole number from 1 to 400 BPM".into());
    }
    let saved = set
        .progression_arrangement
        .as_ref()
        .ok_or("Save an arrangement first")?;
    if saved.alignment.time_signatures.is_empty() {
        return Err("Author a time-signature map before writing notation".into());
    }
    let context = crate::card_shapes::ShapeContext::from_saved(&set)?;
    let score = crate::notation::draft::score_from_set(
        &context,
        &set,
        &saved.alignment.name,
        f32::from(bpm),
    )?;
    let document = crate::notation::MusicDocument::from_score(&score)?;
    raw.get_mut("set")
        .and_then(Value::as_object_mut)
        .ok_or("Missing owning Set")?
        .insert(
            "music".into(),
            serde_json::to_value(document).map_err(|e| e.to_string())?,
        );
    let mut result = attrs.clone();
    result.insert(
        SOURCE.into(),
        serde_json::to_string(&raw).map_err(|e| e.to_string())?,
    );
    Ok(result)
}

pub fn arrangement(set: &Set) -> Result<&ProgressionArrangement, String> {
    let saved = set
        .progression_arrangement
        .as_ref()
        .ok_or("Set has no saved progression arrangement")?;
    if saved.version != 1 {
        return Err("Unsupported saved arrangement version".into());
    }
    if saved.roles.len() != saved.alignment.role_spans.len() {
        return Err("Role expectations do not match their spans".into());
    }
    let arr = &saved.alignment;
    let valid = |lane: u16, s: crate::rehearsal::arrangement::Span| {
        lane == 0
            && s.length > 0
            && s.start
                .checked_add(s.length)
                .is_some_and(|e| e <= arr.extent)
    };
    if arr.role_spans.iter().any(|s| !valid(0, *s))
        || arr.cards.iter().any(|c| {
            c.role.is_some_and(|r| r >= saved.roles.len())
                || c.placement.as_ref().is_some_and(|p| !valid(p.lane, p.span))
        })
        || arr.rests.iter().any(|r| {
            r.role.is_some_and(|i| i >= saved.roles.len())
                || !valid(r.placement.lane, r.placement.span)
        })
    {
        return Err("Invalid source musical alignment".into());
    }
    Ok(arr)
}
/// Read the owner's authored meter map without deriving a default meter.
pub fn time_signatures(
    attrs: &BTreeMap<String, String>,
) -> Result<Vec<TimeSignatureChange>, String> {
    let (_, set) = source(attrs)?;
    Ok(arrangement(&set)?.time_signatures.clone())
}

/// Patch a validated meter/group result into the retained source tree. The
/// caller supplies the owner's complete proposal; this helper does not choose
/// how meter changes should affect groups or musical material.
pub fn meter_attributes(
    attrs: &BTreeMap<String, String>,
    time_signatures: &[TimeSignatureChange],
    groups: &[BarGroup],
) -> Result<BTreeMap<String, String>, String> {
    let (mut raw, set) = source(attrs)?;
    let original = arrangement(&set)?;
    let mut next = original.clone();
    next.time_signatures = time_signatures.to_vec();
    next.bar_groups = groups.to_vec();
    bars::validate_groups(&next)?;
    if &next == original {
        return Ok(attrs.clone());
    }
    let alignment = raw
        .pointer_mut("/set/progression_arrangement/alignment")
        .and_then(Value::as_object_mut)
        .ok_or("Missing source arrangement")?;
    alignment.insert(
        "time_signatures".into(),
        serde_json::to_value(time_signatures).map_err(|e| e.to_string())?,
    );
    alignment.insert(
        "bar_groups".into(),
        serde_json::to_value(groups).map_err(|e| e.to_string())?,
    );
    let mut result = attrs.clone();
    result.insert(
        SOURCE.into(),
        serde_json::to_string(&raw).map_err(|e| e.to_string())?,
    );
    Ok(result)
}
