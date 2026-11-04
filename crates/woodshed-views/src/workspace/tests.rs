use super::*;
use workbench::{DropTarget, Edge, TilePath};

fn split_workspace(workspace: &mut WoodshedWorkspace) {
    assert_eq!(
        workspace.apply(WorkspaceEvent::Tile(TileEvent::Dragged {
            tile: WorkspacePanel::Settings.tile_id(),
            to: DropTarget::Edge {
                tile: WorkspacePanel::Practice.tile_id(),
                edge: Edge::Right,
            },
        })),
        WorkspaceOutcome::Changed
    );
}

#[test]
fn panels_use_stable_ids_and_open_content_lanes() {
    let workspace = WoodshedWorkspace::new();
    assert_eq!(
        workspace
            .tree()
            .tiles()
            .iter()
            .map(|tile| tile.id)
            .collect::<Vec<_>>(),
        vec![
            WorkspacePanel::Overview.tile_id(),
            WorkspacePanel::Practice.tile_id(),
            WorkspacePanel::Set.tile_id(),
            WorkspacePanel::Related.tile_id(),
            WorkspacePanel::Arrangement.tile_id(),
            WorkspacePanel::Settings.tile_id(),
        ]
    );
    for tile in workspace.tree().tiles() {
        let ContentSource::Open { kind, .. } = &tile.content else {
            panic!("{} must use an open Woodshed lane", tile.title);
        };
        assert!(kind.starts_with("woodshed."));
    }
}

#[test]
fn activation_selects_the_existing_set_surface() {
    let mut workspace = WoodshedWorkspace::new();
    assert_eq!(workspace.active_panel(), Some(WorkspacePanel::Practice));
    assert_eq!(
        workspace.apply(WorkspaceEvent::activate(WorkspacePanel::Set)),
        WorkspaceOutcome::Activated(WorkspacePanel::Set)
    );
    assert_eq!(workspace.active_panel(), Some(WorkspacePanel::Set));
}

#[test]
fn split_activation_retains_the_panel_routed_by_the_host() {
    let mut workspace = WoodshedWorkspace::new();
    split_workspace(&mut workspace);
    assert_eq!(workspace.active_panel(), Some(WorkspacePanel::Practice));

    assert_eq!(
        workspace.apply(WorkspaceEvent::activate(WorkspacePanel::Settings)),
        WorkspaceOutcome::Activated(WorkspacePanel::Settings)
    );
    assert_eq!(workspace.active_panel(), Some(WorkspacePanel::Settings));
}

#[test]
fn outside_drop_requests_a_typed_tear_out_without_losing_the_panel() {
    let mut workspace = WoodshedWorkspace::new();
    let before = workspace.snapshot();
    assert_eq!(
        workspace.apply(WorkspaceEvent::Tile(TileEvent::Dragged {
            tile: WorkspacePanel::Related.tile_id(),
            to: DropTarget::Outside,
        })),
        WorkspaceOutcome::Effect(WorkspaceEffect::TearOut {
            panel: WorkspacePanel::Related,
        })
    );
    assert_eq!(
        workspace.snapshot(),
        before,
        "Woodshed retains content custody"
    );
}

#[test]
fn host_snapshot_restore_is_deterministic() {
    let mut workspace = WoodshedWorkspace::new();
    workspace.apply(WorkspaceEvent::activate(WorkspacePanel::Settings));
    let snapshot = workspace.snapshot();
    let restored = WoodshedWorkspace::restore(snapshot.clone());
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.active_panel(), Some(WorkspacePanel::Settings));
}

#[test]
fn host_snapshot_json_round_trips_the_tree_and_active_panel() {
    let mut workspace = WoodshedWorkspace::new();
    split_workspace(&mut workspace);
    workspace.apply(WorkspaceEvent::activate(WorkspacePanel::Settings));
    let json = workspace.to_snapshot_json().unwrap();
    assert!(
        json.as_bytes()
            .windows(b"woodshed.settings".len())
            .any(|window| { window == b"woodshed.settings" })
    );
    assert!(json.contains("\"axis\":\"row\""));
    assert!(json.contains("\"fraction\":0.5"));

    let restored = WoodshedWorkspace::from_snapshot_json(&json).unwrap();
    assert_eq!(restored.snapshot(), workspace.snapshot());
    assert_eq!(restored.active_panel(), Some(WorkspacePanel::Settings));
    assert_eq!(restored.to_snapshot_json().unwrap(), json);
}

#[test]
fn malformed_divider_never_enters_or_restores_from_the_workspace_snapshot() {
    let mut workspace = WoodshedWorkspace::new();
    split_workspace(&mut workspace);
    let before = workspace.snapshot();
    assert_eq!(
        workspace.apply(WorkspaceEvent::Tile(TileEvent::DividerMoved {
            split: TilePath(vec![]),
            fractions: vec![0.7, 0.4],
        })),
        WorkspaceOutcome::Unchanged
    );
    assert_eq!(workspace.snapshot(), before);

    let mut malformed: serde_json::Value =
        serde_json::from_str(&workspace.to_snapshot_json().unwrap()).unwrap();
    malformed["tree"]["split"]["children"][0]["fraction"] = serde_json::json!(0.7);
    malformed["tree"]["split"]["children"][1]["fraction"] = serde_json::json!(0.4);
    assert!(matches!(
        WoodshedWorkspace::from_snapshot_json(&serde_json::to_string(&malformed).unwrap()),
        Err(WorkspaceSnapshotError::Invalid(_))
    ));
}

#[test]
fn host_snapshot_json_rejects_an_unknown_or_altered_panel() {
    let json = WoodshedWorkspace::new().to_snapshot_json().unwrap();
    let unknown = json.replacen("woodshed.practice", "woodshed.unknown", 1);
    assert!(matches!(
        WoodshedWorkspace::from_snapshot_json(&unknown),
        Err(WorkspaceSnapshotError::Invalid(_))
    ));

    let malformed = json.replacen("\"practice\"", "\"unknown\"", 1);
    assert!(matches!(
        WoodshedWorkspace::from_snapshot_json(&malformed),
        Err(WorkspaceSnapshotError::Decode(_))
    ));
}
#[test]
fn legacy_four_panel_snapshot_restores_exactly_then_adds_mere_on_request() {
    let legacy = WorkspaceSnapshot {
        tree: TileTree::stack(
            vec![
                WorkspacePanel::Practice.tile(),
                WorkspacePanel::Set.tile(),
                WorkspacePanel::Related.tile(),
                WorkspacePanel::Settings.tile(),
            ],
            1,
        ),
        active_panel: Some(WorkspacePanel::Set),
    };
    let original = WoodshedWorkspace::restore(legacy.clone());
    let mut restored =
        WoodshedWorkspace::from_snapshot_json(&original.to_snapshot_json().unwrap()).unwrap();
    assert_eq!(restored.snapshot(), legacy);
    assert!(
        restored
            .tree()
            .find(WorkspacePanel::Overview.tile_id())
            .is_none()
    );
    restored.ensure_panel(WorkspacePanel::Overview);
    assert_eq!(restored.active_panel(), Some(WorkspacePanel::Set));
    assert_eq!(restored.tree().tiles().len(), 5);
    assert_eq!(
        restored.apply(WorkspaceEvent::activate(WorkspacePanel::Overview)),
        WorkspaceOutcome::Activated(WorkspacePanel::Overview)
    );
    let once = restored.snapshot();
    restored.ensure_panel(WorkspacePanel::Overview);
    assert_eq!(once, restored.snapshot());
}
