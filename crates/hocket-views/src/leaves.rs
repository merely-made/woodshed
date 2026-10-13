// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Real waveform and meter leaves for reusable recorder mounts.
//! Media projection stays in the portable Recorder. Hosts own the registry;
//! this module qualifies keys by mount and reconciles only its own leaves.

use std::collections::{HashMap, HashSet};

use audio_primitives::WaveformPeak;
use hocket_model::{MediaRef, PhraseId, TrackColor, TrackId};
use sprigging::{ColorF, Leaf, LeafRegistry, PaintCx, Path, Size, SizeHint};

#[cfg(test)]
use paint_list_api::PaintCmd;
#[cfg(test)]
use sprigging::RenderedLeaves;

use hocket_recorder::Recorder;

// Mount ownership is explicit; callers give each simultaneously mounted
// recorder a distinct id and retain the same id while rebuilding that mount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecorderMount(u64);

impl RecorderMount {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
    pub const fn id(self) -> u64 {
        self.0
    }
    pub fn wave_key(self, track: TrackId) -> u64 {
        TRACK_WAVE_NAMESPACE
            | (hash_ids(&[&self.0.to_le_bytes(), track.0.as_bytes()]) & KEY_PAYLOAD_MASK)
    }
    pub fn layer_wave_key(self, track: TrackId, layer_index: usize, phrase: PhraseId) -> u64 {
        LAYER_WAVE_NAMESPACE
            | (hash_ids(&[
                &self.0.to_le_bytes(),
                track.0.as_bytes(),
                &(layer_index as u64).to_le_bytes(),
                phrase.0.as_bytes(),
            ]) & KEY_PAYLOAD_MASK)
    }
    pub fn meter_key(self, channel: u8) -> u64 {
        METER_NAMESPACE | (hash_ids(&[&self.0.to_le_bytes(), &[channel]]) & KEY_PAYLOAD_MASK)
    }
}

const TRACK_WAVE_NAMESPACE: u64 = 0x1000_0000_0000_0000;
const LAYER_WAVE_NAMESPACE: u64 = 0x2000_0000_0000_0000;
const METER_NAMESPACE: u64 = 0x3000_0000_0000_0000;
const KEY_PAYLOAD_MASK: u64 = 0x0fff_ffff_ffff_ffff;
const TRACK_COLUMNS: usize = 128;
const LAYER_COLUMNS: usize = 48;

