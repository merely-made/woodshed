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
    assert!(taproot::matching(&dom, &Selector::class("stage-context-panel")).is_empty());
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
