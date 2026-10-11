// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Execute the production graph without enumerating or opening audio devices.

use super::*;
use audioadapter_buffers::direct::InterleavedSlice;
use firewheel::{
    ActivateInfo, backend::BackendProcessInfo, node::StreamStatus, nodes::stream::ReadStatus,
    processor::FirewheelProcessor,
};
use std::time::Duration;

struct Offline {
    // Return the processor before the context is dropped.
    processor: FirewheelProcessor,
    engine: Engine,
    frames: u64,
}

impl Offline {
    fn new(sample_rate: u32) -> Self {
        Self::with_input_channels(sample_rate, 1)
    }

    fn with_input_channels(sample_rate: u32, input_channels: u32) -> Self {
        let mut cx = FirewheelContext::new(FirewheelConfig {
            num_graph_inputs: ChannelCount::MONO,
            ..Default::default()
        });
        let processor = cx
            .activate(ActivateInfo {
                sample_rate: NonZeroU32::new(sample_rate).unwrap(),
                max_block_frames: NonZeroU32::new(8192).unwrap(),
                num_stream_in_channels: input_channels,
                num_stream_out_channels: 2,
                input_to_output_latency_seconds: 0.0,
            })
            .unwrap();
        let mut engine = Engine::build_graph(cx, None, sample_rate).unwrap();
        engine.set_click_enabled(false);
        engine.cx.update().unwrap();
        let mut offline = Self {
            processor,
            engine,
            frames: 0,
        };
        if input_channels > 0 {
            offline.block(&[0.0; 256]);
        }
        offline
    }

    fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let mut output = vec![0.0; input.len() * 2];
        self.processor.process(
            &InterleavedSlice::new(input, 1, input.len()).unwrap(),
            &mut InterleavedSlice::new_mut(&mut output, 2, input.len()).unwrap(),
            BackendProcessInfo {
                frames: input.len(),
                process_timestamp: None,
                duration_since_stream_start: Duration::from_secs_f64(
                    self.frames as f64 / self.engine.sample_rate as f64,
                ),
                input_stream_status: StreamStatus::empty(),
                output_stream_status: StreamStatus::empty(),
                dropped_frames: 0,
                process_to_playback_delay: None,
            },
        );
        self.frames += input.len() as u64;
        output
    }

    fn block(&mut self, input: &[f32]) -> Vec<f32> {
        let output = self.process(input);
        self.engine.tick().unwrap();
        output
    }

    fn fast_bar(&mut self) {
        self.engine.set_tempo(600.0, 1, 4).unwrap();
        self.engine.set_click_enabled(false);
        self.engine.cx.update().unwrap();
        self.block(&[0.0; 256]);
        assert_eq!(self.engine.samples_per_bar(), 4800);
    }

    fn key(index: u16) -> LayerKey {
        LayerKey::new(TrackId::new(), index)
    }
}

fn assert_level(output: &[f32], expected: f32) {
    // Ignore the initial declick ramp, then require both channels to render.
    assert!(
        output[output.len() / 2..]
            .iter()
            .all(|v| (v - expected).abs() < 1e-5),
        "expected stereo level {expected}, tail {:?}",
        &output[output.len() - 8..]
    );
}

#[test]
fn input_only_reader_captures_without_monitor_and_keeps_silence() {
    let mut offline = Offline::new(48_000);
    offline.engine.start_free_capture().unwrap();
    assert_level(&offline.block(&[0.25; 128]), 0.0);
    assert_level(&offline.block(&[0.0; 64]), 0.0);
    offline.engine.stop_free_capture();
    let capture = offline.engine.take_bar_aligned_capture().unwrap();
    assert_eq!(capture, [vec![0.25; 128], vec![0.0; 64]].concat());
    assert_eq!(
        offline
            .engine
            .reader_state
            .stats()
            .unwrap()
            .latency_output_frames,
        0
    );
}

