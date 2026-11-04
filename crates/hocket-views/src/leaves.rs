// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Real waveform and meter leaves for reusable recorder mounts.
//! Media projection stays in the portable Recorder. Hosts own the registry;
//! this module qualifies keys by mount and reconciles only its own leaves.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use crate::{
    HocketKey, RecorderProjection, WaveformProjection, WaveformSource, recorder_projection,
};
use audio_primitives::WaveformPeak;
use hocket_model::{PhraseId, TrackId};
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
        self.projected_wave_key(&HocketKey::Track(track))
    }
    pub fn layer_wave_key(self, track: TrackId, layer_index: usize, phrase: PhraseId) -> u64 {
        self.projected_layer_wave_key(&HocketKey::Layer {
            track,
            layer_index,
            phrase,
        })
    }
    pub fn projected_wave_key<K: Hash>(self, key: &K) -> u64 {
        TRACK_WAVE_NAMESPACE
            | (hash_ids(&[&self.0.to_le_bytes(), &key_hash(key).to_le_bytes()]) & KEY_PAYLOAD_MASK)
    }
    pub fn projected_layer_wave_key<K: Hash>(self, key: &K) -> u64 {
        LAYER_WAVE_NAMESPACE
            | (hash_ids(&[&self.0.to_le_bytes(), &key_hash(key).to_le_bytes()]) & KEY_PAYLOAD_MASK)
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

pub(crate) fn hash_ids(parts: &[&[u8]]) -> u64 {
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

        // Accepted PCM always has a visible baseline, even when its true
        // amplitude envelope is smaller than one pixel. Do not normalize it.
        cx.fill_rect(0.0, mid, s.width, 1.0, self.color);
        if self
            .peaks
            .iter()
            .all(|peak| peak.min == 0.0 && peak.max == 0.0)
        {
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

// --- reconcile the same leaves from either display authority ------------

/// Each mount owns only its registered keys. The waveform leaf retains its
/// accepted peak envelope, so unchanged lazy projections never run again.
pub struct RecorderLeaves {
    mount: RecorderMount,
    keys: HashSet<u64>,
}

impl RecorderLeaves {
    pub fn new(mount: RecorderMount) -> Self {
        Self {
            mount,
            keys: HashSet::new(),
        }
    }
    pub fn mount(&self) -> RecorderMount {
        self.mount
    }

    pub fn reconcile(&mut self, registry: &mut LeafRegistry<u64>, state: &Recorder) {
        self.reconcile_projection(registry, &recorder_projection(state));
    }

    pub fn reconcile_projection<K: Hash>(
        &mut self,
        registry: &mut LeafRegistry<u64>,
        projection: &RecorderProjection<'_, K>,
    ) {
        let mut active = HashSet::new();
        for lane in &projection.lanes {
            let color = ColorF {
                r: f32::from(lane.color[0]) / 255.0,
                g: f32::from(lane.color[1]) / 255.0,
                b: f32::from(lane.color[2]) / 255.0,
                a: 1.0,
            };
            let key = self.mount.projected_wave_key(&lane.key);
            if reconcile_wave(
                registry,
                key,
                &lane.waveform,
                color,
                TRACK_COLUMNS,
                Size {
                    width: 280.0,
                    height: 40.0,
                },
            ) {
                active.insert(key);
            }
            for layer in &lane.layers {
                let key = self.mount.projected_layer_wave_key(&layer.key);
                if reconcile_wave(
                    registry,
                    key,
                    &layer.waveform,
                    color,
                    LAYER_COLUMNS,
                    Size {
                        width: 180.0,
                        height: 11.0,
                    },
                ) {
                    active.insert(key);
                }
            }
        }
        if let Some(meters) = &projection.meters {
            for (channel, reading) in meters.iter().enumerate() {
                let key = self.mount.meter_key(channel as u8);
                ensure_meter(registry, key, reading.level, reading.peak);
                active.insert(key);
            }
        }
        registry.retain(|key| !self.keys.contains(key) || active.contains(key));
        self.keys = active;
    }

    pub fn unmount(&mut self, registry: &mut LeafRegistry<u64>) {
        registry.retain(|key| !self.keys.contains(key));
        self.keys.clear();
    }
}

fn reconcile_wave(
    registry: &mut LeafRegistry<u64>,
    key: u64,
    waveform: &WaveformProjection<'_>,
    color: ColorF,
    columns: usize,
    intrinsic: Size,
) -> bool {
    let WaveformProjection::Data { revision, source } = waveform else {
        return false;
    };
    if matches!(source, WaveformSource::Pcm(samples) if samples.is_empty())
        || matches!(source, WaveformSource::Peaks(peaks) if peaks.is_empty())
    {
        return false;
    }
    let sig = hash_ids(&[
        &revision.to_le_bytes(),
        &color.r.to_bits().to_le_bytes(),
        &color.g.to_bits().to_le_bytes(),
        &color.b.to_bits().to_le_bytes(),
    ]);
    if registry
        .get_mut_as::<WaveformLeaf>(&key)
        .is_some_and(|leaf| leaf.sig == sig)
    {
        return true;
    }
    let peaks = match source {
        WaveformSource::Pcm(samples) if !samples.is_empty() => {
            audio_primitives::min_max_peaks(samples, columns)
        },
        WaveformSource::Peaks(peaks) if !peaks.is_empty() => peaks.to_vec(),
        WaveformSource::Lazy(project) => match project(columns) {
            Some(peaks) if !peaks.is_empty() => peaks,
            _ => return false,
        },
        _ => return false,
    };
    ensure_waveform(registry, key, peaks, color, sig, intrinsic);
    true
}

fn key_hash<K: Hash>(key: &K) -> u64 {
    struct KeyHasher(u64);
    impl Hasher for KeyHasher {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        fn write_usize(&mut self, value: usize) {
            self.write(&(value as u64).to_le_bytes());
        }
    }
    let mut hasher = KeyHasher(0xcbf2_9ce4_8422_2325);
    key.hash(&mut hasher);
    hasher.finish()
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
mod tests;
