use woodshed_core::audio::AudioRequest;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::settings::StageGraphReading;
use woodshedding::pitch::PitchClass;

use super::{UiState, set_graph_snapshot, set_graph_swatch_from_snapshot};

fn keyed(formula_id: &str, root: u8) -> KeyedCatalogRef {
    KeyedCatalogRef {
        formula_id: formula_id.into(),
        root: PitchClass::new(root),
    }
}

fn c_major() -> KeyedCatalogRef {
    keyed("chord:Major", 0)
}

fn a_minor() -> KeyedCatalogRef {
    keyed("chord:Minor", 9)
}

#[test]
fn tonnetz_focus_expands_without_moving_or_authoring_material() {
    use cambium::GraphCanvasEvent;
    use woodshed_core::stage_context::StageNodeId;
    let mut ui = UiState::new();
    assert!(ui.focus_context_catalog(c_major()));
    ui.stage_current(None);
    ui.set_graph_reading(StageGraphReading::Tonnetz);
    let before = set_graph_snapshot(&ui);
    let swatch = set_graph_swatch_from_snapshot(&before, &ui, true);
    let target = keyed("chord:Minor", 4);
    let instance = before
        .instance_of_node(&StageNodeId::Catalog(target.clone()))
        .unwrap();
    ui.handle_set_graph_event(
        &before,
        GraphCanvasEvent::Activate(super::StageInstanceRef {
            epoch: before.epoch(),
            instance,
        }),
    );
    ui.sync();
    assert_eq!(ui.set.cards.len(), 1);
    assert_eq!(ui.context_focus.as_ref(), Some(&target));
    assert!(ui.audio_requests.is_empty());
    let after = set_graph_snapshot(&ui);
    let after_swatch = set_graph_swatch_from_snapshot(&after, &ui, true);
    assert!(after.nodes.len() > before.nodes.len());
    for node in &swatch.graph.nodes {
        let id = &before.node_of(node.id.instance).unwrap().id;
        let new_instance = after
            .instance_of_node(id)
            .expect("disclosed material retained");
        assert_eq!(
            after_swatch
                .graph
                .nodes
                .iter()
                .find(|node| node.id.instance == new_instance)
                .unwrap()
                .position,
            node.position
        );
    }
}

#[test]
fn focusing_a_keyed_material_preserves_root_through_sync() {
    let mut ui = UiState::new();
    assert!(ui.focus_context_catalog(c_major()));
    ui.sync();
    assert_eq!(ui.stage.root_idx, 3);
    assert_eq!(ui.root_dd.selected, 3);
    assert_eq!(ui.stage.catalog_id().as_deref(), Some("chord:Major"));
}

#[test]
fn context_audition_queues_immutable_pitches_after_board_changes() {
    let mut ui = UiState::new();
    assert!(ui.focus_context_catalog(c_major()));
    ui.audition_context_focus();
    let expected = match ui.audio_requests.first().expect("preview request") {
        AudioRequest::PreviewPitches { pitches, .. } => pitches.clone(),
        other => panic!("unexpected request: {other:?}"),
    };
    ui.stage.set_lens(woodshed_core::Lens::Scales);
    ui.stage.set_root(9);
    ui.section = woodshed_core::storage::AppSection::Rehearsal;
    match &ui.audio_requests[0] {
        AudioRequest::PreviewPitches { pitches, .. } => assert_eq!(pitches, &expected),
        other => panic!("unexpected request: {other:?}"),
    }
}

#[test]
fn adding_focused_material_resolves_focus_to_its_new_foreground_occurrence() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    assert!(ui.focus_context_catalog(a_minor()));
    ui.add_context_focus_to_set();
    let snapshot = set_graph_snapshot(&ui);
    let focused = snapshot
        .nodes
        .iter()
        .find(|node| node.foreground && node.keyed.as_ref() == Some(&a_minor()))
        .expect("new foreground occurrence");
    assert!(focused.material.is_some());
    let swatch = set_graph_swatch_from_snapshot(&snapshot, &ui, true);
    assert_eq!(
        swatch.focus.map(|reference| reference.instance),
        snapshot.instance_of_node(&focused.id)
    );
}

