// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! An open popover's panel lays out against its trigger: below it and lined
//! up with its start, or above it and lined up with its end, in a flex
//! toolbar as in a plain block. POPOVER_CSS places the panel with
//! percentage insets, which resolve against the anchor's height. The panel
//! and the dismiss layer are positioned out of flow, so opening the popover
//! moves nothing in its toolbar.

use cambium::{
    AnyView, GenetCtx, GenetElement, POPOVER_CSS, Popover, PopoverPlacement, PopoverState, button,
    el, popover,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};

#[derive(Clone, Copy)]
struct Case {
    placement: PopoverPlacement,
    toolbar: &'static str,
    open: bool,
}

type Child = Box<dyn AnyView<Case, (), GenetCtx, GenetElement>>;
type Logic = fn(&Case) -> Child;
type Rect = (f32, f32, f32, f32);

fn root(case: &Case) -> Child {
    let state = PopoverState {
        open: case.open,
        return_focus: false,
    };
    let pop = popover(
        Popover::new("Open", &state)
            .with_placement(case.placement)
            .with_trigger_attr("id", "trigger"),
        |_: &mut Case, _| {},
        || Some(Box::new(el("div", "Panel").attr("style", "width:160px; height:40px;")) as Child),
    );
    Box::new(el(
        "div",
        (
            el("div", ()).attr("style", "height:120px;"),
            el(
                "div",
                (
                    button("Before", |_: &mut Case, _| {}),
                    pop,
                    button("After", |_: &mut Case, _| {}).attr("id", "after"),
                ),
            )
            .attr("style", case.toolbar),
        ),
    ))
}

fn find(dom: &ScriptedDom, node: NodeId, name: &str, value: &str) -> Option<NodeId> {
    if dom.attribute(node, &Namespace::from(""), &LocalName::from(name)) == Some(value) {
        return Some(node);
    }
    dom.dom_children(node)
        .find_map(|child| find(dom, child, name, value))
}

/// The painted rect of the first element whose `name` attribute is `value`.
fn rect_of(case: Case, name: &str, value: &str) -> Rect {
    let mut host = Harness::with_hooks(
        Init {
            state: case,
            logic: root as Logic,
            sheet: POPOVER_CSS.to_string(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(800.0, 400.0);
    let node = host.with_dom(|dom| find(dom, dom.document(), name, value).expect(value));
    host.painted_rect(node).expect("layout")
}

/// The trigger's and the open panel's painted rects.
fn rects(case: Case) -> (Rect, Rect) {
    (
        rect_of(case, "id", "trigger"),
        rect_of(case, "class", "popover"),
    )
}

#[track_caller]
fn near(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() <= 1.0,
        "{what}: expected {expected}, got {actual}",
    );
}

const TOOLBARS: [&str; 2] = ["display:flex; gap:8px;", "display:block;"];

#[test]
fn a_below_start_panel_opens_under_its_trigger() {
    for toolbar in TOOLBARS {
        let ((x, y, _, height), (panel_x, panel_y, _, _)) = rects(Case {
            placement: PopoverPlacement::BelowStart,
            toolbar,
            open: true,
        });
        near(
            panel_y,
            y + height,
            &format!("the panel's top in `{toolbar}`"),
        );
        near(panel_x, x, &format!("the panel's start in `{toolbar}`"));
    }
}

#[test]
fn an_above_panel_opens_over_its_trigger() {
    for toolbar in TOOLBARS {
        let ((_, y, _, _), (_, panel_y, _, panel_height)) = rects(Case {
            placement: PopoverPlacement::AboveStart,
            toolbar,
            open: true,
        });
        near(
            panel_y + panel_height,
            y,
            &format!("the panel's bottom in `{toolbar}`"),
        );
    }
}

/// A wrapping toolbar reflows if the open anchor grows with its panel or
/// with the window-wide dismiss layer.
#[test]
fn an_open_popover_moves_nothing_in_its_toolbar() {
    let after = |open| {
        rect_of(
            Case {
                placement: PopoverPlacement::BelowStart,
                toolbar: "display:flex; flex-wrap:wrap; gap:8px; width:300px;",
                open,
            },
            "id",
            "after",
        )
    };
    assert_eq!(after(true), after(false), "the button after the popover");
}
