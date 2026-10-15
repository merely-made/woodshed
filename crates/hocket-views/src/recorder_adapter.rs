// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Compatibility projection for the standalone portable Recorder. It shares
//! the host-projection renderer and leaves rather than owning another UI.

use crate::{
    CommandControl, LaneControls, LaneProjection, LayerProjection, MeterReading, ProjectedAction,
    RecorderProjection, RecordingState, TempoControl, ToggleControl, TransportProjection,
    WaveformProjection, WaveformSource,
};
use hocket_model::{PhraseId, PlaybackMode, SessionId, TrackId};
use hocket_recorder::{Recorder, RecorderAction};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HocketKey {
    Transport(SessionId),
    Track(TrackId),
    Layer {
        track: TrackId,
        layer_index: usize,
        phrase: PhraseId,
    },
}

pub fn recorder_projection(state: &Recorder) -> RecorderProjection<'_, HocketKey> {
    let mut lanes = Vec::with_capacity(state.session.tracks.len());
    let recording = match state.capture_phase() {
        hocket_recorder::CapturePhase::Recording { .. }
        | hocket_recorder::CapturePhase::FreeRecording { .. } => RecordingState::Recording,
        _ if state.is_recording() => RecordingState::Armed,
        _ => RecordingState::Idle,
    };
    for (index, track) in state.session.tracks.iter().enumerate() {
        let key = HocketKey::Track(track.id);
        let signature = track_sig(track, state);
        let waveform = if track.layers.is_empty() {
            WaveformProjection::Empty("no layers yet · arm and record to start the loop".into())
        } else if state.track_waveform_available(index) {
            WaveformProjection::Data {
                revision: signature,
                source: WaveformSource::Lazy(Box::new(move |columns| {
                    state.render_track_waveform(index, columns)
                })),
            }
        } else {
            WaveformProjection::Unavailable(
                if state.track_has_audible_layers(index) {
                    "media unavailable"
                } else {
                    "all layers muted"
                }
                .into(),
            )
        };
        let layers = track
            .layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let layer_key = HocketKey::Layer {
                    track: track.id,
                    layer_index,
                    phrase: layer.phrase_id,
                };
                let waveform = if state.layer_waveform_available(index, layer_index) {
                    let media = state
                        .session
                        .phrases
                        .get(&layer.phrase_id)
                        .map(|phrase| phrase.media.0)
                        .unwrap_or([0; 32]);
                    let revision =
                        crate::leaves::hash_ids(&[&media, &layer.gain.to_bits().to_le_bytes()]);
                    WaveformProjection::Data {
                        revision,
                        source: WaveformSource::Lazy(Box::new(move |columns| {
                            state.render_layer_waveform(index, layer_index, columns)
                        })),
                    }
                } else {
                    WaveformProjection::Unavailable("media unavailable".into())
                };
                LayerProjection {
                    key: layer_key,
                    label: format!("L{}", layer_index + 1).into(),
                    muted: layer.muted,
                    waveform,
                    mute: Some(ToggleControl {
                        target: layer_key,
                        label: format!("Mute layer L{}", layer_index + 1).into(),
                        enabled: u16::try_from(layer_index).is_ok(),
                        active: layer.muted,
                    }),
                }
            })
            .collect();
        lanes.push(LaneProjection {
            key,
            name: track.name.as_str().into(),
            owner: "you".into(),
            clock: if track.layers.is_empty() {
                "empty".into()
            } else {
                "".into()
            },
            color: [track.color.r, track.color.g, track.color.b],
            selected: track.armed,
            recording: if state.active_capture_track() == Some(track.id) {
                recording
            } else {
                RecordingState::Idle
            },
            waveform,
            layers,
            controls: LaneControls {
                select: Some(CommandControl {
                    target: key,
                    label: format!("Arm {}", track.name).into(),
                    enabled: true,
                }),
                mute: Some(ToggleControl {
                    target: key,
                    label: format!("Mute {}", track.name).into(),
                    enabled: true,
                    active: track.muted,
                }),
                solo: Some(ToggleControl {
                    target: key,
                    label: format!("Solo {}", track.name).into(),
                    enabled: true,
                    active: state.solo.contains(&track.id),
                }),
                ..Default::default()
            },
        });
    }
    let context = HocketKey::Transport(state.session.id);
    let mut transport = TransportProjection::new(context);
    transport.playing = state.accepted_voice_count() > 0;
    transport.recording = recording;
    transport.record = Some(CommandControl {
        target: state
            .active_capture_track()
            .map(HocketKey::Track)
            .unwrap_or_else(|| {
                state.armed_index().map_or(context, |index| {
                    HocketKey::Track(state.session.tracks[index].id)
                })
            }),
        label: "Record".into(),
        enabled: state.input_available(),
    });
    transport.record_note = if let Some(track) = state
        .active_capture_track()
        .and_then(|id| state.session.track(id))
    {
        format!(
            " · {} capture {}",
            track.name,
            if recording == RecordingState::Recording {
                "recording"
            } else {
                "armed"
            }
        )
        .into()
    } else {
        state
            .armed_index()
            .map_or(" · nothing armed".into(), |index| {
                format!(" · {} armed", state.session.tracks[index].name).into()
            })
    };
    transport.stop = Some(CommandControl {
        target: context,
        label: "Stop all loops".into(),
        enabled: true,
    });
    transport.tempo = Some(TempoControl {
        target: context,
        label: "tempo".into(),
        enabled: true,
        bpm: state.session.bpm,
        step: 2.0,
    });
    transport.meter_label = Some(
        format!(
            "{}/{}",
            state.session.time_signature.numerator, state.session.time_signature.denominator
        )
        .into(),
    );
    transport.click = Some(ToggleControl {
        target: context,
        label: "Click track".into(),
        enabled: true,
        active: state.click,
    });
    transport.master_clock = Some(ToggleControl {
        target: context,
        label: "Master clock".into(),
        enabled: true,
        active: state.session.master_clock_enabled,
    });
    transport.add = Some(CommandControl {
        target: context,
        label: "Add track".into(),
        enabled: true,
    });
    RecorderProjection {
        title: "loops".into(),
        hint: "tap a dot to arm · tap a layer to mute".into(),
        lanes,
        transport,
        meters: Some(std::array::from_fn(|channel| MeterReading {
            level: state.meter_level(channel),
            peak: state.meter_peak_level(channel),
            db: state.meter_db(channel),
        })),
    }
}

