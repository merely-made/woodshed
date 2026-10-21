// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! An application's request to bring one of its own elements into view,
//! issued from a hook through `AppCtx::scroll_into_view` and resolved by the
//! host against its next layout.
//!
//! Every box is placed in pixels and holds no text, so the offsets below are
//! arithmetic: the page is 2000px tall in a 400px window, so the viewport
//! scrolls at most 1600.

use std::cell::RefCell;
use std::rc::Rc;

use cambium::{AnyView, GenetCtx, GenetElement, Keyed, el};
use cambium_genet_winit_host::{Harness, Init, ScrollAlign, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};

#[derive(Default)]
struct App {
    /// Drop everything below 900px, so the page is shorter than an offset
    /// taken against the full one.
    short: bool,
    /// Remove `#gone`, leaving a stale handle to it.
    removed: bool,
    /// Bumped to force a DOM mutation that changes no geometry.
    marker: u32,
}

type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type Logic = fn(&App) -> Child;
type Host = Harness<App, Logic, Child>;
type Queue = Rc<RefCell<Vec<(NodeId, ScrollAlign)>>>;

fn place(x: i32, y: i32, w: i32, h: i32) -> String {
    format!("position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px;")
}

fn block(id: &'static str, style: String) -> Child {
    Box::new(el("div", ()).attr("id", id).attr("style", style))
}

/// | element     | box / content                                          |
/// |-------------|--------------------------------------------------------|
/// | `#scroller` | (0, 100, 200, 100), `overflow:auto` over 600px         |
/// | `#inner`    | 300px down inside it, 40px tall: laid out at y=400     |
/// | `#framed`   | (250, 100, 100, 100) under a 10px top border, scrolls  |
/// | `#framed-in`| 300px down inside it: laid out at y=410                |
/// | `#top`      | (0, 250, 100, 40), visible without scrolling           |
/// | `#mid`      | (0, 600, 100, 40)                                      |
/// | `#gone`     | (0, 800, 100, 40), until `removed`                     |
/// | `#deep`     | (0, 1000, 200, 100), `overflow:auto` over 600px        |
/// | `#deeper`   | 300px down inside it, 40px tall: laid out at y=1300    |
/// | `#far`      | (0, 1200, 100, 40)                                     |
/// | `#grows`    | (0, 1400), `overflow:auto` but as tall as its content  |
/// | `#grown`    | 80px inside `#grows`                                   |
/// | `#last`     | (0, 1900, 100, 40), inside the last screenful          |
fn root(state: &App) -> Child {
    let mut children: Vec<(u32, Child)> = vec![
        (
            0,
            Box::new(
                el(
                    "div",
                    (
                        el("div", ()).attr("style", "height:300px;"),
                        el("div", ())
                            .attr("id", "inner")
                            .attr("style", "height:40px;"),
                        el("div", ()).attr("style", "height:260px;"),
                    ),
                )
                .attr("id", "scroller")
                .attr(
                    "style",
                    format!("{}overflow:auto;", place(0, 100, 200, 100)),
                ),
            ),
        ),
        (
            8,
            Box::new(
                el(
                    "div",
                    (
                        el("div", ()).attr("style", "height:300px;"),
                        el("div", ())
                            .attr("id", "framed-in")
                            .attr("style", "height:40px;"),
                        el("div", ()).attr("style", "height:260px;"),
                    ),
                )
                .attr("id", "framed")
                .attr(
                    "style",
                    format!(
                        "{}overflow:auto;border-top:10px solid black;",
                        place(250, 100, 100, 100)
                    ),
                ),
            ),
        ),
        (1, block("top", place(0, 250, 100, 40))),
        (2, block("mid", place(0, 600, 100, 40))),
    ];
    if !state.removed {
        children.push((3, block("gone", place(0, 800, 100, 40))));
    }
    if !state.short {
        children.push((
            7,
            Box::new(
                el(
                    "div",
                    (
                        el("div", ()).attr("style", "height:300px;"),
                        el("div", ())
                            .attr("id", "deeper")
                            .attr("style", "height:40px;"),
                        el("div", ()).attr("style", "height:260px;"),
                    ),
                )
                .attr("id", "deep")
                .attr(
                    "style",
                    format!("{}overflow:auto;", place(0, 1000, 200, 100)),
                ),
            ),
        ));
        children.push((4, block("far", place(0, 1200, 100, 40))));
        children.push((
            5,
            Box::new(
                el(
                    "div",
                    el("div", ())
                        .attr("id", "grown")
                        .attr("style", "height:80px;"),
                )
                .attr("id", "grows")
                .attr(
                    "style",
                    "position:absolute;left:0px;top:1400px;width:150px;overflow:auto;",
                ),
            ),
        ));
        children.push((6, block("last", place(0, 1900, 100, 40))));
    }
    let height = if state.short { 900 } else { 2000 };
    Box::new(
        el("div", Keyed::new(children))
            .attr("data-marker", state.marker.to_string())
            .attr(
                "style",
                format!("position:relative;width:400px;height:{height}px;"),
            ),
    )
}

