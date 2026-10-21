// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeSet;

use hocket_model::{Edit, Layer, Phrase, PlaybackMode, ProjectBundle, TrackId};
use hocket_project::media::{InMemoryStore, MediaStore};
use hocket_recorder::{
    AudioAvailability, AudioObservation, CaptureCompletion, CaptureMode, CapturePhase,
    CaptureToken, LayerKey, Recorder, RecorderAudio,
};

#[derive(Clone, Debug, PartialEq)]
struct Played {
    key: LayerKey,
    samples: Vec<f32>,
    sample_rate: u32,
    gain: f32,
    aligned: bool,
}

#[derive(Default)]
struct FakeAudio {
    completed: Option<CaptureCompletion>,
    accepted_capture: Option<CaptureToken>,
    capture_modes: Vec<CaptureMode>,
    plays: Vec<Played>,
    stops: Vec<LayerKey>,
    active: std::collections::BTreeMap<LayerKey, Played>,
}

impl RecorderAudio for FakeAudio {
    fn availability(&self) -> AudioAvailability {
        AudioAvailability {
            sample_rate: 48_000,
            input: true,
            error: None,
        }
    }

    fn poll(&mut self) -> AudioObservation {
        AudioObservation {
            phase: self
                .completed
                .as_ref()
                .map_or(CapturePhase::Idle, |_| CapturePhase::Complete),
            completed: self.completed.take(),
            ..AudioObservation::default()
        }
    }

    fn set_clock(&mut self, _bpm: f32, _n: u8, _d: u8, _click: bool) -> Result<(), String> {
        Ok(())
    }

    fn set_click(&mut self, _enabled: bool) -> Result<(), String> {
        Ok(())
    }

    fn begin_capture(&mut self, token: CaptureToken, mode: CaptureMode) -> Result<(), String> {
        self.accepted_capture = Some(token);
        self.capture_modes.push(mode);
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
        let played = Played {
            key,
            samples,
            sample_rate,
            gain,
            aligned,
        };
        self.active.insert(key, played.clone());
        self.plays.push(played);
        Ok(())
    }

    fn stop_layer(&mut self, key: LayerKey) -> Result<(), String> {
        self.stops.push(key);
        self.active.remove(&key);
        Ok(())
    }
}

fn service(recorder: &mut Recorder, audio: &mut FakeAudio) {
    recorder.service(audio, 100, 0.016);
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

fn set_mode(recorder: &mut Recorder, track: usize, mode: PlaybackMode) {
    let target_id = recorder.session.tracks[track].id;
    let from = recorder.session.tracks[track].playback_mode;
    recorder.history.commit(
        Edit::SetTrackPlaybackMode {
            track_id: target_id,
            from,
            to: mode,
        },
        &mut recorder.session,
        2,
    );
}

fn expected_audible(recorder: &Recorder) -> BTreeSet<LayerKey> {
    recorder
        .session
        .tracks
        .iter()
        .filter(|track| {
            !track.muted && (recorder.solo.is_empty() || recorder.solo.contains(&track.id))
        })
        .flat_map(|track| {
            track
                .layers
                .iter()
                .enumerate()
                .filter_map(move |(index, layer)| {
                    let index = u16::try_from(index).ok()?;
                    track
                        .playback_mode
                        .is_layer_audible(index, layer.muted)
                        .then_some(LayerKey {
                            track_id: track.id,
                            layer_index: index,
                        })
                })
        })
        .collect()
}

fn active_keys(audio: &FakeAudio) -> BTreeSet<LayerKey> {
    audio.active.keys().copied().collect()
}

#[test]
fn reconciliation_admits_only_model_selected_layers_and_preserves_sum_behavior() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1], 44_100, 0.5);
    append_layer(&mut recorder, 0, &[0.2], 48_000, 0.75);
    append_layer(&mut recorder, 0, &[0.3], 32_000, 1.0);
    append_layer(&mut recorder, 1, &[0.4], 48_000, 1.0);
    set_mode(
        &mut recorder,
        0,
        PlaybackMode::SelectOne { active: Some(1) },
    );
    let session_id = recorder.session.id;
    let bundle = ProjectBundle::new(recorder.session.clone(), recorder.history.clone());
    let restored = ProjectBundle::from_bytes(&bundle.to_bytes().unwrap()).unwrap();
    recorder = Recorder::new(
        restored.session,
        restored.history,
        recorder.store.clone(),
        BTreeSet::new(),
    );
    assert_eq!(recorder.session.id, session_id);
    assert_eq!(
        recorder.session.tracks[0].playback_mode,
        PlaybackMode::SelectOne { active: Some(1) }
    );
    let mut audio = FakeAudio::default();
    service(&mut recorder, &mut audio);

    recorder.reconcile_all_playback();
    service(&mut recorder, &mut audio);
    assert_eq!(active_keys(&audio), expected_audible(&recorder));
    assert_eq!(recorder.accepted_voice_count(), 2);
    assert!(
        audio
            .plays
            .iter()
            .any(|p| p.key.track_id == recorder.session.tracks[0].id && p.key.layer_index == 1)
    );
    assert!(
        audio
            .plays
            .iter()
            .any(|p| p.key.track_id == recorder.session.tracks[1].id)
    );

    set_mode(&mut recorder, 0, PlaybackMode::SelectOne { active: None });
    recorder.reconcile_all_playback();
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 1);
    assert_eq!(active_keys(&audio), expected_audible(&recorder));
    assert!(audio.stops.contains(&LayerKey {
        track_id: recorder.session.tracks[0].id,
        layer_index: 1,
    }));
}

