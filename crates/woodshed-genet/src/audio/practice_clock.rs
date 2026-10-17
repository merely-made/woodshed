//! Product-owned binding between retained practice and the single output clock.
//!
//! Audio receives only a disposable schedule and opaque occurrence tokens. This
//! adapter retains session/source authority and admits observations only after
//! a matching processor transition.

use woodshed_audio::output_runtime::{
    BEAT_TICKS, OutputRuntime, ProgramCard, ProgramDuration, ProgramEnd, ProgramEndReason,
    ProgramEpoch, ProgramEvent, ProgramLoop, ProgramPlan, ProgramPosition, ProgramSnapshot,
    ProgramStatus, ProgramTransitionKind,
};
use woodshed_core::practice_clock::{
    LiveSequenceKind, PracticeCard, PracticeHold, PracticeProgram,
};
use woodshed_core::working_sets::WorkingSetId;
use woodshed_views::stage::{
    PracticeCardEndReason, PracticeClockCommand, PracticeClockObservation, PracticeClockStatus,
    UiState,
};

#[derive(Default)]
pub(super) struct PracticeClockHost {
    active: Option<BoundProgram>,
}

struct BoundProgram {
    session: std::sync::Arc<()>,
    epoch: ProgramEpoch,
    view_epoch: u64,
    owner: Option<woodshed_core::working_sets::WorkingSetId>,
    source: PracticeProgram,
    refusals: Vec<Option<String>>,
    paused: bool,
    retiring: bool,
    live: Option<LiveSequenceKind>,
    loop_mode: ProgramLoop,
}

impl super::CpalBackend {
    /// A UI poll observes the clock and delivers commands; elapsed UI time is
    /// never an input to musical advancement.
    pub(crate) fn sync_practice_clock(&mut self, ui: &mut UiState) -> bool {
        if !ui.practice_clock_managed {
            return false;
        }
        let Some(output) = self.output.as_mut() else {
            if ui.rehearsal_running {
                ui.stop_rehearsal();
                ui.card_timing_notice = Some(
                    "Audio output is unavailable. Run requires an active output clock.".into(),
                );
            }
            stop_live(ui);
            return false;
        };
        self.practice_clock.sync(output, ui)
    }
}

