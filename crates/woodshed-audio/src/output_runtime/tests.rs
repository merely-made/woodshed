use super::*;
use crate::{BackingEvent, BackingPhrase, Bar, ChordRef, TimeSignature};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
    static FREES: Cell<usize> = const { Cell::new(0) };
}
struct CheckedAllocator;
// SAFETY: every operation delegates to System with the original pointer and
// layout. Thread-local counters only observe allocations in guarded rendering.
unsafe impl GlobalAlloc for CheckedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|t| {
            if t.get() {
                ALLOCS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        TRACK.with(|t| {
            if t.get() {
                FREES.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, len: usize) -> *mut u8 {
        TRACK.with(|t| {
            if t.get() {
                ALLOCS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, len) }
    }
}
#[global_allocator]
static ALLOCATOR: CheckedAllocator = CheckedAllocator;

#[test]
fn nonfinite_preview_inputs_are_refused_before_normalization_without_cancelling_valid_work() {
    let mut r = runtime();
    r.preview_pitches(&[220.0], 0.2, 0.0).unwrap();
    let admitted_revision = r.shared.preview_revision.load(Ordering::Acquire);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(r.preview_note(220.0, invalid).is_err());
        assert!(r.preview_pitches(&[220.0], invalid, 0.0).is_err());
        assert!(r.preview_pitches(&[220.0], 0.2, invalid).is_err());
        assert_eq!(
            r.shared.preview_revision.load(Ordering::Acquire),
            admitted_revision,
            "refused input must not cancel or replace already accepted preview work"
        );
    }
    assert!(r.preview_busy());
    // Preserve the existing finite convenience normalization at this audio
    // boundary; imported Card policy is admitted separately by the product.
    assert!(r.preview_note(220.0, 0.0).is_ok());
    assert!(r.preview_pitches(&[220.0], 0.0, -1.0).is_ok());
}

fn render(runtime: &mut OutputRuntime, frames: usize) -> Vec<f32> {
    let mut output = vec![13.0; frames * 2];
    ALLOCS.with(|n| n.set(0));
    FREES.with(|n| n.set(0));
    TRACK.with(|t| t.set(true));
    let result = runtime.render_offline(&mut output);
    TRACK.with(|t| t.set(false));
    result.unwrap();
    assert_eq!(ALLOCS.with(Cell::get), 0, "output processor allocated");
    assert_eq!(FREES.with(Cell::get), 0, "output processor deallocated");
    assert!(output.chunks_exact(2).all(|s| s[0] == s[1]));
    output
}
fn runtime() -> OutputRuntime {
    let mut r = OutputRuntime::offline(1000).unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    r
}
fn song(bars: usize) -> Song {
    let mut song = Song::new();
    song.bars = (0..bars)
        .map(|_| Bar {
            bpm: 6000.0,
            time_signature: TimeSignature::new(1, 4),
            ..Default::default()
        })
        .collect();
    song.click_enabled = false;
    song
}
fn feed(
    runtime: &mut OutputRuntime,
    capacity: usize,
) -> (
    crate::capture_feed::CaptureFeedProducer,
    crate::capture_feed::CaptureFeedControl,
) {
    let (mut producer, consumer, control) = crate::capture_feed::channel(capacity);
    producer.prepare(runtime.rate);
    control.set_enabled(true);
    runtime
        .send(Command::Capture(Some(OwnedGc::new(consumer))))
        .unwrap();
    runtime.capture_attached = true;
    runtime.service().unwrap();
    render(runtime, 1);
    (producer, control)
}

fn program_with_card_count(count: usize, end: ProgramEnd, bpm: f64) -> ProgramPlan {
    ProgramPlan {
        cards: (0..count)
            .map(|index| ProgramCard {
                token: 10_000 + index as u64,
                bpm,
                end,
                events: if index == 0 {
                    vec![ProgramEvent {
                        token: 20_000,
                        onset: match end {
                            ProgramEnd::DeviceFrames(_) => ProgramPosition::DeviceFrames(0),
                            _ => ProgramPosition::BeatTicks(0),
                        },
                        onset_offset_frames: 0,
                        gate: ProgramDuration::UntilCardEnd,
                        frequency_hz: 100.0,
                        amplitude: 1.0,
                    }]
                } else {
                    Vec::new()
                },
            })
            .collect(),
        start_index: 0,
        loop_mode: ProgramLoop::Once,
    }
}

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
fn unrepresentable_future_tone_becomes_a_per_card_refusal_sentinel() {
    let mut plan = program_with_card_count(2, ProgramEnd::DeviceFrames(100), 120.0);
    plan.cards[0].events[0].frequency_hz = 0.01;
    let prepared = PreparedProgram::prepare(plan, 1000).unwrap();
    assert_eq!(prepared.cards[0].end, ProgramEnd::Refusal(1));
    assert!(prepared.refusal_reason(0).is_some());
    assert_eq!(prepared.cards[1].end, ProgramEnd::DeviceFrames(100));
}

#[test]
fn fixed_strum_offset_must_fit_before_the_card_ends() {
    let mut plan = program_with_card_count(1, ProgramEnd::DeviceFrames(10), 120.0);
    plan.cards[0].events[0].onset = ProgramPosition::DeviceFrames(8);
    plan.cards[0].events[0].onset_offset_frames = 2;
    assert!(PreparedProgram::prepare(plan, 1000).is_err());
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

#[test]
fn production_graph_is_silent_stopped_and_has_one_device_clock() {
    let mut r = runtime();
    let frames = r.snapshot().device_frames;
    assert_eq!(r.native_stream_count(), 0);
    assert!(render(&mut r, 127).iter().all(|s| *s == 0.0));
    assert_eq!(r.snapshot().device_frames, frames + 127);
    r.set_metronome(120.0, true).unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 80).iter().any(|s| s.abs() > 1e-6));
    r.stop_click().unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 127).iter().all(|s| *s == 0.0));
}

