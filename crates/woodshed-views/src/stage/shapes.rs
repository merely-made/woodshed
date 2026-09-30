//! Explicit selected chord shapes, shared by Stage's editor and Rehearsal.

use cambium::{clickable, el, text};
use woodshed_core::CardShapeStatus;
use woodshedding::rehearsal::Material;

use super::{UiChild, UiState};
use crate::fretboard_leaf::{BoardGeom, Orientation};

impl UiState {
    pub fn step_card_shape(&mut self, direction: i8) {
        let Some(cursor) = self.selected_card_index() else {
            return;
        };
        let card = &mut self.set.cards[cursor];
        let result = if direction < 0 {
            self.stage.select_previous_card_shape(card)
        } else {
            self.stage.select_next_card_shape(card)
        };
        self.card_shape_notice = result.err().map(|reason| (card.id, reason.to_string()));
        self.hover_peek = None;
    }

    pub fn clear_selected_card_shape(&mut self) {
        let Some(cursor) = self.selected_card_index() else {
            return;
        };
        let card = &mut self.set.cards[cursor];
        self.stage.clear_card_shape(card);
        self.card_shape_notice = None;
        self.hover_peek = None;
    }

    /// The scale setup and written/concert key are distinct from a fingering.
    pub fn rehearsal_scale_status(&self) -> Option<Result<String, String>> {
        let card = self.current_card()?;
        if !matches!(card.material, Material::Scale { .. }) {
            return None;
        }
        Some(self.stage.scale_card_realization(card).map(|scale| {
            let written = woodshed_core::harmony::KeyedCatalogRef::from_material(&card.material)
                .expect("resolved scale formula");
            let mut concert = written.clone();
            concert.root = scale.concert_root;
            format!("{} / {} · capo {} · written {} · concert {} · {} strings · physical frets {}–{}",
                scale.setup.instrument, scale.setup.tuning_name, scale.setup.capo,
                written.label().expect("resolved scale"), concert.label().expect("resolved scale"),
                scale.geometry.string_count, scale.geometry.physical_fret_start, scale.geometry.physical_fret_end)
        }).map_err(|reason| format!("Scale unavailable: {reason}")))
    }

    /// Native neck paint and retained note labels must use the same Card setup.
    pub fn rehearsal_board_geometry(&self) -> BoardGeom {
        let resolved = self.current_card().and_then(|card| {
            if matches!(card.material, Material::Scale { .. }) {
                Some(
                    self.stage
                        .scale_card_realization(card)
                        .map(|scale| scale.geometry)
                        .unwrap_or(woodshed_core::card_shapes::CardShapeGeometry {
                            string_count: 0,
                            physical_fret_start: 0,
                            physical_fret_end: 0,
                            capo: 0,
                        }),
                )
            } else {
                self.stage.card_shape_geometry(card)
            }
        });
        BoardGeom {
            string_count: resolved
                .as_ref()
                .map_or(self.stage.string_count(), |g| g.string_count),
            fret_start: resolved
                .as_ref()
                .map_or(self.stage.fret_start, |g| g.physical_fret_start),
            fret_count: resolved
                .as_ref()
                .map_or(self.stage.fret_count, |g| g.physical_fret_end),
            orientation: Orientation::from_name(&self.app_settings.fretboard.orientation),
        }
    }
}

pub(super) fn controls(ui: &UiState) -> UiChild {
    let Some(card) = ui.current_card() else {
        return Box::new(el("div", ()));
    };
    if !matches!(card.material, Material::Chord { .. }) {
        return Box::new(el("div", ()));
    }
    let status = match ui.stage.card_shape_status(card) {
        CardShapeStatus::NotChord | CardShapeStatus::Unselected => "All chord tones".to_string(),
        CardShapeStatus::Available(shape) => format!(
            "Shape {} of {}{} · {} / {} · capo {}",
            shape.index + 1,
            shape.count,
            if shape.limited_inventory {
                " · bounded selection"
            } else {
                ""
            },
            shape.instrument,
            shape.tuning_name,
            shape.geometry.capo,
        ),
        CardShapeStatus::Unavailable(reason) => format!("Shape unavailable: {reason}"),
    };
    let notice = ui
        .card_shape_notice
        .as_ref()
        .filter(|(id, _)| *id == card.id)
        .map(|(_, message)| message.clone());
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text("Previous shape")).attr("class", "t-btn card-shape-prev"),
                    |ui: &mut UiState, _| ui.step_card_shape(-1),
                ),
                el("div", text(status)).attr("class", "t-readout card-shape-status"),
                clickable(
                    el("div", text("Next shape")).attr("class", "t-btn card-shape-next"),
                    |ui: &mut UiState, _| ui.step_card_shape(1),
                ),
                clickable(
                    el("div", text("All tones")).attr("class", "t-btn card-shape-clear"),
                    |ui: &mut UiState, _| ui.clear_selected_card_shape(),
                ),
                notice.map(|message| el("div", text(message)).attr("class", "card-shape-notice")),
                movement_details(ui),
            ),
        )
        .attr("class", "card-shape-controls"),
    )
}

