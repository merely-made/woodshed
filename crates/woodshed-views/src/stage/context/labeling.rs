use super::*;

pub(super) fn context_title(material: &KeyedCatalogRef) -> String {
    let family = material
        .formula_id
        .split_once(':')
        .map(|(kind, _)| kind)
        .unwrap_or("catalog");
    format!(
        "{} · {}",
        family,
        material.label().unwrap_or_else(|| material.wire_key())
    )
}

/// Compact musical labels stay next to their dots as focus changes the edges.
/// Full names remain on the graph's accessible node targets and focus panel.
pub(in super::super) fn labels(
    ui: &UiState,
    swatch: &cambium::GraphCanvasSwatch<super::super::StageInstanceRef, &'static str>,
) -> UiChild {
    if !matches!(
        ui.app_settings.stage.set_graph_reading,
        woodshed_core::settings::StageGraphReading::CircleOfFifths
            | woodshed_core::settings::StageGraphReading::Tonnetz
            | woodshed_core::settings::StageGraphReading::PitchMotion
    ) || ui.set_graph_drag_active
    {
        return Box::new(el("div", ()));
    }
    let mut labels: Vec<UiChild> = swatch
        .graph
        .nodes
        .iter()
        .zip(swatch.projected_positions())
        .filter(|(node, _)| swatch.projected_node_footprint(&node.id).is_none())
        .map(|(node, (_, (x, y)))| {
            let tonnetz = ui.app_settings.stage.set_graph_reading
                == woodshed_core::settings::StageGraphReading::Tonnetz;
            let label = node
                .label
                .replace("Major", "maj")
                .replace("Minor", "min")
                .replace("Dominant", "dom");
            let label = if tonnetz {
                label.replace(" maj", "").replace(" min", "m")
            } else {
                label
            };
            let width = label.chars().count() as f32 * 5.8 + 4.0;
            let left = if tonnetz {
                x - width / 2.0
            } else if x > swatch.width as f32 * 0.65 {
                x - width - 10.0
            } else {
                x + 10.0
            };
            let active = swatch.focus.as_ref() == Some(&node.id)
                || swatch.selected.as_ref() == Some(&node.id);
            Box::new(
                el("span", text(label))
                    .attr(
                        "class",
                        if active {
                            "stage-context-node-label active"
                        } else {
                            "stage-context-node-label"
                        },
                    )
                    .attr("aria-hidden", "true")
                    .attr(
                        "style",
                        format!(
                            "left:{}px;top:{}px;width:{width}px;",
                            left.max(0.0),
                            (if tonnetz { y + 5.0 } else { y - 6.0 }).max(0.0)
                        ),
                    ),
            ) as UiChild
        })
        .collect();
    labels.extend(tonnetz_pitch_labels(ui, swatch));
    labels.extend(super::super::pitch_motion::guide_labels(ui, swatch));
    Box::new(el("div", labels).attr("class", "stage-context-labels"))
}

/// Pointer-transparent pitch labels over the natively painted Tonnetz. The core
/// supplies the musical triangle and fixed lattice position; this layer only
/// projects those world coordinates into the current swatch viewport.
pub(super) fn tonnetz_pitch_labels(
    ui: &UiState,
    swatch: &cambium::GraphCanvasSwatch<super::super::StageInstanceRef, &'static str>,
) -> Vec<UiChild> {
    if ui.app_settings.stage.set_graph_reading
        != woodshed_core::settings::StageGraphReading::Tonnetz
    {
        return Vec::new();
    }
    let snapshot = super::super::set_graph_snapshot(ui);
    let materials = snapshot
        .nodes
        .iter()
        .filter_map(|node| node.keyed.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let inset = swatch.node_radius + swatch.edge_width;
    let project = |point: (f32, f32)| {
        let normalized = super::super::scene_position(&snapshot, point.0, point.1);
        swatch.viewport.project(
            normalized,
            sprigging::Size {
                width: swatch.width as f32,
                height: swatch.height as f32,
            },
            inset,
        )
    };
    let mut cells = Vec::new();
    let mut labels = std::collections::BTreeMap::<(i32, i32), String>::new();
    for keyed in materials {
        let Some(vertices) = woodshed_core::tonnetz::triangle(&keyed) else {
            continue;
        };
        for (x, y, pitch) in vertices {
            labels
                .entry((x.round() as i32, y.round() as i32))
                .or_insert_with(|| {
                    let p = Pitch::from_midi(60 + i32::from(pitch.value()), Spelling::Sharps);
                    format!("{}{}", p.name, p.accidental)
                });
        }
    }
    for ((x, y), label) in labels {
        let (px, py) = project((x as f32, y as f32));
        cells.push(Box::new(
            el("span", text(label))
                .attr("class", "stage-context-node-label")
                .attr(
                    "style",
                    format!(
                        "left:{:.1}px;top:{:.1}px;width:22px;height:13px;",
                        px - 5.0,
                        py - 6.0
                    ),
                ),
        ) as UiChild);
    }
    cells
}

pub(super) fn pitch_class_names(
    set: &std::collections::BTreeSet<woodshedding::pitch::PitchClass>,
) -> String {
    if set.is_empty() {
        return "none".to_string();
    }
    set.iter()
        .map(|tone| {
            let pitch = Pitch::from_midi(60 + i32::from(tone.value()), Spelling::Sharps);
            format!("{}{}", pitch.name, pitch.accidental)
        })
        .collect::<Vec<_>>()
        .join(" ")
}
