// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl Edit {
    /// Apply this edit forward to the given session.
    pub fn apply(&self, session: &mut Session) {
        match self {
            Edit::Genesis => {},
            Edit::SetBpm { to, .. } => session.bpm = *to,
            Edit::SetTimeSignature { to, .. } => session.time_signature = *to,
            Edit::SetBarsPerPhrase { to, .. } => session.bars_per_phrase = *to,
            Edit::SetMasterClock { to, .. } => session.master_clock_enabled = *to,
            Edit::SetCountInBars { to, .. } => session.count_in_bars = *to,
            Edit::AddTrack { track } => session.tracks.push(track.clone()),
            Edit::RenameTrack { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.name = to.clone();
                }
            },
            Edit::SetTrackColor { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.color = *to;
                }
            },
            Edit::ArmTrack { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.armed = *to;
                }
            },
            Edit::MuteTrack { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.muted = *to;
                }
            },
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            } => {
                session
                    .phrases
                    .entry(phrase.id)
                    .or_insert_with(|| phrase.clone());
                if let Some(t) = session.track_mut(*track_id) {
                    t.layers.push(*layer);
                }
            },
            Edit::SetLayerGain {
                track_id,
                layer_index,
                to,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.gain = *to;
                    }
                }
            },
            Edit::SetLayerMute {
                track_id,
                layer_index,
                to,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.muted = *to;
                    }
                }
            },
            Edit::SetTrackPlaybackMode { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.playback_mode = *to;
                }
            },
            Edit::SelectActiveLayer { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let PlaybackMode::SelectOne { active } = &mut t.playback_mode {
                        *active = *to;
                    }
                    // No-op on Sum-mode tracks (documented).
                }
            },
        }
    }

    /// Invert this edit, restoring the session to its pre-apply state.
    pub fn invert(&self, session: &mut Session) {
        match self {
            Edit::Genesis => {},
            Edit::SetBpm { from, .. } => session.bpm = *from,
            Edit::SetTimeSignature { from, .. } => session.time_signature = *from,
            Edit::SetBarsPerPhrase { from, .. } => session.bars_per_phrase = *from,
            Edit::SetMasterClock { from, .. } => session.master_clock_enabled = *from,
            Edit::SetCountInBars { from, .. } => session.count_in_bars = *from,
            Edit::AddTrack { track } => {
                if let Some(index) = session.tracks.iter().rposition(|t| t.id == track.id) {
                    session.tracks.remove(index);
                }
            },
            Edit::RenameTrack { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.name = from.clone();
                }
            },
            Edit::SetTrackColor { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.color = *from;
                }
            },
            Edit::ArmTrack { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.armed = *from;
                }
            },
            Edit::MuteTrack { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.muted = *from;
                }
            },
            Edit::AppendLayer { track_id, .. } => {
                // Layers are append-only; the layer being undone is
                // always the most recent one pushed onto this track.
                if let Some(t) = session.track_mut(*track_id) {
                    t.layers.pop();
                }
                // The phrase stays in the pool (monotonic-pool invariant).
            },
            Edit::SetLayerGain {
                track_id,
                layer_index,
                from,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.gain = *from;
                    }
                }
            },
            Edit::SetLayerMute {
                track_id,
                layer_index,
                from,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.muted = *from;
                    }
                }
            },
            Edit::SetTrackPlaybackMode { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.playback_mode = *from;
                }
            },
            Edit::SelectActiveLayer { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let PlaybackMode::SelectOne { active } = &mut t.playback_mode {
                        *active = *from;
                    }
                }
            },
        }
    }
}
