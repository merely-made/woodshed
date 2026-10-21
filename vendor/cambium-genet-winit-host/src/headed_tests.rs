// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Headed receipts for the host's one render core (stack seams P2). They open
//! a real window on a real device, so they are ignored by default; winit allows
//! one event loop per process, so run them one at a time:
//!
//! ```text
//! cargo test -p cambium-genet-winit-host --lib headed_tests::one_core -- --ignored --exact
//! cargo test -p cambium-genet-winit-host --lib headed_tests::control -- --ignored --exact
//! ```
//!
//! A desktop event loop never suspends, so the driver calls the host's own
//! `suspended` and `resumed` handlers, which is the code a mobile platform
//! would reach.

use std::cell::RefCell;
use std::rc::Rc;

use cambium::{AnyView, GenetCtx, GenetElement, el, text};

use super::*;

type Child = Box<dyn AnyView<(), (), GenetCtx, GenetElement>>;
type Logic = fn(&()) -> Child;

fn root(_: &()) -> Child {
    Box::new(el("div", text("one render core")))
}

/// What the driver read around one forced suspend and resume.
#[derive(Debug, Default)]
struct Readings {
    boots_before: u32,
    boots_after: u32,
    presents_before: u64,
    presents_after: u64,
    surface_dropped: bool,
    core_kept: bool,
    tenant_device_is_surface_device: bool,
}

struct Driver {
    host: WinitHost<(), Logic, Child>,
    /// The core as a tenant saw it through `AppCtx`, before the suspend.
    tenant: Rc<RefCell<Option<Arc<RenderCore>>>>,
    /// The control: forget the core between suspend and resume, as the host
    /// did before S2, so the instrument has to read a second boot.
    drop_core: bool,
    readings: Rc<RefCell<Option<Readings>>>,
}

impl ApplicationHandler<HostEvent> for Driver {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.host.resumed(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        self.host.user_event(event_loop, event);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.host.window_event(event_loop, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.host.about_to_wait(event_loop);
        if self.readings.borrow().is_some() {
            return;
        }
        // The first resume drew the first frame synchronously, so by the first
        // idle turn the core, a surface and the tenant's handle all exist.
        let before = self
            .host
            .s
            .shared
            .render_core
            .clone()
            .expect("a core was booted");
        let mut readings = Readings {
            boots_before: self.host.core_boots,
            presents_before: self.host.s.presentation_sequence,
            ..Readings::default()
        };
        self.host.suspended(event_loop);
        readings.surface_dropped = self.host.s.surface.is_none();
        if self.drop_core {
            self.host.s.shared.render_core = None;
        }
        self.host.resumed(event_loop);
        let after = self
            .host
            .s
            .shared
            .render_core
            .clone()
            .expect("a core after resume");
        readings.boots_after = self.host.core_boots;
        readings.presents_after = self.host.s.presentation_sequence;
        readings.core_kept = Arc::ptr_eq(&before, &after);
        // Identity, not `==`: wgpu compares devices by a per-instance id, and
        // every core boot makes its own instance, so two devices from two
        // boots can compare equal. The control caught exactly that.
        readings.tenant_device_is_surface_device = self
            .tenant
            .borrow()
            .as_ref()
            .zip(self.host.s.surface.as_ref())
            .is_some_and(|(tenant, surface)| std::ptr::eq(tenant.device(), surface.device()));
        *self.readings.borrow_mut() = Some(readings);
        event_loop.exit();
    }
}

fn drive(drop_core: bool) -> Readings {
    let mut builder = EventLoop::<HostEvent>::with_user_event();
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_any_thread(true);
    }
    let event_loop = builder.build().expect("an event loop");
    let s = HostState::new();
    let proxy = event_loop.create_proxy();
    let wake = HostWake::new(
        s.wake_pending.clone(),
        Arc::new(move || {
            let _ = proxy.send_event(HostEvent::Wake);
        }),
    );
    let tenant = Rc::new(RefCell::new(None));
    let mut hooks: HostHooks<(), Logic, Child> = inert_hooks();
    hooks.frame = Box::new({
        let tenant = tenant.clone();
        move |ctx| {
            if tenant.borrow().is_none() {
                *tenant.borrow_mut() = ctx.render_core.cloned();
            }
            false
        }
    });
    let options = HostOptions {
        title: "cambium one render core".into(),
        ..Default::default()
    };
    let init = |_: &dyn HostWindow, _: &WindowCommands, _: &HostWake| Init {
        state: (),
        logic: root as Logic,
        sheet: String::new(),
        fonts: Vec::new(),
        images: Vec::new(),
    };
    let readings = Rc::new(RefCell::new(None));
    let mut driver = Driver {
        host: WinitHost::new(Host::new(options, Some(Box::new(init)), hooks, s, wake)),
        tenant,
        drop_core,
        readings: readings.clone(),
    };
    event_loop.run_app(&mut driver).expect("the event loop ran");
    let readings = readings
        .borrow_mut()
        .take()
        .expect("the driver read the host");
    eprintln!("[headed] drop_core={drop_core} {readings:?}");
    readings
}

