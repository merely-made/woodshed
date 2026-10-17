//! A derived backing lane over an explicitly owned working Set.
//!
//! Cards remain the authority. Plans are disposable playback snapshots; they
//! are never saved as a second copy of the Set or converted into SongDoc.

use std::collections::BTreeSet;

pub use audio_primitives::timing::EventTime;
use serde::{Deserialize, Serialize};
use woodshedding::rehearsal::{CardId, Hold, Material, Set, Touch};

use crate::{StageState, working_sets::WorkingSetId};

/// Occurrence identity is local to a Set, so both owners qualify a capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LoopSegmentId {
    pub set: WorkingSetId,
    pub card: CardId,
}

/// Existing saves continue opening their authored backing song.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LooperSource {
    #[default]
    SavedSong,
    Set(WorkingSetId),
}

/// Explicit clock policy for Cards, whose Timing does not contain a meter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LooperConfig {
    pub source: LooperSource,
    pub inherited_bpm: f32,
    /// Quarter-note beats per bar; the denominator is explicitly 4.
    pub beats: u8,
    pub one_shot: bool,
    pub click: bool,
}

impl Default for LooperConfig {
    fn default() -> Self {
        Self {
            source: LooperSource::SavedSong,
            inherited_bpm: 120.0,
            beats: 4,
            one_shot: false,
            click: true,
        }
    }
}

