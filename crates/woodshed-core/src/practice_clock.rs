//! Disposable, pure lowering of a retained Set or live Stage sequence into
//! occurrence-addressed practice events. Hosts may schedule this program, but
//! it does not author or mutate the Set and is not persisted.

use woodshedding::{
    exercise::ExerciseParams,
    fretboard::Fretboard,
    pitch::PitchClass,
    rehearsal::{Card, CardId, Hold, MarkMode, Material, Set, Setting, Timing, Touch},
};

use crate::{ArpeggioRun, StageState, arpeggio::generate_shapes, card_shapes::CardShapeStatus};

/// The temporal unit for a card's held phrase. Events use the matching unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PracticeHold {
    /// Authored bars/repetitions resolve against quarter-note beats.
    Beats(f64),
    /// Authored seconds remain absolute across tempo edits.
    Seconds(f64),
    /// One phrase is played; the Set waits for explicit advancement.
    Manual {
        phrase_beats: f64,
        tail_seconds: f64,
    },
}

/// One audible occurrence. Ordinal is identity within the derived program;
/// repeated pitches and repeated physical contacts remain distinct events.
#[derive(Clone, Debug, PartialEq)]
pub struct PracticeEvent {
    pub ordinal: u32,
    /// Relative onset, in beats for Beats/Manual or seconds for Seconds.
    pub at: f64,
    /// Internal chord-tone onset spread for a grouped event, preserving the
    /// existing block-chord strum articulation. Sequential events use zero.
    pub strum_seconds: f64,
    /// A block/chord may contain several pitches; a walk contains one.
    pub pitches: Vec<f32>,
    /// Physical `(string_index, nut-relative fret)` contacts for this event.
    pub contacts: Vec<(usize, u8)>,
}

#[derive(Clone, Copy, Debug)]
struct NoteVisit {
    pitch: f32,
    contact: (usize, u8),
}

/// One retained Card and its derived playback. Unavailable material stays in
/// place with an explanation instead of disappearing from the program.
#[derive(Clone, Debug)]
pub struct PracticeCard {
    pub card: Card,
    pub bpm: f32,
    pub hold: PracticeHold,
    pub events: Vec<PracticeEvent>,
    pub unavailable: Option<String>,
}

/// A snapshot of a Set's current derived sequence. It is not durable truth.
#[derive(Clone, Debug, Default)]
pub struct PracticeProgram {
    pub cards: Vec<PracticeCard>,
}

/// The three transient one-note Stage clocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveSequenceKind {
    Arpeggio,
    Exercise,
    ScaleRun,
}

impl StageState {
    /// Lower all Cards in Set order. Each card resolves using its retained
    /// setup and authored timing, with `fallback_bpm` used only when the Card
    /// has no BPM of its own.
    pub fn practice_program(&self, set: &Set, fallback_bpm: f32) -> PracticeProgram {
        PracticeProgram {
            cards: set
                .cards
                .iter()
                .map(|card| self.lower_card(card, fallback_bpm))
                .collect(),
        }
    }

    /// Lower one transient Stage clock to its ordered contacts. `bpm` is
    /// supplied by the host because tempo is not part of StageState.
    pub fn live_clock_card(&self, kind: LiveSequenceKind, bpm: f32) -> PracticeCard {
        let card = self.live_card_template(kind);
        let events = self.live_events(kind);
        let unavailable = if !bpm.is_finite() || bpm <= 0.0 {
            Some("Live sequence requires a finite positive tempo".to_string())
        } else if events.is_empty() {
            Some("Live sequence has no playable contacts".to_string())
        } else {
            None
        };
        PracticeCard {
            card,
            bpm,
            hold: PracticeHold::Manual {
                phrase_beats: events.len().saturating_sub(1) as f64,
                tail_seconds: 0.0,
            },
            events: if unavailable.is_some() {
                Vec::new()
            } else {
                events
            },
            unavailable,
        }
    }

