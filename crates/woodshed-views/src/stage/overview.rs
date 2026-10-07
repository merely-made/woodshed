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

fn shared_tones_detail(ui: &UiState, from: &OverviewNodeId, to: &OverviewNodeId) -> String {
    let (
        OverviewNodeId::Artifact(SessionArtifactId::ContextItem(a)),
        OverviewNodeId::Artifact(SessionArtifactId::ContextItem(b)),
    ) = (from, to)
    else {
        return String::new();
    };
    let Some(a) = ui
        .musical_context
        .get(*a)
        .and_then(|item| item.subject.pitch_classes())
    else {
        return String::new();
    };
    let Some(b) = ui
        .musical_context
        .get(*b)
        .and_then(|item| item.subject.pitch_classes())
    else {
        return String::new();
    };
    let tones: Vec<_> = a
        .intersection(&b)
        .map(|pc| {
            let pitch = woodshedding::pitch::Pitch::from_midi(
                60 + i32::from(pc.value()),
                woodshedding::pitch::Spelling::Sharps,
            );
            format!("{}{}", pitch.name, pitch.accidental)
        })
        .collect();
    format!(" · common tones: {}", tones.join(", "))
}

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
    let mut snapshot = configured_session_overview(&ConfiguredSessionOverviewInput {
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
    });
    if ui.relationship_reading_json.is_some() {
        use woodshed_core::session_overview::{
            OverviewNode, OverviewRelation, OverviewRelationKind,
        };
        let id = OverviewNodeId::Artifact(SessionArtifactId::RelationshipReading);
        let reading = ui.relationship_reading();
        let (label,detail) = match &reading {
            Ok(Some(reading)) => (reading.snapshot.recipe.definition.label.clone(), format!("Captured relationship reading: {} occurrences, {} explained relationships. Evidence is retained until explicit rebind.", reading.dataset.dataset.occurrences.len(),reading.dataset.relationships.len())),
            _ => ("Unavailable relationship reading".into(), "The retained reading cannot be validated. Its payload is preserved; existing Sets remain available.".into()),
        };
        snapshot.nodes.push(OverviewNode {
            id: id.clone(),
            label,
            detail,
        });
        if let Ok(Some(reading)) = reading {
            if let Some(source) = reading.source {
                let owner =
                    OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(source.owner));
                if snapshot.nodes.iter().any(|node| node.id == owner) {
                    snapshot.relations.push(OverviewRelation {
                        from: id,
                        to: owner,
                        kind: OverviewRelationKind::CapturedReadingOf,
                    });
                }
            }
        }
    }
    woodshed_core::session_overview::append_musical_context(&mut snapshot, &ui.musical_context);
    snapshot
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

fn overview_arrangement_swatch(ui: &UiState) -> GraphCanvasSwatch<OverviewNodeId, &'static str> {
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
            kind: if ui.overview_background.contains(&node.id) {
                "background"
            } else {
                match node.id {
                    OverviewNodeId::Artifact(_) => "artifact",
                    OverviewNodeId::View(_) => "view",
                    OverviewNodeId::Process(_) => "process",
                    OverviewNodeId::Catalog(_) => "catalog",
                }
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

pub fn overview_swatch(ui: &UiState) -> GraphCanvasSwatch<OverviewNodeId, &'static str> {
    let mut swatch = overview_arrangement_swatch(ui);
    let positions = ui.overview_dynamics.positions();
    for node in &mut swatch.graph.nodes {
        if let Some(position) = positions.get(&node.id) {
            node.position = *position;
        }
    }
    swatch
}

impl UiState {
    pub fn reconcile_overview_dynamics(&mut self) {
        let swatch = overview_arrangement_swatch(self);
        let items: Vec<_> = swatch
            .graph
            .nodes
            .into_iter()
            .map(|node| (node.id, node.position, node.kind.to_string()))
            .collect();
        self.overview_dynamics.reconcile(
            &items,
            self.overview_motion,
            self.overview_reduced_motion,
            pictograph::canvas::PhysicsLaw::Springs.id(),
            &self
                .overview_roles
                .iter()
                .map(|(id, role)| (id.wire_key(), role.clone()))
                .collect(),
        );
    }

    pub fn tick_overview_dynamics(&mut self) -> bool {
        // Keep background views and running rehearsal clocks independent.
        if self.workspace.active_panel() != Some(WorkspacePanel::Overview) {
            return false;
        }
        self.reconcile_overview_dynamics();
        self.overview_dynamics.tick()
    }

