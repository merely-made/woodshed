//! The scenario lane: woodshed drives itself.
//!
//! Woodshed had no self-drive hook, so every headed receipt was SendKeys plus a
//! desktop grab: synthetic input that loses the foreground race whenever the
//! machine is in use, and captures that can silently photograph the wrong
//! window. This lane replaces both. The generic half (parsing, the verb loop,
//! selector resolution, assertions) is [`taproot`]; what lives here is only
//! what is woodshed's: which surfaces it has, what it can be asked to observe,
//! its named commands, and its acceptance diagnostics. Mesquite owns captures and completion.
//!
//! Since the host extraction it no longer owns *routing*. A delivered point is
//! queued as a [`HostPointer`] and the host runs it through the same hit test,
//! capture, and dispatch a real mouse takes — so a receipt exercises the
//! shipping path rather than an app-local imitation of it.
//!
//! Two env vars turn it on:
//!
//! - `WOODSHED_SCENARIO` — path to a `.scn` file (grammar in `taproot`).
//! - `WOODSHED_CAPTURE_DIR` — where `capture <name>` writes `<name>.png` and,
//!   at the end, `scenario.done` whose first line is `RESULT ok` or
//!   `RESULT fail`.
//!
//! Captures are in-process readbacks of the same rasterized view the frame
//! presented, so they need no compositor, no foreground, and no ffmpeg, and
//! they cannot photograph another window by mistake.
//!
//! `WOODSHED_STATE` overrides the session file. A scenario run must set it:
//! without it the run would read, and then overwrite, the real practice
//! session.

use std::cell::RefCell;
use std::rc::Rc;

use cambium_genet_winit_host::HostPointer;
use taproot::{Automatable, AutomatableExt, ProbeSnapshot, ProbeSurface, Selector};
use woodshed_core::Lens;
use woodshed_views::stage::{
    UiState, set_graph_relation_choices, set_graph_snapshot, set_graph_swatch_from_snapshot,
};

use crate::shared::Shared;
use crate::sync::Ctx;

/// Product commands and observations; Mesquite owns the frame pump and captures.
pub struct Product {
    shared: Rc<RefCell<Shared>>,
    sheet: String,
}

pub fn from_env(shared: Rc<RefCell<Shared>>) -> Option<mesquite::Lane<Product>> {
    let config = mesquite::LaneConfig::from_env("WOODSHED")?;
    let sheet = shared.borrow().accessible_sheet();
    Some(
        mesquite::Lane::from_config(
            config,
            Product { shared, sheet },
            cambium_genet_winit_host::read_file,
        )
        .expect("load Woodshed scenario"),
    )
}

impl mesquite::Product for Product {
    type State = UiState;
    type Logic = crate::sync::Logic;
    type View = woodshed_views::stage::UiChild;
    const KIND: &'static str = "woodshed";
    const SURFACE: &'static str = "woodshed";
    const LOG_PREFIX: &'static str = "woodshed-genet";
    fn sheet(&self) -> &str {
        &self.sheet
    }
    fn snapshot(&self, ctx: &Ctx<'_>, _: usize, _: f32) -> ProbeSnapshot {
        Snapshot {
            ctx,
            shared: &self.shared.borrow(),
        }
        .snapshot()
    }
    fn drain_events(&mut self, _: &mut Ctx<'_>) -> Vec<String> {
        std::mem::take(&mut self.shared.borrow_mut().events)
    }
    fn busy(&self, _: &Ctx<'_>, capture_pending: bool) -> Option<bool> {
        Some(capture_pending)
    }
    fn act(&mut self, ctx: &mut Ctx<'_>, label: &str) -> bool {
        Probe {
            ctx,
            shared: &mut self.shared.borrow_mut(),
            clicks: &mut mesquite::Clicks::default(),
        }
        .act(label)
    }
    fn app_step_with_clicks(
        &mut self,
        ctx: &mut Ctx<'_>,
        _: mesquite::Checkpoints<'_>,
        clicks: &mut mesquite::Clicks,
        line: &str,
    ) -> Result<(), String> {
        Probe {
            ctx,
            shared: &mut self.shared.borrow_mut(),
            clicks,
        }
        .app_step(line)
    }
    fn receipt_lines(&self) -> Vec<String> {
        let shared = self.shared.borrow();
        if shared.drag_frame_metrics.samples > 0 {
            vec![shared.drag_frame_metrics.summary()]
        } else {
            vec![]
        }
    }
}

pub fn drive(lane: &mut mesquite::Lane<Product>, ctx: &mut Ctx<'_>) {
    {
        let product = lane.product_mut();
        let mut shared = product.shared.borrow_mut();
        note_events(&mut shared, ctx.runner.state());
        product.sheet = shared.accessible_sheet();
    }
    lane.after_frame(ctx);
}

/// The state a scenario asserts against, sampled each time it may have changed.
/// Kept beside the events it produces so a transition is reported once, with
/// what it was and what it became.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observed {
    pub cards: usize,
    pub cursor_id: u64,
    pub cursor_number: usize,
    pub relations: String,
    pub edges: usize,
}

impl Observed {
    fn read(ui: &UiState) -> Self {
        let graph = ui
            .set
            .graph()
            .with_relations(&ui.app_settings.stage.visible_relations());
        Self {
            cards: ui.set.cards.len(),
            cursor_id: ui.set.cursor_id().map(|id| id.0).unwrap_or_default(),
            cursor_number: if ui.set.cards.is_empty() {
                0
            } else {
                ui.set.cursor.min(ui.set.cards.len() - 1) + 1
            },
            relations: relation_summary(ui),
            edges: graph.edges.len(),
        }
    }
}

