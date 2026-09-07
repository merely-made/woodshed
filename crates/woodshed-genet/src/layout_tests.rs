//! Real-host layout regressions for the expanded Set graph.
//!
//! These tests deliberately drive the windowless production host. They cover
//! sizing and scrolling in wide, narrow, and short windows without depending
//! on exact text metrics or screen coordinates.

use cambium_genet_winit_host::Harness;
use genet_probe::Selector;
use woodshed_core::settings::StageGraphReading;
use woodshed_views::stage::{UiChild, UiState, stage_root};

fn harness(width: f32, height: f32) -> Harness<UiState, crate::sync::Logic, UiChild> {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.set_viewport_width(width);
    ui.set_viewport_height(height);
    ui.set_graph_reading(StageGraphReading::Tonnetz);
    ui.set_tray_expanded = true;
    let logic: crate::sync::Logic = Box::new(stage_root);
    let mut harness = Harness::new(woodshed_views::theme::slate_stage_css(), ui, logic);
    harness.layout_at(width, height);
    harness
}

fn rect(
    harness: &Harness<UiState, crate::sync::Logic, UiChild>,
    class: &str,
) -> (f32, f32, f32, f32) {
    let id = {
        let node = harness.runner().dom();
        let dom = node.borrow();
        genet_probe::matching(&dom, &Selector::class(class))
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("missing .{class}"))
    };
    harness
        .painted_rect(id)
        .unwrap_or_else(|| panic!("unpainted .{class}"))
}

#[test]
fn wide_stage_keeps_board_and_expanded_tray_reachable() {
    let mut harness = harness(1_500.0, 664.0);
    assert_eq!(
        harness.state().viewport,
        woodshed_views::stage::ViewportClass::Wide
    );
    let _ = rect(&harness, "stage-body");
    let board = rect(&harness, "board");
    let tray = rect(&harness, "set-tray");
    eprintln!("wide before: board={board:?}, tray={tray:?}");
    assert!(board.3 >= 200.0, "board collapsed: {board:?}");
    assert!(
        tray.1 >= board.1 + board.3,
        "tray overlaps board: {board:?} {tray:?}"
    );

    let before = harness.element_scroll_total();
    let header = rect(&harness, "transport");
    harness.move_to(header.0 + 8.0, header.1 + 8.0);
    harness.wheel(0.0, 10_000.0);
    let after_tray = rect(&harness, "set-tray");
    eprintln!("wide after vertical wheel: tray={after_tray:?}");
    assert!(
        harness.element_scroll_total() > before,
        "expanded Set has no vertical scroll"
    );
    assert!(
        after_tray.1 < tray.1,
        "wheel failed to move tray: {tray:?} -> {after_tray:?}"
    );
    assert!(
        after_tray.1 + after_tray.3 <= 664.0,
        "Set bottom unreachable: {after_tray:?}"
    );
}

#[test]
fn narrow_stage_preserves_graph_width_and_horizontal_scroll() {
    let mut harness = harness(420.0, 664.0);
    let graph = rect(&harness, "set-graph-canvas-stack");
    assert!(
        graph.2 >= 520.0,
        "graph shrank into its narrow parent: {graph:?}"
    );

    let header = rect(&harness, "transport");
    harness.move_to(header.0 + 8.0, header.1 + 8.0);
    harness.wheel(0.0, (graph.1 - 220.0).max(0.0));
    let graph = rect(&harness, "set-graph-canvas-stack");
    assert!(
        graph.1 < 664.0 && graph.1 + graph.3 > 0.0,
        "graph not reachable: {graph:?}"
    );
    let before = harness.element_scroll_total();
    harness.move_to(graph.0 + 60.0, graph.1.max(0.0) + 20.0);
    harness.wheel(500.0, 0.0);
    let after_graph = rect(&harness, "set-graph-canvas-stack");
    eprintln!("narrow horizontal wheel: graph={graph:?} -> {after_graph:?}");
    assert!(
        harness.element_scroll_total() > before,
        "narrow graph row has no horizontal scroll"
    );
    assert!(
        after_graph.0 < graph.0,
        "wheel did not reveal graph's right side"
    );
    assert!((after_graph.2 - 520.0).abs() < 1.0);
}

#[test]
fn settings_uses_the_shared_page_scroll_owner() {
    let mut harness = harness(1_100.0, 300.0);
    harness.update(|ui| {
        ui.section = woodshed_core::storage::AppSection::Settings;
        ui.app_settings.page = woodshed_core::settings::SettingsPage::AudioMidi;
    });
    let viewport = rect(&harness, "workspace-screen");
    let page = rect(&harness, "settings-shell");
    let before = harness.element_scroll_total();
    harness.move_to(viewport.0 + 8.0, viewport.1 + 8.0);
    harness.wheel(0.0, 10_000.0);
    let after = rect(&harness, "settings-shell");
    eprintln!("settings page wheel: {page:?} -> {after:?}");
    assert!(harness.element_scroll_total() > before);
    assert!(after.1 < page.1);
    assert!(
        after.1 + after.3 <= 300.0,
        "settings bottom unreachable: {after:?}"
    );
}
