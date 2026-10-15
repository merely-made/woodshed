// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The Hocket UI as genet views (2026-07-08 concept; S2: real data).
//!
//! One screen: the pass-the-mic circle | the loop table | the transport.
//! The shared recorder surface reads `AppState.recorder` and queues typed
//! actions for its portable controller. This host keeps the project toolbar,
//! local-performer rail, appearance and device controls. Actual waveforms and
//! meters use the shared recorder leaves in the host-owned registry.

use cambium::{AnyView, GenetCtx, GenetElement, SelectState, clickable, el, lens, select, text};
use hocket_model::PlaybackMode;
use hocket_recorder::RecorderAction;
use hocket_views::RecorderLayout;

use crate::leaves::STANDALONE_MOUNT;
use crate::state::AppState;

pub type Child = Box<dyn AnyView<AppState, (), GenetCtx, GenetElement>>;

pub fn root(state: &AppState) -> Child {
    if state.appearance.editor_open {
        return crate::appearance_view::editor(state);
    }
    let mut children = vec![crate::appearance_view::chrome(state)];
    if state.appearance.open {
        children.push(crate::appearance_view::panel(state));
    }
    children.push(Box::new(
        el(
            "div",
            (
                top(state),
                body(state),
                hocket_views::transport(
                    &state.recorder,
                    STANDALONE_MOUNT,
                    RecorderLayout::Full,
                    recorder_action,
                    Some(audio_device_controls(state)),
                ),
            ),
        )
        .attr("class", "app"),
    ));
    Box::new(el("div", children).attr("class", "hocket-desktop"))
}

/// The update state, shown only when there is something true to say.
///
/// Silent for every state the user cannot act on and is not waiting for:
/// `Idle`, `UpToDate`, `Disabled`, and `Unsupported`. A chip that always
/// shows something trains the eye to ignore it, and the point of this
/// indicator is that when it speaks, it is reporting a state the app is
/// genuinely in and the user might do something about.
///
/// `Unsupported` is silent because it is the *normal* state for a dev build
/// and for a package-managed install — a permanent "updates unavailable"
/// banner in the toolbar is noise, and a long one crowds the controls onto
/// two lines. `--update-now` still reports it in full when asked.
fn update_status_chip(state: &AppState) -> Child {
    use crate::update::UpdateStatus;
    let status = state.update_status();
    let worth_showing = !matches!(
        status,
        UpdateStatus::Idle
            | UpdateStatus::UpToDate { .. }
            | UpdateStatus::Disabled
            | UpdateStatus::Unsupported(_)
    );
    if !worth_showing {
        return Box::new(el("span", ()));
    }
    let summary = status.summary();
    match state.update_action_label() {
        // Actionable: the chip is the button. One affordance instead of
        // three toolbar buttons, in a row that is already full, and the
        // click always means "carry on from the state shown".
        Some(action) => Box::new(clickable(
            el("div", text(format!("{summary} · {action}")))
                .attr("class", "update-status update-action mono")
                .attr("role", "button")
                .attr("aria-live", "polite")
                .attr("aria-label", format!("{summary}. Click to {action}.")),
            |state: &mut AppState, _| state.advance_update(),
        )),
        None => Box::new(
            el("span", text(summary))
                .attr("class", "update-status mono")
                .attr("aria-live", "polite"),
        ),
    }
}

