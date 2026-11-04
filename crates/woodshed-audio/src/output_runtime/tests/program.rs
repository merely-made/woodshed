use super::*;

#[test]
fn idle_attached_capture_does_not_block_program_after_take_is_retired() {
    let mut r = runtime();
    let (mut producer, _) = feed(&mut r, 64);
    let mut s = song(1);
    s.playing = true;
    r.set_song(s).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.arm_capture(0, true).unwrap();
    producer.push_samples(&[0.25; 64]);
    r.service().unwrap();
    render(&mut r, 12);
    assert!(
        r.snapshot().recording,
        "fixture must actually enter the recording state"
    );
    r.stop_capture_now().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    assert!(
        !r.capture_busy(),
        "recording owner must be retired before Run"
    );
    assert!(r.capture_attached, "the idle input stream remains attached");
    r.stop().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let prepared = r
        .prepare_program(program_with_card_count(1, ProgramEnd::BeatTicks(960), 60.0))
        .unwrap();
    let epoch = r.start_program(prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().program.epoch, epoch);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Playing);
}

#[test]
fn arbitrary_finite_tempo_edits_preserve_beat_phase_and_loop_changes_do_not_restart() {
    let mut r = runtime();
    let prepared = r
        .prepare_program(program_with_card_count(2, ProgramEnd::BeatTicks(960), 10.0))
        .unwrap();
    let epoch = r.start_program(prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    render(&mut r, 24);
    let before = r.snapshot().program;
    assert!(before.beat_ticks > 0);
    assert_eq!(f64::from_bits(before.bpm_bits), 10.0);
    assert_eq!(before.click_subdivision_ticks, PROGRAM_TICKS_PER_BEAT);
    r.pause_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let paused = r.snapshot().program;
    assert_eq!(paused.status, ProgramStatus::Paused);
    r.set_program_tempo(epoch, 0, 401.0).unwrap();
    r.set_program_loop(epoch, ProgramLoop::Set).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let after = r.snapshot().program;
    assert_eq!(
        after.beat_ticks, paused.beat_ticks,
        "tempo edit while paused preserves exact beat position"
    );
    assert_eq!(
        f64::from_bits(after.bpm_bits),
        401.0,
        "snapshot publishes the applied host-visible tempo"
    );
    assert_eq!(
        after.card_token, before.card_token,
        "loop edit cannot restart or re-credit the active card"
    );
    r.resume_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert!(r.snapshot().program.beat_ticks >= paused.beat_ticks + 6);
    assert!(r.set_program_tempo(epoch, 0, 0.0).is_err());
    assert!(r.set_program_tempo(epoch, 0, f64::MAX).is_err());
}

#[test]
fn manual_program_keeps_started_span_open_until_explicit_advance() {
    let mut r = runtime();
    let mut plan = program_with_card_count(
        2,
        ProgramEnd::Manual {
            preview_ticks: 0,
            tail_frames: 5,
        },
        120.0,
    );
    plan.cards[0].events[0].gate = ProgramDuration::UntilCardEnd;
    let epoch = r.start_program(r.prepare_program(plan).unwrap()).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let transitions = r.drain_program_transitions();
    assert!(
        transitions
            .iter()
            .any(|t| t.epoch == epoch && t.kind == ProgramTransitionKind::Started)
    );
    render(&mut r, 6);
    let waiting = r.snapshot().program;
    assert_eq!(waiting.status, ProgramStatus::WaitingManual);
    assert!(waiting.active_frames >= 6);
    assert_eq!(
        waiting.card_frames, 6,
        "card clock freezes once the manual tail ends"
    );
    assert!(
        !r.drain_program_transitions()
            .iter()
            .any(|t| matches!(t.kind, ProgramTransitionKind::Ended(_)))
    );
    render(&mut r, 4);
    assert!(r.snapshot().program.active_frames >= waiting.active_frames + 4);
    r.advance_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert!(
        r.drain_program_transitions()
            .iter()
            .any(|t| t.kind == ProgramTransitionKind::Ended(ProgramEndReason::Completed))
    );
    assert_eq!(r.snapshot().program.card_index, 1);
}

#[test]
fn focused_card_auto_repeats_but_explicit_next_leaves_it_even_while_paused() {
    let mut r = runtime();
    let mut plan = program_with_card_count(2, ProgramEnd::DeviceFrames(10), 120.0);
    plan.loop_mode = ProgramLoop::Card;
    let epoch = r.start_program(r.prepare_program(plan).unwrap()).unwrap();
    r.service().unwrap();
    render(&mut r, 35);
    assert_eq!(r.snapshot().program.card_index, 0);
    let transitions = r.drain_program_transitions();
    assert!(
        transitions
            .iter()
            .filter(|t| t.kind == ProgramTransitionKind::Started)
            .count()
            >= 3
    );
    assert!(transitions.iter().all(|t| t.card_token == 10_000));
    r.pause_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.advance_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let selected = r.snapshot().program;
    assert_eq!(selected.card_index, 1);
    assert_eq!(selected.status, ProgramStatus::Paused);
    assert!(
        !r.drain_program_transitions()
            .iter()
            .any(|t| t.card_token == 10_001 && t.kind == ProgramTransitionKind::Started)
    );
    r.resume_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 35);
    assert_eq!(r.snapshot().program.card_index, 1);
    assert!(
        r.drain_program_transitions()
            .iter()
            .filter(|t| t.card_token == 10_001 && t.kind == ProgramTransitionKind::Started)
            .count()
            >= 3
    );
    r.advance_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Complete);
}

