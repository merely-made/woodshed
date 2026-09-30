//! A bounded scale realization shared by display and audition.
//!
//! All contacts resolve on the Card's own tuning and nut-relative physical
//! frets. A capo raises the written scale root to concert pitch. The ascending
//! sequence chooses one deterministic contact per distinct MIDI pitch (lowest
//! fret, then string index); it is a simple traversal, not a fingering solver.
//! Explicit setup errors never substitute live instrument data.

use std::collections::BTreeSet;

use woodshedding::{
    fretboard::Fretboard,
    pitch::{Pitch, PitchClass, Spelling},
    rehearsal::{Card, Hold, MarkMode, Material, Touch},
};

use crate::card_shapes::ResolvedShapeSetup;
use crate::{CardShapeGeometry, CardShapeUnavailable, FretDot, StageState};

#[derive(Clone, Debug)]
pub struct ScaleCardNote {
    pub pitch: Pitch,
    pub string_index: usize,
    pub physical_fret: u8,
}

#[derive(Clone, Debug)]
pub struct ResolvedScaleCard {
    pub setup: ResolvedShapeSetup,
    pub geometry: CardShapeGeometry,
    pub dots: Vec<FretDot>,
    pub concert_root: PitchClass,
    pub notes: Vec<ScaleCardNote>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScaleRealizationUnavailable {
    NotScale,
    UnknownScale(String),
    Setup(CardShapeUnavailable),
    InvalidFormula(String),
    NoNotes,
}

impl std::fmt::Display for ScaleRealizationUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotScale => f.write_str("this Card is not scale material"),
            Self::UnknownScale(name) => write!(f, "unknown scale: {name}"),
            Self::Setup(reason) => write!(f, "scale setup unavailable: {reason}"),
            Self::InvalidFormula(reason) => write!(f, "scale formula unavailable: {reason}"),
            Self::NoNotes => f.write_str("no scale notes are available in this fret window"),
        }
    }
}
impl std::error::Error for ScaleRealizationUnavailable {}

