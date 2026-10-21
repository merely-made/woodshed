//! Offline rendering — turn a [`SequencerPattern`] into PCM samples
//! (or a `.wav` file) without going through cpal.
//!
//! Useful for:
//! - exporting a pattern as a `.wav` to share with bandmates,
//! - regression tests that need a deterministic audio buffer,
//! - future bouncing of a loop record / overdub session.
//!
//! This pure pattern export preserves the historical renderer. Native output
//! uses the bounded Firewheel processor in `output_runtime`; no device or
//! shared callback mixer is constructed by this module.

use std::path::Path;

use crate::samples::SampleError;
use crate::sequencer::SequencerPattern;

mod song_export;
pub use song_export::{
    SongExportError, SongExportReport, export_song_wav_to, export_song_wav_with_pattern_to,
};

pub use audio_pattern::render::render_pattern;
/// Render a pattern straight to a `.wav` file on disk. The file is
/// 16-bit PCM at the requested sample rate; multi-channel uses
/// duplicated mono.
pub fn export_wav(
    pattern: SequencerPattern,
    sample_rate_hz: u32,
    duration_secs: f32,
    channels: u16,
    path: impl AsRef<Path>,
) -> Result<(), SampleError> {
    let samples = render_pattern(pattern, sample_rate_hz as f32, duration_secs, channels);
    let spec = hound::WavSpec {
        channels,
        sample_rate: sample_rate_hz,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path.as_ref(), spec)?;
    let scale = i16::MAX as f32;
    for &s in &samples {
        let clamped = s.clamp(-1.0, 1.0);
        writer.write_sample((clamped * scale) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::{Subdivision, TimeSignature};

    #[test]
    fn render_pattern_returns_correct_buffer_length() {
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let buf = render_pattern(p, 48_000.0, 1.0, 2);
        assert_eq!(buf.len(), 48_000 * 2);
    }

    #[test]
    fn render_pattern_produces_audio_not_silence() {
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let buf = render_pattern(p, 48_000.0, 1.0, 1);
        let max_abs = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            max_abs > 0.0,
            "expected some non-silence in rendered output; max abs = {max_abs}"
        );
    }

    #[test]
    fn render_pattern_clicks_are_localized_to_beats() {
        // 120 BPM, 4/4 quarter clicks: clicks at 0.0s, 0.5s, 1.0s, 1.5s.
        // Across 2 seconds of audio there should be ~4 distinct
        // high-energy windows.
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let buf = render_pattern(p, 48_000.0, 2.0, 1);

        // Bucket the buffer into 50ms windows and count windows with
        // amplitude above a threshold.
        let window = 48_000 / 20; // 2400 samples = 50ms
        let mut loud = 0;
        let mut prev_was_loud = false;
        for chunk in buf.chunks(window) {
            let peak = chunk.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
            let is_loud = peak > 0.05;
            if is_loud && !prev_was_loud {
                loud += 1;
            }
            prev_was_loud = is_loud;
        }
        // Expect ~4 click events; allow some slack for boundary cases.
        assert!(
            (3..=5).contains(&loud),
            "expected ~4 click events; got {loud}"
        );
    }

    #[test]
    fn export_wav_writes_readable_file() {
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let path = std::env::temp_dir().join("woodshed_export_test.wav");
        export_wav(p, 48_000, 0.5, 1, &path).unwrap();

        // Read it back and verify shape.
        let reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 48_000);
        assert_eq!(spec.bits_per_sample, 16);
        // 0.5s × 48000 Hz = 24000 samples.
        let frame_count = reader.duration() as usize;
        assert_eq!(frame_count, 24_000);

        let _ = std::fs::remove_file(&path);
    }
}
