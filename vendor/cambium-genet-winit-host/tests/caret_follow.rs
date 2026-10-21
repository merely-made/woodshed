// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A multi-line field that grows with its text keeps a moved caret in view
//! down the page: the host scrolls the nearest pane that scrolls, else the
//! window, and a caret that has not moved leaves the reader's scroll alone.
//!
//! Sixty lines of 20px make the field 1200px tall, in a 300px window or a
//! 200px pane. A lead block above the pane is empty unless a test gives it
//! height.

use cambium::{
    AnyView, CaretPosition, CaretSelection, GenetCtx, GenetElement, TextCommand, TextInput, el,
    lens, textarea_typed,
};
use cambium_genet_winit_host::{FocusedTextSlot, Harness, HostHooks, Init, Modifiers, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};

struct Doc {
    text: TextInput,
}

type Child = Box<dyn AnyView<Doc, (), GenetCtx, GenetElement>>;
type Logic = fn(&Doc) -> Child;
type Host = Harness<Doc, Logic, Child>;

const FIELD: &str = "body { margin:0; } \
     [role=\"textbox\"] { display:block; width:300px; font-size:16px; line-height:20px; padding:0; border:none; white-space:pre-wrap; }";

/// The field grows with its text and the window scrolls.
const IN_WINDOW: &str = "";

/// The field grows with its text inside a pane that scrolls.
const IN_PANE: &str = "#pane { height:200px; overflow:auto; }";

/// The same pane, itself below the 300px window.
const IN_PANE_BELOW: &str = "#lead { height:400px; } #pane { height:200px; overflow:auto; }";

fn source() -> String {
    (0..60).map(|line| format!("line {line}\n")).collect()
}

fn root(_: &Doc) -> Child {
    Box::new(el(
        "div",
        (
            el("div", ()).attr("id", "lead"),
            el(
                "div",
                lens(
                    |text: &mut TextInput| textarea_typed(text),
                    |doc: &mut Doc| &mut doc.text,
                ),
            )
            .attr("id", "pane"),
        ),
    ))
}

fn host(layout: &str) -> Host {
    let hooks: HostHooks<Doc, Logic, Child> = HostHooks {
        focused_text: Box::new(|runner| {
            let focused = runner.focus()?;
            let dom = runner.dom();
            let dom_ref = dom.borrow();
            dom_ref
                .attribute(
                    focused,
                    &Namespace::from(""),
                    &LocalName::from("data-cambium-text-value"),
                )
                .is_some()
                .then(|| FocusedTextSlot {
                    node: focused,
                    get: Box::new(|doc: &Doc| &doc.text),
                    get_mut: Box::new(|doc: &mut Doc| &mut doc.text),
                })
        }),
        ..inert_hooks()
    };
    let mut host = Harness::with_hooks(
        Init {
            state: Doc {
                text: TextInput::new(source()),
            },
            logic: root as Logic,
            sheet: format!("{FIELD} {layout}"),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    host.layout_at(400.0, 300.0);
    host.set_modifiers(Modifiers::NONE);
    host
}

fn by_id(host: &Host, id: &str) -> NodeId {
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

/// Focus the field near its top, then put the caret at the end of its text.
fn caret_to_end(host: &mut Host) {
    host.click_at(20.0, 10.0);
    assert!(host.focus().is_some(), "the click focused the field");
    caret_at(host, source().len());
}

/// Put the caret at `byte` of the field's text.
fn caret_at(host: &mut Host, byte: usize) {
    let at = CaretPosition {
        byte,
        ..CaretPosition::default()
    };
    host.update(|doc| {
        doc.text.apply(TextCommand::SetSelection(CaretSelection {
            anchor: at,
            focus: at,
        }));
    });
}

#[test]
fn a_moved_caret_scrolls_the_window_down_to_its_line() {
    let mut host = host(IN_WINDOW);
    caret_to_end(&mut host);
    assert!(
        host.viewport_scroll().1 > 0.0,
        "the window scrolled to the caret"
    );
    let (_, y, _, height) = host.caret_rect().expect("the caret paints");
    assert!(
        y >= 0.0 && y + height <= 300.0,
        "the caret at {y}..{} shows in the 300px window",
        y + height
    );
}

#[test]
fn a_moved_caret_scrolls_its_pane_and_not_the_window() {
    let mut host = host(IN_PANE);
    caret_to_end(&mut host);
    let pane = by_id(&host, "pane");
    assert!(
        host.element_scroll(pane).1 > 0.0,
        "the pane scrolled to the caret"
    );
    assert_eq!(host.viewport_scroll(), (0.0, 0.0), "the window stayed");
    let (_, y, _, height) = host.caret_rect().expect("the caret paints");
    assert!(
        y >= 0.0 && y + height <= 200.0,
        "the caret at {y}..{} shows in the 200px pane",
        y + height
    );
}

/// The pane is itself below the window: scrolling it alone would leave the
/// caret out of sight, so the window follows.
#[test]
fn a_moved_caret_brings_a_pane_below_the_window_along() {
    let mut host = host(IN_PANE_BELOW);
    host.tab(true);
    assert!(host.focus().is_some(), "Tab focused the field");
    // Tab leaves the caret at the end and follows it there; start again from
    // the first line, with the window back at its top.
    caret_at(&mut host, 0);
    host.move_to(100.0, 50.0);
    host.wheel(0.0, -1000.0);
    assert_eq!(host.viewport_scroll(), (0.0, 0.0));
    caret_at(&mut host, source().len());
    let pane = by_id(&host, "pane");
    assert!(
        host.element_scroll(pane).1 > 0.0,
        "the pane scrolled to the caret"
    );
    assert!(
        host.viewport_scroll().1 > 0.0,
        "the window scrolled to the pane"
    );
    let (_, y, _, height) = host.caret_rect().expect("the caret paints");
    assert!(
        y >= 0.0 && y + height <= 300.0,
        "the caret at {y}..{} shows in the 300px window",
        y + height
    );
}

#[test]
fn a_caret_that_has_not_moved_leaves_the_readers_scroll() {
    let mut host = host(IN_PANE);
    caret_to_end(&mut host);
    let pane = by_id(&host, "pane");
    let followed = host.element_scroll(pane).1;
    host.move_to(100.0, 100.0);
    host.wheel(0.0, -400.0);
    let read_back = host.element_scroll(pane).1;
    assert!(read_back < followed, "the wheel scrolled the pane back up");
    host.relayout();
    assert_eq!(
        host.element_scroll(pane).1,
        read_back,
        "a relayout with the caret unmoved keeps the reader's scroll"
    );
}