impl LooperConfig {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopSegment {
    pub id: LoopSegmentId,
    pub label: String,
    pub bpm: f32,
    pub beats: u8,
    /// Positive for Bars holds; zero when `seconds` supplies the exact duration.
    pub bars: u8,
    /// Absolute authored duration, independent of BPM and meter. `None` for Bars.
    pub seconds: Option<f64>,
    /// Resolved pitches for the existing block-Chord path; phrase segments
    /// carry their ordered pitches in `phrase`.
    pub pitches_hz: Vec<f32>,
    /// Ordered articulation for phrase-backed Cards. `None` retains the
    /// existing once-per-measure Bars block-chord backing behavior. Seconds
    /// always carry a phrase; an empty phrase is deliberate block-Chord mute.
    pub phrase: Option<LoopPhrase>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopPhrase {
    pub events: Vec<LoopEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopEvent {
    /// Relative onset in the segment's explicit clock unit.
    pub time: EventTime,
    pub pitches_hz: Vec<f32>,
    pub strum_seconds: f64,
}

impl LoopSegment {
    pub fn duration_secs(&self) -> f32 {
        self.seconds.map_or_else(
            || f32::from(self.beats) * f32::from(self.bars) * 60.0 / self.bpm,
            |seconds| seconds as f32,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopPlan {
    pub owner: WorkingSetId,
    pub name: String,
    pub one_shot: bool,
    pub click: bool,
    pub segments: Vec<LoopSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopReadiness {
    pub segment: Option<LoopSegmentId>,
    pub reason: String,
}

impl LoopPlan {
    /// Lower Cards the backing renderer can represent without changing their
    /// authored material or articulation. Unsupported Cards fail admission
    /// together, without dropping or rounding any of them.
    pub fn lower(
        owner: WorkingSetId,
        name: &str,
        set: &Set,
        stage: &StageState,
        clock: &LooperConfig,
    ) -> Result<Self, Vec<LoopReadiness>> {
        let mut issues = Vec::new();
        let mut segments = Vec::new();
        let mut seen = BTreeSet::new();
        if owner.0 == 0 || set.cards.is_empty() {
            issues.push(LoopReadiness {
                segment: None,
                reason: "Choose a nonempty working Set.".into(),
            });
        }
        if clock.beats == 0 || clock.beats > 16 {
            issues.push(LoopReadiness {
                segment: None,
                reason: "Choose a quarter-note meter from 1/4 to 16/4.".into(),
            });
        }
        for card in &set.cards {
            let id = LoopSegmentId {
                set: owner,
                card: card.id,
            };
            let legacy_block_chord = matches!(card.material, Material::Chord { .. })
                && matches!(card.touch, Touch::Block);
            let phrase_material = matches!(
                card.material,
                Material::Scale { .. }
                    | Material::ScalePattern { .. }
                    | Material::Path { .. }
                    | Material::Chord { .. }
            ) || matches!(card.timing.hold, Hold::Seconds(_))
                && matches!(card.material, Material::ChordApproach { .. });
            let reason = if card.id == CardId::UNASSIGNED || !seen.insert(card.id) {
                Some("Card identity is missing or duplicated.")
            } else if !legacy_block_chord && !phrase_material {
                Some(
                    "Backing supports Chords, Scales, Scale Patterns and Paths; Chord Approaches require Seconds.",
                )
            } else if matches!(card.timing.hold, Hold::Seconds(seconds) if !seconds.is_finite() || seconds < 0.5) {
                Some("Choose finite Seconds of at least 0.5; backing never rounds up a shorter hold.")
            } else if !matches!(card.timing.hold, Hold::Bars(1..) | Hold::Seconds(_)) {
                Some(
                    "Choose positive Bars or finite Seconds of at least 0.5; Manual and Reps are not backing durations.",
                )
            } else if card.setting.instrument.is_empty() || card.setting.tuning.is_none() {
                Some("Choose an explicit Card instrument and tuning before backing playback.")
            } else if stage.resolve_card_tuning(card).is_err() {
                Some("The Card's instrument or tuning is unavailable.")
            } else {
                None
            };
            if let Some(reason) = reason {
                issues.push(LoopReadiness {
                    segment: Some(id),
                    reason: reason.into(),
                });
                continue;
            }
            let bpm = card.timing.bpm.unwrap_or(clock.inherited_bpm);
            if !bpm.is_finite() || bpm < 1.0 {
                issues.push(LoopReadiness {
                    segment: Some(id),
                    reason: "Choose a finite tempo of at least 1 bpm.".into(),
                });
                continue;
            }
            let (bars, seconds) = match card.timing.hold {
                Hold::Bars(bars) => (bars, None),
                Hold::Seconds(seconds) => (0, Some(f64::from(seconds))),
                _ => unreachable!("hold admitted above"),
            };
            // Keep the existing block-chord path intact, including deliberate
            // mute: a resolved empty marked pitch list is a clocked silent bar.
            let (pitches_hz, phrase) = if legacy_block_chord && seconds.is_none() {
                let resolved = stage.card_voicing(card).and_then(|unmarked| {
                    stage
                        .card_sounding_pitches_at_tempo(card, bpm)
                        .map(|marked| (unmarked, marked))
                });
                let ((unmarked, _, _), (pitches_hz, _, _)) = match resolved {
                    Ok(preview) => preview,
                    Err(error) => {
                        issues.push(LoopReadiness {
                            segment: Some(id),
                            reason: error.to_string(),
                        });
                        continue;
                    },
                };
                if unmarked.is_empty()
                    || pitches_hz
                        .iter()
                        .any(|pitch| !pitch.is_finite() || *pitch <= 0.0)
                {
                    issues.push(LoopReadiness {
                        segment: Some(id),
                        reason: "The Card's chord or selected voicing cannot be resolved.".into(),
                    });
                    continue;
                }
                (pitches_hz, None)
            } else {
                // P5 refuses eventless marks. Preserve the established explicit
                // block mute by validating its unmarked P5 realization before
                // retaining a clocked empty phrase; unresolved material must
                // still fail rather than turning into silence.
                let mut realization_card = card.clone();
                let silent_block = legacy_block_chord
                    && seconds.is_some()
                    && !card.setting.marked.is_empty()
                    && card.setting.mark_mode != woodshedding::rehearsal::MarkMode::Off
                    && stage.card_voicing(card).is_ok_and(|preview| !preview.0.is_empty())
                    && stage.card_sounding_pitches_at_tempo(card, bpm)
                        .is_ok_and(|preview| preview.0.is_empty());
                if silent_block {
                    realization_card.setting.mark_mode = woodshedding::rehearsal::MarkMode::Off;
                    realization_card.setting.marked.clear();
                }
                let mut one_card = Set::default();
                one_card.push(realization_card);
                let mut program = stage.practice_program(&one_card, bpm);
                let lowered = program
                    .cards
                    .pop()
                    .expect("one Card produces one practice result");
                if let Some(reason) = lowered.unavailable {
                    issues.push(LoopReadiness {
                        segment: Some(id),
                        reason,
                    });
                    continue;
                }
                if lowered.events.is_empty()
                    || lowered.events.iter().any(|event| {
                        !event.at.is_finite()
                            || event.at < 0.0
                            || event.pitches.is_empty()
                            || event
                                .pitches
                                .iter()
                                .any(|pitch| !pitch.is_finite() || *pitch <= 0.0)
                            || !event.strum_seconds.is_finite()
                            || event.strum_seconds < 0.0
                    })
                    || lowered
                        .events
                        .windows(2)
                        .any(|pair| pair[0].at >= pair[1].at)
                {
                    issues.push(LoopReadiness {
                        segment: Some(id),
                        reason:
                            "The Card's phrase has no finite, audible events for backing playback."
                                .into(),
                    });
                    continue;
                }
                let mut events: Vec<_> = lowered
                    .events
                    .into_iter()
                    .map(|event| LoopEvent {
                        // Scale only P5's four-quarter Bars grid to the explicit
                        // Looper meter. Seconds retain P5's absolute positions.
                        time: if seconds.is_some() {
                            EventTime::Seconds(event.at)
                        } else {
                            EventTime::Beats(event.at * f64::from(clock.beats) / 4.0)
                        },
                        pitches_hz: event.pitches,
                        strum_seconds: event.strum_seconds,
                    })
                    .collect();
                let end = seconds.unwrap_or(f64::from(clock.beats) * f64::from(bars));
                let offset = |event: &LoopEvent| match event.time {
                    EventTime::Beats(at) | EventTime::Seconds(at) => at,
                };
                if events.iter().any(|event| {
                    let at = offset(event);
                    !at.is_finite() || at < 0.0 || at >= end
                }) || events.windows(2).any(|pair| offset(&pair[0]) >= offset(&pair[1]))
                {
                    issues.push(LoopReadiness {
                        segment: Some(id),
                        reason: "The Card's phrase events do not fit its ordered backing clock."
                            .into(),
                    });
                    continue;
                }
                if silent_block {
                    events.clear();
                }
                (Vec::new(), Some(LoopPhrase { events }))
            };
            segments.push(LoopSegment {
                id,
                label: card.label.clone(),
                bpm,
                beats: clock.beats,
                bars,
                seconds,
                pitches_hz,
                phrase,
            });
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        Ok(Self {
            owner,
            name: name.into(),
            one_shot: clock.one_shot,
            click: clock.click,
            segments,
        })
    }

    pub fn index_of(&self, id: LoopSegmentId) -> Option<usize> {
        self.segments.iter().position(|segment| segment.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lens;

    fn fixture() -> (StageState, Set, LooperConfig) {
        let mut stage = StageState::new();
        stage.set_lens(Lens::Chords);
        let mut card = stage.card_from_lens().unwrap();
        card.timing.hold = Hold::Bars(2);
        card.timing.bpm = Some(90.0);
        let mut set = Set::default();
        set.push(card.clone());
        card.label = "second".into();
        card.timing.bpm = None;
        set.push(card);
        (
            stage,
            set,
            LooperConfig {
                beats: 3,
                inherited_bpm: 150.0,
                ..Default::default()
            },
        )
    }

    #[test]
    fn explicit_meter_tempo_and_exact_resolved_pitches_survive_lowering() {
        let (stage, set, clock) = fixture();
        let plan = LoopPlan::lower(WorkingSetId(7), "three", &set, &stage, &clock).unwrap();
        assert_eq!(plan.segments[0].bpm, 90.0);
        assert_eq!(plan.segments[0].beats, 3);
        assert_eq!(plan.segments[0].bars, 2);
        assert_eq!(plan.segments[0].seconds, None);
        assert_eq!(plan.segments[0].duration_secs(), 4.0);
        assert_eq!(plan.segments[1].bpm, 150.0);
        assert_eq!(
            plan.segments[0].pitches_hz,
            stage.card_sounding_pitches(&set.cards[0]).unwrap().0
        );
    }

    #[test]
    fn reorder_and_edit_preserve_occurrence_identity_but_other_owner_does_not_alias() {
        let (stage, mut set, clock) = fixture();
        let first = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap();
        set.cards.swap(0, 1);
        set.cards[1].label = "renamed".into();
        let edited = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap();
        assert_eq!(edited.segments[1].id, first.segments[0].id);
        assert_eq!(edited.index_of(first.segments[0].id), Some(1));
        let other = LoopPlan::lower(WorkingSetId(8), "", &set, &stage, &clock).unwrap();
        assert_ne!(other.segments[1].id, edited.segments[1].id);
    }

    #[test]
    fn unsupported_or_unclocked_cards_fail_together_without_lossy_subset() {
        let (stage, mut set, clock) = fixture();
        set.cards[0].timing.hold = Hold::Seconds(0.1);
        set.cards[1].material = Material::Riff {
            name: "Unknown".into(),
        };
        let issues = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap_err();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].segment.unwrap().card, set.cards[0].id);
        assert_eq!(issues[1].segment.unwrap().card, set.cards[1].id);
    }

    #[test]
    fn unresolved_material_identity_and_invalid_clock_never_become_silence() {
        let (stage, mut set, mut clock) = fixture();
        set.cards[1].id = set.cards[0].id;
        set.cards[0].material = Material::Chord {
            name: "gone".into(),
            root: woodshedding::pitch::PitchClass::new(0),
        };
        clock.beats = 0;
        let issues = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap_err();
        assert_eq!(issues.len(), 3);
        set.cards[1].id = CardId(22);
        set.cards[1].timing.bpm = Some(f32::NAN);
        assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_err());
    }

    #[test]
    fn captured_shape_resolves_its_own_tuning_and_never_substitutes_a_missing_shape() {
        let (mut stage, mut set, clock) = fixture();
        let card = &mut set.cards[0];
        stage.select_card_shape_index(card, 0).unwrap();
        let exact = stage.card_sounding_pitches(card).unwrap().0;
        let other_tuning = crate::tunings()
            .iter()
            .position(|tuning| tuning.name == "Drop D")
            .unwrap();
        stage.set_tuning(other_tuning);
        let plan = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap();
        assert_eq!(plan.segments[0].pitches_hz, exact);
        set.cards[0].setting.voicing_idx = Some(usize::MAX);
        let issues = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap_err();
        assert_eq!(issues[0].segment.unwrap().card, set.cards[0].id);
        assert!(issues[0].reason.contains("cannot be resolved"));
    }

    #[test]
    fn deliberate_mute_keeps_a_clocked_silent_segment() {
        let (stage, mut set, clock) = fixture();
        set.cards[0].setting.marked = stage
            .dots_for_card(&set.cards[0])
            .into_iter()
            .map(|dot| (dot.string_index, dot.fret))
            .collect();
        set.cards[0].setting.mark_mode = woodshedding::rehearsal::MarkMode::Mute;
        let plan = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap();
        assert!(plan.segments[0].pitches_hz.is_empty());
        assert_eq!(plan.segments[0].duration_secs(), 4.0);
        assert!(!plan.segments[1].pitches_hz.is_empty());
        assert!(plan.segments[0].phrase.is_none());
    }

    fn mixed_phrase_set() -> (StageState, Set) {
        let mut stage = StageState::new();
        stage.set_lens(Lens::Chords);
        let mut block = stage.card_from_lens().unwrap();
        block.timing.hold = Hold::Bars(2);
        let mut arpeggio = block.clone();
        arpeggio.touch = Touch::Arpeggiate {
            direction: crate::ArpeggioDirection::UpDown,
            inversion: 0,
        };
        arpeggio.label = "Chord arpeggio".into();
        let mut set = Set::default();
        set.push(block);
        set.push(arpeggio);

        stage.set_lens(Lens::Scales);
        let mut walk = stage.card_from_lens().unwrap();
        walk.timing.hold = Hold::Bars(2);
        walk.touch = Touch::Walk;
        walk.label = "Scale walk".into();
        set.push(walk.clone());

        let dots = stage.dots_for_card(&walk);
        assert!(dots.len() >= 2);
        let mut path = walk;
        path.material = Material::Path {
            positions: vec![
                (dots[0].string_index, dots[0].fret),
                (dots[1].string_index, dots[1].fret),
                (dots[0].string_index, dots[0].fret),
            ],
            root: woodshedding::pitch::PitchClass::new(9),
        };
        path.label = "Repeated path".into();
        set.push(path);
        (stage, set)
    }

    #[test]
    fn mixed_material_and_touch_lowering_preserves_block_and_ordered_phrase_identity() {
        let (stage, set) = mixed_phrase_set();
        let mut clock = LooperConfig {
            beats: 3,
            ..Default::default()
        };
        let three = LoopPlan::lower(WorkingSetId(7), "mixed", &set, &stage, &clock).unwrap();
        assert!(three.segments[0].phrase.is_none());
        assert!(!three.segments[0].pitches_hz.is_empty());
        for segment in &three.segments[1..] {
            assert!(segment.pitches_hz.is_empty());
            assert!(
                segment
                    .phrase
                    .as_ref()
                    .is_some_and(|phrase| !phrase.events.is_empty())
            );
            assert!(segment.phrase.as_ref().unwrap().events.iter().all(|event| {
                matches!(event.time, EventTime::Beats(at)
                    if at >= 0.0 && at < f64::from(segment.beats) * f64::from(segment.bars))
            }));
        }
        let path_id = three.segments[3].id;
        let path_events = &three.segments[3].phrase.as_ref().unwrap().events;
        assert_eq!(path_events[0].pitches_hz, path_events[2].pitches_hz);

        clock.beats = 7;
        let seven = LoopPlan::lower(WorkingSetId(7), "mixed", &set, &stage, &clock).unwrap();
        let path_seven = seven
            .segments
            .iter()
            .find(|segment| segment.id == path_id)
            .unwrap();
        let mut path_set = Set::default();
        path_set.push(set.cards[3].clone());
        let original = stage.practice_program(&path_set, 120.0);
        let expected = &original.cards[0].events;
        let actual = &path_seven.phrase.as_ref().unwrap().events;
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.pitches_hz, expected.pitches);
            assert_eq!(actual.time, EventTime::Beats(expected.at * 7.0 / 4.0));
            assert_eq!(actual.strum_seconds, expected.strum_seconds);
        }
        assert!(matches!(actual.last().unwrap().time, EventTime::Beats(at) if at < 14.0));
    }

    #[test]
    fn phrase_cards_reject_manual_short_seconds_reps_and_eventless_riffs_together() {
        let (stage, mut set) = mixed_phrase_set();
        set.cards[1].timing.hold = Hold::Manual;
        set.cards[2].timing.hold = Hold::Seconds(0.1);
        set.cards[3].timing.hold = Hold::Reps(2);
        set.cards[0].material = Material::Riff {
            name: "Known".into(),
        };
        let issues = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &LooperConfig::default())
            .unwrap_err();
        assert_eq!(issues.len(), 4);
        assert!(issues.iter().all(|issue| issue.segment.is_some()));
    }

    #[test]
    fn seconds_use_the_exact_shared_realization_for_every_playable_material() {
        let (mut stage, mut set) = mixed_phrase_set();
        stage.set_lens(Lens::Scales);
        let major = stage.scales().iter().position(|scale| scale.name == "Major").unwrap();
        stage.select_scale(major);
        let mut pattern = stage.card_from_lens().expect("selected Major scale Card");
        let Material::Scale { name, root } = pattern.material.clone() else { unreachable!() };
        pattern.material = Material::ScalePattern {
            name,
            root,
            pattern: woodshedding::rehearsal::ScalePattern::Thirds,
        };
        pattern.touch = Touch::Walk;
        set.push(pattern);
        let mut approach = set.cards[0].clone();
        let Material::Chord { name, root } = &approach.material else { unreachable!() };
        approach.material = Material::ChordApproach {
            name: name.clone(),
            root: *root,
            direction: woodshedding::rehearsal::ApproachDirection::Below,
        };
        approach.touch = Touch::Walk;
        set.push(approach);
        for card in &mut set.cards {
            card.timing.hold = Hold::Seconds(2.1);
        }
        let before = serde_json::to_value(&set).unwrap();
        let clock = LooperConfig { beats: 7, inherited_bpm: 137.0, ..Default::default() };
        let plan = LoopPlan::lower(WorkingSetId(7), "seconds", &set, &stage, &clock).unwrap();
        let shared = stage.practice_program(&set, clock.inherited_bpm);
        for ((segment, original), card) in plan.segments.iter().zip(shared.cards).zip(&set.cards) {
            assert_eq!(segment.id.card, card.id);
            assert_eq!(segment.bars, 0);
            assert_eq!(segment.seconds, Some(f64::from(2.1f32)));
            assert_eq!(segment.duration_secs(), 2.1f32);
            assert!(segment.pitches_hz.is_empty());
            assert!(original.unavailable.is_none());
            let events = &segment.phrase.as_ref().unwrap().events;
            assert!(!events.is_empty());
            assert_eq!(events.len(), original.events.len());
            for (actual, expected) in events.iter().zip(original.events) {
                assert_eq!(actual.time, EventTime::Seconds(expected.at));
                assert_eq!(actual.pitches_hz, expected.pitches);
                assert_eq!(actual.strum_seconds, expected.strum_seconds);
                assert!(matches!(actual.time, EventTime::Seconds(at)
                    if at >= 0.0 && at < segment.seconds.unwrap()));
            }
        }
        assert_eq!(plan.segments[0].phrase.as_ref().unwrap().events.len(), 1,
            "Seconds block Chord is one grouped P5 event, not repeated fake measures");
        let path = &plan.segments[3].phrase.as_ref().unwrap().events;
        assert_eq!(path[0].pitches_hz, path[2].pitches_hz);
        assert_eq!(serde_json::to_value(&set).unwrap(), before);
    }

    #[test]
    fn seconds_duration_and_events_are_independent_of_tempo_and_meter() {
        let (stage, mut set) = mixed_phrase_set();
        for card in &mut set.cards {
            card.timing.hold = Hold::Seconds(2.1);
            card.timing.bpm = None;
        }
        let slow = LooperConfig { inherited_bpm: 31.0, beats: 3, ..Default::default() };
        let fast = LooperConfig { inherited_bpm: 299.0, beats: 7, ..Default::default() };
        let first = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &slow).unwrap();
        let second = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &fast).unwrap();
        for (a, b) in first.segments.iter().zip(&second.segments) {
            assert_ne!(a.bpm, b.bpm);
            assert_ne!(a.beats, b.beats);
            assert_eq!(a.seconds, b.seconds);
            assert_eq!(a.duration_secs(), b.duration_secs());
            assert_eq!(a.phrase, b.phrase);
        }
        for card in &mut set.cards { card.timing.bpm = Some(90.0); }
        let authored = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &fast).unwrap();
        for (a, b) in authored.segments.iter().zip(&second.segments) {
            assert_eq!(a.bpm, 90.0);
            assert_eq!(a.seconds, b.seconds);
            assert_eq!(a.phrase, b.phrase);
        }
    }

