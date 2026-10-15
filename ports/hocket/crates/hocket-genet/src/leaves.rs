// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Standalone recorder leaf adapter. Actual projection, retained rendering and
//! mount-qualified keys are shared with embedded recorder surfaces.

use crate::state::AppState;
use hocket_views::{RecorderLeaves, RecorderMount};
use sprigging::LeafRegistry;

pub const STANDALONE_MOUNT: RecorderMount = RecorderMount::new(1);

pub struct WaveformCache(RecorderLeaves);

impl Default for WaveformCache {
    fn default() -> Self {
        Self::new()
    }
}

impl WaveformCache {
    pub fn new() -> Self {
        Self(RecorderLeaves::new(STANDALONE_MOUNT))
    }
}

pub fn reconcile(registry: &mut LeafRegistry<u64>, cache: &mut WaveformCache, state: &AppState) {
    cache.0.reconcile(registry, &state.recorder);
}
