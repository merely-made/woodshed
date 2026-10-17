// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Standalone application services around the reusable Hocket recorder.
//! Recorder gestures, capture custody, history edits and waveforms live in
//! hocket-recorder; this wrapper supplies native audio, project actors, identity,
//! clipboard, updater, appearance and window policy.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use crate::recorder_audio::DesktopRecorderAudio;
use armillary::{ActorHandle, Correlated, RequestId, RequestIds};
use audio_primitives::MeterBallistics;
use cambium::SelectState;
use genet_clipboard::{Clipboard, ClipboardItem, SystemClipboard, TextClipboard};
use hocket_engine::{AudioDeviceSelection, AudioDevices, available_audio_devices};
use hocket_model::{History, MediaRef, ProjectBundle, Session};
use hocket_project::export::ExportLength;
use hocket_project::handoff::{HandoffEnvelope, ReceivedHandoff};
use hocket_project::media::{InMemoryStore, MediaStore};
use hocket_recorder::Recorder;
use personae::{Ed25519PublicKey, IdentityProvider};

use crate::identity::{LocalIdentity, parse_contact_token};
use crate::project_io::{ProjectCommand, ProjectReply, ProjectRequest, ProjectScope, ProjectUpdate};

enum ProjectStatus {
    Idle,
    Saving,
    Loading,
    Exporting,
    WritingHandoff,
    ReadingHandoff,
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

struct PendingProject {
    request: RequestId,
    scope: ProjectScope,
    saved_revision: Option<ProjectRevision>,
}

/// A save acknowledges the whole retained document, including inactive branches.
/// The store is append-only and content-addressed: its count also detects new
/// buffers not yet referenced by a history head, without cloning PCM per frame.
struct ProjectRevision {
    bundle: ProjectBundle,
    media_count: usize,
    missing_media: BTreeSet<MediaRef>,
}

impl ProjectRevision {
    fn current(recorder: &Recorder) -> Self {
        Self {
            bundle: ProjectBundle::new(recorder.session.clone(), recorder.history.clone()),
            media_count: recorder.store.len(),
            missing_media: recorder.missing_media.clone(),
        }
    }

