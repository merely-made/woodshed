use super::*;

impl Processor {
    pub(super) fn program_transition(
        &mut self,
        kind: ProgramTransitionKind,
        action: ProgramPendingAction,
    ) -> bool {
        let Some(card) = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index))
        else {
            return false;
        };
        let transition = ProgramTransition {
            epoch: self.program_epoch,
            card_token: card.token,
            device_frame: self.current_frame,
            active_frames: self.program_active_frames,
            kind,
        };
        match self.program_transitions.push(transition) {
            Ok(()) => {
                self.apply_program_action(action);
                true
            },
            Err(rtrb::PushError::Full(transition)) => {
                self.program_transition_pending = Some(transition);
                self.program_pending_action = action;
                self.program_resume_after_observer = self.program_status;
                self.program_status = ProgramStatus::WaitingObserver;
                false
            },
        }
    }

    pub(super) fn apply_program_action(&mut self, action: ProgramPendingAction) {
        match action {
            ProgramPendingAction::None => {},
            ProgramPendingAction::CardStarted => {
                self.program_card_started = true;
                self.program_status = ProgramStatus::Playing;
            },
            ProgramPendingAction::NextCard => {
                if let Some((revision, replacement)) = self.program_replacement.take() {
                    let next = if self.program_loop_mode == ProgramLoop::Card {
                        Some(self.program_card_index)
                    } else {
                        self.program_card_index.checked_add(1)
                    }
                    .filter(|index| *index < replacement.cards.len())
                    .unwrap_or(0);
                    self.program = Some(replacement);
                    self.program_revision = revision;
                    self.move_program_card(Some(next));
                } else {
                    self.move_program_card(None);
                }
            },
            ProgramPendingAction::AdvanceCard => self.advance_program_card(),
            ProgramPendingAction::Seek(card) => self.move_program_card(Some(card)),
            ProgramPendingAction::Stop => {
                for voice in &mut self.program_voices {
                    *voice = None;
                }
                self.program_replacement = None;
                self.program = None;
                self.program_status = ProgramStatus::Stopped;
                self.program_card_started = false;
                self.program_event_token = None;
                self.program_deferred_controls = std::array::from_fn(|_| None);
                self.program_deferred_len = 0;
                self.program_stop_pending = false;
            },
        }
    }

    pub(super) fn apply_program_control(&mut self, control: DeferredProgramControl) {
        let was_paused = self.program_status == ProgramStatus::Paused;
        match control {
            DeferredProgramControl::Seek(card) if self.program_card_started => {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Seek),
                    ProgramPendingAction::Seek(card),
                );
            },
            DeferredProgramControl::Seek(card) => self.move_program_card(Some(card)),
            DeferredProgramControl::Stop if self.program_card_started => {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Stop),
                    ProgramPendingAction::Stop,
                );
            },
            DeferredProgramControl::Stop => self.apply_program_action(ProgramPendingAction::Stop),
        }
        if was_paused
            && !matches!(
                self.program_status,
                ProgramStatus::Stopped | ProgramStatus::Complete | ProgramStatus::WaitingObserver
            )
        {
            self.program_resume_after_pause = self.program_status;
            self.program_status = ProgramStatus::Paused;
        }
    }

    pub(super) fn defer_program_control(&mut self, control: DeferredProgramControl) {
        // Cancel as soon as the control is accepted, including while an Ended
        // transition awaits observation; it must not apply on a deferred seek.
        self.program_replacement = None;
        if self.program_transition_pending.is_some() {
            match control {
                // Stop is a dedicated bounded pending bit and always wins
                // over queued seeks; observer pressure cannot lose it.
                DeferredProgramControl::Stop => self.program_stop_pending = true,
                DeferredProgramControl::Seek(card) => {
                    if self.program_deferred_len < self.program_deferred_controls.len() {
                        self.program_deferred_controls[self.program_deferred_len] =
                            Some(DeferredProgramControl::Seek(card));
                        self.program_deferred_len += 1;
                    } else if self.program_deferred_len > 0 {
                        // Under pressure, accepted seeks coalesce to the
                        // newest destination. Their order is not meaningful;
                        // Stop remains lossless and takes precedence.
                        self.program_deferred_controls[self.program_deferred_len - 1] =
                            Some(DeferredProgramControl::Seek(card));
                    }
                },
            }
            return;
        }
        self.apply_program_control(control);
    }

    pub(super) fn dispatch_deferred_program_control(&mut self) {
        if self.program_transition_pending.is_some() {
            return;
        }
        if self.program_stop_pending {
            self.program_stop_pending = false;
            self.program_deferred_controls = std::array::from_fn(|_| None);
            self.program_deferred_len = 0;
            if self.program.is_some() {
                self.apply_program_control(DeferredProgramControl::Stop);
            }
            return;
        }
        if self.program_deferred_len == 0 {
            return;
        }
        let control = self.program_deferred_controls[0]
            .take()
            .expect("deferred program control slot is occupied");
        for index in 1..self.program_deferred_len {
            self.program_deferred_controls[index - 1] =
                self.program_deferred_controls[index].take();
        }
        self.program_deferred_len -= 1;
        self.apply_program_control(control);
    }

    pub(super) fn flush_program_transition(&mut self) -> bool {
        let Some(transition) = self.program_transition_pending.take() else {
            return true;
        };
        match self.program_transitions.push(transition) {
            Ok(()) => {
                let action =
                    std::mem::replace(&mut self.program_pending_action, ProgramPendingAction::None);
                let was_paused = self.program_resume_after_observer == ProgramStatus::Paused;
                self.program_status = if was_paused {
                    self.program_resume_after_pause
                } else {
                    self.program_resume_after_observer
                };
                self.apply_program_action(action);
                if was_paused
                    && !matches!(
                        self.program_status,
                        ProgramStatus::Stopped | ProgramStatus::Complete
                    )
                {
                    self.program_resume_after_pause = self.program_status;
                    self.program_status = ProgramStatus::Paused;
                }
                self.dispatch_deferred_program_control();
                true
            },
            Err(rtrb::PushError::Full(transition)) => {
                self.program_transition_pending = Some(transition);
                self.program_status = ProgramStatus::WaitingObserver;
                false
            },
        }
    }

    /// Explicit Next leaves a focused Card repeat. Automatic completion alone
    /// consults that repeat mode; Set-wide looping still wraps at the boundary.
    pub(super) fn advance_program_card(&mut self) {
        if self.program_loop_mode == ProgramLoop::Card {
            self.move_program_card(Some(self.program_card_index.saturating_add(1)));
        } else {
            self.move_program_card(None);
        }
    }

    pub(super) fn move_program_card(&mut self, requested: Option<usize>) {
        let Some(program) = self.program.as_ref() else {
            return;
        };
        let next = if let Some(card) = requested {
            Some(card)
        } else {
            match self.program_loop_mode {
                ProgramLoop::Card => Some(self.program_card_index),
                ProgramLoop::Set if self.program_card_index + 1 >= program.cards.len() => Some(0),
                ProgramLoop::Set => Some(self.program_card_index + 1),
                ProgramLoop::Once if self.program_card_index + 1 < program.cards.len() => {
                    Some(self.program_card_index + 1)
                },
                ProgramLoop::Once => None,
            }
        };
        match next {
            Some(index) if index < program.cards.len() => {
                self.program_card_index = index;
                self.program_event_index = 0;
                self.program_tick_phase_q32 = 0;
                self.program_card_frames = 0;
                self.program_tail_remaining = 0;
                self.program_tail_started = false;
                self.program_active_frames = 0;
                self.program_card_started = false;
                self.program_click_last_beat = None;
                self.program_click_last_step = None;
                self.program_event_due_frame = None;
                self.program_event_group_base = None;
                self.program_event_token = None;
                self.program_manual_body_done = false;
                self.program_status = ProgramStatus::Playing;
            },
            _ => {
                self.program_status = ProgramStatus::Complete;
                self.program_card_started = false;
            },
        }
    }

    pub(super) fn enter_program_card(&mut self) {
        let Some(program) = self.program.as_ref() else {
            return;
        };
        let Some(card) = program.cards.get(self.program_card_index) else {
            self.program_status = ProgramStatus::Complete;
            return;
        };
        match card.end {
            ProgramEnd::Refusal(code) => {
                self.program_status = ProgramStatus::Refused;
                let _ = self.program_transition(
                    ProgramTransitionKind::Refused(code),
                    ProgramPendingAction::None,
                );
            },
            _ => {
                let bpm = card.bpm;
                self.program_tick_step_q32 = (bpm
                    * super::super::clock::PROGRAM_TICKS_PER_BEAT as f64
                    * (u32::MAX as f64 + 1.0)
                    / (60.0 * self.rate as f64))
                    .round() as u64;
                self.program_status = ProgramStatus::Playing;
                let _ = self.program_transition(
                    ProgramTransitionKind::Started,
                    ProgramPendingAction::CardStarted,
                );
            },
        }
    }
}
