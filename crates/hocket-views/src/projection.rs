// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A read-only recorder display over the host's actual authority. These types
//! contain no session, history, scheduler, capture state machine or device.
//! Every exposed command carries the key captured when its control was built.

use std::borrow::Cow;

use audio_primitives::WaveformPeak;

pub struct RecorderProjection<'a, K> {
    pub title: Cow<'a, str>,
    pub hint: Cow<'a, str>,
    pub lanes: Vec<LaneProjection<'a, K>>,
    pub transport: TransportProjection<'a, K>,
    /// Omit unless the host has truthful meter observations.
    pub meters: Option<[MeterReading; 2]>,
}

pub struct LaneProjection<'a, K> {
    pub key: K,
    pub name: Cow<'a, str>,
    pub owner: Cow<'a, str>,
    /// The host supplies the exact lane/capture clock, including mixed clocks.
    pub clock: Cow<'a, str>,
    pub color: [u8; 3],
    pub selected: bool,
    pub recording: RecordingState,
    pub waveform: WaveformProjection<'a>,
    pub layers: Vec<LayerProjection<'a, K>>,
    pub controls: LaneControls<'a, K>,
}

pub struct LayerProjection<'a, K> {
    /// Include the occurrence when a phrase is referenced more than once.
    pub key: K,
    pub label: Cow<'a, str>,
    pub muted: bool,
    pub waveform: WaveformProjection<'a>,
    pub mute: Option<ToggleControl<'a, K>>,
}

pub struct LaneControls<'a, K> {
    pub select: Option<CommandControl<'a, K>>,
    pub record: Option<CommandControl<'a, K>>,
    pub clear: Option<CommandControl<'a, K>>,
    pub mute: Option<ToggleControl<'a, K>>,
    pub solo: Option<ToggleControl<'a, K>>,
}

impl<K> Default for LaneControls<'_, K> {
    fn default() -> Self {
        Self {
            select: None,
            record: None,
            clear: None,
            mute: None,
            solo: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordingState {
    #[default]
    Idle,
    Armed,
    Recording,
}

pub struct TransportProjection<'a, K> {
    /// Accepted source/epoch context. Transport actions never infer this from
    /// whichever row happens to be selected when the event is dispatched.
    pub context: K,
    pub playing: bool,
    pub recording: RecordingState,
    pub status: Cow<'a, str>,
    pub input_status: Cow<'a, str>,
    pub record_note: Cow<'a, str>,
    /// This target can be an accepted capture owner distinct from selection.
    /// Its enabled capability is independent of whether lazy input is open.
    pub record: Option<CommandControl<'a, K>>,
    pub stop_capture: Option<CommandControl<'a, K>>,
    pub play: Option<CommandControl<'a, K>>,
    pub stop: Option<CommandControl<'a, K>>,
    pub rewind: Option<CommandControl<'a, K>>,
    pub tempo: Option<TempoControl<'a, K>>,
    /// Host-owned heading for the timing summary. Standalone meter displays
    /// retain their default; hosts may summarize duration and click clocks too.
    pub meter_heading: Cow<'a, str>,
    pub meter_label: Option<Cow<'a, str>>,
    pub click: Option<ToggleControl<'a, K>>,
    pub replace: Option<ToggleControl<'a, K>>,
    pub master_clock: Option<ToggleControl<'a, K>>,
    pub add: Option<CommandControl<'a, K>>,
}

impl<'a, K> TransportProjection<'a, K> {
    pub fn new(context: K) -> Self {
        Self {
            context,
            playing: false,
            recording: RecordingState::Idle,
            status: "".into(),
            input_status: "".into(),
            record_note: "".into(),
            record: None,
            stop_capture: None,
            play: None,
            stop: None,
            rewind: None,
            tempo: None,
            meter_heading: "meter".into(),
            meter_label: None,
            click: None,
            replace: None,
            master_clock: None,
            add: None,
        }
    }
}

pub struct CommandControl<'a, K> {
    pub target: K,
    pub label: Cow<'a, str>,
    pub enabled: bool,
}

pub struct ToggleControl<'a, K> {
    pub target: K,
    pub label: Cow<'a, str>,
    pub enabled: bool,
    pub active: bool,
}

pub struct TempoControl<'a, K> {
    pub target: K,
    pub label: Cow<'a, str>,
    pub enabled: bool,
    pub bpm: f32,
    pub step: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct MeterReading {
    pub level: f32,
    pub peak: f32,
    pub db: f32,
}

pub enum WaveformProjection<'a> {
    Empty(Cow<'a, str>),
    Unavailable(Cow<'a, str>),
    Data {
        /// Must change when immutable material, gain or its projection changes.
        revision: u64,
        source: WaveformSource<'a>,
    },
}

pub enum WaveformSource<'a> {
    /// Borrow from the host's immutable accepted PCM snapshot, without copying.
    Pcm(&'a [f32]),
    Peaks(Cow<'a, [WaveformPeak]>),
    /// Existing recorder mixed-media projection, evaluated only when its
    /// signature changes. The callback runs in host UI/control reconciliation.
    Lazy(Box<dyn Fn(usize) -> Option<Vec<WaveformPeak>> + 'a>),
}