pub(crate) fn recorder_action(action: ProjectedAction<HocketKey>) -> Option<RecorderAction> {
    Some(match action {
        ProjectedAction::Select(HocketKey::Track(id)) => RecorderAction::Arm(id),
        ProjectedAction::Record(_) => RecorderAction::ToggleRecord,
        ProjectedAction::Stop(_) => RecorderAction::StopAll,
        ProjectedAction::NudgeTempo { delta, .. } => RecorderAction::NudgeBpm(delta),
        ProjectedAction::ToggleClick(_) => RecorderAction::ToggleClick,
        ProjectedAction::ToggleMasterClock(_) => RecorderAction::ToggleMasterClock,
        ProjectedAction::Add(_) => RecorderAction::AddTrack,
        ProjectedAction::Mute(HocketKey::Track(id)) => RecorderAction::ToggleTrackMute(id),
        ProjectedAction::Solo(HocketKey::Track(id)) => RecorderAction::ToggleSolo(id),
        ProjectedAction::LayerMute(HocketKey::Layer {
            track,
            layer_index,
            phrase,
        }) => RecorderAction::ToggleLayerMute {
            track,
            layer_index: u16::try_from(layer_index).ok()?,
            phrase,
        },
        _ => return None,
    })
}

pub(crate) fn track_sig(track: &hocket_model::Track, state: &Recorder) -> u64 {
    let mut signature = crate::leaves::hash_ids(&[track.id.0.as_bytes()]);
    signature = match track.playback_mode {
        PlaybackMode::Sum => signature ^ 0x51,
        PlaybackMode::SelectOne { active } => {
            signature ^ 0xa7 ^ u64::from(active.unwrap_or(u16::MAX))
        },
    };
    for layer in &track.layers {
        let media = state
            .session
            .phrases
            .get(&layer.phrase_id)
            .map(|phrase| phrase.media.0)
            .unwrap_or([0; 32]);
        signature = crate::leaves::hash_ids(&[
            &signature.to_le_bytes(),
            &media,
            &layer.gain.to_bits().to_le_bytes(),
        ]);
        if layer.muted {
            signature ^= 0x9e37_79b9_7f4a_7c15;
        }
    }
    signature
}
