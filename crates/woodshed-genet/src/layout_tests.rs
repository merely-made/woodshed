//! Real-host layout regressions for the expanded Set graph.
//!
//! These tests deliberately drive the windowless production host. They cover
//! sizing and scrolling in wide, narrow, and short windows without depending
//! on exact text metrics or screen coordinates.

use cambium_genet_winit_host::Harness;
use taproot::Selector;
use woodshed_core::settings::StageGraphReading;
use woodshed_views::stage::{UiChild, UiState, stage_root};

fn harness(width: f32, height: f32) -> Harness<UiState, crate::sync::Logic, UiChild> {
    let mut ui = UiState::new();
    ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Practice);
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
        taproot::matching(&dom, &Selector::class(class))
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("missing .{class}"))
    };
    harness
        .painted_rect(id)
        .unwrap_or_else(|| panic!("unpainted .{class}"))
}

fn class_count(h: &Harness<UiState, crate::sync::Logic, UiChild>, class: &str) -> usize {
    let node = h.runner().dom();
    let dom = node.borrow();
    taproot::matching(&dom, &Selector::class(class)).len()
}

#[test]
fn session_overview_inspects_retains_and_opens_real_work_without_navigation_side_effects() {
    use woodshed_core::session_overview::{OverviewNodeId, OverviewProcess};
    use woodshed_views::workspace::WorkspacePanel;
    for width in [1_100.0, 420.0] {
        let mut h = harness(width, 664.0);
        h.update(|ui| {
            ui.now_ms = Some(1_000);
            ui.toggle_rehearsal();
            ui.song_playing = true;
            ui.tuner.enabled = true;
        });
        let originals = serde_json::to_value(&h.state().set).unwrap();
        let history_len = h.state().practice_history.len();
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        let snapshot = woodshed_views::stage::overview_snapshot(h.state());
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.id == OverviewNodeId::Process(OverviewProcess::Rehearsal))
        );
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.id == OverviewNodeId::Process(OverviewProcess::Looper))
        );
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.id == OverviewNodeId::Process(OverviewProcess::Tuner))
        );
        assert!(h.click_on(&Selector::class("overview-node-title").containing("Working Set")));
        assert_eq!(
            h.state().workspace.active_panel(),
            Some(WorkspacePanel::Overview)
        );
        assert_eq!(serde_json::to_value(&h.state().set).unwrap(), originals);
        assert_eq!(h.state().practice_history.len(), history_len);
        assert!(h.state().rehearsal_running && h.state().song_playing && h.state().tuner.enabled);
        assert!(h.click_on(&Selector::class("overview-open")));
        assert_eq!(
            h.state().workspace.active_panel(),
            Some(WorkspacePanel::Set)
        );
        assert!(h.state().rehearsal_running);
        for (label, section) in [
            ("Looper playing", woodshed_core::storage::AppSection::Looper),
            ("Tuner listening", woodshed_core::storage::AppSection::Tools),
        ] {
            assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
            assert!(h.click_on(&Selector::class("overview-node-title").containing(label)));
            assert!(h.click_on(&Selector::class("overview-open")));
            assert_eq!(h.state().section, section);
            assert!(
                h.state().rehearsal_running && h.state().song_playing && h.state().tuner.enabled
            );
        }
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        assert!(
            h.click_on(&Selector::class("overview-node-title").containing("Rehearsal running"))
        );
        assert!(h.click_on(&Selector::class("overview-open")));
        assert_eq!(
            h.state().workspace.active_panel(),
            Some(WorkspacePanel::Set)
        );
        assert!(h.state().rehearsal_running);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        assert!(h.click_on(&Selector::class("overview-save")));
        let saved_id = h.state().retained_sets.entries[0].id;
        let snapshot =
            serde_json::to_value(h.state().retained_sets.get(saved_id).unwrap()).unwrap();
        let old_id = h.state().set.cards[0].id;
        let old_owner = h.state().working_sets.active_id;
        h.update(|ui| {
            ui.set.cards[0].timing.bpm = Some(137.0);
            assert!(ui.inspect_chord_arpeggio(old_id));
            ui.now_ms = Some(2_500);
        });
        let edited = serde_json::to_value(&h.state().set).unwrap();
        assert!(h.click_on(&Selector::class("overview-open").containing("Open copy")));
        assert_eq!(
            h.state().workspace.active_panel(),
            Some(WorkspacePanel::Set)
        );
        assert!(h.state().rehearsal_running);
        assert_eq!(h.state().rehearsal_owner, Some(old_owner));
        assert!(h.state().arpeggio_source.is_none());
        h.update(|ui| {
            ui.audio_requests.clear();
            ui.hear_chord_arpeggio();
            assert!(ui.audio_requests.is_empty());
            assert!(ui.stage_chord_arpeggio().is_none());
        });
        assert_ne!(h.state().set.cards[0].id, old_id);
        assert_eq!(
            serde_json::to_value(h.state().retained_sets.get(saved_id).unwrap()).unwrap(),
            snapshot
        );
        assert_eq!(
            serde_json::to_value(
                h.state()
                    .working_sets
                    .get(old_owner, &h.state().set)
                    .unwrap()
            )
            .unwrap(),
            edited
        );
        assert_eq!(h.state().retained_sets.entries.len(), 1);
        assert_eq!(h.state().set.cards[0].timing.bpm, None);
        let opened_id = h.state().set.cards[0].id;
        let opened_owner = h.state().working_sets.active_id;
        h.update(|ui| {
            assert!(ui.open_retained_set(saved_id));
        });
        assert_ne!(
            (h.state().working_sets.active_id, h.state().set.cards[0].id),
            (opened_owner, opened_id)
        );
        assert_ne!(h.state().set.cards[0].id, old_id);
    }
}

