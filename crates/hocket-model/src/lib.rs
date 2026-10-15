// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket session data model.
//!
//! Framework-agnostic by design. The audio engine, the UI, and the
//! sync layer all consume this crate as peers. No dependencies on
//! cpal, xilem, masonry, winit, or any UI/audio framework.
//!
//! ## Modules
//!
//! - [`ids`] — `SessionId`, `TrackId`, `PhraseId`, `NodeId`, `MediaRef`
//! - [`phrase`] — `Phrase`, `Layer`
//! - [`track`] — `Track`, `TrackColor`
//! - [`session`] — `Session`, `TimeSignature`, documented defaults
//! - [`history`] — `History`, `HistoryNode`, `Edit`, apply/invert/commit/checkout
//! - [`persistence`] — `ProjectBundle`, CBOR save/load
//!
//! ## Defaults (per `PROJECT_DESCRIPTION.md`)
//!
//! - 4 tracks (collaborator-scaled extension)
//! - Variable-length layers per track (looper-pedal model, append-only)
//! - 4 bars per phrase (session default for new captures)
//! - 120 BPM, 4/4
//!
//! All counts are *defaults, not limits* — the model stores them
//! explicitly so widening is a session-config change, not a refactor.

pub mod history;
pub mod ids;
pub mod persistence;
pub mod phrase;
pub mod session;
pub mod track;

pub use history::{Edit, History, HistoryError, HistoryNode};
pub use ids::{MediaRef, NodeId, PhraseId, SessionId, TrackId};
pub use persistence::{PersistenceError, ProjectBundle};
pub use phrase::{Layer, Phrase};
pub use session::{defaults, Session, TimeSignature};
pub use track::{PlaybackMode, Track, TrackColor};
