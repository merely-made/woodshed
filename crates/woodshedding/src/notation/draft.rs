//! Explicit Set-to-notation drafting shared by hosts. No Card timing is inferred from dwell.
use crate::card_shapes::{CardShapeStatus, ShapeContext};
use crate::{
    notation::{
        ChordKind, ChordSymbol, FrettedNote, Measure, NoteEvent, NoteValue, Score, TimeSignature,
    },
    rehearsal::{Card, CardId, Material, Set, arrangement::bars},
};
use std::collections::{HashMap, HashSet};

/// Preserve an authored meter/alignment, or explicitly draft each Card as one
/// 4/4 bar. The caller labels that second action so it cannot imply playback
/// timing was recovered. Every Card must resolve before a score is returned.
pub fn score_from_set(
    stage: &ShapeContext,
    set: &Set,
    title: &str,
    fallback_bpm: f32,
) -> Result<Score, String> {
    if set.cards.is_empty() {
        return Err("Add a resolved chord Card before writing notation".into());
    }
    let authored = set
        .progression_arrangement
        .as_ref()
        .filter(|saved| !saved.alignment.time_signatures.is_empty());
    let saved_stage = if authored.is_some() {
        Some(ShapeContext::from_saved(set)?)
    } else {
        None
    };
    let context = saved_stage.as_ref().unwrap_or(stage);
    let mut score = Score {
        title: title.into(),
        instrument: String::new(),
        tuning: Vec::new(),
        tempo: 96,
        measures: Vec::new(),
    };
    let mut resolved = Vec::new();
    let mut tempo = None;
    for card in &set.cards {
        {
            let bpm = card.timing.bpm.unwrap_or(fallback_bpm);
            if !bpm.is_finite() || bpm.fract() != 0.0 || !(1.0..=400.0).contains(&bpm) {
                return Err(format!(
                    "Card '{}': notation requires a whole-number tempo from 1 to 400 BPM",
                    card.label
                ));
            }
            let bpm = bpm as u16;
            if tempo.is_some_and(|previous| previous != bpm) {
                return Err("This notation profile cannot flatten different Card tempos".into());
            }
            tempo = Some(bpm);
        }
        let (tuning, instrument, event, harmony) = resolve(context, card)?;
        if score.tuning.is_empty() {
            score.tuning = tuning;
            score.instrument = instrument;
        } else if score.tuning != tuning || score.instrument != instrument {
            return Err(
                "Cards use different instrument setups; one score requires one exact tuning".into(),
            );
        }
        resolved.push((event, harmony));
    }
    score.tempo = tempo.ok_or("No tempo was resolved")?;
    if let Some(saved) = authored {
        let alignment = &saved.alignment;
        let derived = bars::bars(alignment)?;
        if derived.iter().any(|bar| !bar.complete) {
            return Err(
                "Incomplete authored bars cannot be written by this notation profile".into(),
            );
        }
        let by_id: HashMap<CardId, usize> = set
            .cards
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let mut seen = HashSet::new();
        let mut spans = Vec::new();
        for binding in &alignment.cards {
            let index = *by_id
                .get(&binding.card)
                .ok_or("An aligned Card is missing from this Set")?;
            if !seen.insert(binding.card) {
                return Err("A Card has multiple authored placements".into());
            }
            let placement = binding
                .placement
                .as_ref()
                .ok_or("Every Card needs a complete authored placement")?;
            if placement.lane != 0 {
                return Err(
                    "Multiple arrangement lanes are not supported in this score profile".into(),
                );
            }
            let mut event = resolved[index].0.clone();
            event.value = value(placement.span.length)?;
            spans.push((placement.span, event, Some(resolved[index].1)));
        }
        if seen.len() != set.cards.len() {
            return Err(
                "Every Set Card must have an authored placement; no Cards were written".into(),
            );
        }
        for rest in &alignment.rests {
            if rest.placement.lane != 0 {
                return Err(
                    "Multiple arrangement lanes are not supported in this score profile".into(),
                );
            }
            spans.push((
                rest.placement.span,
                NoteEvent {
                    notes: vec![],
                    value: value(rest.placement.span.length)?,
                    rest: true,
                },
                None,
            ));
        }
        spans.sort_by_key(|(span, _, _)| span.start);
        let mut end = 0;
        for (span, _, _) in &spans {
            let next = span
                .start
                .checked_add(span.length)
                .ok_or("Authored span overflows")?;
            if span.start < end {
                return Err(
                    "Overlapping Card/rest spans cannot be flattened into one voice".into(),
                );
            }
            if !derived
                .iter()
                .any(|bar| span.start >= bar.start && next <= bar.start + bar.length)
            {
                return Err("A Card/rest span crosses a barline or the arrangement extent; split it explicitly".into());
            }
            end = next;
        }
        for bar in derived {
            let meter = alignment
                .time_signatures
                .iter()
                .rev()
                .find(|m| m.start <= bar.start)
                .ok_or("Authored meter is missing")?;
            let mut measure = Measure {
                meter: TimeSignature {
                    beats: meter.numerator,
                    beat_type: meter.denominator,
                },
                harmony: None,
                events: vec![],
            };
            let mut cursor = bar.start;
            for (span, event, harmony) in spans.iter().filter(|(span, _, _)| {
                span.start >= bar.start && span.start < bar.start + bar.length
            }) {
                rests(&mut measure.events, span.start - cursor)?;
                if let Some(harmony) = harmony {
                    if measure.harmony.is_some_and(|previous| previous != *harmony) {
                        return Err("More than one different chord symbol in a measure requires a richer notation profile".into());
                    }
                    measure.harmony = Some(*harmony);
                }
                measure.events.push(event.clone());
                cursor = span.start + span.length;
            }
            rests(&mut measure.events, bar.start + bar.length - cursor)?;
            score.measures.push(measure);
        }
    } else {
        for (event, harmony) in resolved {
            score.measures.push(Measure {
                meter: TimeSignature {
                    beats: 4,
                    beat_type: 4,
                },
                harmony: Some(harmony),
                events: vec![event],
            });
        }
    }
    score.validate()?;
    Ok(score)
}

