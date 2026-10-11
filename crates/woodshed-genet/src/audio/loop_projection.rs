//! Device-free projection and capture custody used by the desktop audio seam.

use std::collections::{BTreeMap, BTreeSet};

use woodshed_audio::{
    Bar, ChordRef, PendingChange, SampleBuffer, Song, Subdivision, TimeSignature,
};
use woodshed_core::loop_plan::{LoopPlan, LoopSegment, LoopSegmentId};

#[derive(Clone)]
struct Capture {
    buffer: SampleBuffer,
    /// Preserve the clock under which the PCM was captured. A Card clock
    /// change requires an explicit clear rather than rewriting that take.
    bpm: f32,
    beats: u8,
    bars: u8,
}

impl Capture {
    fn clock_matches(&self, segment: &LoopSegment) -> bool {
        self.bpm == segment.bpm && self.beats == segment.beats && self.bars == segment.bars
    }
}

#[derive(Default)]
pub(super) struct LoopCaptures {
    pub(super) plan: Option<LoopPlan>,
    cards: BTreeMap<LoopSegmentId, Capture>,
    /// Saved backing retains its existing index-based compatibility semantics.
    pub(super) saved_song: Vec<Option<SampleBuffer>>,
}

impl LoopCaptures {
    pub(super) fn prepare_replacement(
        &mut self,
        current: &Song,
        plan: &LoopPlan,
    ) -> Result<Song, String> {
        if capture_busy(current) {
            return Err("Stop recording / armed capture before updating the loaded backing. The currently loaded Cards remain active.".into());
        }
        self.remember(current);
        self.prepare(plan)
    }

    pub(super) fn remember(&mut self, song: &Song) {
        if let Some(plan) = &self.plan {
            for (segment, bar) in plan.segments.iter().zip(&song.bars) {
                if let Some(buffer) = &bar.audio_buffer {
                    self.cards.insert(
                        segment.id,
                        Capture {
                            buffer: buffer.clone(),
                            bpm: segment.bpm,
                            beats: segment.beats,
                            bars: segment.bars,
                        },
                    );
                } else {
                    self.cards.remove(&segment.id);
                }
            }
        } else {
            self.saved_song = song
                .bars
                .iter()
                .map(|bar| bar.audio_buffer.clone())
                .collect();
        }
    }

    pub(super) fn prepare(&self, plan: &LoopPlan) -> Result<Song, String> {
        if plan.segments.is_empty() || plan.owner.0 == 0 {
            return Err("Set backing requires a nonempty, explicitly owned plan.".into());
        }
        let mut seen = BTreeSet::new();
        let mut song = Song::new();
        song.name = plan.name.clone();
        song.one_shot = plan.one_shot;
        song.click_enabled = plan.click;
        song.bars.clear();
        for segment in &plan.segments {
            if segment.id.set != plan.owner
                || segment.id.card.0 == 0
                || !seen.insert(segment.id)
                || !segment.bpm.is_finite()
                || segment.bpm < 1.0
                || !(1..=16).contains(&segment.beats)
                || segment.bars == 0
                || segment
                    .pitches_hz
                    .iter()
                    .any(|pitch| !pitch.is_finite() || *pitch <= 0.0)
            {
                return Err("Set backing contains an invalid identity, clock or pitch.".into());
            }
            let capture = self.cards.get(&segment.id);
            if capture.is_some_and(|capture| !capture.clock_matches(segment)) {
                return Err(format!(
                    "Clear the captured loop for '{}' before changing its tempo, meter or hold. The loaded backing keeps its original clock.",
                    segment.label
                ));
            }
            song.bars.push(Bar {
                bpm: segment.bpm,
                time_signature: TimeSignature::new(segment.beats, 4),
                subdivision: Subdivision::QUARTER,
                chord_ref: segment.pitches_hz.first().map(|root| ChordRef {
                    formula_name: "Set Card".into(),
                    root_freq_hz: *root,
                    pitches_hz: segment.pitches_hz.clone(),
                    label: segment.label.clone(),
                }),
                audio_buffer: capture.map(|capture| capture.buffer.clone()),
                label: segment.label.clone(),
                length: segment.bars,
            });
        }
        Ok(song)
    }

    pub(super) fn index_of(&self, id: LoopSegmentId) -> Result<usize, String> {
        self.plan
            .as_ref()
            .and_then(|plan| plan.index_of(id))
            .ok_or_else(|| "That Card is not in the currently loaded Set backing.".into())
    }

    pub(super) fn clear(&mut self, id: LoopSegmentId) {
        self.cards.remove(&id);
    }
}

