// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket's device-free recorder product, shared by standalone and embedded hosts.
//! Hosts own devices, files, identities and scheduling. The recorder owns edits
//! and only promotes audio returned for its current, accepted capture ticket.
#![forbid(unsafe_code)]

pub mod host_policy;

use audio_primitives::{MeterBallistics, PeakMeterSmoother, WaveformPeak};
use hocket_model::{
    Edit, History, Layer, MediaRef, Phrase, PhraseId, Session, SessionId, Track, TrackColor,
    TrackId,
};
use hocket_project::media::{InMemoryStore, MediaStore};
use std::collections::{BTreeSet, VecDeque};

const ACTION_BOUND: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub enum RecorderAction {
    Arm(TrackId),
    AddTrack,
    ToggleRecord,
    ToggleTrackMute(TrackId),
    ToggleLayerMute {
        track: TrackId,
        layer_index: u16,
        phrase: PhraseId,
    },
    NudgeBpm(f32),
    ToggleMasterClock,
    ToggleClick,
    ToggleSolo(TrackId),
    StopAll,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeAudio {
        availability: AudioAvailability,
        phase: CapturePhase,
        poll_error: Option<String>,
        completed: Option<CaptureCompletion>,
        retired: Option<CaptureToken>,
        accepted: Option<CaptureToken>,
        fail_arm: bool,
        fail_play: bool,
        fail_clock: bool,
        fail_stop: bool,
        stop_failures: usize,
        stop_attempts: Vec<LayerKey>,
        plays: Vec<(LayerKey, u32)>,
        stops: Vec<LayerKey>,
        clocks: Vec<(f32, bool)>,
    }
    impl Default for FakeAudio {
        fn default() -> Self {
            Self {
                availability: AudioAvailability {
                    sample_rate: 48_000,
                    input: true,
                    error: None,
                },
                phase: CapturePhase::Idle,
                poll_error: None,
                completed: None,
                retired: None,
                accepted: None,
                fail_arm: false,
                fail_play: false,
                fail_clock: false,
                fail_stop: false,
                stop_failures: 0,
                stop_attempts: Vec::new(),
                plays: Vec::new(),
                stops: Vec::new(),
                clocks: Vec::new(),
            }
        }
    }
    impl RecorderAudio for FakeAudio {
        fn availability(&self) -> AudioAvailability {
            self.availability.clone()
        }
        fn poll(&mut self) -> AudioObservation {
            AudioObservation {
                phase: self.phase.clone(),
                completed: self.completed.take(),
                retired_capture: self.retired.take(),
                error: self.poll_error.take(),
                ..Default::default()
            }
        }
        fn set_clock(&mut self, bpm: f32, _n: u8, _d: u8, click: bool) -> Result<(), String> {
            if self.fail_clock {
                return Err("clock refused".into());
            }
            self.clocks.push((bpm, click));
            Ok(())
        }
        fn set_click(&mut self, _enabled: bool) -> Result<(), String> {
            Ok(())
        }
        fn begin_capture(&mut self, token: CaptureToken, mode: CaptureMode) -> Result<(), String> {
            if self.fail_arm {
                return Err("capture refused".into());
            }
            self.accepted = Some(token);
            self.phase = match mode {
                CaptureMode::Free => CapturePhase::FreeRecording { samples_done: 0 },
                CaptureMode::Clocked { .. } => CapturePhase::Waiting {
                    bars_remaining: 1,
                    samples_until_next_bar: 120,
                },
            };
            Ok(())
        }
        fn stop_free_capture(&mut self, token: CaptureToken) -> Result<(), String> {
            if self.accepted != Some(token) {
                return Err("stale stop".into());
            }
            self.finish(token, 48_000, vec![0.25; 8]);
            Ok(())
        }
        fn play_layer(
            &mut self,
            key: LayerKey,
            _samples: Vec<f32>,
            rate: u32,
            _gain: f32,
            _aligned: bool,
        ) -> Result<(), String> {
            if self.fail_play {
                return Err("playback refused".into());
            }
            self.plays.push((key, rate));
            Ok(())
        }
        fn stop_layer(&mut self, key: LayerKey) -> Result<(), String> {
            self.stop_attempts.push(key);
            if self.fail_stop {
                return Err("stop refused".into());
            }
            if self.stop_failures > 0 {
                self.stop_failures -= 1;
                return Err("stop refused once".into());
            }
            self.stops.push(key);
            Ok(())
        }
    }
    impl FakeAudio {
        fn finish(&mut self, token: CaptureToken, rate: u32, samples: Vec<f32>) {
            self.phase = CapturePhase::Complete;
            self.completed = Some(CaptureCompletion {
                token,
                sample_rate: rate,
                samples,
            });
            self.accepted = None;
        }
    }
    fn service(r: &mut Recorder, a: &mut FakeAudio) {
        r.service(a, 100, 0.016);
    }
    fn armed() -> (Recorder, FakeAudio, CaptureToken) {
        let mut r = Recorder::default();
        let mut a = FakeAudio::default();
        r.arm(0);
        r.toggle_record();
        service(&mut r, &mut a);
        let token = r.active_capture_token().unwrap();
        (r, a, token)
    }
    fn add_layer(r: &mut Recorder, track: usize, rate: u32) -> PhraseId {
        let media = r.store.put(&[0.125, -0.25, -0.0, 0.5], rate);
        let phrase = Phrase::new(media, 4, 120.0, 1);
        let id = phrase.id;
        r.history.commit(
            Edit::AppendLayer {
                track_id: r.session.tracks[track].id,
                layer: Layer::new(id),
                phrase,
            },
            &mut r.session,
            1,
        );
        id
    }

    #[test]
    fn delayed_capture_keeps_admitted_track_clock_and_sample_rate() {
        let (mut r, mut a, token) = armed();
        let id = r.session.tracks[0].id;
        r.session.tracks.swap(0, 1);
        r.session.bpm = 90.0;
        r.session.bars_per_phrase = 2;
        a.availability.sample_rate = 96_000;
        a.finish(token, 48_000, vec![0.25, -0.0]);
        service(&mut r, &mut a);
        let track = r.session.track(id).unwrap();
        assert_eq!(track.layers.len(), 1);
        let phrase = &r.session.phrases[&track.layers[0].phrase_id];
        assert_eq!((phrase.bpm, phrase.length_bars), (120.0, 4));
        let buffer = r.store.get(&phrase.media).unwrap();
        assert_eq!(buffer.sample_rate, 48_000);
        assert_eq!(buffer.samples[1].to_bits(), (-0.0f32).to_bits());
        assert_eq!(a.plays[0].1, 48_000);
        assert_eq!(r.accepted_voice_count(), 1);
    }
    #[test]
    fn same_session_replacement_retires_delayed_capture_and_queued_actions() {
        let (mut r, mut a, token) = armed();
        let session = r.session.clone();
        let history = r.history.clone();
        r.add_track();
        r.replace_project(session, history, InMemoryStore::new(), BTreeSet::new());
        service(&mut r, &mut a);
        assert!(r.is_recording());
        r.toggle_record();
        service(&mut r, &mut a);
        assert_eq!(a.accepted, Some(token));
        let head = r.history.head;
        a.finish(token, 48_000, vec![0.5]);
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert!(r.store.is_empty());
        assert_eq!(r.session.tracks.len(), 4);
        assert!(!r.is_recording());
    }
    #[test]
    fn stale_completion_cannot_consume_current_ticket_and_idle_needs_explicit_retirement() {
        let (mut r, mut a, token) = armed();
        a.phase = CapturePhase::Idle;
        service(&mut r, &mut a);
        assert_eq!(r.active_capture_token(), Some(token));
        a.finish(
            CaptureToken {
                generation: token.generation.wrapping_add(1),
                ..token
            },
            48_000,
            vec![0.5],
        );
        service(&mut r, &mut a);
        assert_eq!(r.active_capture_token(), Some(token));
        assert!(r.store.is_empty());
        a.phase = CapturePhase::Idle;
        a.retired = Some(token);
        service(&mut r, &mut a);
        assert!(!r.is_recording());
    }
    #[test]
    fn capture_admission_and_invalid_completion_fail_without_history_or_media_mutation() {
        let mut r = Recorder::default();
        let mut a = FakeAudio {
            fail_arm: true,
            ..Default::default()
        };
        r.arm(0);
        service(&mut r, &mut a);
        let head = r.history.head;
        r.toggle_record();
        service(&mut r, &mut a);
        assert!(!r.is_recording());
        assert_eq!(r.history.head, head);
        a.fail_arm = false;
        r.toggle_record();
        service(&mut r, &mut a);
        let token = r.active_capture_token().unwrap();
        a.finish(token, 44_100, vec![0.5]);
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert!(r.store.is_empty());
        r.toggle_record();
        service(&mut r, &mut a);
        let token = r.active_capture_token().unwrap();
        a.finish(token, 48_000, vec![f32::NAN]);
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert!(r.store.is_empty());
    }
    #[test]
    fn deleted_capture_target_does_not_receive_or_store_a_delayed_take() {
        let (mut r, mut a, token) = armed();
        r.session.tracks.remove(0);
        let head = r.history.head;
        a.finish(token, 48_000, vec![0.5]);
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert!(r.store.is_empty());
        assert!(r.session.phrases.is_empty());
    }
    #[test]
    fn fixed_rearm_and_clock_changes_are_refused_while_free_stop_keeps_its_target() {
        let (mut r, mut a, token) = armed();
        let head = r.history.head;
        r.arm(1);
        r.bpm_nudge(2.0);
        r.toggle_master_clock();
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert_eq!(r.active_capture_token(), Some(token));
        a.phase = CapturePhase::Idle;
        a.retired = Some(token);
        service(&mut r, &mut a);
        r.toggle_master_clock();
        r.toggle_record();
        service(&mut r, &mut a);
        let old_id = r.session.tracks[0].id;
        assert_eq!(r.active_capture_track(), Some(old_id));
        r.arm(1);
        service(&mut r, &mut a);
        assert!(r.is_recording());
        assert_eq!(r.active_capture_track(), Some(old_id));
        assert_ne!(r.session.tracks[r.armed_index().unwrap()].id, old_id);
        service(&mut r, &mut a);
        assert_eq!(r.session.track(old_id).unwrap().layers.len(), 1);
        assert!(r.session.tracks[1].armed);
        assert_eq!(r.active_capture_track(), None);
    }
    #[test]
    fn muted_click_clock_admission_and_input_absence_remain_truthful() {
        let mut r = Recorder::default();
        let mut a = FakeAudio::default();
        service(&mut r, &mut a);
        assert_eq!(r.audio_status_label(), "system audio");
        r.toggle_click();
        r.bpm_nudge(2.0);
        service(&mut r, &mut a);
        assert_eq!(a.clocks.last(), Some(&(122.0, false)));
        let head = r.history.head;
        a.fail_clock = true;
        r.bpm_nudge(2.0);
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert_eq!(r.session.bpm, 122.0);
        a.availability.input = false;
        r.arm(0);
        service(&mut r, &mut a);
        let head = r.history.head;
        r.toggle_record();
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
        assert!(!r.is_recording());
        assert!(!r.input_available());
    }

    #[test]
    fn offline_project_authors_clock_then_syncs_on_recovery_but_live_refusal_does_not_author() {
        let mut r = Recorder::default();
        let mut a = FakeAudio {
            availability: AudioAvailability {
                sample_rate: 0,
                input: false,
                error: Some("audio unavailable".into()),
            },
            fail_clock: true,
            ..Default::default()
        };
        let initial = r.history.head;
        r.bpm_nudge(2.0);
        r.toggle_click();
        service(&mut r, &mut a);
        assert_eq!(r.session.bpm, 122.0);
        assert!(!r.click);
        assert_ne!(r.history.head, initial);
        assert!(r.needs_sync);
        assert!(!r.input_available());
        assert_eq!(r.accepted_voice_count(), 0);
        assert_eq!(r.audio_status_label(), "audio unavailable");
        assert!(a.clocks.is_empty());
        let authored = r.history.head;
        let bundle = hocket_model::ProjectBundle::new(r.session.clone(), r.history.clone());
        let reopened =
            hocket_model::ProjectBundle::from_bytes(&bundle.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.session.bpm, 122.0);
        assert_eq!(reopened.history.head, authored);
        service(&mut r, &mut a);
        assert_eq!(r.audio_status_label(), "audio unavailable");
        assert!(r.needs_sync);
        assert!(a.clocks.is_empty());
        a.availability = AudioAvailability {
            sample_rate: 48_000,
            input: true,
            error: None,
        };
        a.fail_clock = false;
        service(&mut r, &mut a);
        assert_eq!(a.clocks.last(), Some(&(122.0, false)));
        assert!(!r.needs_sync);
        assert_eq!(r.history.head, authored);
        assert_eq!(r.audio_status_label(), "system audio");
        a.fail_clock = true;
        r.bpm_nudge(2.0);
        service(&mut r, &mut a);
        assert_eq!(r.session.bpm, 122.0);
        assert_eq!(r.history.head, authored);
        assert!(r.audio_status_label().contains("clock refused"));
    }
    #[test]
    fn playback_failure_retains_completed_pcm_without_claiming_an_accepted_voice() {
        let (mut r, mut a, token) = armed();
        a.fail_play = true;
        a.finish(token, 48_000, vec![0.5]);
        service(&mut r, &mut a);
        assert_eq!(r.session.tracks[0].layers.len(), 1);
        assert_eq!(r.store.len(), 1);
        assert_eq!(r.accepted_voice_count(), 0);
        assert!(r.audio_status_label().contains("playback refused"));
    }
    #[test]
    fn target_mute_keeps_unrelated_voices_and_missing_media_remains_visible() {
        let mut r = Recorder::default();
        let phrase = add_layer(&mut r, 0, 44_100);
        add_layer(&mut r, 1, 48_000);
        let mut a = FakeAudio::default();
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        let unrelated = r.session.tracks[1].id;
        a.plays.clear();
        a.stops.clear();
        r.toggle_layer_mute(0, 0);
        service(&mut r, &mut a);
        assert!(a.plays.is_empty());
        assert_eq!(a.stops.len(), 1);
        assert_ne!(a.stops[0].track_id, unrelated);
        a.stops.clear();
        r.toggle_track_mute(0);
        service(&mut r, &mut a);
        assert!(a.stops.is_empty());
        let media = r.session.phrases[&phrase].media;
        r.store = InMemoryStore::new();
        r.missing_media.insert(media);
        assert!(!r.layer_waveform_available(0, 0));
        assert!(r.session.phrases.contains_key(&phrase));
    }
    #[test]
    fn replacing_project_stops_old_voices_even_while_retired_capture_drains() {
        let (mut r, mut a, token) = armed();
        add_layer(&mut r, 1, 48_000);
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        assert_eq!(r.accepted_voice_count(), 1);
        let clocks = a.clocks.len();
        r.replace_project(
            r.session.clone(),
            r.history.clone(),
            r.store.clone(),
            BTreeSet::new(),
        );
        service(&mut r, &mut a);
        assert_eq!(r.accepted_voice_count(), 0);
        assert_eq!(a.clocks.len(), clocks);
        assert!(r.is_recording());
        a.phase = CapturePhase::Idle;
        a.retired = Some(token);
        service(&mut r, &mut a);
        assert!(!r.is_recording());
    }

    #[test]
    fn duplicate_phrase_action_targets_qualified_layer_occurrence() {
        let mut r = Recorder::default();
        let phrase = add_layer(&mut r, 0, 48_000);
        r.session.tracks[0].layers.push(Layer {
            phrase_id: phrase,
            gain: 0.25,
            muted: false,
        });
        let track = r.session.tracks[0].id;
        let mut a = FakeAudio::default();
        r.request(RecorderAction::ToggleLayerMute {
            track,
            layer_index: 1,
            phrase,
        });
        service(&mut r, &mut a);
        assert!(!r.session.tracks[0].layers[0].muted);
        assert!(r.session.tracks[0].layers[1].muted);
        assert_eq!(r.session.tracks[0].layers[1].gain, 0.25);
        let head = r.history.head;
        r.request(RecorderAction::ToggleLayerMute {
            track,
            layer_index: 1,
            phrase: PhraseId::new(),
        });
        service(&mut r, &mut a);
        assert_eq!(r.history.head, head);
    }

    #[test]
    fn tempo_nudge_requeues_accepted_voice_without_stopping_it() {
        let mut r = Recorder::default();
        add_layer(&mut r, 0, 48_000);
        let mut a = FakeAudio::default();
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        let head = r.history.head;
        let clocks = a.clocks.len();
        let plays = a.plays.len();
        a.fail_stop = true;
        r.bpm_nudge(2.0);
        service(&mut r, &mut a);
        // Same-key admission replaces the engine's queued bar-boundary
        // start. The active voice remains accepted until that boundary, so
        // tempo changes no longer depend on a successful stop.
        assert_ne!(r.history.head, head);
        assert_eq!(a.clocks.len(), clocks + 1);
        assert_eq!(a.plays.len(), plays + 1);
        assert!(a.stops.is_empty());
        assert!(a.stop_attempts.is_empty());
        assert_eq!(r.accepted_voice_count(), 1);
        assert_eq!(r.session.bpm, 122.0);
    }

    #[test]
    fn recovered_poll_error_resets_status_from_healthy_availability() {
        let mut r = Recorder::default();
        let mut a = FakeAudio::default();
        a.poll_error = Some("observer failed".into());
        service(&mut r, &mut a);
        assert_eq!(r.audio_status_label(), "observer failed");
        assert_eq!(r.availability.error.as_deref(), Some("observer failed"));

        service(&mut r, &mut a);
        assert_eq!(r.audio_status_label(), "system audio");
        assert!(r.availability.error.is_none());
    }

    #[test]
    fn recreated_controller_for_same_session_cannot_reuse_capture_ticket() {
        let (old, _, old_token) = armed();
        let mut r = Recorder::new(
            old.session.clone(),
            old.history.clone(),
            InMemoryStore::new(),
            BTreeSet::new(),
        );
        let mut a = FakeAudio::default();
        r.toggle_record();
        service(&mut r, &mut a);
        let current = r.active_capture_token().unwrap();
        assert_eq!(old_token.session, current.session);
        assert_eq!(old_token.generation, current.generation);
        assert_ne!(old_token.epoch, current.epoch);
        a.finish(old_token, 48_000, vec![0.5]);
        service(&mut r, &mut a);
        assert_eq!(r.active_capture_token(), Some(current));
        assert!(r.store.is_empty());
    }

    #[test]
    fn imported_audio_only_starts_the_appended_layer_at_its_stored_rate() {
        let mut r = Recorder::default();
        add_layer(&mut r, 1, 48_000);
        let mut a = FakeAudio::default();
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        a.plays.clear();
        a.stops.clear();
        let track = r.session.tracks[0].id;
        let phrase = r
            .append_imported_audio(track, &[0.25, -0.0], 44_100, 19)
            .unwrap();
        service(&mut r, &mut a);
        assert!(a.stops.is_empty());
        assert_eq!(a.plays.len(), 1);
        assert_eq!(
            a.plays[0],
            (
                LayerKey {
                    track_id: track,
                    layer_index: 0
                },
                44_100
            )
        );
        let buffer = r.store.get(&r.session.phrases[&phrase].media).unwrap();
        assert_eq!(buffer.samples[1].to_bits(), (-0.0f32).to_bits());
    }

    #[test]
    fn failed_target_mute_retries_without_another_action_or_unrelated_restart() {
        let mut r = Recorder::default();
        add_layer(&mut r, 0, 48_000);
        add_layer(&mut r, 1, 48_000);
        let mut a = FakeAudio::default();
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        let target = LayerKey {
            track_id: r.session.tracks[0].id,
            layer_index: 0,
        };
        a.plays.clear();
        a.stops.clear();
        a.stop_attempts.clear();
        a.stop_failures = 1;
        r.toggle_track_mute(0);
        service(&mut r, &mut a);
        assert!(r.session.tracks[0].muted);
        assert_eq!(r.accepted_voice_count(), 2);
        assert_eq!(
            r.pending_stops.iter().copied().collect::<Vec<_>>(),
            vec![target]
        );
        assert!(a.stops.is_empty());
        assert!(a.plays.is_empty());
        service(&mut r, &mut a);
        assert_eq!(r.accepted_voice_count(), 1);
        assert!(r.pending_stops.is_empty());
        assert_eq!(a.stops, vec![target]);
        assert_eq!(a.stop_attempts, vec![target, target]);
        assert!(a.plays.is_empty());
        r.toggle_track_mute(0);
        service(&mut r, &mut a);
        assert_eq!(a.plays, vec![(target, 48_000)]);
        assert_eq!(a.stops, vec![target]);
        assert_eq!(r.accepted_voice_count(), 2);
    }

    #[test]
    fn failed_stop_all_retries_only_remaining_accepted_keys() {
        let mut r = Recorder::default();
        add_layer(&mut r, 0, 48_000);
        add_layer(&mut r, 1, 48_000);
        let mut a = FakeAudio::default();
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        a.plays.clear();
        a.stops.clear();
        a.stop_attempts.clear();
        a.stop_failures = 1;
        r.stop_all();
        service(&mut r, &mut a);
        assert_eq!(r.accepted_voice_count(), 1);
        assert_eq!(r.pending_stops.len(), 1);
        let retained = *r.pending_stops.first().unwrap();
        service(&mut r, &mut a);
        assert_eq!(r.accepted_voice_count(), 0);
        assert!(r.pending_stops.is_empty());
        assert_eq!(a.stop_attempts.len(), 3);
        assert_eq!(a.stop_attempts[2], retained);
        assert_eq!(a.stops.len(), 2);
        assert!(a.plays.is_empty());
    }

    #[test]
    fn later_unmute_supersedes_a_still_failed_stop_without_late_silencing() {
        let mut r = Recorder::default();
        add_layer(&mut r, 0, 48_000);
        let mut a = FakeAudio::default();
        r.reconcile_all_playback();
        service(&mut r, &mut a);
        a.plays.clear();
        a.stops.clear();
        a.fail_stop = true;
        r.toggle_layer_mute(0, 0);
        service(&mut r, &mut a);
        assert_eq!(r.pending_stops.len(), 1);
        assert_eq!(r.accepted_voice_count(), 1);
        r.toggle_layer_mute(0, 0);
        service(&mut r, &mut a);
        assert!(!r.session.tracks[0].layers[0].muted);
        assert!(r.pending_stops.is_empty());
        let attempts = a.stop_attempts.len();
        a.fail_stop = false;
        service(&mut r, &mut a);
        assert_eq!(a.stop_attempts.len(), attempts);
        assert!(a.stops.is_empty());
        assert!(a.plays.is_empty());
        assert_eq!(r.accepted_voice_count(), 1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LayerKey {
    pub track_id: TrackId,
    pub layer_index: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureToken {
    pub session: SessionId,
    /// Fresh control-owner epoch, independent of the persisted session ID.
    /// Recreating a Recorder cannot reuse an earlier instance's tickets.
    pub epoch: SessionId,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    Clocked { bars: u8, count_in: u8 },
    Free,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum CapturePhase {
    #[default]
    Idle,
    Waiting {
        bars_remaining: u8,
        samples_until_next_bar: usize,
    },
    Recording {
        progress: f32,
    },
    FreeRecording {
        samples_done: usize,
    },
    Complete,
}

impl CapturePhase {
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Waiting { .. } | Self::Recording { .. } | Self::FreeRecording { .. }
        )
    }
}

pub struct CaptureCompletion {
    pub token: CaptureToken,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct AudioAvailability {
    /// Zero means no output runtime has an admitted clock. Desired project
    /// clock edits remain authorable and wait for output recovery.
    pub sample_rate: u32,
    pub input: bool,
    pub error: Option<String>,
}

pub struct AudioObservation {
    pub phase: CapturePhase,
    pub meter_db: [f32; 2],
    pub completed: Option<CaptureCompletion>,
    pub retired_capture: Option<CaptureToken>,
    pub error: Option<String>,
}

impl Default for AudioObservation {
    fn default() -> Self {
        Self {
            phase: CapturePhase::Idle,
            meter_db: [f32::NEG_INFINITY; 2],
            completed: None,
            retired_capture: None,
            error: None,
        }
    }
}

/// Fallible control and observations supplied by a host. No device is opened
/// by this trait or the product. Capture completion MUST retain the token and
/// sample rate admitted by `begin_capture`, even after device/session changes.
pub trait RecorderAudio {
    fn availability(&self) -> AudioAvailability;
    fn poll(&mut self) -> AudioObservation;
    /// Admit the complete clock/click update together. Failure must preserve
    /// the previously accepted clock; no sequential default is supplied.
    fn set_clock(
        &mut self,
        bpm: f32,
        numerator: u8,
        denominator: u8,
        click: bool,
    ) -> Result<(), String>;
    fn set_click(&mut self, enabled: bool) -> Result<(), String>;
    fn begin_capture(&mut self, token: CaptureToken, mode: CaptureMode) -> Result<(), String>;
    fn stop_free_capture(&mut self, token: CaptureToken) -> Result<(), String>;
    /// Admit playback for this key. A failed same-key replacement must leave
    /// any currently accepted voice for that key running and accepted.
    fn play_layer(
        &mut self,
        key: LayerKey,
        samples: Vec<f32>,
        sample_rate: u32,
        gain: f32,
        aligned: bool,
    ) -> Result<(), String>;
    fn stop_layer(&mut self, key: LayerKey) -> Result<(), String>;
}

struct Ticket {
    token: CaptureToken,
    track: TrackId,
    bpm: f32,
    bars: u8,
    sample_rate: u32,
    mode: CaptureMode,
}

pub struct Recorder {
    pub session: Session,
    pub history: History,
    pub store: InMemoryStore,
    pub missing_media: BTreeSet<MediaRef>,
    pub click: bool,
    pub solo: BTreeSet<TrackId>,
    generation: u64,
    epoch: SessionId,
    ticket: Option<Ticket>,
    phase: CapturePhase,
    actions: VecDeque<RecorderAction>,
    playing: BTreeSet<LayerKey>,
    pending_stops: BTreeSet<LayerKey>,
    availability: AudioAvailability,
    status: String,
    meter_db: [f32; 2],
    meters: [PeakMeterSmoother; 2],
    needs_sync: bool,
    needs_replay: bool,
    retire_voices: bool,
    imported_replay: Vec<(TrackId, u16, PhraseId)>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new(
            Session::new_default(),
            History::new(),
            InMemoryStore::new(),
            BTreeSet::new(),
        )
    }
}

impl Recorder {
    pub fn new(
        session: Session,
        history: History,
        store: InMemoryStore,
        missing_media: BTreeSet<MediaRef>,
    ) -> Self {
        Self {
            session,
            history,
            store,
            missing_media,
            click: true,
            solo: BTreeSet::new(),
            generation: 0,
            epoch: SessionId::new(),
            ticket: None,
            phase: CapturePhase::Idle,
            actions: VecDeque::new(),
            playing: BTreeSet::new(),
            pending_stops: BTreeSet::new(),
            availability: AudioAvailability::default(),
            status: "audio unavailable".into(),
            meter_db: [f32::NEG_INFINITY; 2],
            meters: [PeakMeterSmoother::default(), PeakMeterSmoother::default()],
            needs_sync: true,
            needs_replay: false,
            retire_voices: false,
            imported_replay: Vec::new(),
        }
    }

    /// Replacement always retires the capture epoch, even for the same SessionId.
    /// A backend still draining a fixed capture remains visibly busy until its
    /// observation says otherwise; its old completion cannot author this project.
    pub fn replace_project(
        &mut self,
        session: Session,
        history: History,
        store: InMemoryStore,
        missing_media: BTreeSet<MediaRef>,
    ) {
        if self.is_recording() {
            self.status =
                "previous capture is draining before the recorder clock can change".into();
        }
        self.generation = self.generation.wrapping_add(1);
        self.epoch = SessionId::new();
        self.ticket = None;
        self.actions.clear();
        self.imported_replay.clear();
        self.session = session;
        self.history = history;
        self.store = store;
        self.missing_media = missing_media;
        self.solo.clear();
        self.needs_sync = true;
        self.needs_replay = true;
        self.retire_voices = true;
    }

    pub fn request(&mut self, action: RecorderAction) {
        if self.actions.len() < ACTION_BOUND {
            self.actions.push_back(action);
        } else {
            self.status = "recorder action queue is full".into();
        }
    }

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

    /// Host-decoded clipboard/import PCM enters through the same model owner.
    /// Only the appended occurrence is queued for playback, at its stored rate.
    pub fn append_imported_audio(
        &mut self,
        track: TrackId,
        samples: &[f32],
        sample_rate: u32,
        now: u64,
    ) -> Result<PhraseId, String> {
        if self.is_recording() {
            return Err("stop capture before importing audio".into());
        }
        if self.imported_replay.len() >= ACTION_BOUND {
            return Err("recorder import queue is full".into());
        }
        if samples.is_empty()
            || sample_rate == 0
            || samples.iter().any(|sample| !sample.is_finite())
        {
            return Err("imported audio is empty or invalid".into());
        }
        let t = self
            .session
            .track(track)
            .ok_or("import track is unavailable")?;
        let index =
            u16::try_from(t.layers.len()).map_err(|_| "imported layer exceeds recorder bound")?;
        let media = self.store.put(samples, sample_rate);
        let phrase = Phrase::new(media, self.session.bars_per_phrase, self.session.bpm, now);
        let id = phrase.id;
        self.history.commit(
            Edit::AppendLayer {
                track_id: track,
                layer: Layer::new(id),
                phrase,
            },
            &mut self.session,
            now,
        );
        self.imported_replay.push((track, index, id));
        Ok(id)
    }

    pub fn audio_restarted(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.epoch = SessionId::new();
        self.ticket = None;
        self.phase = CapturePhase::Idle;
        self.playing.clear();
        self.pending_stops.clear();
        self.needs_sync = true;
        self.needs_replay = true;
    }

    fn apply(
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

    fn promote(
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

    fn sync_audio(&mut self, audio: &mut impl RecorderAudio) -> Result<(), String> {
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
    fn stop_voices(&mut self, audio: &mut impl RecorderAudio) {
        for key in self.playing.clone() {
            self.stop_voice(audio, key);
        }
    }
    fn stop_voice(&mut self, audio: &mut impl RecorderAudio, key: LayerKey) {
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
    fn stop_track(&mut self, audio: &mut impl RecorderAudio, id: TrackId) {
        for key in self
            .playing
            .clone()
            .into_iter()
            .filter(|key| key.track_id == id)
        {
            self.stop_voice(audio, key);
        }
    }
    fn reset_status_from_availability(&mut self) {
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
    fn occurrence_is_audible(&self, track_index: usize, layer_index: usize) -> bool {
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
    fn reconcile_all(&mut self, audio: &mut impl RecorderAudio) {
        self.stop_voices(audio);
        for index in 0..self.session.tracks.len() {
            self.reconcile_track(audio, index);
        }
    }
    fn reconcile_track(&mut self, audio: &mut impl RecorderAudio, index: usize) {
        for layer in 0..self.session.tracks[index].layers.len() {
            self.play_from_store(audio, index, layer);
        }
    }
    fn play_from_store(&mut self, audio: &mut impl RecorderAudio, index: usize, li: usize) {
        self.play_from_store_inner(audio, index, li, false);
    }
    fn play_from_store_inner(
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
    pub fn armed_index(&self) -> Option<usize> {
        self.session.tracks.iter().position(|track| track.armed)
    }
    pub fn is_recording(&self) -> bool {
        self.ticket.is_some() || self.phase.is_busy()
    }
    pub fn input_available(&self) -> bool {
        self.availability.input && self.availability.sample_rate != 0
    }
    pub fn audio_status_label(&self) -> &str {
        &self.status
    }
    fn offline_authoring(&self) -> bool {
        self.availability.sample_rate == 0
            && self.playing.is_empty()
            && self.pending_stops.is_empty()
            && !self.is_recording()
    }
    pub fn report_audio_status(&mut self, status: String) {
        self.status = status;
    }
    pub fn active_capture_token(&self) -> Option<CaptureToken> {
        self.ticket.as_ref().map(|ticket| ticket.token)
    }
    /// The accepted capture owner, separate from the currently armed track.
    /// A stopped free take retains its owner until completion is serviced.
    pub fn active_capture_track(&self) -> Option<TrackId> {
        self.ticket.as_ref().map(|ticket| ticket.track)
    }
    pub fn accepted_voice_count(&self) -> usize {
        self.playing.len()
    }
    pub fn capture_phase(&self) -> &CapturePhase {
        &self.phase
    }
    pub fn meter_level(&self, ch: usize) -> f32 {
        self.meters
            .get(ch)
            .map_or(0.0, |meter| meter.reading().level)
    }
    pub fn meter_peak_level(&self, ch: usize) -> f32 {
        self.meters
            .get(ch)
            .map_or(0.0, |meter| meter.reading().peak)
    }
    pub fn meter_db(&self, ch: usize) -> f32 {
        self.meter_db.get(ch).copied().unwrap_or(f32::NEG_INFINITY)
    }
    pub fn set_meter_ballistics(&mut self, ballistics: MeterBallistics) {
        for meter in &mut self.meters {
            meter.set_ballistics(ballistics);
        }
    }

    pub fn arm(&mut self, index: usize) {
        if let Some(track) = self.session.tracks.get(index) {
            self.request(RecorderAction::Arm(track.id));
        }
    }
    pub fn add_track(&mut self) {
        self.request(RecorderAction::AddTrack);
    }
    pub fn toggle_record(&mut self) {
        self.request(RecorderAction::ToggleRecord);
    }
    pub fn toggle_track_mute(&mut self, index: usize) {
        if let Some(track) = self.session.tracks.get(index) {
            self.request(RecorderAction::ToggleTrackMute(track.id));
        }
    }
    pub fn toggle_layer_mute(&mut self, track: usize, layer: u16) {
        if let Some(t) = self.session.tracks.get(track) {
            if let Some(l) = t.layers.get(usize::from(layer)) {
                self.request(RecorderAction::ToggleLayerMute {
                    track: t.id,
                    layer_index: layer,
                    phrase: l.phrase_id,
                });
            }
        }
    }
    pub fn bpm_nudge(&mut self, delta: f32) {
        self.request(RecorderAction::NudgeBpm(delta));
    }
    pub fn toggle_master_clock(&mut self) {
        self.request(RecorderAction::ToggleMasterClock);
    }
    pub fn toggle_click(&mut self) {
        self.request(RecorderAction::ToggleClick);
    }
    pub fn toggle_solo(&mut self, index: usize) {
        if let Some(track) = self.session.tracks.get(index) {
            self.request(RecorderAction::ToggleSolo(track.id));
        }
    }
    pub fn stop_all(&mut self) {
        self.request(RecorderAction::StopAll);
    }

    pub fn render_track_waveform(&self, track: usize, columns: usize) -> Option<Vec<WaveformPeak>> {
        hocket_project::waveform::render_track_peaks(&self.session, &self.store, track, columns)
            .ok()
    }
    pub fn render_layer_waveform(
        &self,
        track: usize,
        layer: usize,
        columns: usize,
    ) -> Option<Vec<WaveformPeak>> {
        hocket_project::waveform::render_layer_peaks(
            &self.session,
            &self.store,
            track,
            layer,
            columns,
        )
        .ok()
    }
    pub fn layer_waveform_available(&self, track: usize, layer: usize) -> bool {
        self.session
            .tracks
            .get(track)
            .and_then(|t| t.layers.get(layer))
            .and_then(|l| self.session.phrases.get(&l.phrase_id))
            .and_then(|p| self.store.get(&p.media))
            .is_some_and(|b| !b.samples.is_empty())
    }
    pub fn track_has_audible_layers(&self, track: usize) -> bool {
        self.session.tracks.get(track).is_some_and(|t| {
            t.layers
                .iter()
                .enumerate()
                .any(|(i, l)| t.playback_mode.is_layer_audible(i as u16, l.muted))
        })
    }
    pub fn track_waveform_available(&self, track: usize) -> bool {
        let Some(t) = self.session.tracks.get(track) else {
            return false;
        };
        let mut rate = None;
        let mut found = false;
        for (i, l) in t.layers.iter().enumerate() {
            if !t.playback_mode.is_layer_audible(i as u16, l.muted) {
                continue;
            }
            let Some(b) = self
                .session
                .phrases
                .get(&l.phrase_id)
                .and_then(|p| self.store.get(&p.media))
            else {
                return false;
            };
            if b.samples.is_empty() || rate.is_some_and(|r| r != b.sample_rate) {
                return false;
            }
            rate = Some(b.sample_rate);
            found = true;
        }
        found
    }
}
