// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The app state behind the genet UI: the real `hocket_model::Session` +
//! `History`, plus the audio `hocket_engine::Engine` and its content store.
//!
//! Every data-bearing gesture commits a real `Edit`, so undo/redo + the future
//! sync layer see exactly what the UI did; the engine-driving methods mirror the
//! host's (`record` / `arm` / `stop_all` / `play_layer_from_store` / …)
//! so audio behaviour remains independent of the UI framework. The `Engine` is
//! `!Send`; winit runs the app on the main thread, so it lives directly here and
//! is advanced by [`tick`](AppState::tick) from the host's ~60fps timer.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use armillary::ActorHandle;
use audio_primitives::{MeterBallistics, PeakMeterSmoother, WaveformPeak};
use hocket_engine::export::ExportLength;
use hocket_engine::media::{InMemoryStore, MediaStore};
use hocket_engine::{
    AudioDeviceSelection, AudioDevices, CapturePhase, Engine, LayerKey, available_audio_devices,
};
use hocket_model::{
    Edit, History, Layer, MediaRef, Phrase, ProjectBundle, Session, Track, TrackColor, TrackId,
};
use cambium::SelectState;
use genet_clipboard::{Clipboard, ClipboardItem, SystemClipboard, TextClipboard};
use hocket_engine::handoff::{HandoffEnvelope, ReceivedHandoff};
use personae::{Ed25519PublicKey, IdentityProvider};

use crate::identity::{LocalIdentity, parse_contact_token};
use crate::project_io::{ProjectCommand, ProjectUpdate};

/// Meter dB floor for the 0..1 level the output meters display.
const METER_FLOOR_DB: f32 = -60.0;

fn normalize_meter_db(db: f32) -> f32 {
    if !db.is_finite() || db <= METER_FLOOR_DB {
        0.0
    } else {
        ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
    }
}

enum ProjectStatus {
    Idle,
    Saving,
    Loading,
    Exporting,
    Saved,
    Exported(PathBuf),
    HandedOff(PathBuf),
    HandoffReceived(String),
    TokenCopied,
    RecipientSet(String),
    HandoffAccepted,
    AudioCopied,
    AudioPasted,
    Error(String),
}

/// A persona switch waiting for confirmation, and what it costs.
#[derive(Clone, Debug)]
pub struct PersonaSwitch {
    /// The family persona this would become.
    pub profile: String,
    /// The contact token that stops naming this application once it does.
    /// Shown so the confirmation is about a concrete thing the user has
    /// already given people, not an abstraction.
    pub losing_token: String,
}

pub struct AppState {
    pub session: Session,
    pub history: History,

    // === audio ===
    engine: Result<Engine, String>,
    sample_rate: u32,
    store: InMemoryStore,
    capture_phase: CapturePhase,
    /// The track the in-flight capture targets (snapshot at Record time, so
    /// re-arming mid-capture does not redirect it).
    capturing_track: Option<usize>,
    /// Engine layer keys currently looping, so stop-all / mute can stop them.
    playing: Vec<LayerKey>,
    /// Latest output peak, dB per channel, read back each tick.
    meter_db: [f32; 2],
    /// Host-local display smoothing; timing is UI policy, not session state.
    meter_display: [PeakMeterSmoother; 2],
    last_meter_tick: Instant,
    /// Blobs referenced by an opened project but unavailable locally. Their
    /// layers remain in the model and stay silent until the blobs arrive.
    pub missing_media: BTreeSet<MediaRef>,
    project_path: Option<PathBuf>,
    saved_head: hocket_model::NodeId,
    project_status: ProjectStatus,
    project_worker: ActorHandle<ProjectCommand>,
    /// What the app is actually doing about updates. Host-local presentation
    /// state: it never enters a project or a hand-off.
    update_status: crate::update::UpdateStatus,
    /// The user's update policy, channel and cadence.
    update_settings: crate::update::UpdateSettings,
    /// The update actor, so the status chip can act rather than only report.
    update_worker: ActorHandle<crate::update::worker::UpdateCommand>,
    /// Durable host identity. Its secret and unlock state never enter a project.
    identity: Result<LocalIdentity, String>,
    /// The OS clipboard, through genet's shared service. `None` on a headless
    /// host with no clipboard. Host-local: it carries tokens and, later, audio,
    /// never project state.
    clipboard: Option<SystemClipboard>,
    /// The peer this session will be handed to, set by pasting their contact
    /// token or by accepting an incoming hand-off (which addresses the reply
    /// back to its sender). Host-local; a recipient is not project state.
    recipient: Option<Ed25519PublicKey>,
    /// A persona switch staged for confirmation, and what it will cost.
    ///
    /// Staged rather than applied on the click, for the same reason an
    /// incoming hand-off is: joining the family persona changes the contact
    /// token peers already hold, and that is not something to discover after
    /// the fact.
    persona_switch: Option<PersonaSwitch>,
    /// Said once after a switch: the token has changed and needs re-sharing.
    /// Cleared when the user copies it, because copying is the act the notice
    /// is asking for.
    reshare_notice: bool,
    /// A received, authenticated hand-off staged for review. It is not applied
    /// to the live session until the musician accepts it; discarding drops it.
    incoming: Option<ReceivedHandoff>,
    /// Host-local export intent. It changes only the rendered file, never the
    /// project graph or its syncable history.
    export_length: ExportLength,
    /// Per-launch device catalog and selections. Device IDs belong to the host,
    /// not to a project that might travel to another machine.
    audio_devices: AudioDevices,
    pub(crate) audio_input_select: SelectState,
    pub(crate) audio_output_select: SelectState,
    applied_audio_input: usize,
    applied_audio_output: usize,
    audio_status: String,

    // === app-local UI (graduation notes) ===
    /// Audible metronome. Drives the engine click directly; distinct from the
    /// master clock (which governs bar-aligned capture + count-in).
    pub click: bool,
    /// Solo set. App-local: the model has no solo yet (a mix-bus concern).
    pub solo: BTreeSet<TrackId>,
}

