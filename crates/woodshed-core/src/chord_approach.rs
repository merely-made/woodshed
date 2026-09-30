//! Chromatic same-string approach pairs to explicit chord targets.
//! The source chord supplies passage context, not an inferred key or a
//! voice-leading route. Target contacts use their saved setup/selected shape.

use crate::{
    CardShapeGeometry, CardShapeStatus, CardShapeUnavailable, FretDot, StageState,
    card_shapes::ResolvedShapeSetup, harmony::KeyedCatalogRef,
};
use std::collections::BTreeSet;
use woodshedding::{
    fretboard::Fretboard,
    pitch::{Pitch, PitchClass, Spelling},
    rehearsal::{ApproachDirection, Card, CardId, MarkMode, Material, Recipe, Set, Touch},
};

#[derive(Clone, Debug)]
pub struct ChordApproachNote {
    pub pitch: Pitch,
    pub string_index: usize,
    pub physical_fret: u8,
    pub is_target: bool,
}

#[derive(Clone, Debug)]
pub struct ResolvedChordApproach {
    pub setup: ResolvedShapeSetup,
    pub geometry: CardShapeGeometry,
    pub dots: Vec<FretDot>,
    pub concert_root: PitchClass,
    pub notes: Vec<ChordApproachNote>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordApproachUnavailable {
    NotApproach,
    UnknownChord(String),
    Setup(CardShapeUnavailable),
    Shape(CardShapeUnavailable),
    NoCompletePairs,
}
impl std::fmt::Display for ChordApproachUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotApproach => f.write_str("this Card is not a chord approach recipe"),
            Self::UnknownChord(name) => write!(f, "unknown target chord: {name}"),
            Self::Setup(reason) => write!(f, "approach setup unavailable: {reason}"),
            Self::Shape(reason) => write!(f, "target shape unavailable: {reason}"),
            Self::NoCompletePairs => f.write_str(
                "no complete approach-to-target pairs are available in this fret window",
            ),
        }
    }
}
impl std::error::Error for ChordApproachUnavailable {}

#[derive(Clone, Debug)]
pub struct ChordApproachDiscovery {
    pub source_card_id: CardId,
    pub target_card_id: CardId,
    pub direction: ApproachDirection,
    pub label: String,
    pub explanation: String,
    pub preview: Card,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordApproachDiscoveryUnavailable {
    MissingSource(CardId),
    MissingTarget(CardId),
    NotChordPair,
    NotAdjacent,
    Realization(ChordApproachUnavailable),
}
impl std::fmt::Display for ChordApproachDiscoveryUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSource(_) => f.write_str("the source chord is no longer in the Set"),
            Self::MissingTarget(_) => f.write_str("the target chord is no longer in the Set"),
            Self::NotChordPair => {
                f.write_str("select two ordinary chord Cards for this approach recipe")
            },
            Self::NotAdjacent => {
                f.write_str("the selected target must still immediately follow the source chord")
            },
            Self::Realization(reason) => reason.fmt(f),
        }
    }
}
impl std::error::Error for ChordApproachDiscoveryUnavailable {}