/// One device for the host's life: a suspend drops the surface, the resume
/// makes a new one from the same core and presents through it, and a tenant
/// handed the core before the suspend shares the new surface's device.
#[test]
#[ignore = "headed: opens a window on a GPU"]
#[cfg(not(target_os = "macos"))]
fn one_core() {
    let r = drive(false);
    assert!(r.surface_dropped, "the suspend dropped the surface");
    assert_eq!(r.boots_before, 1, "one core booted for the first frame");
    assert_eq!(r.boots_after, 1, "the resume booted no core");
    assert!(r.core_kept, "the same core before and after");
    assert!(
        r.presents_after > r.presents_before,
        "the resumed surface presented"
    );
    assert!(
        r.tenant_device_is_surface_device,
        "the tenant shares the device"
    );
}

/// The control: a host that forgets its core across the suspend must read a
/// second boot and a tenant left on the old device, or the receipt above
/// proves nothing.
#[test]
#[ignore = "headed: opens a window on a GPU"]
#[cfg(not(target_os = "macos"))]
fn control() {
    let r = drive(true);
    assert_eq!(r.boots_after, 2, "forgetting the core boots another");
    assert!(!r.core_kept);
    assert!(
        !r.tenant_device_is_surface_device,
        "the tenant is on the old device"
    );
    assert!(r.presents_after > r.presents_before);
}

// ------------------------------------------------- stage 4: many windows
//
// cargo test -p cambium-genet-winit-host --lib headed_tests::windows -- --ignored --exact
// cargo test -p cambium-genet-winit-host --lib headed_tests::windows_control -- --ignored --exact

use std::cell::Cell;

use cambium::{ProjectionId, clickable};
use cambium_rootstock::WindowDom;
use genet_scripted_dom::NodeId;
use layout_dom_api::LayoutDom as _;

use crate::windows::{WindowHooks, WindowsInit, WinitWindows};

#[derive(Default)]
struct Shared {
    clicks: u32,
}

type WChild = Box<dyn AnyView<Shared, (), GenetCtx, GenetElement>>;
type WLogic = Box<dyn FnMut(&Shared) -> WChild>;

const WSHEET: &str = "div { display: block; height: 20px; } \
                      button { display: block; width: 120px; height: 30px; }";

fn window_lens(label: &'static str) -> WLogic {
    Box::new(move |shared: &Shared| {
        Box::new(el(
            "div",
            (
                el("div", text(format!("window {label}"))),
                clickable(
                    el("button", text(format!("count {label}"))),
                    |s: &mut Shared, _| s.clicks += 1,
                ),
                el("div", text(format!("clicks:{}", shared.clicks))),
            ),
        )) as WChild
    })
}

fn find(dom: &ScriptedDom, root: NodeId, needle: &str) -> Option<NodeId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if dom.dom_children(node).any(|c| dom.text(c) == Some(needle)) {
            return Some(node);
        }
        stack.extend(dom.dom_children(node));
    }
    None
}

fn text_under(dom: &ScriptedDom, root: NodeId) -> String {
    let mut out = String::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(t) = dom.text(node) {
            out.push_str(t);
            out.push('|');
        }
        stack.extend(dom.dom_children(node));
    }
    out
}

