use super::*;

#[test]
fn persona_reset_preserves_process_midi_routing_and_inventory() {
    let mut ui = UiState::new();
    ui.midi.input_ports = vec!["keyboard".into()];
    ui.midi.output_ports = vec!["synth".into()];
    ui.midi.input_dd.selected = 1;
    ui.midi.output_dd.selected = 1;
    ui.midi.clock_out = true;
    ui.midi.connected_out = Some("synth".into());
    ui.midi.error = Some("pending diagnostic".into());
    reset_for_persona(&mut ui);
    assert_eq!(ui.midi.input_ports, ["keyboard"]);
    assert_eq!(ui.midi.output_ports, ["synth"]);
    assert_eq!(ui.midi.input_dd.selected, 1);
    assert_eq!(ui.midi.output_dd.selected, 1);
    assert!(ui.midi.clock_out);
    assert_eq!(ui.midi.connected_out.as_deref(), Some("synth"));
    assert_eq!(ui.midi.error.as_deref(), Some("pending diagnostic"));
}

#[test]
fn admitted_persona_keeps_host_availability_without_promoting_tuner_intent() {
    use woodshed_core::audio::{AudioAvailability, AudioStreamStatus};
    let mut ui = UiState::new();
    ui.audio_availability = AudioAvailability {
        output: AudioStreamStatus::Offline,
        input: AudioStreamStatus::AwaitingPermission,
        tuner_active: false,
    };
    ui.audio_error = Some("retained operation diagnostic".into());
    let observed = ui.audio_availability.clone();
    let mut incoming = UiState::new();
    incoming.tuner.enabled = true;
    install_persona_state(&mut ui, incoming);
    assert_eq!(ui.audio_availability, observed);
    assert_eq!(
        ui.audio_error.as_deref(),
        Some("retained operation diagnostic")
    );
    assert!(ui.tuner.enabled && !ui.tuner_is_listening());
}

#[test]
fn persona_reset_keeps_the_host_clock_binding_and_clears_run_position() {
    let mut ui = UiState::new();
    ui.enable_practice_clock();
    ui.event_clock = Some(|| Some(789));
    ui.stage_clock_contacts = vec![(0, 1)];
    ui.stage_clock_event = Some(3);
    reset_for_persona(&mut ui);
    assert!(ui.practice_clock_managed);
    ui.refresh_event_time();
    assert_eq!(ui.now_ms, Some(789));
    assert!(ui.stage_clock_contacts.is_empty());
    assert_eq!(ui.stage_clock_event, None);
    assert!(!ui.rehearsal_running);
    assert!(ui.rehearsal_clock_program.is_none());
}

#[test]
fn switching_persona_retains_the_global_authored_library_but_restores_incoming_selection() {
    use woodshed_core::{settings::AppSettings, storage::SessionStore};
    use woodshed_views::{appearance::AppearanceState, theme::ThemeMode};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("themes.json");
    let mut appearance = AppearanceState::load(&path).unwrap();
    let mut incoming = AppSettings::default();
    incoming.appearance.theme = "Ember".into();
    appearance.begin_edit(&incoming.appearance).unwrap();
    appearance.workshop.save();
    appearance.apply_workshop(&mut incoming.appearance).unwrap();
    let authored_id = appearance.workshop.draft_theme().id.clone();
    appearance.workshop_open = false;
    let reader = appearance.workshop.reader_preview();
    assert!(path.exists(), "the authored definition was really saved");

    let mut ui = UiState::new();
    ui.appearance = appearance;
    ui.appearance_authoring_available = true;
    ui.appearance_notice = Some("Outgoing application selection applied.".into());
    ui.appearance_close_app = true;
    ui.set_theme(ThemeMode::Parchment);
    ui.app_settings.accessibility.reduce_motion = true;

    reset_for_persona(&mut ui);
    assert_eq!(
        ui.app_settings,
        AppSettings::default(),
        "outgoing persona settings must not survive"
    );
    assert!(!ui.appearance_close_app);
    assert!(ui.appearance_notice.is_none());
    assert!(ui.appearance_authoring_available);
    assert_eq!(ui.appearance.workshop.library_path(), Some(path.as_path()));
    assert!(
        std::rc::Rc::ptr_eq(&reader, &ui.appearance.workshop.reader_preview()),
        "retain the actual shared editor rather than reloading it"
    );

    // Use the real settings-only session decoder against a memory store;
    // this never reads the process environment or a user's profile/vault.
    let backend: crate::storage::HostBackend = Box::<muniment::MemoryBackend>::default();
    let storage = SessionStore::new(backend);
    storage.save_settings(&serde_json::to_string(&incoming).unwrap());
    crate::session::restore(&storage, &mut ui);
    assert_eq!(ui.app_settings.appearance, incoming.appearance);
    let resolved = ui.appearance.resolve(&ui.app_settings.appearance);
    assert_eq!(resolved.theme.id, authored_id);
    assert!(resolved.fallback_reason.is_none());
    assert!(ui.appearance_authoring_available);
}