#[test]
fn sampler_resource_events_restart_existing_voice_and_swap_click_in_place() {
    let mut offline = Offline::new(48_000);
    let key = Offline::key(0);
    offline
        .engine
        .play_layer(key, vec![0.25; 32], 1.0, true)
        .unwrap();
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.25);
    let id = offline.engine.voice_nodes[&key];
    offline
        .engine
        .play_layer(key, vec![-0.5; 64], 0.5, true)
        .unwrap();
    offline.engine.cx.update().unwrap();
    assert_eq!(offline.engine.voice_nodes[&key], id);
    assert_eq!(offline.engine.voice_count(), 1);
    assert_level(&offline.block(&[0.0; 2048]), -0.25);
    offline.engine.set_layer_gain(key, 0.25);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), -0.125);
    offline.engine.stop_layer(key);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.0);

    let click_id = offline.engine.click_id;
    offline.engine.set_tempo(240.0, 3, 8).unwrap();
    offline.engine.cx.update().unwrap();
    let click = offline.block(&[0.0; 2048]);
    assert_eq!(offline.engine.click_id, click_id);
    assert_eq!(offline.engine.samples_per_bar(), 18_000);
    assert!(click.iter().any(|v| v.abs() > 0.1));
    assert!(offline.engine.click_in_bar_phase().unwrap() > 0);
}

#[test]
fn count_in_waits_a_full_bar_then_captures_exact_phrase_length() {
    let mut offline = Offline::new(48_000);
    offline.fast_bar();
    offline.engine.arm_bar_aligned_capture(1, 1).unwrap();
    let armed_at = offline.frames;
    let mut started_at = None;
    for _ in 0..65 {
        offline.block(&[0.25; 256]);
        if matches!(
            offline.engine.pending_capture_progress(),
            CapturePhase::Recording { .. }
        ) {
            started_at.get_or_insert(offline.frames - armed_at);
        }
        if matches!(
            offline.engine.pending_capture_progress(),
            CapturePhase::Complete
        ) {
            break;
        }
    }
    let started_at = started_at.expect("capture must start after the count-in");
    // Initial phase is 256 frames into the bar. Existing UI-tick scheduling
    // admits the complete drain batch on the first observed boundary.
    assert!(
        (9000..=10_000).contains(&started_at),
        "started at {started_at}"
    );
    let captured = offline.engine.take_bar_aligned_capture().unwrap();
    assert_eq!(captured, vec![0.25; 4800]);
    assert_eq!(
        offline.engine.pending_capture_progress(),
        CapturePhase::Idle
    );
    offline.engine.arm_bar_aligned_capture(1, 0).unwrap();
    assert!(offline.engine.arm_bar_aligned_capture(1, 0).is_err());
}

#[test]
fn queued_replay_waits_for_boundary_then_loops() {
    let mut offline = Offline::new(48_000);
    offline.fast_bar();
    let key = Offline::key(0);
    offline
        .engine
        .play_layer_at_next_bar(key, vec![0.2; 48], 1.0, true)
        .unwrap();
    for _ in 0..15 {
        assert_level(&offline.block(&[0.0; 256]), 0.0);
        assert!(!offline.engine.is_layer_assigned(key));
    }
    for _ in 0..10 {
        offline.block(&[0.0; 256]);
    }
    assert!(offline.engine.is_layer_assigned(key));
    assert_level(&offline.block(&[0.0; 2048]), 0.2);
    assert_level(&offline.block(&[0.0; 2048]), 0.2);
}

