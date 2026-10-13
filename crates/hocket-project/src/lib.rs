// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket's portable project operations.
//!
//! The model owns session/history and MediaRef identity. This crate carries
//! media, encodes projects through a host-supplied Muniment backend, renders
//! offline loop exports, and verifies recipient-bound signed handoffs. It
//! opens no audio device and has no Firewheel or CPAL dependency.

#![forbid(unsafe_code)]

pub mod export;
pub mod handoff;
pub mod media;
pub mod project_store;
pub mod waveform;
