//! Seek to a marked region in a local WAVE and retain only the selected audio.
use std::{fs::File, io::Cursor, path::Path};

use symphonia::core::{
    audio::SampleBuffer,
    formats::{SeekMode, SeekTo},
    io::MediaSourceStream,
    probe::Hint,
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
    let mut format = symphonia::default::get_probe()
        .format(&hint, stream, &Default::default(), &Default::default())?
        .format;
    let track = format.default_track().ok_or("missing audio track")?;
    let rate = track
        .codec_params
        .sample_rate
        .ok_or("missing sample rate")?;
    let channels = track
        .codec_params
        .channels
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
    if let Some(length) = track.codec_params.n_frames {
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
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;
    if first != 0 {
        format.seek(
            SeekMode::Accurate,
            SeekTo::TimeStamp {
                ts: first,
                track_id: id,
            },
        )?;
    }
    let mut next = first;
    while next < end {
        let packet = format.next_packet()?;
        if packet.track_id() != id {
            continue;
        }
        let audio = decoder.decode(&packet)?;
        if audio.spec().channels.count() != channels
            || audio.spec().rate != rate
            || packet.ts() > next
        {
            return Err("discontinuous or changing WAV format".into());
        }
        let mut converted = SampleBuffer::<f32>::new(audio.capacity() as u64, *audio.spec());
        converted.copy_interleaved_ref(audio);
        let skip = usize::try_from(next - packet.ts())?;
        for samples in converted
            .samples()
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
}
