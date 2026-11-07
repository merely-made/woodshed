// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! Bar grouping proposals and the attribute patch that applies them.
use super::{BlockChange, SOURCE, arrangement, source};
use crate::rehearsal::arrangement::{BarGroup, ProgressionArrangement, bars};
use serde_json::Value;
use std::collections::BTreeMap;

fn counts(pattern: &str) -> Result<Vec<u32>, String> {
    if pattern.len() > 1024 {
        return Err("Bar grouping is too long".into());
    }
    let counts: Vec<u32> = pattern
        .split(|c: char| c.is_whitespace() || c == '+')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| "Use bar counts such as 2 + 2".to_string())
        })
        .collect::<Result<_, _>>()?;
    if counts.is_empty() || counts.contains(&0) {
        return Err("Use positive bar counts such as 2 + 2".into());
    }
    Ok(counts)
}
fn grouping(groups: &[BarGroup]) -> String {
    groups
        .iter()
        .map(|g| g.count.to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}
fn old_selected(arr: &ProgressionArrangement, selected: &[usize]) -> Vec<BarGroup> {
    arr.bar_groups
        .iter()
        .filter(|g| selected.contains(&(g.first as usize)))
        .cloned()
        .collect()
}
pub fn rows(attrs: &BTreeMap<String, String>) -> Result<Vec<String>, String> {
    let (_, set) = source(attrs)?;
    let arr = arrangement(&set)?;
    Ok(bars::bars(arr)?
        .iter()
        .enumerate()
        .map(|(i, b)| {
            format!(
                "Bar {} · {} quarter-note beats{}",
                i + 1,
                f64::from(b.length) / 4.0,
                if b.complete { "" } else { " · partial" }
            )
        })
        .collect())
}
pub fn choices(
    attrs: &BTreeMap<String, String>,
    selected: &[usize],
    pattern: &str,
) -> Result<Vec<BlockChange>, String> {
    let (_, set) = source(attrs)?;
    let arr = arrangement(&set)?;
    let next = bars::regroup(arr, selected, &counts(pattern)?)?;
    let old = old_selected(arr, selected);
    let after: Vec<_> = next
        .iter()
        .filter(|g| selected.contains(&(g.first as usize)))
        .cloned()
        .collect();
    let before = (!old.is_empty()).then(|| grouping(&old));
    let after = grouping(&after);
    let conflict = before.as_ref().is_some_and(|b| b != &after);
    Ok(vec![BlockChange {
        index: selected[0],
        label: format!(
            "Bars {}–{} · grouping",
            selected[0] + 1,
            selected[selected.len() - 1] + 1
        ),
        before,
        after,
        conflict,
        use_update: !conflict,
    }])
}
pub fn attributes(
    attrs: &BTreeMap<String, String>,
    selected: &[usize],
    changes: &[BlockChange],
) -> Result<BTreeMap<String, String>, String> {
    let (mut raw, set) = source(attrs)?;
    let arr = arrangement(&set)?;
    let change = changes
        .first()
        .filter(|_| changes.len() == 1)
        .ok_or("Preview one bar grouping at a time")?;
    let expected = choices(attrs, selected, &change.after)?;
    if change.index != expected[0].index || change.before != expected[0].before {
        return Err("Bar grouping changed; review a fresh preview".into());
    }
    if !change.use_update {
        return Ok(attrs.clone());
    }
    let groups = bars::regroup(arr, selected, &counts(&change.after)?)?;
    if groups == arr.bar_groups {
        return Ok(attrs.clone());
    }
    let alignment = raw
        .pointer_mut("/set/progression_arrangement/alignment")
        .and_then(Value::as_object_mut)
        .ok_or("Missing source arrangement")?;
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
