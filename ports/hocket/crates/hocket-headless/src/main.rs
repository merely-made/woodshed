// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Manual audio-device demonstration, separate from Hocket's desktop host.
//! `hocket-headless --run` opens the real default input/output devices and
//! exercises bar-aligned capture followed by queued replay. A person must
//! listen to assess sound, timing and feedback. This is not a CI acceptance
//! gate: missing devices and incomplete execution are failures, never skips.
//!
//! Demo timeline (turn your volume down — feedback risk if mic and
//! speakers are close):
//!
//! 1. **0–4 s** — click loop plays. Lets you hear the bar boundaries.
//! 2. **4 s** — arm bar-aligned capture for 1 bar with a 1-bar
//!    count-in. The capture *waits* for the next bar boundary, then
//!    counts one more bar of click, then captures one bar.
//! 3. After capture completes — queue replay at the next bar
//!    boundary. Replay loops endlessly until shutdown.
//! 4. **~12 s** — done. Stop the replay loop.

use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use hocket_engine::{CapturePhase, Engine, EngineError, LayerKey, ModelTrackId};

const TICK_INTERVAL: Duration = Duration::from_millis(15);
const METER_PRINT_INTERVAL: Duration = Duration::from_millis(250);
const CLICK_ONLY_PHASE: Duration = Duration::from_secs(4);
const TOTAL_RUN: Duration = Duration::from_secs(12);

/// Bars to capture (each is 2 s at 120 BPM 4/4).
const CAPTURE_BARS: u8 = 1;
/// Bars of click count-in before capture begins.
const COUNT_IN_BARS: u8 = 1;

const HELP: &str = "hocket-headless — manual audio-device demonstration\n\
Usage: hocket-headless --run\n\
       hocket-headless --help\n\n\
--run opens the default microphone and speakers for about 12 seconds.\n\
Turn the speakers down and use headphones to avoid feedback.\n\
Success means capture completed and replay was queued without engine errors.\n\
Audibility, calibration and musical timing require a person's review.\n\
Missing hardware or incomplete execution exits unsuccessfully; no CI skip.\n";

#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    Help,
    Run,
}

fn invocation(args: impl IntoIterator<Item = String>) -> Result<Invocation, String> {
    let args: Vec<_> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(Invocation::Help),
        [arg] if arg == "--help" || arg == "-h" => Ok(Invocation::Help),
        [arg] if arg == "--run" => Ok(Invocation::Run),
        _ => Err("expected --run or --help".into()),
    }
}

fn main() -> ExitCode {
    match invocation(std::env::args().skip(1)) {
        Ok(Invocation::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        },
        Ok(Invocation::Run) => match run_demo(Engine::new) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("manual demo failed: {error}");
                ExitCode::FAILURE
            },
        },
        Err(error) => {
            eprintln!("{error}\n{HELP}");
            ExitCode::from(2)
        },
    }
}

fn run_demo(start_audio: impl FnOnce() -> Result<Engine, EngineError>) -> Result<(), String> {
    println!("hocket — manual bar-aligned capture/replay demo");
    println!("turn your speakers down (feedback risk).");
    println!();

    let mut engine =
        start_audio().map_err(|error| format!("could not start audio engine: {error}"))?;
    let result = demonstrate(&mut engine);
    engine.stop();
    result
}

fn demonstrate(engine: &mut Engine) -> Result<(), String> {
    let sample_rate = engine.sample_rate();
    let bar_samples = engine.samples_per_bar();
    println!(
        "audio backend running at {sample_rate} Hz; one bar = {bar_samples} samples ({} ms)",
        bar_samples * 1000 / sample_rate as usize
    );

    let demo_layer_key = LayerKey::new(ModelTrackId::new(), 0);

    let start = Instant::now();
    let arm_at = start + CLICK_ONLY_PHASE;

    let mut armed = false;
    let mut replay_scheduled = false;
    let mut next_meter_print = start + METER_PRINT_INTERVAL;

    println!();
    println!("[0:00] click playing — listen for bar boundaries…");

    loop {
        let now = Instant::now();

        if !armed && now >= arm_at {
            match engine.arm_bar_aligned_capture(CAPTURE_BARS, COUNT_IN_BARS) {
                Ok(()) => {
                    println!(
                        "[{:>4}] capture armed: wait for next bar boundary, then \
                         {COUNT_IN_BARS} bar count-in, then capture {CAPTURE_BARS} bar(s)",
                        fmt_elapsed(now - start),
                    );
                },
                Err(err) => return Err(format!("arm failed: {err}")),
            }
            armed = true;
        }

        // Advance the engine: drains mic input, advances the
        // bar-aligned capture + queued layers, flushes Firewheel.
        if let Err(err) = engine.tick() {
            return Err(format!("engine tick error: {err}"));
        }

        // Promote capture to replay when complete.
        if !replay_scheduled {
            if let Some(captured) = engine.take_bar_aligned_capture() {
                if captured.is_empty() {
                    return Err("capture completed without any input samples".into());
                }
                println!(
                    "[{:>4}] captured {} samples (~{:.2} s); queuing replay at next bar…",
                    fmt_elapsed(now - start),
                    captured.len(),
                    captured.len() as f32 / sample_rate as f32
                );
                if let Err(err) = engine.play_layer_at_next_bar(demo_layer_key, captured, 1.0, true)
                {
                    return Err(format!("schedule replay failed: {err}"));
                }
                replay_scheduled = true;
            }
        }

        if now >= next_meter_print {
            let [l, r] = engine.peak_db();
            let phase = engine.pending_capture_progress();
            let phase_str = match phase {
                CapturePhase::Idle => "Idle".to_string(),
                CapturePhase::Waiting {
                    bars_remaining,
                    samples_until_next_bar,
                } => format!(
                    "Waiting (bars left: {}, next bar in {} ms)",
                    bars_remaining,
                    samples_until_next_bar * 1000 / sample_rate as usize
                ),
                CapturePhase::Recording { progress } => {
                    format!("Recording {:.0}%", progress * 100.0)
                },
                CapturePhase::FreeRecording { samples_done } => {
                    format!("FreeRecording ({samples_done} samples)")
                },
                CapturePhase::Complete => "Complete".to_string(),
            };
            println!(
                "[{:>4}] {phase_str:<32} meter L={l:>6.1} dB  R={r:>6.1} dB",
                fmt_elapsed(now - start),
            );
            next_meter_print = now + METER_PRINT_INTERVAL;
        }

        if now >= start + TOTAL_RUN {
            break;
        }

        thread::sleep(TICK_INTERVAL);
    }

    engine.stop_layer(demo_layer_key);

    if !replay_scheduled {
        return Err("capture did not complete and queue replay within the 12-second demo".into());
    }

    println!();
    println!("capture completed; replay queued. Review audible timing separately.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_run_opens_the_hardware_demonstration() {
        for args in [vec![], vec!["--help".into()], vec!["-h".into()]] {
            assert_eq!(invocation(args), Ok(Invocation::Help));
        }
        assert_eq!(invocation(vec!["--run".into()]), Ok(Invocation::Run));
        assert!(invocation(vec!["--run".into(), "--help".into()]).is_err());
        assert!(invocation(vec!["--ci".into()]).is_err());
    }

    #[test]
    fn unavailable_audio_is_an_execution_failure_not_a_successful_skip() {
        let result = run_demo(|| Err(EngineError::CpalInit("fixture: no device".into())));
        assert!(result.unwrap_err().contains("fixture: no device"));
    }
}

fn fmt_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    let centi = (d.subsec_millis() / 100) as u8;
    format!("{secs}.{centi}s")
}
