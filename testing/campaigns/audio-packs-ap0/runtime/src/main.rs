//! AP0 disposable device-free probes using actual product and supplier APIs.
use audioadapter_buffers::direct::InterleavedSlice;
use firewheel::{
    ActivateInfo, FirewheelContext,
    backend::BackendProcessInfo,
    channel_config::{ChannelConfig, ChannelCount},
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        NodeError, ProcBuffers, ProcExtra, ProcInfo, ProcessStatus, StreamStatus,
    },
};
use fundsp::{audiounit::AudioUnit, prelude32::*};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    io::Cursor,
    num::NonZeroU32,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use woodshed_audio::{OutputRuntime, SampleBank, SequencerPattern, Sound};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
    static FREES: Cell<usize> = const { Cell::new(0) };
}
struct Observer;
// SAFETY: preserve System's allocator arguments/results; counters are thread-local.
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|t| {
            if t.get() {
                ALLOCS.with(|n| n.set(n.get() + 1))
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        TRACK.with(|t| {
            if t.get() {
                FREES.with(|n| n.set(n.get() + 1))
            }
        });
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK.with(|t| {
            if t.get() {
                ALLOCS.with(|n| n.set(n.get() + 1))
            }
        });
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Observer = Observer;
fn observed<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCS.with(|n| n.set(0));
    FREES.with(|n| n.set(0));
    TRACK.with(|t| t.set(true));
    let result = f();
    TRACK.with(|t| t.set(false));
    (result, ALLOCS.with(Cell::get), FREES.with(Cell::get))
}

#[derive(Clone)]
enum Kind {
    Fundsp,
    SoundFont(Arc<SoundFont>),
}
struct ProbeNode(Kind);
enum Voice {
    Fundsp(Box<dyn AudioUnit>),
    SoundFont(Synthesizer),
}
struct ProbeProcessor {
    voice: Voice,
    frames: usize,
}
impl AudioNode for ProbeNode {
    type Configuration = EmptyConfig;
    fn info(&self, _: &EmptyConfig) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("AP0_candidate")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::STEREO,
            }))
    }
    fn construct_processor(
        &self,
        _: &EmptyConfig,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        let rate = cx.stream_info.sample_rate.get();
        let voice = match &self.0 {
            Kind::Fundsp => {
                let mut unit = Box::new(sine_hz(110.0) * 0.2) as Box<dyn AudioUnit>;
                unit.set_sample_rate(rate as f64);
                unit.allocate();
                unit.reset();
                Voice::Fundsp(unit)
            }
            Kind::SoundFont(sf) => {
                let mut settings = SynthesizerSettings::new(rate as i32);
                settings.maximum_polyphony = 16;
                settings.enable_reverb_and_chorus = false;
                Voice::SoundFont(Synthesizer::new(sf, &settings).unwrap())
            }
        };
        Ok(ProbeProcessor { voice, frames: 0 })
    }
}
impl AudioNodeProcessor for ProbeProcessor {
    fn process(&mut self, _: &ProcInfo, buffers: ProcBuffers, _: &mut ProcExtra) -> ProcessStatus {
        let (left, right) = buffers.outputs.split_at_mut(1);
        match &mut self.voice {
            Voice::Fundsp(unit) => {
                for (l, r) in left[0].iter_mut().zip(right[0].iter_mut()) {
                    let v = unit.get_mono();
                    *l = v;
                    *r = v;
                }
            }
            Voice::SoundFont(synth) => {
                if self.frames == 0 {
                    synth.note_on(9, 36, 100);
                }
                if self.frames == 4096 {
                    synth.note_off(9, 36);
                }
                if self.frames == 8192 {
                    for _ in 0..40 {
                        synth.note_on(9, 36, 100);
                    }
                }
                if self.frames == 12288 {
                    synth.note_off_all(true);
                }
                synth.render(left[0], right[0]);
            }
        }
        self.frames += left[0].len();
        ProcessStatus::OutputsModified
    }
}
fn candidate(kind: Kind, rate: u32) -> serde_json::Value {
    let begin = Instant::now();
    let mut cx = FirewheelContext::new(Default::default());
    let node = cx.add_node(ProbeNode(kind), None).unwrap();
    cx.connect(node, cx.graph_out_node_id(), &[(0, 0), (1, 1)], false)
        .unwrap();
    let mut processor = cx
        .activate(ActivateInfo {
            sample_rate: NonZeroU32::new(rate).unwrap(),
            max_block_frames: NonZeroU32::new(256).unwrap(),
            num_stream_in_channels: 0,
            num_stream_out_channels: 2,
            input_to_output_latency_seconds: 0.0,
        })
        .unwrap();
    cx.update().unwrap();
    let prepared_us = begin.elapsed().as_micros();
    let mut output = [0.0f32; 512];
    let input: [f32; 0] = [];
    let mut peak = 0.0f32;
    let mut allocs = 0;
    let mut frees = 0;
    let mut render_us = 0;
    let mut final_peak = 0.0f32;
    for block in 0..256 {
        let begin = Instant::now();
        let (_, a, d) = observed(|| {
            processor.process(
                &InterleavedSlice::new(&input, 0, 256).unwrap(),
                &mut InterleavedSlice::new_mut(&mut output, 2, 256).unwrap(),
                BackendProcessInfo {
                    frames: 256,
                    process_timestamp: None,
                    duration_since_stream_start: Duration::from_secs_f64(
                        block as f64 * 256.0 / rate as f64,
                    ),
                    input_stream_status: StreamStatus::empty(),
                    output_stream_status: StreamStatus::empty(),
                    dropped_frames: 0,
                    process_to_playback_delay: None,
                },
            )
        });
        render_us += begin.elapsed().as_micros();
        allocs += a;
        frees += d;
        assert!(output.iter().all(|s| s.is_finite()));
        final_peak = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        peak = peak.max(final_peak);
        cx.update().unwrap();
    }
    assert!(peak > 0.001);
    drop(processor);
    cx.update().unwrap();
    json!({"sample_rate":rate,"frames":65536,"peak":peak,"final_block_peak":final_peak,"callback_allocations":allocs,"callback_deallocations":frees,"prepare_us":prepared_us,"render_us_instrumented_debug":render_us})
}
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let dest = Path::new(&arg);
    let mut sf_bytes = Cursor::new(std::fs::read(dest.join("synthetic.sf2")).unwrap());
    let sf = Arc::new(SoundFont::new(&mut sf_bytes).unwrap());
    let mut reports = Vec::new();
    for rate in [48000, 44100] {
        for (name, kind) in [
            ("fundsp", Kind::Fundsp),
            ("rustysynth", Kind::SoundFont(sf.clone())),
        ] {
            reports.push(json!({"supplier":name,"graph":candidate(kind,rate)}));
        }
    }
    let mut pattern: SequencerPattern =
        serde_json::from_slice(&std::fs::read(dest.join("pattern-4-4-4.json")).unwrap()).unwrap();
    let mut bank = SampleBank::new();
    let pcm: Vec<f32> = (0..2400)
        .map(|i| ((i as f32 * 0.1).sin()) * 0.3 * (-(i as f32) / 600.0).exp())
        .collect();
    let wav_path = dest.join("synthetic-kick.wav");
    let mut writer = hound::WavWriter::create(
        &wav_path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for sample in pcm {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
    bank.load_wav("synthetic-kick", wav_path).unwrap();
    assert_eq!(bank.rehydrate_pattern(&mut pattern), 1);
    let mut runtime = OutputRuntime::offline(48000).unwrap();
    runtime.start_click_pattern(pattern.clone()).unwrap();
    runtime.service().unwrap();
    let mut out = [0.0f32; 512];
    let mut allocs = 0;
    let mut frees = 0;
    let mut peak = 0.0f32;
    let mut onsets = Vec::new();
    let mut quiet = 128usize;
    for block in 0..375 {
        let (result, a, d) = observed(|| runtime.render_offline(&mut out));
        result.unwrap();
        allocs += a;
        frees += d;
        peak = peak.max(out.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
        runtime.service().unwrap();
        for (frame, s) in out.chunks_exact(2).enumerate() {
            if s[0].abs() > 0.001 {
                if quiet >= 128 {
                    onsets.push(block * 256 + frame);
                }
                quiet = 0;
            } else {
                quiet += 1;
            }
        }
    }
    assert!(peak > 0.001);
    assert_eq!(onsets, (0..16).map(|n| n * 6000 + 1).collect::<Vec<_>>());
    runtime.stop_click().unwrap();
    runtime.service().unwrap();
    for _ in 0..8 {
        runtime.render_offline(&mut out).unwrap();
        runtime.service().unwrap();
    }
    assert!(out.iter().all(|s| s.abs() < 1e-6));
    runtime.start_click_pattern(pattern).unwrap();
    runtime.service().unwrap();
    runtime.render_offline(&mut out).unwrap();
    assert!(out.iter().any(|s| s.abs() > 0.001));
    reports.push(json!({"supplier":"existing_sample_pattern_production_OutputRuntime","frames":96000,"peak":peak,"sample_onset_frames":onsets,"midi_onset_frames":(0..16).map(|n|n*6000).collect::<Vec<_>>(),"synthetic_sample_first_nonzero_offset":1,"float32_wav_loaded":true,"callback_allocations":allocs,"callback_deallocations":frees,"stop_then_restart":true}));
    let missing = Sound::sample("absent");
    assert!(missing.render_sample(0, 48000.0, false).is_none());
    let mut zero = SequencerPattern::metronome(
        120.0,
        Default::default(),
        woodshed_audio::Subdivision {
            divisions_per_beat: 0,
        },
    );
    assert!(runtime.set_click_pattern(zero.clone()).is_err());
    zero.subdivision.divisions_per_beat = 1;
    zero.bpm = f32::NAN;
    assert!(runtime.set_click_pattern(zero).is_err());
    reports.push(json!({"missing_sample_current_behavior":"no rendered voice; admission must report missing asset","zero_divisions_refused":true,"nan_bpm_refused":true}));
    println!("{}", serde_json::to_string_pretty(&reports).unwrap());
}
