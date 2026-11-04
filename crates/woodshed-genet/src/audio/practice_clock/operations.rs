use super::*;

impl PracticeClockHost {
    pub(super) fn sync(&mut self, output: &mut OutputRuntime, ui: &mut UiState) -> bool {
        if self.music.active()
            && (ui.rehearsal_running || live_kind(ui).is_some() || ui.song_playing)
        {
            ui.stop_music_practice();
        }
        if ui.music_practice_requested() || self.music.active() {
            if ui.music_practice_requested() {
                if let Some(bound) = &mut self.active {
                    request_stop(output, bound);
                    output.drain_program_transitions();
                    let snapshot = output.snapshot().program;
                    if snapshot.epoch == bound.epoch
                        && matches!(
                            snapshot.status,
                            ProgramStatus::Stopped | ProgramStatus::Complete
                        )
                    {
                        self.active = None;
                    } else {
                        return true;
                    }
                }
            }
            let busy = self.music.sync(output, ui);
            if busy || ui.music_practice_requested() {
                return busy;
            }
        }
        let snapshot = output.snapshot().program;
        if let Some(bound) = &mut self.active {
            let same_session = std::sync::Arc::ptr_eq(&bound.session, &ui.audio_session);
            let desired = same_session
                && if let Some(kind) = bound.live {
                    !ui.rehearsal_running && live_kind(ui).is_none_or(|desired| desired == kind)
                } else {
                    ui.rehearsal_running
                        && ui.rehearsal_epoch == bound.view_epoch
                        && ui.rehearsal_owner == bound.owner
                };
            if !desired && !bound.retiring {
                request_stop(output, bound);
                if !bound.retiring {
                    return true;
                }
            }
            if bound.retiring {
                // The processor must acknowledge retirement before a successor
                // can acquire this lane, including repeated session-local IDs.
                output.drain_program_transitions();
                if snapshot.epoch == bound.epoch
                    && matches!(
                        snapshot.status,
                        ProgramStatus::Stopped | ProgramStatus::Complete
                    )
                {
                    self.active = None;
                } else {
                    return true;
                }
            }
        }
        if self.active.is_none() {
            output.drain_program_transitions();
            let (source, owner, live, start, loop_mode) = if ui.rehearsal_running {
                let Some(source) = ui.practice_clock_program() else {
                    return false;
                };
                stop_live(ui);
                let loop_mode = match ui.rehearsal_set().loop_mode {
                    woodshedding::rehearsal::LoopMode::Off => ProgramLoop::Once,
                    woodshedding::rehearsal::LoopMode::All => ProgramLoop::Set,
                    woodshedding::rehearsal::LoopMode::Card => ProgramLoop::Card,
                };
                (
                    source,
                    ui.rehearsal_owner,
                    None,
                    ui.practice_clock_start_index(),
                    loop_mode,
                )
            } else if let Some(kind) = live_kind(ui) {
                let source = PracticeProgram {
                    cards: vec![ui.stage.live_clock_card(kind, ui.transport.bpm)],
                };
                (source, None, Some(kind), 0, ProgramLoop::Card)
            } else {
                ui.stage_clock_contacts.clear();
                ui.stage_clock_event = None;
                return false;
            };
            let (plan, mut refusals) = lower_program(
                &source,
                start,
                loop_mode,
                output.sample_rate(),
                live.is_some(),
            );
            let prepared = match output.prepare_program(plan) {
                Ok(prepared) => prepared,
                Err(error) => {
                    refuse(ui, owner, ui.rehearsal_epoch, start, error.to_string());
                    return false;
                },
            };
            for (index, reason) in refusals.iter_mut().enumerate() {
                if reason.is_none() {
                    *reason = prepared.refusal_reason(index).map(str::to_owned);
                }
            }
            match output.start_program(prepared) {
                Ok(epoch) => {
                    self.active = Some(BoundProgram {
                        session: ui.audio_session.clone(),
                        epoch,
                        view_epoch: ui.rehearsal_epoch,
                        owner,
                        source,
                        refusals,
                        paused: false,
                        retiring: false,
                        live,
                        loop_mode,
                    })
                },
                Err(error) => {
                    refuse(ui, owner, ui.rehearsal_epoch, start, error.to_string());
                    return false;
                },
            }
        }
        let bound = self.active.as_mut().expect("started or retained program");
        let rate = output.sample_rate();
        // Validate the retained source before processing even an already queued
        // Started transition. Tempo edits preserve occurrence identity.
        let live_source = if let Some(kind) = bound.live {
            let live = PracticeProgram {
                cards: vec![ui.stage.live_clock_card(kind, ui.transport.bpm)],
            };
            if same_source(&bound.source, &live) {
                Ok(live)
            } else {
                Err("The live sequence changed.".into())
            }
        } else {
            ui.live_practice_program_for_host()
        };
        match live_source {
            Ok(live) => {
                for (index, (old, new)) in bound.source.cards.iter_mut().zip(live.cards).enumerate()
                {
                    if old.bpm.to_bits() != new.bpm.to_bits() {
                        if let Err(error) =
                            output.set_program_tempo(bound.epoch, index, f64::from(new.bpm))
                        {
                            refuse(ui, bound.owner, bound.view_epoch, index, error.to_string());
                            request_stop(output, bound);
                            return true;
                        }
                        old.bpm = new.bpm;
                    }
                }
            },
            Err(reason) => {
                // Live controls replace their derived phrase; authored Card
                // edits require a new explicit Run and get no replacement credit.
                if bound.live.is_none() {
                    refuse(
                        ui,
                        bound.owner,
                        bound.view_epoch,
                        snapshot.card_index,
                        reason,
                    );
                }
                request_stop(output, bound);
                return true;
            },
        }
        let desired_loop = if bound.live.is_some() {
            ProgramLoop::Card
        } else {
            match ui.rehearsal_set().loop_mode {
                woodshedding::rehearsal::LoopMode::Off => ProgramLoop::Once,
                woodshedding::rehearsal::LoopMode::All => ProgramLoop::Set,
                woodshedding::rehearsal::LoopMode::Card => ProgramLoop::Card,
            }
        };
        if bound.loop_mode != desired_loop {
            // Loop policy is transport intent, not a mutation of retained Cards.
            // Use an epoch-fenced command with no restart or new observation.
            if output.set_program_loop(bound.epoch, desired_loop).is_ok() {
                bound.loop_mode = desired_loop;
            }
        }
        let desired_pause = if bound.live.is_some() {
            live_kind(ui).is_none()
        } else {
            ui.rehearsal_paused
        };
        if bound.paused != desired_pause {
            let result = if desired_pause {
                output.pause_program(bound.epoch)
            } else {
                output.resume_program(bound.epoch)
            };
            if result.is_ok() {
                bound.paused = desired_pause;
            }
        }
        if bound.live.is_none() {
            let commands = std::mem::take(&mut ui.practice_clock_commands);
            for (index, command) in commands.iter().enumerate() {
                let result = match command {
                    PracticeClockCommand::Advance => output.advance_program(bound.epoch),
                    PracticeClockCommand::Seek(card) => output.seek_program(bound.epoch, *card),
                };
                if let Err(error) = result {
                    ui.card_timing_notice = Some(error.to_string());
                    ui.practice_clock_commands
                        .extend_from_slice(&commands[index..]);
                    break;
                }
            }
        }
        for transition in output.drain_program_transitions() {
            if transition.epoch != bound.epoch {
                continue;
            }
            let Some(index) = transition
                .card_token
                .checked_sub(1)
                .and_then(|value| usize::try_from(value).ok())
            else {
                continue;
            };
            if index >= bound.source.cards.len() {
                continue;
            }
            if let Some(owner) = bound.owner {
                match transition.kind {
                    ProgramTransitionKind::Started => {
                        let accepted = ui.practice_card_started(PracticeClockObservation {
                            epoch: bound.view_epoch,
                            owner,
                            card_index: index,
                            event_ordinal: None,
                            device_frame: transition.device_frame,
                            active_frames: transition.active_frames,
                            sample_rate: rate,
                            beat_ticks: 0,
                            ticks_per_beat: BEAT_TICKS,
                            card_frames: 0,
                            status: PracticeClockStatus::Playing,
                        });
                        if !accepted {
                            request_stop(output, bound);
                            return true;
                        }
                    },
                    ProgramTransitionKind::Ended(reason) => {
                        ui.practice_card_ended(
                            bound.view_epoch,
                            owner,
                            index,
                            match reason {
                                ProgramEndReason::Completed => PracticeCardEndReason::Completed,
                                ProgramEndReason::Seek => PracticeCardEndReason::Seek,
                                ProgramEndReason::Stop => PracticeCardEndReason::Stop,
                            },
                            transition.active_frames,
                            rate,
                        );
                    },
                    ProgramTransitionKind::Refused(_) => {
                        let reason = bound.refusals[index].clone().unwrap_or_else(|| {
                            "This Card cannot play on the active output clock.".into()
                        });
                        refuse(ui, bound.owner, bound.view_epoch, index, reason);
                        request_stop(output, bound);
                    },
                }
            } else if matches!(transition.kind, ProgramTransitionKind::Refused(_)) {
                refuse(
                    ui,
                    None,
                    bound.view_epoch,
                    index,
                    bound.refusals[index].clone().unwrap_or_else(|| {
                        "This live sequence cannot play on the active output clock.".into()
                    }),
                );
                request_stop(output, bound);
            }
        }
        let snapshot = output.snapshot().program;
        if snapshot.epoch != bound.epoch {
            return true;
        }
        if let Some(owner) = bound.owner {
            if ui.rehearsal_owner == Some(owner)
                && ui.rehearsal_epoch == bound.view_epoch
                && snapshot.card_index < ui.rehearsal_set().cards.len()
            {
                // Cursor projection is independent of admission. A paused seek
                // can select the next Card before that Card has sounded.
                ui.rehearsal_set_mut().cursor = snapshot.card_index;
            }
            ui.observe_practice_clock(observation(bound.view_epoch, owner, snapshot, rate));
            if snapshot.status == ProgramStatus::Complete {
                ui.practice_program_finished(bound.view_epoch, owner);
            }
        } else {
            ui.stage_clock_event = event_ordinal(snapshot);
            ui.stage_clock_beat_ticks = snapshot.beat_ticks;
            ui.stage_clock_contacts = if snapshot.status == ProgramStatus::Playing {
                ui.stage_clock_event
                    .and_then(|ordinal| {
                        bound.source.cards[0]
                            .events
                            .iter()
                            .find(|event| event.ordinal == ordinal)
                    })
                    .map(|event| event.contacts.clone())
                    .unwrap_or_default()
            } else {
                ui.stage_clock_contacts.clone()
            };
            if let Some(ordinal) = ui.stage_clock_event {
                match bound.live {
                    Some(LiveSequenceKind::Arpeggio) => {
                        ui.stage.arpeggio_step_idx = ordinal as usize
                    },
                    Some(LiveSequenceKind::Exercise) => {
                        ui.stage.exercise_step_idx = ordinal as usize
                    },
                    Some(LiveSequenceKind::ScaleRun) => {
                        ui.stage.scale_run_step = ordinal as usize + 1;
                        ui.stage.scale_run_active = ui.stage_clock_contacts.first().copied();
                    },
                    None => {},
                }
            }
        }
        if matches!(
            snapshot.status,
            ProgramStatus::Stopped | ProgramStatus::Complete
        ) {
            self.active = None;
            false
        } else {
            true
        }
    }
}
