//! Selected recording owns one immutable segment index until capture retirement.
use super::*;

fn selected_song() -> Song {
    let mut s = song(3);
    s.playing = true;
    s.one_shot = true;
    s.bars[1].bpm = 4000.0;
    s.bars[1].time_signature = TimeSignature::new(2, 4);
    s.bars[2].bpm = 137.0;
    s.bars[2].duration_seconds = Some(0.503);
    for bar in &mut s.bars {
        bar.audio_buffer = Some(crate::SampleBuffer::new(
            vec![0.125; bar.checked_duration_frames(1000).unwrap() as usize],
            1000,
        ));
    }
    s
}

fn begin_selected(
    r: &mut OutputRuntime,
    target: usize,
    replace: bool,
) -> (crate::capture_feed::CaptureFeedProducer, Song) {
    let (producer, _) = feed(r, 4096);
    let mut s = selected_song();
    s.record_replace = replace;
    r.set_song(s.clone()).unwrap();
    r.service().unwrap();
    render(r, 4);
    r.arm_selected_capture(target).unwrap();
    (producer, s)
}

#[test]
fn selected_bars_and_seconds_repeat_replace_or_overdub_only_the_original_target() {
    for target in [1, 2] {
        for replace in [false, true] {
            let mut r = runtime();
            let (mut producer, original) = begin_selected(&mut r, target, replace);
            let duration = original.bars[target].checked_duration_frames(1000).unwrap() as usize;
            producer.push_samples(&vec![0.25; 6 + duration * 2 + 16]);
            r.service().unwrap();
            render(&mut r, 6);
            assert_eq!(
                r.snapshot().cursor,
                crate::SongCursor {
                    bar_idx: target,
                    sample_in_bar: 0
                }
            );
            assert!(r.snapshot().recording);
            render(&mut r, duration * 2 + 7);
            let snapshot = r.snapshot();
            assert_eq!(snapshot.cursor.bar_idx, target);
            assert_eq!(snapshot.cursor.sample_in_bar, 7);
            assert!(snapshot.playing && snapshot.recording);
            assert!(!snapshot.capture_invalid);
            assert!(r.set_song(song(1)).is_err());
            r.stop_capture_now().unwrap();
            assert!(
                r.capture_busy(),
                "completion still belongs to the admitted owner"
            );
            r.service().unwrap();
            render(&mut r, 1);
            assert!(
                r.set_song(song(1)).is_err(),
                "collect before allowing replacement"
            );
            r.service().unwrap();
            assert!(!r.capture_busy());
            let captured = r.song();
            for (index, (old, new)) in original.bars.iter().zip(&captured.bars).enumerate() {
                assert_eq!(old.bpm.to_bits(), new.bpm.to_bits());
                assert_eq!(old.duration_seconds, new.duration_seconds);
                assert_eq!(old.time_signature, new.time_signature);
                assert_eq!(old.length, new.length);
                let pcm = new.audio_buffer.as_ref().unwrap();
                if index == target {
                    assert_eq!(pcm.len(), duration);
                    for (frame, sample) in pcm.data.iter().enumerate() {
                        let expected: f32 = if replace {
                            0.25
                        } else if frame < 7 {
                            0.875
                        } else {
                            0.625
                        };
                        assert_eq!(
                            sample.to_bits(),
                            expected.to_bits(),
                            "target {target}, frame {frame}"
                        );
                    }
                } else {
                    assert!(Arc::ptr_eq(
                        &old.audio_buffer.as_ref().unwrap().data,
                        &pcm.data
                    ));
                }
            }
            // Stop releases pinning, without resetting the selected clock.
            render(&mut r, duration - 8);
            if target == 1 {
                assert_eq!(r.snapshot().cursor.bar_idx, 2);
                assert!(r.snapshot().playing);
            } else {
                assert!(
                    !r.snapshot().playing,
                    "ordinary one-shot termination is restored"
                );
            }
        }
    }
}

#[test]
fn selection_and_stale_seek_or_rewind_cannot_redirect_a_reserved_or_active_take() {
    let mut r = runtime();
    let (mut producer, _) = begin_selected(&mut r, 1, true);
    assert!(r.queue(PendingChange::SeekTo { bar_idx: 2 }).is_err());
    assert!(r.rewind().is_err());
    assert!(r.arm_selected_capture(2).is_err());
    // Model commands already in flight: processor protection is independent
    // of the immediate control-side refusal, before and after the first wrap.
    r.send_boundary(Command::Boundary(PendingChange::SeekTo { bar_idx: 2 }))
        .unwrap();
    r.send(Command::Rewind).unwrap();
    producer.push_samples(&[0.25; 200]);
    r.service().unwrap();
    render(&mut r, 6);
    assert_eq!(r.snapshot().cursor.bar_idx, 1);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 0);
    r.send_boundary(Command::Boundary(PendingChange::SeekTo { bar_idx: 0 }))
        .unwrap();
    r.send(Command::Rewind).unwrap();
    r.service().unwrap();
    render(&mut r, 65);
    assert_eq!(r.snapshot().cursor.bar_idx, 1);
    assert_eq!(r.snapshot().cursor.sample_in_bar, 5);
    assert!(r.snapshot().recording && !r.snapshot().capture_invalid);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    // Inspect the actual completion envelope, not a synthesized host result.
    let completed = r.returns.pop().expect("one selected completion");
    assert!(completed.stamp.valid);
    assert_eq!(completed.storage.scope, RecordingScope::Selected(1));
    assert_eq!(
        completed
            .storage
            .bars
            .iter()
            .enumerate()
            .filter_map(|(i, bar)| bar.touched.then_some(i))
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert!(r.returns.pop().is_err());
}

