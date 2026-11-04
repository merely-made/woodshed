// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl Recorder {
    pub(super) fn stop_voices(&mut self, audio: &mut impl RecorderAudio) {
        for key in self.playing.clone() {
            self.stop_voice(audio, key);
        }
    }

    pub(super) fn stop_voice(&mut self, audio: &mut impl RecorderAudio, key: LayerKey) {
        match audio.stop_layer(key) {
            Ok(()) => {
                self.playing.remove(&key);
                self.pending_stops.remove(&key);
            },
            Err(error) => {
                self.pending_stops.insert(key);
                self.status = error;
            },
        }
    }

    pub(super) fn stop_track(&mut self, audio: &mut impl RecorderAudio, id: TrackId) {
        for key in self
            .playing
            .clone()
            .into_iter()
            .filter(|key| key.track_id == id)
        {
            self.stop_voice(audio, key);
        }
    }

    pub(super) fn occurrence_is_audible(&self, track_index: usize, layer_index: usize) -> bool {
        let Some(track) = self.session.tracks.get(track_index) else {
            return false;
        };
        let Ok(layer_index_u16) = u16::try_from(layer_index) else {
            return false;
        };
        !track.muted
            && (self.solo.is_empty() || self.solo.contains(&track.id))
            && track.layers.get(layer_index).is_some_and(|layer| {
                track
                    .playback_mode
                    .is_layer_audible(layer_index_u16, layer.muted)
            })
    }

    pub(super) fn reconcile_all(&mut self, audio: &mut impl RecorderAudio) {
        self.stop_voices(audio);
        for index in 0..self.session.tracks.len() {
            self.reconcile_track(audio, index);
        }
    }

    pub(super) fn reconcile_track(&mut self, audio: &mut impl RecorderAudio, index: usize) {
        for layer in 0..self.session.tracks[index].layers.len() {
            self.play_from_store(audio, index, layer);
        }
    }

    pub(super) fn play_from_store(
        &mut self,
        audio: &mut impl RecorderAudio,
        index: usize,
        li: usize,
    ) {
        self.play_from_store_inner(audio, index, li, false);
    }

    pub(super) fn play_from_store_inner(
        &mut self,
        audio: &mut impl RecorderAudio,
        index: usize,
        li: usize,
        restart_accepted: bool,
    ) {
        if !self.occurrence_is_audible(index, li) {
            return;
        }
        if self.needs_sync || self.retire_voices {
            self.needs_replay = true;
            return;
        }
        let track = &self.session.tracks[index];
        let layer = &track.layers[li];
        let Some(buffer) = self
            .session
            .phrases
            .get(&layer.phrase_id)
            .and_then(|phrase| self.store.get(&phrase.media))
        else {
            return;
        };
        let Ok(layer_index) = u16::try_from(li) else {
            self.status = "layer exceeds recorder bound".into();
            return;
        };
        let key = LayerKey {
            track_id: track.id,
            layer_index,
        };
        if self.playing.contains(&key) && !restart_accepted {
            // Unmute/solo/replay is newer intent than a previously failed
            // ordinary stop. The already accepted voice remains audible.
            self.pending_stops.remove(&key);
            return;
        }
        match audio.play_layer(
            key,
            buffer.samples.clone(),
            buffer.sample_rate,
            layer.gain,
            true,
        ) {
            Ok(()) => {
                self.playing.insert(key);
            },
            Err(error) => self.status = error,
        }
    }

    pub fn resync_tempo(&mut self) {
        self.needs_sync = true;
    }

    pub fn reconcile_all_playback(&mut self) {
        self.needs_replay = true;
    }
}