#[test]
fn tempo_edit_preserves_fractional_metronome_phase() {
    let mut r = runtime();
    r.set_metronome(120.0, true).unwrap();
    r.service().unwrap();
    render(&mut r, 250);
    assert_eq!(r.snapshot().click_step, 0);
    r.set_metronome(60.0, true).unwrap();
    r.service().unwrap();
    render(&mut r, 499);
    assert_eq!(r.snapshot().click_step, 0);
    render(&mut r, 3);
    assert_eq!(r.snapshot().click_step, 1);
}

#[test]
fn a_one_step_click_pattern_retriggers_every_cycle() {
    let mut r = runtime();
    let mut pattern =
        SequencerPattern::metronome(120.0, TimeSignature::new(1, 4), crate::Subdivision::QUARTER);
    pattern.tracks[0].sound =
        Sound::sample_with_buffer("pulse", SampleBuffer::new(vec![0.25; 3], 1000));
    if let Sound::Sample { accent_gain, .. } = &mut pattern.tracks[0].sound {
        *accent_gain = 1.0;
    }
    r.set_click_pattern(pattern).unwrap();
    r.play_click().unwrap();
    r.service().unwrap();
    let output = render(&mut r, 1003);
    assert_eq!(output[0], 0.25);
    assert_eq!(output[1000], 0.25);
    assert_eq!(output[2000], 0.25);
    assert!(output[6..1000].iter().all(|s| *s == 0.0));
}

#[test]
fn atomic_calibration_start_uses_one_slot_and_full_queue_rejection_preserves_pattern() {
    let mut r = runtime();
    for _ in 0..COMMAND_BOUND - 1 {
        r.set_output_gain(0.8).unwrap();
    }
    r.start_click_pattern(SequencerPattern::metronome(
        60.0,
        TimeSignature::new(1, 4),
        crate::Subdivision::QUARTER,
    ))
    .unwrap();
    r.service().unwrap();
    render(&mut r, 250);
    assert!(r.snapshot().click_playing);
    for _ in 0..COMMAND_BOUND {
        r.set_output_gain(0.8).unwrap();
    }
    assert!(
        r.start_click_pattern(SequencerPattern::metronome(
            6000.0,
            TimeSignature::new(4, 4),
            crate::Subdivision::QUARTER
        ))
        .is_err()
    );
    r.service().unwrap();
    render(&mut r, 249);
    assert_eq!(r.metronome, (60.0, true));
    assert_eq!(r.snapshot().click_step, 0);
}

