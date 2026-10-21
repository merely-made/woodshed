// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A fold projection laid out by line: each row's gutter cell sits beside
//! its line, every line's text starts at the same x, an empty line still
//! takes a line's height, and a long line wraps within its own column.

use cambium::{
    AnyView, FOLD_ROWS_CSS, FieldChild, FoldProjectionLine, GenetCtx, GenetElement, el,
    fold_projection,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};

const SOURCE: &str =
    "first\n\nthird, a line long enough to wrap in a narrow column of source text\n";

type Child = Box<dyn AnyView<(), (), GenetCtx, GenetElement>>;
type Logic = fn(&()) -> Child;
type Rect = (f32, f32, f32, f32);

fn root(_: &()) -> Child {
    let projection = fold_projection(SOURCE, SOURCE.len(), &[], &[]).expect("projection");
    Box::new(
        el(
            "div",
            projection.rows(|line: &FoldProjectionLine| {
                if line.number == 1 {
                    vec![Box::new(el::<_, (), ()>("span", "▸")) as FieldChild<(), ()>]
                } else {
                    Vec::new()
                }
            }),
        )
        .attr("style", "width:300px; font-size:16px; line-height:20px;"),
    )
}

fn nodes_with_class(dom: &ScriptedDom, node: NodeId, class: &str, found: &mut Vec<NodeId>) {
    if dom.has_class(node, class) {
        found.push(node);
    }
    for child in dom.dom_children(node) {
        nodes_with_class(dom, child, class, found);
    }
}

/// The painted rect of every row and of every row's line cell, in order.
fn rects() -> (Vec<Rect>, Vec<Rect>) {
    let mut host = Harness::with_hooks(
        Init {
            state: (),
            logic: root as Logic,
            sheet: FOLD_ROWS_CSS.to_string(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(400.0, 400.0);
    let (rows, lines) = host.with_dom(|dom| {
        let (mut rows, mut lines) = (Vec::new(), Vec::new());
        nodes_with_class(dom, dom.document(), "fold-row", &mut rows);
        nodes_with_class(dom, dom.document(), "fold-line", &mut lines);
        let _ = dom.attribute(rows[0], &Namespace::from(""), &LocalName::from("data-line"));
        (rows, lines)
    });
    let rect = |node| host.painted_rect(node).expect("laid out");
    (
        rows.into_iter().map(rect).collect(),
        lines.into_iter().map(rect).collect(),
    )
}

#[test]
fn rows_stack_with_the_gutter_beside_each_line() {
    let (rows, lines) = rects();
    assert_eq!(rows.len(), 3);
    let height = |rect: &Rect| rect.3;
    assert!(
        (height(&rows[1]) - height(&rows[0])).abs() <= 1.0 && height(&rows[1]) > 0.0,
        "the empty line takes a line's height: {rows:?}"
    );
    assert!(
        height(&rows[2]) > height(&rows[0]) * 1.5,
        "the long line wraps within its column: {rows:?}"
    );
    for pair in rows.windows(2) {
        assert!(
            (pair[1].1 - (pair[0].1 + pair[0].3)).abs() <= 1.0,
            "rows stack with no gap or overlap: {rows:?}"
        );
    }
    assert!(
        lines.iter().all(|line| (line.0 - lines[0].0).abs() <= 0.5) && lines[0].0 > rows[0].0,
        "every line's text starts at the same x, past the gutter: {lines:?}"
    );
}