impl StageState {
    pub fn scale_card_realization(
        &self,
        card: &Card,
    ) -> Result<ResolvedScaleCard, ScaleRealizationUnavailable> {
        let Material::Scale { name, root } = &card.material else {
            return Err(ScaleRealizationUnavailable::NotScale);
        };
        let formula = woodshedding::scale::catalog()
            .iter()
            .find(|formula| formula.name == name)
            .ok_or_else(|| ScaleRealizationUnavailable::UnknownScale(name.clone()))?;
        let tuning = self
            .resolve_card_tuning(card)
            .map_err(ScaleRealizationUnavailable::Setup)?;
        let capo = card.setting.capo.unwrap_or(0);
        let instrument_end = tuning.instrument.standard_fret_count();
        let (start, end) = if let Some(window) = card.setting.fret_window {
            (
                window.start,
                window.start.saturating_add(window.span).min(instrument_end),
            )
        } else {
            (self.fret_start, self.fret_count.min(instrument_end))
        };
        if end < capo {
            return Err(ScaleRealizationUnavailable::Setup(
                CardShapeUnavailable::WindowBelowCapo,
            ));
        }
        let start = start.max(capo);
        if start > end {
            return Err(ScaleRealizationUnavailable::Setup(
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
        let concert_root =
            PitchClass::new(((u16::from(root.value()) + u16::from(capo)) % 12) as u8);
        let root_pitch = Pitch::from_midi(
            48 + i32::from(root.value()) + i32::from(capo),
            Spelling::Sharps,
        );
        let board = Fretboard::new(tuning, end);
        let positions = board
            .positions_for_scale(formula, root_pitch)
            .map_err(|reason| ScaleRealizationUnavailable::InvalidFormula(reason.to_string()))?
            .into_iter()
            .filter(|position| position.fret >= start)
            .collect::<Vec<_>>();
        if positions.is_empty() {
            return Err(ScaleRealizationUnavailable::NoNotes);
        }
        let mut notes = positions
            .iter()
            .map(|position| ScaleCardNote {
                pitch: position.pitch,
                string_index: position.string_index,
                physical_fret: position.fret,
            })
            .collect::<Vec<_>>();
        notes.sort_by_key(|note| (note.pitch.midi(), note.physical_fret, note.string_index));
        notes.dedup_by_key(|note| note.pitch.midi());
        let dots = positions.into_iter().map(FretDot::from_position).collect();
        Ok(ResolvedScaleCard {
            setup,
            geometry,
            dots,
            concert_root,
            notes,
        })
    }

    pub(crate) fn scale_card_preview(
        &self,
        card: &Card,
        apply_marks: bool,
    ) -> (Vec<f32>, f32, f32) {
        let Ok(scale) = self.scale_card_realization(card) else {
            return (Vec::new(), 0.0, 0.0);
        };
        let mut pitches = scale
            .notes
            .iter()
            .map(|note| note.pitch.frequency() as f32)
            .collect::<Vec<_>>();
        if apply_marks && !card.setting.marked.is_empty() {
            let marked = card.setting.marked.iter().copied().collect::<BTreeSet<_>>();
            let contacts = scale
                .dots
                .iter()
                .filter(|dot| marked.contains(&(dot.string_index, dot.fret)))
                .map(|dot| dot.frequency)
                .collect::<Vec<_>>();
            match card.setting.mark_mode {
                MarkMode::Off => {},
                MarkMode::Solo => pitches = contacts,
                MarkMode::Mute => {
                    let muted = contacts
                        .iter()
                        .map(|hz| crate::pc_from_hz(*hz))
                        .collect::<BTreeSet<_>>();
                    pitches.retain(|hz| !muted.contains(&crate::pc_from_hz(*hz)));
                },
            }
        }
        pitches.sort_by(f32::total_cmp);
        pitches.dedup();
        if pitches.is_empty() {
            return (pitches, 0.0, 0.0);
        }
        if !matches!(card.touch, Touch::Walk) {
            let (duration, offset) = crate::voicing_shape(pitches.len(), false);
            return (pitches, duration, offset);
        }
        let bpm = card
            .timing
            .bpm
            .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
            .unwrap_or(120.0)
            .max(30.0);
        let dwell = match card.timing.hold {
            Hold::Manual => None,
            Hold::Bars(n) => Some(4.0 * 60.0 / bpm * f32::from(n.max(1))),
            Hold::Reps(n) => Some(4.0 * 60.0 / bpm * f32::from(n.max(1))),
            Hold::Seconds(seconds) => Some(if seconds.is_finite() {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::{
        rehearsal::{CardId, FretWindow, Setting, Timing},
        tuning::{Instrument, Tuning},
    };

    fn scale() -> Card {
        Card {
            id: CardId(42),
            label: "C major".into(),
            material: Material::Scale {
                name: "Major".into(),
                root: PitchClass::new(0),
            },
            setting: Setting {
                instrument: "Ukulele".into(),
                tuning: Some("Standard (high-G)".into()),
                capo: Some(2),
                fret_window: Some(FretWindow { start: 2, span: 4 }),
                ..Default::default()
            },
            touch: Touch::Walk,
            timing: Timing::default(),
            from: None,
        }
    }

    #[test]
    fn reentrant_ukulele_capo_overrides_live_guitar_and_preserves_instruction() {
        let mut state = StageState::new();
        state.fret_start = 12;
        state.fret_count = 20;
        let card = scale();
        let original = serde_json::to_value(&card).unwrap();
        let result = state.scale_card_realization(&card).unwrap();
        assert_eq!(result.geometry.string_count, 4);
        assert_eq!(result.geometry.physical_fret_start, 2);
        assert_eq!(result.geometry.physical_fret_end, 6);
        assert_eq!(result.setup.instrument, Instrument::Ukulele);
        assert_eq!(result.concert_root, PitchClass::new(2));
        assert!(result.setup.concert_open_midi[0] > result.setup.concert_open_midi[1]);
        let tuning = Tuning::find_for("Standard (high-G)", Instrument::Ukulele).unwrap();
        for dot in &result.dots {
            assert!(dot.string_index < 4 && dot.fret >= 2 && dot.fret <= 6);
            let actual = Pitch::from_midi(
                tuning.strings[dot.string_index].midi() + i32::from(dot.fret),
                Spelling::Sharps,
            );
            assert!((dot.frequency - actual.frequency() as f32).abs() < 0.001);
        }
        assert_eq!(serde_json::to_value(&card).unwrap(), original);
    }

    #[test]
    fn display_and_sound_share_exact_pitch_facts_and_deterministic_contacts() {
        let state = StageState::new();
        let card = scale();
        let result = state.scale_card_realization(&card).unwrap();
        let displayed = result
            .dots
            .iter()
            .map(|dot| crate::pc_from_hz(dot.frequency))
            .collect::<BTreeSet<_>>();
        let (sound, _, _) = state.card_sounding_pitches(&card);
        let sounded = sound
            .iter()
            .map(|hz| crate::pc_from_hz(*hz))
            .collect::<BTreeSet<_>>();
        assert_eq!(displayed, sounded);
        assert!(
            result
                .notes
                .windows(2)
                .all(|pair| pair[0].pitch.midi() < pair[1].pitch.midi())
        );
        for note in &result.notes {
            assert!(
                result
                    .dots
                    .iter()
                    .any(|dot| dot.string_index == note.string_index
                        && dot.fret == note.physical_fret
                        && (dot.frequency - note.pitch.frequency() as f32).abs() < 0.001)
            );
        }
        assert_eq!(
            result
                .notes
                .iter()
                .map(|note| note.pitch.frequency() as f32)
                .collect::<Vec<_>>(),
            sound
        );
    }

    #[test]
    fn marks_use_resolved_physical_contacts_and_unknown_marks_do_not_substitute() {
        let state = StageState::new();
        let mut card = scale();
        let result = state.scale_card_realization(&card).unwrap();
        let target = &result.dots[0];
        card.setting.marked = vec![(target.string_index, target.fret)];
        card.setting.mark_mode = MarkMode::Solo;
        let sound = state.card_sounding_pitches(&card).0;
        assert_eq!(sound, vec![target.frequency]);
        card.setting.mark_mode = MarkMode::Mute;
        let sound = state.card_sounding_pitches(&card).0;
        assert!(
            sound
                .iter()
                .all(|hz| crate::pc_from_hz(*hz) != crate::pc_from_hz(target.frequency))
        );
        card.setting.marked = vec![(99, 99)];
        card.setting.mark_mode = MarkMode::Solo;
        assert!(state.card_sounding_pitches(&card).0.is_empty());
    }

    #[test]
    fn explicit_setup_and_stale_scale_fail_closed() {
        let state = StageState::new();
        let mut card = scale();
        card.setting.instrument = "Unknown lute".into();
        assert!(matches!(
            state.scale_card_realization(&card),
            Err(ScaleRealizationUnavailable::Setup(
                CardShapeUnavailable::UnknownInstrument(_)
            ))
        ));
        assert!(state.dots_for_card(&card).is_empty());
        assert!(state.card_sounding_pitches(&card).0.is_empty());
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Custom not persisted".into());
        assert!(matches!(
            state.scale_card_realization(&card),
            Err(ScaleRealizationUnavailable::Setup(
                CardShapeUnavailable::UnknownTuning(_)
            ))
        ));
        card.setting.tuning = None;
        assert!(state.scale_card_realization(&card).is_ok());
        card.material = Material::Scale {
            name: "Missing".into(),
            root: PitchClass::new(0),
        };
        assert!(matches!(
            state.scale_card_realization(&card),
            Err(ScaleRealizationUnavailable::UnknownScale(_))
        ));
    }

    #[test]
    fn capo_and_instrument_limits_bound_physical_window() {
        let state = StageState::new();
        let mut card = scale();
        card.setting.fret_window = Some(FretWindow { start: 0, span: 1 });
        assert!(matches!(
            state.scale_card_realization(&card),
            Err(ScaleRealizationUnavailable::Setup(
                CardShapeUnavailable::WindowBelowCapo
            ))
        ));
        card.setting.fret_window = Some(FretWindow { start: 20, span: 4 });
        assert!(matches!(
            state.scale_card_realization(&card),
            Err(ScaleRealizationUnavailable::Setup(
                CardShapeUnavailable::EmptyWindow
            ))
        ));
        card.setting.fret_window = Some(FretWindow { start: 14, span: 4 });
        assert_eq!(
            state
                .scale_card_realization(&card)
                .unwrap()
                .geometry
                .physical_fret_end,
            15
        );
        card.setting.capo = Some(16);
        assert!(state.scale_card_realization(&card).is_err());
    }

    #[test]
    fn legacy_setup_inherits_live_board_and_nonstandard_catalog_tuning_is_explicit() {
        let mut state = StageState::new();
        let mut card = scale();
        card.setting.instrument.clear();
        card.setting.tuning = None;
        card.setting.fret_window = None;
        state.fret_start = 3;
        state.fret_count = 8;
        let inherited = state.scale_card_realization(&card).unwrap();
        assert_eq!(inherited.setup.instrument, state.tuning().instrument);
        assert_eq!(inherited.geometry.physical_fret_start, 3);
        assert_eq!(inherited.geometry.physical_fret_end, 8);
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Baritone".into());
        let alternate = state.scale_card_realization(&card).unwrap();
        assert_eq!(alternate.setup.tuning_name, "Baritone");
        assert_eq!(alternate.geometry.string_count, 4);
    }

    #[test]
    fn walk_uses_tempo_and_fits_hold_with_inherited_clock() {
        let state = StageState::new();
        let mut card = scale();
        let (pitches, duration, offset) = state.card_sounding_pitches_at_tempo(&card, 90.0);
        assert!((offset - 666.6667).abs() < 0.01);
        assert!(duration > (pitches.len() - 1) as f32 * offset / 1000.0);
        card.timing.hold = Hold::Bars(1);
        let (pitches, duration, offset) = state.card_sounding_pitches_at_tempo(&card, 180.0);
        let dwell = crate::card_dwell(&card, 180.0).unwrap().as_secs_f32();
        assert!((duration - dwell).abs() < 0.00001);
        assert!((pitches.len() - 1) as f32 * offset < dwell * 1000.0);
        assert_eq!(card.timing.bpm, None);
    }
}