    fn matches(&self, recorder: &Recorder) -> bool {
        self.bundle.session == recorder.session
            && self.bundle.history == recorder.history
            && self.media_count == recorder.store.len()
            && self.missing_media == recorder.missing_media
    }
}

#[derive(Clone, Copy)]
enum Preservation {
    Close,
    Open,
    Accept,
    Restart,
}

pub struct AppState {
    pub appearance: crate::appearance::AppearanceState,
    pub window_commands: cambium_genet_winit_host::WindowCommands,
    pub recorder: Recorder,
    audio: DesktopRecorderAudio,
    last_meter_tick: Instant,
    project_path: Option<PathBuf>,
    saved_revision: ProjectRevision,
    preservation: Option<Preservation>,
    loaded_project: Option<(PathBuf, hocket_project::project_store::LoadedProject)>,
    continue_after_save: bool,
    close_approval: Option<ProjectRevision>,
    project_status: ProjectStatus,
    project_requests: RequestIds,
    project_generation: u64,
    pending_project: Option<PendingProject>,
    project_worker: ActorHandle<ProjectRequest>,
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
}

impl AppState {
    /// A new, empty looper-pedal session. Captures append real playable layers.
    pub fn new(
        project_worker: ActorHandle<ProjectRequest>,
        update_worker: ActorHandle<crate::update::worker::UpdateCommand>,
        update_settings: crate::update::UpdateSettings,
        identity: Result<LocalIdentity, String>,
        receipt: bool,
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
            receipt,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_project_parts(
        session: Session,
        history: History,
        store: InMemoryStore,
        missing_media: BTreeSet<MediaRef>,
        project_worker: ActorHandle<ProjectRequest>,
        update_worker: ActorHandle<crate::update::worker::UpdateCommand>,
        update_settings: crate::update::UpdateSettings,
        identity: Result<LocalIdentity, String>,
        receipt: bool,
    ) -> Self {
        let audio_devices = if receipt {
            AudioDevices::default()
        } else {
            available_audio_devices()
        };
        let audio = DesktopRecorderAudio::new(receipt);
        let audio_status = if receipt {
            "appearance receipt · audio inactive"
        } else {
            "system audio"
        }
        .to_string();
        let recorder = Recorder::new(session, history, store, missing_media);
        let saved_revision = ProjectRevision::current(&recorder);
        let mut state = Self {
            appearance: crate::appearance::AppearanceState::default(),
            window_commands: cambium_genet_winit_host::WindowCommands::new(),
            recorder,
            audio,
            last_meter_tick: Instant::now(),
            project_path: None,
            saved_revision,
            preservation: None,
            loaded_project: None,
            continue_after_save: false,
            close_approval: None,
            project_status: ProjectStatus::Idle,
            project_requests: RequestIds::default(),
            project_generation: 0,
            pending_project: None,
            project_worker,
            update_status: crate::update::UpdateStatus::Idle,
            update_settings,
            update_worker,
            identity,
            // Native project receipts exercise the real token controls while
            // keeping audio devices disabled. Appearance-only receipts retain
            // their clipboard isolation unless explicitly opted in.
            clipboard: if receipt
                && std::env::var("HOCKET_RECEIPT_CLIPBOARD").as_deref() != Ok("1")
            {
                None
            } else {
                SystemClipboard::new().ok()
            },
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
        };
        state.set_meter_ballistics(MeterBallistics::default());
        // Apply the initial click + tempo to the engine.
        state.resync_tempo();
        state
    }

    pub fn protect_appearance_exports(&mut self) {
        let mut paths = vec![crate::update::settings_path()];
        paths.extend(self.project_path.clone());
        self.appearance.protect_media(paths);
        self.appearance
            .workshop
            .set_protected_export_directories(crate::identity::protected_data_directories());
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
            ProjectStatus::WritingHandoff => "writing hand-off".to_string(),
            ProjectStatus::ReadingHandoff => "opening hand-off".to_string(),
            ProjectStatus::Saved => {
                if self.is_dirty() {
                    "unsaved changes".to_string()
                } else {
                    "saved".to_string()
                }
            },
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
            },
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
        self.identity
            .as_ref()
            .ok()
            .map(LocalIdentity::contact_token)
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
                },
            },
            None => self.project_status = ProjectStatus::Error("clipboard unavailable".to_string()),
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
        let mix = match hocket_project::export::render_mix(
            &self.recorder.session,
            &self.recorder.store,
            &self.recorder.solo,
            self.export_length,
        ) {
            Ok(mix) => mix,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("copy audio: {error}"));
                return;
            },
        };
        let bytes = match hocket_project::export::encode_stereo_wav_bytes(&mix) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("copy audio: {error}"));
                return;
            },
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
                },
            },
            None => self.project_status = ProjectStatus::Error("clipboard unavailable".to_string()),
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
                },
            },
            None => {
                self.project_status = ProjectStatus::Error("clipboard unavailable".to_string());
                return;
            },
        };
        let mix = match hocket_project::export::decode_wav_bytes(&bytes) {
            Ok(mix) => mix,
            Err(error) => {
                self.project_status = ProjectStatus::Error(format!("paste audio: {error}"));
                return;
            },
        };
        if mix.samples.is_empty() {
            self.project_status = ProjectStatus::Error("pasted audio is empty".to_string());
            return;
        }
        let track_id = self.recorder.session.tracks[track_idx].id;
        if let Err(error) = self.recorder.append_imported_audio(
            track_id,
            &mix.samples,
            mix.sample_rate,
            Self::now_ms(),
        ) {
            self.project_status = ProjectStatus::Error(format!("paste audio: {error}"));
            return;
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
                },
            },
            None => {
                self.project_status = ProjectStatus::Error("clipboard unavailable".to_string());
                return;
            },
        };
        match parse_contact_token(&text) {
            Ok(key) => {
                let fingerprint = key_fingerprint(&key);
                self.recipient = Some(key);
                self.project_status = ProjectStatus::RecipientSet(fingerprint);
            },
            Err(reason) => {
                self.project_status = ProjectStatus::Error(format!("recipient: {reason}"));
            },
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
            },
        };
        let bundle =
            ProjectBundle::new(self.recorder.session.clone(), self.recorder.history.clone());
        let envelope =
            match HandoffEnvelope::create(&bundle, &self.recorder.store, recipient, identity) {
                Ok(envelope) => envelope,
                Err(error) => {
                    self.project_status = ProjectStatus::Error(format!("hand off: {error}"));
                    return;
                },
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
        self.submit_project(ProjectCommand::WriteHandoff { path, envelope });
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
        match identity.join_family() {
            Ok(()) => {
                self.retire_project_work();
                self.reshare_notice = true;
                // The addressed reply belonged to the old identity's
                // conversation; keeping it would send the next hand-off as
                // somebody the peer no longer recognises.
                self.recipient = None;
            },
            Err(error) => {
                eprintln!("[hocket] could not join the family persona: {error}");
            },
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
            let continues = session.id == self.recorder.session.id;
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
            },
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Hocket hand-off", &["hocket"])
            .pick_file()
        else {
            return;
        };
        self.submit_project(ProjectCommand::ReadHandoff { path, recipient });
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
        if self.incoming.is_none() || self.preservation.is_some() {
            return;
        }
        self.preservation = Some(Preservation::Accept);
        self.retry_preservation();
    }

    fn adopt_incoming(&mut self) {
        // Keep the original staged payload on failure so acceptance is retryable.
        if let Some(received) = self.incoming.as_ref() {
            if let Err(error) = received.bundle.validate() {
                self.project_status = ProjectStatus::Error(format!("accept: {error}"));
                return;
            }
        }
        let Some(received) = self.incoming.clone() else {
            return;
        };
        let sender = received.sender;
        if received.bundle.session.id == self.recorder.session.id {
            let mut bundle =
                ProjectBundle::new(self.recorder.session.clone(), self.recorder.history.clone());
            let mut media = self.recorder.store.clone();
            match received.accept_branch(&mut bundle, &mut media) {
                Ok(_report) => {
                    self.stop_all();
                    self.retire_project_work();
                    self.recorder.replace_project(
                        bundle.session,
                        bundle.history,
                        media,
                        self.recorder.missing_media.clone(),
                    );
                    self.recipient = Some(sender);
                    self.project_status = ProjectStatus::HandoffAccepted;
                    self.resync_tempo();
                    self.reconcile_all_playback();
                },
                Err(error) => {
                    self.project_status = ProjectStatus::Error(format!("accept: {error}"));
                },
            }
        } else {
            // A new session from a peer: adopt it wholesale, like opening a
            // project, but with no file yet so it reads as unsaved.
            self.stop_all();
            self.retire_project_work();
            self.recorder.replace_project(
                received.bundle.session,
                received.bundle.history,
                received.media,
                BTreeSet::new(),
            );
            self.project_path = None;
            self.recipient = Some(sender);
            self.project_status = ProjectStatus::HandoffAccepted;
            self.resync_tempo();
            self.reconcile_all_playback();
        }
    }

    fn project_scope(&self) -> ProjectScope {
        ProjectScope {
            generation: self.project_generation,
            session: self.recorder.session.id,
        }
    }

    fn retire_project_work(&mut self) {
        self.project_generation = self.project_generation.wrapping_add(1);
        self.pending_project = None;
        self.incoming = None;
        self.project_status = ProjectStatus::Idle;
    }

    pub fn is_project_io_active(&self) -> bool {
        self.pending_project
            .as_ref()
            .is_some_and(|pending| pending.scope == self.project_scope())
    }

    fn submit_project(&mut self, command: ProjectCommand) -> bool {
        if self.is_project_io_active() {
            return false;
        }
        let request = self.project_requests.issue();
        let scope = self.project_scope();
        self.project_status = match &command {
            ProjectCommand::Save { .. } => ProjectStatus::Saving,
            ProjectCommand::Open { .. } => ProjectStatus::Loading,
            ProjectCommand::ExportMix { .. } => ProjectStatus::Exporting,
            ProjectCommand::WriteHandoff { .. } => ProjectStatus::WritingHandoff,
            ProjectCommand::ReadHandoff { .. } => ProjectStatus::ReadingHandoff,
        };
        let saved_revision = match &command {
            ProjectCommand::Save { bundle, media, .. } => Some(ProjectRevision {
                bundle: bundle.clone(),
                media_count: media.len(),
                missing_media: self.recorder.missing_media.clone(),
            }),
            _ => None,
        };
        self.pending_project = Some(PendingProject { request, scope, saved_revision });
        if !self.project_worker.command(ProjectRequest {
            scope,
            command: Correlated::new(request, command),
        }) {
            self.pending_project = None;
            self.project_status = ProjectStatus::Error("project worker stopped".into());
            return false;
        }
        true
    }

    pub fn choose_project_to_open(&mut self) {
        if self.loaded_project.is_some() {
            if self.preservation.is_none() {
                self.preservation = Some(Preservation::Open);
                self.retry_preservation();
            }
            return;
        }
        if self.preservation.is_some() {
            return;
        }
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
            self.submit_project(ProjectCommand::Open { path });
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
        self.save_project_at(path);
    }

    /// Shared dialog outcome seam: cancellation never grants preservation.
    fn save_project_at(&mut self, path: Option<PathBuf>) {
        self.continue_after_save = false;
        if self.is_recording() || self.is_project_io_active() {
            return;
        }
        if let Some(path) = path {
            let bundle =
                ProjectBundle::new(self.recorder.session.clone(), self.recorder.history.clone());
            let admitted = self.submit_project(ProjectCommand::Save {
                path,
                bundle,
                media: self.recorder.store.clone(),
                saved_head: self.recorder.history.head,
            });
            self.continue_after_save = admitted && self.preservation.is_some();
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
            self.submit_project(ProjectCommand::ExportMix {
                path,
                session: self.recorder.session.clone(),
                media: self.recorder.store.clone(),
                solo: self.recorder.solo.clone(),
                length: self.export_length,
            });
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
        self.export_length = ExportLength::Bars(self.recorder.session.bars_per_phrase.max(1));
    }

    pub fn adjust_export_bars(&mut self, delta: i8) {
        let bars = self
            .export_bars()
            .unwrap_or_else(|| self.recorder.session.bars_per_phrase.max(1));
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
            UpdateStatus::ReadyToRestart { .. } => {
                if self.preservation.is_none() {
                    self.preservation = Some(Preservation::Restart);
                    self.retry_preservation();
                }
                None
            },
            // A failure is worth one more try; the reason stays visible until
            // the retry reports something new.
            UpdateStatus::Failed { .. } | UpdateStatus::Idle | UpdateStatus::UpToDate { .. } => {
                Some(UpdateCommand::Check {
                    settings: self.update_settings,
                    user_asked: true,
                })
            },
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

    pub fn apply_project_update(&mut self, reply: ProjectReply) {
        let Some(pending) = self.pending_project.as_ref() else {
            return;
        };
        if pending.request != reply.update.request || pending.scope != reply.scope {
            return;
        }
        // A matching old terminal reply may retire its own pending slot, but
        // never changes the replacement session or releases a newer request.
        let pending = self.pending_project.take().expect("matched pending request");
        if reply.scope != self.project_scope() {
            return;
        }
        match reply.update.value {
            ProjectUpdate::Saved { path, saved_head } => {
                let Some(revision) = pending.saved_revision else {
                    return;
                };
                if revision.bundle.history.head != saved_head {
                    self.continue_after_save = false;
                    self.project_status = ProjectStatus::Error("save revision mismatch".into());
                    return;
                }
                self.project_path = Some(path);
                self.saved_revision = revision;
                self.project_status = ProjectStatus::Saved;
                if std::mem::take(&mut self.continue_after_save) {
                    self.retry_preservation();
                }
            },
            ProjectUpdate::Opened { path, loaded } => {
                self.loaded_project = Some((path, loaded));
                if self.preservation.is_none() {
                    self.preservation = Some(Preservation::Open);
                    // Completion admission checks the live capture and full
                    // revision, even if dispatch originally observed a clean host.
                    self.retry_preservation();
                }
            },
            ProjectUpdate::Exported { path } => {
                self.project_status = ProjectStatus::Exported(path);
            },
            ProjectUpdate::HandoffWritten { path } => {
                self.project_status = ProjectStatus::HandedOff(path);
            },
            ProjectUpdate::HandoffReceived { received } => {
                // Stage the authenticated hand-off for review. It does not touch
                // the live session until the musician accepts it.
                self.project_status =
                    ProjectStatus::HandoffReceived(key_fingerprint(&received.sender));
                self.incoming = Some(received);
            },
            ProjectUpdate::Failed { action, message } => {
                self.continue_after_save = false;
                self.project_status = ProjectStatus::Error(format!("{action}: {message}"));
            },
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        !self.saved_revision.matches(&self.recorder)
    }

    pub fn preservation_pending(&self) -> bool {
        self.preservation.is_some()
    }

    pub fn has_loaded_project(&self) -> bool {
        self.loaded_project.is_some()
    }

    pub fn preservation_message(&self) -> String {
        let action = match self.preservation {
            Some(Preservation::Close) => "closing Hocket",
            Some(Preservation::Open) => "opening the loaded project",
            Some(Preservation::Accept) => "accepting the hand-off",
            Some(Preservation::Restart) => "restarting Hocket",
            None => return String::new(),
        };
        let message = if self.is_recording() {
            format!("Stop capture before {action}, then Retry. Your live capture is retained.")
        } else if self.is_project_io_active() {
            format!("Wait for project I/O before {action}, then Retry.")
        } else if self.is_dirty() {
            format!("Save your project before {action}, discard its unsaved changes, or cancel.")
        } else {
            format!("Retry to continue {action}, or cancel.")
        };
        if let ProjectStatus::Error(error) = &self.project_status {
            format!("{message} Last operation failed: {error}")
        } else {
            message
        }
    }

    /// Appearance permission is handled by the caller; it never grants project
    /// permission. A discard approval is valid only for the exact revision.
    pub fn request_project_close(&mut self) -> bool {
        if !self.is_recording() && !self.is_project_io_active()
            && self.close_approval.as_ref().is_some_and(|r| r.matches(&self.recorder))
        {
            return true;
        }
        self.close_approval = None;
        if self.preservation.is_none() {
            self.preservation = Some(Preservation::Close);
        }
        if matches!(self.preservation, Some(Preservation::Close | Preservation::Restart)) {
            self.retry_preservation();
        }
        self.close_approval.is_some()
    }

    pub fn project_close_ready(&self) -> bool {
        self.close_approval.is_some()
    }

    pub fn wait_for_appearance_close(&mut self) {
        // Do not keep pumping an approved project close while the appearance
        // confirmation is open: its Cancel must remain effective.
        self.close_approval = None;
    }

    pub fn cancel_preservation(&mut self) {
        self.preservation = None;
        self.continue_after_save = false;
        self.close_approval = None;
        // Loaded Open and hand-off payloads deliberately survive cancellation.
    }

    pub fn retry_loaded_project(&mut self) {
        if self.loaded_project.is_some() && self.preservation.is_none() {
            self.preservation = Some(Preservation::Open);
            self.retry_preservation();
        }
    }

    pub fn discard_loaded_project(&mut self) {
        if self.preservation.is_none() {
            self.loaded_project = None;
        }
    }

    pub fn retry_preservation(&mut self) {
        self.resolve_preservation(false);
    }

    pub fn discard_for_preservation(&mut self) {
        self.resolve_preservation(true);
    }

    fn resolve_preservation(&mut self, discard: bool) {
        // A busy capture owns a usable stop token. Neither discard nor an Open
        // reply may retire it, and admitted worker work must also finish first.
        if self.is_recording() || self.is_project_io_active() || (!discard && self.is_dirty()) {
            return;
        }
        let Some(intent) = self.preservation.take() else { return; };
        self.continue_after_save = false;
        match intent {
            Preservation::Close => {
                self.close_approval = Some(ProjectRevision::current(&self.recorder));
            },
            Preservation::Accept => self.adopt_incoming(),
            Preservation::Open => {
                if let Some((_, loaded)) = self.loaded_project.as_ref() {
                    if let Err(error) = loaded.bundle.validate() {
                        self.preservation = Some(Preservation::Open);
                        self.project_status = ProjectStatus::Error(format!("open: {error}"));
                        return;
                    }
                }
                let Some((path, loaded)) = self.loaded_project.take() else { return; };
                self.stop_all();
                self.retire_project_work();
                self.recorder.replace_project(
                    loaded.bundle.session, loaded.bundle.history, loaded.media, loaded.missing_media,
                );
                self.project_path = Some(path);
                self.saved_revision = ProjectRevision::current(&self.recorder);
                self.project_status = ProjectStatus::Saved;
                self.resync_tempo();
                self.reconcile_all_playback();
            },
            Preservation::Restart => {
                // Re-enter the ordinary close path so an appearance draft must
                // resolve too. Dispatching the updater is done by that guard.
                self.preservation = Some(Preservation::Restart);
                self.close_approval = Some(ProjectRevision::current(&self.recorder));
            },
        }
    }

    pub fn apply_restart_after_close_guard(&mut self) -> bool {
        if self.is_recording() || self.is_project_io_active()
            || !self.close_approval.as_ref().is_some_and(|r| r.matches(&self.recorder))
        {
            return false;
        }
        if matches!(self.preservation, Some(Preservation::Restart)) {
            self.preservation = None;
            self.close_approval = None;
            self.update_worker.command(crate::update::worker::UpdateCommand::ApplyAndRestart);
            return true;
        }
        false
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn input_device_options(&self) -> Vec<String> {
        audio_device_options(&self.audio_devices.inputs)
    }

    pub fn output_device_options(&self) -> Vec<String> {
        audio_device_options(&self.audio_devices.outputs)
    }

    pub fn audio_status_label(&self) -> &str {
        self.recorder.audio_status_label()
    }

    /// Service the host adapter and device-free recorder controller.
    pub fn tick(&mut self) {
        let now = Instant::now();
        let delta = now
            .duration_since(self.last_meter_tick)
            .as_secs_f32()
            .clamp(0.0, 0.25);
        self.last_meter_tick = now;
        self.apply_audio_device_selection();
        self.recorder
            .service(&mut self.audio, Self::now_ms(), delta);
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
            self.recorder.report_audio_status(self.audio_status.clone());
            return;
        }

        let selection = AudioDeviceSelection {
            input_id: selected_device_id(&self.audio_devices.inputs, input),
            output_id: selected_device_id(&self.audio_devices.outputs, output),
        };
        self.stop_all();
        match self.audio.restart(&selection) {
            Ok(()) => {
                self.applied_audio_input = input;
                self.applied_audio_output = output;
                self.audio_status = "audio switched".into();
                self.recorder.audio_restarted();
            },
            Err(error) => {
                self.audio_input_select.selected = 0;
                self.audio_output_select.selected = 0;
                self.applied_audio_input = 0;
                self.applied_audio_output = 0;
                match self.audio.restart(&AudioDeviceSelection::default()) {
                    Ok(()) => {
                        self.audio_status =
                            format!("audio switch failed: {error}; restored default");
                        self.recorder.audio_restarted();
                    },
                    Err(fallback) => {
                        self.audio_status = format!("audio unavailable: {error}; {fallback}")
                    },
                }
            },
        }
        self.recorder.report_audio_status(self.audio_status.clone());
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
}

impl std::ops::Deref for AppState {
    type Target = Recorder;
    fn deref(&self) -> &Recorder {
        &self.recorder
    }
}
impl std::ops::DerefMut for AppState {
    fn deref_mut(&mut self) -> &mut Recorder {
        &mut self.recorder
    }
}

#[cfg(test)]
#[path = "project_correlation_tests.rs"]
mod project_correlation_tests;

#[cfg(test)]
#[path = "native_lifecycle_tests.rs"]
mod native_lifecycle_tests;
