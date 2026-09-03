// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The Hocket UI as genet views (2026-07-08 concept; S2: real data).
//!
//! One screen: the pass-the-mic circle | the loop table | the transport.
//! Everything data-bearing derives from [`AppState`]'s `Session`; gestures
//! call the `AppState` methods, which commit real `Edit`s. The rail currently
//! shows the local performer only; peers arrive with the sync layer. Waveform
//! and meter drawing use host-owned chisel leaves.

use hocket_model::{PhraseId, PlaybackMode, TrackColor, TrackId};
use cambium::{
    AnyView, SelectState, GenetCtx, GenetElement, clickable, el, lens, select, text,
};

use crate::leaves::{METER_L, METER_R, layer_wave_key, wave_key};
use crate::state::AppState;

pub type Child = Box<dyn AnyView<AppState, (), GenetCtx, GenetElement>>;

/// A `<chisel-leaf>` block: carries only its key + box; the host renders the
/// registered leaf's Path-A commands into it (see [`crate::leaves`]).
fn chisel_leaf(key: u64, w: u32, h: u32) -> Child {
    Box::new(el("chisel-leaf", ()).attr("key", key.to_string()).attr(
        "style",
        format!("display: block; width: {w}px; height: {h}px"),
    ))
}

fn responsive_chisel_leaf(key: u64, height: u32) -> Child {
    Box::new(
        el("chisel-leaf", ())
            .attr("key", key.to_string())
            .attr("aria-hidden", "true")
            .attr(
                "style",
                format!("display: block; width: 100%; height: {height}px"),
            ),
    )
}

fn wave_leaf(track: TrackId) -> Child {
    responsive_chisel_leaf(wave_key(track), 40)
}

fn layer_wave_leaf(track: TrackId, phrase: PhraseId) -> Child {
    responsive_chisel_leaf(layer_wave_key(track, phrase), 11)
}

fn hex(c: TrackColor) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

