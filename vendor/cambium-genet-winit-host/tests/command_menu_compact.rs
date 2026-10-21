// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Compact menus reuse one bounded popup instead of sideways flyouts.

use cambium::{
    AnyView, COMMAND_MENU_BAR_CSS, CommandEvent, CommandItem, CommandMenuBarState, GenetCtx,
    GenetElement, command_menu_bar, component, el,
};
use cambium_genet_winit_host::{Harness, Init, inert_hooks};
use layout_dom_api::LayoutDom;
use taproot::Selector;

#[derive(Default)]
struct App {
    activations: Vec<Vec<usize>>,
}
type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type TestHost = Harness<App, fn(&App) -> Child, Child>;

fn root(_: &App) -> Child {
    let items = vec![
        CommandItem::new("View").with_children([
            CommandItem::new("Reading")
                .with_children([CommandItem::new("Preview"), CommandItem::new("Outline")]),
            CommandItem::new("Choose reading").with_shortcut("Ctrl+Shift+R"),
            CommandItem::new("View source").with_shortcut("Ctrl+Shift+U"),
            CommandItem::new("Reading font size preferences"),
        ]),
    ];
    let bar = component(
        items,
        |_| CommandMenuBarState::default(),
        |_, _, _| {},
        |items, state| Box::new(command_menu_bar(state, items, true)),
        |app: &mut App, event| {
            if let CommandEvent::Activate(path) = event {
                app.activations.push(path);
            }
        },
    );
    Box::new(el("div", bar).attr("style", "position:absolute;left:62px;top:8px;"))
}

fn host() -> TestHost {
    let mut host = Harness::with_hooks(
        Init {
            state: App::default(),
            logic: root as fn(&App) -> Child,
            sheet: format!("body {{ margin:0;font-size:13px; }} {COMMAND_MENU_BAR_CSS}"),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        inert_hooks(),
    );
    host.layout_at(320.0, 175.0);
    host
}

fn assert_bounded(host: &TestHost) {
    let menus = host.with_dom(|dom| dom.all_with_class(dom.document(), "command-menu-bar-menu"));
    assert_eq!(menus.len(), 1, "one menu level is mounted");
    let (x, y, w, h) = host.painted_rect(menus[0]).unwrap();
    assert!(
        x >= -0.5 && y >= -0.5 && x + w <= 320.5 && y + h <= 175.5,
        "menu outside viewport: ({x}, {y}, {w}, {h})"
    );
    assert!(host.with_dom(|dom| {
        dom.all_with_class(dom.document(), "command-menu-bar-submenu")
            .is_empty()
    }));
}

#[test]
fn nested_compact_menu_and_back_stay_in_one_popup_at_320_by_175() {
    let mut host = host();
    assert!(host.click_on(&Selector::role("menuitem").containing("Menu")));
    host.relayout();
    assert_bounded(&host);
    let view_rows = host.with_dom(|dom| dom.all_with_class(dom.document(), "command-menu-bar-row"));
    assert!(
        host.click_on(&Selector::class("command-menu-bar-row").containing("View")),
        "rows {:?}",
        view_rows
            .iter()
            .map(|node| (node, host.painted_rect(*node), host.visible_rect(*node)))
            .collect::<Vec<_>>()
    );
    host.relayout();
    assert_bounded(&host);
    assert!(host.click_on(&Selector::class("command-menu-bar-row").containing("Reading")));
    host.relayout();
    assert_bounded(&host);
    assert!(host.click_on(&Selector::class("command-menu-bar-back")));
    host.relayout();
    assert_bounded(&host);
    assert!(host.click_on(&Selector::class("command-menu-bar-row").containing("Reading")));
    host.relayout();
    assert!(host.click_on(&Selector::class("command-menu-bar-row").containing("Preview")));
    assert_eq!(host.state().activations, [vec![0, 0, 0]]);
    host.relayout();
    assert!(host.with_dom(|dom| {
        dom.all_with_class(dom.document(), "command-menu-bar-menu")
            .is_empty()
    }));
}