impl StageState {
    pub fn chord_approach_realization(
        &self,
        card: &Card,
    ) -> Result<ResolvedChordApproach, ChordApproachUnavailable> {
        let Material::ChordApproach {
            name,
            root,
            direction,
        } = &card.material
        else {
            return Err(ChordApproachUnavailable::NotApproach);
        };
        let formula = woodshedding::chord::catalog()
            .iter()
            .find(|formula| formula.name == name)
            .ok_or_else(|| ChordApproachUnavailable::UnknownChord(name.clone()))?;
        let tuning = self
            .resolve_card_tuning(card)
            .map_err(ChordApproachUnavailable::Setup)?;
        let mut target_card = card.clone();
        target_card.material = Material::Chord {
            name: name.clone(),
            root: *root,
        };
        let (setup, geometry, mut targets) = if card.setting.voicing_idx.is_some() {
            let shape = match self.card_shape_status(&target_card) {
                CardShapeStatus::Available(shape) => shape,
                CardShapeStatus::Unavailable(reason) => {
                    return Err(ChordApproachUnavailable::Shape(reason));
                },
                _ => return Err(ChordApproachUnavailable::NoCompletePairs),
            };
            let board = Fretboard::new(tuning.clone(), shape.geometry.physical_fret_end);
            let targets = shape
                .physical_positions()
                .into_iter()
                .map(|(string_index, physical_fret)| ChordApproachNote {
                    pitch: board.pitch_at(string_index, physical_fret),
                    string_index,
                    physical_fret,
                    is_target: true,
                })
                .collect::<Vec<_>>();
            (shape.setup, shape.geometry, targets)
        } else {
            let capo = card.setting.capo.unwrap_or(0);
            let end_limit = tuning.instrument.standard_fret_count();
            let (start, end) = card.setting.fret_window.map_or(
                (self.fret_start, self.fret_count.min(end_limit)),
                |window| {
                    (
                        window.start,
                        window.start.saturating_add(window.span).min(end_limit),
                    )
                },
            );
            if end < capo {
                return Err(ChordApproachUnavailable::Setup(
                    CardShapeUnavailable::WindowBelowCapo,
                ));
            }
            let start = start.max(capo);
            if start > end {
                return Err(ChordApproachUnavailable::Setup(
                    CardShapeUnavailable::EmptyWindow,
                ));
            }
            let geometry = CardShapeGeometry {
                string_count: tuning.string_count(),
                physical_fret_start: start,
                physical_fret_end: end,
                capo,
            };
            let setup = ResolvedShapeSetup {
                instrument: tuning.instrument,
                tuning_name: tuning.name.clone(),
                capo,
                concert_open_midi: tuning
                    .strings
                    .iter()
                    .map(|pitch| pitch.midi() + i32::from(capo))
                    .collect(),
            };
            let concert_root = Pitch::from_midi(
                48 + i32::from(root.value()) + i32::from(capo),
                Spelling::Sharps,
            );
            let board = Fretboard::new(tuning.clone(), end);
            let positions = board
                .positions_for_chord(formula, concert_root)
                .map_err(|_| ChordApproachUnavailable::UnknownChord(name.clone()))?;
            let mut targets = positions
                .into_iter()
                .filter(|position| {
                    position.fret >= start
                        && direction.partner_fret(position.fret, start, end).is_some()
                })
                .map(|position| ChordApproachNote {
                    pitch: position.pitch,
                    string_index: position.string_index,
                    physical_fret: position.fret,
                    is_target: true,
                })
                .collect::<Vec<_>>();
            targets.sort_by_key(|note| (note.pitch.midi(), note.physical_fret, note.string_index));
            targets.dedup_by_key(|note| note.pitch.midi());
            (setup, geometry, targets)
        };
        targets.sort_by_key(|note| (note.pitch.midi(), note.physical_fret, note.string_index));
        let concert_root =
            PitchClass::new(((u16::from(root.value()) + u16::from(geometry.capo)) % 12) as u8);
        let board = Fretboard::new(tuning, geometry.physical_fret_end);
        let mut notes = Vec::new();
        for target in targets {
            let Some(physical_fret) = direction.partner_fret(
                target.physical_fret,
                geometry.physical_fret_start,
                geometry.physical_fret_end,
            ) else {
                continue;
            };
            notes.push(ChordApproachNote {
                pitch: board.pitch_at(target.string_index, physical_fret),
                string_index: target.string_index,
                physical_fret,
                is_target: false,
            });
            notes.push(target);
        }
        if notes.is_empty() {
            return Err(ChordApproachUnavailable::NoCompletePairs);
        }
        let mut seen = BTreeSet::new();
        let dots = notes
            .iter()
            .filter(|note| seen.insert((note.string_index, note.physical_fret)))
            .map(|note| {
                let semitones = (i32::from(note.pitch.pitch_class())
                    - i32::from(concert_root.value()))
                .rem_euclid(12);
                FretDot {
                    string_index: note.string_index,
                    fret: note.physical_fret,
                    is_root: semitones == 0,
                    label: format!("{}{}", note.pitch.name, note.pitch.accidental),
                    octave: note.pitch.octave,
                    degree: crate::degree_label(semitones).into(),
                    interval_name: crate::interval_name(semitones).into(),
                    frequency: note.pitch.frequency() as f32,
                }
            })
            .collect();
        Ok(ResolvedChordApproach {
            setup,
            geometry,
            dots,
            concert_root,
            notes,
        })
    }