#[test]
fn both_metronome_transport_edges_cancel_previews_but_tempo_only_preserves_them() {
    let mut r = runtime();
    r.set_metronome(120.0, true).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    let revision = r.shared.preview_revision.load(Ordering::Acquire);
    r.send(Command::Preview {
        revision,
        buffer: Some(ArcGc::new(SampleBuffer::new(vec![0.01; 1000], 1000))),
    })
    .unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().preview_voices, 1);
    r.set_metronome(60.0, true).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().preview_voices, 1);
    r.preview_note(220.0, 5.0).unwrap();
    r.set_metronome(60.0, false).unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().preview_voices, 0);
    assert!(render(&mut r, 64).iter().all(|s| *s == 0.0));
}

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
fn prepared_chord_matches_legacy_strum_and_warm_cache_reuses_pcm() {
    let mut s = song(1);
    s.bars[0].bpm = 120.0;
    s.bars[0].chord_ref = Some(ChordRef {
        formula_name: "exact".into(),
        root_freq_hz: 220.0,
        pitches_hz: vec![220.0, 333.0],
        label: "x".into(),
    });
    let mut cache = Vec::new();
    let a = PreparedBacking::prepare(&s, 1000, &mut cache).unwrap();
    let pcm_a = cache[0].pcm.data.clone();
    let expected = render_chord(&ChordRender::strum(vec![220.0, 333.0], 0.5), 1000);
    assert_eq!(*pcm_a, *expected.data);
    s.click_enabled = true;
    let b = PreparedBacking::prepare(&s, 1000, &mut cache).unwrap();
    assert!(Arc::ptr_eq(&pcm_a, &cache[0].pcm.data));
    drop((a, b));
}

#[test]
fn phrase_mixes_with_capture_and_click_across_loop_boundaries_without_callback_allocation() {
    let mut s = song(1);
    s.playing = true;
    s.click_enabled = false;
    s.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![
            BackingEvent {
                time: audio_primitives::timing::EventTime::Beats(0.0),
                pitches_hz: vec![173.3, 261.6],
                strum_seconds: 0.002,
            },
            BackingEvent {
                time: audio_primitives::timing::EventTime::Beats(0.5),
                pitches_hz: vec![349.2],
                strum_seconds: 0.0,
            },
        ],
    });
    s.bars[0].audio_buffer = Some(crate::SampleBuffer::new(vec![0.125; 10], 1000));
    let mut live = runtime();
    live.set_song(s.clone()).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    let repeated = render(&mut live, 20);
    assert_ne!(&repeated[..10], &repeated[10..20]);
    assert_eq!(&repeated[..20], &repeated[20..]);

    let mut with_click = s;
    with_click.click_enabled = true;
    let mut click_runtime = runtime();
    click_runtime.set_song(with_click).unwrap();
    click_runtime.rewind().unwrap();
    click_runtime.service().unwrap();
    assert_ne!(&repeated[..4], &render(&mut click_runtime, 2)[..4]);
}

