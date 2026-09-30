//! Session Mere: inspect real assets, workspace views and host activity before acting.
use super::{UiChild, UiState, WorkspacePanel};
use cambium::{
    GraphCanvasEvent, GraphCanvasNode, GraphCanvasRelation, GraphCanvasSubgraph, GraphCanvasSwatch,
    clickable, el, graph_canvas, map_state, text, text_field,
};
use std::collections::BTreeMap;
use woodshed_core::retained_sets::SavedSetId;
use woodshed_core::session_overview::{
    ConfiguredSessionOverviewInput, OverviewNodeId, OverviewProcess, OverviewSnapshot,
    SessionActivity, SessionArtifactId, SessionOverviewInput, SessionView, SessionViewDomain,
    configured_session_overview, overview_scene,
};
use woodshed_core::storage::AppSection;

pub const OVERVIEW_GRAPH_LEAF_KEY: u64 = 0x5753_4d45;

pub fn overview_snapshot(ui: &UiState) -> OverviewSnapshot {
    let views: Vec<_> = WorkspacePanel::ALL
        .into_iter()
        .filter(|panel| ui.workspace.tree().find(panel.tile_id()).is_some())
        .map(|panel| SessionView {
            id: panel.tile_id().0.to_string(),
            label: panel.label().into(),
            domain: match panel {
                WorkspacePanel::Overview => SessionViewDomain::Overview,
                WorkspacePanel::Practice => SessionViewDomain::Stage,
                WorkspacePanel::Related => SessionViewDomain::Catalog,
                WorkspacePanel::Set => SessionViewDomain::Rehearsal,
                WorkspacePanel::Settings => SessionViewDomain::Tools,
            },
        })
        .collect();
    let active_exploration = ui.capture_exploration();
    configured_session_overview(&ConfiguredSessionOverviewInput {
        working_sets: &ui.working_sets,
        explorations: &ui.catalog_explorations,
        active_exploration: &active_exploration,
        runner_owner: ui.rehearsal_owner,
        session: SessionOverviewInput {
            working_set: &ui.set,
            retained_sets: &ui.retained_sets,
            song: &ui.song,
            history: &ui.practice_history,
            views: &views,
            activity: SessionActivity {
                rehearsal: ui.rehearsal_running,
                looper: ui.song_playing,
                tuner: ui.tuner.enabled,
            },
        },
    })
}

/// Canvas labels stay readable in the small projection; identity and inspector
/// copy retain the full name. Count characters rather than slicing UTF-8 bytes.
fn canvas_label(label: &str, narrow: bool) -> String {
    let compact = if narrow {
        label
            .strip_prefix("Working Set ")
            .map(|name| format!("Set {name}"))
            .or_else(|| {
                label
                    .strip_prefix("Catalog exploration ")
                    .map(|name| format!("Explore {name}"))
            })
            .unwrap_or_else(|| label.to_string())
    } else {
        label.to_string()
    };
    if narrow && compact.chars().count() > 10 {
        let mut short: String = compact.chars().take(9).collect();
        short.push('…');
        short
    } else {
        compact
    }
}

/// Narrow Mere uses a two-column roster graph instead of compressing the
/// desktop's artifact/view/activity bands into crowded horizontal lanes.
fn narrow_positions(
    snapshot: &OverviewSnapshot,
    width: u32,
) -> (BTreeMap<OverviewNodeId, (f32, f32)>, u32) {
    let group = |id: &OverviewNodeId| match id {
        OverviewNodeId::Artifact(_) => 0,
        OverviewNodeId::View(_) => 1,
        OverviewNodeId::Process(_) => 2,
        OverviewNodeId::Catalog(_) => 3,
    };
    let mut ordered: Vec<_> = snapshot.nodes.iter().collect();
    ordered.sort_by_key(|node| group(&node.id));
    let rows = ordered.len().div_ceil(2).max(1);
    let height = (rows as u32 * 52 + 36).max(320);
    let inset = 7.0;
    let left = (20.0 - inset) / (width as f32 - inset * 2.0);
    let mut positions = BTreeMap::new();
    for (index, node) in ordered.into_iter().enumerate() {
        let y = (index / 2) as f32 * 52.0 + 44.0;
        positions.insert(
            node.id.clone(),
            (
                if index % 2 == 0 { left } else { 1.0 - left },
                (y - inset) / (height as f32 - inset * 2.0),
            ),
        );
    }
    (positions, height)
}

