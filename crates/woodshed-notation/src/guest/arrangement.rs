// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! The arrangement backdrop scene: the Set's bars, groups, Cards and rests as a
//! `sceno-backdrops` `SceneInput`, ready for a host's backdrop surface.
use sceno::SourceRef;
use sceno_backdrops::{
    BACKDROP_SCENE_VERSION, SceneFacts, SceneInput,
    template::{
        Placement, SequenceTemplate, Span, TemplateGroup, TemplateOccurrence, TemplatedSequence,
    },
};
use std::collections::BTreeMap;
use woodshedding::{blocks, rehearsal::arrangement::bars};

fn span(s: woodshedding::rehearsal::arrangement::Span) -> Span {
    Span {
        start: s.start,
        length: s.length,
    }
}

/// The scene of the block's saved arrangement; `address` names the block in its document.
pub(super) fn scene(
    address: &str,
    revision: &str,
    attrs: &BTreeMap<String, String>,
) -> Result<SceneInput, String> {
    let set = blocks::set(attrs)?;
    let arr = blocks::arrangement(&set)?;
    let bars = bars::bars(arr)?;
    let mut groups = Vec::new();
    for (i, g) in arr.bar_groups.iter().enumerate() {
        let first = &bars[g.first as usize];
        let last = &bars[(g.first + g.count - 1) as usize];
        groups.push(TemplateGroup {
            key: format!("group-{i}"),
            lane: 0,
            span: Span {
                start: first.start,
                length: last.start + last.length - first.start,
            },
            parent: None,
            label: format!("Bars {}–{}", g.first + 1, g.first + g.count),
        });
    }
    for (i, b) in bars.iter().enumerate() {
        let parent = arr
            .bar_groups
            .iter()
            .position(|g| g.first as usize <= i && i < ((g.first + g.count) as usize))
            .map(|g| format!("group-{g}"));
        groups.push(TemplateGroup {
            key: format!("bar-{i}"),
            lane: 0,
            span: Span {
                start: b.start,
                length: b.length,
            },
            parent,
            label: format!(
                "Bar {}{}",
                i + 1,
                if b.complete { "" } else { " (partial)" }
            ),
        });
    }
    let mut occurrences = Vec::new();
    for card in &set.cards {
        let binding = arr.cards.iter().find(|c| c.card == card.id);
        occurrences.push(TemplateOccurrence {
            key: format!("card-{}", card.id.0),
            source: SourceRef::new("woodshed.card", format!("{address}#card={}", card.id.0)),
            label: card.label.clone(),
            detail: "Woodshed Card · supplied timing and voicing retained in source".into(),
            placement: binding
                .and_then(|c| c.placement.as_ref())
                .map(|p| Placement {
                    lane: p.lane,
                    span: span(p.span),
                }),
            slot: None,
        });
    }
    for rest in &arr.rests {
        occurrences.push(TemplateOccurrence {
            key: format!("rest-{}", rest.key),
            source: SourceRef::new("woodshed.rest", format!("{address}#rest={}", rest.key)),
            label: "Rest".into(),
            detail: "Explicit authored rest".into(),
            placement: Some(Placement {
                lane: rest.placement.lane,
                span: span(rest.placement.span),
            }),
            slot: None,
        });
    }
    let input = SceneInput {
        version: BACKDROP_SCENE_VERSION,
        source: SourceRef::new("woodshed.arrangement", address),
        revision: revision.into(),
        title: arr.name.clone(),
        facts: SceneFacts::TemplatedSequence(TemplatedSequence {
            template: SequenceTemplate {
                lanes: vec!["Woodshed arrangement".into()],
                extent: arr.extent,
                ticks_per_unit: 4,
                unit_label: "Quarter-note beats · source time-signature map".into(),
                groups,
            },
            slots: vec![],
            occurrences,
            relations: vec![],
        }),
    };
    input.validate().map_err(|e| e.to_string())?;
    Ok(input)
}