#[test]
fn invalid_phrase_replacement_preserves_the_accepted_runtime() {
    let mut accepted = song(1);
    accepted.playing = true;
    accepted.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![BackingEvent {
            time: audio_primitives::timing::EventTime::Beats(0.0),
            pitches_hz: vec![220.0],
            strum_seconds: 0.0,
        }],
    });
    let mut live = runtime();
    live.set_song(accepted.clone()).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    render(&mut live, 3);
    let before = live.snapshot();
    let mut invalid = accepted;
    invalid.bars[0].chord_ref = Some(ChordRef {
        formula_name: "conflict".into(),
        root_freq_hz: 220.0,
        pitches_hz: vec![220.0],
        label: String::new(),
    });
    assert!(live.set_song(invalid).is_err());
    assert_eq!(live.snapshot(), before);
    assert!(live.song().bars[0].chord_ref.is_none());
    assert!(live.song().bars[0].backing_phrase.is_some());
    let after_rejection = render(&mut live, 5);
    let mut control = runtime();
    let mut control_song = live.song();
    control_song.playing = true;
    control.set_song(control_song).unwrap();
    control.rewind().unwrap();
    control.service().unwrap();
    render(&mut control, 3);
    assert_eq!(after_rejection, render(&mut control, 5));
}

#[test]
fn backing_phrase_render_work_is_bounded_across_the_whole_snapshot() {
    let phrase = || BackingPhrase {
        events: (0..256)
            .map(|index| BackingEvent {
                time: audio_primitives::timing::EventTime::Beats((300 + index) as f64),
                pitches_hz: vec![220.0; 4],
                strum_seconds: 0.0,
            })
            .collect(),
    };
    let mut s = Song::new();
    s.bars = (0..2)
        .map(|_| Bar {
            bpm: 600.0,
            time_signature: TimeSignature::new(4, 4),
            length: 250,
            backing_phrase: Some(phrase()),
            ..Default::default()
        })
        .collect();
    assert!(PreparedBacking::prepare(&s, 1000, &mut Vec::new()).is_err());
}

#[test]
fn absolute_backing_duration_and_phrase_are_block_size_invariant_without_callback_allocation() {
    use audio_primitives::timing::EventTime;
    let mut s = song(1);
    s.playing = true;
    s.one_shot = true;
    s.bars[0].bpm = 137.0;
    s.bars[0].duration_seconds = Some(0.537);
    s.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![BackingEvent {
            time: EventTime::Seconds(0.031),
            pitches_hz: vec![173.3, 261.6],
            strum_seconds: 0.002,
        }],
    });
    let mut expected = None;
    for chunk in [1, 7, 64, 137, 537, 1024] {
        let mut r = runtime();
        r.set_song(s.clone()).unwrap();
        r.rewind().unwrap();
        r.service().unwrap();
        let mut pcm = Vec::new();
        let mut remaining = 537;
        while remaining > 0 {
            let n = remaining.min(chunk);
            pcm.extend(render(&mut r, n));
            remaining -= n;
            r.service().unwrap();
        }
        assert!(!r.snapshot().playing);
        assert!(pcm[..62].iter().all(|s| *s == 0.0));
        assert!(pcm[62..].iter().any(|s| *s != 0.0));
        let bits: Vec<_> = pcm.iter().map(|s| s.to_bits()).collect();
        if let Some(expected) = &expected {
            assert_eq!(&bits, expected);
        } else {
            expected = Some(bits);
        }
    }
}