fn top(state: &AppState) -> Child {
    let profile = match state.session.default_playback_mode {
        PlaybackMode::Sum => "looper-pedal",
        PlaybackMode::SelectOne { .. } => "deeler",
    };
    let chip = format!("{} tracks \u{00b7} {}", state.session.tracks.len(), profile);
    let status = state.project_status_label();
    Box::new(
        el(
            "div",
            (
                el(
                    "span",
                    (
                        el("span", text("hocket")),
                        el("span", text(".")).attr("class", "brand-dot"),
                    ),
                )
                .attr("class", "brand"),
                el("span", text(state.project_label())).attr("class", "session-name mono"),
                el("span", text(status)).attr("class", "project-status mono"),
                update_status_chip(state),
                el("div", ()).attr("class", "top-spacer"),
                el("span", text(chip)).attr("class", "chip mono"),
                clickable(
                    el("div", text("Open"))
                        .attr("class", "project-command")
                        .attr("role", "button")
                        .attr("aria-label", "Open project"),
                    |state: &mut AppState, _| state.choose_project_to_open(),
                ),
                clickable(
                    el("div", text("Save"))
                        .attr("class", "project-command project-save")
                        .attr("role", "button")
                        .attr("aria-label", "Save project"),
                    |state: &mut AppState, _| state.choose_project_to_save(),
                ),
                export_length_control(state),
                clickable(
                    el("div", text("Export mix"))
                        .attr("class", "project-command")
                        .attr("role", "button")
                        .attr("aria-label", "Export current loop mix as WAV"),
                    |state: &mut AppState, _| state.choose_mix_export(),
                ),
                clickable(
                    el("div", text("Copy audio"))
                        .attr("class", "project-command")
                        .attr("role", "button")
                        .attr("aria-label", "Copy the loop mix to the clipboard as audio"),
                    |state: &mut AppState, _| state.copy_audio(),
                ),
                clickable(
                    el("div", text("Paste audio"))
                        .attr("class", "project-command")
                        .attr("role", "button")
                        .attr(
                            "aria-label",
                            "Paste clipboard audio as a new layer on the armed track",
                        ),
                    |state: &mut AppState, _| state.paste_audio(),
                ),
            ),
        )
        .attr("class", "top"),
    )
}

fn export_length_control(state: &AppState) -> Child {
    let cycle_class = if state.export_uses_bars() {
        "export-mode-choice"
    } else {
        "export-mode-choice export-mode-choice-on"
    };
    let bars_class = if state.export_uses_bars() {
        "export-mode-choice export-mode-choice-on"
    } else {
        "export-mode-choice"
    };
    let mut children: Vec<Child> = vec![
        Box::new(clickable(
            el("div", text("Cycle"))
                .attr("class", cycle_class)
                .attr("role", "button")
                .attr("aria-label", "Export one shared loop cycle")
                .attr("aria-pressed", (!state.export_uses_bars()).to_string()),
            |state: &mut AppState, _| state.export_one_cycle(),
        )),
        Box::new(clickable(
            el("div", text("Bars"))
                .attr("class", bars_class)
                .attr("role", "button")
                .attr("aria-label", "Export a selected number of bars")
                .attr("aria-pressed", state.export_uses_bars().to_string()),
            |state: &mut AppState, _| state.export_session_bars(),
        )),
    ];
    if let Some(bars) = state.export_bars() {
        children.extend([
            Box::new(clickable(
                el("div", text("-"))
                    .attr("class", "export-step")
                    .attr("role", "button")
                    .attr("aria-label", "Export one fewer bar"),
                |state: &mut AppState, _| state.adjust_export_bars(-1),
            )) as Child,
            Box::new(el("span", text(format!("{bars} bars"))).attr("class", "export-bars mono")),
            Box::new(clickable(
                el("div", text("+"))
                    .attr("class", "export-step")
                    .attr("role", "button")
                    .attr("aria-label", "Export one more bar"),
                |state: &mut AppState, _| state.adjust_export_bars(1),
            )) as Child,
        ]);
    }
    Box::new(el("div", children).attr("class", "export-length"))
}

fn body(state: &AppState) -> Child {
    Box::new(
        el(
            "div",
            (
                rail(state),
                hocket_views::table(
                    &state.recorder,
                    STANDALONE_MOUNT,
                    RecorderLayout::Full,
                    recorder_action,
                ),
            ),
        )
        .attr("class", "body"),
    )
}

