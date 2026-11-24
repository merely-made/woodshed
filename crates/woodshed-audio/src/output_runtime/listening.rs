use super::*;
use hound::{SampleFormat, WavReader};
use std::io::Cursor;

pub const MAX_LISTENING_WAV_BYTES: usize = 16 * 1024 * 1024;

/// State published after a processed block. The region cursor names the next
/// output frame to render, relative to the selected region's start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListeningWavObservation {
    pub stream_epoch: u64,
    pub device_frames: u64,
    pub region_frames: u64,
    pub region_cursor: u64,
    pub completed_wraps: u64,
    pub playing: bool,
    pub sample_rate: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ListeningSnapshot {
    pub revision: u64,
    pub region_frames: u64,
    pub region_cursor: u64,
    pub completed_wraps: u64,
    pub playing: bool,
}

pub(super) struct PreparedListening {
    source: Vec<f32>,
    source_frames: usize,
    channels: usize,
    state: ListeningSnapshot,
}

impl PreparedListening {
    pub fn set_playing(&mut self, playing: bool) {
        self.state.playing = playing;
    }

    pub fn snapshot(&self) -> ListeningSnapshot {
        self.state
    }

    pub fn frame(&mut self) -> [f32; 2] {
        let state = &mut self.state;
        if !state.playing {
            return [0.0; 2];
        }
        // Use the loop period, rather than an accumulating fractional phase,
        // so each wrap starts at the same source frame. Interpolate across
        // the seam using only PCM inside the marked region.
        let phase = u128::from(state.region_cursor) * self.source_frames as u128;
        let period = u128::from(state.region_frames);
        let index = (phase / period) as usize;
        let fraction = (phase % period) as f32 / state.region_frames as f32;
        let first = index * self.channels;
        let second = ((index + 1) % self.source_frames) * self.channels;
        state.region_cursor += 1;
        if state.region_cursor == state.region_frames {
            state.region_cursor = 0;
            state.completed_wraps = state.completed_wraps.saturating_add(1);
        }
        std::array::from_fn(|channel| {
            let channel = channel % self.channels;
            let a = self.source[first + channel];
            let b = self.source[second + channel];
            a + (b - a) * fraction
        })
    }
}

fn open_region(
    bytes: &[u8],
    marks: Option<(u64, u64)>,
) -> Result<(WavReader<Cursor<&[u8]>>, u32, u32), OutputError> {
    if marks.is_some() && bytes.len() > MAX_LISTENING_WAV_BYTES {
        return Err(err("listening WAV exceeds 16 MiB"));
    }
    let riff_size = bytes
        .get(4..8)
        .map(|field| u32::from_le_bytes(field.try_into().unwrap()))
        .ok_or_else(|| err("missing listening WAV header"))?;
    let declared = 8_u64 + u64::from(riff_size);
    if declared > bytes.len() as u64 {
        return Err(err("truncated listening WAV"));
    }
    let mut reader = WavReader::new(Cursor::new(&bytes[..declared as usize]))
        .map_err(|error| err(format!("listening WAV: {error}")))?;
    let spec = reader.spec();
    if !matches!(
        (spec.sample_format, spec.bits_per_sample),
        (SampleFormat::Int, 16) | (SampleFormat::Float, 32)
    ) || !matches!(spec.channels, 1 | 2)
        || valid_rate(spec.sample_rate).is_err()
    {
        return Err(err(
            "listening WAV must be mono or stereo PCM16 or float32 at 1000–384000 Hz",
        ));
    }
    let frame_at = |ms: u64| u128::from(ms) * u128::from(spec.sample_rate) / 1000;
    let (start, end) = marks.map_or((0, u128::from(reader.duration())), |(s, e)| {
        (frame_at(s), frame_at(e))
    });
    if end > u128::from(reader.duration()) || end <= start {
        return Err(err("marked listening region is outside the WAV frames"));
    }
    // Hound trusts the data chunk's declared length until a read reaches it.
    reader
        .seek(reader.duration() - 1)
        .map_err(|error| err(format!("listening WAV: {error}")))?;
    if spec.sample_format == SampleFormat::Float {
        for sample in reader.samples::<f32>().take(usize::from(spec.channels)) {
            sample.map_err(|error| err(format!("listening WAV: {error}")))?;
        }
    } else {
        for sample in reader.samples::<i16>().take(usize::from(spec.channels)) {
            sample.map_err(|error| err(format!("listening WAV: {error}")))?;
        }
    }
    reader
        .seek(start as u32)
        .map_err(|error| err(format!("listening WAV: {error}")))?;
    Ok((reader, start as u32, end as u32))
}

