// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! The `block-guest` world of `mere:script`: Woodshed as a pure wasm component a host
//! loads to edit and show a Djot `progression` block (feature `block-guest`).
//!
//! The host sends a copy of the block and gets back proposals, the whole desired
//! attribute map, or a view: a `sceno` scene naming one region per authored event and a
//! `vector-sheet` the host paints with its own theme. The adapter logic is
//! `woodshedding::blocks`; the engraving is [`crate::NotationLeaf`]. The guest never
//! edits a document and has no I/O.
mod arrangement;
mod claim;
mod sheet;
mod view;

#[cfg(test)]
mod tests;

// The WIT world is copied to `wit/world.wit` by the build script.
wit_bindgen::generate!({ path: "wit", world: "block-guest" });

use exports::mere::script::block_adapter::{
    Attr, Block, Change, Claim, Guest, Row, RunResult, ViewAsk, ViewResult,
};
use std::collections::BTreeMap;
use woodshedding::blocks::{self, BlockChange};

struct Component;

fn map(attrs: &[Attr]) -> BTreeMap<String, String> {
    attrs
        .iter()
        .map(|a| (a.key.clone(), a.value.clone()))
        .collect()
}
fn list(attrs: BTreeMap<String, String>) -> Vec<Attr> {
    attrs
        .into_iter()
        .map(|(key, value)| Attr { key, value })
        .collect()
}

/// The bars a command applies to: the host's selection, or every bar when none.
fn selection(b: &Block, attrs: &BTreeMap<String, String>) -> Result<Vec<usize>, String> {
    let bars = blocks::rows(attrs)?;
    let selected: Vec<usize> = if b.selected.is_empty() {
        (0..bars.len()).collect()
    } else {
        b.selected.iter().map(|&i| i as usize).collect()
    };
    if selected.is_empty() {
        return Err("Select the bars to group".into());
    }
    Ok(selected)
}

fn require_group_bars(command: &str) -> Result<(), String> {
    if command == claim::GROUP_BARS {
        Ok(())
    } else {
        Err(format!("Unknown choice command {command}"))
    }
}

impl Guest for Component {
    fn describe() -> Claim {
        claim::claim()
    }

    fn rows(b: Block) -> Result<Vec<Row>, String> {
        Ok(blocks::rows(&map(&b.attrs))?
            .into_iter()
            .enumerate()
            .map(|(i, label)| Row {
                index: i as u32,
                label,
            })
            .collect())
    }

    fn choices(b: Block, command: String) -> Result<Vec<Change>, String> {
        require_group_bars(&command)?;
        let attrs = map(&b.attrs);
        let selected = selection(&b, &attrs)?;
        Ok(blocks::choices(&attrs, &selected, &b.pattern)?
            .into_iter()
            .map(|c| Change {
                index: c.index as u32,
                label: c.label,
                before: c.before,
                after: c.after,
                conflict: c.conflict,
                use_update: c.use_update,
            })
            .collect())
    }

    fn apply(b: Block, command: String, changes: Vec<Change>) -> Result<Vec<Attr>, String> {
        require_group_bars(&command)?;
        let attrs = map(&b.attrs);
        let selected = selection(&b, &attrs)?;
        let changes: Vec<BlockChange> = changes
            .into_iter()
            .map(|c| BlockChange {
                index: c.index as usize,
                label: c.label,
                before: c.before,
                after: c.after,
                conflict: c.conflict,
                use_update: c.use_update,
            })
            .collect();
        blocks::attributes(&attrs, &selected, &changes).map(list)
    }

    fn run(b: Block, command: String, arg: String) -> Result<RunResult, String> {
        let attrs = map(&b.attrs);
        match command.as_str() {
            claim::WRITE_NOTATION => {
                let bpm = arg
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| "Enter a whole-number notation tempo from 1 to 400 BPM")?;
                blocks::write_music(&attrs, bpm).map(|a| RunResult::Attrs(list(a)))
            },
            claim::EXPORT_PLAINTEXT => blocks::plaintext(&attrs).map(RunResult::Text),
            claim::ARRANGEMENT_SCENE => {
                let address = if arg.is_empty() {
                    "block"
                } else {
                    arg.as_str()
                };
                let input = arrangement::scene(address, &b.revision, &attrs)?;
                serde_json::to_string(&input)
                    .map(RunResult::Text)
                    .map_err(|e| e.to_string())
            },
            other => Err(format!("Unknown command {other}")),
        }
    }

    fn view(b: Block, ask: ViewAsk) -> Result<ViewResult, String> {
        view::view(&b, &ask)
    }
}

// Exported only from the wasm component: a native cdylib would leave the WIT symbols
// undefined, and native builds use the rlib (and its tests) instead.
#[cfg(target_arch = "wasm32")]
export!(Component);