fn movement_details(ui: &UiState) -> UiChild {
    use woodshed_core::shape_movement::{
        ShapeContact, ShapeMovementUnavailable as Unavailable, ShapeStringMovement,
    };
    let Some(index) = ui.selected_card_index() else {
        return Box::new(el("div", ()));
    };
    let Some(previous) = index
        .checked_sub(1)
        .and_then(|index| ui.set.cards.get(index))
    else {
        return Box::new(
            el(
                "div",
                text("Neck movement: select a later Card to compare with its predecessor."),
            )
            .attr("class", "shape-movement"),
        );
    };
    let selected = &ui.set.cards[index];
    let heading = format!("Neck movement · {} → {}", previous.label, selected.label);
    let detail = match ui.stage.compare_cards(previous, selected) {
        Err(reason) => {
            let message = match reason {
                Unavailable::LeftNotChord => "The previous Card is not a chord.".into(),
                Unavailable::RightNotChord => "The selected Card is not a chord.".into(),
                Unavailable::LeftUnselected => "Choose a shape on the previous Card.".into(),
                Unavailable::RightUnselected => "Choose a shape on the selected Card.".into(),
                Unavailable::LeftUnavailable(reason) => {
                    format!("Previous shape unavailable: {reason}")
                },
                Unavailable::RightUnavailable(reason) => {
                    format!("Selected shape unavailable: {reason}")
                },
                Unavailable::SetupMismatch { .. } => {
                    "Use the same instrument, tuning and capo on both Cards.".into()
                },
            };
            Box::new(el("div", text(message)).attr("class", "shape-movement-unavailable"))
                as UiChild
        },
        Ok(movement) => {
            let contact = |contact: &ShapeContact| match contact {
                ShapeContact::Open { .. } => "open".to_string(),
                ShapeContact::Fretted { fret, .. } => format!("fret {fret}"),
            };
            let rows = movement
                .per_string
                .iter()
                .map(|change| {
                    let value = match change {
                        ShapeStringMovement::Held { contact: point } => {
                            format!("held {}", contact(point))
                        },
                        ShapeStringMovement::Moved {
                            from_fret,
                            to_fret,
                            fret_travel,
                            ..
                        } => format!("fret {from_fret} → {to_fret} · {fret_travel} frets"),
                        ShapeStringMovement::Added { contact: point } => {
                            format!("added {}", contact(point))
                        },
                        ShapeStringMovement::Dropped { contact: point } => {
                            format!("dropped {}", contact(point))
                        },
                    };
                    el(
                        "div",
                        text(format!("String {}: {value}", change.string_index() + 1)),
                    )
                })
                .collect::<Vec<_>>();
            let span = |value: Option<u8>| {
                value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "no fretted contacts".into())
            };
            let shift = movement
                .position_shift
                .map(|value| format!("{value:+} frets"))
                .unwrap_or_else(|| "unavailable".into());
            Box::new(el("div", (
                el("div", text(format!("Matched fret travel: {} · Fretted span: {} → {} · Lowest fretted position shift: {shift}", movement.total_matched_fret_travel, span(movement.left_fretted_span), span(movement.right_fretted_span)))).attr("class", "shape-movement-summary"),
                el("div", rows).attr("class", "shape-movement-strings"),
                el("div", text("String numbers follow tuning order. Open, added and dropped strings carry no travel penalty. Solo and Mute do not change these contacts; fingerings are not assigned.")),
            ))) as UiChild
        },
    };
    Box::new(el("div", (el("div", text(heading)), detail)).attr("class", "shape-movement"))
}

#[cfg(test)]
mod scale_tests {
    use super::*;
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::FretWindow;

    fn scale_ui() -> UiState {
        let mut ui = UiState::new();
        let mut card = KeyedCatalogRef {
            formula_id: "scale:Major".into(),
            root: PitchClass::new(0),
        }
        .to_card()
        .unwrap();
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Standard (high-G)".into());
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 2, span: 4 });
        ui.set.push(card);
        ui
    }

    #[test]
    fn scale_geometry_uses_stored_four_string_setup_and_physical_capo_window() {
        let ui = scale_ui();
        assert_eq!(ui.stage.string_count(), 6);
        let geom = ui.rehearsal_board_geometry();
        assert_eq!(geom.string_count, 4);
        assert_eq!(geom.fret_start, 2);
        assert_eq!(geom.fret_count, 6);
        let card = &ui.set.cards[0];
        let dots = ui.stage.dots_for_card(card);
        assert!(!dots.is_empty());
        assert!(
            dots.iter()
                .all(|dot| dot.string_index < 4 && geom.in_window(dot.fret))
        );
        let status = ui.rehearsal_scale_status().unwrap().unwrap();
        assert!(status.contains("Ukulele / Standard (high-G)"));
        assert!(status.contains("written C Major"));
        assert!(status.contains("concert D Major"));
        assert!(status.contains("physical frets 2–6"));
        assert!(
            ui.stage
                .card_sounding_pitches_at_tempo(card, 90.0)
                .0
                .iter()
                .all(|pitch| *pitch > 250.0)
        );
    }

    #[test]
    fn invalid_scale_setup_has_no_live_guitar_geometry_notes_or_working_status() {
        let mut ui = scale_ui();
        ui.set.cards[0].setting.tuning = Some("Missing saved tuning".into());
        assert!(
            ui.rehearsal_scale_status()
                .unwrap()
                .unwrap_err()
                .contains("Missing saved tuning")
        );
        assert_eq!(ui.rehearsal_board_geometry().string_count, 0);
        assert!(ui.stage.dots_for_card(&ui.set.cards[0]).is_empty());
        assert!(
            ui.stage
                .card_sounding_pitches_at_tempo(&ui.set.cards[0], 90.0)
                .0
                .is_empty()
        );
    }
}