impl OutputRuntime {
    pub fn validate_listening_wav(
        bytes: &[u8],
        start_ms: u64,
        end_ms: u64,
    ) -> Result<(), OutputError> {
        open_region(bytes, Some((start_ms, end_ms))).map(|_| ())
    }

    /// Load a bounded PCM16 region paused at its first frame.
    pub fn set_listening_wav(
        &mut self,
        bytes: &[u8],
        start_ms: u64,
        end_ms: u64,
    ) -> Result<(), OutputError> {
        self.load_wav(bytes, Some((start_ms, end_ms)))
    }

    /// Every frame of a saved excerpt, including its final fractional millisecond.
    pub fn set_extracted_wav(&mut self, bytes: &[u8]) -> Result<(), OutputError> {
        self.load_wav(bytes, None)
    }

    fn load_wav(&mut self, bytes: &[u8], marks: Option<(u64, u64)>) -> Result<(), OutputError> {
        let (mut reader, start, end) = open_region(bytes, marks)?;
        let spec = reader.spec();
        let channels = usize::from(spec.channels);
        let source_frames = (end - start) as usize;
        let sample_count = source_frames * channels;
        let source: Vec<f32> = if spec.sample_format == SampleFormat::Float {
            reader
                .samples::<f32>()
                .take(sample_count)
                .collect::<Result<_, _>>()
        } else {
            reader
                .samples::<i16>()
                .take(sample_count)
                .map(|s| s.map(|v| f32::from(v) / 32768.0))
                .collect::<Result<_, _>>()
        }
        .map_err(|error| err(format!("listening WAV: {error}")))?;
        if source.iter().any(|v| !v.is_finite()) {
            return Err(err("Nonfinite practice audio"));
        }
        if source.len() != sample_count {
            return Err(err("truncated listening WAV data"));
        }
        let revision = self.listening_revision.wrapping_add(1);
        let region_frames = (u64::from(end - start) * u64::from(self.rate)
            + u64::from(spec.sample_rate) / 2)
            / u64::from(spec.sample_rate);
        self.send(Command::Listening {
            data: Some(OwnedGc::new(PreparedListening {
                source,
                source_frames,
                channels,
                state: ListeningSnapshot {
                    revision,
                    region_frames: region_frames.max(1),
                    ..Default::default()
                },
            })),
        })?;
        self.listening_revision = revision;
        self.listening_loaded = true;
        Ok(())
    }

    pub fn set_listening_wav_playing(&mut self, playing: bool) -> Result<(), OutputError> {
        if !self.listening_loaded {
            return Err(err("select a marked listening region first"));
        }
        self.send(Command::ListeningPlaying(playing))
    }

    pub fn clear_listening_wav(&mut self) -> Result<(), OutputError> {
        if self.listening_loaded {
            self.send(Command::Listening { data: None })?;
            self.listening_loaded = false;
        }
        Ok(())
    }

    pub fn listening_wav_observation(&self) -> Option<ListeningWavObservation> {
        let snapshot = self.snapshot();
        let region = snapshot.listening;
        (self.listening_loaded && region.revision == self.listening_revision).then_some(
            ListeningWavObservation {
                stream_epoch: self.observation_id(),
                device_frames: snapshot.device_frames,
                region_frames: region.region_frames,
                region_cursor: region.region_cursor,
                completed_wraps: region.completed_wraps,
                playing: region.playing,
                sample_rate: self.sample_rate(),
            },
        )
    }
}