    fn lower_card(&self, card: &Card, fallback_bpm: f32) -> PracticeCard {
        let bpm = card.timing.bpm.unwrap_or(fallback_bpm);
        let failed = |reason: String| PracticeCard {
            card: card.clone(),
            bpm,
            hold: PracticeHold::Manual {
                phrase_beats: 0.0,
                tail_seconds: 0.0,
            },
            events: Vec::new(),
            unavailable: Some(reason),
        };

        if let Err(error) = crate::resolve_card_timing(&card.timing, fallback_bpm) {
            return failed(error.to_string());
        }
        // Riff Cards intentionally remain silent in the existing preview
        // contract. The Exercise lens has its own live sequence lowering.
        if matches!(card.material, Material::Riff { .. }) {
            if let Material::Riff { name } = &card.material {
                if !woodshedding::exercise::catalog()
                    .iter()
                    .any(|exercise| exercise.name == name)
                {
                    return failed(format!("Unknown exercise: {name}"));
                }
            }
            return PracticeCard {
                card: card.clone(),
                bpm,
                hold: authored_hold(card),
                events: Vec::new(),
                unavailable: None,
            };
        }

        let mut visits = match self.card_visits(card) {
            Ok(visits) => visits,
            Err(error) => return failed(error),
        };
        if visits.is_empty() {
            return failed("Card realization produced no playable notes".to_string());
        }
        self.apply_card_marks(card, &mut visits);
        self.apply_card_touch(card, &mut visits);
        if visits.is_empty() {
            return failed("Card marks or touch produced no playable notes".to_string());
        }

        let (expected_pitches, duration, strum_ms) =
            match self.card_sounding_pitches_at_tempo(card, fallback_bpm) {
                Ok(preview) => preview,
                Err(error) => return failed(error.to_string()),
            };
        let sequential = matches!(card.material, Material::Path { .. })
            || matches!(card.touch, Touch::Walk | Touch::Arpeggiate { .. });
        let realized_pitches = visits.iter().map(|visit| visit.pitch).collect::<Vec<_>>();
        if !same_pitches(&realized_pitches, &expected_pitches) {
            return failed(
                "Contact realization does not match the existing Card preview pitch order".into(),
            );
        }
        let groups = if sequential {
            visits.iter().map(|visit| vec![*visit]).collect::<Vec<_>>()
        } else {
            vec![visits]
        };

        let mut events = Vec::with_capacity(groups.len());
        for (ordinal, group) in groups.into_iter().enumerate() {
            events.push(PracticeEvent {
                ordinal: ordinal as u32,
                at: 0.0,
                strum_seconds: if sequential {
                    0.0
                } else {
                    f64::from(strum_ms) / 1000.0
                },
                pitches: group.iter().map(|visit| visit.pitch).collect(),
                contacts: group.iter().map(|visit| visit.contact).collect(),
            });
        }

        let hold = match authored_hold(card) {
            PracticeHold::Beats(total) => {
                set_even_onsets(&mut events, total);
                PracticeHold::Beats(total)
            },
            PracticeHold::Seconds(total) => {
                set_even_onsets(&mut events, total);
                PracticeHold::Seconds(total)
            },
            PracticeHold::Manual { .. } if sequential => {
                for event in &mut events {
                    event.at = f64::from(event.ordinal);
                }
                let phrase_beats = events.len().saturating_sub(1) as f64;
                PracticeHold::Manual {
                    phrase_beats,
                    // ordered_preview's sequential visit model reserves a
                    // fixed 400ms final tail. Keep that authored articulation
                    // invariant across tempo changes.
                    tail_seconds: 0.4,
                }
            },
            PracticeHold::Manual { .. } => PracticeHold::Manual {
                phrase_beats: 0.0,
                tail_seconds: f64::from(duration),
            },
        };

        PracticeCard {
            card: card.clone(),
            bpm,
            hold,
            events,
            unavailable: None,
        }
    }

