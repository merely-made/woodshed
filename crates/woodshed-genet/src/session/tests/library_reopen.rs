#[test]
fn retained_library_and_overview_reopen_ordered_music_in_a_fresh_process() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["seed", "verify"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::retained_library_receipt_worker",
                "--nocapture",
            ])
            .env("WOODSHED_LIBRARY_WORKER", mode)
            .env(
                "WOODSHED_LIBRARY_EXPECTED",
                dir.path().join("expected-library.json"),
            )
            .env("WOODSHED_DATA_DIR", dir.path())
            .env_remove("WOODSHED_STATE")
            .env_remove("WOODSHED_SETTINGS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("retained-library PASS"));
    }
}

#[test]
fn retained_library_receipt_worker() {
    use woodshed_core::{
        Lens,
        session_overview::{OverviewNodeId, SessionArtifactId},
    };
    use woodshedding::rehearsal::{FretWindow, Hold, ScalePattern};
    let Ok(mode) = std::env::var("WOODSHED_LIBRARY_WORKER") else {
        return;
    };
    let expected_path = std::env::var("WOODSHED_LIBRARY_EXPECTED").unwrap();
    let storage = filesystem_fixture_store();
    if mode == "seed" {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Scales);
        let major = ui
            .stage
            .scales()
            .iter()
            .position(|scale| scale.name == "Major")
            .unwrap();
        ui.stage.select_scale(major);
        ui.stage.set_root(0);
        ui.stage_current(None);
        let source = &mut ui.set.cards[0];
        source.setting.instrument = "Ukulele".into();
        source.setting.tuning = Some("Standard (high-G)".into());
        source.setting.capo = Some(2);
        source.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        source.timing.bpm = Some(83.0);
        source.timing.hold = Hold::Bars(2);
        let source_id = source.id;
        assert!(ui.inspect_scale_pattern(source_id, ScalePattern::Thirds));
        let exercise_id = ui.stage_scale_pattern().unwrap();
        ui.set.select_id(exercise_id);
        ui.activate_workspace_panel(WorkspacePanel::Set);
        let sound = ui.preview_voicing().unwrap();
        assert!(sound.0.windows(2).any(|pair| pair[1] < pair[0]));
        let saved_id = ui.save_working_set().unwrap();
        let saved = serde_json::to_value(ui.retained_sets.get(saved_id).unwrap()).unwrap();
        ui.set.cards[0].label = "Edited working scale".into();
        ui.set.cards[0].timing.bpm = Some(137.0);
        ui.now_ms = Some(1_000);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        let expected = serde_json::json!({ "saved": saved, "saved_id": saved_id, "sound": sound, "working": ui.set });
        std::fs::write(&expected_path, serde_json::to_vec(&expected).unwrap()).unwrap();
        storage.save(&serde_json::to_string(&ui.to_persisted()).unwrap());
        storage.save_settings(&serde_json::to_string(&ui.app_settings).unwrap());
    } else {
        assert_eq!(mode, "verify");
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(expected_path).unwrap()).unwrap();
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Overview));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), expected["working"]);
        assert!(!ui.rehearsal_running && !ui.song_playing && !ui.tuner.enabled);
        assert!(
            woodshed_views::stage::overview_snapshot(&ui)
                .nodes
                .iter()
                .all(|node| !matches!(node.id, OverviewNodeId::Process(_)))
        );
        let saved_id = serde_json::from_value(expected["saved_id"].clone()).unwrap();
        assert_eq!(ui.retained_sets.entries.len(), 1);
        assert_eq!(
            serde_json::to_value(ui.retained_sets.get(saved_id).unwrap()).unwrap(),
            expected["saved"]
        );
        let old_ids = ui.set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        let old_owner = ui.working_sets.active_id;
        ui.inspect_overview_node(OverviewNodeId::Artifact(SessionArtifactId::SavedSet(
            saved_id,
        )));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), expected["working"]);
        ui.open_overview_node(OverviewNodeId::Artifact(SessionArtifactId::SavedSet(
            saved_id,
        )));
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
        assert!(!ui.rehearsal_running);
        assert!(ui.set.cards.iter().all(|card| !old_ids.contains(&card.id)));
        assert_eq!(ui.retained_sets.entries.len(), 1);
        assert_eq!(
            serde_json::to_value(ui.retained_sets.get(saved_id).unwrap()).unwrap(),
            expected["saved"]
        );
        assert_eq!(
            serde_json::to_value(ui.working_sets.get(old_owner, &ui.set).unwrap()).unwrap(),
            expected["working"]
        );
        assert_eq!(ui.set.cards[0].timing.bpm, Some(83.0));
        assert_eq!(ui.set.cursor, 1);
        assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
        let sound: (Vec<f32>, f32, f32) =
            serde_json::from_value(expected["sound"].clone()).unwrap();
        assert_eq!(
            ui.preview_voicing().unwrap(),
            sound,
            "restored copy replays exact ordered repeated music through saved Ukulele/capo setup"
        );
        assert!(ui.pattern_source.is_none());
        assert_eq!(
            ui.practice_history
                .total_practiced_ms("exercise:scale-thirds/v1:Major"),
            0
        );
    }
    println!("retained-library PASS {mode}");
}

