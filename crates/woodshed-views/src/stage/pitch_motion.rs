//! Captured layout anchor, independent of browsing focus and Set authoring.

use super::{UiChild, UiState};
use cambium::{clickable, el, text};
use woodshed_core::{harmony::KeyedCatalogRef, settings::StageGraphReading};

impl UiState {
    pub(super) fn pitch_motion_anchor_choice(&self) -> Option<KeyedCatalogRef> {
        self.context_focus
            .clone()
            .or_else(|| {
                self.current_card()
                    .and_then(|card| KeyedCatalogRef::from_material(&card.material))
            })
            .or_else(|| {
                self.stage
                    .card_from_lens()
                    .and_then(|card| KeyedCatalogRef::from_material(&card.material))
            })
    }

    pub fn recenter_pitch_motion(&mut self) {
        if self.app_settings.stage.set_graph_reading != StageGraphReading::PitchMotion {
            return;
        }
        self.pitch_motion_anchor = self.pitch_motion_anchor_choice();
        self.set_graph_positions.clear();
        self.context_positions.clear();
        self.set_graph_viewport = Default::default();
        self.set_graph_relation = None;
    }
}

pub(super) fn controls(ui: &UiState) -> UiChild {
    if ui.app_settings.stage.set_graph_reading != StageGraphReading::PitchMotion {
        return Box::new(el("div", ()));
    }
    let anchor = ui
        .pitch_motion_anchor
        .as_ref()
        .map(|key| key.label().unwrap_or_else(|| key.wire_key()))
        .unwrap_or_else(|| "unavailable".into());
    Box::new(el("div", (
        el("div", text(format!("Pitch motion from {anchor}"))).attr("class", "pitch-motion-anchor"),
        el("div", text("The inner ring is zero. Each outer ring adds summed semitones from this anchor, not every pair. Unscored material is on the right. Pan and zoom preserve the reading.")),
        clickable(el("div", text("Recenter on focus / selected Card")).attr("class", "t-btn pitch-motion-recenter"),
            |ui: &mut UiState, _| ui.recenter_pitch_motion()),
    )).attr("class", "pitch-motion-controls"))
}

pub(super) fn guide_labels(
    ui: &UiState,
    swatch: &cambium::GraphCanvasSwatch<super::StageInstanceRef, &'static str>,
) -> Vec<UiChild> {
    if ui.app_settings.stage.set_graph_reading != StageGraphReading::PitchMotion {
        return Vec::new();
    }
    let snapshot = super::set_graph_snapshot(ui);
    let mut entries = vec![((590.0, -770.0), "Unscored".to_string())];
    if let Some(anchor) = &ui.pitch_motion_anchor {
        for (cost, radius) in woodshed_core::pitch_motion_reading::guides(anchor) {
            entries.push((
                (woodshed_core::pitch_motion_reading::CENTER.0, -radius),
                format!("{cost} st"),
            ));
        }
    }
    entries
        .into_iter()
        .map(|((x, y), label)| {
            let normalized = super::scene_position(&snapshot, x, y);
            let (x, y) = swatch.viewport.project(
                normalized,
                sprigging::Size {
                    width: swatch.width as f32,
                    height: swatch.height as f32,
                },
                swatch.node_radius + swatch.edge_width,
            );
            Box::new(
                el("span", text(label))
                    .attr("class", "stage-context-node-label")
                    .attr("aria-hidden", "true")
                    .attr(
                        "style",
                        format!("left:{x}px;top:{}px;width:70px;", y - 12.0),
                    ),
            ) as UiChild
        })
        .collect()
}
