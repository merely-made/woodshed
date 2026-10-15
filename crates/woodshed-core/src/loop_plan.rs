//! A derived backing lane over an explicitly owned working Set.
//!
//! Cards remain the authority. Plans are disposable playback snapshots; they
//! are never saved as a second copy of the Set or converted into SongDoc.

use std::collections::BTreeSet;

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
    pub bars: u8,
    /// Concert pitches resolved from the Card's material and selected shape.
    pub pitches_hz: Vec<f32>,
}

impl LoopSegment {
    pub fn duration_secs(&self) -> f32 {
        f32::from(self.beats) * f32::from(self.bars) * 60.0 / self.bpm
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
    /// Lower only material the current block-chord backing engine can honor.
    /// Unsupported Cards fail admission together, without dropping or rounding
    /// any of them. Hear and rehearsal retain their richer articulation paths.
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
            let reason = if card.id == CardId::UNASSIGNED || !seen.insert(card.id) {
                Some("Card identity is missing or duplicated.")
            } else if !matches!(card.material, Material::Chord { .. })
                || !matches!(card.touch, Touch::Block)
            {
                Some(
                    "Backing currently supports block Chords; Hear and rehearsal play this articulation.",
                )
            } else if !matches!(card.timing.hold, Hold::Bars(1..)) {
                Some(
                    "Choose a positive Bars hold in rehearsal; Manual, Seconds and Reps are not backing durations.",
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
            // Resolve before marks as well: an unavailable shape/formula must
            // not masquerade as an intentionally muted, silent segment.
            let (unmarked, _, _) = stage.card_voicing(card);
            let (pitches_hz, _, _) = stage.card_sounding_pitches_at_tempo(card, bpm);
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
            let Hold::Bars(bars) = card.timing.hold else {
                unreachable!()
            };
            segments.push(LoopSegment {
                id,
                label: card.label.clone(),
                bpm,
                beats: clock.beats,
                bars,
                pitches_hz,
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
        assert_eq!(plan.segments[0].duration_secs(), 4.0);
        assert_eq!(plan.segments[1].bpm, 150.0);
        assert_eq!(
            plan.segments[0].pitches_hz,
            stage.card_sounding_pitches(&set.cards[0]).0
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
        set.cards[0].timing.hold = Hold::Seconds(2.1);
        set.cards[1].touch = Touch::Walk;
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
        let exact = stage.card_sounding_pitches(card).0;
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
    }
}