fn hash_ids(parts: &[&[u8]]) -> u64 {
    parts
        .iter()
        .flat_map(|part| part.iter().copied())
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

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

// --- reconcile from the portable Recorder -------------------------------------------

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
struct ProjectionCache {
    tracks: HashMap<(TrackId, u64, usize), Vec<WaveformPeak>>,
    layers: HashMap<(MediaRef, u32, usize), Vec<WaveformPeak>>,
}

impl ProjectionCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn track_peaks(
        &mut self,
        state: &Recorder,
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
        state: &Recorder,
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

/// Per-mount leaf ownership and cached real media projections. Reconciliation
/// removes only this mount's retired leaves, preserving unrelated host leaves.
pub struct RecorderLeaves {
    mount: RecorderMount,
    cache: ProjectionCache,
    keys: HashSet<u64>,
}

impl RecorderLeaves {
    pub fn new(mount: RecorderMount) -> Self {
        Self {
            mount,
            cache: ProjectionCache::new(),
            keys: HashSet::new(),
        }
    }

    pub fn mount(&self) -> RecorderMount {
        self.mount
    }

    pub fn reconcile(&mut self, registry: &mut LeafRegistry<u64>, state: &Recorder) {
        let mut active_keys = HashSet::from([self.mount.meter_key(0), self.mount.meter_key(1)]);
        let mut track_signatures = HashMap::new();
        let mut active_media = HashSet::new();
        for (i, track) in state.session.tracks.iter().enumerate() {
            let color = color_of(track.color);
            let color_sig = (u64::from(track.color.r) << 16)
                | (u64::from(track.color.g) << 8)
                | u64::from(track.color.b);
            let signature = track_sig(track, state);
            if state.track_waveform_available(i) {
                let sig = signature ^ color_sig;
                track_signatures.insert(track.id, sig);
                if let Some(peaks) = self.cache.track_peaks(state, i, sig, TRACK_COLUMNS) {
                    let key = self.mount.wave_key(track.id);
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
                let signature = signature ^ hash_ids(&[layer.phrase_id.0.as_bytes()]);
                if let Some(peaks) = self.cache.layer_peaks(state, i, layer_index, LAYER_COLUMNS) {
                    let key = self
                        .mount
                        .layer_wave_key(track.id, layer_index, layer.phrase_id);
                    ensure_waveform(
                        registry,
                        key,
                        peaks,
                        color,
                        signature ^ color_sig,
                        Size {
                            width: 180.0,
                            height: 11.0,
                        },
                    );
                    active_keys.insert(key);
                }
            }
        }
        for channel in 0..2 {
            ensure_meter(
                registry,
                self.mount.meter_key(channel as u8),
                state.meter_level(channel),
                state.meter_peak_level(channel),
            );
        }
        registry.retain(|key| !self.keys.contains(key) || active_keys.contains(key));
        self.keys = active_keys;
        self.cache.retain_current(&track_signatures, &active_media);
    }

    /// The host calls this when unmounting; other mounts retain their leaves.
    pub fn unmount(&mut self, registry: &mut LeafRegistry<u64>) {
        registry.retain(|key| !self.keys.contains(key));
        self.keys.clear();
        self.cache = ProjectionCache::new();
    }
}

/// Renderer content signature, independent of row position and audio devices.
fn track_sig(track: &hocket_model::Track, state: &Recorder) -> u64 {
    let mut signature = hash_ids(&[track.id.0.as_bytes()]);
    signature = match track.playback_mode {
        hocket_model::PlaybackMode::Sum => signature ^ 0x51,
        hocket_model::PlaybackMode::SelectOne { active } => {
            signature ^ 0xa7 ^ u64::from(active.unwrap_or(u16::MAX))
        },
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
        assert_eq!(
            RecorderMount::new(1).wave_key(track_a),
            RecorderMount::new(1).wave_key(track_a)
        );
        assert_ne!(
            RecorderMount::new(1).wave_key(track_a),
            RecorderMount::new(1).wave_key(track_b)
        );
        assert_ne!(
            RecorderMount::new(1).wave_key(track_a),
            RecorderMount::new(1).layer_wave_key(track_a, 0, phrase)
        );
        assert_ne!(
            RecorderMount::new(1).layer_wave_key(track_a, 0, phrase),
            RecorderMount::new(1).layer_wave_key(track_b, 0, phrase)
        );
    }

    #[test]
    fn unchanged_waveform_signature_produces_zero_repaints() {
        let key = RecorderMount::new(1).wave_key(TrackId::new());
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

    fn recorder_with_aliases() -> Recorder {
        use hocket_model::{History, Layer, Phrase, Session};
        use hocket_project::media::{InMemoryStore, MediaStore};
        use std::collections::BTreeSet;

        let mut session = Session::new_default();
        let mut store = InMemoryStore::new();
        let media = store.put(&[-0.5, 0.75, -0.25, 0.5], 48_000);
        let phrase = Phrase::new(media, 1, 120.0, 0);
        let mut quieter = Layer::new(phrase.id);
        quieter.gain = 0.25;
        session.tracks[0].layers = vec![Layer::new(phrase.id), quieter];
        session.phrases.insert(phrase.id, phrase);
        Recorder::new(session, History::new(), store, BTreeSet::new())
    }

    #[test]
    fn same_phrase_occurrences_render_independent_real_gain_envelopes() {
        let recorder = recorder_with_aliases();
        let mount = RecorderMount::new(8);
        let track = &recorder.session.tracks[0];
        let phrase = track.layers[0].phrase_id;
        let first = mount.layer_wave_key(track.id, 0, phrase);
        let second = mount.layer_wave_key(track.id, 1, phrase);
        assert_ne!(first, second);
        let mut registry = LeafRegistry::new();
        RecorderLeaves::new(mount).reconcile(&mut registry, &recorder);
        let first_max = registry
            .get_mut_as::<WaveformLeaf>(&first)
            .unwrap()
            .peaks
            .iter()
            .map(|peak| peak.max)
            .fold(f32::NEG_INFINITY, f32::max);
        let second_max = registry
            .get_mut_as::<WaveformLeaf>(&second)
            .unwrap()
            .peaks
            .iter()
            .map(|peak| peak.max)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(first_max > 0.0);
        assert!((second_max - first_max * 0.25).abs() < 1e-6);
    }

    #[test]
    fn mount_reconciliation_reorder_and_unmount_preserve_other_leaf_owners() {
        let mut recorder = recorder_with_aliases();
        let a = RecorderMount::new(8);
        let b = RecorderMount::new(9);
        let track = recorder.session.tracks[0].id;
        assert_ne!(a.wave_key(track), b.wave_key(track));
        assert_ne!(a.meter_key(0), b.meter_key(0));
        assert_ne!(a.meter_key(0), a.meter_key(1));
        let mut first = RecorderLeaves::new(a);
        let mut second = RecorderLeaves::new(b);
        let mut registry = LeafRegistry::new();
        first.reconcile(&mut registry, &recorder);
        second.reconcile(&mut registry, &recorder);
        recorder.session.tracks.swap(0, 1);
        first.reconcile(&mut registry, &recorder);
        assert!(
            registry
                .get_mut_as::<WaveformLeaf>(&a.wave_key(track))
                .is_some()
        );
        assert!(
            registry
                .get_mut_as::<WaveformLeaf>(&b.wave_key(track))
                .is_some()
        );
        recorder.session.tracks[1].layers.clear();
        first.reconcile(&mut registry, &recorder);
        assert!(
            registry
                .get_mut_as::<WaveformLeaf>(&a.wave_key(track))
                .is_none()
        );
        assert!(
            registry
                .get_mut_as::<WaveformLeaf>(&b.wave_key(track))
                .is_some()
        );
        first.unmount(&mut registry);
        assert!(
            registry
                .get_mut_as::<sprigging::Meter>(&a.meter_key(0))
                .is_none()
        );
        assert!(
            registry
                .get_mut_as::<sprigging::Meter>(&b.meter_key(0))
                .is_some()
        );
        assert!(
            registry
                .get_mut_as::<WaveformLeaf>(&b.wave_key(track))
                .is_some()
        );
    }
}
