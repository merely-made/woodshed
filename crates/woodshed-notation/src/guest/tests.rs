// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! The guest's behaviour through its WIT-shaped entry points, on the native rlib.
use super::exports::mere::script::block_adapter::{
    Attr, Block, CommandKind, Field, Guest, RunResult, ViewAsk,
};
use super::{Component, list};
use sceno::{Footprint, Scene};
use sceno_backdrops::{SceneFacts, SceneInput};
use std::collections::BTreeMap;
use vector_sheet::{FillRule, Op, Role, Seg, VectorSheet};
use woodshedding::blocks::{self, EXAMPLE_SOURCE, SOURCE};

fn example_attrs() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("class".into(), "progression".into()),
        (SOURCE.into(), EXAMPLE_SOURCE.into()),
        ("custom".into(), "keep me".into()),
    ])
}
fn block(attrs: BTreeMap<String, String>) -> Block {
    Block {
        class: "progression".into(),
        body: "Woodshed ii–V–I · structured arrangement\n".into(),
        attrs: list(attrs),
        revision: "rev-1".into(),
        selected: vec![],
        pattern: "2 + 2".into(),
    }
}
/// The example after an explicit "Write notation", as a host would hold it.
fn written() -> Block {
    let RunResult::Attrs(attrs) =
        Component::run(block(example_attrs()), "write-notation".into(), "96".into()).unwrap()
    else {
        panic!("write-notation returns the attribute map")
    };
    let mut b = block(BTreeMap::new());
    b.attrs = attrs;
    b
}
fn ask(mode: &str, selected: Option<(u32, u32)>) -> ViewAsk {
    ViewAsk {
        mode: mode.into(),
        width: 640.0,
        selected,
    }
}
fn decode_scene(bytes: &[u8]) -> Scene {
    assert_eq!(bytes[0], 1, "version byte");
    ciborium::from_reader(&bytes[1..]).unwrap()
}
fn events_in(b: &Block) -> usize {
    let attrs: BTreeMap<_, _> = b
        .attrs
        .iter()
        .map(|a| (a.key.clone(), a.value.clone()))
        .collect();
    let score = blocks::music(&attrs).unwrap().unwrap();
    score.measures.iter().map(|m| m.events.len()).sum()
}

fn title(b: &Block) -> String {
    let attrs = super::map(&b.attrs);
    blocks::music(&attrs).unwrap().unwrap().title
}

#[test]
fn describe_claims_progression_blocks() {
    let claim = Component::describe();
    assert_eq!(claim.class, "progression");
    assert_eq!(claim.source_key, "woodshed");
    assert_eq!(claim.source_versions, [1]);
    let commands: Vec<_> = claim
        .commands
        .iter()
        .map(|c| (c.id.as_str(), c.label.as_str(), c.kind))
        .collect();
    assert_eq!(
        commands,
        [
            ("group-bars", "Apply bar grouping…", CommandKind::Choice),
            ("write-notation", "Write notation", CommandKind::Write),
            (
                "export-plaintext",
                "Export bar-line plaintext",
                CommandKind::Text
            ),
            ("backdrop-scene", "Swatch scene", CommandKind::Text),
        ]
    );
    let views: Vec<_> = claim
        .views
        .iter()
        .map(|v| (v.id.as_str(), v.label.as_str()))
        .collect();
    assert_eq!(
        views,
        [("chart", "Chart"), ("tab", "Tab"), ("score", "Score")]
    );
    let field = |f: &Option<Field>| f.as_ref().map(|f| (f.label.clone(), f.default.clone()));
    assert_eq!(
        field(&claim.pattern),
        Some(("Bar grouping".into(), "2 + 2".into()))
    );
    let args: Vec<_> = claim.commands.iter().map(|c| field(&c.arg)).collect();
    assert_eq!(
        args,
        [
            None,
            Some(("Notation tempo (BPM)".into(), "96".into())),
            None,
            None
        ]
    );
    let example = claim.example.unwrap();
    assert_eq!(example.body, "Woodshed ii–V–I · structured arrangement\n");
    let attrs = super::map(&example.attrs);
    assert_eq!(attrs["class"], "progression");
    // The example is a usable block: its bars read, and it writes notation.
    assert_eq!(blocks::rows(&attrs).unwrap().len(), 4);
    assert!(blocks::write_music(&attrs, 96).is_ok());
}

