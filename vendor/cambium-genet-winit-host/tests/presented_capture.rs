// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Synthetic delayed pixels qualify pairing mechanics, not GPU/display success.
use cambium::{AnyView, GenetCtx, GenetElement, button, el};
use cambium_genet_winit_host::{Harness, HostHooks, Init, inert_hooks};
use cambium_rootstock::{CaptureFn, Frame, HostPointer, PresentedFrame, StampedCaptureFn};
use mesquite::{
    CaptureBackend, CaptureObserver, CaptureProjection, CaptureProjectionLimits, Ctx, Lane,
    Product, Readback, StampedFrame, StampedReadback,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

type View = Box<dyn AnyView<u32, (), GenetCtx, GenetElement>>;
type Logic = fn(&u32) -> View;
fn view(count: &u32) -> View {
    Box::new(el(
        "main",
        button(format!("Count {count}"), |state: &mut u32, _| *state += 1),
    ))
}
fn stamp(sequence: u64) -> PresentedFrame {
    PresentedFrame {
        context_generation: cambium_rootstock::ContextGeneration(1),
        host: 1,
        sequence,
        width: 2,
        height: 1,
        layout_scale: 1.0,
    }
}
struct TestProduct {
    seen: Rc<RefCell<Vec<(u32, usize)>>>,
    fail_observer: bool,
}
impl Product for TestProduct {
    type State = u32;
    type Logic = Logic;
    type View = View;
    const KIND: &'static str = "paired-test";
    const SURFACE: &'static str = "test";
    const LOG_PREFIX: &'static str = "test";
    fn sheet(&self) -> &str {
        "button { display: block; width: 64px; height: 32px; }"
    }
    fn snapshot(&self, ctx: &Ctx<'_, Self>, _: usize, _: f32) -> taproot::ProbeSnapshot {
        taproot::ProbeSnapshot::default().with_field("count", ctx.runner.state().to_string())
    }
    fn capture_observer(&self) -> Option<CaptureObserver<Self>> {
        let seen = self.seen.clone();
        let fail = self.fail_observer;
        Some(CaptureObserver {
            run: "fixture-fresh-run".into(),
            observe: Box::new(move |ctx, _| {
                if fail {
                    return Err("injected observer failure".into());
                }
                let count = *ctx.runner.state();
                seen.borrow_mut().push((count, ctx.pointer.len()));
                Ok(CaptureProjection {
                    fields: BTreeMap::from([("count".into(), count.to_string())]),
                    viewport: None,
                    product: serde_json::json!({"counter": count}),
                })
            }),
        })
    }
}
struct Delayed {
    fail: bool,
    foreign: bool,
    never: bool,
    wrong_request: bool,
}
impl CaptureBackend for Delayed {
    fn arm(&mut self, _: &mut Option<CaptureFn>) -> Readback {
        panic!("paired capture used legacy backend")
    }
    fn arm_stamped(
        &mut self,
        _: &mut Option<StampedCaptureFn>,
        mut identity: mesquite::CaptureRequest,
    ) -> Result<StampedReadback, String> {
        let fail = self.fail;
        let foreign = self.foreign;
        let never = self.never;
        if self.wrong_request {
            identity.request += 1;
        }
        let mut polls = 0;
        Ok(Box::new(move || {
            polls += 1;
            if polls < 5 || never {
                return None;
            }
            Some(if fail {
                Err("injected mapping failure".into())
            } else {
                Ok(StampedFrame {
                    identity: identity.clone(),
                    presentation: stamp(if foreign { 2 } else { 1 }),
                    pixels: Frame {
                        width: 2,
                        height: 1,
                        rgba: vec![10, 20, 30, 255, 40, 50, 60, 255],
                    },
                })
            })
        }))
    }
    fn writes_files(&self) -> bool {
        false
    }
    fn patience(&self) -> u64 {
        8
    }
}
#[derive(Default)]
struct Case {
    missing: bool,
    foreign: bool,
    map_fail: bool,
    observer_fail: bool,
    never: bool,
    limits: Option<CaptureProjectionLimits>,
    second: bool,
    wrong_request: bool,
}

type ReceiptReadings = Vec<(String, serde_json::Value)>;
type RunResult = (i32, Vec<(u32, usize)>, ReceiptReadings);
fn run(case: Case) -> RunResult {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let exit = Rc::new(Cell::new(0));
    let scenario = if case.second {
        "capture first\ncapture second\n"
    } else {
        "capture first\n"
    };
    let mut lane = Lane::new(
        TestProduct {
            seen: seen.clone(),
            fail_observer: case.observer_fail,
        },
        Some(taproot::Scenario::parse(scenario).unwrap()),
        None,
        None,
        exit.clone(),
    )
    .with_capture_backend(Delayed {
        fail: case.map_fail,
        foreign: case.foreign,
        never: case.never,
        wrong_request: case.wrong_request,
    });
    if let Some(limits) = case.limits {
        lane = lane.with_capture_projection_limits(limits);
    }
    let lane = Rc::new(RefCell::new(lane));
    let driver = lane.clone();
    let mut host = Harness::with_hooks(
        Init {
            state: 0,
            logic: view as Logic,
            sheet: "button { display: block; width: 64px; height: 32px; }".into(),
            fonts: vec![],
            images: vec![],
        },
        HostHooks {
            frame: Box::new(|ctx| {
                ctx.pointer.extend([
                    HostPointer::Press(16.0, 16.0),
                    HostPointer::Release(16.0, 16.0),
                ]);
                false
            }),
            after_frame: Box::new(move |ctx| driver.borrow_mut().after_frame(ctx)),
            ..inert_hooks()
        },
    );
    for _ in 0..25 {
        host.prepare_frame();
        host.layout_at(100.0, 100.0);
        let before = *host.state();
        if !case.missing {
            host.observe_presentation(stamp(1));
        }
        assert_eq!(*host.state(), before, "sealing dispatched pending input");
        host.drain_pointer();
        host.update(|state| *state += 10); // state advances while pixels are delayed
        host.after_frame();
    }
    assert!(
        lane.borrow().finished(),
        "capture lifecycle did not terminate"
    );
    let receipts = lane
        .borrow()
        .capture_receipts()
        .iter()
        .map(|capture| {
            (
                capture.fields()["count"].clone(),
                serde_json::to_value(capture.pairing().unwrap()).unwrap(),
            )
        })
        .collect();
    let observations = seen.borrow().clone();
    (exit.get(), observations, receipts)
}

#[test]
fn delayed_pixels_keep_the_sealed_state_before_pointer_dispatch() {
    let (exit, seen, receipts) = run(Case::default());
    assert_eq!(exit, 0);
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].1, 2, "observer ran after queued pointer dispatch");
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        receipts[0].0,
        seen[0].0.to_string(),
        "capture fields were sampled at collection"
    );
    assert_eq!(receipts[0].1["product_projection"]["counter"], seen[0].0);
    assert_eq!(receipts[0].1["presentation"]["sequence"], 1);
}
#[test]
fn missing_foreign_stale_mapping_observer_and_deadline_failures_cannot_pass() {
    for case in [
        Case {
            missing: true,
            ..Default::default()
        },
        Case {
            foreign: true,
            ..Default::default()
        },
        Case {
            wrong_request: true,
            ..Default::default()
        },
        Case {
            second: true,
            ..Default::default()
        },
        Case {
            map_fail: true,
            ..Default::default()
        },
        Case {
            observer_fail: true,
            ..Default::default()
        },
        Case {
            never: true,
            ..Default::default()
        },
    ] {
        assert_eq!(run(case).0, 1);
    }
}
#[test]
fn requested_projection_byte_total_and_count_limits_are_enforced() {
    for limits in [
        CaptureProjectionLimits {
            max_projection_bytes: 1,
            ..Default::default()
        },
        CaptureProjectionLimits {
            max_total_bytes: 1,
            ..Default::default()
        },
        CaptureProjectionLimits {
            max_captures: 0,
            ..Default::default()
        },
    ] {
        let (exit, _, receipts) = run(Case {
            limits: Some(limits),
            ..Default::default()
        });
        assert_eq!(exit, 1);
        assert!(receipts.is_empty());
    }
}