fn peer(name: &str, initials: &str, voice: &str, sub: &str, turn: bool) -> Child {
    let cls = if turn { "peer peer-turn" } else { "peer" };
    let av = el("div", text(initials.to_string()))
        .attr("class", "av")
        .attr("style", format!("background-color: {voice}"));
    let who = el(
        "div",
        (
            el("span", text(name.to_string())).attr("class", "who-name"),
            el("span", text(sub.to_string())).attr("class", "who-sub"),
        ),
    )
    .attr("class", "who");
    let mut kids: Vec<Child> = vec![Box::new(av), Box::new(who)];
    if turn {
        kids.push(Box::new(
            el("span", text("\u{25cf}"))
                .attr("class", "mic")
                .attr("style", format!("color: {voice}")),
        ));
    }
    Box::new(el("div", kids).attr("class", cls))
}

/// The pass-the-mic rail shows the local performer until sharing exists.
fn rail(state: &AppState) -> Child {
    let you_sub = if state.is_recording() {
        "your turn \u{00b7} recording"
    } else {
        "your turn"
    };
    let amber = "#e0a64b";
    let session_note = if state.missing_media.is_empty() {
        state.identity_status_label()
    } else {
        format!("{} media blob(s) unavailable", state.missing_media.len())
    };
    let mut kids: Vec<Child> = vec![
        Box::new(el("div", text("the circle")).attr("class", "eyebrow")),
        peer("You", "YU", amber, you_sub, true),
        Box::new(el("div", text(session_note)).attr("class", "handoff-note")),
    ];
    // Whose identity this is: shared with the rest of the Merely family, or
    // Hocket's own. Beside the token rather than buried in a settings screen,
    // because it is the same question the token answers for a peer, and a
    // musician about to paste one deserves to know which person it names.
    kids.push(Box::new(
        el("div", text(state.identity_home_label()))
            .attr("class", "handoff-note")
            .attr("role", "status"),
    ));
    // The switch, staged. Offered only when there is a family persona to join;
    // the confirmation names the token that stops working, because that is the
    // thing the user has already handed to people.
    if let Some(staged) = state.persona_switch() {
        let short: String = staged.losing_token.chars().take(12).collect();
        kids.push(Box::new(
            el(
                "div",
                (
                    el(
                        "div",
                        text(format!("Speak as persona {}?", staged.profile)),
                    )
                    .attr("class", "handoff-note"),
                    el(
                        "div",
                        text(format!(
                            "Your contact token changes. The one you have shared ({short}...) stops reaching you, so peers holding it need the new one."
                        )),
                    )
                    .attr("class", "handoff-note"),
                    Box::new(clickable(
                        el("div", text("Switch and re-share"))
                            .attr("class", "project-command")
                            .attr("role", "button"),
                        |state: &mut AppState, _| state.confirm_persona_switch(),
                    )),
                    Box::new(clickable(
                        el("div", text("Keep this identity"))
                            .attr("class", "project-command")
                            .attr("role", "button"),
                        |state: &mut AppState, _| state.decline_persona_switch(),
                    )),
                ),
            )
            .attr("class", "handoff-review")
            .attr("role", "dialog")
            .attr("aria-label", "Confirm persona switch"),
        ));
    } else if state.family_to_join().is_some() {
        kids.push(Box::new(clickable(
            el("div", text("Use my shared persona"))
                .attr("class", "project-command")
                .attr("role", "button")
                .attr(
                    "aria-label",
                    "Speak as the persona shared with your other Merely apps",
                ),
            |state: &mut AppState, _| state.offer_persona_switch(),
        )));
    }
    // After a switch: the old token is dead and peers need the new one. A row
    // rather than a one-shot dialog, because it stays true until they act.
    if state.needs_reshare() {
        kids.push(Box::new(
            el(
                "div",
                text("Your contact token changed. Copy it and send it to anyone who hands off to you."),
            )
            .attr("class", "handoff-note")
            .attr("role", "status"),
        ));
    }
    // Your contact token: a peer needs it to address a hand-off here. The rail
    // keeps the short fingerprint visible; the full 64-char token goes to the
    // clipboard on demand rather than cluttering the circle.
    if state.contact_token().is_some() {
        kids.push(Box::new(clickable(
            el("div", text("Copy token"))
                .attr("class", "project-command")
                .attr("role", "button")
                .attr("aria-label", "Copy your contact token to the clipboard"),
            |state: &mut AppState, _| state.copy_contact_token(),
        )));
    }
    // Address a hand-off: paste a peer's token, then send. Replies are
    // pre-addressed after accepting one, so only first contact needs a paste.
    kids.push(Box::new(clickable(
        el("div", text("Paste recipient"))
            .attr("class", "project-command")
            .attr("role", "button")
            .attr(
                "aria-label",
                "Set the hand-off recipient from a token on the clipboard",
            ),
        |state: &mut AppState, _| state.paste_recipient(),
    )));
    if let Some(fingerprint) = state.recipient_fingerprint() {
        kids.push(Box::new(
            el("div", text(format!("handing to {fingerprint}"))).attr("class", "handoff-note"),
        ));
        if state.can_hand_off() {
            kids.push(Box::new(clickable(
                el("div", text("Hand off"))
                    .attr("class", "project-command project-save")
                    .attr("role", "button")
                    .attr("aria-label", "Write a signed hand-off for the recipient"),
                |state: &mut AppState, _| state.hand_off(),
            )));
        }
    }
    // Receive a hand-off someone addressed to you.
    kids.push(Box::new(clickable(
        el("div", text("Receive"))
            .attr("class", "project-command")
            .attr("role", "button")
            .attr("aria-label", "Open a hand-off file addressed to you"),
        |state: &mut AppState, _| state.choose_handoff_to_open(),
    )));
    // A staged incoming hand-off: show who and what before it touches the
    // session, with Accept (branch or adopt) and Discard.
    if state.has_incoming() {
        let sender = state.incoming_sender_fingerprint().unwrap_or_default();
        let summary = state.incoming_summary().unwrap_or_default();
        kids.push(Box::new(
            el(
                "div",
                (
                    el("div", text(format!("from {sender}"))).attr("class", "eyebrow"),
                    el("div", text(summary)).attr("class", "handoff-note"),
                    el(
                        "div",
                        (
                            clickable(
                                el("div", text("Accept"))
                                    .attr("class", "project-command project-save")
                                    .attr("role", "button")
                                    .attr("aria-label", "Accept the staged hand-off"),
                                |state: &mut AppState, _| state.accept_incoming(),
                            ),
                            clickable(
                                el("div", text("Discard"))
                                    .attr("class", "project-command")
                                    .attr("role", "button")
                                    .attr("aria-label", "Discard the staged hand-off"),
                                |state: &mut AppState, _| state.discard_incoming(),
                            ),
                        ),
                    )
                    .attr("class", "handoff-card-actions"),
                ),
            )
            .attr("class", "handoff-card"),
        ));
    }
    Box::new(el("div", kids).attr("class", "rail"))
}

