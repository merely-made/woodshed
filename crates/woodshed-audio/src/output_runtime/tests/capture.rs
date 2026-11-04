use super::*;

#[test]
fn cancellation_keeps_accepted_boundary_operations_in_fifo_order() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(1);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.arm_capture(0, true).unwrap();
    r.queue(PendingChange::SetPlaying(true)).unwrap();
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    r.queue(PendingChange::SetPlaying(false)).unwrap();
    r.service().unwrap();
    render(&mut r, 8);
    assert!(!r.snapshot().playing);
    assert!(!r.snapshot().recording);
}

#[test]
fn terminal_one_shot_cancels_an_unreachable_arm() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(1);
    s.playing = true;
    s.one_shot = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 4);
    r.arm_capture(0, true).unwrap();
    r.service().unwrap();
    render(&mut r, 10);
    r.service().unwrap();
    assert!(!r.snapshot().playing);
    assert!(!r.snapshot().recording);
    assert!(!r.capture_busy());
}

#[test]
fn future_saved_bar_arm_survives_variable_intermediate_boundaries() {
    let mut r = runtime();
    let (mut producer, _) = feed(&mut r, 64);
    let mut s = song(3);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);

    r.arm_capture(2, false).unwrap();
    r.service().unwrap();
    for (frames, expected_bar) in [(3, 0), (8, 1), (4, 1), (4, 2)] {
        render(&mut r, frames);
        assert_eq!(r.snapshot().cursor.bar_idx, expected_bar);
        if expected_bar < 2 {
            assert!(!r.snapshot().recording, "must wait through earlier bars");
        }
    }
    assert_eq!(r.snapshot().cursor.bar_idx, 2);
    assert!(
        r.snapshot().recording,
        "future arm must start at its exact bar"
    );
    producer.push_samples(&[0.375; 2]);
    render(&mut r, 2);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(!r.snapshot().capture_invalid);
    assert!(!r.capture_busy());
    let song = r.song();
    assert!(song.bars[0].audio_buffer.is_none());
    assert!(song.bars[1].audio_buffer.is_none());
    let target = song.bars[2].audio_buffer.as_ref().unwrap();
    assert_eq!(target.data[0].to_bits(), 0.375f32.to_bits());
    assert_eq!(target.data[1].to_bits(), 0.375f32.to_bits());
}

#[test]
fn cancelling_a_future_arm_before_its_target_releases_it_without_recording() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(3);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.arm_capture(2, false).unwrap();
    r.service().unwrap();
    render(&mut r, 10);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(!r.snapshot().recording);
    assert!(!r.capture_busy());
    assert!(!r.snapshot().capture_invalid);
}

#[test]
fn a_new_arm_after_cancellation_supersedes_the_old_future_target() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(4);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.arm_capture(3, false).unwrap();
    r.service().unwrap();
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(!r.capture_busy());
    r.arm_capture(1, false).unwrap();
    r.service().unwrap();
    render(&mut r, 8);
    assert_eq!(r.snapshot().cursor.bar_idx, 1);
    assert!(r.snapshot().recording);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(!r.snapshot().recording);
    assert!(!r.capture_busy());
}

#[test]
fn boundary_stop_cancels_an_earlier_unstarted_future_arm() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(3);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.arm_capture(2, false).unwrap();
    r.queue(PendingChange::StopRecording).unwrap();
    r.service().unwrap();
    render(&mut r, 9);
    assert_eq!(r.snapshot().cursor.bar_idx, 1);
    assert!(!r.snapshot().recording);
    assert!(!r.snapshot().capture_armed);
    r.service().unwrap();
    assert!(!r.capture_busy());
}

#[test]
fn arm_after_seek_resolves_at_that_same_ordered_boundary() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(3);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.queue(PendingChange::SeekTo { bar_idx: 2 }).unwrap();
    r.arm_capture(2, false).unwrap();
    r.service().unwrap();
    render(&mut r, 9);
    assert_eq!(r.snapshot().cursor.bar_idx, 2);
    assert!(r.snapshot().recording);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
}

#[test]
fn pausing_preserves_a_future_arm_until_playback_resumes() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(3);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.arm_capture(2, false).unwrap();
    r.queue(PendingChange::SetPlaying(false)).unwrap();
    r.service().unwrap();
    render(&mut r, 9);
    assert!(!r.snapshot().playing);
    assert!(!r.snapshot().recording);
    assert!(r.snapshot().capture_armed);
    r.play().unwrap();
    r.service().unwrap();
    render(&mut r, 10);
    assert_eq!(r.snapshot().cursor.bar_idx, 2);
    assert!(r.snapshot().recording);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
}

