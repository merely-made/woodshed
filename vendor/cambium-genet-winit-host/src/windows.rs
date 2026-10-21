// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The multi-window desktop entry (stack seams S23 to S26), beside [`run`]:
//! one application state, one forest document, one render core, and a native
//! window per projection, each a lens over the state.
//!
//! Each window runs the single-window host's per-window pipeline in its turn
//! (see [`MultiHost`]): events route to the window they arrived for, a window
//! redraws and presents on its own requests at its own monitor's rate, and an
//! idle window presents nothing. A change one window makes reaches the others
//! in the same pass, and those it reached are asked to redraw.
//!
//! [`run`]: crate::run

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use cambium::ProjectionId;
use cambium_rootstock::meristem_bounds::RootView;
use cambium_rootstock::{
    AppShared, Host, HostFont, HostHooks, HostImage, HostOptions, HostState, HostWake, HostWindow,
    IdlePolicy, MultiHost, TitlebarInsets, WindowSlot, WindowTree,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::{HostEvent, WinitHost, WinitWindow, control_flow, env_ui_zoom};

/// A window's frame as its host sees it under the multi-window entry: a
/// redraw it asks for is noted, and the idle turn serves every noted window.
///
/// On Windows a frame arrives as `WM_PAINT`, and Win32 hands `WM_PAINT` to the
/// first window needing paint, every time: a window that asks for its next
/// frame from each frame, as an animating one does, starves every window
/// behind it (F18). So there the idle turn draws each window that asked, in
/// turn, through the same handler a delivered paint runs; `Fifo` presentation
/// paces each to its monitor. Elsewhere it hands the requests to the platform,
/// whose own frame delivery paces each window.
struct PacedWindow {
    window: WinitWindow,
    wanted: Rc<Cell<bool>>,
}

impl HostWindow for PacedWindow {
    fn request_redraw(&self) {
        self.wanted.set(true);
    }

    fn inner_size(&self) -> (u32, u32) {
        self.window.inner_size()
    }

    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
    }

    fn set_ime_cursor_area(&self, x: f64, y: f64, width: f64, height: f64) {
        self.window.set_ime_cursor_area(x, y, width, height);
    }

    fn titlebar_insets(&self) -> TitlebarInsets {
        self.window.titlebar_insets()
    }
}

/// One window of a multi-window application: the winit host over that
/// window's projection.
pub type WindowHost<State, Logic, V> = WinitHost<State, Logic, V, WindowTree<State, Logic, V>>;

/// A multi-window application's hooks. Each runs in one window's turn, and
/// `ctx.runner` is that window's [`WindowTree`]: which window it is, the
/// shared state, and `open` and `close` for windows.
pub type WindowHooks<State, Logic, V> = HostHooks<State, Logic, V, WindowTree<State, Logic, V>>;

/// What a multi-window application's init hands back.
pub struct WindowsInit<State, Logic> {
    /// The application state every window renders.
    pub state: State,
    /// The stylesheet every window lays out under.
    pub sheet: String,
    /// Host-supplied faces.
    pub fonts: Vec<HostFont>,
    /// Host-supplied images.
    pub images: Vec<HostImage>,
    /// The windows to open at launch, each a lens over the state with its own
    /// options. The application ends when its last window closes.
    pub windows: Vec<(Logic, HostOptions)>,
}

impl<State, Logic, V> WindowSlot<State, Logic, V> for WindowHost<State, Logic, V>
where
    State: 'static,
    Logic: FnMut(&State) -> V,
    V: RootView<State>,
{
    fn host(&self) -> &Host<State, Logic, V, WindowTree<State, Logic, V>> {
        &self.core
    }

    fn host_mut(&mut self) -> &mut Host<State, Logic, V, WindowTree<State, Logic, V>> {
        &mut self.core
    }
}

/// Run a multi-window Cambium application to completion.
///
/// `init` receives the wake handle for application-owned workers. A window's
/// close policy deciding [`Exit`](crate::CloseDisposition::Exit), or a hook
/// setting `ctx.close`, closes that window.
pub fn run_windows<State, Logic, V>(
    init: impl FnOnce(&HostWake) -> WindowsInit<State, Logic> + 'static,
    hooks: WindowHooks<State, Logic, V>,
) -> Result<(), winit::error::EventLoopError>
where
    State: 'static,
    Logic: FnMut(&State) -> V + 'static,
    V: RootView<State>,
{
    let event_loop = EventLoop::<HostEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut windows = WinitWindows::new(&event_loop, Box::new(init), hooks);
    event_loop.run_app(&mut windows)
}