#[test]
fn capture_promotion_obeys_select_one_and_keeps_free_or_clocked_alignment() {
    for (capture_mode, playback_mode, active, expected_alignment) in [
        (
            CaptureMode::Free,
            PlaybackMode::SelectOne { active: None },
            None,
            false,
        ),
        (
            CaptureMode::Free,
            PlaybackMode::SelectOne { active: Some(0) },
            Some(0),
            false,
        ),
        (
            CaptureMode::Free,
            PlaybackMode::SelectOne { active: Some(1) },
            Some(1),
            false,
        ),
        (CaptureMode::Free, PlaybackMode::Sum, None, false),
        (
            CaptureMode::Clocked {
                bars: 4,
                count_in: 0,
            },
            PlaybackMode::Sum,
            None,
            true,
        ),
        (
            CaptureMode::Clocked {
                bars: 4,
                count_in: 0,
            },
            PlaybackMode::SelectOne { active: Some(1) },
            Some(1),
            true,
        ),
    ] {
        let mut recorder = Recorder::default();
        let track_id = recorder.session.tracks[0].id;
        if let PlaybackMode::SelectOne {
            active: Some(active),
        } = playback_mode
        {
            if active == 0 {
                append_layer(&mut recorder, 0, &[0.7, -0.7], 44_100, 0.25);
            }
        }
        set_mode(&mut recorder, 0, playback_mode);
        if matches!(playback_mode, PlaybackMode::SelectOne { .. }) {
            if let PlaybackMode::SelectOne {
                active: Some(active),
            } = playback_mode
            {
                if active == 1 {
                    append_layer(&mut recorder, 0, &[0.7, -0.7], 44_100, 0.25);
                }
            }
        }
        recorder.session.master_clock_enabled = matches!(capture_mode, CaptureMode::Clocked { .. });
        if let CaptureMode::Clocked { count_in, .. } = capture_mode {
            recorder.session.count_in_bars = count_in;
        }
        let mut audio = FakeAudio::default();
        service(&mut recorder, &mut audio);
        recorder.reconcile_all_playback();
        service(&mut recorder, &mut audio);
        recorder.arm(0);
        service(&mut recorder, &mut audio);
        recorder.toggle_record();
        service(&mut recorder, &mut audio);
        let token = audio.accepted_capture.expect("capture admitted");
        assert_eq!(audio.capture_modes, vec![capture_mode]);
        audio.completed = Some(CaptureCompletion {
            token,
            sample_rate: 48_000,
            samples: vec![0.11, -0.2],
        });
        service(&mut recorder, &mut audio);

        let expected_layers = match playback_mode {
            PlaybackMode::SelectOne { active: Some(0) } => 2,
            PlaybackMode::SelectOne { active: Some(1) } => 2,
            _ => 1,
        };
        assert_eq!(
            recorder.session.track(track_id).unwrap().layers.len(),
            expected_layers
        );
        let eligible = expected_audible(&recorder);
        assert_eq!(active_keys(&audio), eligible);
        if active == Some(1) || playback_mode == PlaybackMode::Sum {
            let captured = audio
                .plays
                .iter()
                .find(|p| {
                    p.key.track_id == track_id && p.key.layer_index == (expected_layers - 1) as u16
                })
                .expect("eligible appended capture plays");
            assert_eq!(captured.sample_rate, 48_000);
            assert_eq!(captured.samples, vec![0.11, -0.2]);
            assert_eq!(captured.aligned, expected_alignment);
        } else if active == Some(0) {
            assert_eq!(
                audio.plays.len(),
                1,
                "existing active variation remains playing"
            );
            assert_eq!(audio.plays[0].key.layer_index, 0);
            assert!(!audio.plays.iter().any(|p| p.key.layer_index == 1));
        } else {
            assert!(audio.plays.is_empty());
        }
        assert_eq!(recorder.store.len(), expected_layers);
    }
}

