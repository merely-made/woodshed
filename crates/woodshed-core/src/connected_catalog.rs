//! The first connected catalog query: a chord occurrence's sequential form.
//! Queries never author the Set; staging resolves the source again by identity.

use woodshedding::rehearsal::{ArpeggioDirection, Card, CardId, Material, Set, Touch};

use crate::{CardShapeStatus, CardShapeUnavailable, StageState, harmony::KeyedCatalogRef};

#[derive(Clone, Debug)]
pub struct ChordArpeggioDiscovery {
    pub source_card_id: CardId,
    pub subject: KeyedCatalogRef,
    pub label: String,
    pub explanation: String,
    pub preview: Card,
    /// Without a selected shape, sound is a formula preview, not an instrument realization.
    pub formula_preview: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryUnavailable {
    NotChord,
    UnknownChord(String),
    MissingSource(CardId),
    Shape(CardShapeUnavailable),
}

impl std::fmt::Display for DiscoveryUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotChord => f.write_str("select a chord to discover its arpeggio"),
            Self::UnknownChord(name) => write!(f, "unknown chord: {name}"),
            Self::MissingSource(_) => f.write_str("the source chord is no longer in the Set"),
            Self::Shape(reason) => write!(f, "selected shape unavailable: {reason}"),
        }
    }
}
impl std::error::Error for DiscoveryUnavailable {}

impl StageState {
    pub fn chord_arpeggio_discovery(
        &self,
        source: &Card,
    ) -> Result<ChordArpeggioDiscovery, DiscoveryUnavailable> {
        let Material::Chord { name, .. } = &source.material else {
            return Err(DiscoveryUnavailable::NotChord);
        };
        let mut subject = KeyedCatalogRef::from_material(&source.material)
            .ok_or_else(|| DiscoveryUnavailable::UnknownChord(name.clone()))?;
        let formula_preview = source.setting.voicing_idx.is_none();
        if !formula_preview {
            if let CardShapeStatus::Unavailable(reason) = self.card_shape_status(source) {
                return Err(DiscoveryUnavailable::Shape(reason));
            }
        }
        subject.formula_id = woodshed_graph::arpeggio_id(name);
        let label = subject
            .label()
            .expect("validated chord has arpeggio identity");
        let mut preview = source.clone();
        preview.id = CardId::UNASSIGNED;
        preview.label = label.clone();
        preview.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::UpDown,
            inversion: 0,
        };
        // Preserve the recipe that supplied the material. Discovery identity
        // is separate from recipe provenance and authored occurrence identity.
        Ok(ChordArpeggioDiscovery {
            source_card_id: source.id,
            subject,
            label,
            explanation: if formula_preview {
                "The same rooted chord tones, played sequentially. Formula preview; no instrument shape selected."
            } else {
                "The same selected shape and concert pitches, played sequentially. Instrument, tuning, capo, marks, and timing are retained."
            }.to_string(),
            preview,
            formula_preview,
        })
    }

    /// Explicit authoring; always uses the source's current setup and timing.
    pub fn stage_chord_arpeggio(
        &self,
        set: &mut Set,
        source_id: CardId,
    ) -> Result<CardId, DiscoveryUnavailable> {
        let source = set
            .cards
            .iter()
            .find(|card| card.id == source_id)
            .ok_or(DiscoveryUnavailable::MissingSource(source_id))?;
        let discovery = self.chord_arpeggio_discovery(source)?;
        set.push(discovery.preview);
        Ok(set.cards.last().expect("push adds a card").id)
    }
}