type WindowsInitFn<State, Logic> = Box<dyn FnOnce(&HostWake) -> WindowsInit<State, Logic>>;

/// The multi-window event source.
pub(crate) struct WinitWindows<State: 'static, Logic, V>
where
    Logic: FnMut(&State) -> V,
    V: RootView<State>,
{
    pub(crate) multi: Option<MultiHost<State, Logic, V, WindowHost<State, Logic, V>>>,
    init: Option<WindowsInitFn<State, Logic>>,
    hooks: Option<WindowHooks<State, Logic, V>>,
    wake: HostWake,
    by_native: HashMap<WindowId, ProjectionId>,
}

impl<State, Logic, V> WinitWindows<State, Logic, V>
where
    State: 'static,
    Logic: FnMut(&State) -> V + 'static,
    V: RootView<State>,
{
    pub(crate) fn new(
        event_loop: &EventLoop<HostEvent>,
        init: WindowsInitFn<State, Logic>,
        hooks: WindowHooks<State, Logic, V>,
    ) -> Self {
        let proxy = event_loop.create_proxy();
        let wake = HostWake::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(move || {
                let _ = proxy.send_event(HostEvent::Wake);
            }),
        );
        Self {
            multi: None,
            init: Some(init),
            hooks: Some(hooks),
            wake,
            by_native: HashMap::new(),
        }
    }

    fn multi(&mut self) -> &mut MultiHost<State, Logic, V, WindowHost<State, Logic, V>> {
        self.multi.as_mut().expect("the windows opened")
    }

    /// A host for one window, under the receipt aid `CAMBIUM_UI_ZOOM` as
    /// [`run`](crate::run) applies it.
    fn window_host(&self, mut options: HostOptions) -> WindowHost<State, Logic, V> {
        if let Some(zoom) = env_ui_zoom() {
            options.ui_zoom = zoom;
        }
        WinitHost::new(Host::new(
            options,
            None,
            HostHooks::inert(),
            HostState::new(),
            self.wake.clone(),
        ))
    }

    /// Give window `id` its native window and first frame, in its turn.
    fn show(&mut self, event_loop: &ActiveEventLoop, id: ProjectionId) {
        let native = self.multi().with_window(id, |window| {
            let (native, restored) = window.open_native_window(event_loop);
            let native_id = native.id();
            window
                .install_window(native.clone(), restored)
                .expect("boot genet host");
            let wanted = Rc::new(Cell::new(false));
            window.s.window = Some(Box::new(PacedWindow {
                window: WinitWindow(native),
                wanted: wanted.clone(),
            }));
            window.paced = Some(wanted);
            window.first_frame();
            native_id
        });
        if let Some(native_id) = native {
            self.by_native.insert(native_id, id);
        }
    }

    fn close(&mut self, id: ProjectionId) {
        if let Some(window) = self.multi().close(id)
            && let Some(native) = window.native_window.as_ref()
        {
            self.by_native.remove(&native.id());
        }
    }

    /// Ask window `id` for a frame, through its paced request, so the idle
    /// turn serves it with every other window's.
    fn want_frame(&mut self, id: ProjectionId) {
        if let Some(wanted) = self.multi().slot(id).and_then(|w| w.paced.as_ref()) {
            wanted.set(true);
        }
    }

    /// What turns leave for the event source: windows hooks opened, closed or
    /// asked to redraw, windows whose close policy ended them, and windows a
    /// change reached or a sheet swap left behind, which are asked for a frame.
    /// `acting` are the windows whose turns these were: a change a window made
    /// to itself is its own to show, as it is for a single window.
    pub(crate) fn after_turn(&mut self, event_loop: &ActiveEventLoop, acting: &[ProjectionId]) {
        let mut closing = Vec::new();
        // A window opened from another window's first frame is opened too, to
        // a bound: a frame that opens a window every time would never end.
        for _ in 0..64 {
            let requests = self.multi().take_requests();
            closing.extend(requests.closed);
            for id in requests.redraws {
                self.want_frame(id);
            }
            if requests.opened.is_empty() {
                break;
            }
            for (id, options) in requests.opened {
                let window = self.window_host(options);
                self.multi().attach(id, window);
                self.show(event_loop, id);
            }
        }
        let multi = self.multi();
        closing.extend(multi.windows().filter(|id| {
            multi
                .slot(*id)
                .is_some_and(|window| window.s.close_requested)
        }));
        for id in closing {
            self.close(id);
        }
        let multi = self.multi();
        let mut owed: Vec<ProjectionId> = multi
            .touched_windows()
            .into_iter()
            .filter(|id| !acting.contains(id))
            .collect();
        owed.extend(multi.behind_on_sheet());
        for id in owed {
            self.want_frame(id);
        }
        if self.multi().windows().next().is_none() {
            event_loop.exit();
        }
    }

    // Recovery participation cannot depend on a visible window receiving a
    // redraw. Application worker draining remains first-window-only below.
    fn poll_shared_device_recovery(&mut self) {
        let multi = self.multi();
        for id in multi.windows().collect::<Vec<_>>() {
            multi.with_window(id, |window| window.core.poll_device_recovery());
        }
    }

    fn launch(&mut self, event_loop: &ActiveEventLoop) {
        let init = self.init.take().expect("launched once");
        let WindowsInit {
            state,
            sheet,
            fonts,
            images,
            windows,
        } = init(&self.wake);
        let mut shared = AppShared::default();
        shared.sheet = sheet;
        shared.fonts = fonts;
        shared.images = images
            .into_iter()
            .map(|image| (image.url, image.bytes))
            .collect();
        let hooks = self.hooks.take().expect("launched once");
        self.multi = Some(MultiHost::new(state, shared, hooks));
        for (logic, options) in windows {
            let window = self.window_host(options);
            let id = self.multi().open(logic, window);
            self.show(event_loop, id);
        }
        self.after_turn(event_loop, &[]);
    }
}

