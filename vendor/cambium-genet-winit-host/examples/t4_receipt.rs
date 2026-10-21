// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Netrender roadmap T4 receipt, from a Cambium host.
//!
//! `crates/cambium/cambium-rootstock/src/frame.rs`'s `sync_leaf_fragments`
//! registers every fragmentable custom leaf (Path-A, no `DrawExternalTexture`
//! / `DrawShadow` in its splice) as a netrender retained fragment, placed
//! per frame. netrender T4 changed what happens when that placement lands
//! inside an open `PushLayer` scope (clip, opacity, filter) — see
//! `C:\Users\mark_\Code\repos\netrender\netrender-notes\2026-09-04_wgpu_execution_graph_plan.md`,
//! "Retained placements inside layer scopes" and "Filter texture slots".
//! Genet's own hosts never place retained fragments; this is Cambium's own
//! exercise of that path, headless-GPU, self-driven, no window interaction.
//!
//! Run:
//! ```text
//! cargo run -p cambium-genet-winit-host --example t4_receipt --locked --offline
//! ```
//! Writes PNGs and a results log under `T4_RECEIPT_DIR` (default
//! `C:\Users\mark_\Code\testing\mere\cambium_t4_receipt_20260916`).
//!
//! Each correctness fixture lays out three side-by-side panels under one
//! window: the custom leaf under the ancestor layer (the retained path under
//! test), a plain `<div>` under the *same* ancestor layer (the independent,
//! non-fragment expansion — `PaintCx::fill_rect` is documented to produce a
//! `DrawRect` byte-identical to a native background fill, so this reference
//! never touches the fragment machinery), and the same plain `<div>` with no
//! ancestor layer at all (the non-vacuity control: if this equals the
//! reference, the layer changed nothing and the comparison would be
//! meaningless). A control fixture registers a Path-B (`cx.scene()`) leaf,
//! whose splice is a `DrawExternalTexture` — the exclusion `sync_leaf_fragments`
//! states in its doc comment — and checks the renderer's monotonic
//! `fragment_lower_count()` does not move when it is added.
//!
//! One important gap found while building this: genet has no CSS `filter` /
//! `backdrop-filter` property (grep across
//! `C:\Users\mark_\Code\repos\genet` for `FilterOp` — the only type
//! `paint_list_api::LayerSpec::filters` accepts — returns zero hits outside
//! `paint_list_api` itself). `sprigging::PaintCx` has no layer/filter push
//! either. So there is no way, today, for a Cambium document to place a
//! `LayerSpec` with a non-empty `filters` vec, and the netrender
//! "element-filter layer" and "filter texture slot" receipts cannot be
//! exercised from this host at all. This file therefore only covers the
//! `overflow:hidden` (clip) and `opacity` layer kinds; the filter kind is a
//! documented open item, not a silent omission.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cambium::{AnyView, GenetCtx, GenetElement, custom_leaf, el};
use cambium_genet_winit_host::{
    AppCtx, Frame, FrameProfile, HostHooks, HostOptions, Init, inert_hooks, run,
};
use paint_list_api::ColorF;
use sprigging::{Leaf, PaintCx, Size, SizeHint};

type Child = Box<dyn AnyView<St, (), GenetCtx, GenetElement>>;
type Logic = fn(&St) -> Child;

// ── colors: the same u8/255.0 expression on both the leaf's fill and the
// CSS string, so the two paths feed the rasterizer the identical f32 —
// `PaintCx::fill_rect`'s doc comment promises byte-identical output for a
// matching color, and this is what makes that promise checkable. ───────────

fn colorf(r: u8, g: u8, b: u8) -> ColorF {
    ColorF {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: 1.0,
    }
}

fn css_rgb(r: u8, g: u8, b: u8) -> String {
    format!("rgb({r}, {g}, {b})")
}

const LEAF_RGB: (u8, u8, u8) = (51, 102, 204);
const LEAF_W: u32 = 64;
const LEAF_H: u32 = 48;

// ── the two leaves under test ───────────────────────────────────────────