fn audio_device_controls(state: &AppState) -> Child {
    let input_labels = state.input_device_options();
    let output_labels = state.output_device_options();
    let input_select = lens(
        move |select_state: &mut SelectState| {
            let options: Vec<&str> = input_labels.iter().map(String::as_str).collect();
            select(select_state, &options)
        },
        |state: &mut AppState| &mut state.audio_input_select,
    );
    let output_select = lens(
        move |select_state: &mut SelectState| {
            let options: Vec<&str> = output_labels.iter().map(String::as_str).collect();
            select(select_state, &options)
        },
        |state: &mut AppState| &mut state.audio_output_select,
    );
    Box::new(
        el(
            "div",
            (
                el(
                    "div",
                    (
                        el("span", text("in")).attr("class", "device-label mono"),
                        input_select,
                    ),
                )
                .attr("class", "device-select"),
                el(
                    "div",
                    (
                        el("span", text("out")).attr("class", "device-label mono"),
                        output_select,
                    ),
                )
                .attr("class", "device-select"),
                el("span", text(state.audio_status_label().to_string()))
                    .attr("class", "audio-status mono"),
            ),
        )
        .attr("class", "audio-devices"),
    )
}

fn recorder_action(state: &mut AppState, action: RecorderAction) {
    state.recorder.request(action);
}