    fn card_visits(&self, card: &Card) -> Result<Vec<NoteVisit>, String> {
        match &card.material {
            Material::Scale { .. } | Material::ScalePattern { .. } => {
                let scale = self
                    .scale_card_realization(card)
                    .map_err(|error| error.to_string())?;
                Ok(scale
                    .notes
                    .into_iter()
                    .map(|note| NoteVisit {
                        pitch: note.pitch.frequency() as f32,
                        contact: (note.string_index, note.physical_fret),
                    })
                    .collect())
            },
            Material::ChordApproach { .. } => {
                let approach = self
                    .chord_approach_realization(card)
                    .map_err(|error| error.to_string())?;
                Ok(approach
                    .notes
                    .into_iter()
                    .map(|note| NoteVisit {
                        pitch: note.pitch.frequency() as f32,
                        contact: (note.string_index, note.physical_fret),
                    })
                    .collect())
            },
            Material::Chord { name, root } => self.chord_visits(card, name, *root),
            Material::Riff { .. } => Ok(Vec::new()),
            Material::Path { .. } => Ok(self
                .path_contact_pitches(card)?
                .into_iter()
                .map(|(contact, pitch)| NoteVisit {
                    pitch: pitch.frequency() as f32,
                    contact,
                })
                .collect()),
        }
    }

    fn chord_visits(
        &self,
        card: &Card,
        name: &str,
        root: PitchClass,
    ) -> Result<Vec<NoteVisit>, String> {
        let formula = woodshedding::chord::catalog()
            .iter()
            .find(|formula| formula.name == name)
            .ok_or_else(|| format!("Unknown chord: {name}"))?;
        if card.setting.voicing_idx.is_some() {
            let shape = match self.card_shape_status(card) {
                CardShapeStatus::Available(shape) => shape,
                CardShapeStatus::Unavailable(reason) => return Err(reason.to_string()),
                CardShapeStatus::NotChord => return Err("Card is not chord material".into()),
                CardShapeStatus::Unselected => return Err("Card shape is not selected".into()),
            };
            return Ok(shape
                .physical_positions()
                .into_iter()
                .zip(shape.concert_pitches())
                .map(|(contact, pitch)| NoteVisit {
                    pitch: pitch.frequency() as f32,
                    contact,
                })
                .collect());
        }

        // An unselected formula chord is the existing abstract voicing. Give
        // each formula tone one deterministic exact physical realization in
        // the Card's saved window; never choose a nearest pitch/contact.
        let tuning = self
            .resolve_card_tuning(card)
            .map_err(|error| error.to_string())?;
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
        let start = start.max(card.setting.capo.unwrap_or(0));
        if start > end {
            return Err("Chord setup has no physical contacts in its fret window".into());
        }
        let board = Fretboard::new(tuning, end);
        let root_pitch = woodshedding::pitch::Pitch::from_midi(
            48 + i32::from(root.value()),
            woodshedding::pitch::Spelling::Sharps,
        );
        let expected = formula
            .apply_to(root_pitch)
            .map_err(|error| error.to_string())?;
        let positions = board
            .positions_for_chord(formula, root_pitch)
            .map_err(|error| error.to_string())?;
        let mut visits = Vec::with_capacity(expected.len());
        for (index, pitch) in expected.into_iter().enumerate() {
            let interval = formula.intervals.get(index).copied();
            let contact = positions
                .iter()
                .filter(|position| {
                    position.fret >= start
                        && position.fret <= end
                        && position.interval_from_root == interval
                        && position.pitch.midi() == pitch.midi()
                })
                .min_by_key(|position| (position.fret, position.string_index))
                .map(|position| (position.string_index, position.fret))
                .ok_or_else(|| {
                    format!("Chord tone {} has no exact fretboard contact", pitch.midi())
                })?;
            visits.push(NoteVisit {
                pitch: pitch.frequency() as f32,
                contact,
            });
        }
        Ok(visits)
    }