    #[test]
    fn mixed_bars_and_seconds_keep_distinct_time_units_and_stable_identities() {
        let (stage, mut set) = mixed_phrase_set();
        set.cards[1].timing.hold = Hold::Seconds(0.5);
        set.cards[3].timing.hold = Hold::Seconds(2.1);
        let clock = LooperConfig { beats: 3, ..Default::default() };
        let plan = LoopPlan::lower(WorkingSetId(7), "mixed", &set, &stage, &clock).unwrap();
        assert_eq!(plan.segments[0].seconds, None);
        assert_eq!(plan.segments[0].bars, 2);
        assert!(plan.segments[0].phrase.is_none());
        assert_eq!(plan.segments[1].seconds, Some(0.5));
        assert_eq!(plan.segments[1].bars, 0);
        assert!(plan.segments[1].phrase.as_ref().unwrap().events.iter()
            .all(|event| matches!(event.time, EventTime::Seconds(_))));
        assert!(plan.segments[2].phrase.as_ref().unwrap().events.iter()
            .all(|event| matches!(event.time, EventTime::Beats(_))));
        set.cards.swap(1, 3);
        let reordered = LoopPlan::lower(WorkingSetId(7), "mixed", &set, &stage, &clock).unwrap();
        assert_eq!(reordered.segments[3], plan.segments[1]);
        assert_eq!(reordered.segments[1], plan.segments[3]);
    }

