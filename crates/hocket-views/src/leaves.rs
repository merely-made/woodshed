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

    #[test]
    fn quiet_pcm_keeps_visible_silence_baseline_and_exact_subpixel_envelope() {
        use paint_list_api::PathCommand;

        let size = Size {
            width: 80.0,
            height: 40.0,
        };
        let paint = |samples: &[f32]| {
            let peaks = audio_primitives::min_max_peaks(samples, 4);
            let mut leaf = WaveformLeaf::new(peaks, color(), 1, size);
            let mut commands = Vec::new();
            leaf.paint(&mut PaintCx::new(&mut commands, size));
            commands
        };
        let silence = paint(&[0.0; 4]);
        let [PaintCmd::DrawRect(silent_baseline)] = silence.as_slice() else {
            panic!("accepted silence must remain a single visible baseline");
        };

        // The hydrated public capture fixture peaks at approximately this
        // amplitude: its 40px leaf envelope is only about 0.015px high.
        let peak = 0.000_400_066;
        let quiet = paint(&[0.0, peak, -peak, 0.0]);
        let [PaintCmd::DrawRect(baseline), PaintCmd::DrawPath(envelope)] = quiet.as_slice() else {
            panic!("quiet PCM must paint a baseline beneath its real envelope");
        };
        assert_eq!(baseline.placement, silent_baseline.placement);
        assert_eq!(baseline.placement.bounds.size.width, size.width);
        assert_eq!(baseline.placement.bounds.size.height, 1.0);
        let ys: Vec<_> = envelope
            .path
            .commands
            .iter()
            .filter_map(|command| match command {
                PathCommand::MoveTo(point) | PathCommand::LineTo(point) => Some(point.y),
                _ => None,
            })
            .collect();
        let min = ys.iter().copied().fold(f32::INFINITY, f32::min);
        let max = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mid = size.height * 0.5;
        let amplitude = size.height * 0.5 - 1.0;
        assert!((min - (mid - peak * amplitude)).abs() < 1e-6);
        assert!((max - (mid + peak * amplitude)).abs() < 1e-6);
        assert!(max > min && max - min < 0.02);
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

    fn projected_pcm<'a>(samples: &'a [f32], revision: u64) -> RecorderProjection<'a, (u64, u64)> {
        use crate::{LaneControls, LaneProjection, RecordingState, TransportProjection};
        RecorderProjection {
            title: "Set backing".into(),
            hint: "".into(),
            meters: None,
            transport: TransportProjection::new((7, 0)),
            lanes: vec![LaneProjection {
                key: (7, 9),
                name: "authored card".into(),
                owner: "Set 7".into(),
                clock: "83.75 bpm · 7/8 · 3 bars".into(),
                color: [80, 180, 140],
                selected: true,
                recording: RecordingState::Idle,
                waveform: WaveformProjection::Data {
                    revision,
                    source: WaveformSource::Pcm(samples),
                },
                layers: Vec::new(),
                controls: LaneControls::default(),
            }],
        }
    }

    #[test]
    fn host_owned_pcm_uses_same_retained_leaf_without_fabricated_meters() {
        let mut registry = LeafRegistry::new();
        let mount = RecorderMount::new(71);
        let mut leaves = RecorderLeaves::new(mount);
        leaves.reconcile_projection(&mut registry, &projected_pcm(&[-0.5, 0.75], 1));
        let key = mount.projected_wave_key(&(7u64, 9u64));
        let leaf = registry.get_mut_as::<WaveformLeaf>(&key).unwrap();
        assert!(leaf.peaks.iter().any(|peak| peak.min == -0.5));
        assert!(leaf.peaks.iter().any(|peak| peak.max == 0.75));
        assert!(
            registry
                .get_mut_as::<sprigging::Meter>(&mount.meter_key(0))
                .is_none()
        );
        leaves.reconcile_projection(&mut registry, &projected_pcm(&[-0.25, 0.5], 2));
        let leaf = registry.get_mut_as::<WaveformLeaf>(&key).unwrap();
        assert!(leaf.peaks.iter().any(|peak| peak.max == 0.5));
        leaves.reconcile_projection(&mut registry, &projected_pcm(&[], 2));
        assert!(registry.get_mut_as::<WaveformLeaf>(&key).is_none());
    }

    #[test]
    fn lazy_host_projection_is_evaluated_only_when_material_revision_changes() {
        let calls = std::cell::Cell::new(0);
        let mut registry = LeafRegistry::new();
        let mut leaves = RecorderLeaves::new(RecorderMount::new(72));
        for revision in [1, 1, 2] {
            let mut projection = projected_pcm(&[0.0], revision);
            projection.lanes[0].waveform = WaveformProjection::Data {
                revision,
                source: WaveformSource::Lazy(Box::new(|_| {
                    calls.set(calls.get() + 1);
                    Some(vec![WaveformPeak {
                        min: -0.25,
                        max: 0.5,
                    }])
                })),
            };
            leaves.reconcile_projection(&mut registry, &projection);
        }
        assert_eq!(calls.get(), 2);
    }
}