/// What the driver read across the run.
#[derive(Debug, Default)]
struct WindowsReadings {
    windows: usize,
    boots: u32,
    one_device: bool,
    sizes: Vec<(f32, f32)>,
    other_saw_click: bool,
    /// Frames A presented for the click, once both windows settled.
    click_frames: u64,
    boots_after_resume: u32,
    animated_frames: u64,
    idle_frames: u64,
    idle_presents_held: bool,
    open_after_close: usize,
    other_still_shown: bool,
    third_boots: u32,
    third_on_its_own_device: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum AfterSettle {
    /// The windows have shown themselves (the platform's first paints are
    /// in): click in A.
    ClickNow,
    /// Count A's frames for the click, then suspend and resume.
    Click { a_before: u64 },
    /// Start A animating.
    Animate,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Launched,
    /// Waiting for both windows to present what they owe and then hold still.
    Settling {
        stable: u32,
        last: (u64, u64),
        next: AfterSettle,
    },
    Animating {
        a0: u64,
        b0: u64,
    },
    Done,
}

struct WindowsDriver {
    windows: WinitWindows<Shared, WLogic, WChild>,
    animate_a: Rc<Cell<bool>>,
    tenant: Rc<RefCell<Option<Arc<RenderCore>>>>,
    /// The control: a third window opened after the core was forgotten must
    /// read a second boot on a device of its own.
    control: bool,
    step: Step,
    turns: u32,
    readings: WindowsReadings,
    /// RedrawRequested events seen per native window, for a stalled run.
    redraws: std::collections::HashMap<WindowId, u32>,
}

const A: ProjectionId = ProjectionId(0);
const B: ProjectionId = ProjectionId(1);

impl WindowsDriver {
    fn boots(&self) -> u32 {
        let multi = self.windows.multi.as_ref().unwrap();
        multi
            .windows()
            .filter_map(|id| multi.slot(id).map(|w| w.core_boots))
            .sum()
    }

    fn presents(&self, id: ProjectionId) -> u64 {
        let multi = self.windows.multi.as_ref().unwrap();
        multi.slot(id).map_or(0, |w| w.s.presentation_sequence)
    }

    fn device_of(&self, id: ProjectionId) -> Option<*const wgpu::Device> {
        let multi = self.windows.multi.as_ref().unwrap();
        multi
            .slot(id)
            .and_then(|w| w.s.surface.as_ref())
            .map(|s| s.device() as *const wgpu::Device)
    }

    fn launched(&mut self) {
        let multi = self.windows.multi.as_ref().unwrap();
        self.readings.windows = multi.windows().count();
        self.readings.sizes = multi
            .windows()
            .filter_map(|id| multi.slot(id).map(|w| w.s.layout_size))
            .collect();
        self.readings.boots = self.boots();
        let devices = [self.device_of(A), self.device_of(B)];
        let tenant = self
            .tenant
            .borrow()
            .as_ref()
            .map(|core| core.device() as *const wgpu::Device);
        self.readings.one_device =
            devices[0].is_some() && devices[0] == devices[1] && tenant == devices[0];

        self.step = Step::Settling {
            stable: 0,
            last: (self.presents(A), self.presents(B)),
            next: AfterSettle::ClickNow,
        };
    }

    fn click(&mut self, event_loop: &ActiveEventLoop) {
        // A click in A, through A's host.
        let a_before = self.presents(A);
        let multi = self.windows.multi.as_mut().unwrap();
        multi.with_window(A, |w| {
            let tree = w.s.runner.as_ref().unwrap();
            let (dom, root) = (HostTree::dom(tree), HostTree::mount(tree));
            let (x, y, bw, bh) = {
                let d = dom.borrow();
                let button = find(&d, root, "count A").expect("A's button");
                w.s.layout
                    .as_ref()
                    .unwrap()
                    .painted_rect(&WindowDom::new(&d, root), button)
                    .expect("A paints its button")
            };
            w.pointer_moved(x + bw / 2.0, y + bh / 2.0);
            w.press_left();
            w.release();
        });
        self.windows.after_turn(event_loop, &[A]);
        let multi = self.windows.multi.as_ref().unwrap();
        let dom = multi.dom();
        self.readings.other_saw_click =
            text_under(&dom.borrow(), multi.window_root(B).unwrap()).contains("clicks:1");

        self.step = Step::Settling {
            stable: 0,
            last: (self.presents(A), self.presents(B)),
            next: AfterSettle::Click { a_before },
        };
    }