    pub(crate) fn chord_approach_preview(
        &self,
        card: &Card,
        apply_marks: bool,
    ) -> (Vec<f32>, f32, f32) {
        let Ok(realization) = self.chord_approach_realization(card) else {
            return (Vec::new(), 0.0, 0.0);
        };
        let mut pitches = realization
            .notes
            .iter()
            .map(|note| note.pitch.frequency() as f32)
            .collect::<Vec<_>>();
        if apply_marks && !card.setting.marked.is_empty() {
            let marked = card.setting.marked.iter().copied().collect::<BTreeSet<_>>();
            let selected = realization
                .dots
                .iter()
                .filter(|dot| marked.contains(&(dot.string_index, dot.fret)))
                .map(|dot| dot.frequency)
                .collect::<Vec<_>>();
            match card.setting.mark_mode {
                MarkMode::Off => {},
                MarkMode::Solo => pitches.retain(|pitch| selected.contains(pitch)),
                MarkMode::Mute => {
                    let muted = selected
                        .iter()
                        .map(|hz| crate::pc_from_hz(*hz))
                        .collect::<BTreeSet<_>>();
                    pitches.retain(|pitch| !muted.contains(&crate::pc_from_hz(*pitch)));
                },
            }
        }
        crate::scale_realization::ordered_card_preview(card, pitches)
    }

    pub fn chord_approach_discovery(
        &self,
        set: &Set,
        source_id: CardId,
        target_id: CardId,
        direction: ApproachDirection,
    ) -> Result<ChordApproachDiscovery, ChordApproachDiscoveryUnavailable> {
        let source_index = set
            .cards
            .iter()
            .position(|card| card.id == source_id)
            .ok_or(ChordApproachDiscoveryUnavailable::MissingSource(source_id))?;
        let target_index = set
            .cards
            .iter()
            .position(|card| card.id == target_id)
            .ok_or(ChordApproachDiscoveryUnavailable::MissingTarget(target_id))?;
        let (source, target) = (&set.cards[source_index], &set.cards[target_index]);
        if !matches!(source.material, Material::Chord { .. })
            || !matches!(target.material, Material::Chord { .. })
        {
            return Err(ChordApproachDiscoveryUnavailable::NotChordPair);
        }
        if target_index != source_index + 1 {
            return Err(ChordApproachDiscoveryUnavailable::NotAdjacent);
        }
        // Unknown source material is not credible passage context.
        if KeyedCatalogRef::from_material(&source.material).is_none() {
            return Err(ChordApproachDiscoveryUnavailable::NotChordPair);
        }
        let Material::Chord { name, root } = &target.material else {
            unreachable!();
        };
        let mut preview = target.clone();
        preview.id = CardId::UNASSIGNED;
        preview.material = Material::ChordApproach {
            name: name.clone(),
            root: *root,
            direction,
        };
        preview.touch = Touch::Walk;
        preview.setting.marked.clear();
        preview.setting.mark_mode = MarkMode::Off;
        preview.from = Some(Recipe::Exercise {
            name: format!("Chord tone approach: {}", direction.slug()),
        });
        let realized = self
            .chord_approach_realization(&preview)
            .map_err(ChordApproachDiscoveryUnavailable::Realization)?;
        let label = KeyedCatalogRef::from_material(&preview.material)
            .and_then(|keyed| keyed.label())
            .expect("realized chord recipe has identity");
        preview.label = label.clone();
        let basis = if target.setting.voicing_idx.is_some() {
            "selected target shape"
        } else {
            "available target chord contacts"
        };
        let explanation = format!(
            "From this adjacent passage, explore {} from one chromatic semitone {} each target tone on the same string. {} complete pairs use the {basis}. The approach tone is transient and may lie outside the chord. This is a standalone exercise, not an inferred voice-leading route. Target setup, shape and timing are retained; marked notes are cleared.",
            target.label,
            direction.slug(),
            realized.notes.len() / 2
        );
        Ok(ChordApproachDiscovery {
            source_card_id: source_id,
            target_card_id: target_id,
            direction,
            label,
            explanation,
            preview,
        })
    }

