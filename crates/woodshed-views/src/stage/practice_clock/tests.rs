use super::super::UiState;
use super::*;
use woodshedding::rehearsal::Set;
use woodshedding::rehearsal::{Hold, Material, Timing};

fn managed_ui() -> UiState {
    let mut ui = UiState::new();
    ui.practice_clock_managed = true;
    ui.stage_current(None);
    // These fixtures isolate Run admission from the preceding Stage action.
    ui.practice_history = Default::default();
    ui.set.cards[0].timing = Timing {
        bpm: None,
        hold: Hold::Bars(1),
    };
    ui
}

#[test]
fn managed_run_freezes_exact_owner_program_and_tempo_retime_preserves_source() {
    let mut ui = managed_ui();
    let card_id = ui.set.cards[0].id;
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;
    assert!(ui.rehearsal_running && ui.rehearsal_pending);
    assert_eq!(
        ui.rehearsal_clock_program.as_ref().unwrap().cards[0]
            .card
            .id,
        card_id
    );
    assert!(ui.live_practice_program_for_host().is_ok());

    ui.transport.bpm = 400.0;
    assert!(ui.live_practice_program_for_host().is_ok());
    assert_eq!(ui.rehearsal_epoch, epoch);
    assert!(ui.rehearsal_clock_program.is_some());
}

#[test]
fn started_only_opens_history_and_pause_resume_preserves_the_same_observation() {
    let mut ui = managed_ui();
    ui.now_ms = Some(100);
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;
    let owner = ui.rehearsal_owner.unwrap();
    let started = PracticeClockObservation {
        epoch,
        owner,
        card_index: 0,
        event_ordinal: Some(0),
        device_frame: 4800,
        active_frames: 0,
        sample_rate: 48_000,
        beat_ticks: 0,
        ticks_per_beat: 960,
        card_frames: 0,
        status: PracticeClockStatus::Playing,
    };
    assert!(ui.practice_history.is_empty());
    assert!(ui.practice_card_started(started.clone()));
    assert_eq!(ui.practice_history.len(), 1);
    let open = ui.rehearsal_observed_card.as_ref().unwrap().id;
    ui.toggle_rehearsal();
    assert!(ui.rehearsal_paused);
    assert_eq!(ui.rehearsal_epoch, epoch);
    assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, open);
    ui.toggle_rehearsal();
    assert!(!ui.rehearsal_paused);
    assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, open);
    assert!(!ui.practice_card_started(started));
}

#[test]
fn pause_intent_during_startup_does_not_discard_a_real_started_transition() {
    let mut ui = managed_ui();
    ui.now_ms = Some(100);
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;
    let owner = ui.rehearsal_owner.unwrap();
    ui.toggle_rehearsal();
    assert!(ui.rehearsal_paused);
    let started = PracticeClockObservation {
        epoch,
        owner,
        card_index: 0,
        event_ordinal: None,
        device_frame: 4800,
        active_frames: 0,
        sample_rate: 48_000,
        beat_ticks: 0,
        ticks_per_beat: 960,
        card_frames: 0,
        status: PracticeClockStatus::Paused,
    };
    assert!(ui.practice_card_started(started));
    assert!(ui.rehearsal_paused);
    assert_eq!(ui.practice_history.len(), 1);
}

#[test]
fn nonzero_start_cursor_keeps_original_program_order_and_card_tokens() {
    let mut ui = managed_ui();
    let first = ui.set.cards[0].clone();
    ui.set.push(first);
    let second_id = ui.set.cards[1].id;
    ui.set.cursor = 1;
    ui.toggle_rehearsal();
    let program = ui.practice_clock_program().unwrap();
    assert_eq!(program.cards[0].card.id, ui.set.cards[0].id);
    assert_eq!(program.cards[1].card.id, second_id);
    assert_eq!(ui.practice_clock_start_index(), 1);

    let owner = ui.rehearsal_owner.unwrap();
    let epoch = ui.rehearsal_epoch;
    assert!(ui.practice_card_started(PracticeClockObservation {
        epoch,
        owner,
        card_index: 1,
        event_ordinal: None,
        device_frame: 0,
        active_frames: 0,
        sample_rate: 48_000,
        beat_ticks: 0,
        ticks_per_beat: 960,
        card_frames: 0,
        status: PracticeClockStatus::Playing,
    }));
    assert_eq!(ui.rehearsal_set().cursor, 1);
    assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, second_id);
}

