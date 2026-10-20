//! Disposable score-derived practice timing. MusicXML remains musical authority;
//! this module owns neither transport state nor audio/device preparation.
use woodshedding::notation::{FrettedNote, Score};

/// Quarter-note tick scale shared with the existing generic output program.
pub const TICKS_PER_QUARTER: u64 = 960;
/// One score measure becomes one generic program segment (engine limit: 256).
pub const MAX_PLAYBACK_MEASURES: usize = 256;
/// Each sounding contact becomes one generic program event (engine limit: 4096).
pub const MAX_PLAYBACK_NOTES: usize = 4096;
/// Also bound logical events, including rests, before allocating a schedule.
pub const MAX_PLAYBACK_EVENTS: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub struct ArrangementSchedule {
    /// Applied practice tempo; lowering never changes the score's authored tempo.
    pub tempo_bpm: f64,
    pub measures: Vec<PlaybackMeasure>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackMeasure {
    pub measure_index: usize,
    pub duration_ticks: u64,
    pub events: Vec<PlaybackEvent>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackEvent {
    pub event_index: usize,
    pub onset_ticks: u64,
    pub gate_ticks: u64,
    /// Empty for an authored rest. Chord contacts retain their authored order.
    pub notes: Vec<PlaybackNote>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackNote {
    /// Physical contact uses the score's highest-first string numbering.
    pub contact: FrettedNote,
    pub midi: u8,
    pub frequency_hz: f64,
}
impl PlaybackEvent {
    pub fn is_rest(&self) -> bool {
        self.notes.is_empty()
    }
}
impl ArrangementSchedule {
    pub fn from_score(score: &Score, tempo_bpm: f64) -> Result<Self, String> {
        // Bound vector walks before full validation; reject the entire score,
        // never partially lower a prefix or omit a problematic event.
        if score.measures.is_empty() || score.measures.len() > MAX_PLAYBACK_MEASURES {
            return Err("Arrangement playback requires 1..=256 measures.".into());
        }
        let mut events = 0usize;
        let mut notes = 0usize;
        for measure in &score.measures {
            events = events
                .checked_add(measure.events.len())
                .ok_or("Playback event count overflow.")?;
            if events > MAX_PLAYBACK_EVENTS {
                return Err("Arrangement exceeds 4096 playback events including rests.".into());
            }
            for event in &measure.events {
                notes = notes
                    .checked_add(event.notes.len())
                    .ok_or("Playback note count overflow.")?;
                if notes > MAX_PLAYBACK_NOTES {
                    return Err("Arrangement exceeds 4096 sounding playback contacts.".into());
                }
            }
        }
        if !tempo_bpm.is_finite() || !(1.0..=400.0).contains(&tempo_bpm) {
            return Err(
                "Practice tempo must be finite and within 1..=400 quarter notes per minute.".into(),
            );
        }
        score.validate()?;
        let mut measures = Vec::with_capacity(score.measures.len());
        for (measure_index, measure) in score.measures.iter().enumerate() {
            let numerator = u64::from(measure.meter.beats)
                .checked_mul(4)
                .and_then(|n| n.checked_mul(TICKS_PER_QUARTER))
                .ok_or("Measure duration overflow.")?;
            let denominator = u64::from(measure.meter.beat_type);
            if denominator == 0 || numerator % denominator != 0 {
                return Err("Meter cannot be represented exactly on the practice clock.".into());
            }
            let duration_ticks = numerator / denominator;
            let mut onset_ticks = 0u64;
            let mut events = Vec::with_capacity(measure.events.len());
            for (event_index, event) in measure.events.iter().enumerate() {
                // NoteValue uses six exact integer units per quarter, including
                // triplet eighths. Never derive clock timing from floats.
                let numerator = u64::from(event.value.units())
                    .checked_mul(TICKS_PER_QUARTER)
                    .ok_or("Event duration overflow.")?;
                if numerator % 6 != 0 {
                    return Err(
                        "Note duration cannot be represented exactly on the practice clock.".into(),
                    );
                }
                let gate_ticks = numerator / 6;
                let end = onset_ticks
                    .checked_add(gate_ticks)
                    .filter(|end| gate_ticks > 0 && *end <= duration_ticks)
                    .ok_or("Event gate exceeds its measure.")?;
                let mut notes = Vec::with_capacity(event.notes.len());
                for contact in &event.notes {
                    let midi = score.pitch(*contact)?;
                    let frequency_hz = 440.0 * 2.0_f64.powf((f64::from(midi) - 69.0) / 12.0);
                    if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
                        return Err("Invalid playback frequency.".into());
                    }
                    notes.push(PlaybackNote {
                        contact: *contact,
                        midi,
                        frequency_hz,
                    });
                }
                events.push(PlaybackEvent {
                    event_index,
                    onset_ticks,
                    gate_ticks,
                    notes,
                });
                onset_ticks = end;
            }
            if onset_ticks != duration_ticks {
                return Err("Playback events do not fill their measure exactly.".into());
            }
            measures.push(PlaybackMeasure {
                measure_index,
                duration_ticks,
                events,
            });
        }
        Ok(Self {
            tempo_bpm,
            measures,
        })
    }
    /// Select the authored event at a bar-local quarter tick, including rests.
    /// Events are half-open intervals; a bar's end is outside that bar.
    pub fn event_at(&self, measure_index: usize, beat_ticks: u64) -> Option<&PlaybackEvent> {
        let measure = self.measures.get(measure_index)?;
        if beat_ticks >= measure.duration_ticks {
            return None;
        }
        let index = measure
            .events
            .partition_point(|event| event.onset_ticks <= beat_ticks)
            .checked_sub(1)?;
        let event = measure.events.get(index)?;
        (beat_ticks < event.onset_ticks.checked_add(event.gate_ticks)?).then_some(event)
    }
}
/// Lower one complete score without changing musical authority.
pub fn lower_score(score: &Score, tempo_bpm: f64) -> Result<ArrangementSchedule, String> {
    ArrangementSchedule::from_score(score, tempo_bpm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::notation::{Measure, NoteEvent, NoteValue, TimeSignature};
    #[test]
    fn progression_keeps_chords_and_override_is_disposable() {
        let score = Score::progression_example();
        let before = score.clone();
        let plan = lower_score(&score, 73.5).unwrap();
        assert_eq!(plan.tempo_bpm, 73.5);
        assert_eq!(score, before);
        assert_eq!(plan.measures.len(), 4);
        for (bar, measure) in plan.measures.iter().enumerate() {
            assert_eq!(measure.measure_index, bar);
            assert_eq!(measure.duration_ticks, 3840);
            let e = &measure.events[0];
            assert_eq!((e.onset_ticks, e.gate_ticks), (0, 3840));
            assert_eq!(e.notes.len(), score.measures[bar].events[0].notes.len());
            for n in &e.notes {
                assert_eq!(score.pitch(n.contact).unwrap(), n.midi);
                assert!(n.frequency_hz > 0.0);
            }
        }
    }
    #[test]
    fn rhythm_gates_triplets_and_rest_lookup_are_exact() {
        let p = lower_score(&Score::rhythm_example(), 96.0).unwrap();
        let bar = &p.measures[0];
        assert_eq!(
            bar.events.iter().map(|e| e.onset_ticks).collect::<Vec<_>>(),
            [0, 960, 1440, 1920, 2880]
        );
        assert_eq!(
            bar.events.iter().map(|e| e.gate_ticks).collect::<Vec<_>>(),
            [960, 480, 480, 960, 960]
        );
        assert!(!p.event_at(0, 2879).unwrap().is_rest());
        assert!(p.event_at(0, 2880).unwrap().is_rest());
        assert!(p.event_at(0, 3839).unwrap().is_rest());
        assert!(p.event_at(0, 3840).is_none());
        assert!(p.event_at(2, 0).is_none());
        assert_eq!(
            p.measures[1]
                .events
                .iter()
                .take(3)
                .map(|e| e.gate_ticks)
                .collect::<Vec<_>>(),
            [320, 320, 320]
        );
        assert_eq!(p.event_at(1, 319).unwrap().event_index, 0);
        assert_eq!(p.event_at(1, 320).unwrap().event_index, 1);
        assert_eq!(p.event_at(1, 960).unwrap().event_index, 3);
    }
    #[test]
    fn mixed_meters_and_reentrant_four_strings_preserve_physical_contacts() {
        let mut s = Score::rhythm_example();
        s.tuning = vec![69, 64, 60, 67];
        let e = NoteEvent {
            notes: vec![FrettedNote { string: 4, fret: 0 }],
            value: NoteValue::Quarter,
            rest: false,
        };
        s.measures = vec![
            Measure {
                meter: TimeSignature {
                    beats: 4,
                    beat_type: 4,
                },
                harmony: None,
                events: vec![e.clone(); 4],
            },
            Measure {
                meter: TimeSignature {
                    beats: 3,
                    beat_type: 4,
                },
                harmony: None,
                events: vec![e.clone(); 3],
            },
            Measure {
                meter: TimeSignature {
                    beats: 6,
                    beat_type: 8,
                },
                harmony: None,
                events: vec![
                    NoteEvent {
                        value: NoteValue::Eighth,
                        ..e
                    };
                    6
                ],
            },
        ];
        let p = lower_score(&s, 100.0).unwrap();
        assert_eq!(
            p.measures
                .iter()
                .map(|m| m.duration_ticks)
                .collect::<Vec<_>>(),
            [3840, 2880, 2880]
        );
        for m in &p.measures {
            for e in &m.events {
                assert_eq!(e.notes[0].midi, 67);
                assert_eq!(e.notes[0].contact.string, 4)
            }
        }
    }
    #[test]
    fn invalid_score_tempo_and_resource_exhaustion_are_atomic_refusals() {
        let mut s = Score::rhythm_example();
        for bpm in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -10.0,
            0.5,
            401.0,
        ] {
            assert!(lower_score(&s, bpm).is_err());
        }
        s.measures[1].events[0].notes[0].string = 99;
        assert!(lower_score(&s, 96.0).is_err());
        s = Score::rhythm_example();
        s.measures[0].events.pop();
        assert!(lower_score(&s, 96.0).is_err());
        s = Score::progression_example();
        s.measures = vec![s.measures[0].clone(); 257];
        assert!(lower_score(&s, 96.0).is_err());
        s = Score::progression_example();
        let e = NoteEvent {
            notes: s.measures[0].events[0].notes.clone(),
            value: NoteValue::Eighth,
            rest: false,
        };
        s.measures[0].events = vec![e; 8];
        s.measures = vec![s.measures[0].clone(); 129];
        assert!(lower_score(&s, 96.0).unwrap_err().contains("contacts"));
    }
    #[test]
    fn all_rest_bar_keeps_authored_event_selection_and_timing() {
        let mut score = Score::rhythm_example();
        for event in &mut score.measures[0].events {
            event.notes.clear();
            event.rest = true;
        }
        let before = score.clone();
        let schedule = lower_score(&score, 88.0).unwrap();
        assert_eq!(score, before);
        assert_eq!(schedule.measures[0].events.len(), 5);
        assert!(
            schedule.measures[0]
                .events
                .iter()
                .all(PlaybackEvent::is_rest)
        );
        assert_eq!(schedule.event_at(0, 1440).unwrap().event_index, 2);
        assert_eq!(schedule.event_at(0, 2880).unwrap().gate_ticks, 960);
        assert!(schedule.event_at(0, 3840).is_none());
    }
    #[test]
    fn absolute_frequency_uses_sounding_pitch_not_guitar_written_octave() {
        let mut s = Score::progression_example();
        s.tuning = vec![69];
        for m in &mut s.measures {
            m.events[0].notes = vec![FrettedNote { string: 1, fret: 0 }];
        }
        let p = lower_score(&s, 1.0).unwrap();
        assert_eq!(p.measures[0].events[0].notes[0].frequency_hz, 440.0);
    }
}