    pub fn stage_chord_approach(
        &self,
        set: &mut Set,
        source_id: CardId,
        target_id: CardId,
        direction: ApproachDirection,
    ) -> Result<CardId, ChordApproachDiscoveryUnavailable> {
        let discovery = self.chord_approach_discovery(set, source_id, target_id, direction)?;
        set.push(discovery.preview);
        Ok(set.cards.last().expect("append adds a Card").id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::catalog_id_for_card;
    use woodshedding::{
        rehearsal::{FretWindow, Hold, Setting, Timing},
        tuning::{Instrument, Tuning},
    };

    fn pair() -> Set {
        let mut set = Set::default();
        for (name, root) in [("Minor 7", 9), ("Major 7", 0)] {
            set.push(Card {
                id: CardId::UNASSIGNED,
                label: format!("{name} target"),
                material: Material::Chord {
                    name: name.into(),
                    root: PitchClass::new(root),
                },
                setting: Setting {
                    instrument: "Ukulele".into(),
                    tuning: Some("Standard (high-G)".into()),
                    capo: Some(2),
                    fret_window: Some(FretWindow { start: 2, span: 12 }),
                    ..Default::default()
                },
                touch: Touch::Block,
                timing: Timing::default(),
                from: None,
            });
        }
        set
    }

    #[test]
    fn chromatic_pairs_are_actual_same_string_contacts_with_target_setup() {
        let state = StageState::new();
        let set = pair();
        let tuning = Tuning::find_for("Standard (high-G)", Instrument::Ukulele).unwrap();
        for direction in ApproachDirection::ALL {
            let discovery = state
                .chord_approach_discovery(&set, set.cards[0].id, set.cards[1].id, direction)
                .unwrap();
            let realized = state
                .chord_approach_realization(&discovery.preview)
                .unwrap();
            assert_eq!(realized.geometry.string_count, 4);
            assert_eq!(realized.concert_root, PitchClass::new(2));
            for pair in realized.notes.chunks_exact(2) {
                assert!(!pair[0].is_target && pair[1].is_target);
                assert_eq!(pair[0].string_index, pair[1].string_index);
                assert_eq!(
                    i16::from(pair[0].physical_fret) - i16::from(pair[1].physical_fret),
                    direction.fret_delta()
                );
                assert_eq!(
                    pair[0].pitch.midi() - pair[1].pitch.midi(),
                    i32::from(direction.fret_delta())
                );
                for note in pair {
                    assert_eq!(
                        note.pitch.midi(),
                        tuning.strings[note.string_index].midi() + i32::from(note.physical_fret)
                    );
                    assert!(note.physical_fret >= 2 && note.physical_fret <= 14);
                }
            }
            let sounded = state.card_sounding_pitches(&discovery.preview).0;
            assert_eq!(
                sounded,
                realized
                    .notes
                    .iter()
                    .map(|note| note.pitch.frequency() as f32)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn selected_shape_is_retained_exactly_and_stale_shape_is_not_substituted() {
        let state = StageState::new();
        let mut set = pair();
        state.select_next_card_shape(&mut set.cards[1]).unwrap();
        let before = serde_json::to_value(&set).unwrap();
        let discovery = state
            .chord_approach_discovery(
                &set,
                set.cards[0].id,
                set.cards[1].id,
                ApproachDirection::Above,
            )
            .unwrap();
        assert_eq!(serde_json::to_value(&set).unwrap(), before);
        assert_eq!(
            discovery.preview.setting.voicing_fingerprint,
            set.cards[1].setting.voicing_fingerprint
        );
        assert_eq!(
            discovery.preview.setting.voicing_profile,
            set.cards[1].setting.voicing_profile
        );
        let shape = match state.card_shape_status(&set.cards[1]) {
            CardShapeStatus::Available(shape) => shape,
            _ => panic!("selected shape"),
        };
        let realized = state
            .chord_approach_realization(&discovery.preview)
            .unwrap();
        assert!(
            realized
                .notes
                .iter()
                .filter(|note| note.is_target)
                .all(|note| shape
                    .physical_positions()
                    .contains(&(note.string_index, note.physical_fret)))
        );
        set.cards[1].setting.voicing_fingerprint = Some("stale".into());
        assert!(matches!(
            state.chord_approach_discovery(
                &set,
                set.cards[0].id,
                set.cards[1].id,
                ApproachDirection::Above
            ),
            Err(ChordApproachDiscoveryUnavailable::Realization(
                ChordApproachUnavailable::Shape(_)
            ))
        ));
    }

    #[test]
    fn explicit_pair_rejects_reorder_removal_replacement_and_invalid_setup() {
        let state = StageState::new();
        let mut set = pair();
        let (source, target) = (set.cards[0].id, set.cards[1].id);
        set.cards.swap(0, 1);
        assert_eq!(
            state
                .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
                .unwrap_err(),
            ChordApproachDiscoveryUnavailable::NotAdjacent
        );
        assert_eq!(set.cards.len(), 2);
        set.cards.swap(0, 1);
        set.cards[1].setting.tuning = Some("Missing".into());
        assert!(matches!(
            state.stage_chord_approach(&mut set, source, target, ApproachDirection::Below),
            Err(ChordApproachDiscoveryUnavailable::Realization(
                ChordApproachUnavailable::Setup(_)
            ))
        ));
        set.cards[1].material = Material::Scale {
            name: "Major".into(),
            root: PitchClass::new(0),
        };
        assert_eq!(
            state
                .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
                .unwrap_err(),
            ChordApproachDiscoveryUnavailable::NotChordPair
        );
        set.remove(1);
        assert!(matches!(
            state.stage_chord_approach(&mut set, source, target, ApproachDirection::Below),
            Err(ChordApproachDiscoveryUnavailable::MissingTarget(_))
        ));
    }

    #[test]
    fn gaps_never_invent_partners_and_valid_alternative_contacts_are_used() {
        let state = StageState::new();
        let mut set = pair();
        set.cards[1].setting.fret_window = Some(FretWindow { start: 2, span: 0 });
        assert!(matches!(
            state.chord_approach_discovery(
                &set,
                set.cards[0].id,
                set.cards[1].id,
                ApproachDirection::Below
            ),
            Err(ChordApproachDiscoveryUnavailable::Realization(
                ChordApproachUnavailable::NoCompletePairs
            ))
        ));
        set.cards[1].setting.instrument = "Guitar".into();
        set.cards[1].setting.tuning = Some("Standard".into());
        set.cards[1].setting.capo = None;
        set.cards[1].setting.fret_window = Some(FretWindow { start: 0, span: 7 });
        let discovery = state
            .chord_approach_discovery(
                &set,
                set.cards[0].id,
                set.cards[1].id,
                ApproachDirection::Below,
            )
            .unwrap();
        let realized = state
            .chord_approach_realization(&discovery.preview)
            .unwrap();
        assert!(
            realized
                .notes
                .iter()
                .filter(|note| note.is_target)
                .all(|note| note.physical_fret > 0)
        );
        // E4's open top string cannot approach from below; the same pitch on
        // the B string at fret 5 provides a valid partner within this window.
        assert!(
            realized
                .notes
                .iter()
                .any(|note| note.is_target && note.pitch.midi() == 64 && note.physical_fret == 5)
        );
        assert!(
            realized
                .notes
                .iter()
                .any(|note| note.is_target && note.pitch.midi() == 52)
        );
    }

    #[test]
    fn order_marks_and_timing_preserve_visits_and_fit_inherited_dwell() {
        let state = StageState::new();
        let mut set = pair();
        set.cards[1].timing.hold = Hold::Bars(1);
        let mut recipe = state
            .chord_approach_discovery(
                &set,
                set.cards[0].id,
                set.cards[1].id,
                ApproachDirection::Above,
            )
            .unwrap()
            .preview;
        let realized = state.chord_approach_realization(&recipe).unwrap();
        let baseline = state.card_sounding_pitches(&recipe).0;
        assert!(baseline.windows(2).any(|pair| pair[0] > pair[1]));
        let target = &realized.notes[1];
        recipe.setting.marked = vec![(target.string_index, target.physical_fret)];
        recipe.setting.mark_mode = MarkMode::Solo;
        let expected = baseline
            .iter()
            .copied()
            .filter(|pitch| *pitch == target.pitch.frequency() as f32)
            .collect::<Vec<_>>();
        assert_eq!(state.card_sounding_pitches(&recipe).0, expected);
        recipe.setting.mark_mode = MarkMode::Mute;
        let expected = baseline
            .iter()
            .copied()
            .filter(|pitch| crate::pc_from_hz(*pitch) != target.pitch.pitch_class())
            .collect::<Vec<_>>();
        assert_eq!(state.card_sounding_pitches(&recipe).0, expected);
        recipe.setting.mark_mode = MarkMode::Off;
        let (pitches, duration, offset) = state.card_sounding_pitches_at_tempo(&recipe, 80.0);
        assert!((duration - 3.0).abs() < 0.00001);
        assert!((pitches.len() - 1) as f32 * offset < duration * 1000.0);
    }

    #[test]
    fn explicit_append_reopens_as_a_distinct_recipe_and_pitch_sets_include_approaches() {
        let state = StageState::new();
        let mut set = pair();
        let before = serde_json::to_value(&set.cards).unwrap();
        let (source, target) = (set.cards[0].id, set.cards[1].id);
        let first = state
            .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
            .unwrap();
        let second = state
            .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
            .unwrap();
        assert_ne!(first, second);
        assert_eq!(serde_json::to_value(&set.cards[..2]).unwrap(), before);
        let reopened: Set = serde_json::from_str(&serde_json::to_string(&set).unwrap()).unwrap();
        assert_eq!(reopened.cards[2].id, first);
        assert_eq!(
            catalog_id_for_card(&reopened.cards[2]).as_deref(),
            Some("exercise:chord-approach-below/v1:Major 7")
        );
        let keyed = KeyedCatalogRef::from_card(&reopened.cards[2]).unwrap();
        assert_eq!(keyed.wire_key(), "chord-approach:below:Major 7@pc:0");
        let pcs = keyed.pitch_classes().unwrap();
        assert!(pcs.contains(&PitchClass::new(3)));
        assert!(pcs.contains(&PitchClass::new(6)));
        assert_eq!(
            state.card_sounding_pitches(&reopened.cards[2]).0,
            state.card_sounding_pitches(&set.cards[2]).0
        );
    }
}