#[test]
fn hiding_context_preserves_staged_normalized_positions() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.stage_current(None);
    ui.app_settings.stage.set_graph_reading = StageGraphReading::CircleOfFifths;
    let shown_snapshot = set_graph_snapshot(&ui);
    let shown = set_graph_swatch_from_snapshot(&shown_snapshot, &ui, true);
    let before: Vec<_> = shown
        .graph
        .nodes
        .iter()
        .filter_map(|node| {
            shown_snapshot
                .node_of(node.id.instance)
                .filter(|item| item.foreground)
                .map(|item| (item.id.clone(), node.position))
        })
        .collect();
    ui.app_settings.stage.context.enabled = false;
    let hidden_snapshot = set_graph_snapshot(&ui);
    let hidden = set_graph_swatch_from_snapshot(&hidden_snapshot, &ui, true);
    assert!(!before.is_empty());
    for (id, position) in before {
        let instance = hidden_snapshot
            .instance_of_node(&id)
            .expect("staged node survives");
        let node = hidden
            .graph
            .nodes
            .iter()
            .find(|node| node.id.instance == instance)
            .expect("staged node is rendered");
        assert_eq!(node.position, position);
    }
}

#[test]
fn circle_context_keeps_glyph_centres_smaller_than_hit_rects() {
    let mut ui = UiState::new();
    assert!(ui.focus_context_catalog(c_major()));
    ui.stage_current(None);
    ui.set_graph_reading(StageGraphReading::CircleOfFifths);
    let swatch = set_graph_swatch_from_snapshot(&set_graph_snapshot(&ui), &ui, true);
    assert_eq!((swatch.width, swatch.height), (520, 520));
    assert!(swatch.graph.nodes.len() >= 13);
    assert!(swatch.hit_size > swatch.node_radius * 2.0);
    assert!(
        swatch
            .graph
            .nodes
            .iter()
            .all(|node| { node.position.0.is_finite() && node.position.1.is_finite() })
    );
}

#[test]
fn nearby_refresh_keeps_focus_without_authoring_or_auditioning() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    assert!(ui.focus_context_catalog(c_major()));
    ui.stage_current(None);
    ui.app_settings.stage.set_graph_reading = StageGraphReading::CircleOfFifths;
    let before_cards = serde_json::to_value(&ui.set.cards).unwrap();
    let before_focus = ui.context_focus.clone();
    ui.show_nearby_context();
    assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before_cards);
    assert_eq!(ui.context_focus, before_focus);
    assert!(ui.audio_requests.is_empty());
    assert!(!ui.context_disclosed.is_empty());
}

#[test]
fn saturated_nearby_refresh_admits_an_unseen_ranked_triad() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    assert!(ui.focus_context_catalog(c_major()));
    ui.stage_current(None);
    ui.set_graph_reading(StageGraphReading::CircleOfFifths);
    ui.app_settings.stage.context.node_limit = 36;
    ui.context_focus = Some(c_major());
    ui.context_disclosed = ["scale:Major", "scale:Minor", "chord:Major 7"]
        .into_iter()
        .flat_map(|formula| (0..12).map(move |root| keyed(formula, root)))
        .collect();
    let before = set_graph_snapshot(&ui);
    assert_eq!(
        before.nodes.iter().filter(|node| !node.foreground).count(),
        36
    );
    let before_positions = set_graph_swatch_from_snapshot(&before, &ui, true)
        .graph
        .nodes
        .into_iter()
        .map(|node| (node.key, node.position))
        .collect::<std::collections::BTreeMap<_, _>>();
    let displayed = before
        .nodes
        .iter()
        .filter_map(|node| node.keyed.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let authored = ui.set.cards.len();
    let ranked = woodshed_core::stage_candidates::ranked_candidates(
        StageGraphReading::CircleOfFifths,
        &c_major(),
        &std::collections::BTreeSet::new(),
    );
    let unseen = ranked
        .iter()
        .find(|candidate| !displayed.contains(&candidate.keyed))
        .expect("bounded context leaves a ranked frontier")
        .keyed
        .clone();
    ui.show_nearby_context();
    let after = set_graph_snapshot(&ui);
    let after_swatch = set_graph_swatch_from_snapshot(&after, &ui, true);
    assert!(
        after
            .nodes
            .iter()
            .any(|node| node.keyed.as_ref() == Some(&unseen))
    );
    assert_eq!(ui.set.cards.len(), authored);
    let common = after_swatch
        .graph
        .nodes
        .iter()
        .filter_map(|node| {
            before_positions
                .get(&node.key)
                .map(|position| (*position, node.position))
        })
        .collect::<Vec<_>>();
    assert!(
        !common.is_empty(),
        "refresh discarded every existing placement"
    );
    assert!(common.iter().all(|(before, after)| before == after));
}

