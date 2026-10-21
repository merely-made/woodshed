// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A single-line field keeps a long value on one line at a width the value
//! does not set, and the host keeps its caret in view: scrolling the field as
//! the caret moves past an edge, painting the caret where the scrolled text
//! is, and placing a clicked caret under the pointer.
//!
//! The field is 200px of content inside 8px of padding and a 1px border, so
//! its padding box runs from 11 to 227 on x.

use cambium::{
    AnyView, CaretPosition, CaretSelection, GenetCtx, GenetElement, TextCommand, TextInput, el,
    lens, text_field,
};
use cambium_genet_winit_host::{FocusedTextSlot, Harness, HostHooks, Init, Modifiers, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::LayoutDom;
use winit::keyboard::{Key, NamedKey};

struct Field {
    text: TextInput,
}

type Child = Box<dyn AnyView<Field, (), GenetCtx, GenetElement>>;
type Logic = fn(&Field) -> Child;
type Host = Harness<Field, Logic, Child>;

const LONG: &str = "C:/Users/someone/AppData/Local/Temp/a/very/long/folder/structure/that/keeps/going/document.djot";

const SIZED: &str = "[role=\"textbox\"] { position:absolute; left:10px; top:10px; width:200px; \
     padding:4px 8px; border:1px solid black; font-size:16px; }";

/// No width anywhere: the field's width is its own default.
const UNSIZED: &str = "[role=\"textbox\"] { position:absolute; left:10px; top:10px; \
     padding:4px 8px; border:1px solid black; font-size:16px; }";

fn root(_: &Field) -> Child {
    Box::new(el(
        "div",
        lens(
            |text: &mut TextInput| text_field(text),
            |field: &mut Field| &mut field.text,
        ),
    ))
}

fn host(value: &str, sheet: &str) -> Host {
    let hooks: HostHooks<Field, Logic, Child> = HostHooks {
        focused_text: Box::new(|runner| {
            let focused = runner.focus()?;
            let dom = runner.dom();
            let dom_ref = dom.borrow();
            dom_ref
                .attribute(
                    focused,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("data-cambium-text-value"),
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
                text: TextInput::new(value),
            },
            logic: root as Logic,
            sheet: sheet.to_string(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    host.layout_at(400.0, 100.0);
    host.set_modifiers(Modifiers::NONE);
    host
}

fn field(host: &Host) -> NodeId {
    fn find(dom: &ScriptedDom, node: NodeId) -> Option<NodeId> {
        if dom
            .attribute(
                node,
                &layout_dom_api::Namespace::from(""),
                &layout_dom_api::LocalName::from("data-cambium-text-value"),
            )
            .is_some()
        {
            return Some(node);
        }
        dom.dom_children(node).find_map(|child| find(dom, child))
    }
    host.with_dom(|dom| find(dom, dom.document()))
        .expect("the field is in the DOM")
}

fn size(host: &Host) -> (f32, f32) {
    let (_, _, width, height) = host.painted_rect(field(host)).expect("the field paints");
    (width, height)
}

fn sizing_style(host: &Host) -> Vec<(&'static str, Option<String>)> {
    [
        "position",
        "width",
        "contain",
        "contain-intrinsic-size",
        "font-size",
    ]
    .into_iter()
    .map(|property| (property, host.computed_value(field(host), property)))
    .collect()
}

/// Focus the field with a click near its left edge.
fn focus(host: &mut Host) {
    let (x, y, _, height) = host.painted_rect(field(host)).expect("the field paints");
    host.click_at(x + 12.0, y + height / 2.0);
    assert!(host.focus().is_some(), "the click focused the field");
}

#[track_caller]
fn assert_caret_inside(host: &Host) {
    let (x, _, width, _) = host.caret_rect().expect("the caret paints");
    assert!(
        x >= 11.0 && x + width <= 227.0,
        "the caret at {x}..{} sits inside the field's 11..227",
        x + width,
    );
}

#[test]
fn a_long_value_stays_on_one_line_at_the_width_the_sheet_sets() {
    let short = size(&host("notes.djot", SIZED));
    let long = size(&host(LONG, SIZED));
    assert_eq!(short.0, 218.0, "200px of content, padding and border");
    assert_eq!(
        long, short,
        "the long value neither widens nor wraps the field"
    );
}

#[test]
fn a_field_with_no_width_keeps_its_own_whatever_the_value() {
    let short_host = host("notes.djot", UNSIZED);
    let long_host = host(LONG, UNSIZED);
    let short = size(&short_host);
    let long = size(&long_host);
    assert_eq!(
        short.0,
        178.0,
        "the former 20-column default plus padding and border; resolved {:?}",
        sizing_style(&short_host)
    );
    assert_eq!(long, short, "the value does not set the field's size");
}

#[test]
fn a_host_can_set_the_unsized_fields_intrinsic_width() {
    let sheet = format!("{UNSIZED} [role=\"textbox\"] {{ --cambium-field-intrinsic-width:12em; }}");
    let host = host(LONG, &sheet);
    assert_eq!(
        size(&host).0,
        210.0,
        "the host's 12em fallback plus padding and border; resolved {:?}",
        sizing_style(&host)
    );
}

#[test]
fn moving_the_caret_past_an_edge_scrolls_the_field_to_it() {
    let mut host = host(LONG, SIZED);
    focus(&mut host);
    let node = field(&host);
    assert_eq!(host.element_scroll(node), (0.0, 0.0));

    host.key(Key::Named(NamedKey::End));
    assert!(host.element_scroll(node).0 > 0.0, "End scrolled the field");
    assert_caret_inside(&host);

    host.key_injected("x");
    assert!(host.state().text.text().ends_with("djotx"));
    assert_caret_inside(&host);

    host.key(Key::Named(NamedKey::Home));
    assert_eq!(host.element_scroll(node).0, 0.0, "Home scrolled it back");
    assert_caret_inside(&host);
}

#[test]
fn a_field_scrolled_by_hand_keeps_its_scroll_until_the_caret_moves() {
    let mut host = host(LONG, SIZED);
    focus(&mut host);
    let node = field(&host);
    host.key(Key::Named(NamedKey::End));
    assert!(host.element_scroll(node).0 > 0.0);

    host.wheel(-10_000.0, 0.0);
    assert_eq!(
        host.element_scroll(node).0,
        0.0,
        "the wheel scrolled it back"
    );
    assert!(
        host.caret_rect().is_none(),
        "the caret is out of view, unpainted"
    );
    host.relayout();
    assert_eq!(
        host.element_scroll(node).0,
        0.0,
        "a relayout leaves it there"
    );

    host.key(Key::Named(NamedKey::ArrowLeft));
    assert!(
        host.element_scroll(node).0 > 0.0,
        "a caret move follows it again"
    );
    assert_caret_inside(&host);
}

#[test]
fn a_click_in_a_scrolled_field_places_the_caret_under_the_pointer() {
    let mut host = host(LONG, SIZED);
    focus(&mut host);
    host.key(Key::Named(NamedKey::End));
    let (x, y, width, height) = host.painted_rect(field(&host)).expect("the field paints");
    // Near the right edge the scrolled field shows the end of the value.
    host.click_at(x + width - 12.0, y + height / 2.0);
    let byte = host.state().text.caret_position().byte;
    assert!(
        byte + 6 >= LONG.len(),
        "the click landed at byte {byte} of {}, not in the scrolled-away start",
        LONG.len(),
    );
}

#[test]
fn an_empty_field_paints_its_caret_at_the_start_of_its_content() {
    let mut host = host("", SIZED);
    focus(&mut host);
    let (x, _, _, height) = host.caret_rect().expect("the caret paints");
    assert_eq!(x, 19.0, "10px in, past the 1px border and 8px of padding");
    assert!(height > 0.0, "a line tall");
}

#[test]
fn a_selection_across_a_scrolled_field_paints_only_inside_it() {
    let mut host = host(LONG, SIZED);
    focus(&mut host);
    let at = |byte| CaretPosition {
        byte,
        ..CaretPosition::default()
    };
    host.update(|field| {
        field.text.apply(TextCommand::SetSelection(CaretSelection {
            anchor: at(0),
            focus: at(LONG.len()),
        }));
    });
    assert!(
        host.element_scroll(field(&host)).0 > 0.0,
        "the caret end scrolled"
    );
    let rects = host.selection_rects();
    assert!(!rects.is_empty(), "the selection paints");
    for (x, _, width, _) in rects {
        assert!(
            x >= 11.0 && x + width <= 227.0,
            "a selection run at {x}..{} stays inside 11..227",
            x + width,
        );
    }
}