impl AppState {
    /// A new, empty looper-pedal session. Captures append real playable layers.
    pub fn new(
        project_worker: ActorHandle<ProjectCommand>,
        update_worker: ActorHandle<crate::update::worker::UpdateCommand>,
        update_settings: crate::update::UpdateSettings,
        identity: Result<LocalIdentity, String>,
    ) -> Self {
        Self::from_project_parts(
            Session::new_default(),
            History::new(),
            InMemoryStore::new(),
            BTreeSet::new(),
            project_worker,
            update_worker,
            update_settings,
            identity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_project_parts(
        session: Session,
        history: History,
        store: InMemoryStore,
        missing_media: BTreeSet<MediaRef>,
        project_worker: ActorHandle<ProjectCommand>,
        update_worker: ActorHandle<crate::update::worker::UpdateCommand>,
        update_settings: crate::update::UpdateSettings,
        identity: Result<LocalIdentity, String>,
    ) -> Self {
        let audio_devices = available_audio_devices();
        let (engine, sample_rate, audio_status) = match Engine::new() {
            Ok(e) => {
                let sr = e.sample_rate();
                (Ok(e), sr, "system audio".to_string())
            }
            Err(e) => (Err(e.to_string()), 0, "audio unavailable".to_string()),
        };
        let saved_head = history.head;
        let mut state = Self {
            session,
            history,
            engine,
            sample_rate,
            store,
            capture_phase: CapturePhase::Idle,
            capturing_track: None,
            playing: Vec::new(),
            meter_db: [f32::NEG_INFINITY; 2],
            meter_display: [PeakMeterSmoother::default(), PeakMeterSmoother::default()],
            last_meter_tick: Instant::now(),
            missing_media,
            project_path: None,
            saved_head,
            project_status: ProjectStatus::Idle,
            project_worker,
            update_status: crate::update::UpdateStatus::Idle,
            update_settings,
            update_worker,
            identity,
            clipboard: SystemClipboard::new().ok(),
            recipient: None,
            persona_switch: None,
            reshare_notice: false,
            incoming: None,
            export_length: ExportLength::OneCycle,
            audio_devices,
            audio_input_select: SelectState::default(),
            audio_output_select: SelectState::default(),
            applied_audio_input: 0,
            applied_audio_output: 0,
            audio_status,
            click: true,
            solo: BTreeSet::new(),
        };
        state.set_meter_ballistics(MeterBallistics::default());
        // Apply the initial click + tempo to the engine.
        state.resync_tempo();
        state
    }

    pub fn project_label(&self) -> String {
        self.project_path
            .as_deref()
            .and_then(|path| path.file_stem())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".to_string())
    }

    pub fn project_status_label(&self) -> String {
        match &self.project_status {
            ProjectStatus::Saving => "saving".to_string(),
            ProjectStatus::Loading => "opening".to_string(),
            ProjectStatus::Exporting => "exporting mix".to_string(),
            ProjectStatus::Saved => {
                if self.is_dirty() {
                    "unsaved changes".to_string()
                } else {
                    "saved".to_string()
                }
            }
            ProjectStatus::Exported(path) => format!(
                "exported {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            ProjectStatus::HandedOff(path) => format!(
                "handed off {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            ProjectStatus::HandoffReceived(sender) => format!("hand-off received from {sender}"),
            ProjectStatus::TokenCopied => "contact token copied".to_string(),
            ProjectStatus::RecipientSet(fingerprint) => format!("handing to {fingerprint}"),
            ProjectStatus::HandoffAccepted => "hand-off accepted".to_string(),
            ProjectStatus::AudioCopied => "loop audio copied".to_string(),
            ProjectStatus::AudioPasted => "audio pasted as a layer".to_string(),
            ProjectStatus::Error(message) => format!("project error: {message}"),
            ProjectStatus::Idle => {
                if self.is_dirty() {
                    "unsaved changes".to_string()
                } else {
                    "new session".to_string()
                }
            }
        }
    }

    pub fn identity_status_label(&self) -> String {
        match &self.identity {
            Ok(identity) => format!("local session · {}", identity.fingerprint()),
            Err(_) => "local session · identity unavailable".to_string(),
        }
    }

    /// Where this identity lives: shared with the rest of the Merely family, or
    /// Hocket's own, and why.
    ///
    /// Separate from [`Self::identity_status_label`], which answers "which key
    /// is this" with a fingerprint. This answers "is it the same person the
    /// other applications know", which is the question a musician handing off a
    /// session actually has.
    pub fn identity_home_label(&self) -> String {
        match &self.identity {
            Ok(identity) => identity.home().summary(),
            Err(error) => format!("Identity unavailable: {error}"),
        }
    }

    /// The local identity's contact token (full public key as hex), if the
    /// identity is available. A peer needs this to address a hand-off here.
    pub fn contact_token(&self) -> Option<String> {
        self.identity.as_ref().ok().map(LocalIdentity::contact_token)
    }

    /// Copy the contact token to the OS clipboard through genet's shared
    /// service, reporting the outcome honestly (including an unavailable
    /// clipboard or identity).
    pub fn copy_contact_token(&mut self) {
        // Copying is what the re-share notice is asking for, so doing it
        // answers the notice rather than leaving it up to nag.
        self.reshare_notice = false;
        let Some(token) = self.contact_token() else {
            self.project_status = ProjectStatus::Error("identity unavailable".to_string());
            return;
        };
        match &mut self.clipboard {
            Some(clipboard) => match clipboard.set_text(&token) {
                Ok(()) => self.project_status = ProjectStatus::TokenCopied,
                Err(error) => {
                    self.project_status = ProjectStatus::Error(format!("copy token: {error}"))
                }
            },
            None => {
                self.project_status = ProjectStatus::Error("clipboard unavailable".to_string())
            }
        }
    }

    /// Render the current loop mix and place it on the clipboard as `audio/wav`
    /// (plus a plain-text label), so it can be pasted into another Hocket or a
    /// MIME-aware app. Universal DAW paste wants a platform-native audio format,
    /// a later refinement; this is the multi-format clipboard's audio lane.
    pub fn copy_audio(&mut self) {
        if self.is_recording() {
            self.project_status =
                ProjectStatus::Error("stop recording before copying audio".to_string());
            return;
        }
        let mix = match hocket_engine::export::render_mix(
            &self.session,
            &self.store,
            &self.solo,
            self.export_length,
        ) {
            Ok(mix) => mix,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("copy audio: {error}"));
                return;
            }
        };
        let bytes = match hocket_engine::export::encode_stereo_wav_bytes(&mix) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("copy audio: {error}"));
                return;
            }
        };
        let label = format!("{} loop", self.project_label());
        let item = ClipboardItem::new()
            .with_custom("audio/wav", bytes)
            .with_text(label);
        match &mut self.clipboard {
            Some(clipboard) => match clipboard.write(&item) {
                Ok(()) => self.project_status = ProjectStatus::AudioCopied,
                Err(error) => {
                    self.project_status = ProjectStatus::Error(format!("copy audio: {error}"))
                }
            },
            None => {
                self.project_status = ProjectStatus::Error("clipboard unavailable".to_string())
            }
        }
    }

    /// Read `audio/wav` from the clipboard, decode it, and append it as a new
    /// layer on the armed track (the paste counterpart of a capture).
    pub fn paste_audio(&mut self) {
        if self.is_recording() {
            self.project_status =
                ProjectStatus::Error("stop recording before pasting audio".to_string());
            return;
        }
        let Some(track_idx) = self.armed_index() else {
            self.project_status =
                ProjectStatus::Error("arm a track to paste audio into".to_string());
            return;
        };
        let bytes = match &mut self.clipboard {
            Some(clipboard) => match clipboard.read_format("audio/wav") {
                Ok(bytes) => bytes,
                Err(_) => {
                    self.project_status =
                        ProjectStatus::Error("no audio on the clipboard".to_string());
                    return;
                }
            },
            None => {
                self.project_status = ProjectStatus::Error("clipboard unavailable".to_string());
                return;
            }
        };
        let mix = match hocket_engine::export::decode_wav_bytes(&bytes) {
            Ok(mix) => mix,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("paste audio: {error}"));
                return;
            }
        };
        if mix.samples.is_empty() {
            self.project_status = ProjectStatus::Error("pasted audio is empty".to_string());
            return;
        }
        let media = self.store.put(&mix.samples, mix.sample_rate);
        let phrase = Phrase::new(
            media,
            self.session.bars_per_phrase,
            self.session.bpm,
            Self::now_ms(),
        );
        let layer = Layer::new(phrase.id);
        let track_id = self.session.tracks[track_idx].id;
        let layer_index = self.session.tracks[track_idx].layers.len() as u16;
        self.history.commit(
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            },
            &mut self.session,
            Self::now_ms(),
        );
        if self.track_is_audible(track_idx) {
            self.play_layer_from_store(track_idx, layer_index as usize);
        }
        self.project_status = ProjectStatus::AudioPasted;
    }

    /// The current recipient's short fingerprint for display, if one is set.
    pub fn recipient_fingerprint(&self) -> Option<String> {
        self.recipient.as_ref().map(key_fingerprint)
    }

    /// Whether a hand-off can be sent now: a recipient and a usable identity.
    pub fn can_hand_off(&self) -> bool {
        self.recipient.is_some() && self.identity.is_ok()
    }

    /// Read a contact token from the clipboard and set it as the hand-off
    /// recipient. The pasted token is the peer's public key; parsing it is the
    /// only validation the address needs.
    pub fn paste_recipient(&mut self) {
        let text = match &mut self.clipboard {
            Some(clipboard) => match clipboard.get_text() {
                Ok(text) => text,
                Err(error) => {
                    self.project_status = ProjectStatus::Error(format!("paste: {error}"));
                    return;
                }
            },
            None => {
                self.project_status = ProjectStatus::Error("clipboard unavailable".to_string());
                return;
            }
        };
        match parse_contact_token(&text) {
            Ok(key) => {
                let fingerprint = key_fingerprint(&key);
                self.recipient = Some(key);
                self.project_status = ProjectStatus::RecipientSet(fingerprint);
            }
            Err(reason) => {
                self.project_status = ProjectStatus::Error(format!("recipient: {reason}"));
            }
        }
    }

    /// Build a signed hand-off for the current recipient and write it to a
    /// `.hocket` file the user picks. The envelope carries a complete snapshot;
    /// the file is a cleartext transfer artifact, not an encrypted channel, so
    /// it should travel over a carrier the peers already trust.
    pub fn hand_off(&mut self) {
        if self.is_project_io_active() {
            return;
        }
        if self.is_recording() {
            self.project_status =
                ProjectStatus::Error("stop recording before handing off".to_string());
            return;
        }
        let Some(recipient) = self.recipient else {
            self.project_status = ProjectStatus::Error("paste a recipient first".to_string());
            return;
        };
        let identity = match self.identity.as_ref() {
            Ok(identity) => identity,
            Err(_) => {
                self.project_status = ProjectStatus::Error("identity unavailable".to_string());
                return;
            }
        };
        let bundle = ProjectBundle::new(self.session.clone(), self.history.clone());
        let envelope = match HandoffEnvelope::create(&bundle, &self.store, recipient, identity) {
            Ok(envelope) => envelope,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("hand off: {error}"));
                return;
            }
        };
        let file_name = format!("{}.hocket", self.project_label());
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Hocket hand-off", &["hocket"])
            .set_file_name(file_name)
            .save_file()
            .map(|path| ensure_extension(path, "hocket"))
        else {
            return;
        };
        if !self
            .project_worker
            .command(ProjectCommand::WriteHandoff { path, envelope })
        {
            self.project_status = ProjectStatus::Error("project worker stopped".to_string());
        }
    }

    /// Offer to join the family persona, staged for confirmation.
    ///
    /// Reads the cost off the identity rather than describing it in the
    /// abstract: the token that stops working is the one on screen.
    pub fn offer_persona_switch(&mut self) {
        let Ok(identity) = self.identity.as_ref() else {
            return;
        };
        let Some(profile) = identity.family_to_join() else {
            return;
        };
        self.persona_switch = Some(PersonaSwitch {
            profile: profile.to_string(),
            losing_token: identity.contact_token(),
        });
    }

    /// The family persona this identity could join, when there is one.
    pub fn family_to_join(&self) -> Option<String> {
        self.identity
            .as_ref()
            .ok()
            .and_then(|identity| identity.family_to_join().map(str::to_string))
    }

    /// The staged switch, for the confirmation card.
    pub fn persona_switch(&self) -> Option<&PersonaSwitch> {
        self.persona_switch.as_ref()
    }

    /// Back out. Nothing has changed, which is the point of staging.
    pub fn decline_persona_switch(&mut self) {
        self.persona_switch = None;
    }

    /// Do it: speak as the family persona from now on.
    ///
    /// The re-share notice goes up on success, because the switch has just
    /// invalidated the token every peer holds and saying so is the only thing
    /// that makes the change recoverable for them.
    pub fn confirm_persona_switch(&mut self) {
        let Some(_staged) = self.persona_switch.take() else {
            return;
        };
        let Ok(identity) = self.identity.as_mut() else {
            return;
        };
        match identity.join_family(&personae::bootstrap::default_vault_dir()) {
            Ok(()) => {
                self.reshare_notice = true;
                // The addressed reply belonged to the old identity's
                // conversation; keeping it would send the next hand-off as
                // somebody the peer no longer recognises.
                self.recipient = None;
            }
            Err(error) => {
                eprintln!("[hocket] could not join the family persona: {error}");
            }
        }
    }

    /// Whether the token needs re-sharing after a switch.
    pub fn needs_reshare(&self) -> bool {
        self.reshare_notice
    }

    /// Whether a hand-off is staged for review.
    pub fn has_incoming(&self) -> bool {
        self.incoming.is_some()
    }

    /// The staged hand-off's sender fingerprint, for the review card.
    pub fn incoming_sender_fingerprint(&self) -> Option<String> {
        self.incoming
            .as_ref()
            .map(|received| key_fingerprint(&received.sender))
    }

    /// A one-line summary of the staged hand-off: whether it continues this
    /// session or opens a new one, and its track, layer, and media counts.
    pub fn incoming_summary(&self) -> Option<String> {
        self.incoming.as_ref().map(|received| {
            let session = &received.bundle.session;
            let layers: usize = session.tracks.iter().map(|track| track.layers.len()).sum();
            let continues = session.id == self.session.id;
            format!(
                "{} \u{00b7} {} tracks \u{00b7} {} layers \u{00b7} {} media",
                if continues {
                    "continues this session"
                } else {
                    "a new session"
                },
                session.tracks.len(),
                layers,
                received.media.len()
            )
        })
    }

    /// Open a `.hocket` hand-off addressed to this identity. Reading and
    /// authenticating it run off the kernel thread; the result stages for
    /// review via [`apply_project_update`](Self::apply_project_update).
    pub fn choose_handoff_to_open(&mut self) {
        if self.is_project_io_active() {
            return;
        }
        let recipient = match self.identity.as_ref() {
            Ok(identity) => identity.master_public_key(),
            Err(_) => {
                self.project_status = ProjectStatus::Error("identity unavailable".to_string());
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Hocket hand-off", &["hocket"])
            .pick_file()
        else {
            return;
        };
        if !self
            .project_worker
            .command(ProjectCommand::ReadHandoff { path, recipient })
        {
            self.project_status = ProjectStatus::Error("project worker stopped".to_string());
        }
    }

    /// Discard the staged hand-off without applying it.
    pub fn discard_incoming(&mut self) {
        self.incoming = None;
    }

    /// Accept the staged hand-off into the live session. When it continues this
    /// session, its branch is integrated and its head checked out, preserving
    /// the local branch; when it is a new session, it is adopted wholesale. The
    /// sender becomes the reply recipient either way.
    pub fn accept_incoming(&mut self) {
        let Some(received) = self.incoming.take() else {
            return;
        };
        let sender = received.sender;
        if received.bundle.session.id == self.session.id {
            let mut bundle = ProjectBundle::new(self.session.clone(), self.history.clone());
            let mut media = self.store.clone();
            match received.accept_branch(&mut bundle, &mut media) {
                Ok(_report) => {
                    self.stop_all();
                    self.session = bundle.session;
                    self.history = bundle.history;
                    self.store = media;
                    self.recipient = Some(sender);
                    self.project_status = ProjectStatus::HandoffAccepted;
                    self.resync_tempo();
                    self.reconcile_all_playback();
                }
                Err(error) => {
                    self.project_status = ProjectStatus::Error(format!("accept: {error}"));
                }
            }
        } else {
            // A new session from a peer: adopt it wholesale, like opening a
            // project, but with no file yet so it reads as unsaved.
            self.stop_all();
            self.session = received.bundle.session;
            self.history = received.bundle.history;
            self.store = received.media;
            self.missing_media.clear();
            self.capture_phase = CapturePhase::Idle;
            self.capturing_track = None;
            self.solo.clear();
            self.project_path = None;
            self.recipient = Some(sender);
            self.project_status = ProjectStatus::HandoffAccepted;
            self.resync_tempo();
            self.reconcile_all_playback();
        }
    }

    pub fn is_project_io_active(&self) -> bool {
        matches!(
            self.project_status,
            ProjectStatus::Saving | ProjectStatus::Loading | ProjectStatus::Exporting
        )
    }

    pub fn choose_project_to_open(&mut self) {
        if self.is_project_io_active() {
            return;
        }
        if self.is_recording() {
            self.project_status = ProjectStatus::Error("stop recording before opening".to_string());
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Hocket project", &["hock"])
            .pick_file()
        {
            self.project_status = ProjectStatus::Loading;
            if !self.project_worker.command(ProjectCommand::Open { path }) {
                self.project_status = ProjectStatus::Error("project worker stopped".to_string());
            }
        }
    }

    pub fn choose_project_to_save(&mut self) {
        if self.is_project_io_active() {
            return;
        }
        if self.is_recording() {
            self.project_status = ProjectStatus::Error("stop recording before saving".to_string());
            return;
        }
        let path = match self.project_path.clone() {
            Some(path) => Some(path),
            None => rfd::FileDialog::new()
                .add_filter("Hocket project", &["hock"])
                .set_file_name("untitled.hock")
                .save_file()
                .map(|path| ensure_extension(path, "hock")),
        };
        if let Some(path) = path {
            let bundle = ProjectBundle::new(self.session.clone(), self.history.clone());
            self.project_status = ProjectStatus::Saving;
            if !self.project_worker.command(ProjectCommand::Save {
                path,
                bundle,
                media: self.store.clone(),
                saved_head: self.history.head,
            }) {
                self.project_status = ProjectStatus::Error("project worker stopped".to_string());
            }
        }
    }

    pub fn choose_mix_export(&mut self) {
        if self.is_project_io_active() {
            return;
        }
        if self.is_recording() {
            self.project_status =
                ProjectStatus::Error("stop recording before exporting".to_string());
            return;
        }
        let file_name = format!("{}-mix.wav", self.project_label());
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name(file_name)
            .save_file()
            .map(|path| ensure_extension(path, "wav"))
        {
            self.project_status = ProjectStatus::Exporting;
            if !self.project_worker.command(ProjectCommand::ExportMix {
                path,
                session: self.session.clone(),
                media: self.store.clone(),
                solo: self.solo.clone(),
                length: self.export_length,
            }) {
                self.project_status = ProjectStatus::Error("project worker stopped".to_string());
            }
        }
    }

    pub fn export_uses_bars(&self) -> bool {
        matches!(self.export_length, ExportLength::Bars(_))
    }

    pub fn export_bars(&self) -> Option<u8> {
        match self.export_length {
            ExportLength::Bars(bars) => Some(bars),
            ExportLength::OneCycle => None,
        }
    }

    pub fn export_one_cycle(&mut self) {
        self.export_length = ExportLength::OneCycle;
    }

    pub fn export_session_bars(&mut self) {
        self.export_length = ExportLength::Bars(self.session.bars_per_phrase.max(1));
    }

    pub fn adjust_export_bars(&mut self, delta: i8) {
        let bars = self
            .export_bars()
            .unwrap_or_else(|| self.session.bars_per_phrase.max(1));
        self.export_length = ExportLength::Bars(if delta.is_negative() {
            bars.saturating_sub(delta.unsigned_abs()).max(1)
        } else {
            bars.saturating_add(delta as u8)
        });
    }

    /// The current update status, for the view.
    pub fn update_status(&self) -> &crate::update::UpdateStatus {
        &self.update_status
    }

    /// Apply a status the update worker reported.
    pub fn apply_update_status(&mut self, status: crate::update::UpdateStatus) {
        self.update_status = status;
    }

    /// Do whatever the current update status invites, if anything.
    ///
    /// One entry point rather than three buttons: the chip already says what
    /// state the app is in, so the click means "carry on from here" — fetch
    /// the offered version, restart into the staged one, or re-check after a
    /// failure. Statuses with nothing to act on return without sending a
    /// command, so a stray click cannot invent work.
    pub fn advance_update(&mut self) {
        use crate::update::UpdateStatus;
        use crate::update::worker::UpdateCommand;

        let command = match &self.update_status {
            UpdateStatus::Available { version } => Some(UpdateCommand::Download {
                version: version.clone(),
                settings: self.update_settings,
            }),
            UpdateStatus::ReadyToRestart { .. } => Some(UpdateCommand::ApplyAndRestart),
            // A failure is worth one more try; the reason stays visible until
            // the retry reports something new.
            UpdateStatus::Failed { .. } | UpdateStatus::Idle | UpdateStatus::UpToDate { .. } => {
                Some(UpdateCommand::Check {
                    settings: self.update_settings,
                    user_asked: true,
                })
            }
            // In flight, off, or impossible here: nothing to advance.
            UpdateStatus::Checking
            | UpdateStatus::Downloading { .. }
            | UpdateStatus::Disabled
            | UpdateStatus::Unsupported(_) => None,
        };
        if let Some(command) = command {
            self.update_worker.command(command);
        }
    }

    /// What clicking the update chip would do, for its label and aria text.
    /// `None` when the chip is not actionable.
    pub fn update_action_label(&self) -> Option<&'static str> {
        self.update_status.action_label()
    }

    pub fn apply_project_update(&mut self, update: ProjectUpdate) {
        match update {
            ProjectUpdate::Saved { path, saved_head } => {
                self.project_path = Some(path);
                self.saved_head = saved_head;
                self.project_status = ProjectStatus::Saved;
            }
            ProjectUpdate::Opened { path, loaded } => {
                self.stop_all();
                self.session = loaded.bundle.session;
                self.history = loaded.bundle.history;
                self.store = loaded.media;
                self.missing_media = loaded.missing_media;
                self.capture_phase = CapturePhase::Idle;
                self.capturing_track = None;
                self.solo.clear();
                self.project_path = Some(path);
                self.saved_head = self.history.head;
                self.project_status = ProjectStatus::Saved;
                self.resync_tempo();
                self.reconcile_all_playback();
            }
            ProjectUpdate::Exported { path } => {
                self.project_status = ProjectStatus::Exported(path);
            }
            ProjectUpdate::HandoffWritten { path } => {
                self.project_status = ProjectStatus::HandedOff(path);
            }
            ProjectUpdate::HandoffReceived { received } => {
                // Stage the authenticated hand-off for review. It does not touch
                // the live session until the musician accepts it.
                self.project_status = ProjectStatus::HandoffReceived(key_fingerprint(&received.sender));
                self.incoming = Some(received);
            }
            ProjectUpdate::Failed { action, message } => {
                self.project_status = ProjectStatus::Error(format!("{action}: {message}"));
            }
        }
    }

    fn is_dirty(&self) -> bool {
        self.history.head != self.saved_head
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    // === derived state the view reads ===

    pub fn armed_index(&self) -> Option<usize> {
        self.session.tracks.iter().position(|t| t.armed)
    }

    /// Whether a capture is in progress (drives the record light + rail).
    pub fn is_recording(&self) -> bool {
        matches!(
            self.capture_phase,
            CapturePhase::Waiting { .. }
                | CapturePhase::Recording { .. }
                | CapturePhase::FreeRecording { .. }
        )
    }

    /// The output meter level `0..1` for channel `ch` (0 = L, 1 = R), mapped
    /// from the read-back dB against the display floor.
    pub fn meter_level(&self, ch: usize) -> f32 {
        self.meter_display
            .get(ch)
            .map(|meter| meter.reading().level)
            .unwrap_or(0.0)
    }

    pub fn meter_peak_level(&self, ch: usize) -> f32 {
        self.meter_display
            .get(ch)
            .map(|meter| meter.reading().peak)
            .unwrap_or(0.0)
    }

    /// Latest output level in dB, for the textual peak readout.
    pub fn meter_db(&self, ch: usize) -> f32 {
        self.meter_db.get(ch).copied().unwrap_or(f32::NEG_INFINITY)
    }

    /// Host-local meter timing. A settings surface can replace this without
    /// mutating project or collaboration state.
    pub fn set_meter_ballistics(&mut self, ballistics: MeterBallistics) {
        for meter in &mut self.meter_display {
            meter.set_ballistics(ballistics);
        }
    }

    pub(crate) fn render_track_waveform(
        &self,
        track_index: usize,
        columns: usize,
    ) -> Option<Vec<WaveformPeak>> {
        hocket_engine::waveform::render_track_peaks(
            &self.session,
            &self.store,
            track_index,
            columns,
        )
        .ok()
    }

    pub(crate) fn render_layer_waveform(
        &self,
        track_index: usize,
        layer_index: usize,
        columns: usize,
    ) -> Option<Vec<WaveformPeak>> {
        hocket_engine::waveform::render_layer_peaks(
            &self.session,
            &self.store,
            track_index,
            layer_index,
            columns,
        )
        .ok()
    }

    pub fn layer_waveform_available(&self, track_index: usize, layer_index: usize) -> bool {
        self.session
            .tracks
            .get(track_index)
            .and_then(|track| track.layers.get(layer_index))
            .and_then(|layer| self.session.phrases.get(&layer.phrase_id))
            .and_then(|phrase| self.store.get(&phrase.media))
            .is_some_and(|buffer| !buffer.samples.is_empty())
    }

    pub fn track_has_audible_layers(&self, track_index: usize) -> bool {
        self.session.tracks.get(track_index).is_some_and(|track| {
            track.layers.iter().enumerate().any(|(index, layer)| {
                track
                    .playback_mode
                    .is_layer_audible(index as u16, layer.muted)
            })
        })
    }

    pub fn track_waveform_available(&self, track_index: usize) -> bool {
        let Some(track) = self.session.tracks.get(track_index) else {
            return false;
        };
        let mut sample_rate = None;
        let mut found = false;
        for (index, layer) in track.layers.iter().enumerate() {
            if !track
                .playback_mode
                .is_layer_audible(index as u16, layer.muted)
            {
                continue;
            }
            let Some(buffer) = self
                .session
                .phrases
                .get(&layer.phrase_id)
                .and_then(|phrase| self.store.get(&phrase.media))
            else {
                return false;
            };
            if buffer.samples.is_empty() {
                return false;
            }
            match sample_rate {
                Some(expected) if expected != buffer.sample_rate => return false,
                None => sample_rate = Some(buffer.sample_rate),
                _ => {}
            }
            found = true;
        }
        found
    }

    pub fn input_device_options(&self) -> Vec<String> {
        audio_device_options(&self.audio_devices.inputs)
    }

    pub fn output_device_options(&self) -> Vec<String> {
        audio_device_options(&self.audio_devices.outputs)
    }

    pub fn audio_status_label(&self) -> &str {
        &self.audio_status
    }

    // === the engine tick (host calls ~60fps via runner.update) ===

    /// Advance the engine, read back the meter + capture phase, and promote a
    /// completed capture into a model layer + looping playback.
    pub fn tick(&mut self) {
        let now = Instant::now();
        let meter_delta = now
            .duration_since(self.last_meter_tick)
            .as_secs_f32()
            .clamp(0.0, 0.25);
        self.last_meter_tick = now;
        self.apply_audio_device_selection();
        let (meter, phase, captured) = match &mut self.engine {
            Ok(engine) => {
                let _ = engine.tick();
                (
                    engine.peak_db(),
                    engine.pending_capture_progress(),
                    engine.take_bar_aligned_capture(),
                )
            }
            Err(_) => ([f32::NEG_INFINITY; 2], self.capture_phase.clone(), None),
        };
        self.meter_db = meter;
        for (channel, smoother) in self.meter_display.iter_mut().enumerate() {
            smoother.update(normalize_meter_db(meter[channel]), meter_delta);
        }
        self.capture_phase = phase;

        if let Some(samples) = captured {
            let Some(track_idx) = self.capturing_track.take() else {
                return;
            };
            // A free capture stopped instantly yields an empty buffer — skip.
            if samples.is_empty() || track_idx >= self.session.tracks.len() {
                return;
            }
            let media = self.store.put(&samples, self.sample_rate);
            let phrase = Phrase::new(
                media,
                self.session.bars_per_phrase,
                self.session.bpm,
                Self::now_ms(),
            );
            let layer = Layer::new(phrase.id);
            let track_id = self.session.tracks[track_idx].id;
            let layer_index = self.session.tracks[track_idx].layers.len() as u16;
            self.history.commit(
                Edit::AppendLayer {
                    track_id,
                    phrase,
                    layer,
                },
                &mut self.session,
                Self::now_ms(),
            );
            if self.track_is_audible(track_idx) {
                let key = LayerKey::new(track_id, layer_index);
                let clocked = self.session.master_clock_enabled;
                if let Ok(engine) = &mut self.engine {
                    let _ = if clocked {
                        engine.play_layer_at_next_bar(key, samples, 1.0, true)
                    } else {
                        engine.play_layer(key, samples, 1.0, true)
                    };
                }
                self.playing.push(key);
            }
        }
    }

    // === gestures (commit a real Edit, then drive the engine) ===

    /// Arm track `idx` (single-arm: unarm the previous holder). Stops a live
    /// capture first so the flag never points at an un-armed track.
    pub fn arm(&mut self, idx: usize) {
        if self.session.tracks.get(idx).is_none_or(|t| t.armed) {
            return;
        }
        if self.is_recording() {
            self.stop_capture();
        }
        let now = Self::now_ms();
        if let Some(prev) = self.armed_index() {
            let track_id = self.session.tracks[prev].id;
            self.history.commit(
                Edit::ArmTrack {
                    track_id,
                    from: true,
                    to: false,
                },
                &mut self.session,
                now,
            );
        }
        let track_id = self.session.tracks[idx].id;
        self.history.commit(
            Edit::ArmTrack {
                track_id,
                from: false,
                to: true,
            },
            &mut self.session,
            now,
        );
    }

    /// Add an empty track using the session's selected playback profile.
    pub fn add_track(&mut self) {
        let index = self.session.tracks.len();
        let track = Track::new_with_mode(
            format!("track {}", index + 1),
            TrackColor::from_palette_index(index),
            self.session.default_playback_mode,
        );
        self.history
            .commit(Edit::AddTrack { track }, &mut self.session, Self::now_ms());
    }

    /// The Record gesture. Master clock on → a bar-aligned, count-in, fixed
    /// capture. Master clock off → toggle a free/variable-length capture.
    pub fn toggle_record(&mut self) {
        let Some(armed) = self.armed_index() else {
            return;
        };
        if self.session.master_clock_enabled {
            let count_in = self.session.count_in_bars;
            if let Ok(engine) = &mut self.engine {
                if engine
                    .arm_bar_aligned_capture(self.session.bars_per_phrase, count_in)
                    .is_ok()
                {
                    self.capturing_track = Some(armed);
                }
            }
        } else if matches!(self.capture_phase, CapturePhase::FreeRecording { .. }) {
            self.stop_capture();
        } else if let Ok(engine) = &mut self.engine {
            if engine.start_free_capture().is_ok() {
                self.capturing_track = Some(armed);
            }
        }
    }

    /// Stop an in-flight free capture (the tick picks up the Complete buffer).
    fn stop_capture(&mut self) {
        if let Ok(engine) = &mut self.engine {
            engine.stop_free_capture();
        }
    }

    /// Toggle track-level mute (the lane's M): stop / replay the track's voices.
    pub fn toggle_track_mute(&mut self, idx: usize) {
        let Some(track) = self.session.tracks.get(idx) else {
            return;
        };
        let (track_id, from) = (track.id, track.muted);
        self.history.commit(
            Edit::MuteTrack {
                track_id,
                from,
                to: !from,
            },
            &mut self.session,
            Self::now_ms(),
        );
        if !from {
            // Now muted: stop every voice on this track.
            let keys: Vec<LayerKey> = self
                .playing
                .iter()
                .copied()
                .filter(|k| k.track_id == track_id)
                .collect();
            for key in keys {
                self.stop_layer_key(key);
            }
        } else if self.track_is_audible(idx) {
            self.reconcile_track_playback(idx);
        }
    }

    /// Toggle one layer's mute (tap a layer row): stop / replay that voice.
    pub fn toggle_layer_mute(&mut self, track_idx: usize, layer_index: u16) {
        let Some(track) = self.session.tracks.get(track_idx) else {
            return;
        };
        let track_id = track.id;
        let Some(layer) = track.layers.get(layer_index as usize) else {
            return;
        };
        let from = layer.muted;
        self.history.commit(
            Edit::SetLayerMute {
                track_id,
                layer_index,
                from,
                to: !from,
            },
            &mut self.session,
            Self::now_ms(),
        );
        let key = LayerKey::new(track_id, layer_index);
        if !from {
            self.stop_layer_key(key);
        } else if self.track_is_audible(track_idx) {
            self.play_layer_from_store(track_idx, layer_index as usize);
        }
    }

    /// Nudge tempo (clamped) and re-sync the engine grid.
    pub fn bpm_nudge(&mut self, delta: f32) {
        let from = self.session.bpm;
        let to = (from + delta).clamp(40.0, 240.0);
        if (to - from).abs() < f32::EPSILON {
            return;
        }
        self.history
            .commit(Edit::SetBpm { from, to }, &mut self.session, Self::now_ms());
        self.resync_tempo();
    }

    /// Toggle the master clock (bar-aligned capture + count-in). Model-only;
    /// the audible metronome is the separate [`toggle_click`](Self::toggle_click).
    pub fn toggle_master_clock(&mut self) {
        let from = self.session.master_clock_enabled;
        self.history.commit(
            Edit::SetMasterClock { from, to: !from },
            &mut self.session,
            Self::now_ms(),
        );
    }

    /// Toggle the audible metronome click on the engine.
    pub fn toggle_click(&mut self) {
        self.click = !self.click;
        if let Ok(engine) = &mut self.engine {
            engine.set_click_enabled(self.click);
        }
    }

    /// Toggle solo membership for track `idx` and reconcile the engine with the
    /// resulting audible-track set.
    pub fn toggle_solo(&mut self, idx: usize) {
        let Some(track) = self.session.tracks.get(idx) else {
            return;
        };
        let id = track.id;
        if !self.solo.remove(&id) {
            self.solo.insert(id);
        }
        self.reconcile_all_playback();
    }

    // === engine playback helpers ===

    /// Push tempo + full time signature to the click and re-apply the click enable.
    /// Stops all playing loops first — they were captured at the old grid and
    /// would drift.
    fn resync_tempo(&mut self) {
        self.stop_all();
        let bpm = self.session.bpm;
        let time_signature = self.session.time_signature;
        let click = self.click;
        if let Ok(engine) = &mut self.engine {
            let _ = engine.set_tempo(bpm, time_signature.numerator, time_signature.denominator);
            engine.set_click_enabled(click);
        }
    }

    fn apply_audio_device_selection(&mut self) {
        let input = self.audio_input_select.selected;
        let output = self.audio_output_select.selected;
        if input == self.applied_audio_input && output == self.applied_audio_output {
            return;
        }
        if self.is_recording() {
            self.audio_input_select.selected = self.applied_audio_input;
            self.audio_output_select.selected = self.applied_audio_output;
            self.audio_status = "stop capture before changing audio".to_string();
            return;
        }

        let selection = AudioDeviceSelection {
            input_id: selected_device_id(&self.audio_devices.inputs, input),
            output_id: selected_device_id(&self.audio_devices.outputs, output),
        };
        self.stop_all();
        let old_engine = std::mem::replace(&mut self.engine, Err("restarting audio".to_string()));
        drop(old_engine);

        match Engine::new_with_audio_devices(&selection) {
            Ok(engine) => {
                self.sample_rate = engine.sample_rate();
                self.engine = Ok(engine);
                self.applied_audio_input = input;
                self.applied_audio_output = output;
                self.audio_status = "audio switched".to_string();
                self.resync_tempo();
                self.reconcile_all_playback();
            }
            Err(error) => {
                self.audio_input_select.selected = self.applied_audio_input;
                self.audio_output_select.selected = self.applied_audio_output;
                match Engine::new() {
                    Ok(engine) => {
                        self.sample_rate = engine.sample_rate();
                        self.engine = Ok(engine);
                        self.audio_input_select.selected = 0;
                        self.audio_output_select.selected = 0;
                        self.applied_audio_input = 0;
                        self.applied_audio_output = 0;
                        self.audio_status =
                            format!("audio switch failed: {error}; restored default");
                        self.resync_tempo();
                        self.reconcile_all_playback();
                    }
                    Err(fallback_error) => {
                        self.engine = Err(format!(
                            "{error}; default audio also failed: {fallback_error}"
                        ));
                        self.audio_status = "audio unavailable".to_string();
                    }
                }
            }
        }
    }

    /// Stop every live loop. The session layers remain intact and can be
    /// projected back into the engine after a transport command or state change.
    pub fn stop_all(&mut self) {
        let keys: Vec<LayerKey> = self.playing.drain(..).collect();
        if let Ok(engine) = &mut self.engine {
            for key in keys {
                engine.stop_layer(key);
            }
        }
    }

    fn stop_layer_key(&mut self, key: LayerKey) {
        if let Ok(engine) = &mut self.engine {
            engine.stop_layer(key);
        }
        self.playing.retain(|k| *k != key);
    }

    /// (Re)play every audible layer of track `idx` from the store.
    fn reconcile_track_playback(&mut self, idx: usize) {
        let Some(track) = self.session.tracks.get(idx) else {
            return;
        };
        let audible: Vec<usize> = track
            .layers
            .iter()
            .enumerate()
            .filter(|(li, layer)| {
                track
                    .playback_mode
                    .is_layer_audible(*li as u16, layer.muted)
            })
            .map(|(li, _)| li)
            .collect();
        for li in audible {
            self.play_layer_from_store(idx, li);
        }
    }

    fn track_is_audible(&self, idx: usize) -> bool {
        let Some(track) = self.session.tracks.get(idx) else {
            return false;
        };
        !track.muted && (self.solo.is_empty() || self.solo.contains(&track.id))
    }

    fn reconcile_all_playback(&mut self) {
        self.stop_all();
        let audible: Vec<usize> = (0..self.session.tracks.len())
            .filter(|&idx| self.track_is_audible(idx))
            .collect();
        for idx in audible {
            self.reconcile_track_playback(idx);
        }
    }

    /// Play a layer from the store at the next bar, at its stored gain. No-op
    /// when the media is unavailable in the current store.
    fn play_layer_from_store(&mut self, track_idx: usize, layer_idx: usize) {
        let Some(track) = self.session.tracks.get(track_idx) else {
            return;
        };
        let Some(layer) = track.layers.get(layer_idx) else {
            return;
        };
        let Some(phrase) = self.session.phrases.get(&layer.phrase_id) else {
            return;
        };
        let Some(buf) = self.store.get(&phrase.media) else {
            return;
        };
        let samples = buf.samples.clone();
        let sample_rate = buf.sample_rate;
        let (gain, key) = (layer.gain, LayerKey::new(track.id, layer_idx as u16));
        if let Ok(engine) = &mut self.engine {
            let _ =
                engine.play_layer_at_next_bar_at_sample_rate(key, samples, sample_rate, gain, true);
        }
        if !self.playing.contains(&key) {
            self.playing.push(key);
        }
    }
}

fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    path
}

/// Six-byte hex fingerprint of a public key, for compact display. Matches
/// `LocalIdentity::fingerprint`'s form so the same key reads the same anywhere.
fn key_fingerprint(key: &Ed25519PublicKey) -> String {
    key.to_bytes()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn audio_device_options(devices: &[hocket_engine::AudioDevice]) -> Vec<String> {
    std::iter::once("System default".to_string())
        .chain(devices.iter().map(|device| {
            if device.is_default {
                format!("{} (default)", device.name)
            } else {
                device.name.clone()
            }
        }))
        .collect()
}

fn selected_device_id(devices: &[hocket_engine::AudioDevice], selected: usize) -> Option<String> {
    devices
        .get(selected.checked_sub(1)?)
        .map(|device| device.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_options_reserve_zero_for_the_system_default() {
        let devices = vec![hocket_engine::AudioDevice {
            id: "input-1".to_string(),
            name: "Studio input".to_string(),
            is_default: true,
        }];
        assert_eq!(
            audio_device_options(&devices),
            vec!["System default", "Studio input (default)"]
        );
        assert_eq!(selected_device_id(&devices, 0), None);
        assert_eq!(selected_device_id(&devices, 1).as_deref(), Some("input-1"));
    }

    #[test]
    fn meter_db_normalization_has_an_explicit_floor() {
        assert_eq!(normalize_meter_db(f32::NEG_INFINITY), 0.0);
        assert_eq!(normalize_meter_db(-60.0), 0.0);
        assert!((normalize_meter_db(-30.0) - 0.5).abs() < f32::EPSILON);
        assert_eq!(normalize_meter_db(0.0), 1.0);
    }
}
