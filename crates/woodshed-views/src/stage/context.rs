//! View-only controls for the wider Stage context graph.
//!
//! The graph projection itself lives beside the Stage scene adapter. This
//! module owns the interaction boundary: background focus is a selection aid,
//! while hearing and adding to the Set are explicit actions.

use cambium::{clickable, el, text};
use woodshed_core::Lens;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::harmony::compare_pitch_sets;
use woodshed_core::history::ObservationProvenance;
use woodshedding::pitch::{Pitch, Spelling};
use woodshedding::rehearsal::Material;

use super::{UiChild, UiState};

#[cfg(test)]
#[path = "context/tests.rs"]
mod tests;

fn comparison(ui: &UiState, focused: &KeyedCatalogRef) -> Option<UiChild> {
    let card = ui
        .set
        .cards
        .get(ui.set.cursor.min(ui.set.cards.len().saturating_sub(1)))?;
    let selected = KeyedCatalogRef::from_material(&card.material)?;
    let comparison = compare_pitch_sets(&selected, focused)?;
    let transformation = (ui.app_settings.stage.set_graph_reading
        == woodshed_core::settings::StageGraphReading::Tonnetz)
        .then(|| {
            woodshed_core::tonnetz::neighbors(&selected)
                .into_iter()
                .find(|(neighbor, _)| neighbor == focused)
                .map(|(_, kind)| match kind {
                    "woodshed:tonnetz-parallel" => "Parallel (P)",
                    "woodshed:tonnetz-relative" => "Relative (R)",
                    _ => "Leading-tone exchange (L)",
                })
        })
        .flatten();
    let tones = |set: &std::collections::BTreeSet<woodshedding::pitch::PitchClass>| {
        set.iter()
            .map(|tone| {
                let pitch = Pitch::from_midi(60 + i32::from(tone.value()), Spelling::Sharps);
                format!("{}{}", pitch.name, pitch.accidental)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut motion_lines = Vec::new();
    match woodshed_core::harmony::compare_pitch_motion(&selected, focused) {
        Some(motion) => {
            let unit = if motion.total_semitones == 1 {
                "semitone"
            } else {
                "semitones"
            };
            motion_lines.push(Box::new(
                el(
                    "div",
                    text(format!(
                        "Pitch motion: {} {} total",
                        motion.total_semitones, unit
                    )),
                )
                .attr("class", "stage-context-compare-line"),
            ) as UiChild);
            for movement in motion.moves {
                let from =
                    Pitch::from_midi(60 + i32::from(movement.from.value()), Spelling::Sharps);
                let to = Pitch::from_midi(60 + i32::from(movement.to.value()), Spelling::Sharps);
                let unit = if movement.semitones == 1 {
                    "semitone"
                } else {
                    "semitones"
                };
                motion_lines.push(Box::new(
                    el(
                        "div",
                        text(format!(
                            "{}{} → {}{}: {} {}",
                            from.name,
                            from.accidental,
                            to.name,
                            to.accidental,
                            movement.semitones,
                            unit
                        )),
                    )
                    .attr("class", "stage-context-compare-line"),
                ) as UiChild);
            }
            motion_lines.push(Box::new(
                el(
                    "div",
                    text("Octave-free; voicing and fingering may differ."),
                )
                .attr("class", "stage-context-compare-line"),
            ) as UiChild);
        },
        None if comparison.left_only.len() != comparison.right_only.len() => {
            motion_lines.push(Box::new(
                el("div", text("Pitch motion: different tone counts"))
                    .attr("class", "stage-context-compare-line"),
            ) as UiChild);
        },
        None => {},
    }
    Some(Box::new(
        el(
            "div",
            (
                el(
                    "div",
                    text(format!(
                        "Compare {} / {}",
                        selected.label()?,
                        focused.label()?
                    )),
                )
                .attr("class", "stage-context-compare-heading"),
                transformation.map(|label| {
                    el("div", text(label)).attr("class", "stage-context-compare-line")
                }),
                el("div", motion_lines),
                el(
                    "div",
                    text(format!("Shared tones: {}", tones(&comparison.shared))),
                )
                .attr("class", "stage-context-compare-line"),
                el(
                    "div",
                    text(format!("Selected only: {}", tones(&comparison.left_only))),
                )
                .attr("class", "stage-context-compare-line"),
                el(
                    "div",
                    text(format!("Focused only: {}", tones(&comparison.right_only))),
                )
                .attr("class", "stage-context-compare-line"),
            ),
        )
        .attr("class", "stage-context-comparison"),
    ) as UiChild)
}

fn nearby_candidates(ui: &UiState, focused: &KeyedCatalogRef) -> UiChild {
    let authored = ui
        .set
        .cards
        .iter()
        .filter_map(|card| KeyedCatalogRef::from_material(&card.material))
        .collect::<std::collections::BTreeSet<_>>();
    let mut candidates = woodshed_core::stage_candidates::ranked_candidates(
        ui.app_settings.stage.set_graph_reading,
        focused,
        &authored,
    );
    let displayed = super::set_graph_snapshot(ui)
        .nodes
        .iter()
        .filter(|node| !node.foreground)
        .filter_map(|node| node.keyed.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let unseen = candidates
        .iter()
        .filter(|candidate| !displayed.contains(&candidate.keyed))
        .count();
    let at_capacity = displayed.len() >= ui.app_settings.stage.context.node_limit();
    candidates.sort_by_key(|candidate| displayed.contains(&candidate.keyed));
    let rows = candidates
        .into_iter()
        .take(6)
        .map(|candidate| {
            let label = candidate
                .keyed
                .label()
                .unwrap_or_else(|| candidate.keyed.wire_key());
            let label = if displayed.contains(&candidate.keyed) {
                format!("Shown · {label}")
            } else {
                label
            };
            let reason = match candidate.reason {
                woodshed_core::stage_candidates::StageCandidateReason::CircleOfFifths {
                    distance,
                } => format!("Fifth distance: {distance}"),
                woodshed_core::stage_candidates::StageCandidateReason::Tonnetz { depth } => {
                    format!("P/L/R depth: {depth}")
                },
                woodshed_core::stage_candidates::StageCandidateReason::Unavailable => {
                    "Reading metric: unavailable".to_string()
                },
                woodshed_core::stage_candidates::StageCandidateReason::PitchMotion { .. } => {
                    "Ranked from browsing focus".to_string()
                },
            };
            let motion = candidate
                .pitch_motion
                .as_ref()
                .map(|motion| format!("Pitch motion: {} semitones", motion.total_semitones))
                .unwrap_or_else(|| "Pitch motion: unavailable".to_string());
            let tone_detail = woodshed_core::harmony::compare_pitch_sets(focused, &candidate.keyed)
                .map(|comparison| {
                    if let Some(motion) = &candidate.pitch_motion {
                        format!(
                            "Held: {} · Moves: {}",
                            pitch_class_names(&comparison.shared),
                            if motion.moves.is_empty() {
                                "none".to_string()
                            } else {
                                motion
                                    .moves
                                    .iter()
                                    .map(|movement| {
                                        format!(
                                            "{} → {}",
                                            pitch_class_names(&std::collections::BTreeSet::from([
                                                movement.from
                                            ])),
                                            pitch_class_names(&std::collections::BTreeSet::from([
                                                movement.to
                                            ]))
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            },
                        )
                    } else {
                        format!(
                            "Held: {} · Removed: {} · Added: {}",
                            pitch_class_names(&comparison.shared),
                            pitch_class_names(&comparison.left_only),
                            pitch_class_names(&comparison.right_only),
                        )
                    }
                })
                .unwrap_or_else(|| "Tone comparison: unavailable".to_string());
            let target = candidate.keyed.clone();
            Box::new(clickable(
                el(
                    "div",
                    (
                        el("div", text(label)).attr("class", "stage-context-candidate-label"),
                        el("div", text(format!("{reason} · {motion}")))
                            .attr("class", "stage-context-candidate-meta"),
                        el("div", text(tone_detail)).attr("class", "stage-context-candidate-meta"),
                    ),
                )
                .attr("class", "stage-context-candidate"),
                move |ui: &mut UiState, _| {
                    let displayed = super::set_graph_snapshot(ui)
                        .nodes
                        .into_iter()
                        .filter(|node| !node.foreground)
                        .filter_map(|node| node.keyed);
                    ui.context_disclosed.extend(displayed);
                    ui.focus_context_catalog(target.clone());
                },
            )) as UiChild
        })
        .collect::<Vec<_>>();
    Box::new(
        el(
            "div",
            (
                el("div", text("Nearby candidates")).attr("class", "stage-context-compare-heading"),
                el(
                    "div",
                    text(format!(
                        "From {} · unshown first",
                        focused.label().unwrap_or_else(|| focused.wire_key())
                    )),
                )
                .attr("class", "stage-context-label"),
                el("div", rows).attr("class", "stage-context-candidate-list"),
                (unseen == 0).then(|| el("div", text("All candidate triads are shown."))),
                (unseen > 0).then(|| {
                    clickable(
                        el(
                            "div",
                            text(if at_capacity {
                                "Replace quiet context"
                            } else {
                                "Show nearby"
                            }),
                        )
                        .attr("class", "t-btn stage-context-nearby-show"),
                        |ui: &mut UiState, _| ui.show_nearby_context(),
                    )
                }),
            ),
        )
        .attr("class", "stage-context-candidates"),
    )
}

fn select_keyed_material(stage: &mut woodshed_core::StageState, keyed: &KeyedCatalogRef) -> bool {
    let Some(material) = keyed.to_material() else {
        return false;
    };
    stage.set_root((keyed.root.value() as usize + 3) % 12);
    if let Some(name) = keyed.formula_id.strip_prefix("arpeggio:") {
        let Some(index) = stage.chords().iter().position(|item| item.name == name) else {
            return false;
        };
        stage.set_lens(Lens::Arpeggios);
        stage.select_arpeggio(index);
        return true;
    }
    match material {
        Material::Chord { name, .. } | Material::ChordApproach { name, .. } => {
            let Some(index) = stage.chords().iter().position(|item| item.name == name) else {
                return false;
            };
            stage.set_lens(Lens::Chords);
            stage.select_chord(index);
            true
        },
        Material::Scale { name, .. } | Material::ScalePattern { name, .. } => {
            let Some(index) = stage.scales().iter().position(|item| item.name == name) else {
                return false;
            };
            stage.set_lens(Lens::Scales);
            stage.select_scale(index);
            true
        },
        Material::Riff { .. } | Material::Path { .. } => false,
    }
}

/// Compact action panel shared by the wider graph and any fallback context
/// presentation. It intentionally exposes the three distinct user actions.
fn more_button(ui: &UiState) -> UiChild {
    if !ui.app_settings.stage.context.enabled {
        return Box::new(el("div", ()));
    }
    if ui.app_settings.stage.set_graph_reading
        == woodshed_core::settings::StageGraphReading::Tonnetz
    {
        let shown = super::set_graph_snapshot(ui)
            .nodes
            .iter()
            .filter_map(|node| node.keyed.as_ref())
            .filter(|keyed| woodshed_core::tonnetz::position(keyed).is_some())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if shown >= 24 {
            return Box::new(el("div", text("All 24 major/minor triads shown.")));
        }
    }
    let initial = ui.context_focus.is_none() && ui.context_disclosed.is_empty();
    if !initial && ui.app_settings.stage.context.node_limit() >= 36 {
        if ui.context_focus.is_some() {
            return Box::new(el("div", text("Context limit reached.")));
        }
        return Box::new(el(
            "div",
            (
                el("div", text("Context limit reached.")),
                clickable(
                    el("div", text("Explore from here")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.context_disclosed.clear();
                    },
                ),
            ),
        ));
    }
    Box::new(clickable(
        el("div", text("Show more")).attr("class", "t-btn stage-context-more"),
        |ui: &mut UiState, _| ui.show_more_context(),
    ))
}

pub(super) fn panel(ui: &UiState) -> UiChild {
    let reading = ui.app_settings.stage.set_graph_reading;
    if !matches!(
        reading,
        woodshed_core::settings::StageGraphReading::CircleOfFifths
            | woodshed_core::settings::StageGraphReading::Tonnetz
            | woodshed_core::settings::StageGraphReading::PitchMotion
    ) {
        return Box::new(el("div", ()));
    }
    let (title, guidance) = match reading {
        woodshed_core::settings::StageGraphReading::PitchMotion => (
            "Pitch motion",
            "Browse exact pitch-class movement. The anchor stays fixed until you recenter.",
        ),
        woodshed_core::settings::StageGraphReading::Tonnetz => (
            "Tonnetz",
            "Triangles show major and minor triads. Shared edges mark P, L, and R transformations; the lattice wraps at its boundaries.",
        ),
        _ => (
            "Circle of fifths",
            "Major chords mark the circle. Relative minor chords sit inside it; scales appear alongside their key.",
        ),
    };
    let Some(material) = ui.context_focus.as_ref() else {
        return Box::new(el("div", (
            super::pitch_motion::controls(ui),
            el("div", text(title)).attr("class", "stage-context-title"),
            el("div", text(guidance)),
            el("div", text("Select a quiet node to reveal its connections and compare it with the selected Set card.")),
            ui.current_card().and_then(|card| KeyedCatalogRef::from_material(&card.material))
                .map(|focus| nearby_candidates(ui, &focus)),
            more_button(ui),
        )).attr("class", "stage-context-panel"));
    };
    Box::new(
        el(
            "div",
            (
                el("div", text("Focused context")).attr("class", "stage-context-label"),
                super::pitch_motion::controls(ui),
                el("div", text(context_title(material))).attr("class", "stage-context-title"),
                comparison(ui, material),
                super::scale_patterns::context_detail(ui, material),
                super::chord_approaches::context_detail(ui, material),
                el(
                    "div",
                    (
                        clickable(
                            el("div", text("Hear")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.audition_context_focus(),
                        ),
                        clickable(
                            el("div", text("Add to Set")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.add_context_focus_to_set(),
                        ),
                        (material.formula_id.starts_with("chord:")
                            || material.formula_id.starts_with("scale:"))
                        .then(|| {
                            clickable(
                                el("button", text("Keep nearby"))
                                    .attr("class", "t-btn stage-context-keep"),
                                |ui: &mut UiState, _| {
                                    ui.keep_context_focus_nearby();
                                },
                            )
                        }),
                        clickable(
                            el("div", text("Clear focus")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.clear_context_focus(),
                        ),
                    ),
                )
                .attr("class", "stage-context-actions"),
                nearby_candidates(ui, material),
                more_button(ui),
            ),
        )
        .attr("class", "stage-context-panel"),
    )
}

#[path = "context/operations.rs"]
mod operations;

#[path = "context/labeling.rs"]
mod labeling;

use labeling::{context_title, pitch_class_names};

pub(super) use labeling::labels;
