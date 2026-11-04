use super::*;

#[test]
fn midi_sync_keeps_requested_and_accepted_output_distinct() {
    let mut ui = UiState::new();
    ui.midi.output_ports = vec!["A".into(), "B".into()];
    ui.midi.output_dd.selected = 2;
    let mut backend = RoutingBackend {
        accepted: Some("A".into()),
        error: Some("B refused".into()),
        ..Default::default()
    };
    sync_midi_devices(&mut ui, &mut backend);
    assert_eq!(backend.requested.as_deref(), Some("B"));
    assert_eq!(ui.midi.connected_out.as_deref(), Some("A"));
    assert_eq!(ui.midi.error.as_deref(), Some("B refused"));
    backend.completion = Some("B".into());
    backend.error = None;
    // An ordinary frame observes completion without a new input dispatch.
    poll_midi_state(&mut ui, &mut backend);
    assert_eq!(ui.midi.connected_out.as_deref(), Some("B"));
    assert!(ui.midi.error.is_none());
}

#[test]
fn midi_sync_error_reaches_settings_status() {
    use cambium_genet_winit_host::Harness;
    use taproot::Selector;
    let mut ui = UiState::new();
    ui.select_app_section(woodshed_core::storage::AppSection::Settings);
    ui.app_settings.page = woodshed_core::settings::SettingsPage::AudioMidi;
    let mut backend = RoutingBackend {
        error: Some("output refused by fake connector".into()),
        ..Default::default()
    };
    sync_midi_devices(&mut ui, &mut backend);
    let logic: Logic = Box::new(woodshed_views::stage::stage_root);
    let mut harness = Harness::new(woodshed_views::theme::slate_stage_css(), ui, logic);
    harness.layout_at(1100.0, 900.0);
    let dom = harness.runner().dom();
    let dom = dom.borrow();
    assert_eq!(
        taproot::matching_classes(
            &dom,
            &Selector::class("settings-line")
                .with_attr("role", "status")
                .containing("output refused by fake connector")
        )
        .len(),
        1
    );
}

#[test]
fn midi_sync_forwards_return_to_accepted_route_to_cancel_pending_replacement() {
    let mut ui = UiState::new();
    ui.midi.output_ports = vec!["A".into(), "B".into()];
    ui.midi.output_dd.selected = 1;
    let mut backend = RoutingBackend {
        accepted: Some("A".into()),
        requested: Some("B".into()),
        ..Default::default()
    };
    sync_midi_devices(&mut ui, &mut backend);
    assert_eq!(backend.requested.as_deref(), Some("A"));
    ui.midi.output_dd.selected = 0;
    sync_midi_devices(&mut ui, &mut backend);
    assert!(backend.requested.is_none());
}

#[test]
fn midi_sync_rescan_keeps_route_by_name() {
    let mut ui = UiState::new();
    ui.midi.output_ports = vec!["A".into(), "B".into()];
    ui.midi.output_dd.selected = 2;
    ui.midi.refresh_requested = true;
    let mut backend = RoutingBackend::default();
    sync_midi_devices(&mut ui, &mut backend);
    assert_eq!(ui.midi.output_dd.selected, 1);
    assert_eq!(backend.requested.as_deref(), Some("B"));
    assert!(!ui.midi.refresh_requested);
}
