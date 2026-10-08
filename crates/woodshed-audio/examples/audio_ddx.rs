//! Muted hardware diagnostic: compare idle streams, ordinary chord previews,
//! long scale previews and synthesis without the song engine mutex.
//! Run `cargo run -p woodshed-audio --example audio_ddx --locked -- <mode>`.
//! Modes: raw, idle, chord, scale, render. Output is silent in every mode.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use woodshed_audio::{
    ChordRender, SequencerEngine, SequencerPattern, Song, SongEngine, Subdivision, TimeSignature,
    render_chord,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "raw".into());
    assert!(["raw", "idle", "chord", "scale", "render"].contains(&mode.as_str()));
    let start = Instant::now();
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output device")?;
    let supported = device.default_output_config()?;
    eprintln!(
        "DDX mode={mode} device={:?} config={supported:?}",
        device.description()?
    );
    let config: cpal::StreamConfig = supported.into();
    let callbacks = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let max_gap = Arc::new(AtomicU64::new(0));
    let (c, e, g) = (callbacks.clone(), errors.clone(), max_gap.clone());
    let mut last = Instant::now();
    let monitor = device.build_output_stream(
        config,
        move |data: &mut [f32], _| {
            let now = Instant::now();
            if c.fetch_add(1, Ordering::Relaxed) > 0 {
                g.fetch_max(
                    now.duration_since(last).as_micros() as u64,
                    Ordering::Relaxed,
                );
            }
            last = now;
            data.fill(0.0);
        },
        move |error| {
            e.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "DDX {:.3}s device_event={error}",
                start.elapsed().as_secs_f64()
            );
        },
        None,
    )?;
    eprintln!("DDX negotiated_frames={:?}", monitor.buffer_size());
    monitor.play()?;
    std::thread::sleep(Duration::from_secs(2));
    let _sequencer = (mode != "raw" && mode != "render")
        .then(|| {
            SequencerEngine::new(SequencerPattern::metronome(
                120.0,
                TimeSignature::default(),
                Subdivision::QUARTER,
            ))
        })
        .transpose()?;
    let song = (mode != "raw" && mode != "render")
        .then(|| SongEngine::new(Song::new()))
        .transpose()?;
    let handle = song.as_ref().map(SongEngine::handle);
    if let Some(h) = &handle {
        h.set_output_gain(0.0);
    }
    std::thread::sleep(Duration::from_secs(2));
    let pitches: Vec<f32> = if mode == "chord" {
        vec![261.63, 329.63, 392.0, 493.88]
    } else {
        (0..32)
            .map(|i| 110.0 * 2.0_f32.powf((i % 20) as f32 / 12.0))
            .collect()
    };
    let (duration, offset) = if mode == "chord" {
        (1.4, 18.0)
    } else {
        (31.0 * 60.0 / 92.0 + 0.4, 60_000.0 / 92.0)
    };
    for trial in 0..3 {
        eprintln!(
            "DDX {:.3}s trial={trial} begin",
            start.elapsed().as_secs_f64()
        );
        let t = Instant::now();
        match mode.as_str() {
            "chord" | "scale" => handle
                .as_ref()
                .unwrap()
                .play_chord_now(&pitches, duration, offset),
            "render" => {
                let _buffer = render_chord(
                    &ChordRender {
                        pitches_hz: pitches.clone(),
                        duration_seconds: duration,
                        strum_offset_ms: offset,
                        ..Default::default()
                    },
                    config.sample_rate,
                );
            },
            _ => {},
        }
        eprintln!(
            "DDX {:.3}s trial={trial} work_ms={:.3}",
            start.elapsed().as_secs_f64(),
            t.elapsed().as_secs_f64() * 1000.0
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    eprintln!(
        "DDX complete callbacks={} device_events={} max_callback_gap_us={}",
        callbacks.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed),
        max_gap.load(Ordering::Relaxed)
    );
    Ok(())
}