#[test]
fn refusal_selects_the_failed_card_for_retry_without_crediting_it() {
    let mut ui = managed_ui();
    let first = ui.set.cards[0].clone();
    ui.set.push(first);
    let failed = ui.set.cards[1].id;
    let owner = ui.working_sets.active_id;
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;

    assert!(ui.practice_card_refused(epoch, owner, 1, "unsupported Card".into()));
    assert_eq!(ui.rehearsal_set().cursor, 1);
    assert!(!ui.rehearsal_running);
    assert!(ui.practice_history.is_empty());
    let notice = ui.card_timing_notice.as_deref().unwrap();
    assert!(notice.contains(&owner.0.to_string()));
    assert!(notice.contains(&failed.0.to_string()));

    ui.toggle_rehearsal();
    assert_eq!(ui.practice_clock_start_index(), 1);
    assert_eq!(
        ui.practice_clock_program().unwrap().cards[1].card.id,
        failed
    );
}

#[test]
fn parked_owner_refusal_keeps_its_cursor_without_switching_sets() {
    let mut ui = managed_ui();
    let first = ui.set.cards[0].clone();
    ui.set.push(first);
    let failed = ui.set.cards[1].id;
    let owner = ui.working_sets.active_id;
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;
    ui.create_working_set_from("Other", Set::default());
    let selected = ui.working_sets.active_id;

    assert!(ui.practice_card_refused(epoch, owner, 1, "unsupported Card".into()));
    assert_eq!(ui.working_sets.active_id, selected);
    assert_eq!(ui.working_sets.get(owner, &ui.set).unwrap().cursor, 1);
    let notice = ui.card_timing_notice.as_deref().unwrap();
    assert!(notice.contains(&failed.0.to_string()));
    assert!(notice.contains("Select this Set to retry"));
}

#[test]
fn frozen_background_owner_survives_exploration_of_another_set() {
    let mut ui = managed_ui();
    let owner = ui.working_sets.active_id;
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;
    let card_id = ui.rehearsal_clock_program.as_ref().unwrap().cards[0]
        .card
        .id;

    ui.create_working_set_from("Other", Set::default());
    ui.stage.root_idx = (ui.stage.root_idx + 1) % woodshed_core::ROOT_NAMES.len();
    assert_ne!(ui.working_sets.active_id, owner);
    assert!(ui.rehearsal_running);
    assert!(ui.live_practice_program_for_host().is_ok());
    assert!(ui.practice_card_started(PracticeClockObservation {
        epoch,
        owner,
        card_index: 0,
        event_ordinal: None,
        device_frame: 0,
        active_frames: 0,
        sample_rate: 48_000,
        beat_ticks: 0,
        ticks_per_beat: 960,
        card_frames: 0,
        status: PracticeClockStatus::Playing,
    }));
    assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, card_id);
    assert_eq!(ui.rehearsal_set().cursor, 0);
}

#[test]
fn persisted_session_clears_clock_but_keeps_host_managed_mode() {
    let mut ui = managed_ui();
    ui.enable_practice_clock();
    ui.toggle_rehearsal();
    ui.rehearsal_paused = true;
    let saved = ui.to_persisted();
    let app_settings = ui.app_settings.clone();

    ui.apply_persisted(&saved, app_settings);

    assert!(ui.practice_clock_managed);
    assert!(!ui.rehearsal_running);
    assert!(!ui.rehearsal_paused);
    assert!(ui.rehearsal_clock_program.is_none());
    assert!(ui.practice_clock_projection.is_none());
    assert!(ui.practice_clock_commands.is_empty());
}

#[test]
fn changed_card_source_refuses_stale_started_transition_without_crediting_it() {
    let mut ui = managed_ui();
    ui.toggle_rehearsal();
    let epoch = ui.rehearsal_epoch;
    let owner = ui.rehearsal_owner.unwrap();
    ui.set.cards[0].material = Material::Riff {
        name: "changed".into(),
    };
    let observation = PracticeClockObservation {
        epoch,
        owner,
        card_index: 0,
        event_ordinal: Some(0),
        device_frame: 10,
        active_frames: 0,
        sample_rate: 48_000,
        beat_ticks: 0,
        ticks_per_beat: 960,
        card_frames: 0,
        status: PracticeClockStatus::Playing,
    };
    assert!(!ui.practice_card_started(observation));
    assert!(ui.practice_history.is_empty());
}
