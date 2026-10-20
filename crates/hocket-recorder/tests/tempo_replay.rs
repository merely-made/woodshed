// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::collections::{BTreeMap, BTreeSet};

use hocket_model::{Edit, Layer, Phrase, PlaybackMode};
use hocket_project::media::MediaStore;
use hocket_recorder::{
    AudioAvailability, AudioObservation, CaptureMode, CaptureToken, LayerKey, Recorder,
    RecorderAudio,
};

#[derive(Clone, Debug, PartialEq)]
struct Played {
    samples: Vec<f32>,
    sample_rate: u32,
    gain: f32,
    aligned: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Event {
    Stop(LayerKey),
    Clock(f32),
    Play(LayerKey),
}

struct FakeAudio {
    sample_rate: u32,
    active: BTreeMap<LayerKey, Played>,
    events: Vec<Event>,
    fail_clock: bool,
    fail_stop: BTreeSet<LayerKey>,
    fail_play: BTreeSet<LayerKey>,
}

impl Default for FakeAudio {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            active: BTreeMap::new(),
            events: Vec::new(),
            fail_clock: false,
            fail_stop: BTreeSet::new(),
            fail_play: BTreeSet::new(),
        }
    }
}

impl RecorderAudio for FakeAudio {
    fn availability(&self) -> AudioAvailability {
        AudioAvailability {
            sample_rate: self.sample_rate,
            input: true,
            error: None,
        }
    }

    fn poll(&mut self) -> AudioObservation {
        AudioObservation::default()
    }

    fn set_clock(
        &mut self,
        bpm: f32,
        _numerator: u8,
        _denominator: u8,
        _click: bool,
    ) -> Result<(), String> {
        self.events.push(Event::Clock(bpm));
        if self.fail_clock {
            return Err("clock refused".into());
        }
        Ok(())
    }

    fn set_click(&mut self, _enabled: bool) -> Result<(), String> {
        Ok(())
    }

    fn begin_capture(&mut self, _token: CaptureToken, _mode: CaptureMode) -> Result<(), String> {
        Ok(())
    }

    fn stop_free_capture(&mut self, _token: CaptureToken) -> Result<(), String> {
        Ok(())
    }

    fn play_layer(
        &mut self,
        key: LayerKey,
        samples: Vec<f32>,
        sample_rate: u32,
        gain: f32,
        aligned: bool,
    ) -> Result<(), String> {
        self.events.push(Event::Play(key));
        if self.fail_play.contains(&key) {
            return Err("playback refused".into());
        }
        self.active.insert(
            key,
            Played {
                samples,
                sample_rate,
                gain,
                aligned,
            },
        );
        Ok(())
    }

    fn stop_layer(&mut self, key: LayerKey) -> Result<(), String> {
        self.events.push(Event::Stop(key));
        if self.fail_stop.contains(&key) {
            return Err("stop refused".into());
        }
        self.active.remove(&key);
        Ok(())
    }
}

fn service(recorder: &mut Recorder, audio: &mut FakeAudio) {
    recorder.service(audio, 100, 0.016);
}

fn start_playback(recorder: &mut Recorder, audio: &mut FakeAudio) {
    recorder.reconcile_all_playback();
    service(recorder, audio);
}

fn append_layer(recorder: &mut Recorder, track: usize, samples: &[f32], rate: u32, gain: f32) {
    let media = recorder.store.put(samples, rate);
    let phrase = Phrase::new(media, 4, 120.0, 1);
    let id = phrase.id;
    let track_id = recorder.session.tracks[track].id;
    recorder.history.commit(
        Edit::AppendLayer {
            track_id,
            layer: Layer {
                phrase_id: id,
                gain,
                muted: false,
            },
            phrase,
        },
        &mut recorder.session,
        1,
    );
}

fn set_select_one(recorder: &mut Recorder, track: usize, active: Option<u16>) {
    let track_id = recorder.session.tracks[track].id;
    let from = recorder.session.tracks[track].playback_mode;
    recorder.history.commit(
        Edit::SetTrackPlaybackMode {
            track_id,
            from,
            to: PlaybackMode::SelectOne { active },
        },
        &mut recorder.session,
        2,
    );
}

fn key(recorder: &Recorder, track: usize, layer: u16) -> LayerKey {
    LayerKey {
        track_id: recorder.session.tracks[track].id,
        layer_index: layer,
    }
}

#[test]
fn successful_nudges_replay_only_currently_eligible_live_voices_in_order() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1, -0.2], 44_100, 0.35);
    append_layer(&mut recorder, 0, &[0.3], 32_000, 0.6);
    append_layer(&mut recorder, 1, &[0.4, 0.5], 22_050, 0.8);
    append_layer(&mut recorder, 1, &[0.6], 48_000, 0.9);
    set_select_one(&mut recorder, 1, Some(1));

    let mut audio = FakeAudio::default();
    start_playback(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 3);
    assert_eq!(audio.active[&key(&recorder, 0, 0)].sample_rate, 44_100);
    assert_eq!(audio.active[&key(&recorder, 0, 1)].gain, 0.6);
    assert_eq!(audio.active[&key(&recorder, 1, 1)].samples, vec![0.6]);

    // Mute one summed voice and solo the Sum track. Only the remaining live,
    // audible Sum voice should be restarted by tempo changes.
    recorder.toggle_layer_mute(0, 0);
    service(&mut recorder, &mut audio);
    recorder.toggle_solo(0);
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 1);
    let eligible = key(&recorder, 0, 1);

    for (expected_bpm, delta) in [(122.0, 2.0), (125.0, 3.0)] {
        audio.events.clear();
        recorder.bpm_nudge(delta);
        service(&mut recorder, &mut audio);
        assert_eq!(recorder.session.bpm, expected_bpm);
        assert_eq!(recorder.accepted_voice_count(), 1);
        assert_eq!(
            audio.active.keys().copied().collect::<BTreeSet<_>>(),
            [eligible].into()
        );
        let clock_at = audio
            .events
            .iter()
            .position(|event| *event == Event::Clock(expected_bpm));
        let play_at = audio
            .events
            .iter()
            .position(|event| *event == Event::Play(eligible));
        assert!(clock_at.zip(play_at).is_some_and(|(c, p)| c < p));
        assert!(
            !audio
                .events
                .iter()
                .any(|event| matches!(event, Event::Stop(_)))
        );
        assert_eq!(audio.active[&eligible].sample_rate, 32_000);
        assert_eq!(audio.active[&eligible].gain, 0.6);
        assert!(audio.active[&eligible].aligned);
        assert!(!audio.active.contains_key(&key(&recorder, 1, 1)));
    }
}