impl PracticeClockHost {
    fn sync(&mut self, output: &mut OutputRuntime, ui: &mut UiState) -> bool {
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

fn observation(
    epoch: u64,
    owner: WorkingSetId,
    snapshot: ProgramSnapshot,
    rate: u32,
) -> PracticeClockObservation {
    PracticeClockObservation {
        epoch,
        owner,
        card_index: snapshot.card_index,
        event_ordinal: event_ordinal(snapshot),
        device_frame: snapshot.device_frame,
        active_frames: snapshot.active_frames,
        sample_rate: rate,
        beat_ticks: snapshot.beat_ticks,
        ticks_per_beat: BEAT_TICKS,
        card_frames: snapshot.card_frames,
        status: match snapshot.status {
            ProgramStatus::Stopped => PracticeClockStatus::Stopped,
            ProgramStatus::Playing => PracticeClockStatus::Playing,
            ProgramStatus::Paused => PracticeClockStatus::Paused,
            ProgramStatus::WaitingManual => PracticeClockStatus::WaitingManual,
            ProgramStatus::Refused => PracticeClockStatus::Refused,
            ProgramStatus::Complete => PracticeClockStatus::Complete,
            ProgramStatus::WaitingObserver => PracticeClockStatus::WaitingObserver,
        },
    }
}

fn event_ordinal(snapshot: ProgramSnapshot) -> Option<u32> {
    snapshot
        .event_token?
        .checked_sub(1)
        .and_then(|token| u32::try_from(token).ok())
}

fn request_stop(output: &mut OutputRuntime, bound: &mut BoundProgram) {
    // A full observer ring may refuse a control command. Release obsolete
    // receipts, then retry on the next poll; never wait for an unaccepted Stop.
    bound.retiring = output.stop_program(bound.epoch).is_ok();
    if !bound.retiring {
        output.drain_program_transitions();
    }
}

fn same_source(a: &PracticeProgram, b: &PracticeProgram) -> bool {
    a.cards.len() == b.cards.len()
        && a.cards.iter().zip(&b.cards).all(|(a, b)| {
            let mut a_card = a.card.clone();
            let mut b_card = b.card.clone();
            a_card.timing.bpm = None;
            b_card.timing.bpm = None;
            serde_json::to_value(a_card).ok() == serde_json::to_value(b_card).ok()
                && a.events == b.events
                && a.hold == b.hold
                && a.unavailable == b.unavailable
        })
}

fn live_kind(ui: &UiState) -> Option<LiveSequenceKind> {
    if ui.stage.arpeggio_playing {
        Some(LiveSequenceKind::Arpeggio)
    } else if ui.stage.exercise_playing {
        Some(LiveSequenceKind::Exercise)
    } else if ui.stage.scale_run_playing {
        Some(LiveSequenceKind::ScaleRun)
    } else {
        None
    }
}

fn stop_live(ui: &mut UiState) {
    ui.stage.arpeggio_playing = false;
    ui.stage.exercise_playing = false;
    ui.stage.scale_run_playing = false;
    ui.stage_clock_contacts.clear();
    ui.stage_clock_event = None;
}

fn refuse(ui: &mut UiState, owner: Option<WorkingSetId>, epoch: u64, index: usize, reason: String) {
    if let Some(owner) = owner {
        let card = ui
            .practice_clock_program()
            .and_then(|source| source.cards.get(index).map(|card| card.card.id.0));
        ui.practice_card_refused(
            epoch,
            owner,
            index,
            format!("Set {} · Card {}: {reason}", owner.0, card.unwrap_or(0)),
        );
    } else {
        stop_live(ui);
        ui.card_timing_notice = Some(reason);
    }
}

/// Device conversion is fallible here; the portable core has no frame limit.
fn units(value: f64, scale: f64, positive: bool) -> Result<u64, String> {
    let scaled = value * scale;
    if !scaled.is_finite() || scaled < 0.0 || scaled >= u64::MAX as f64 {
        return Err("The authored timing cannot be represented by this output clock.".into());
    }
    let rounded = scaled.round() as u64;
    if positive && rounded == 0 {
        return Err("The authored timing is shorter than one output clock unit.".into());
    }
    Ok(rounded)
}

fn lower_card(
    card: &PracticeCard,
    index: usize,
    rate: u32,
    live: bool,
) -> Result<ProgramCard, String> {
    if let Some(reason) = &card.unavailable {
        return Err(reason.clone());
    }
    let rate = f64::from(rate);
    let ticks = BEAT_TICKS as f64;
    let end = if live {
        ProgramEnd::BeatTicks(units(card.events.len() as f64, ticks, true)?)
    } else {
        match card.hold {
            PracticeHold::Beats(beats) => ProgramEnd::BeatTicks(units(beats, ticks, true)?),
            PracticeHold::Seconds(seconds) => ProgramEnd::DeviceFrames(units(seconds, rate, true)?),
            PracticeHold::Manual {
                phrase_beats,
                tail_seconds,
            } => ProgramEnd::Manual {
                preview_ticks: units(phrase_beats, ticks, false)?,
                tail_frames: units(tail_seconds, rate, true)?,
            },
        }
    };
    let mut events = Vec::new();
    let voice_count: usize = if live {
        card.events
            .iter()
            .map(|event| event.pitches.len())
            .max()
            .unwrap_or(1)
    } else {
        card.events.iter().map(|event| event.pitches.len()).sum()
    };
    let amplitude = 0.45 / (voice_count.max(1) as f32).sqrt();
    for event in &card.events {
        let onset = match card.hold {
            PracticeHold::Seconds(_) if !live => {
                ProgramPosition::DeviceFrames(units(event.at, rate, false)?)
            },
            _ => ProgramPosition::BeatTicks(units(event.at, ticks, false)?),
        };
        for (voice, pitch) in event.pitches.iter().enumerate() {
            events.push(ProgramEvent {
                token: u64::from(event.ordinal) + 1,
                onset,
                onset_offset_frames: units(event.strum_seconds * voice as f64, rate, false)?,
                gate: if live {
                    ProgramDuration::BeatTicks(units(0.85, ticks, true)?)
                } else {
                    ProgramDuration::UntilCardEnd
                },
                frequency_hz: f64::from(*pitch),
                amplitude,
            });
        }
    }
    Ok(ProgramCard {
        token: index as u64 + 1,
        bpm: f64::from(card.bpm),
        end,
        events,
    })
}

fn lower_program(
    source: &PracticeProgram,
    start_index: usize,
    loop_mode: ProgramLoop,
    rate: u32,
    live: bool,
) -> (ProgramPlan, Vec<Option<String>>) {
    let mut cards = Vec::with_capacity(source.cards.len());
    let mut refusals = Vec::with_capacity(source.cards.len());
    for (index, card) in source.cards.iter().enumerate() {
        match lower_card(card, index, rate, live) {
            Ok(card) => {
                cards.push(card);
                refusals.push(None);
            },
            Err(reason) => {
                // A failed future Card stays at its original boundary. It does
                // not make preceding playable Cards unavailable or earn credit.
                cards.push(ProgramCard {
                    token: index as u64 + 1,
                    bpm: 120.0,
                    end: ProgramEnd::Refusal(1),
                    events: Vec::new(),
                });
                refusals.push(Some(reason));
            },
        }
    }
    (
        ProgramPlan {
            cards,
            start_index,
            loop_mode,
        },
        refusals,
    )
}

#[cfg(test)]
mod tests {
    use super::super::CpalBackend;
    use super::*;
    use woodshed_core::history::EngagementKind;
    use woodshedding::rehearsal::{CardId, Hold};

    fn fixture(holds: &[Hold]) -> (UiState, CpalBackend) {
        let mut ui = UiState::new();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        let template = ui.set.cards[0].clone();
        ui.set.cards.clear();
        for hold in holds {
            let mut card = template.clone();
            card.id = CardId::UNASSIGNED;
            card.timing.hold = hold.clone();
            card.timing.bpm = Some(120.0);
            ui.set.push(card);
        }
        ui.set.cursor = 0;
        ui.enable_practice_clock();
        (ui, CpalBackend::offline_for_test())
    }

    fn tick(ui: &mut UiState, backend: &mut CpalBackend, frames: usize) {
        backend.sync_practice_clock(ui);
        backend.process_for_test(frames);
        backend.sync_practice_clock(ui);
    }

    fn count(ui: &UiState, kind: EngagementKind) -> usize {
        ui.practice_history
            .recent(100)
            .into_iter()
            .filter(|event| event.kind == kind)
            .count()
    }

    #[test]
    fn card_loop_records_each_device_visit_and_next_selects_manual_without_recrediting_pause() {
        let (mut ui, mut backend) = fixture(&[Hold::Seconds(0.5), Hold::Manual]);
        ui.set.loop_mode = woodshedding::rehearsal::LoopMode::Card;
        let baseline = count(&ui, EngagementKind::Rehearsed);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 60_000);
        let repeated = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(repeated.card_index, 0);
        assert!(count(&ui, EngagementKind::Completed) >= 2);
        assert!(count(&ui, EngagementKind::Rehearsed) >= baseline + 3);

        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 1);
        let before = backend.output.as_ref().unwrap().snapshot().program;
        ui.set.loop_mode = woodshedding::rehearsal::LoopMode::All;
        tick(&mut ui, &mut backend, 200);
        let after = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(after.epoch, before.epoch);
        assert_eq!(after.beat_ticks, before.beat_ticks);
        assert_eq!(after.card_frames, before.card_frames);
        ui.set.loop_mode = woodshedding::rehearsal::LoopMode::Card;
        ui.queue_practice_clock_command(PracticeClockCommand::Advance);
        let starts_before = count(&ui, EngagementKind::Rehearsed);
        tick(&mut ui, &mut backend, 1);
        assert_eq!(ui.set.cursor, 1);
        assert!(ui.rehearsal_paused);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), starts_before);
        assert!(ui.rehearsal_observed_card.is_none());

        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 150_000);
        assert_eq!(
            backend.output.as_ref().unwrap().snapshot().program.status,
            ProgramStatus::WaitingManual
        );
        let waiting_count = ui.practice_history.len();
        tick(&mut ui, &mut backend, 1_000);
        assert_eq!(ui.practice_history.len(), waiting_count);
        ui.queue_practice_clock_command(PracticeClockCommand::Advance);
        tick(&mut ui, &mut backend, 1);
        assert!(!ui.rehearsal_running);
        assert_eq!(
            backend.output.as_ref().unwrap().snapshot().program.status,
            ProgramStatus::Complete
        );
        assert!(ui.practice_history.recent(1)[0].kind == EngagementKind::Completed);
    }

    #[test]
    fn instruction_edit_stops_old_epoch_and_explicit_run_admits_the_corrected_card() {
        let (mut ui, mut backend) = fixture(&[Hold::Bars(2), Hold::Manual]);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 12_000);
        let old_epoch = backend.output.as_ref().unwrap().snapshot().program.epoch;
        let old_card = ui.rehearsal_observed_card.clone().unwrap();
        ui.set.cards[0].touch = woodshedding::rehearsal::Touch::Walk;
        tick(&mut ui, &mut backend, 1_000);
        assert!(!ui.rehearsal_running);
        assert!(
            ui.card_timing_notice
                .as_deref()
                .unwrap()
                .contains("start Run again")
        );
        assert_eq!(count(&ui, EngagementKind::Completed), 0);
        let interrupted = ui.practice_history.recent(1)[0].clone();
        assert_eq!(interrupted.kind, EngagementKind::Rehearsed);
        assert_eq!(
            interrupted.provenance.unwrap().card_snapshot,
            serde_json::to_value(old_card).unwrap()
        );
        tick(&mut ui, &mut backend, 100);
        assert!(
            !ui.rehearsal_running,
            "edits cannot automatically restart the run"
        );
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 1_000);
        assert!(ui.rehearsal_running);
        assert!(backend.output.as_ref().unwrap().snapshot().program.epoch.0 > old_epoch.0);
        assert!(ui.card_timing_notice.is_none());
        assert!(matches!(
            ui.rehearsal_observed_card.as_ref().unwrap().touch,
            woodshedding::rehearsal::Touch::Walk
        ));
        assert_eq!(count(&ui, EngagementKind::Completed), 0);
    }

    #[test]
    fn processor_start_opens_history_pause_resume_and_tempo_preserve_one_span() {
        let (mut ui, mut backend) = fixture(&[Hold::Bars(1)]);
        let before = count(&ui, EngagementKind::Rehearsed);
        ui.toggle_rehearsal();
        backend.sync_practice_clock(&mut ui);
        assert!(ui.rehearsal_pending);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), before);
        tick(&mut ui, &mut backend, 12_000);
        assert!(!ui.rehearsal_pending);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), before + 1);
        let position = backend.output.as_ref().unwrap().snapshot().program;
        assert!(position.event_token.is_some());
        assert!(
            !ui.practice_clock_projection
                .as_ref()
                .unwrap()
                .contacts
                .is_empty()
        );

        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 9_000);
        let paused = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(paused.status, ProgramStatus::Paused);
        assert_eq!(paused.beat_ticks, position.beat_ticks);
        assert_eq!(paused.active_frames, position.active_frames);
        assert_eq!(paused.card_frames, position.card_frames);

        ui.set.cards[0].timing.bpm = Some(240.0);
        tick(&mut ui, &mut backend, 1_000);
        let retimed_pause = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(retimed_pause.beat_ticks, position.beat_ticks);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 12_000);
        let resumed = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(resumed.epoch, position.epoch);
        assert!(
            resumed
                .beat_ticks
                .abs_diff(position.beat_ticks + BEAT_TICKS)
                <= 1
        );
        assert_eq!(resumed.active_frames, 24_000);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), before + 1);
        assert_eq!(count(&ui, EngagementKind::Completed), 0);
    }

    #[test]
    fn real_chord_strum_maps_to_absolute_device_offsets() {
        let (ui, _) = fixture(&[Hold::Bars(1)]);
        let source = ui.stage.practice_program(&ui.set, ui.transport.bpm);
        let card = lower_card(&source.cards[0], 0, 48_000, false).unwrap();
        assert!(card.events.len() > 1);
        assert!(card.events.iter().all(|event| event.token == 1));
        assert_eq!(
            card.events
                .iter()
                .map(|event| event.onset_offset_frames)
                .collect::<Vec<_>>(),
            (0..card.events.len())
                .map(|voice| voice as u64 * 864)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn explicit_next_interrupts_timed_card_and_pause_keeps_next_uncredited_until_resume() {
        let (mut ui, mut backend) = fixture(&[Hold::Bars(1), Hold::Bars(1)]);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 6_000);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 1);
        ui.queue_practice_clock_command(PracticeClockCommand::Advance);
        tick(&mut ui, &mut backend, 1);
        assert_eq!(ui.rehearsal_set().cursor, 1);
        assert!(ui.rehearsal_paused);
        assert_eq!(count(&ui, EngagementKind::Completed), 1);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), 1);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 1);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), 2);
    }

    #[test]
    fn seconds_excludes_pause_and_keeps_duration_after_tempo_edit() {
        let (mut ui, mut backend) = fixture(&[Hold::Seconds(0.5)]);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 6_000);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 3_000);
        ui.set.cards[0].timing.bpm = Some(20.0);
        tick(&mut ui, &mut backend, 1);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 18_001);
        assert!(!ui.rehearsal_running);
        let completed = ui
            .practice_history
            .recent(100)
            .into_iter()
            .find(|event| event.kind == EngagementKind::Completed)
            .unwrap();
        assert_eq!(completed.practiced_ms, Some(500));
    }

    #[test]
    fn manual_waits_for_explicit_next_and_keeps_its_span_open() {
        let (mut ui, mut backend) = fixture(&[Hold::Manual, Hold::Seconds(0.5)]);
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 160_000);
        assert_eq!(ui.rehearsal_set().cursor, 0);
        assert_eq!(
            backend.output.as_ref().unwrap().snapshot().program.status,
            ProgramStatus::WaitingManual
        );
        assert!(ui.rehearsal_observed_card.is_some());
        assert_eq!(count(&ui, EngagementKind::Completed), 0);
        assert_eq!(
            ui.practice_clock_projection.as_ref().unwrap().status,
            PracticeClockStatus::WaitingManual
        );
        ui.queue_practice_clock_command(PracticeClockCommand::Advance);
        tick(&mut ui, &mut backend, 1);
        assert_eq!(ui.rehearsal_set().cursor, 1);
        assert_eq!(count(&ui, EngagementKind::Completed), 1);
        let completed = ui
            .practice_history
            .recent(100)
            .into_iter()
            .find(|event| event.kind == EngagementKind::Completed)
            .unwrap();
        assert!(completed.practiced_ms.unwrap() >= 3_333);
    }

    #[test]
    fn starting_later_with_loop_off_never_plays_earlier_cards() {
        let (mut ui, mut backend) = fixture(&[Hold::Seconds(0.5), Hold::Seconds(0.5)]);
        ui.set.cursor = 1;
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 24_001);
        assert!(!ui.rehearsal_running);
        assert_eq!(count(&ui, EngagementKind::Completed), 1);
        assert_eq!(ui.set.cursor, 1);
    }

    #[test]
    fn future_unavailable_card_refuses_at_boundary_without_credit() {
        let (mut ui, mut backend) = fixture(&[Hold::Seconds(0.5), Hold::Seconds(0.5)]);
        ui.set.cards[1].setting.instrument = "Missing instrument".into();
        ui.toggle_rehearsal();
        tick(&mut ui, &mut backend, 12_000);
        assert!(ui.rehearsal_running);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), 1);
        tick(&mut ui, &mut backend, 12_001);
        assert!(!ui.rehearsal_running);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), 1);
        assert_eq!(count(&ui, EngagementKind::Completed), 1);
        assert!(ui.card_timing_notice.as_ref().unwrap().contains("Card"));
    }

    #[test]
    fn source_edit_or_session_replacement_rejects_queued_started_transition() {
        let (mut ui, mut backend) = fixture(&[Hold::Bars(1)]);
        ui.toggle_rehearsal();
        backend.sync_practice_clock(&mut ui);
        backend.process_for_test(1);
        ui.set.cards[0].label = "Edited before observing start".into();
        backend.sync_practice_clock(&mut ui);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), 0);
        assert!(!ui.rehearsal_running);

        let (mut ui, mut backend) = fixture(&[Hold::Bars(1)]);
        ui.toggle_rehearsal();
        backend.sync_practice_clock(&mut ui);
        backend.process_for_test(1);
        let persisted = ui.to_persisted();
        ui.apply_persisted(&persisted, ui.app_settings.clone());
        backend.sync_practice_clock(&mut ui);
        assert_eq!(count(&ui, EngagementKind::Rehearsed), 0);
        assert!(!ui.rehearsal_running);
        assert!(ui.practice_clock_projection.is_none());
    }

    #[test]
    fn live_stage_projection_and_pause_resume_follow_processor_occurrences() {
        let mut ui = UiState::new();
        ui.enable_practice_clock();
        ui.stage.set_lens(woodshed_core::Lens::Exercises);
        ui.stage.exercise_playing = true;
        let expected = ui
            .stage
            .live_clock_card(LiveSequenceKind::Exercise, ui.transport.bpm);
        let mut backend = CpalBackend::offline_for_test();
        tick(&mut ui, &mut backend, 24_001);
        assert_eq!(ui.stage_clock_event, Some(1));
        assert_eq!(ui.stage_clock_contacts, expected.events[1].contacts);
        assert_eq!(ui.stage.exercise_step_idx, 1);
        let before = backend.output.as_ref().unwrap().snapshot().program;
        ui.stage.exercise_playing = false;
        tick(&mut ui, &mut backend, 10_000);
        let paused = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(paused.status, ProgramStatus::Paused);
        assert_eq!(paused.beat_ticks, before.beat_ticks);
        ui.stage.exercise_playing = true;
        tick(&mut ui, &mut backend, 24_000);
        let resumed = backend.output.as_ref().unwrap().snapshot().program;
        assert_eq!(resumed.epoch, before.epoch);
        assert_eq!(ui.stage_clock_event, Some(2));
        assert_eq!(ui.stage_clock_contacts, expected.events[2].contacts);
        assert!(
            ui.practice_history.is_empty(),
            "live Stage is not a Set rehearsal"
        );
    }
}
