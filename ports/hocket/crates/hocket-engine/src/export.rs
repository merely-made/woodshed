// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Offline WAV export of Hocket's current audible loop mix.
//!
//! Export is intentionally loop-first: the default path renders one complete
//! shared cycle. It refuses unequal loop lengths rather than guessing a song
//! duration. A caller can instead select an explicit number of musical bars,
//! which repeats free loops until that duration is filled.

use std::collections::BTreeSet;
use std::path::Path;

use hocket_model::{MediaRef, PhraseId, Session, TrackId};

use crate::media::MediaStore;

#[derive(Clone, Debug, PartialEq)]
pub struct RenderedMix {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// The duration policy for an offline mix.
///
/// `OneCycle` preserves loop-first semantics. `Bars` is explicit and derives
/// its frame count from the session tempo and time signature.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExportLength {
    OneCycle,
    Bars(u8),
}

#[derive(Debug)]
pub enum ExportError {
    NoAudibleLayers,
    MissingTrack(usize),
    MissingLayer { track: usize, layer: usize },
    MissingPhrase(PhraseId),
    MissingMedia(MediaRef),
    EmptyMedia(MediaRef),
    SampleRateMismatch { expected: u32, found: u32 },
    UnequalLoopLengths { expected: usize, found: usize },
    InvalidBarDuration(u8),
    InvalidTransport,
    AllocationFailed { frames: usize },
    Wav(hound::Error),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAudibleLayers => f.write_str("there are no audible layers to export"),
            Self::MissingTrack(index) => write!(f, "track index {index} does not exist"),
            Self::MissingLayer { track, layer } => {
                write!(f, "layer index {layer} does not exist on track {track}")
            }
            Self::MissingPhrase(id) => write!(f, "layer references missing phrase {id}"),
            Self::MissingMedia(reference) => write!(f, "media {reference} is unavailable"),
            Self::EmptyMedia(reference) => write!(f, "media {reference} has no samples"),
            Self::SampleRateMismatch { expected, found } => {
                write!(f, "media sample rate {found} does not match {expected}")
            }
            Self::UnequalLoopLengths { expected, found } => {
                write!(
                    f,
                    "loop length {found} does not match shared cycle {expected}"
                )
            }
            Self::InvalidBarDuration(0) => f.write_str("export length must be at least one bar"),
            Self::InvalidBarDuration(bars) => write!(f, "invalid export length: {bars} bars"),
            Self::InvalidTransport => {
                f.write_str("session tempo or time signature cannot define an export duration")
            }
            Self::AllocationFailed { frames } => {
                write!(f, "not enough memory to render {frames} audio frames")
            }
            Self::Wav(error) => write!(f, "WAV export failed: {error}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<hound::Error> for ExportError {
    fn from(error: hound::Error) -> Self {
        Self::Wav(error)
    }
}

/// Render one full shared loop cycle from the current audible session state.
/// Track mute, layer mute, SelectOne, layer gain, and host-local solo all apply.
pub fn render_one_cycle(
    session: &Session,
    media: &impl MediaStore,
    solo: &BTreeSet<TrackId>,
) -> Result<RenderedMix, ExportError> {
    render_mix(session, media, solo, ExportLength::OneCycle)
}

/// Render the requested offline mix duration.
pub fn render_mix(
    session: &Session,
    media: &impl MediaStore,
    solo: &BTreeSet<TrackId>,
    length: ExportLength,
) -> Result<RenderedMix, ExportError> {
    let sources = collect_audible_sources(session, media, solo)?;
    let frames = match length {
        ExportLength::OneCycle => shared_cycle_frames(&sources)?,
        ExportLength::Bars(bars) => frames_for_bars(session, sources[0].sample_rate, bars)?,
    };
    render_sources_for_frames(sources, frames)
}

fn shared_cycle_frames(sources: &[Source<'_>]) -> Result<usize, ExportError> {
    let expected = sources[0].samples.len();
    for source in &sources[1..] {
        if source.samples.len() != expected {
            return Err(ExportError::UnequalLoopLengths {
                expected,
                found: source.samples.len(),
            });
        }
    }
    Ok(expected)
}

fn frames_for_bars(session: &Session, sample_rate: u32, bars: u8) -> Result<usize, ExportError> {
    if bars == 0 {
        return Err(ExportError::InvalidBarDuration(bars));
    }
    let numerator = session.time_signature.numerator;
    let denominator = session.time_signature.denominator;
    if !session.bpm.is_finite() || session.bpm <= 0.0 || numerator == 0 || denominator == 0 {
        return Err(ExportError::InvalidTransport);
    }
    let frames = crate::click::frames_per_bar(sample_rate, session.bpm, numerator, denominator)
        .checked_mul(usize::from(bars))
        .ok_or(ExportError::InvalidTransport)?;
    if frames == 0 {
        return Err(ExportError::InvalidTransport);
    }
    Ok(frames)
}

/// Render a caller-selected duration. Shorter loop buffers repeat to fill the
/// requested frame count, which is useful for future free-capture export UI.
pub fn render_mix_for_frames(
    session: &Session,
    media: &impl MediaStore,
    solo: &BTreeSet<TrackId>,
    frames: usize,
) -> Result<RenderedMix, ExportError> {
    let sources = collect_audible_sources(session, media, solo)?;
    render_sources_for_frames(sources, frames)
}

/// Write a stereo floating-point WAV. Hocket captures mono today, so the
/// offline mix is duplicated to L/R until pan or stereo devices exist.
pub fn write_stereo_wav(path: impl AsRef<Path>, mix: &RenderedMix) -> Result<(), ExportError> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: mix.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in &mix.samples {
        let sample = sample.clamp(-1.0, 1.0);
        writer.write_sample(sample)?;
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Encode the stereo WAV to bytes in memory (the clipboard form of the file
/// [`write_stereo_wav`] writes): 32-bit float stereo, the mono mix duplicated to
/// L/R. This is what "copy audio" places on the clipboard.
pub fn encode_stereo_wav_bytes(mix: &RenderedMix) -> Result<Vec<u8>, ExportError> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: mix.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut buffer, spec)?;
        for sample in &mix.samples {
            let sample = sample.clamp(-1.0, 1.0);
            writer.write_sample(sample)?;
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
    }
    Ok(buffer.into_inner())
}

/// Decode a WAV (any channel count, float or integer) into a mono mix, for
/// pasting audio in as a layer. Interleaved channels are averaged to mono,
/// matching Hocket's mono capture.
pub fn decode_wav_bytes(bytes: &[u8]) -> Result<RenderedMix, ExportError> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes))?;
    let spec = reader.spec();
    let channels = (spec.channels as usize).max(1);
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    let samples: Vec<f32> = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok(RenderedMix {
        sample_rate: spec.sample_rate,
        samples,
    })
}