pub fn overview_swatch(ui: &UiState) -> GraphCanvasSwatch<OverviewNodeId, &'static str> {
    let snapshot = overview_snapshot(ui);
    let scene = overview_scene(&snapshot);
    let bounds = scene.tables.bounds;
    let narrow = ui.viewport == super::ViewportClass::Narrow;
    let width = if ui.viewport_width > 0.0 {
        (ui.viewport_width - 48.0).clamp(280.0, 1000.0) as u32
    } else if narrow {
        340
    } else {
        720
    };
    let (narrow_layout, height) = if narrow {
        narrow_positions(&snapshot, width)
    } else {
        (BTreeMap::new(), 320)
    };
    let nodes = snapshot
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| GraphCanvasNode {
            id: node.id.clone(),
            kind: match node.id {
                OverviewNodeId::Artifact(_) => "artifact",
                OverviewNodeId::View(_) => "view",
                OverviewNodeId::Process(_) => "process",
                OverviewNodeId::Catalog(_) => "catalog",
            },
            position: ui
                .overview_positions
                .get(&node.id)
                .copied()
                .or_else(|| narrow_layout.get(&node.id).copied())
                .unwrap_or((
                    (scene.tables.items[index]
                        .as_ref()
                        .unwrap()
                        .transform
                        .translate
                        .x
                        - bounds.origin.x)
                        / bounds.size.w,
                    (scene.tables.items[index]
                        .as_ref()
                        .unwrap()
                        .transform
                        .translate
                        .y
                        - bounds.origin.y)
                        / bounds.size.h,
                )),
            label: canvas_label(&node.label, ui.viewport == super::ViewportClass::Narrow),
            key: Some(node.id.wire_key()),
        })
        .collect();
    let relations = snapshot
        .relations
        .iter()
        .enumerate()
        .map(|(index, relation)| GraphCanvasRelation {
            id: format!("overview:{index}"),
            from: relation.from.clone(),
            to: relation.to.clone(),
            kind: relation.kind.label().into(),
            label: relation.kind.label().into(),
            route: Vec::new(),
            visible: true,
            emphasized: false,
        })
        .collect();
    let mut swatch = GraphCanvasSwatch::new(
        OVERVIEW_GRAPH_LEAF_KEY,
        GraphCanvasSubgraph {
            nodes,
            edges: Vec::new(),
        },
    )
    .with_relations(relations)
    .with_size(width, height)
    .with_label("Woodshed Mere graph")
    .with_expand(false)
    .with_node_labels(true);
    swatch.cull_labels = narrow;
    swatch.selected = ui.overview_focus.clone();
    swatch.viewport = ui.overview_viewport;
    swatch
}

impl UiState {
    pub fn save_working_set(&mut self) -> Option<SavedSetId> {
        if self.set.cards.is_empty() {
            self.overview_notice = Some("Stage material before retaining a Set.".into());
            return None;
        }
        self.sync_card_rename();
        let name = self.retained_set_name.text().trim().to_string();
        let name = if name.is_empty() {
            format!("Retained Set {}", self.retained_sets.entries.len() + 1)
        } else {
            name
        };
        let id =
            self.retained_sets
                .save_snapshot_from(&self.set, name, self.working_sets.active_id);
        self.overview_focus = Some(OverviewNodeId::Artifact(SessionArtifactId::SavedSet(id)));
        self.overview_notice =
            Some("Set retained. Later edits to the working Set leave this snapshot intact.".into());
        Some(id)
    }

    pub fn open_retained_set(&mut self, id: SavedSetId) -> bool {
        let Some(saved) = self.retained_sets.get(id) else {
            self.overview_notice = Some("This retained Set is no longer available.".into());
            return false;
        };
        let name = format!("{} copy", saved.name);
        let mut copy = saved.set.clone();
        if !self.retained_sets.restore_into(id, &mut copy) {
            return false;
        }
        self.create_working_set();
        self.set = copy;
        self.working_sets.active_name = name;
        self.overview_notice = Some(
            "Opened an independent working copy. Your previous Set remains available in Mere."
                .into(),
        );
        true
    }