#[test]
fn admitted_persona_install_keeps_shared_editor_and_clears_outgoing_appearance_requests() {
    use std::cell::Cell;
    use tabard::typography::{FontFamily, FontRole};
    use woodshed_core::{settings::AppSettings, storage::SessionStore};
    use woodshed_views::appearance::{AppearanceState, commit_selection};

    let directory = tempfile::tempdir().unwrap();
    let library = directory.path().join("themes.json");
    let mut appearance = AppearanceState::load(&library).unwrap();
    let mut incoming_settings = AppSettings::default();
    incoming_settings.appearance.theme = "Ember".into();
    appearance
        .begin_edit(&incoming_settings.appearance)
        .unwrap();
    appearance.workshop.save();
    appearance
        .apply_workshop(&mut incoming_settings.appearance)
        .unwrap();
    let authored_id = appearance.workshop.draft_theme().id.clone();
    incoming_settings
        .appearance
        .theme_choice
        .as_mut()
        .unwrap()
        .fonts
        .0
        .insert(
            FontRole::Interface,
            vec![FontFamily::new("Incoming Interface").unwrap()],
        );
    appearance.set_available_fonts(vec![
        "Incoming Interface".into(),
        "Outgoing Interface".into(),
    ]);
    let reader = appearance.workshop.reader_preview();
    *appearance.workshop.text_field_mut("name").unwrap() =
        cambium::TextInput::new("Unfinished shared theme title");
    appearance.workshop_open = true;

    let mut ui = UiState::new();
    ui.appearance = appearance;
    ui.appearance_authoring_available = true;
    ui.appearance
        .request_selection(&ui.app_settings.appearance, "woodshed:parchment", None)
        .unwrap();
    *ui.appearance.font_editor.field_mut(FontRole::Interface) =
        cambium::TextInput::new("Outgoing Interface");
    ui.appearance.font_editor.request_apply();
    let backend: crate::storage::HostBackend = Box::<muniment::MemoryBackend>::default();
    let store = SessionStore::new(backend);
    store
        .try_save_settings(&serde_json::to_string(&incoming_settings).unwrap())
        .unwrap();
    let candidate = crate::session::admit(&store).unwrap();
    install_persona_state(&mut ui, candidate);

    assert_eq!(ui.app_settings, incoming_settings);
    assert_eq!(
        ui.appearance.workshop.library_path(),
        Some(library.as_path())
    );
    assert!(std::rc::Rc::ptr_eq(
        &reader,
        &ui.appearance.workshop.reader_preview()
    ));
    assert!(ui.appearance.workshop_open);
    assert_eq!(
        ui.appearance.workshop.text_field("name").unwrap().text(),
        "Unfinished shared theme title"
    );
    assert!(ui.appearance_authoring_available);
    assert_eq!(
        ui.appearance.font_editor.field(FontRole::Interface).text(),
        "Incoming Interface"
    );
    assert_eq!(
        ui.appearance.available_fonts,
        vec!["Incoming Interface", "Outgoing Interface"]
    );
    let writes = Cell::new(0);
    assert!(!commit_selection(&mut ui, |_| {
        writes.set(writes.get() + 1);
        Ok(())
    }));
    assert_eq!(
        writes.get(),
        0,
        "no outgoing request may write incoming settings"
    );
    assert_eq!(ui.app_settings, incoming_settings);
    let resolved = ui.appearance.resolve(&ui.app_settings.appearance);
    assert_eq!(resolved.theme.id, authored_id);
    assert!(resolved.fallback_reason.is_none());
}

#[test]
fn admitted_persona_install_reopens_same_settings_against_the_current_shared_definition() {
    use woodshed_views::appearance::AppearanceState;

    let directory = tempfile::tempdir().unwrap();
    let mut ui = UiState::new();
    ui.appearance = AppearanceState::load(directory.path().join("themes.json")).unwrap();
    ui.appearance_authoring_available = true;
    ui.appearance
        .begin_edit(&ui.app_settings.appearance)
        .unwrap();
    ui.appearance.workshop.save();
    ui.appearance
        .apply_workshop(&mut ui.app_settings.appearance)
        .unwrap();
    let opening_name = ui
        .appearance
        .resolve(&ui.app_settings.appearance)
        .theme
        .name;
    *ui.appearance.workshop.text_field_mut("name").unwrap() =
        cambium::TextInput::new("Current shared definition");
    ui.appearance.workshop.save();
    assert_eq!(
        ui.appearance.workshop.draft_theme().name,
        "Current shared definition"
    );
    assert_eq!(
        ui.appearance
            .resolve(&ui.app_settings.appearance)
            .theme
            .name,
        opening_name,
        "saving a definition alone intentionally keeps the active presentation"
    );
    let mut candidate = UiState::new();
    candidate.app_settings = ui.app_settings.clone();
    install_persona_state(&mut ui, candidate);
    assert_eq!(
        ui.appearance
            .resolve(&ui.app_settings.appearance)
            .theme
            .name,
        "Current shared definition"
    );
}

#[test]
fn switching_persona_retains_the_global_library_load_error() {
    let mut ui = UiState::new();
    ui.appearance_authoring_available = false;
    ui.appearance_notice = Some("Could not open theme library: corrupt library.".into());
    ui.set_theme(woodshed_views::theme::ThemeMode::Ember);
    let reader = ui.appearance.workshop.reader_preview();
    reset_for_persona(&mut ui);
    assert_eq!(
        ui.app_settings,
        woodshed_core::settings::AppSettings::default()
    );
    assert!(!ui.appearance_authoring_available);
    assert_eq!(
        ui.appearance_notice.as_deref(),
        Some("Could not open theme library: corrupt library.")
    );
    assert!(std::rc::Rc::ptr_eq(
        &reader,
        &ui.appearance.workshop.reader_preview()
    ));
}