    fn settling(
        &mut self,
        event_loop: &ActiveEventLoop,
        stable: u32,
        last: (u64, u64),
        next: AfterSettle,
    ) {
        // Keep the loop turning while nothing animates, so the counts can be
        // seen to hold.
        event_loop.set_control_flow(ControlFlow::wait_duration(
            std::time::Duration::from_millis(16),
        ));
        let now = (self.presents(A), self.presents(B));
        if now != last || stable < 20 {
            let stable = if now == last { stable + 1 } else { 0 };
            self.step = Step::Settling {
                stable,
                last: now,
                next,
            };
            return;
        }
        if next == AfterSettle::ClickNow {
            self.click(event_loop);
            return;
        }
        if let AfterSettle::Click { a_before } = next {
            self.readings.click_frames = now.0 - a_before;
            // Suspend and resume every window.
            self.windows.suspended(event_loop);
            self.windows.resumed(event_loop);
            self.readings.boots_after_resume = self.boots();
            self.step = Step::Settling {
                stable: 0,
                last: (self.presents(A), self.presents(B)),
                next: AfterSettle::Animate,
            };
            return;
        }
        let b = now.1;
        // B has held still for twenty turns: A animates, B is left alone.
        self.animate_a.set(true);
        let multi = self.windows.multi.as_ref().unwrap();
        let kicked: &[ProjectionId] = if self.control { &[A, B] } else { &[A] };
        for &id in kicked {
            if let Some(native) = multi.slot(id).and_then(|w| w.native_window.as_ref()) {
                native.request_redraw();
            }
        }
        self.step = Step::Animating {
            a0: self.presents(A),
            b0: b,
        };
    }

