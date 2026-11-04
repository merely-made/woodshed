/// The production filesystem backend and restore path, exercised in two
/// processes so an in-memory roundtrip cannot satisfy reopening by accident.
#[test]
fn connected_session_reopens_in_a_fresh_process() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["seed", "verify"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::connected_session_receipt_worker",
                "--nocapture",
            ])
            .env("WOODSHED_CONNECTED_WORKER", mode)
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
        assert!(String::from_utf8_lossy(&output.stdout).contains("connected-session PASS"));
    }
}

#[test]
fn connected_session_receipt_worker() {
    use woodshed_core::{Lens, storage::AppSection};
    use woodshedding::rehearsal::{Hold, Touch};
    let Ok(mode) = std::env::var("WOODSHED_CONNECTED_WORKER") else {
        return;
    };
    let storage = filesystem_fixture_store();
    if mode == "seed" {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Chords);
        ui.stage.set_root(3); // C in the A-first picker.
        ui.root_dd.selected = 3;
        let index = ui
            .stage
            .chords()
            .iter()
            .position(|chord| chord.name == "Major 7")
            .unwrap();
        ui.stage.select_chord(index);
        ui.stage_current(None);
        ui.step_card_shape(1);
        ui.set.cards[0].setting.capo = Some(2);
        // Re-select after changing setup; the persisted shape fingerprint
        // then describes the actual capoed instruction.
        ui.clear_selected_card_shape();
        ui.step_card_shape(1);
        ui.set.cards[0].timing.bpm = Some(90.0);
        ui.set.cards[0].timing.hold = Hold::Bars(1);
        let source = ui.set.cards[0].id;
        assert!(ui.inspect_chord_arpeggio(source));
        let authored_before = ui.set.cards.len();
        ui.hear_chord_arpeggio();
        assert_eq!(ui.set.cards.len(), authored_before);
        let first = ui.stage_chord_arpeggio().unwrap();
        let second = ui.stage_chord_arpeggio().unwrap();
        assert_ne!(first, second);
        ui.set.cursor = ui.set.index_of(first).unwrap();
        ui.select_app_section(AppSection::Rehearsal);
        ui.now_ms = Some(1000);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        ui.now_ms = Some(2500);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        storage.save(&serde_json::to_string(&ui.to_persisted()).unwrap());
        storage.save_settings(&serde_json::to_string(&ui.app_settings).unwrap());
        assert!(storage.load().is_some());
    } else {
        assert_eq!(mode, "verify");
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.set.cards.len(), 3);
        assert_ne!(ui.set.cards[1].id, ui.set.cards[2].id);
        assert_eq!(ui.section, AppSection::Rehearsal);
        assert!(!ui.rehearsal_running);
        for card in &ui.set.cards[1..] {
            assert!(matches!(card.touch, Touch::Arpeggiate { .. }));
            assert_eq!(card.setting.capo, Some(2));
            assert_eq!(card.timing.bpm, Some(90.0));
            let pitches = ui
                .stage
                .card_sounding_pitches_at_tempo(card, ui.transport.bpm)
                .unwrap();
            assert!(!pitches.0.is_empty());
            assert!(pitches.2 > 0.0);
        }
        let subject = woodshed_core::history::catalog_id_for_card(&ui.set.cards[1]).unwrap();
        assert_eq!(subject, "arpeggio:Major 7");
        assert_eq!(ui.practice_history.total_practiced_ms(&subject), 1500);
        assert!(
            ui.practice_history
                .recent(10)
                .iter()
                .any(|event| event.provenance.is_some())
        );
    }
    println!("connected-session PASS {mode}");
}

#[test]
fn mixed_catalog_session_reopens_in_a_fresh_process() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["seed", "verify"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::mixed_catalog_session_receipt_worker",
                "--nocapture",
            ])
            .env("WOODSHED_SCALE_SESSION_WORKER", mode)
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
        assert!(String::from_utf8_lossy(&output.stdout).contains("mixed-catalog-session PASS"));
    }
}