/// Path-A: paints one solid fill, then reports itself clean. A single leaf
/// instance is registered once per key and never repaints again, so any
/// pixel movement across frames comes only from the DOM changing where the
/// custom-leaf box sits — the "placement-only" frames the done-conditions
/// ask for.
struct SolidLeaf {
    color: ColorF,
    size: Size,
    painted: bool,
}

impl Leaf for SolidLeaf {
    fn measure(&mut self, _known: SizeHint, _available: SizeHint) -> Size {
        self.size
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let s = cx.size();
        cx.fill_rect(0.0, 0.0, s.width, s.height, self.color);
        self.painted = true;
    }
    fn paint_dirty(&self) -> bool {
        !self.painted
    }
}

/// Path-B: encodes an (empty is enough) vello scene, which
/// `sprigging::LeafRegistry::render_into` turns into a splice of exactly one
/// `PaintCmd::DrawExternalTexture` — the shape `sync_leaf_fragments` names in
/// its doc comment as the thing that must keep the inline path. This is the
/// control for done-condition 2.
struct SceneLeaf {
    started: bool,
}

impl Leaf for SceneLeaf {
    fn measure(&mut self, _known: SizeHint, _available: SizeHint) -> Size {
        Size {
            width: 48.0,
            height: 32.0,
        }
    }
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let _ = cx.scene();
        self.started = true;
    }
    fn paint_dirty(&self) -> bool {
        !self.started
    }
}

// ── the step table ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LayerKind {
    Clip,
    Opacity,
}

#[derive(Clone, Debug)]
enum StepKind {
    /// A moving placement of the fragmentable leaf, its independent
    /// expansion, and the no-layer non-vacuity control, under `kind`.
    Pair {
        kind: LayerKind,
        key: u64,
        dx: f32,
        dy: f32,
    },
    /// The Path-B exclusion control.
    Control,
    /// Timing: the leaf under a clip layer, steady position.
    TimingClip,
    /// Timing: the same leaf with no ancestor layer at all.
    TimingBare,
}

#[derive(Clone, Debug)]
struct Step {
    label: String,
    kind: StepKind,
}

fn build_steps() -> Vec<Step> {
    let mut steps = Vec::new();
    let moves = [(4.0f32, 4.0f32), (7.0, 6.0), (10.0, 8.0), (13.0, 10.0)];
    for (label, kind, key) in [
        ("clip", LayerKind::Clip, 101u64),
        ("opacity", LayerKind::Opacity, 102u64),
    ] {
        for (i, (dx, dy)) in moves.iter().enumerate() {
            steps.push(Step {
                label: format!("{label}-frame{i}"),
                kind: StepKind::Pair {
                    kind,
                    key,
                    dx: *dx,
                    dy: *dy,
                },
            });
        }
    }
    steps.push(Step {
        label: "control".into(),
        kind: StepKind::Control,
    });
    for i in 0..10 {
        steps.push(Step {
            label: format!("timing-clip-{i}"),
            kind: StepKind::TimingClip,
        });
    }
    for i in 0..10 {
        steps.push(Step {
            label: format!("timing-bare-{i}"),
            kind: StepKind::TimingBare,
        });
    }
    steps
}

// ── application state and view ──────────────────────────────────────────

struct St {
    steps: Rc<Vec<Step>>,
    idx: usize,
}

fn leaf_style(w: u32, h: u32, dx: f32, dy: f32) -> String {
    format!("display:block;position:absolute;left:{dx}px;top:{dy}px;width:{w}px;height:{h}px;")
}

fn plain_style(dx: f32, dy: f32, css: &str) -> String {
    format!(
        "display:block;position:absolute;left:{dx}px;top:{dy}px;width:{LEAF_W}px;height:{LEAF_H}px;background-color:{css};"
    )
}

