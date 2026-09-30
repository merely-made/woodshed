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
}