#[test]
fn independent_working_sets_and_explorations_reopen_in_a_fresh_process() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["seed", "verify"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::independent_instances_receipt_worker",
                "--nocapture",
            ])
            .env("WOODSHED_INSTANCES_WORKER", mode)
            .env(
                "WOODSHED_INSTANCES_EXPECTED",
                dir.path().join("expected-instances.json"),
            )
            .env("WOODSHED_DATA_DIR", dir.path())
            .env_remove("WOODSHED_STATE")
            .env_remove("WOODSHED_SETTINGS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("independent-instances PASS"));
    }
}

#[test]
fn independent_instances_receipt_worker() {
    use woodshed_core::{Lens, session_overview::OverviewNodeId};
    use woodshedding::rehearsal::{FretWindow, Hold, Material, ScalePattern};
    let Ok(mode) = std::env::var("WOODSHED_INSTANCES_WORKER") else {
        return;
    };
    let expected_path = std::env::var("WOODSHED_INSTANCES_EXPECTED").unwrap();
    let storage = filesystem_fixture_store();
    if mode == "seed" {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Scales);
        let major = ui
            .stage
            .scales()
            .iter()
            .position(|scale| scale.name == "Major")
            .unwrap();
        ui.stage.select_scale(major);
        ui.stage.set_root(0);
        ui.app_settings.fretboard.neck_start = 2;
        ui.app_settings.fretboard.neck_end = Some(14);
        ui.stage_current(None);
        let card = &mut ui.set.cards[0];
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Standard (high-G)".into());
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        card.timing.bpm = Some(83.0);
        card.timing.hold = Hold::Bars(2);
        let source = card.id;
        assert!(ui.inspect_scale_pattern(source, ScalePattern::Thirds));
        let exercise = ui.stage_scale_pattern().unwrap();
        ui.set.select_id(exercise);
        ui.activate_workspace_panel(WorkspacePanel::Set);
        let first = ui.working_sets.active_id;
        let sound_a = ui.preview_voicing().unwrap();
        let snapshot = ui.save_working_set().unwrap();
        let library = serde_json::to_value(&ui.retained_sets).unwrap();
        let exploration_a = ui.catalog_explorations.active_id;
        let config_a = ui.capture_exploration();
        let second = ui.duplicate_working_set();
        ui.set.cursor = 0;
        ui.set.cards[0].label = "Second Set divergent scale".into();
        ui.set.cards[0].setting.capo = Some(4);
        ui.set.cards[0].setting.fret_window = Some(FretWindow { start: 4, span: 8 });
        ui.set.cards[0].timing.bpm = Some(117.0);
        let sound_b = ui.preview_voicing().unwrap();
        assert_ne!(sound_a.0, sound_b.0);
        let exploration_b = ui.create_exploration();
        ui.stage.set_lens(Lens::Chords);
        ui.stage.set_root(9);
        ui.stage.set_tuning(1);
        ui.app_settings.fretboard.neck_start = 5;
        ui.app_settings.fretboard.neck_end = Some(17);
        ui.search = cambium::TextInput::new("Minor");
        let config_b = ui.capture_exploration();
        let set_a = serde_json::to_value(ui.working_sets.get(first, &ui.set).unwrap()).unwrap();
        let set_b = serde_json::to_value(&ui.set).unwrap();
        assert!(matches!(
            ui.set.cards[1].material,
            Material::ScalePattern { .. }
        ));
        assert!(ui.activate_working_set(first));
        ui.now_ms = Some(1_000);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        assert!(ui.activate_working_set(second));
        assert_eq!(ui.rehearsal_owner, Some(first));
        assert_eq!(ui.rehearsal_sounding_pitches().unwrap().unwrap(), sound_a);
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        let expected = serde_json::json!({
            "first":first,"second":second,"exploration_a":exploration_a,"exploration_b":exploration_b,
            "set_a":set_a,"set_b":set_b,"config_a":config_a,"config_b":config_b,
            "sound_a":sound_a,"sound_b":sound_b,"library":library,"snapshot":snapshot,
        });
        std::fs::write(&expected_path, serde_json::to_vec(&expected).unwrap()).unwrap();
        storage.save(&serde_json::to_string(&ui.to_persisted()).unwrap());
        storage.save_settings(&serde_json::to_string(&ui.app_settings).unwrap());
    } else {
        assert_eq!(mode, "verify");
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(expected_path).unwrap()).unwrap();
        let first = serde_json::from_value(expected["first"].clone()).unwrap();
        let second = serde_json::from_value(expected["second"].clone()).unwrap();
        let exploration_a = serde_json::from_value(expected["exploration_a"].clone()).unwrap();
        let exploration_b = serde_json::from_value(expected["exploration_b"].clone()).unwrap();
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Overview));
        assert_eq!(ui.working_sets.active_id, second);
        assert_eq!(ui.catalog_explorations.active_id, exploration_b);
        assert_eq!(ui.working_sets.summaries(&ui.set).len(), 2);
        assert_eq!(ui.catalog_explorations.summaries().len(), 2);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), expected["set_b"]);
        assert_eq!(
            serde_json::to_value(ui.working_sets.get(first, &ui.set).unwrap()).unwrap(),
            expected["set_a"]
        );
        assert_eq!(
            serde_json::to_value(ui.capture_exploration()).unwrap(),
            expected["config_b"]
        );
        assert_eq!(
            serde_json::to_value(&ui.retained_sets).unwrap(),
            expected["library"]
        );
        assert!(!ui.rehearsal_running && ui.rehearsal_owner.is_none());
        assert!(
            woodshed_views::stage::overview_snapshot(&ui)
                .nodes
                .iter()
                .all(|node| !matches!(node.id, OverviewNodeId::Process(_)))
        );
        assert!(ui.activate_working_set(first));
        assert_eq!(ui.set.cursor, 1);
        assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
        let sound_a: (Vec<f32>, f32, f32) =
            serde_json::from_value(expected["sound_a"].clone()).unwrap();
        assert_eq!(ui.preview_voicing().unwrap(), sound_a);
        assert!(ui.activate_exploration(exploration_a));
        assert_eq!(
            serde_json::to_value(ui.capture_exploration()).unwrap(),
            expected["config_a"]
        );
        assert!(ui.activate_exploration(exploration_b));
        assert_eq!(
            serde_json::to_value(ui.capture_exploration()).unwrap(),
            expected["config_b"]
        );
        assert!(ui.activate_working_set(second));
        assert_eq!(ui.set.cursor, 0);
        let sound_b: (Vec<f32>, f32, f32) =
            serde_json::from_value(expected["sound_b"].clone()).unwrap();
        assert_eq!(ui.preview_voicing().unwrap(), sound_b);
        assert_eq!(
            serde_json::to_value(&ui.retained_sets).unwrap(),
            expected["library"]
        );
    }
    println!("independent-instances PASS {mode}");
}
