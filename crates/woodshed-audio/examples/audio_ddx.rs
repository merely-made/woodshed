//! Muted hardware diagnostic for the single output runtime.
//!
//! Run `cargo run -p woodshed-audio --example audio_ddx --locked -- <mode>`.
//! Modes: raw, idle, chord, scale, render, song. Each mode owns one output
//! stream. Runtime processing blocks are graph blocks, not CPAL callbacks.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use woodshed_audio::{ChordRef, ChordRender, OutputRuntime, Song, render_chord};

type DiagnosticResult = Result<(), Box<dyn std::error::Error>>;

fn main() -> DiagnosticResult {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "raw".into());
    assert!(["raw", "idle", "chord", "scale", "render", "song"].contains(&mode.as_str()));
    if mode == "raw" || mode == "render" {
        return raw_diagnostic(&mode);
    }
    runtime_diagnostic(&mode)
}

fn pitches(mode: &str) -> (Vec<f32>, f32, f32) {
    if mode == "chord" {
        (vec![261.63, 329.63, 392.0, 493.88], 1.4, 18.0)
    } else {
        (
            (0..32)
                .map(|i| 110.0 * 2.0_f32.powf((i % 20) as f32 / 12.0))
                .collect(),
            31.0 * 60.0 / 92.0 + 0.4,
            60_000.0 / 92.0,
        )
    }
}

fn service_for(runtime: &mut OutputRuntime, duration: Duration) -> DiagnosticResult {
    let began = Instant::now();
    while began.elapsed() < duration {
        runtime.service()?;
        std::thread::sleep(Duration::from_millis(2));
    }
    runtime.service()?;
    Ok(())
}

fn runtime_diagnostic(mode: &str) -> DiagnosticResult {
    let start = Instant::now();
    let mut runtime = OutputRuntime::new()?;
    runtime.set_output_gain(0.0)?;
    assert_eq!(runtime.native_stream_count(), 1);
    eprintln!(
        "DDX mode={mode} output_streams={} sample_rate={}",
        runtime.native_stream_count(),
        runtime.sample_rate()
    );
    service_for(&mut runtime, Duration::from_secs(2))?;
    let (pitches, duration, offset) = pitches(mode);
    let mut arrangement = Song::new();
    arrangement.click_enabled = false;
    for i in 0..4 {
        if i > 0 {
            arrangement.add_bar();
        }
        arrangement.bars[i].bpm = 240.0;
        arrangement.bars[i].chord_ref = Some(ChordRef {
            formula_name: "diagnostic".into(),
            root_freq_hz: 110.0,
            pitches_hz: (0..8)
                .map(|n| 110.0 * 2.0_f32.powf((n + i) as f32 / 12.0))
                .collect(),
            label: format!("diagnostic {i}"),
        });
    }
    for trial in 0..3 {
        let phase = ["cold", "warm", "edited"][trial];
        let began = Instant::now();
        match mode {
            "chord" | "scale" => runtime.preview_pitches(&pitches, duration, offset)?,
            "song" => {
                if trial != 1 {
                    if trial == 2 {
                        for bar in &mut arrangement.bars {
                            bar.bpm = 300.0;
                            for pitch in &mut bar.chord_ref.as_mut().unwrap().pitches_hz {
                                *pitch *= 1.059463;
                            }
                        }
                    }
                    runtime.set_song(arrangement.clone())?;
                }
                runtime.rewind()?;
                runtime.play()?;
            },
            _ => {},
        }
        eprintln!(
            "DDX {:.3}s phase={phase} request_ms={:.3}",
            start.elapsed().as_secs_f64(),
            began.elapsed().as_secs_f64() * 1000.0
        );
        while runtime.preview_busy() {
            runtime.service()?;
            assert!(
                began.elapsed() < Duration::from_secs(10),
                "preview preparation timed out"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        runtime.service()?;
        eprintln!(
            "DDX {:.3}s phase={phase} ready_ms={:.3}",
            start.elapsed().as_secs_f64(),
            began.elapsed().as_secs_f64() * 1000.0
        );
        service_for(
            &mut runtime,
            Duration::from_secs(if mode == "song" { 5 } else { 2 }),
        )?;
        if mode == "song" {
            runtime.stop()?;
        } else {
            runtime.cancel_preview()?;
        }
        runtime.service()?;
    }
    let snapshot = runtime.snapshot();
    assert!(
        snapshot.device_frames > 0,
        "native output never processed frames"
    );
    eprintln!(
        "DDX complete output_streams={} device_frames={} processing_blocks={} device_events={}",
        runtime.native_stream_count(),
        snapshot.device_frames,
        snapshot.processing_blocks,
        runtime.device_error_count(),
    );
    if let Some(error) = runtime.last_error() {
        eprintln!("DDX last_output_error={error}");
    }
    Ok(())
}

fn raw_diagnostic(mode: &str) -> DiagnosticResult {
    let start = Instant::now();
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output device")?;
    let supported = device.default_output_config()?;
    eprintln!(
        "DDX mode={mode} output_streams=1 device={:?} config={supported:?}",
        device.description()?
    );
    let config: cpal::StreamConfig = supported.into();
    let callbacks = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let max_gap = Arc::new(AtomicU64::new(0));
    let (c, e, g) = (callbacks.clone(), errors.clone(), max_gap.clone());
    let mut last = Instant::now();
    let stream = device.build_output_stream(
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
    stream.play()?;
    std::thread::sleep(Duration::from_secs(2));
    let (pitches, duration, offset) = pitches("scale");
    for trial in 0..3 {
        let began = Instant::now();
        if mode == "render" {
            let _buffer = render_chord(
                &ChordRender {
                    pitches_hz: pitches.clone(),
                    duration_seconds: duration,
                    strum_offset_ms: offset,
                    ..Default::default()
                },
                config.sample_rate,
            );
        }
        eprintln!(
            "DDX trial={trial} work_ms={:.3}",
            began.elapsed().as_secs_f64() * 1000.0
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    eprintln!(
        "DDX complete output_streams=1 callbacks={} device_events={} max_callback_gap_us={}",
        callbacks.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed),
        max_gap.load(Ordering::Relaxed)
    );
    assert_eq!(
        errors.load(Ordering::Relaxed),
        0,
        "raw stream reported a device error"
    );
    Ok(())
}
