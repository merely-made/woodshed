use super::*;

#[test]
fn grouped_strum_offsets_share_one_onset_and_all_256_legacy_pitch_slots_are_supported() {
    let mut r = runtime();
    let events = (0..256)
        .map(|index| ProgramEvent {
            token: 77,
            onset: ProgramPosition::BeatTicks(0),
            onset_offset_frames: index as u64 * 18,
            gate: ProgramDuration::UntilCardEnd,
            frequency_hz: 100.0 + index as f64,
            amplitude: 0.1,
        })
        .collect();
    let plan = ProgramPlan {
        cards: vec![ProgramCard {
            token: 88,
            bpm: 120.0,
            end: ProgramEnd::Manual {
                preview_ticks: 0,
                tail_frames: 5000,
            },
            events,
        }],
        start_index: 0,
        loop_mode: ProgramLoop::Once,
    };
    let prepared = r.prepare_program(plan).unwrap();
    let epoch = r.start_program(prepared).unwrap();
    r.service().unwrap();
    render(&mut r, 5000);
    let snapshot = r.snapshot().program;
    assert_eq!(snapshot.epoch, epoch);
    assert_eq!(snapshot.event_index, 256);
    assert_eq!(snapshot.event_token, Some(77));
}

#[test]
fn prepared_integer_cycle_tone_meets_frequency_error_and_has_bounded_loop_seam() {
    let plan = ProgramPlan {
        cards: vec![ProgramCard {
            token: 1,
            bpm: 120.0,
            end: ProgramEnd::DeviceFrames(100),
            events: vec![ProgramEvent {
                token: 1,
                onset: ProgramPosition::DeviceFrames(0),
                onset_offset_frames: 0,
                gate: ProgramDuration::UntilCardEnd,
                frequency_hz: 440.123,
                amplitude: 0.5,
            }],
        }],
        start_index: 0,
        loop_mode: ProgramLoop::Once,
    };
    let prepared = PreparedProgram::prepare(plan, 48_000).unwrap();
    let tone = &prepared.tones[0];
    assert!(tone.cycles > 0);
    assert!(!tone.pcm.is_empty());
    assert!(tone.pcm.len() <= clock::MAX_PROGRAM_TONE_FRAMES);
    assert!((tone.actual_frequency_hz - 440.123).abs() / 440.123 <= 1.0e-6);
    let seam_step_bound = std::f32::consts::TAU * tone.cycles as f32 / tone.pcm.len() as f32
        * (1.0 + 2.0 * 0.25 + 3.0 * 0.12);
    let seam_delta = (tone.pcm.data[0] - *tone.pcm.data.last().unwrap()).abs();
    assert!(seam_delta <= seam_step_bound + f32::EPSILON);
}

#[test]
fn fallback_g2_tone_is_admitted_at_common_device_rates_without_changing_fast_path() {
    let frequency = 97.998_855_590_820_31;
    for rate in [44_100, 48_000, 96_000] {
        let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(100), 120.0);
        plan.cards[0].events[0].frequency_hz = frequency;
        let prepared = PreparedProgram::prepare(plan, rate).unwrap();
        let tone = &prepared.tones[0];
        assert!(tone.pcm.len() <= clock::MAX_PROGRAM_TONE_FRAMES);
        assert_eq!(tone.requested_frequency_hz, frequency);
        assert!((tone.actual_frequency_hz - frequency).abs() / frequency <= 1.0e-6);
        if rate == 44_100 {
            assert!(tone.phase_step_q64.is_some());
            assert_eq!(tone.pcm.len(), clock::FALLBACK_PERIODIC_WAVE_FRAMES);
            assert_eq!(tone.cycles, 0);
            assert!((tone.pcm.data[1024] - 0.88).abs() < 1.0e-6);
        } else {
            assert_eq!(tone.phase_step_q64, None);
            assert!(tone.cycles > 0);
            assert!(tone.pcm.len() <= clock::MAX_PROGRAM_TONE_FRAMES);
        }
    }
}

#[test]
fn fractional_phase_fallback_preserves_pitch_and_waveform_across_blocks() {
    const RATE: u32 = 44_100;
    const FREQUENCY: f64 = 97.998_855_590_820_31;
    fn started() -> OutputRuntime {
        let mut runtime = OutputRuntime::offline(RATE).unwrap();
        render(&mut runtime, 1);
        runtime.service().unwrap();
        let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(88_200), 120.0);
        plan.cards[0].events[0].frequency_hz = FREQUENCY;
        let prepared = runtime.prepare_program(plan).unwrap();
        runtime.start_program(prepared).unwrap();
        runtime.service().unwrap();
        runtime
    }

    let mut whole = started();
    let all = render(&mut whole, 66_000);
    let mono = all
        .chunks_exact(2)
        .map(|frame| frame[0])
        .collect::<Vec<_>>();
    let rising = (2_000..mono.len() - 1)
        .filter(|index| mono[*index - 1] < 0.0 && mono[*index] >= 0.0)
        .collect::<Vec<_>>();
    let first = *rising.first().unwrap() as f64;
    let last = *rising.last().unwrap() as f64;
    let measured = (rising.len() - 1) as f64 * RATE as f64 / (last - first);
    assert!((measured - FREQUENCY).abs() < 0.2, "measured {measured} Hz");
    assert!(mono.iter().any(|sample| sample.abs() > 1.0e-4));

    let mut chunked = started();
    let mut pieces = Vec::new();
    for count in [1, 17, 128, 4093, 257, 8191, 2_000, 51_313] {
        let block = render(&mut chunked, count);
        pieces.extend(block);
    }
    assert_eq!(pieces, all);
}