    fn apply_card_marks(&self, card: &Card, visits: &mut Vec<NoteVisit>) {
        if card.setting.marked.is_empty() || card.setting.mark_mode == MarkMode::Off {
            return;
        }
        let marked = card
            .setting
            .marked
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let dots = self.card_contact_pitches(card).unwrap_or_default();
        match card.setting.mark_mode {
            MarkMode::Off => {},
            MarkMode::Mute => {
                let muted = dots
                    .iter()
                    .filter(|(contact, _)| marked.contains(contact))
                    .map(|(_, frequency)| crate::pc_from_hz(*frequency))
                    .collect::<std::collections::BTreeSet<_>>();
                visits.retain(|visit| !muted.contains(&crate::pc_from_hz(visit.pitch)));
            },
            MarkMode::Solo
                if matches!(&card.material, Material::Chord { .. } if card.setting.voicing_idx.is_none())
                    || matches!(&card.material, Material::Scale { .. }) =>
            {
                // These legacy previews define Solo as the selected fretboard
                // contacts, sorted/deduplicated by pitch. The contact remains
                // the authored selected physical position.
                let mut selected = dots
                    .into_iter()
                    .filter(|(contact, _)| marked.contains(contact))
                    .map(|(contact, frequency)| NoteVisit {
                        pitch: frequency,
                        contact,
                    })
                    .collect::<Vec<_>>();
                selected.sort_by(|a, b| a.pitch.total_cmp(&b.pitch));
                selected.dedup_by(|a, b| (a.pitch - b.pitch).abs() < 0.02);
                *visits = selected;
            },
            MarkMode::Solo => {
                let selected = dots
                    .into_iter()
                    .filter(|(contact, _)| marked.contains(contact))
                    .map(|(contact, frequency)| NoteVisit {
                        pitch: frequency,
                        contact,
                    })
                    .collect::<Vec<_>>();
                let is_pattern = matches!(&card.material, Material::ScalePattern { .. });
                visits.retain_mut(|visit| {
                    // Pattern preview filters repeated recipe visits by the
                    // pitches found at marked contacts. It does not replace a
                    // recipe's physical contact with another equal-pitch dot.
                    if is_pattern {
                        selected
                            .iter()
                            .any(|marked| same_pitch(marked.pitch, visit.pitch))
                    } else {
                        marked.contains(&visit.contact)
                    }
                });
            },
        }
    }

    fn apply_card_touch(&self, card: &Card, visits: &mut Vec<NoteVisit>) {
        let Touch::Arpeggiate {
            direction,
            inversion,
        } = card.touch
        else {
            return;
        };
        visits.retain(|visit| visit.pitch.is_finite() && visit.pitch > 0.0);
        visits.sort_by(|a, b| a.pitch.total_cmp(&b.pitch));
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
                    let bass = (i32::from(root.value()) + capo + interval.semitones())
                        .rem_euclid(12) as u8;
                    if let Some(start) = visits
                        .iter()
                        .position(|visit| crate::pc_from_hz(visit.pitch) == bass)
                    {
                        visits.drain(..start);
                    }
                }
            }
        }
        match direction {
            crate::ArpeggioDirection::Up => {},
            crate::ArpeggioDirection::Down => visits.reverse(),
            crate::ArpeggioDirection::UpDown => {
                let back = visits
                    .iter()
                    .skip(1)
                    .take(visits.len().saturating_sub(2))
                    .rev()
                    .copied()
                    .collect::<Vec<_>>();
                visits.extend(back);
            },
        }
    }

    fn live_card_template(&self, kind: LiveSequenceKind) -> Card {
        let mut card = self.card_from_lens().unwrap_or_else(|| Card {
            id: CardId::UNASSIGNED,
            label: "Live sequence".into(),
            material: Material::Scale {
                name: self.scale().name.into(),
                root: PitchClass::new((9 + self.root_idx as u8) % 12),
            },
            setting: live_setting(self),
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        });
        match kind {
            LiveSequenceKind::Arpeggio => {
                card.label = self.material_name();
                card.material = Material::Chord {
                    name: self.arpeggio_chord().name.into(),
                    root: PitchClass::new((9 + self.root_idx as u8) % 12),
                };
                card.setting = live_setting(self);
                card.touch = Touch::Arpeggiate {
                    direction: self.arpeggio_direction,
                    inversion: self.arpeggio_inversion,
                };
            },
            LiveSequenceKind::Exercise => {
                card.label = self.exercise().name.into();
                card.material = Material::Riff {
                    name: self.exercise().name.into(),
                };
                card.setting = live_setting(self);
                card.touch = Touch::Walk;
            },
            LiveSequenceKind::ScaleRun => {},
        }
        card
    }

    fn live_events(&self, kind: LiveSequenceKind) -> Vec<PracticeEvent> {
        let contacts = match kind {
            LiveSequenceKind::Arpeggio => {
                let board = Fretboard::new(self.tuning(), self.fret_count);
                let shapes = generate_shapes(
                    &board,
                    self.arpeggio_chord(),
                    self.root(),
                    self.arpeggio_bass(),
                );
                let Some(shape) = shapes.get(
                    self.arpeggio_position_idx
                        .min(shapes.len().saturating_sub(1)),
                ) else {
                    return Vec::new();
                };
                let run = ArpeggioRun::new(
                    &shape.positions,
                    self.arpeggio_bass(),
                    self.arpeggio_direction,
                );
                (0..run.walk_len())
                    .filter_map(|ordinal| {
                        let index = run.position_at(ordinal)?;
                        let position = shape.positions.get(index)?;
                        Some((
                            position.string_index,
                            position.fret,
                            position.pitch.frequency() as f32,
                        ))
                    })
                    .collect::<Vec<_>>()
            },
            LiveSequenceKind::Exercise => {
                let exercise = self.exercise();
                let params = ExerciseParams {
                    starting_fret: self.exercise_starting_fret,
                    ..ExerciseParams::default()
                };
                let board = Fretboard::new(self.tuning(), self.fret_count);
                exercise
                    .generate(&self.tuning(), &params)
                    .into_iter()
                    .map(|step| {
                        (
                            step.string_index,
                            step.fret,
                            board.pitch_at(step.string_index, step.fret).frequency() as f32,
                        )
                    })
                    .collect()
            },
            LiveSequenceKind::ScaleRun => self.effective_run_path(),
        };
        contacts
            .into_iter()
            .enumerate()
            .map(|(ordinal, (string, fret, pitch))| PracticeEvent {
                ordinal: ordinal as u32,
                at: ordinal as f64,
                strum_seconds: 0.0,
                pitches: vec![pitch],
                contacts: vec![(string, fret)],
            })
            .collect()
    }
}