#[test]
fn independent_instances_keep_background_rehearsal_and_explicit_staging_bound_to_their_owners() {
    use woodshed_core::session_overview::{
        OverviewNodeId, OverviewProcess, OverviewRelationKind, SessionArtifactId,
    };
    use woodshedding::rehearsal::{FretWindow, Hold};
    for width in [1_100.0, 420.0] {
        let mut h = harness(width, 664.0);
        h.update(|ui| {
            ui.stage.set_lens(woodshed_core::Lens::Scales);
            let major = ui
                .stage
                .scales()
                .iter()
                .position(|scale| scale.name == "Major")
                .unwrap();
            ui.stage.select_scale(major);
            ui.stage.set_root(0);
            ui.set.cards.clear();
            ui.stage_current(None);
            ui.stage.set_root(7);
            ui.stage_current(None);
            for card in &mut ui.set.cards {
                card.setting.instrument = "Ukulele".into();
                card.setting.tuning = Some("Standard (high-G)".into());
                card.setting.capo = Some(2);
                card.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
                card.timing.bpm = Some(83.0);
                card.timing.hold = Hold::Bars(2);
            }
            ui.set.cursor = 0;
            ui.now_ms = Some(1_000);
        });
        let owner_a = h.state().working_sets.active_id;
        let card_a = h.state().set.cards[0].clone();
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Set")));
        let sound_a = h.state().preview_voicing();
        assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
        assert_eq!(h.state().rehearsal_owner, Some(owner_a));
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        assert!(h.click_on(&Selector::class("overview-duplicate-set")));
        let owner_b = h.state().working_sets.active_id;
        assert_ne!(owner_a, owner_b);
        assert_ne!(h.state().set.cards[0].id, card_a.id);
        h.update(|ui| {
            ui.set.cursor = 1;
            ui.set.cards[1].setting.capo = Some(4);
            ui.set.cards[1].setting.fret_window = Some(FretWindow { start: 4, span: 8 });
            ui.set.cards[1].timing.bpm = Some(117.0);
        });
        let b = serde_json::to_value(&h.state().set).unwrap();
        let sound_b = h.state().preview_voicing();
        assert_ne!(sound_a.0, sound_b.0);
        assert_eq!(h.state().rehearsal_sounding_pitches().unwrap(), sound_a);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        assert!(h.click_on(&Selector::class("overview-new-set")));
        let owner_c = h.state().working_sets.active_id;
        assert!(h.state().set.cards.is_empty());
        assert_eq!(h.state().rehearsal_owner, Some(owner_a));
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        let exploration_a = h.state().catalog_explorations.active_id;
        let config_a = serde_json::to_value(h.state().capture_exploration()).unwrap();
        assert!(h.click_on(&Selector::class("overview-new-exploration")));
        let exploration_b = h.state().catalog_explorations.active_id;
        assert_ne!(exploration_a, exploration_b);
        for class in ["stage-screen", "instance-binding", "stage-body", "board"] {
            if class == "stage-body" && class_count(&h, class) == 0 {
                continue; // FullCanvas and narrow Stage place the board directly.
            }
            let bounds = rect(&h, class);
            assert!(
                [bounds.0, bounds.1, bounds.2, bounds.3]
                    .iter()
                    .all(|value| value.is_finite()),
                "new exploration has non-finite .{class} geometry: {bounds:?}"
            );
            assert!(
                bounds.2 > 0.0 && bounds.3 > 0.0 && bounds.2 < 10_000.0 && bounds.3 < 10_000.0,
                "new exploration has invalid .{class} geometry: {bounds:?}"
            );
            eprintln!("new exploration {width}: .{class}={bounds:?}");
        }
        h.update(|ui| {
            ui.stage.set_lens(woodshed_core::Lens::Chords);
            ui.stage.set_root(9);
            ui.stage.set_tuning(1);
            ui.root_dd.selected = 9;
            ui.tuning_dd.selected = 1;
            ui.app_settings.fretboard.neck_start = 5;
            ui.app_settings.fretboard.neck_end = Some(17);
            ui.search = cambium::TextInput::new("Minor");
        });
        let config_b = serde_json::to_value(h.state().capture_exploration()).unwrap();
        assert_eq!(h.state().rehearsal_sounding_pitches().unwrap(), sound_a);
        assert!(h.click_on(&Selector::class("staging-target").containing("Working Set 1 copy")));
        assert_eq!(h.state().working_sets.active_id, owner_b);
        assert_eq!(h.state().catalog_explorations.active_id, exploration_b);
        assert!(h.click_on(&Selector::class("staging-target").containing("Working Set 3")));
        assert_eq!(h.state().working_sets.active_id, owner_c);
        assert_eq!(h.state().catalog_explorations.active_id, exploration_b);
        assert!(h.click_on(&Selector::class("t-btn").containing("Stage")));
        assert_eq!(h.state().set.cards.len(), 1);
        assert!(
            matches!(&h.state().set.cards[0].material,woodshedding::rehearsal::Material::Chord{root,..} if root.value()==6)
        );
        assert_eq!(
            serde_json::to_value(h.state().working_sets.get(owner_b, &h.state().set).unwrap())
                .unwrap(),
            b
        );
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        let overview = woodshed_views::stage::overview_snapshot(h.state());
        assert!(overview.relations.iter().any(|relation| relation.from
            == OverviewNodeId::Process(OverviewProcess::Rehearsal)
            && relation.to
                == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(owner_a))
            && relation.kind == OverviewRelationKind::ActsOn));
        assert!(!overview.relations.iter().any(|relation| relation.from
            == OverviewNodeId::Process(OverviewProcess::Rehearsal)
            && relation.to
                == OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(owner_c))));
        h.update(|ui| {
            ui.now_ms = Some(2_500);
            assert!(ui.advance_rehearsal_cursor());
        });
        assert_eq!(
            h.state()
                .working_sets
                .get(owner_a, &h.state().set)
                .unwrap()
                .cursor,
            1
        );
        assert_eq!(h.state().set.cursor, 0);
        let completed = h
            .state()
            .practice_history
            .recent(30)
            .into_iter()
            .find(|event| event.practiced_ms == Some(1_500))
            .unwrap();
        let provenance = completed.provenance.unwrap();
        assert_eq!(provenance.working_set_id, Some(owner_a));
        assert_eq!(provenance.occurrence_id, card_a.id);
        assert_eq!(
            provenance.card_snapshot,
            serde_json::to_value(&card_a).unwrap()
        );
        assert_eq!(
            provenance.presented_midi,
            Some(
                sound_a
                    .0
                    .iter()
                    .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
                    .collect()
            ),
            "the background observation must retain the pitches from A's saved setup"
        );
        assert!(
            h.click_on(&Selector::class("overview-node-title").containing("Catalog exploration 1"))
        );
        assert!(h.click_on(&Selector::class("overview-open")));
        assert_eq!(
            serde_json::to_value(h.state().capture_exploration()).unwrap(),
            config_a
        );
        assert!(h.state().rehearsal_running);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        assert!(
            h.click_on(&Selector::class("overview-node-title").containing("Catalog exploration 2"))
        );
        assert!(h.click_on(&Selector::class("overview-open")));
        assert_eq!(
            serde_json::to_value(h.state().capture_exploration()).unwrap(),
            config_b
        );
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
        assert!(
            h.click_on(&Selector::class("overview-node-title").containing("Working Set 1 copy"))
        );
        assert!(h.click_on(&Selector::class("overview-open")));
        assert_eq!(h.state().working_sets.active_id, owner_b);
        assert_eq!(h.state().preview_voicing(), sound_b);
        h.update(|ui| ui.now_ms = Some(3_000));
        assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
        assert_eq!(h.state().rehearsal_owner, Some(owner_b));
        h.update(|ui| ui.now_ms = Some(4_000));
        assert!(h.click_on(&Selector::class("t-btn").containing("Pause")));
        assert!(h.state().rehearsal_owner.is_none());
        assert!(h.state().practice_history.recent(30).iter().any(|event| {
            event.practiced_ms == Some(1_000)
                && event
                    .provenance
                    .as_ref()
                    .is_some_and(|provenance| provenance.working_set_id == Some(owner_b))
        }));
    }
}

#[test]
fn background_runner_inherits_the_starting_exploration_setup() {
    let mut h = harness(1_100.0, 664.0);
    h.update(|ui| {
        ui.stage.set_lens(woodshed_core::Lens::Scales);
        let major = ui
            .stage
            .scales()
            .iter()
            .position(|scale| scale.name == "Major")
            .unwrap();
        ui.stage.select_scale(major);
        ui.stage.set_root(5);
        ui.root_dd.selected = 5;
        ui.set.cards.clear();
        ui.stage_current(None);
        ui.set.cards[0].setting.tuning = None;
        ui.set.cards[0].setting.instrument.clear();
        ui.set.cards[0].setting.fret_window =
            Some(woodshedding::rehearsal::FretWindow { start: 0, span: 2 });
        ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
        ui.now_ms = Some(1_000);
    });
    assert!(h.state().set.cards[0].setting.tuning.is_none());
    let inherited = h.state().preview_voicing();
    assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
    assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
    assert!(h.click_on(&Selector::class("overview-new-exploration")));
    h.update(|ui| {
        ui.stage.set_tuning(1);
        ui.tuning_dd.selected = 1;
    });
    assert_ne!(
        h.state()
            .stage
            .card_sounding_pitches_at_tempo(&h.state().set.cards[0], h.state().transport.bpm)
            .0,
        inherited.0
    );
    assert_eq!(h.state().rehearsal_sounding_pitches().unwrap(), inherited);
    assert!(h.click_on(&Selector::class("workspace-panel").containing("Mere")));
    assert!(h.click_on(&Selector::class("overview-node-title").containing("Rehearsal running")));
    assert!(h.click_on(&Selector::class("overview-pause-rehearsal")));
    assert!(!h.state().rehearsal_running);
}

