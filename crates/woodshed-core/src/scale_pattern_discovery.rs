//! Explicit scale-degree recipes discovered from an ordinary scale occurrence.

use crate::{StageState, harmony::KeyedCatalogRef, scale_realization::ScaleRealizationUnavailable};
use woodshedding::rehearsal::{Card, CardId, MarkMode, Material, Recipe, ScalePattern, Set, Touch};

#[derive(Clone, Debug)]
pub struct ScalePatternDiscovery {
    pub source_card_id: CardId,
    pub pattern: ScalePattern,
    pub label: String,
    pub explanation: String,
    pub preview: Card,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalePatternDiscoveryUnavailable {
    NotOrdinaryScale,
    MissingSource(CardId),
    Realization(ScaleRealizationUnavailable),
}

impl std::fmt::Display for ScalePatternDiscoveryUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotOrdinaryScale => {
                f.write_str("select an ordinary scale Card to discover a degree pattern")
            },
            Self::MissingSource(_) => f.write_str("the source scale is no longer in the Set"),
            Self::Realization(reason) => reason.fmt(f),
        }
    }
}
impl std::error::Error for ScalePatternDiscoveryUnavailable {}

impl StageState {
    pub fn scale_pattern_discovery(
        &self,
        source: &Card,
        pattern: ScalePattern,
    ) -> Result<ScalePatternDiscovery, ScalePatternDiscoveryUnavailable> {
        let Material::Scale { name, root } = &source.material else {
            return Err(ScalePatternDiscoveryUnavailable::NotOrdinaryScale);
        };
        let mut preview = source.clone();
        preview.id = CardId::UNASSIGNED;
        preview.material = Material::ScalePattern {
            name: name.clone(),
            root: *root,
            pattern,
        };
        preview.touch = Touch::Walk;
        preview.setting.voicing_idx = None;
        preview.setting.voicing_fingerprint = None;
        preview.setting.voicing_profile = None;
        preview.setting.marked.clear();
        preview.setting.mark_mode = MarkMode::Off;
        preview.from = Some(Recipe::Exercise {
            name: format!("Scale degree pairs: {}", pattern.label().to_lowercase()),
        });
        let realization = self
            .scale_card_realization(&preview)
            .map_err(ScalePatternDiscoveryUnavailable::Realization)?;
        let label = KeyedCatalogRef::from_material(&preview.material)
            .and_then(|subject| subject.label())
            .expect("realized scale has a catalog identity");
        preview.label = label.clone();
        let grammar = match pattern {
            ScalePattern::Thirds => "1–3, 2–4, 3–5",
            ScalePattern::Fourths => "1–4, 2–5, 3–6",
        };
        let explanation = format!(
            "Play {} as diatonic pairs ({grammar}…), continuing across octaves. {} complete pairs are available in this instrument and fret window. Only available pairs are included; missing degrees keep their place in the scale. Contacts are chosen deterministically; this is not a fingering recommendation. Setup and timing are retained; marked notes are cleared.",
            pattern.label().to_lowercase(),
            realization.notes.len() / 2
        );
        Ok(ScalePatternDiscovery {
            source_card_id: source.id,
            pattern,
            label,
            explanation,
            preview,
        })
    }

