use super::*;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(
        std::env::var_os("REDSHANK_PHASE4_FIXTURES")
            .expect("set REDSHANK_PHASE4_FIXTURES to the generated fixture directory"),
    )
    .join(name)
}

fn wait_for(
    runtime: &PlaybackRuntime,
    predicate: impl Fn(&PlaybackSnapshot) -> bool,
) -> PlaybackSnapshot {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let snapshot = runtime.snapshot();
        if predicate(&snapshot) || Instant::now() >= deadline {
            return snapshot;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn inspect_controls(runtime: &PlaybackRuntime) -> (u16, Option<u16>, u8) {
    let (sender, receiver) = std::sync::mpsc::channel();
    runtime
        .command(PlaybackCommand::InspectControls(sender))
        .unwrap();
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("worker control facts")
}

/// A pure tone pair written as PCM, decodable by the same Symphonia path
/// a real enclosure takes. Returns the source frame count.
fn synthetic_source(path: &PathBuf, seconds: u32) -> u64 {
    let rate = 48_000_u32;
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    let frames = rate * seconds;
    for frame in 0..frames {
        let phase = 2.0 * std::f32::consts::PI * frame as f32 / rate as f32;
        let scale = f32::from(i16::MAX) * 0.5;
        writer
            .write_sample(((phase * 440.0).sin() * scale) as i16)
            .unwrap();
        writer
            .write_sample(((phase * 660.0).sin() * scale) as i16)
            .unwrap();
    }
    writer.finalize().unwrap();
    u64::from(frames)
}

/// Fundamental of one channel from hysteresis zero crossings, which a
/// lossy decoder's noise floor cannot fake.
fn fundamental(samples: &[f32], channels: usize, channel: usize) -> f32 {
    let peak = samples
        .iter()
        .skip(channel)
        .step_by(channels)
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let threshold = peak * 0.25;
    let mut armed = false;
    let mut first = None;
    let mut last = 0;
    let mut cycles = 0_u32;
    for frame in 0..samples.len() / channels {
        let sample = samples[frame * channels + channel];
        if sample < -threshold {
            armed = true;
        } else if armed && sample > threshold {
            armed = false;
            if first.is_none() {
                first = Some(frame);
            } else {
                cycles += 1;
            }
            last = frame;
        }
    }
    let first = first.expect("no cycles in the measured channel");
    f32::from(cycles as u16) * 48_000.0 / (last - first) as f32
}

mod new_runtime_has_an_honest_empty_snapshot_cases;
mod retiming_delivers_the_inverse_of_the_rate_in_cases;
mod supplied_stereo_fixtures_preserve_two_distinct_channels_cases;