#[test]
fn card_parameters_and_explanations_do_not_overlap_and_related_rows_still_stage() {
    use layout_dom_api::LayoutDom;
    for width in [1_100.0, 420.0] {
        let mut h = harness(width, 664.0);
        h.update(|ui| ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Set));
        let controls = rect(&h, "card-control-row");
        let explanations = rect(&h, "card-explanation-section");
        assert!(
            explanations.1 >= controls.1 + controls.3,
            "parameters overlap explanations at {width}: {controls:?} {explanations:?}"
        );
        let button_ids = {
            let node = h.runner().dom();
            let dom = node.borrow();
            let row = taproot::matching(&dom, &Selector::class("card-control-row"))[0];
            dom.dom_children(row).collect::<Vec<_>>()
        };
        for id in button_ids {
            if let Some(action) = h.painted_rect(id) {
                assert!(
                    action.0 >= controls.0 - 1.0
                        && action.0 + action.2 <= controls.0 + controls.2 + 1.0,
                    "Card control escapes its paragraph: {action:?} inside {controls:?}"
                );
            }
        }
        assert!(class_count(&h, "related-graph-col") == 0);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Practice")));
        assert!(class_count(&h, "related-graph-col") == 0);
        assert!(class_count(&h, "related-stage") > 0);
        let count = h.state().set.cards.len();
        assert!(h.click_on(&Selector::class("related-stage")));
        assert_eq!(h.state().set.cards.len(), count + 1);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Set")));
        assert!(class_count(&h, "related-graph-col") == 0);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Related")));
        assert_eq!(class_count(&h, "related-graph-col"), 1);
    }
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

#[test]
fn narrow_settings_navigation_stacks_and_selects_a_page() {
    let mut h = harness(420.0, 700.0);
    assert!(h.click_on(&Selector::class("workspace-panel").containing("Settings")));
    assert_eq!(
        h.state().section,
        woodshed_core::storage::AppSection::Settings
    );
    let nav = rect(&h, "settings-page-nav");
    let page = rect(&h, "settings-page");
    eprintln!("narrow settings: nav={nav:?} page={page:?}");
    assert!(nav.0 >= 0.0 && nav.0 + nav.2 <= 420.0);
    assert!(page.1 >= nav.1 + nav.3, "page overlaps navigation");
    assert!(page.0 >= 0.0 && page.0 + page.2 <= 420.0);
    assert!(h.click_on(&Selector::class("side-item").containing("Instrument")));
    assert_eq!(
        h.state().app_settings.page,
        woodshed_core::settings::SettingsPage::Instrument
    );
}

#[test]
fn rehearsal_board_scroll_preserves_extent_and_note_hits() {
    let mut h = harness(420.0, 900.0);
    h.update(|ui| ui.section = woodshed_core::storage::AppSection::Rehearsal);
    // Card paragraphs can grow above the board. Bring the whole board into
    // the native viewport before choosing a physical note hit below.
    let initial = rect(&h, "rehearsal-board-viewport");
    h.move_to(30.0, 130.0);
    h.wheel(0.0, (initial.1 - 350.0).max(0.0));
    let viewport = rect(&h, "rehearsal-board-viewport");
    let before = rect(&h, "fretboard-stack");
    assert!(viewport.0 >= 0.0 && viewport.0 + viewport.2 <= 420.0);
    assert!(
        before.2 > viewport.2,
        "fixture must have an oversized board"
    );
    assert!(
        viewport.1 + 25.0 < 900.0,
        "fixture viewport must be visible"
    );
    h.move_to(viewport.0 + 20.0, viewport.1 + 20.0);
    h.wheel(240.0, 0.0);
    let after = rect(&h, "fretboard-stack");
    eprintln!("rehearsal board scroll: viewport={viewport:?} inner={before:?} -> {after:?}");
    assert!(after.0 < before.0);
    assert!((after.2 - before.2).abs() < 1.0);
    let nodes = {
        let dom = h.runner().dom();
        let dom = dom.borrow();
        taproot::matching(&dom, &Selector::class("fret-label"))
    };
    let point = nodes
        .into_iter()
        .filter_map(|id| h.painted_rect(id))
        .find_map(|r| {
            let center = (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
            (center.0 > viewport.0 + 8.0
                && center.0 < viewport.0 + viewport.2 - 8.0
                && center.1 > viewport.1 + 8.0
                && center.1 < viewport.1 + viewport.3 - 8.0)
                .then_some(center)
        })
        .expect("visible note after scrolling");
    assert!(h.state().set.cards[0].setting.marked.is_empty());
    h.click_at(point.0, point.1);
    assert_eq!(
        h.state().set.cards[0].setting.marked.len(),
        1,
        "scrolled note hit must mark exactly one position"
    );
}

#[test]
fn rehearsal_shape_controls_change_notes_and_clear_to_the_map() {
    let mut h = harness(700.0, 900.0);
    h.update(|ui| ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal));
    let all_tones = h.state().stage.dots_for_card(&h.state().set.cards[0]);
    assert!(h.click_on(&Selector::class("card-shape-next")));
    let first = h.state().stage.dots_for_card(&h.state().set.cards[0]);
    assert!(h.state().set.cards[0].setting.voicing_idx.is_some());
    assert!(!first.is_empty() && first.len() < all_tones.len());
    let first_positions = first
        .iter()
        .map(|dot| (dot.string_index, dot.fret))
        .collect::<Vec<_>>();
    let sound = h.state().preview_voicing().0;
    let expected = first.iter().map(|dot| dot.frequency).collect::<Vec<_>>();
    assert_eq!(
        sound, expected,
        "audition must use the painted shape's concert pitches"
    );
    assert!(h.click_on(&Selector::class("card-shape-next")));
    let second = h.state().stage.dots_for_card(&h.state().set.cards[0]);
    assert_ne!(
        first_positions,
        second
            .iter()
            .map(|dot| (dot.string_index, dot.fret))
            .collect::<Vec<_>>()
    );
    assert!(h.click_on(&Selector::class("card-shape-prev")));
    assert_eq!(
        h.state()
            .stage
            .dots_for_card(&h.state().set.cards[0])
            .iter()
            .map(|dot| (dot.string_index, dot.fret))
            .collect::<Vec<_>>(),
        first_positions
    );
    assert!(h.click_on(&Selector::class("card-shape-clear")));
    assert!(h.state().set.cards[0].setting.voicing_idx.is_none());
    assert_eq!(
        h.state().stage.dots_for_card(&h.state().set.cards[0]).len(),
        all_tones.len()
    );
}