#[test]
fn reader_finish_drains_and_restart_discards_old_generation() {
    let mut offline = Offline::new(48_000);
    offline.engine.reader_state.stop_stream();
    let event = offline
        .engine
        .reader_state
        .start_stream(
            NonZeroU32::new(48_000).unwrap(),
            NonZeroU32::new(48_000).unwrap(),
            media_channel_config(0.01),
        )
        .unwrap();
    offline
        .engine
        .cx
        .queue_event_for(offline.engine.reader_id, event.into());
    offline.engine.cx.update().unwrap();
    let input: Vec<f32> = (0..17).map(|i| i as f32 / 32.0).collect();
    offline.process(&input);
    offline.engine.reader_state.finish_stream();
    offline.process(&[0.75; 8]);
    let stats = offline.engine.reader_state.stats().unwrap();
    assert!(stats.finished);
    assert!(!stats.drained);
    assert_eq!(stats.accepted_source_frames, 17);
    let mut captured = vec![0.0; 17];
    assert_eq!(
        offline.engine.reader_state.read_interleaved(&mut captured),
        Some(ReadStatus::Ok)
    );
    assert_eq!(captured, input);
    assert!(offline.engine.reader_state.stats().unwrap().drained);
    offline.engine.reader_state.stop_stream();
    let event = offline
        .engine
        .reader_state
        .start_stream(
            NonZeroU32::new(48_000).unwrap(),
            NonZeroU32::new(48_000).unwrap(),
            media_channel_config(0.01),
        )
        .unwrap();
    offline
        .engine
        .cx
        .queue_event_for(offline.engine.reader_id, event.into());
    offline.engine.cx.update().unwrap();
    offline.process(&[-0.25; 5]);
    let mut restarted = [0.0; 5];
    assert_eq!(
        offline.engine.reader_state.read_interleaved(&mut restarted),
        Some(ReadStatus::Ok)
    );
    assert_eq!(restarted, [-0.25; 5]);
    assert_eq!(
        offline
            .engine
            .reader_state
            .stats()
            .unwrap()
            .accepted_source_frames,
        5
    );
}

#[test]
fn reader_overflow_aborts_capture_and_next_capture_is_clean() {
    let mut offline = Offline::new(48_000);
    offline.engine.start_free_capture().unwrap();
    for _ in 0..4 {
        offline.process(&[0.25; 8192]);
    }
    let rejected = offline
        .engine
        .reader_state
        .stats()
        .unwrap()
        .rejected_source_frames;
    assert!(rejected > 0);
    assert!(matches!(offline.engine.tick(), Err(EngineError::InputOverflow(n)) if n == rejected));
    assert_eq!(
        offline.engine.pending_capture_progress(),
        CapturePhase::Idle
    );
    assert!(offline.engine.take_bar_aligned_capture().is_none());
    offline.engine.start_free_capture().unwrap();
    offline.block(&[-0.25; 128]);
    offline.engine.stop_free_capture();
    assert_eq!(
        offline.engine.take_bar_aligned_capture().unwrap(),
        vec![-0.25; 128]
    );
}

#[test]
fn stop_clears_capture_and_queued_replay() {
    let mut offline = Offline::new(48_000);
    offline.engine.start_free_capture().unwrap();
    offline.block(&[0.25; 128]);
    offline
        .engine
        .play_layer_at_next_bar(Offline::key(0), vec![0.2; 48], 1.0, true)
        .unwrap();
    offline.engine.stop();
    assert_eq!(
        offline.engine.pending_capture_progress(),
        CapturePhase::Idle
    );
    assert!(!offline.engine.reader_state.is_active());
    assert!(offline.engine.pending_layers.is_empty());
    assert!(offline.engine.take_bar_aligned_capture().is_none());
    // Full device restart is represented by construction of a fresh engine.
    drop(offline);
    let mut restarted = Offline::new(44_100);
    restarted.engine.start_free_capture().unwrap();
    restarted.block(&[-0.25; 64]);
    restarted.engine.stop_free_capture();
    assert_eq!(
        restarted.engine.take_bar_aligned_capture().unwrap(),
        vec![-0.25; 64]
    );
    assert_eq!(restarted.engine.sample_rate(), 44_100);
}

#[test]
fn rendered_gain_and_voice_mix_follow_linear_mute_solo_projection() {
    let mut offline = Offline::new(48_000);
    let first = Offline::key(0);
    let second = Offline::key(1);
    offline
        .engine
        .play_layer(first, vec![0.2; 32], 1.0, true)
        .unwrap();
    offline
        .engine
        .play_layer(second, vec![0.3; 32], 1.0, true)
        .unwrap();
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.5);
    // Muting removes a voice; solo's host projection removes every other key.
    offline.engine.stop_layer(second);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.2);
    offline
        .engine
        .play_layer(second, vec![0.3; 32], 1.0, true)
        .unwrap();
    offline.engine.stop_layer(first);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.3);
    offline.engine.set_layer_gain(second, 0.5);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.15);
    offline.engine.set_layer_gain(second, 0.0);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.0);
    offline.engine.set_layer_gain(second, 1.0);
    offline.engine.cx.update().unwrap();
    assert_level(&offline.block(&[0.0; 2048]), 0.3);
}