#[test]
fn mixed_catalog_session_receipt_worker() {
    use woodshed_core::{Lens, harmony::KeyedCatalogRef, storage::AppSection};
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{FretWindow, Hold, MarkMode, Material, Touch},
    };
    let Ok(mode) = std::env::var("WOODSHED_SCALE_SESSION_WORKER") else {
        return;
    };
    let storage = filesystem_fixture_store();
    if mode == "seed" {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Chords);
        ui.stage.set_root(3);
        ui.root_dd.selected = 3;
        let index = ui
            .stage
            .chords()
            .iter()
            .position(|chord| chord.name == "Major 7")
            .unwrap();
        ui.stage.select_chord(index);
        ui.stage_current(None);
        ui.set.cards[0].setting.instrument = "Ukulele".into();
        ui.set.cards[0].setting.tuning = Some("Standard (high-G)".into());
        ui.set.cards[0].setting.capo = Some(2);
        ui.set.cards[0].setting.fret_window = Some(FretWindow { start: 2, span: 4 });
        ui.step_card_shape(1);
        assert!(ui.set.cards[0].setting.voicing_fingerprint.is_some());
        ui.set.cards[0].timing.bpm = Some(90.0);
        ui.set.cards[0].timing.hold = Hold::Bars(1);
        let source = ui.set.cards[0].id;
        assert!(ui.inspect_chord_arpeggio(source));
        let arpeggio = ui.stage_chord_arpeggio().unwrap();
        assert!(ui.explore_chord_scales(source));
        assert!(ui.inspect_chord_scale(
            source,
            KeyedCatalogRef {
                formula_id: "scale:Major".into(),
                root: PitchClass::new(0),
            }
        ));
        ui.hear_chord_scale();
        assert_eq!(ui.set.cards.len(), 2, "audition cannot author material");
        let scale = ui.stage_chord_scale().unwrap();
        assert_ne!(source, scale);
        assert_ne!(arpeggio, scale);
        let scale_card = ui.set.card_mut(scale).unwrap();
        scale_card.setting.marked = vec![(1, 2)];
        scale_card.setting.mark_mode = MarkMode::Solo;
        ui.set.select_id(arpeggio);
        ui.select_app_section(AppSection::Rehearsal);
        ui.now_ms = Some(1_000);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        ui.now_ms = Some(2_000);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        ui.set.select_id(scale);
        ui.now_ms = Some(10_000);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        ui.now_ms = Some(11_500);
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        storage.save(&serde_json::to_string(&ui.to_persisted()).unwrap());
        storage.save_settings(&serde_json::to_string(&ui.app_settings).unwrap());
        assert!(storage.load().is_some());
    } else {
        assert_eq!(mode, "verify");
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.set.cards.len(), 3);
        let ids = ui
            .set
            .cards
            .iter()
            .map(|card| card.id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            ids.len(),
            3,
            "each saved occurrence retains a distinct identity"
        );
        assert!(ids.iter().all(|id| id.is_assigned()));
        assert!(
            !ui.rehearsal_running,
            "reopen cannot resume or invent practice"
        );
        assert_eq!(ui.section, AppSection::Rehearsal);
        assert_eq!(ui.stage.string_count(), 6, "the live setup remains Guitar");
        let source = &ui.set.cards[0];
        let arpeggio = &ui.set.cards[1];
        let scale = &ui.set.cards[2];
        assert!(source.setting.voicing_fingerprint.is_some());
        assert_eq!(
            arpeggio.setting.voicing_fingerprint,
            source.setting.voicing_fingerprint
        );
        assert!(matches!(arpeggio.touch, Touch::Arpeggiate { .. }));
        assert!(
            matches!(&scale.material, Material::Scale { name, root } if name == "Major" && *root == PitchClass::new(0))
        );
        assert!(matches!(scale.touch, Touch::Walk));
        assert!(scale.setting.voicing_idx.is_none());
        assert!(scale.setting.voicing_fingerprint.is_none());
        assert!(scale.setting.voicing_profile.is_none());
        assert_eq!(scale.setting.marked, vec![(1, 2)]);
        assert_eq!(scale.setting.mark_mode, MarkMode::Solo);
        let realized = ui.stage.scale_card_realization(scale).unwrap();
        assert_eq!(realized.geometry.string_count, 4);
        assert_eq!(realized.geometry.physical_fret_start, 2);
        assert_eq!(realized.geometry.physical_fret_end, 6);
        let dot = realized
            .dots
            .iter()
            .find(|dot| dot.string_index == 1 && dot.fret == 2)
            .unwrap();
        let d4_hz = 440.0 * 2.0_f32.powf((62.0 - 69.0) / 12.0);
        assert!(
            (dot.frequency - d4_hz).abs() < 0.001,
            "saved physical contact is D4 on the Ukulele, not B2 on the live Guitar"
        );
        let sounding = ui.preview_voicing().unwrap();
        assert_eq!(sounding.0.len(), 1);
        assert!((sounding.0[0] - d4_hz).abs() < 0.001);
        assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
        for card in &ui.set.cards {
            assert_eq!(card.setting.instrument, source.setting.instrument);
            assert_eq!(card.setting.instrument, "Ukulele");
            assert_eq!(card.setting.tuning, source.setting.tuning);
            assert_eq!(card.setting.capo, Some(2));
            assert_eq!(card.timing.bpm, Some(90.0));
            assert!(matches!(card.timing.hold, Hold::Bars(1)));
            assert!(
                !ui.stage
                    .card_sounding_pitches_at_tempo(card, ui.transport.bpm)
                    .unwrap()
                    .0
                    .is_empty()
            );
        }
        assert_eq!(
            ui.practice_history.total_practiced_ms("arpeggio:Major 7"),
            1_000
        );
        assert_eq!(ui.practice_history.total_practiced_ms("scale:Major"), 1_500);
        assert_eq!(ui.practice_history.total_practiced_ms("chord:Major 7"), 0);
        let observations = ui.practice_history.recent(20);
        let scale_observation = observations
            .iter()
            .find(|event| event.subject_id == "scale:Major" && event.practiced_ms == Some(1_500))
            .unwrap();
        assert_eq!(
            scale_observation.provenance.as_ref().unwrap().occurrence_id,
            scale.id
        );
        assert_eq!(
            scale_observation.provenance.as_ref().unwrap().card_snapshot,
            serde_json::to_value(scale).unwrap()
        );
    }
    println!("mixed-catalog-session PASS {mode}");
}
