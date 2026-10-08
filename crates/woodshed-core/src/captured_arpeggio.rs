//! Copied musical recipe instructions. Source occurrence is historical metadata;
//! playback and addition resolve only the saved instruction, never a live Card.
use crate::{CardShapeStatus, StageState, harmony::KeyedCatalogRef};
use serde::{Deserialize, Serialize};
use woodshedding::rehearsal::{Card, CardId, FretWindow, Hold, Material, Touch};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedRecipe {
    pub source_card: CardId,
    pub instruction: serde_json::Value,
}

/// Compatibility name for the original captured-arpeggio API and wire payload.
pub type CapturedArpeggio = CapturedRecipe;

fn supported(card: &Card) -> bool {
    matches!(card.material, Material::ScalePattern { .. })
        || (matches!(card.material, Material::Chord { .. })
            && matches!(card.touch, Touch::Arpeggiate { .. }))
}

impl CapturedRecipe {
    pub fn capture(source: &Card, stage: &StageState, bpm: f32) -> Result<Self, String> {
        if source.id == CardId::UNASSIGNED {
            return Err("Keep a recipe from an authored Card occurrence.".into());
        }
        let mut card = source.clone();
        if !supported(&card) {
            return Err("Select an authored chord arpeggio or scale-pattern Card.".into());
        }
        let tuning = stage
            .resolve_card_tuning(&card)
            .map_err(|error| error.to_string())?;
        card.setting.instrument = tuning.instrument.to_string();
        card.setting.tuning = Some(tuning.name.clone());
        card.setting.capo = Some(card.setting.capo.unwrap_or(0));
        card.setting.fret_window = Some(card.setting.fret_window.unwrap_or(FretWindow {
            start: stage.fret_start,
            span: stage.fret_count.saturating_sub(stage.fret_start),
        }));
        card.timing.bpm = Some(card.timing.bpm.unwrap_or(bpm));
        card.id = CardId::UNASSIGNED;
        let recipe = Self {
            source_card: source.id,
            instruction: serde_json::to_value(card).map_err(|error| error.to_string())?,
        };
        recipe.preview()?;
        Ok(recipe)
    }

