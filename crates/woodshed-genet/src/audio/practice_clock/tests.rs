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
