//! Device-free projection and capture custody used by the desktop audio seam.

use std::collections::{BTreeMap, BTreeSet};

use woodshed_audio::{
    BackingEvent, BackingPhrase, Bar, ChordRef, PendingChange, SampleBuffer, Song, Subdivision,
    TimeSignature,
};
use woodshed_core::loop_plan::{LoopPlan, LoopSegment, LoopSegmentId};

/// Completed Set/Card PCM and the exact clock under which it was recorded.
/// Cloning this snapshot shares the immutable samples; legacy saved-song
/// compatibility buffers never enter this custody boundary.
#[derive(Clone)]
pub(crate) struct CaptureSnapshot {
    pub id: LoopSegmentId,
    pub bpm: f32,
    pub beats: u8,
    pub bars: u8,
    pub buffer: SampleBuffer,
    pub legacy_clock: bool,
}

impl CaptureSnapshot {
    pub(crate) fn clock_field(&self) -> Option<String> {
        (!self.legacy_clock).then(|| woodshed_core::loop_capture::PER_BEAT_CLOCK.to_string())
    }
}

const MAX_STORED_CAPTURES: usize = 256;
const MAX_STORED_FRAMES: usize = 16_777_216;

#[derive(Clone)]
struct Capture {
    buffer: SampleBuffer,
    legacy_clock: bool,
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
    pub(super) fn snapshot(&self) -> Vec<CaptureSnapshot> {
        self.cards
            .iter()
            .map(|(&id, capture)| CaptureSnapshot {
                legacy_clock: capture.legacy_clock,
                id,
                bpm: capture.bpm,
                beats: capture.beats,
                bars: capture.bars,
                buffer: capture.buffer.clone(),
            })
            .collect()
    }

    /// Validate the complete incoming bank before replacing any completed
    /// take. Output admission still checks its actual device rate separately.
    pub(super) fn restore(&mut self, takes: Vec<CaptureSnapshot>) -> Result<(), String> {
        if takes.len() > MAX_STORED_CAPTURES {
            return Err("Stored Set captures exceed the 256-take bound.".into());
        }
        let mut frames = 0usize;
        let mut validated = BTreeMap::new();
        for take in takes {
            if take.id.set.0 == 0
                || take.id.card.0 == 0
                || validated.contains_key(&take.id)
                || !take.bpm.is_finite()
                || take.bpm < 1.0
                || !(1..=16).contains(&take.beats)
                || take.bars == 0
                || !(1000..=384_000).contains(&take.buffer.sample_rate_hz)
            {
                return Err("Stored Set capture has an invalid or duplicate identity, clock or sample rate.".into());
            }
            frames = frames
                .checked_add(take.buffer.len())
                .filter(|&frames| frames <= MAX_STORED_FRAMES)
                .ok_or("Stored Set captures exceed the 16777216-frame bound.")?;
            let expected = Bar {
                legacy_clock: take.legacy_clock,
                bpm: take.bpm,
                time_signature: TimeSignature::new(take.beats, 4),
                length: take.bars,
                ..Bar::default()
            }
            .duration_samples(take.buffer.sample_rate_hz as f32);
            if expected == 0 || expected != take.buffer.len() as u64 {
                return Err(
                    "Stored Set capture PCM length does not match its recorded clock.".into(),
                );
            }
            if take.buffer.data.iter().any(|sample| !sample.is_finite()) {
                return Err("Stored Set capture contains nonfinite PCM samples.".into());
            }
            validated.insert(
                take.id,
                Capture {
                    legacy_clock: take.legacy_clock,
                    buffer: take.buffer,
                    bpm: take.bpm,
                    beats: take.beats,
                    bars: take.bars,
                },
            );
        }
        self.cards = validated;
        Ok(())
    }

