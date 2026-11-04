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
    music: super::music_clock::MusicClockHost,
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
            if ui.music_practice_requested() {
                ui.stop_music_practice();
                ui.music_editor.notice = Some(
                    "Audio output is unavailable. Practice requires an active output clock.".into(),
                );
            }
            return false;
        };
        self.practice_clock.sync(output, ui)
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
#[path = "practice_clock/tests.rs"]
mod tests;

#[path = "practice_clock/operations.rs"]
mod operations;