#[test]
fn repeated_paused_navigation_never_starts_an_unheard_card_before_resume() {
    let mut r = runtime();
    let epoch = r
        .start_program(
            r.prepare_program(program_with_card_count(
                4,
                ProgramEnd::DeviceFrames(10_000),
                120.0,
            ))
            .unwrap(),
        )
        .unwrap();
    r.service().unwrap();
    render(&mut r, 10);
    r.drain_program_transitions();
    r.pause_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    for target in [1, 2] {
        r.advance_program(epoch).unwrap();
        r.service().unwrap();
        render(&mut r, 1);
        assert_eq!(r.snapshot().program.card_index, target);
        assert_eq!(r.snapshot().program.status, ProgramStatus::Paused);
    }
    for target in [0, 3] {
        r.seek_program(epoch, target).unwrap();
        r.service().unwrap();
        render(&mut r, 1);
        assert_eq!(r.snapshot().program.card_index, target);
        assert_eq!(r.snapshot().program.status, ProgramStatus::Paused);
    }
    assert!(
        !r.drain_program_transitions()
            .iter()
            .any(|t| t.kind == ProgramTransitionKind::Started)
    );
    r.resume_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let started = r.drain_program_transitions();
    assert_eq!(
        started
            .iter()
            .filter(|t| t.kind == ProgramTransitionKind::Started && t.card_token == 10_003)
            .count(),
        1
    );
}

#[test]
fn explicit_next_completes_a_timed_card_once_and_starts_the_next_card() {
    let mut r = runtime();
    let epoch = r
        .start_program(
            r.prepare_program(program_with_card_count(
                2,
                ProgramEnd::DeviceFrames(10_000),
                120.0,
            ))
            .unwrap(),
        )
        .unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let first = r.drain_program_transitions();
    assert_eq!(
        first
            .iter()
            .filter(|t| t.kind == ProgramTransitionKind::Started)
            .count(),
        1
    );
    r.advance_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let next = r.drain_program_transitions();
    assert_eq!(
        next.iter()
            .filter(|t| t.kind == ProgramTransitionKind::Ended(ProgramEndReason::Completed))
            .count(),
        1,
        "explicit Next closes the accepted partial span exactly once"
    );
    assert_eq!(
        next.iter()
            .filter(|t| t.kind == ProgramTransitionKind::Started)
            .count(),
        1,
        "Next starts the following card without changing ownership"
    );
    assert_eq!(r.snapshot().program.card_index, 1);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Playing);
}

#[test]
fn pause_freezes_program_position_and_pcm_then_resume_continues_the_same_event() {
    fn started() -> (OutputRuntime, ProgramEpoch) {
        let mut r = runtime();
        let mut pattern = SequencerPattern::metronome(
            120.0,
            TimeSignature::new(1, 4),
            crate::Subdivision::QUARTER,
        );
        pattern.tracks[0].sound =
            Sound::sample_with_buffer("pause-click", SampleBuffer::new(vec![0.125; 128], 1000));
        if let Sound::Sample { accent_gain, .. } = &mut pattern.tracks[0].sound {
            *accent_gain = 1.0;
        }
        r.set_click_pattern_playing(pattern, true).unwrap();
        let epoch = r
            .start_program(
                r.prepare_program(program_with_card_count(
                    1,
                    ProgramEnd::DeviceFrames(800),
                    120.0,
                ))
                .unwrap(),
            )
            .unwrap();
        r.service().unwrap();
        render(&mut r, 24);
        (r, epoch)
    }

    let mut continuous = started().0;
    let expected = render(&mut continuous, 32);
    let (mut paused_runtime, epoch) = started();
    let before = paused_runtime.snapshot().program;
    assert_eq!(before.event_index, 1);
    paused_runtime.pause_program(epoch).unwrap();
    paused_runtime.service().unwrap();
    let silent = render(&mut paused_runtime, 50);
    assert!(
        silent.iter().all(|sample| *sample == 0.0),
        "pause freezes both program and an already-running click tail"
    );
    let paused = paused_runtime.snapshot().program;
    assert_eq!(paused.status, ProgramStatus::Paused);
    assert_eq!(paused.beat_ticks, before.beat_ticks);
    assert_eq!(paused.card_frames, before.card_frames);
    assert_eq!(paused.event_token, before.event_token);
    paused_runtime.resume_program(epoch).unwrap();
    paused_runtime.service().unwrap();
    assert_eq!(
        render(&mut paused_runtime, 32),
        expected,
        "resume continues the exact PCM phase after the pause gap"
    );
}

