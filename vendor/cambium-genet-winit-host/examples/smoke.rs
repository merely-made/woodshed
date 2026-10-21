// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Headed smoke: the host's whole input surface, driven semantically.
//!
//! Run it by hand to poke at it:
//!
//! ```text
//! cargo run -p cambium-genet-winit-host --example smoke
//! ```
//!
//! Or run it bounded, self-driving, and leaving a receipt — the form CI and a
//! headed-verify pass want:
//!
//! ```text
//! HOST_SMOKE_SCENARIO=crates/cambium/cambium-genet-winit-host/examples/smoke.scn \
//! HOST_SMOKE_RECEIPT=smoke.receipt \
//!   cargo run -p cambium-genet-winit-host --example smoke
//! ```
//!
//! The scenario drives by **role and label**, not by coordinates, and every
//! pointer event it produces goes back through the host's own routing via
//! [`HostPointer`] — so what the receipt exercised is the shipping code path.
//! Captures are in-process readbacks of the frame that was presented, recorded
//! as size + digest, which is how the receipt can claim a frame was really
//! drawn (and, after a resize, really redrawn at the new size).
//!
//! The scenario runs on the shared [`mesquite::Lane`]; this file supplies only
//! the smoke's own half through [`mesquite::Product`], and is the reference for how an
//! application does that.

use mesquite::{CaptureRecord, LaneConfig};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use taproot::ProbeSnapshot;

use cambium::{
    AnyView, GenetCtx, GenetElement, PointerEvent, PointerPhase, WheelEvent, clickable, el,
    focusable, on_pointer, on_wheel, text,
};
#[cfg(not(target_os = "macos"))]
use cambium_genet_winit_host::WindowCommands;
use cambium_genet_winit_host::{
    AppCtx, AppFrameInsets, Frame, HostHooks, HostOptions, Init, Runner, WindowFrame, run,
};

// ------------------------------------------------------------------ state

#[derive(Default)]
struct Smoke {
    /// The window-verb seam. One field, not four bools.
    #[cfg(not(target_os = "macos"))]
    window: WindowCommands,
    clicks: usize,
    /// 0..=1, driven by dragging the rail.
    level: f32,
    /// Wheel notches the panel absorbed.
    notches: i32,
    /// Semantic transitions the scenario can assert on.
    events: Vec<String>,
    /// The same frame policy used to create the native window. The view reads
    /// it too, which prevents a host frame and an app title bar appearing
    /// together.
    window_frame: WindowFrame,
    /// Transparent outer margin used by the X11 shadow receipt.
    app_frame_inset: u32,
}

impl Smoke {
    fn note(&mut self, event: String) {
        self.events.push(event);
    }
}

type Child = Box<dyn AnyView<Smoke, (), GenetCtx, GenetElement>>;
type Logic = fn(&Smoke) -> Child;

/// The client-side title bar.
///
/// The whole bar is `--app-region: drag` (see the sheet), so pressing it moves
/// the window, double-clicking it maximizes, and right-clicking raises the
/// system menu — none of which this file implements. Windows/Linux add the
/// product-drawn caption buttons; macOS keeps AppKit's traffic lights and the
/// same stylesheet uses `--titlebar-area-*` to place only the title beside them.
#[cfg(not(target_os = "macos"))]
fn caption(label: &'static str, name: &'static str, verb: fn(&WindowCommands)) -> Child {
    Box::new(focusable(clickable(
        el("button", text(label))
            .attr("class", "caption")
            .attr("aria-label", name),
        move |s: &mut Smoke, _| {
            verb(&s.window);
            s.note(format!("window {name}"));
        },
    )))
}

fn title_bar() -> Child {
    #[cfg(target_os = "macos")]
    let children: Vec<Child> = vec![Box::new(
        el("div", text("host smoke")).attr("class", "caption-title"),
    )];
    #[cfg(not(target_os = "macos"))]
    let mut children: Vec<Child> = vec![Box::new(
        el("div", text("host smoke")).attr("class", "caption-title"),
    )];
    #[cfg(not(target_os = "macos"))]
    children.extend([
        caption("–", "Minimize", WindowCommands::minimize),
        caption("□", "Maximize", WindowCommands::toggle_maximize),
        caption("×", "Close", WindowCommands::close),
    ]);
    Box::new(el("div", children).attr("class", "bar"))
}

