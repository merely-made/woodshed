// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Track and TrackColor types.
//!
//! A track is a stack of layers. Per-track `PlaybackMode` controls how
//! the layers map to audible output:
//!
//! - [`PlaybackMode::Sum`] — looper-pedal model: all unmuted layers
//!   play simultaneously and sum at the track's mixer. This is the
//!   default for the looper-pedal profile.
//! - [`PlaybackMode::SelectOne`] — Deeler-profile model: exactly one
//!   layer is "active" and plays; the others are dormant. Switching
//!   `active` is the variation-picking gesture.
//!
//! Layers are append-only regardless of mode. "Remove from playback"
//! is `muted = true` in Sum mode or `active = None` / `active = Some(other)`
//! in SelectOne mode. Old layers persist in the pool so history
//! scrubbing and CRDT merges have stable references.

use serde::{Deserialize, Serialize};

use crate::ids::TrackId;
use crate::phrase::Layer;

/// RGB color for a track strip. No alpha; the UI handles transparency
/// at render time.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TrackColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Default palette indexed by track position. Wraps if
    /// `index >= len(palette)`.
    pub fn from_palette_index(index: usize) -> Self {
        // Eight-color default palette. Distinct hues, similar perceived
        // brightness; suitable for both light and dark UI themes.
        const PALETTE: &[TrackColor] = &[
            TrackColor::rgb(0xE8, 0x6A, 0x6A), // warm red
            TrackColor::rgb(0xE8, 0xA0, 0x4A), // amber
            TrackColor::rgb(0xD9, 0xC2, 0x4A), // ochre
            TrackColor::rgb(0x6A, 0xB8, 0x6A), // sage
            TrackColor::rgb(0x4A, 0xA8, 0xC8), // teal
            TrackColor::rgb(0x6A, 0x8A, 0xE8), // periwinkle
            TrackColor::rgb(0xA8, 0x6A, 0xE8), // violet
            TrackColor::rgb(0xE8, 0x6A, 0xB8), // rose
        ];
        PALETTE[index % PALETTE.len()]
    }
}

impl Default for TrackColor {
    fn default() -> Self {
        Self::from_palette_index(0)
    }
}

/// How a track's layers map to audible output.
///
/// Each session profile picks a default:
/// - **Looper-pedal profile** (Hocket's default): `Sum`
/// - **Deeler profile**: `SelectOne { active: None }`
///
/// Profiles are not enforced at the type level — `PlaybackMode` is
/// per-track and can be changed at runtime via `Edit::SetTrackPlaybackMode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaybackMode {
    /// All unmuted layers play simultaneously, summed at the track
    /// mixer. The looper-pedal model.
    Sum,
    /// Exactly one layer is audible at a time (or none, if `active`
    /// is `None`). The Deeler variation-picking model. The `active`
    /// field is the layer index that's currently playing; switching
    /// it is the "pick a different variation" gesture.
    SelectOne {
        active: Option<u16>,
    },
}

impl Default for PlaybackMode {
    fn default() -> Self {
        Self::Sum
    }
}

impl PlaybackMode {
    /// Returns the layer index of the currently active layer in
    /// `SelectOne` mode, or `None` for `Sum` mode (where the concept
    /// doesn't apply).
    pub fn active_layer(&self) -> Option<u16> {
        match self {
            Self::Sum => None,
            Self::SelectOne { active } => *active,
        }
    }

    /// Returns `true` if layer `index` would be audible under this mode,
    /// given its `muted` flag.
    pub fn is_layer_audible(&self, index: u16, layer_muted: bool) -> bool {
        match self {
            Self::Sum => !layer_muted,
            Self::SelectOne { active } => *active == Some(index) && !layer_muted,
        }
    }
}

/// A single track in a session. A stack of layers, plus per-track
/// playback mode, mute, and arm state.
///
/// `layers` is append-only in v0 — recording appends a new layer;
/// "removing" a layer from playback is done via mute (Sum mode) or
/// by switching `playback_mode.active` to a different index (SelectOne
/// mode). Mix-down (a future user gesture) consolidates multiple
/// layers into a single new phrase + replaces them with one layer
/// referencing it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: TrackColor,
    pub layers: Vec<Layer>,
    /// How this track's layers play. Defaults to `Sum` (looper-pedal).
    /// Deeler-profile sessions set this to `SelectOne` per track at
    /// session-construction time.
    pub playback_mode: PlaybackMode,
    /// True when this track is the active capture target.
    pub armed: bool,
    /// True when this track is muted at the track level (independent
    /// of per-layer mute state and of `playback_mode`).
    pub muted: bool,
}

impl Track {
    /// Construct an empty track in `Sum` (looper-pedal) playback mode.
    /// No layers initially.
    pub fn new(name: impl Into<String>, color: TrackColor) -> Self {
        Self::new_with_mode(name, color, PlaybackMode::default())
    }

    /// Construct an empty track with an explicit playback mode.
    /// Deeler-profile sessions use this with
    /// `PlaybackMode::SelectOne { active: None }`.
    pub fn new_with_mode(
        name: impl Into<String>,
        color: TrackColor,
        playback_mode: PlaybackMode,
    ) -> Self {
        Self {
            id: TrackId::new(),
            name: name.into(),
            color,
            layers: Vec::new(),
            playback_mode,
            armed: false,
            muted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_new_has_no_layers_and_sum_mode() {
        let t = Track::new("track 1", TrackColor::default());
        assert!(t.layers.is_empty());
        assert!(!t.armed);
        assert!(!t.muted);
        assert_eq!(t.playback_mode, PlaybackMode::Sum);
    }

    #[test]
    fn track_new_with_mode_records_mode() {
        let t = Track::new_with_mode(
            "drums",
            TrackColor::default(),
            PlaybackMode::SelectOne { active: None },
        );
        assert_eq!(t.playback_mode, PlaybackMode::SelectOne { active: None });
    }

    #[test]
    fn playback_mode_sum_audible_when_unmuted() {
        let m = PlaybackMode::Sum;
        assert!(m.is_layer_audible(0, false));
        assert!(m.is_layer_audible(1, false));
        assert!(!m.is_layer_audible(0, true));
    }

    #[test]
    fn playback_mode_select_one_only_active_layer_audible() {
        let m = PlaybackMode::SelectOne { active: Some(2) };
        assert!(!m.is_layer_audible(0, false));
        assert!(!m.is_layer_audible(1, false));
        assert!(m.is_layer_audible(2, false));
        assert!(!m.is_layer_audible(2, true)); // muted overrides
    }

    #[test]
    fn playback_mode_select_one_none_means_silent() {
        let m = PlaybackMode::SelectOne { active: None };
        assert!(!m.is_layer_audible(0, false));
        assert!(!m.is_layer_audible(1, false));
    }

    #[test]
    fn palette_indices_wrap() {
        let a = TrackColor::from_palette_index(0);
        let b = TrackColor::from_palette_index(8); // wraps to 0
        assert_eq!(a, b);
    }

    #[test]
    fn palette_indices_differ() {
        let a = TrackColor::from_palette_index(0);
        let b = TrackColor::from_palette_index(1);
        assert_ne!(a, b);
    }
}
