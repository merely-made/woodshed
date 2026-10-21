// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Native event routing without a window, surface, adapter or device.

use super::*;
use cambium::{AnyView, GenetCtx, GenetElement};
use std::cell::Cell;

type Child = Box<dyn AnyView<(), (), GenetCtx, GenetElement>>;
type Logic = fn(&()) -> Child;

struct Window(Rc<Cell<usize>>);
impl HostWindow for Window {
    fn request_redraw(&self) {
        self.0.set(self.0.get() + 1);
    }
    fn inner_size(&self) -> (u32, u32) {
        (1280, 920)
    }
    fn scale_factor(&self) -> f64 {
        1.0
    }
    fn set_ime_allowed(&self, _: bool) {}
    fn set_ime_cursor_area(&self, _: f64, _: f64, _: f64, _: f64) {}
}

#[test]
fn unocclusion_requests_a_frame_without_booting_or_claiming_presentation() {
    let requests = Rc::new(Cell::new(0));
    let mut state = HostState::new();
    state.window = Some(Box::new(Window(requests.clone())));
    let wake = HostWake::new(state.wake_pending.clone(), Arc::new(|| {}));
    let core: Host<(), Logic, Child> = Host::new(
        HostOptions::default(),
        None,
        HostHooks::inert(),
        state,
        wake,
    );
    let mut host = WinitHost::new(core);

    // An occluded attempt must not create an immediate redraw loop.
    host.handle_window_event(WindowEvent::Occluded(true));
    assert_eq!(requests.get(), 0);
    // The OS visibility transition supplies the next presentation opportunity.
    host.handle_window_event(WindowEvent::Occluded(false));
    assert_eq!(requests.get(), 1);
    assert!(host.s.last_redraw_presentation.is_none());
    assert_eq!(host.s.presentation_sequence, 0);
    assert_eq!(host.core_boots, 0);
    assert!(host.s.shared.render_core.is_none());
    assert!(host.s.surface.is_none());

    // A deliberately application-hidden window remains asleep.
    host.s.hidden = true;
    host.handle_window_event(WindowEvent::Occluded(false));
    assert_eq!(requests.get(), 1);
}
