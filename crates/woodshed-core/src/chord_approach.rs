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
    ) -> crate::CardPreviewResult {
        let Ok(realization) = self.chord_approach_realization(card) else {
            return Ok((Vec::new(), 0.0, 0.0));
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
mod tests;