fn pair_view(kind: LayerKind, key: u64, dx: f32, dy: f32) -> Child {
    let css = css_rgb(LEAF_RGB.0, LEAF_RGB.1, LEAF_RGB.2);
    let (aw, ah, extra) = match kind {
        LayerKind::Clip => (40u32, 30u32, "overflow:hidden;"),
        LayerKind::Opacity => (90u32, 70u32, "opacity:0.5;"),
    };
    let ancestor_style = |left: u32| {
        format!(
            "display:block;position:absolute;left:{left}px;top:16px;width:{aw}px;height:{ah}px;{extra}"
        )
    };
    let leaf_col = el(
        "div",
        custom_leaf::<St, ()>(key, LEAF_W, LEAF_H)
            .attr("style", leaf_style(LEAF_W, LEAF_H, dx, dy)),
    )
    .attr("style", ancestor_style(16));
    let reference_col = el(
        "div",
        el("div", ()).attr("style", plain_style(dx, dy, &css)),
    )
    .attr("style", ancestor_style(316));
    let bare_col = el("div", ()).attr(
        "style",
        format!(
            "display:block;position:absolute;left:{}px;top:16px;{}",
            616,
            plain_style(dx, dy, &css)
        ),
    );
    Box::new(
        el("div", (leaf_col, reference_col, bare_col))
            .attr("style", "position:relative;width:900px;height:300px;"),
    )
}

fn control_view() -> Child {
    Box::new(
        el(
            "div",
            el(
                "div",
                custom_leaf::<St, ()>(900, 48, 32).attr("style", leaf_style(48, 32, 8.0, 6.0)),
            )
            .attr(
                "style",
                "display:block;position:absolute;left:16px;top:16px;width:90px;height:70px;opacity:0.5;",
            ),
        )
        .attr("style", "position:relative;width:900px;height:300px;"),
    )
}

fn timing_clip_view() -> Child {
    Box::new(
        el(
            "div",
            el(
                "div",
                custom_leaf::<St, ()>(951, LEAF_W, LEAF_H).attr("style", leaf_style(LEAF_W, LEAF_H, 8.0, 6.0)),
            )
            .attr(
                "style",
                "display:block;position:absolute;left:16px;top:16px;width:40px;height:30px;overflow:hidden;",
            ),
        )
        .attr("style", "position:relative;width:900px;height:300px;"),
    )
}

fn timing_bare_view() -> Child {
    Box::new(
        el(
            "div",
            custom_leaf::<St, ()>(952, LEAF_W, LEAF_H)
                .attr("style", leaf_style(LEAF_W, LEAF_H, 24.0, 22.0)),
        )
        .attr("style", "position:relative;width:900px;height:300px;"),
    )
}

fn view(state: &St) -> Child {
    match &state.steps[state.idx].kind {
        StepKind::Pair { kind, key, dx, dy } => pair_view(*kind, *key, *dx, *dy),
        StepKind::Control => control_view(),
        StepKind::TimingClip => timing_clip_view(),
        StepKind::TimingBare => timing_bare_view(),
    }
}

// ── PNG (no dependency; stored deflate blocks only) ─────────────────────

fn adler32(data: &[u8]) -> u32 {
    const MODULO: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % MODULO;
        b = (b + a) % MODULO;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut i = 0;
    if data.is_empty() {
        out.push(1);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    }
    while i < data.len() {
        let remain = data.len() - i;
        let take = remain.min(65_535);
        let is_final = i + take >= data.len();
        out.push(u8::from(is_final));
        out.extend_from_slice(&(take as u16).to_le_bytes());
        out.extend_from_slice(&(!(take as u16)).to_le_bytes());
        out.extend_from_slice(&data[i..i + take]);
        i += take;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let mut raw = Vec::with_capacity((height * (1 + width * 4)) as usize);
    for row in 0..height {
        raw.push(0u8);
        let start = (row * width * 4) as usize;
        let end = start + (width * 4) as usize;
        raw.extend_from_slice(&rgba[start..end]);
    }
    let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &ihdr);
    let idat = deflate_stored(&raw);
    write_chunk(&mut png, b"IDAT", &idat);
    write_chunk(&mut png, b"IEND", &[]);
    std::fs::write(path, png)
}

fn crop(frame: &Frame, x0: u32, x1: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(((x1 - x0) * frame.height * 4) as usize);
    for row in 0..frame.height {
        let start = ((row * frame.width) + x0) as usize * 4;
        let end = ((row * frame.width) + x1) as usize * 4;
        out.extend_from_slice(&frame.rgba[start..end]);
    }
    out
}

