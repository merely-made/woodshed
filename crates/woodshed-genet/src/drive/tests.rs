use super::*;

#[test]
fn audio_observation_refreshes_availability_and_clears_unobserved_tuner_readings() {
    let mut backend = crate::audio::CpalBackend::offline_for_test();
    let mut ui = UiState::new();
    ui.tuner.enabled = true;
    ui.tuner.reading = Some(woodshed_core::audio::TunerReading {
        note: "A".into(),
        octave: 4,
        cents: 0.0,
        in_tune: true,
        level: 0.1,
    });
    let observation = observe_audio(&mut backend);
    assert!(observation.changed(&ui));
    observation.apply(&mut ui);
    assert_eq!(
        ui.audio_availability.output,
        woodshed_core::audio::AudioStreamStatus::Offline
    );
    assert_eq!(
        ui.audio_availability.input,
        woodshed_core::audio::AudioStreamStatus::Unopened
    );
    assert!(ui.tuner.enabled && !ui.tuner_is_listening());
    assert!(ui.tuner.reading.is_none());
    assert!(!observe_audio(&mut backend).changed(&ui));
}

#[test]
fn accepted_one_shot_status_stops_the_ui_without_restarting_or_retargeting_backing() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.set.cards[0].timing.hold = woodshedding::rehearsal::Hold::Bars(1);
    ui.looper.inherited_bpm = 300.0;
    ui.looper.beats = 1;
    ui.looper.one_shot = true;
    ui.looper.click = false;
    ui.loop_working_set(ui.working_sets.active_id);
    let plan = ui.requested_loop_plan().unwrap();
    ui.loop_admitted = Some(plan.clone());
    let payload = serde_json::to_value(ui.to_persisted()).unwrap();
    let mut backend = crate::audio::CpalBackend::offline_for_test();
    backend.set_loop_plan(&plan).unwrap();
    backend.set_song_transport(true);
    // Before a graph block, queued Play remains the accepted control fact.
    observe_audio(&mut backend).apply(&mut ui);
    assert!(ui.song_playing);
    for _ in 0..3 {
        backend.process_for_test(4096);
    }
    // An unrelated dispatch may arrive before any wake/frame publication.
    // Its stale true is not an explicit new Play request.
    backend.set_song_transport(ui.song_playing);
    assert!(!backend.song_playing());
    let stopped = observe_audio(&mut backend);
    assert!(stopped.changed(&ui));
    stopped.apply(&mut ui);
    assert!(!ui.song_playing);
    // Ordinary dispatch now sees false rather than reissuing stale Play.
    backend.set_song_transport(ui.song_playing);
    backend.process_for_test(256);
    assert!(!backend.song_playing());
    // After stopped status has reached the UI, a new Play is authoritative.
    backend.set_song_transport(true);
    assert!(backend.song_playing());
    backend.process_for_test(256);
    assert!(backend.song_playing());
    assert_eq!(ui.loop_admitted, Some(plan));
    assert_eq!(serde_json::to_value(ui.to_persisted()).unwrap(), payload);
    assert!(backend.capture_source_rate().is_none());
}

#[derive(Default)]
struct RecordedAudio {
    previews: Vec<(Vec<f32>, f32, f32)>,
    admissions: usize,
    refuse_on_admission: Option<usize>,
}

impl CardPreviewAdmission for RecordedAudio {
    fn admit_card(
        &mut self,
        preview: &woodshed_core::card_timing::CardPreview,
    ) -> Result<(), String> {
        self.admissions += 1;
        if self.refuse_on_admission == Some(self.admissions) {
            return Err("PCM frame budget exceeded".into());
        }
        self.preview_pitches(&preview.0, preview.1, preview.2);
        Ok(())
    }
}
impl AudioBackend for RecordedAudio {
    fn set_metronome(&mut self, _: woodshed_core::audio::TransportState) {}
    fn set_tuner_enabled(&mut self, _: bool) {}
    fn tuner_reading(&self) -> Option<woodshed_core::audio::TunerReading> {
        None
    }
    fn set_song(&mut self, _: &woodshed_core::song::SongDoc) {}
    fn set_song_transport(&mut self, _: bool) {}
    fn song_rewind(&mut self) {}
    fn song_bar(&self) -> Option<usize> {
        None
    }
    fn error(&self) -> Option<&str> {
        None
    }
    fn preview_pitches(&mut self, pitches: &[f32], duration: f32, strum: f32) {
        self.previews.push((pitches.to_vec(), duration, strum));
    }
}

