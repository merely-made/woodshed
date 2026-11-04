// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl Recorder {
    pub fn service(&mut self, audio: &mut impl RecorderAudio, now_ms: u64, delta_secs: f32) {
        let had_error = self.availability.error.is_some();
        self.availability = audio.availability();
        if self.availability.sample_rate == 0 {
            self.status = self
                .availability
                .error
                .clone()
                .unwrap_or_else(|| "audio unavailable".into());
        } else if self.availability.error.is_none()
            && (had_error || self.status == "audio unavailable")
        {
            self.reset_status_from_availability();
        }
        if let Some(error) = &self.availability.error {
            self.status = error.clone();
        }
        let observation = audio.poll();
        self.phase = observation.phase;
        self.meter_db = observation.meter_db;
        for (meter, db) in self.meters.iter_mut().zip(self.meter_db) {
            let normalized = if db.is_finite() {
                ((db + 60.0) / 60.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            meter.update(normalized, delta_secs.clamp(0.0, 0.25));
        }
        if let Some(error) = observation.error {
            self.status = error.clone();
            // A failing observation cannot admit a new take in this service
            // pass. A later healthy availability/observation can recover it.
            self.availability.input = false;
            self.availability.error = Some(error);
        }
        if let Some(completion) = observation.completed {
            if let Err(error) = self.promote(completion, audio, now_ms) {
                self.status = error;
            }
        } else if observation.retired_capture.is_some_and(|token| {
            self.ticket
                .as_ref()
                .is_some_and(|ticket| ticket.token == token)
        }) {
            // Explicit retirement distinguishes an abort from a temporarily
            // idle backend that has not executed an accepted command yet.
            self.ticket = None;
        }
        // Reconcile failed targeted stop intent even without another gesture.
        // New audible intent may supersede a pending stop in play_from_store.
        for key in self.pending_stops.clone() {
            self.stop_voice(audio, key);
        }
        if self.retire_voices {
            self.stop_voices(audio);
            self.retire_voices = !self.playing.is_empty();
        }
        if self.needs_sync
            && !self.is_recording()
            && !self.retire_voices
            && self.availability.sample_rate != 0
        {
            match self.sync_audio(audio) {
                Ok(()) => {
                    self.needs_sync = false;
                    if self.status.starts_with("previous capture is draining") {
                        self.reset_status_from_availability();
                    }
                },
                Err(error) => self.status = error,
            }
        }
        if self.needs_replay && !self.needs_sync {
            self.reconcile_all(audio);
            self.needs_replay = false;
        }
        while let Some(action) = self.actions.pop_front() {
            if let Err(error) = self.apply(action, audio, now_ms) {
                self.status = error;
            }
        }
        for (track, layer, phrase) in std::mem::take(&mut self.imported_replay) {
            if let Some(index) = self.session.tracks.iter().position(|t| t.id == track) {
                if self
                    .session
                    .tracks
                    .get(index)
                    .and_then(|track| track.layers.get(usize::from(layer)))
                    .is_some_and(|occurrence| occurrence.phrase_id == phrase)
                {
                    self.play_from_store(audio, index, usize::from(layer));
                }
            }
        }
    }

    pub(super) fn promote(
        &mut self,
        completion: CaptureCompletion,
        audio: &mut impl RecorderAudio,
        now: u64,
    ) -> Result<(), String> {
        let Some(ticket) = &self.ticket else {
            return Ok(());
        };
        if completion.token != ticket.token || completion.token.session != self.session.id {
            return Ok(());
        }
        let ticket = self.ticket.take().unwrap();
        if completion.sample_rate != ticket.sample_rate
            || completion.sample_rate == 0
            || completion.samples.iter().any(|sample| !sample.is_finite())
        {
            return Err(
                "captured audio differs from its admitted rate or contains invalid samples".into(),
            );
        }
        if completion.samples.is_empty() {
            return Ok(());
        }
        let index = self
            .session
            .tracks
            .iter()
            .position(|track| track.id == ticket.track)
            .ok_or("captured track is no longer available")?;
        let layer_index = u16::try_from(self.session.tracks[index].layers.len())
            .map_err(|_| "captured layer exceeds recorder bound")?;
        let media = self.store.put(&completion.samples, ticket.sample_rate);
        let phrase = Phrase::new(media, ticket.bars, ticket.bpm, now);
        let layer = Layer::new(phrase.id);
        self.history.commit(
            Edit::AppendLayer {
                track_id: ticket.track,
                phrase,
                layer,
            },
            &mut self.session,
            now,
        );
        let key = LayerKey {
            track_id: ticket.track,
            layer_index,
        };
        if self.occurrence_is_audible(index, usize::from(layer_index)) {
            audio.play_layer(
                key,
                completion.samples,
                ticket.sample_rate,
                1.0,
                matches!(ticket.mode, CaptureMode::Clocked { .. }),
            )?;
            self.playing.insert(key);
        }
        Ok(())
    }

    pub(super) fn sync_audio(&mut self, audio: &mut impl RecorderAudio) -> Result<(), String> {
        if !self.session.bpm.is_finite()
            || self.session.bpm < 1.0
            || self.session.time_signature.numerator == 0
            || self.session.time_signature.denominator == 0
        {
            return Err("recorder clock is invalid".into());
        }
        self.stop_voices(audio);
        if !self.playing.is_empty() {
            return Err("previous recorder voices have not stopped".into());
        }
        audio.set_clock(
            self.session.bpm,
            self.session.time_signature.numerator,
            self.session.time_signature.denominator,
            self.click,
        )
    }

    pub(super) fn reset_status_from_availability(&mut self) {
        if self.availability.error.is_some() || !self.pending_stops.is_empty() {
            return;
        }
        self.status = if self.availability.input {
            "system audio"
        } else {
            "output only · input unavailable"
        }
        .into();
    }
}
