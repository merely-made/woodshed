// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Session: top-level container for everything in a hocket project.
//!
//! Holds transport settings, tracks (each a stack of layers), the
//! pool of all phrases captured in this session, and a reference to
//! the history graph (which lives in [`crate::history::History`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{PhraseId, SessionId};
use crate::phrase::Phrase;
use crate::track::{PlaybackMode, Track, TrackColor};

/// Time signature in beats-per-bar + beat-unit.
///
/// Mirrors `woodshed_audio::TimeSignature`'s shape so the engine can
/// translate without loss. Kept local to keep `hocket-model`
/// framework-agnostic per `CLAUDE.md`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl TimeSignature {
    pub const fn new(numerator: u8, denominator: u8) -> Self {
        Self { numerator, denominator }
    }
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self::new(4, 4)
    }
}

/// Defaults documented in `PROJECT_DESCRIPTION.md`:
/// 4 tracks (collaborator-scaled), variable-length layers, 4 bars per
/// phrase, 120 BPM, 4/4 — the looper-pedal profile.
pub mod defaults {
    pub const TRACK_COUNT: usize = 4;
    pub const BARS_PER_PHRASE: u8 = 4;
    pub const BPM: f32 = 120.0;
    /// Master clock on by default — the click is audible, captures
    /// bar-align, and count-in applies. Turning it off mutes the click
    /// and skips count-in (the "no metronome" looper mode).
    pub const MASTER_CLOCK_ENABLED: bool = true;
    /// Bars of count-in before a capture begins (only when the master
    /// clock is on).
    pub const COUNT_IN_BARS: u8 = 1;
}

/// Deeler-profile defaults: 10 mono tracks, 4 variation slots per
/// track (`SelectOne` mode), fixed 4-bar phrases, click-driven.
/// Matches the structure described in the Menomena / Tape Op
/// interview about the Max/MSP patch.
pub mod deeler_defaults {
    pub const TRACK_COUNT: usize = 10;
    pub const BARS_PER_PHRASE: u8 = 4;
    pub const BPM: f32 = 120.0;
}

/// A hocket session.
///
/// Counts (track count, bars per phrase) are stored explicitly rather
/// than baked into types, so widening any of them is a session-config
/// change, not a refactor — per "defaults, not limits."
///
/// `phrases` is the append-only content-addressed pool: each
/// `PhraseId` resolves to exactly one `Phrase`. Layers in tracks hold
/// `PhraseId`s referring into this pool. Re-recording adds a new
/// phrase and appends a new layer; the old phrase stays in the pool
/// (reachable via history or via any layers still referencing it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub bpm: f32,
    pub time_signature: TimeSignature,
    /// Default phrase length for new captures, in bars. The engine
    /// reads this when arming capture; per-phrase length is stored on
    /// the `Phrase` itself so historical phrases survive a session
    /// config change.
    pub bars_per_phrase: u8,
    /// Playback mode for newly-added tracks in this session. Existing
    /// tracks keep their per-track `playback_mode`; this is the value
    /// session-profile constructors apply to the initial track set
    /// and that future "add track" edits would inherit.
    ///
    /// - Looper-pedal profile: `PlaybackMode::Sum`
    /// - Deeler profile: `PlaybackMode::SelectOne { active: None }`
    pub default_playback_mode: PlaybackMode,
    /// When true, the session has an audible click, bar-aligned capture,
    /// and count-in. When false, the click is muted and count-in is
    /// skipped (the "free / no metronome" looper mode). Tempo + time
    /// signature stay informational either way.
    pub master_clock_enabled: bool,
    /// Bars of count-in before capture starts. Only applied when
    /// `master_clock_enabled`.
    pub count_in_bars: u8,
    pub tracks: Vec<Track>,
    /// Pool of all phrases captured in this session, addressable by
    /// id. `BTreeMap` (not `HashMap`) for deterministic serialization
    /// order.
    pub phrases: BTreeMap<PhraseId, Phrase>,
}

impl Session {
    /// Construct a new session in the **looper-pedal profile**:
    /// 4 tracks in `PlaybackMode::Sum`, 4 bars per phrase, 120 BPM,
    /// 4/4. Tracks start with zero layers — capture appends them.
    pub fn new_default() -> Self {
        Self::new_with_track_count(defaults::TRACK_COUNT, PlaybackMode::Sum)
    }

    /// Construct a new session in the **Deeler profile**: 10 tracks
    /// in `PlaybackMode::SelectOne { active: None }`, 4 bars per
    /// phrase, 120 BPM, 4/4. Matches the Menomena / Deeler workflow:
    /// pick one of up to four (UI-conventional) variations per track,
    /// summed across tracks not within tracks.
    pub fn new_deeler_profile() -> Self {
        Self::new_with_track_count(
            deeler_defaults::TRACK_COUNT,
            PlaybackMode::SelectOne { active: None },
        )
    }

    /// Construct a session with a custom initial track count and
    /// default playback mode. All initial tracks are created in
    /// the given mode.
    pub fn new_with_track_count(
        track_count: usize,
        default_playback_mode: PlaybackMode,
    ) -> Self {
        let mut tracks = Vec::with_capacity(track_count);
        for i in 0..track_count {
            tracks.push(Track::new_with_mode(
                format!("track {}", i + 1),
                TrackColor::from_palette_index(i),
                default_playback_mode,
            ));
        }
        Self {
            id: SessionId::new(),
            bpm: defaults::BPM,
            time_signature: TimeSignature::default(),
            bars_per_phrase: defaults::BARS_PER_PHRASE,
            default_playback_mode,
            master_clock_enabled: defaults::MASTER_CLOCK_ENABLED,
            count_in_bars: defaults::COUNT_IN_BARS,
            tracks,
            phrases: BTreeMap::new(),
        }
    }

    /// Look up a track by id.
    pub fn track(&self, id: crate::ids::TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Mutable lookup by id.
    pub fn track_mut(&mut self, id: crate::ids::TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_default_is_looper_profile() {
        let s = Session::new_default();
        assert_eq!(s.bpm, 120.0);
        assert_eq!(s.time_signature, TimeSignature::new(4, 4));
        assert_eq!(s.bars_per_phrase, 4);
        assert_eq!(s.tracks.len(), 4);
        assert_eq!(s.default_playback_mode, PlaybackMode::Sum);
        for t in &s.tracks {
            assert_eq!(t.playback_mode, PlaybackMode::Sum);
        }
        assert!(s.phrases.is_empty());
    }

    #[test]
    fn new_default_tracks_start_empty() {
        let s = Session::new_default();
        for t in &s.tracks {
            assert!(t.layers.is_empty(), "tracks start with no layers");
        }
    }

    #[test]
    fn new_deeler_profile_has_ten_select_one_tracks() {
        let s = Session::new_deeler_profile();
        assert_eq!(s.tracks.len(), 10);
        assert_eq!(
            s.default_playback_mode,
            PlaybackMode::SelectOne { active: None }
        );
        for t in &s.tracks {
            assert_eq!(t.playback_mode, PlaybackMode::SelectOne { active: None });
            assert!(t.layers.is_empty());
        }
    }

    #[test]
    fn new_with_track_count_respects_count_and_mode() {
        let s = Session::new_with_track_count(7, PlaybackMode::SelectOne { active: None });
        assert_eq!(s.tracks.len(), 7);
        for t in &s.tracks {
            assert_eq!(t.playback_mode, PlaybackMode::SelectOne { active: None });
        }
    }
}