    pub fn inspect_overview_node(&mut self, id: OverviewNodeId) {
        if overview_snapshot(self)
            .nodes
            .iter()
            .any(|node| node.id == id)
        {
            self.overview_focus = Some(id);
        }
    }

    /// Navigation changes the shown surface only; it never starts or pauses a process.
    pub fn open_overview_node(&mut self, id: OverviewNodeId) {
        if !overview_snapshot(self)
            .nodes
            .iter()
            .any(|node| node.id == id)
        {
            return;
        }
        match id {
            OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(id)) => {
                self.activate_working_set(id);
            },
            OverviewNodeId::Artifact(SessionArtifactId::Exploration(id)) => {
                self.activate_exploration(id);
            },
            OverviewNodeId::Artifact(SessionArtifactId::SavedSet(saved)) => {
                self.open_retained_set(saved);
            },
            OverviewNodeId::Artifact(SessionArtifactId::WorkingSet) => {
                self.activate_workspace_panel(WorkspacePanel::Set)
            },
            OverviewNodeId::Artifact(SessionArtifactId::Song)
            | OverviewNodeId::Process(OverviewProcess::Looper) => {
                self.select_app_section(AppSection::Looper)
            },
            OverviewNodeId::Process(OverviewProcess::Rehearsal) => {
                if let Some(owner) = self.rehearsal_owner {
                    self.activate_working_set(owner);
                } else {
                    self.activate_workspace_panel(WorkspacePanel::Set);
                }
            },
            OverviewNodeId::Process(OverviewProcess::Tuner) => {
                self.select_app_section(AppSection::Tools)
            },
            OverviewNodeId::Catalog(_) => self.activate_workspace_panel(WorkspacePanel::Related),
            OverviewNodeId::Artifact(SessionArtifactId::History) => {
                self.app_settings.stage.related.graph_scope =
                    woodshed_core::settings::RelatedGraphScope::Mere;
                self.activate_workspace_panel(WorkspacePanel::Related);
            },
            OverviewNodeId::View(id) => {
                if let Some(panel) = WorkspacePanel::ALL
                    .into_iter()
                    .find(|panel| panel.tile_id().0.to_string() == id)
                {
                    self.activate_workspace_panel(panel);
                }
            },
        }
    }
}