#[test]
fn background_dwell_keeps_its_owner_clock_cursor_and_inherited_sound() {
    use woodshedding::rehearsal::{Hold, MarkMode, Set};
    let mut ui = UiState::new();
    ui.now_ms = Some(1_000);
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.stage_current(None);
    for card in &mut ui.set.cards {
        card.timing.hold = Hold::Seconds(0.5);
        // Both absent identities inherit the runner's Stage setup. An
        // explicit Guitar with no tuning selects that instrument's default.
        card.setting.instrument.clear();
        card.setting.tuning = None;
        card.setting.mark_mode = MarkMode::Solo;
        card.setting.marked = vec![(0, 0)];
    }
    let owner = ui.working_sets.active_id;
    ui.toggle_rehearsal();
    crate::tests::acknowledge_card_preview_fixture(&mut ui);
    let expected_sound = ui.rehearsal_sounding_pitches().unwrap().unwrap();
    let observation_started = ui.card_started_ms;
    let observations = ui.practice_history.len();
    let began = std::time::Instant::now();
    let mut last = Some(began);
    let mut instruction = None;
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    last = Some(began);

    // A different editor owner may have the same local CardId; neither its
    // edits nor its visible setup may alter the running owner's dwell.
    let mut other = Set::default();
    let mut card = ui.set.cards[0].clone();
    card.label = "Foreground instruction".into();
    other.push(card);
    ui.working_sets.create(&mut ui.set, "Other Set", other);
    ui.set.cards[0].timing.bpm = Some(137.0);
    let drop_d = woodshed_core::tunings()
        .iter()
        .position(|t| t.name == "Drop D")
        .unwrap();
    ui.stage.set_tuning(drop_d);
    ui.tuning_dd.selected = drop_d;
    ui.now_ms = Some(1_500);
    ui.sync();
    assert_eq!(ui.card_started_ms, observation_started);
    assert_eq!(ui.practice_history.len(), observations);
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    assert_eq!(last, Some(began));
    assert_eq!(ui.rehearsal_owner, Some(owner));
    assert_eq!(
        ui.rehearsal_sounding_pitches().unwrap().unwrap(),
        expected_sound
    );
    assert_ne!(
        ui.stage
            .card_sounding_pitches_at_tempo(ui.rehearsal_card().unwrap(), ui.transport.bpm)
            .unwrap(),
        expected_sound,
    );

    ui.now_ms = Some(1_600);
    let mut backend = RecordedAudio::default();
    assert!(rehearsal_dwell(
        &mut last,
        &mut ui,
        &mut backend,
        began + std::time::Duration::from_millis(600),
    ));
    assert_eq!(ui.rehearsal_set().cursor, 1);
    assert_eq!(ui.set.cursor, 0);
    assert_eq!(ui.set.cards[0].label, "Foreground instruction");
    assert_eq!(backend.previews, vec![expected_sound]);
    assert!(ui.rehearsal_running);

    ui.now_ms = Some(2_200);
    assert!(!rehearsal_dwell(
        &mut last,
        &mut ui,
        &mut backend,
        began + std::time::Duration::from_millis(1200),
    ));
    assert!(!ui.rehearsal_running);
    assert_eq!(ui.set.cursor, 0);
    assert_eq!(backend.previews.len(), 1);
}

