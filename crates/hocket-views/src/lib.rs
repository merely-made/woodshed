// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Reusable loop recorder controls over the portable recorder controller.
//! Hosts provide typed action routing and optionally their own transport
//! accessory. Project, identity, appearance chrome and device selection stay
//! outside this surface. Leaf keys include an explicit mount identity.

use cambium::{AnyView, GenetCtx, GenetElement, clickable, el, text};
use hocket_model::{PhraseId, TrackColor, TrackId};
use hocket_recorder::{Recorder, RecorderAction};

pub mod leaves;
pub mod theme;

pub use leaves::{RecorderLeaves, RecorderMount};

pub type Child<S> = Box<dyn AnyView<S, (), GenetCtx, GenetElement>>;
pub type Dispatch<S> = fn(&mut S, RecorderAction);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecorderLayout {
    #[default]
    Full,
    Compact,
}

impl RecorderLayout {
    fn class(self) -> &'static str {
        match self {
            Self::Full => "hocket-recorder-full",
            Self::Compact => "hocket-recorder-compact",
        }
    }
}

/// A complete recorder mount. Both sizes share the same controls and actions.
/// Callers that supply standalone chrome can instead compose table/transport.
pub fn surface<S: 'static>(
    state: &Recorder,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: Dispatch<S>,
) -> Child<S> {
    Box::new(
        el(
            "div",
            (
                table(state, mount, layout, dispatch),
                transport(state, mount, layout, dispatch, None),
            ),
        )
        .attr(
            "class",
            format!("hocket-recorder hocket-recorder-surface {}", layout.class()),
        )
        .attr("data-recorder-mount", mount.id().to_string()),
    )
}

fn chisel_leaf<S: 'static>(key: u64, w: u32, h: u32) -> Child<S> {
    Box::new(
        el("chisel-leaf", ())
            .attr("class", "hocket-recorder-meter-leaf")
            .attr("key", key.to_string())
            .attr(
                "style",
                format!("display: block; width: {w}px; height: {h}px"),
            ),
    )
}

fn responsive_chisel_leaf<S: 'static>(key: u64, height: u32) -> Child<S> {
    Box::new(
        el("chisel-leaf", ())
            .attr("class", "hocket-recorder-wave-leaf")
            .attr("key", key.to_string())
            .attr("aria-hidden", "true")
            .attr(
                "style",
                format!("display: block; width: 100%; height: {height}px"),
            ),
    )
}

fn wave_leaf<S: 'static>(mount: RecorderMount, track: TrackId) -> Child<S> {
    responsive_chisel_leaf(mount.wave_key(track), 40)
}

fn layer_wave_leaf<S: 'static>(
    mount: RecorderMount,
    track: TrackId,
    layer_index: usize,
    phrase: PhraseId,
) -> Child<S> {
    responsive_chisel_leaf(mount.layer_wave_key(track, layer_index, phrase), 11)
}

fn hex(c: TrackColor) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

pub fn table<S: 'static>(
    state: &Recorder,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: Dispatch<S>,
) -> Child<S> {
    let mut kids: Vec<Child<S>> = vec![Box::new(
        el(
            "div",
            (
                el("div", text("loops")).attr("class", "hocket-recorder-eyebrow"),
                el("div", text("tap a dot to arm \u{00b7} tap a layer to mute"))
                    .attr("class", "hocket-recorder-table-hint"),
            ),
        )
        .attr("class", "hocket-recorder-table-head"),
    )];
    for i in 0..state.session.tracks.len() {
        kids.push(lane(state, i, mount, dispatch));
    }
    kids.push(Box::new(clickable(
        el("div", text("+ add track"))
            .attr(
                "class",
                format!(
                    "{} hocket-recorder-mount-{}",
                    "hocket-recorder-add-track",
                    mount.id()
                ),
            )
            .attr("role", "button")
            .attr("data-recorder-mount", mount.id().to_string()),
        move |state: &mut S, _| dispatch(state, RecorderAction::AddTrack),
    )));
    Box::new(el("div", kids).attr(
        "class",
        format!("hocket-recorder hocket-recorder-table {}", layout.class()),
    ))
}

