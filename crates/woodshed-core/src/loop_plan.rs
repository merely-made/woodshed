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
            } else if matches!(card.timing.hold, Hold::Seconds(seconds) if !seconds.is_finite() || seconds < 0.5)
            {
                Some(
                    "Choose finite Seconds of at least 0.5; backing never rounds up a shorter hold.",
                )
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
                    && stage
                        .card_voicing(card)
                        .is_ok_and(|preview| !preview.0.is_empty())
                    && stage
                        .card_sounding_pitches_at_tempo(card, bpm)
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
                }) || events
                    .windows(2)
                    .any(|pair| offset(&pair[0]) >= offset(&pair[1]))
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
mod tests;