#[test]
fn invalid_absolute_timing_preserves_admitted_backing_and_capture_refuses_it() {
    use audio_primitives::timing::EventTime;
    let mut accepted = song(1);
    accepted.playing = true;
    accepted.bars[0].duration_seconds = Some(0.537);
    accepted.bars[0].backing_phrase = Some(BackingPhrase {
        events: vec![BackingEvent {
            time: EventTime::Seconds(0.0),
            pitches_hz: vec![220.0],
            strum_seconds: 0.0,
        }],
    });
    let mut r = runtime();
    r.set_song(accepted.clone()).unwrap();
    r.rewind().unwrap();
    r.service().unwrap();
    render(&mut r, 3);
    let before = r.snapshot();
    for case in [
        "zero",
        "negative",
        "nan",
        "infinite",
        "subframe",
        "below minimum",
        "overflow",
        "legacy",
        "beats",
        "too large",
    ] {
        let mut invalid = accepted.clone();
        match case {
            "zero" => invalid.bars[0].duration_seconds = Some(0.0),
            "negative" => invalid.bars[0].duration_seconds = Some(-1.0),
            "nan" => invalid.bars[0].duration_seconds = Some(f64::NAN),
            "infinite" => invalid.bars[0].duration_seconds = Some(f64::INFINITY),
            "subframe" => invalid.bars[0].duration_seconds = Some(0.000001),
            "below minimum" => invalid.bars[0].duration_seconds = Some(0.499),
            "overflow" => invalid.bars[0].duration_seconds = Some(f64::MAX),
            "legacy" => invalid.bars[0].legacy_clock = true,
            "beats" => {
                invalid.bars[0].backing_phrase.as_mut().unwrap().events[0].time =
                    EventTime::Beats(0.0)
            },
            "too large" => invalid.bars[0].duration_seconds = Some(20_000.0),
            _ => unreachable!(),
        }
        assert!(r.set_song(invalid.clone()).is_err(), "{case}");
        assert_eq!(r.snapshot(), before, "{case}");
        assert_eq!(r.song().bars[0].duration_seconds, Some(0.537));
        assert!(Recording::prepare(&invalid, 1_000, 1).is_err(), "{case}");
    }
    let mut wrong_domain = accepted;
    wrong_domain.bars[0].duration_seconds = None;
    assert!(r.set_song(wrong_domain).is_err());
    assert_eq!(r.snapshot(), before);
}

#[test]
fn seconds_phrase_work_is_bounded_across_segments() {
    use audio_primitives::timing::EventTime;
    let mut s = Song::new();
    s.bars = (0..2)
        .map(|_| Bar {
            duration_seconds: Some(100.0),
            backing_phrase: Some(BackingPhrase {
                events: (0..256)
                    .map(|index| BackingEvent {
                        time: EventTime::Seconds((300 + index) as f64 / 10.0),
                        pitches_hz: vec![220.0; 4],
                        strum_seconds: 0.0,
                    })
                    .collect(),
            }),
            ..Default::default()
        })
        .collect();
    assert!(PreparedBacking::prepare(&s, 1_000, &mut Vec::new()).is_err());
}

#[test]
fn backing_click_is_independent_of_calibration_pattern() {
    let mut r = runtime();
    let mut s = song(1);
    s.click_enabled = true;
    s.playing = true;
    r.set_song(s.clone()).unwrap();
    r.service().unwrap();
    let first = render(&mut r, 8);
    let mut custom =
        SequencerPattern::metronome(30.0, Default::default(), crate::Subdivision::QUARTER);
    custom.tracks[0].sound = Sound::Click {
        frequency_hz: 77.0,
        accent_frequency_hz: 77.0,
        duration_seconds: 0.001,
        amplitude: 0.01,
    };
    r.set_click_pattern(custom).unwrap();
    r.set_song(s).unwrap();
    r.service().unwrap();
    assert_eq!(first, render(&mut r, 8));
}

#[test]
fn boundary_operations_remain_bounded_after_command_acknowledgement() {
    let mut r = runtime();
    for _ in 0..COMMAND_BOUND {
        r.queue(PendingChange::SeekTo { bar_idx: 0 }).unwrap();
    }
    r.service().unwrap();
    render(&mut r, 1);
    assert!(r.queue(PendingChange::SeekTo { bar_idx: 0 }).is_err());
    r.clear_session().unwrap();
    r.service().unwrap();
    render(&mut r, 1);
    r.service().unwrap();
    r.queue(PendingChange::SeekTo { bar_idx: 0 }).unwrap();
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
fn cancelled_previews_do_not_publish_and_preview_voices_are_bounded() {
    let mut r = runtime();
    r.preview_pitches(&[110.0, 220.0], 1.0, 18.0).unwrap();
    r.cancel_preview().unwrap();
    for _ in 0..100 {
        r.service().unwrap();
        render(&mut r, 3);
        if !r.preview_busy() {
            break;
        }
        std::thread::yield_now();
    }
    assert!(render(&mut r, 64).iter().all(|v| *v == 0.0));
    assert_eq!(
        r.snapshot().preview_starts,
        0,
        "cancelled preparation is not processor admission"
    );
    for _ in 0..20 {
        let revision = r.shared.preview_revision.load(Ordering::Acquire);
        r.send(Command::Preview {
            revision,
            buffer: Some(ArcGc::new(SampleBuffer::new(vec![0.01; 1000], 1000))),
        })
        .unwrap();
    }
    r.service().unwrap();
    render(&mut r, 1);
    assert_eq!(r.snapshot().preview_voices, 16);
    assert_eq!(r.snapshot().preview_starts, 20);
    r.cancel_preview().unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 64).iter().all(|v| *v == 0.0));
    assert_eq!(
        r.snapshot().preview_starts,
        20,
        "ended voices retain honest admission evidence"
    );
}

