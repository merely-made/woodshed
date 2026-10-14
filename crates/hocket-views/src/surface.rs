// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! One renderer for portable Recorder and host-owned display projections.

use crate::{
    Child, CommandControl, LaneProjection, ProjectedAction, RecorderLayout, RecorderMount,
    RecorderProjection, RecordingState, ToggleControl, WaveformProjection,
};
use cambium::{clickable, el, text};
use std::hash::Hash;

pub fn projected_surface<S, K, F>(
    projection: &RecorderProjection<'_, K>,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: F,
) -> Child<S>
where
    S: 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + Clone + 'static,
{
    Box::new(
        el(
            "div",
            (
                projected_table(projection, mount, layout, dispatch.clone()),
                projected_transport(projection, mount, layout, dispatch, None),
            ),
        )
        .attr(
            "class",
            format!("hocket-recorder hocket-recorder-surface {}", layout.class()),
        )
        .attr("data-recorder-mount", mount.id().to_string()),
    )
}

pub fn projected_table<S, K, F>(
    projection: &RecorderProjection<'_, K>,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: F,
) -> Child<S>
where
    S: 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + Clone + 'static,
{
    let mut children: Vec<Child<S>> = vec![Box::new(
        el(
            "div",
            (
                el("div", text(projection.title.to_string()))
                    .attr("class", "hocket-recorder-eyebrow"),
                el("div", text(projection.hint.to_string()))
                    .attr("class", "hocket-recorder-table-hint"),
            ),
        )
        .attr("class", "hocket-recorder-table-head"),
    )];
    for lane in &projection.lanes {
        children.push(projected_lane(lane, mount, dispatch.clone()));
    }
    if let Some(control) = &projection.transport.add {
        children.push(command(
            control,
            ProjectedAction::Add(control.target.clone()),
            "+ add track",
            "hocket-recorder-add-track",
            mount,
            dispatch,
        ));
    }
    Box::new(el("div", children).attr(
        "class",
        format!("hocket-recorder hocket-recorder-table {}", layout.class()),
    ))
}