/// Apply the Card's arpeggio touch after pitch selection/mark filtering.
/// This consumes actual selected-shape concert pitches where supplied; it never
/// manufactures another voicing or shifts an exact shape to fake an inversion.
/// Inversion starts at the requested chord tone, omitting lower pitches as the
/// existing arpeggio transport does. It does not promise full-shape coverage.
/// Tempo is quarter-note spacing for Manual, defaulting to 120 bpm. A timed
/// hold distributes one whole traversal across its dwell, including its tail.
pub fn arpeggiate_preview(card: &Card, preview: (Vec<f32>, f32, f32)) -> (Vec<f32>, f32, f32) {
    let Touch::Arpeggiate {
        direction,
        inversion,
    } = card.touch
    else {
        return preview;
    };
    let (mut pitches, _, _) = preview;
    pitches.retain(|hz| hz.is_finite() && *hz > 0.0);
    if pitches.is_empty() {
        return (pitches, 0.0, 0.0);
    }
    pitches.sort_by(f32::total_cmp);
    if let Material::Chord { name, root } = &card.material {
        if let Some(formula) = woodshedding::chord::catalog()
            .iter()
            .find(|formula| formula.name == name)
        {
            let index = usize::from(inversion).min(formula.intervals.len().saturating_sub(1));
            if let Some(interval) = formula.intervals.get(index) {
                let capo = if card.setting.voicing_idx.is_some() {
                    i32::from(card.setting.capo.unwrap_or(0))
                } else {
                    0
                };
                let bass =
                    (i32::from(root.value()) + capo + interval.semitones()).rem_euclid(12) as u8;
                if let Some(start) = pitches.iter().position(|hz| crate::pc_from_hz(*hz) == bass) {
                    pitches.drain(..start);
                }
            }
        }
    }
    match direction {
        ArpeggioDirection::Up => {},
        ArpeggioDirection::Down => pitches.reverse(),
        ArpeggioDirection::UpDown => {
            let back = pitches
                .iter()
                .skip(1)
                .take(pitches.len().saturating_sub(2))
                .rev()
                .copied()
                .collect::<Vec<_>>();
            pitches.extend(back);
        },
    }
    let bpm = card
        .timing
        .bpm
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
        .unwrap_or(120.0)
        .clamp(30.0, 300.0);
    let dwell = match card.timing.hold {
        woodshedding::rehearsal::Hold::Manual => None,
        woodshedding::rehearsal::Hold::Bars(n) => Some(4.0 * 60.0 / bpm * f32::from(n.max(1))),
        woodshedding::rehearsal::Hold::Reps(n) => Some(4.0 * 60.0 / bpm * f32::from(n.max(1))),
        woodshedding::rehearsal::Hold::Seconds(seconds) => Some(if seconds.is_finite() {
            seconds.max(0.5)
        } else {
            0.5
        }),
    };
    let step_secs = dwell.map_or(60.0 / bpm, |duration| duration / pitches.len() as f32);
    let duration =
        dwell.unwrap_or_else(|| (pitches.len().saturating_sub(1) as f32 * step_secs) + 0.4);
    (pitches, duration, step_secs * 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{FretWindow, Recipe, Setting, Timing},
    };

    fn source() -> Card {
        Card {
            id: CardId(1),
            label: "D major seventh".into(),
            material: Material::Chord {
                name: "Major 7".into(),
                root: PitchClass::new(2),
            },
            setting: Setting {
                instrument: "Guitar".into(),
                fret_window: Some(FretWindow { start: 0, span: 4 }),
                ..Default::default()
            },
            touch: Touch::Block,
            timing: Timing {
                bpm: Some(90.0),
                ..Default::default()
            },
            from: Some(Recipe::Progression {
                name: "Source recipe".into(),
                key: PitchClass::new(2),
            }),
        }
    }

    #[test]
    fn discovery_is_rooted_and_does_not_author_the_source() {
        let state = StageState::new();
        let source = source();
        let before = serde_json::to_value(&source).unwrap();
        let discovery = state.chord_arpeggio_discovery(&source).unwrap();
        assert_eq!(discovery.subject.wire_key(), "arpeggio:Major 7@pc:2");
        assert_eq!(discovery.source_card_id, CardId(1));
        assert_eq!(discovery.preview.id, CardId::UNASSIGNED);
        assert_eq!(discovery.preview.timing.bpm, Some(90.0));
        assert!(discovery.formula_preview);
        assert!(discovery.label.contains("D Major 7 arpeggio"));
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
        assert_eq!(
            serde_json::to_value(&discovery.preview.from).unwrap(),
            before["from"]
        );
    }

    #[test]
    fn selected_shape_is_retained_and_stale_shape_is_unavailable() {
        let state = StageState::new();
        let mut card = source();
        state.select_next_card_shape(&mut card).unwrap();
        let discovery = state.chord_arpeggio_discovery(&card).unwrap();
        assert!(!discovery.formula_preview);
        assert_eq!(
            serde_json::to_value(&card.setting).unwrap(),
            serde_json::to_value(&discovery.preview.setting).unwrap()
        );
        card.setting.voicing_idx = Some(usize::MAX);
        assert!(matches!(
            state.chord_arpeggio_discovery(&card),
            Err(DiscoveryUnavailable::Shape(_))
        ));
    }

    #[test]
    fn selected_shape_discovery_sounds_the_same_concert_pitches_in_sequence() {
        let state = StageState::new();
        let mut card = source();
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 2, span: 4 });
        state.select_next_card_shape(&mut card).unwrap();
        let discovery = state.chord_arpeggio_discovery(&card).unwrap();
        let mut chord = state.card_sounding_pitches(&card).0;
        let (mut arpeggio, _, offset) = state.card_sounding_pitches(&discovery.preview);
        chord.sort_by(f32::total_cmp);
        chord.dedup();
        arpeggio.sort_by(f32::total_cmp);
        arpeggio.dedup();
        assert_eq!(chord, arpeggio);
        assert!(offset > 18.0);
        assert_eq!(
            state.dots_for_card(&card).len(),
            state.dots_for_card(&discovery.preview).len()
        );
    }

    #[test]
    fn capoed_inversion_uses_concert_tonic_when_shape_root_is_another_chord_tone() {
        let state = StageState::new();
        let mut card = source();
        card.material = Material::Chord {
            name: "Major".into(),
            root: PitchClass::new(0),
        };
        card.setting.capo = Some(5);
        card.setting.fret_window = Some(FretWindow { start: 5, span: 4 });
        state.select_next_card_shape(&mut card).unwrap();
        let discovery = state.chord_arpeggio_discovery(&card).unwrap();
        let mut chord = state.card_sounding_pitches(&card).0;
        let mut arpeggio = state.card_sounding_pitches(&discovery.preview).0;
        chord.sort_by(f32::total_cmp);
        chord.dedup();
        arpeggio.sort_by(f32::total_cmp);
        arpeggio.dedup();
        assert_eq!(
            chord, arpeggio,
            "C shape capo 5 must retain F/A/C concert tones"
        );
    }

    #[test]
    fn stage_resolves_current_source_and_assigns_a_new_occurrence() {
        let state = StageState::new();
        let mut set = Set::default();
        set.push(source());
        let id = set.cards[0].id;
        set.cards[0].timing.bpm = Some(72.0);
        let new_id = state.stage_chord_arpeggio(&mut set, id).unwrap();
        assert_ne!(new_id, id);
        assert_eq!(set.cards[1].timing.bpm, Some(72.0));
        assert!(matches!(set.cards[1].touch, Touch::Arpeggiate { .. }));
        assert!(state.stage_chord_arpeggio(&mut set, CardId(999)).is_err());
        assert_eq!(set.cards.len(), 2);
    }

    #[test]
    fn repeated_arpeggios_reopen_with_distinct_identity_touch_and_provenance() {
        let state = StageState::new();
        let mut set = Set::default();
        set.push(source());
        let source = set.cards[0].id;
        let first = state.stage_chord_arpeggio(&mut set, source).unwrap();
        let second = state.stage_chord_arpeggio(&mut set, source).unwrap();
        let reopened: Set = serde_json::from_str(&serde_json::to_string(&set).unwrap()).unwrap();
        assert_ne!(first, second);
        assert_eq!(reopened.cards[1].id, first);
        assert_eq!(reopened.cards[2].id, second);
        assert_eq!(
            KeyedCatalogRef::from_card(&reopened.cards[1])
                .unwrap()
                .wire_key(),
            "arpeggio:Major 7@pc:2"
        );
        assert!(matches!(
            reopened.cards[1].from,
            Some(Recipe::Progression { .. })
        ));
    }

    #[test]
    fn unknown_material_never_substitutes_a_catalog_entry() {
        let state = StageState::new();
        let mut card = source();
        card.material = Material::Chord {
            name: "Missing".into(),
            root: PitchClass::new(0),
        };
        assert_eq!(
            state.chord_arpeggio_discovery(&card).unwrap_err(),
            DiscoveryUnavailable::UnknownChord("Missing".into())
        );
    }

    #[test]
    fn sequential_touch_orders_selected_pitches_and_uses_tempo() {
        let mut card = source();
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::UpDown,
            inversion: 0,
        };
        let hz = |midi| {
            woodshedding::pitch::Pitch::from_midi(midi, woodshedding::pitch::Spelling::Sharps)
                .frequency() as f32
        };
        let input = vec![hz(73), hz(62), hz(69), hz(66)];
        let (pitches, duration, offset) = arpeggiate_preview(&card, (input, 1.4, 18.0));
        assert_eq!(
            pitches,
            vec![hz(62), hz(66), hz(69), hz(73), hz(69), hz(66)]
        );
        assert!((offset - 666.6667).abs() < 0.01);
        assert!(duration > 3.7);
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::Down,
            inversion: 1,
        };
        let (pitches, _, _) =
            arpeggiate_preview(&card, (vec![hz(62), hz(66), hz(69), hz(73)], 0.0, 0.0));
        assert_eq!(pitches, vec![hz(73), hz(69), hz(66)]);
    }

    #[test]
    fn timed_hold_fits_the_whole_traversal_before_auto_advance() {
        let mut card = source();
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::UpDown,
            inversion: 0,
        };
        card.timing.hold = woodshedding::rehearsal::Hold::Seconds(1.0);
        let (pitches, duration, offset) =
            arpeggiate_preview(&card, (vec![100.0, 200.0, 300.0, 400.0], 1.4, 18.0));
        assert_eq!(duration, 1.0);
        assert!(offset * ((pitches.len() - 1) as f32) < duration * 1000.0);
    }

    #[test]
    fn rehearsal_inherited_tempo_fits_native_dwell_without_mutating_instruction() {
        let state = StageState::new();
        let mut card = source();
        card.touch = Touch::Arpeggiate {
            direction: ArpeggioDirection::UpDown,
            inversion: 0,
        };
        card.timing.bpm = None;
        card.timing.hold = woodshedding::rehearsal::Hold::Bars(1);
        let (pitches, duration, offset) = state.card_sounding_pitches_at_tempo(&card, 180.0);
        let dwell = crate::card_dwell(&card, 180.0).unwrap().as_secs_f32();
        assert!(!pitches.is_empty());
        assert!((duration - dwell).abs() < 0.00001);
        assert!(offset * ((pitches.len() - 1) as f32) < dwell * 1000.0);
        assert_eq!(card.timing.bpm, None);
        card.timing.bpm = Some(90.0);
        let (_, explicit_duration, _) = state.card_sounding_pitches_at_tempo(&card, 180.0);
        assert!((explicit_duration - dwell * 2.0).abs() < 0.00001);
    }
}
