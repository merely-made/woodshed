// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Phrase and Layer types.
//!
//! A `Phrase` is an immutable captured-audio record: a content-addressed
//! media reference plus the musical metadata at capture time. Phrases
//! live in a session's append-only phrase pool, keyed by `PhraseId`.
//!
//! A `Layer` is one entry in a track's layer stack. It references a
//! phrase by id and carries playback parameters (gain, mute). Multiple
//! layers on the same track sum at playback (the overdub / looper-pedal
//! model). New captures append a layer; v0 has no remove-layer operation
//! — muting is the user-facing "remove from playback" action, and
//! mix-down (a future user gesture) consolidates layers into a single
//! new phrase.

use serde::{Deserialize, Serialize};

use crate::ids::{MediaRef, PhraseId};

/// A captured phrase — a content-addressed audio reference plus the
/// musical metadata at capture time.
///
/// Phrases are immutable. Re-recording does not modify a phrase; it
/// adds a new phrase to the session pool and a new layer pointing at
/// it. The previous phrase remains addressable through the history
/// graph and through any layers still referencing it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phrase {
    pub id: PhraseId,
    pub media: MediaRef,
    /// Length of the captured audio in bars. Stored per-phrase so a
    /// session config change doesn't invalidate historical phrases.
    pub length_bars: u8,
    /// BPM at the moment of capture.
    pub bpm: f32,
    /// Milliseconds since the Unix epoch at capture time.
    pub captured_at_ms: u64,
}

impl Phrase {
    /// Construct a Phrase with a fresh id.
    pub fn new(media: MediaRef, length_bars: u8, bpm: f32, captured_at_ms: u64) -> Self {
        Self {
            id: PhraseId::new(),
            media,
            length_bars,
            bpm,
            captured_at_ms,
        }
    }
}

/// One entry in a track's layer stack.
///
/// References a `Phrase` from the session pool; carries playback
/// parameters that the audio runtime (Firewheel) applies when this
/// layer plays. Layers on the same track sum at the track's mixer
/// node.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub phrase_id: PhraseId,
    /// Linear gain multiplier. 1.0 = unity.
    pub gain: f32,
    /// When true, this layer is excluded from the track's playback
    /// sum. The layer is preserved in history (this is the v0 way to
    /// "remove" a layer from audible playback).
    pub muted: bool,
}

impl Layer {
    /// Construct a Layer at unity gain, unmuted.
    pub fn new(phrase_id: PhraseId) -> Self {
        Self {
            phrase_id,
            gain: 1.0,
            muted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_new_assigns_fresh_id() {
        let p1 = Phrase::new(MediaRef::ZERO, 4, 120.0, 0);
        let p2 = Phrase::new(MediaRef::ZERO, 4, 120.0, 0);
        assert_ne!(p1.id, p2.id);
    }

    #[test]
    fn layer_new_defaults() {
        let id = PhraseId::new();
        let l = Layer::new(id);
        assert_eq!(l.phrase_id, id);
        assert_eq!(l.gain, 1.0);
        assert!(!l.muted);
    }
}
