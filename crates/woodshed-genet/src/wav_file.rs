//! Seek to a marked region in a local WAVE and retain only the selected audio.
use std::{fs::File, io::Cursor, path::Path};

use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    formats::{SeekMode, SeekTo, TrackType, probe::Hint},
    io::MediaSourceStream,
    units::Timestamp,
};

pub(crate) fn extract(path: &Path, start_ms: u64, end_ms: u64) -> Result<Vec<u8>, String> {
    extract_region(path, start_ms, end_ms).map_err(|e| format!("Could not read WAV phrase: {e}"))
}

fn extract_region(
    path: &Path,
    start_ms: u64,
    end_ms: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        stream,
        Default::default(),
        Default::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("missing audio track")?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("missing audio track")?;
    let rate = params.sample_rate.ok_or("missing sample rate")?;
    let channels = params
        .channels
        .as_ref()
        .ok_or("missing channel layout")?
        .count();
    if !(1_000..=384_000).contains(&rate) || channels == 0 {
        return Err("unsupported WAV sample rate or channel layout".into());
    }
    let frame = |ms: u64| {
        ms.checked_mul(u64::from(rate))
            .map(|n| n / 1000)
            .ok_or("invalid time mark")
    };
    let first = frame(start_ms)?;
    let mut end = frame(end_ms)?;
    if let Some(length) = track.num_frames {
        // A duration reported in milliseconds can round the final partial ms up.
        if end > length && end_ms == (length * 1000).div_ceil(u64::from(rate)) {
            end = length;
        }
        if end > length {
            return Err("out mark exceeds the recording".into());
        }
    }
    if first >= end {
        return Err("the out mark must follow the in mark".into());
    }
    let out_channels = channels.min(2);
    let nbytes = (end - first)
        .checked_mul(out_channels as u64 * 4)
        .ok_or("phrase too large")?;
    if nbytes > u64::from(u32::MAX) - 128 {
        return Err("the selected phrase exceeds the RIFF/WAVE file size limit".into());
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(nbytes as usize + 128)?;
    let mut writer = hound::WavWriter::new(
        Cursor::new(&mut bytes),
        hound::WavSpec {
            channels: out_channels as u16,
            sample_rate: rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default().gapless(false))?;
    if first != 0 {
        format.seek(
            SeekMode::Accurate,
            SeekTo::Timestamp {
                ts: Timestamp::new(i64::try_from(first)?),
                track_id: id,
            },
        )?;
    }
    let mut next = first;
    let mut converted: Vec<f32> = Vec::new();
    while next < end {
        let packet = format
            .next_packet()?
            .ok_or("the recording ends before the out mark")?;
        if packet.track_id != id {
            continue;
        }
        let audio = decoder.decode(&packet)?;
        let pts = u64::try_from(packet.pts.get()).map_err(|_| "negative packet position")?;
        if audio.spec().channels().count() != channels || audio.spec().rate() != rate || pts > next
        {
            return Err("discontinuous or changing WAV format".into());
        }
        audio.copy_to_vec_interleaved::<f32>(&mut converted);
        let skip = usize::try_from(next - pts)?;
        for samples in converted
            .chunks_exact(channels)
            .skip(skip)
            .take((end - next) as usize)
        {
            if samples.iter().any(|v| !v.is_finite()) {
                return Err("nonfinite sample in recording".into());
            }
            if channels <= 2 {
                for &v in samples {
                    writer.write_sample(v)?;
                }
            } else {
                // Fold additional channels into both sides, retaining the front stereo image.
                let surround = samples[2..].iter().map(|&v| f64::from(v)).sum::<f64>();
                for &front in &samples[..2] {
                    writer.write_sample(
                        ((2.0 * f64::from(front) + surround) / channels as f64) as f32,
                    )?;
                }
            }
            next += 1;
        }
    }
    writer.finalize()?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_marks_preserve_fractional_millisecond_and_24_bit_samples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a second take Ω.WAV");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut output = hound::WavWriter::create(&path, spec).unwrap();
        for frame in 0..4410i32 {
            output.write_sample(frame * 127).unwrap();
        }
        output.finalize().unwrap();
        // 9–11 ms selects frames 396..485, a length of 89 frames.
        let bytes = extract(&path, 9, 11).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        let selected: Vec<_> = reader.samples::<f32>().map(Result::unwrap).collect();
        assert_eq!(selected.len(), 89);
        assert_eq!(selected[0], (396 * 127) as f32 / 8_388_608.0);
        assert_eq!(selected[88], (484 * 127) as f32 / 8_388_608.0);
        assert!(extract(&path, 50, 101).is_err());
        assert!(extract(&path, 50, 49).is_err());
    }

    #[test]
    fn stereo_extract_keeps_interleaving_and_seeks_to_the_requested_frame() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut output = hound::WavWriter::create(&path, spec).unwrap();
        for frame in 0..8000i16 {
            output.write_sample(frame).unwrap();
            output.write_sample(-frame).unwrap();
        }
        output.finalize().unwrap();
        // 0..250 ms from the start: 2000 frames, no seek.
        let head = extract(&path, 0, 250).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(head)).unwrap();
        assert_eq!(reader.spec().channels, 2);
        let head: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
        assert_eq!(head.len(), 2000 * 2);
        assert_eq!(head[2], 1.0 / 32768.0);
        assert_eq!(head[3], -1.0 / 32768.0);
        // 500..501 ms seeks to frame 4000 and keeps 8 frames, L/R interleaved.
        let tail = extract(&path, 500, 501).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(tail)).unwrap();
        let tail: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
        assert_eq!(tail.len(), 8 * 2);
        assert_eq!(tail[0], 4000.0 / 32768.0);
        assert_eq!(tail[1], -4000.0 / 32768.0);
        assert_eq!(tail[14], 4007.0 / 32768.0);
        // The recording's last frame is readable and one more is refused.
        assert!(extract(&path, 999, 1000).is_ok());
        assert!(extract(&path, 999, 1001).is_err());
    }
}
