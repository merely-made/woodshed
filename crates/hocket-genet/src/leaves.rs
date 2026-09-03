// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! S5: chisel leaves for the loop waveforms + the output meter.
//!
//! The signature visual — each track's summed loop — is now a chisel Path-A
//! leaf (a filled, mirrored amplitude envelope) rather than a row of CSS bars.
//! The output meter is Sprigging's built-in [`sprigging::Meter`]. The view places a
//! `<chisel-leaf key=…>` box; the host owns the leaves out of band in a
//! [`sprigging::LeafRegistry`] and reconciles them from [`AppState`] each frame,
//! so genet stays a uniform-DOM engine and the widget content lives host-side.
//!
//! Keys derive from stable track/phrase identities inside disjoint waveform and
//! meter namespaces, so reordering does not retarget retained leaf state.

use std::collections::{HashMap, HashSet};

use audio_primitives::WaveformPeak;
use hocket_model::{MediaRef, PhraseId, TrackColor, TrackId};
use sprigging::{ColorF, Leaf, LeafRegistry, PaintCx, Path, Size, SizeHint};

#[cfg(test)]
use paint_list_api::PaintCmd;
#[cfg(test)]
use sprigging::RenderedLeaves;

use crate::state::AppState;

// --- key scheme ---------------------------------------------------------

const TRACK_WAVE_NAMESPACE: u64 = 0x1000_0000_0000_0000;
const LAYER_WAVE_NAMESPACE: u64 = 0x2000_0000_0000_0000;
const KEY_PAYLOAD_MASK: u64 = 0x0fff_ffff_ffff_ffff;
const TRACK_COLUMNS: usize = 128;
const LAYER_COLUMNS: usize = 48;

/// Stable summed-waveform leaf key for a model track.
pub fn wave_key(track: TrackId) -> u64 {
    TRACK_WAVE_NAMESPACE | (hash_ids(&[track.0.as_bytes()]) & KEY_PAYLOAD_MASK)
}

/// Stable mini-waveform leaf key for one layer within a track.
pub fn layer_wave_key(track: TrackId, phrase: PhraseId) -> u64 {
    LAYER_WAVE_NAMESPACE | (hash_ids(&[track.0.as_bytes(), phrase.0.as_bytes()]) & KEY_PAYLOAD_MASK)
}

fn hash_ids(parts: &[&[u8]]) -> u64 {
    parts
        .iter()
        .flat_map(|part| part.iter().copied())
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}
/// The two output-meter leaves (L / R).
pub const METER_L: u64 = 0x3000_0000_0000_0000;
pub const METER_R: u64 = 0x3000_0000_0000_0001;

// --- the waveform leaf --------------------------------------------------

/// A filled, mirrored amplitude envelope. Peaks are `0..1` column samples; the
/// leaf paints a closed polygon from the top envelope across and back along the
/// mirrored bottom, filled with the owner's colour. Resolution-independent and
/// tile-cached — the payoff over the CSS-bar stand-in.
pub struct WaveformLeaf {
    peaks: Vec<WaveformPeak>,
    color: ColorF,
    intrinsic: Size,
    /// Content signature (owner colour + peak fingerprint). Repaint only when it
    /// moves — a stable take paints once, then the retention gate holds.
    sig: u64,
    dirty: bool,
}

impl WaveformLeaf {
    fn new(peaks: Vec<WaveformPeak>, color: ColorF, sig: u64, intrinsic: Size) -> Self {
        Self {
            peaks,
            color,
            intrinsic,
            sig,
            dirty: true,
        }
    }

    /// Re-seed from new content only when the signature moved — the peak Vec is
    /// computed lazily so an unchanged take costs nothing on the redraw path.
    fn update(&mut self, peaks: impl FnOnce() -> Vec<WaveformPeak>, color: ColorF, sig: u64) {
        if sig != self.sig {
            self.peaks = peaks();
            self.color = color;
            self.sig = sig;
            self.dirty = true;
        }
    }
}

impl Leaf for WaveformLeaf {
    fn measure(&mut self, _known: SizeHint, _available: SizeHint) -> Size {
        self.intrinsic
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let s = cx.size();
        let n = self.peaks.len().max(1);
        let mid = s.height * 0.5;
        let amplitude = (s.height * 0.5 - 1.0).max(1.0);
        let y = |sample: f32| mid - sample.clamp(-1.0, 1.0) * amplitude;
        let x_at = |i: usize| (i as f32 / (n - 1).max(1) as f32) * s.width;

        if self
            .peaks
            .iter()
            .all(|peak| peak.min == 0.0 && peak.max == 0.0)
        {
            cx.fill_rect(0.0, mid, s.width, 1.0, self.color);
            self.dirty = false;
            return;
        }

        // Maximum envelope left-to-right, then minimum right-to-left.
        let mut path = Path::new().move_to(0.0, y(self.peaks[0].max));
        for i in 1..n {
            path = path.line_to(x_at(i), y(self.peaks[i].max));
        }
        for i in (0..n).rev() {
            path = path.line_to(x_at(i), y(self.peaks[i].min));
        }
        cx.fill_path(path.close().build(), self.color);
        self.dirty = false;
    }

