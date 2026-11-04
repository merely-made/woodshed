use super::*;

impl OutputRuntime {
    /// Validate and prepare the complete disposable schedule on the control
    /// thread. This does not start playback or mutate a retained song.
    pub fn prepare_program(&self, plan: ProgramPlan) -> Result<PreparedProgram, OutputError> {
        PreparedProgram::prepare(plan, self.rate)
    }

    /// Start an immutable prepared program at its declared start index.
    pub fn start_program(
        &mut self,
        prepared: PreparedProgram,
    ) -> Result<ProgramEpoch, OutputError> {
        if prepared.rate != self.rate {
            return Err(err(
                "prepared program sample rate differs from this output runtime",
            ));
        }
        if self.snapshot().playing
            || self.song.playing
            || self.capture_requested
            || self.capture_busy()
        {
            return Err(err(
                "program playback conflicts with backing or capture ownership",
            ));
        }
        if self.preview.busy() || self.snapshot().preview_voices != 0 {
            return Err(err(
                "wait for one-shot preview playback to finish before starting a program",
            ));
        }
        if (self.active_program_cards.is_some()
            && !matches!(
                self.snapshot().program.status,
                ProgramStatus::Stopped | ProgramStatus::Complete
            ))
            || self.snapshot().program.status == ProgramStatus::Playing
            || self.snapshot().program.status == ProgramStatus::Paused
            || self.snapshot().program.status == ProgramStatus::WaitingManual
            || self.snapshot().program.status == ProgramStatus::Refused
            || self.snapshot().program.status == ProgramStatus::WaitingObserver
        {
            return Err(err("a prepared program already owns the output lane"));
        }
        self.next_program_epoch = self.next_program_epoch.wrapping_add(1).max(1);
        let epoch = ProgramEpoch(self.next_program_epoch);
        let card_count = prepared.cards.len();
        self.send(Command::ProgramStart {
            data: Some(OwnedGc::new(prepared)),
            epoch,
        })?;
        self.active_program_cards = Some(card_count);
        self.requested_program_revision = 0;
        Ok(epoch)
    }

    /// Queue an already-prepared complete program for the next natural segment
    /// boundary. The current segment is unchanged. Revisions increase within an
    /// epoch; the latest queued revision wins. After swapping, playback continues
    /// at the next ordinal segment (the same ordinal for Card repeat), or at
    /// segment zero if that ordinal no longer exists.
    /// Pause defers the boundary. Seek, explicit advance, stop and session clear
    /// cancel pending replacements. Loop controls remain independently applied.
    pub fn replace_program_at_boundary(
        &mut self,
        epoch: ProgramEpoch,
        revision: u64,
        prepared: PreparedProgram,
    ) -> Result<(), OutputError> {
        if prepared.rate != self.rate {
            return Err(err(
                "replacement sample rate differs from this output runtime",
            ));
        }
        let snapshot = self.snapshot().program;
        if epoch.0 == 0
            || epoch.0 != self.next_program_epoch
            || self.active_program_cards.is_none()
            || (snapshot.epoch == epoch
                && matches!(
                    snapshot.status,
                    ProgramStatus::Stopped | ProgramStatus::Complete
                ))
        {
            return Err(err("replacement requires the current active program epoch"));
        }
        if revision == 0 || revision <= self.requested_program_revision {
            return Err(err(
                "replacement revision must increase within its program epoch",
            ));
        }
        self.send(Command::ProgramReplace {
            epoch,
            revision,
            data: Some(OwnedGc::new(prepared)),
        })?;
        self.requested_program_revision = revision;
        Ok(())
    }

    pub(super) fn program_card_count(&self, epoch: ProgramEpoch) -> Option<usize> {
        let snapshot = self.snapshot().program;
        if snapshot.epoch == epoch && snapshot.card_count != 0 {
            Some(snapshot.card_count)
        } else {
            self.active_program_cards
        }
    }

