#[test]
fn scale_pattern_session_replays_ordered_visits_in_a_fresh_process() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["seed", "verify"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::scale_pattern_session_receipt_worker",
                "--nocapture",
            ])
            .env("WOODSHED_PATTERN_SESSION_WORKER", mode)
            .env(
                "WOODSHED_PATTERN_EXPECTED",
                dir.path().join("expected-patterns.json"),
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
        assert!(String::from_utf8_lossy(&output.stdout).contains("scale-pattern-session PASS"));
    }
}

#[test]
fn scale_pattern_session_receipt_worker() {
    use woodshed_core::{Lens, history::catalog_id_for_card, storage::AppSection};
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{FretWindow, Hold, Material, ScalePattern},
    };
    let Ok(mode) = std::env::var("WOODSHED_PATTERN_SESSION_WORKER") else {
        return;
    };
    let expected_path = std::env::var("WOODSHED_PATTERN_EXPECTED").unwrap();
    let storage = filesystem_fixture_store();
    if mode == "seed" {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Scales);
        ui.stage.set_root(3);
        ui.root_dd.selected = 3;
        let major = ui
            .stage
            .scales()
            .iter()
            .position(|scale| scale.name == "Major")
            .unwrap();
        ui.stage.select_scale(major);
        ui.stage_current(None);
        let source = ui.set.cards[0].id;
        let card = &mut ui.set.cards[0];
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Standard (high-G)".into());
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        card.timing.bpm = Some(90.0);
        card.timing.hold = Hold::Bars(1);
        let source_before = serde_json::to_value(card).unwrap();
        let mut sounds = Vec::new();
        let mut subjects = Vec::new();
        for (index, pattern) in [ScalePattern::Thirds, ScalePattern::Fourths]
            .into_iter()
            .enumerate()
        {
            assert!(ui.inspect_scale_pattern(source, pattern));
            ui.hear_scale_pattern();
            assert_eq!(ui.set.cards.len(), index + 1);
            let id = ui.stage_scale_pattern().unwrap();
            ui.set.select_id(id);
            ui.select_app_section(AppSection::Rehearsal);
            sounds.push(ui.preview_voicing().unwrap());
            let subject = catalog_id_for_card(ui.set.card(id).unwrap()).unwrap();
            assert_ne!(subject, "scale:Major");
            subjects.push(subject);
            ui.now_ms = Some(1_000 + index as u64 * 10_000);
            ui.toggle_rehearsal();
            crate::tests::acknowledge_card_preview_fixture(&mut ui);
            ui.now_ms = Some(2_500 + index as u64 * 10_000);
            ui.toggle_rehearsal();
            crate::tests::acknowledge_card_preview_fixture(&mut ui);
        }
        assert_eq!(
            serde_json::to_value(&ui.set.cards[0]).unwrap(),
            source_before
        );
        let ids = ui.set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        assert_eq!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
            3
        );
        std::fs::write(
            &expected_path,
            serde_json::to_vec(&serde_json::json!({
                "source": source_before, "ids": ids, "sounds": sounds, "subjects": subjects,
            }))
            .unwrap(),
        )
        .unwrap();
        storage.save(&serde_json::to_string(&ui.to_persisted()).unwrap());
        storage.save_settings(&serde_json::to_string(&ui.app_settings).unwrap());
    } else {
        assert_eq!(mode, "verify");
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(expected_path).unwrap()).unwrap();
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.set.cards.len(), 3);
        assert!(!ui.rehearsal_running);
        assert_eq!(ui.stage.string_count(), 6);
        assert_eq!(
            serde_json::to_value(&ui.set.cards[0]).unwrap(),
            expected["source"]
        );
        assert_eq!(
            serde_json::to_value(ui.set.cards.iter().map(|card| card.id).collect::<Vec<_>>())
                .unwrap(),
            expected["ids"]
        );
        for (index, (pattern, prefix)) in [
            (ScalePattern::Thirds, [62, 66, 64, 67, 66, 69]),
            (ScalePattern::Fourths, [62, 67, 64, 69, 66, 71]),
        ]
        .into_iter()
        .enumerate()
        {
            ui.set.cursor = index + 1;
            let card = &ui.set.cards[index + 1];
            assert!(
                matches!(&card.material, Material::ScalePattern { name, root, pattern: saved } if name == "Major" && *root == PitchClass::new(0) && *saved == pattern)
            );
            assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
            let sounding = ui.preview_voicing().unwrap();
            let expected_sound: (Vec<f32>, f32, f32) =
                serde_json::from_value(expected["sounds"][index].clone()).unwrap();
            assert_eq!(
                sounding, expected_sound,
                "fresh-process playback must retain the full ordered repeated sequence at its native f32 precision"
            );
            let midi = sounding
                .0
                .iter()
                .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
                .collect::<Vec<_>>();
            assert_eq!(&midi[..6], &prefix);
            let subject = catalog_id_for_card(card).unwrap();
            assert_eq!(subject, expected["subjects"][index].as_str().unwrap());
            assert_eq!(ui.practice_history.total_practiced_ms(&subject), 1_500);
            let observation = ui
                .practice_history
                .recent(20)
                .into_iter()
                .find(|event| event.subject_id == subject && event.practiced_ms == Some(1_500))
                .unwrap();
            let provenance = observation.provenance.unwrap();
            assert_eq!(provenance.occurrence_id, card.id);
            assert_eq!(
                provenance.card_snapshot,
                serde_json::to_value(card).unwrap()
            );
        }
        assert_eq!(ui.practice_history.total_practiced_ms("scale:Major"), 0);
    }
    println!("scale-pattern-session PASS {mode}");
}

