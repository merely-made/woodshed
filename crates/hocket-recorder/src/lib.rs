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
mod tests;

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
    /// Local pack draft saved by the project owner. Added PCM uses normal history.
    pub audio_pack: Option<audio_packs::PackSelection>,
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
            audio_pack: None,
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
        self.audio_pack = None;
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

    pub fn project_bundle(&self) -> hocket_model::ProjectBundle {
        let mut bundle =
            hocket_model::ProjectBundle::new(self.session.clone(), self.history.clone());
        bundle.audio_pack = self.audio_pack.clone();
        bundle
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

mod actions;

mod capture;

mod playback;
