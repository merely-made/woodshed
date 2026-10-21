// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Shared titlebar slots and native caption commands exercised through the
//! real retained host's layout, frame-aware pointer routing and keyboard path.

use cambium::{TITLE_BAR_CSS, TitleBarSlot, button, el, title_bar};
use cambium_genet_winit_host::{
    AppRegion, CaptionLabels, Harness, HostOptions, WindowCommand, WindowCommands,
    platform_caption_controls, window_caption_controls,
};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};
use taproot::Selector;
use winit::keyboard::NamedKey;

struct App {
    window: WindowCommands,
    labels: CaptionLabels,
    save_clicks: usize,
    platform_policy: bool,
}

type Child = TitleBarSlot<App>;
type Logic = fn(&App) -> Child;
type TestHost = Harness<App, Logic, Child>;

fn root(state: &App) -> Child {
    let captions = if state.platform_policy {
        platform_caption_controls(&state.window, &state.labels)
    } else {
        window_caption_controls(&state.window, &state.labels)
    };
    Box::new(
        el(
            "div",
            (
                title_bar(
                    Box::new(el("span", "T").attr("id", "ornament")),
                    Box::new(el("span", "Test title").attr("id", "title")),
                    Box::new(
                        button("Save", |state: &mut App, _| state.save_clicks += 1)
                            .attr("aria-label", "Save"),
                    ),
                    captions,
                ),
                el("div", "Body").attr("id", "body"),
            ),
        )
        .attr("id", "fixture-root"),
    )
}

fn harness(extra_css: &str) -> TestHost {
    let sheet = format!(
        "{TITLE_BAR_CSS}\n\
         #fixture-root {{ width:600px; height:250px; }}\n\
         #ornament {{ display:block; width:24px; height:24px; }}\n\
         #title {{ display:block; height:24px; }}\n\
         .title-bar-actions button {{ width:60px; height:30px; }}\n\
         .title-bar-captions button {{ width:40px; height:30px; }}\n\
         #body {{ height:160px; }}\n\
         {extra_css}"
    );
    let mut host = Harness::with_commands(sheet, root as Logic, |commands| App {
        window: commands.clone(),
        labels: CaptionLabels::default(),
        save_clicks: 0,
        platform_policy: false,
    });
    host.layout_at(600.0, 250.0);
    host
}

fn node_with_attr(dom: &ScriptedDom, node: NodeId, attribute: &str, value: &str) -> Option<NodeId> {
    if dom.attribute(node, &Namespace::from(""), &LocalName::from(attribute)) == Some(value) {
        return Some(node);
    }
    dom.dom_children(node)
        .find_map(|child| node_with_attr(dom, child, attribute, value))
}

fn node(host: &TestHost, attribute: &str, value: &str) -> NodeId {
    let dom = host.runner().dom();
    let dom = dom.borrow();
    node_with_attr(&dom, host.runner().root(), attribute, value)
        .unwrap_or_else(|| panic!("missing {attribute}={value}"))
}

fn center(host: &TestHost, attribute: &str, value: &str) -> (f32, f32) {
    let (x, y, width, height) = host.painted_rect(node(host, attribute, value)).unwrap();
    assert!(width > 0.0 && height > 0.0);
    (x + width / 2.0, y + height / 2.0)
}

fn caption(action: &str) -> Selector {
    Selector::role("button").with_attr("data-window-action", action)
}

#[test]
fn slot_content_keeps_drag_regions_and_controls_carve_out_no_drag() {
    let mut host = harness("");
    for id in ["ornament", "title"] {
        let (x, y) = center(&host, "id", id);
        assert_eq!(host.app_region_at(x, y), AppRegion::Drag);
    }
    let (x, y) = center(&host, "id", "title");
    host.press_at(x, y);
    host.release_at(x, y);
    assert_eq!(host.performed(), &[WindowCommand::Drag]);
    let (x, y) = host
        .resolve(&Selector::role("button").containing("Save"))
        .unwrap();
    assert_eq!(host.app_region_at(x, y), AppRegion::NoDrag);
    assert!(host.click_on(&Selector::role("button").containing("Save")));
    assert_eq!(host.state().save_clicks, 1);
    assert_eq!(host.performed(), &[WindowCommand::Drag]);
    for action in ["minimize", "maximize", "close"] {
        let (x, y) = host.resolve(&caption(action)).unwrap();
        assert_eq!(host.app_region_at(x, y), AppRegion::NoDrag);
    }
    let (x, y) = center(&host, "id", "body");
    assert_eq!(host.app_region_at(x, y), AppRegion::NoDrag);
}