#[test]
fn selected_card_geometry_overrides_the_live_guitar() {
    let mut h = harness(700.0, 900.0);
    h.update(|ui| {
        ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
        ui.stage.fret_count = 7;
        ui.stage.set_root(3); // C has a root-bass high-G ukulele shape in frets 0..4.
        let material = ui.stage.card_from_lens().unwrap().material;
        ui.shift_card_window(0);
        let card = &mut ui.set.cards[0];
        card.material = material;
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = None;
        ui.stage.select_next_card_shape(card).unwrap();
    });
    assert_eq!(h.state().stage.string_count(), 6);
    let geom = h.state().rehearsal_board_geometry();
    assert_eq!(
        (geom.string_count, geom.fret_start, geom.fret_count),
        (4, 0, 4)
    );
    let board = rect(&h, "fretboard-stack");
    let expected = geom.size_u32();
    assert_eq!((board.2, board.3), (expected.0 as f32, expected.1 as f32));
    assert!(
        h.state()
            .stage
            .dots_for_card(&h.state().set.cards[0])
            .iter()
            .all(|dot| dot.string_index < 4)
    );
    h.update(|ui| {
        for _ in 0..20 {
            ui.shift_card_window(1);
        }
    });
    assert_eq!(
        h.state().set.cards[0].setting.fret_window.unwrap().start,
        11,
        "the selected ukulele window must reach its 15th fret despite the live guitar ending at 7"
    );
    h.update(|ui| {
        ui.set.cards[0].setting.capo = Some(7);
        for _ in 0..20 {
            ui.shift_card_window(-1);
        }
    });
    assert_eq!(h.state().set.cards[0].setting.fret_window.unwrap().start, 7);
}

#[test]
fn neck_comparison_tracks_previous_card_and_shape_controls() {
    let mut h = harness(700.0, 900.0);
    h.update(|ui| {
        ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
        ui.step_card_shape(1);
        ui.set.duplicate(0);
        ui.set.cursor = 1;
    });
    let movement = h
        .state()
        .stage
        .compare_cards(&h.state().set.cards[0], &h.state().set.cards[1])
        .unwrap();
    assert_eq!(movement.total_matched_fret_travel, 0);
    assert!(h.click_on(&Selector::class("card-shape-next")));
    let changed = h
        .state()
        .stage
        .compare_cards(&h.state().set.cards[0], &h.state().set.cards[1])
        .unwrap();
    assert_ne!(movement.per_string, changed.per_string);
    assert!(h.click_on(&Selector::class("card-shape-clear")));
    assert!(matches!(
        h.state()
            .stage
            .compare_cards(&h.state().set.cards[0], &h.state().set.cards[1]),
        Err(woodshed_core::shape_movement::ShapeMovementUnavailable::RightUnselected)
    ));
}

#[test]
fn connected_discovery_controls_stage_the_selected_shape_and_run_it() {
    for width in [1_100.0, 420.0] {
        let mut h = harness(width, 900.0);
        h.update(|ui| {
            ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
            ui.now_ms = Some(1_000);
        });
        assert!(h.click_on(&Selector::class("card-shape-next")));
        let source = h.state().set.cards[0].id;
        let setting = serde_json::to_value(&h.state().set.cards[0].setting).unwrap();
        assert!(h.click_on(&Selector::class("t-btn").containing("Discover ")));
        assert_eq!(h.state().set.cards.len(), 1);
        assert_eq!(h.state().arpeggio_source, Some(source));
        assert!(h.click_on(&Selector::class("t-btn").containing("Hear arpeggio")));
        assert_eq!(h.state().set.cards.len(), 1);
        assert!(!h.state().audio_requests.is_empty());
        assert!(h.click_on(&Selector::class("t-btn").containing("Stage arpeggio")));
        assert_eq!(h.state().set.cards.len(), 2);
        assert_eq!(
            serde_json::to_value(&h.state().set.cards[1].setting).unwrap(),
            setting
        );
        assert_ne!(h.state().set.cards[1].id, source);
        h.update(|ui| {
            ui.set.cursor = 1;
            ui.now_ms = Some(2_000);
        });
        assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
        assert!(h.state().rehearsal_running);
        h.update(|ui| ui.now_ms = Some(3_000));
        assert!(h.click_on(&Selector::class("t-btn").containing("Pause")));
        assert!(!h.state().rehearsal_running);
        assert_eq!(
            h.state()
                .practice_history
                .total_practiced_ms("arpeggio:Major"),
            1_000
        );
    }
}

#[test]
fn expanded_graph_editor_keeps_discovery_outside_the_canvas() {
    let mut h = harness(1_500.0, 1_500.0);
    h.update(|ui| ui.set_graph_card_expanded = true);
    let graph = rect(&h, "set-graph-canvas-stack");
    let editor = rect(&h, "set-graph-selected-card");
    assert!(
        editor.0 >= graph.0 + graph.2 || editor.1 >= graph.1 + graph.3,
        "editor overlaps graph: {editor:?} {graph:?}"
    );
    assert!(h.click_on(&Selector::class("t-btn").containing("Discover")));
    assert!(h.state().arpeggio_source.is_some());
    assert!(h.click_on(&Selector::class("t-btn").containing("Stage arpeggio")));
    assert_eq!(h.state().set.cards.len(), 2);
}

#[test]
fn compatible_scale_controls_choose_audition_stage_and_drill_without_changing_the_chord() {
    use woodshed_core::{audio::AudioRequest, storage::AppSection};
    use woodshedding::rehearsal::{Hold, Material, Touch};
    for width in [1_100.0, 420.0] {
        let mut h = harness(width, 900.0);
        h.update(|ui| {
            ui.set.cards.clear();
            ui.stage.set_root(3); // C in the A-first picker.
            ui.root_dd.selected = 3;
            let major_seven = ui
                .stage
                .chords()
                .iter()
                .position(|chord| chord.name == "Major 7")
                .unwrap();
            ui.stage.select_chord(major_seven);
            ui.stage_current(None);
            ui.set.cards[0].timing.bpm = Some(90.0);
            ui.set.cards[0].timing.hold = Hold::Bars(1);
            ui.select_app_section(AppSection::Rehearsal);
            ui.now_ms = Some(1_000);
        });
        assert!(h.click_on(&Selector::class("card-shape-next")));
        let source = h.state().set.cards[0].id;
        let source_before = serde_json::to_value(&h.state().set.cards[0]).unwrap();
        assert!(h.click_on(&Selector::class("t-btn").containing("Explore compatible scales")));
        assert_eq!(h.state().set.cards.len(), 1);
        assert!(h.click_on(&Selector::class("scale-choice").containing("C Major")));
        assert!(h.click_on(&Selector::class("t-btn").containing("Hear scale")));
        assert_eq!(
            h.state().set.cards.len(),
            1,
            "hearing a relationship cannot author the Set"
        );
        let heard = match h.state().audio_requests.last().unwrap() {
            AudioRequest::PreviewPitches {
                pitches,
                duration_s,
                strum_s,
            } => {
                assert!(!pitches.is_empty());
                assert!(
                    *strum_s > 0.0,
                    "scale audition visits its notes in sequence"
                );
                (pitches.clone(), *duration_s, *strum_s)
            },
            request => panic!("unexpected scale audition request: {request:?}"),
        };
        assert!(h.click_on(&Selector::class("t-btn").containing("Stage scale")));
        assert_eq!(h.state().set.cards.len(), 2);
        assert_eq!(
            serde_json::to_value(&h.state().set.cards[0]).unwrap(),
            source_before,
            "discovery and explicit staging preserve the source chord and its chosen shape"
        );
        let added = h.state().set.cards[1].id;
        let scale = &h.state().set.cards[1];
        assert_ne!(added, source);
        assert!(matches!(&scale.material, Material::Scale { name, .. } if name == "Major"));
        assert!(matches!(scale.touch, Touch::Walk));
        assert!(scale.setting.voicing_idx.is_none());
        assert!(scale.setting.voicing_fingerprint.is_none());
        assert!(scale.setting.voicing_profile.is_none());
        assert_eq!(
            serde_json::to_value(&scale.timing).unwrap(),
            source_before["timing"]
        );
        assert_eq!(
            h.state().practice_history.total_practiced_ms("scale:Major"),
            0
        );
        h.update(|ui| {
            ui.set.select_id(added);
            ui.now_ms = Some(2_000);
        });
        assert_eq!(
            h.state().preview_voicing(),
            heard,
            "rehearsal consumes the material auditioned during discovery"
        );
        assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
        assert!(h.state().rehearsal_running);
        h.update(|ui| ui.now_ms = Some(3_000));
        assert!(h.click_on(&Selector::class("t-btn").containing("Pause")));
        assert!(!h.state().rehearsal_running);
        assert_eq!(
            h.state().practice_history.total_practiced_ms("scale:Major"),
            1_000
        );
        assert_eq!(
            h.state()
                .practice_history
                .total_practiced_ms("chord:Major 7"),
            0
        );
    }
}

