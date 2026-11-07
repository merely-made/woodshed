// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! What the guest claims at mount.
use super::exports::mere::script::block_adapter::{
    Attr, Claim, Command, CommandKind, Example, Field, ViewMode,
};
use woodshedding::blocks;

pub(super) const GROUP_BARS: &str = "group-bars";
pub(super) const WRITE_NOTATION: &str = "write-notation";
pub(super) const EXPORT_PLAINTEXT: &str = "export-plaintext";
pub(super) const ARRANGEMENT_SCENE: &str = "arrangement-scene";

pub(super) const CLASS: &str = "progression";
const EXAMPLE_BODY: &str = "Woodshed ii–V–I · structured arrangement\n";

fn command(id: &str, label: &str, kind: CommandKind, arg: Option<Field>) -> Command {
    Command {
        id: id.into(),
        label: label.into(),
        kind,
        arg,
    }
}
fn field(label: &str, default: &str) -> Field {
    Field {
        label: label.into(),
        default: default.into(),
    }
}

pub(super) fn claim() -> Claim {
    // The source is re-serialised compactly, as a host stores it in the attribute.
    let source = serde_json::from_str::<serde_json::Value>(blocks::EXAMPLE_SOURCE)
        .map(|v| v.to_string())
        .unwrap_or_default();
    Claim {
        class: CLASS.into(),
        source_key: blocks::SOURCE.into(),
        source_versions: vec![1],
        commands: vec![
            command(GROUP_BARS, "Apply bar grouping…", CommandKind::Choice, None),
            command(
                WRITE_NOTATION,
                "Write notation",
                CommandKind::Write,
                Some(field("Notation tempo (BPM)", "96")),
            ),
            command(
                EXPORT_PLAINTEXT,
                "Export bar-line plaintext",
                CommandKind::Text,
                None,
            ),
            command(
                ARRANGEMENT_SCENE,
                "Arrangement scene",
                CommandKind::Text,
                None,
            ),
        ],
        views: ["chart", "tab", "score"]
            .into_iter()
            .zip(["Chart", "Tab", "Score"])
            .map(|(id, label)| ViewMode {
                id: id.into(),
                label: label.into(),
            })
            .collect(),
        pattern: Some(field("Bar grouping", "2 + 2")),
        example: Some(Example {
            title: EXAMPLE_BODY.trim_end().into(),
            body: EXAMPLE_BODY.into(),
            attrs: vec![
                Attr {
                    key: "class".into(),
                    value: CLASS.into(),
                },
                Attr {
                    key: blocks::SOURCE.into(),
                    value: source,
                },
            ],
        }),
    }
}