fn resolve(
    stage: &ShapeContext,
    card: &Card,
) -> Result<(Vec<u8>, String, NoteEvent, ChordSymbol), String> {
    let Material::Chord { name, root } = &card.material else {
        return Err(format!(
            "Card '{}': only chord material can be written in this first notation profile",
            card.label
        ));
    };
    let kind = match name.as_str() {
        "Major" => ChordKind::Major,
        "Minor" => ChordKind::Minor,
        "Minor 7" => ChordKind::Minor7,
        "Dominant 7" => ChordKind::Dominant7,
        "Major 7" => ChordKind::Major7,
        "Diminished" => ChordKind::Diminished,
        "Augmented" => ChordKind::Augmented,
        "Suspended 2nd" => ChordKind::Sus2,
        "Suspended 4th" => ChordKind::Sus4,
        _ => return Err(format!("Chord '{name}' is outside this notation profile")),
    };
    let shape = match stage.selected_card_shape(card) {
        CardShapeStatus::Available(shape) => shape,
        CardShapeStatus::Unavailable(reason) => {
            return Err(format!("Card '{}': {reason}", card.label));
        },
        _ => {
            return Err(format!(
                "Card '{}': select a concrete chord shape before writing notation",
                card.label
            ));
        },
    };
    let mut tuning: Vec<u8> = shape
        .setup
        .concert_open_midi
        .iter()
        .map(|open| {
            u8::try_from(*open - i32::from(shape.geometry.capo))
                .map_err(|_| "Resolved nut tuning is outside MIDI range".to_string())
        })
        .collect::<Result<_, _>>()?;
    tuning.reverse();
    let notes: Vec<_> = shape
        .physical_positions()
        .into_iter()
        .map(|(string, fret)| {
            Ok(FrettedNote {
                string: u8::try_from(tuning.len() - string)
                    .map_err(|_| "Too many strings for this notation profile".to_string())?,
                fret,
            })
        })
        .collect::<Result<_, String>>()?;
    let actual: Vec<_> = notes
        .iter()
        .map(|n| i32::from(tuning[usize::from(n.string - 1)]) + i32::from(n.fret))
        .collect();
    let expected: Vec<_> = shape.concert_pitches().iter().map(|p| p.midi()).collect();
    if actual != expected {
        return Err(
            "Resolved physical contacts do not reproduce the selected concert pitches".into(),
        );
    }
    Ok((
        tuning,
        shape.instrument.to_string(),
        NoteEvent {
            notes,
            value: NoteValue::Whole,
            rest: false,
        },
        ChordSymbol {
            root: (root.value() + shape.geometry.capo) % 12,
            kind,
        },
    ))
}

