//! Reading the practice session back out of whichever store opened.
//!
//! Lifted out of `boot_state` when the persona gate arrived: a machine that
//! asks which persona to practise as cannot restore at boot, because the key
//! that decrypts the session is the answer to the question still on screen. So
//! the restore has two callers and one run, and it lives here rather than in
//! either of them.

use woodshed_core::settings::AppSettings;
use woodshed_core::storage::SessionStore;
use woodshed_views::stage::UiState;

use crate::storage::HostBackend;

/// Read the settings slot without applying it. Window creation happens before
/// [`restore`], so the desktop entrypoint uses this same decoder to supply the
/// host's initial geometry.
pub fn load_settings(storage: &SessionStore<HostBackend>) -> Option<AppSettings> {
    storage
        .load_settings()
        .and_then(|json| match serde_json::from_str(&json) {
            Ok(settings) => Some(settings),
            Err(error) => {
                eprintln!("[woodshed-genet] ignoring corrupt application settings: {error}");
                None
            },
        })
}

/// Apply the stored session and application settings to a fresh [`UiState`].
///
/// A corrupt file is reported and skipped rather than raised: a session that
/// will not parse must not stop the application opening, and the next save
/// replaces it. A legacy flat session migrates its settings when no split
/// settings file exists yet.
///
/// Settings ride in through [`UiState::apply_persisted`] rather than being
/// assigned, because several pieces of state are derived from them (the
/// transport's bpm, the tuning and root dropdowns, the legacy relation-set
/// migration). A settings-only store applies them over a default session, so a
/// preference file remains authoritative even when the practice artifact has
/// not been written yet.
pub fn restore(storage: &SessionStore<HostBackend>, ui: &mut UiState) {
    let stored_settings = load_settings(storage);
    let mut app_settings = stored_settings.clone().unwrap_or_default();
    let Some(json) = storage.load() else {
        if stored_settings.is_some() {
            if let Some(error) = ui.apply_persisted(
                &woodshed_core::storage::PersistedSession::default(),
                app_settings,
            ) {
                eprintln!("[woodshed-genet] ignoring invalid workspace snapshot: {error}");
            }
        }
        return;
    };
    match woodshed_core::storage::decode_session(&json) {
        Ok(loaded) => {
            if stored_settings.is_none() {
                if let Some(legacy) = loaded.legacy_settings {
                    app_settings = legacy;
                }
            }
            if let Some(error) = ui.apply_persisted(&loaded.session, app_settings) {
                eprintln!("[woodshed-genet] ignoring invalid workspace snapshot: {error}");
            }
        },
        Err(error) => eprintln!("[woodshed-genet] ignoring corrupt session: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::settings::WindowSettings;
    use woodshed_views::workspace::WorkspacePanel;

    #[test]
    fn settings_apply_without_a_practice_session() {
        let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
        let storage = SessionStore::new(backend);
        let mut settings = AppSettings::default();
        settings.appearance.theme = "Ember".into();
        settings.window = Some(WindowSettings {
            x: 120.0,
            y: 80.0,
            width: 900.0,
            height: 640.0,
            maximized: false,
        });
        storage.save_settings(&serde_json::to_string(&settings).unwrap());

        let mut ui = UiState::new();
        restore(&storage, &mut ui);

        assert_eq!(ui.app_settings, settings);
    }

    #[test]
    fn host_session_restores_the_workspace_policy() {
        let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
        let storage = SessionStore::new(backend);
        let mut saved = UiState::new();
        saved.activate_workspace_panel(WorkspacePanel::Related);
        let saved_session = saved.to_persisted();
        let saved_workspace = saved_session.workspace_json.clone();
        storage.save(&serde_json::to_string(&saved_session).unwrap());

        let mut restored = UiState::new();
        restore(&storage, &mut restored);

        assert_eq!(
            restored.workspace.active_panel(),
            Some(WorkspacePanel::Related)
        );
        assert_eq!(restored.section, woodshed_core::storage::AppSection::Stage);
        assert!(restored.related_expanded);
        assert_eq!(restored.to_persisted().workspace_json, saved_workspace);
    }

    #[test]
    fn legacy_four_panel_workspace_and_session_restore_without_a_retained_library() {
        // Freeze the pre-overview workspace wire contract rather than deriving
        // this fixture from today's list of product panels.
        let tabs = [
            (
                "practice",
                0x574f_5052u64,
                "Practice",
                "woodshed.practice",
                "stage",
            ),
            ("set", 0x574f_5345, "Set", "woodshed.set", "rehearsal"),
            (
                "related",
                0x574f_5245,
                "Related",
                "woodshed.related",
                "stage",
            ),
            (
                "settings",
                0x574f_5347,
                "Settings",
                "woodshed.settings",
                "settings",
            ),
        ]
        .map(|(panel, id, title, kind, content_id)| {
            serde_json::json!({
                "panel": panel, "id": id, "title": title,
                "kind": kind, "content_id": content_id,
            })
        });
        let workspace = serde_json::json!({
            "tree": {"stack": {"tabs": tabs, "active": 1}},
            "active_panel": "set",
        });
        let mut legacy =
            serde_json::to_value(woodshed_core::storage::PersistedSession::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("retained_sets");
        legacy["workspace_json"] = serde_json::Value::String(workspace.to_string());
        legacy["section"] = serde_json::json!("Rehearsal");
        let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
        let storage = SessionStore::new(backend);
        storage.save(&legacy.to_string());
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
        assert_eq!(ui.section, woodshed_core::storage::AppSection::Rehearsal);
        assert!(!ui.rehearsal_running);
        assert!(!ui.song_playing);
        assert!(ui.to_persisted().retained_sets.entries.is_empty());
    }

    #[test]
    fn legacy_single_owner_session_adopts_instances_without_changing_authored_work() {
        use woodshedding::rehearsal::{FretWindow, Hold};
        let mut old = UiState::new();
        old.stage.set_lens(woodshed_core::Lens::Scales);
        old.stage.set_root(6);
        old.stage_current(None);
        old.stage.set_root(9);
        old.stage_current(None);
        old.set.cursor = 1;
        old.set.cards[1].setting.instrument = "Ukulele".into();
        old.set.cards[1].setting.tuning = Some("Standard (high-G)".into());
        old.set.cards[1].setting.capo = Some(2);
        old.set.cards[1].setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        old.set.cards[1].timing.bpm = Some(83.0);
        old.set.cards[1].timing.hold = Hold::Bars(2);
        old.app_settings.fretboard.neck_start = 4;
        old.app_settings.fretboard.neck_end = Some(17);
        let expected_set = serde_json::to_value(&old.set).unwrap();
        let mut legacy = serde_json::to_value(woodshed_core::storage::PersistedSession::capture(
            &old.stage,
            woodshed_core::storage::AppSection::Rehearsal,
            &old.set,
            &old.song,
            &old.practice_history,
        ))
        .unwrap();
        for key in ["working_sets", "catalog_explorations", "active_exploration"] {
            legacy.as_object_mut().unwrap().remove(key);
        }
        let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
        let storage = SessionStore::new(backend);
        storage.save(&legacy.to_string());
        storage.save_settings(&serde_json::to_string(&old.app_settings).unwrap());
        let mut ui = UiState::new();
        restore(&storage, &mut ui);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), expected_set);
        assert_eq!(ui.working_sets.summaries(&ui.set).len(), 1);
        assert!(ui.working_sets.inactive.is_empty());
        assert!(ui.catalog_explorations.inactive.is_empty());
        assert_eq!(ui.catalog_explorations.summaries().len(), 1);
        assert_eq!(ui.stage.root_idx, 9);
        assert_eq!(ui.stage.lens, woodshed_core::Lens::Scales);
        assert_eq!(ui.app_settings.fretboard.neck_start, 4);
        assert_eq!(ui.app_settings.fretboard.neck_end, Some(17));
        assert!(!ui.rehearsal_running);
        assert!(ui.rehearsal_owner.is_none());
        let persisted = ui.to_persisted();
        assert!(persisted.active_exploration.is_some());
        assert_eq!(persisted.working_sets.active_id, ui.working_sets.active_id);
    }
    /// The production filesystem backend and restore path, exercised in two
    /// processes so an in-memory roundtrip cannot satisfy reopening by accident.
    #[test]
    fn connected_session_reopens_in_a_fresh_process() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("practice.json");
        for mode in ["seed", "verify"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "session::tests::connected_session_receipt_worker",
                    "--nocapture",
                ])
                .env("WOODSHED_CONNECTED_WORKER", mode)
                .env("WOODSHED_STATE", &path)
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
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
        let backend: HostBackend = Box::<crate::storage::FsBackend>::default();
        let storage = SessionStore::new(backend);
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
            ui.now_ms = Some(2500);
            ui.toggle_rehearsal();
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
                    .card_sounding_pitches_at_tempo(card, ui.transport.bpm);
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
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
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
        let backend: HostBackend = Box::<crate::storage::FsBackend>::default();
        let storage = SessionStore::new(backend);
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
            ui.now_ms = Some(2_000);
            ui.toggle_rehearsal();
            ui.set.select_id(scale);
            ui.now_ms = Some(10_000);
            ui.toggle_rehearsal();
            ui.now_ms = Some(11_500);
            ui.toggle_rehearsal();
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
            let sounding = ui.preview_voicing();
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
                .find(|event| {
                    event.subject_id == "scale:Major" && event.practiced_ms == Some(1_500)
                })
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
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
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
        let backend: HostBackend = Box::<crate::storage::FsBackend>::default();
        let storage = SessionStore::new(backend);
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
                sounds.push(ui.preview_voicing());
                let subject = catalog_id_for_card(ui.set.card(id).unwrap()).unwrap();
                assert_ne!(subject, "scale:Major");
                subjects.push(subject);
                ui.now_ms = Some(1_000 + index as u64 * 10_000);
                ui.toggle_rehearsal();
                ui.now_ms = Some(2_500 + index as u64 * 10_000);
                ui.toggle_rehearsal();
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
                let sounding = ui.preview_voicing();
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
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("chord-approach-session PASS")
            );
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
        let backend: HostBackend = Box::<crate::storage::FsBackend>::default();
        let storage = SessionStore::new(backend);
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
                sounds.push(ui.preview_voicing());
                let subject = catalog_id_for_card(ui.set.card(id).unwrap()).unwrap();
                assert_ne!(subject, "chord:Major 7");
                subjects.push(subject);
                ui.now_ms = Some(1_000 + index as u64 * 10_000);
                ui.toggle_rehearsal();
                ui.now_ms = Some(2_500 + index as u64 * 10_000);
                ui.toggle_rehearsal();
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
                let sounding = ui.preview_voicing();
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
                ui.preview_voicing().0.is_empty(),
                "an unavailable saved shape must not fall back to a formula preview after reopen"
            );
        }
        println!("chord-approach-session PASS {mode}");
    }
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
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
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
        let backend: HostBackend = Box::<crate::storage::FsBackend>::default();
        let storage = SessionStore::new(backend);
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
            let sound = ui.preview_voicing();
            assert!(sound.0.windows(2).any(|pair| pair[1] < pair[0]));
            let saved_id = ui.save_working_set().unwrap();
            let saved = serde_json::to_value(ui.retained_sets.get(saved_id).unwrap()).unwrap();
            ui.set.cards[0].label = "Edited working scale".into();
            ui.set.cards[0].timing.bpm = Some(137.0);
            ui.now_ms = Some(1_000);
            ui.toggle_rehearsal();
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
                ui.preview_voicing(),
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
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
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
        let backend: HostBackend = Box::<crate::storage::FsBackend>::default();
        let storage = SessionStore::new(backend);
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
            let sound_a = ui.preview_voicing();
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
            let sound_b = ui.preview_voicing();
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
            assert!(ui.activate_working_set(second));
            assert_eq!(ui.rehearsal_owner, Some(first));
            assert_eq!(ui.rehearsal_sounding_pitches().unwrap(), sound_a);
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
            assert_eq!(ui.preview_voicing(), sound_a);
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
            assert_eq!(ui.preview_voicing(), sound_b);
            assert_eq!(
                serde_json::to_value(&ui.retained_sets).unwrap(),
                expected["library"]
            );
        }
        println!("independent-instances PASS {mode}");
    }
}