impl<State, Logic, V> ApplicationHandler<HostEvent> for WinitWindows<State, Logic, V>
where
    State: 'static,
    Logic: FnMut(&State) -> V + 'static,
    V: RootView<State>,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.multi.is_none() {
            self.launch(event_loop);
            return;
        }
        let multi = self.multi();
        for id in multi.windows().collect::<Vec<_>>() {
            multi.with_window(id, |window| window.resume_surface());
        }
        self.after_turn(event_loop, &[]);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        let multi = self.multi();
        for id in multi.windows().collect::<Vec<_>>() {
            multi.with_window(id, |window| window.suspend_surface());
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(multi) = self.multi.as_mut() else {
            return;
        };
        // Each window asks for its own frames; the loop wakes for the soonest.
        let mut policy = IdlePolicy::Wait;
        for id in multi.windows().collect::<Vec<_>>() {
            let Some(window_policy) = multi.with_window(id, |window| window.idle_turn()) else {
                continue;
            };
            policy = match (policy, window_policy) {
                (IdlePolicy::A11yWake, _) | (_, IdlePolicy::A11yWake) => IdlePolicy::A11yWake,
                (IdlePolicy::Animate(a), IdlePolicy::Animate(b)) => IdlePolicy::Animate(a.min(b)),
                (IdlePolicy::Animate(a), IdlePolicy::Wait)
                | (IdlePolicy::Wait, IdlePolicy::Animate(a)) => IdlePolicy::Animate(a),
                (IdlePolicy::Wait, IdlePolicy::Wait) => IdlePolicy::Wait,
            };
        }
        // Every frame the windows asked for since the last idle turn.
        let asked = |multi: &MultiHost<State, Logic, V, WindowHost<State, Logic, V>>, id| {
            multi
                .slot(id)
                .and_then(|window| window.paced.as_ref())
                .is_some_and(|wanted| wanted.replace(false))
        };
        #[cfg(target_os = "windows")]
        let owed = {
            let mut drew = Vec::new();
            for id in multi.windows().collect::<Vec<_>>() {
                // A hidden or minimized window gets no paint from the platform,
                // and gets none here: its request waits for it to be shown.
                let showing = multi.slot(id).is_some_and(|window| {
                    !window.s.hidden
                        && window
                            .native_window
                            .as_ref()
                            .is_some_and(|native| native.is_minimized() != Some(true))
                });
                if showing && asked(multi, id) {
                    multi.with_window(id, |window| {
                        window.handle_window_event(WindowEvent::RedrawRequested)
                    });
                    drew.push(id);
                }
            }
            if !drew.is_empty() {
                self.after_turn(event_loop, &drew);
            }
            let multi = self.multi();
            // A frame drawn just now that asked for the next one: come straight
            // back. Acquiring it waits on the swapchain, so this paces at the
            // monitor's rate.
            multi.windows().any(|id| {
                multi.slot(id).is_some_and(|window| {
                    !window.s.hidden && window.paced.as_ref().is_some_and(|wanted| wanted.get())
                })
            })
        };
        #[cfg(not(target_os = "windows"))]
        let owed = {
            for id in multi.windows().collect::<Vec<_>>() {
                if asked(multi, id)
                    && let Some(native) = multi.slot(id).and_then(|w| w.native_window.as_ref())
                {
                    native.request_redraw();
                }
            }
            false
        };
        event_loop.set_control_flow(if owed {
            ControlFlow::Poll
        } else {
            control_flow(policy)
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        match event {
            // The wake is the application's, not a window's: its drain hook
            // runs once, in the first open window's turn.
            HostEvent::Wake => {
                self.poll_shared_device_recovery();
                let multi = self.multi();
                let first = multi.windows().next();
                if let Some(id) = first {
                    multi.with_window(id, |window| window.wake_turn());
                }
                // What the drain changed may show in any window, through a
                // leaf or a producer as easily as the document, so every
                // window is asked for a frame.
                for id in self.multi().windows().collect::<Vec<_>>() {
                    self.want_frame(id);
                }
            },
        }
        self.after_turn(event_loop, &[]);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, native: WindowId, event: WindowEvent) {
        let Some(&id) = self.by_native.get(&native) else {
            return;
        };
        self.multi()
            .with_window(id, |window| window.handle_window_event(event));
        self.after_turn(event_loop, &[id]);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use cambium::{AnyView, GenetCtx, GenetElement, el, text};
    use genet_render_host::{DeviceLossNotice, RecoveryStatus, SharedRenderCore};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct App {
        draft: String,
    }
    type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;
    type Logic = fn(&App) -> Child;
    fn root(app: &App) -> Child {
        Box::new(el("div", text(app.draft.clone())))
    }

    #[test]
    fn wake_recovery_polls_hidden_multiwindow_host_without_a_redraw() {
        let wake = HostWake::new(Arc::new(AtomicBool::new(false)), Arc::new(|| {}));
        let mut multi: MultiHost<App, Logic, Child, WindowHost<App, Logic, Child>> = MultiHost::new(
            App {
                draft: "retained authored draft".into(),
            },
            AppShared::default(),
            HostHooks::inert(),
        );
        let mut open = |hidden| {
            let mut state = HostState::new();
            state.hidden = hidden;
            multi.open(
                root as Logic,
                WinitHost::new(Host::new(
                    HostOptions::default(),
                    None,
                    HostHooks::inert(),
                    state,
                    wake.clone(),
                )),
            )
        };
        let visible = open(false);
        let hidden = open(true);
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let owner = SharedRenderCore::from_detached_loss(
            DeviceLossNotice {
                generation: 903,
                reason: wgpu::DeviceLostReason::Unknown,
                message: "CPU-only actual multiwindow routing control".into(),
            },
            Arc::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
                Err("deliberate bounded replacement refusal".into())
            }),
        );
        for id in [visible, hidden] {
            multi.with_window(id, |window| {
                window.core.attach_shared_recovery(owner.clone())
            });
        }
        assert!(matches!(
            multi.with_window(visible, |window| window.core.poll_device_recovery()),
            Some(RecoveryStatus::Waiting {
                remaining_hosts: 1,
                ..
            })
        ));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "first-window drain alone leaves the hidden host's barrier closed"
        );
        let mut windows = WinitWindows {
            multi: Some(multi),
            init: None,
            hooks: None,
            wake,
            by_native: HashMap::new(),
        };
        // This is the exact pre-drain method called by HostEvent::Wake; no
        // event loop, adapter, native window or frame is constructed.
        windows.poll_shared_device_recovery();
        assert!(matches!(owner.status(), RecoveryStatus::Failed { .. }));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        windows.poll_shared_device_recovery();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let multi = windows.multi.as_ref().unwrap();
        assert!(multi.slot(hidden).unwrap().s.hidden);
        assert_eq!(multi.state().draft, "retained authored draft");
        for id in [visible, hidden] {
            let host = multi.slot(id).unwrap();
            assert!(host.s.surface.is_none());
            assert!(matches!(
                host.s.recovery_status,
                Some(RecoveryStatus::Failed { .. })
            ));
        }
    }
}