#[test]
fn source_sample_rate_preserves_one_shot_duration_after_device_change() {
    let mut offline = Offline::new(48_000);
    let key = Offline::key(0);
    offline
        .engine
        .play_layer_at_sample_rate(key, vec![0.25; 4410], 44_100, 1.0, false)
        .unwrap();
    offline.engine.cx.update().unwrap();
    let mut output = Vec::new();
    for _ in 0..12 {
        output.extend(offline.block(&[0.0; 512]));
    }
    assert!(output[4096 * 2..4352 * 2].iter().all(|v| *v > 0.2));
    assert!(output[5120 * 2..].iter().all(|v| v.abs() < 1e-5));
    let state = offline
        .engine
        .cx
        .node_state::<SamplerState>(offline.engine.voice_nodes[&key])
        .unwrap();
    assert!(!state.currently_playing());
}

#[test]
fn deferred_voice_limit_error_reaches_tick_and_existing_key_can_restart() {
    let mut offline = Offline::new(48_000);
    offline.fast_bar();
    let keys: Vec<_> = (0..VOICE_POOL_SIZE)
        .map(|index| Offline::key(index as u16))
        .collect();
    for &key in &keys {
        offline
            .engine
            .play_layer(key, vec![0.0; 32], 1.0, true)
            .unwrap();
    }
    offline.engine.cx.update().unwrap();
    let extra = Offline::key(32);
    assert!(matches!(
        offline.engine.play_layer(extra, vec![0.0; 32], 1.0, true),
        Err(EngineError::NoFreeVoices)
    ));
    offline
        .engine
        .play_layer(keys[0], vec![0.1; 32], 1.0, true)
        .unwrap();
    offline
        .engine
        .play_layer_at_next_bar(extra, vec![0.0; 32], 1.0, true)
        .unwrap();
    offline.engine.cx.update().unwrap();
    let mut failed = false;
    for _ in 0..25 {
        offline.process(&[0.0; 256]);
        if matches!(offline.engine.tick(), Err(EngineError::NoFreeVoices)) {
            failed = true;
            break;
        }
    }
    assert!(
        failed,
        "scheduled playback must report its voice allocation failure"
    );
    assert_eq!(offline.engine.voice_count(), VOICE_POOL_SIZE);
}

#[test]
fn completed_capture_survives_later_background_reader_overflow() {
    let mut offline = Offline::new(48_000);
    offline.engine.start_free_capture().unwrap();
    offline.block(&[0.25; 32]);
    offline.engine.stop_free_capture();
    for _ in 0..4 {
        offline.process(&[0.5; 8192]);
    }
    assert!(matches!(
        offline.engine.tick(),
        Err(EngineError::InputOverflow(_))
    ));
    assert_eq!(
        offline.engine.pending_capture_progress(),
        CapturePhase::Complete
    );
    assert_eq!(
        offline.engine.take_bar_aligned_capture().unwrap(),
        vec![0.25; 32]
    );
}

#[test]
fn output_only_stream_refuses_both_capture_modes() {
    let mut offline = Offline::with_input_channels(48_000, 0);
    assert!(!offline.engine.input_available());
    assert!(matches!(
        offline.engine.start_free_capture(),
        Err(EngineError::InputUnavailable)
    ));
    assert!(matches!(
        offline.engine.arm_bar_aligned_capture(1, 1),
        Err(EngineError::InputUnavailable)
    ));
    assert_eq!(
        offline.engine.pending_capture_progress(),
        CapturePhase::Idle
    );
    assert!(offline.engine.take_bar_aligned_capture().is_none());
    let mut input = Offline::new(48_000);
    assert!(input.engine.input_available());
    input.engine.stop();
    assert!(!input.engine.input_available());
}
