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

#[path = "selected_capture_tests.rs"]
mod selected_capture_tests;

#[path = "tests/program.rs"]
mod program;

#[path = "tests/tones.rs"]
mod tones;

#[path = "tests/capture.rs"]
mod capture;

#[path = "tests/backing.rs"]
mod backing;

#[path = "tests/replacement.rs"]
mod replacement;