    pub fn tick_overview_atmosphere(&mut self) -> bool {
        if self.workspace.active_panel() != Some(WorkspacePanel::Overview) {
            return false;
        }
        let changed = self.overview_ambient.reconcile(self.overview_atmosphere);
        self.overview_ambient
            .tick(self.overview_motion && !self.overview_reduced_motion)
            | changed
    }

    pub fn set_overview_atmosphere(&mut self, kind: super::overview_atmosphere::AtmosphereKind) {
        self.overview_atmosphere.kind = kind;
        self.overview_ambient.reconcile(self.overview_atmosphere);
    }

    pub fn new_overview_atmosphere_pattern(&mut self) {
        self.overview_atmosphere.seed = self.overview_atmosphere.seed.wrapping_add(2);
        self.overview_ambient.reconcile(self.overview_atmosphere);
    }

    pub fn move_overview_selection(&mut self, delta: (f32, f32)) {
        let Some(id) = self.overview_focus.clone() else {
            return;
        };
        self.reconcile_overview_dynamics();
        if self.overview_dynamics.begin_key_move(&id) {
            let zoom = self.overview_viewport.zoom.max(0.25);
            self.overview_dynamics
                .key_move_by((delta.0 / zoom, delta.1 / zoom));
            self.overview_dynamics.end_key_move(true);
            if let Some(position) = self.overview_dynamics.positions().get(&id) {
                self.overview_positions.insert(id, *position);
            }
        }
    }
    /// Fit the current arrangement without changing its retained positions.
    pub fn fit_overview(&mut self) {
        let graph = overview_swatch(self);
        if graph.graph.nodes.is_empty() {
            self.overview_viewport = cambium::GraphViewport::default();
            return;
        }
        let mut min = (f32::INFINITY, f32::INFINITY);
        let mut max = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for node in graph.graph.nodes {
            min.0 = min.0.min(node.position.0);
            min.1 = min.1.min(node.position.1);
            max.0 = max.0.max(node.position.0);
            max.1 = max.1.max(node.position.1);
        }
        let zoom = (0.82 / (max.0 - min.0).max(max.1 - min.1).max(0.1)).clamp(0.25, 8.0);
        self.overview_viewport.zoom = zoom;
        self.overview_viewport.pan = (
            (0.5 - (min.0 + max.0) * 0.5) * zoom,
            (0.5 - (min.1 + max.1) * 0.5) * zoom,
        );
    }

    pub fn restore_overview_arrangement(&mut self) {
        self.overview_positions.clear();
        self.overview_dynamics = Default::default();
        self.overview_viewport = cambium::GraphViewport::default();
    }

