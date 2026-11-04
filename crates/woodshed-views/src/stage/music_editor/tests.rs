use super::*;
fn fixture(rhythm: bool) -> UiState {
    let mut ui = UiState::new();
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    ui.music_example(rhythm);
    ui
}
#[test]
fn scope_set_switch_clears_owner_gestures_without_stopping_bound_practice() {
    let mut ui = fixture(false);
    let owner = ui.working_sets.active_id;
    ui.practice_music();
    assert!(ui.music_practice_requested());
    let run_epoch = ui.music_editor.practice.run_epoch();
    let other = ui
        .working_sets
        .create(&mut ui.set, "Other Set", Default::default());
    assert!(ui.working_sets.activate(owner, &mut ui.set));
    ui.preview_music_pattern(pattern::PickingPattern::EighthUpDown);
    ui.music_editor.active_tone = Some(FrettedNote { string: 1, fret: 3 });
    assert!(ui.music_preview_active());
    assert!(ui.activate_working_set_for_scope(other));
    assert_eq!(ui.working_sets.active_id, other);
    assert_eq!(ui.music_editor.practice.owner(), Some(owner));
    assert_eq!(ui.music_editor.practice.run_epoch(), run_epoch);
    assert!(ui.music_practice_requested());
    assert!(!ui.music_preview_active());
    assert_eq!(ui.music_editor.active_tone, None);
}
#[test]
fn musical_edits_share_one_history_across_views_save_and_reopen() {
    let mut ui = fixture(true);
    ui.save_music();
    let original = ui.set.music.clone();
    let reading = ui.music_draft().unwrap().score.clone();
    ui.set_music_view(NotationView::Tab);
    ui.select_music(1, 0);
    assert!(!ui.music_draft().unwrap().dirty());
    assert!(!ui.music_draft().unwrap().can_undo());
    assert_eq!(ui.set.music, original);
    ui.select_music(0, 0);
    ui.music_fret(2, 3);
    ui.music_split(true);
    ui.music_rest();
    assert!(ui.music_draft().unwrap().score.measures[0].events[0].rest);
    assert_eq!(
        ui.music_draft().unwrap().score.measures[0].events[0]
            .notes
            .len(),
        0
    );
    ui.music_undo();
    assert!(!ui.music_draft().unwrap().score.measures[0].events[0].rest);
    ui.music_redo();
    ui.save_music();
    let saved = ui.set.music.clone().unwrap();
    assert!(!ui.music_draft().unwrap().dirty());
    ui.music_undo();
    assert!(ui.music_draft().unwrap().dirty());
    ui.music_redo();
    assert!(!ui.music_draft().unwrap().dirty());
    assert_eq!(ui.set.music.as_ref(), Some(&saved));
    assert_ne!(saved.score().unwrap(), reading);
    let session = ui.to_persisted();
    let mut reopened = UiState::new();
    reopened.apply_persisted(&session, ui.app_settings.clone());
    reopened.activate_workspace_panel(WorkspacePanel::Arrangement);
    reopened.sync_music_editor();
    assert_eq!(reopened.set.music, Some(saved.clone()));
    assert_eq!(
        reopened.music_draft().unwrap().score,
        saved.score().unwrap()
    );
}
#[test]
fn symbol_is_authored_and_suggestions_do_not_rewrite_it() {
    let mut ui = fixture(false);
    let written = ui.music_draft().unwrap().score.measures[0].harmony.clone();
    ui.music_fret(6, 2);
    assert_eq!(ui.music_draft().unwrap().score.measures[0].harmony, written);
    assert!(ui.music_draft().unwrap().score.chord_mismatch(0));
    let notes = ui.music_draft().unwrap().score.measures[0].events.clone();
    ui.open_music_chord();
    ui.music_editor.chord_root.selected = 7;
    ui.music_editor.chord_kind.selected = 2;
    ui.apply_music_chord();
    assert_eq!(
        ui.music_draft().unwrap().score.measures[0]
            .harmony
            .as_ref()
            .unwrap()
            .label(),
        "G7"
    );
    assert_eq!(ui.music_draft().unwrap().score.measures[0].events, notes);
}
#[test]
fn each_set_has_its_own_draft_and_normal_session_storage() {
    let mut ui = fixture(false);
    let first = ui.working_sets.active_id;
    ui.save_music();
    let first_xml = ui.set.music.clone();
    ui.music_fret(1, 2);
    let first_reading = ui.music_draft().unwrap().score.clone();
    let second = ui.create_working_set();
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    ui.music_example(true);
    ui.save_music();
    let second_xml = ui.set.music.clone();
    ui.activate_working_set(first);
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    assert_eq!(ui.music_draft().unwrap().score, first_reading);
    assert_eq!(ui.set.music, first_xml);
    ui.save_music();
    let encoded = serde_json::to_string(&ui.to_persisted()).unwrap();
    let session = woodshed_core::storage::decode_session(&encoded)
        .unwrap()
        .session;
    assert_eq!(
        session
            .working_sets
            .get(second, &session.set)
            .unwrap()
            .music,
        second_xml
    );
}
#[test]
fn reopening_a_session_clears_previous_owner_drafts() {
    let mut ui = fixture(true);
    ui.save_music();
    let session = ui.to_persisted();
    ui.music_fret(1, 5);
    assert!(ui.arrangement_drafts_dirty());
    ui.apply_persisted(&session, ui.app_settings.clone());
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    ui.sync_music_editor();
    assert!(!ui.music_draft().unwrap().dirty());
    assert_eq!(
        ui.music_draft().unwrap().score,
        session.set.music.unwrap().score().unwrap()
    );
}
#[test]
fn invalid_pitch_and_export_do_not_change_saved_source() {
    let mut ui = fixture(true);
    ui.save_music();
    let saved = ui.set.music.clone();
    let before = ui.music_draft().unwrap().score.clone();
    ui.music_fret(50, 0);
    assert_eq!(ui.music_draft().unwrap().score, before);
    assert!(!ui.music_draft().unwrap().can_undo());
    ui.music_fret(2, 3);
    ui.export_music();
    let xml = &ui.music_editor.export_request.as_ref().unwrap().1;
    assert_eq!(
        MusicDocument {
            musicxml: xml.clone()
        }
        .score()
        .unwrap(),
        ui.music_draft().unwrap().score
    );
    assert_eq!(ui.set.music, saved);
}
#[test]
fn new_exercise_respects_explicit_meter_and_selected_tuning() {
    let mut ui = UiState::new();
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    ui.transport.bpm = 120.0;
    ui.new_music_exercise(6, 8);
    let score = &ui.music_draft().unwrap().score;
    assert_eq!(score.tempo, 120);
    assert_eq!(
        score.measures[0].meter,
        TimeSignature {
            beats: 6,
            beat_type: 8
        }
    );
    assert_eq!(score.measures[0].events.len(), 6);
    assert!(
        score.measures[0]
            .events
            .iter()
            .all(|e| e.rest && e.notes.is_empty())
    );
    assert_eq!(
        score.tuning,
        ui.stage
            .tuning()
            .strings
            .iter()
            .rev()
            .map(|p| p.midi() as u8)
            .collect::<Vec<_>>()
    );
}
#[test]
fn collected_and_duplicated_sets_keep_independent_music() {
    let mut ui = fixture(false);
    ui.save_music();
    let source = ui.set.clone();
    let mut copy = woodshedding::rehearsal::Set::default();
    copy.copy_instructions_from(&source).unwrap();
    assert_eq!(copy.music, source.music);
    copy.music.as_mut().unwrap().musicxml.push(' ');
    assert_ne!(copy.music, source.music);
}