pub fn root(state: &AppState) -> Child {
    Box::new(el("div", (top(state), body(state), transport(state))).attr("class", "app"))
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
    Box::new(el("div", (rail(state), table(state))).attr("class", "body"))
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

fn table(state: &AppState) -> Child {
    let mut kids: Vec<Child> = vec![Box::new(
        el(
            "div",
            (
                el("div", text("loops")).attr("class", "eyebrow"),
                el("div", text("tap a dot to arm \u{00b7} tap a layer to mute"))
                    .attr("class", "table-hint"),
            ),
        )
        .attr("class", "table-head"),
    )];
    for i in 0..state.session.tracks.len() {
        kids.push(lane(state, i));
    }
    kids.push(Box::new(clickable(
        el("div", text("+ add track"))
            .attr("class", "add-track")
            .attr("role", "button"),
        |state: &mut AppState, _| state.add_track(),
    )));
    Box::new(el("div", kids).attr("class", "table"))
}

fn lane(state: &AppState, i: usize) -> Child {
    let track = &state.session.tracks[i];
    let recording = track.armed && state.is_recording();
    let mut cls = String::from("lane");
    if recording {
        cls.push_str(" lane-rec");
    } else if track.armed {
        cls.push_str(" lane-armed");
    }
    let style = format!("--voice: {}", hex(track.color));
    let owner = "you";
    let n = track.layers.len();

    let id = el(
        "div",
        (
            el(
                "div",
                (
                    clickable(
                        el("div", ())
                            .attr("class", "arm")
                            .attr("role", "switch")
                            .attr("aria-checked", if track.armed { "true" } else { "false" })
                            .attr("aria-label", format!("Arm {}", track.name)),
                        move |state: &mut AppState, _| {
                            state.arm(i);
                        },
                    ),
                    el("span", text(track.name.clone())).attr("class", "lane-title"),
                ),
            )
            .attr("class", "lane-name"),
            el(
                "div",
                (
                    el("span", text(owner.to_string())).attr("class", "tag tag-voice"),
                    el(
                        "span",
                        text(if n == 0 {
                            "empty".to_string()
                        } else {
                            format!("{n} layers")
                        }),
                    )
                    .attr("class", "tag mono"),
                ),
            )
            .attr("class", "lane-meta"),
        ),
    )
    .attr("class", "lane-id");

    let wavecol: Child = if n == 0 {
        Box::new(
            el(
                "div",
                el(
                    "span",
                    text("no layers yet \u{00b7} arm and record to start the loop"),
                )
                .attr("class", "wave-empty"),
            )
            .attr("class", "lane-wave"),
        )
    } else {
        // Newest layer on top: display index li walks the stack from the end.
        let layer_rows: Vec<Child> = (0..n)
            .rev()
            .map(|li| {
                let muted = track.layers[li].muted;
                let phrase_id = track.layers[li].phrase_id;
                let cls = if muted { "layer layer-muted" } else { "layer" };
                let waveform: Child = if state.layer_waveform_available(i, li) {
                    layer_wave_leaf(track.id, phrase_id)
                } else {
                    Box::new(
                        el("span", text("media unavailable")).attr("class", "wave-unavailable"),
                    )
                };
                Box::new(clickable(
                    el(
                        "div",
                        (
                            el("span", text(format!("L{}", li + 1))).attr("class", "lnum mono"),
                            el("div", waveform).attr("class", "layer-wave"),
                        ),
                    )
                    .attr("class", cls)
                    .attr("role", "switch")
                    .attr("aria-checked", if muted { "true" } else { "false" })
                    .attr("aria-label", format!("Mute layer L{}", li + 1)),
                    move |state: &mut AppState, _| {
                        state.toggle_layer_mute(i, li as u16);
                    },
                )) as Child
            })
            .collect();
        let summary: Child = if state.track_waveform_available(i) {
            wave_leaf(track.id)
        } else {
            Box::new(
                el(
                    "span",
                    text(if state.track_has_audible_layers(i) {
                        "media unavailable"
                    } else {
                        "all layers muted"
                    }),
                )
                .attr("class", "wave-unavailable"),
            )
        };
        Box::new(
            el(
                "div",
                (summary, el("div", layer_rows).attr("class", "layers")),
            )
            .attr("class", "lane-wave"),
        )
    };

    let m_cls = if track.muted { "lctl lctl-on" } else { "lctl" };
    let s_cls = if state.solo.contains(&track.id) {
        "lctl lctl-on"
    } else {
        "lctl"
    };
    let ctl = el(
        "div",
        (
            clickable(
                el("div", text("M"))
                    .attr("class", m_cls)
                    .attr("role", "switch")
                    .attr("aria-checked", if track.muted { "true" } else { "false" })
                    .attr("aria-label", format!("Mute {}", track.name)),
                move |state: &mut AppState, _| state.toggle_track_mute(i),
            ),
            clickable(
                el("div", text("S"))
                    .attr("class", s_cls)
                    .attr("role", "switch")
                    .attr(
                        "aria-checked",
                        if state.solo.contains(&track.id) {
                            "true"
                        } else {
                            "false"
                        },
                    )
                    .attr("aria-label", format!("Solo {}", track.name)),
                move |state: &mut AppState, _| state.toggle_solo(i),
            ),
        ),
    )
    .attr("class", "lane-ctl");

    Box::new(
        el("div", (id, wavecol, ctl))
            .attr("class", cls)
            .attr("style", style),
    )
}

fn transport(state: &AppState) -> Child {
    let rec_cls = if state.is_recording() {
        "record record-armed"
    } else {
        "record"
    };
    let bpm = format!("{}", state.session.bpm.round() as u32);
    let ts = state.session.time_signature;
    let meter_txt = format!("{}/{}", ts.numerator, ts.denominator);
    let rec_label = match state.armed_index() {
        Some(i) => format!(" \u{00b7} {} armed", state.session.tracks[i].name),
        None => " \u{00b7} nothing armed".to_string(),
    };
    let click_cls = if state.click {
        "toggle toggle-on"
    } else {
        "toggle"
    };
    let clock_cls = if state.session.master_clock_enabled {
        "toggle toggle-on"
    } else {
        "toggle"
    };

    let left = el(
        "div",
        (
            el(
                "div",
                (
                    el("div", text("tempo")).attr("class", "eyebrow"),
                    el(
                        "div",
                        (
                            clickable(
                                el("div", text("\u{2212}"))
                                    .attr("class", "step")
                                    .attr("role", "button")
                                    .attr("aria-label", "Decrease tempo"),
                                |state: &mut AppState, _| state.bpm_nudge(-2.0),
                            ),
                            el("span", text(bpm)).attr("class", "step-val mono"),
                            clickable(
                                el("div", text("+"))
                                    .attr("class", "step")
                                    .attr("role", "button")
                                    .attr("aria-label", "Increase tempo"),
                                |state: &mut AppState, _| state.bpm_nudge(2.0),
                            ),
                        ),
                    )
                    .attr("class", "stepper"),
                ),
            )
            .attr("class", "readout"),
            el(
                "div",
                (
                    el("div", text("meter")).attr("class", "eyebrow"),
                    el("div", text(meter_txt)).attr("class", "readout-val mono"),
                ),
            )
            .attr("class", "readout"),
            el(
                "div",
                (
                    clickable(
                        el(
                            "div",
                            (
                                el("span", ()).attr("class", "led"),
                                el("span", text("Click")),
                            ),
                        )
                        .attr("class", click_cls)
                        .attr("role", "switch")
                        .attr("aria-checked", if state.click { "true" } else { "false" })
                        .attr("aria-label", "Click track"),
                        |state: &mut AppState, _| state.toggle_click(),
                    ),
                    clickable(
                        el(
                            "div",
                            (
                                el("span", ()).attr("class", "led"),
                                el("span", text("Master clock")),
                            ),
                        )
                        .attr("class", clock_cls)
                        .attr("role", "switch")
                        .attr(
                            "aria-checked",
                            if state.session.master_clock_enabled {
                                "true"
                            } else {
                                "false"
                            },
                        )
                        .attr("aria-label", "Master clock"),
                        |state: &mut AppState, _| state.toggle_master_clock(),
                    ),
                ),
            )
            .attr("class", "toggles"),
        ),
    )
    .attr("class", "t-left");

    let center = el(
        "div",
        (
            clickable(
                el("div", el("div", ()).attr("class", "record-core"))
                    .attr("class", rec_cls)
                    .attr("role", "switch")
                    .attr(
                        "aria-checked",
                        if state.is_recording() {
                            "true"
                        } else {
                            "false"
                        },
                    )
                    .attr("aria-label", "Record"),
                |state: &mut AppState, _| state.toggle_record(),
            ),
            el(
                "div",
                (el("b", text("Record")), el("span", text(rec_label))),
            )
            .attr("class", "record-label"),
        ),
    )
    .attr("class", "record-wrap");

    let right = el(
        "div",
        el(
            "div",
            (
                audio_device_controls(state),
                clickable(
                    el("div", text("\u{25a0}"))
                        .attr("class", "stop")
                        .attr("role", "button")
                        .attr("aria-label", "Stop all loops"),
                    |state: &mut AppState, _| state.stop_all(),
                ),
                meter(state),
            ),
        )
        .attr("class", "t-right-inner"),
    )
    .attr("class", "t-right");

    Box::new(el("div", (left, center, right)).attr("class", "transport"))
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

fn meter(state: &AppState) -> Child {
    // The L/R output bars are chisel `Meter` leaves (host-owned).
    let col = |key: u64, label: &str| -> Child {
        Box::new(
            el(
                "div",
                (
                    chisel_leaf(key, 10, 46),
                    el("span", text(label.to_string())).attr("class", "mlbl mono"),
                ),
            )
            .attr("class", "mcol"),
        )
    };
    let peak = state.meter_db(0).max(state.meter_db(1));
    let peak_text = if peak.is_finite() {
        format!("{peak:.1} dB")
    } else {
        "-- dB".to_string()
    };
    Box::new(
        el(
            "div",
            (
                el("div", (col(METER_L, "L"), col(METER_R, "R"))).attr("class", "mbars"),
                el(
                    "div",
                    (
                        el("div", text("out")).attr("class", "eyebrow"),
                        el("div", text(peak_text)).attr("class", "mpeak mono"),
                    ),
                )
                .attr("class", "readout"),
            ),
        )
        .attr("class", "meter"),
    )
}