#[test]
fn caption_clicks_use_the_host_queue_without_dragging_the_frame() {
    let mut host = harness("");
    for action in ["minimize", "maximize", "close"] {
        assert!(host.click_on(&caption(action)), "{action}");
    }
    assert_eq!(
        host.performed(),
        &[
            WindowCommand::Minimize,
            WindowCommand::ToggleMaximize,
            WindowCommand::Close,
        ]
    );
    assert!(
        host.commands().is_empty(),
        "the real host drained the shared queue"
    );
}

#[test]
fn tab_enter_and_space_activate_existing_caption_buttons_in_order() {
    let mut host = harness("");
    host.key_named(NamedKey::Tab);
    assert_eq!(host.focus(), Some(node(&host, "aria-label", "Save")));
    host.key_named(NamedKey::Tab);
    assert_eq!(
        host.focus(),
        Some(node(&host, "data-window-action", "minimize"))
    );
    host.key_named(NamedKey::Enter);
    host.key_named(NamedKey::Tab);
    assert_eq!(
        host.focus(),
        Some(node(&host, "data-window-action", "maximize"))
    );
    host.key_named(NamedKey::Space);
    host.key_named(NamedKey::Tab);
    assert_eq!(
        host.focus(),
        Some(node(&host, "data-window-action", "close"))
    );
    host.key_named(NamedKey::Enter);
    assert_eq!(
        host.performed(),
        &[
            WindowCommand::Minimize,
            WindowCommand::ToggleMaximize,
            WindowCommand::Close,
        ]
    );
    assert_eq!(host.state().save_clicks, 0);
}

#[test]
fn native_caption_area_reserves_both_sides_and_its_minimum_height() {
    let host = harness(
        "#fixture-root { --titlebar-area-x:78px; --titlebar-area-width:400px; \
         --titlebar-area-height:64px; --titlebar-padding:12px; }",
    );
    let (_, _, width, height) = host
        .painted_rect(node(&host, "class", "cambium-title-bar"))
        .unwrap();
    assert!((width - 600.0).abs() < 0.1);
    assert!(
        height >= 64.0,
        "native caption-area height was reserved: {height}"
    );
    let (ornament_x, _, _, _) = host.painted_rect(node(&host, "id", "ornament")).unwrap();
    assert!(
        (ornament_x - 90.0).abs() < 0.1,
        "78px inset plus 12px padding: {ornament_x}"
    );
    let (close_x, _, close_width, _) = host
        .painted_rect(node(&host, "data-window-action", "close"))
        .unwrap();
    assert!(
        close_x + close_width <= 466.1,
        "right controls must end before 122px native reservation plus 12px padding"
    );
}

#[test]
fn caption_accessible_names_are_configurable_and_match_the_snap_contract() {
    let mut host = harness("");
    assert_eq!(
        host.state().labels.maximize,
        HostOptions::default().maximize_control_label
    );
    host.update(|state| {
        state.labels = CaptionLabels {
            minimize: "Reduce window".into(),
            maximize: "Expand window".into(),
            close: "Dismiss window".into(),
        }
    });
    host.relayout();
    for (action, label) in [
        ("minimize", "Reduce window"),
        ("maximize", "Expand window"),
        ("close", "Dismiss window"),
    ] {
        assert_eq!(
            node(&host, "aria-label", label),
            node(&host, "data-window-action", action)
        );
    }
}

#[test]
fn platform_policy_keeps_native_mac_controls_and_adds_explicit_controls_elsewhere() {
    let mut host = harness("");
    host.update(|state| state.platform_policy = true);
    host.relayout();
    let dom = host.runner().dom();
    let dom = dom.borrow();
    let minimize = node_with_attr(&dom, host.runner().root(), "data-window-action", "minimize");
    assert_eq!(minimize.is_none(), cfg!(target_os = "macos"));
    assert!(host.performed().is_empty());
}
