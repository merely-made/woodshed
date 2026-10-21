// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The `wheel` scenario verb, end to end and headless: a scenario line becomes a
//! queued [`HostPointer::Wheel`], the host's own wheel path hit-tests it and
//! dispatches it, and a [`graph_canvas_with_wheel`] consumer receives a genuine
//! [`WheelEvent`] (both axes, cursor-local position, modifiers). Nothing here
//! calls `Host::wheel` or builds a `WheelEvent` by hand.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use cambium::{
    AnyView, GRAPH_CANVAS_SWATCH_CSS, GenetCtx, GenetElement, GraphCanvasEdge, GraphCanvasInput,
    GraphCanvasNode, GraphCanvasSubgraph, GraphCanvasSwatch, el, graph_canvas_with_wheel, text,
};
use cambium_genet_winit_host::{
    AppCtx, HOST_WHEEL_LINE_PX, Harness, HostHooks, HostPointer, Init, Modifiers, WheelUnit,
    inert_hooks,
};
use mesquite::{Lane, Product};
use taproot::{ProbeSnapshot, Scenario};

/// What the consumer was handed for one notch.
#[derive(Clone, Debug, PartialEq)]
struct Notch {
    delta: (f32, f32),
    local: (f32, f32),
    size: (f32, f32),
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
}

#[derive(Default)]
struct App {
    notches: Vec<Notch>,
    other_events: usize,
}

type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
type Logic = fn(&App) -> Child;

fn swatch() -> GraphCanvasSwatch<u8, &'static str> {
    GraphCanvasSwatch::new(
        7,
        GraphCanvasSubgraph {
            nodes: vec![
                GraphCanvasNode {
                    id: 1,
                    kind: "document",
                    position: (0.2, 0.5),
                    label: "First".into(),
                    key: None,
                },
                GraphCanvasNode {
                    id: 2,
                    kind: "person",
                    position: (0.8, 0.5),
                    label: "Second".into(),
                    key: None,
                },
            ],
            edges: vec![GraphCanvasEdge { from: 1, to: 2 }],
        },
    )
    .with_size(240, 120)
}

fn root(_state: &App) -> Child {
    Box::new(el(
        "main",
        (
            el("p", text("A graph canvas that takes the wheel whole")),
            graph_canvas_with_wheel(&swatch(), |state: &mut App, input| match input {
                GraphCanvasInput::Wheel(event) => {
                    state.notches.push(Notch {
                        delta: event.delta,
                        local: event.local,
                        size: event.size,
                        ctrl: event.modifiers.ctrl,
                        shift: event.modifiers.shift,
                        alt: event.modifiers.alt,
                        meta: event.modifiers.meta,
                    });
                    // The consumer takes the notch, so the host's scroll and
                    // Ctrl-zoom defaults stay out of the way.
                    event.prevent_default();
                },
                GraphCanvasInput::Event(_) => state.other_events += 1,
            }),
        ),
    ))
}

struct Lane0;

impl Product for Lane0 {
    type State = App;
    type Logic = Logic;
    type View = Child;
    const KIND: &'static str = "wheel-scenario-test";
    const SURFACE: &'static str = "app";
    const LOG_PREFIX: &'static str = "wheel-scenario-test";

    fn sheet(&self) -> &str {
        GRAPH_CANVAS_SWATCH_CSS
    }

    fn snapshot(&self, ctx: &AppCtx<'_, App, Logic, Child>, _: usize, _: f32) -> ProbeSnapshot {
        let state = ctx.runner.state();
        let last = state.notches.last();
        let at_centre = last.is_some_and(|n| {
            (n.local.0 - n.size.0 * 0.5).abs() < 1.5 && (n.local.1 - n.size.1 * 0.5).abs() < 1.5
        });
        let field = |value: Option<f32>| value.map_or_else(|| "none".into(), |v| format!("{v}"));
        ProbeSnapshot::default()
            .with_field("notches", state.notches.len().to_string())
            .with_field("other_events", state.other_events.to_string())
            .with_field("dx", field(last.map(|n| n.delta.0)))
            .with_field("dy", field(last.map(|n| n.delta.1)))
            .with_field("local_x", field(last.map(|n| n.local.0)))
            .with_field("local_y", field(last.map(|n| n.local.1)))
            .with_field("at_centre", at_centre.to_string())
            .with_field("ctrl", last.is_some_and(|n| n.ctrl).to_string())
            .with_field("shift", last.is_some_and(|n| n.shift).to_string())
            .with_field("alt", last.is_some_and(|n| n.alt).to_string())
            .with_field("meta", last.is_some_and(|n| n.meta).to_string())
    }

    fn drain_events(&mut self, _: &mut AppCtx<'_, App, Logic, Child>) -> Vec<String> {
        Vec::new()
    }

    fn default_capture_path(&self) -> PathBuf {
        PathBuf::from("unused.png")
    }
}