#[test]
fn one_shot_ending_before_a_future_arm_reports_invalid_capture() {
    let mut r = runtime();
    let _ = feed(&mut r, 64);
    let mut s = song(1);
    s.playing = true;
    s.one_shot = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 4);
    r.arm_capture(0, true).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert!(r.snapshot().capture_armed);
    assert!(!r.snapshot().recording);
    render(&mut r, 5);
    assert!(!r.snapshot().playing);
    assert!(!r.snapshot().recording);
    assert!(r.snapshot().capture_invalid);
    assert!(r.song().bars[0].audio_buffer.is_none());
    r.service().unwrap();
    assert!(!r.capture_busy());
    r.arm_capture(0, false).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert!(r.snapshot().capture_armed);
    assert!(!r.snapshot().recording);
    assert!(
        !r.snapshot().capture_invalid,
        "a new arm must not inherit the prior attempt's failure"
    );
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(!r.capture_busy());
}

#[test]
fn fractional_tempo_count_in_uses_one_canonical_measure() {
    let mut r = runtime();
    let mut s = song(1);
    s.bars[0].bpm = 137.0;
    s.bars[0].time_signature = TimeSignature::new(4, 4);
    s.click_enabled = true;
    s.playing = true;
    let expected = audio_primitives::frames_per_bar(1000, 137.0, 4, 4);
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, expected);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 0);
    render(&mut r, 1);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 1);
}

#[test]
fn variable_tempo_blocks_and_one_shot_clear_the_entire_tail() {
    let mut r = runtime();
    let mut s = song(2);
    s.bars[0].audio_buffer = Some(SampleBuffer::new(vec![0.25; 10], 1000));
    s.bars[1].bpm = 3000.0;
    s.bars[1].time_signature = TimeSignature::new(3, 8);
    s.bars[1].length = 2;
    s.bars[1].audio_buffer = Some(SampleBuffer::new(vec![0.5; 60], 1000));
    s.playing = true;
    s.one_shot = true;
    r.set_song(s).unwrap();
    assert!(r.song().playing);
    r.service().unwrap();
    let output = render(&mut r, 173);
    assert!(output[..20].iter().all(|v| (*v - 0.2).abs() < 1e-6));
    assert!(output[20..140].iter().all(|v| (*v - 0.4).abs() < 1e-6));
    assert!(output[140..].iter().all(|v| *v == 0.0));
    assert!(!r.song().playing);
}

#[test]
fn clearing_pcm_preserves_play_accepted_before_the_first_graph_block() {
    let mut r = runtime();
    let mut s = song(1);
    s.bars[0].audio_buffer = Some(SampleBuffer::new(vec![0.25; 10], 1000));
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.play().unwrap();
    r.clear_recording(0).unwrap();
    assert!(r.song().playing);
    r.service().unwrap();
    assert!(render(&mut r, 4).iter().all(|v| *v == 0.0));
    assert!(r.song().playing);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 4);
}

#[test]
fn count_in_waits_one_measure_and_muted_click_skips_it() {
    let mut r = runtime();
    let mut s = song(1);
    s.click_enabled = true;
    s.playing = true;
    s.bars[0].audio_buffer = Some(SampleBuffer::new(vec![0.25; 10], 1000));
    r.set_song(s.clone()).unwrap();
    r.service().unwrap();
    render(&mut r, 9);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 0);
    render(&mut r, 2);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 1);
    s.click_enabled = false;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 3);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 3);
}

#[test]
fn primed_input_survives_in_block_seek_arm_and_replays_without_edit() {
    let mut r = runtime();
    let (mut producer, _) = feed(&mut r, 64);
    let mut s = song(2);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 4);
    r.arm_capture(1, true).unwrap();
    assert!(r.capture_busy());
    assert!(r.set_song(song(1)).is_err());
    producer.push_samples(&[0.5; 20]);
    r.service().unwrap();
    render(&mut r, 10);
    assert_eq!(r.snapshot().cursor.bar_idx, 1);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 4);
    assert!(r.snapshot().recording);
    assert!(!r.snapshot().capture_invalid);
    r.stop_capture_now().unwrap();
    assert!(r.capture_busy());
    r.service().unwrap();
    render(&mut r, 1);
    // Return still belongs to this admitted owner until control collects it.
    assert!(r.set_song(song(1)).is_err());
    r.service().unwrap();
    assert!(!r.capture_busy());
    let pcm = r.song().bars[1].audio_buffer.clone().unwrap();
    assert_eq!(&pcm.data[..4], &[0.5; 4]);
    r.queue(PendingChange::SeekTo { bar_idx: 1 }).unwrap();
    r.service().unwrap();
    render(&mut r, 5);
    let playback = render(&mut r, 4);
    assert!(playback.iter().all(|v| (*v - 0.4).abs() < 1e-6));
}

