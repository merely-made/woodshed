// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Feature Target 3a validation: capture pipeline end-to-end
//! (updated for the layered model).
//!
//! Synthesized input samples → Capture state machine → MediaStore →
//! Phrase + Layer → `Edit::AppendLayer` commit →
//! `Session.tracks[i].layers` grows by one + the buffer is
//! retrievable from the store by its ref.
//!
//! No cpal, no real audio device. FT3b-prime brings Firewheel's
//! cpal-backed input + the audible playback path.

use std::f32::consts::TAU;

use hocket_engine::capture::{Capture, CaptureState};
use hocket_engine::media::{InMemoryStore, MediaStore};
use hocket_model::{Edit, History, Layer, Phrase, Session};

const SAMPLE_RATE: u32 = 48_000;

/// Synthesize `samples` frames of a sine at `freq_hz` at SAMPLE_RATE.
fn synth_sine(samples: usize, freq_hz: f32) -> Vec<f32> {
    let dt = 1.0 / SAMPLE_RATE as f32;
    (0..samples)
        .map(|i| {
            let t = i as f32 * dt;
            (t * freq_hz * TAU).sin() * 0.5
        })
        .collect()
}

#[test]
fn end_to_end_capture_one_phrase_appends_layer() {
    let mut session = Session::new_default();
    let mut history = History::new();
    let store = std::cell::RefCell::new(InMemoryStore::new());

    // 0.1 second of synthetic audio at 48kHz.
    let samples_per_phrase = (SAMPLE_RATE as f32 * 0.1) as usize;

    let mut capture = Capture::new(samples_per_phrase);
    capture.arm();
    let input = synth_sine(samples_per_phrase, 440.0);
    capture.feed_slice(&input);

    assert_eq!(capture.state(), &CaptureState::Complete);
    let captured = capture.take_completed().expect("buffer should be ready");
    assert_eq!(captured.len(), samples_per_phrase);
    assert_eq!(captured, input);

    let media_ref = store.borrow_mut().put(&captured, SAMPLE_RATE);

    // Build a Phrase + Layer and commit AppendLayer.
    let track_id = session.tracks[0].id;
    let phrase = Phrase::new(media_ref, session.bars_per_phrase, session.bpm, 0);
    let phrase_id = phrase.id;
    let layer = Layer::new(phrase_id);

    history.commit(
        Edit::AppendLayer {
            track_id,
            phrase,
            layer,
        },
        &mut session,
        0,
    );

    // Track 0 has exactly one layer now.
    assert_eq!(session.tracks[0].layers.len(), 1);
    assert_eq!(session.tracks[0].layers[0].phrase_id, phrase_id);
    // Phrase is in the pool.
    assert!(session.phrases.contains_key(&phrase_id));
    assert_eq!(session.phrases[&phrase_id].media, media_ref);
    // Buffer retrievable from media store.
    let stored = store.borrow();
    let buf = stored.get(&media_ref).expect("buffer should be in store");
    assert_eq!(buf.samples, input);
    assert_eq!(buf.sample_rate, SAMPLE_RATE);
}

/// Two captures, two layers on the same track — the overdub model.
/// Both phrases live in the pool, both layers in the track's stack.
#[test]
fn two_captures_on_one_track_stack_layers() {
    let mut session = Session::new_default();
    let mut history = History::new();
    let mut store = InMemoryStore::new();
    let samples_per_phrase = (SAMPLE_RATE as f32 * 0.05) as usize;
    let track_id = session.tracks[0].id;

    // Capture phrase A.
    let buf_a = capture_one(samples_per_phrase, synth_sine(samples_per_phrase, 440.0));
    let ref_a = store.put(&buf_a, SAMPLE_RATE);
    let phrase_a = Phrase::new(ref_a, session.bars_per_phrase, session.bpm, 0);
    let id_a = phrase_a.id;
    let layer_a = Layer::new(id_a);
    history.commit(
        Edit::AppendLayer {
            track_id,
            phrase: phrase_a,
            layer: layer_a,
        },
        &mut session,
        0,
    );

    // Capture phrase B (different freq).
    let buf_b = capture_one(samples_per_phrase, synth_sine(samples_per_phrase, 660.0));
    let ref_b = store.put(&buf_b, SAMPLE_RATE);
    assert_ne!(ref_a, ref_b);
    let phrase_b = Phrase::new(ref_b, session.bars_per_phrase, session.bpm, 1);
    let id_b = phrase_b.id;
    let layer_b = Layer::new(id_b);
    history.commit(
        Edit::AppendLayer {
            track_id,
            phrase: phrase_b,
            layer: layer_b,
        },
        &mut session,
        1,
    );

    // Track 0 has both layers, in order.
    assert_eq!(session.tracks[0].layers.len(), 2);
    assert_eq!(session.tracks[0].layers[0].phrase_id, id_a);
    assert_eq!(session.tracks[0].layers[1].phrase_id, id_b);
    assert_eq!(session.phrases.len(), 2);
    // Both buffers retrievable.
    assert_eq!(store.get(&ref_a).unwrap().samples, buf_a);
    assert_eq!(store.get(&ref_b).unwrap().samples, buf_b);
}

/// Mute is the v0 "remove from playback." Undo restores the prior
/// mute state; the layer itself is never popped by mute operations.
#[test]
fn mute_layer_is_undoable() {
    let mut session = Session::new_default();
    let mut history = History::new();
    let mut store = InMemoryStore::new();
    let samples_per_phrase = (SAMPLE_RATE as f32 * 0.05) as usize;
    let track_id = session.tracks[0].id;

    // One layer.
    let buf = capture_one(samples_per_phrase, synth_sine(samples_per_phrase, 440.0));
    let media = store.put(&buf, SAMPLE_RATE);
    let phrase = Phrase::new(media, session.bars_per_phrase, session.bpm, 0);
    let layer = Layer::new(phrase.id);
    let after_append = history.commit(
        Edit::AppendLayer {
            track_id,
            phrase,
            layer,
        },
        &mut session,
        0,
    );
    assert!(!session.tracks[0].layers[0].muted);

    // Mute it.
    history.commit(
        Edit::SetLayerMute {
            track_id,
            layer_index: 0,
            from: false,
            to: true,
        },
        &mut session,
        1,
    );
    assert!(session.tracks[0].layers[0].muted);

    // Undo: back to unmuted; layer still present.
    history.checkout(after_append, &mut session).unwrap();
    assert!(!session.tracks[0].layers[0].muted);
    assert_eq!(session.tracks[0].layers.len(), 1);
}

/// Helper: drive a Capture to completion with the given samples.
fn capture_one(samples_per_phrase: usize, input: Vec<f32>) -> Vec<f32> {
    let mut c = Capture::new(samples_per_phrase);
    c.arm();
    c.feed_slice(&input);
    c.take_completed().expect("capture should complete")
}