pub(crate) struct Source<'a> {
    pub(crate) samples: &'a [f32],
    pub(crate) gain: f32,
    pub(crate) sample_rate: u32,
}

fn collect_audible_sources<'a>(
    session: &'a Session,
    media: &'a impl MediaStore,
    solo: &BTreeSet<TrackId>,
) -> Result<Vec<Source<'a>>, ExportError> {
    let mut sources = Vec::new();
    for (track_index, track) in session.tracks.iter().enumerate() {
        if track.muted || (!solo.is_empty() && !solo.contains(&track.id)) {
            continue;
        }
        sources.extend(collect_track_sources(session, media, track_index)?);
    }
    if sources.is_empty() {
        return Err(ExportError::NoAudibleLayers);
    }
    validate_source_sample_rates(&sources)?;
    Ok(sources)
}

pub(crate) fn collect_track_sources<'a>(
    session: &'a Session,
    media: &'a impl MediaStore,
    track_index: usize,
) -> Result<Vec<Source<'a>>, ExportError> {
    let track = session
        .tracks
        .get(track_index)
        .ok_or(ExportError::MissingTrack(track_index))?;
    let mut sources = Vec::new();
    for (index, layer) in track.layers.iter().enumerate() {
        if !track
            .playback_mode
            .is_layer_audible(index as u16, layer.muted)
        {
            continue;
        }
        let phrase = session
            .phrases
            .get(&layer.phrase_id)
            .ok_or(ExportError::MissingPhrase(layer.phrase_id))?;
        let buffer = media
            .get(&phrase.media)
            .ok_or(ExportError::MissingMedia(phrase.media))?;
        if buffer.samples.is_empty() {
            return Err(ExportError::EmptyMedia(phrase.media));
        }
        sources.push(Source {
            samples: &buffer.samples,
            gain: layer.gain,
            sample_rate: buffer.sample_rate,
        });
    }
    validate_source_sample_rates(&sources)?;
    Ok(sources)
}