#[test]
fn overflow_before_boundary_is_ignored_but_during_take_invalidates() {
    let mut r = runtime();
    let (mut producer, _) = feed(&mut r, 16);
    let mut s = song(1);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 4);
    r.arm_capture(0, true).unwrap();
    producer.push_samples(&[0.25; 24]);
    r.service().unwrap();
    render(&mut r, 10);
    assert!(r.snapshot().recording);
    assert!(!r.snapshot().capture_invalid);
    producer.push_samples(&[0.3; 24]);
    render(&mut r, 1);
    assert!(r.snapshot().capture_invalid);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(r.song().bars[0].audio_buffer.is_none());
    assert!(r.last_error().is_some());
}

#[test]
fn starvation_invalidates_and_reset_does_not_publish_old_owner_pcm() {
    let mut r = runtime();
    let (mut producer, _) = feed(&mut r, 32);
    let mut s = song(1);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 4);
    r.arm_capture(0, true).unwrap();
    producer.push_samples(&[0.3; 7]);
    r.service().unwrap();
    render(&mut r, 10);
    assert!(r.snapshot().capture_invalid);
    r.clear_session().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(!r.capture_busy());
    assert!(r.song().bars[0].audio_buffer.is_none());
    assert!(!r.snapshot().capture_invalid);
}

#[test]
fn fractional_and_eighth_meter_count_in_and_capture_end_on_exact_frames_without_allocating() {
    for (rate, bpm, beats, unit, measures) in [
        (1000, 137.0, 4, 4, 1),
        (48_000, 120.0, 3, 8, 2),
        (48_000, 113.5, 5, 4, 2),
    ] {
        let bar = Bar {
            bpm,
            time_signature: TimeSignature::new(beats, unit),
            length: measures,
            ..Default::default()
        };
        let measure = audio_primitives::frames_per_bar(rate, bpm, beats, unit);
        let frames = measure * measures as usize;
        let mut r = OutputRuntime::offline(rate).unwrap();
        let (mut producer, _) = feed(&mut r, frames + measure + 16);
        let mut s = Song::new();
        s.bars = vec![bar];
        s.click_enabled = true;
        s.playing = true;
        r.set_song(s).unwrap();
        r.service().unwrap();
        r.arm_capture(0, true).unwrap();
        r.service().unwrap();
        render(&mut r, measure);
        assert_eq!(r.snapshot().cursor.sample_in_bar, 0);
        // Existing schedule: a queued Arm starts at the first bar wrap, not
        // at the end of count-in. Capture exactly the following block.
        render(&mut r, frames);
        assert!(r.snapshot().recording);
        producer.push_samples(&vec![0.25; frames]);
        render(&mut r, frames - 1);
        assert_eq!(r.snapshot().cursor.sample_in_bar, frames as u64 - 1);
        render(&mut r, 1);
        r.stop_capture_now().unwrap();
        r.service().unwrap();
        render(&mut r, 1);
        r.service().unwrap();
        assert!(!r.capture_busy());
        assert!(!r.snapshot().capture_invalid);
        let captured = r.song().bars[0].audio_buffer.clone().unwrap();
        assert_eq!(captured.len(), frames);
        assert!(captured.data.iter().all(|sample| *sample == 0.25));
    }
}

#[test]
fn legacy_capture_plays_exactly_but_clear_is_required_before_new_recording() {
    let mut r = OutputRuntime::offline(48_000).unwrap();
    let _ = feed(&mut r, 16);
    let mut s = Song::new();
    let pcm = SampleBuffer::new(vec![0.25; 253_744], 48_000);
    s.bars = vec![Bar {
        bpm: 113.5,
        time_signature: TimeSignature::new(5, 4),
        length: 2,
        legacy_clock: true,
        audio_buffer: Some(pcm.clone()),
        ..Default::default()
    }];
    s.click_enabled = false;
    s.playing = true;
    s.one_shot = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    assert!(
        r.arm_capture(0, true)
            .unwrap_err()
            .to_string()
            .contains("legacy-clock")
    );
    let output = render(&mut r, 253_745);
    assert!(
        output[..253_744 * 2]
            .iter()
            .all(|value| (*value - 0.2).abs() < 1e-6)
    );
    assert!(output[253_744 * 2..].iter().all(|value| *value == 0.0));
    assert!(Arc::ptr_eq(
        &r.song().bars[0].audio_buffer.as_ref().unwrap().data,
        &pcm.data
    ));
    r.clear_recording(0).unwrap();
    assert!(!r.song().bars[0].legacy_clock);
    assert_eq!(r.song().bars[0].duration_samples(48_000.0), 253_740);
    r.arm_capture(0, true).unwrap();
}
