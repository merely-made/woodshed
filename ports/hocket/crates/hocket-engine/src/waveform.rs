// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Real waveform overviews projected from Hocket session and media state.

use audio_primitives::{WaveformPeak, min_max_peaks};
use hocket_model::Session;

use crate::export::{ExportError, collect_track_sources, render_sources_for_frames};
use crate::media::MediaStore;

/// Render one track's audible layer mix into signed min/max columns.
///
/// Layer mute, gain, and `PlaybackMode` apply. Track mute and host-local solo do
/// not: this is the track's content overview, even while its output is silenced.
/// Shorter free loops repeat to the longest audible layer, matching explicit
/// duration export semantics.
pub fn render_track_peaks(
    session: &Session,
    media: &impl MediaStore,
    track_index: usize,
    columns: usize,
) -> Result<Vec<WaveformPeak>, ExportError> {
    if columns == 0 {
        return Ok(Vec::new());
    }
    let sources = collect_track_sources(session, media, track_index)?;
    let frames = sources
        .iter()
        .map(|source| source.samples.len())
        .max()
        .ok_or(ExportError::NoAudibleLayers)?;
    let mix = render_sources_for_frames(sources, frames)?;
    Ok(min_max_peaks(&mix.samples, columns))
}

/// Render one layer's stored media into signed min/max columns.
///
/// The layer remains visible while muted; its linear gain still scales the
/// shape so the overview matches its contribution when audible.
pub fn render_layer_peaks(
    session: &Session,
    media: &impl MediaStore,
    track_index: usize,
    layer_index: usize,
    columns: usize,
) -> Result<Vec<WaveformPeak>, ExportError> {
    let track = session
        .tracks
        .get(track_index)
        .ok_or(ExportError::MissingTrack(track_index))?;
    let layer = track
        .layers
        .get(layer_index)
        .ok_or(ExportError::MissingLayer {
            track: track_index,
            layer: layer_index,
        })?;
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
    Ok(min_max_peaks(&buffer.samples, columns)
        .into_iter()
        .map(|peak| peak.scaled(layer.gain))
        .collect())
}

#[cfg(test)]
mod tests {
    use hocket_model::{Layer, Phrase, PlaybackMode, Session};

    use super::*;
    use crate::media::{InMemoryStore, MediaStore};

    fn add_layer(
        session: &mut Session,
        store: &mut InMemoryStore,
        track: usize,
        samples: &[f32],
        gain: f32,
    ) {
        let media = store.put(samples, 48_000);
        let phrase = Phrase::new(media, session.bars_per_phrase, session.bpm, 1);
        let mut layer = Layer::new(phrase.id);
        layer.gain = gain;
        session.phrases.insert(phrase.id, phrase);
        session.tracks[track].layers.push(layer);
    }

    #[test]
    fn track_peaks_use_real_mixed_samples_and_gain() {
        let mut session = Session::new_default();
        let mut store = InMemoryStore::new();
        add_layer(&mut session, &mut store, 0, &[1.0, -1.0, 0.0, 0.5], 0.5);
        add_layer(&mut session, &mut store, 0, &[0.5, 0.5], 1.0);

        assert_eq!(
            render_track_peaks(&session, &store, 0, 2).unwrap(),
            vec![
                WaveformPeak { min: 0.0, max: 1.0 },
                WaveformPeak {
                    min: 0.5,
                    max: 0.75
                },
            ]
        );
    }

    #[test]
    fn track_peaks_follow_select_one_and_layer_mute() {
        let mut session = Session::new_default();
        let mut store = InMemoryStore::new();
        add_layer(&mut session, &mut store, 0, &[1.0, -1.0], 1.0);
        add_layer(&mut session, &mut store, 0, &[0.25, -0.25], 1.0);
        session.tracks[0].playback_mode = PlaybackMode::SelectOne { active: Some(1) };

        assert_eq!(
            render_track_peaks(&session, &store, 0, 1).unwrap(),
            vec![WaveformPeak {
                min: -0.25,
                max: 0.25
            }]
        );
        session.tracks[0].layers[1].muted = true;
        assert!(matches!(
            render_track_peaks(&session, &store, 0, 1),
            Err(ExportError::NoAudibleLayers)
        ));
    }

    #[test]
    fn layer_peaks_remain_available_while_muted() {
        let mut session = Session::new_default();
        let mut store = InMemoryStore::new();
        add_layer(&mut session, &mut store, 0, &[-0.5, 1.0], 0.5);
        session.tracks[0].layers[0].muted = true;

        assert_eq!(
            render_layer_peaks(&session, &store, 0, 0, 1).unwrap(),
            vec![WaveformPeak {
                min: -0.25,
                max: 0.5
            }]
        );
    }

    #[test]
    fn missing_media_is_reported_instead_of_fabricated() {
        let mut session = Session::new_default();
        let mut populated = InMemoryStore::new();
        add_layer(&mut session, &mut populated, 0, &[1.0], 1.0);

        assert!(matches!(
            render_track_peaks(&session, &InMemoryStore::new(), 0, 8),
            Err(ExportError::MissingMedia(_))
        ));
    }

    #[test]
    fn zero_columns_is_an_empty_projection() {
        let session = Session::new_default();
        assert!(
            render_track_peaks(&session, &InMemoryStore::new(), 0, 0)
                .unwrap()
                .is_empty()
        );
    }
}