pub(super) fn screen(ui: &UiState) -> UiChild {
    let snapshot = overview_snapshot(ui);
    let swatch = overview_swatch(ui);
    let graph = graph_canvas(
        &swatch,
        |ui: &mut UiState, event: GraphCanvasEvent<OverviewNodeId>| match event {
            GraphCanvasEvent::Activate(id) => ui.inspect_overview_node(id),
            GraphCanvasEvent::Drag(drag) => {
                if overview_snapshot(ui)
                    .nodes
                    .iter()
                    .any(|node| node.id == drag.id)
                {
                    ui.overview_positions.insert(drag.id, drag.position);
                }
            },
            GraphCanvasEvent::Pan { delta } => {
                ui.overview_viewport.pan.0 += delta.0;
                ui.overview_viewport.pan.1 += delta.1;
            },
            GraphCanvasEvent::Zoom { factor } => {
                ui.overview_viewport.zoom = (ui.overview_viewport.zoom * factor).clamp(0.25, 8.0)
            },
            _ => {},
        },
    );
    let roster: Vec<UiChild> = snapshot
        .nodes
        .iter()
        .map(|node| {
            let id = node.id.clone();
            Box::new(clickable(
                el(
                    "div",
                    (
                        el("div", text(node.label.clone())).attr("class", "overview-node-title"),
                        el(
                            "div",
                            text(match node.id {
                                OverviewNodeId::Artifact(_) => "Retained / working material",
                                OverviewNodeId::View(_) => "Workspace view",
                                OverviewNodeId::Process(_) => "Live activity",
                                OverviewNodeId::Catalog(_) => "Catalog reference",
                            }),
                        )
                        .attr("class", "overview-node-kind"),
                    ),
                )
                .attr(
                    "class",
                    if ui.overview_focus.as_ref() == Some(&node.id) {
                        "overview-node overview-node-selected"
                    } else {
                        "overview-node"
                    },
                ),
                move |ui: &mut UiState, _| ui.inspect_overview_node(id.clone()),
            )) as UiChild
        })
        .collect();
    let inspector: UiChild = if let Some(node) = ui
        .overview_focus
        .as_ref()
        .and_then(|id| snapshot.nodes.iter().find(|node| &node.id == id))
    {
        let id = node.id.clone();
        let label = match id {
            OverviewNodeId::Artifact(SessionArtifactId::SavedSet(_)) => "Open copy",
            OverviewNodeId::Catalog(_) => "Explore catalog",
            OverviewNodeId::Artifact(SessionArtifactId::History) => {
                "Explore recorded relationships"
            },
            _ => "Open view",
        };
        let relations: Vec<UiChild> = snapshot
            .relations
            .iter()
            .filter(|relation| relation.from == node.id || relation.to == node.id)
            .map(|relation| {
                let from = snapshot
                    .nodes
                    .iter()
                    .find(|node| node.id == relation.from)
                    .map(|node| node.label.as_str())
                    .unwrap_or("Unavailable");
                let to = snapshot
                    .nodes
                    .iter()
                    .find(|node| node.id == relation.to)
                    .map(|node| node.label.as_str())
                    .unwrap_or("Unavailable");
                Box::new(
                    el(
                        "div",
                        text(format!("{from} → {} → {to}", relation.kind.label())),
                    )
                    .attr("class", "overview-relation"),
                ) as UiChild
            })
            .collect();
        let history_rows: Vec<UiChild> =
            if matches!(id, OverviewNodeId::Artifact(SessionArtifactId::History)) {
                ui.practice_history
                    .recent(8)
                    .into_iter()
                    .map(|event| {
                        let title = event
                            .provenance
                            .as_ref()
                            .and_then(|provenance| provenance.card_snapshot.get("label"))
                            .and_then(|value| value.as_str())
                            .unwrap_or("Catalog observation");
                        let span = event
                            .practiced_ms
                            .map(|ms| format!(" · {ms} ms practiced"))
                            .unwrap_or_default();
                        Box::new(
                            el(
                                "div",
                                text(format!("{} · {title}{span}", event.kind.label())),
                            )
                            .attr("class", "overview-history-event"),
                        ) as UiChild
                    })
                    .collect()
            } else {
                Vec::new()
            };
        Box::new(
            el(
                "div",
                (
                    el("div", text(node.label.clone())).attr("class", "overview-inspector-title"),
                    el("div", text(if matches!(node.id,OverviewNodeId::View(_)) { "Workspace presentation of the shared session. Opening this view leaves live activity running.".into() } else { node.detail.clone() })).attr("class", "overview-detail"),
                    el("div", relations).attr("class", "overview-relations"),
                    el("div", history_rows).attr("class", "overview-history"),
                    matches!(id, OverviewNodeId::Artifact(SessionArtifactId::SavedSet(_))).then(
                        || {
                            el(
                                "div",
                                text("Opening creates another working Set and keeps your current Set available."),
                            )
                            .attr("class", "overview-detail")
                        },
                    ),
                    matches!(id,OverviewNodeId::Process(OverviewProcess::Rehearsal)).then(||clickable(el("div",text("Pause rehearsal")).attr("class","t-btn overview-pause-rehearsal"),|ui:&mut UiState,_|ui.stop_rehearsal())),
                    clickable(
                        el("div", text(label)).attr("class", "t-btn overview-open"),
                        move |ui: &mut UiState, _| ui.open_overview_node(id.clone()),
                    ),
                ),
            )
            .attr("class", "overview-inspector"),
        )
    } else {
        Box::new(
            el(
                "div",
                text("Select material, a view or live activity to inspect its relationships."),
            )
            .attr("class", "overview-inspector overview-detail"),
        )
    };
    Box::new(
        el(
            "div",
            (
                el("div", text("Woodshed Mere")).attr("class", "overview-title"),
                el(
                    "div",
                    text("Retained material, working views and live activity."),
                )
                .attr("class", "overview-subtitle"),
                el(
                    "div",
                    (
                        clickable(
                            el("div", text("New Set")).attr("class", "t-btn overview-new-set"),
                            |ui: &mut UiState, _| {
                                ui.create_working_set();
                            },
                        ),
                        clickable(
                            el("div", text("Duplicate Set"))
                                .attr("class", "t-btn overview-duplicate-set"),
                            |ui: &mut UiState, _| {
                                ui.duplicate_working_set();
                            },
                        ),
                        clickable(
                            el("div", text("New exploration"))
                                .attr("class", "t-btn overview-new-exploration"),
                            |ui: &mut UiState, _| {
                                ui.create_exploration();
                            },
                        ),
                        clickable(
                            el("div", text("Duplicate exploration"))
                                .attr("class", "t-btn overview-duplicate-exploration"),
                            |ui: &mut UiState, _| {
                                ui.duplicate_exploration();
                            },
                        ),
                    ),
                )
                .attr("class", "overview-instance-actions"),
                el(
                    "div",
                    (
                        map_state(text_field(&ui.retained_set_name), |ui: &mut UiState| {
                            &mut ui.retained_set_name
                        }),
                        clickable(
                            el("div", text("Retain Set")).attr("class", "t-btn overview-save"),
                            |ui: &mut UiState, _| {
                                ui.save_working_set();
                            },
                        ),
                    ),
                )
                .attr("class", "overview-save-row"),
                ui.overview_notice
                    .as_ref()
                    .map(|notice| el("div", text(notice.clone())).attr("class", "overview-notice")),
                el("div", graph).attr("class", "overview-graph"),
                el(
                    "div",
                    (
                        el("div", roster).attr("class", "overview-roster"),
                        inspector,
                    ),
                )
                .attr("class", "overview-body"),
            ),
        )
        .attr("class", "session-overview"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::rehearsal::CardId;

    fn with_card() -> UiState {
        let mut ui = UiState::new();
        ui.stage_current(None);
        assert!(!ui.set.cards.is_empty());
        ui
    }

    #[test]
    fn retaining_and_inspecting_does_not_edit_or_practice_the_working_set() {
        let mut ui = with_card();
        let original = ui.set.clone();
        let history = ui.practice_history.clone();
        let id = ui.save_working_set().unwrap();
        ui.inspect_overview_node(OverviewNodeId::Artifact(SessionArtifactId::SavedSet(id)));
        assert_eq!(
            serde_json::to_value(&ui.set).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&ui.practice_history).unwrap(),
            serde_json::to_value(&history).unwrap()
        );
        ui.set.cards[0].label = "Edited working instruction".into();
        assert_ne!(
            ui.retained_sets.get(id).unwrap().set.cards[0].label,
            ui.set.cards[0].label
        );
        let persisted = ui.to_persisted();
        assert_eq!(persisted.retained_sets.entries.len(), 1);
    }

    #[test]
    fn open_copy_keeps_previous_work_and_invalidates_occurrence_inspectors() {
        let mut ui = with_card();
        let id = ui.save_working_set().unwrap();
        let old = ui.set.cards[0].id;
        ui.set.cards[0].label = "Keep this edit".into();
        ui.arpeggio_source = Some(old);
        ui.scale_source = Some(old);
        ui.pattern_source = Some(old);
        ui.approach_source = Some(old);
        ui.card_rename_for = None;
        let owner = ui.working_sets.active_id;
        ui.toggle_rehearsal();
        assert!(ui.open_retained_set(id));
        assert_ne!(ui.set.cards[0].id, old);
        assert_ne!(ui.set.cards[0].id, CardId::UNASSIGNED);
        assert_eq!(ui.retained_sets.entries.len(), 1);
        assert_eq!(
            ui.working_sets.get(owner, &ui.set).unwrap().cards[0].label,
            "Keep this edit"
        );
        assert!(ui.rehearsal_running);
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert_eq!(ui.arpeggio_source, None);
        assert_eq!(ui.scale_source, None);
        assert_eq!(ui.pattern_source, None);
        assert_eq!(ui.approach_source, None);
        let before = ui.set.clone();
        assert!(!ui.open_retained_set(SavedSetId(u64::MAX)));
        assert_eq!(
            serde_json::to_value(&ui.set).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        assert_eq!(ui.retained_sets.entries.len(), 1);
    }

    #[test]
    fn navigation_and_graph_inspection_do_not_start_or_pause_live_processes() {
        let mut ui = with_card();
        ui.rehearsal_running = true;
        ui.song_playing = true;
        ui.tuner.enabled = true;
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        let snapshot = overview_snapshot(&ui);
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .filter(|node| matches!(node.id, OverviewNodeId::Process(_)))
                .count(),
            3
        );
        ui.inspect_overview_node(OverviewNodeId::Process(OverviewProcess::Rehearsal));
        ui.open_overview_node(OverviewNodeId::Process(OverviewProcess::Rehearsal));
        assert!(ui.rehearsal_running && ui.song_playing && ui.tuner.enabled);
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        ui.open_overview_node(OverviewNodeId::Process(OverviewProcess::Tuner));
        assert_eq!(ui.section, AppSection::Tools);
        assert!(ui.rehearsal_running && ui.song_playing && ui.tuner.enabled);
        let swatch = overview_swatch(&ui);
        assert!(
            swatch
                .graph
                .nodes
                .iter()
                .all(|node| (0.0..=1.0).contains(&node.position.0)
                    && (0.0..=1.0).contains(&node.position.1))
        );
        assert_eq!(swatch.selected, ui.overview_focus);
    }
    #[test]
    fn narrow_canvas_abbreviates_names_without_changing_full_read_model_or_identity() {
        let mut ui = with_card();
        ui.retained_set_name = cambium::TextInput::new("長い名前の保持されたセット音楽");
        let saved = ui.save_working_set().unwrap();
        ui.set_viewport_width(420.0);
        let full = overview_snapshot(&ui);
        let identity = OverviewNodeId::Artifact(SessionArtifactId::SavedSet(saved));
        let node = full.nodes.iter().find(|node| node.id == identity).unwrap();
        assert_eq!(node.label, "長い名前の保持されたセット音楽");
        let narrow = overview_swatch(&ui);
        let canvas = narrow
            .graph
            .nodes
            .iter()
            .find(|node| node.id == identity)
            .unwrap();
        assert_eq!(canvas.label.chars().count(), 10);
        assert!(canvas.label.ends_with('…'));
        assert_eq!(canvas.key.as_deref(), Some(identity.wire_key().as_str()));
        assert_eq!(canvas_label("Short", true), "Short");
        ui.set_viewport_width(1200.0);
        let wide = overview_swatch(&ui);
        assert_eq!(
            wide.graph
                .nodes
                .iter()
                .find(|node| node.id == identity)
                .unwrap()
                .label,
            node.label
        );
    }
    #[test]
    fn narrow_instance_graph_has_separate_rows_and_distinct_compact_names() {
        let mut ui = with_card();
        ui.duplicate_working_set();
        ui.create_exploration();
        ui.set_viewport_width(420.0);
        let full = overview_snapshot(&ui);
        let swatch = overview_swatch(&ui);
        assert!(swatch.height > 320);
        assert_eq!(canvas_label("Working Set 1", true), "Set 1");
        assert_eq!(canvas_label("Working Set 1 copy", true), "Set 1 copy");
        assert_ne!(
            canvas_label("Catalog exploration 1", true),
            canvas_label("Catalog exploration 2", true)
        );
        let projected = swatch.projected_positions();
        for (index, (_, a)) in projected.iter().enumerate() {
            for (_, b) in &projected[index + 1..] {
                assert!((a.0 - b.0).abs() > 200.0 || (a.1 - b.1).abs() >= 50.0);
            }
        }
        assert!(
            full.nodes
                .iter()
                .any(|node| node.label == "Working Set 1 copy")
        );
        ui.set_viewport_width(1100.0);
        assert_eq!(overview_swatch(&ui).height, 320);
    }
}