#[test]
fn imported_replay_respects_selection_and_uses_stored_rate_and_gain() {
    let mut recorder = Recorder::default();
    let track = recorder.session.tracks[0].id;
    append_layer(&mut recorder, 0, &[0.1, 0.2], 44_100, 0.3);
    set_mode(
        &mut recorder,
        0,
        PlaybackMode::SelectOne { active: Some(0) },
    );
    let mut audio = FakeAudio::default();
    service(&mut recorder, &mut audio);
    recorder.reconcile_all_playback();
    service(&mut recorder, &mut audio);
    assert_eq!(audio.active.len(), 1);
    assert_eq!(audio.active.keys().next().unwrap().layer_index, 0);
    let initial_play_count = audio.plays.len();
    let imported = recorder
        .append_imported_audio(track, &[0.8, -0.4], 22_050, 3)
        .unwrap();
    service(&mut recorder, &mut audio);
    assert_eq!(active_keys(&audio), expected_audible(&recorder));
    assert_eq!(recorder.accepted_voice_count(), 1);
    assert_eq!(audio.plays.len(), initial_play_count);
    assert!(!audio.plays.iter().any(|play| play.key.layer_index == 1));
    assert_eq!(audio.plays[0].sample_rate, 44_100);
    assert_eq!(audio.plays[0].gain, 0.3);
    let phrase = &recorder.session.phrases[&imported];
    let stored = recorder.store.get(&phrase.media).unwrap();
    assert_eq!(stored.sample_rate, 22_050);
    assert_eq!(stored.samples, vec![0.8, -0.4]);
}

#[test]
fn one_cycle_export_pcm_matches_sum_and_selected_recorder_voices() {
    for mode in [
        PlaybackMode::Sum,
        PlaybackMode::SelectOne { active: Some(1) },
    ] {
        let mut recorder = Recorder::default();
        append_layer(&mut recorder, 0, &[0.1, -0.2, 0.3], 48_000, 0.5);
        append_layer(&mut recorder, 0, &[-0.4, 0.6, -0.8], 48_000, 0.25);
        set_mode(&mut recorder, 0, mode);
        let mut audio = FakeAudio::default();
        service(&mut recorder, &mut audio);
        recorder.reconcile_all_playback();
        service(&mut recorder, &mut audio);

        let exported = hocket_project::export::render_one_cycle(
            &recorder.session,
            &recorder.store,
            &BTreeSet::new(),
        )
        .unwrap();
        let mut admitted_mix = vec![0.0; 3];
        for voice in audio.active.values() {
            assert_eq!(voice.sample_rate, exported.sample_rate);
            for (sample, output) in voice.samples.iter().zip(&mut admitted_mix) {
                *output += sample * voice.gain;
            }
        }
        assert_eq!(admitted_mix, exported.samples);
        assert_eq!(active_keys(&audio), expected_audible(&recorder));
    }

    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1, -0.2, 0.3], 48_000, 0.5);
    set_mode(&mut recorder, 0, PlaybackMode::SelectOne { active: None });
    let mut audio = FakeAudio::default();
    service(&mut recorder, &mut audio);
    recorder.reconcile_all_playback();
    service(&mut recorder, &mut audio);
    assert!(audio.active.is_empty());
    assert!(matches!(
        hocket_project::export::render_one_cycle(
            &recorder.session,
            &recorder.store,
            &BTreeSet::new()
        ),
        Err(hocket_project::export::ExportError::NoAudibleLayers)
    ));
}