fn root(state: &Smoke) -> Child {
    let filled = (state.level * 240.0).round() as i32;
    let content: Child = Box::new(
        el(
            "div",
            (
                el("div", text("cambium-genet-winit-host smoke")).attr("class", "title"),
                focusable(clickable(
                    el("button", text(format!("clicked {} times", state.clicks)))
                        .attr("class", "button"),
                    |s: &mut Smoke, _| {
                        s.clicks += 1;
                        let n = s.clicks;
                        s.note(format!("clicks {n}"));
                    },
                )),
                el("span", text("Reset all controls")).attr("id", "reset-label"),
                focusable(clickable(
                    el("button", text("Restore defaults"))
                        .attr("class", "button")
                        .attr("aria-label", "Superseded reset label")
                        .attr("aria-labelledby", "reset-label"),
                    |s: &mut Smoke, _| {
                        s.clicks = 0;
                        s.level = 0.0;
                        s.note("reset".into());
                    },
                )),
                // A drag rail: the receipt that Down/Move/Up carry the
                // captured element's own coordinates. The handler normalizes
                // with nothing but `local` and `size`.
                on_pointer(
                    el(
                        "div",
                        el("div", ())
                            .attr("class", "rail-fill")
                            .attr("style", format!("width:{filled}px;")),
                    )
                    .attr("class", "rail")
                    .attr("role", "slider")
                    .attr("aria-label", "Level"),
                    |s: &mut Smoke, e: PointerEvent| {
                        if e.size.0 > 0.0 && !matches!(e.phase, PointerPhase::Up) {
                            let level = (e.local.0 / e.size.0).clamp(0.0, 1.0);
                            if (level - s.level).abs() > 0.004 {
                                s.level = level;
                                let pct = (level * 100.0).round() as i32;
                                s.note(format!("level {pct}"));
                            }
                        }
                    },
                ),
                // A wheel panel: the receipt that a handler sees the notch
                // before the layout scrolls, and can keep it.
                on_wheel(
                    el("div", text(format!("wheel notches: {}", state.notches)))
                        .attr("class", "panel")
                        .attr("aria-label", "Wheel panel"),
                    |s: &mut Smoke, e: WheelEvent| {
                        s.notches += e.delta.1.signum() as i32;
                        let n = s.notches;
                        s.note(format!("notches {n}"));
                        // Keep the notch: the page behind must not also scroll.
                        e.prevent_default();
                    },
                ),
            ),
        )
        .attr("class", "content"),
    );
    let mut children = Vec::with_capacity(2);
    if state.window_frame == WindowFrame::App {
        children.push(title_bar());
    }
    children.push(content);
    let frame = el("div", children).attr("class", "frame");
    let frame = if state.app_frame_inset > 0 {
        let inset = state.app_frame_inset;
        let framed = frame.attr(
            "style",
            format!(
                "margin:{inset}px; height:calc(100% - {}px); box-shadow:0 4px 12px rgba(0,0,0,0.55); border-radius:7px;",
                inset * 2
            ),
        );
        // A root element's background propagates to the canvas. Keep the
        // product frame one level down so the outer margin retains the alpha
        // that the native window and `_GTK_FRAME_EXTENTS` describe.
        return Box::new(el("div", framed).attr("style", "height:100%;"));
    } else {
        frame
    };
    Box::new(frame)
}