    fn paint_dirty(&self) -> bool {
        self.dirty
    }
}

// --- reconcile from AppState -------------------------------------------

fn color_of(c: TrackColor) -> ColorF {
    ColorF {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: 1.0,
    }
}

/// Host-owned waveform projection cache. Leaf retention prevents per-frame
/// recomputation; these maps additionally deduplicate media projections across
/// views and discard superseded track mixes during reconciliation.
#[derive(Default)]
pub struct WaveformCache {
    tracks: HashMap<(TrackId, u64, usize), Vec<WaveformPeak>>,
    layers: HashMap<(MediaRef, u32, usize), Vec<WaveformPeak>>,
}

impl WaveformCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn track_peaks(
        &mut self,
        state: &AppState,
        track_index: usize,
        signature: u64,
        columns: usize,
    ) -> Option<Vec<WaveformPeak>> {
        let track_id = state.session.tracks.get(track_index)?.id;
        let key = (track_id, signature, columns);
        if !self.tracks.contains_key(&key) {
            self.tracks
                .insert(key, state.render_track_waveform(track_index, columns)?);
        }
        self.tracks.get(&key).cloned()
    }

    fn layer_peaks(
        &mut self,
        state: &AppState,
        track_index: usize,
        layer_index: usize,
        columns: usize,
    ) -> Option<Vec<WaveformPeak>> {
        let track = state.session.tracks.get(track_index)?;
        let layer = track.layers.get(layer_index)?;
        let media = state.session.phrases.get(&layer.phrase_id)?.media;
        let key = (media, layer.gain.to_bits(), columns);
        if !self.layers.contains_key(&key) {
            self.layers.insert(
                key,
                state.render_layer_waveform(track_index, layer_index, columns)?,
            );
        }
        self.layers.get(&key).cloned()
    }

    fn retain_current(
        &mut self,
        track_signatures: &HashMap<TrackId, u64>,
        active_media: &HashSet<MediaRef>,
    ) {
        self.tracks
            .retain(|(track, signature, _), _| track_signatures.get(track) == Some(signature));
        self.layers
            .retain(|(media, _, _), _| active_media.contains(media));
    }
}

/// A signature of a track's audible content: its unmuted layers' phrase ids.
/// Moves when a layer is added, muted, or unmuted — which is exactly when the
/// summed envelope should re-seed.
fn track_sig(track: &hocket_model::Track, state: &AppState) -> u64 {
    let mut signature = hash_ids(&[track.id.0.as_bytes()]);
    signature = match track.playback_mode {
        hocket_model::PlaybackMode::Sum => signature ^ 0x51,
        hocket_model::PlaybackMode::SelectOne { active } => {
            signature ^ 0xa7 ^ u64::from(active.unwrap_or(u16::MAX))
        }
    };
    for layer in &track.layers {
        let media = state
            .session
            .phrases
            .get(&layer.phrase_id)
            .map(|phrase| phrase.media.0)
            .unwrap_or([0; 32]);
        signature = hash_ids(&[
            &signature.to_le_bytes(),
            &media,
            &layer.gain.to_bits().to_le_bytes(),
        ]);
        if layer.muted {
            signature ^= 0x9e37_79b9_7f4a_7c15;
        }
    }
    signature
}