#[test]
fn mute_solo_and_reconcile_keep_active_selection_and_refuse_missing_or_muted_layers() {
    let mut recorder = Recorder::default();
    append_layer(&mut recorder, 0, &[0.1], 48_000, 1.0);
    append_layer(&mut recorder, 0, &[0.2], 48_000, 1.0);
    append_layer(&mut recorder, 1, &[0.3], 48_000, 1.0);
    set_mode(
        &mut recorder,
        0,
        PlaybackMode::SelectOne { active: Some(0) },
    );
    let active_track: TrackId = recorder.session.tracks[0].id;
    let muted_phrase = recorder.session.tracks[0].layers[1].phrase_id;
    recorder.session.tracks[0].layers[1].muted = true;
    let missing_phrase = recorder.session.tracks[1].layers[0].phrase_id;
    let missing_media = recorder.session.phrases[&missing_phrase].media;
    let retained = recorder.session.tracks[0]
        .layers
        .iter()
        .filter_map(|layer| {
            recorder
                .session
                .phrases
                .get(&layer.phrase_id)
                .and_then(|phrase| {
                    recorder
                        .store
                        .get(&phrase.media)
                        .map(|buffer| (buffer.samples.clone(), buffer.sample_rate))
                })
        })
        .collect::<Vec<_>>();
    recorder.store = InMemoryStore::new();
    for (samples, rate) in retained {
        recorder.store.put(&samples, rate);
    }
    recorder.missing_media.insert(missing_media);
    let mut audio = FakeAudio::default();
    service(&mut recorder, &mut audio);
    recorder.reconcile_all_playback();
    service(&mut recorder, &mut audio);
    let model_eligible = expected_audible(&recorder);
    let missing_key = LayerKey {
        track_id: recorder.session.tracks[1].id,
        layer_index: 0,
    };
    assert!(model_eligible.contains(&missing_key));
    assert!(!active_keys(&audio).contains(&missing_key));
    let mut media_available_eligible = model_eligible;
    media_available_eligible.remove(&missing_key);
    assert_eq!(active_keys(&audio), media_available_eligible);
    assert_eq!(audio.plays.len(), 1);
    assert_eq!(audio.plays[0].key.layer_index, 0);

    recorder.toggle_layer_mute(0, 1);
    service(&mut recorder, &mut audio);
    assert_eq!(
        audio.plays.len(),
        1,
        "unmuting an inactive variation cannot start it"
    );
    assert!(audio.stops.is_empty());
    assert_eq!(recorder.session.tracks[0].layers[1].phrase_id, muted_phrase);

    append_layer(&mut recorder, 0, &[0.5], 44_100, 0.4);
    set_mode(
        &mut recorder,
        0,
        PlaybackMode::SelectOne { active: Some(2) },
    );
    recorder.reconcile_all_playback();
    service(&mut recorder, &mut audio);
    let mut expected_after_append = expected_audible(&recorder);
    expected_after_append.remove(&missing_key);
    assert_eq!(active_keys(&audio), expected_after_append);
    assert_eq!(audio.plays.last().unwrap().sample_rate, 44_100);
    assert_eq!(audio.plays.last().unwrap().gain, 0.4);

    recorder.toggle_track_mute(0);
    service(&mut recorder, &mut audio);
    assert_eq!(recorder.accepted_voice_count(), 0);
    recorder.toggle_track_mute(0);
    recorder.toggle_solo(0);
    service(&mut recorder, &mut audio);
    assert_eq!(active_keys(&audio), expected_audible(&recorder));
    assert!(audio.plays.iter().all(|p| p.key.track_id == active_track));
}
