use super::*;
use crate::{Bar, ChordRef, TimeSignature};
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
    r.cancel_preview().unwrap();
    r.service().unwrap();
    assert!(render(&mut r, 64).iter().all(|v| *v == 0.0));
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
    let mut pattern = SequencerPattern::metronome(
        137.0,
        TimeSignature::new(3, 8),
        crate::Subdivision::QUARTER,
    );
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
