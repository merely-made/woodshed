// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

pub fn projected_transport<S, K, F>(
    projection: &RecorderProjection<'_, K>,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: F,
    accessory: Option<Child<S>>,
) -> Child<S>
where
    S: 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + Clone + 'static,
{
    let transport = &projection.transport;
    let mut left: Vec<Child<S>> = Vec::new();
    if let Some(tempo) = &transport.tempo {
        let down = CommandControl {
            target: tempo.target.clone(),
            label: "Decrease tempo".into(),
            enabled: tempo.enabled,
        };
        let up = CommandControl {
            target: tempo.target.clone(),
            label: "Increase tempo".into(),
            enabled: tempo.enabled,
        };
        left.push(Box::new(
            el(
                "div",
                (
                    el("div", text(tempo.label.to_string()))
                        .attr("class", "hocket-recorder-eyebrow"),
                    el(
                        "div",
                        (
                            command(
                                &down,
                                ProjectedAction::NudgeTempo {
                                    target: tempo.target.clone(),
                                    delta: -tempo.step,
                                },
                                "−",
                                "hocket-recorder-step",
                                mount,
                                dispatch.clone(),
                            ),
                            Box::new(
                                el("span", text(tempo.bpm.to_string()))
                                    .attr("class", "hocket-recorder-step-val hocket-recorder-mono"),
                            ),
                            command(
                                &up,
                                ProjectedAction::NudgeTempo {
                                    target: tempo.target.clone(),
                                    delta: tempo.step,
                                },
                                "+",
                                "hocket-recorder-step",
                                mount,
                                dispatch.clone(),
                            ),
                        ),
                    )
                    .attr("class", "hocket-recorder-stepper"),
                ),
            )
            .attr("class", "hocket-recorder-readout"),
        ));
    }
    if let Some(label) = &transport.meter_label {
        left.push(Box::new(
            el(
                "div",
                (
                    el("div", text(transport.meter_heading.to_string()))
                        .attr("class", "hocket-recorder-eyebrow"),
                    el("div", text(label.to_string()))
                        .attr("class", "hocket-recorder-readout-val hocket-recorder-mono"),
                ),
            )
            .attr("class", "hocket-recorder-readout"),
        ));
    }
    let mut toggles: Vec<Child<S>> = Vec::new();
    if let Some(control) = &transport.click {
        toggles.push(toggle(
            control,
            ProjectedAction::ToggleClick(control.target.clone()),
            "Click",
            "hocket-recorder-toggle",
            mount,
            dispatch.clone(),
        ));
    }
    if let Some(control) = &transport.master_clock {
        toggles.push(toggle(
            control,
            ProjectedAction::ToggleMasterClock(control.target.clone()),
            &control.label,
            "hocket-recorder-toggle",
            mount,
            dispatch.clone(),
        ));
    }
    if let Some(control) = &transport.replace {
        toggles.push(toggle(
            control,
            ProjectedAction::SetReplace {
                target: control.target.clone(),
                replace: !control.active,
            },
            &control.label,
            "hocket-recorder-toggle",
            mount,
            dispatch.clone(),
        ));
    }
    if !toggles.is_empty() {
        left.push(Box::new(
            el("div", toggles).attr("class", "hocket-recorder-toggles"),
        ));
    }
    let mut center: Vec<Child<S>> = Vec::new();
    if let Some(control) = &transport.record {
        let target = control.target.clone();
        let enabled = control.enabled;
        let dispatch = dispatch.clone();
        let active = transport.recording != RecordingState::Idle;
        center.push(Box::new(clickable(
            el(
                "div",
                el("div", ()).attr("class", "hocket-recorder-record-core"),
            )
            .attr(
                "class",
                scoped(
                    if active {
                        "hocket-recorder-record hocket-recorder-record-armed"
                    } else {
                        "hocket-recorder-record"
                    },
                    mount,
                ),
            )
            .attr("role", "switch")
            .attr("aria-checked", active.to_string())
            .attr("aria-disabled", (!enabled).to_string())
            .attr("aria-label", control.label.to_string())
            .attr("data-recorder-mount", mount.id().to_string()),
            move |state: &mut S, _| {
                if enabled {
                    dispatch(state, ProjectedAction::Record(target.clone()));
                }
            },
        )));
        center.push(Box::new(
            el(
                "div",
                (
                    el("b", text(control.label.to_string())),
                    el("span", text(format!(" {}", transport.record_note))),
                ),
            )
            .attr("class", "hocket-recorder-record-label"),
        ));
    }
    let mut right: Vec<Child<S>> = accessory.into_iter().collect();
    for (control, label, kind) in [
        (&transport.play, "▶", 0),
        (&transport.stop, "■", 1),
        (&transport.rewind, "↤", 2),
        (&transport.stop_capture, "Stop capture", 3),
    ] {
        if let Some(control) = control {
            let label = if kind == 0 && transport.playing {
                "⏸"
            } else {
                label
            };
            let class = if kind == 3 {
                "hocket-recorder-stop hocket-recorder-stop-capture"
            } else if kind == 0 && transport.playing {
                "hocket-recorder-stop hocket-recorder-playing"
            } else {
                "hocket-recorder-stop"
            };
            let action = match kind {
                0 => ProjectedAction::Play(control.target.clone()),
                1 => ProjectedAction::Stop(control.target.clone()),
                2 => ProjectedAction::Rewind(control.target.clone()),
                _ => ProjectedAction::StopCapture(control.target.clone()),
            };
            right.push(command(
                control,
                action,
                label,
                class,
                mount,
                dispatch.clone(),
            ));
        }
    }
    if projection.meters.is_some() {
        right.push(meter(projection, mount));
    }
    let mut children: Vec<Child<S>> = vec![Box::new(
        el(
            "div",
            (
                el("div", left).attr("class", "hocket-recorder-t-left"),
                el("div", center).attr("class", "hocket-recorder-record-wrap"),
                el(
                    "div",
                    el("div", right).attr("class", "hocket-recorder-t-right-inner"),
                )
                .attr("class", "hocket-recorder-t-right"),
            ),
        )
        .attr(
            "class",
            format!(
                "hocket-recorder hocket-recorder-transport {}",
                layout.class()
            ),
        ),
    )];
    for status in [&transport.status, &transport.input_status] {
        if !status.is_empty() {
            children.push(Box::new(
                el("div", text(status.to_string()))
                    .attr("class", "hocket-recorder-status")
                    .attr("role", "status"),
            ));
        }
    }
    Box::new(
        el("div", children)
            .attr("class", "hocket-recorder-transport-wrap")
            .attr("data-recorder-playing", transport.playing.to_string()),
    )
}
