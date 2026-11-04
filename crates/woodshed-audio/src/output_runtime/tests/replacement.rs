use super::*;

fn replacement_plan(first_hz: f64, next_hz: f64) -> ProgramPlan {
    let mut plan = program_with_card_count(3, ProgramEnd::DeviceFrames(400), 120.0);
    for (bar, card) in plan.cards.iter_mut().enumerate() {
        card.events = vec![ProgramEvent {
            token: 1,
            onset: ProgramPosition::DeviceFrames(0),
            onset_offset_frames: 0,
            gate: ProgramDuration::DeviceFrames(100),
            frequency_hz: if bar == 0 { first_hz } else { next_hz },
            amplitude: 0.3,
        }];
        if bar == 1 {
            // Simultaneous chord followed by another gated event, then silence.
            let mut chord = card.events[0].clone();
            chord.frequency_hz = 250.0;
            card.events.push(chord);
            card.events.push(ProgramEvent {
                token: 2,
                onset: ProgramPosition::DeviceFrames(200),
                onset_offset_frames: 0,
                gate: ProgramDuration::DeviceFrames(40),
                frequency_hz: 300.0,
                amplitude: 0.2,
            });
        }
    }
    plan
}

fn start_replacement_fixture(r: &mut OutputRuntime, plan: ProgramPlan) -> ProgramEpoch {
    let prepared = r.prepare_program(plan).unwrap();
    let epoch = r.start_program(prepared).unwrap();
    r.service().unwrap();
    epoch
}

#[test]
fn boundary_replacement_preserves_current_pcm_then_uses_new_chords_gates_and_rests() {
    let mut actual = runtime();
    let mut reference = runtime();
    let epoch = start_replacement_fixture(&mut actual, replacement_plan(100.0, 150.0));
    start_replacement_fixture(&mut reference, replacement_plan(100.0, 200.0));
    assert_eq!(render(&mut actual, 100), render(&mut reference, 100));
    let prepared = actual
        .prepare_program(replacement_plan(180.0, 200.0))
        .unwrap();
    actual
        .replace_program_at_boundary(epoch, 1, prepared)
        .unwrap();
    actual.service().unwrap();
    assert_eq!(actual.snapshot().program.revision, 0);
    let before = render(&mut actual, 299);
    assert_eq!(before, render(&mut reference, 299));
    assert_eq!(actual.snapshot().program.revision, 0);
    assert!(before[500..].iter().all(|x| x.abs() < 0.00001));
    assert_eq!(render(&mut actual, 2), render(&mut reference, 2));
    assert_eq!(actual.snapshot().program.revision, 1);
    assert_eq!(actual.snapshot().program.card_index, 1);
    assert_eq!(render(&mut actual, 805), render(&mut reference, 805));
    assert_eq!(actual.snapshot().program.status, ProgramStatus::Complete);
}

#[test]
fn latest_boundary_revision_wins_and_pause_waits_for_natural_end() {
    let mut r = runtime();
    let epoch = start_replacement_fixture(&mut r, replacement_plan(100.0, 150.0));
    render(&mut r, 40);
    let first = r.prepare_program(replacement_plan(180.0, 190.0)).unwrap();
    let latest = r.prepare_program(replacement_plan(200.0, 210.0)).unwrap();
    r.replace_program_at_boundary(epoch, 1, first).unwrap();
    r.replace_program_at_boundary(epoch, 2, latest).unwrap();
    r.pause_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 500);
    assert_eq!(r.snapshot().program.revision, 0);
    assert_eq!(r.snapshot().program.card_frames, 40);
    r.resume_program(epoch).unwrap();
    r.service().unwrap();
    render(&mut r, 361);
    assert_eq!(r.snapshot().program.revision, 2);
    assert_eq!(r.snapshot().program.card_index, 1);
    assert_eq!(r.snapshot().program.event_token, Some(1));
    let stale = r.prepare_program(replacement_plan(100.0, 150.0)).unwrap();
    assert!(r.replace_program_at_boundary(epoch, 1, stale).is_err());
}