fn live_setting(stage: &StageState) -> Setting {
    let tuning = stage.tuning();
    Setting {
        instrument: tuning.instrument.to_string(),
        tuning: Some(tuning.name.clone()),
        ..Setting::default()
    }
}

fn authored_hold(card: &Card) -> PracticeHold {
    match &card.timing.hold {
        Hold::Manual => PracticeHold::Manual {
            phrase_beats: 0.0,
            tail_seconds: 0.0,
        },
        Hold::Bars(bars) => PracticeHold::Beats(f64::from((*bars).max(1)) * 4.0),
        Hold::Reps(reps) => PracticeHold::Beats(f64::from((*reps).max(1)) * 4.0),
        Hold::Seconds(seconds) => PracticeHold::Seconds(f64::from((*seconds).max(0.5))),
    }
}

fn set_even_onsets(events: &mut [PracticeEvent], total: f64) {
    if events.is_empty() {
        return;
    }
    let step = total / events.len() as f64;
    for event in events {
        event.at = f64::from(event.ordinal) * step;
    }
}

fn same_pitch(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.02
}

fn same_pitches(actual: &[f32], expected: &[f32]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| same_pitch(*actual, *expected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::rehearsal::{FretWindow, Recipe, ScalePattern, Setting};

    fn stage() -> StageState {
        StageState::new()
    }

    fn set(cards: Vec<Card>) -> Set {
        let mut set = Set::default();
        set.cards = cards;
        set
    }

    fn path_card(stage: &StageState, positions: Vec<(usize, u8)>) -> Card {
        Card {
            id: CardId(7),
            label: "Repeated path".into(),
            material: Material::Path {
                positions,
                root: PitchClass::new(9),
            },
            setting: Setting {
                instrument: stage.tuning().instrument.to_string(),
                tuning: Some(stage.tuning().name),
                ..Setting::default()
            },
            touch: Touch::Walk,
            timing: Timing::default(),
            from: None,
        }
    }

    #[test]
    fn set_lowering_preserves_ordered_repeated_physical_visits_and_seconds() {
        let mut stage = stage();
        let card = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
        let expected = stage.card_voicing(&card).unwrap().0;
        stage.set_tuning(1);
        let mut card = card;
        card.timing.hold = Hold::Seconds(3.0);
        let set = set(vec![card]);
        let program = stage.practice_program(&set, 120.0);
        let lowered = &program.cards[0];
        assert!(lowered.unavailable.is_none());
        assert_eq!(lowered.hold, PracticeHold::Seconds(3.0));
        assert_eq!(lowered.events.len(), 3);
        assert_eq!(
            lowered
                .events
                .iter()
                .map(|event| event.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(lowered.events[0].contacts, vec![(0, 1)]);
        assert_eq!(lowered.events[2].contacts, vec![(0, 1)]);
        assert_eq!(lowered.events[0].pitches, lowered.events[2].pitches);
        assert_eq!(
            lowered
                .events
                .iter()
                .map(|event| event.pitches[0])
                .collect::<Vec<_>>(),
            expected,
            "generic preview and managed lowering use the same retained Path contacts"
        );
        assert!(
            lowered
                .events
                .iter()
                .all(|event| event.strum_seconds == 0.0)
        );
        assert!((lowered.events[1].at - 1.0).abs() < 1e-9);
    }

    #[test]
    fn unavailable_saved_path_contacts_refuse_display_sound_and_managed_lowering() {
        let stage = stage();
        let card = path_card(&stage, vec![(usize::MAX, u8::MAX)]);
        assert!(stage.dots_for_card(&card).is_empty());
        assert!(stage.card_voicing(&card).unwrap().0.is_empty());
        let program = stage.practice_program(&set(vec![card]), 120.0);
        assert!(program.cards[0].unavailable.is_some());
        assert!(program.cards[0].events.is_empty());
    }

    #[test]
    fn path_solo_marks_share_full_ordered_contacts_outside_viewport() {
        let mut stage = stage();
        let mut card = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
        card.setting.mark_mode = MarkMode::Solo;
        card.setting.marked = vec![(0, 1), (0, 2)];
        stage.fret_start = 8;
        stage.fret_count = 12;

        assert!(stage.dots_for_card(&card).is_empty(), "viewport clips dots only");
        let generic = stage.card_sounding_pitches(&card).unwrap().0;
        let program = stage.practice_program(&set(vec![card]), 120.0);
        let managed = program.cards[0]
            .events
            .iter()
            .flat_map(|event| event.pitches.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(generic, managed);
        assert_eq!(generic.len(), 3, "the authored repeated contact remains repeated");
        assert_eq!(program.cards[0].events[0].contacts, vec![(0, 1)]);
        assert_eq!(program.cards[0].events[2].contacts, vec![(0, 1)]);
    }

    #[test]
    fn path_mute_marks_share_full_ordered_contacts_outside_viewport() {
        let mut stage = stage();
        let mut card = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
        card.setting.mark_mode = MarkMode::Mute;
        card.setting.marked = vec![(0, 2)];
        stage.fret_start = 8;
        stage.fret_count = 12;

        assert!(stage.dots_for_card(&card).is_empty(), "viewport clips dots only");
        let generic = stage.card_sounding_pitches(&card).unwrap().0;
        let program = stage.practice_program(&set(vec![card]), 120.0);
        let managed = program.cards[0]
            .events
            .iter()
            .flat_map(|event| event.pitches.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(generic, managed);
        assert_eq!(generic.len(), 2);
        assert_eq!(program.cards[0].events[0].contacts, vec![(0, 1)]);
        assert_eq!(program.cards[0].events[1].contacts, vec![(0, 1)]);
    }

    #[test]
    fn path_contact_setup_supports_legacy_inheritance_and_refuses_unknown_identity() {
        let stage = stage();
        let mut inherited = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
        inherited.setting.instrument.clear();
        inherited.setting.tuning = None;
        let expected = stage.card_voicing(&inherited).unwrap().0;
        let program = stage.practice_program(&set(vec![inherited.clone()]), 120.0);
        assert_eq!(
            program.cards[0]
                .events
                .iter()
                .map(|event| event.pitches[0])
                .collect::<Vec<_>>(),
            expected
        );

        inherited.setting.instrument = "unknown instrument".into();
        inherited.setting.tuning = Some("unknown tuning".into());
        assert!(stage.card_voicing(&inherited).unwrap().0.is_empty());
        let refused = stage.practice_program(&set(vec![inherited]), 120.0);
        assert!(refused.cards[0].unavailable.is_some());
        assert!(refused.cards[0].events.is_empty());
    }

    #[test]
    fn equal_pitch_path_visits_keep_each_authored_contact() {
        let stage = stage();
        let board = Fretboard::new(stage.tuning(), stage.fret_count);
        let mut equal_pitch_contacts = None;
        'outer: for left_string in 0..stage.string_count() {
            for left_fret in 0..=stage.fret_count {
                for right_string in left_string + 1..stage.string_count() {
                    for right_fret in 0..=stage.fret_count {
                        if board.pitch_at(left_string, left_fret).midi()
                            == board.pitch_at(right_string, right_fret).midi()
                        {
                            equal_pitch_contacts =
                                Some(((left_string, left_fret), (right_string, right_fret)));
                            break 'outer;
                        }
                    }
                }
            }
        }
        let (first, second) =
            equal_pitch_contacts.expect("tuning has alternate equal-pitch contacts");
        let lowered = stage
            .practice_program(
                &set(vec![path_card(&stage, vec![first, second, first])]),
                120.0,
            )
            .cards
            .remove(0);
        assert!(lowered.unavailable.is_none(), "{:?}", lowered.unavailable);
        assert_eq!(lowered.events.len(), 3);
        assert_eq!(lowered.events[0].contacts, vec![first]);
        assert_eq!(lowered.events[1].contacts, vec![second]);
        assert_eq!(lowered.events[2].contacts, vec![first]);
        assert_eq!(lowered.events[0].pitches, lowered.events[1].pitches);
        assert_eq!(lowered.events[0].ordinal, 0);
        assert_eq!(lowered.events[1].ordinal, 1);
        assert_eq!(lowered.events[2].ordinal, 2);
    }

    #[test]
    fn manual_phrase_and_beat_holds_have_distinct_units() {
        let stage = stage();
        let path = path_card(&stage, vec![(0, 1), (0, 2), (0, 3)]);
        let mut set = set(vec![path.clone()]);
        let manual = stage.practice_program(&set, 120.0).cards.remove(0);
        assert!(
            matches!(manual.hold, PracticeHold::Manual { phrase_beats: 2.0, tail_seconds } if (tail_seconds - 0.4).abs() < 1e-6)
        );
        assert_eq!(
            manual
                .events
                .iter()
                .map(|event| event.at)
                .collect::<Vec<_>>(),
            vec![0.0, 1.0, 2.0]
        );
        let slower_manual = stage.practice_program(&set, 72.0).cards.remove(0);
        assert!(
            matches!(slower_manual.hold, PracticeHold::Manual { tail_seconds, .. } if tail_seconds == 0.4)
        );

        set.cards[0].timing.hold = Hold::Bars(2);
        let bars = stage.practice_program(&set, 120.0).cards.remove(0);
        assert_eq!(bars.hold, PracticeHold::Beats(8.0));
        assert_eq!(
            bars.events.iter().map(|event| event.at).collect::<Vec<_>>(),
            vec![0.0, 8.0 / 3.0, 16.0 / 3.0]
        );
    }

    #[test]
    fn scale_pattern_repeats_are_events_and_marked_contacts_are_retained() {
        let mut stage = stage();
        let major = stage
            .scales()
            .iter()
            .position(|scale| scale.name == "Major")
            .unwrap();
        stage.select_scale(major);
        let mut card = stage.card_from_lens().expect("default scale lens card");
        card.material = Material::ScalePattern {
            name: stage.scale().name.into(),
            root: PitchClass::new(9),
            pattern: ScalePattern::Thirds,
        };
        card.touch = Touch::Walk;
        let realization = stage.scale_card_realization(&card).unwrap();
        assert!(realization.notes.len() >= 4);
        let chosen = realization.notes[0].clone();
        card.setting.marked = vec![(chosen.string_index, chosen.physical_fret)];
        card.setting.mark_mode = MarkMode::Solo;
        let lowered = stage
            .practice_program(&set(vec![card]), 120.0)
            .cards
            .remove(0);
        assert!(lowered.unavailable.is_none());
        assert!(!lowered.events.is_empty());
        assert!(
            lowered
                .events
                .iter()
                .all(|event| event.contacts == vec![(chosen.string_index, chosen.physical_fret)])
        );
    }

    #[test]
    fn chord_block_and_chord_approach_keep_realized_physical_contacts() {
        let mut stage = stage();
        stage.set_lens(crate::Lens::Chords);
        let chord = stage.card_from_lens().unwrap();
        let block = stage
            .practice_program(&set(vec![chord.clone()]), 120.0)
            .cards
            .remove(0);
        assert!(block.unavailable.is_none());
        assert_eq!(block.events.len(), 1, "Block is one grouped chord event");
        assert_eq!(block.events[0].strum_seconds, 0.018);
        assert!(block.events[0].pitches.len() >= 3);
        assert_eq!(
            block.events[0].contacts.len(),
            block.events[0].pitches.len()
        );

        let Material::Chord { name, root } = chord.material else {
            panic!("chord lens card")
        };
        let approach = Card {
            id: CardId(9),
            label: "Approach".into(),
            material: Material::ChordApproach {
                name,
                root,
                direction: woodshedding::rehearsal::ApproachDirection::Below,
            },
            setting: chord.setting,
            touch: Touch::Walk,
            timing: Timing::default(),
            from: None,
        };
        let lowered = stage
            .practice_program(&set(vec![approach]), 120.0)
            .cards
            .remove(0);
        assert!(
            lowered.unavailable.is_none(),
            "{}",
            lowered.unavailable.unwrap_or_default()
        );
        assert!(lowered.events.len() >= 2);
        assert!(
            lowered
                .events
                .iter()
                .all(|event| event.pitches.len() == 1 && !event.contacts.is_empty())
        );
    }

    #[test]
    fn live_arpeggio_and_scale_run_lower_their_actual_cursor_paths() {
        let mut stage = stage();
        let arpeggio = stage.live_clock_card(LiveSequenceKind::Arpeggio, 100.0);
        assert!(arpeggio.unavailable.is_none());
        assert!(arpeggio.events.len() >= 3);
        assert!(
            arpeggio
                .events
                .iter()
                .all(|event| event.pitches.len() == 1 && event.contacts.len() == 1)
        );

        stage.set_lens(crate::Lens::Scales);
        let scale_run = stage.live_clock_card(LiveSequenceKind::ScaleRun, 100.0);
        assert!(scale_run.unavailable.is_none());
        assert!(!scale_run.events.is_empty());
        assert!(
            scale_run
                .events
                .iter()
                .all(|event| event.contacts.len() == 1)
        );
        assert!(
            stage
                .live_clock_card(LiveSequenceKind::ScaleRun, f32::NAN)
                .unavailable
                .is_some()
        );
    }

    #[test]
    fn live_both_exercise_keeps_repeated_turnaround_event_identity() {
        let mut stage = stage();
        let index = stage
            .exercises()
            .iter()
            .position(|exercise| exercise.name == "Chromatic 1-2-3-4")
            .unwrap();
        stage.select_exercise(index);
        let card = stage.live_clock_card(LiveSequenceKind::Exercise, 120.0);
        assert!(card.unavailable.is_none());
        let repeated = card
            .events
            .windows(2)
            .find(|pair| pair[0].contacts == pair[1].contacts)
            .unwrap();
        assert_ne!(repeated[0].ordinal, repeated[1].ordinal);
        assert_eq!(repeated[0].pitches, repeated[1].pitches);
    }

    #[test]
    fn invalid_setup_is_retained_as_unavailable_and_riff_silence_is_intentional() {
        let stage = stage();
        let mut scale = stage.card_from_lens().unwrap();
        scale.setting.fret_window = Some(FretWindow { start: 30, span: 2 });
        let riff = Card {
            id: CardId(8),
            label: "Chromatic 1-2-3-4".into(),
            material: Material::Riff {
                name: "Chromatic 1-2-3-4".into(),
            },
            setting: scale.setting.clone(),
            touch: Touch::Block,
            timing: Timing::default(),
            from: Some(Recipe::Exercise {
                name: "Chromatic 1-2-3-4".into(),
            }),
        };
        let program = stage.practice_program(&set(vec![scale, riff]), 120.0);
        assert_eq!(program.cards.len(), 2);
        assert!(program.cards[0].unavailable.is_some());
        assert!(program.cards[1].unavailable.is_none());
        assert!(program.cards[1].events.is_empty());
    }
}