#[test]
fn refused_clock_keeps_existing_voices_and_history_untouched() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1, 0.2], 44_100, 0.5);
    let mut audio = FakeAudio::default();
    start_playback(&mut recorder, &mut audio);
    let live_before = audio.active.clone();
    assert_eq!(live_before.len(), 1);
    let bpm_before = recorder.session.bpm;
    let history_before = recorder.history.head;
    let accepted_before = recorder.accepted_voice_count();
    audio.events.clear();
    audio.fail_clock = true;

    recorder.bpm_nudge(5.0);
    service(&mut recorder, &mut audio);

    assert_eq!(audio.active, live_before);
    assert_eq!(recorder.session.bpm, bpm_before);
    assert_eq!(recorder.history.head, history_before);
    assert_eq!(recorder.accepted_voice_count(), accepted_before);
    assert!(
        audio
            .events
            .iter()
            .all(|event| matches!(event, Event::Clock(_)))
    );
    assert_eq!(recorder.audio_status_label(), "clock refused");

    audio.fail_clock = false;
    audio.events.clear();
    recorder.bpm_nudge(5.0);
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.session.bpm, bpm_before + 5.0);
    assert_eq!(recorder.accepted_voice_count(), accepted_before);
    assert!(
        audio
            .events
            .iter()
            .any(|event| matches!(event, Event::Play(_)))
    );
    assert_eq!(recorder.audio_status_label(), "system audio");
}

#[test]
fn stop_all_then_nudge_does_not_replay_silent_model_layers() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1], 48_000, 1.0);
    append_layer(&mut recorder, 0, &[0.2], 48_000, 0.4);
    let mut audio = FakeAudio::default();
    start_playback(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 2);

    recorder.stop_all();
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 0);
    audio.events.clear();
    recorder.bpm_nudge(1.0);
    service(&mut recorder, &mut audio);

    assert_eq!(recorder.accepted_voice_count(), 0);
    assert!(audio.active.is_empty());
    assert!(
        !audio
            .events
            .iter()
            .any(|event| matches!(event, Event::Play(_)))
    );
}

#[test]
fn refused_stop_or_replay_reports_only_accepted_voices() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1], 48_000, 0.7);
    append_layer(&mut recorder, 0, &[0.2], 44_100, 0.8);
    let mut audio = FakeAudio::default();
    start_playback(&mut recorder, &mut audio);
    let first = key(&recorder, 0, 0);
    let second = key(&recorder, 0, 1);

    audio.events.clear();
    audio.fail_stop.insert(first);
    recorder.stop_all();
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 1);
    assert!(audio.active.contains_key(&first));
    assert!(!audio.active.contains_key(&second));
    assert_eq!(recorder.audio_status_label(), "stop refused");
    audio.events.clear();
    recorder.bpm_nudge(2.0);
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 1);
    assert!(audio.active.contains_key(&first));
    assert!(
        !audio
            .events
            .iter()
            .any(|event| *event == Event::Play(first))
    );
    assert_eq!(recorder.audio_status_label(), "stop refused");

    // A fresh, non-pending accepted key remains live if keyed replacement is refused.
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.3], 48_000, 0.7);
    append_layer(&mut recorder, 0, &[0.4], 44_100, 0.8);
    audio = FakeAudio::default();
    start_playback(&mut recorder, &mut audio);
    let first = key(&recorder, 0, 0);
    let second = key(&recorder, 0, 1);
    audio.fail_play.insert(second);
    audio.events.clear();
    recorder.bpm_nudge(2.0);
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 2);
    assert!(audio.active.contains_key(&first));
    assert!(audio.active.contains_key(&second));
    assert_eq!(recorder.audio_status_label(), "playback refused");

    audio.fail_play.clear();
    audio.events.clear();
    recorder.bpm_nudge(2.0);
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 2);
    assert_eq!(recorder.audio_status_label(), "system audio");
}

#[test]
fn offline_tempo_authoring_replays_audible_layers_after_audio_recovers() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1, -0.1], 22_050, 0.45);
    let mut audio = FakeAudio {
        sample_rate: 0,
        ..FakeAudio::default()
    };
    let before = recorder.history.head;
    service(&mut recorder, &mut audio);
    recorder.bpm_nudge(4.0);
    service(&mut recorder, &mut audio);
    assert!(recorder.history.head != before);
    assert_eq!(recorder.session.bpm, 124.0);
    assert_eq!(recorder.accepted_voice_count(), 0);

    audio.sample_rate = 48_000;
    recorder.audio_restarted();
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 1);
    let played = audio.active.values().next().unwrap();
    assert_eq!(played.sample_rate, 22_050);
    assert_eq!(played.gain, 0.45);
}
