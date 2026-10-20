//! Application context wrapper over woodshedding's shared notation conversion.
use crate::StageState;
use woodshedding::{card_shapes::ShapeContext, notation::Score, rehearsal::Set};
pub fn score_from_set(
    stage: &StageState,
    set: &Set,
    title: &str,
    fallback_bpm: f32,
) -> Result<Score, String> {
    if set
        .progression_arrangement
        .as_ref()
        .is_some_and(|a| !a.alignment.time_signatures.is_empty())
    {
        crate::saved_arrangements::saved_arrangement_stage(set)?;
    }
    let context = ShapeContext {
        tuning: stage.tuning(),
        fret_start: stage.fret_start,
        fret_count: stage.fret_count,
    };
    woodshedding::notation::draft::score_from_set(&context, set, title, fallback_bpm)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CardShapeStatus;
    use woodshedding::{
        notation::NoteValue,
        rehearsal::{Card, CardId, Material},
    };
    use woodshedding::{
        pitch::PitchClass,
        rehearsal::{
            FretWindow, Setting, Timing, Touch,
            arrangement::{
                ArrangementRest, CardAlignment, Placement, ProgressionArrangement, Span,
                TimeSignatureChange,
            },
        },
    };
    fn card(stage: &StageState, instrument: &str, tuning: &str, capo: u8) -> Card {
        let mut card = Card {
            id: CardId::UNASSIGNED,
            label: "Saved C".into(),
            material: Material::Chord {
                name: "Major".into(),
                root: PitchClass::new(0),
            },
            setting: Setting {
                instrument: instrument.into(),
                tuning: Some(tuning.into()),
                capo: Some(capo),
                fret_window: Some(FretWindow { start: 0, span: 5 }),
                ..Setting::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        };
        stage.select_card_shape_index(&mut card, 0).unwrap();
        card
    }
    #[test]
    fn saved_tuning_and_capo_contacts_reproduce_exact_concert_pitches() {
        let stage = StageState::new();
        for (instrument, tuning, capo) in [
            ("Guitar", "Standard", 2),
            ("Ukulele", "Standard (high-G)", 0),
        ] {
            let card = card(&stage, instrument, tuning, capo);
            let CardShapeStatus::Available(shape) = stage.card_shape_status(&card) else {
                panic!("resolved")
            };
            let mut set = Set::default();
            set.push(card);
            let score = score_from_set(&stage, &set, "Saved setup", 96.0).unwrap();
            assert_eq!(score.instrument, instrument);
            let event = &score.measures[0].events[0];
            let pitches: Vec<_> = event
                .notes
                .iter()
                .map(|n| i32::from(score.pitch(*n).unwrap()))
                .collect();
            assert_eq!(
                pitches,
                shape
                    .concert_pitches()
                    .iter()
                    .map(|p| p.midi())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                score
                    .tuning
                    .iter()
                    .copied()
                    .rev()
                    .map(i32::from)
                    .collect::<Vec<_>>(),
                shape
                    .setup
                    .concert_open_midi
                    .iter()
                    .map(|p| p - i32::from(capo))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                event
                    .notes
                    .iter()
                    .map(|n| usize::from(n.string))
                    .collect::<Vec<_>>(),
                shape
                    .physical_positions()
                    .iter()
                    .map(|(s, _)| shape.geometry.string_count - s)
                    .collect::<Vec<_>>()
            );
        }
    }
    fn aligned() -> (StageState, Set) {
        let mut stage = StageState::new();
        let index = stage
            .progressions()
            .iter()
            .position(|p| p.name == "ii-V-I (Jazz)")
            .unwrap();
        stage.select_progression(index);
        let mut set = Set::default();
        set.push(card(&stage, "Guitar", "Standard", 0));
        let alignment = ProgressionArrangement {
            name: "Half chord, half rest".into(),
            extent: 16,
            role_spans: vec![
                Span {
                    start: 0,
                    length: 16
                };
                3
            ],
            cards: vec![CardAlignment {
                card: set.cards[0].id,
                role: Some(0),
                placement: Some(Placement {
                    lane: 0,
                    span: Span {
                        start: 0,
                        length: 8,
                    },
                }),
            }],
            rests: vec![ArrangementRest {
                key: "rest".into(),
                role: None,
                placement: Placement {
                    lane: 0,
                    span: Span {
                        start: 8,
                        length: 8,
                    },
                },
            }],
            time_signatures: vec![TimeSignatureChange {
                start: 0,
                numerator: 4,
                denominator: 4,
            }],
            bar_groups: vec![],
        };
        crate::saved_arrangements::save_progression_arrangement(&stage, &mut set, alignment)
            .unwrap();
        (stage, set)
    }
    #[test]
    fn authored_half_chord_and_half_rest_preserve_spans() {
        let (stage, set) = aligned();
        let score = score_from_set(&stage, &set, "Authored", 96.0).unwrap();
        assert_eq!(score.measures.len(), 1);
        let events = &score.measures[0].events;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].value, NoteValue::Half);
        assert!(!events[0].rest);
        assert_eq!(events[1].value, NoteValue::Half);
        assert!(events[1].rest);
    }
    #[test]
    fn cross_bar_and_incomplete_or_overlapping_spans_refuse() {
        let (stage, mut set) = aligned();
        let a = &mut set.progression_arrangement.as_mut().unwrap().alignment;
        a.extent = 32;
        a.rests.clear();
        a.cards[0].placement.as_mut().unwrap().span = Span {
            start: 12,
            length: 8,
        };
        assert!(
            score_from_set(&stage, &set, "Cross bar", 96.0)
                .unwrap_err()
                .contains("crosses a barline")
        );
        let (stage, mut set) = aligned();
        set.progression_arrangement
            .as_mut()
            .unwrap()
            .alignment
            .extent = 15;
        assert!(score_from_set(&stage, &set, "Incomplete", 96.0).is_err());
        let (stage, mut set) = aligned();
        set.progression_arrangement
            .as_mut()
            .unwrap()
            .alignment
            .rests[0]
            .placement
            .span
            .start = 4;
        assert!(
            score_from_set(&stage, &set, "Overlap", 96.0)
                .unwrap_err()
                .contains("Overlapping")
        );
    }
    #[test]
    fn unresolved_cards_fail_atomically_and_no_meter_is_explicit_full_bar_draft() {
        let stage = StageState::new();
        let mut set = Set::default();
        set.push(card(&stage, "Guitar", "Standard", 0));
        assert_eq!(
            score_from_set(&stage, &set, "Draft", 96.0)
                .unwrap()
                .measures[0]
                .events[0]
                .value,
            NoteValue::Whole
        );
        let mut other = set.cards[0].clone();
        other.setting.voicing_idx = None;
        set.push(other);
        assert!(score_from_set(&stage, &set, "Refuse", 96.0).is_err());
        assert!(set.music.is_none());
    }
}
