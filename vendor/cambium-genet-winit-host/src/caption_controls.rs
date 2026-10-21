// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Native caption controls over the existing portable titlebar slots and
//! window-command queue. Platform framing remains the host's responsibility.

use cambium::{TitleBarSlot, button, el};

use crate::WindowCommands;

/// Accessible names for the three window controls.
///
/// `maximize` must equal [`crate::HostOptions::maximize_control_label`] so the
/// Windows Snap bridge finds the same retained maximize button by its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptionLabels {
    pub minimize: String,
    pub maximize: String,
    pub close: String,
}

impl Default for CaptionLabels {
    fn default() -> Self {
        Self {
            minimize: "Minimize".into(),
            maximize: "Maximize".into(),
            close: "Close".into(),
        }
    }
}

/// Three ordinary Cambium buttons sharing the host's window-command queue.
///
/// Hosts drain these commands through their existing window and close-policy
/// paths. The returned slot owns no native window handle and supplies no
/// product theme. It remains available on every platform for explicit custom
/// frames; use [`platform_caption_controls`] for the usual platform policy.
pub fn window_caption_controls<State: 'static>(
    commands: &WindowCommands,
    labels: &CaptionLabels,
) -> TitleBarSlot<State> {
    let minimize = commands.clone();
    let maximize = commands.clone();
    let close = commands.clone();
    let buttons: Vec<TitleBarSlot<State>> = vec![
        Box::new(
            button("—", move |_: &mut State, _| minimize.minimize())
                .attr("aria-label", labels.minimize.clone())
                .attr("data-window-action", "minimize"),
        ),
        Box::new(
            button("□", move |_: &mut State, _| maximize.toggle_maximize())
                .attr("aria-label", labels.maximize.clone())
                .attr("data-window-action", "maximize"),
        ),
        Box::new(
            button("×", move |_: &mut State, _| close.close())
                .attr("aria-label", labels.close.clone())
                .attr("data-window-action", "close"),
        ),
    ];
    Box::new(
        el("div", buttons)
            .attr("class", "window-caption-controls")
            .attr("style", "display: flex; --app-region: no-drag;"),
    )
}

/// The usual native frame policy: macOS retains its traffic lights and receives
/// an empty caption slot; other platforms receive the explicit controls.
pub fn platform_caption_controls<State: 'static>(
    commands: &WindowCommands,
    labels: &CaptionLabels,
) -> TitleBarSlot<State> {
    if cfg!(target_os = "macos") {
        Box::new(String::new())
    } else {
        window_caption_controls(commands, labels)
    }
}
