use std::{io::Cursor, path::Path};

use hound::{SampleFormat, WavReader, WavWriter};
use woodshedding::rehearsal::WavPhraseSource;

use super::{MAX_PRACTICE_WAV_BYTES, canonical_local_file, http_url, read_request};

pub(super) fn read_phrase(path: &Path, source: &WavPhraseSource) -> Result<Vec<u8>, String> {
    http_url(&source.source_page_url)
        .map_err(|_| "A page phrase requires an HTTP(S) page address without credentials.")?;
    let bytes = read_request(&canonical_local_file(path)?, MAX_PRACTICE_WAV_BYTES as u64)?;
    audio_between_marks(&bytes, source.start_ms, source.end_ms)
        .map_err(|error| format!("Invalid captured phrase: {error}"))
}

fn audio_between_marks(
    bytes: &[u8],
    start_ms: u64,
    end_ms: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if hound::read_wave_header(&mut &bytes[..])? != bytes.len() as u64 {
        return Err("incomplete or trailing WAV data".into());
    }
    let mut reader = WavReader::new(bytes)?;
    let spec = reader.spec();
    if !matches!(spec.channels, 1 | 2)
        || !(1_000..=384_000).contains(&spec.sample_rate)
        || !matches!(
            (spec.sample_format, spec.bits_per_sample),
            (SampleFormat::Int, 16) | (SampleFormat::Float, 32)
        )
    {
        return Err("expected mono or stereo PCM16 or float32 audio".into());
    }
    // At 44.1 kHz the two marks may straddle fractional frame boundaries.
    let frame_at = |ms| u128::from(spec.sample_rate) * u128::from(ms) / 1000;
    if start_ms >= end_ms || frame_at(end_ms) - frame_at(start_ms) != u128::from(reader.duration())
    {
        return Err("the captured audio length does not match the page marks".into());
    }
    // Keep only the selected samples. A WAV container can also hold arbitrary
    // metadata and additional chunks; none of those belong in the practice.
    let mut audio = Vec::with_capacity(bytes.len());
    let mut writer = WavWriter::new(Cursor::new(&mut audio), spec)?;
    match spec.sample_format {
        SampleFormat::Int => {
            for sample in reader.samples::<i16>() {
                writer.write_sample(sample?)?;
            }
        },
        SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                let value = sample?;
                if !value.is_finite() {
                    return Err("nonfinite audio sample".into());
                }
                writer.write_sample(value)?;
            }
        },
    }
    writer.finalize()?;
    Ok(audio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavSpec;

    #[test]
    fn page_phrase_preserves_only_frames_between_source_marks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        let spec = WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&path, spec).unwrap();
        // The boundaries at 9 and 11 ms enclose 89 frames (396..485).
        for i in 0..89i16 {
            writer.write_sample(i).unwrap();
            writer.write_sample(-i).unwrap();
        }
        writer.finalize().unwrap();
        let expected = std::fs::read(&path).unwrap();
        let mut with_metadata = expected.clone();
        with_metadata.extend_from_slice(b"JUNK\x06\x00\x00\x00secret");
        let riff_size = (with_metadata.len() - 8) as u32;
        with_metadata[4..8].copy_from_slice(&riff_size.to_le_bytes());
        std::fs::write(&path, with_metadata).unwrap();
        let mut source = WavPhraseSource {
            title: "Lesson".into(),
            source_page_url: "https://example.test/watch?id=1".into(),
            start_ms: 9,
            end_ms: 11,
            note: "Repeat".into(),
        };
        assert_eq!(read_phrase(&path, &source).unwrap(), expected);
        source.start_ms = 0;
        assert!(read_phrase(&path, &source).is_err());
        source.start_ms = 9;
        for invalid in ["file:///local.wav", "https://user:pass@example.test/watch"] {
            source.source_page_url = invalid.into();
            assert!(read_phrase(&path, &source).is_err());
        }
    }

    #[test]
    fn truncated_or_nonfinite_phrase_is_refused_before_saving() {
        let mut bytes = Vec::new();
        let spec = WavSpec {
            channels: 1,
            sample_rate: 1_000,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut writer = WavWriter::new(Cursor::new(&mut bytes), spec).unwrap();
        writer.write_sample(0.25f32).unwrap();
        writer.finalize().unwrap();
        assert_eq!(audio_between_marks(&bytes, 4000, 4001).unwrap(), bytes);
        assert!(audio_between_marks(&bytes[..bytes.len() - 1], 4000, 4001).is_err());
        let sample = bytes.len() - 4;
        bytes[sample..].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(audio_between_marks(&bytes, 4000, 4001).is_err());
    }
}
