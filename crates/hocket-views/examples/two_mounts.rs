// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Second host for the shared recorder, using an explicitly synthetic backend.
//! HOCKET_VIEWS_SCENARIO / HOCKET_VIEWS_CAPTURE_DIR use Mesquite's finite lane.
//! No Engine, audio device, project path, identity or updater is constructed.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use cambium::{el, text};
use cambium_genet_winit_host::{AppCtx, HostHooks, HostOptions, Init, run};
use hocket_recorder::{
    AudioAvailability, AudioObservation, CaptureCompletion, CaptureMode, CapturePhase,
    CaptureToken, LayerKey, Recorder, RecorderAction, RecorderAudio,
};
use hocket_views::{Child, RecorderLayout, RecorderLeaves, RecorderMount, surface};

const FULL: RecorderMount = RecorderMount::new(11);
const COMPACT: RecorderMount = RecorderMount::new(12);
const RATE: u32 = 48_000;
type Logic = fn(&MountHost) -> Child<MountHost>;
type Ctx<'a> = AppCtx<'a, MountHost, Logic, Child<MountHost>>;

/// Deliberately distinct outer state: the embedding contract is not AppState.
struct MountHost {
    full: Recorder,
    compact: Recorder,
    full_audio: SyntheticAudio,
    compact_audio: SyntheticAudio,
    now_ms: u64,
}

impl MountHost {
    fn seeded() -> Self {
        let mut full = Recorder::default();
        let mut full_audio = SyntheticAudio::default();
        full.service(&mut full_audio, 0, 1.0 / 60.0);
        let first = full.session.tracks[0].id;
        full.request(RecorderAction::Arm(first));
        full.request(RecorderAction::ToggleRecord);
        full.service(&mut full_audio, 16, 1.0 / 60.0);
        full.service(&mut full_audio, 32, 1.0 / 60.0);
        let mut compact = Recorder::new(
            full.session.clone(),
            full.history.clone(),
            full.store.clone(),
            full.missing_media.clone(),
        );
        // The same model ids in two mounts exercise key scoping directly.
        compact.reconcile_all_playback();
        let mut compact_audio = SyntheticAudio::default();
        compact.service(&mut compact_audio, 32, 1.0 / 60.0);
        let mut host = Self {
            full,
            compact,
            full_audio,
            compact_audio,
            now_ms: 32,
        };
        host.tick();
        host
    }

    fn tick(&mut self) {
        self.now_ms += 16;
        self.full
            .service(&mut self.full_audio, self.now_ms, 1.0 / 60.0);
        self.compact
            .service(&mut self.compact_audio, self.now_ms, 1.0 / 60.0);
    }
}

fn dispatch_full(state: &mut MountHost, action: RecorderAction) {
    state.full.request(action);
}
fn dispatch_compact(state: &mut MountHost, action: RecorderAction) {
    state.compact.request(action);
}

fn root(state: &MountHost) -> Child<MountHost> {
    Box::new(
        el(
            "div",
            (
                el(
                    "div",
                    text("Reusable Hocket recorder · synthetic audio backend · no devices"),
                )
                .attr("class", "mount-heading"),
                el(
                    "div",
                    text("Host-owned table/transport styles remain independent"),
                )
                .attr("class", "table transport unrelated-host"),
                el(
                    "div",
                    (
                        el(
                            "div",
                            (
                                el("div", text("Full mount")).attr("class", "mount-label"),
                                surface(&state.full, FULL, RecorderLayout::Full, dispatch_full),
                            ),
                        )
                        .attr("class", "mount-panel full-panel"),
                        el(
                            "div",
                            (
                                el("div", text("Compact mount")).attr("class", "mount-label"),
                                surface(
                                    &state.compact,
                                    COMPACT,
                                    RecorderLayout::Compact,
                                    dispatch_compact,
                                ),
                            ),
                        )
                        .attr("class", "mount-panel compact-panel"),
                    ),
                )
                .attr("class", "mount-row"),
            ),
        )
        .attr("class", "mount-host"),
    )
}

