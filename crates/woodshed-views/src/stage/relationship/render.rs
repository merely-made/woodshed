use super::*;

pub(in super::super) fn screen(ui: &UiState) -> UiChild {
    let mut children: Vec<UiChild> = vec![
        Box::new(el("h3", text("Relationship recipe"))),
        action("Back to Mere", "t-btn relationship-back", |ui| {
            ui.relationship_open = false
        }),
        Box::new(el(
            "p",
            text(
                "Choose occurrences from the current Set. Consecutive selected Cards in authored order supply exact shared tones, differences, equality or directed containment. Sparse choices compare the selected sequence, not every intervening Card. A retained reading keeps its captured evidence until you explicitly rebind.",
            ),
        )),
    ];
    let choices: Vec<UiChild> = ui
        .set
        .cards
        .iter()
        .take(64)
        .enumerate()
        .map(|(index, card)| {
            let id = card.id;
            let selected = ui.relationship_cards.contains(&id);
            action(
                format!(
                    "{} {} · {}",
                    if selected { "Selected" } else { "Choose" },
                    index + 1,
                    card.label
                ),
                "t-btn relationship-material",
                move |ui| {
                    if ui.relationship_cards.contains(&id) {
                        ui.relationship_cards.retain(|c| *c != id)
                    } else {
                        ui.relationship_cards.push(id)
                    }
                },
            )
        })
        .collect();
    children.push(Box::new(
        el("div", choices).attr("class", "relationship-choices"),
    ));
    children.push(action(
        if ui.relationship_reading_json.is_some() {
            "Rebind recipe to selected Cards"
        } else {
            "Use selected Card relationships"
        },
        "t-btn relationship-bind",
        |ui| {
            let result = ui.bind_relationship_recipe(ui.relationship_reading_json.is_some());
            ui.relationship_result(result)
        },
    ));
    if let Some(notice) = &ui.relationship_notice {
        children.push(Box::new(
            el("p", text(notice.clone())).attr("class", "relationship-notice"),
        ));
    }
    match ui.relationship_reading() {
        Ok(Some(reading)) => {
            let compiled = reading.compile().expect("decoded reading validated");
            if reading.version >= 3 {
                let count = reading.dataset.dataset.occurrences.len();
                children.push(Box::new(el(
                    "p",
                    text(format!(
                        "{count} selected occurrences · {} consecutive comparisons",
                        count.saturating_sub(1)
                    )),
                )));
            }
            children.push(Box::new(el(
                "h4",
                text(reading.snapshot.recipe.definition.label.clone()),
            )));
            children.push(Box::new(
                el(
                    "div",
                    map_state(text_field(&ui.relationship_label), |ui: &mut UiState| {
                        &mut ui.relationship_label
                    }),
                )
                .attr("class", "relationship-label"),
            ));
            children.push(action(
                "Apply recipe label",
                "t-btn relationship-apply-label",
                |ui| {
                    let r = ui.edit_relationship_recipe(
                        Some(ui.relationship_label.text().to_string()),
                        None,
                    );
                    ui.relationship_result(r)
                },
            ));
            let spacing = reading.snapshot.recipe.definition.arrangement.spacing;
            children.push(Box::new(el("p", text(format!("Grid spacing: {spacing}")))));
            children.push(action(
                "Increase recipe spacing",
                "t-btn relationship-spacing",
                move |ui| {
                    let r = ui.edit_relationship_recipe(None, Some(spacing.saturating_add(8)));
                    ui.relationship_result(r)
                },
            ));
            for compiled_relation in &compiled.relationships {
                let relation = &compiled_relation.disclosure;
                let id = relation.id.clone();
                let left = relation.from_occurrence.clone();
                children.push(action(
                    format!("Explain {}", relation.label),
                    "t-btn relationship-explain",
                    move |ui| {
                        let r = ui.select_relationship(Some(left.clone()), Some(id.clone()));
                        ui.relationship_result(r)
                    },
                ));
                if reading.snapshot.selected_relationship.as_ref() == Some(&relation.id) {
                    children.push(Box::new(
                        el(
                            "p",
                            text(format!(
                                "{} Method: {} v{} · {}.",
                                relation.explanation,
                                relation.provenance.method,
                                relation.provenance.method_version,
                                relation.provenance.provider
                            )),
                        )
                        .attr("class", "relationship-explanation"),
                    ));
                }
            }
            {
                let mut swatch = swatch_from(
                    &reading,
                    &compiled,
                    ui.viewport_width,
                    ui.relationship_opened_link.as_ref(),
                );
                swatch.viewport = ui.relationship_viewport;
                children.push(Box::new(
                    el(
                        "div",
                        el(
                            "div",
                            graph_canvas_with_wheel(&swatch, |ui: &mut UiState, input| {
                                let event = match input {
                                    GraphCanvasInput::Wheel(event) => {
                                        super::super::canvas_camera::apply_wheel(
                                            ui,
                                            super::super::canvas_camera::CanvasSurface::Relationship,
                                            &event,
                                        );
                                        return;
                                    },
                                    GraphCanvasInput::Event(event) => event,
                                };
                                let r = match event {
                                    GraphCanvasEvent::Activate(id) => {
                                        ui.relationship_opened_link = None;
                                        ui.relationship_opened_link_layout = None;
                                        ui.select_relationship(Some(id), None)
                                    },
                                    GraphCanvasEvent::RelationActivate(id) => {
                                        open_relationship_link(ui, &id)
                                    },
                                    GraphCanvasEvent::Pan { delta } => {
                                        ui.relationship_viewport.pan.0 += delta.0;
                                        ui.relationship_viewport.pan.1 += delta.1;
                                        return;
                                    },
                                    GraphCanvasEvent::Zoom { factor } => {
                                        let command = super::super::canvas_camera::zoom_by(
                                            ui,
                                            super::super::canvas_camera::CanvasSurface::Relationship,
                                            factor,
                                        );
                                        super::super::canvas_camera::apply_canvas_command(
                                            ui,
                                            super::super::canvas_camera::CanvasSurface::Relationship,
                                            command,
                                        );
                                        return;
                                    },
                                    _ => return,
                                };
                                ui.relationship_result(r)
                            }),
                        )
                        .attr(
                            "style",
                            format!("width:{}px;height:{}px;", swatch.width, swatch.height),
                        ),
                    )
                    .attr("class", "relationship-graph"),
                ));
                children.push(Box::new(
                    el(
                        "div",
                        super::super::canvas_camera::camera_controls(
                            super::super::canvas_camera::CanvasSurface::Relationship,
                            "relationship-camera",
                        ),
                    )
                    .attr("class", "relationship-camera-controls"),
                ));
            }
            if let Some(key) = ui.relationship_opened_link.as_ref() {
                if let Some(pair) = relationship_pair_reading(&reading, key) {
                    let swatch = Swatch {
                        id: format!("woodshed-relationship-link-{}", key.stable_id()),
                        scope: Scope::Link(key.clone()),
                        mode: SwatchMode::Projection {
                            definition_id: "opened-link".into(),
                            variant_id: None,
                        },
                    };
                    let layout = ui
                        .relationship_opened_link_layout
                        .clone()
                        .unwrap_or_else(|| OpenedLinkLayout::default_for(&pair));
                    let can_open_source = relationship_source_is_current(ui, &reading);
                    children.push(Box::new(cambium::opened_link_swatch(
                        &swatch,
                        &pair,
                        &layout,
                        scenograph::link::OpenedLinkOptions { now_ms: ui.now_ms },
                        false,
                        move |_source, _key, bucket, _assertion| {
                            if can_open_source {
                                vec![LinkMemberAction {
                                    id: "open-source-card".into(),
                                    label: format!(
                                        "Open source Card for {} → {}",
                                        bucket.displayed_from, bucket.displayed_to
                                    ),
                                }]
                            } else {
                                Vec::new()
                            }
                        },
                        handle_relationship_link_event,
                    )));
                }
            }
            for occurrence in &reading.dataset.dataset.occurrences {
                let id = occurrence.occurrence_id.clone();
                let label = compiled
                    .projection
                    .instance_by_occurrence
                    .get(&id)
                    .and_then(|i| compiled.projection.labels.get(i))
                    .cloned()
                    .unwrap_or_else(|| id.clone());
                children.push(action(
                    format!("Inspect {label} · {id}"),
                    "t-btn relationship-occurrence",
                    move |ui| {
                        let r = ui.select_relationship(Some(id.clone()), None);
                        ui.relationship_result(r)
                    },
                ));
            }

            children.push(action(
                "Open selected source Card",
                "t-btn relationship-source",
                |ui| {
                    let r = ui.open_relationship_source();
                    ui.relationship_result(r)
                },
            ));
            children.push(Box::new(el("p",text("This captured reading is included in session persistence. Reopening preserves its evidence and selections; opening a source Card validates the current owner and instructions."))));
        },
        Err(error) => children.push(Box::new(
            el(
                "p",
                text(format!(
                    "{error} The retained payload is preserved; your Sets remain available."
                )),
            )
            .attr("class", "relationship-refusal"),
        )),
        Ok(None) => {},
    }
    Box::new(el("div", children).attr("class", "relationship-screen claims-link-inspection"))
}