fn projected_lane<S, K, F>(
    lane: &LaneProjection<'_, K>,
    mount: RecorderMount,
    dispatch: F,
) -> Child<S>
where
    S: 'static,
    K: Clone + Eq + Hash + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + Clone + 'static,
{
    let mut class = String::from("hocket-recorder-lane");
    if lane.recording != RecordingState::Idle {
        class.push_str(" hocket-recorder-lane-rec");
    } else if lane.selected {
        class.push_str(" hocket-recorder-lane-armed");
    }
    let mut name: Vec<Child<S>> = Vec::new();
    if let Some(control) = &lane.controls.select {
        name.push(switch(
            control,
            lane.selected,
            ProjectedAction::Select(control.target.clone()),
            "",
            "hocket-recorder-arm",
            mount,
            dispatch.clone(),
        ));
    }
    name.push(Box::new(
        el("span", text(lane.name.to_string())).attr("class", "hocket-recorder-lane-title"),
    ));
    let mut tags: Vec<Child<S>> = Vec::new();
    if !lane.owner.is_empty() {
        tags.push(Box::new(
            el("span", text(lane.owner.to_string()))
                .attr("class", "hocket-recorder-tag hocket-recorder-tag-voice"),
        ));
    }
    if !lane.clock.is_empty() {
        tags.push(Box::new(
            el("span", text(lane.clock.to_string()))
                .attr("class", "hocket-recorder-tag hocket-recorder-mono"),
        ));
    }
    if !lane.layers.is_empty() {
        tags.push(Box::new(
            el("span", text(format!("{} layers", lane.layers.len())))
                .attr("class", "hocket-recorder-tag hocket-recorder-mono"),
        ));
    }
    let id = el(
        "div",
        (
            el("div", name).attr("class", "hocket-recorder-lane-name"),
            el("div", tags).attr("class", "hocket-recorder-lane-meta"),
        ),
    )
    .attr("class", "hocket-recorder-lane-id");
    let mut waves = vec![wave(
        &lane.waveform,
        mount.projected_wave_key(&lane.key),
        40,
    )];
    let layers: Vec<Child<S>> = lane
        .layers
        .iter()
        .rev()
        .map(|layer| {
            let children = (
                el("span", text(layer.label.to_string()))
                    .attr("class", "hocket-recorder-lnum hocket-recorder-mono"),
                el(
                    "div",
                    wave::<S>(
                        &layer.waveform,
                        mount.projected_layer_wave_key(&layer.key),
                        11,
                    ),
                )
                .attr("class", "hocket-recorder-layer-wave"),
            );
            let mut class = "hocket-recorder-layer".to_string();
            if layer.muted {
                class.push_str(" hocket-recorder-layer-muted");
            }
            let element = el("div", children).attr("class", scoped(&class, mount));
            match &layer.mute {
                Some(control) => {
                    let target = control.target.clone();
                    let enabled = control.enabled;
                    let dispatch = dispatch.clone();
                    Box::new(clickable(
                        element
                            .attr("role", "switch")
                            .attr("aria-checked", layer.muted.to_string())
                            .attr("aria-disabled", (!enabled).to_string())
                            .attr("aria-label", control.label.to_string())
                            .attr("data-recorder-mount", mount.id().to_string()),
                        move |state: &mut S, _| {
                            if enabled {
                                dispatch(state, ProjectedAction::LayerMute(target.clone()));
                            }
                        },
                    )) as Child<S>
                },
                None => Box::new(element),
            }
        })
        .collect();
    if !layers.is_empty() {
        waves.push(Box::new(
            el("div", layers).attr("class", "hocket-recorder-layers"),
        ));
    }
    let mut controls: Vec<Child<S>> = Vec::new();
    if let Some(control) = &lane.controls.record {
        controls.push(command(
            control,
            ProjectedAction::Record(control.target.clone()),
            "R",
            "hocket-recorder-lctl",
            mount,
            dispatch.clone(),
        ));
    }
    if let Some(control) = &lane.controls.clear {
        controls.push(command(
            control,
            ProjectedAction::Clear(control.target.clone()),
            "Clear",
            "hocket-recorder-lctl hocket-recorder-clear",
            mount,
            dispatch.clone(),
        ));
    }
    if let Some(control) = &lane.controls.mute {
        controls.push(toggle(
            control,
            ProjectedAction::Mute(control.target.clone()),
            "M",
            "hocket-recorder-lctl",
            mount,
            dispatch.clone(),
        ));
    }
    if let Some(control) = &lane.controls.solo {
        controls.push(toggle(
            control,
            ProjectedAction::Solo(control.target.clone()),
            "S",
            "hocket-recorder-lctl",
            mount,
            dispatch,
        ));
    }
    Box::new(
        el(
            "div",
            (
                id,
                el("div", waves).attr("class", "hocket-recorder-lane-wave"),
                el("div", controls).attr("class", "hocket-recorder-lane-ctl"),
            ),
        )
        .attr("class", class)
        .attr(
            "style",
            format!(
                "--voice: #{:02x}{:02x}{:02x}",
                lane.color[0], lane.color[1], lane.color[2]
            ),
        )
        .attr(
            "data-recorder-key",
            mount.projected_wave_key(&lane.key).to_string(),
        ),
    )
}

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
                    el("div", text("meter")).attr("class", "hocket-recorder-eyebrow"),
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
                    el("span", text(transport.record_note.to_string())),
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

fn scoped(class: &str, mount: RecorderMount) -> String {
    format!("{class} hocket-recorder-mount-{}", mount.id())
}

fn command<S, K, F>(
    control: &CommandControl<'_, K>,
    action: ProjectedAction<K>,
    label: &str,
    class: &str,
    mount: RecorderMount,
    dispatch: F,
) -> Child<S>
where
    S: 'static,
    K: Clone + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + 'static,
{
    let enabled = control.enabled;
    Box::new(clickable(
        el("div", text(label.to_string()))
            .attr("class", scoped(class, mount))
            .attr("role", "button")
            .attr("aria-label", control.label.to_string())
            .attr("aria-disabled", (!enabled).to_string())
            .attr("data-recorder-mount", mount.id().to_string()),
        move |state: &mut S, _| {
            if enabled {
                dispatch(state, action.clone());
            }
        },
    ))
}

fn switch<S, K, F>(
    control: &CommandControl<'_, K>,
    active: bool,
    action: ProjectedAction<K>,
    label: &str,
    class: &str,
    mount: RecorderMount,
    dispatch: F,
) -> Child<S>
where
    S: 'static,
    K: Clone + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + 'static,
{
    let enabled = control.enabled;
    Box::new(clickable(
        el("div", text(label.to_string()))
            .attr("class", scoped(class, mount))
            .attr("role", "switch")
            .attr("aria-label", control.label.to_string())
            .attr("aria-checked", active.to_string())
            .attr("aria-disabled", (!enabled).to_string())
            .attr("data-recorder-mount", mount.id().to_string()),
        move |state: &mut S, _| {
            if enabled {
                dispatch(state, action.clone());
            }
        },
    ))
}

