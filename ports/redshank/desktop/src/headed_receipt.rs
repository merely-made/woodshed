//! Automated native-window receipts, driven through desktop commands.
//!
//! `identity-guard` uses synthetic note receipts against a real local decoder
//! and output runtime. It verifies projections and acknowledged/refused seeks;
//! it does not simulate pointer input, HTTP drift, or human visual inspection.
//! `alignment-guard` captures native source-time evidence while playing at 2x,
//! searches a synthetic cached copy with 30 seconds added, and opens the result.

use std::time::{Duration, Instant};

use redshank_model::{
    Annotation, AnnotationId, CaptureAnchor, MediaSource, NoteBody, RepresentationIdentity,
    RepresentationReceipt,
};
use redshank_playback::{PlaybackCommand, PlaybackState};
use redshank_surfaces::{CompactCommand, RedshankSurfaceState};

use super::Desktop;

const NOTE: &str = "Redshank headed restart receipt";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Seed,
    Verify,
    CacheSeed,
    CacheVerify,
    IdentityGuard,
    AlignmentGuard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    WaitLoaded,
    WaitCached,
    WaitPlaying,
    WaitSaved,
    IdentityWaitSame,
    IdentityWaitPaused,
    IdentityWaitRefused,
    IdentityWaitApproximate,
    AlignmentWaitCapture,
    AlignmentWaitReload,
    AlignmentWaitSearch,
    AlignmentWaitSeek,
    Closing,
    Complete,
}

pub(super) struct HeadedReceipt {
    mode: Mode,
    stage: Stage,
    started: Instant,
    expected_resume_ms: u64,
    observed_offset_ms: u64,
    failure: Option<String>,
    identity_notes: Vec<Annotation>,
    identity_request: Option<u64>,
    identity_paused_ms: u64,
    identity_refused_at: Option<Instant>,
    seek_started: Option<Instant>,
    alignment_original_source: Option<MediaSource>,
}

impl HeadedReceipt {
    pub(super) fn from_environment() -> Result<Option<Self>, String> {
        let Some(value) = std::env::var_os("REDSHANK_HEADED_RECEIPT") else {
            return Ok(None);
        };
        let mode = match value.to_string_lossy().as_ref() {
            "seed" => Mode::Seed,
            "verify" => Mode::Verify,
            "cache-seed" => Mode::CacheSeed,
            "cache-verify" => Mode::CacheVerify,
            "identity-guard" => Mode::IdentityGuard,
            "alignment-guard" => Mode::AlignmentGuard,
            other => {
                return Err(format!(
                    "REDSHANK_HEADED_RECEIPT must be seed, verify, cache-seed, cache-verify, identity-guard, or alignment-guard; got {other}"
                ));
            },
        };
        Ok(Some(Self {
            mode,
            stage: Stage::WaitLoaded,
            started: Instant::now(),
            expected_resume_ms: 0,
            observed_offset_ms: 0,
            failure: None,
            identity_notes: Vec::new(),
            identity_request: None,
            identity_paused_ms: 0,
            identity_refused_at: None,
            seek_started: None,
            alignment_original_source: None,
        }))
    }

    pub(super) fn active(&self) -> bool {
        self.stage != Stage::Complete
    }