/// Ensure the registry holds exactly the leaves the current session needs, with
/// up-to-date content. Called each frame before rendering; cheap when nothing
/// changed (signatures match → no re-seed, no repaint).
pub fn reconcile(registry: &mut LeafRegistry<u64>, cache: &mut WaveformCache, state: &AppState) {
    let mut active_keys = HashSet::from([METER_L, METER_R]);
    let mut track_signatures = HashMap::new();
    let mut active_media = HashSet::new();

    for (i, track) in state.session.tracks.iter().enumerate() {
        let c = track.color;
        let sig = track_sig(track, state) ^ ((c.r as u64) << 16 | (c.g as u64) << 8 | c.b as u64);
        track_signatures.insert(track.id, sig);
        let color = color_of(c);

        if state.track_waveform_available(i) {
            let key = wave_key(track.id);
            if let Some(peaks) = cache.track_peaks(state, i, sig, TRACK_COLUMNS) {
                ensure_waveform(
                    registry,
                    key,
                    peaks,
                    color,
                    sig,
                    Size {
                        width: 280.0,
                        height: 40.0,
                    },
                );
                active_keys.insert(key);
            }
        }

        for (layer_index, layer) in track.layers.iter().enumerate() {
            let Some(phrase) = state.session.phrases.get(&layer.phrase_id) else {
                continue;
            };
            active_media.insert(phrase.media);
            if !state.layer_waveform_available(i, layer_index) {
                continue;
            }
            let key = layer_wave_key(track.id, layer.phrase_id);
            let layer_sig = sig ^ hash_ids(&[layer.phrase_id.0.as_bytes()]);
            if let Some(peaks) = cache.layer_peaks(state, i, layer_index, LAYER_COLUMNS) {
                ensure_waveform(
                    registry,
                    key,
                    peaks,
                    color,
                    layer_sig,
                    Size {
                        width: 180.0,
                        height: 11.0,
                    },
                );
                active_keys.insert(key);
            }
        }
    }
    ensure_meter(
        registry,
        METER_L,
        state.meter_level(0),
        state.meter_peak_level(0),
    );
    ensure_meter(
        registry,
        METER_R,
        state.meter_level(1),
        state.meter_peak_level(1),
    );
    registry.retain(|key| active_keys.contains(key));
    cache.retain_current(&track_signatures, &active_media);
}

fn ensure_waveform(
    registry: &mut LeafRegistry<u64>,
    key: u64,
    peaks: Vec<WaveformPeak>,
    color: ColorF,
    sig: u64,
    intrinsic: Size,
) {
    if let Some(leaf) = registry.get_mut_as::<WaveformLeaf>(&key) {
        leaf.update(|| peaks, color, sig);
    } else {
        registry.insert(
            key,
            Box::new(WaveformLeaf::new(peaks, color, sig, intrinsic)),
        );
    }
}

fn ensure_meter(registry: &mut LeafRegistry<u64>, key: u64, level: f32, peak: f32) {
    if let Some(m) = registry.get_mut_as::<sprigging::Meter>(&key) {
        m.set_level(level, Some(peak));
    } else {
        let mut m = sprigging::Meter::new(
            true,
            Size {
                width: 10.0,
                height: 46.0,
            },
        );
        // Match the sheet: teal fill on a dim track, amber peak tick.
        m.track_color = ColorF {
            r: 0.16,
            g: 0.14,
            b: 0.10,
            a: 1.0,
        };
        m.fill_color = ColorF {
            r: 0.34,
            g: 0.70,
            b: 0.66,
            a: 1.0,
        };
        m.peak_color = ColorF {
            r: 0.88,
            g: 0.65,
            b: 0.29,
            a: 1.0,
        };
        m.set_level(level, Some(peak));
        registry.insert(key, Box::new(m));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color() -> ColorF {
        ColorF {
            r: 0.2,
            g: 0.7,
            b: 0.5,
            a: 1.0,
        }
    }

    #[test]
    fn model_identity_produces_stable_distinct_leaf_keys() {
        let track_a = TrackId::new();
        let track_b = TrackId::new();
        let phrase = PhraseId::new();
        assert_eq!(wave_key(track_a), wave_key(track_a));
        assert_ne!(wave_key(track_a), wave_key(track_b));
        assert_ne!(wave_key(track_a), layer_wave_key(track_a, phrase));
        assert_ne!(
            layer_wave_key(track_a, phrase),
            layer_wave_key(track_b, phrase)
        );
    }

    #[test]
    fn unchanged_waveform_signature_produces_zero_repaints() {
        let key = wave_key(TrackId::new());
        let peaks = vec![WaveformPeak {
            min: -0.5,
            max: 0.75,
        }];
        let size = Size {
            width: 100.0,
            height: 40.0,
        };
        let mut registry = LeafRegistry::new();
        let mut rendered = RenderedLeaves::new();
        ensure_waveform(&mut registry, key, peaks.clone(), color(), 7, size);
        assert_eq!(registry.render_into(|_| Some(size), &mut rendered), 1);

        ensure_waveform(&mut registry, key, peaks, color(), 7, size);
        assert_eq!(registry.render_into(|_| Some(size), &mut rendered), 0);
    }

    #[test]
    fn silent_waveform_paints_a_center_line() {
        let size = Size {
            width: 80.0,
            height: 20.0,
        };
        let mut leaf = WaveformLeaf::new(vec![WaveformPeak::default(); 4], color(), 1, size);
        let mut commands = Vec::new();
        let mut cx = PaintCx::new(&mut commands, size);
        leaf.paint(&mut cx);
        assert!(matches!(commands.as_slice(), [PaintCmd::DrawRect(_)]));
    }
}
