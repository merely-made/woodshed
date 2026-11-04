use super::*;
use std::sync::Arc;
use woodshed_core::working_sets::WorkingSetId;
use woodshedding::rehearsal::CardId;

fn seconds_take(owner: u64, card: u64, seconds: f64, rate: u32) -> CaptureSnapshot {
    let frames = (seconds * f64::from(rate)).round() as usize;
    CaptureSnapshot {
        id: LoopSegmentId {
            set: WorkingSetId(owner),
            card: CardId(card),
        },
        bpm: 113.5,
        beats: 7,
        bars: 0,
        seconds: Some(seconds),
        legacy_clock: false,
        buffer: SampleBuffer::new(
            (0..frames)
                .map(|i| [0.125, -0.0, -0.25, 0.5][i % 4])
                .collect(),
            rate,
        ),
    }
}

fn seconds_plan(take: &CaptureSnapshot) -> LoopPlan {
    let mut plan = plan(take.id.set.0);
    let segment = &mut plan.segments[0];
    segment.id = take.id;
    segment.bpm = take.bpm;
    segment.beats = take.beats;
    segment.bars = 0;
    segment.seconds = take.seconds;
    segment.pitches_hz.clear();
    segment.phrase = Some(woodshed_core::loop_plan::LoopPhrase {
        events: vec![woodshed_core::loop_plan::LoopEvent {
            time: woodshed_core::loop_plan::EventTime::Seconds(0.125),
            pitches_hz: vec![261.625],
            strum_seconds: 0.0,
        }],
    });
    plan
}

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
                seconds: None,
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
        seconds: None,
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
        seconds: None,
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
            seconds: None,
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

#[path = "tests/seconds.rs"]
mod seconds;