fn sheet() -> String {
    format!(
        "{}\n{}",
        hocket_views::theme::sheet(),
        r#"
.mount-host { display:flex; flex-direction:column; width:100%; height:100%; min-height:0; background-color:#17130e; color:#ece3d2; padding:12px; box-sizing:border-box; font-family:sans-serif; }
.mount-heading { font-size:16px; margin-bottom:8px; }
.table { padding:2px; }
.transport { font-size:11px; }
.unrelated-host { height:18px; background-color:#26352b; margin-bottom:8px; }
.mount-row { display:flex; flex-grow:1; min-height:0; gap:12px; }
.mount-panel { display:flex; flex-direction:column; min-height:0; min-width:0; }
.full-panel { flex-grow:1; }
.compact-panel { width:340px; flex-shrink:0; }
.mount-label { height:22px; color:#e0a64b; }
"#
    )
}

/// A finite producer of actual PCM and meter observations. Its synthetic
/// capture completes on the next service poll, independently of native input.
struct SyntheticAudio {
    pending: Option<(CaptureToken, CaptureMode)>,
    voices: BTreeSet<LayerKey>,
    bpm: f32,
    numerator: u8,
    denominator: u8,
    refuse_capture: bool,
}

impl Default for SyntheticAudio {
    fn default() -> Self {
        Self {
            pending: None,
            voices: BTreeSet::new(),
            bpm: 120.0,
            numerator: 4,
            denominator: 4,
            refuse_capture: false,
        }
    }
}

impl RecorderAudio for SyntheticAudio {
    fn availability(&self) -> AudioAvailability {
        AudioAvailability {
            sample_rate: RATE,
            input: true,
            error: None,
        }
    }
    fn poll(&mut self) -> AudioObservation {
        let completed = self.pending.take().map(|(token, mode)| {
            let bars = match mode {
                CaptureMode::Clocked { bars, .. } => bars,
                CaptureMode::Free => 1,
            };
            let frames = (RATE as f64 * 60.0 / self.bpm as f64 * f64::from(self.numerator) * 4.0
                / f64::from(self.denominator)
                * f64::from(bars))
            .round() as usize;
            let samples = (0..frames)
                .map(|i| ((i % 96) as f32 / 48.0 - 1.0) * 0.4)
                .collect();
            CaptureCompletion {
                token,
                sample_rate: RATE,
                samples,
            }
        });
        AudioObservation {
            phase: if completed.is_some() {
                CapturePhase::Complete
            } else {
                CapturePhase::Idle
            },
            meter_db: if self.voices.is_empty() {
                [f32::NEG_INFINITY; 2]
            } else {
                [-15.0, -18.0]
            },
            completed,
            ..Default::default()
        }
    }
    fn set_clock(
        &mut self,
        bpm: f32,
        numerator: u8,
        denominator: u8,
        _: bool,
    ) -> Result<(), String> {
        self.bpm = bpm;
        self.numerator = numerator;
        self.denominator = denominator;
        Ok(())
    }
    fn set_click(&mut self, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn begin_capture(&mut self, token: CaptureToken, mode: CaptureMode) -> Result<(), String> {
        if self.refuse_capture {
            return Err("synthetic capture refused".into());
        }
        self.pending = Some((token, mode));
        Ok(())
    }
    fn stop_free_capture(&mut self, _: CaptureToken) -> Result<(), String> {
        Ok(())
    }
    fn play_layer(
        &mut self,
        key: LayerKey,
        _: Vec<f32>,
        rate: u32,
        _: f32,
        _: bool,
    ) -> Result<(), String> {
        if rate != RATE {
            return Err("synthetic playback rate mismatch".into());
        }
        self.voices.insert(key);
        Ok(())
    }
    fn stop_layer(&mut self, key: LayerKey) -> Result<(), String> {
        self.voices.remove(&key);
        Ok(())
    }
}

struct Runtime {
    full: RecorderLeaves,
    compact: RecorderLeaves,
    lane: Option<mesquite::Lane<Product>>,
}

impl Runtime {
    fn new() -> Self {
        let lane = mesquite::LaneConfig::from_env("HOCKET_VIEWS").map(|config| {
            mesquite::Lane::from_config(
                config,
                Product { sheet: sheet() },
                cambium_genet_winit_host::read_file,
            )
            .expect("load recorder mount scenario")
        });
        Self {
            full: RecorderLeaves::new(FULL),
            compact: RecorderLeaves::new(COMPACT),
            lane,
        }
    }
}

fn hooks(runtime: Rc<RefCell<Runtime>>) -> HostHooks<MountHost, Logic, Child<MountHost>> {
    let mut hooks = HostHooks::inert();
    let frame = runtime.clone();
    hooks.frame = Box::new(move |ctx: &mut Ctx<'_>| {
        ctx.runner.update(MountHost::tick);
        let mut runtime = frame.borrow_mut();
        runtime.full.reconcile(ctx.leaves, &ctx.runner.state().full);
        runtime
            .compact
            .reconcile(ctx.leaves, &ctx.runner.state().compact);
        true
    });
    hooks.after_dispatch = Box::new(|ctx| {
        ctx.runner.update(MountHost::tick);
    });
    hooks.after_frame = Box::new(move |ctx| {
        if let Some(lane) = runtime.borrow_mut().lane.as_mut() {
            lane.after_frame(ctx);
        }
    });
    hooks
}

struct Product {
    sheet: String,
}
impl mesquite::Product for Product {
    type State = MountHost;
    type Logic = Logic;
    type View = Child<MountHost>;
    const KIND: &'static str = "hocket-views-two-mounts";
    const SURFACE: &'static str = "hocket-recorder";
    const LOG_PREFIX: &'static str = "hocket-views";
    fn sheet(&self) -> &str {
        &self.sheet
    }
    fn snapshot(&self, ctx: &Ctx<'_>, _: usize, _: f32) -> taproot::ProbeSnapshot {
        let state = ctx.runner.state();
        taproot::ProbeSnapshot::default()
            .with_field("backend", "synthetic")
            .with_field("full-tracks", state.full.session.tracks.len().to_string())
            .with_field(
                "compact-tracks",
                state.compact.session.tracks.len().to_string(),
            )
            .with_field("full-bpm", state.full.session.bpm.to_string())
            .with_field("compact-bpm", state.compact.session.bpm.to_string())
            .with_field(
                "full-layers",
                state
                    .full
                    .session
                    .tracks
                    .iter()
                    .map(|t| t.layers.len())
                    .sum::<usize>()
                    .to_string(),
            )
            .with_field(
                "compact-layers",
                state
                    .compact
                    .session
                    .tracks
                    .iter()
                    .map(|t| t.layers.len())
                    .sum::<usize>()
                    .to_string(),
            )
            .with_field("full-voices", state.full.accepted_voice_count().to_string())
            .with_field(
                "compact-voices",
                state.compact.accepted_voice_count().to_string(),
            )
            .with_field("full-input", state.full.input_available().to_string())
            .with_field("compact-input", state.compact.input_available().to_string())
    }
}

fn main() {
    let runtime = Rc::new(RefCell::new(Runtime::new()));
    run(
        HostOptions {
            title: "Hocket reusable views · synthetic backend".into(),
            initial_logical_size: (1_240.0, 820.0),
            ..Default::default()
        },
        |_, _, _| Init {
            state: MountHost::seeded(),
            logic: root as Logic,
            sheet: sheet(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks(runtime),
    )
    .expect("run reusable recorder mount host");
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium_genet_winit_host::Harness;
    use taproot::Selector;

    type Host = Harness<MountHost, Logic, Child<MountHost>>;
    fn mounted() -> Host {
        let mut host = Harness::with_hooks(
            Init {
                state: MountHost::seeded(),
                logic: root as Logic,
                sheet: sheet(),
                fonts: Vec::new(),
                images: Vec::new(),
            },
            hooks(Rc::new(RefCell::new(Runtime {
                full: RecorderLeaves::new(FULL),
                compact: RecorderLeaves::new(COMPACT),
                lane: None,
            }))),
        );
        host.layout_at(1_240.0, 900.0);
        host.prepare_frame();
        host
    }
    fn control(mount: RecorderMount, role: &str, label: &str) -> Selector {
        Selector::role(role)
            .with_attr("data-recorder-mount", mount.id().to_string())
            .containing(label)
    }
    fn click(host: &mut Host, selector: Selector) {
        assert!(host.click_on(&selector), "mounted control: {selector:?}");
        host.prepare_frame();
        host.relayout();
    }

    #[test]
    fn full_and_compact_gestures_route_to_independent_controllers() {
        let mut host = mounted();
        click(&mut host, control(FULL, "button", "Increase tempo"));
        assert_eq!(host.state().full.session.bpm, 122.0);
        assert_eq!(host.state().compact.session.bpm, 120.0);
        click(&mut host, control(COMPACT, "button", "Increase tempo"));
        assert_eq!(host.state().compact.session.bpm, 122.0);
        click(&mut host, control(FULL, "switch", "Arm track 2"));
        assert_eq!(host.state().full.armed_index(), Some(1));
        assert_eq!(host.state().compact.armed_index(), Some(0));
        click(&mut host, control(COMPACT, "switch", "Mute track 1"));
        assert!(host.state().compact.session.tracks[0].muted);
        assert!(!host.state().full.session.tracks[0].muted);
    }

    #[test]
    fn second_host_promotes_capture_and_exposes_real_media_leaves() {
        let mut host = mounted();
        let initial = host.state().compact.session.tracks[0].layers.len();
        click(&mut host, control(COMPACT, "switch", "Record"));
        host.prepare_frame();
        host.relayout();
        assert_eq!(
            host.state().compact.session.tracks[0].layers.len(),
            initial + 1
        );
        assert_eq!(host.state().full.session.tracks[0].layers.len(), initial);
        assert!(host.state().compact.track_waveform_available(0));
        assert!(host.state().compact.meter_level(0) > 0.0);
        let track = host.state().compact.session.tracks[0].id;
        assert_ne!(FULL.wave_key(track), COMPACT.wave_key(track));
        for mount in [FULL, COMPACT] {
            let key = mount.wave_key(track).to_string();
            assert!(host.with_dom(|dom| {
                !taproot::matching(
                    dom,
                    &Selector::class("hocket-recorder-wave-leaf").with_attr("key", &key),
                )
                .is_empty()
            }));
        }
    }

    #[test]
    fn rejected_synthetic_capture_preserves_existing_layers_and_other_mount() {
        let mut host = mounted();
        let initial = host.state().compact.session.tracks[0].layers.len();
        host.update(|state| state.compact_audio.refuse_capture = true);
        click(&mut host, control(COMPACT, "switch", "Record"));
        assert!(!host.state().compact.is_recording());
        assert_eq!(host.state().compact.session.tracks[0].layers.len(), initial);
        assert!(
            host.state()
                .compact
                .audio_status_label()
                .contains("refused")
        );
        assert_eq!(host.state().full.session.tracks[0].layers.len(), initial);
    }

    #[test]
    fn duplicate_phrase_second_occurrence_routes_and_renders_independently() {
        let mut host = mounted();
        host.update(|state| {
            let mut alias = state.compact.session.tracks[0].layers[0];
            alias.gain = 0.25;
            state.compact.session.tracks[0].layers.push(alias);
        });
        host.prepare_frame();
        host.relayout();
        let track = &host.state().compact.session.tracks[0];
        let phrase = track.layers[0].phrase_id;
        let first = COMPACT.layer_wave_key(track.id, 0, phrase);
        let second = COMPACT.layer_wave_key(track.id, 1, phrase);
        assert_ne!(first, second);
        for key in [first, second] {
            assert!(host.with_dom(|dom| {
                !taproot::matching(
                    dom,
                    &Selector::class("hocket-recorder-wave-leaf").with_attr("key", key.to_string()),
                )
                .is_empty()
            }));
        }
        click(&mut host, control(COMPACT, "switch", "Mute layer L2"));
        assert!(!host.state().compact.session.tracks[0].layers[0].muted);
        assert!(host.state().compact.session.tracks[0].layers[1].muted);
        assert!(!host.state().full.session.tracks[0].layers[0].muted);
    }
}
