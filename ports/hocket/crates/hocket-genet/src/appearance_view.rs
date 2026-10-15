// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Reachable application controls and the existing shared workshop surface.

use cambium::{PointerClick, button, el, lens, text};
use tabard_workshop::workshop_view;

use crate::appearance::PRODUCT_DEFAULT;
use crate::state::AppState;
use crate::view::Child;

pub const EDITOR_CSS: &str = "
.hocket-editor { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.hocket-editor > .tabard-workshop { flex: 1; min-height: 0; }
.hocket-editor-controls { display: flex; align-items: center; flex-wrap: wrap; gap: 12px; padding: 10px 18px; background: #fff; color: #253247; }
.hocket-editor-controls button { display: block; padding: 8px 12px; border: 1px solid currentColor; }
.hocket-editor-notice { display: block; padding: 8px 18px; margin: 0; background: #fff4d8; color: #493d25; }
";

pub const APPEARANCE_CSS: &str = "
.hocket-desktop { display:flex;flex-direction:column;height:100vh;min-height:0;overflow:hidden; background: var(--ground); color: var(--text); }
.hocket-desktop > .app { flex:1;height:auto;min-height:0; }
.hocket-appearance { max-height:35vh;overflow:scroll;flex-shrink:0; }

.hocket-appearance { display: block; padding: 14px 24px; background: var(--surface); color: var(--text); }
.hocket-appearance-options { display: flex; flex-wrap: wrap; gap: 8px; margin: 10px 0; }
.hocket-appearance-options button, .hocket-appearance-edit { display: block; padding: 8px 12px; border: 1px solid currentColor; background: transparent; color: inherit; }
.hocket-appearance-options button[aria-pressed=\"true\"] { background: var(--voice-amber); color: #17130e; }
.hocket-appearance-options button:focus-visible, .hocket-appearance-edit:focus-visible { box-shadow: inset 0 0 0 2px currentColor; }
.hocket-appearance-notice { display: block; line-height: 1.5; margin: 8px 0; }
.hocket-desktop > .cambium-title-bar { background: var(--surface); color: var(--text); --titlebar-min-height: 42px; }
.hocket-mark { display: block; font-weight: 700; padding: 4px 7px; border: 1px solid currentColor; }
.hocket-appearance-toggle { display: block; padding: 6px 10px; background: transparent; border: 1px solid currentColor; color: inherit; }
";

pub fn chrome(state: &AppState) -> Child {
    cambium::title_bar(
        Box::new(
            el("span", text("H"))
                .attr("class", "hocket-mark")
                .attr("aria-hidden", "true"),
        ),
        Box::new(el("span", text("Hocket"))),
        Box::new(
            button("Appearance", |state: &mut AppState, _: PointerClick| {
                state.appearance.open = !state.appearance.open;
            })
            .attr("class", "hocket-appearance-toggle")
            .attr("data-action", "toggle-appearance")
            .attr("aria-expanded", state.appearance.open.to_string()),
        ),
        cambium_genet_winit_host::platform_caption_controls(
            &state.window_commands,
            &cambium_genet_winit_host::CaptionLabels::default(),
        ),
    )
}

pub fn panel(state: &AppState) -> Child {
    let appearance = &state.appearance;
    let mut themes: Vec<Child> = vec![Box::new(
        button("Hocket warm", |state: &mut AppState, _: PointerClick| {
            state.appearance.notice = state.appearance.request_select(PRODUCT_DEFAULT, None).err();
        })
        .attr("data-appearance-theme", PRODUCT_DEFAULT)
        .attr(
            "aria-pressed",
            (appearance.active_id() == PRODUCT_DEFAULT).to_string(),
        ),
    )];
    themes.extend(
        appearance
            .workshop
            .registry()
            .list()
            .into_iter()
            .filter(|theme| theme.id != PRODUCT_DEFAULT)
            .map(|theme| {
                let id = theme.id.clone();
                Box::new(
                    button(
                        theme.name.clone(),
                        move |state: &mut AppState, _: PointerClick| {
                            state.appearance.notice =
                                state.appearance.request_select(&id, None).err();
                        },
                    )
                    .attr("data-appearance-theme", theme.id.clone())
                    .attr(
                        "aria-pressed",
                        (appearance.active_id() == theme.id).to_string(),
                    ),
                ) as Child
            }),
    );
    let modes: Vec<Child> = appearance
        .modes()
        .into_iter()
        .map(|mode| {
            let key = mode.as_key();
            let active = appearance
                .applied()
                .is_some_and(|resolved| resolved.resolved.theme_mode.as_ref() == Some(&mode));
            Box::new(
                button(
                    mode.label(),
                    move |state: &mut AppState, _: PointerClick| {
                        let id = state.appearance.active_id().to_owned();
                        state.appearance.notice = state
                            .appearance
                            .request_select(&id, Some(mode.clone()))
                            .err();
                    },
                )
                .attr("data-appearance-mode", key)
                .attr("aria-pressed", active.to_string()),
            ) as Child
        })
        .collect();
    let mut children: Vec<Child> = vec![
        Box::new(el("h2", text("Appearance"))),
        Box::new(
            el("div", themes)
                .attr("class", "hocket-appearance-options")
                .attr("aria-label", "Theme"),
        ),
        Box::new(
            el("div", modes)
                .attr("class", "hocket-appearance-options")
                .attr("aria-label", "Presentation mode"),
        ),
    ];
    children.extend(appearance.diagnostics().into_iter().map(|notice| {
        Box::new(
            el("p", text(notice))
                .attr("class", "hocket-appearance-notice")
                .attr("role", "status"),
        ) as Child
    }));
    if appearance.authoring_available {
        children.push(Box::new(
            button("Edit themes…", |state: &mut AppState, _: PointerClick| {
                state.appearance.notice = state.appearance.begin_edit().err();
            })
            .attr("class", "hocket-appearance-edit")
            .attr("data-action", "edit-appearance"),
        ));
    }
    Box::new(
        el("section", children)
            .attr("class", "hocket-appearance")
            .attr("aria-label", "Application appearance"),
    )
}

pub fn editor(state: &AppState) -> Child {
    let mut children: Vec<Child> = vec![Box::new(
        el(
            "div",
            (
                button("Back to Hocket", |state: &mut AppState, _: PointerClick| {
                    state.appearance.close_app = false;
                    if state.appearance.workshop.request_close() {
                        state.appearance.workshop.cancel_close();
                        state.appearance.editor_open = false;
                    }
                })
                .attr("data-action", "back-to-hocket"),
                button(
                    "Apply to Hocket",
                    |state: &mut AppState, _: PointerClick| {
                        state.appearance.notice = match state.appearance.apply_workshop() {
                            Ok(()) => None,
                            Err(error) => Some(error),
                        };
                    },
                )
                .attr("data-action", "apply-to-hocket"),
                el(
                    "p",
                    text("Preview and save your definition here, then apply it to Hocket."),
                ),
            ),
        )
        .attr("class", "hocket-editor-controls"),
    )];
    for notice in state.appearance.diagnostics() {
        children.push(Box::new(
            el("p", text(notice))
                .attr("class", "hocket-editor-notice")
                .attr("role", "status"),
        ));
    }
    children.push(Box::new(lens(
        |workshop: &mut tabard_workshop::WorkshopState| workshop_view(workshop),
        |state: &mut AppState| &mut state.appearance.workshop,
    )));
    Box::new(
        el("div", children)
            .attr("class", "hocket-editor")
            .attr("data-surface", "hocket.appearance-workshop.v1"),
    )
}

// Keep the lens's existing focus/input state; the native adapter resolves
// workshop fields through this same shared state, rather than adding widgets.
pub fn sync_editor(state: &mut AppState, exit_requested: bool) -> bool {
    if !state.appearance.editor_open {
        return false;
    }
    if exit_requested {
        if state.appearance.workshop.has_changes() {
            state.appearance.workshop.discard();
        }
        state.appearance.workshop.cancel_close();
        state.appearance.editor_open = false;
        return std::mem::take(&mut state.appearance.close_app);
    }
    if !state.appearance.workshop.close_requested() {
        state.appearance.close_app = false;
    }
    false
}