#[test]
fn reopened_scale_editor_has_no_empty_chord_discovery_panels() {
    use layout_dom_api::LayoutDom;
    let mut h = harness(1_100.0, 900.0);
    h.update(|ui| {
        ui.section = woodshed_core::storage::AppSection::Rehearsal;
        ui.set.cards[0].material = woodshedding::rehearsal::Material::Scale {
            name: "Major".into(),
            root: woodshedding::pitch::PitchClass::new(0),
        };
    });
    let dom = h.runner().dom();
    let dom = dom.borrow();
    assert!(
        taproot::matching(
            &dom,
            &Selector::class("t-btn").containing("Explore compatible scales")
        )
        .is_empty(),
        "an ordinary scale must not render chord-to-scale discovery"
    );
    assert!(
        taproot::matching(&dom, &Selector::class("t-btn").containing("Discover ")).is_empty(),
        "an ordinary scale must not render chord-to-arpeggio discovery"
    );
    assert_eq!(
        taproot::matching(&dom, &Selector::class("stage-context-panel")).len(),
        1,
        "only the populated scale-pattern panel should remain; no empty styled chord panels"
    );
    assert_eq!(
        taproot::matching(&dom, &Selector::class("scale-patterns")).len(),
        1
    );
    assert_eq!(
        taproot::matching(&dom, &Selector::class("scale-pattern-choice")).len(),
        2,
        "thirds and fourths are useful scale actions rather than an empty panel"
    );
    let panel = taproot::matching(&dom, &Selector::class("scale-patterns"))[0];
    assert!(
        dom.dom_children(panel).any(|child| {
            dom.dom_children(child)
                .filter_map(|node| dom.text(node))
                .any(|text| text.contains("seven-note formulas are supported"))
        }),
        "the populated scale recipe panel explains its scope"
    );
}

#[test]
fn discovered_scale_uses_its_saved_ukulele_board_and_physical_solo_contact() {
    use woodshed_core::{audio::AudioRequest, storage::AppSection};
    use woodshedding::rehearsal::{FretWindow, Hold, MarkMode};
    let mut h = harness(700.0, 1_500.0);
    h.update(|ui| {
        ui.set.cards.clear();
        ui.stage.set_root(3);
        ui.root_dd.selected = 3;
        let chord = ui
            .stage
            .chords()
            .iter()
            .position(|chord| chord.name == "Major 7")
            .unwrap();
        ui.stage.select_chord(chord);
        ui.stage_current(None);
        let card = &mut ui.set.cards[0];
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Standard (high-G)".into());
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 2, span: 4 });
        card.timing.bpm = None;
        card.timing.hold = Hold::Bars(1);
        ui.transport.bpm = 80.0;
        ui.select_app_section(AppSection::Rehearsal);
    });
    assert!(h.click_on(&Selector::class("card-shape-next")));
    let source = h.state().set.cards[0].id;
    let source_before = serde_json::to_value(&h.state().set.cards[0]).unwrap();
    assert!(h.click_on(&Selector::class("t-btn").containing("Explore compatible scales")));
    assert!(h.click_on(&Selector::class("scale-choice").containing("C Major")));
    assert!(h.click_on(&Selector::class("t-btn").containing("Hear scale")));
    let heard = match h.state().audio_requests.last().unwrap() {
        AudioRequest::PreviewPitches {
            pitches,
            duration_s,
            strum_s,
        } => (pitches.clone(), *duration_s, *strum_s),
        request => panic!("unexpected scale audition: {request:?}"),
    };
    assert!(h.click_on(&Selector::class("t-btn").containing("Stage scale")));
    let scale_id = h.state().set.cards[1].id;
    assert_ne!(source, scale_id);
    assert_eq!(
        serde_json::to_value(&h.state().set.cards[0]).unwrap(),
        source_before
    );
    h.update(|ui| {
        ui.set.select_id(scale_id);
    });
    assert_eq!(
        h.state().stage.string_count(),
        6,
        "live Guitar must not reinterpret the Ukulele Card"
    );
    let geom = h.state().rehearsal_board_geometry();
    assert_eq!(
        (geom.string_count, geom.fret_start, geom.fret_count),
        (4, 2, 6)
    );
    let board = rect(&h, "fretboard-stack");
    let board_size = geom.size_u32();
    assert_eq!(
        (board.2, board.3),
        (board_size.0 as f32, board_size.1 as f32)
    );
    assert_eq!(h.state().preview_voicing(), heard);
    let dwell =
        woodshed_core::card_dwell(&h.state().set.cards[1], h.state().transport.bpm).unwrap();
    assert!(
        (dwell.as_secs_f32() - 3.0).abs() < 0.001,
        "one bar inherits the 80 BPM runner tempo"
    );
    assert!(
        (heard.1 - dwell.as_secs_f32()).abs() < 0.001,
        "sequential sound finishes within the runner's inherited-tempo bar"
    );
    let resolved = h
        .state()
        .stage
        .scale_card_realization(&h.state().set.cards[1])
        .unwrap();
    let dot = resolved
        .dots
        .iter()
        .find(|dot| dot.string_index == 1 && dot.fret == 2)
        .unwrap();
    let d4_hz = 440.0 * 2.0_f32.powf((62.0 - 69.0) / 12.0);
    assert!((dot.frequency - d4_hz).abs() < 0.001);
    let label = woodshed_core::marker_a11y_label(dot, 4);
    assert!(h.click_on(&Selector::class("fret-label").with_attr("aria-label", label)));
    assert_eq!(
        h.state().set.cards[1].setting.marked,
        vec![(1, 2)],
        "the painted physical note edits its actual contact"
    );
    assert!(h.click_on(&Selector::class("seg").containing("Solo")));
    assert_eq!(h.state().set.cards[1].setting.mark_mode, MarkMode::Solo);
    let solo = h.state().preview_voicing();
    assert_eq!(solo.0.len(), 1);
    assert!(
        (solo.0[0] - d4_hz).abs() < 0.001,
        "Solo resolves D4 from the saved Ukulele contact"
    );
    h.update(|ui| {
        ui.audio_requests.clear();
        ui.now_ms = Some(1_000);
    });
    assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
    assert!(
        matches!(h.state().audio_requests.last(), Some(AudioRequest::PreviewPitches { pitches, .. })
        if pitches.len() == 1 && (pitches[0] - d4_hz).abs() < 0.001)
    );
    h.update(|ui| ui.now_ms = Some(2_000));
    assert!(h.click_on(&Selector::class("t-btn").containing("Pause")));
    assert_eq!(
        h.state().practice_history.total_practiced_ms("scale:Major"),
        1_000
    );
}