/// A harness whose `after_dispatch` hook turns the queue into scroll requests,
/// the way an application asks after a dispatch. Not yet laid out.
fn unlaid() -> (Host, Queue) {
    let queue = Queue::default();
    let mut hooks = inert_hooks();
    let requests = queue.clone();
    hooks.after_dispatch = Box::new(move |ctx| {
        for (node, align) in requests.borrow_mut().drain(..) {
            ctx.scroll_into_view(node, align);
        }
    });
    let host = Harness::with_hooks(
        Init {
            state: App::default(),
            logic: root as Logic,
            sheet: String::new(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    (host, queue)
}

fn harness() -> (Host, Queue) {
    let (mut host, queue) = unlaid();
    host.layout_at(400.0, 400.0);
    (host, queue)
}

/// Queue requests, run the dispatch tail that issues them, then lay out.
fn request(host: &mut Host, queue: &Queue, requests: &[(NodeId, ScrollAlign)]) {
    queue.borrow_mut().extend_from_slice(requests);
    host.after_dispatch();
    host.relayout();
}

fn node(host: &Host, id: &str) -> NodeId {
    fn find(dom: &ScriptedDom, node: NodeId, id: &str) -> Option<NodeId> {
        if dom.attribute(node, &Namespace::from(""), &LocalName::from("id")) == Some(id) {
            return Some(node);
        }
        dom.dom_children(node)
            .find_map(|child| find(dom, child, id))
    }
    host.with_dom(|dom| find(dom, dom.document(), id))
        .unwrap_or_else(|| panic!("no #{id} in the DOM"))
}

fn top(host: &Host, node: NodeId) -> f32 {
    host.painted_rect(node).expect("the element paints").1
}

#[track_caller]
fn near(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.5,
        "{what}: expected {expected}, got {actual}",
    );
}

#[test]
fn a_request_brings_an_element_far_below_the_fold_to_the_viewport_top() {
    let (mut host, queue) = harness();
    let far = node(&host, "far");
    assert!(top(&host, far) >= 400.0, "#far starts below the fold");

    request(&mut host, &queue, &[(far, ScrollAlign::Start)]);
    near(top(&host, far), 0.0, "#far's top after the request");
    near(host.viewport_scroll().1, 1200.0, "viewport offset");

    // The request is relative to where the page is now, so an element above
    // the current offset scrolls back up to the top rather than past it.
    let mid = node(&host, "mid");
    request(&mut host, &queue, &[(mid, ScrollAlign::Start)]);
    near(top(&host, mid), 0.0, "#mid's top after the second request");
}

#[test]
fn inside_a_nested_scroll_container_the_container_scrolls_not_the_window() {
    let (mut host, queue) = harness();
    let scroller = node(&host, "scroller");
    let inner = node(&host, "inner");
    let before = host.painted_rect(scroller);

    request(&mut host, &queue, &[(inner, ScrollAlign::Start)]);
    assert_eq!(
        host.viewport_scroll(),
        (0.0, 0.0),
        "the window did not move"
    );
    assert_eq!(host.painted_rect(scroller), before, "nor did the container");
    near(
        top(&host, inner),
        top(&host, scroller),
        "#inner's top against the container's",
    );
    near(host.element_scroll_total(), 300.0, "the container's offset");
}

/// A container is measured by its scrollport, inside its border: `Start` puts
/// the element under the border, not beneath it.
#[test]
fn start_aligns_to_the_scrollport_inside_a_border() {
    let (mut host, queue) = harness();
    let framed = node(&host, "framed");
    let inner = node(&host, "framed-in");

    request(&mut host, &queue, &[(inner, ScrollAlign::Start)]);
    near(
        top(&host, inner),
        top(&host, framed) + 10.0,
        "#framed-in's top against the scrollport's",
    );
    near(
        host.element_scroll(framed).1,
        300.0,
        "the container's offset",
    );
}

/// The container moves by the request's alignment, and the window then moves
/// only as far as the element needs: `#deep` is itself below the fold, so the
/// container alone would leave `#deeper` out of sight.
#[test]
fn a_container_below_the_fold_brings_the_window_along() {
    let (mut host, queue) = harness();
    let deep = node(&host, "deep");
    let deeper = node(&host, "deeper");

    request(&mut host, &queue, &[(deeper, ScrollAlign::Start)]);
    near(host.element_scroll(deep).1, 300.0, "the container's offset");
    near(
        top(&host, deeper),
        top(&host, deep),
        "#deeper's top against the container's",
    );
    near(host.viewport_scroll().1, 640.0, "viewport offset");
    near(
        top(&host, deeper),
        360.0,
        "#deeper's bottom meets the window's",
    );
}

/// Knot's preview pane is `overflow:auto` but grows to its content, so the
/// window carries the offset. A request must pass over such a box.
#[test]
fn an_overflow_box_that_grows_to_its_content_is_passed_over_for_the_window() {
    let (mut host, queue) = harness();
    let grown = node(&host, "grown");

    request(&mut host, &queue, &[(grown, ScrollAlign::Start)]);
    near(top(&host, grown), 0.0, "#grown's top after the request");
    near(host.viewport_scroll().1, 1400.0, "viewport offset");
}

#[test]
fn a_request_for_an_element_that_does_not_exist_is_a_no_op() {
    let (mut host, queue) = harness();
    let far = node(&host, "far");
    request(&mut host, &queue, &[(far, ScrollAlign::Start)]);
    let gone = node(&host, "gone");
    host.update(|state| state.removed = true);
    assert_eq!(host.painted_rect(gone), None, "#gone left the layout");

    request(&mut host, &queue, &[(gone, ScrollAlign::Start)]);
    near(host.viewport_scroll().1, 1200.0, "the offset is untouched");

    // Nor does it take the requests queued with it down.
    let mid = node(&host, "mid");
    request(
        &mut host,
        &queue,
        &[(gone, ScrollAlign::Start), (mid, ScrollAlign::Start)],
    );
    near(top(&host, mid), 0.0, "#mid, queued after the stale request");
}

#[test]
fn a_request_issued_before_the_first_layout_resolves_after_it() {
    let (mut host, queue) = unlaid();
    let far = node(&host, "far");
    queue.borrow_mut().push((far, ScrollAlign::Start));
    host.after_dispatch();
    assert_eq!(host.painted_rect(far), None, "nothing is laid out yet");

    host.layout_at(400.0, 400.0);
    near(top(&host, far), 0.0, "#far's top after the first layout");
}

#[test]
fn the_offsets_survive_a_rebuild() {
    let (mut host, queue) = harness();
    let far = node(&host, "far");
    let inner = node(&host, "inner");
    // `#inner` first: its container shows it without the window moving, and
    // `#far` then moves the window alone, so both planes hold an offset.
    request(
        &mut host,
        &queue,
        &[(inner, ScrollAlign::Start), (far, ScrollAlign::Start)],
    );
    near(host.viewport_scroll().1, 1200.0, "viewport offset");

    // A DOM mutation rebuilds the retained session in place.
    host.update(|state| state.marker += 1);
    assert!(host.relayout_profile().layout_rebuilt);
    near(top(&host, far), 0.0, "#far after an in-place rebuild");

    // A new surface size builds a fresh session, and the frame carries both
    // planes onto it.
    host.layout_at(380.0, 400.0);
    near(top(&host, far), 0.0, "#far after a fresh session");
    near(
        top(&host, inner),
        top(&host, node(&host, "scroller")),
        "#inner after a fresh session",
    );
}

#[test]
fn an_offset_past_the_end_of_the_content_is_clamped() {
    let (mut host, queue) = harness();
    let last = node(&host, "last");
    request(&mut host, &queue, &[(last, ScrollAlign::Start)]);
    near(
        host.viewport_scroll().1,
        1600.0,
        "the furthest the page scrolls",
    );
    near(
        top(&host, last),
        300.0,
        "#last cannot reach the viewport top",
    );

    // The content shrinks under that offset: the rebuild pulls it back.
    host.update(|state| state.short = true);
    near(
        host.viewport_scroll().1,
        500.0,
        "the shorter page's furthest",
    );
    near(
        top(&host, node(&host, "mid")),
        100.0,
        "#mid on the shorter page",
    );
}

#[test]
fn nearest_moves_only_as_far_as_the_element_needs() {
    let (mut host, queue) = harness();
    let visible = node(&host, "top");
    request(&mut host, &queue, &[(visible, ScrollAlign::Nearest)]);
    assert_eq!(host.viewport_scroll(), (0.0, 0.0), "already fully visible");

    // Below the fold: its bottom edge meets the viewport's bottom.
    let far = node(&host, "far");
    request(&mut host, &queue, &[(far, ScrollAlign::Nearest)]);
    near(
        top(&host, far),
        360.0,
        "#far's top once its bottom is in view",
    );

    // Above the viewport: its top edge meets the viewport's top.
    let mid = node(&host, "mid");
    request(&mut host, &queue, &[(mid, ScrollAlign::Nearest)]);
    near(top(&host, mid), 0.0, "#mid's top once it is in view");
}