fn value(ticks: u32) -> Result<NoteValue, String> {
    match ticks {
        16 => Ok(NoteValue::Whole),
        8 => Ok(NoteValue::Half),
        4 => Ok(NoteValue::Quarter),
        2 => Ok(NoteValue::Eighth),
        _ => Err(format!(
            "Authored {ticks}-tick span has no supported exact note value; no timing was quantized"
        )),
    }
}
fn rests(events: &mut Vec<NoteEvent>, mut ticks: u32) -> Result<(), String> {
    if ticks % 2 != 0 {
        return Err("A gap falls between supported eighth-note boundaries".into());
    }
    for length in [16, 8, 4, 2] {
        while ticks >= length {
            events.push(NoteEvent {
                notes: vec![],
                value: value(length)?,
                rest: true,
            });
            ticks -= length;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        notation::MusicDocument,
        pitch::PitchClass,
        rehearsal::{Setting, Timing, Touch},
        tuning::{Instrument, Tuning},
    };

    fn resolved_set() -> (ShapeContext, Set) {
        let context = ShapeContext {
            tuning: Tuning::find_for("Standard", Instrument::Guitar).unwrap(),
            fret_start: 0,
            fret_count: 5,
        };
        let mut set = Set::default();
        set.push(Card {
            id: CardId::UNASSIGNED,
            label: "C".into(),
            material: Material::Chord {
                name: "Major".into(),
                root: PitchClass::new(0),
            },
            setting: Setting {
                instrument: "Guitar".into(),
                tuning: Some("Standard".into()),
                voicing_idx: Some(0),
                ..Setting::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        });
        (context, set)
    }

    #[test]
    fn full_written_tempo_range_round_trips_from_fallback_and_card() {
        let (context, mut set) = resolved_set();
        for bpm in [1, 19, 20, 400] {
            set.cards[0].timing.bpm = None;
            let score = score_from_set(&context, &set, "Tempo", f32::from(bpm)).unwrap();
            assert_eq!(score.tempo, bpm);
            assert_eq!(
                MusicDocument::from_score(&score).unwrap().score().unwrap(),
                score
            );
            set.cards[0].timing.bpm = Some(f32::from(bpm));
            let score = score_from_set(&context, &set, "Tempo", 96.0).unwrap();
            assert_eq!(score.tempo, bpm);
            assert_eq!(
                MusicDocument::from_score(&score).unwrap().score().unwrap(),
                score
            );
        }
    }

    #[test]
    fn invalid_written_tempos_are_rejected_from_fallback_and_card() {
        let (context, mut set) = resolved_set();
        for bpm in [-1.0, 0.0, 0.5, 1.5, 400.5, 401.0, f32::NAN, f32::INFINITY] {
            set.cards[0].timing.bpm = None;
            assert!(
                score_from_set(&context, &set, "Tempo", bpm)
                    .unwrap_err()
                    .contains("whole-number tempo from 1 to 400 BPM")
            );
            set.cards[0].timing.bpm = Some(bpm);
            assert!(
                score_from_set(&context, &set, "Tempo", 96.0)
                    .unwrap_err()
                    .contains("whole-number tempo from 1 to 400 BPM")
            );
        }
    }
}
