// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

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
    assert_eq!(baseline.placement.bounds.width(), size.width);
    assert_eq!(baseline.placement.bounds.height(), 1.0);
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
