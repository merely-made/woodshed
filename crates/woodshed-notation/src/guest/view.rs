// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! A block's saved notation as a view: a vector sheet to paint and a scene of regions.
use super::exports::mere::script::block_adapter::{Block, ViewAsk, ViewResult};
use super::map;
use super::sheet::{ACCENT, INK, from_paint};
use crate::{NotationLeaf, NotationView};
use sceno::{
    InstanceId, ProjectedItem, Rect, Region, Representation, Scene, Size2, SourceIx, SourceRef,
    Transform2, Vec2,
};
use sprigging::{Leaf, PaintCx};
use woodshedding::{blocks, notation::Score};

/// A cap on the encoded scene, as the interface's payloads: one MiB.
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const SCENE_VERSION: u8 = 1;

fn reading(mode: &str) -> Result<NotationView, String> {
    match mode {
        "chart" => Ok(NotationView::Chart),
        "tab" => Ok(NotationView::Tab),
        "score" => Ok(NotationView::Score),
        _ => Err("Select a notation reading".into()),
    }
}

/// The accessible name of one event, as the host pane has always labelled its buttons.
fn event_label(score: &Score, bar: usize, event: usize) -> String {
    let note = &score.measures[bar].events[event];
    format!(
        "Bar {}, event {} · {} · {}",
        bar + 1,
        event + 1,
        note.value.label(),
        if note.rest {
            "Rest".to_string()
        } else {
            note.notes
                .iter()
                .map(|n| format!("string {} fret {}", n.string, n.fret))
                .collect::<Vec<_>>()
                .join(", ")
        }
    )
}

/// One item per authored event (its footprint the hit rectangle) and one labelled
/// region per event over that item. The scene carries no rendered content.
fn scene(leaf: &NotationLeaf, score: &Score, width: f32, height: f32) -> Scene {
    let mut scene = Scene::new();
    for (bar, event, (x, y, w, h)) in leaf.event_rects() {
        let source = SourceIx(scene.sources.len() as u32);
        scene.sources.push(SourceRef::new(
            "woodshed.event",
            format!("bar:{bar}/event:{event}"),
        ));
        let id = InstanceId(scene.items.len() as u32);
        scene.items.push(ProjectedItem {
            source,
            space: Scene::WORLD,
            transform: Transform2 {
                translate: Vec2::new(x + w / 2.0, y + h / 2.0),
                ..Transform2::IDENTITY
            },
            footprint: sceno::Footprint::Rect {
                size: Size2::new(w, h),
            },
            representation: Representation::Open {
                kind: "woodshed:event".into(),
            },
            layer: 1,
            visible: true,
            hit: None,
            channels: vec![],
        });
        scene.regions.push(Region {
            members: vec![id],
            space: Scene::WORLD,
            contour: None,
            label: Some(event_label(score, bar, event)),
            confidence: 1.0,
        });
    }
    scene.bounds = Rect::new(Vec2::ZERO, Size2::new(width, height));
    scene
}

fn encode_scene(scene: &Scene) -> Result<Vec<u8>, String> {
    let mut bytes = vec![SCENE_VERSION];
    ciborium::into_writer(scene, &mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err("Notation scene exceeds one MiB".into());
    }
    Ok(bytes)
}

pub(super) fn view(b: &Block, ask: &ViewAsk) -> Result<ViewResult, String> {
    let attrs = map(&b.attrs);
    let score = blocks::music(&attrs)?
        .ok_or("This Set has no written notation. Choose a tempo, then Write notation.")?;
    let mode = reading(&ask.mode)?;
    let selected = ask
        .selected
        .map_or((usize::MAX, usize::MAX), |(bar, event)| {
            (bar as usize, event as usize)
        });
    let mut leaf = NotationLeaf::new(score.clone(), mode, selected, ask.width, INK, ACCENT);
    let size = leaf.intrinsic_size();
    let mut cmds = Vec::new();
    leaf.paint(&mut PaintCx::new(&mut cmds, size));
    let sheet = from_paint(&cmds, &leaf.labels(), size.width, size.height);
    let sheet = sheet.encode().map_err(|e| format!("{e:?}"))?;
    Ok(ViewResult {
        scene: encode_scene(&scene(&leaf, &score, size.width, size.height))?,
        sheet,
        width: size.width,
        height: size.height,
        description: format!("{} notation · {}", mode.label(), score.title),
    })
}