/// Run `scenario` to its receipt headless, as the headed host would: lay out,
/// tick the lane, deliver the pointer it queued, repeat.
fn run(scenario: &str) -> (String, Harness<App, Logic, Child>) {
    let receipt = std::env::temp_dir().join(format!(
        "cambium-wheel-scenario-{}-{:x}.done",
        std::process::id(),
        scenario.len() ^ scenario.bytes().map(usize::from).sum::<usize>()
    ));
    let _ = std::fs::remove_file(&receipt);
    let mut lane = Lane::new(
        Lane0,
        Some(Scenario::parse(scenario).expect("scenario parses")),
        Some(receipt.clone()),
        None,
        Rc::new(Cell::new(0)),
    );
    let hooks: HostHooks<App, Logic, Child> = HostHooks {
        after_frame: Box::new(move |ctx| lane.after_frame(ctx)),
        ..inert_hooks()
    };
    let mut h = Harness::with_hooks(
        Init {
            state: App::default(),
            logic: root as Logic,
            sheet: GRAPH_CANVAS_SWATCH_CSS.into(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    for _ in 0..200 {
        h.layout_at(400.0, 300.0);
        h.after_frame();
        h.drain_pointer();
        h.after_dispatch();
        if h.close_requested() {
            break;
        }
    }
    assert!(h.close_requested(), "the lane never wrote its receipt");
    let text = std::fs::read_to_string(&receipt).expect("receipt written");
    let _ = std::fs::remove_file(&receipt);
    (text, h)
}

/// The receipt's own verdict: `ok` and no errors.
fn ok(receipt: &str) {
    let json: serde_json::Value = serde_json::from_str(receipt).expect("receipt is JSON");
    assert_eq!(json["ok"], true, "{receipt}");
    assert_eq!(json["errors"], serde_json::json!([]), "{receipt}");
}

/// The receipt failed, and the scenario log says why.
fn failed(receipt: &str, why: &str) {
    let json: serde_json::Value = serde_json::from_str(receipt).expect("receipt is JSON");
    assert_eq!(json["ok"], false, "{receipt}");
    assert!(json["scenario_log"].to_string().contains(why), "{receipt}");
}

/// The host's pixel-and-line conversion and this crate's restatement agree.
#[test]
fn a_wheel_line_is_worth_the_desktop_hosts_line_pixels() {
    assert_eq!(HOST_WHEEL_LINE_PX, genet_winit_host::WHEEL_LINE_PX);
}

#[test]
fn a_wheel_on_a_selector_reaches_a_graph_canvas_consumer_whole() {
    let (receipt, h) = run("settle 2
wheel 0 3 on .graph-canvas-swatch
settle 2
assert snap notches == 1
assert snap other_events == 0
assert snap dx == 0
assert snap dy == 3
assert snap at_centre == true
assert snap ctrl == false
");
    ok(&receipt);
    let notch = &h.state().notches[0];
    assert_eq!(notch.delta, (0.0, 3.0));
    // The swatch is 240 by 120 inside a one-pixel border.
    assert!((notch.size.0 - 242.0).abs() < 0.5 && (notch.size.1 - 122.0).abs() < 0.5);
}

#[test]
fn lines_are_converted_and_modifiers_are_held_for_one_notch() {
    let (receipt, h) = run("settle 2
wheel 0 2 lines alt meta on .graph-canvas-swatch
settle 2
assert snap dy == 96
assert snap alt == true
assert snap meta == true
assert snap ctrl == false
wheel 0 1 on .graph-canvas-swatch
settle 2
assert snap notches == 2
assert snap dy == 1
assert snap alt == false
assert snap meta == false
");
    ok(&receipt);
    assert_eq!(h.state().notches[0].delta, (0.0, 2.0 * HOST_WHEEL_LINE_PX));
}

#[test]
fn shift_turns_a_vertical_notch_sideways_as_a_real_wheel_does() {
    let (receipt, h) = run("settle 2
wheel 0 5 shift on .graph-canvas-swatch
settle 2
assert snap dx == 5
assert snap dy == 0
assert snap shift == true
");
    ok(&receipt);
    assert_eq!(h.state().notches[0].delta, (5.0, 0.0));
}

#[test]
fn a_wheel_at_a_point_lands_where_it_says_and_off_the_canvas_reaches_nobody() {
    let (probe, h) = run("settle 2
wheel 0 1 on .graph-canvas-swatch
settle 2
assert snap notches == 1
wheel 0 1 at 390 290
settle 2
assert snap notches == 1
");
    ok(&probe);
    assert_eq!(
        h.state().notches.len(),
        1,
        "a point off the canvas hit nothing"
    );
}

#[test]
fn a_wheel_with_no_target_or_a_typo_fails_the_scenario() {
    let (receipt, _) = run("settle 2
wheel 0 1 on .nothing-like-it
");
    failed(&receipt, "no current DOM target");
    let (receipt, _) = run("wheel 0 1 sideways at 0 0\n");
    failed(&receipt, "unknown wheel word");
}

/// The queued input is plain data a host or demo can build without a scenario.
#[test]
fn the_wheel_input_is_plain_copyable_data() {
    let notch = HostPointer::Wheel {
        x: 1.0,
        y: 2.0,
        dx: 0.0,
        dy: 1.0,
        unit: WheelUnit::Lines,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(notch, notch);
}
