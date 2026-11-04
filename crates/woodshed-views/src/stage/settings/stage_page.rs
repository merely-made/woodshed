use super::*;

pub(super) fn stage_page(ui: &UiState) -> UiChild {
    let history_label = if ui.app_settings.stage.related.use_history {
        "History ranking: on"
    } else {
        "History ranking: off"
    };
    let graph_label = if ui.app_settings.stage.related.show_neighborhood {
        "Neighborhood graph: on"
    } else {
        "Neighborhood graph: off"
    };
    let scope_label = format!(
        "Graph scope: {}",
        ui.app_settings.stage.related.graph_scope.label()
    );
    let depth_label = format!(
        "Selection depth: {}",
        ui.app_settings.stage.related.relation_depth
    );
    let swatch_arrangement = format!(
        "Mere arrangement: {}",
        ui.app_settings.stage.related.arrangement.label()
    );
    let set_arrangement = format!(
        "Set arrangement: {}",
        ui.app_settings.stage.set_arrangement.label()
    );
    let reading = format!(
        "Set reading: {}",
        ui.app_settings.stage.set_graph_reading.label()
    );
    let context = if ui.app_settings.stage.context.enabled {
        format!(
            "Circle context: on · {} nodes",
            ui.app_settings.stage.context.node_limit()
        )
    } else {
        "Circle context: off".to_string()
    };
    let (set_graph_width, set_graph_height) = ui.app_settings.stage.set_graph_size();
    let set_graph_size = format!("Set canvas: {set_graph_width} × {set_graph_height} · reset");
    let hidden_count = ui.app_settings.stage.related.dismissed_ids.len();
    // One entry per derivable relation family, so the Set graph's visible
    // relations are configured here rather than through a single switch that
    // stops describing the projection as families are added.
    let relation_items: Vec<UiChild> = SetGraphEdgeKind::ALL
        .into_iter()
        .map(|kind| {
            let state = if ui.app_settings.stage.shows_relation(kind) {
                "on"
            } else {
                "off"
            };
            Box::new(clickable(
                el(
                    "div",
                    text(format!(
                        "Set {} edges: {state}",
                        kind.label().to_lowercase()
                    )),
                )
                .attr("class", "side-item"),
                move |ui: &mut UiState, _| {
                    ui.app_settings.stage.toggle_relation(kind);
                    ui.set_graph_relation = None;
                },
            )) as UiChild
        })
        .collect();
    Box::new(
        el(
            "div",
            (
                el("div", text("Stage")).attr("class", "settings-heading"),
                clickable(
                    el("div", text(history_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.use_history = !related.use_history;
                    },
                ),
                clickable(
                    el("div", text(graph_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.show_neighborhood = !related.show_neighborhood;
                    },
                ),
                clickable(
                    el("div", text(scope_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.graph_scope = related.graph_scope.toggle();
                        ui.related_relation = None;
                    },
                ),
                clickable(
                    el("div", text(depth_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let depth = &mut ui.app_settings.stage.related.relation_depth;
                        *depth = if *depth >= 6 { 1 } else { *depth + 1 };
                        ui.related_relation = None;
                    },
                ),
                clickable(
                    el("div", text(swatch_arrangement)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.arrangement = related.arrangement.next();
                    },
                ),
                clickable(
                    el("div", text(set_arrangement)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        ui.set_graph_arrangement(ui.app_settings.stage.set_arrangement.next());
                    },
                ),
                clickable(
                    el("div", text(reading)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let reading = ui.app_settings.stage.set_graph_reading.next();
                        ui.set_graph_reading(reading);
                    },
                ),
                clickable(
                    el("div", text(context)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        ui.app_settings.stage.context.enabled =
                            !ui.app_settings.stage.context.enabled;
                    },
                ),
                clickable(
                    el("div", text("Context breadth: next")).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let limit = ui.app_settings.stage.context.node_limit();
                        ui.app_settings.stage.context.node_limit =
                            if limit >= 36 { 12 } else { limit + 12 };
                    },
                ),
                clickable(
                    el("div", text(set_graph_size)).attr("class", "side-item"),
                    |ui: &mut UiState, _| ui.app_settings.stage.reset_set_graph_size(),
                ),
                clickable(
                    el("div", text(format!("Restore hidden ({hidden_count})")))
                        .attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        ui.app_settings.stage.related.dismissed_ids.clear();
                    },
                ),
                el("div", relation_items),
            ),
        )
        .attr("class", "board settings-page"),
    )
}