fn lane<S: 'static>(
    state: &Recorder,
    i: usize,
    mount: RecorderMount,
    dispatch: Dispatch<S>,
) -> Child<S> {
    let track = &state.session.tracks[i];
    let track_id = track.id;
    let recording = track.armed && state.is_recording();
    let mut cls = String::from("hocket-recorder-lane");
    if recording {
        cls.push_str(" hocket-recorder-lane-rec");
    } else if track.armed {
        cls.push_str(" hocket-recorder-lane-armed");
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
                            .attr(
                                "class",
                                format!(
                                    "{} hocket-recorder-mount-{}",
                                    "hocket-recorder-arm",
                                    mount.id()
                                ),
                            )
                            .attr("role", "switch")
                            .attr("data-recorder-mount", mount.id().to_string())
                            .attr("data-recorder-track", track_id.0.to_string())
                            .attr("aria-checked", if track.armed { "true" } else { "false" })
                            .attr("aria-label", format!("Arm {}", track.name)),
                        move |state: &mut S, _| {
                            dispatch(state, RecorderAction::Arm(track_id));
                        },
                    ),
                    el("span", text(track.name.clone()))
                        .attr("class", "hocket-recorder-lane-title"),
                ),
            )
            .attr("class", "hocket-recorder-lane-name"),
            el(
                "div",
                (
                    el("span", text(owner.to_string()))
                        .attr("class", "hocket-recorder-tag hocket-recorder-tag-voice"),
                    el(
                        "span",
                        text(if n == 0 {
                            "empty".to_string()
                        } else {
                            format!("{n} layers")
                        }),
                    )
                    .attr("class", "hocket-recorder-tag hocket-recorder-mono"),
                ),
            )
            .attr("class", "hocket-recorder-lane-meta"),
        ),
    )
    .attr("class", "hocket-recorder-lane-id");

    let wavecol: Child<S> = if n == 0 {
        Box::new(
            el(
                "div",
                el(
                    "span",
                    text("no layers yet \u{00b7} arm and record to start the loop"),
                )
                .attr("class", "hocket-recorder-wave-empty"),
            )
            .attr("class", "hocket-recorder-lane-wave"),
        )
    } else {
        // Newest layer on top: display index li walks the stack from the end.
        let layer_rows: Vec<Child<S>> = (0..n)
            .rev()
            .map(|li| {
                let muted = track.layers[li].muted;
                let phrase_id = track.layers[li].phrase_id;
                let layer_index = u16::try_from(li).ok();
                let cls = if muted {
                    "hocket-recorder-layer hocket-recorder-layer-muted"
                } else {
                    "hocket-recorder-layer"
                };
                let waveform: Child<S> = if state.layer_waveform_available(i, li) {
                    layer_wave_leaf(mount, track.id, li, phrase_id)
                } else {
                    Box::new(
                        el("span", text("media unavailable"))
                            .attr("class", "hocket-recorder-wave-unavailable"),
                    )
                };
                Box::new(clickable(
                    el(
                        "div",
                        (
                            el("span", text(format!("L{}", li + 1)))
                                .attr("class", "hocket-recorder-lnum hocket-recorder-mono"),
                            el("div", waveform).attr("class", "hocket-recorder-layer-wave"),
                        ),
                    )
                    .attr(
                        "class",
                        format!("{} hocket-recorder-mount-{}", cls, mount.id()),
                    )
                    .attr("role", "switch")
                    .attr("data-recorder-mount", mount.id().to_string())
                    .attr("data-recorder-track", track_id.0.to_string())
                    .attr("aria-checked", if muted { "true" } else { "false" })
                    .attr("aria-disabled", layer_index.is_none().to_string())
                    .attr("data-recorder-layer", li.to_string())
                    .attr("aria-label", format!("Mute layer L{}", li + 1)),
                    move |state: &mut S, _| {
                        if let Some(layer_index) = layer_index {
                            dispatch(
                                state,
                                RecorderAction::ToggleLayerMute {
                                    track: track_id,
                                    layer_index,
                                    phrase: phrase_id,
                                },
                            );
                        }
                    },
                )) as Child<S>
            })
            .collect();
        let summary: Child<S> = if state.track_waveform_available(i) {
            wave_leaf(mount, track.id)
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
                .attr("class", "hocket-recorder-wave-unavailable"),
            )
        };
        Box::new(
            el(
                "div",
                (
                    summary,
                    el("div", layer_rows).attr("class", "hocket-recorder-layers"),
                ),
            )
            .attr("class", "hocket-recorder-lane-wave"),
        )
    };

    let m_cls = if track.muted {
        "hocket-recorder-lctl hocket-recorder-lctl-on"
    } else {
        "hocket-recorder-lctl"
    };
    let s_cls = if state.solo.contains(&track.id) {
        "hocket-recorder-lctl hocket-recorder-lctl-on"
    } else {
        "hocket-recorder-lctl"
    };
    let ctl = el(
        "div",
        (
            clickable(
                el("div", text("M"))
                    .attr(
                        "class",
                        format!("{} hocket-recorder-mount-{}", m_cls, mount.id()),
                    )
                    .attr("role", "switch")
                    .attr("data-recorder-mount", mount.id().to_string())
                    .attr("data-recorder-track", track_id.0.to_string())
                    .attr("aria-checked", if track.muted { "true" } else { "false" })
                    .attr("aria-label", format!("Mute {}", track.name)),
                move |state: &mut S, _| dispatch(state, RecorderAction::ToggleTrackMute(track_id)),
            ),
            clickable(
                el("div", text("S"))
                    .attr(
                        "class",
                        format!("{} hocket-recorder-mount-{}", s_cls, mount.id()),
                    )
                    .attr("role", "switch")
                    .attr("data-recorder-mount", mount.id().to_string())
                    .attr("data-recorder-track", track_id.0.to_string())
                    .attr(
                        "aria-checked",
                        if state.solo.contains(&track.id) {
                            "true"
                        } else {
                            "false"
                        },
                    )
                    .attr("aria-label", format!("Solo {}", track.name)),
                move |state: &mut S, _| dispatch(state, RecorderAction::ToggleSolo(track_id)),
            ),
        ),
    )
    .attr("class", "hocket-recorder-lane-ctl");

    Box::new(
        el("div", (id, wavecol, ctl))
            .attr("class", cls)
            .attr("style", style),
    )
}