// ── results ──────────────────────────────────────────────────────────────

struct PairResult {
    label: String,
    matched: bool,
    non_vacuous: bool,
    lower_count: Option<u64>,
}

#[derive(Default)]
struct Results {
    pairs: Vec<PairResult>,
    control_lower_before: Option<u64>,
    control_lower_after: Option<u64>,
    timing_clip: Vec<(String, FrameProfile)>,
    timing_bare: Vec<(String, FrameProfile)>,
}

fn main() {
    let receipt_dir = std::env::var("T4_RECEIPT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(r"C:\Users\mark_\Code\testing\mere\cambium_t4_receipt_20260916")
        });
    std::fs::create_dir_all(&receipt_dir).expect("create receipt dir");
    std::fs::create_dir_all(receipt_dir.join("pixels")).expect("create pixels dir");

    let steps = Rc::new(build_steps());
    let results: Rc<RefCell<Results>> = Rc::new(RefCell::new(Results::default()));

    let mut hooks: HostHooks<St, Logic, Child> = inert_hooks();
    type PixelJob = (
        String,
        Rc<std::cell::Cell<Option<u64>>>,
        cambium_rootstock::CaptureFrame,
    );
    let pending: Rc<RefCell<Option<PixelJob>>> = Rc::new(RefCell::new(None));

    {
        let results = results.clone();
        let pending = pending.clone();
        hooks.frame = Box::new(move |ctx: &mut AppCtx<St, Logic, Child>| {
            if pending.borrow().is_some() {
                return true;
            }
            let i = ctx.runner.state().idx;
            let steps = ctx.runner.state().steps.clone();
            let step = steps[i].clone();

            match &step.kind {
                StepKind::Pair { key, .. } => {
                    if !ctx.leaves.contains(key) {
                        ctx.leaves.insert(
                            *key,
                            Box::new(SolidLeaf {
                                color: colorf(LEAF_RGB.0, LEAF_RGB.1, LEAF_RGB.2),
                                size: Size {
                                    width: LEAF_W as f32,
                                    height: LEAF_H as f32,
                                },
                                painted: false,
                            }),
                        );
                    }
                },
                StepKind::Control => {
                    if !ctx.leaves.contains(&900) {
                        ctx.leaves
                            .insert(900, Box::new(SceneLeaf { started: false }));
                    }
                },
                StepKind::TimingClip => {
                    if !ctx.leaves.contains(&951) {
                        ctx.leaves.insert(
                            951,
                            Box::new(SolidLeaf {
                                color: colorf(LEAF_RGB.0, LEAF_RGB.1, LEAF_RGB.2),
                                size: Size {
                                    width: LEAF_W as f32,
                                    height: LEAF_H as f32,
                                },
                                painted: false,
                            }),
                        );
                    }
                },
                StepKind::TimingBare => {
                    if !ctx.leaves.contains(&952) {
                        ctx.leaves.insert(
                            952,
                            Box::new(SolidLeaf {
                                color: colorf(LEAF_RGB.0, LEAF_RGB.1, LEAF_RGB.2),
                                size: Size {
                                    width: LEAF_W as f32,
                                    height: LEAF_H as f32,
                                },
                                painted: false,
                            }),
                        );
                    }
                },
            }

            let want_pixels = matches!(step.kind, StepKind::Pair { .. });
            let want_control_lower = matches!(step.kind, StepKind::Control);
            let label = step.label.clone();
            let lower = Rc::new(std::cell::Cell::new(None));
            let observed = lower.clone();
            let results_for_observer = results.clone();
            *ctx.presentation_observer = Some(Box::new(move |ctx, _stamp| {
                let count = ctx
                    .render_core
                    .and_then(|core| core.renderer().fragment_lower_count());
                observed.set(count);
                if want_control_lower {
                    results_for_observer.borrow_mut().control_lower_after = count;
                }
            }));
            if want_pixels {
                let sink = pending.clone();
                *ctx.capture = Some(Box::new(move |capture| {
                    *sink.borrow_mut() = Some((
                        label,
                        lower,
                        capture.expect("retained fragment receipt capture refused"),
                    ));
                }));
            }
            true
        });
    }

    {
        let results = results.clone();
        let pending = pending.clone();
        let receipt_dir = receipt_dir.clone();
        hooks.after_frame = Box::new(move |ctx: &mut AppCtx<St, Logic, Child>| {
            let completed = pending
                .borrow_mut()
                .as_mut()
                .and_then(|(_, _, job)| job.poll());
            if let Some(result) = completed {
                let (label, lower, _) = pending.borrow_mut().take().unwrap();
                let (_presentation, frame) = result.expect("readback");
                let col_w = frame.width / 3;
                let leaf_bytes = crop(&frame, 0, col_w);
                let reference_bytes = crop(&frame, col_w, 2 * col_w);
                let bare_bytes = crop(&frame, 2 * col_w, 3 * col_w);
                if label.ends_with("frame0") || label.ends_with("frame3") {
                    write_png(
                        &receipt_dir.join("pixels").join(format!("{label}.png")),
                        frame.width,
                        frame.height,
                        &frame.rgba,
                    )
                    .expect("receipt PNG");
                }
                results.borrow_mut().pairs.push(PairResult {
                    label,
                    matched: leaf_bytes == reference_bytes,
                    non_vacuous: reference_bytes != bare_bytes,
                    lower_count: lower.get(),
                });
            }
            if pending.borrow().is_some() || ctx.presentation.is_none() {
                return;
            }
            let i = ctx.runner.state().idx;
            let steps = ctx.runner.state().steps.clone();
            let step = &steps[i];
            if let Some(profile) = ctx.frame_profile {
                match step.kind {
                    StepKind::TimingClip => {
                        results
                            .borrow_mut()
                            .timing_clip
                            .push((step.label.clone(), profile));
                    },
                    StepKind::TimingBare => {
                        results
                            .borrow_mut()
                            .timing_bare
                            .push((step.label.clone(), profile));
                    },
                    _ => {},
                }
            }
            // The control step's "before" reading is the previous (opacity)
            // fixture's last recorded lower_count.
            if matches!(step.kind, StepKind::Control) {
                let mut r = results.borrow_mut();
                if r.control_lower_before.is_none() {
                    r.control_lower_before = r.pairs.last().and_then(|p| p.lower_count);
                }
            }
            if i + 1 < steps.len() {
                ctx.runner.update(|s| s.idx = i + 1);
            } else {
                *ctx.close = true;
            }
        });
    }

    let options = HostOptions {
        title: "cambium t4 receipt".into(),
        initial_logical_size: (960.0, 340.0),
        ..Default::default()
    };

    run(
        options,
        move |_window, _commands, _wake| Init {
            state: St { steps, idx: 0 },
            logic: view as Logic,
            sheet: String::new(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    )
    .expect("event loop");

    // ── write results.md ────────────────────────────────────────────────
    let r = results.borrow();
    let mut out = String::new();
    out.push_str("# Cambium T4 receipt — raw results\n\n");
    out.push_str("## Correctness pairs (leaf == reference, reference != bare)\n\n");
    for p in &r.pairs {
        out.push_str(&format!(
            "- {}: matched={} non_vacuous={} fragment_lower_count={:?}\n",
            p.label, p.matched, p.non_vacuous, p.lower_count
        ));
    }
    out.push_str("\n## Control (Path-B leaf must not move fragment_lower_count)\n\n");
    out.push_str(&format!(
        "before={:?} after={:?} unchanged={}\n",
        r.control_lower_before,
        r.control_lower_after,
        r.control_lower_before == r.control_lower_after
    ));
    out.push_str("\n## Timing: clip fixture, ten frames\n\n");
    for (label, profile) in &r.timing_clip {
        out.push_str(&format!("- {label}: {}\n", profile.summary()));
    }
    out.push_str("\n## Timing: no-layer fixture, ten frames\n\n");
    for (label, profile) in &r.timing_bare {
        out.push_str(&format!("- {label}: {}\n", profile.summary()));
    }
    std::fs::write(receipt_dir.join("results_raw.md"), out).expect("write results");
    println!("t4_receipt: wrote receipt to {}", receipt_dir.display());
}
