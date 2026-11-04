// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

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
fn project_bundle_retains_edited_pack_through_cbor() {
    let mut r = Recorder::default();
    let selection = audio_packs::files::load(std::path::Path::new("bundled"), None).unwrap();
    let selection = selection
        .edit(
            &selection.revision(),
            audio_packs::PackEdit::MoveHit {
                track: 0,
                from: 0,
                to: 1,
            },
        )
        .unwrap();
    let midi = selection.midi().unwrap().bytes;
    let pcm = selection.render_bar(48_000).unwrap();
    r.audio_pack = Some(selection);
    let bundle = r.project_bundle();
    let reopened = hocket_model::ProjectBundle::from_bytes(&bundle.to_bytes().unwrap()).unwrap();
    let pack = reopened.audio_pack.unwrap();
    assert_eq!(midi, pack.midi().unwrap().bytes);
    assert_eq!(pcm, pack.render_bar(48_000).unwrap());
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
    let reopened = hocket_model::ProjectBundle::from_bytes(&bundle.to_bytes().unwrap()).unwrap();
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

#[path = "tests/capture.rs"]
mod capture;