fn relation_summary(ui: &UiState) -> String {
    let visible = ui.app_settings.stage.visible_relations();
    if visible.is_empty() {
        return "none".to_string();
    }
    visible
        .iter()
        .map(|kind| kind.label())
        .collect::<Vec<_>>()
        .join(",")
}

/// Sample the observed state and emit an event for anything that moved. Events
/// are the app's own transitions, not the driver's intentions, so a scenario
/// asserting one is asserting that the app really did it.
pub fn note_events(shared: &mut Shared, ui: &UiState) {
    let now = Observed::read(ui);
    let before = std::mem::replace(&mut shared.observed, now.clone());
    if before == now {
        return;
    }
    if before.cards != now.cards {
        shared.events.push(format!("set-size {}", now.cards));
    }
    if before.cursor_id != now.cursor_id || before.cursor_number != now.cursor_number {
        shared.events.push(format!(
            "set-cursor id={} number={}",
            now.cursor_id, now.cursor_number
        ));
    }
    if before.relations != now.relations {
        shared
            .events
            .push(format!("relations-visible {}", now.relations));
    }
    if before.edges != now.edges {
        shared.events.push(format!("graph-edges {}", now.edges));
    }
}

/// The `Automatable` view of woodshed, borrowed for the duration of one tick.
///
/// The host owns the runner, so the application cannot hold a long-lived `&mut`
/// to it; it borrows the hook's context for exactly as long as the driver needs
/// and queues pointer delivery back through the host.
struct Probe<'a, 'c> {
    ctx: &'a mut Ctx<'c>,
    shared: &'a mut Shared,
    clicks: &'a mut mesquite::Clicks,
}

struct Snapshot<'a, 'c> {
    ctx: &'a Ctx<'c>,
    shared: &'a Shared,
}

impl Snapshot<'_, '_> {
    /// The `data-key` of the Set-graph node the canvas is currently painting
    /// as focused, or `"none"`. The canvas owns that emphasis, so the DOM is
    /// the authority for it.
    fn graph_focus_key(&self) -> String {
        use layout_dom_api::LayoutDom;
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.all_with_class(dom.document(), "graph-canvas-swatch-node")
            .into_iter()
            .find(|node| {
                dom.attribute(
                    *node,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("class"),
                )
                .is_some_and(|class| class.split_whitespace().any(|token| token == "focused"))
            })
            .and_then(|node| {
                dom.attribute(
                    node,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("data-key"),
                )
                .map(str::to_owned)
            })
            .unwrap_or_else(|| "none".to_string())
    }

    fn graph_label_placements(&self) -> (usize, usize) {
        use layout_dom_api::LayoutDom;
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.all_with_class(dom.document(), "graph-canvas-swatch-label")
            .into_iter()
            .fold((0, 0), |(above, side), node| {
                let placement = dom.attribute(
                    node,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("data-label-placement"),
                );
                match placement {
                    Some("above") => (above + 1, side),
                    Some("left" | "right") => (above, side + 1),
                    _ => (above, side),
                }
            })
    }

    fn graph_node_card_count(&self) -> usize {
        use layout_dom_api::LayoutDom;
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.all_with_class(dom.document(), "set-graph-selected-card")
            .len()
    }