#[test]
fn selected_arm_waits_through_count_in_and_boundary_without_recording_lead_input() {
    let mut r = runtime();
    let (mut producer, _) = feed(&mut r, 2048);
    let mut s = selected_song();
    s.click_enabled = true;
    s.record_replace = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 4); // Six count-in frames and ten body frames remain.
    r.arm_selected_capture(1).unwrap();
    producer.push_samples(&[0.9; 16]);
    r.service().unwrap();
    render(&mut r, 6);
    assert!(r.snapshot().capture_armed && !r.snapshot().recording);
    render(&mut r, 10);
    assert!(r.snapshot().recording);
    assert_eq!(r.snapshot().cursor.bar_idx, 1);
    producer.push_samples(&[0.25; 60]);
    render(&mut r, 60);
    assert!(!r.snapshot().capture_invalid);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(
        r.song().bars[1]
            .audio_buffer
            .as_ref()
            .unwrap()
            .data
            .iter()
            .all(|s| *s == 0.25)
    );
}

#[test]
fn selected_invalid_input_never_publishes_partial_pcm_or_changes_other_takes() {
    for overflow in [false, true] {
        let mut r = runtime();
        let (mut producer, original) = begin_selected(&mut r, 1, true);
        producer.push_samples(&[0.25; 7]);
        r.service().unwrap();
        render(&mut r, 7); // Six discarded lead frames, then one admitted sample.
        if overflow {
            producer.push_samples(&vec![0.5; 5000]);
        }
        render(&mut r, 1);
        assert!(r.snapshot().capture_invalid);
        assert_eq!(r.snapshot().cursor.bar_idx, 1);
        r.stop_capture_now().unwrap();
        r.service().unwrap();
        render(&mut r, 1);
        r.service().unwrap();
        assert!(r.last_error().is_some());
        for (old, new) in original.bars.iter().zip(r.song().bars.iter()) {
            assert!(Arc::ptr_eq(
                &old.audio_buffer.as_ref().unwrap().data,
                &new.audio_buffer.as_ref().unwrap().data
            ));
        }
    }
}

#[test]
fn selected_stop_cancels_unstarted_arm_and_preserves_follow_and_legacy_guards() {
    let mut r = runtime();
    assert!(
        r.arm_selected_capture(0).is_err(),
        "input admission remains required"
    );
    assert!(!r.capture_busy());
    let (_producer, original) = begin_selected(&mut r, 1, true);
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 20);
    r.service().unwrap();
    assert!(!r.capture_busy());
    assert!(!r.snapshot().recording);
    assert!(
        r.returns.pop().is_err(),
        "an unstarted arm cannot publish a take"
    );
    for (old, new) in original.bars.iter().zip(r.song().bars.iter()) {
        assert!(Arc::ptr_eq(
            &old.audio_buffer.as_ref().unwrap().data,
            &new.audio_buffer.as_ref().unwrap().data
        ));
    }
    let mut legacy = selected_song();
    legacy.bars[0].legacy_clock = true;
    r.set_song(legacy).unwrap();
    assert!(
        r.arm_selected_capture(1).is_err(),
        "global legacy refusal is unchanged"
    );
    assert!(!r.capture_busy());
}

#[test]
fn existing_follow_capture_still_crosses_nonuniform_song_segments_with_either_seek_flag() {
    for seek in [false, true] {
        let mut r = runtime();
        let (mut producer, _) = feed(&mut r, 256);
        let mut s = song(3);
        s.playing = true;
        s.bars[1].length = 2;
        s.bars[2].length = 3;
        r.set_song(s).unwrap();
        r.service().unwrap();
        render(&mut r, 4);
        r.arm_capture(1, seek).unwrap();
        producer.push_samples(&[0.25; 100]);
        r.service().unwrap();
        render(&mut r, 6 + 35);
        assert_eq!(r.snapshot().cursor.bar_idx, 2);
        assert_eq!(r.snapshot().cursor.sample_in_bar, 15);
        assert!(r.snapshot().recording);
        r.stop_capture_now().unwrap();
        r.service().unwrap();
        render(&mut r, 1);
        r.service().unwrap();
        let saved = r.song();
        assert!(saved.bars[0].audio_buffer.is_none());
        assert!(
            saved.bars[1]
                .audio_buffer
                .as_ref()
                .unwrap()
                .data
                .iter()
                .all(|s| *s == 0.25)
        );
        let last = &saved.bars[2].audio_buffer.as_ref().unwrap().data;
        assert!(last[..15].iter().all(|s| *s == 0.25));
        assert!(last[15..].iter().all(|s| *s == 0.0));
    }
}
