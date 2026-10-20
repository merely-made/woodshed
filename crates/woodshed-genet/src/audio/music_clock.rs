//! Native custody of a score-derived run on the shared device sample clock.
use std::{collections::BTreeMap, sync::Arc};
use woodshed_audio::output_runtime::{OutputRuntime, ProgramEpoch, ProgramLoop, ProgramStatus};
use woodshed_core::working_sets::WorkingSetId;
use woodshed_views::stage::{
    UiState,
    music_editor::practice::{PracticeObservation, PracticeStatus},
};
use woodshedding::notation::Score;

#[derive(Default)]
pub(super) struct MusicClockHost {
    active: Option<BoundMusic>,
}
struct BoundMusic {
    owner: WorkingSetId,
    session: Arc<()>,
    run: u64,
    epoch: ProgramEpoch,
    paused: bool,
    retiring: bool,
    submitted: BTreeMap<u64, Score>,
    last_revision: u64,
    tempo: f64,
    repeat: ProgramLoop,
}
impl MusicClockHost {
    pub(super) fn active(&self) -> bool {
        self.active.is_some()
    }
    pub(super) fn sync(&mut self, output: &mut OutputRuntime, ui: &mut UiState) -> bool {
        if let Some(bound) = self.active.as_mut() {
            let desired = ui.music_practice_requested()
                && ui.music_editor.practice.owner() == Some(bound.owner)
                && ui.working_sets.active_id == bound.owner
                && ui.music_editor.practice.run_epoch() == bound.run
                && Arc::ptr_eq(&bound.session, &ui.audio_session);
            if !desired {
                bound.retiring = true;
            }
            if bound.retiring {
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
                    let _ = output.stop_program(bound.epoch);
                    return true;
                }
            }
        }
        if self.active.is_none() {
            if !ui.music_practice_requested() {
                return false;
            }
            // A preceding preview/backing stop must reach the processor first.
            if output.preview_busy()
                || output.snapshot().preview_voices != 0
                || output.snapshot().playing
                || output.song().playing
            {
                return true;
            }
            output.drain_program_transitions();
            let practice = &ui.music_editor.practice;
            let Some(owner) = practice.owner() else {
                return false;
            };
            let Some(schedule) = practice.desired_schedule() else {
                return false;
            };
            let repeat = repeat_mode(ui);
            let plan = super::music_program::lower_program(schedule, repeat).map(|mut plan| {
                if repeat == ProgramLoop::Card {
                    plan.start_index = ui.music_draft().map(|d| d.selected.0).unwrap_or(0);
                }
                plan
            });
            let prepared = plan.and_then(|plan| prepare_checked(output, plan));
            let epoch = match prepared
                .and_then(|prepared| output.start_program(prepared).map_err(|e| e.to_string()))
            {
                Ok(epoch) => epoch,
                Err(reason) => {
                    fail(ui, reason);
                    return false;
                },
            };
            self.active = Some(BoundMusic {
                owner,
                session: ui.audio_session.clone(),
                run: practice.run_epoch(),
                epoch,
                paused: false,
                retiring: false,
                submitted: BTreeMap::from([(0, practice.desired_score().unwrap().clone())]),
                last_revision: 0,
                tempo: schedule.tempo_bpm,
                repeat,
            });
        }
        let bound = self.active.as_mut().unwrap();
        // Capture every accepted visible musical edit; rapid edits coalesce in
        // the processor while its current bar keeps its original schedule.
        let edited = ui
            .music_practice_score_ref()
            .filter(|score| ui.music_editor.practice.desired_score() != Some(*score))
            .cloned();
        if let Some(score) = edited {
            if let Err(reason) = ui.music_editor.practice.request_update(&score) {
                ui.music_editor.notice = Some(format!("Playback update unavailable: {reason}"));
            }
        }
        let tempo = ui.music_practice_tempo();
        if tempo != bound.tempo {
            match output.set_program_tempo_all(bound.epoch, tempo) {
                Ok(()) => match ui.music_editor.practice.set_tempo(tempo) {
                    Ok(()) => bound.tempo = tempo,
                    Err(reason) => {
                        fail(ui, reason);
                        bound.retiring = true;
                        return true;
                    },
                },
                Err(reason) => {
                    ui.music_editor.notice = Some(format!("Practice tempo waiting: {reason}"))
                },
            }
        }
        let snapshot = output.snapshot().program;
        if snapshot.epoch == bound.epoch {
            if let Some(score) = bound.submitted.get(&snapshot.revision) {
                if ui.music_editor.practice.applied_revision() != snapshot.revision {
                    if let Err(reason) = ui.music_editor.practice.acknowledge_revision(
                        snapshot.revision,
                        score,
                        bound.tempo,
                    ) {
                        fail(ui, reason);
                        bound.retiring = true;
                        return true;
                    }
                }
                bound
                    .submitted
                    .retain(|revision, _| *revision >= snapshot.revision);
            }
        }
        let practice = &ui.music_editor.practice;
        if practice.desired_revision() > bound.last_revision && bound.submitted.len() < 128 {
            let revision = practice.desired_revision();
            let plan = super::music_program::lower_program(
                practice.desired_schedule().unwrap(),
                repeat_mode(ui),
            );
            let submitted = plan
                .and_then(|plan| prepare_checked(output, plan))
                .and_then(|prepared| {
                    output
                        .replace_program_at_boundary(bound.epoch, revision, prepared)
                        .map_err(|e| e.to_string())
                });
            match submitted {
                Ok(()) => {
                    bound
                        .submitted
                        .insert(revision, practice.desired_score().unwrap().clone());
                    bound.last_revision = revision;
                },
                Err(reason) => {
                    ui.music_editor.notice = Some(format!("Playback update waiting: {reason}"))
                },
            }
        }
        let paused = ui.music_editor.practice.status() == PracticeStatus::Paused;
        if paused != bound.paused {
            let result = if paused {
                output.pause_program(bound.epoch)
            } else {
                output.resume_program(bound.epoch)
            };
            if result.is_ok() {
                bound.paused = paused;
            }
        }
        let repeat = repeat_mode(ui);
        if bound.repeat != repeat {
            if output.set_program_loop(bound.epoch, repeat).is_ok() {
                bound.repeat = repeat;
            }
        }
        // Draining releases the engine's bounded observation fence. These are
        // music positions, so no Card practice-history event is fabricated.
        let refused = output.drain_program_transitions().iter().any(|t| {
            t.epoch == bound.epoch
                && matches!(
                    t.kind,
                    woodshed_audio::output_runtime::ProgramTransitionKind::Refused(_)
                )
        });
        if refused {
            fail(ui, "The output clock refused this arrangement.".into());
            bound.retiring = true;
            return true;
        }
        let snapshot = output.snapshot().program;
        if snapshot.epoch != bound.epoch {
            return true;
        }
        let status = match snapshot.status {
            ProgramStatus::Complete => PracticeStatus::Complete,
            ProgramStatus::Stopped => PracticeStatus::Stopped,
            ProgramStatus::Paused => PracticeStatus::Paused,
            _ => {
                if bound.paused {
                    PracticeStatus::Paused
                } else {
                    PracticeStatus::Playing
                }
            },
        };
        ui.music_editor.practice.observe(&PracticeObservation {
            owner: bound.owner,
            session: bound.session.clone(),
            run_epoch: bound.run,
            applied_revision: snapshot.revision,
            bar_index: snapshot.card_index,
            bar_beat_ticks: snapshot.beat_ticks,
            status,
        });
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
fn fail(ui: &mut UiState, reason: String) {
    ui.music_editor.practice.stop();
    ui.music_editor.notice = Some(reason);
}

fn repeat_mode(ui: &UiState) -> ProgramLoop {
    match ui.set.loop_mode {
        woodshedding::rehearsal::LoopMode::Off => ProgramLoop::Once,
        woodshedding::rehearsal::LoopMode::All => ProgramLoop::Set,
        woodshedding::rehearsal::LoopMode::Card => ProgramLoop::Card,
    }
}

fn prepare_checked(
    output: &OutputRuntime,
    plan: woodshed_audio::output_runtime::ProgramPlan,
) -> Result<woodshed_audio::output_runtime::PreparedProgram, String> {
    let count = plan.cards.len();
    let prepared = output.prepare_program(plan).map_err(|e| e.to_string())?;
    for bar in 0..count {
        if let Some(reason) = prepared.refusal_reason(bar) {
            return Err(format!("Bar {} cannot play: {reason}", bar + 1));
        }
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::CpalBackend;
    use woodshedding::rehearsal::LoopMode;

    fn fixture() -> (UiState, CpalBackend) {
        let mut ui = UiState::new();
        ui.enable_practice_clock();
        ui.music_example(false);
        ui.set.loop_mode = LoopMode::Off;
        ui.set_music_practice_tempo(120.0);
        (ui, CpalBackend::offline_for_test())
    }
    fn pump(ui: &mut UiState, backend: &mut CpalBackend, frames: usize) {
        let mut left = frames;
        while left > 0 {
            backend.sync_practice_clock(ui);
            let count = left.min(1024);
            backend.process_for_test(count);
            backend.sync_practice_clock(ui);
            left -= count;
        }
    }
    #[test]
    fn unsaved_music_drives_clock_without_saving_or_fabricating_card_history() {
        let (mut ui, mut backend) = fixture();
        let before = ui.music_draft().unwrap().score.clone();
        let cards = serde_json::to_value(&ui.set.cards).unwrap();
        let history = ui.practice_history.len();
        assert!(ui.set.music.is_none());
        ui.practice_music();
        pump(&mut ui, &mut backend, 48_000);
        assert_eq!(ui.music_editor.practice.status(), PracticeStatus::Playing);
        assert_eq!(ui.music_editor.practice.playhead().unwrap().bar_index, 0);
        assert_eq!(ui.music_editor.practice.applied_score(), Some(&before));
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(ui.set.music.is_none());
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), cards);
        assert_eq!(ui.practice_history.len(), history);
        pump(&mut ui, &mut backend, 336_001);
        assert_eq!(ui.music_editor.practice.status(), PracticeStatus::Complete);
        assert!(!ui.music_practice_requested());
    }
    #[test]
    fn native_music_edits_apply_at_next_bar_and_pause_holds_clock() {
        let (mut ui, mut backend) = fixture();
        ui.practice_music();
        pump(&mut ui, &mut backend, 24_000);
        let old = ui.music_editor.practice.applied_score().unwrap().clone();
        ui.music_fret(6, 1);
        backend.sync_practice_clock(&mut ui);
        assert!(ui.music_editor.practice.pending_update());
        assert_eq!(ui.music_editor.practice.applied_score(), Some(&old));
        ui.pause_music_practice();
        pump(&mut ui, &mut backend, 1);
        let paused = backend.output_snapshot().unwrap().program;
        pump(&mut ui, &mut backend, 24_000);
        let still = backend.output_snapshot().unwrap().program;
        assert_eq!(paused.beat_ticks, still.beat_ticks);
        assert_eq!(still.revision, 0);
        assert_eq!(ui.music_editor.practice.status(), PracticeStatus::Paused);
        ui.resume_music_practice();
        pump(&mut ui, &mut backend, 72_001);
        assert_eq!(ui.music_editor.practice.status(), PracticeStatus::Playing);
        assert_eq!(ui.music_editor.practice.playhead().unwrap().bar_index, 1);
        assert_eq!(ui.music_editor.practice.applied_revision(), 1);
        assert!(!ui.music_editor.practice.pending_update());
        assert_eq!(
            ui.music_editor.practice.applied_score(),
            Some(&ui.music_draft().unwrap().score)
        );
    }
    #[test]
    fn adapter_refuses_large_start_without_changing_the_editable_music() {
        let (mut ui, mut backend) = fixture();
        let owner = ui.working_sets.active_id;
        let score = &mut ui.music_editor.drafts.get_mut(&owner).unwrap().score;
        score.measures = vec![score.measures[0].clone(); 257];
        let before = score.clone();
        ui.practice_music();
        assert!(ui.music_practice_requested()); // portable timing accepts the music
        pump(&mut ui, &mut backend, 1024);
        assert_eq!(ui.music_editor.practice.status(), PracticeStatus::Stopped);
        assert!(ui.music_editor.notice.as_ref().unwrap().contains("256"));
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(ui.set.music.is_none());
    }
    #[test]
    fn adapter_refuses_large_update_keeps_playing_and_accepts_a_later_edit() {
        let (mut ui, mut backend) = fixture();
        ui.practice_music();
        pump(&mut ui, &mut backend, 1024);
        let original = ui.music_draft().unwrap().score.clone();
        let epoch = backend.output_snapshot().unwrap().program.epoch;
        let owner = ui.working_sets.active_id;
        ui.music_editor
            .drafts
            .get_mut(&owner)
            .unwrap()
            .score
            .measures = vec![original.measures[0].clone(); 257];
        pump(&mut ui, &mut backend, 96_000);
        assert!(ui.music_editor.practice.pending_update());
        assert!(ui.music_editor.notice.as_ref().unwrap().contains("256"));
        let active = backend.output_snapshot().unwrap().program;
        assert_eq!(active.epoch, epoch);
        assert_eq!(active.revision, 0);
        assert_eq!(active.status, ProgramStatus::Playing);
        assert_eq!(ui.music_editor.practice.applied_score(), Some(&original));

        ui.music_editor.drafts.get_mut(&owner).unwrap().score = original;
        ui.music_fret(6, 1);
        pump(&mut ui, &mut backend, 96_000);
        assert_eq!(backend.output_snapshot().unwrap().program.epoch, epoch);
        assert_eq!(ui.music_editor.practice.applied_revision(), 2);
        assert!(!ui.music_editor.practice.pending_update());
        assert_eq!(
            ui.music_editor.practice.applied_score(),
            Some(&ui.music_draft().unwrap().score)
        );
    }
    #[test]
    fn practice_tempo_and_shared_bar_repeat_preserve_authored_music() {
        let (mut ui, mut backend) = fixture();
        let written = ui.music_draft().unwrap().score.tempo;
        ui.select_music(2, 0);
        ui.set.loop_mode = LoopMode::Card;
        ui.practice_music();
        pump(&mut ui, &mut backend, 48_000);
        assert_eq!(ui.music_editor.practice.playhead().unwrap().bar_index, 2);
        ui.set_music_practice_tempo(60.0);
        pump(&mut ui, &mut backend, 1);
        assert_eq!(
            f64::from_bits(backend.output_snapshot().unwrap().program.bpm_bits),
            60.0
        );
        assert_eq!(ui.music_draft().unwrap().score.tempo, written);
        pump(&mut ui, &mut backend, 144_001);
        assert_eq!(ui.music_editor.practice.playhead().unwrap().bar_index, 2);
        ui.match_music_tempo();
        pump(&mut ui, &mut backend, 1);
        assert_eq!(
            ui.music_editor.practice.tempo_bpm(),
            Some(f64::from(written))
        );
        assert!(ui.set.music.is_none());
    }
    #[test]
    fn replacement_session_and_owner_do_not_admit_old_music_observations() {
        let (mut ui, mut backend) = fixture();
        ui.practice_music();
        pump(&mut ui, &mut backend, 1);
        let old_epoch = backend.output_snapshot().unwrap().program.epoch;
        ui.audio_session = Arc::new(());
        ui.stop_music_practice();
        ui.practice_music();
        pump(&mut ui, &mut backend, 4096);
        let now = backend.output_snapshot().unwrap().program;
        assert_ne!(old_epoch, now.epoch);
        assert_eq!(ui.music_editor.practice.status(), PracticeStatus::Playing);
        ui.leave_music_owner();
        pump(&mut ui, &mut backend, 4096);
        assert_eq!(
            backend.output_snapshot().unwrap().program.status,
            ProgramStatus::Stopped
        );
        assert_eq!(ui.music_editor.practice.playhead(), None);
    }
    #[test]
    fn legacy_set_practice_remains_available_and_music_handoff_keeps_one_lane() {
        let (mut ui, mut backend) = fixture();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.set.cards[0].timing.hold = woodshedding::rehearsal::Hold::Bars(1);
        ui.toggle_rehearsal();
        pump(&mut ui, &mut backend, 4096);
        let legacy = backend.output_snapshot().unwrap().program.epoch;
        assert!(ui.rehearsal_running);
        ui.practice_music();
        pump(&mut ui, &mut backend, 4096);
        assert!(!ui.rehearsal_running);
        assert!(ui.music_practice_requested());
        assert_ne!(backend.output_snapshot().unwrap().program.epoch, legacy);
        ui.toggle_rehearsal();
        pump(&mut ui, &mut backend, 4096);
        assert!(!ui.music_practice_requested());
        assert!(ui.rehearsal_running);
        assert_eq!(ui.music_editor.practice.playhead(), None);
    }
}
