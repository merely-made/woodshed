use std::time::{Duration, Instant};

use redshank_model::NoteBody;
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    WaitLoaded,
    WaitCached,
    WaitPlaying,
    WaitSaved,
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
            other => {
                return Err(format!(
                    "REDSHANK_HEADED_RECEIPT must be seed, verify, cache-seed, or cache-verify; got {other}"
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
        if self.started.elapsed() > TIMEOUT {
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

        match (self.mode, self.stage) {
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
        }
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
