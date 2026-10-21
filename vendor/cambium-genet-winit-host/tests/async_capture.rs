// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The shared lane must wait for asynchronous pixels and propagate map failures.
use cambium::{AnyView, GenetCtx, GenetElement, el};
use cambium_genet_winit_host::{Harness, HostHooks, Init, inert_hooks};
use cambium_rootstock::{CaptureFn, Frame};
use mesquite::{CaptureBackend, Ctx, Lane, Product, Readback};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type View = Box<dyn AnyView<(), (), GenetCtx, GenetElement>>;
type Logic = fn(&()) -> View;
fn view(_: &()) -> View {
    Box::new(el("main", "capture"))
}
#[derive(Default)]
struct Observed {
    inspections: usize,
    outcomes: Vec<bool>,
}
struct TestProduct(Rc<RefCell<Observed>>);
impl Product for TestProduct {
    type State = ();
    type Logic = Logic;
    type View = View;
    const KIND: &'static str = "async-capture";
    const SURFACE: &'static str = "test";
    const LOG_PREFIX: &'static str = "test";
    fn sheet(&self) -> &str {
        ""
    }
    fn snapshot(&self, _: &Ctx<'_, Self>, _: usize, _: f32) -> taproot::ProbeSnapshot {
        taproot::ProbeSnapshot::default()
    }
    fn inspect(&mut self, _: &str, _: &Frame) {
        self.0.borrow_mut().inspections += 1;
    }
    fn complete(
        &mut self,
        _: &mut Ctx<'_, Self>,
        outcome: &taproot::Outcome,
    ) -> Result<(), String> {
        self.0.borrow_mut().outcomes.push(outcome.ok);
        Ok(())
    }
}
struct Delayed(bool);
impl CaptureBackend for Delayed {
    fn arm(&mut self, _: &mut Option<CaptureFn>) -> Readback {
        let fail = self.0;
        let mut polls = 0;
        Box::new(move || {
            polls += 1;
            if polls < 12 {
                return None;
            }
            Some(if fail {
                Err("injected map failure".into())
            } else {
                Ok(Frame {
                    width: 2,
                    height: 1,
                    rgba: vec![20, 30, 40, 255, 90, 80, 70, 255],
                })
            })
        })
    }
    fn writes_files(&self) -> bool {
        false
    }
    fn patience(&self) -> u64 {
        20
    }
}
#[test]
fn final_async_capture_is_collected_before_completion_and_failure_cannot_pass() {
    for fail in [false, true] {
        let observed = Rc::new(RefCell::new(Observed::default()));
        let exit = Rc::new(Cell::new(0));
        let mut lane = Lane::new(
            TestProduct(observed.clone()),
            Some(taproot::Scenario::parse("capture last\n").unwrap()),
            None,
            None,
            exit.clone(),
        )
        .with_capture_backend(Delayed(fail));
        let mut host = Harness::with_hooks(
            Init {
                state: (),
                logic: view as Logic,
                sheet: String::new(),
                fonts: vec![],
                images: vec![],
            },
            HostHooks {
                after_frame: Box::new(move |ctx| lane.after_frame(ctx)),
                ..inert_hooks()
            },
        );
        for _ in 0..10 {
            host.layout_at(100.0, 100.0);
            host.after_frame();
        }
        assert!(
            observed.borrow().outcomes.is_empty(),
            "completion raced readback"
        );
        assert!(!host.close_requested());
        for _ in 0..20 {
            host.layout_at(100.0, 100.0);
            host.after_frame();
        }
        assert_eq!(observed.borrow().outcomes, [!fail]);
        assert_eq!(observed.borrow().inspections, usize::from(!fail));
        assert_eq!(exit.get(), i32::from(fail));
        assert!(host.close_requested());
    }
}