#[test]
fn seconds_domain_card_and_gate_keep_fixed_device_frame_duration_across_tempo_edit() {
    let mut r = runtime();
    let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(400), 120.0);
    plan.cards[0].events[0].onset = ProgramPosition::DeviceFrames(0);
    plan.cards[0].events[0].gate = ProgramDuration::DeviceFrames(300);
    let epoch = r.start_program(r.prepare_program(plan).unwrap()).unwrap();
    r.service().unwrap();
    render(&mut r, 100);
    r.set_program_tempo(epoch, 0, 400.0).unwrap();
    r.service().unwrap();
    render(&mut r, 200);
    let middle = r.snapshot().program;
    assert_eq!(middle.card_frames, 300);
    assert_eq!(middle.status, ProgramStatus::Playing);
    render(&mut r, 100);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Complete);
}

#[test]
fn stale_epoch_commands_cannot_pause_or_retime_a_replacement_program() {
    let mut r = runtime();
    let first = r
        .start_program(
            r.prepare_program(program_with_card_count(
                1,
                ProgramEnd::DeviceFrames(10),
                120.0,
            ))
            .unwrap(),
        )
        .unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.stop_program(first).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.drain_program_transitions();
    let second = r
        .start_program(
            r.prepare_program(program_with_card_count(
                1,
                ProgramEnd::DeviceFrames(100),
                120.0,
            ))
            .unwrap(),
        )
        .unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.pause_program(first).unwrap();
    r.set_program_tempo(first, 0, 400.0).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().program.epoch, second);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Playing);
}

#[test]
fn full_transition_queue_freezes_clock_and_resume_preserves_every_card_event() {
    let mut r = runtime();
    let prepared = r
        .prepare_program(program_with_card_count(
            130,
            ProgramEnd::DeviceFrames(1),
            120.0,
        ))
        .unwrap();
    let epoch = r.start_program(prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 260);
    let blocked = r.snapshot().program;
    assert_eq!(blocked.status, ProgramStatus::WaitingObserver);
    assert!(
        r.stop_program(epoch).is_err(),
        "Stop must wait for the observer to drain already-pending transitions"
    );
    assert!(
        r.seek_program(epoch, 129).is_err(),
        "Seek must not be accepted and then lost under queue pressure"
    );
    let queued = r.drain_program_transitions();
    assert_eq!(queued.len(), 256);
    render(&mut r, 260);
    let completed = r.snapshot().program;
    assert_eq!(completed.status, ProgramStatus::Complete);
    let remainder = r.drain_program_transitions();
    assert_eq!(remainder.len(), 4);
    assert_eq!(
        queued
            .iter()
            .filter(|transition| transition.kind == ProgramTransitionKind::Started)
            .count(),
        128
    );
    assert_eq!(
        queued
            .iter()
            .filter(|transition| matches!(
                transition.kind,
                ProgramTransitionKind::Ended(ProgramEndReason::Completed)
            ))
            .count(),
        128
    );
    assert_eq!(
        remainder
            .iter()
            .filter(|transition| transition.kind == ProgramTransitionKind::Started)
            .count(),
        2
    );
    assert_eq!(
        remainder
            .iter()
            .filter(|transition| matches!(
                transition.kind,
                ProgramTransitionKind::Ended(ProgramEndReason::Completed)
            ))
            .count(),
        2
    );
}

#[test]
fn clear_session_silences_and_retires_program_even_when_observer_queue_is_full() {
    let mut r = runtime();
    let prepared = r
        .prepare_program(program_with_card_count(
            130,
            ProgramEnd::DeviceFrames(1),
            120.0,
        ))
        .unwrap();
    let epoch = r.start_program(prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 260);
    assert_eq!(r.snapshot().program.status, ProgramStatus::WaitingObserver);

    // Session reset is a custody boundary: even though the observer must
    // drain the earlier event first, old program PCM stops at this callback.
    r.clear_session().unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 1).iter().all(|sample| *sample == 0.0));
    assert_eq!(r.snapshot().program.status, ProgramStatus::WaitingObserver);

    let first_batch = r.drain_program_transitions();
    assert_eq!(first_batch.len(), 256);
    assert!(render(&mut r, 1).iter().all(|sample| *sample == 0.0));
    assert_eq!(r.snapshot().program.status, ProgramStatus::Stopped);
    let final_batch = r.drain_program_transitions();
    assert!(final_batch.iter().any(|transition| {
        transition.epoch == epoch
            && transition.kind == ProgramTransitionKind::Ended(ProgramEndReason::Stop)
    }));
}

#[test]
fn clear_session_stops_an_audible_program_at_the_next_callback_boundary() {
    let mut r = runtime();
    let epoch = r
        .start_program(
            r.prepare_program(program_with_card_count(
                1,
                ProgramEnd::DeviceFrames(10_000),
                120.0,
            ))
            .unwrap(),
        )
        .unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 2).iter().any(|sample| *sample != 0.0));
    r.clear_session().unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 4).iter().all(|sample| *sample == 0.0));
    assert_eq!(r.snapshot().program.status, ProgramStatus::Stopped);
    assert!(r.drain_program_transitions().iter().any(|transition| {
        transition.epoch == epoch
            && transition.kind == ProgramTransitionKind::Ended(ProgramEndReason::Stop)
    }));
}