#[test]
fn replacement_tempo_controls_coexist_and_changed_length_is_observable() {
    let mut r = runtime();
    let mut initial = program_with_card_count(2, ProgramEnd::BeatTicks(960), 120.0);
    initial.cards[0].events.clear();
    let epoch = start_replacement_fixture(&mut r, initial.clone());
    render(&mut r, 100);
    let mut replacement = initial.clone();
    replacement.cards.push(initial.cards[1].clone());
    replacement.cards[2].token = 10_002;
    let prepared = r.prepare_program(replacement).unwrap();
    r.replace_program_at_boundary(epoch, 1, prepared).unwrap();
    r.set_program_tempo(epoch, 0, 240.0).unwrap();
    r.set_program_tempo(epoch, 1, 300.0).unwrap();
    r.service().unwrap();
    render(&mut r, 210);
    let snapshot = r.snapshot().program;
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.card_index, 1);
    assert_eq!(snapshot.card_count, 3);
    assert_eq!(f64::from_bits(snapshot.bpm_bits), 300.0);
    r.set_program_tempo(epoch, 2, 150.0).unwrap();
    r.seek_program(epoch, 2).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().program.card_index, 2);
    assert_eq!(f64::from_bits(r.snapshot().program.bpm_bits), 150.0);
}

#[test]
fn boundary_replacement_prepare_failure_stale_epoch_seek_stop_and_advance_are_safe() {
    let mut r = runtime();
    let epoch = start_replacement_fixture(&mut r, replacement_plan(100.0, 150.0));
    render(&mut r, 30);
    let mut invalid = replacement_plan(100.0, 180.0);
    invalid.cards[1].events[0].gate = ProgramDuration::DeviceFrames(u64::MAX);
    assert!(r.prepare_program(invalid).is_err());
    let prepared = r.prepare_program(replacement_plan(100.0, 180.0)).unwrap();
    assert!(
        r.replace_program_at_boundary(ProgramEpoch(epoch.0 + 1), 1, prepared)
            .is_err()
    );
    for action in [0, 1, 2] {
        let prepared = r.prepare_program(replacement_plan(100.0, 180.0)).unwrap();
        r.replace_program_at_boundary(epoch, action + 1, prepared)
            .unwrap();
        match action {
            0 => r.seek_program(epoch, 0).unwrap(),
            1 => r.advance_program(epoch).unwrap(),
            _ => r.stop_program(epoch).unwrap(),
        }
        r.service().unwrap();
        render(&mut r, 1);
        assert_eq!(r.snapshot().program.revision, 0);
    }
    assert_eq!(r.snapshot().program.status, ProgramStatus::Stopped);
    r.drain_program_transitions();
    let next = start_replacement_fixture(&mut r, replacement_plan(100.0, 150.0));
    render(&mut r, 1);
    let prepared = r.prepare_program(replacement_plan(100.0, 180.0)).unwrap();
    assert!(r.replace_program_at_boundary(epoch, 4, prepared).is_err());
    assert_eq!(r.snapshot().program.epoch, next);
    let stale = r.prepare_program(replacement_plan(100.0, 180.0)).unwrap();
    r.send(Command::ProgramReplace {
        epoch,
        revision: 99,
        data: Some(OwnedGc::new(stale)),
    })
    .unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().program.revision, 0);
    assert_eq!(r.snapshot().program.epoch, next);
}

#[test]
fn shortened_replacement_without_next_ordinal_restarts_edited_program_once() {
    let mut r = runtime();
    let epoch = start_replacement_fixture(&mut r, replacement_plan(100.0, 150.0));
    render(&mut r, 1);
    let mut short = replacement_plan(200.0, 210.0);
    short.cards.truncate(1);
    short.loop_mode = ProgramLoop::Set;
    let prepared = r.prepare_program(short).unwrap();
    r.replace_program_at_boundary(epoch, 7, prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 400);
    assert_eq!(r.snapshot().program.revision, 7);
    assert_eq!(r.snapshot().program.card_count, 1);
    assert_eq!(r.snapshot().program.card_index, 0);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Playing);
    // Replacement loop_mode is not a transport command: the original Once
    // mode completes after playing the edited version from its start.
    render(&mut r, 400);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Complete);
}

#[test]
fn observer_pressure_fences_replacement_and_clear_cancels_it_without_callback_allocations() {
    let mut r = runtime();
    let mut plan = program_with_card_count(130, ProgramEnd::DeviceFrames(1), 120.0);
    for c in &mut plan.cards {
        c.events.clear();
    }
    let epoch = start_replacement_fixture(&mut r, plan.clone());
    render(&mut r, 260);
    assert_eq!(r.snapshot().program.status, ProgramStatus::WaitingObserver);
    let prepared = r.prepare_program(plan.clone()).unwrap();
    r.replace_program_at_boundary(epoch, 1, prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 10);
    assert_eq!(r.snapshot().program.revision, 0);
    r.drain_program_transitions();
    render(&mut r, 10);
    assert_eq!(r.snapshot().program.revision, 1);
    // A new epoch plus observer pressure also exercises session clear's
    // dedicated cancellation fence rather than applying the pending version.
    r.drain_program_transitions();
    let next = start_replacement_fixture(&mut r, plan.clone());
    render(&mut r, 260);
    let prepared = r.prepare_program(plan).unwrap();
    r.replace_program_at_boundary(next, 2, prepared).unwrap();
    r.clear_session().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.drain_program_transitions();
    render(&mut r, 5);
    assert_eq!(r.snapshot().program.revision, 0);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Stopped);
}