    pub fn pause_program(&mut self, epoch: ProgramEpoch) -> Result<(), OutputError> {
        self.send(Command::ProgramPause {
            epoch,
            paused: true,
        })
    }

    pub fn resume_program(&mut self, epoch: ProgramEpoch) -> Result<(), OutputError> {
        self.send(Command::ProgramPause {
            epoch,
            paused: false,
        })
    }

    pub fn set_program_tempo(
        &mut self,
        epoch: ProgramEpoch,
        card: usize,
        bpm: f64,
    ) -> Result<(), OutputError> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(err("program tempo must be finite and positive"));
        }
        if !self
            .program_card_count(epoch)
            .is_some_and(|count| card < count)
        {
            return Err(err(
                "program tempo card index is outside the prepared program",
            ));
        }
        let step = bpm * PROGRAM_TICKS_PER_BEAT as f64 * (u32::MAX as f64 + 1.0)
            / (60.0 * self.rate as f64);
        if !step.is_finite() || step < 1.0 || step > u64::MAX as f64 {
            return Err(err(
                "program tempo is outside the representable output clock range",
            ));
        }
        self.send(Command::ProgramTempo { epoch, card, bpm })
    }

    /// Apply one tempo command to the complete active and pending programs.
    /// Keeps the current tick phase and avoids one queue entry per segment.
    pub fn set_program_tempo_all(
        &mut self,
        epoch: ProgramEpoch,
        bpm: f64,
    ) -> Result<(), OutputError> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(err("program tempo must be finite and positive"));
        }
        let step = bpm * PROGRAM_TICKS_PER_BEAT as f64 * (u32::MAX as f64 + 1.0)
            / (60.0 * self.rate as f64);
        if !step.is_finite() || step < 1.0 || step > u64::MAX as f64 {
            return Err(err(
                "program tempo is outside the representable output clock range",
            ));
        }
        self.send(Command::ProgramTempoAll { epoch, bpm })
    }

    /// Change looping without restarting the current card or re-crediting its
    /// Started transition. The command is fenced to the current program epoch.
    pub fn set_program_loop(
        &mut self,
        epoch: ProgramEpoch,
        mode: ProgramLoop,
    ) -> Result<(), OutputError> {
        self.send(Command::ProgramLoopMode { epoch, mode })
    }

    pub fn advance_program(&mut self, epoch: ProgramEpoch) -> Result<(), OutputError> {
        if self.snapshot().program.status == ProgramStatus::WaitingObserver {
            return Err(err("drain pending program transitions before advancing"));
        }
        self.send(Command::ProgramAdvance { epoch })
    }

    /// Seek to a card in the current program. If accepted seeks accumulate
    /// behind an observer-blocked transition before that status is published,
    /// they coalesce to the latest requested destination; a pending stop
    /// always takes precedence.
    pub fn seek_program(&mut self, epoch: ProgramEpoch, card: usize) -> Result<(), OutputError> {
        if !self
            .program_card_count(epoch)
            .is_some_and(|count| card < count)
        {
            return Err(err(
                "program seek card index is outside the prepared program",
            ));
        }
        if self.snapshot().program.status == ProgramStatus::WaitingObserver {
            return Err(err("drain pending program transitions before seeking"));
        }
        self.send(Command::ProgramSeek { epoch, card })
    }

    /// Stop the current program. An accepted stop is retained independently
    /// of the bounded seek queue and cannot be displaced by observer pressure.
    pub fn stop_program(&mut self, epoch: ProgramEpoch) -> Result<(), OutputError> {
        if self.snapshot().program.status == ProgramStatus::WaitingObserver {
            return Err(err("drain pending program transitions before stopping"));
        }
        self.send(Command::ProgramStop { epoch })
    }

    /// Drain callback-originated card transitions on the control thread.
    pub fn drain_program_transitions(&mut self) -> Vec<ProgramTransition> {
        let mut transitions = Vec::new();
        while let Ok(transition) = self.program_transitions.pop() {
            transitions.push(transition);
        }
        transitions
    }
}
