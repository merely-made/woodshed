//! A frozen accepted backing snapshot rendered by the production output graph.

use std::io::{BufWriter, Seek, Write};

use crate::output_runtime::{MAX_BARS, MAX_PCM_FRAMES};
use crate::{OutputError, OutputRuntime, PendingChange, Song};

const CHUNK_FRAMES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongExportReport {
    pub frames: u64,
    pub sample_rate: u32,
    /// This export is mono, matching the backing's mono mix before device fanout.
    pub channels: u16,
}

#[derive(Debug)]
pub enum SongExportError {
    InvalidSnapshot(String),
    Runtime(OutputError),
    Wav(hound::Error),
}

impl std::fmt::Display for SongExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSnapshot(message) => f.write_str(message),
            Self::Runtime(error) => write!(f, "backing renderer: {error}"),
            Self::Wav(error) => write!(f, "WAV export: {error}"),
        }
    }
}
impl std::error::Error for SongExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Wav(error) => Some(error),
            Self::InvalidSnapshot(_) => None,
        }
    }
}
impl From<OutputError> for SongExportError {
    fn from(error: OutputError) -> Self {
        Self::Runtime(error)
    }
}
impl From<hound::Error> for SongExportError {
    fn from(error: hound::Error) -> Self {
        Self::Wav(error)
    }
}

/// Export one complete accepted backing cycle from bar zero as mono 32-bit
/// IEEE-float WAV. The writer must be an empty, seekable destination positioned
/// at zero; the host owns temporary-file publication and cleanup on I/O error.
///
/// Completed per-bar captures, variable clocks, chord strikes, backing click,
/// live default gain (0.8), and clipping use the production Firewheel processor.
/// Count-in, previews and the independent metronome lane are excluded. The end
/// is exactly the final segment boundary, with no appended envelope/reverb tail,
/// whether the original source loops or is one-shot. Snapshot cursor/play state
/// and unrelated queued transport commands do not alter the complete cycle.
///
/// The whole snapshot and production graph preparation are checked before the
/// first writer operation. Active/queued capture is refused. Render and write
/// buffers stay bounded; this creates no native input or output device and never
/// mutates a live runtime or the caller's snapshot.
pub fn export_song_wav_to<W: Write + Seek>(
    song: &Song,
    sample_rate: u32,
    writer: W,
) -> Result<SongExportReport, SongExportError> {
    let frames = validate(song, sample_rate)?;
    let mut frozen = song.clone();
    frozen.playing = true;
    frozen.one_shot = true;
    frozen.cursor = Default::default();
    frozen.pending.clear();
    let mut runtime = OutputRuntime::offline(sample_rate)?;
    runtime.set_song(frozen)?;
    // Rewind is processed after backing admission. It retains Playing(true),
    // clears its normal count-in, and starts the first bar on the first frame.
    runtime.rewind()?;
    runtime.service()?;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::new(BufWriter::with_capacity(65536, writer), spec)?;
    let mut scratch = [0.0f32; CHUNK_FRAMES * 2];
    let mut remaining = frames;
    while remaining > 0 {
        let block_frames = remaining.min(CHUNK_FRAMES as u64) as usize;
        runtime.render_offline(&mut scratch[..block_frames * 2])?;
        for frame in scratch[..block_frames * 2].chunks_exact(2) {
            // The live backing is mono and duplicates this value to channels.
            if !frame[0].is_finite() {
                return Err(invalid("Backing renderer produced nonfinite PCM."));
            }
            wav.write_sample(frame[0])?;
        }
        runtime.service()?;
        remaining -= block_frames as u64;
    }
    wav.finalize()?;
    Ok(SongExportReport {
        frames,
        sample_rate,
        channels: 1,
    })
}

fn invalid(message: impl Into<String>) -> SongExportError {
    SongExportError::InvalidSnapshot(message.into())
}

fn validate(song: &Song, sample_rate: u32) -> Result<u64, SongExportError> {
    if !(1000..=384000).contains(&sample_rate) {
        return Err(invalid(
            "Backing export sample rate must be 1000..=384000 Hz.",
        ));
    }
    if song.bars.is_empty() || song.bars.len() > MAX_BARS {
        return Err(invalid("Backing export requires 1..=256 segments."));
    }
    if song.recording
        || song.pending.iter().any(|change| {
            matches!(
                change,
                PendingChange::StartRecording { .. } | PendingChange::StopRecording
            )
        })
    {
        return Err(invalid(
            "Stop and collect recording before exporting completed backing.",
        ));
    }
    let mut frames = 0u64;
    let mut phrase_work = 0u64;
    for bar in &song.bars {
        if !bar.bpm.is_finite()
            || bar.bpm <= 0.0
            || bar.time_signature.numerator == 0
            || bar.time_signature.denominator == 0
            || bar.length == 0
        {
            return Err(invalid("Backing export contains an invalid segment clock."));
        }
        // Match the live processor's exact rounded clock and minimum frame.
        let duration = bar.duration_samples(sample_rate as f32).max(1);
        frames = frames
            .checked_add(duration)
            .filter(|n| *n <= MAX_PCM_FRAMES as u64)
            .ok_or_else(|| invalid("Backing export exceeds 16777216 cycle frames."))?;
        if bar.chord_ref.is_some() && bar.backing_phrase.is_some() {
            return Err(invalid(
                "Backing export chord and phrase are mutually exclusive.",
            ));
        }
        if let Some(phrase) = &bar.backing_phrase {
            let beats =
                u64::from(bar.time_signature.numerator).saturating_mul(u64::from(bar.length));
            let layout = crate::backing_phrase::preflight(
                phrase,
                duration as usize,
                bar.beat_frames(sample_rate),
                beats,
                sample_rate,
            )
            .map_err(|message| invalid(format!("Backing export phrase: {message}.")))?;
            phrase_work = phrase_work
                .checked_add(layout.work_frames)
                .filter(|work| *work <= crate::backing_phrase::MAX_RENDER_WORK)
                .ok_or_else(|| invalid("Backing export exceeds phrase render-work bound."))?;
        }
        if let Some(chord) = &bar.chord_ref {
            if chord.pitches_hz.len() > 256 {
                return Err(invalid("Backing export chord exceeds 256 pitches."));
            }
            let fallback = [chord.root_freq_hz];
            let pitches = if chord.pitches_hz.is_empty() {
                &fallback[..]
            } else {
                &chord.pitches_hz
            };
            let chord_duration = (bar.time_signature.numerator as f32 * (60.0 / bar.bpm.max(1.0)))
                .max(0.5)
                .clamp(0.3, 2.0);
            if pitches.iter().any(|frequency| {
                !frequency.is_finite()
                    || *frequency <= 0.0
                    || *frequency as f64
                        * chord_duration as f64
                        * 3.0
                        * std::f32::consts::TAU as f64
                        > f32::MAX as f64 / 2.0
            }) {
                return Err(invalid(
                    "Backing export chord frequencies cannot render finite PCM.",
                ));
            }
        }
        if let Some(capture) = &bar.audio_buffer {
            if capture.sample_rate_hz != sample_rate || capture.len() as u64 != duration {
                return Err(invalid(
                    "Captured loop rate or frame count differs from its accepted backing clock.",
                ));
            }
            if capture.data.iter().any(|sample| !sample.is_finite()) {
                return Err(invalid("Captured loop contains nonfinite PCM."));
            }
        }
    }
    Ok(frames)
}

#[cfg(test)]
mod tests;