#[test]
fn invalid_saved_scale_setup_shows_the_reason_and_cannot_sound_the_live_guitar() {
    use woodshed_core::storage::AppSection;
    use woodshedding::{pitch::PitchClass, rehearsal::Material};
    let mut h = harness(700.0, 1_100.0);
    h.update(|ui| {
        ui.set.cards[0].material = Material::Scale {
            name: "Major".into(),
            root: PitchClass::new(0),
        };
        ui.set.cards[0].setting.instrument = "Ukulele".into();
        ui.set.cards[0].setting.tuning = Some("missing-test-tuning".into());
        ui.select_app_section(AppSection::Rehearsal);
        ui.audio_requests.clear();
    });
    assert_eq!(h.state().stage.string_count(), 6);
    assert!(
        !h.state().stage.voicing_preview().0.is_empty(),
        "live Guitar has sound, making the fallback regression observable"
    );
    let unavailable = {
        let dom = h.runner().dom();
        let dom = dom.borrow();
        taproot::matching(
            &dom,
            &Selector::class("scale-setup-unavailable").containing("missing-test-tuning"),
        )
    };
    assert!(
        !unavailable.is_empty(),
        "the unavailable persisted setup needs a visible reason"
    );
    let labels = {
        let dom = h.runner().dom();
        let dom = dom.borrow();
        taproot::matching(&dom, &Selector::class("fret-label"))
    };
    assert!(
        labels.is_empty(),
        "an invalid scale cannot present clickable live-Guitar contacts"
    );
    assert_eq!(h.state().rehearsal_board_geometry().string_count, 0);
    assert!(h.state().preview_voicing().0.is_empty());
    assert!(h.click_on(&Selector::class("t-btn").containing("♪ Hear")));
    assert!(
        h.state().preview_voicing().0.is_empty(),
        "the host's audition seam remains silent"
    );
    assert!(
        h.state()
            .audio_requests
            .iter()
            .all(|request| !matches!(request,
        woodshed_core::audio::AudioRequest::PreviewPitches { pitches, .. } if !pitches.is_empty()))
    );
}

#[test]
fn scale_pattern_controls_preserve_order_source_and_occurrences_in_wide_and_narrow_views() {
    use woodshed_core::{audio::AudioRequest, history::catalog_id_for_card, storage::AppSection};
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{FretWindow, Hold, Material, ScalePattern, Touch},
    };
    for width in [1_100.0, 420.0] {
        for (pattern, button, expected_prefix) in [
            (
                ScalePattern::Thirds,
                "Inspect diatonic thirds",
                vec![62, 66, 64, 67, 66, 69],
            ),
            (
                ScalePattern::Fourths,
                "Inspect diatonic fourths",
                vec![62, 67, 64, 69, 66, 71],
            ),
        ] {
            let mut h = harness(width, 1_500.0);
            h.update(|ui| {
                let card = &mut ui.set.cards[0];
                card.label = "C Major on Ukulele".into();
                card.material = Material::Scale {
                    name: "Major".into(),
                    root: PitchClass::new(0),
                };
                card.setting.instrument = "Ukulele".into();
                card.setting.tuning = Some("Standard (high-G)".into());
                card.setting.capo = Some(2);
                card.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
                card.touch = Touch::Walk;
                card.timing.bpm = Some(90.0);
                card.timing.hold = Hold::Bars(1);
                ui.select_app_section(AppSection::Rehearsal);
                ui.audio_requests.clear();
            });
            let source = h.state().set.cards[0].id;
            let before = serde_json::to_value(&h.state().set.cards[0]).unwrap();
            assert!(h.click_on(&Selector::class("t-btn").containing(button)));
            assert_eq!(h.state().set.cards.len(), 1);
            assert!(h.click_on(&Selector::class("t-btn").containing("Hear pattern")));
            assert_eq!(
                h.state().set.cards.len(),
                1,
                "pattern audition cannot author the source"
            );
            let heard = match h.state().audio_requests.last().unwrap() {
                AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s,
                } => (pitches.clone(), *duration_s, *strum_s),
                request => panic!("unexpected pattern audition: {request:?}"),
            };
            let midi = heard
                .0
                .iter()
                .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
                .collect::<Vec<_>>();
            assert_eq!(
                &midi[..6],
                expected_prefix.as_slice(),
                "degree-pair contour must survive actual audio requests"
            );
            assert!(
                midi.windows(2).any(|pair| pair[1] < pair[0]),
                "ascending pairs require a nonmonotonic return between pairs"
            );
            assert!(
                midi.iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    < midi.len(),
                "shared degrees are visited repeatedly"
            );
            assert!(h.click_on(&Selector::class("t-btn").containing("Stage pattern")));
            assert!(h.click_on(&Selector::class("t-btn").containing("Stage pattern")));
            assert_eq!(h.state().set.cards.len(), 3);
            let first = h.state().set.cards[1].id;
            let second = h.state().set.cards[2].id;
            assert_ne!(source, first);
            assert_ne!(first, second);
            assert_eq!(
                serde_json::to_value(&h.state().set.cards[0]).unwrap(),
                before
            );
            assert!(
                matches!(&h.state().set.cards[1].material, Material::ScalePattern { name, root, pattern: saved } if name == "Major" && *root == PitchClass::new(0) && *saved == pattern)
            );
            h.update(|ui| {
                ui.set.select_id(first);
                ui.now_ms = Some(1_000);
            });
            assert_eq!(h.state().stage.string_count(), 6);
            assert_eq!(h.state().rehearsal_board_geometry().string_count, 4);
            assert_eq!(
                h.state().preview_voicing(),
                heard,
                "rehearsal must preserve ordered repeated pattern visits"
            );
            let subject = catalog_id_for_card(&h.state().set.cards[1]).unwrap();
            assert_ne!(
                subject, "scale:Major",
                "the exercise cannot masquerade as ordinary scale history"
            );
            assert_eq!(h.state().practice_history.total_practiced_ms(&subject), 0);
            assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
            assert!(
                matches!(h.state().audio_requests.last(), Some(AudioRequest::PreviewPitches { pitches, .. }) if *pitches == heard.0)
            );
            h.update(|ui| ui.now_ms = Some(2_500));
            assert!(h.click_on(&Selector::class("t-btn").containing("Pause")));
            assert_eq!(
                h.state().practice_history.total_practiced_ms(&subject),
                1_500
            );
            assert_eq!(
                h.state().practice_history.total_practiced_ms("scale:Major"),
                0
            );
        }
    }
}

#[test]
fn inspected_pattern_revalidates_removed_and_invalid_sources_before_authoring() {
    use woodshed_core::storage::AppSection;
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{FretWindow, Material, Touch},
    };
    for remove_source in [false, true] {
        let mut h = harness(700.0, 1_500.0);
        h.update(|ui| {
            let card = &mut ui.set.cards[0];
            card.material = Material::Scale {
                name: "Major".into(),
                root: PitchClass::new(0),
            };
            card.setting.instrument = "Ukulele".into();
            card.setting.tuning = Some("Standard (high-G)".into());
            card.setting.capo = Some(2);
            card.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
            card.touch = Touch::Walk;
            ui.set.duplicate(0);
            ui.set.cursor = 0;
            ui.select_app_section(AppSection::Rehearsal);
        });
        let inspected = h.state().set.cards[0].id;
        assert!(h.click_on(&Selector::class("t-btn").containing("Inspect diatonic thirds")));
        assert!(h.click_on(&Selector::class("t-btn").containing("Hear pattern")));
        assert!(!h.state().audio_requests.is_empty());
        h.update(|ui| {
            ui.audio_requests.clear();
            if remove_source {
                ui.set.remove(0);
            } else {
                ui.set.cards[0].setting.tuning = Some("missing-test-tuning".into());
            }
            // The retained action resolves the original occurrence again,
            // rather than substituting the duplicate now under the cursor.
            ui.hear_scale_pattern();
            assert!(ui.stage_scale_pattern().is_none());
        });
        assert_eq!(h.state().pattern_source, Some(inspected));
        assert_eq!(h.state().set.cards.len(), if remove_source { 1 } else { 2 });
        assert!(h.state().audio_requests.is_empty());
        assert!(
            !h.click_on(&Selector::class("t-btn").containing("Stage pattern")),
            "unavailable retained recipe must not offer an authoring action"
        );
        let reason = if remove_source {
            "no longer"
        } else {
            "missing-test-tuning"
        };
        let unavailable = {
            let dom = h.runner().dom();
            let dom = dom.borrow();
            taproot::matching(
                &dom,
                &Selector::class("scale-pattern-unavailable").containing(reason),
            )
        };
        assert!(
            !unavailable.is_empty(),
            "the production inspector explains why the retained source cannot be used"
        );
    }
}

