use super::*;

#[test]
fn forged_seconds_without_phrase_is_rejected_atomically_but_explicit_mute_is_admitted() {
    let take = seconds_take(1, 1, 0.50013, 44_100);
    let mut bank = LoopCaptures::default();
    bank.restore(vec![take.clone()]).unwrap();
    let admitted = seconds_plan(&take);
    let song = bank.prepare(&admitted).unwrap();
    bank.plan = Some(admitted.clone());
    for pitches in [Vec::new(), vec![261.625, 391.995]] {
        let mut forged = admitted.clone();
        forged.segments[0].phrase = None;
        forged.segments[0].pitches_hz = pitches;
        assert!(bank.prepare_replacement(&song, &forged).is_err());
        assert_eq!(bank.plan, Some(admitted.clone()));
        let retained = bank.snapshot();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].seconds, take.seconds);
        assert_eq!(retained[0].bars, 0);
        assert!(Arc::ptr_eq(&retained[0].buffer.data, &take.buffer.data));
        assert_eq!(retained[0].buffer.data.as_ref(), take.buffer.data.as_ref());
    }
    let mut muted = admitted;
    muted.segments[0].phrase.as_mut().unwrap().events.clear();
    let projected = bank.prepare_replacement(&song, &muted).unwrap();
    assert!(projected.bars[0].chord_ref.is_none());
    assert!(
        projected.bars[0]
            .backing_phrase
            .as_ref()
            .unwrap()
            .events
            .is_empty()
    );
    assert_eq!(projected.bars[0].duration_seconds, take.seconds);
    assert_eq!(projected.bars[0].length, 1);
    assert!(Arc::ptr_eq(
        &projected.bars[0].audio_buffer.as_ref().unwrap().data,
        &take.buffer.data
    ));
}

#[test]
fn seconds_restore_projection_and_reorder_keep_exact_clock_pcm_and_event_units() {
    for rate in [1_000, 44_100, 48_000] {
        let take = seconds_take(1, 1, 2.34567, rate);
        let mut bank = LoopCaptures::default();
        bank.restore(vec![take.clone()]).unwrap();
        let plan = seconds_plan(&take);
        let song = bank.prepare(&plan).unwrap();
        let bar = &song.bars[0];
        assert_eq!(bar.duration_seconds, take.seconds);
        assert_eq!(bar.length, 1);
        assert_eq!(bar.duration_samples(rate as f32), take.buffer.len() as u64);
        assert_eq!(
            bar.backing_phrase.as_ref().unwrap().events[0].time,
            woodshed_core::loop_plan::EventTime::Seconds(0.125)
        );
        assert!(Arc::ptr_eq(
            &bar.audio_buffer.as_ref().unwrap().data,
            &take.buffer.data
        ));
        assert_eq!(song.bars[1].duration_seconds, None);
        assert_eq!(song.bars[1].length, plan.segments[1].bars);
        bank.plan = Some(plan.clone());
        bank.remember(&song);
        let mut reordered = plan;
        reordered.segments.swap(0, 1);
        let next = bank.prepare_replacement(&song, &reordered).unwrap();
        assert!(Arc::ptr_eq(
            &next.bars[1].audio_buffer.as_ref().unwrap().data,
            &take.buffer.data
        ));
        let snapshot = bank.snapshot();
        assert_eq!(snapshot[0].id, take.id);
        assert_eq!(snapshot[0].seconds, take.seconds);
        assert_eq!(snapshot[0].bars, 0);
        assert_eq!(
            snapshot[0].clock_field().as_deref(),
            Some(woodshed_core::loop_capture::SECONDS_CLOCK)
        );
        assert!(Arc::ptr_eq(&snapshot[0].buffer.data, &take.buffer.data));
    }
}

#[test]
fn seconds_take_refuses_clock_edits_without_touching_pcm_until_explicit_clear() {
    let take = seconds_take(1, 1, 0.50013, 44_100);
    for change in 0..4 {
        let mut bank = LoopCaptures::default();
        bank.restore(vec![take.clone()]).unwrap();
        let original = seconds_plan(&take);
        let mut edited = original.clone();
        match change {
            0 => edited.segments[0].bpm = 137.0,
            1 => edited.segments[0].beats = 5,
            2 => edited.segments[0].seconds = Some(0.75),
            _ => {
                edited.segments[0].seconds = None;
                edited.segments[0].bars = 1;
            },
        }
        assert!(
            bank.prepare(&edited)
                .unwrap_err()
                .contains("Clear the captured loop")
        );
        assert!(Arc::ptr_eq(
            &bank.snapshot()[0].buffer.data,
            &take.buffer.data
        ));
        assert_eq!(bank.snapshot()[0].seconds, take.seconds);
        bank.clear(take.id);
        assert!(
            bank.prepare(&edited).unwrap().bars[0]
                .audio_buffer
                .is_none()
        );
    }
    // Duration itself is independent of BPM/meter before capture exists.
    let bank = LoopCaptures::default();
    let mut plan = seconds_plan(&take);
    let before = bank.prepare(&plan).unwrap().bars[0].duration_samples(44_100.0);
    plan.segments[0].bpm = 300.0;
    plan.segments[0].beats = 3;
    assert_eq!(
        bank.prepare(&plan).unwrap().bars[0].duration_samples(44_100.0),
        before
    );
}

#[test]
fn invalid_seconds_restore_is_atomic_and_seconds_quota_uses_absolute_frames() {
    let take = seconds_take(1, 1, 0.50013, 44_100);
    let mut bank = LoopCaptures::default();
    bank.restore(vec![take.clone()]).unwrap();
    for change in 0..6 {
        let mut bad = take.clone();
        match change {
            0 => bad.seconds = Some(f64::NAN),
            1 => bad.seconds = Some(f64::INFINITY),
            2 => bad.seconds = Some(0.49),
            3 => bad.bars = 1,
            4 => bad.legacy_clock = true,
            _ => bad.buffer = SampleBuffer::new(vec![0.0; take.buffer.len() - 1], 44_100),
        }
        assert!(bank.restore(vec![bad]).is_err());
        assert!(Arc::ptr_eq(
            &bank.snapshot()[0].buffer.data,
            &take.buffer.data
        ));
    }
    let mut plan = seconds_plan(&take);
    plan.segments[0].seconds = Some(MAX_STORED_FRAMES as f64 / 44_100.0 + 1.0);
    let empty = LoopCaptures::default();
    let song = empty.prepare(&plan).unwrap();
    bank.plan = Some(plan);
    assert!(
        bank.check_capture_quota(&song, 44_100)
            .unwrap_err()
            .contains("16777216")
    );
}