    pub fn stage_scale_pattern(
        &self,
        set: &mut Set,
        source_id: CardId,
        pattern: ScalePattern,
    ) -> Result<CardId, ScalePatternDiscoveryUnavailable> {
        let source = set
            .cards
            .iter()
            .find(|card| card.id == source_id)
            .ok_or(ScalePatternDiscoveryUnavailable::MissingSource(source_id))?;
        let discovery = self.scale_pattern_discovery(source, pattern)?;
        set.push(discovery.preview);
        Ok(set.cards.last().expect("push adds a card").id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::catalog_id_for_card;
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{FretWindow, Hold, Setting, Timing},
    };

    fn scale() -> Card {
        Card {
            id: CardId(6),
            label: "C major".into(),
            material: Material::Scale {
                name: "Major".into(),
                root: PitchClass::new(0),
            },
            setting: Setting {
                instrument: "Ukulele".into(),
                tuning: Some("Standard (high-G)".into()),
                capo: Some(2),
                fret_window: Some(FretWindow { start: 2, span: 12 }),
                ..Default::default()
            },
            touch: Touch::Walk,
            timing: Timing::default(),
            from: None,
        }
    }

    fn midi(pitches: &[f32]) -> Vec<i32> {
        pitches
            .iter()
            .map(|hz| (69.0 + 12.0 * (*hz / 440.0).log2()).round() as i32)
            .collect()
    }

    #[test]
    fn pairs_preserve_degree_order_repetition_and_capo_concert_pitches() {
        let state = StageState::new();
        let source = scale();
        for (pattern, expected) in [
            (ScalePattern::Thirds, vec![62, 66, 64, 67, 66, 69]),
            (ScalePattern::Fourths, vec![62, 67, 64, 69, 66, 71]),
        ] {
            let discovery = state.scale_pattern_discovery(&source, pattern).unwrap();
            let realized = state.scale_card_realization(&discovery.preview).unwrap();
            let pitches = state.card_sounding_pitches(&discovery.preview).0;
            let heard = midi(&pitches);
            assert_eq!(&heard[..6], expected.as_slice());
            assert!(heard.windows(2).any(|pair| pair[0] > pair[1]));
            assert_eq!(
                realized
                    .notes
                    .iter()
                    .map(|note| note.pitch.midi())
                    .collect::<Vec<_>>(),
                heard
            );
            for note in &realized.notes {
                assert!(
                    realized
                        .dots
                        .iter()
                        .any(|dot| dot.string_index == note.string_index
                            && dot.fret == note.physical_fret
                            && (dot.frequency - note.pitch.frequency() as f32).abs() < 0.001)
                );
            }
        }
    }

    #[test]
    fn query_is_immutable_and_clears_old_selection_while_stamping_explicit_recipe() {
        let state = StageState::new();
        let mut source = scale();
        source.setting.marked = vec![(1, 2)];
        source.setting.mark_mode = MarkMode::Solo;
        source.setting.voicing_idx = Some(99);
        source.setting.voicing_profile = Some("old".into());
        source.setting.voicing_fingerprint = Some("old".into());
        source.timing.bpm = Some(73.0);
        let original = serde_json::to_value(&source).unwrap();
        let discovery = state
            .scale_pattern_discovery(&source, ScalePattern::Thirds)
            .unwrap();
        assert_eq!(serde_json::to_value(&source).unwrap(), original);
        assert_eq!(discovery.source_card_id, source.id);
        assert_eq!(discovery.preview.id, CardId::UNASSIGNED);
        assert_eq!(discovery.preview.timing.bpm, Some(73.0));
        assert_eq!(discovery.preview.setting.tuning, source.setting.tuning);
        assert_eq!(discovery.preview.setting.capo, Some(2));
        assert!(discovery.preview.setting.marked.is_empty());
        assert_eq!(discovery.preview.setting.mark_mode, MarkMode::Off);
        assert_eq!(discovery.preview.setting.voicing_idx, None);
        assert_eq!(discovery.preview.setting.voicing_profile, None);
        assert_eq!(discovery.preview.setting.voicing_fingerprint, None);
        assert!(
            matches!(discovery.preview.from, Some(Recipe::Exercise { ref name }) if name == "Scale degree pairs: thirds")
        );
        assert_eq!(
            catalog_id_for_card(&discovery.preview).as_deref(),
            Some("exercise:scale-thirds/v1:Major")
        );
        assert_eq!(
            KeyedCatalogRef::from_material(&discovery.preview.material)
                .unwrap()
                .wire_key(),
            "scale-pattern:thirds:Major@pc:0"
        );
    }

    #[test]
    fn marks_filter_existing_visits_without_replacing_or_sorting_recipe() {
        let state = StageState::new();
        let mut pattern = state
            .scale_pattern_discovery(&scale(), ScalePattern::Thirds)
            .unwrap()
            .preview;
        let realization = state.scale_card_realization(&pattern).unwrap();
        let chosen = realization
            .notes
            .iter()
            .find(|note| note.pitch.midi() == 66)
            .unwrap();
        pattern.setting.marked = vec![(chosen.string_index, chosen.physical_fret)];
        pattern.setting.mark_mode = MarkMode::Solo;
        let solo = midi(&state.card_sounding_pitches(&pattern).0);
        assert!(solo.len() >= 2 && solo.iter().all(|pitch| *pitch == 66));
        pattern.setting.mark_mode = MarkMode::Mute;
        let muted = midi(&state.card_sounding_pitches(&pattern).0);
        let expected = realization
            .notes
            .iter()
            .map(|note| note.pitch.midi())
            .filter(|pitch| pitch.rem_euclid(12) != 6)
            .collect::<Vec<_>>();
        assert_eq!(muted, expected);
    }

    #[test]
    fn unsupported_formulas_invalid_setup_and_no_pairs_are_explicit() {
        let state = StageState::new();
        let mut source = scale();
        source.material = Material::Scale {
            name: "Major Pentatonic".into(),
            root: PitchClass::new(0),
        };
        let error = state
            .scale_pattern_discovery(&source, ScalePattern::Thirds)
            .unwrap_err();
        assert!(error.to_string().contains("seven"));
        source.material = Material::Scale {
            name: "Missing".into(),
            root: PitchClass::new(0),
        };
        assert!(
            state
                .scale_pattern_discovery(&source, ScalePattern::Thirds)
                .is_err()
        );
        source = scale();
        source.setting.tuning = Some("Missing".into());
        assert!(
            state
                .scale_pattern_discovery(&source, ScalePattern::Thirds)
                .is_err()
        );
        source = scale();
        source.setting.instrument = "Guitar".into();
        source.setting.tuning = Some("Ostrich".into());
        source.setting.capo = None;
        source.setting.fret_window = Some(FretWindow { start: 0, span: 0 });
        // All open strings share E, so no genuine third/fourth partner exists.
        assert!(
            state
                .scale_pattern_discovery(&source, ScalePattern::Thirds)
                .is_err()
        );
    }

    #[test]
    fn timed_pattern_traversal_fits_inherited_dwell_and_reopens_with_distinct_ids() {
        let state = StageState::new();
        let mut set = Set::default();
        let mut source = scale();
        source.timing.hold = Hold::Bars(1);
        set.push(source);
        let id = set.cards[0].id;
        let first = state
            .stage_scale_pattern(&mut set, id, ScalePattern::Thirds)
            .unwrap();
        let second = state
            .stage_scale_pattern(&mut set, id, ScalePattern::Thirds)
            .unwrap();
        assert_ne!(first, second);
        let (pitches, duration, offset) = state.card_sounding_pitches_at_tempo(&set.cards[1], 80.0);
        assert!((duration - 3.0).abs() < 0.00001);
        assert!((pitches.len() - 1) as f32 * offset < duration * 1000.0);
        let reopened: Set = serde_json::from_str(&serde_json::to_string(&set).unwrap()).unwrap();
        assert_eq!(reopened.cards[1].id, first);
        assert_eq!(
            state
                .card_sounding_pitches_at_tempo(&reopened.cards[1], 80.0)
                .0,
            pitches
        );
        set.cards[0].material = Material::Chord {
            name: "Major".into(),
            root: PitchClass::new(0),
        };
        assert_eq!(
            state
                .stage_scale_pattern(&mut set, id, ScalePattern::Thirds)
                .unwrap_err(),
            ScalePatternDiscoveryUnavailable::NotOrdinaryScale
        );
        assert_eq!(set.cards.len(), 3);
        set.remove(0);
        assert!(matches!(
            state.stage_scale_pattern(&mut set, id, ScalePattern::Thirds),
            Err(ScalePatternDiscoveryUnavailable::MissingSource(_))
        ));
    }
}