fn approach_harness(width: f32) -> Harness<UiState, crate::sync::Logic, UiChild> {
    use woodshed_core::storage::AppSection;
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{CardId, FretWindow, Hold, Material, Touch},
    };
    let mut h = harness(width, 1_500.0);
    h.update(|ui| {
        let source = &mut ui.set.cards[0];
        source.label = "Am7 source".into();
        source.material = Material::Chord {
            name: "Minor 7".into(),
            root: PitchClass::new(9),
        };
        source.setting.instrument = "Ukulele".into();
        source.setting.tuning = Some("Standard (high-G)".into());
        source.setting.capo = Some(2);
        source.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        source.touch = Touch::Block;
        source.timing.bpm = Some(90.0);
        source.timing.hold = Hold::Bars(1);
        let mut target = source.clone();
        target.id = CardId::UNASSIGNED;
        target.label = "Cmaj7 target".into();
        target.material = Material::Chord {
            name: "Major 7".into(),
            root: PitchClass::new(0),
        };
        ui.set.push(target);
        ui.set.cursor = 1;
        ui.select_app_section(AppSection::Rehearsal);
        ui.audio_requests.clear();
    });
    assert!(h.click_on(&Selector::class("card-shape-next")));
    h.update(|ui| ui.set.cursor = 0);
    h
}

#[test]
fn adjacent_chord_approach_controls_preserve_physical_pairs_shapes_and_originals() {
    use woodshed_core::{CardShapeStatus, audio::AudioRequest, history::catalog_id_for_card};
    use woodshedding::rehearsal::{ApproachDirection, Material};
    for width in [1_100.0, 420.0] {
        for (direction, button, shift) in [
            (ApproachDirection::Below, "Inspect from below", -1_i32),
            (ApproachDirection::Above, "Inspect from above", 1_i32),
        ] {
            let mut h = approach_harness(width);
            let originals = serde_json::to_value(&h.state().set.cards).unwrap();
            let CardShapeStatus::Available(shape) =
                h.state().stage.card_shape_status(&h.state().set.cards[1])
            else {
                panic!("target selected shape must resolve");
            };
            // Independent expected pitches from the known high-G open strings
            // and the selected shape's physical contacts, including capo-bound
            // omissions. The approach realizer is not used to build this oracle.
            let opens = [67_i32, 60, 64, 69];
            let mut targets = shape
                .physical_positions()
                .into_iter()
                .map(|(string, fret)| (opens[string] + i32::from(fret), fret, string))
                .collect::<Vec<_>>();
            targets.sort();
            let expected = targets
                .iter()
                .filter_map(|&(target_midi, fret, _)| {
                    let partner = i32::from(fret) + shift;
                    (partner >= 2 && partner <= 14).then_some([target_midi + shift, target_midi])
                })
                .flatten()
                .collect::<Vec<_>>();
            assert!(!expected.is_empty());
            assert!(h.click_on(&Selector::class("chord-approach-choice").containing(button)));
            assert_eq!(h.state().set.cards.len(), 2);
            assert!(h.click_on(&Selector::class("t-btn").containing("Hear approach")));
            let heard = match h.state().audio_requests.last().unwrap() {
                AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s,
                } => (pitches.clone(), *duration_s, *strum_s),
                request => panic!("unexpected approach audio request: {request:?}"),
            };
            let midi = heard
                .0
                .iter()
                .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
                .collect::<Vec<_>>();
            assert_eq!(
                midi, expected,
                "the audible recipe must preserve each same-string chromatic approach and target in order"
            );
            if direction == ApproachDirection::Above {
                assert!(
                    midi.windows(2).any(|pair| pair[1] < pair[0]),
                    "above approaches must descend into their target"
                );
            }
            assert_eq!(h.state().set.cards.len(), 2);
            assert!(h.click_on(&Selector::class("t-btn").containing("Append approach exercise")));
            assert!(h.click_on(&Selector::class("t-btn").containing("Append approach exercise")));
            assert_eq!(h.state().set.cards.len(), 4);
            assert_eq!(
                serde_json::to_value(&h.state().set.cards[..2]).unwrap(),
                originals
            );
            assert_eq!(
                h.state().set.cursor,
                0,
                "explicit append must not retarget the inspected original pair"
            );
            let first = h.state().set.cards[2].id;
            assert_ne!(first, h.state().set.cards[3].id);
            assert!(
                matches!(&h.state().set.cards[2].material, Material::ChordApproach { name, root, direction: saved } if name == "Major 7" && root.value() == 0 && *saved == direction)
            );
            assert_eq!(
                h.state().set.cards[2].setting.voicing_fingerprint,
                h.state().set.cards[1].setting.voicing_fingerprint,
                "recipe retains the target's exact selected shape"
            );
            h.update(|ui| {
                ui.set.select_id(first);
                ui.now_ms = Some(1_000);
            });
            assert_eq!(h.state().stage.string_count(), 6);
            assert_eq!(h.state().rehearsal_board_geometry().string_count, 4);
            assert_eq!(h.state().preview_voicing(), heard);
            let subject = catalog_id_for_card(&h.state().set.cards[2]).unwrap();
            assert_ne!(subject, "chord:Major 7");
            assert!(h.click_on(&Selector::class("t-btn").containing("Run")));
            assert!(
                matches!(h.state().audio_requests.last(), Some(AudioRequest::PreviewPitches { pitches, .. }) if *pitches == heard.0)
            );
            h.update(|ui| ui.now_ms = Some(2_500));
            assert!(h.click_on(&Selector::class("t-btn").containing("Pause")));
            assert_eq!(
                h.state().practice_history.total_practiced_ms(&subject),
                1_500
            );
            assert_eq!(
                h.state()
                    .practice_history
                    .total_practiced_ms("chord:Major 7"),
                0
            );
            assert_eq!(
                h.state()
                    .practice_history
                    .total_practiced_ms("chord:Minor 7"),
                0
            );
        }
    }
}

