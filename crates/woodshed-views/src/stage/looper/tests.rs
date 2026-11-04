use super::*;
use woodshed_core::Lens;
use woodshedding::rehearsal::{Hold, Set};

fn ready_ui() -> UiState {
    let mut ui = UiState::new();
    ui.song.bars.push(Default::default());
    ui.stage.set_lens(Lens::Chords);
    let mut card = ui.stage.card_from_lens().unwrap();
    card.timing.hold = Hold::Bars(2);
    ui.set.push(card);
    ui.loop_working_set(ui.working_sets.active_id);
    ui
}

#[test]
fn wav_export_controls_require_host_collision_token_and_reset_with_session() {
    use cambium_genet_winit_host::Harness;
    use taproot::Selector;

    let mut ui = ready_ui();
    ui.select_app_section(woodshed_core::storage::AppSection::Looper);
    ui.set_viewport_width(1100.0);
    ui.set_viewport_height(900.0);
    let logic: Box<dyn FnMut(&UiState) -> UiChild> = Box::new(super::super::stage_root);
    let mut h = Harness::new(crate::theme::slate_stage_css(), ui, logic);
    h.layout_at(1100.0, 900.0);
    let export = Selector::class("t-btn").containing("Export WAV");
    let replace = Selector::class("t-btn").containing("Replace WAV");
    assert!(!h.click_on(&replace));
    assert!(h.click_on(&export));
    let _ = h.click_on(&export);
    assert_eq!(
        h.state().loop_export_requests,
        [LoopExportRequest::ChooseDestination]
    );

    h.update(|ui| {
        ui.loop_export_requests.clear();
        ui.loop_export_busy = true;
        ui.loop_export_notice = Some("Choosing WAV destination…".into());
    });
    let _ = h.click_on(&export);
    assert!(h.state().loop_export_requests.is_empty());
    h.update(|ui| {
        ui.loop_export_busy = false;
        ui.loop_export_replace = Some((73, "practice.wav".into()));
        ui.loop_export_notice = Some("Choose Replace WAV to replace the existing file.".into());
    });
    let _ = h.click_on(&export);
    assert!(h.state().loop_export_requests.is_empty());
    assert!(h.click_on(&replace));
    assert_eq!(
        h.state().loop_export_requests,
        [LoopExportRequest::Replace(73)]
    );
    let _ = h.click_on(&replace);
    assert_eq!(h.state().loop_export_requests.len(), 1);

    h.update(|ui| {
        ui.loop_export_requests.clear();
        ui.loop_export_replace = Some((91, "second.wav".into()));
    });
    assert!(h.click_on(&Selector::class("t-btn").containing("Cancel")));
    assert_eq!(
        h.state().loop_export_requests,
        [LoopExportRequest::CancelReplace(91)]
    );
    h.update(|ui| {
        ui.loop_export_requests.clear();
        ui.loop_export_replace = None;
        ui.loop_export_notice = Some("Export cancelled.".into());
    });
    assert!(h.click_on(&export));
    assert_eq!(
        h.state().loop_export_requests,
        [LoopExportRequest::ChooseDestination]
    );

    h.update(|ui| {
        ui.loop_export_busy = true;
        ui.loop_export_replace = Some((117, "never-persisted.wav".into()));
        ui.loop_export_notice = Some("Write refused; previous WAV retained.".into());
        let persisted = ui.to_persisted();
        let json = serde_json::to_value(&persisted).unwrap();
        for key in [
            "loop_export_requests",
            "loop_export_busy",
            "loop_export_notice",
            "loop_export_replace",
        ] {
            assert!(json.get(key).is_none());
        }
        ui.apply_persisted(&persisted, Default::default());
    });
    assert!(h.state().loop_export_requests.is_empty());
    assert!(!h.state().loop_export_busy);
    assert!(h.state().loop_export_notice.is_none());
    assert!(h.state().loop_export_replace.is_none());
}

#[test]
fn source_stays_with_its_set_while_exploration_and_rehearsal_choose_other_owners() {
    let mut ui = ready_ui();
    let owner = ui.working_sets.active_id;
    let plan = ui.requested_loop_plan().unwrap();
    let card = plan.segments[0].id;
    let mut other = Set::default();
    let mut copy = ui.set.cards[0].clone();
    copy.label = "other material".into();
    other.push(copy);
    let other_owner = ui.working_sets.create(&mut ui.set, "Other", other);
    ui.rehearsal_owner = Some(other_owner);
    assert_eq!(ui.requested_loop_plan().unwrap(), plan);
    ui.open_loop_card(card);
    assert_eq!(ui.working_sets.active_id, owner);
    assert_eq!(ui.set.cursor_id(), Some(card.card));
    assert_eq!(ui.rehearsal_owner, Some(other_owner));
    assert_eq!(ui.looper.source, LooperSource::Set(owner));
}

#[test]
fn selection_and_queued_capture_keep_identity_across_reorder() {
    let mut ui = ready_ui();
    let mut second = ui.set.cards[0].clone();
    second.label = "second".into();
    ui.set.push(second);
    ui.loop_admitted = Some(ui.requested_loop_plan().unwrap());
    let id = ui.loop_admitted.as_ref().unwrap().segments[0].id;
    ui.loop_selected = Some(id);
    ui.request(AudioRequest::LoopClear(id));
    ui.set.cards.reverse();
    ui.loop_admitted = Some(ui.requested_loop_plan().unwrap());
    assert_eq!(ui.selected_loop_segment(), Some(id));
    assert_eq!(ui.loop_admitted.as_ref().unwrap().index_of(id), Some(1));
    assert_eq!(ui.audio_requests, vec![AudioRequest::LoopClear(id)]);
}

#[test]
fn persisted_source_reopens_without_copying_or_revoicing_legacy_song() {
    let mut ui = ready_ui();
    ui.looper.beats = 7;
    ui.looper.inherited_bpm = 137.25;
    ui.song.bars[0].pitches_hz = vec![253.125, 381.777];
    ui.song.bars[0].beats = 3;
    ui.song.bars[0].length = 4;
    let expected_song = ui.song.clone();
    let expected_plan = ui.requested_loop_plan().unwrap();
    let json = serde_json::to_string(&ui.to_persisted()).unwrap();
    let session: woodshed_core::storage::PersistedSession = serde_json::from_str(&json).unwrap();
    let mut reopened = UiState::new();
    reopened.apply_persisted(&session, Default::default());
    assert_eq!(reopened.looper, ui.looper);
    assert_eq!(reopened.requested_loop_plan().unwrap(), expected_plan);
    assert_eq!(reopened.song, expected_song);
    assert!(reopened.loop_admitted.is_none());
    assert!(!reopened.song_playing);
    let mut old = serde_json::to_value(&session).unwrap();
    old.as_object_mut().unwrap().remove("looper");
    let legacy = serde_json::from_value(old).unwrap();
    reopened.apply_persisted(&legacy, Default::default());
    assert_eq!(reopened.looper.source, LooperSource::SavedSong);
    assert_eq!(reopened.song, expected_song);
    assert!(
        serde_json::to_value(reopened.to_persisted())
            .unwrap()
            .get("looper")
            .is_none()
    );
}
