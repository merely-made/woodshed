// SPDX-License-Identifier: MPL-2.0
//! Render a shared admitted kit through the existing production Firewheel
//! processor without opening a device. MIDI-file export needs only audio-packs.
use std::{error::Error, path::Path};
use woodshed_audio::OutputRuntime;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "Usage: drum_pack <bundled|pack-directory|snapshot.json> <pattern> <new-output.wav>"
                .into(),
        );
    }
    let id = args[1].to_str().ok_or("Pattern identity must be UTF-8")?;
    let pack = audio_packs::files::load(Path::new(&args[0]), Some(id))?;
    let pattern = pack.hydrated_pattern()?;
    let rate = 48_000;
    let mut remaining = audio_primitives::frames_per_bar(
        rate,
        pattern.bpm,
        pattern.time_signature.numerator,
        pattern.time_signature.denominator,
    );
    if remaining > rate as usize * 60 {
        return Err(
            "This file demonstration is limited to a one-bar render of at most 60 seconds".into(),
        );
    }
    let mut runtime = OutputRuntime::offline(rate)?;
    runtime.start_click_pattern(pattern)?;
    runtime.service()?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::new(file, spec)?;
    let mut scratch = [0.0f32; 512];
    while remaining > 0 {
        let frames = remaining.min(256);
        runtime.render_offline(&mut scratch[..frames * 2])?;
        for frame in scratch[..frames * 2].chunks_exact(2) {
            if !frame[0].is_finite() {
                return Err("Runtime produced nonfinite audio".into());
            }
            writer.write_sample(frame[0])?;
        }
        runtime.service()?;
        remaining -= frames;
    }
    writer.finalize()?;
    println!(
        "Rendered {id} from {} [{}]",
        pack.identity().id,
        pack.identity().digest
    );
    Ok(())
}
