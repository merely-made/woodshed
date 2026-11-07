// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! A readable, intentionally lossy bar-line export.
use super::{arrangement, source};
use crate::rehearsal::{Material, arrangement::bars};
use std::collections::BTreeMap;

/// A readable, intentionally lossy chart. It never becomes the edit source:
/// exact durations, voicings, identities and settings remain in the Set JSON.
pub fn plaintext(attrs: &BTreeMap<String, String>) -> Result<String, String> {
    let (_, set) = source(attrs)?;
    let arr = arrangement(&set)?;
    let bars = bars::bars(arr)?;
    if bars.is_empty() {
        return Err("Author a time-signature map before exporting bar lines".into());
    }
    let mut events = Vec::new();
    for card in &set.cards {
        let p = arr
            .cards
            .iter()
            .find(|c| c.card == card.id)
            .and_then(|c| c.placement.as_ref())
            .ok_or("Plaintext export needs alignment for every Card")?;
        if !matches!(card.material, Material::Chord { .. }) {
            return Err("Plaintext chart currently accepts chord Cards and explicit rests".into());
        }
        events.push((
            p.span.start,
            p.span.start + p.span.length,
            card.label.clone(),
        ));
    }
    for r in &arr.rests {
        events.push((
            r.placement.span.start,
            r.placement.span.start + r.placement.span.length,
            "r".into(),
        ));
    }
    events.sort_by_key(|e| e.0);
    if events.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err("Overlapping events need a richer notation than this plaintext chart".into());
    }
    let clean = |s: &str| s.replace(['|', '\n', '\r'], " ");
    let mut result = String::from(
        "# Readable export; structured Woodshed source retains exact timing and voicings.\n# r = authored rest; ? = unspecified time; ~ = continuation across a bar line.\n",
    );
    for (i, b) in bars.iter().enumerate() {
        if let Some(m) = arr.time_signatures.iter().find(|m| m.start == b.start) {
            result.push_str(&format!("\n{}/{} ", m.numerator, m.denominator));
        } else if i == 0 {
            result.push('\n');
        }
        result.push_str("| ");
        let mut cursor = b.start;
        for e in events
            .iter()
            .filter(|e| e.0 < b.start + b.length && e.1 > b.start)
        {
            if e.0 > cursor {
                result.push_str("? ");
            }
            if e.0 < b.start {
                result.push('~');
            }
            result.push_str(&clean(&e.2));
            result.push(' ');
            cursor = e.1.min(b.start + b.length);
        }
        if cursor < b.start + b.length {
            result.push_str("? ");
        }
        if !b.complete {
            result.push_str("[partial] ");
        }
    }
    result.push_str("|\n");
    Ok(result)
}
