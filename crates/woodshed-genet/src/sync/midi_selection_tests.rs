use woodshed_views::stage::UiState;
#[test]
fn refresh_preserves_output_name_across_reordering() {
    let mut ui = UiState::new();
    ui.midi.output_ports = vec!["wanted".into()];
    ui.midi.output_dd.selected = 1;
    ui.midi
        .refresh_ports(vec![], vec!["new".into(), "wanted".into()]);
    assert_eq!(ui.midi.output_dd.selected, 2);
}
#[test]
fn disappeared_output_becomes_none_instead_of_another_port() {
    let mut ui = UiState::new();
    ui.midi.output_ports = vec!["gone".into()];
    ui.midi.output_dd.selected = 1;
    ui.midi.refresh_ports(vec![], vec!["other".into()]);
    assert_eq!(ui.midi.output_dd.selected, 0);
    ui.midi.refresh_ports(vec![], vec!["another".into()]);
    assert_eq!(ui.midi.output_dd.selected, 0);
}