#[test]
fn pitch_motion_focus_preserves_anchor_and_projected_positions_until_recenter() {
    let mut ui = UiState::new();
    assert!(ui.focus_context_catalog(c_major()));
    ui.stage_current(None);
    ui.set_graph_reading(StageGraphReading::PitchMotion);
    assert_eq!(ui.pitch_motion_anchor, Some(c_major()));
    let before = set_graph_snapshot(&ui);
    let positions = set_graph_swatch_from_snapshot(&before, &ui, true)
        .graph
        .nodes
        .into_iter()
        .map(|node| (node.key, node.position))
        .collect::<std::collections::BTreeMap<_, _>>();
    let cards = serde_json::to_string(&ui.set.cards).unwrap();
    ui.context_disclosed
        .extend(before.nodes.iter().filter_map(|node| node.keyed.clone()));
    assert!(ui.focus_context_catalog(a_minor()));
    ui.show_nearby_context();
    assert_eq!(ui.pitch_motion_anchor, Some(c_major()));
    assert_eq!(serde_json::to_string(&ui.set.cards).unwrap(), cards);
    assert!(ui.audio_requests.is_empty());
    let after = set_graph_snapshot(&ui);
    let projected = set_graph_swatch_from_snapshot(&after, &ui, true);
    let common = projected
        .graph
        .nodes
        .iter()
        .filter_map(|node| positions.get(&node.key).map(|old| (*old, node.position)))
        .collect::<Vec<_>>();
    assert!(!common.is_empty());
    assert!(common.iter().all(|(old, new)| old == new));
    ui.recenter_pitch_motion();
    assert_eq!(ui.pitch_motion_anchor, Some(a_minor()));
    let recentered = set_graph_snapshot(&ui);
    let new_positions = set_graph_swatch_from_snapshot(&recentered, &ui, true);
    assert!(new_positions.graph.nodes.iter().any(|node| {
        positions
            .get(&node.key)
            .is_some_and(|old| *old != node.position)
    }));
    assert_eq!(serde_json::to_string(&ui.set.cards).unwrap(), cards);
    assert!(ui.audio_requests.is_empty());
}

#[test]
fn pitch_motion_occurrences_keep_distinct_positions_across_reorder() {
    let mut ui = UiState::new();
    assert!(ui.focus_context_catalog(c_major()));
    ui.stage_current(None);
    ui.set.duplicate(0);
    ui.set_graph_reading(StageGraphReading::PitchMotion);
    let before = set_graph_snapshot(&ui);
    let positions = set_graph_swatch_from_snapshot(&before, &ui, true)
        .graph
        .nodes
        .into_iter()
        .map(|node| (node.key, node.position))
        .collect::<std::collections::BTreeMap<_, _>>();
    let first = positions
        .get(&Some(format!("stage-card-{}", ui.set.cards[0].id.0)))
        .unwrap();
    let second = positions
        .get(&Some(format!("stage-card-{}", ui.set.cards[1].id.0)))
        .unwrap();
    assert_ne!(first, second);
    assert!(
        (first.0 - second.0).hypot(first.1 - second.1) > 0.04,
        "repeated zero-cost Cards need separated pointer targets at the default graph size"
    );
    ui.set.cards.swap(0, 1);
    let after = set_graph_snapshot(&ui);
    for node in set_graph_swatch_from_snapshot(&after, &ui, true)
        .graph
        .nodes
    {
        if let Some(old) = positions.get(&node.key) {
            assert_eq!(*old, node.position);
        }
    }
}

#[test]
fn empty_set_pitch_motion_browses_from_captured_live_material() {
    let mut ui = UiState::new();
    ui.stage.set_lens(woodshed_core::Lens::Chords);
    ui.set_graph_reading(StageGraphReading::PitchMotion);
    assert!(ui.pitch_motion_anchor.is_some());
    assert!(!set_graph_snapshot(&ui).nodes.is_empty());
    assert!(ui.set.cards.is_empty());
}
