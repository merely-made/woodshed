// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Host geometry for severity-first status overflow at the reflow width.

use cambium::{
    AnyView, GenetCtx, GenetElement, POPOVER_CSS, STATUS_BAR_CSS, StatusBar,
    StatusBarState, StatusChip, StatusSeverity, el, status_bar,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use layout_dom_api::LayoutDom;
use taproot::Selector;

#[derive(Default)]
struct App {
    bar: StatusBarState,
    limit: usize,
}
type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type TestHost = Harness<App, fn(&App) -> Child, Child>;

fn root(app: &App) -> Child {
    let chips = vec![
        StatusChip::new("format", "Djot"),
        StatusChip::new("save", "Save refused").with_severity(StatusSeverity::Refused),
        StatusChip::new("posture", "File target"),
        StatusChip::new("retention", "Not retained"),
        StatusChip::new("recovery", "Recovery · 0 copies"),
    ];
    Box::new(
        el(
            "div",
            (
                el("div", ()).attr("style", "flex:1;"),
                status_bar(
                    StatusBar::new(
                        "The document refused a save; details remain available.",
                        &chips,
                    )
                    .with_chip_limit(app.limit),
                    &app.bar,
                    |app: &mut App, event| app.bar.apply(event),
                    |key| Some(Box::new(el("div", format!("Details for {key}"))) as Child),
                ),
            ),
        )
        .attr(
            "style",
            "display:flex;flex-direction:column;height:100vh;min-width:0;",
        ),
    )
}

fn host() -> TestHost {
    let mut host = Harness::with_hooks(
        Init {
            state: App {
                limit: 1,
                ..App::default()
            },
            logic: root as fn(&App) -> Child,
            sheet: format!(
                "body {{ margin:0;font-size:13px; }} {POPOVER_CSS} {STATUS_BAR_CSS} \
            .status-chip {{ padding:2px 8px; }} .popover {{ width:280px;padding:8px; }}"
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
fn refusal_and_overflow_fit_at_320_and_hidden_chips_are_not_mounted() {
    let host = host();
    let chips = host.with_dom(|dom| dom.all_with_class(dom.document(), "status-chip"));
    assert_eq!(chips.len(), 2);
    for chip in chips {
        let (x, _, width, _) = host.painted_rect(chip).unwrap();
        assert!(
            x >= -0.5 && x + width <= 320.5,
            "chip outside reflow width: x={x}, w={width}"
        );
    }
    assert!(
        host.with_dom(|dom| taproot::matching_estimated_fixture(
            dom,
            &Selector::role("button").containing("Save refused")
        )
        .len())
            == 1
    );
    assert!(host.with_dom(|dom| {
        taproot::matching_estimated_fixture(dom, &Selector::role("button").containing("Not retained")).is_empty()
    }));
}

#[test]
fn overflow_panel_is_bounded_and_escape_removes_details() {
    let mut host = host();
    assert!(host.click_on(&Selector::role("button").containing("+4")));
    host.relayout();
    let panels = host.with_dom(|dom| dom.all_with_class(dom.document(), "popover"));
    assert_eq!(panels.len(), 1);
    let (x, _, width, _) = host.painted_rect(panels[0]).unwrap();
    assert!(
        x >= -0.5 && x + width <= 320.5,
        "overflow outside reflow width: x={x}, w={width}"
    );
    host.key_named(winit::keyboard::NamedKey::Escape);
    host.relayout();
    assert_eq!(host.state().bar.open, None);
    assert!(host.with_dom(|dom| dom.all_with_class(dom.document(), "popover").is_empty()));
}