#[test]
fn views_return_decodable_scene_and_sheet_for_each_reading() {
    let b = written();
    for mode in ["chart", "tab", "score"] {
        let result = Component::view(b.clone(), ask(mode, None)).unwrap();
        let sheet = VectorSheet::decode(&result.sheet).unwrap();
        assert_eq!((sheet.width, sheet.height), (result.width, result.height));
        assert!(!sheet.ops.is_empty());
        let scene = decode_scene(&result.scene);
        assert_eq!(scene.items.len(), events_in(&b));
        let label = ["Chart", "Tab", "Score"][["chart", "tab", "score"]
            .iter()
            .position(|m| *m == mode)
            .unwrap()];
        assert_eq!(
            result.description,
            format!("{label} notation · {}", title(&b))
        );
    }
    assert!(Component::view(b, ask("wheel", None)).is_err());
}

#[test]
fn score_sheet_keeps_counters_as_subpaths_of_one_fill() {
    let result = Component::view(written(), ask("score", None)).unwrap();
    let sheet = VectorSheet::decode(&result.sheet).unwrap();
    let most = sheet
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::Fill { path, rule, .. } => {
                assert_eq!(*rule, FillRule::NonZero);
                Some(
                    path.segs
                        .iter()
                        .filter(|s| matches!(s, Seg::Move(..)))
                        .count(),
                )
            },
            _ => None,
        })
        .max()
        .unwrap();
    assert!(
        most > 1,
        "a compound glyph keeps its holes; most subpaths in one fill: {most}"
    );
    assert!(sheet.ops.iter().any(|op| matches!(op, Op::Text { .. })));
}

#[test]
fn event_regions_match_authored_events_and_lie_inside_the_view() {
    let b = written();
    let total = events_in(&b);
    for mode in ["chart", "tab", "score"] {
        let result = Component::view(b.clone(), ask(mode, None)).unwrap();
        let scene = decode_scene(&result.scene);
        assert_eq!(scene.regions.len(), total);
        assert_eq!(scene.items.len(), total);
        for (region, item) in scene.regions.iter().zip(&scene.items) {
            assert_eq!(region.members.len(), 1);
            let Footprint::Rect { size } = item.footprint else {
                panic!("an event's footprint is its hit rectangle")
            };
            let (cx, cy) = (item.transform.translate.x, item.transform.translate.y);
            assert!(cx - size.w / 2.0 >= 0.0 && cx + size.w / 2.0 <= result.width);
            assert!(cy - size.h / 2.0 >= 0.0 && cy + size.h / 2.0 <= result.height);
            assert!(
                region
                    .label
                    .as_deref()
                    .is_some_and(|l| l.starts_with("Bar "))
            );
        }
        let first = scene.regions[0].label.as_deref().unwrap();
        assert!(first.starts_with("Bar 1, event 1 · "), "{first}");
    }
    // A rest reads as one, a sounding event as its strings and frets.
    let scene = decode_scene(&Component::view(b, ask("chart", None)).unwrap().scene);
    let labels: Vec<_> = scene
        .regions
        .iter()
        .filter_map(|r| r.label.as_deref())
        .collect();
    assert!(labels.iter().any(|l| l.ends_with("Rest")));
    assert!(
        labels
            .iter()
            .any(|l| l.contains("string ") && l.contains(" fret "))
    );
}

#[test]
fn the_selected_event_paints_in_the_accent_role() {
    let b = written();
    let accents = |selected| {
        let r = Component::view(b.clone(), ask("tab", selected)).unwrap();
        VectorSheet::decode(&r.sheet)
            .unwrap()
            .ops
            .iter()
            .filter(|op| matches!(op, Op::Fill { paint, .. } if paint.role == Role::Accent))
            .count()
    };
    assert_eq!(accents(None), 0);
    assert!(accents(Some((0, 0))) > 0);
}