#[test]
fn fractional_phase_program_clear_stops_at_next_callback_boundary() {
    let mut runtime = OutputRuntime::offline(44_100).unwrap();
    render(&mut runtime, 1);
    runtime.service().unwrap();
    let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(64), 120.0);
    plan.cards[0].events[0].frequency_hz = 97.998_855_590_820_31;
    let epoch = runtime
        .start_program(runtime.prepare_program(plan).unwrap())
        .unwrap();
    runtime.service().unwrap();
    assert!(
        render(&mut runtime, 16)
            .iter()
            .any(|sample| sample.abs() > 1.0e-4)
    );
    runtime.clear_session().unwrap();
    runtime.service().unwrap();
    assert!(render(&mut runtime, 8).iter().all(|sample| *sample == 0.0));
    assert_eq!(runtime.snapshot().program.status, ProgramStatus::Stopped);
    assert!(
        runtime
            .drain_program_transitions()
            .iter()
            .any(|transition| {
                transition.epoch == epoch
                    && transition.kind == ProgramTransitionKind::Ended(ProgramEndReason::Stop)
            })
    );
}

#[test]
fn fractional_phase_program_honors_device_duration_and_ends_silent() {
    let mut runtime = OutputRuntime::offline(44_100).unwrap();
    render(&mut runtime, 1);
    runtime.service().unwrap();
    let plan = ProgramPlan {
        cards: vec![ProgramCard {
            token: 1,
            bpm: 120.0,
            end: ProgramEnd::DeviceFrames(64),
            events: vec![ProgramEvent {
                token: 1,
                onset: ProgramPosition::DeviceFrames(0),
                onset_offset_frames: 0,
                gate: ProgramDuration::DeviceFrames(64),
                frequency_hz: 97.998_855_590_820_31,
                amplitude: 0.5,
            }],
        }],
        start_index: 0,
        loop_mode: ProgramLoop::Once,
    };
    runtime
        .start_program(runtime.prepare_program(plan).unwrap())
        .unwrap();
    runtime.service().unwrap();
    assert!(
        render(&mut runtime, 63)
            .iter()
            .any(|sample| sample.abs() > 1.0e-5)
    );
    assert_eq!(runtime.snapshot().program.card_frames, 63);
    assert_eq!(runtime.snapshot().program.status, ProgramStatus::Playing);
    render(&mut runtime, 1);
    assert_eq!(runtime.snapshot().program.card_frames, 64);
    assert_eq!(runtime.snapshot().program.status, ProgramStatus::Complete);
    let tail = render(&mut runtime, 11_100);
    assert!(tail[tail.len() - 256..].iter().all(|sample| *sample == 0.0));
    assert_eq!(runtime.snapshot().program.status, ProgramStatus::Complete);
}

#[test]
fn invalid_and_nyquist_program_tones_remain_refused_without_fallback() {
    for frequency in [0.0, -10.0, f64::NAN, f64::INFINITY, 22_050.0, 30_000.0] {
        let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(100), 120.0);
        plan.cards[0].events[0].frequency_hz = frequency;
        let prepared = PreparedProgram::prepare(plan, 44_100).unwrap();
        assert_eq!(prepared.cards[0].end, ProgramEnd::Refusal(1));
        assert!(prepared.refusal_reason(0).is_some());
        assert!(prepared.tones.is_empty());
    }
}

#[test]
fn low_frequency_valid_tone_uses_fallback_instead_of_refusal() {
    let mut plan = program_with_card_count(2, ProgramEnd::DeviceFrames(100), 120.0);
    plan.cards[0].events[0].frequency_hz = 0.01;
    let prepared = PreparedProgram::prepare(plan, 1000).unwrap();
    assert_eq!(prepared.cards[0].end, ProgramEnd::DeviceFrames(100));
    assert!(prepared.refusal_reason(0).is_none());
    assert!(prepared.tones[0].phase_step_q64.is_some());
    assert_eq!(prepared.cards[1].end, ProgramEnd::DeviceFrames(100));
}

#[test]
fn fixed_strum_offset_must_fit_before_the_card_ends() {
    let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(10), 120.0);
    plan.cards[0].events[0].onset = ProgramPosition::DeviceFrames(8);
    plan.cards[0].events[0].onset_offset_frames = 2;
    assert!(PreparedProgram::prepare(plan, 1000).is_err());
}
