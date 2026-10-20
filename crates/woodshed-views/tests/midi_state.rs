use woodshed_views::stage::MidiUiState;

fn ports(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn refresh_preserves_selected_ports_by_name_when_order_changes() {
    let mut state = MidiUiState::new();
    state.refresh_ports(
        ports(&["Input A", "Input B"]),
        ports(&["Output A", "Output B"]),
    );
    state.input_dd.selected = 2;
    state.output_dd.selected = 1;

    state.refresh_ports(
        ports(&["Input B", "Input A"]),
        ports(&["Output B", "Output A"]),
    );

    assert_eq!(state.input_dd.selected, 1);
    assert_eq!(state.output_dd.selected, 2);
    assert_eq!(state.selected_input(), Some("Input B"));
    assert_eq!(state.selected_output(), Some("Output A"));
}

#[test]
fn refresh_clears_removed_port_selections_instead_of_reusing_the_old_index() {
    let mut state = MidiUiState::new();
    state.refresh_ports(
        ports(&["Input A", "Input B"]),
        ports(&["Output A", "Output B"]),
    );
    state.input_dd.selected = 2;
    state.output_dd.selected = 2;

    state.refresh_ports(
        ports(&["Replacement input"]),
        ports(&["Replacement output"]),
    );

    assert_eq!(state.input_dd.selected, 0);
    assert_eq!(state.output_dd.selected, 0);
}

#[test]
fn refresh_clears_an_invalid_stale_index() {
    let mut state = MidiUiState::new();
    state.refresh_ports(ports(&["Input A"]), ports(&["Output A"]));
    state.input_dd.selected = 99;
    state.output_dd.selected = 99;

    assert_eq!(state.selected_input(), None);
    assert_eq!(state.selected_output(), None);
    state.refresh_ports(
        ports(&["Replacement input"]),
        ports(&["Replacement output"]),
    );

    assert_eq!(state.input_dd.selected, 0);
    assert_eq!(state.output_dd.selected, 0);
}

#[test]
fn refresh_keeps_none_selected() {
    let mut state = MidiUiState::new();
    state.refresh_ports(ports(&["Input A"]), ports(&["Output A"]));

    state.refresh_ports(
        ports(&["Input B", "Input A"]),
        ports(&["Output B", "Output A"]),
    );

    assert_eq!(state.input_dd.selected, 0);
    assert_eq!(state.output_dd.selected, 0);
}

#[test]
fn backend_error_starts_empty() {
    assert_eq!(MidiUiState::new().error, None);
}