#[test]
fn a_block_without_written_notation_has_no_view() {
    let err = Component::view(block(example_attrs()), ask("chart", None)).unwrap_err();
    assert!(err.contains("no written notation"), "{err}");
}

#[test]
fn run_writes_notation_exports_plaintext_and_projects_the_arrangement() {
    let b = written();
    // Notation is written once: a second write refuses and names why.
    let err = Component::run(b, "write-notation".into(), "96".into())
        .err()
        .unwrap();
    assert!(err.contains("already has MusicXML"), "{err}");
    for bad in ["", "abc", "0", "401"] {
        assert!(
            Component::run(block(example_attrs()), "write-notation".into(), bad.into()).is_err()
        );
    }
    let RunResult::Text(text) = Component::run(
        block(example_attrs()),
        "export-plaintext".into(),
        String::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert!(text.contains("4/4 |") && text.contains(" r |"));
    let RunResult::Text(json) = Component::run(
        block(example_attrs()),
        "backdrop-scene".into(),
        "document#passage".into(),
    )
    .unwrap() else {
        panic!()
    };
    let input: SceneInput = serde_json::from_str(&json).unwrap();
    input.validate().unwrap();
    let SceneFacts::TemplatedSequence(sequence) = input.facts else {
        panic!()
    };
    assert_eq!(sequence.occurrences.len(), 5);
    assert_eq!(sequence.template.groups.len(), 4);
    assert!(Component::run(block(example_attrs()), "nonsense".into(), String::new()).is_err());
}

#[test]
fn choices_and_apply_group_bars_over_attribute_maps() {
    let b = block(example_attrs());
    let changes = Component::choices(b.clone(), "group-bars".into()).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(!changes[0].conflict);
    let after = Component::apply(b.clone(), "group-bars".into(), changes).unwrap();
    let after: BTreeMap<_, _> = after
        .into_iter()
        .map(|Attr { key, value }| (key, value))
        .collect();
    let before = example_attrs();
    let differing: Vec<_> = after
        .iter()
        .filter(|(k, v)| before[*k] != **v)
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(differing, [SOURCE]);
    assert!(Component::choices(b.clone(), "write-notation".into()).is_err());
    let mut bad = b;
    bad.pattern = "x".into();
    assert!(Component::choices(bad, "group-bars".into()).is_err());
}

#[test]
fn a_block_with_no_woodshed_source_is_refused_everywhere() {
    let plain = block(BTreeMap::from([("class".into(), "progression".into())]));
    let refused = |e: String| assert!(e.contains("no structured Woodshed source"), "{e}");
    refused(Component::choices(plain.clone(), "group-bars".into()).unwrap_err());
    refused(Component::apply(plain.clone(), "group-bars".into(), vec![]).unwrap_err());
    for command in ["write-notation", "export-plaintext", "backdrop-scene"] {
        refused(
            Component::run(plain.clone(), command.into(), "96".into())
                .err()
                .unwrap(),
        );
    }
    refused(Component::view(plain, ask("score", None)).unwrap_err());
}

#[test]
fn rows_list_the_bars_with_the_hosts_labels() {
    let rows = Component::rows(block(example_attrs())).unwrap();
    let got: Vec<_> = rows.iter().map(|r| (r.index, r.label.as_str())).collect();
    assert_eq!(
        got,
        [
            (0, "Bar 1 · 4 quarter-note beats"),
            (1, "Bar 2 · 4 quarter-note beats"),
            (2, "Bar 3 · 4 quarter-note beats"),
            (3, "Bar 4 · 4 quarter-note beats"),
        ]
    );
    let plain = block(BTreeMap::from([("class".into(), "progression".into())]));
    assert!(
        Component::rows(plain)
            .unwrap_err()
            .contains("no structured Woodshed source")
    );
}
