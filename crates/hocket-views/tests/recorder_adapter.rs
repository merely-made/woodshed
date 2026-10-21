// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use cambium_genet_winit_host::Harness;
use hocket_recorder::{
    AudioAvailability, AudioObservation, CaptureCompletion, CaptureMode, CapturePhase,
    CaptureToken, LayerKey, Recorder, RecorderAction, RecorderAudio,
};
use hocket_views::{Child, RecorderLayout, RecorderMount, surface};
use taproot::Selector;

#[derive(Default)]
struct Runtime {
    fail_capture: bool,
    fail_playback: bool,
    observation_error: Option<String>,
    accepted: Option<CaptureToken>,
    completed: Option<CaptureCompletion>,
}

impl RecorderAudio for Runtime {
    fn availability(&self) -> AudioAvailability {
        AudioAvailability {
            sample_rate: 48_000,
            input: true,
            error: None,
        }
    }

    fn poll(&mut self) -> AudioObservation {
        AudioObservation {
            phase: if self.completed.is_some() {
                CapturePhase::Complete
            } else {
                CapturePhase::Idle
            },
            completed: self.completed.take(),
            error: self.observation_error.take(),
            ..Default::default()
        }
    }

    fn set_clock(&mut self, _: f32, _: u8, _: u8, _: bool) -> Result<(), String> {
        Ok(())
    }

    fn set_click(&mut self, _: bool) -> Result<(), String> {
        Ok(())
    }

    fn begin_capture(&mut self, token: CaptureToken, _: CaptureMode) -> Result<(), String> {
        if self.fail_capture {
            return Err("capture refused by runtime".into());
        }
        self.accepted = Some(token);
        Ok(())
    }

    fn stop_free_capture(&mut self, _: CaptureToken) -> Result<(), String> {
        Ok(())
    }

    fn play_layer(
        &mut self,
        _: LayerKey,
        _: Vec<f32>,
        _: u32,
        _: f32,
        _: bool,
    ) -> Result<(), String> {
        if self.fail_playback {
            Err("playback refused by runtime".into())
        } else {
            Ok(())
        }
    }

    fn stop_layer(&mut self, _: LayerKey) -> Result<(), String> {
        Ok(())
    }
}

struct UiState {
    recorder: Recorder,
}

fn root(state: &UiState) -> Child<UiState> {
    surface(
        &state.recorder,
        RecorderMount::new(91),
        RecorderLayout::Full,
        |_, _| {},
    )
}

type Host = Harness<UiState, fn(&UiState) -> Child<UiState>, Child<UiState>>;

fn mount(recorder: Recorder) -> Host {
    let mut host = Harness::new(
        hocket_views::theme::sheet(),
        UiState { recorder },
        root as fn(&UiState) -> Child<UiState>,
    );
    host.layout_at(1000.0, 700.0);
    host
}

fn assert_status(host: &Host, expected: &str) {
    assert!(host.with_dom(|dom| {
        !taproot::matching_with_projection(
            dom,
            &Selector::role("status").containing(expected),
            &host.a11y_projection(),
        )
        .is_empty()
    }));
}

fn assert_no_status(host: &Host, unexpected: &str) {
    assert!(host.with_dom(|dom| {
        taproot::matching_with_projection(
            dom,
            &Selector::role("status").containing(unexpected),
            &host.a11y_projection(),
        )
        .is_empty()
    }));
}

#[test]
fn actual_recorder_wrapper_renders_healthy_and_runtime_failure_status() {
    let mut runtime = Runtime::default();
    let host = mount(Recorder::default());
    assert_status(&host, "audio unavailable");

    let mut recorder = Recorder::default();
    recorder.service(&mut runtime, 0, 0.016);
    let healthy = mount(recorder);
    assert_status(&healthy, "system audio");

    let mut recorder = Recorder::default();
    runtime.observation_error = Some("backend observation failed".into());
    recorder.service(&mut runtime, 16, 0.016);
    let failed = mount(recorder);
    assert_status(&failed, "backend observation failed");

    let mut recorder = Recorder::default();
    runtime.observation_error = Some("backend observation failed".into());
    recorder.service(&mut runtime, 16, 0.016);
    recorder.service(&mut runtime, 32, 0.016);
    let recovered = mount(recorder);
    assert_status(&recovered, "system audio");
    assert_no_status(&recovered, "backend observation failed");
}

#[test]
fn actual_recorder_wrapper_renders_capture_and_playback_refusals() {
    let mut recorder = Recorder::default();
    let mut runtime = Runtime {
        fail_capture: true,
        ..Default::default()
    };
    recorder.service(&mut runtime, 0, 0.016);
    let track = recorder.session.tracks[0].id;
    recorder.request(RecorderAction::Arm(track));
    recorder.service(&mut runtime, 16, 0.016);
    recorder.request(RecorderAction::ToggleRecord);
    recorder.service(&mut runtime, 32, 0.016);
    let capture_refused = mount(recorder);
    assert_status(&capture_refused, "capture refused by runtime");

    let mut recorder = Recorder::default();
    let mut runtime = Runtime::default();
    recorder.service(&mut runtime, 0, 0.016);
    recorder.request(RecorderAction::Arm(recorder.session.tracks[0].id));
    recorder.service(&mut runtime, 16, 0.016);
    recorder.request(RecorderAction::ToggleRecord);
    recorder.service(&mut runtime, 32, 0.016);
    let token = runtime.accepted.take().expect("capture was admitted");
    runtime.fail_playback = true;
    runtime.completed = Some(CaptureCompletion {
        token,
        sample_rate: 48_000,
        samples: vec![0.25, -0.25],
    });
    recorder.service(&mut runtime, 48, 0.016);
    let playback_refused = mount(recorder);
    assert_status(&playback_refused, "playback refused by runtime");
}