    /// A take may continue through every admitted bar. Reserve those slots
    /// together with inactive custody before input is opened or recording is
    /// armed; existing current-plan IDs are replaced rather than counted twice.
    pub(super) fn check_capture_quota(&self, song: &Song, output_rate: u32) -> Result<(), String> {
        if song.bars.iter().any(|bar| bar.legacy_clock) {
            return Err("Clear the legacy-clock loops before recording. Existing takes keep their original playback clocks.".into());
        }
        let Some(plan) = &self.plan else {
            // Legacy saved-song recording has separate custody and the
            // runtime's own allocation bounds; it cannot grow the Set bank.
            return Ok(());
        };
        if plan.segments.len() != song.bars.len() || output_rate == 0 {
            return Err("Set capture quota requires the current admitted backing clock.".into());
        }
        let current: BTreeSet<_> = plan.segments.iter().map(|segment| segment.id).collect();
        if current.len() != plan.segments.len() {
            return Err("Set capture quota contains duplicate admitted Card identities.".into());
        }
        let mut count = current.len();
        let mut frames = 0u64;
        for (id, capture) in &self.cards {
            if !current.contains(id) {
                count += 1;
                frames = frames
                    .checked_add(capture.buffer.len() as u64)
                    .filter(|&frames| frames <= MAX_STORED_FRAMES as u64)
                    .ok_or("Set capture quota exceeds the stored frame bound.")?;
            }
        }
        if count > MAX_STORED_CAPTURES {
            return Err("Set capture quota would exceed 256 stored takes. Clear stored Card loops before recording this Set.".into());
        }
        for bar in &song.bars {
            // Match Recording::prepare's exact per-bar storage reservation.
            frames = frames
                .checked_add(bar.duration_samples(output_rate as f32).max(1))
                .filter(|&frames| frames <= MAX_STORED_FRAMES as u64)
                .ok_or("Set capture quota would exceed 16777216 stored frames. Clear stored Card loops before recording this Set.")?;
        }
        Ok(())
    }

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
                            legacy_clock: bar.legacy_clock,
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
                || (segment.phrase.is_some() && !segment.pitches_hz.is_empty())
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
                legacy_clock: capture.is_some_and(|capture| capture.legacy_clock),
                bpm: segment.bpm,
                time_signature: TimeSignature::new(segment.beats, 4),
                subdivision: Subdivision::QUARTER,
                chord_ref: segment.pitches_hz.first().map(|root| ChordRef {
                    formula_name: "Set Card".into(),
                    root_freq_hz: *root,
                    pitches_hz: segment.pitches_hz.clone(),
                    label: segment.label.clone(),
                }),
                backing_phrase: segment.phrase.as_ref().map(|phrase| BackingPhrase {
                    events: phrase
                        .events
                        .iter()
                        .map(|event| BackingEvent {
                            beat: event.beat,
                            pitches_hz: event.pitches_hz.clone(),
                            strum_seconds: event.strum_seconds,
                        })
                        .collect(),
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
                    phrase: None,
                })
                .collect(),
        }
    }

    fn take(owner: u64, card: u64, rate: u32) -> CaptureSnapshot {
        let bar = Bar {
            bpm: 137.0,
            time_signature: TimeSignature::new(7, 4),
            length: 3,
            ..Bar::default()
        };
        CaptureSnapshot {
            legacy_clock: false,
            id: LoopSegmentId {
                set: WorkingSetId(owner),
                card: CardId(card),
            },
            bpm: bar.bpm,
            beats: bar.time_signature.numerator,
            bars: bar.length,
            buffer: SampleBuffer::new(vec![0.25; bar.duration_samples(rate as f32) as usize], rate),
        }
    }

    fn quota_take(id: LoopSegmentId, buffer: &SampleBuffer) -> CaptureSnapshot {
        CaptureSnapshot {
            legacy_clock: false,
            id,
            bpm: 60.0 * buffer.sample_rate_hz as f32 / buffer.len() as f32,
            beats: 1,
            bars: 1,
            buffer: buffer.clone(),
        }
    }

    #[test]
    fn quota_reserves_every_current_bar_not_only_the_requested_first_bar() {
        let pcm = SampleBuffer::new(vec![0.0; 65536], 48000);
        let takes = (1..=254)
            .map(|card| {
                quota_take(
                    LoopSegmentId {
                        set: WorkingSetId(2),
                        card: CardId(card),
                    },
                    &pcm,
                )
            })
            .collect();
        let mut bank = LoopCaptures::default();
        bank.restore(takes).unwrap();
        let mut admitted = plan(1);
        for segment in &mut admitted.segments {
            segment.beats = 1;
            segment.bars = 1;
        }
        admitted.segments[0].bpm = 6000.0;
        admitted.segments[1].bpm = 60.0 * 48000.0 / 131000.0;
        let song = bank.prepare(&admitted).unwrap();
        bank.plan = Some(admitted);
        assert_eq!(song.bars[0].duration_samples(48000.0), 480);
        assert!(song.bars[0].duration_samples(48000.0) <= (MAX_STORED_FRAMES - 254 * 65536) as u64);
        assert!(
            bank.check_capture_quota(&song, 48000)
                .unwrap_err()
                .contains("16777216")
        );
        assert_eq!(bank.snapshot().len(), 254);
    }

    #[test]
    fn quota_replaces_existing_ids_once_and_keeps_legacy_custody_separate() {
        let pcm = SampleBuffer::new(vec![0.0; 65536], 48000);
        let mut takes: Vec<_> = (1..=255)
            .map(|card| {
                quota_take(
                    LoopSegmentId {
                        set: WorkingSetId(2),
                        card: CardId(card),
                    },
                    &pcm,
                )
            })
            .collect();
        let id = LoopSegmentId {
            set: WorkingSetId(1),
            card: CardId(1),
        };
        takes.push(quota_take(id, &pcm));
        let mut bank = LoopCaptures::default();
        bank.restore(takes).unwrap();
        let mut admitted = plan(1);
        admitted.segments.truncate(1);
        admitted.segments[0].bpm = 60.0 * 48000.0 / 65536.0;
        admitted.segments[0].beats = 1;
        admitted.segments[0].bars = 1;
        let song = bank.prepare(&admitted).unwrap();
        bank.plan = Some(admitted);
        bank.saved_song = vec![Some(SampleBuffer::new(vec![0.1; 1], 1000))];
        bank.check_capture_quota(&song, 48000).unwrap();
        assert_eq!(bank.snapshot().len(), 256);
        bank.plan = None;
        bank.check_capture_quota(&Song::new(), 48000).unwrap();
    }

    #[test]
    fn snapshots_share_completed_pcm_across_inactive_owners_and_exclude_saved_slots() {
        let first = take(1, 1, 1000);
        let inactive = take(2, 9, 1000);
        let mut bank = LoopCaptures::default();
        bank.saved_song = vec![Some(SampleBuffer::new(vec![0.2; 10], 1000))];
        bank.restore(vec![inactive.clone(), first.clone()]).unwrap();
        let snapshots = bank.snapshot();
        assert_eq!(
            snapshots.iter().map(|take| take.id).collect::<Vec<_>>(),
            vec![first.id, inactive.id]
        );
        assert!(Arc::ptr_eq(&snapshots[0].buffer.data, &first.buffer.data));
        assert!(Arc::ptr_eq(
            &snapshots[1].buffer.data,
            &inactive.buffer.data
        ));
        assert!(bank.saved_song[0].is_some());
    }

    #[test]
    fn restore_validates_exact_runtime_frames_at_integer_and_fractional_clocks() {
        let mut bank = LoopCaptures::default();
        for rate in [1000, 44100, 48000] {
            let good = take(1, 7, rate);
            bank.restore(vec![good.clone()]).unwrap();
            for delta in [-1, 1] {
                let mut bad = good.clone();
                bad.buffer = SampleBuffer::new(
                    vec![0.0; (good.buffer.len() as isize + delta) as usize],
                    rate,
                );
                assert!(bank.restore(vec![bad]).unwrap_err().contains("length"));
                assert!(Arc::ptr_eq(
                    &bank.snapshot()[0].buffer.data,
                    &good.buffer.data
                ));
            }
        }
    }

    #[test]
    fn malformed_restore_cannot_partially_replace_the_current_bank() {
        let original = take(1, 1, 1000);
        let incoming = take(2, 2, 1000);
        let mut bank = LoopCaptures::default();
        bank.restore(vec![original.clone()]).unwrap();
        for corruption in 0..10 {
            let mut bad = take(3, 3, 1000);
            match corruption {
                0 => bad.id.set.0 = 0,
                1 => bad.id.card.0 = 0,
                2 => bad.bpm = f32::NAN,
                3 => bad.bpm = f32::INFINITY,
                4 => bad.bpm = 0.0,
                5 => bad.beats = 0,
                6 => bad.beats = 17,
                7 => bad.bars = 0,
                8 => bad.buffer.sample_rate_hz = 0,
                _ => Arc::make_mut(&mut bad.buffer.data)[0] = f32::NAN,
            }
            assert!(bank.restore(vec![incoming.clone(), bad]).is_err());
            let snapshot = bank.snapshot();
            assert_eq!(snapshot.len(), 1);
            assert_eq!(snapshot[0].id, original.id);
            assert!(Arc::ptr_eq(&snapshot[0].buffer.data, &original.buffer.data));
        }
        assert!(bank.restore(vec![incoming.clone(), incoming]).is_err());
        assert_eq!(bank.snapshot()[0].id, original.id);
    }

    #[test]
    fn restore_checks_take_count_and_aggregate_frames_without_reassigning_custody() {
        let original = take(1, 1, 1000);
        let mut bank = LoopCaptures::default();
        bank.restore(vec![original.clone()]).unwrap();
        let too_many: Vec<_> = (1..=257)
            .map(|card| CaptureSnapshot {
                legacy_clock: false,
                id: LoopSegmentId {
                    set: WorkingSetId(2),
                    card: CardId(card),
                },
                ..original.clone()
            })
            .collect();
        assert!(bank.restore(too_many).unwrap_err().contains("256"));
        let rate = 48000;
        let frames = 65537;
        let bpm = 60.0 * rate as f32 / frames as f32;
        let bar = Bar {
            bpm,
            time_signature: TimeSignature::new(1, 4),
            ..Bar::default()
        };
        assert_eq!(bar.duration_samples(rate as f32), frames as u64);
        let pcm = SampleBuffer::new(vec![0.0; frames], rate);
        let oversized: Vec<_> = (1..=256)
            .map(|card| CaptureSnapshot {
                legacy_clock: false,
                id: LoopSegmentId {
                    set: WorkingSetId(2),
                    card: CardId(card),
                },
                bpm,
                beats: 1,
                bars: 1,
                buffer: pcm.clone(),
            })
            .collect();
        assert!(bank.restore(oversized).unwrap_err().contains("16777216"));
        assert_eq!(bank.snapshot()[0].id, original.id);
        assert!(Arc::ptr_eq(
            &bank.snapshot()[0].buffer.data,
            &original.buffer.data
        ));
    }

    #[test]
    fn hydrated_clock_custody_survives_reorder_missing_cards_and_pitch_edits_until_clear() {
        let mut loaded = plan(1);
        loaded.segments[0].bpm = 137.0;
        loaded.segments[0].beats = 7;
        loaded.segments[0].bars = 3;
        let original = take(1, 1, 1000);
        let mut bank = LoopCaptures::default();
        bank.restore(vec![original.clone(), take(2, 9, 1000)])
            .unwrap();
        loaded.segments.reverse();
        loaded.segments[1].pitches_hz = vec![333.1];
        let projected = bank.prepare(&loaded).unwrap();
        assert!(Arc::ptr_eq(
            &projected.bars[1].audio_buffer.as_ref().unwrap().data,
            &original.buffer.data
        ));
        let mut edited = loaded.clone();
        edited.segments[1].bpm = 90.0;
        assert!(
            bank.prepare(&edited)
                .unwrap_err()
                .contains("Clear the captured loop")
        );
        let mut missing = loaded;
        missing.segments.retain(|segment| segment.id != original.id);
        assert!(
            bank.prepare(&missing).unwrap().bars[0]
                .audio_buffer
                .is_none()
        );
        assert_eq!(bank.snapshot().len(), 2);
        bank.clear(original.id);
        assert!(
            bank.prepare(&edited).unwrap().bars[1]
                .audio_buffer
                .is_none()
        );
        assert_eq!(bank.snapshot().len(), 1);
        assert_eq!(bank.snapshot()[0].id.set, WorkingSetId(2));
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