    fn pointer_capture_class(&self) -> String {
        use layout_dom_api::LayoutDom;
        let Some(node) = self.ctx.runner.pointer_capture() else {
            return "none".to_string();
        };
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.attribute(
            node,
            &layout_dom_api::Namespace::from(""),
            &layout_dom_api::LocalName::from("class"),
        )
        .unwrap_or("unclassed")
        .to_string()
    }
}
impl Snapshot<'_, '_> {
    fn snapshot(&self) -> ProbeSnapshot {
        let ui = self.ctx.runner.state();
        let observed = Observed::read(ui);
        let overview = woodshed_views::stage::overview_snapshot(ui);
        let stage_snapshot = set_graph_snapshot(ui);
        let stage_swatch =
            set_graph_swatch_from_snapshot(&stage_snapshot, ui, ui.set_tray_expanded);
        let routed_lanes = stage_swatch
            .projected_relations()
            .iter()
            .filter(|(_, route)| route.len() == 4)
            .count();
        let (card_incident_relations, card_anchored_relations) = stage_swatch
            .selected
            .as_ref()
            .and_then(|selected| {
                let region = stage_swatch.projected_node_footprint(selected)?;
                let right = region.left + region.width;
                let bottom = region.top + region.height;
                let mut incident = 0;
                let mut anchored = 0;
                for (relation, route) in stage_swatch.projected_relations() {
                    let endpoint = if &relation.from == selected {
                        route.first().copied()
                    } else if &relation.to == selected {
                        route.last().copied()
                    } else {
                        None
                    };
                    let Some(point) = endpoint else {
                        continue;
                    };
                    incident += 1;
                    let on_vertical =
                        (point.0 - region.left).abs() < 0.01 || (point.0 - right).abs() < 0.01;
                    let on_horizontal =
                        (point.1 - region.top).abs() < 0.01 || (point.1 - bottom).abs() < 0.01;
                    if (on_vertical && point.1 >= region.top && point.1 <= bottom)
                        || (on_horizontal && point.0 >= region.left && point.0 <= right)
                    {
                        anchored += 1;
                    }
                }
                Some((incident, anchored))
            })
            .unwrap_or_default();
        let (labels_above, labels_side) = self.graph_label_placements();
        let relation_choices = set_graph_relation_choices(&stage_snapshot, ui);
        let cursor_label = ui
            .set
            .cursor_id()
            .and_then(|id| ui.set.card(id))
            .map(|card| card.label.clone())
            .unwrap_or_default();
        let reading = ui.relationship_reading().ok().flatten();
        let mut snap = ProbeSnapshot::default()
            .with_field("recipe-valid", reading.is_some().to_string())
            .with_field("recipe-label", reading.as_ref().map(|r|r.snapshot.recipe.definition.label.as_str()).unwrap_or(""))
            .with_field("recipe-spacing", reading.as_ref().map(|r|r.snapshot.recipe.definition.arrangement.spacing).unwrap_or(0).to_string())
            .with_field("recipe-occurrence", reading.as_ref().and_then(|r|r.snapshot.selected_occurrence.as_deref()).unwrap_or(""))
            .with_field("recipe-relationship", reading.as_ref().and_then(|r|r.snapshot.selected_relationship.as_deref()).unwrap_or(""))
            .with_field("recipe-owner", reading.as_ref().and_then(|r|r.source.as_ref()).map(|s|s.owner.0).unwrap_or(0).to_string())
            .with_field(
                "pattern-sequential",
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ScalePattern { .. }
                    )
                }) && ui.preview_voicing().2 > 0.0)
                    .to_string(),
            )
            .with_field(
                "approach-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ChordApproach { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field("retained-sets", ui.retained_sets.entries.len().to_string())
            .with_field(
                "working-sets",
                (ui.working_sets.inactive.len() + 1).to_string(),
            )
            .with_field("working-set-id", ui.working_sets.active_id.0.to_string())
            .with_field(
                "explorations",
                (ui.catalog_explorations.inactive.len() + 1).to_string(),
            )
            .with_field(
                "exploration-id",
                ui.catalog_explorations.active_id.0.to_string(),
            )
            .with_field("catalog-root", ui.stage.root_idx.to_string())
            .with_field("catalog-lens", format!("{:?}", ui.stage.lens))
            .with_field("catalog-tuning", ui.stage.tuning().name.clone())
            .with_field("catalog-search", ui.search.text())
            .with_field(
                "runner-set-id",
                ui.rehearsal_owner.map_or(0, |id| id.0).to_string(),
            )
            .with_field("runner-cursor", ui.rehearsal_set().cursor.to_string())
            .with_field(
                "runner-foreground",
                ui.is_current_set_rehearsing().to_string(),
            )
            .with_field("overview-nodes", overview.nodes.len().to_string())
            .with_field(
                "overview-views",
                overview
                    .nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.id,
                            woodshed_core::session_overview::OverviewNodeId::View(_)
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-processes",
                overview
                    .nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.id,
                            woodshed_core::session_overview::OverviewNodeId::Process(_)
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-active",
                (ui.workspace.active_panel()
                    == Some(woodshed_views::workspace::WorkspacePanel::Overview)
                    && ui.section == woodshed_core::storage::AppSection::Stage)
                    .to_string(),
            )
            .with_field(
                "working-saved-shared-ids",
                ui.retained_sets
                    .entries
                    .first()
                    .map_or(0, |saved| {
                        ui.set
                            .cards
                            .iter()
                            .filter(|card| {
                                saved
                                    .set
                                    .cards
                                    .iter()
                                    .any(|original| original.id == card.id)
                            })
                            .count()
                    })
                    .to_string(),
            )
            .with_field(
                "approach-sequential",
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ChordApproach { .. }
                    )
                }) && ui.preview_voicing().2 > 0.0)
                    .to_string(),
            )
            .with_field(
                "approach-descends",
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ChordApproach { .. }
                    )
                }) && ui
                    .preview_voicing()
                    .0
                    .windows(2)
                    .any(|notes| notes[1] < notes[0]))
                .to_string(),
            )
            .with_field(
                "context-approaches",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == woodshed_core::stage_context::StageNodeKind::ChordApproach
                            && !node.foreground
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "rehearsal-strings",
                ui.rehearsal_board_geometry().string_count.to_string(),
            )
            .with_field(
                "context-patterns",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == woodshed_core::stage_context::StageNodeKind::ScalePattern
                            && !node.foreground
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "pattern-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ScalePattern { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "pattern-descends",
                ui.set
                    .cards
                    .get(ui.set.cursor)
                    .is_some_and(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ScalePattern { .. }
                        ) && ui
                            .preview_voicing()
                            .0
                            .windows(2)
                            .any(|notes| notes[1] < notes[0])
                    })
                    .to_string(),
            )
            .with_field("pattern-repeats", {
                let pitches = ui.preview_voicing().0;
                let unique = pitches
                    .iter()
                    .map(|pitch| pitch.to_bits())
                    .collect::<std::collections::BTreeSet<_>>();
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ScalePattern { .. }
                    )
                }) && unique.len() < pitches.len())
                .to_string()
            })
            .with_field("set-cards", observed.cards.to_string())
            .with_field("rehearsal-running", ui.rehearsal_running.to_string())
            .with_field("history-events", ui.practice_history.len().to_string())
            .with_field(
                "history-observed",
                ui.practice_history
                    .recent(1000)
                    .iter()
                    .filter(|event| event.provenance.is_some())
                    .count()
                    .to_string(),
            )
            .with_field(
                "arpeggio-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.touch,
                            woodshedding::rehearsal::Touch::Arpeggiate { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "scale-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::Scale { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "context-arpeggios",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == woodshed_core::stage_context::StageNodeKind::Arpeggio
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "arpeggio-shape",
                ui.set
                    .cards
                    .iter()
                    .find(|card| {
                        matches!(
                            card.touch,
                            woodshedding::rehearsal::Touch::Arpeggiate { .. }
                        )
                    })
                    .and_then(|card| card.setting.voicing_idx)
                    .map_or_else(|| "none".into(), |index| index.to_string()),
            )
            .with_field("cursor-id", observed.cursor_id.to_string())
            .with_field("cursor-number", observed.cursor_number.to_string())
            .with_field("cursor-label", cursor_label.clone())
            .with_field("graph-nodes", ui.set.graph().nodes.len().to_string())
            .with_field("graph-total-nodes", stage_snapshot.nodes.len().to_string())
            .with_field(
                "graph-context-nodes",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| !node.foreground)
                    .count()
                    .to_string(),
            )
            .with_field(
                "graph-reading",
                format!("{:?}", ui.app_settings.stage.set_graph_reading),
            )
            .with_field(
                "graph-context-focus",
                ui.context_focus
                    .as_ref()
                    .map(|focus| focus.wire_key())
                    .unwrap_or_default(),
            )
            .with_field("graph-edges", observed.edges.to_string())
            .with_field("graph-relations", stage_swatch.relations.len().to_string())
            .with_field(
                "graph-arrangement",
                ui.app_settings.stage.set_arrangement.label(),
            )
            .with_field("graph-routed-lanes", routed_lanes.to_string())
            .with_field("graph-labels-above", labels_above.to_string())
            .with_field("graph-labels-side", labels_side.to_string())
            .with_field("graph-width", stage_swatch.width.to_string())
            .with_field("graph-height", stage_swatch.height.to_string())
            .with_field("graph-node-cards", self.graph_node_card_count().to_string())
            .with_field(
                "graph-card-incident-relations",
                card_incident_relations.to_string(),
            )
            .with_field(
                "graph-card-anchored-relations",
                card_anchored_relations.to_string(),
            )
            .with_field("pointer-capture", self.pointer_capture_class())
            .with_field(
                "graph-visible-relations",
                relation_choices
                    .iter()
                    .filter(|relation| relation.visible)
                    .count()
                    .to_string(),
            )
            .with_field("graph-relation-choices", relation_choices.len().to_string())
            .with_field("graph-epoch", stage_snapshot.epoch().0.to_string())
            .with_field("graph-drag-active", ui.set_graph_drag_active.to_string())
            .with_field(
                "graph-selected-relation",
                ui.set_graph_relation.is_some().to_string(),
            )
            .with_field(
                "graph-moved-nodes",
                ui.set_graph_positions.len().to_string(),
            )
            .with_field(
                "view-only-dispatches",
                self.shared.view_only_dispatches.to_string(),
            )
            .with_field(
                "full-dispatch-syncs",
                self.shared.full_dispatch_syncs.to_string(),
            )
            .with_field(
                "drag-present-samples",
                self.shared.drag_frame_metrics.samples.to_string(),
            )
            .with_field(
                "drag-present-average-us",
                self.shared.drag_frame_metrics.average_us().to_string(),
            )
            .with_field(
                "drag-present-max-us",
                self.shared.drag_frame_metrics.max_us.to_string(),
            )
            .with_field(
                "drag-viewport-us",
                self.shared.drag_frame_metrics.viewport_us.to_string(),
            )
            .with_field(
                "drag-drive-us",
                self.shared.drag_frame_metrics.drive_us.to_string(),
            )
            .with_field(
                "drag-leaves-us",
                self.shared.drag_frame_metrics.leaves_us.to_string(),
            )
            .with_field(
                "drag-root-rebuilds",
                self.shared.drag_frame_metrics.root_rebuilds.to_string(),
            )
            .with_field(
                "drag-host-samples",
                self.shared.drag_frame_metrics.host_samples.to_string(),
            )
            .with_field(
                "drag-host-average-us",
                self.shared.drag_frame_metrics.host_average_us().to_string(),
            )
            .with_field(
                "drag-host-max-us",
                self.shared.drag_frame_metrics.host_max_us.to_string(),
            )
            .with_field(
                "drag-host-relayout-us",
                self.shared.drag_frame_metrics.host_relayout_us.to_string(),
            )
            .with_field(
                "drag-host-layout-update-us",
                self.shared
                    .drag_frame_metrics
                    .host_layout_update_us
                    .to_string(),
            )
            .with_field(
                "drag-host-layout-apply-us",
                self.shared
                    .drag_frame_metrics
                    .host_layout_apply_us
                    .to_string(),
            )
            .with_field(
                "drag-host-layout-mutations",
                self.shared
                    .drag_frame_metrics
                    .host_layout_mutations
                    .to_string(),
            )
            .with_field(
                "drag-host-layout-rebuilds",
                self.shared
                    .drag_frame_metrics
                    .host_layout_rebuilds
                    .to_string(),
            )
            .with_field(
                "drag-host-leaf-boxes-us",
                self.shared
                    .drag_frame_metrics
                    .host_leaf_boxes_us
                    .to_string(),
            )
            .with_field(
                "drag-host-leaf-render-us",
                self.shared
                    .drag_frame_metrics
                    .host_leaf_render_us
                    .to_string(),
            )
            .with_field(
                "drag-host-leaf-repaints",
                self.shared
                    .drag_frame_metrics
                    .host_leaf_repaints
                    .to_string(),
            )
            .with_field(
                "drag-host-fragments-us",
                self.shared.drag_frame_metrics.host_fragments_us.to_string(),
            )
            .with_field(
                "drag-host-emit-us",
                self.shared.drag_frame_metrics.host_emit_us.to_string(),
            )
            .with_field(
                "drag-host-raster-us",
                self.shared.drag_frame_metrics.host_raster_us.to_string(),
            )
            .with_field(
                "drag-host-acquire-us",
                self.shared.drag_frame_metrics.host_acquire_us.to_string(),
            )
            .with_field(
                "drag-host-clear-us",
                self.shared.drag_frame_metrics.host_clear_us.to_string(),
            )
            .with_field(
                "drag-host-compose-us",
                self.shared.drag_frame_metrics.host_compose_us.to_string(),
            )
            .with_field(
                "drag-host-present-us",
                self.shared.drag_frame_metrics.host_present_us.to_string(),
            )
            .with_field(
                "drag-host-a11y-us",
                self.shared.drag_frame_metrics.host_a11y_us.to_string(),
            )
            .with_field(
                "drag-raster-inner-us",
                self.shared.drag_frame_metrics.raster_inner_us.to_string(),
            )
            .with_field(
                "drag-raster-invalidate-us",
                self.shared
                    .drag_frame_metrics
                    .tile_invalidate_us
                    .to_string(),
            )
            .with_field(
                "drag-raster-rebuild-us",
                self.shared
                    .drag_frame_metrics
                    .dirty_tile_rebuild_us
                    .to_string(),
            )
            .with_field(
                "drag-raster-master-us",
                self.shared.drag_frame_metrics.master_compose_us.to_string(),
            )
            .with_field(
                "drag-raster-vello-us",
                self.shared.drag_frame_metrics.vello_render_us.to_string(),
            )
            .with_field(
                "drag-dirty-tiles",
                self.shared.drag_frame_metrics.dirty_tiles.to_string(),
            )
            .with_field(
                "drag-max-dirty-tiles",
                self.shared.drag_frame_metrics.max_dirty_tiles.to_string(),
            )
            .with_field("relations", observed.relations)
            .with_field("tray-expanded", ui.set_tray_expanded.to_string())
            .with_field("card-expanded", ui.set_graph_card_expanded.to_string())
            // Graph focus emphasis belongs to the canvas component now, so it
            // is observed where it actually is — the focused node's rendered
            // `data-key` — rather than from a UiState field that would report
            // "none" forever.
            .with_field("graph-focus", self.graph_focus_key())
            .with_field("section", ui.section.label().to_string())
            .with_field(
                "shape-index",
                ui.set
                    .cards
                    .get(ui.set.cursor)
                    .and_then(|card| card.setting.voicing_idx)
                    .map_or_else(|| "none".to_string(), |index| index.to_string()),
            )
            .with_field(
                "shape-dots",
                ui.set
                    .cards
                    .get(ui.set.cursor)
                    .map_or(0, |card| ui.stage.dots_for_card(card).len())
                    .to_string(),
            )
            .with_field("lens", format!("{:?}", ui.stage.lens))
            .with_field("material", ui.stage.material_name());
        // The Related frontier, observed as the app computes it: the top
        // neighbor and how many distinct relations connect it. A scenario can
        // then assert that multiplicity survived ranking, rather than sniffing
        // for a substring in the DOM.
        let related = ui.stage.related_material_configured(
            &ui.practice_history,
            &ui.app_settings.stage.related,
            8,
        );
        if let Some(top) = related.first() {
            snap = snap
                .with_field("related-top", top.title.clone())
                .with_field("related-top-relations", top.relation_count().to_string())
                .with_field(
                    "related-top-kinds",
                    top.relations
                        .iter()
                        .map(|relation| relation.kind.label())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .with_field(
                    "related-top-authorities",
                    top.relations
                        .iter()
                        .map(|relation| format!("{:?}", relation.authority))
                        .collect::<Vec<_>>()
                        .join(","),
                );
        }
        snap = snap
            .with_field("related-count", related.len().to_string())
            .with_field(
                "related-multi-count",
                related
                    .iter()
                    .filter(|neighbor| neighbor.relation_count() > 1)
                    .count()
                    .to_string(),
            );
        // The richest pair on the frontier: the one carrying the most relations
        // at once. A single top row can honestly have one reason (an arpeggio
        // realizes its chord, and that is all it does), so multiplicity is
        // asserted where it actually lives.
        if let Some(richest) = related
            .iter()
            .max_by_key(|neighbor| (neighbor.relation_count(), neighbor.score))
        {
            snap = snap
                .with_field("related-richest", richest.title.clone())
                .with_field(
                    "related-richest-relations",
                    richest.relation_count().to_string(),
                )
                .with_field(
                    "related-richest-kinds",
                    richest
                        .relations
                        .iter()
                        .map(|relation| relation.kind.label())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .with_field(
                    "related-richest-authorities",
                    richest
                        .relations
                        .iter()
                        .map(|relation| format!("{:?}", relation.authority))
                        .collect::<Vec<_>>()
                        .join(","),
                );
        }
        if !cursor_label.is_empty() {
            snap = snap.with_focus(cursor_label);
        }
        snap
    }
}

