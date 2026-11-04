// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn delayed_capture_keeps_admitted_track_clock_and_sample_rate() {
    let (mut r, mut a, token) = armed();
    let id = r.session.tracks[0].id;
    r.session.tracks.swap(0, 1);
    r.session.bpm = 90.0;
    r.session.bars_per_phrase = 2;
    a.availability.sample_rate = 96_000;
    a.finish(token, 48_000, vec![0.25, -0.0]);
    service(&mut r, &mut a);
    let track = r.session.track(id).unwrap();
    assert_eq!(track.layers.len(), 1);
    let phrase = &r.session.phrases[&track.layers[0].phrase_id];
    assert_eq!((phrase.bpm, phrase.length_bars), (120.0, 4));
    let buffer = r.store.get(&phrase.media).unwrap();
    assert_eq!(buffer.sample_rate, 48_000);
    assert_eq!(buffer.samples[1].to_bits(), (-0.0f32).to_bits());
    assert_eq!(a.plays[0].1, 48_000);
    assert_eq!(r.accepted_voice_count(), 1);
}

#[test]
fn same_session_replacement_retires_delayed_capture_and_queued_actions() {
    let (mut r, mut a, token) = armed();
    let session = r.session.clone();
    let history = r.history.clone();
    r.add_track();
    r.replace_project(session, history, InMemoryStore::new(), BTreeSet::new());
    service(&mut r, &mut a);
    assert!(r.is_recording());
    r.toggle_record();
    service(&mut r, &mut a);
    assert_eq!(a.accepted, Some(token));
    let head = r.history.head;
    a.finish(token, 48_000, vec![0.5]);
    service(&mut r, &mut a);
    assert_eq!(r.history.head, head);
    assert!(r.store.is_empty());
    assert_eq!(r.session.tracks.len(), 4);
    assert!(!r.is_recording());
}

#[test]
fn stale_completion_cannot_consume_current_ticket_and_idle_needs_explicit_retirement() {
    let (mut r, mut a, token) = armed();
    a.phase = CapturePhase::Idle;
    service(&mut r, &mut a);
    assert_eq!(r.active_capture_token(), Some(token));
    a.finish(
        CaptureToken {
            generation: token.generation.wrapping_add(1),
            ..token
        },
        48_000,
        vec![0.5],
    );
    service(&mut r, &mut a);
    assert_eq!(r.active_capture_token(), Some(token));
    assert!(r.store.is_empty());
    a.phase = CapturePhase::Idle;
    a.retired = Some(token);
    service(&mut r, &mut a);
    assert!(!r.is_recording());
}

#[test]
fn capture_admission_and_invalid_completion_fail_without_history_or_media_mutation() {
    let mut r = Recorder::default();
    let mut a = FakeAudio {
        fail_arm: true,
        ..Default::default()
    };
    r.arm(0);
    service(&mut r, &mut a);
    let head = r.history.head;
    r.toggle_record();
    service(&mut r, &mut a);
    assert!(!r.is_recording());
    assert_eq!(r.history.head, head);
    a.fail_arm = false;
    r.toggle_record();
    service(&mut r, &mut a);
    let token = r.active_capture_token().unwrap();
    a.finish(token, 44_100, vec![0.5]);
    service(&mut r, &mut a);
    assert_eq!(r.history.head, head);
    assert!(r.store.is_empty());
    r.toggle_record();
    service(&mut r, &mut a);
    let token = r.active_capture_token().unwrap();
    a.finish(token, 48_000, vec![f32::NAN]);
    service(&mut r, &mut a);
    assert_eq!(r.history.head, head);
    assert!(r.store.is_empty());
}

#[test]
fn deleted_capture_target_does_not_receive_or_store_a_delayed_take() {
    let (mut r, mut a, token) = armed();
    r.session.tracks.remove(0);
    let head = r.history.head;
    a.finish(token, 48_000, vec![0.5]);
    service(&mut r, &mut a);
    assert_eq!(r.history.head, head);
    assert!(r.store.is_empty());
    assert!(r.session.phrases.is_empty());
}

#[test]
fn fixed_rearm_and_clock_changes_are_refused_while_free_stop_keeps_its_target() {
    let (mut r, mut a, token) = armed();
    let head = r.history.head;
    r.arm(1);
    r.bpm_nudge(2.0);
    r.toggle_master_clock();
    service(&mut r, &mut a);
    assert_eq!(r.history.head, head);
    assert_eq!(r.active_capture_token(), Some(token));
    a.phase = CapturePhase::Idle;
    a.retired = Some(token);
    service(&mut r, &mut a);
    r.toggle_master_clock();
    r.toggle_record();
    service(&mut r, &mut a);
    let old_id = r.session.tracks[0].id;
    assert_eq!(r.active_capture_track(), Some(old_id));
    r.arm(1);
    service(&mut r, &mut a);
    assert!(r.is_recording());
    assert_eq!(r.active_capture_track(), Some(old_id));
    assert_ne!(r.session.tracks[r.armed_index().unwrap()].id, old_id);
    service(&mut r, &mut a);
    assert_eq!(r.session.track(old_id).unwrap().layers.len(), 1);
    assert!(r.session.tracks[1].armed);
    assert_eq!(r.active_capture_track(), None);
}

#[test]
fn recreated_controller_for_same_session_cannot_reuse_capture_ticket() {
    let (old, _, old_token) = armed();
    let mut r = Recorder::new(
        old.session.clone(),
        old.history.clone(),
        InMemoryStore::new(),
        BTreeSet::new(),
    );
    let mut a = FakeAudio::default();
    r.toggle_record();
    service(&mut r, &mut a);
    let current = r.active_capture_token().unwrap();
    assert_eq!(old_token.session, current.session);
    assert_eq!(old_token.generation, current.generation);
    assert_ne!(old_token.epoch, current.epoch);
    a.finish(old_token, 48_000, vec![0.5]);
    service(&mut r, &mut a);
    assert_eq!(r.active_capture_token(), Some(current));
    assert!(r.store.is_empty());
}