    fn animating(&mut self, event_loop: &ActiveEventLoop, a0: u64, b0: u64) {
        let frames = self.presents(A).saturating_sub(a0);
        if frames < 8 {
            return;
        }
        self.animate_a.set(false);
        self.readings.animated_frames = frames;
        self.readings.idle_frames = self.presents(B).saturating_sub(b0);
        self.readings.idle_presents_held = self.presents(B) == b0;
        if self.control {
            let multi = self.windows.multi.as_mut().unwrap();
            let third = multi
                .with_window(B, |w| {
                    w.s.shared.render_core = None;
                    w.s.runner.as_mut().unwrap().open(
                        window_lens("C"),
                        HostOptions {
                            title: "cambium window C".into(),
                            initial_logical_size: (360.0, 240.0),
                            ..Default::default()
                        },
                    )
                })
                .unwrap();
            self.windows.after_turn(event_loop, &[B]);
            self.readings.third_boots = self.boots();
            self.readings.third_on_its_own_device =
                self.device_of(third).is_some() && self.device_of(third) != self.device_of(B);
        }
        // Close A as the platform would.
        let multi = self.windows.multi.as_mut().unwrap();
        multi.with_window(A, |w| w.request_close(crate::CloseRequest::Native));
        self.windows.after_turn(event_loop, &[A]);
        let multi = self.windows.multi.as_ref().unwrap();
        self.readings.open_after_close = multi.windows().count();
        self.readings.other_still_shown = multi.slot(B).is_some_and(|w| w.native_window.is_some());
        self.step = Step::Done;
        event_loop.exit();
    }
}

impl ApplicationHandler<HostEvent> for WindowsDriver {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.windows.resumed(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        self.windows.user_event(event_loop, event);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            *self.redraws.entry(id).or_default() += 1;
        }
        self.windows.window_event(event_loop, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.windows.about_to_wait(event_loop);
        if self.step == Step::Done {
            return;
        }
        self.turns += 1;
        if self.turns > 5_000 {
            let multi = self.windows.multi.as_ref().unwrap();
            for id in multi.windows() {
                let w = multi.slot(id).unwrap();
                let native = w.native_window.as_ref().map(|n| n.id());
                eprintln!(
                    "[headed] window {:?}: presents {} redraw events {:?} last frame {:?}",
                    id,
                    w.s.presentation_sequence,
                    native.and_then(|n| self.redraws.get(&n)),
                    w.s.last_frame_profile.map(|p| (p.acquire_us, p.total_us)),
                );
            }
            eprintln!("[headed] gave up at {:?}", self.step);
            event_loop.exit();
            return;
        }
        match self.step {
            Step::Launched => self.launched(),
            Step::Settling { stable, last, next } => self.settling(event_loop, stable, last, next),
            Step::Animating { a0, b0 } => self.animating(event_loop, a0, b0),
            Step::Done => {},
        }
    }
}

fn drive_windows(control: bool) -> WindowsReadings {
    let mut builder = EventLoop::<HostEvent>::with_user_event();
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_any_thread(true);
    }
    let event_loop = builder.build().expect("an event loop");
    let animate_a = Rc::new(Cell::new(false));
    let tenant = Rc::new(RefCell::new(None));
    let mut hooks: WindowHooks<Shared, WLogic, WChild> = HostHooks::inert();
    hooks.frame = Box::new({
        let (animate_a, tenant) = (animate_a.clone(), tenant.clone());
        move |ctx| {
            if tenant.borrow().is_none() {
                *tenant.borrow_mut() = ctx.render_core.cloned();
            }
            // The control animates B too, so the idle count has frames to see.
            animate_a.get() && (control || ctx.runner.window() == A)
        }
    });
    let window = |label: &'static str, size: (f64, f64)| {
        (
            window_lens(label),
            HostOptions {
                title: format!("cambium window {label}"),
                initial_logical_size: size,
                ..Default::default()
            },
        )
    };
    let windows = vec![window("A", (480.0, 360.0)), window("B", (720.0, 480.0))];
    let init = move |_: &HostWake| WindowsInit {
        state: Shared::default(),
        sheet: WSHEET.into(),
        fonts: Vec::new(),
        images: Vec::new(),
        windows,
    };
    let mut driver = WindowsDriver {
        windows: WinitWindows::new(&event_loop, Box::new(init), hooks),
        animate_a,
        tenant,
        control,
        step: Step::Launched,
        turns: 0,
        readings: WindowsReadings::default(),
        redraws: Default::default(),
    };
    event_loop.run_app(&mut driver).expect("the event loop ran");
    eprintln!("[headed] windows control={control} {:?}", driver.readings);
    assert_eq!(driver.step, Step::Done, "the driver finished");
    driver.readings
}

/// Two windows over one state: one device for both, a click in one changes
/// the other, a suspend and resume boots nothing, an idle window presents
/// nothing while the other animates, and closing one leaves the other.
#[test]
#[ignore = "headed: opens two windows on a GPU"]
#[cfg(not(target_os = "macos"))]
fn windows() {
    let r = drive_windows(false);
    assert_eq!(r.windows, 2);
    assert_eq!(r.boots, 1, "one core for both windows");
    assert!(
        r.one_device,
        "both surfaces and the tenant share one device"
    );
    assert_ne!(
        r.sizes[0], r.sizes[1],
        "the windows lay out at their own sizes"
    );
    assert!(r.other_saw_click, "B shows the click A took");
    assert_eq!(r.click_frames, 1, "A presented one frame for the click");
    assert_eq!(r.boots_after_resume, 1, "the resume booted nothing");
    assert!(r.animated_frames >= 8);
    assert!(r.idle_presents_held, "B presented nothing while A animated");
    assert_eq!(r.open_after_close, 1, "closing A left one window");
    assert!(r.other_still_shown, "B is still up");
}

/// The control: a window opened after the core was forgotten boots a second
/// core and sits on its own device, and a B that animates too presents frames,
/// so the receipt above can see both.
#[test]
#[ignore = "headed: opens three windows on a GPU"]
#[cfg(not(target_os = "macos"))]
fn windows_control() {
    let r = drive_windows(true);
    assert_eq!(r.third_boots, 2, "forgetting the core boots another");
    assert!(r.third_on_its_own_device);
    assert!(r.idle_frames > 0, "an animating B presents");
    assert_eq!(r.open_after_close, 2);
}