pub(super) fn capture_busy(song: &Song) -> bool {
    song.recording
        || song
            .pending
            .iter()
            .any(|change| matches!(change, PendingChange::StartRecording { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use woodshed_core::working_sets::WorkingSetId;
    use woodshedding::rehearsal::CardId;

    fn plan(owner: u64) -> LoopPlan {
        LoopPlan {
            owner: WorkingSetId(owner),
            name: "Set".into(),
            one_shot: false,
            click: true,
            segments: [1, 2]
                .into_iter()
                .map(|card| LoopSegment {
                    id: LoopSegmentId {
                        set: WorkingSetId(owner),
                        card: CardId(card),
                    },
                    label: format!("Card {card}"),
                    bpm: 120.0,
                    beats: 3,
                    bars: 2,
                    pitches_hz: vec![261.125 + card as f32, 391.777],
                })
                .collect(),
        }
    }

    #[test]
    fn captures_follow_card_across_reorder_and_pitch_edit_and_never_cross_sets() {
        let original = plan(1);
        let mut bank = LoopCaptures::default();
        let mut song = bank.prepare(&original).unwrap();
        let buffer = SampleBuffer::new(vec![0.25, -0.75, 0.5], 48_000);
        song.bars[0].audio_buffer = Some(buffer.clone());
        bank.plan = Some(original.clone());
        bank.remember(&song);
        let mut edited = original;
        edited.segments.reverse();
        edited.segments[1].label = "edited".into();
        edited.segments[1].pitches_hz = vec![433.123, 567.89];
        let projected = bank.prepare(&edited).unwrap();
        assert!(projected.bars[0].audio_buffer.is_none());
        assert!(Arc::ptr_eq(
            &projected.bars[1].audio_buffer.as_ref().unwrap().data,
            &buffer.data
        ));
        assert_eq!(
            projected.bars[1].chord_ref.as_ref().unwrap().pitches_hz,
            vec![433.123, 567.89]
        );
        assert_eq!(projected.bars[1].duration_secs(), 3.0);
        assert!(
            bank.prepare(&plan(2))
                .unwrap()
                .bars
                .iter()
                .all(|bar| bar.audio_buffer.is_none())
        );
        bank.plan = Some(plan(2));
        bank.remember(&bank.prepare(&plan(2)).unwrap());
        assert!(
            bank.prepare(&edited).unwrap().bars[1]
                .audio_buffer
                .is_some()
        );
    }

    #[test]
    fn recorded_clock_edits_are_refused_without_resizing_pcm_then_clear_permits_them() {
        for change in 0..3 {
            let original = plan(1);
            let mut bank = LoopCaptures::default();
            let mut song = bank.prepare(&original).unwrap();
            let buffer = SampleBuffer::new(vec![0.75; 13], 44_100);
            song.bars[0].audio_buffer = Some(buffer.clone());
            bank.plan = Some(original.clone());
            bank.remember(&song);
            let mut edited = original.clone();
            match change {
                0 => edited.segments[0].bpm = 90.0,
                1 => edited.segments[0].bars = 3,
                _ => edited.segments[0].beats = 7,
            }
            assert!(
                bank.prepare(&edited)
                    .unwrap_err()
                    .contains("Clear the captured loop")
            );
            assert_eq!(bank.plan, Some(original));
            assert_eq!(bank.cards[&edited.segments[0].id].buffer.len(), 13);
            assert!(Arc::ptr_eq(
                &bank.cards[&edited.segments[0].id].buffer.data,
                &buffer.data
            ));
            bank.clear(edited.segments[0].id);
            assert!(
                bank.prepare(&edited).unwrap().bars[0]
                    .audio_buffer
                    .is_none()
            );
        }
    }

    #[test]
    fn armed_or_active_capture_snapshot_refuses_replacement() {
        let mut song = Song::new();
        let loaded = plan(1);
        let mut bank = LoopCaptures {
            plan: Some(loaded.clone()),
            ..Default::default()
        };
        song.pending
            .push(PendingChange::StartRecording { bar_idx: 0 });
        assert!(capture_busy(&song));
        assert!(
            bank.prepare_replacement(&song, &plan(2))
                .unwrap_err()
                .contains("armed")
        );
        assert_eq!(bank.plan, Some(loaded.clone()));
        song.pending.clear();
        assert!(!capture_busy(&song));
        song.on_bar_boundary();
        assert!(!song.recording);
        assert!(song.pending.is_empty());
        song.recording = true;
        assert!(capture_busy(&song));
        assert!(bank.prepare_replacement(&song, &plan(2)).is_err());
        assert_eq!(bank.plan, Some(loaded));
        song.recording = false;
        assert!(!capture_busy(&song));
        assert!(bank.prepare_replacement(&song, &plan(2)).is_ok());
    }

    #[test]
    fn stale_or_other_owner_capture_commands_cannot_resolve_to_a_bar() {
        let mut bank = LoopCaptures {
            plan: Some(plan(1)),
            ..Default::default()
        };
        let id = bank.plan.as_ref().unwrap().segments[0].id;
        assert_eq!(bank.index_of(id).unwrap(), 0);
        bank.plan.as_mut().unwrap().segments.reverse();
        assert_eq!(bank.index_of(id).unwrap(), 1);
        assert!(
            bank.index_of(LoopSegmentId {
                set: WorkingSetId(2),
                ..id
            })
            .is_err()
        );
        bank.plan
            .as_mut()
            .unwrap()
            .segments
            .retain(|segment| segment.id != id);
        assert!(bank.index_of(id).is_err());
    }

    #[test]
    fn saved_backing_capture_slots_are_independent_of_card_capture_custody() {
        let mut bank = LoopCaptures::default();
        let mut saved = Song::new();
        let legacy_pcm = SampleBuffer::new(vec![0.2, 0.3], 44_100);
        saved.bars[0].audio_buffer = Some(legacy_pcm.clone());
        bank.remember(&saved);
        let set_plan = plan(1);
        let mut backing = bank.prepare(&set_plan).unwrap();
        assert!(backing.bars.iter().all(|bar| bar.audio_buffer.is_none()));
        let card_pcm = SampleBuffer::new(vec![-0.4; 7], 48_000);
        backing.bars[0].audio_buffer = Some(card_pcm.clone());
        bank.plan = Some(set_plan);
        bank.remember(&backing);
        assert!(Arc::ptr_eq(
            &bank.saved_song[0].as_ref().unwrap().data,
            &legacy_pcm.data
        ));
        bank.plan = None;
        bank.remember(&saved);
        let reloaded = bank.prepare(&plan(1)).unwrap();
        assert!(Arc::ptr_eq(
            &reloaded.bars[0].audio_buffer.as_ref().unwrap().data,
            &card_pcm.data
        ));
    }
}
