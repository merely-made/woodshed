// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Reusable recorder views over read-only host projections. The existing
//! portable Recorder API adapts to the same renderer and retained leaves.

use cambium::{AnyView, GenetCtx, GenetElement};
use hocket_recorder::{Recorder, RecorderAction};

pub mod leaves;
pub mod projection;
mod recorder_adapter;
mod surface;
pub mod theme;

pub use hocket_recorder::host_policy::HostAction as ProjectedAction;
pub use leaves::{RecorderLeaves, RecorderMount};
pub use projection::*;
pub use recorder_adapter::{HocketKey, recorder_projection};
pub use surface::{projected_surface, projected_table, projected_transport};

pub type Child<S> = Box<dyn AnyView<S, (), GenetCtx, GenetElement>>;
pub type Dispatch<S> = fn(&mut S, RecorderAction);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecorderLayout {
    #[default]
    Full,
    Compact,
}

impl RecorderLayout {
    fn class(self) -> &'static str {
        match self {
            Self::Full => "hocket-recorder-full",
            Self::Compact => "hocket-recorder-compact",
        }
    }
}

pub fn surface<S: 'static>(
    state: &Recorder,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: Dispatch<S>,
) -> Child<S> {
    projected_surface(
        &recorder_projection(state),
        mount,
        layout,
        move |state, action| {
            if let Some(action) = recorder_adapter::recorder_action(action) {
                dispatch(state, action);
            }
        },
    )
}

pub fn table<S: 'static>(
    state: &Recorder,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: Dispatch<S>,
) -> Child<S> {
    projected_table(
        &recorder_projection(state),
        mount,
        layout,
        move |state, action| {
            if let Some(action) = recorder_adapter::recorder_action(action) {
                dispatch(state, action);
            }
        },
    )
}

pub fn transport<S: 'static>(
    state: &Recorder,
    mount: RecorderMount,
    layout: RecorderLayout,
    dispatch: Dispatch<S>,
    accessory: Option<Child<S>>,
) -> Child<S> {
    projected_transport(
        &recorder_projection(state),
        mount,
        layout,
        move |state, action| {
            if let Some(action) = recorder_adapter::recorder_action(action) {
                dispatch(state, action);
            }
        },
        accessory,
    )
}

#[cfg(test)]
mod projection_tests;

#[cfg(test)]
mod recorder_adapter_tests;