    pub fn set_overview_background(&mut self, id: OverviewNodeId, background: bool) {
        if !overview_snapshot(self)
            .nodes
            .iter()
            .any(|node| node.id == id)
        {
            return;
        }
        if background {
            self.overview_background.insert(id);
        } else {
            self.overview_background.remove(&id);
        }
    }

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
            OverviewNodeId::Artifact(SessionArtifactId::ContextItem(id)) => {
                self.open_musical_context(id);
            },
            OverviewNodeId::Artifact(SessionArtifactId::RelationshipReading) => {
                self.open_relationship_recipe()
            },
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
                    ui.reconcile_overview_dynamics();
                    match drag.phase {
                        cambium::PointerPhase::Down => {
                            ui.overview_dynamics.drag_start(&drag.id);
                        },
                        cambium::PointerPhase::Move => {
                            ui.overview_dynamics.drag_move(drag.position);
                        },
                        cambium::PointerPhase::Up => {
                            ui.overview_dynamics.drag_move(drag.position);
                            ui.overview_dynamics.drag_end();
                            if let Some(position) = ui.overview_dynamics.positions().get(&drag.id) {
                                ui.overview_positions.insert(drag.id, *position);
                            }
                        },
                    }
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
    let mut controls: Vec<UiChild> = [
        ("Fit scene", "overview-fit", 0),
        ("Restore arrangement", "overview-restore", 1),
        ("Zoom in", "overview-zoom-in", 2),
        ("Zoom out", "overview-zoom-out", 3),
        ("Pan left", "overview-pan-left", 4),
        ("Pan right", "overview-pan-right", 5),
        ("Pan up", "overview-pan-up", 6),
        ("Pan down", "overview-pan-down", 7),
        ("Move selected left", "overview-move-left", 8),
        ("Move selected right", "overview-move-right", 9),
        ("Move selected up", "overview-move-up", 10),
        ("Move selected down", "overview-move-down", 11),
    ]
    .into_iter()
    .map(|(label, class, command)| {
        Box::new(clickable(
            el("button", text(label)).attr("class", format!("t-btn {class}")),
            move |ui: &mut UiState, _| match command {
                0 => ui.fit_overview(),
                1 => ui.restore_overview_arrangement(),
                2 => {
                    ui.overview_viewport.zoom = (ui.overview_viewport.zoom * 1.25).clamp(0.25, 8.0)
                },
                3 => {
                    ui.overview_viewport.zoom = (ui.overview_viewport.zoom / 1.25).clamp(0.25, 8.0)
                },
                4 => ui.overview_viewport.pan.0 += 0.1,
                5 => ui.overview_viewport.pan.0 -= 0.1,
                6 => ui.overview_viewport.pan.1 += 0.1,
                7 => ui.overview_viewport.pan.1 -= 0.1,
                8 => ui.move_overview_selection((-0.03, 0.0)),
                9 => ui.move_overview_selection((0.03, 0.0)),
                10 => ui.move_overview_selection((0.0, -0.03)),
                _ => ui.move_overview_selection((0.0, 0.03)),
            },
        )) as UiChild
    })
    .collect();
    let movement_controls = controls.split_off(4);
    let atmosphere_controls: Vec<UiChild> = [
        (
            "Off",
            "overview-atmosphere-none",
            super::overview_atmosphere::AtmosphereKind::None,
        ),
        (
            "Orbits",
            "overview-atmosphere-orbits",
            super::overview_atmosphere::AtmosphereKind::Orbits,
        ),
        (
            "Cells",
            "overview-atmosphere-cells",
            super::overview_atmosphere::AtmosphereKind::Cells,
        ),
    ]
    .into_iter()
    .map(|(label, class, kind)| {
        Box::new(clickable(
            el("button", text(label))
                .attr(
                    "class",
                    format!(
                        "t-btn {class}{}",
                        if ui.overview_atmosphere.kind == kind {
                            " overview-atmosphere-selected"
                        } else {
                            ""
                        }
                    ),
                )
                .attr(
                    "aria-pressed",
                    if ui.overview_atmosphere.kind == kind {
                        "true"
                    } else {
                        "false"
                    },
                ),
            move |ui: &mut UiState, _| ui.set_overview_atmosphere(kind),
        )) as UiChild
    })
    .collect();
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
        let role_id = id.clone();
        let background = ui.overview_background.contains(&id);
        let role = ui
            .overview_roles
            .get(&id)
            .map(String::as_str)
            .unwrap_or("seeded");
        let role_controls: Vec<UiChild> = [
            ("Free placement", "seeded"),
            ("Return to anchor", "anchored"),
            ("Pin in place", "pinned"),
        ]
        .into_iter()
        .map(|(label, role)| {
            let id = id.clone();
            Box::new(clickable(
                el("button", text(label)).attr("class", format!("t-btn overview-role-{role}")),
                move |ui: &mut UiState, _| {
                    if overview_snapshot(ui).nodes.iter().any(|node| node.id == id) {
                        ui.overview_roles.insert(id.clone(), role.into());
                        ui.reconcile_overview_dynamics();
                    }
                },
            )) as UiChild
        })
        .collect();
        let label = match id {
            OverviewNodeId::Artifact(SessionArtifactId::ContextItem(_)) => "Open in Stage",
            OverviewNodeId::Artifact(SessionArtifactId::SavedSet(_)) => "Open copy",
            OverviewNodeId::Catalog(_) => "Explore catalog",
            OverviewNodeId::Artifact(SessionArtifactId::History) => {
                "Explore recorded relationships"
            },
            _ => "Open view",
        };
        let context_actions: Vec<UiChild> =
            if let OverviewNodeId::Artifact(SessionArtifactId::ContextItem(context_id)) = id {
                vec![
                    Box::new(clickable(
                        el("button", text("Hear")).attr("class", "t-btn overview-context-hear"),
                        move |ui: &mut UiState, _| {
                            ui.hear_musical_context(context_id);
                        },
                    )) as UiChild,
                    Box::new(clickable(
                        el(
                            "button",
                            text(format!("Add to {}", ui.working_sets.active_name)),
                        )
                        .attr("class", "t-btn overview-context-add"),
                        move |ui: &mut UiState, _| {
                            ui.add_musical_context_to_set(context_id);
                        },
                    )) as UiChild,
                    Box::new(clickable(
                        el("button", text("Remove from nearby"))
                            .attr("class", "t-btn overview-context-remove"),
                        move |ui: &mut UiState, _| {
                            ui.remove_musical_context(context_id);
                        },
                    )) as UiChild,
                ]
            } else {
                Vec::new()
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
                        text(format!(
                            "{from} → {} → {to}{}",
                            relation.kind.label(),
                            shared_tones_detail(ui, &relation.from, &relation.to)
                        )),
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
                    el("div", text(if background { "Background context" } else { "Foreground material" })).attr("class", "overview-presentation-role"),
                    clickable(
                        el("button", text(if background { "Bring to foreground" } else { "Move to background" })).attr("class", "t-btn overview-role-toggle"),
                        move |ui: &mut UiState, _| ui.set_overview_background(role_id.clone(), !background),
                    ),
                    el("div", text(match role {
                        "pinned" => "Pinned: movement is unavailable until you choose another placement rule.",
                        "anchored" => "Anchored: moving returns to the arrangement after release.",
                        _ => "Free placement: moving retains the dropped position.",
                    })).attr("class", "overview-placement-rule"),
                    el("div", role_controls).attr("class", "overview-instance-actions"),
                    el("div", text("Presentation emphasis changes the scene; Sets and running sessions keep their own state.")).attr("class", "overview-detail"),
                    el("div", context_actions).attr("class", "overview-instance-actions"),
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
                clickable(
                    el("div", text("Relationship recipe"))
                        .attr("class", "t-btn overview-relationship"),
                    |ui: &mut UiState, _| ui.open_relationship_recipe(),
                ),
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
                el("div", controls).attr(
                    "class",
                    "overview-instance-actions overview-camera-controls",
                ),
                clickable(
                    el(
                        "button",
                        text(if ui.overview_movement_controls {
                            "Hide movement controls"
                        } else {
                            "Show movement controls"
                        }),
                    )
                    .attr("class", "t-btn overview-movement-toggle")
                    .attr(
                        "aria-expanded",
                        if ui.overview_movement_controls {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                    |ui: &mut UiState, _| {
                        ui.overview_movement_controls = !ui.overview_movement_controls;
                    },
                ),
                ui.overview_movement_controls.then(|| {
                    el("div", movement_controls).attr(
                        "class",
                        "overview-instance-actions overview-movement-controls",
                    )
                }),
                el(
                    "div",
                    (
                        el("span", text("Atmosphere")).attr("class", "overview-detail"),
                        el("div", atmosphere_controls).attr("class", "overview-instance-actions"),
                        (ui.overview_atmosphere.kind
                            != super::overview_atmosphere::AtmosphereKind::None)
                            .then(|| {
                                clickable(
                                    el("button", text("New pattern"))
                                        .attr("class", "t-btn overview-atmosphere-seed"),
                                    |ui: &mut UiState, _| ui.new_overview_atmosphere_pattern(),
                                )
                            }),
                    ),
                )
                .attr(
                    "class",
                    "overview-instance-actions overview-atmosphere-controls",
                ),
                el(
                    "div",
                    (
                        clickable(
                            el(
                                "button",
                                text(if ui.overview_motion {
                                    "Pause scene motion"
                                } else {
                                    "Enable scene motion"
                                }),
                            )
                            .attr("class", "t-btn overview-motion-toggle"),
                            |ui: &mut UiState, _| {
                                ui.overview_motion = !ui.overview_motion;
                                ui.reconcile_overview_dynamics();
                            },
                        ),
                        clickable(
                            el(
                                "button",
                                text(if ui.overview_reduced_motion {
                                    "Allow scene motion"
                                } else {
                                    "Reduce scene motion"
                                }),
                            )
                            .attr("class", "t-btn overview-reduced-motion-toggle"),
                            |ui: &mut UiState, _| {
                                ui.overview_reduced_motion = !ui.overview_reduced_motion;
                                ui.reconcile_overview_dynamics();
                            },
                        ),
                        el(
                            "div",
                            text(if ui.overview_reduced_motion {
                                "Reduced motion: arrangement and atmosphere stay still."
                            } else if ui.overview_motion {
                                "Scene motion animates arrangement and atmosphere."
                            } else {
                                "Scene motion is paused."
                            }),
                        )
                        .attr("class", "overview-detail"),
                    ),
                )
                .attr("class", "overview-instance-actions"),
                el("div", graph).attr("class", "overview-graph"),
                el("div", (
                    el("div", text(format!("Keep nearby · {} / 12", ui.musical_context.items().len()))).attr("class", "overview-node-title"),
                    el("div", text("Keep a keyed chord or scale as session context. Pitch links describe sounding tones; adding a Card is a separate action.")).attr("class", "overview-detail"),
                    el("div", (
                        ui.current_card().and_then(woodshed_core::harmony::KeyedCatalogRef::from_card)
                            .filter(|subject| subject.formula_id.starts_with("chord:") || subject.formula_id.starts_with("scale:"))
                            .and_then(|subject| subject.label()).map(|label| clickable(
                                el("button", text(format!("Keep selected Card: {label}"))).attr("class", "t-btn overview-context-keep-card"),
                                |ui: &mut UiState, _| { ui.keep_current_card_nearby(); },
                            )),
                        ui.context_focus.as_ref().filter(|subject| subject.formula_id.starts_with("chord:") || subject.formula_id.starts_with("scale:"))
                            .and_then(|subject| subject.label()).map(|label| clickable(
                                el("button", text(format!("Keep catalog focus: {label}"))).attr("class", "t-btn overview-context-keep-focus"),
                                |ui: &mut UiState, _| { ui.keep_context_focus_nearby(); },
                            )),
                    )).attr("class", "overview-instance-actions"),
                )).attr("class", "overview-context-shelf"),
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
    fn atmosphere_ticks_only_visible_unreduced_scene_and_preserves_owner_facts() {
        let mut ui = with_card();
        let owner = serde_json::to_value(&ui.set).unwrap();
        ui.set_overview_atmosphere(super::super::overview_atmosphere::AtmosphereKind::Orbits);
        let initial = ui.overview_ambient.revision();
        ui.overview_motion = true;
        ui.overview_reduced_motion = false;
        ui.activate_workspace_panel(WorkspacePanel::Practice);
        assert!(!ui.tick_overview_atmosphere());
        assert_eq!(ui.overview_ambient.revision(), initial);
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        ui.overview_reduced_motion = true;
        assert!(!ui.tick_overview_atmosphere());
        assert_eq!(ui.overview_ambient.revision(), initial);
        ui.overview_reduced_motion = false;
        assert!(ui.tick_overview_atmosphere());
        assert!(ui.overview_ambient.revision() > initial);
        ui.overview_motion = false;
        assert!(!ui.tick_overview_atmosphere());
        ui.new_overview_atmosphere_pattern();
        assert_eq!(ui.overview_atmosphere.seed, 3);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), owner);
        assert!(!ui.rehearsal_running && !ui.song_playing && !ui.tuner.enabled);
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

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn camera_and_emphasis_leave_session_authority_intact() {
        let mut ui = UiState::new();
        ui.stage_current(None);
        ui.rehearsal_running = true;
        let before = serde_json::to_value(&ui.set).unwrap();
        let id = overview_snapshot(&ui).nodes[0].id.clone();
        ui.overview_positions.insert(id.clone(), (0.3, 0.4));
        ui.set_overview_background(id.clone(), true);
        ui.fit_overview();
        assert!(ui.overview_viewport.zoom.is_finite());
        assert!(ui.overview_positions.contains_key(&id));
        assert_eq!(
            overview_swatch(&ui)
                .graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .unwrap()
                .kind,
            "background"
        );
        ui.restore_overview_arrangement();
        assert!(ui.overview_positions.is_empty());
        assert!(ui.overview_background.contains(&id));
        assert!(ui.rehearsal_running);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
    }

    #[test]
    fn shared_pin_refuses_host_move_and_restore_keeps_rule() {
        let mut ui = UiState::new();
        let id = overview_snapshot(&ui).nodes[0].id.clone();
        ui.overview_focus = Some(id.clone());
        ui.reconcile_overview_dynamics();
        let before = ui.overview_dynamics.positions()[&id];
        ui.move_overview_selection((0.03, 0.0));
        let moved = ui.overview_positions[&id];
        assert!(
            (moved.0 - before.0 - 0.03).abs() < 1e-5,
            "before {before:?}, moved {moved:?}, zoom {}",
            ui.overview_viewport.zoom
        );
        ui.overview_roles.insert(id.clone(), "pinned".into());
        ui.move_overview_selection((0.03, 0.0));
        assert_eq!(ui.overview_positions[&id], moved);
        ui.restore_overview_arrangement();
        assert_eq!(ui.overview_roles[&id], "pinned");
        assert!(ui.overview_positions.is_empty());
    }

    #[test]
    fn absent_source_cannot_acquire_presentation_role() {
        let mut ui = UiState::new();
        ui.set_overview_background(OverviewNodeId::View("missing-owner".into()), true);
        assert!(ui.overview_background.is_empty());
    }
}
