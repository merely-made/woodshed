// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Physical hit-testing and reflow bounds for the shared rail/drawer frame.

use cambium::{
    AnyView, FRISKET_CSS, GenetCtx, GenetElement, Slot, button, el, frisket_presented_with,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use layout_dom_api::LayoutDom;
use taproot::Selector;
use workbench::{
    CollapsedStack, ContentSource, DrawerGeometry, SplitAxis, StackPresentation, TabStack, Tile,
    TileBranch, TileId, TileTree, WorkbenchPresentation,
};

struct App {
    tree: TileTree,
    presentation: WorkbenchPresentation,
    frame_height: f32,
}
type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type TestHost = Harness<App, fn(&App) -> Child, Child>;

fn tile(id: u64) -> Tile {
    Tile {
        id: TileId(id),
        title: format!("Tile {id}"),
        content: ContentSource::Open {
            kind: "test".into(),
            id: id.to_string(),
        },
        accent: None,
    }
}

fn root(app: &App) -> Child {
    Box::new(
        el(
            "div",
            frisket_presented_with(
                &app.tree,
                None,
                &|_| None,
                &app.presentation,
                |_: &mut App, _| {},
                |app: &mut App, event| {
                    app.presentation.apply(event);
                },
                |tile: &Tile| {
                    Slot::View(Box::new(button(
                        format!("Content {}", tile.id.0),
                        |_: &mut App, _| {},
                    )))
                },
            ),
        )
        .attr(
            "style",
            format!("width:100vw;height:{}px;min-width:0;", app.frame_height),
        ),
    )
}

fn host() -> TestHost {
    let mut host = Harness::with_hooks(
        Init {
            state: App {
                frame_height: 480.0,
                tree: TileTree::Split {
                    axis: SplitAxis::Row,
                    children: vec![
                        TileBranch::new(
                            0.7,
                            TileTree::Stack(TabStack {
                                tabs: vec![tile(1)],
                                active: 0,
                            }),
                        ),
                        TileBranch::new(
                            0.3,
                            TileTree::Stack(TabStack {
                                tabs: vec![tile(2), tile(3)],
                                active: 1,
                            }),
                        ),
                    ],
                },
                presentation: WorkbenchPresentation {
                    stacks: vec![StackPresentation {
                        anchor: TileId(2),
                        min_width: 280.0,
                        collapsed: Some(CollapsedStack {
                            label: "Reading".into(),
                            rail_width: 28.0,
                            open: false,
                            drawer: Some(DrawerGeometry {
                                trigger: (292.0, 0.0, 28.0, 0.0),
                                panel_size: (400.0, 600.0),
                                bounds: (0.0, 0.0, 320.0, 480.0),
                            }),
                        }),
                    }],
                    return_focus: Some(TileId(1)),
                    ..Default::default()
                },
            },
            logic: root as fn(&App) -> Child,
            sheet: format!(
                "body {{ margin:0; }} {FRISKET_CSS} .frisket-panel-close {{ height:32px;flex:0 0 32px; }}"
            ),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(320.0, 480.0);
    host
}

#[test]
fn a_narrow_rail_keeps_its_full_vertical_label_within_painted_bounds() {
    let host = host();
    let rails = host.with_dom(|dom| dom.all_with_class(dom.document(), "frisket-rail"));
    let labels = host.with_dom(|dom| dom.all_with_class(dom.document(), "frisket-rail-label"));
    assert_eq!(rails.len(), 1);
    assert_eq!(labels.len(), 1);
    let (rx, ry, rw, rh) = host.painted_rect(rails[0]).unwrap();
    let (x, y, width, height) = host.painted_rect(labels[0]).unwrap();
    assert!(
        width < height,
        "rail label remains horizontal: ({x}, {y}, {width}, {height})"
    );
    assert!(width > 0.0 && height > 28.0, "full label must be painted");
    assert!(
        x >= rx - 0.5 && x + width <= rx + rw + 0.5 && y >= ry - 0.5 && y + height <= ry + rh + 0.5,
        "label outside rail: label=({x}, {y}, {width}, {height}); rail=({rx}, {ry}, {rw}, {rh})"
    );
    assert_eq!(
        host.with_dom(|dom| taproot::matching_estimated_fixture(
            dom,
            &Selector::role("button").containing("Open Reading")
        )
        .len()),
        1
    );
}

#[test]
fn rail_label_remains_bounded_and_actionable_in_a_short_reflow_frame() {
    let mut host = host();
    host.update(|app| {
        app.frame_height = 80.0;
        let collapsed = app.presentation.stacks[0].collapsed.as_mut().unwrap();
        collapsed.label = "Navigator".into();
        let drawer = collapsed.drawer.as_mut().unwrap();
        drawer.bounds = (0.0, 0.0, 320.0, 175.0);
    });
    host.layout_at(320.0, 175.0);
    let labels = host.with_dom(|dom| dom.all_with_class(dom.document(), "frisket-rail-label"));
    let (x, y, width, height) = host.painted_rect(labels[0]).unwrap();
    assert!(
        x >= 0.0 && x + width <= 320.5 && y >= -0.5 && y + height <= 80.5,
        "short rail label outside frame: ({x}, {y}, {width}, {height})"
    );
    assert!(host.click_on(&Selector::role("button").containing("Open Navigator")));
    assert!(
        host.state().presentation.stacks[0]
            .collapsed
            .as_ref()
            .unwrap()
            .open
    );
}

#[test]
fn drawer_fits_reflow_width_and_pointer_close_restores_the_rail() {
    let mut host = host();
    let canonical = host.state().tree.clone();
    assert!(host.click_on(&Selector::role("button").containing("Reading")));
    host.relayout();
    let panels = host.with_dom(|dom| dom.all_with_class(dom.document(), "overlay-surface-panel"));
    assert_eq!(panels.len(), 1);
    let (x, y, width, height) = host.painted_rect(panels[0]).unwrap();
    assert!(
        x >= -0.5 && y >= -0.5 && x + width <= 320.5 && y + height <= 480.5,
        "drawer outside root: ({x}, {y}, {width}, {height})"
    );
    assert!(host.click_on(&Selector::class("frisket-panel-close")));
    host.relayout();
    assert!(host.with_dom(|dom| {
        dom.all_with_class(dom.document(), "overlay-surface-panel")
            .is_empty()
    }));
    assert!(
        !host.state().presentation.stacks[0]
            .collapsed
            .as_ref()
            .unwrap()
            .open
    );
    assert_eq!(host.state().tree, canonical);
    let sources =
        host.with_dom(|dom| dom.all_with_class(dom.document(), "frisket-presented-stack"));
    assert_eq!(host.focus(), sources.first().copied());
    assert!(host.with_dom(|dom| {
        taproot::matching_estimated_fixture(dom, &Selector::role("button").containing("Content 3")).is_empty()
    }));
}

#[test]
fn escape_from_drawer_content_dismisses_without_changing_canonical_selection() {
    let mut host = host();
    let canonical = host.state().tree.clone();
    assert!(host.click_on(&Selector::role("button").containing("Reading")));
    host.relayout();
    assert!(host.click_on(&Selector::role("button").containing("Content 3")));
    host.key_named(winit::keyboard::NamedKey::Escape);
    host.relayout();
    assert!(
        !host.state().presentation.stacks[0]
            .collapsed
            .as_ref()
            .unwrap()
            .open
    );
    assert_eq!(host.state().tree, canonical);
    let sources =
        host.with_dom(|dom| dom.all_with_class(dom.document(), "frisket-presented-stack"));
    assert_eq!(host.focus(), sources.first().copied());
}
