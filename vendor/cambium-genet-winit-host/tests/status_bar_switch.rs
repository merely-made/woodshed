// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A status bar switches popovers on one click: with one chip's popover open,
//! a click on another chip opens that one. The click has to reach the chip
//! through the open popover's click-outside layer, which covers the window.
//!
//! The bar is docked along the bottom of a 600 by 400 window, at the foot of
//! a column as tall as the window, as Knot docks it.

use cambium::{
    AnyView, GenetCtx, GenetElement, POPOVER_CSS, STATUS_BAR_CSS, StatusBar, StatusBarState,
    StatusChip, el, status_bar,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use taproot::Selector;

#[derive(Default)]
struct App {
    bar: StatusBarState,
}

type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type Logic = fn(&App) -> Child;

fn root(app: &App) -> Child {
    let chips = [
        StatusChip::new("format", "Djot"),
        StatusChip::new("save", "Saved"),
    ];
    Box::new(
        el(
            "div",
            (
                el("div", ()).attr("style", "flex:1;"),
                status_bar(
                    StatusBar::new("Ready.", &chips),
                    &app.bar,
                    |app: &mut App, event| app.bar.apply(event),
                    |key: &str| Some(Box::new(el("div", format!("{key} detail"))) as Child),
                ),
            ),
        )
        .attr(
            "style",
            "display:flex; flex-direction:column; height:100vh;",
        ),
    )
}

#[test]
fn a_click_on_another_chip_opens_its_popover() {
    let mut host = Harness::with_hooks(
        Init {
            state: App::default(),
            logic: root as Logic,
            sheet: format!("body {{ margin:0; }} {POPOVER_CSS} {STATUS_BAR_CSS}"),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(600.0, 400.0);
    assert!(host.click_on(&Selector::role("button").containing("Djot")));
    assert_eq!(host.state().bar.open.as_deref(), Some("format"));

    assert!(host.click_on(&Selector::role("button").containing("Saved")));
    assert_eq!(
        host.state().bar.open.as_deref(),
        Some("save"),
        "one click switched the popover"
    );

    // A click on the open chip still closes it, and a click elsewhere
    // still dismisses.
    assert!(host.click_on(&Selector::role("button").containing("Saved")));
    assert_eq!(host.state().bar.open, None);
    assert!(host.click_on(&Selector::role("button").containing("Djot")));
    host.click_at(300.0, 100.0);
    assert_eq!(host.state().bar.open, None, "a click outside closed it");
}