#[test]
fn chord_approach_session_replays_saved_target_shapes_in_a_fresh_process() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["seed", "verify"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::chord_approach_session_receipt_worker",
                "--nocapture",
            ])
            .env("WOODSHED_APPROACH_SESSION_WORKER", mode)
            .env(
                "WOODSHED_APPROACH_EXPECTED",
                dir.path().join("expected-approaches.json"),
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
        assert!(String::from_utf8_lossy(&output.stdout).contains("chord-approach-session PASS"));
    }
}

#[test]
fn chord_approach_session_receipt_worker() {
    use woodshed_core::{Lens, history::catalog_id_for_card, storage::AppSection};
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{ApproachDirection, CardId, FretWindow, Hold, Material},
    };
    let Ok(mode) = std::env::var("WOODSHED_APPROACH_SESSION_WORKER") else {
        return;
    };
    let expected_path = std::env::var("WOODSHED_APPROACH_EXPECTED").unwrap();
    let storage = filesystem_fixture_store();
    if mode == "seed" {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Chords);
        ui.stage.set_root(0);
        ui.root_dd.selected = 0;
        let minor_seven = ui
            .stage
            .chords()
            .iter()
            .position(|chord| chord.name == "Minor 7")
            .unwrap();
        ui.stage.select_chord(minor_seven);
        ui.stage_current(None);
        let source = &mut ui.set.cards[0];
        source.setting.instrument = "Ukulele".into();
        source.setting.tuning = Some("Standard (high-G)".into());
        source.setting.capo = Some(2);
        source.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        source.timing.bpm = Some(90.0);
        source.timing.hold = Hold::Bars(1);
        let source_id = source.id;
        let mut target = source.clone();
        target.id = CardId::UNASSIGNED;
        target.label = "Cmaj7 target".into();
        target.material = Material::Chord {
            name: "Major 7".into(),
            root: PitchClass::new(0),
        };
        ui.set.push(target);
        ui.set.cursor = 1;
        ui.step_card_shape(1);
        let target_id = ui.set.cards[1].id;
        assert!(ui.set.cards[1].setting.voicing_fingerprint.is_some());
        let originals = serde_json::to_value(&ui.set.cards).unwrap();
        let mut sounds = Vec::new();
        let mut subjects = Vec::new();
        for (index, direction) in [ApproachDirection::Below, ApproachDirection::Above]
            .into_iter()
            .enumerate()
        {
            assert!(ui.inspect_chord_approach(source_id, target_id, direction));
            ui.hear_chord_approach();
            assert_eq!(ui.set.cards.len(), index + 2);
            let id = ui.stage_chord_approach().unwrap();
            ui.set.select_id(id);
            ui.select_app_section(AppSection::Rehearsal);
            sounds.push(ui.preview_voicing().unwrap());
            let subject = catalog_id_for_card(ui.set.card(id).unwrap()).unwrap();
            assert_ne!(subject, "chord:Major 7");
            subjects.push(subject);
            ui.now_ms = Some(1_000 + index as u64 * 10_000);
            ui.toggle_rehearsal();
            crate::tests::acknowledge_card_preview_fixture(&mut ui);
            ui.now_ms = Some(2_500 + index as u64 * 10_000);
            ui.toggle_rehearsal();
            crate::tests::acknowledge_card_preview_fixture(&mut ui);
        }
        assert_eq!(serde_json::to_value(&ui.set.cards[..2]).unwrap(), originals);
        let ids = ui.set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        assert_eq!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
            4
        );
        std::fs::write(
            &expected_path,
            serde_json::to_vec(&serde_json::json!({
                "originals": originals, "ids": ids, "sounds": sounds, "subjects": subjects,
            }))
            .unwrap(),
        )
        .unwrap();
        storage.save(&serde_json::to_string(&ui.to_persisted()).unwrap());
        storage.save_settings(&serde_json::to_string(&ui.app_settings).unwrap());
    } else {
        assert_eq!(mode, "verify");
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(expected_path).unwrap()).unwrap();
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.set.cards.len(), 4);
        assert!(!ui.rehearsal_running);
        assert_eq!(ui.stage.string_count(), 6);
        assert_eq!(
            serde_json::to_value(&ui.set.cards[..2]).unwrap(),
            expected["originals"]
        );
        assert_eq!(
            serde_json::to_value(ui.set.cards.iter().map(|card| card.id).collect::<Vec<_>>())
                .unwrap(),
            expected["ids"]
        );
        for (index, (direction, shift)) in [
            (ApproachDirection::Below, -1),
            (ApproachDirection::Above, 1),
        ]
        .into_iter()
        .enumerate()
        {
            ui.set.cursor = index + 2;
            let card = &ui.set.cards[index + 2];
            assert!(
                matches!(&card.material, Material::ChordApproach { name, root, direction: saved } if name == "Major 7" && *root == PitchClass::new(0) && *saved == direction)
            );
            assert_eq!(
                card.setting.voicing_fingerprint,
                ui.set.cards[1].setting.voicing_fingerprint
            );
            assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
            let sounding = ui.preview_voicing().unwrap();
            let expected_sound: (Vec<f32>, f32, f32) =
                serde_json::from_value(expected["sounds"][index].clone()).unwrap();
            assert_eq!(
                sounding, expected_sound,
                "fresh-process playback must replay every saved shape approach pair in order"
            );
            let midi = sounding
                .0
                .iter()
                .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
                .collect::<Vec<_>>();
            assert!(!midi.is_empty());
            assert!(midi.chunks_exact(2).all(|pair| pair[0] - pair[1] == shift));
            let subject = catalog_id_for_card(card).unwrap();
            assert_eq!(subject, expected["subjects"][index].as_str().unwrap());
            assert_eq!(ui.practice_history.total_practiced_ms(&subject), 1_500);
            let observation = ui
                .practice_history
                .recent(20)
                .into_iter()
                .find(|event| event.subject_id == subject && event.practiced_ms == Some(1_500))
                .unwrap();
            let provenance = observation.provenance.unwrap();
            assert_eq!(provenance.occurrence_id, card.id);
            assert_eq!(
                provenance.card_snapshot,
                serde_json::to_value(card).unwrap()
            );
        }
        assert_eq!(ui.practice_history.total_practiced_ms("chord:Major 7"), 0);
        assert_eq!(ui.practice_history.total_practiced_ms("chord:Minor 7"), 0);
        ui.set.cards[3].setting.voicing_fingerprint = Some("stale-test-shape".into());
        ui.set.cursor = 3;
        assert!(
            ui.preview_voicing().unwrap().0.is_empty(),
            "an unavailable saved shape must not fall back to a formula preview after reopen"
        );
    }
    println!("chord-approach-session PASS {mode}");
}