#[test]
fn inspected_chord_approach_rejects_stale_pairs_and_saved_shape_failures() {
    use woodshedding::rehearsal::ApproachDirection;
    for change in [
        "remove-source",
        "remove-target",
        "reorder",
        "insert-between",
        "invalid-shape",
    ] {
        let mut h = approach_harness(700.0);
        let source = h.state().set.cards[0].id;
        let target = h.state().set.cards[1].id;
        assert!(
            h.click_on(&Selector::class("chord-approach-choice").containing("Inspect from above"))
        );
        assert!(h.click_on(&Selector::class("t-btn").containing("Hear approach")));
        assert!(!h.state().audio_requests.is_empty());
        let mut count_after_change = 0;
        h.update(|ui| {
            ui.audio_requests.clear();
            match change {
                "remove-source" => {
                    ui.set.remove(0);
                },
                "remove-target" => {
                    ui.set.remove(1);
                },
                "reorder" => ui.set.cards.swap(0, 1),
                "insert-between" => {
                    ui.set.duplicate(0);
                },
                "invalid-shape" => {
                    ui.set.cards[1].setting.voicing_fingerprint = Some("stale-test-shape".into())
                },
                _ => unreachable!(),
            }
            count_after_change = ui.set.cards.len();
            // Retained commands must resolve the originally inspected IDs and
            // shape again, never the new adjacent Card or a formula fallback.
            ui.hear_chord_approach();
            assert!(ui.stage_chord_approach().is_none(), "{change}");
        });
        assert_eq!(h.state().set.cards.len(), count_after_change);
        assert!(
            h.state().audio_requests.is_empty(),
            "{change} cannot sound an obsolete recipe"
        );
        assert!(!h.click_on(&Selector::class("t-btn").containing("Append approach exercise")));
        let unavailable = {
            let dom = h.runner().dom();
            let dom = dom.borrow();
            taproot::matching(&dom, &Selector::class("chord-approach-unavailable"))
        };
        assert!(
            !unavailable.is_empty(),
            "{change} needs a visible unavailable reason"
        );
        // Selecting the same recipe again with the stale explicit pair also
        // fails, even when an ordinary duplicate is now adjacent.
        h.update(|ui| {
            assert!(!ui.inspect_chord_approach(source, target, ApproachDirection::Above))
        });
    }
}

#[test]
fn unselected_target_approach_keeps_repeated_pitch_visits_in_production_audio() {
    use woodshed_core::audio::AudioRequest;
    let mut h = approach_harness(700.0);
    h.update(|ui| ui.set.cursor = 1);
    assert!(h.click_on(&Selector::class("card-shape-clear")));
    h.update(|ui| ui.set.cursor = 0);
    assert!(h.click_on(&Selector::class("chord-approach-choice").containing("Inspect from above")));
    assert!(h.click_on(&Selector::class("t-btn").containing("Hear approach")));
    let heard = match h.state().audio_requests.last().unwrap() {
        AudioRequest::PreviewPitches {
            pitches,
            duration_s,
            strum_s,
        } => (pitches.clone(), *duration_s, *strum_s),
        request => panic!("unexpected approach audio request: {request:?}"),
    };
    let midi = heard
        .0
        .iter()
        .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
        .collect::<Vec<_>>();
    assert!(midi.chunks_exact(2).all(|pair| pair[0] == pair[1] + 1));
    assert!(
        midi.iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            < midi.len(),
        "the semitone above a major seventh repeats an octave root used by a later pair"
    );
    assert!(h.click_on(&Selector::class("t-btn").containing("Append approach exercise")));
    h.update(|ui| ui.set.cursor = 2);
    assert_eq!(
        h.state().preview_voicing(),
        heard,
        "staging cannot sort or deduplicate the repeated recipe visits"
    );
}

#[test]
fn relationship_recipe_controls_compile_explain_and_return_to_exact_source_at_two_widths() {
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    for width in [1100.0, 420.0] {
        let mut h = harness(width, 900.0);
        h.update(|ui| {
            ui.set = Default::default();
            for (label, formula) in [("First", "Major"), ("Again", "Major"), ("Third", "Minor")] {
                let mut card = KeyedCatalogRef {
                    formula_id: format!("chord:{formula}"),
                    root: PitchClass::new(0),
                }
                .to_card()
                .unwrap();
                card.label = label.into();
                ui.set.push(card);
            }
            ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Overview);
        });
        assert!(h.click_on(&Selector::class("overview-relationship")));
        assert!(h.click_on(&Selector::class("relationship-bind")));
        let source = serde_json::to_value(&h.state().set).unwrap();
        assert!(h.click_on(&Selector::class("relationship-spacing")));
        assert_eq!(
            h.state()
                .relationship_reading()
                .unwrap()
                .unwrap()
                .snapshot
                .recipe
                .definition
                .arrangement
                .spacing,
            24
        );
        assert!(h.click_on(
            &Selector::class("graph-canvas-swatch-node").with_attr("data-key", "set:1:card:3")
        ));
        assert_eq!(
            h.state()
                .relationship_reading()
                .unwrap()
                .unwrap()
                .snapshot
                .selected_occurrence
                .as_deref(),
            Some("set:1:card:3"),
            "last graph node at width {width}"
        );
        assert!(h.click_on(
            &Selector::class("graph-canvas-swatch-node").with_attr("data-key", "set:1:card:2")
        ));
        assert_eq!(
            h.state()
                .relationship_reading()
                .unwrap()
                .unwrap()
                .snapshot
                .selected_occurrence
                .as_deref(),
            Some("set:1:card:2"),
            "compiled graph node at width {width}"
        );
        assert!(h.click_on(&Selector::class("relationship-explain")));
        assert_eq!(class_count(&h, "relationship-explanation"), 1);
        assert!(h.click_on(&Selector::class("relationship-occurrence").containing("Again")));
        assert_eq!(serde_json::to_value(&h.state().set).unwrap(), source);
        assert!(h.click_on(&Selector::class("relationship-source")));
        assert_eq!(
            h.state().set.cursor_id(),
            Some(woodshedding::rehearsal::CardId(2))
        );
        assert!(!h.state().rehearsal_running);
    }
}

#[test]
fn exact_tone_relationship_controls_rebind_and_open_the_containing_scale_at_two_widths() {
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    for width in [1100.0, 420.0] {
        let mut h = harness(width, 900.0);
        h.update(|ui| {
            ui.set = Default::default();
            for (name, formula, root) in [
                ("Cmaj7", "chord:Major 7", 0),
                ("Am7", "chord:Minor 7", 9),
                ("C Major scale", "scale:Major", 0),
            ] {
                let mut card = KeyedCatalogRef {
                    formula_id: formula.into(),
                    root: PitchClass::new(root),
                }
                .to_card()
                .unwrap();
                card.label = name.into();
                ui.set.push(card);
            }
            ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Overview);
        });
        let before = serde_json::to_value(&h.state().set).unwrap();
        assert!(h.click_on(&Selector::class("overview-relationship")));
        assert!(h.click_on(&Selector::class("relationship-bind")));
        assert!(h.click_on(
            &Selector::class("relationship-explain").containing("Pitch-class differences")
        ));
        let reading = h.state().relationship_reading().unwrap().unwrap();
        let relation = reading
            .dataset
            .relationships
            .iter()
            .find(|r| Some(&r.id) == reading.snapshot.selected_relationship.as_ref())
            .unwrap();
        assert!(relation.explanation.contains("Only in Cmaj7: B."));
        assert!(relation.explanation.contains("Only in Am7: A."));
        assert!(h.click_on(&Selector::class("relationship-material").containing("Selected 1")));
        assert!(h.click_on(&Selector::class("relationship-bind")));
        assert!(h.click_on(
            &Selector::class("relationship-explain").containing("Pitch-class containment")
        ));
        let reading = h.state().relationship_reading().unwrap().unwrap();
        assert_eq!(
            reading.snapshot.selected_occurrence.as_deref(),
            Some("set:1:card:3")
        );
        assert_eq!(serde_json::to_value(&h.state().set).unwrap(), before);
        assert!(h.click_on(&Selector::class("relationship-source")));
        assert_eq!(
            h.state().set.cursor_id(),
            Some(woodshedding::rehearsal::CardId(3))
        );
        assert!(!h.state().rehearsal_running);
    }
}
