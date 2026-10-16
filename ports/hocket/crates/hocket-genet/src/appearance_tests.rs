// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! CPU acceptance through the production Cambium host's dispatch and layout.
use crate::{AppState, Child, Logic, Runtime, hooks};
use cambium_genet_winit_host::{CloseRequest, Harness, Init};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use taproot::Selector;

type Host = Harness<AppState, Logic, Child>;

fn mounted(dir: &tempfile::TempDir) -> Host {
    mounted_with_runtime(dir, Rc::new(RefCell::new(Runtime::new(None))))
}

pub(super) fn mounted_with_runtime(dir: &tempfile::TempDir, runtime: Rc<RefCell<Runtime>>) -> Host {
    let wake: armillary::Wake = Arc::new(|| {});
    let (project, _) = armillary::spawn::<
        crate::project_io::ProjectRequest,
        crate::project_io::ProjectReply,
        _,
    >(wake.clone(), |_, _| {});
    let (updates, _) = armillary::spawn::<
        crate::update::worker::UpdateCommand,
        crate::update::UpdateStatus,
        _,
    >(wake, |_, _| {});
    let mut state = AppState::new(
        project,
        updates,
        Default::default(),
        Err("pending identity in CPU appearance receipt".into()),
        true,
    );
    state.appearance = crate::appearance::AppearanceState::load(
        dir.path().join("choice.json"),
        dir.path().join("themes.json"),
        vec![],
    );
    let sheet = state.appearance.take_stylesheet_change().unwrap();
    let mut host = Harness::with_hooks(
        Init {
            state,
            logic: crate::view::root as Logic,
            sheet,
            fonts: vec![],
            images: vec![],
        },
        hooks(&runtime),
    );
    host.layout_at(1180.0, 700.0);
    host
}

fn click(host: &mut Host, action: &str) {
    assert!(
        host.click_on(&Selector::role("button").with_attr("data-action", action)),
        "{action}"
    );
}

#[test]
fn shared_workshop_text_dispatch_preserves_recorder_and_applies_only_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = mounted(&dir);
    let session = serde_json::to_value(&host.state().session).unwrap();
    let history_head = host.state().history.head;
    click(&mut host, "toggle-appearance");
    click(&mut host, "edit-appearance");
    assert!(host.state().appearance.editor_open);
    let name = host.state().appearance.workshop.draft_theme().name.clone();
    assert!(host.click_on(&Selector::role("textbox").with_attr("data-field", "name")));
    host.key_injected(" concert");
    assert_ne!(host.state().appearance.workshop.draft_theme().name, name);
    click(&mut host, "save");
    assert_eq!(
        host.state().appearance.active_id(),
        crate::appearance::PRODUCT_DEFAULT
    );
    click(&mut host, "apply-to-hocket");
    assert_ne!(
        host.state().appearance.active_id(),
        crate::appearance::PRODUCT_DEFAULT
    );
    click(&mut host, "back-to-hocket");
    assert!(!host.state().appearance.editor_open);
    assert_eq!(
        serde_json::to_value(&host.state().session).unwrap(),
        session
    );
    assert_eq!(host.state().history.head, history_head);
}

#[test]
fn cancelled_native_close_then_embedded_discard_does_not_exit_hocket() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = mounted(&dir);
    click(&mut host, "toggle-appearance");
    click(&mut host, "edit-appearance");
    host.request_close(CloseRequest::Native);
    host.relayout();
    assert!(!host.close_requested());
    assert!(host.state().appearance.close_app);
    click(&mut host, "cancel-close");
    assert!(!host.state().appearance.close_app);
    click(&mut host, "back-to-hocket");
    click(&mut host, "discard-close");
    assert!(!host.close_requested());
    assert!(!host.state().appearance.editor_open);
}

