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
    fn busy(&self, _: &Ctx<'_>, capture_pending: bool) -> Option<bool> {
        Some(capture_pending)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn existing_and_appearance_scenarios_parse_without_a_product_grammar_fork() {
        for body in [
            include_str!("../../../scenarios/update_status.scn"),
            include_str!("../../../scenarios/update_action.scn"),
            include_str!("../../../scenarios/handoff_circle.scn"),
            include_str!("../../../scenarios/tabard_appearance.scn"),
            include_str!("../../../scenarios/tabard_appearance_reopen.scn"),
            include_str!("../../../scenarios/tabard_appearance_same_id.scn"),
        ] {
            taproot::Scenario::parse(body).expect("shared scenario grammar");
        }
    }
}