// The controls keep genet's UA `display: inline-block`, the standards-correct
// display for a form control. They used to need `display: block`: an inline-level
// box got no fragment of its own, so neither `painted_rect` nor a `taproot`
// selector could locate one, and an app had to style its controls to suit the
// driver. The engine now reads a rect back per inline box, so the scenario below
// drives these buttons by role and label with nothing arranged for it.
//
// Their `padding` / `margin` do not show yet: genet measures an inline-block from
// its content and any definite width/height, so the rest of the box model has
// still to reach the atomic-inline path. That is a sizing gap, not a reachability
// one — the rect the driver gets is the rect that paints.
const SHEET: &str = "
.bar { --app-region: drag; display: flex; margin-left: var(--titlebar-area-x, 0px); margin-top: var(--titlebar-area-y, 0px); width: var(--titlebar-area-width, 100%); height: 32px; background: #1d2733; }
.caption-title { flex-grow: 1; color: #9fb0c4; padding: 6px 8px; }
.caption { --app-region: no-drag; width: 32px; background: #29486b; color: #f0ebdd; }
.caption:hover { background: #3a5d85; }
.frame { font-size: 16px; background: #14181f; color: #f0ebdd; }
.content { padding: 24px; }
.title { margin-bottom: 12px; }
.button { padding: 8px 12px; margin-bottom: 8px; background: #29486b; color: #f0ebdd; width: 240px; }
.button:hover { background: #3a5d85; }
.button:focus { background: #4c76a4; }
.rail { width: 240px; height: 20px; background: #22303f; margin-bottom: 12px; }
.rail-fill { height: 20px; background: #7fb4e8; }
.panel { width: 240px; height: 60px; background: #1d2733; padding: 8px; }
";

// --------------------------------------------------------------- the lane

/// The smoke's half of the scenario lane: its snapshot, its one command, the
/// external probe's release file, and the frame facts its receipt claims.
struct SmokeLane {
    /// Per capture: alpha range, clear pixels, translucent pixels, and
    /// translucent pixels outside the visible frame.
    alpha: Vec<(String, u8, u8, usize, usize, usize)>,
    /// A shadow receipt requires both clear margin and blurred shadow pixels.
    require_alpha: bool,
    frame_inset: u32,
    /// Optional file an external platform probe creates when it has finished
    /// observing this exact window. This keeps a headed receipt bounded without
    /// guessing how many frames native inspection will take.
    release_file: Option<std::path::PathBuf>,
}

impl mesquite::Product for SmokeLane {
    type State = Smoke;
    type Logic = Logic;
    type View = Child;
    const KIND: &'static str = "host-smoke";
    const SURFACE: &'static str = "app";
    const LOG_PREFIX: &'static str = "host-smoke";

    fn sheet(&self) -> &str {
        SHEET
    }

    fn snapshot(
        &self,
        ctx: &AppCtx<'_, Smoke, Logic, Child>,
        _captures: usize,
        _: f32,
    ) -> ProbeSnapshot {
        let state = ctx.runner.state();
        ProbeSnapshot::default()
            .with_field("clicks", state.clicks.to_string())
            .with_field("level", ((state.level * 100.0).round() as i32).to_string())
            .with_field("notches", state.notches.to_string())
            .with_field(
                "window_frame",
                match state.window_frame {
                    WindowFrame::Host => "host",
                    WindowFrame::App => "app",
                },
            )
            .with_field("app_frame_inset", state.app_frame_inset.to_string())
            .with_field(
                "maximized",
                ctx.geometry
                    .map(|geometry| geometry.maximized.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
    }

    fn drain_events(&mut self, ctx: &mut AppCtx<'_, Smoke, Logic, Child>) -> Vec<String> {
        let mut drained = Vec::new();
        ctx.runner
            .update(|s| drained = std::mem::take(&mut s.events));
        drained
    }

    fn act(&mut self, ctx: &mut AppCtx<'_, Smoke, Logic, Child>, label: &str) -> bool {
        match label {
            "reset" => {
                ctx.runner.update(|s| {
                    s.clicks = 0;
                    s.level = 0.0;
                    s.note("reset".into());
                });
                true
            },
            _ => false,
        }
    }

    /// `await-release`: hold the run until the external probe writes
    /// `HOST_SMOKE_RELEASE_FILE`. (`resize` is the lane's own verb.)
    fn app_step(
        &mut self,
        _ctx: &mut AppCtx<'_, Smoke, Logic, Child>,
        _checkpoints: mesquite::Checkpoints<'_>,
        line: &str,
    ) -> Result<(), String> {
        match line.split_whitespace().next() {
            Some("await-release") => {
                let path = std::env::var_os("HOST_SMOKE_RELEASE_FILE")
                    .map(std::path::PathBuf::from)
                    .ok_or("await-release wants HOST_SMOKE_RELEASE_FILE")?;
                self.release_file = Some(path);
                Ok(())
            },
            _ => Err(format!("unknown verb: {line}")),
        }
    }

    fn busy_mut(
        &mut self,
        _ctx: &mut AppCtx<'_, Smoke, Logic, Child>,
        _capture_pending: bool,
    ) -> Option<bool> {
        Some(
            self.release_file
                .as_ref()
                .is_some_and(|path| !path.exists()),
        )
    }

    fn inspect(&mut self, name: &str, frame: &Frame) {
        let (pixels, _) = frame.rgba.as_chunks::<4>();
        let alpha = pixels.iter().map(|pixel| pixel[3]);
        let alpha_min = alpha.clone().min().unwrap_or(0);
        let alpha_max = alpha.clone().max().unwrap_or(0);
        let transparent = alpha.clone().filter(|a| *a == 0).count();
        let translucent = alpha.filter(|a| (1..=254).contains(a)).count();
        let inset = self.frame_inset as usize;
        let width = frame.width as usize;
        let height = frame.height as usize;
        let outer_translucent = pixels
            .iter()
            .enumerate()
            .filter(|(index, pixel)| {
                let x = index % width;
                let y = index / width;
                (1..=254).contains(&pixel[3])
                    && (x < inset
                        || x >= width.saturating_sub(inset)
                        || y < inset
                        || y >= height.saturating_sub(inset))
            })
            .count();
        self.alpha.push((
            name.to_string(),
            alpha_min,
            alpha_max,
            transparent,
            translucent,
            outer_translucent,
        ));
    }

    fn receipt_lines(&self) -> Vec<String> {
        self.alpha
            .iter()
            .map(|(name, min, max, transparent, translucent, outer)| {
                format!(
                    "alpha {name} alpha={min}..{max} transparent={transparent} translucent={translucent} outer-translucent={outer}"
                )
            })
            .collect()
    }

    /// Two claims a scenario's own grammar cannot make: the frames captured
    /// around a state change must differ, and the resize must change the
    /// frame size. A shadow run also needs clear margin and shadow pixels.
    fn receipt_checks(&self, captures: &[CaptureRecord]) -> Vec<String> {
        let digests: BTreeSet<u64> = captures.iter().map(|c| c.digest).collect();
        let sizes: BTreeSet<(u32, u32)> = captures.iter().map(|c| (c.width, c.height)).collect();
        let mut failures = Vec::new();
        if digests.len() < 2 {
            failures.push("frames must differ across a state change".to_string());
        }
        if sizes.len() < 2 {
            failures.push("frames must change size across the resize".to_string());
        }
        if self.require_alpha
            && !self
                .alpha
                .iter()
                .all(|(_, _, _, transparent, translucent, outer)| {
                    *transparent > 0 && *translucent > 0 && *outer > 0
                })
        {
            failures.push(
                "a frame-shadow capture needs transparent margin pixels and translucent shadow pixels outside the visible frame"
                    .to_string(),
            );
        }
        failures
    }
}

// --------------------------------------------------------------- wiring

fn main() {
    let window_frame = match std::env::var("HOST_SMOKE_WINDOW_FRAME") {
        Ok(value) if value.eq_ignore_ascii_case("host") => WindowFrame::Host,
        Ok(value) if value.eq_ignore_ascii_case("app") => WindowFrame::App,
        Ok(value) => panic!("HOST_SMOKE_WINDOW_FRAME must be 'host' or 'app', got {value:?}"),
        Err(std::env::VarError::NotPresent) => WindowFrame::App,
        Err(error) => panic!("HOST_SMOKE_WINDOW_FRAME is not valid Unicode: {error}"),
    };
    let app_frame_inset = match std::env::var("HOST_SMOKE_APP_FRAME_INSET") {
        Ok(value) => value.parse::<u32>().unwrap_or_else(|_| {
            panic!("HOST_SMOKE_APP_FRAME_INSET must be an unsigned integer, got {value:?}")
        }),
        Err(std::env::VarError::NotPresent) => 0,
        Err(error) => panic!("HOST_SMOKE_APP_FRAME_INSET is not valid Unicode: {error}"),
    };
    // HOST_SMOKE_SCENARIO, HOST_SMOKE_CAPTURE_DIR and HOST_SMOKE_RECEIPT.
    let lane = LaneConfig::from_env("HOST_SMOKE").map(|config| {
        let path = config.scenario.display().to_string();
        let lane = mesquite::Lane::from_config(
            config,
            SmokeLane {
                alpha: Vec::new(),
                require_alpha: app_frame_inset > 0,
                frame_inset: app_frame_inset,
                release_file: None,
            },
            cambium_genet_winit_host::read_file,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        eprintln!("[host-smoke] scenario armed: {path}");
        lane
    });
    let lane = Rc::new(RefCell::new(lane));

    let after_frame_lane = lane.clone();
    let hooks: HostHooks<Smoke, Logic, Child> = HostHooks {
        frame: Box::new(|_ctx| false),
        after_dispatch: Box::new(|_ctx| {}),
        after_frame: Box::new(move |ctx: &mut AppCtx<'_, Smoke, Logic, Child>| {
            if let Some(lane) = after_frame_lane.borrow_mut().as_mut() {
                lane.after_frame(ctx);
            }
        }),
        after_wake: Box::new(|_ctx| {}),
        close_request: Box::new(|_ctx, _request| cambium_genet_winit_host::CloseDisposition::Exit),
        focused_text: Box::new(|_runner: &Runner<Smoke, Logic, Child>| None),
        key_intercept: Box::new(|_runner, _press| false),
    };

    let options = HostOptions {
        title: "host smoke".into(),
        initial_logical_size: (420.0, 320.0),
        size_env: Some(("HOST_SMOKE_WIDTH".into(), "HOST_SMOKE_HEIGHT".into())),
        // The view reads this same value before deciding whether to include its
        // title row, so the two frame providers cannot overlap.
        window_frame,
        app_frame_insets: AppFrameInsets::uniform(app_frame_inset),
        ..Default::default()
    };
    run(
        options,
        // The application takes its end of the window-verb seam and keeps it
        // in state; the caption buttons call it from ordinary handlers.
        move |_window, _commands, _wake| Init {
            state: Smoke {
                #[cfg(not(target_os = "macos"))]
                window: _commands.clone(),
                window_frame,
                app_frame_inset,
                ..Smoke::default()
            },
            logic: root as Logic,
            sheet: SHEET.to_string(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    )
    .expect("event loop");
}
