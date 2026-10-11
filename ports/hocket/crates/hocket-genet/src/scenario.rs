// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket observations over Mesquite's shared scenario/capture lifecycle.
//! Set HOCKET_SCENARIO and HOCKET_CAPTURE_DIR; isolate profile paths for any
//! explicit appearance writes. Mesquite owns input text, retained clicks,
//! resizing, captures, presentation evidence, completion and bounded waits.

use crate::{AppState, Child, Logic};
use cambium_genet_winit_host::AppCtx;
type Ctx<'a> = AppCtx<'a, AppState, Logic, Child>;
pub type Run = mesquite::Lane<Product>;
pub struct Product {
    sheet: String,
}

pub fn load() -> Option<Run> {
    let config = mesquite::LaneConfig::from_env("HOCKET")?;
    Some(
        mesquite::Lane::from_config(
            config,
            Product {
                sheet: crate::theme::sheet(),
            },
            cambium_genet_winit_host::read_file,
        )
        .expect("load Hocket scenario"),
    )
}

pub fn drive(lane: &mut Run, ctx: &mut Ctx<'_>) {
    lane.product_mut().sheet = ctx.runner.state().appearance.stylesheet();
    lane.after_frame(ctx);
}

impl mesquite::Product for Product {
    type State = AppState;
    type Logic = Logic;
    type View = Child;
    const KIND: &'static str = "hocket";
    const SURFACE: &'static str = "hocket";
    const LOG_PREFIX: &'static str = "hocket-genet";
    fn sheet(&self) -> &str {
        &self.sheet
    }
    fn snapshot(&self, ctx: &Ctx<'_>, _: usize, _: f32) -> taproot::ProbeSnapshot {
        let state = ctx.runner.state();
        let appearance = &state.appearance;
        taproot::ProbeSnapshot::default()
            .with_field("status", state.project_status_label())
            .with_field(
                "appearance-device-free",
                crate::appearance::device_free_receipt().to_string(),
            )
            .with_field(
                "appearance-authoring",
                appearance.authoring_available.to_string(),
            )
            .with_field("appearance-theme", appearance.active_id())
            .with_field(
                "appearance-mode",
                appearance
                    .applied()
                    .and_then(|applied| applied.resolved.theme_mode.as_ref())
                    .map_or("default".into(), |mode| mode.as_key()),
            )
            .with_field("appearance-workshop", appearance.editor_open.to_string())
            .with_field(
                "appearance-name",
                appearance.workshop.draft_theme().name.clone(),
            )
            .with_field(
                "appearance-dirty",
                appearance.workshop.has_changes().to_string(),
            )
            .with_field("tracks", state.session.tracks.len().to_string())
            .with_field("bpm", state.session.bpm.to_string())
            .with_field(
                "master-clock",
                state.session.master_clock_enabled.to_string(),
            )
            .with_field("recording", state.is_recording().to_string())
            .with_field(
                "armed-track",
                state
                    .armed_index()
                    .map_or("none".into(), |index| index.to_string()),
            )
            .with_field(
                "muted-tracks",
                state
                    .session
                    .tracks
                    .iter()
                    .filter(|track| track.muted)
                    .count()
                    .to_string(),
            )
            .with_field("solo-tracks", state.solo.len().to_string())
            .with_field("project-busy", state.is_project_io_active().to_string())
            .with_field("history-head", format!("{:?}", state.history.head))
    }
    fn viewport(&self, ctx: &Ctx<'_>) -> Option<mesquite::Viewport> {
        // Mesquite compares the recorder, excluding the appearance controls.
        // Geometry is the same retained host rect used for native input.
        if ctx.runner.state().appearance.editor_open {
            return None;
        }
        let node = taproot::matching(&ctx.runner.dom().borrow(), &taproot::Selector::class("app"))
            .into_iter()
            .next()?;
        let (x, y, width, height) = ctx.painted_rect(node)?;
        let scale = ctx.window?.scale_factor() as f32 * ctx.ui_zoom;
        Some(mesquite::Viewport::new([x, y, width, height], scale, 1.0))
    }
    fn busy(&self, ctx: &Ctx<'_>, capture_pending: bool) -> Option<bool> {
        Some(capture_pending || ctx.runner.state().is_project_io_active())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium_genet_winit_host::{CaptureFn, CloseRequest, Frame};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn mounted(
        dir: &tempfile::TempDir,
        body: &str,
        backend: impl mesquite::CaptureBackend + 'static,
    ) -> (
        cambium_genet_winit_host::Harness<AppState, Logic, Child>,
        Rc<RefCell<crate::Runtime>>,
        Rc<Cell<i32>>,
    ) {
        let exit = Rc::new(Cell::new(0));
        let lane = mesquite::Lane::new(
            Product {
                sheet: crate::theme::sheet(),
            },
            Some(taproot::Scenario::parse(body).unwrap()),
            Some(dir.path().join("receipt.json")),
            None,
            exit.clone(),
        )
        .with_capture_backend(backend);
        let runtime = Rc::new(RefCell::new(crate::Runtime::new(Some(lane))));
        let host = crate::appearance_tests::mounted_with_runtime(dir, runtime.clone());
        (host, runtime, exit)
    }

    struct ReadbackFailure;
    impl mesquite::CaptureBackend for ReadbackFailure {
        fn arm(&mut self, _: &mut Option<CaptureFn>) -> mesquite::Readback {
            Box::new(|| Some(Err("fixture GPU map refused".into())))
        }
        fn writes_files(&self) -> bool {
            false
        }
    }

    struct DelayedPixels;
    impl mesquite::CaptureBackend for DelayedPixels {
        fn arm(&mut self, _: &mut Option<CaptureFn>) -> mesquite::Readback {
            let mut polls = 0;
            Box::new(move || {
                polls += 1;
                (polls == 3).then(|| {
                    Ok(Frame {
                        width: 2,
                        height: 1,
                        rgba: vec![1, 2, 3, 255, 4, 5, 6, 255],
                    })
                })
            })
        }
        fn writes_files(&self) -> bool {
            false
        }
    }

    #[test]
    fn readback_failure_is_a_failed_receipt_without_a_completed_capture() {
        let dir = tempfile::tempdir().unwrap();
        let (mut host, runtime, exit) = mounted(&dir, "capture refused\n", ReadbackFailure);
        for _ in 0..6 {
            host.after_frame();
        }
        let runtime = runtime.borrow();
        let lane = runtime.scenario.as_ref().unwrap();
        assert!(lane.finished());
        assert!(lane.captures().is_empty());
        assert_eq!(exit.get(), 1);
        let receipt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("receipt.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["ok"], false);
        assert!(
            receipt["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|error| { error.as_str().unwrap().contains("fixture GPU map refused") })
        );
        assert!(host.close_requested());
    }

    #[test]
    fn native_close_defers_until_requested_pixels_land_and_records_interruption() {
        let dir = tempfile::tempdir().unwrap();
        let (mut host, runtime, exit) =
            mounted(&dir, "capture requested\nsettle 100\n", DelayedPixels);
        host.after_frame();
        assert_eq!(
            runtime.borrow().scenario.as_ref().unwrap().captures().len(),
            0
        );
        host.request_close(CloseRequest::Native);
        assert!(
            !host.close_requested(),
            "Mesquite must finish its queued readback"
        );
        for _ in 0..6 {
            host.after_frame();
        }
        let runtime = runtime.borrow();
        let lane = runtime.scenario.as_ref().unwrap();
        assert!(lane.finished());
        assert_eq!(lane.captures().len(), 1);
        assert_eq!(exit.get(), 1);
        assert!(host.close_requested());
        let receipt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("receipt.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["ok"], false);
        assert!(receipt["errors"].as_array().unwrap().iter().any(|error| {
            error
                .as_str()
                .unwrap()
                .contains("window closed before the scenario completed")
        }));
    }

    fn click(host: &mut cambium_genet_winit_host::Harness<AppState, Logic, Child>, action: &str) {
        assert!(
            host.click_on(&taproot::Selector::role("button").with_attr("data-action", action)),
            "{action}"
        );
    }

    #[test]
    fn cancelling_dirty_close_keeps_the_scenario_running() {
        let dir = tempfile::tempdir().unwrap();
        let (mut host, runtime, exit) = mounted(&dir, "settle 100\n", DelayedPixels);
        click(&mut host, "toggle-appearance");
        click(&mut host, "edit-appearance");
        host.request_close(CloseRequest::Native);
        host.relayout();
        click(&mut host, "cancel-close");
        for _ in 0..3 {
            host.after_frame();
        }
        assert!(!runtime.borrow().scenario.as_ref().unwrap().finished());
        assert!(!host.close_requested());
        assert_eq!(exit.get(), 0);
        assert!(!dir.path().join("receipt.json").exists());
    }

    #[test]
    fn appearance_save_close_finishes_an_interrupted_scenario_before_exit() {
        let dir = tempfile::tempdir().unwrap();
        let (mut host, runtime, exit) = mounted(&dir, "settle 100\n", DelayedPixels);
        click(&mut host, "toggle-appearance");
        click(&mut host, "edit-appearance");
        host.request_close(CloseRequest::Native);
        host.relayout();
        click(&mut host, "save-close");
        assert!(!host.close_requested());
        for _ in 0..3 {
            host.after_frame();
        }
        assert!(runtime.borrow().scenario.as_ref().unwrap().finished());
        assert!(host.close_requested());
        assert_eq!(exit.get(), 1);
        assert!(dir.path().join("themes.json").exists());
        assert!(!dir.path().join("choice.json").exists());
        assert!(dir.path().join("receipt.json").exists());
    }

    #[test]
    fn existing_and_appearance_scenarios_parse_without_a_product_grammar_fork() {
        for body in [
            include_str!("../../../scenarios/update_status.scn"),
            include_str!("../../../scenarios/update_action.scn"),
            include_str!("../../../scenarios/handoff_circle.scn"),
            include_str!("../../../scenarios/tabard_appearance.scn"),
            include_str!("../../../scenarios/tabard_appearance_reopen.scn"),
            include_str!("../../../scenarios/tabard_appearance_same_id.scn"),
            include_str!("../../../scenarios/recorder_controls.scn"),
        ] {
            taproot::Scenario::parse(body).expect("shared scenario grammar");
        }
    }
}