pub fn transport<S: 'static>(
    state: &Recorder,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: Dispatch<S>,
    accessory: Option<Child<S>>,
) -> Child<S> {
    let accessory = accessory.unwrap_or_else(|| Box::new(el("span", ())));
    let rec_cls = if state.is_recording() {
        "hocket-recorder-record hocket-recorder-record-armed"
    } else {
        "hocket-recorder-record"
    };
    let bpm = format!("{}", state.session.bpm.round() as u32);
    let ts = state.session.time_signature;
    let meter_txt = format!("{}/{}", ts.numerator, ts.denominator);
    let rec_label = match state.armed_index() {
        Some(i) => format!(" \u{00b7} {} armed", state.session.tracks[i].name),
        None => " \u{00b7} nothing armed".to_string(),
    };
    let click_cls = if state.click {
        "hocket-recorder-toggle hocket-recorder-toggle-on"
    } else {
        "hocket-recorder-toggle"
    };
    let clock_cls = if state.session.master_clock_enabled {
        "hocket-recorder-toggle hocket-recorder-toggle-on"
    } else {
        "hocket-recorder-toggle"
    };

    let left = el(
        "div",
        (
            el(
                "div",
                (
                    el("div", text("tempo")).attr("class", "hocket-recorder-eyebrow"),
                    el(
                        "div",
                        (
                            clickable(
                                el("div", text("\u{2212}"))
                                    .attr(
                                        "class",
                                        format!(
                                            "{} hocket-recorder-mount-{}",
                                            "hocket-recorder-step",
                                            mount.id()
                                        ),
                                    )
                                    .attr("role", "button")
                                    .attr("data-recorder-mount", mount.id().to_string())
                                    .attr("aria-label", "Decrease tempo"),
                                move |state: &mut S, _| {
                                    dispatch(state, RecorderAction::NudgeBpm(-2.0))
                                },
                            ),
                            el("span", text(bpm))
                                .attr("class", "hocket-recorder-step-val hocket-recorder-mono"),
                            clickable(
                                el("div", text("+"))
                                    .attr(
                                        "class",
                                        format!(
                                            "{} hocket-recorder-mount-{}",
                                            "hocket-recorder-step",
                                            mount.id()
                                        ),
                                    )
                                    .attr("role", "button")
                                    .attr("data-recorder-mount", mount.id().to_string())
                                    .attr("aria-label", "Increase tempo"),
                                move |state: &mut S, _| {
                                    dispatch(state, RecorderAction::NudgeBpm(2.0))
                                },
                            ),
                        ),
                    )
                    .attr("class", "hocket-recorder-stepper"),
                ),
            )
            .attr("class", "hocket-recorder-readout"),
            el(
                "div",
                (
                    el("div", text("meter")).attr("class", "hocket-recorder-eyebrow"),
                    el("div", text(meter_txt))
                        .attr("class", "hocket-recorder-readout-val hocket-recorder-mono"),
                ),
            )
            .attr("class", "hocket-recorder-readout"),
            el(
                "div",
                (
                    clickable(
                        el(
                            "div",
                            (
                                el("span", ()).attr("class", "hocket-recorder-led"),
                                el("span", text("Click")),
                            ),
                        )
                        .attr(
                            "class",
                            format!("{} hocket-recorder-mount-{}", click_cls, mount.id()),
                        )
                        .attr("role", "switch")
                        .attr("data-recorder-mount", mount.id().to_string())
                        .attr("aria-checked", if state.click { "true" } else { "false" })
                        .attr("aria-label", "Click track"),
                        move |state: &mut S, _| dispatch(state, RecorderAction::ToggleClick),
                    ),
                    clickable(
                        el(
                            "div",
                            (
                                el("span", ()).attr("class", "hocket-recorder-led"),
                                el("span", text("Master clock")),
                            ),
                        )
                        .attr(
                            "class",
                            format!("{} hocket-recorder-mount-{}", clock_cls, mount.id()),
                        )
                        .attr("role", "switch")
                        .attr("data-recorder-mount", mount.id().to_string())
                        .attr(
                            "aria-checked",
                            if state.session.master_clock_enabled {
                                "true"
                            } else {
                                "false"
                            },
                        )
                        .attr("aria-label", "Master clock"),
                        move |state: &mut S, _| dispatch(state, RecorderAction::ToggleMasterClock),
                    ),
                ),
            )
            .attr("class", "hocket-recorder-toggles"),
        ),
    )
    .attr("class", "hocket-recorder-t-left");

    let center = el(
        "div",
        (
            clickable(
                el(
                    "div",
                    el("div", ()).attr("class", "hocket-recorder-record-core"),
                )
                .attr(
                    "class",
                    format!("{} hocket-recorder-mount-{}", rec_cls, mount.id()),
                )
                .attr("role", "switch")
                .attr("data-recorder-mount", mount.id().to_string())
                .attr(
                    "aria-checked",
                    if state.is_recording() {
                        "true"
                    } else {
                        "false"
                    },
                )
                .attr("aria-label", "Record")
                .attr(
                    "aria-disabled",
                    if state.input_available() {
                        "false"
                    } else {
                        "true"
                    },
                ),
                move |state: &mut S, _| dispatch(state, RecorderAction::ToggleRecord),
            ),
            el(
                "div",
                (el("b", text("Record")), el("span", text(rec_label))),
            )
            .attr("class", "hocket-recorder-record-label"),
        ),
    )
    .attr("class", "hocket-recorder-record-wrap");

    let right = el(
        "div",
        el(
            "div",
            (
                accessory,
                clickable(
                    el("div", text("\u{25a0}"))
                        .attr(
                            "class",
                            format!(
                                "{} hocket-recorder-mount-{}",
                                "hocket-recorder-stop",
                                mount.id()
                            ),
                        )
                        .attr("role", "button")
                        .attr("data-recorder-mount", mount.id().to_string())
                        .attr("aria-label", "Stop all loops"),
                    move |state: &mut S, _| dispatch(state, RecorderAction::StopAll),
                ),
                meter(state, mount),
            ),
        )
        .attr("class", "hocket-recorder-t-right-inner"),
    )
    .attr("class", "hocket-recorder-t-right");

    Box::new(el("div", (left, center, right)).attr(
        "class",
        format!(
            "hocket-recorder hocket-recorder-transport {}",
            layout.class()
        ),
    ))
}

fn meter<S: 'static>(state: &Recorder, mount: RecorderMount) -> Child<S> {
    // The L/R output bars are chisel `Meter` leaves (host-owned).
    let col = |key: u64, label: &str| -> Child<S> {
        Box::new(
            el(
                "div",
                (
                    chisel_leaf(key, 10, 46),
                    el("span", text(label.to_string()))
                        .attr("class", "hocket-recorder-mlbl hocket-recorder-mono"),
                ),
            )
            .attr("class", "hocket-recorder-mcol"),
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
                el(
                    "div",
                    (col(mount.meter_key(0), "L"), col(mount.meter_key(1), "R")),
                )
                .attr("class", "hocket-recorder-mbars"),
                el(
                    "div",
                    (
                        el("div", text("out")).attr("class", "hocket-recorder-eyebrow"),
                        el("div", text(peak_text))
                            .attr("class", "hocket-recorder-mpeak hocket-recorder-mono"),
                    ),
                )
                .attr("class", "hocket-recorder-readout"),
            ),
        )
        .attr("class", "hocket-recorder-meter"),
    )
}
