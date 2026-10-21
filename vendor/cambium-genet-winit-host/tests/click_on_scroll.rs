// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The harness clicks what a reader could. An element a scrolling pane clips
//! is scrolled into view before the click, so the click lands on it rather
//! than on whatever paints where it would be.
//!
//! The button sits 600px down a 200px pane, in a 300px window. A lead block
//! above the pane is empty unless a test gives it height.

use cambium::{AnyView, GenetCtx, GenetElement, button, el};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};
use taproot::Selector;

struct Clicks {
    count: u32,
}

type Child = Box<dyn AnyView<Clicks, (), GenetCtx, GenetElement>>;
type Logic = fn(&Clicks) -> Child;

const SHEET: &str = "body { margin:0; } #lead { height:0; } \
     #pane { height:200px; overflow:auto; } \
     #spacer { height:600px; } button { display:block; height:40px; }";

fn root(_: &Clicks) -> Child {
    Box::new(el(
        "div",
        (
            el("div", ()).attr("id", "lead"),
            el(
                "div",
                (
                    el("div", ()).attr("id", "spacer"),
                    button("Far down", |clicks: &mut Clicks, _| clicks.count += 1),
                ),
            )
            .attr("id", "pane"),
        ),
    ))
}

fn pane(host: &Harness<Clicks, Logic, Child>) -> NodeId {
    node(host, "pane")
}

fn node(host: &Harness<Clicks, Logic, Child>, id: &str) -> NodeId {
    fn find(dom: &ScriptedDom, node: NodeId, id: &str) -> Option<NodeId> {
        if dom.attribute(node, &Namespace::from(""), &LocalName::from("id")) == Some(id) {
            return Some(node);
        }
        dom.dom_children(node)
            .find_map(|child| find(dom, child, id))
    }
    let dom = host.runner().dom();
    let dom = dom.borrow();
    find(&dom, dom.document(), id).expect("the element is in the DOM")
}

fn horizontal_root(_: &Clicks) -> Child {
    Box::new(el(
        "div",
        (
            el("div", ()).attr("id", "lead"),
            el(
                "div",
                (
                    el("div", ()).attr("id", "spacer"),
                    el(
                        "div",
                        el(
                            "div",
                            (
                                button("Left node", |clicks: &mut Clicks, _| clicks.count += 1)
                                    .attr("id", "left"),
                                button("Right node", |clicks: &mut Clicks, _| clicks.count += 1)
                                    .attr("id", "right"),
                            ),
                        )
                        .attr("id", "canvas"),
                    )
                    .attr("id", "horizontal"),
                ),
            )
            .attr("id", "pane"),
        ),
    ))
}

const HORIZONTAL_SHEET: &str = "body { margin:0; } #lead { height:0; } \
    #pane { width:300px; height:200px; overflow-y:auto; overflow-x:hidden; } \
    #spacer { height:600px; } \
    #horizontal { width:300px; height:100px; overflow-x:auto; overflow-y:hidden; } \
    #canvas { position:relative; width:900px; height:100px; } \
    button { position:absolute; top:0; width:80px; height:40px; } \
    #left { left:0; } #right { left:820px; }";

