// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use crate::*;
use cambium_genet_winit_host::Harness;
use taproot::Selector;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    owner: u64,
    segment: u64,
    session_epoch: u64,
}

struct HostOwned {
    selected: u64,
    accepted_capture: Option<u64>,
    record_enabled: bool,
    pcm: Vec<f32>,
    actions: Vec<ProjectedAction<Key>>,
}

impl Default for HostOwned {
    fn default() -> Self {
        Self {
            selected: 1,
            accepted_capture: None,
            record_enabled: true,
            pcm: vec![-0.25, 0.5],
            actions: Vec::new(),
        }
    }
}

fn key(segment: u64) -> Key {
    Key {
        owner: 7,
        segment,
        session_epoch: 3,
    }
}

fn display(state: &HostOwned) -> RecorderProjection<'_, Key> {
    let mut transport = TransportProjection::new(key(0));
    transport.input_status = "Input opens on Record".into();
    transport.recording = if state.accepted_capture.is_some() {
        RecordingState::Armed
    } else {
        RecordingState::Idle
    };
    transport.record = Some(CommandControl {
        target: key(state.accepted_capture.unwrap_or(state.selected)),
        label: "Record".into(),
        enabled: state.record_enabled,
    });
    transport.stop_capture = state.accepted_capture.map(|segment| CommandControl {
        target: key(segment),
        label: "Stop capture".into(),
        enabled: true,
    });
    RecorderProjection {
        title: "Set backing".into(),
        hint: "Actual host-owned lanes".into(),
        meters: None,
        transport,
        lanes: [1, 2]
            .into_iter()
            .map(|segment| LaneProjection {
                key: key(segment),
                name: format!("segment {segment}").into(),
                owner: "Set 7".into(),
                clock: if segment == 1 {
                    "83.75 bpm · 7/8 · 3 bars"
                } else {
                    "110 bpm · 5/4 · 2 bars"
                }
                .into(),
                color: [80, 180, 140],
                selected: state.selected == segment,
                recording: if state.accepted_capture == Some(segment) {
                    RecordingState::Armed
                } else {
                    RecordingState::Idle
                },
                waveform: WaveformProjection::Data {
                    revision: 1,
                    source: WaveformSource::Pcm(&state.pcm),
                },
                layers: Vec::new(),
                controls: LaneControls {
                    select: Some(CommandControl {
                        target: key(segment),
                        label: format!("Select segment {segment}").into(),
                        enabled: true,
                    }),
                    clear: Some(CommandControl {
                        target: key(segment),
                        label: format!("Clear segment {segment}").into(),
                        enabled: state.accepted_capture.is_none(),
                    }),
                    ..Default::default()
                },
            })
            .collect(),
    }
}

fn root(state: &HostOwned) -> Child<HostOwned> {
    projected_surface(
        &display(state),
        RecorderMount::new(74),
        RecorderLayout::Full,
        |state: &mut HostOwned, action| state.actions.push(action),
    )
}

type Host = Harness<HostOwned, fn(&HostOwned) -> Child<HostOwned>, Child<HostOwned>>;
fn mount(state: HostOwned) -> Host {
    let mut host = Harness::new(
        theme::sheet(),
        state,
        root as fn(&HostOwned) -> Child<HostOwned>,
    );
    host.layout_at(1000.0, 700.0);
    host
}

#[test]
fn lazy_input_does_not_disable_first_record_request() {
    let mut host = mount(HostOwned::default());
    assert!(host.click_on(&Selector::role("switch").containing("Record")));
    assert_eq!(host.state().actions, [ProjectedAction::Record(key(1))]);
    assert!(host.with_dom(|dom| {
        !taproot::matching(
            dom,
            &Selector::role("status").containing("Input opens on Record"),
        )
        .is_empty()
    }));
}

#[test]
fn accepted_capture_target_is_independent_of_current_selection() {
    let mut host = mount(HostOwned {
        selected: 2,
        accepted_capture: Some(1),
        ..Default::default()
    });
    assert!(host.click_on(&Selector::role("button").containing("Stop capture")));
    assert_eq!(host.state().actions, [ProjectedAction::StopCapture(key(1))]);
    assert!(host.click_on(&Selector::role("switch").containing("Record")));
    assert_eq!(
        host.state().actions.last(),
        Some(&ProjectedAction::Record(key(1)))
    );
}

#[test]
fn heterogeneous_clocks_and_unsupported_controls_remain_honest() {
    let host = mount(HostOwned::default());
    for clock in ["83.75 bpm", "7/8", "110 bpm", "5/4"] {
        assert!(host.with_dom(|dom| {
            !taproot::matching(
                dom,
                &Selector::class("hocket-recorder-tag").containing(clock),
            )
            .is_empty()
        }));
    }
    for label in ["Master clock", "Mute segment", "Solo segment"] {
        assert!(host.with_dom(|dom| {
            taproot::matching(dom, &Selector::role("switch").containing(label)).is_empty()
        }));
    }
    for class in [
        "hocket-recorder-add-track",
        "hocket-recorder-meter",
        "hocket-recorder-meter-leaf",
    ] {
        assert!(host.with_dom(|dom| taproot::matching(dom, &Selector::class(class)).is_empty()));
    }
}

#[test]
fn unavailable_command_cannot_emit_a_host_action() {
    let mut host = mount(HostOwned {
        record_enabled: false,
        ..Default::default()
    });
    host.click_on(&Selector::role("switch").containing("Record"));
    assert!(host.state().actions.is_empty());
}

#[test]
fn timing_summary_uses_the_host_heading_and_keeps_standalone_meter_default() {
    for host_heading in [None, Some("Selected timing")] {
        let root = move |state: &HostOwned| {
            let mut projection = display(state);
            projection.transport.meter_label = Some("1.125 s · 137 bpm click · 5/4".into());
            if let Some(heading) = host_heading {
                projection.transport.meter_heading = heading.into();
            }
            projected_surface(
                &projection,
                RecorderMount::new(74),
                RecorderLayout::Full,
                |state: &mut HostOwned, action| state.actions.push(action),
            )
        };
        let mut host = Harness::new(theme::sheet(), HostOwned::default(), root);
        host.layout_at(1000.0, 700.0);
        assert!(host.with_dom(|dom| !taproot::matching(
            dom,
            &Selector::class("hocket-recorder-eyebrow")
                .containing(host_heading.unwrap_or("meter")),
        ).is_empty()));
        if host_heading.is_some() {
            assert!(host.with_dom(|dom| taproot::matching(
                dom,
                &Selector::class("hocket-recorder-eyebrow").containing("meter"),
            ).is_empty()));
        }
    }
}