    pub(super) fn drive(
        &mut self,
        desktop: &mut Desktop,
        state: &mut RedshankSurfaceState,
    ) -> Result<(), String> {
        let timeout = if self.mode == Mode::AlignmentGuard {
            Duration::from_secs(45)
        } else {
            TIMEOUT
        };
        if self.started.elapsed() > timeout {
            return Err(format!(
                "{} receipt timed out in {:?}",
                self.label(),
                self.stage
            ));
        }
        let snapshot = desktop.runtime.snapshot();
        if let PlaybackState::Unavailable(error) = &snapshot.state {
            return Err(format!("playback became unavailable: {error}"));
        }

        if self.mode == Mode::AlignmentGuard && self.stage != Stage::Closing {
            return self.drive_alignment(desktop, state, &snapshot);
        }
        match (self.mode, self.stage) {
            (Mode::IdentityGuard, Stage::WaitLoaded) if snapshot.state == PlaybackState::Paused => {
                self.seed_identity_notes(desktop, state, &snapshot)?;
                self.seek_started = Some(Instant::now());
                desktop.command(
                    state,
                    CompactCommand::OpenNote(self.identity_notes[0].id.clone()),
                )?;
                self.identity_request = desktop.pending_note_seek;
                self.stage = Stage::IdentityWaitSame;
            },
            (Mode::IdentityGuard, Stage::IdentityWaitSame)
                if self.identity_request.is_some()
                    && snapshot.completed_note_seek == self.identity_request
                    && snapshot.state == PlaybackState::Playing =>
            {
                self.check_seek_position(snapshot.position_ms, 1_000)?;
                println!(
                    "identity-guard same-copy seek acknowledged at {} ms",
                    snapshot.position_ms
                );
                desktop.command(state, CompactCommand::Pause)?;
                self.stage = Stage::IdentityWaitPaused;
            },
            (Mode::IdentityGuard, Stage::IdentityWaitPaused)
                if snapshot.state == PlaybackState::Paused =>
            {
                self.identity_paused_ms = snapshot.position_ms;
                let request_counter = desktop.next_note_seek;
                for note in &self.identity_notes[1..] {
                    for command in [
                        CompactCommand::OpenNote(note.id.clone()),
                        CompactCommand::PlaySpan(note.id.clone()),
                    ] {
                        let error = desktop
                            .command(state, command)
                            .err()
                            .ok_or("unverified note was allowed to seek")?;
                        let expected = if note.id == self.identity_notes[1].id {
                            "This copy differs"
                        } else {
                            "Couldn't verify this copy"
                        };
                        if !error.contains(expected) {
                            return Err(format!("wrong refusal for {}: {error}", note.id.0));
                        }
                        state.notice = Some(error);
                    }
                }
                if desktop.next_note_seek != request_counter
                    || desktop.pending_note_open.is_some()
                    || desktop.pending_note_seek.is_some()
                    || desktop.span_stop_ms.is_some()
                {
                    return Err("refused note scheduled a seek, open, or span stop".into());
                }
                desktop.project(state, &snapshot);
                self.check_identity_projection(state)?;
                self.identity_refused_at = Some(Instant::now());
                self.stage = Stage::IdentityWaitRefused;
            },
            (Mode::IdentityGuard, Stage::IdentityWaitRefused) => {
                if snapshot.state != PlaybackState::Paused
                    || snapshot.position_ms != self.identity_paused_ms
                    || snapshot.completed_note_seek != self.identity_request
                {
                    return Err("playback moved after a refused note open or span".into());
                }
                if self
                    .identity_refused_at
                    .is_some_and(|at| at.elapsed() >= Duration::from_millis(300))
                {
                    println!(
                        "identity-guard different/unproven open and span refused; paused position stayed {} ms",
                        snapshot.position_ms
                    );
                    self.seek_started = Some(Instant::now());
                    desktop.command(
                        state,
                        CompactCommand::OpenNoteApproximately(self.identity_notes[1].id.clone()),
                    )?;
                    self.identity_request = desktop.pending_note_seek;
                    self.stage = Stage::IdentityWaitApproximate;
                }
            },
            (Mode::IdentityGuard, Stage::IdentityWaitApproximate)
                if self.identity_request.is_some()
                    && snapshot.completed_note_seek == self.identity_request
                    && snapshot.state == PlaybackState::Playing =>
            {
                self.check_seek_position(snapshot.position_ms, 3_000)?;
                for original in &self.identity_notes {
                    if desktop.session.model.annotations.get(&original.id) != Some(original) {
                        return Err(
                            "identity check or approximate seek rewrote a frozen note".into()
                        );
                    }
                }
                self.observed_offset_ms = snapshot.position_ms;
                println!(
                    "identity-guard approximate seek acknowledged; all original note targets preserved"
                );
                // Only remove records created by this receipt. The runner gives
                // this mode a dedicated data directory and synthetic input.
                for note in &self.identity_notes {
                    desktop.session.model.annotations.remove(&note.id);
                }
                desktop.persistence.changed();
                desktop.close(state)?;
                self.stage = Stage::Closing;
            },
            (Mode::CacheSeed, Stage::WaitLoaded) if snapshot.state == PlaybackState::Paused => {
                let selected = desktop
                    .session
                    .selected
                    .clone()
                    .ok_or("cache receipt has no selected recording")?;
                desktop.command(state, CompactCommand::CacheItem(selected))?;
                self.stage = Stage::WaitCached;
            },
            (Mode::CacheSeed, Stage::WaitCached) if desktop.cache_pending.is_none() => {
                let selected = desktop
                    .session
                    .selected
                    .as_ref()
                    .ok_or("cached selection was lost")?;
                let cached = desktop
                    .session
                    .model
                    .library
                    .get(selected)
                    .is_some_and(|item| item.source().is_cached());
                if !cached {
                    return Err(state.notice.clone().unwrap_or_else(|| {
                        "episode download did not publish a cache entry".into()
                    }));
                }
                desktop.send(PlaybackCommand::Play)?;
                self.stage = Stage::WaitPlaying;
            },
            (Mode::Seed, Stage::WaitLoaded) if snapshot.state == PlaybackState::Paused => {
                desktop.send(PlaybackCommand::Play)?;
                self.stage = Stage::WaitPlaying;
            },
            (Mode::Seed, Stage::WaitPlaying)
                if snapshot.state == PlaybackState::Playing && snapshot.position_ms >= 50 =>
            {
                desktop.begin_note(state)?;
                let anchor = state
                    .text_capture
                    .as_ref()
                    .ok_or("headed receipt did not open the text editor")?
                    .anchor
                    .clone();
                self.observed_offset_ms = anchor.offset_ms;
                state.set_text_draft(NOTE);
                desktop.save_note(state, anchor, NOTE.into())?;
                self.stage = Stage::WaitSaved;
            },
            (Mode::CacheSeed, Stage::WaitPlaying)
                if snapshot.state == PlaybackState::Playing && snapshot.position_ms >= 300 =>
            {
                desktop.begin_note(state)?;
                let anchor = state
                    .text_capture
                    .as_ref()
                    .ok_or("headed receipt did not open the text editor")?
                    .anchor
                    .clone();
                self.observed_offset_ms = anchor.offset_ms;
                state.set_text_draft(NOTE);
                desktop.save_note(state, anchor, NOTE.into())?;
                self.stage = Stage::WaitSaved;
            },
            (Mode::Seed | Mode::CacheSeed, Stage::WaitSaved)
                if state.text_capture.is_none() && has_receipt_note(desktop) =>
            {
                desktop.close(state)?;
                self.stage = Stage::Closing;
            },
            (Mode::Verify | Mode::CacheVerify, Stage::WaitLoaded)
                if snapshot.state == PlaybackState::Paused =>
            {
                if !has_receipt_note(desktop) {
                    return Err("saved headed receipt note was not restored".into());
                }
                let selected = desktop
                    .session
                    .selected
                    .as_ref()
                    .ok_or("saved selection was not restored")?;
                if self.mode == Mode::CacheVerify {
                    let source = desktop
                        .session
                        .model
                        .library
                        .get(selected)
                        .ok_or("cached library item was not restored")?
                        .source();
                    if !source.is_cached() {
                        return Err("restored recording is not using the offline cache".into());
                    }
                    if snapshot
                        .representation
                        .as_ref()
                        .and_then(|receipt| receipt.complete_digest.as_ref())
                        .is_none()
                    {
                        return Err("offline playback did not validate a complete digest".into());
                    }
                }
                self.expected_resume_ms = desktop
                    .session
                    .model
                    .progress
                    .get(selected)
                    .map(|progress| progress.position_ms)
                    .filter(|position| *position > 0)
                    .ok_or("saved nonzero progress was not restored")?;
                desktop.send(PlaybackCommand::Play)?;
                self.stage = Stage::WaitPlaying;
            },
            (Mode::Verify | Mode::CacheVerify, Stage::WaitPlaying)
                if snapshot.state == PlaybackState::Playing
                    && snapshot.position_ms.saturating_add(100) >= self.expected_resume_ms =>
            {
                self.observed_offset_ms = snapshot.position_ms;
                desktop.close(state)?;
                self.stage = Stage::Closing;
            },
            (_, Stage::Closing)
                if desktop.closing
                    && desktop.persistence.durable == desktop.persistence.revision =>
            {
                self.stage = Stage::Complete;
            },
            _ => {},
        }
        Ok(())
    }