#[test]
fn observer_deferred_seek_or_stop_cancels_pending_replacement_before_acknowledgement() {
    for stop in [false, true] {
        let mut r = runtime();
        let mut plan = program_with_card_count(130, ProgramEnd::DeviceFrames(1), 120.0);
        for card in &mut plan.cards {
            card.events.clear();
        }
        let epoch = start_replacement_fixture(&mut r, plan.clone());
        render(&mut r, 260);
        assert_eq!(r.snapshot().program.status, ProgramStatus::WaitingObserver);
        let prepared = r.prepare_program(plan).unwrap();
        r.replace_program_at_boundary(epoch, 1, prepared).unwrap();
        // Commands can have been accepted before the host observed pressure.
        r.send(if stop {
            Command::ProgramStop { epoch }
        } else {
            Command::ProgramSeek { epoch, card: 128 }
        })
        .unwrap();
        r.service().unwrap();
        render(&mut r, 1);
        r.drain_program_transitions();
        render(&mut r, 5);
        assert_eq!(r.snapshot().program.revision, 0);
        if stop {
            assert_eq!(r.snapshot().program.status, ProgramStatus::Stopped);
        }
    }
}

#[test]
fn batch_program_tempo_preserves_phase_and_updates_added_pending_segments_in_one_command() {
    let mut r = runtime();
    let mut initial = program_with_card_count(2, ProgramEnd::BeatTicks(960), 120.0);
    for card in &mut initial.cards {
        card.events.clear();
    }
    let epoch = start_replacement_fixture(&mut r, initial.clone());
    render(&mut r, 100);
    let before = r.snapshot().program;
    let mut next = initial.clone();
    next.cards.push(initial.cards[1].clone());
    next.cards[2].token = 10_002;
    let prepared = r.prepare_program(next).unwrap();
    r.replace_program_at_boundary(epoch, 1, prepared).unwrap();
    r.set_program_tempo_all(epoch, 240.0).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let after = r.snapshot().program;
    assert_eq!(after.card_frames, before.card_frames + 1);
    assert!(after.beat_ticks >= before.beat_ticks + 3 && after.beat_ticks <= before.beat_ticks + 4);
    assert_eq!(f64::from_bits(after.bpm_bits), 240.0);
    render(&mut r, 210);
    assert_eq!(r.snapshot().program.revision, 1);
    assert_eq!(r.snapshot().program.card_count, 3);
    assert_eq!(f64::from_bits(r.snapshot().program.bpm_bits), 240.0);
    r.seek_program(epoch, 2).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(f64::from_bits(r.snapshot().program.bpm_bits), 240.0);
    for bpm in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        assert!(r.set_program_tempo_all(epoch, bpm).is_err());
    }
    // A complete 256-segment program still needs exactly one tempo command.
    let plan = program_with_card_count(256, ProgramEnd::DeviceFrames(1), 120.0);
    let mut other = runtime();
    let next_epoch = start_replacement_fixture(&mut other, plan);
    other.set_program_tempo_all(next_epoch, 180.0).unwrap();
    other.service().unwrap();
    render(&mut other, 1);
    assert_eq!(f64::from_bits(other.snapshot().program.bpm_bits), 180.0);
}

#[test]
fn card_repeat_replacement_keeps_current_ordinal_and_applies_edited_segment() {
    let mut r = runtime();
    let mut original = replacement_plan(100.0, 150.0);
    original.loop_mode = ProgramLoop::Card;
    let epoch = start_replacement_fixture(&mut r, original);
    render(&mut r, 30);
    let mut edited = replacement_plan(180.0, 200.0);
    edited.cards[0].token = 88;
    // Replacement loop_mode/start_index are not independent transport changes.
    edited.loop_mode = ProgramLoop::Once;
    edited.start_index = 2;
    let prepared = r.prepare_program(edited).unwrap();
    r.replace_program_at_boundary(epoch, 1, prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 371);
    let after = r.snapshot().program;
    assert_eq!(after.revision, 1);
    assert_eq!(after.card_index, 0);
    assert_eq!(after.card_token, Some(88));
    render(&mut r, 400);
    assert_eq!(r.snapshot().program.card_index, 0);
    assert_eq!(r.snapshot().program.status, ProgramStatus::Playing);
}