fn toggle<S, K, F>(
    control: &ToggleControl<'_, K>,
    action: ProjectedAction<K>,
    label: &str,
    class: &str,
    mount: RecorderMount,
    dispatch: F,
) -> Child<S>
where
    S: 'static,
    K: Clone + 'static,
    F: Fn(&mut S, ProjectedAction<K>) + 'static,
{
    let class = if control.active {
        format!("{class} hocket-recorder-lctl-on hocket-recorder-toggle-on")
    } else {
        class.into()
    };
    let enabled = control.enabled;
    let mut children: Vec<Child<S>> = Vec::new();
    if class.contains("hocket-recorder-toggle") {
        children.push(Box::new(
            el("span", ()).attr("class", "hocket-recorder-led"),
        ));
    }
    children.push(Box::new(el("span", text(label.to_string()))));
    Box::new(clickable(
        el("div", children)
            .attr("class", scoped(&class, mount))
            .attr("role", "switch")
            .attr("aria-label", control.label.to_string())
            .attr("aria-checked", control.active.to_string())
            .attr("aria-disabled", (!enabled).to_string())
            .attr("data-recorder-mount", mount.id().to_string()),
        move |state: &mut S, _| {
            if enabled {
                dispatch(state, action.clone());
            }
        },
    ))
}

fn wave<S: 'static>(wave: &WaveformProjection<'_>, key: u64, height: u32) -> Child<S> {
    match wave {
        WaveformProjection::Data { source, .. }
            if matches!(source, crate::WaveformSource::Pcm(samples) if samples.is_empty())
                || matches!(source, crate::WaveformSource::Peaks(peaks) if peaks.is_empty()) =>
        {
            Box::new(
                el("span", text("waveform unavailable"))
                    .attr("class", "hocket-recorder-wave-unavailable"),
            )
        },
        WaveformProjection::Data { .. } => Box::new(
            el("chisel-leaf", ())
                .attr("class", "hocket-recorder-wave-leaf")
                .attr("key", key.to_string())
                .attr("aria-hidden", "true")
                .attr(
                    "style",
                    format!("display:block; width:100%; height:{height}px"),
                ),
        ),
        WaveformProjection::Empty(label) => Box::new(
            el("span", text(label.to_string())).attr("class", "hocket-recorder-wave-empty"),
        ),
        WaveformProjection::Unavailable(label) => Box::new(
            el("span", text(label.to_string())).attr("class", "hocket-recorder-wave-unavailable"),
        ),
    }
}

fn meter<S: 'static, K>(projection: &RecorderProjection<'_, K>, mount: RecorderMount) -> Child<S> {
    let meters = projection
        .meters
        .as_ref()
        .expect("caller checks meter observation");
    let columns: Vec<Child<S>> = ["L", "R"]
        .into_iter()
        .enumerate()
        .map(|(channel, label)| {
            Box::new(
                el(
                    "div",
                    (
                        el("chisel-leaf", ())
                            .attr("class", "hocket-recorder-meter-leaf")
                            .attr("key", mount.meter_key(channel as u8).to_string())
                            .attr("style", "display:block; width:10px; height:46px"),
                        el("span", text(label))
                            .attr("class", "hocket-recorder-mlbl hocket-recorder-mono"),
                    ),
                )
                .attr("class", "hocket-recorder-mcol"),
            ) as Child<S>
        })
        .collect();
    let db = meters[0].db.max(meters[1].db);
    let label = if db.is_finite() {
        format!("{db:.1} dB")
    } else {
        "-- dB".into()
    };
    Box::new(
        el(
            "div",
            (
                el("div", columns).attr("class", "hocket-recorder-mbars"),
                el(
                    "div",
                    (
                        el("div", text("out")).attr("class", "hocket-recorder-eyebrow"),
                        el("div", text(label))
                            .attr("class", "hocket-recorder-mpeak hocket-recorder-mono"),
                    ),
                )
                .attr("class", "hocket-recorder-readout"),
            ),
        )
        .attr("class", "hocket-recorder-meter"),
    )
}
