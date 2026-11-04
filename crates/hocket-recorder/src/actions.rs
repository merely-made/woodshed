// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

impl Recorder {
    pub(super) fn apply(
        &mut self,
        action: RecorderAction,
        audio: &mut impl RecorderAudio,
        now: u64,
    ) -> Result<(), String> {
        match action {
            RecorderAction::Arm(id) => {
                let track = self.session.track(id).ok_or("track is unavailable")?;
                if track.armed {
                    return Ok(());
                }
                if self.is_recording() {
                    let ticket = self
                        .ticket
                        .as_ref()
                        .ok_or("previous capture is still draining")?;
                    if ticket.mode != CaptureMode::Free {
                        return Err("stop fixed capture before re-arming".into());
                    }
                    audio.stop_free_capture(ticket.token)?;
                }
                if let Some(index) = self.armed_index() {
                    self.history.commit(
                        Edit::ArmTrack {
                            track_id: self.session.tracks[index].id,
                            from: true,
                            to: false,
                        },
                        &mut self.session,
                        now,
                    );
                }
                self.history.commit(
                    Edit::ArmTrack {
                        track_id: id,
                        from: false,
                        to: true,
                    },
                    &mut self.session,
                    now,
                );
            },
            RecorderAction::AddTrack => {
                let index = self.session.tracks.len();
                let track = Track::new_with_mode(
                    format!("track {}", index + 1),
                    TrackColor::from_palette_index(index),
                    self.session.default_playback_mode,
                );
                self.history
                    .commit(Edit::AddTrack { track }, &mut self.session, now);
            },
            RecorderAction::ToggleRecord => {
                if self.is_recording() {
                    if let Some(ticket) = &self.ticket {
                        if ticket.mode == CaptureMode::Free {
                            return audio.stop_free_capture(ticket.token);
                        }
                    }
                    return Err("capture is already armed or recording".into());
                }
                if !self.input_available() {
                    return Err("audio input is unavailable".into());
                }
                if self.needs_sync {
                    return Err("recorder clock has not been accepted".into());
                }
                let track = self.armed_index().ok_or("arm a track before recording")?;
                let mode = if self.session.master_clock_enabled {
                    CaptureMode::Clocked {
                        bars: self.session.bars_per_phrase,
                        count_in: self.session.count_in_bars,
                    }
                } else {
                    CaptureMode::Free
                };
                if self.session.bars_per_phrase == 0 || self.session.count_in_bars == u8::MAX {
                    return Err("capture bars/count-in are outside the supported range".into());
                }
                self.generation = self.generation.wrapping_add(1);
                let token = CaptureToken {
                    session: self.session.id,
                    epoch: self.epoch,
                    generation: self.generation,
                };
                let ticket = Ticket {
                    token,
                    track: self.session.tracks[track].id,
                    bpm: self.session.bpm,
                    bars: self.session.bars_per_phrase,
                    sample_rate: self.availability.sample_rate,
                    mode,
                };
                audio.begin_capture(token, mode)?;
                self.ticket = Some(ticket);
                self.phase = match mode {
                    CaptureMode::Free => CapturePhase::FreeRecording { samples_done: 0 },
                    CaptureMode::Clocked { count_in, .. } => CapturePhase::Waiting {
                        bars_remaining: count_in.saturating_add(1),
                        samples_until_next_bar: 0,
                    },
                };
            },
            RecorderAction::ToggleTrackMute(id) => {
                let from = self.session.track(id).ok_or("track is unavailable")?.muted;
                self.history.commit(
                    Edit::MuteTrack {
                        track_id: id,
                        from,
                        to: !from,
                    },
                    &mut self.session,
                    now,
                );
                if !from {
                    self.stop_track(audio, id);
                } else if let Some(index) = self.session.tracks.iter().position(|t| t.id == id) {
                    self.reconcile_track(audio, index);
                }
            },
            RecorderAction::ToggleLayerMute {
                track,
                layer_index,
                phrase,
            } => {
                let t = self.session.track(track).ok_or("track is unavailable")?;
                let index = usize::from(layer_index);
                let layer = t
                    .layers
                    .get(index)
                    .filter(|layer| layer.phrase_id == phrase)
                    .ok_or("layer occurrence is unavailable or has changed")?;
                let from = layer.muted;
                self.history.commit(
                    Edit::SetLayerMute {
                        track_id: track,
                        layer_index,
                        from,
                        to: !from,
                    },
                    &mut self.session,
                    now,
                );
                let key = LayerKey {
                    track_id: track,
                    layer_index,
                };
                if !from {
                    self.stop_voice(audio, key);
                } else if let Some(index) = self.session.tracks.iter().position(|t| t.id == track) {
                    self.play_from_store(audio, index, usize::from(layer_index));
                }
            },
            RecorderAction::NudgeBpm(delta) => {
                if self.is_recording() {
                    return Err("stop capture before changing its clock".into());
                }
                if !delta.is_finite() {
                    return Err("tempo delta must be finite".into());
                }
                let from = self.session.bpm;
                let to = (from + delta).clamp(40.0, 240.0);
                if (to - from).abs() < f32::EPSILON {
                    return Ok(());
                }
                if self.offline_authoring() {
                    self.history
                        .commit(Edit::SetBpm { from, to }, &mut self.session, now);
                    self.needs_sync = true;
                    return Ok(());
                }
                audio.set_clock(
                    to,
                    self.session.time_signature.numerator,
                    self.session.time_signature.denominator,
                    self.click,
                )?;
                self.history
                    .commit(Edit::SetBpm { from, to }, &mut self.session, now);
                self.reset_status_from_availability();

                // Queue same-key replacements only for accepted voices that
                // are not already headed toward a stop. The host keeps each
                // current voice running until its next-bar replacement;
                // failed admission therefore leaves its accepted playback
                // intact. Store lookup and B5 audibility policy remain in
                // the shared playback helper below.
                if !self.needs_sync && !self.retire_voices {
                    for key in self
                        .playing
                        .iter()
                        .copied()
                        .filter(|key| !self.pending_stops.contains(key))
                        .collect::<Vec<_>>()
                    {
                        if let Some(index) = self
                            .session
                            .tracks
                            .iter()
                            .position(|track| track.id == key.track_id)
                        {
                            self.play_from_store_inner(
                                audio,
                                index,
                                usize::from(key.layer_index),
                                true,
                            );
                        }
                    }
                }
            },
            RecorderAction::ToggleMasterClock => {
                if self.is_recording() {
                    return Err("stop capture before changing its clock".into());
                }
                let from = self.session.master_clock_enabled;
                self.history.commit(
                    Edit::SetMasterClock { from, to: !from },
                    &mut self.session,
                    now,
                );
            },
            RecorderAction::ToggleClick => {
                if self.offline_authoring() {
                    self.click = !self.click;
                    self.needs_sync = true;
                    return Ok(());
                }
                audio.set_click(!self.click)?;
                self.click = !self.click;
            },
            RecorderAction::ToggleSolo(id) => {
                self.session.track(id).ok_or("track is unavailable")?;
                if !self.solo.remove(&id) {
                    self.solo.insert(id);
                }
                self.reconcile_all(audio);
            },
            RecorderAction::StopAll => self.stop_voices(audio),
        }
        Ok(())
    }
}
