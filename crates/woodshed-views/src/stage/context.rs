//! View-only controls for the wider Stage context graph.
//!
//! The graph projection itself lives beside the Stage scene adapter. This
//! module owns the interaction boundary: background focus is a selection aid,
//! while hearing and adding to the Set are explicit actions.

use cambium::{clickable, el, text};
use woodshed_core::Lens;
use woodshed_core::audio::AudioRequest;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::harmony::compare_pitch_sets;
use woodshedding::pitch::{Pitch, Spelling};
use woodshedding::rehearsal::Material;

use super::{UiChild, UiState};

impl UiState {
    /// Focus a catalog material from the context graph. Selection updates the
    /// board so the surrounding graph can be recomputed, but this never adds a
    /// card to the Set.
    pub fn focus_context_catalog(&mut self, material: KeyedCatalogRef) -> bool {
        if !select_keyed_material(&mut self.stage, &material) {
            return false;
        }
        self.root_dd.selected = self.stage.root_idx;
        self.context_disclosed.insert(material.clone());
        self.context_focus = Some(material);
        self.related_hover = None;
        self.related_relation = None;
        true
    }

    /// Hear the currently focused context material. Audition is intentionally
    /// separate from both focus and Add to Set.
    pub fn audition_context_focus(&mut self) {
        if let Some(material) = self.context_focus.clone() {
            if !select_keyed_material(&mut self.stage, &material) {
                return;
            }
            self.root_dd.selected = self.stage.root_idx;
            let (pitches, duration_s, strum_s) = self.stage.voicing_preview();
            if !pitches.is_empty() {
                self.request(AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s,
                });
            }
        }
    }

    /// Add the focused context material to the Set, preserving the existing
    /// provenance path used by the Related panel's explicit Stage action.
    pub fn add_context_focus_to_set(&mut self) {
        let Some(material) = self.context_focus.clone() else {
            return;
        };
        let from_id = self.stage.catalog_id();
        if self.focus_context_catalog(material) {
            self.stage_current(from_id);
        }
    }

    /// Clear only the context focus. The current board selection and Set stay
    /// intact, which makes this safe for a small dismiss affordance.
    pub fn clear_context_focus(&mut self) {
        self.context_focus = None;
    }

    pub fn show_more_context(&mut self) {
        let snapshot = super::set_graph_snapshot(self);
        let shown = snapshot
            .nodes
            .iter()
            .filter(|node| !node.foreground)
            .count();
        self.context_disclosed.extend(
            snapshot
                .nodes
                .iter()
                .filter_map(|node| (!node.foreground).then(|| node.keyed.clone()).flatten()),
        );
        self.app_settings.stage.context.node_limit = self
            .app_settings
            .stage
            .context
            .node_limit()
            .max(shown + 8)
            .min(36);
    }
}

fn context_title(material: &KeyedCatalogRef) -> String {
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
pub(super) fn labels(
    ui: &UiState,
    swatch: &cambium::GraphCanvasSwatch<super::StageInstanceRef, &'static str>,
) -> UiChild {
    if ui.app_settings.stage.set_graph_reading
        != woodshed_core::settings::StageGraphReading::CircleOfFifths
        || ui.set_graph_drag_active
    {
        return Box::new(el("div", ()));
    }
    let labels: Vec<UiChild> = swatch
        .graph
        .nodes
        .iter()
        .zip(swatch.projected_positions())
        .filter(|(node, _)| swatch.projected_node_footprint(&node.id).is_none())
        .map(|(node, (_, (x, y)))| {
            let label = node
                .label
                .replace("Major", "maj")
                .replace("Minor", "min")
                .replace("Dominant", "dom");
            let width = label.chars().count() as f32 * 5.8 + 4.0;
            let left = if x > swatch.width as f32 * 0.65 {
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
                            (y - 6.0).max(0.0)
                        ),
                    ),
            ) as UiChild
        })
        .collect();
    Box::new(el("div", labels).attr("class", "stage-context-labels"))
}

fn comparison(ui: &UiState, focused: &KeyedCatalogRef) -> Option<UiChild> {
    let card = ui
        .set
        .cards
        .get(ui.set.cursor.min(ui.set.cards.len().saturating_sub(1)))?;
    let selected = KeyedCatalogRef::from_material(&card.material)?;
    let comparison = compare_pitch_sets(&selected, focused)?;
    let tones = |set: &std::collections::BTreeSet<woodshedding::pitch::PitchClass>| {
        set.iter()
            .map(|tone| {
                let pitch = Pitch::from_midi(60 + i32::from(tone.value()), Spelling::Sharps);
                format!("{}{}", pitch.name, pitch.accidental)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
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

fn select_keyed_material(stage: &mut woodshed_core::StageState, keyed: &KeyedCatalogRef) -> bool {
    let Some(material) = keyed.to_material() else {
        return false;
    };
    stage.set_root((keyed.root.value() as usize + 3) % 12);
    match material {
        Material::Chord { name, .. } => {
            let Some(index) = stage.chords().iter().position(|item| item.name == name) else {
                return false;
            };
            stage.set_lens(Lens::Chords);
            stage.select_chord(index);
            true
        },
        Material::Scale { name, .. } => {
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
    let initial = ui.context_focus.is_none() && ui.context_disclosed.is_empty();
    if !initial && ui.app_settings.stage.context.node_limit() >= 36 {
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
    if ui.app_settings.stage.set_graph_reading
        != woodshed_core::settings::StageGraphReading::CircleOfFifths
    {
        return Box::new(el("div", ()));
    }
    let Some(material) = ui.context_focus.as_ref() else {
        return Box::new(el("div", (
            el("div", text("Circle of fifths")).attr("class", "stage-context-title"),
            el("div", text("Major chords mark the circle. Relative minor chords sit inside it; scales appear alongside their key.")),
            el("div", text("Select a quiet node to reveal its connections and compare it with the selected Set card.")),
            more_button(ui),
        )).attr("class", "stage-context-panel"));
    };
    Box::new(
        el(
            "div",
            (
                el("div", text("Focused context")).attr("class", "stage-context-label"),
                el("div", text(context_title(material))).attr("class", "stage-context-title"),
                comparison(ui, material),
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
                        clickable(
                            el("div", text("Clear focus")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.clear_context_focus(),
                        ),
                    ),
                )
                .attr("class", "stage-context-actions"),
                more_button(ui),
            ),
        )
        .attr("class", "stage-context-panel"),
    )
}