#[test]
fn shared_fields_keep_usable_height_and_titlebar_controls_stay_inside_the_viewport() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = mounted(&dir);
    for (width, height) in [(420.0, 1000.0), (1180.0, 700.0)] {
        host.layout_at(width, height);
        let toggle = host.with_dom(|dom| {
            taproot::matching(
                dom,
                &Selector::role("button").with_attr("data-action", "toggle-appearance"),
            )[0]
        });
        let (x, y, w, h) = host.painted_rect(toggle).unwrap();
        assert!(x >= 0.0 && y >= 0.0 && w > 0.0 && h >= 24.0 && x + w <= width + 0.1);
    }
    host.layout_at(1180.0, 700.0);
    click(&mut host, "toggle-appearance");
    click(&mut host, "edit-appearance");
    for (width, height) in [(420.0, 1000.0), (1180.0, 700.0)] {
        host.layout_at(width, height);
        for field in ["name", "seed-hex"] {
            let node = host.with_dom(|dom| {
                taproot::matching(
                    dom,
                    &Selector::role("textbox").with_attr("data-field", field),
                )[0]
            });
            let (_, _, w, h) = host.painted_rect(node).unwrap();
            assert!(w >= 40.0 && h >= 33.9, "{field} {width}px: {w}x{h}");
        }
    }
}

#[test]
fn save_confirmation_on_native_close_saves_definition_without_applying_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = mounted(&dir);
    click(&mut host, "toggle-appearance");
    click(&mut host, "edit-appearance");
    host.request_close(CloseRequest::Native);
    host.relayout();
    click(&mut host, "save-close");
    assert!(host.close_requested());
    assert!(dir.path().join("themes.json").exists());
    assert!(!dir.path().join("choice.json").exists());
}

#[test]
fn saved_same_identity_reopens_for_apply_without_changing_installed_appearance() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = mounted(&dir);
    click(&mut host, "toggle-appearance");
    click(&mut host, "edit-appearance");
    click(&mut host, "save");
    click(&mut host, "apply-to-hocket");
    click(&mut host, "back-to-hocket");
    let id = host.state().appearance.active_id().to_string();
    let installed = host.state().appearance.stylesheet();
    click(&mut host, "edit-appearance");
    host.update(|state| {
        *state
            .appearance
            .workshop
            .text_field_mut("mode-sheet")
            .unwrap() = cambium::TextInput::new(".app { background:#123456; }");
        state.appearance.workshop.apply_stylesheet();
    });
    click(&mut host, "save");
    click(&mut host, "back-to-hocket");
    assert_eq!(host.state().appearance.stylesheet(), installed);
    click(&mut host, "edit-appearance");
    assert_eq!(host.state().appearance.workshop.draft_theme().id, id);
    assert!(!host.state().appearance.workshop.has_changes());
    click(&mut host, "apply-to-hocket");
    click(&mut host, "back-to-hocket");
    assert_eq!(host.state().appearance.active_id(), id);
    assert!(
        host.state()
            .appearance
            .stylesheet()
            .ends_with(".app { background:#123456; }")
    );
    assert!(!host.close_requested());
}

#[test]
fn canonical_mode_changes_preserve_the_visible_record_dot() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = mounted(&dir);
    let dot = |host: &Host| {
        host.with_dom(|dom| taproot::matching(dom, &Selector::class("hocket-recorder-record-core"))[0])
    };
    let baseline = host.computed_value(dot(&host), "background-color").unwrap();
    click(&mut host, "toggle-appearance");
    assert!(
        host.click_on(
            &Selector::role("button").with_attr("data-appearance-theme", "theme:default")
        )
    );
    for mode in ["light", "dark", "hc_light", "hc_dark"] {
        assert!(
            host.click_on(&Selector::role("button").with_attr("data-appearance-mode", mode)),
            "{mode}"
        );
        let node = dot(&host);
        let (_, _, width, height) = host.painted_rect(node).unwrap();
        assert!(
            width >= 35.9 && height >= 35.9,
            "{mode}: record dot {width}x{height}"
        );
        assert_eq!(
            host.computed_value(node, "background-color"),
            Some(baseline.clone()),
            "{mode}"
        );
    }
}