impl Probe<'_, '_> {
    fn stage_node_key(&self, index: usize) -> Option<String> {
        let ui = self.ctx.runner.state();
        let snapshot = set_graph_snapshot(ui);
        set_graph_swatch_from_snapshot(&snapshot, ui, ui.set_tray_expanded)
            .graph
            .nodes
            .get(index)
            .and_then(|node| node.key.clone())
    }

    fn stage_relation_id(&self, index: usize) -> Option<String> {
        let ui = self.ctx.runner.state();
        let snapshot = set_graph_snapshot(ui);
        set_graph_swatch_from_snapshot(&snapshot, ui, ui.set_tray_expanded)
            .relations
            .get(index)
            .map(|relation| relation.id.clone())
    }
}

impl Automatable for Probe<'_, '_> {
    fn click_target(&mut self, selector: &Selector) -> Option<bool> {
        Some(self.clicks.click(self.ctx, selector, |_, _, r| {
            (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5)
        }))
    }

    fn selector_target(&self, selector: &Selector) -> taproot::SelectorTarget {
        let candidates = {
            let dom = self.ctx.runner.dom();
            let dom = dom.borrow();
            taproot::matching(&dom, selector)
        };
        let (width, height) = self.ctx.logical_size;
        for node in candidates {
            let Some((x, y, w, h)) = self.ctx.painted_rect(node) else {
                continue;
            };
            if ![x, y, w, h].iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
                continue;
            }
            let point = (x + w / 2.0, y + h / 2.0);
            if point.0 >= 0.0 && point.1 >= 0.0 && point.0 < width && point.1 < height {
                return taproot::SelectorTarget::Hit(taproot::Hit {
                    surface: "woodshed",
                    point,
                });
            }
        }
        // Participating hosts own both hits and misses. Never re-layout after
        // a missing, hidden or offscreen retained target.
        taproot::SelectorTarget::Miss
    }

    fn with_surfaces<R>(&self, f: impl FnOnce(&[ProbeSurface<'_>]) -> R) -> R {
        let dom = self.ctx.runner.dom();
        let dom_ref = dom.borrow();
        let (w, h) = self.ctx.logical_size;
        f(&[ProbeSurface {
            name: "woodshed",
            dom: &dom_ref,
            // One runner covers the window, and the probe resolves in the same
            // logical coordinates the layout and the cursor use.
            rect: [0.0, 0.0, w, h],
            sheet: &self.shared.accessible_sheet(),
        }])
    }

    fn snapshot(&self) -> ProbeSnapshot {
        Snapshot {
            ctx: self.ctx,
            shared: self.shared,
        }
        .snapshot()
    }

    fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.shared.events)
    }

    fn act(&mut self, label: &str) -> bool {
        if label == "reset-dispatch-sync-counts" {
            self.shared.view_only_dispatches = 0;
            self.shared.full_dispatch_syncs = 0;
            self.shared.drag_frame_metrics.reset();
            self.shared.scenario_drag_origin = None;
            return true;
        }
        let mut known = true;
        self.ctx.runner.update(|ui| match label {
            "tone-relationships-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                ui.stop_rehearsal();
                ui.set = Default::default();
                ui.working_sets = Default::default();
                ui.relationship_reading_json = None;
                for (label, formula, root) in [("Cmaj7", "chord:Major 7", 0), ("Am7", "chord:Minor 7", 9), ("C Major scale", "scale:Major", 0)] {
                    let mut card = KeyedCatalogRef { formula_id: formula.into(), root: PitchClass::new(root) }.to_card().unwrap();
                    card.label = label.into();
                    ui.set.push(card);
                }
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Overview);
            },
            "relationship-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                ui.stop_rehearsal();
                ui.set = Default::default();
                ui.working_sets = Default::default();
                ui.relationship_reading_json = None;
                for (label, formula, root) in [("C Major", "Major", 0),("C Major again", "Major",0),("A Minor","Minor",9)] {
                    let mut card = KeyedCatalogRef {formula_id:format!("chord:{formula}"),root:PitchClass::new(root)}.to_card().unwrap();
                    card.label=label.into();
                    ui.set.push(card);
                }
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Overview);
            },
            "stage-current" => ui.stage_current(None),
            "connected-practice-example" | "connected-scale-setup-example" => {
                use woodshedding::rehearsal::{Hold, Set, Timing};
                ui.set = Set::default();
                ui.practice_history = Default::default();
                ui.stage.set_lens(Lens::Chords);
                for (root, name) in [(3, "Major 7"), (0, "Minor 7")] {
                    ui.stage.set_root(root);
                    ui.root_dd.selected = root;
                    let index = ui
                        .stage
                        .chords()
                        .iter()
                        .position(|formula| formula.name == name)
                        .expect("fixture chord exists");
                    ui.stage.select_chord(index);
                    ui.stage_current(None);
                }
                ui.set.cursor = 0;
                ui.step_card_shape(1);
                for card in &mut ui.set.cards {
                    card.timing = Timing {
                        bpm: Some(120.0),
                        hold: Hold::Seconds(2.0),
                    };
                }
                if label == "connected-scale-setup-example" {
                    for card in &mut ui.set.cards {
                        card.setting.instrument = "Ukulele".into();
                        card.setting.tuning = Some("Standard (high-G)".into());
                        card.setting.capo = Some(2);
                        card.setting.fret_window =
                            Some(woodshedding::rehearsal::FretWindow { start: 2, span: 4 });
                    }
                    ui.step_card_shape(1);
                }
                ui.set_graph_card_expanded = true;
                ui.set_graph_reading(woodshed_core::settings::StageGraphReading::CircleOfFifths);
            },
            "chord-approach-example" | "mere-example" => {
                use woodshedding::rehearsal::{FretWindow, Hold, Set, Timing};
                ui.stop_rehearsal();
                ui.set = Set::default();
                ui.practice_history = Default::default();
                ui.stage.set_lens(Lens::Chords);
                for (root, name) in [(0, "Minor 7"), (3, "Major 7")] {
                    ui.stage.set_root(root);
                    ui.root_dd.selected = root;
                    let index = ui
                        .stage
                        .chords()
                        .iter()
                        .position(|formula| formula.name == name)
                        .expect("fixture chord exists");
                    ui.stage.select_chord(index);
                    ui.stage_current(None);
                }
                ui.set.cards[0].setting.fret_window = Some(FretWindow { start: 2, span: 6 });
                ui.set.cursor = 0;
                ui.step_card_shape(1);
                let target = &mut ui.set.cards[1];
                target.setting.instrument = "Ukulele".into();
                target.setting.tuning = Some("Standard (high-G)".into());
                target.setting.capo = Some(2);
                target.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
                ui.set.cursor = 1;
                ui.step_card_shape(1);
                for card in &mut ui.set.cards {
                    card.timing = Timing {
                        bpm: None,
                        hold: Hold::Bars(1),
                    };
                }
                ui.transport.bpm = 80.0;
                ui.set.cursor = 0;
                ui.set_graph_card_expanded = true;
                ui.set_graph_reading(woodshed_core::settings::StageGraphReading::CircleOfFifths);
                if label == "mere-example" {
                    ui.retained_sets = Default::default();
                    ui.working_sets = Default::default();
                    ui.catalog_explorations = Default::default();
                    for card in &mut ui.set.cards {
                        card.timing.hold = Hold::Manual;
                    }
                }
            },
            "shape-comparison-example" => {
                ui.stage.set_root(3);
                ui.root_dd.selected = 3;
                ui.stage.set_lens(Lens::Chords);
                if let Some(index) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major")
                {
                    ui.stage.select_chord(index);
                }
                ui.stage_current(None);
                ui.step_card_shape(1);
                let index = ui.set.cursor;
                ui.set.duplicate(index);
                ui.set.cursor = index + 1;
                ui.step_card_shape(1);
                ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
            },
            "stage-context-example" => {
                ui.stage.set_root(3); // C in the A-first root picker.
                ui.root_dd.selected = 3;
                ui.stage.set_lens(Lens::Chords);
                if let Some(major) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major")
                {
                    ui.stage.select_chord(major);
                    ui.stage_current(None);
                }
                ui.set_tray_expanded = true;
            },
            "stage-related-pair" => {
                ui.stage.set_lens(Lens::Chords);
                if let Some(major) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major")
                {
                    ui.stage.select_chord(major);
                    ui.stage_current(None);
                }
                if let Some(major_seven) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major 7")
                {
                    ui.stage.select_chord(major_seven);
                    ui.stage_current(None);
                }
                ui.set_tray_expanded = true;
            },
            "hide-first-stage-relation" => {
                let snapshot = set_graph_snapshot(ui);
                if let Some(key) = set_graph_relation_choices(&snapshot, ui)
                    .first()
                    .map(|relation| relation.key.clone())
                {
                    ui.toggle_set_graph_relation(&snapshot, key);
                }
            },
            "hide-all-stage-relations" => {
                let snapshot = set_graph_snapshot(ui);
                ui.hide_all_set_graph_relations(&snapshot);
            },
            "show-all-stage-relations" => ui.show_all_set_graph_relations(),
            "expand-tray" => ui.set_tray_expanded = true,
            "collapse-tray" => ui.set_tray_expanded = false,
            "duplicate-selected" => {
                let cursor = ui.set.cursor;
                ui.set.duplicate(cursor);
            },
            "move-selected-down" => {
                let cursor = ui.set.cursor;
                ui.set.move_card(cursor, 1);
            },
            "move-selected-up" => {
                let cursor = ui.set.cursor;
                ui.set.move_card(cursor, -1);
            },
            "remove-selected" => {
                let cursor = ui.set.cursor;
                ui.set.remove(cursor);
            },
            _ => known = false,
        });
        if known {
            crate::sync::after_dispatch(self.shared, self.ctx);
            note_events(self.shared, self.ctx.runner.state());
        }
        known
    }

    fn press(&mut self, x: f32, y: f32) {
        // Routed by the host: the same hit test, capture, and dispatch a real
        // pointer takes. Delivered once this hook returns, and observed by the
        // next frame's tick — which is where the scenario asserts anyway.
        self.ctx.pointer.push(HostPointer::Press(x, y));
    }

    fn moved(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Moved(x, y));
    }

    fn release(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Release(x, y));
    }

    fn busy(&mut self) -> Option<bool> {
        // Woodshed has no fetch or actor round-trip in this lane: a step's
        // effect is in the state by the time the next frame lays out. Reporting
        // quiet is honest here, and keeps `wait` from burning its cap.
        Some(false)
    }
}