    pub(super) fn fail(&mut self, error: String) {
        self.failure = Some(error);
        self.stage = Stage::Complete;
    }

    pub(super) fn result(&self) -> Result<String, String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.stage != Stage::Complete {
            return Err(format!("{} receipt exited before completion", self.label()));
        }
        Ok(format!(
            "redshank-headed-receipt {} PASS position_ms={}",
            self.label(),
            self.observed_offset_ms
        ))
    }

    fn label(&self) -> &'static str {
        match self.mode {
            Mode::Seed => "seed",
            Mode::Verify => "verify",
            Mode::CacheSeed => "cache-seed",
            Mode::CacheVerify => "cache-verify",
            Mode::IdentityGuard => "identity-guard",
            Mode::AlignmentGuard => "alignment-guard",
        }
    }

    fn check_seek_position(&self, observed_ms: u64, requested_ms: u64) -> Result<(), String> {
        let elapsed_ms = self
            .seek_started
            .ok_or("seek observation has no start time")?
            .elapsed()
            .as_millis() as u64;
        // The presented audio clock can trail the acknowledged decoder seek
        // by an output callback; retain a bounded 100 ms observation tolerance.
        if observed_ms.saturating_add(100) < requested_ms
            || observed_ms > requested_ms.saturating_add(elapsed_ms).saturating_add(250)
        {
            return Err(format!(
                "acknowledged seek position {observed_ms} outside source-time envelope from {requested_ms}, elapsed {elapsed_ms} ms"
            ));
        }
        Ok(())
    }

    fn drive_alignment(
        &mut self,
        desktop: &mut Desktop,
        state: &mut RedshankSurfaceState,
        snapshot: &redshank_playback::PlaybackSnapshot,
    ) -> Result<(), String> {
        match self.stage {
            Stage::WaitLoaded if snapshot.state == PlaybackState::Paused => {
                desktop.session.model.settings.reaction_offset_ms = 500;
                desktop.session.model.settings.alignment_window_ms = 5_000;
                desktop.command(state, CompactCommand::Seek(0))?;
                desktop.command(state, CompactCommand::SetRate(200))?;
                desktop.command(state, CompactCommand::Play)?;
                self.stage = Stage::AlignmentWaitCapture;
            },
            Stage::AlignmentWaitCapture
                if snapshot.state == PlaybackState::Playing && snapshot.position_ms >= 6_000 =>
            {
                if snapshot.rate_percent != 200 {
                    return Err("alignment fixture did not run at two-times playback speed".into());
                }
                let target = desktop.session.capture(snapshot)?;
                let fingerprint = target
                    .fingerprint
                    .as_ref()
                    .ok_or("native capture supplied no source-time fingerprint")?;
                fingerprint
                    .validate()
                    .map_err(|error| format!("captured fingerprint invalid: {error:?}"))?;
                if target.pressed_offset_ms != Some(snapshot.position_ms)
                    || target.offset_ms + 500 != snapshot.position_ms
                {
                    return Err(
                        "native capture failed to preserve the reaction-offset anchor".into(),
                    );
                }
                let id = AnnotationId("headed-alignment-guard".into());
                if desktop.session.model.annotations.contains_key(&id) {
                    return Err("alignment fixture note already exists; preserve failed run evidence and use fresh fixture data".into());
                }
                println!(
                    "alignment-guard captured source-time fingerprint at 2x: pressed_ms={} anchor_ms={} frames={}",
                    snapshot.position_ms,
                    target.offset_ms,
                    fingerprint.frames.len()
                );
                desktop
                    .session
                    .model
                    .add_text_annotation(
                        id.clone(),
                        target.clone(),
                        "Native synthetic alignment receipt".into(),
                        super::now_ms(),
                    )
                    .map_err(|error| format!("could not store captured note: {error:?}"))?;
                self.identity_notes
                    .push(desktop.session.model.annotations[&id].clone());
                self.expected_resume_ms = target.offset_ms + 30_000;
                let path = std::env::var_os("REDSHANK_ALIGNMENT_FIXTURE")
                    .ok_or("alignment-guard needs REDSHANK_ALIGNMENT_FIXTURE")?;
                let path = std::path::PathBuf::from(path);
                let bytes = std::fs::read(&path)
                    .map_err(|error| format!("read shifted fixture: {error}"))?;
                let receipt = RepresentationReceipt {
                    complete_digest: Some(format!("blake3:{}", blake3::hash(&bytes).to_hex())),
                    byte_length: Some(bytes.len() as u64),
                    ..Default::default()
                };
                let item = desktop
                    .session
                    .model
                    .library
                    .get_mut(&target.item_id)
                    .ok_or("captured fixture item disappeared")?;
                self.alignment_original_source = Some(item.source().clone());
                item.replace_source(MediaSource::Cached {
                    path: path.to_string_lossy().into_owned(),
                    origin_url: "https://receipt.invalid/synthetic.wav".into(),
                    representation: Box::new(receipt),
                });
                desktop.command(state, CompactCommand::SetRate(100))?;
                desktop.select(target.item_id)?;
                self.stage = Stage::AlignmentWaitReload;
            },
            Stage::AlignmentWaitReload
                if desktop.session.matches(snapshot) && snapshot.state == PlaybackState::Paused =>
            {
                let original = &self.identity_notes[0];
                if desktop.session.note_identity(&original.target, snapshot)
                    != RepresentationIdentity::Different
                {
                    return Err(
                        "shifted cached fixture was not identified as different bytes".into(),
                    );
                }
                desktop.command(state, CompactCommand::RealignNote(original.id.clone()))?;
                self.stage = Stage::AlignmentWaitSearch;
            },
            Stage::AlignmentWaitSearch if desktop.alignment_pending.is_none() => {
                let original = &self.identity_notes[0];
                let derived = desktop
                    .session
                    .model
                    .derived_positions
                    .get(&original.id)
                    .ok_or_else(|| {
                        format!(
                            "native alignment did not produce a position: {}",
                            state.notice.as_deref().unwrap_or("no notice")
                        )
                    })?;
                if derived.offset_ms.abs_diff(self.expected_resume_ms) > 150
                    || derived.original_target != original.target
                {
                    return Err(format!(
                        "native alignment found {} ms, expected {} +/- 150 ms, or changed original target",
                        derived.offset_ms, self.expected_resume_ms
                    ));
                }
                println!(
                    "alignment-guard digest-bound search estimated_ms={} expected_ms={} score_per_mille={} runner_up_per_mille={}",
                    derived.offset_ms,
                    self.expected_resume_ms,
                    derived.confidence_per_mille,
                    derived.runner_up_per_mille
                );
                self.expected_resume_ms = derived.offset_ms;
                self.seek_started = Some(Instant::now());
                desktop.command(state, CompactCommand::OpenAlignedNote(original.id.clone()))?;
                self.identity_request = desktop.pending_note_seek;
                self.stage = Stage::AlignmentWaitSeek;
            },
            Stage::AlignmentWaitSeek
                if self.identity_request.is_some()
                    && snapshot.completed_note_seek == self.identity_request
                    && snapshot.state == PlaybackState::Playing =>
            {
                self.check_seek_position(snapshot.position_ms, self.expected_resume_ms)?;
                let original = &self.identity_notes[0];
                if desktop.session.model.annotations.get(&original.id) != Some(original) {
                    return Err("opening the aligned estimate rewrote the original note".into());
                }
                self.observed_offset_ms = snapshot.position_ms;
                println!(
                    "alignment-guard aligned seek acknowledged at {} ms; captured original preserved",
                    snapshot.position_ms
                );
                desktop.session.model.annotations.remove(&original.id);
                desktop.session.model.derived_positions.remove(&original.id);
                desktop
                    .session
                    .model
                    .progress
                    .remove(&original.target.item_id);
                desktop.session.hold_progress = true;
                if let Some(source) = self.alignment_original_source.take() {
                    desktop
                        .session
                        .model
                        .library
                        .get_mut(&original.target.item_id)
                        .ok_or("fixture item disappeared")?
                        .replace_source(source);
                }
                desktop.persistence.changed();
                desktop.close(state)?;
                self.stage = Stage::Closing;
            },
            _ => {},
        }
        Ok(())
    }

    fn seed_identity_notes(
        &mut self,
        desktop: &mut Desktop,
        state: &mut RedshankSurfaceState,
        snapshot: &redshank_playback::PlaybackSnapshot,
    ) -> Result<(), String> {
        if snapshot.duration_ms.is_none_or(|duration| duration < 7_000) {
            return Err(
                "identity-guard needs a local audio fixture at least seven seconds long".into(),
            );
        }
        let receipt = snapshot
            .representation
            .clone()
            .ok_or("loaded fixture has no receipt")?;
        if receipt
            .complete_digest
            .as_ref()
            .is_none_or(|digest| digest.is_empty())
        {
            return Err("identity-guard needs a complete local-copy digest".into());
        }
        let item_id = desktop
            .session
            .selected
            .clone()
            .ok_or("identity fixture has no selection")?;
        for (index, label) in ["same", "different", "unproven"].into_iter().enumerate() {
            let id = AnnotationId(format!("headed-identity-guard-{label}"));
            if desktop.session.model.annotations.contains_key(&id) {
                return Err(format!(
                    "receipt fixture note {} already exists; use a fresh dedicated data directory",
                    id.0
                ));
            }
            let mut representation = receipt.clone();
            if index == 1 {
                representation.complete_digest = Some(format!(
                    "blake3:{}",
                    blake3::hash(b"identity-guard different copy").to_hex()
                ));
                if representation.complete_digest == receipt.complete_digest {
                    return Err(
                        "fixture unexpectedly matches the deliberately different digest".into(),
                    );
                }
            } else if index == 2 {
                representation = Default::default();
            }
            let offset_ms = 1_000 + index as u64 * 2_000;
            let target = CaptureAnchor {
                item_id: item_id.clone(),
                offset_ms,
                end_offset_ms: Some(offset_ms + 500),
                pressed_offset_ms: Some(offset_ms + 100),
                representation,
                fingerprint: None,
            };
            desktop
                .session
                .model
                .add_text_annotation(
                    id.clone(),
                    target,
                    format!("Identity receipt: {label}"),
                    super::now_ms(),
                )
                .map_err(|error| format!("could not seed identity receipt: {error:?}"))?;
            self.identity_notes
                .push(desktop.session.model.annotations[&id].clone());
        }
        state.active_tab = redshank_surfaces::SurfaceTab::Notes;
        desktop.project(state, snapshot);
        self.check_identity_projection(state)?;
        Ok(())
    }

    fn check_identity_projection(&self, state: &RedshankSurfaceState) -> Result<(), String> {
        for (note, expected) in self.identity_notes.iter().zip([
            RepresentationIdentity::Same,
            RepresentationIdentity::Different,
            RepresentationIdentity::Unproven,
        ]) {
            if state.note_identities.get(&note.id) != Some(&expected) {
                return Err(format!("note {} did not project {expected:?}", note.id.0));
            }
        }
        Ok(())
    }
}

fn has_receipt_note(desktop: &Desktop) -> bool {
    desktop
        .session
        .model
        .annotations
        .values()
        .any(|annotation| {
            matches!(
                &annotation.body,
                NoteBody::Text { plain_text } if plain_text == NOTE
            )
        })
}