#[test]
fn refused_next_card_stops_at_that_owner_card_without_credit_then_retry_is_admitted() {
    use woodshedding::rehearsal::Hold;

    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.stage_current(None);
    for card in &mut ui.set.cards {
        card.timing.hold = Hold::Seconds(0.5);
    }
    ui.now_ms = Some(1_000);
    let owner = ui.working_sets.active_id;
    let first = ui.set.cards[0].id;
    let rejected = ui.set.cards[1].id;
    let initial_history = ui.practice_history.len();
    let mut backend = RecordedAudio {
        refuse_on_admission: Some(2),
        ..RecordedAudio::default()
    };
    let began = std::time::Instant::now();
    let mut last = None;

    ui.toggle_rehearsal();
    assert!(rehearsal_dwell(&mut last, &mut ui, &mut backend, began));
    assert_eq!(ui.rehearsal_owner, Some(owner));
    assert_eq!(ui.rehearsal_set().cursor, 0);
    assert!(!ui.rehearsal_pending);
    assert_eq!(backend.admissions, 1);
    assert_eq!(ui.practice_history.len(), initial_history + 1);

    // Completing the admitted first Card advances to the second. Its
    // preview is refused, so the run stops on that Card and opens no
    // observation span or practice event for it.
    ui.now_ms = Some(1_600);
    assert!(!rehearsal_dwell(
        &mut last,
        &mut ui,
        &mut backend,
        began + std::time::Duration::from_millis(600),
    ));
    assert!(!ui.rehearsal_running);
    assert!(!ui.rehearsal_pending);
    assert_eq!(ui.rehearsal_owner, None);
    assert_eq!(ui.set.cursor, 1);
    assert_eq!(ui.set.cards[ui.set.cursor].id, rejected);
    assert_eq!(ui.practice_history.len(), initial_history + 2);
    assert!(ui.practice_history.recent(20).iter().any(|event| {
        event
            .provenance
            .as_ref()
            .is_some_and(|p| p.occurrence_id == first)
            && event.kind == woodshed_core::history::EngagementKind::Completed
            && event.provenance.as_ref().and_then(|p| p.working_set_id) == Some(owner)
    }));
    assert!(!ui.practice_history.recent(20).iter().any(|event| {
        event
            .provenance
            .as_ref()
            .is_some_and(|p| p.occurrence_id == rejected)
            && event.kind.is_practice()
    }));
    assert!(
        ui.card_timing_notice
            .as_deref()
            .unwrap()
            .contains("PCM frame budget exceeded")
    );

    // Retrying the same owner and current Card after the transient host
    // refusal succeeds; it does not skip ahead or inherit a foreign Set.
    ui.now_ms = Some(2_000);
    last = None;
    ui.toggle_rehearsal();
    assert_eq!(ui.rehearsal_owner, Some(owner));
    assert!(rehearsal_dwell(
        &mut last,
        &mut ui,
        &mut backend,
        began + std::time::Duration::from_secs(1),
    ));
    assert!(ui.rehearsal_running);
    assert!(!ui.rehearsal_pending);
    assert_eq!(ui.rehearsal_owner, Some(owner));
    assert_eq!(ui.set.cursor, 1);
    assert_eq!(ui.set.cards[ui.set.cursor].id, rejected);
    assert_eq!(backend.admissions, 3);
    let retried = ui
        .practice_history
        .recent(20)
        .into_iter()
        .find(|event| {
            event
                .provenance
                .as_ref()
                .is_some_and(|p| p.occurrence_id == rejected)
                && event.kind.is_practice()
        })
        .expect("successful retry opens an observation for the current Card");
    assert_eq!(
        retried.kind,
        woodshed_core::history::EngagementKind::Rehearsed
    );
    assert_eq!(retried.provenance.unwrap().working_set_id, Some(owner));
}

#[test]
fn manual_hold_waits_without_advancing_or_repeating_admission() {
    use woodshedding::rehearsal::Hold;

    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.set.cards[0].timing.hold = Hold::Manual;
    ui.now_ms = Some(1_000);
    let owner = ui.working_sets.active_id;
    let card = ui.set.cards[0].id;
    let mut backend = RecordedAudio::default();
    let began = std::time::Instant::now();
    let mut last = None;

    ui.toggle_rehearsal();
    assert!(rehearsal_dwell(&mut last, &mut ui, &mut backend, began));
    let history_after_admission = ui.practice_history.len();
    ui.now_ms = Some(61_000);
    assert!(rehearsal_dwell(
        &mut last,
        &mut ui,
        &mut backend,
        began + std::time::Duration::from_secs(60),
    ));
    assert!(ui.rehearsal_running);
    assert_eq!(ui.rehearsal_owner, Some(owner));
    assert_eq!(ui.set.cursor, 0);
    assert_eq!(ui.set.cards[0].id, card);
    assert_eq!(backend.admissions, 1);
    assert_eq!(ui.practice_history.len(), history_after_admission);
    assert_eq!(last, None, "Manual has no timed dwell clock");
}

#[test]
fn dwell_restarts_after_instruction_edits_cursor_changes_and_pause() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.stage_current(None);
    ui.rehearsal_running = true;
    let mut last = None;
    let mut instruction = None;
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    let started = std::time::Instant::now();
    last = Some(started);
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    assert_eq!(last, Some(started));
    ui.set.cards[0].timing.bpm = Some(72.0);
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    assert!(last.is_none());
    last = Some(started);
    ui.set.cursor = 1;
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    assert!(last.is_none());
    last = Some(started);
    ui.transport.bpm = 96.0;
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    assert!(last.is_none());
    last = Some(started);
    ui.rehearsal_running = false;
    refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
    assert!(last.is_none());
    assert!(instruction.is_none());
}