impl Probe<'_, '_> {
    /// Stage-canvas receipt verbs resolve epoch-qualified identities from the
    /// live snapshot, then use the ordinary host pointer lifecycle. This keeps
    /// them distinct from the Related graph, which shares the same CSS classes.
    fn app_step(&mut self, line: &str) -> Result<(), String> {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("ui-zoom") => {
                let zoom: f32 = parts
                    .next()
                    .ok_or("ui-zoom wants a scale")?
                    .parse()
                    .map_err(|_| "invalid UI zoom")?;
                if parts.next().is_some() || !zoom.is_finite() || !(0.5..=2.0).contains(&zoom) {
                    return Err("ui-zoom requires a scale from 0.5 to 2".into());
                }
                *self.ctx.set_ui_zoom = Some(zoom);
                Ok(())
            },
            Some("pointer-click") => {
                // For receipts measured from a presented frame when the probe's
                // independently computed selector layout disagrees with the host.
                let x: f32 = parts
                    .next()
                    .ok_or("pointer-click wants x y")?
                    .parse()
                    .map_err(|_| "invalid pointer x")?;
                let y: f32 = parts
                    .next()
                    .ok_or("pointer-click wants x y")?
                    .parse()
                    .map_err(|_| "invalid pointer y")?;
                let (w, h) = self.ctx.logical_size;
                if parts.next().is_some()
                    || !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= w
                    || y >= h
                {
                    return Err("pointer-click requires an in-window logical point".into());
                }
                self.moved(x, y);
                self.press(x, y);
                self.release(x, y);
                Ok(())
            },
            Some("open-stage-arrangement") => {
                if parts.next().is_some() {
                    return Err("open-stage-arrangement takes no arguments".to_string());
                }
                let selector = Selector::role("combobox").containing("Set arrangement");
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| "open-stage-arrangement missed the picker".to_string())
            },
            Some("choose-stage-arrangement") => {
                let label = parts
                    .next()
                    .ok_or("choose-stage-arrangement wants one arrangement label")?;
                if parts.next().is_some() {
                    return Err("choose-stage-arrangement takes one arrangement label".to_string());
                }
                let selector = Selector::role("option").containing(label);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("choose-stage-arrangement missed {label}"))
            },
            Some("click-context-node") => {
                let target = parts.collect::<Vec<_>>().join(" ");
                let node_key = {
                    let ui = self.ctx.runner.state();
                    let snapshot = set_graph_snapshot(ui);
                    let swatch =
                        set_graph_swatch_from_snapshot(&snapshot, ui, ui.set_tray_expanded);
                    swatch.graph.nodes.iter().find_map(|node| {
                        let subject = snapshot.node_of(node.id.instance)?;
                        (!subject.foreground && subject.id.wire_key() == target)
                            .then(|| node.key.clone())
                            .flatten()
                    })
                }
                .ok_or_else(|| format!("no context node {target}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("click-context-node missed {target}"))
            },
            Some("click-stage-node") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("click-stage-node wants a node index")?;
                if parts.next().is_some() {
                    return Err("click-stage-node takes one index".to_string());
                }
                let node_key = self
                    .stage_node_key(index)
                    .ok_or_else(|| format!("no Stage node at index {index}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("click-stage-node missed index {index}"))
            },
            Some("click-stage-relation") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("click-stage-relation wants a relation index")?;
                if parts.next().is_some() {
                    return Err("click-stage-relation takes one index".to_string());
                }
                let relation_id = self
                    .stage_relation_id(index)
                    .ok_or_else(|| format!("no Stage relation at index {index}"))?;
                let selector = Selector::class("graph-canvas-swatch-relation")
                    .with_attr("data-relation-id", relation_id);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("click-stage-relation missed index {index}"))
            },
            Some("drag-stage-node") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-node wants a node index")?;
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-node wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-node wants a y delta")?;
                if parts.next().is_some() {
                    return Err("drag-stage-node takes exactly index, dx, and dy".to_string());
                }
                let node_key = self
                    .stage_node_key(index)
                    .ok_or_else(|| format!("no Stage node at index {index}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                let hit = self
                    .resolve(&selector)
                    .ok_or_else(|| format!("drag-stage-node missed index {index}"))?;
                let start = hit.point;
                let end = (start.0 + dx, start.1 + dy);
                self.press(start.0, start.1);
                self.moved((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
                self.moved(end.0, end.1);
                self.release(end.0, end.1);
                Ok(())
            },
            Some("drag-stage-graph-resize") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-graph-resize wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-graph-resize wants a y delta")?;
                if parts.next().is_some() {
                    return Err("drag-stage-graph-resize takes exactly dx and dy".to_string());
                }
                let selector = Selector::class("resize-handle");
                let hit = self
                    .resolve(&selector)
                    .ok_or("drag-stage-graph-resize missed the handle")?;
                let start = hit.point;
                let end = (start.0 + dx, start.1 + dy);
                self.press(start.0, start.1);
                self.moved((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
                self.moved(end.0, end.1);
                self.release(end.0, end.1);
                Ok(())
            },
            Some("press-stage-graph-resize") => {
                if parts.next().is_some() {
                    return Err("press-stage-graph-resize takes no arguments".to_string());
                }
                let selector = Selector::class("resize-handle");
                let hit = self
                    .resolve(&selector)
                    .ok_or("press-stage-graph-resize missed the handle")?;
                self.shared.scenario_drag_origin = Some(hit.point);
                self.press(hit.point.0, hit.point.1);
                Ok(())
            },
            Some("move-stage-graph-resize") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-graph-resize wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-graph-resize wants a y delta")?;
                if parts.next().is_some() {
                    return Err("move-stage-graph-resize takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .ok_or("move-stage-graph-resize has no pressed handle")?;
                self.moved(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            Some("release-stage-graph-resize") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-graph-resize wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-graph-resize wants a y delta")?;
                if parts.next().is_some() {
                    return Err("release-stage-graph-resize takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .take()
                    .ok_or("release-stage-graph-resize has no pressed handle")?;
                self.release(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            Some("press-stage-node") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("press-stage-node wants a node index")?;
                if parts.next().is_some() {
                    return Err("press-stage-node takes one index".to_string());
                }
                let node_key = self
                    .stage_node_key(index)
                    .ok_or_else(|| format!("no Stage node at index {index}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                let hit = self
                    .resolve(&selector)
                    .ok_or_else(|| format!("press-stage-node missed index {index}"))?;
                self.shared.scenario_drag_origin = Some(hit.point);
                self.press(hit.point.0, hit.point.1);
                Ok(())
            },
            Some("move-stage-node") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-node wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-node wants a y delta")?;
                if parts.next().is_some() {
                    return Err("move-stage-node takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .ok_or("move-stage-node has no pressed node")?;
                self.moved(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            Some("release-stage-node") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-node wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-node wants a y delta")?;
                if parts.next().is_some() {
                    return Err("release-stage-node takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .take()
                    .ok_or("release-stage-node has no pressed node")?;
                self.release(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            _ => Err(format!("unknown verb: {line}")),
        }
    }
}
