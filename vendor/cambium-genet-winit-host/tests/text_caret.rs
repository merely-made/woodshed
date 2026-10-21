// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The host paints the focused field's caret and selection where its text is,
//! though a highlighted field splits that text across many text nodes: a
//! field counts bytes across all of them, the layout per node.
//!
//! The textarea holds `first\nsecond` in two highlight spans that meet at the
//! line break, 20px lines from y = 10.

use cambium::{
    AnyView, CaretPosition, CaretSelection, GenetCtx, GenetElement, StyleRange, TextCommand,
    TextInput, el, lens, styled_textarea,
};
use cambium_genet_winit_host::{FocusedTextSlot, Harness, HostHooks, Init, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};

struct Field {
    text: TextInput,
}

type Child = Box<dyn AnyView<Field, (), GenetCtx, GenetElement>>;
type Logic = fn(&Field) -> Child;
type Host = Harness<Field, Logic, Child>;

const SHEET: &str = "[role=\"textbox\"] { position:absolute; left:10px; top:10px; width:300px; \
     height:200px; padding:0px; border:0px; font-size:16px; line-height:20px; \
     white-space:pre-wrap; }";

fn styles() -> Vec<StyleRange> {
    vec![
        StyleRange {
            range: 0..6,
            class: "heading".into(),
        },
        StyleRange {
            range: 6..12,
            class: "body".into(),
        },
    ]
}

fn root(_: &Field) -> Child {
    Box::new(el(
        "div",
        lens(
            |text: &mut TextInput| styled_textarea(text, &styles()),
            |field: &mut Field| &mut field.text,
        ),
    ))
}

fn host() -> Host {
    let hooks: HostHooks<Field, Logic, Child> = HostHooks {
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
                    get: Box::new(|field: &Field| &field.text),
                    get_mut: Box::new(|field: &mut Field| &mut field.text),
                })
        }),
        ..inert_hooks()
    };
    let mut host = Harness::with_hooks(
        Init {
            state: Field {
                text: TextInput::new("first\nsecond"),
            },
            logic: root as Logic,
            sheet: SHEET.to_string(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    host.layout_at(400.0, 300.0);
    let textarea = host
        .with_dom(|dom| find(dom, dom.document()))
        .expect("the textbox");
    let (x, y, _, _) = host.painted_rect(textarea).expect("it paints");
    host.click_at(x + 4.0, y + 4.0);
    assert!(host.focus().is_some(), "the click focused the textarea");
    host
}

fn find(dom: &ScriptedDom, node: NodeId) -> Option<NodeId> {
    if dom
        .attribute(
            node,
            &Namespace::from(""),
            &LocalName::from("data-cambium-text-value"),
        )
        .is_some()
    {
        return Some(node);
    }
    dom.dom_children(node).find_map(|child| find(dom, child))
}

fn select(host: &mut Host, anchor: usize, focus: usize) {
    let at = |byte| CaretPosition {
        byte,
        ..CaretPosition::default()
    };
    host.update(|field| {
        field.text.apply(TextCommand::SetSelection(CaretSelection {
            anchor: at(anchor),
            focus: at(focus),
        }));
    });
}

#[test]
fn a_caret_after_a_line_break_between_spans_sits_on_the_next_line() {
    let mut host = host();
    select(&mut host, 3, 3);
    let (_, first_y, _, first_height) = host.caret_rect().expect("the caret paints");
    assert!(first_y < 30.0, "byte 3 is on the first line, at {first_y}");
    assert!(
        first_height <= 20.5,
        "a caret is a line tall, not {first_height}"
    );

    select(&mut host, 6, 6);
    let (x, y, _, height) = host.caret_rect().expect("the caret paints");
    assert!(y >= 29.5, "byte 6 opens the second line, not y = {y}");
    assert!(height <= 20.5, "a caret is a line tall, not {height}");
    assert!(x < 14.0, "at the line's start, not x = {x}");
}

#[test]
fn a_selection_across_the_span_boundary_paints_on_both_lines() {
    let mut host = host();
    select(&mut host, 3, 9);
    let rects = host.selection_rects();
    let lines: Vec<f32> = rects.iter().map(|(_, y, _, _)| *y).collect();
    assert!(
        lines.iter().any(|y| *y < 30.0) && lines.iter().any(|y| *y >= 29.5),
        "the selection covers both lines: {rects:?}",
    );
    assert_eq!(
        host.state().text.text(),
        "first\nsecond",
        "and hides no text"
    );
}