    #[test]
    fn seconds_bounds_fail_without_rounding_or_mutating_authored_cards() {
        let (stage, mut set, clock) = fixture();
        set.cards.truncate(1);
        for seconds in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.0, 0.1, 0.499] {
            set.cards[0].timing.hold = Hold::Seconds(seconds);
            let issues = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap_err();
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].segment.unwrap().card, set.cards[0].id);
            assert!(issues[0].reason.contains("at least 0.5"));
            assert!(matches!(set.cards[0].timing.hold, Hold::Seconds(value)
                if value.to_bits() == seconds.to_bits()));
        }
        set.cards[0].timing.hold = Hold::Seconds(1e30);
        assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_err(),
            "P5 rejects durations not representable by its portable clock");
        set.cards[0].timing.hold = Hold::Seconds(0.5);
        assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_ok());
    }

    #[test]
    fn seconds_keep_deliberate_block_mute_but_refuse_unresolved_and_eventless_cards() {
        let (stage, mut set, clock) = fixture();
        set.cards.truncate(1);
        set.cards[0].timing.hold = Hold::Seconds(2.1);
        set.cards[0].setting.marked = stage.dots_for_card(&set.cards[0]).into_iter()
            .map(|dot| (dot.string_index, dot.fret)).collect();
        set.cards[0].setting.mark_mode = woodshedding::rehearsal::MarkMode::Mute;
        let plan = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap();
        assert_eq!(plan.segments[0].seconds, Some(f64::from(2.1f32)));
        assert_eq!(plan.segments[0].bars, 0);
        assert!(plan.segments[0].pitches_hz.is_empty());
        assert!(plan.segments[0].phrase.as_ref().unwrap().events.is_empty());
        set.cards[0].setting.voicing_idx = Some(usize::MAX);
        assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_err());
        set.cards[0].setting.voicing_idx = None;
        set.cards[0].material = Material::Riff { name: "Known".into() };
        assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_err());
    }

    #[test]
    fn seconds_still_require_valid_tempo_meter_and_material() {
        let (stage, mut set, mut clock) = fixture();
        for card in &mut set.cards { card.timing.hold = Hold::Seconds(2.1); }
        for bpm in [f32::NAN, f32::INFINITY, 0.0, 0.9] {
            set.cards[0].timing.bpm = Some(bpm);
            assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_err());
        }
        set.cards[0].timing.bpm = None;
        for beats in [0, 17] {
            clock.beats = beats;
            assert!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).is_err());
        }
        clock.beats = 4;
        clock.inherited_bpm = f32::NAN;
        assert_eq!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap_err().len(), 2);
        clock.inherited_bpm = 120.0;
        set.cards[0].material = Material::Chord {
            name: "Gone".into(), root: woodshedding::pitch::PitchClass::new(0),
        };
        set.cards[1].id = set.cards[0].id;
        assert_eq!(LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &clock).unwrap_err().len(), 2);
    }

    #[test]
    fn phrase_shape_failures_are_reported_without_substituting_stage_shape() {
        let (stage, mut set) = mixed_phrase_set();
        set.cards[1].setting.voicing_idx = Some(usize::MAX);
        let issues = LoopPlan::lower(WorkingSetId(7), "", &set, &stage, &LooperConfig::default())
            .unwrap_err();
        assert!(issues.iter().any(|issue| issue.segment
            == Some(LoopSegmentId {
                set: WorkingSetId(7),
                card: set.cards[1].id
            })));

        set.cards[1].setting.voicing_idx = None;
        let before = set.cards[1].id;
        let mut other_set = set.clone();
        other_set.cards.retain(|card| card.id == before);
        let plan = LoopPlan::lower(
            WorkingSetId(8),
            "",
            &other_set,
            &stage,
            &LooperConfig::default(),
        );
        assert!(plan.is_ok());
        assert_eq!(plan.unwrap().segments[0].id.card, before);
    }
}