#[test]
fn unsupported_rates_and_aggregate_resources_are_rejected_before_publication() {
    assert!(OutputRuntime::offline(u32::MAX).is_err());
    let mut r = runtime();
    let accepted = r.song();
    let mut huge = song(2);
    huge.bars[0].bpm = 0.0001;
    huge.bars[0].time_signature.numerator = 255;
    huge.bars[0].length = 255;
    assert!(r.set_song(huge).is_err());
    assert_eq!(r.song().bars.len(), accepted.bars.len());
    let mut click = SequencerPattern::metronome_dense(
        120.0,
        TimeSignature::new(255, 4),
        crate::Subdivision::QUARTER,
    );
    click.tracks[0].sound = Sound::Click {
        frequency_hz: 800.0,
        accent_frequency_hz: 1200.0,
        duration_seconds: 1000.0,
        amplitude: 0.4,
    };
    assert!(r.set_click_pattern(click).is_err());
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

#[test]
fn canonical_metronome_uses_rounded_denominator_beats_without_boundary_drift() {
    let mut r = runtime();
    let beat = audio_primitives::frames_per_beat(1000, 137.0, 8);
    let mut pattern =
        SequencerPattern::metronome(137.0, TimeSignature::new(3, 8), crate::Subdivision::QUARTER);
    pattern.tracks[0].sound =
        Sound::sample_with_buffer("impulse", SampleBuffer::new(vec![0.5, 0.0], 1000));
    r.set_click_pattern(pattern).unwrap();
    r.play_click().unwrap();
    r.service().unwrap();
    let output = render(&mut r, beat * 8 + 1);
    let starts: Vec<_> = output
        .chunks_exact(2)
        .enumerate()
        .filter_map(|(i, frame)| (frame[0] != 0.0).then_some(i))
        .collect();
    assert_eq!(starts, (0..=8).map(|i| i * beat).collect::<Vec<_>>());
}

#[test]
fn exact_card_preview_preserves_short_duration_and_refuses_unrepresentable_output() {
    let mut r = runtime(); // 1000 Hz actual offline processor
    r.preview_card_pitches(&[220.0, 330.0], 0.06, 30.0).unwrap();
    let revision = r.shared.preview_revision.load(Ordering::Acquire);
    for (duration, spacing) in [(0.0001, 0.0), (0.4, 0.0001), (1e20, 0.0)] {
        assert!(
            r.preview_card_pitches(&[220.0, 330.0], duration, spacing)
                .is_err()
        );
        assert_eq!(r.shared.preview_revision.load(Ordering::Acquire), revision);
    }
    let began = std::time::Instant::now();
    let completed = loop {
        if let Some(completed) = r.preview.take_completion() {
            break completed;
        }
        assert!(began.elapsed() < std::time::Duration::from_secs(2));
        std::thread::yield_now();
    };
    assert_eq!(
        completed.buffer.data.len(),
        60,
        "exact Card must not inherit the generic 100-frame minimum"
    );
    let mut backing = song(1);
    backing.playing = true;
    r.set_song(backing).unwrap();
    assert!(r.preview_card_pitches(&[220.0], 0.2, 0.0).is_err());
    assert!(
        r.song().playing,
        "a refused preview retains the admitted backing"
    );
}