    /// Validate the complete explicit instruction without enumerating shapes.
    /// Render availability can use this; owner actions must additionally preview.
    pub fn card(&self) -> Result<Card, String> {
        if self.source_card == CardId::UNASSIGNED {
            return Err("The captured recipe has no source occurrence identity.".into());
        }
        if self.instruction.to_string().len() > 64 * 1024 {
            return Err("Captured recipe exceeds the instruction size limit.".into());
        }
        let card: Card = serde_json::from_value(self.instruction.clone())
            .map_err(|_| "Captured recipe instructions are malformed.".to_string())?;
        if card.id != CardId::UNASSIGNED
            || !supported(&card)
            || KeyedCatalogRef::from_card(&card).is_none()
        {
            return Err("Captured recipe is not an available unassigned musical recipe.".into());
        }
        // Shape inventory identities are chord-only. Scale traversal cannot
        // honor these selectors, so keep malformed/stale copies unavailable.
        if matches!(card.material, Material::ScalePattern { .. })
            && (card.setting.voicing_idx.is_some()
                || card.setting.voicing_fingerprint.is_some()
                || card.setting.voicing_profile.is_some())
        {
            return Err("Captured scale-pattern shape metadata is unavailable.".into());
        }
        let bpm = card.timing.bpm.ok_or("Captured recipe tempo is missing.")?;
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err("Captured recipe tempo is invalid.".into());
        }
        if let Hold::Seconds(seconds) = card.timing.hold {
            if !seconds.is_finite() || seconds <= 0.0 {
                return Err("Captured recipe duration is invalid.".into());
            }
        }
        if card.setting.instrument.is_empty()
            || card.setting.tuning.is_none()
            || card.setting.capo.is_none()
            || card.setting.fret_window.is_none()
        {
            return Err("Captured recipe setup is incomplete.".into());
        }
        StageState::new()
            .resolve_card_tuning(&card)
            .map_err(|error| error.to_string())?;
        Ok(card)
    }

    pub fn preview(&self) -> Result<(Vec<f32>, f32, f32), String> {
        let card = self.card()?;
        let stage = StageState::new();
        let end = stage
            .card_fret_limit(&card)
            .map_err(|error| error.to_string())?;
        let window = card
            .setting
            .fret_window
            .as_ref()
            .expect("card validates explicit window");
        let capo = card.setting.capo.unwrap_or(0);
        if window.start > end || window.start.saturating_add(window.span).min(end) < capo {
            return Err("Captured recipe fret window is unavailable.".into());
        }
        if card.setting.voicing_idx.is_some() {
            match stage.card_shape_status(&card) {
                CardShapeStatus::Available(_) => {},
                CardShapeStatus::Unavailable(error) => return Err(error.to_string()),
                _ => return Err("Captured recipe shape is unavailable.".into()),
            }
        }
        if matches!(card.material, Material::ScalePattern { .. }) {
            let scale = stage
                .scale_card_realization(&card)
                .map_err(|error| error.to_string())?;
            if card.setting.marked.iter().any(|position| {
                !scale
                    .dots
                    .iter()
                    .any(|dot| (dot.string_index, dot.fret) == *position)
            }) {
                return Err("Captured scale-pattern marked contact is unavailable.".into());
            }
        }
        let preview = stage.card_sounding_pitches(&card);
        if preview.0.is_empty()
            || preview
                .0
                .iter()
                .any(|pitch| !pitch.is_finite() || *pitch <= 0.0)
            || !preview.1.is_finite()
            || preview.1 <= 0.0
            || !preview.2.is_finite()
            || preview.2 < 0.0
        {
            return Err("Captured recipe has no available sounding notes.".into());
        }
        Ok(preview)
    }

    pub fn detail(&self) -> String {
        match self.card() {
            Ok(card) => {
                let articulation = match (&card.material, &card.touch) {
                    (Material::ScalePattern { pattern, .. }, touch) => format!(
                        "{} scale pattern, ordered contacts, {:?} touch",
                        pattern.label(),
                        touch
                    ),
                    (
                        _,
                        Touch::Arpeggiate {
                            direction,
                            inversion,
                        },
                    ) => format!("{} arpeggio, inversion {}", direction.label(), inversion),
                    _ => unreachable!("card validates supported recipe"),
                };
                let window = card.setting.fret_window.expect("validated window");
                format!(
                    "Copied from Card {}. Saved {} / {}, capo {}, frets {}–{}; {}; {} BPM, {:?}. {}. Source edits or removal leave this copy unchanged.",
                    self.source_card.0,
                    card.setting.instrument,
                    card.setting.tuning.unwrap_or_default(),
                    card.setting.capo.unwrap_or(0),
                    window.start,
                    window.start.saturating_add(window.span),
                    articulation,
                    card.timing.bpm.unwrap_or(120.0),
                    card.timing.hold,
                    match card.setting.voicing_idx {
                        Some(index) => format!(
                            "Saved shape {}, {} marked notes ({})",
                            index.saturating_add(1),
                            card.setting.marked.len(),
                            card.setting.mark_mode.label()
                        ),
                        None => format!(
                            "{}, {} marked notes ({})",
                            if matches!(card.material, Material::ScalePattern { .. }) {
                                "Deterministic scale contacts"
                            } else {
                                "Formula preview"
                            },
                            card.setting.marked.len(),
                            card.setting.mark_mode.label()
                        ),
                    }
                )
            },
            Err(error) => format!(
                "Copied from Card {}. Recipe unavailable: {}. Retained instructions are preserved.",
                self.source_card.0, error
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{ArpeggioDirection, MarkMode, Setting, Timing},
    };
    fn source() -> Card {
        let mut card = KeyedCatalogRef {
            formula_id: "arpeggio:Major".into(),
            root: PitchClass::new(0),
        }
        .to_card()
        .unwrap();
        card.id = CardId(7);
        card.setting = Setting {
            instrument: String::new(),
            ..Default::default()
        };
        card.timing = Timing {
            bpm: None,
            hold: Hold::Bars(2),
        };
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::Down,
            inversion: 1,
        };
        card
    }
    #[test]
    fn selected_shape_fingerprint_marks_and_pinned_window_replay_exactly() {
        let stage = StageState::new();
        let mut card = source();
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::Down,
            inversion: 0,
        };
        card.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        stage.select_next_card_shape(&mut card).unwrap();
        assert!(card.setting.voicing_fingerprint.is_some());
        let source_setup = serde_json::to_value(&card.setting).unwrap();
        let source_pitches = stage.card_sounding_pitches_at_tempo(&card, 91.0);
        let recipe = CapturedArpeggio::capture(&card, &stage, 91.0).unwrap();
        assert_eq!(recipe.preview().unwrap(), source_pitches);
        let saved = recipe.card().unwrap();
        assert_eq!(
            saved.setting.voicing_fingerprint,
            card.setting.voicing_fingerprint
        );
        assert_eq!(saved.setting.voicing_profile, card.setting.voicing_profile);
        assert_eq!(saved.setting.fret_window.unwrap().span, 4);
        assert_eq!(serde_json::to_value(&card.setting).unwrap(), source_setup);
        let mut stale = recipe.clone();
        stale.instruction["setting"]["voicing_fingerprint"] =
            serde_json::json!("stale fingerprint");
        assert!(stale.preview().is_err());
        let legacy: CapturedArpeggio =
            serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
        assert_eq!(legacy.preview().unwrap(), source_pitches);
    }

    #[test]
    fn freezes_inherited_setup_tempo_and_preserves_instruction() {
        let mut stage = StageState::new();
        stage.fret_start = 3;
        stage.fret_count = 14;
        let source = source();
        let before = serde_json::to_value(&source).unwrap();
        let recipe = CapturedArpeggio::capture(&source, &stage, 73.0).unwrap();
        let captured = recipe.card().unwrap();
        assert_eq!(captured.id, CardId::UNASSIGNED);
        assert_eq!(captured.timing.bpm, Some(73.0));
        assert_eq!(captured.setting.fret_window.unwrap().start, 3);
        assert!(matches!(
            captured.touch,
            Touch::Arpeggiate {
                direction: ArpeggioDirection::Down,
                inversion: 1
            }
        ));
        let heard = recipe.preview().unwrap();
        stage.set_tuning(1);
        stage.fret_start = 0;
        stage.fret_count = 4;
        assert_eq!(recipe.preview().unwrap(), heard);
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
        let restored: CapturedArpeggio =
            serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
        assert_eq!(restored.preview().unwrap(), heard);
    }
    #[test]
    fn malformed_stale_and_invalid_setup_fail_closed() {
        let recipe = CapturedArpeggio::capture(&source(), &StageState::new(), 81.0).unwrap();
        for (pointer, value) in [
            ("/material/Chord/name", serde_json::json!("Missing")),
            ("/setting/tuning", serde_json::json!("Missing tuning")),
            ("/timing/bpm", serde_json::json!(0)),
            ("/id", serde_json::json!(9)),
        ] {
            let mut broken = recipe.clone();
            *broken.instruction.pointer_mut(pointer).unwrap() = value;
            assert!(broken.preview().is_err(), "{pointer}");
        }
        let mut shape = recipe.clone();
        shape.instruction["setting"]["voicing_idx"] = serde_json::json!(usize::MAX);
        assert!(shape.preview().is_err());
        let mut no_source = source();
        no_source.id = CardId::UNASSIGNED;
        assert!(CapturedArpeggio::capture(&no_source, &StageState::new(), 81.0).is_err());
        assert!(CapturedArpeggio::capture(&source(), &StageState::new(), f32::NAN).is_err());
    }
    #[test]
    fn marked_formula_resolves_saved_window_and_tuning_not_live_board() {
        let mut card = source();
        card.setting.marked = vec![(1, 5)];
        card.setting.mark_mode = MarkMode::Solo;
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::UpDown,
            inversion: 0,
        };
        let mut stage = StageState::new();
        stage.fret_start = 5;
        stage.fret_count = 12;
        let recipe = CapturedArpeggio::capture(&card, &stage, 80.0).unwrap();
        let frozen = recipe.card().unwrap();
        let before = stage.card_sounding_pitches(&frozen);
        stage.fret_start = 11;
        stage.fret_count = 16;
        stage.set_tuning(1);
        assert_eq!(stage.card_sounding_pitches(&frozen), before);
        assert_eq!(recipe.preview().unwrap(), before);
    }
    fn scale_source(pattern: woodshedding::rehearsal::ScalePattern) -> Card {
        let mut card = KeyedCatalogRef {
            formula_id: format!("scale-pattern:{}:Major", pattern.slug()),
            root: PitchClass::new(0),
        }
        .to_card()
        .unwrap();
        card.id = CardId(17);
        card.setting.instrument.clear();
        card.setting.tuning = None;
        card.timing.bpm = None;
        card.timing.hold = Hold::Bars(2);
        card
    }

    #[test]
    fn captured_scale_patterns_replay_exact_order_repetition_and_marked_setup() {
        use woodshedding::rehearsal::ScalePattern;
        let mut stage = StageState::new();
        stage.set_tuning(1);
        stage.fret_start = 2;
        stage.fret_count = 10;
        for pattern in ScalePattern::ALL {
            let mut card = scale_source(pattern);
            card.setting.capo = Some(2);
            let realization = stage.scale_card_realization(&card).unwrap();
            // Select two pitches that actually occur in the ordered visits.
            card.setting.marked = realization
                .notes
                .iter()
                .take(2)
                .map(|note| (note.string_index, note.physical_fret))
                .collect();
            for mode in [MarkMode::Off, MarkMode::Solo, MarkMode::Mute] {
                card.setting.mark_mode = mode;
                let original = serde_json::to_value(&card).unwrap();
                let expected = stage.card_sounding_pitches_at_tempo(&card, 83.0);
                let recipe = CapturedRecipe::capture(&card, &stage, 83.0).unwrap();
                assert_eq!(recipe.preview().unwrap(), expected);
                assert_eq!(serde_json::to_value(&card).unwrap(), original);
                let saved = recipe.card().unwrap();
                assert_eq!(saved.setting.marked, card.setting.marked);
                assert_eq!(saved.setting.mark_mode, mode);
                assert_eq!(saved.timing.bpm, Some(83.0));
                assert_eq!(saved.setting.fret_window.unwrap().start, 2);
                assert!(recipe.detail().contains(pattern.label()));
                assert!(recipe.detail().contains("ordered contacts"));
                let mut other_stage = StageState::new();
                other_stage.fret_start = 15;
                other_stage.fret_count = 20;
                assert_eq!(other_stage.card_sounding_pitches(&saved), expected);
                let reopened: CapturedRecipe =
                    serde_json::from_value(serde_json::to_value(&recipe).unwrap()).unwrap();
                assert_eq!(reopened.preview().unwrap(), expected);
                if mode == MarkMode::Off {
                    assert!(
                        expected.0.windows(2).any(|pair| pair[1] < pair[0]),
                        "pattern visits must preserve pairing order rather than sort"
                    );
                    assert!(
                        expected
                            .0
                            .iter()
                            .enumerate()
                            .any(|(index, pitch)| expected.0[index + 1..].contains(pitch)),
                        "pattern visits must preserve repeated tones"
                    );
                }
            }
        }
    }

    #[test]
    fn scale_pattern_stale_shapes_marks_catalog_and_empty_solo_refuse() {
        use woodshedding::rehearsal::ScalePattern;
        let stage = StageState::new();
        let card = scale_source(ScalePattern::Thirds);
        let recipe = CapturedRecipe::capture(&card, &stage, 83.0).unwrap();
        for (field, value) in [
            ("voicing_idx", serde_json::json!(0)),
            ("voicing_fingerprint", serde_json::json!("stale")),
            ("voicing_profile", serde_json::json!("stale")),
            ("marked", serde_json::json!([[99, 99]])),
        ] {
            let mut broken = recipe.clone();
            broken.instruction["setting"][field] = value;
            assert!(broken.preview().is_err(), "{field}");
        }
        let mut broken = recipe.clone();
        broken.instruction["material"]["ScalePattern"]["name"] = serde_json::json!("Missing");
        assert!(broken.preview().is_err());
        let mut muted = card.clone();
        muted.setting.marked = stage
            .scale_card_realization(&muted)
            .unwrap()
            .dots
            .iter()
            .map(|dot| (dot.string_index, dot.fret))
            .collect();
        muted.setting.mark_mode = MarkMode::Mute;
        assert!(CapturedRecipe::capture(&muted, &stage, 83.0).is_err());
    }
}