pub(crate) fn validate_source_sample_rates(sources: &[Source<'_>]) -> Result<(), ExportError> {
    let Some(first) = sources.first() else {
        return Ok(());
    };
    let sample_rate = first.sample_rate;
    for source in &sources[1..] {
        if source.sample_rate != sample_rate {
            return Err(ExportError::SampleRateMismatch {
                expected: sample_rate,
                found: source.sample_rate,
            });
        }
    }
    Ok(())
}

pub(crate) fn render_sources_for_frames(
    sources: Vec<Source<'_>>,
    frames: usize,
) -> Result<RenderedMix, ExportError> {
    let sample_rate = sources[0].sample_rate;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(frames)
        .map_err(|_| ExportError::AllocationFailed { frames })?;
    samples.resize(frames, 0.0);
    for source in sources {
        for (frame, output) in samples.iter_mut().enumerate() {
            *output += source.samples[frame % source.samples.len()] * source.gain;
        }
    }
    Ok(RenderedMix {
        sample_rate,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::InMemoryStore;
    use hocket_model::{Edit, History, Layer, Phrase, Session};

    #[test]
    fn wav_bytes_round_trip_preserves_mono_samples() {
        let mix = RenderedMix {
            sample_rate: 48_000,
            samples: vec![0.0, 0.25, -0.5, 0.75, -1.0],
        };
        let bytes = encode_stereo_wav_bytes(&mix).unwrap();
        let decoded = decode_wav_bytes(&bytes).unwrap();

        assert_eq!(decoded.sample_rate, 48_000);
        // Encode duplicates the mono mix to L/R; decode averages L/R back to
        // mono, so the round trip is an identity on the samples.
        assert_eq!(decoded.samples.len(), mix.samples.len());
        for (before, after) in mix.samples.iter().zip(&decoded.samples) {
            assert!((before - after).abs() < 1e-6, "{before} vs {after}");
        }
    }

    fn append_layer(
        session: &mut Session,
        history: &mut History,
        store: &mut InMemoryStore,
        track_index: usize,
        samples: &[f32],
        gain: f32,
    ) {
        let media = store.put(samples, 48_000);
        let phrase = Phrase::new(media, session.bars_per_phrase, session.bpm, 1);
        let mut layer = Layer::new(phrase.id);
        layer.gain = gain;
        let track_id = session.tracks[track_index].id;
        history.commit(
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            },
            session,
            1,
        );
    }

    #[test]
    fn one_cycle_sums_audible_layers_with_gain() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            0,
            &[0.25, -0.5],
            1.0,
        );
        append_layer(&mut session, &mut history, &mut store, 1, &[0.5, 0.25], 0.5);

        let mix = render_one_cycle(&session, &store, &BTreeSet::new()).unwrap();
        assert_eq!(mix.sample_rate, 48_000);
        assert_eq!(mix.samples, vec![0.5, -0.375]);
    }

    #[test]
    fn solo_and_track_mute_control_the_export_mix() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            0,
            &[0.25, 0.25],
            1.0,
        );
        append_layer(&mut session, &mut history, &mut store, 1, &[0.5, 0.5], 1.0);
        let solo = BTreeSet::from([session.tracks[1].id]);

        let mix = render_one_cycle(&session, &store, &solo).unwrap();
        assert_eq!(mix.samples, vec![0.5, 0.5]);
        session.tracks[1].muted = true;
        assert!(matches!(
            render_one_cycle(&session, &store, &solo),
            Err(ExportError::NoAudibleLayers)
        ));
    }

    #[test]
    fn one_cycle_requires_equal_loop_lengths() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            0,
            &[0.25, 0.25],
            1.0,
        );
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            1,
            &[0.5, 0.5, 0.5],
            1.0,
        );

        assert!(matches!(
            render_one_cycle(&session, &store, &BTreeSet::new()),
            Err(ExportError::UnequalLoopLengths { .. })
        ));
    }

    #[test]
    fn explicit_frames_repeat_shorter_loops() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            0,
            &[0.25, -0.25],
            1.0,
        );

        let mix = render_mix_for_frames(&session, &store, &BTreeSet::new(), 5).unwrap();
        assert_eq!(mix.samples, vec![0.25, -0.25, 0.25, -0.25, 0.25]);
    }

    #[test]
    fn bar_duration_repeats_unequal_free_loops_with_session_meter() {
        let mut session = Session::new_default();
        session.bpm = 120.0;
        session.time_signature = hocket_model::TimeSignature::new(3, 8);
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            0,
            &[0.25, -0.25],
            1.0,
        );
        append_layer(
            &mut session,
            &mut history,
            &mut store,
            1,
            &[0.5, 0.5, 0.5],
            1.0,
        );

        let mix = render_mix(&session, &store, &BTreeSet::new(), ExportLength::Bars(1)).unwrap();
        assert_eq!(mix.samples.len(), 36_000);
        assert_eq!(mix.samples[..6], [0.75, 0.25, 0.75, 0.25, 0.75, 0.25]);
    }

    #[test]
    fn bar_duration_requires_at_least_one_bar() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(&mut session, &mut history, &mut store, 0, &[0.25], 1.0);

        assert!(matches!(
            render_mix(&session, &store, &BTreeSet::new(), ExportLength::Bars(0)),
            Err(ExportError::InvalidBarDuration(0))
        ));
    }

    #[test]
    fn render_reports_an_unrepresentable_frame_request() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut store = InMemoryStore::new();
        append_layer(&mut session, &mut history, &mut store, 0, &[0.25], 1.0);

        assert!(matches!(
            render_mix_for_frames(&session, &store, &BTreeSet::new(), usize::MAX),
            Err(ExportError::AllocationFailed { frames: usize::MAX })
        ));
    }

    #[test]
    fn writes_stereo_float_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mix.wav");
        let mix = RenderedMix {
            sample_rate: 48_000,
            samples: vec![1.2, -1.2],
        };

        write_stereo_wav(&path, &mix).unwrap();
        let mut reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 48_000);
        let samples: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
        assert_eq!(samples, vec![1.0, 1.0, -1.0, -1.0]);
    }
}