fn horizontal_host(sheet: String) -> Harness<Clicks, Logic, Child> {
    let mut host = Harness::with_hooks(
        Init {
            state: Clicks { count: 0 },
            logic: horizontal_root as Logic,
            sheet,
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(420.0, 300.0);
    host
}

#[test]
fn click_on_reveals_rightmost_node_in_horizontal_and_vertical_planes() {
    let mut host = horizontal_host(HORIZONTAL_SHEET.to_string());
    assert!(host.visible_rect(node(&host, "right")).is_none());
    assert!(host.click_on(&Selector::role("button").containing("Right node")));
    assert_eq!(
        host.state().count,
        1,
        "semantic click reached the rightmost node"
    );
    assert_eq!(host.element_scroll(node(&host, "horizontal")), (600.0, 0.0));
    assert!(host.element_scroll(pane(&host)).1 > 0.0);
    assert_eq!(host.viewport_scroll(), (0.0, 0.0));
    assert!(host.click_on(&Selector::role("button").containing("Left node")));
    assert_eq!(host.element_scroll(node(&host, "horizontal")), (0.0, 0.0));
    assert_eq!(host.state().count, 2);
}

#[test]
fn click_on_reveals_through_every_horizontal_ancestor() {
    let mut host = horizontal_host(format!(
        "{HORIZONTAL_SHEET} \
        #pane {{ overflow-x:auto; }} #horizontal {{ width:500px; }}"
    ));
    assert!(host.click_on(&Selector::role("button").containing("Right node")));
    assert_eq!(host.state().count, 1);
    assert_eq!(host.element_scroll(node(&host, "horizontal")), (400.0, 0.0));
    assert_eq!(host.element_scroll(pane(&host)).0, 200.0);
    assert!(host.element_scroll(pane(&host)).1 > 0.0);
    assert_eq!(host.viewport_scroll(), (0.0, 0.0));
}

#[test]
fn click_on_reveals_horizontal_document_overflow() {
    let mut host = horizontal_host(format!(
        "{HORIZONTAL_SHEET} \
        #pane {{ width:900px; overflow:visible; }} \
        #horizontal {{ width:900px; overflow:visible; }}"
    ));
    assert!(host.click_on(&Selector::role("button").containing("Right node")));
    assert_eq!(host.state().count, 1);
    assert_eq!(host.viewport_scroll().0, 480.0);
    assert!(host.viewport_scroll().1 > 0.0);
    assert_eq!(host.element_scroll(node(&host, "horizontal")), (0.0, 0.0));
    assert_eq!(host.element_scroll(pane(&host)), (0.0, 0.0));
}

#[test]
fn click_on_wide_target_reveals_visible_part_without_oscillation() {
    let mut host = horizontal_host(format!(
        "{HORIZONTAL_SHEET} \
        #right {{ left:100px; width:800px; }}"
    ));
    assert!(host.click_on(&Selector::role("button").containing("Right node")));
    let scroll = host.element_scroll(node(&host, "horizontal"));
    assert_eq!(scroll, (100.0, 0.0));
    assert!(host.click_on(&Selector::role("button").containing("Right node")));
    assert_eq!(host.element_scroll(node(&host, "horizontal")), scroll);
    assert_eq!(host.state().count, 2);
}

#[test]
fn click_on_does_not_scroll_hidden_overflow_or_chase_it_in_document() {
    for overflow in ["hidden", "clip"] {
        let mut host = horizontal_host(format!(
            "{HORIZONTAL_SHEET} \
            #horizontal {{ overflow-x:{overflow}; }}"
        ));
        assert!(!host.click_on(&Selector::role("button").containing("Right node")));
        assert_eq!(host.state().count, 0);
        assert_eq!(host.element_scroll(node(&host, "horizontal")), (0.0, 0.0));
        assert_eq!(host.element_scroll(pane(&host)).0, 0.0);
        assert_eq!(host.viewport_scroll().0, 0.0);
    }
}

#[test]
fn click_on_scrolls_a_clipped_element_into_view_and_clicks_it() {
    let mut host = Harness::with_hooks(
        Init {
            state: Clicks { count: 0 },
            logic: root as Logic,
            sheet: SHEET.to_string(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(400.0, 300.0);
    assert_eq!(host.element_scroll(pane(&host)), (0.0, 0.0));
    assert!(host.click_on(&Selector::role("button").containing("Far down")));
    assert_eq!(host.state().count, 1, "the click reached the button");
    assert!(
        host.element_scroll(pane(&host)).1 > 0.0,
        "the pane scrolled to the button"
    );
}

/// A pane that is itself below the window: scrolling the pane alone would
/// leave the button out of sight, so the window follows.
#[test]
fn click_on_brings_a_pane_below_the_window_along() {
    let mut host = Harness::with_hooks(
        Init {
            state: Clicks { count: 0 },
            logic: root as Logic,
            sheet: format!("{SHEET} #lead {{ height:400px; }}"),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(400.0, 300.0);
    assert!(host.click_on(&Selector::role("button").containing("Far down")));
    assert_eq!(host.state().count, 1, "the click reached the button");
    assert!(
        host.element_scroll(pane(&host)).1 > 0.0,
        "the pane scrolled to the button"
    );
    assert!(
        host.viewport_scroll().1 > 0.0,
        "the window scrolled to the pane"
    );
}

/// A button taller than its pane is clicked where it shows: its centre, 300px
/// down a 200px pane, is hidden.
#[test]
fn click_on_clicks_the_part_of_a_tall_element_that_shows() {
    let mut host = Harness::with_hooks(
        Init {
            state: Clicks { count: 0 },
            logic: root as Logic,
            sheet: format!("{SHEET} #spacer {{ height:0; }} button {{ height:600px; }}"),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(400.0, 300.0);
    assert!(host.click_on(&Selector::role("button").containing("Far down")));
    assert_eq!(host.state().count, 1, "the click reached the tall button");
}
