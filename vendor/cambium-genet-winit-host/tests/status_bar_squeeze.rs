// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A long message gives way to the chips: the message is cut, and every chip
//! keeps the width and the single line it has beside a short message.
//!
//! The bar spans a 400px window, docked at the foot of a column as tall as
//! the window, as Knot docks it.

use cambium::{
    AnyView, GenetCtx, GenetElement, POPOVER_CSS, STATUS_BAR_CSS, StatusBar, StatusBarState,
    StatusChip, el, status_bar,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use layout_dom_api::LayoutDom;

struct App {
    message: String,
    bar: StatusBarState,
}

type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type Logic = fn(&App) -> Child;

fn root(app: &App) -> Child {
    let chips = [
        StatusChip::new("format", "Djot"),
        StatusChip::new("save", "Unsaved changes"),
        StatusChip::new("posture", "File target"),
    ];
    Box::new(
        el(
            "div",
            (
                el("div", ()).attr("style", "flex:1;"),
                status_bar(
                    StatusBar::new(&app.message, &chips),
                    &app.bar,
                    |app: &mut App, event| app.bar.apply(event),
                    |_key: &str| None,
                ),
            ),
        )
        .attr(
            "style",
            "display:flex; flex-direction:column; height:100vh;",
        ),
    )
}

/// Each chip's painted width and height, with the given message.
fn chips_beside(message: &str) -> Vec<(f32, f32)> {
    let mut host = Harness::with_hooks(
        Init {
            state: App {
                message: message.to_owned(),
                bar: StatusBarState::default(),
            },
            logic: root as Logic,
            sheet: format!(
                "body {{ margin:0; font-size:13px; }} {POPOVER_CSS} {STATUS_BAR_CSS} \
                 .status-chip {{ padding:2px 8px; }}"
            ),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(400.0, 300.0);
    let chips = host.with_dom(|dom| dom.all_with_class(dom.document(), "status-chip"));
    chips
        .into_iter()
        .map(|chip| {
            let (_, _, width, height) = host.painted_rect(chip).expect("the chip paints");
            (width, height)
        })
        .collect()
}

#[test]
fn a_long_message_is_cut_and_the_chips_keep_their_size() {
    let short = chips_beside("Ready.");
    let long = chips_beside(
        &"Opened a document whose path runs far past the width of the bar. ".repeat(4),
    );
    assert_eq!(short.len(), 3, "three chips");
    for (index, ((width, height), (long_width, long_height))) in short.iter().zip(&long).enumerate()
    {
        assert!(
            (width - long_width).abs() < 0.5 && (height - long_height).abs() < 0.5,
            "chip {index} is {width}x{height} beside a short message \
             but {long_width}x{long_height} beside a long one",
        );
    }
}
