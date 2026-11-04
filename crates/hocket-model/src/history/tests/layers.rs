// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn append_layer_round_trip() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let track_id = s.tracks[0].id;
    let phrase = Phrase::new(MediaRef([1; 32]), 4, 120.0, 1000);
    let phrase_id = phrase.id;
    let layer = Layer::new(phrase_id);

    let n1 = h.commit(
        Edit::AppendLayer {
            track_id,
            phrase,
            layer,
        },
        &mut s,
        0,
    );

    assert_eq!(s.tracks[0].layers.len(), 1);
    assert_eq!(s.tracks[0].layers[0].phrase_id, phrase_id);
    assert!(s.phrases.contains_key(&phrase_id));

    // Step back: layer popped, phrase stays in pool.
    h.checkout(h.root, &mut s).unwrap();
    assert!(s.tracks[0].layers.is_empty());
    assert!(
        s.phrases.contains_key(&phrase_id),
        "phrase should remain in the pool across undo"
    );

    // Step forward: layer pushed back.
    h.checkout(n1, &mut s).unwrap();
    assert_eq!(s.tracks[0].layers.len(), 1);
    assert_eq!(s.tracks[0].layers[0].phrase_id, phrase_id);
}

#[test]
fn two_appends_then_mute_one() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let track_id = s.tracks[0].id;

    let phrase_a = Phrase::new(MediaRef([1; 32]), 4, 120.0, 1000);
    let layer_a = Layer::new(phrase_a.id);
    h.commit(
        Edit::AppendLayer {
            track_id,
            phrase: phrase_a,
            layer: layer_a,
        },
        &mut s,
        0,
    );

    let phrase_b = Phrase::new(MediaRef([2; 32]), 4, 120.0, 2000);
    let layer_b = Layer::new(phrase_b.id);
    h.commit(
        Edit::AppendLayer {
            track_id,
            phrase: phrase_b,
            layer: layer_b,
        },
        &mut s,
        0,
    );

    assert_eq!(s.tracks[0].layers.len(), 2);

    // Mute layer 0 (the first one).
    h.commit(
        Edit::SetLayerMute {
            track_id,
            layer_index: 0,
            from: false,
            to: true,
        },
        &mut s,
        0,
    );
    assert!(s.tracks[0].layers[0].muted);
    assert!(!s.tracks[0].layers[1].muted);

    // Undo the mute.
    h.checkout(
        h.nodes
            .values()
            .find(|n| {
                matches!(
                    n.edit,
                    Edit::AppendLayer { ref phrase, .. } if phrase.captured_at_ms == 2000
                )
            })
            .unwrap()
            .id,
        &mut s,
    )
    .unwrap();
    assert!(!s.tracks[0].layers[0].muted);
}

#[test]
fn set_layer_gain_round_trip() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let track_id = s.tracks[0].id;

    let phrase = Phrase::new(MediaRef([1; 32]), 4, 120.0, 1000);
    let layer = Layer::new(phrase.id);
    h.commit(
        Edit::AppendLayer {
            track_id,
            phrase,
            layer,
        },
        &mut s,
        0,
    );

    let after_append = h.head;

    h.commit(
        Edit::SetLayerGain {
            track_id,
            layer_index: 0,
            from: 1.0,
            to: 0.5,
        },
        &mut s,
        0,
    );
    assert_eq!(s.tracks[0].layers[0].gain, 0.5);

    h.checkout(after_append, &mut s).unwrap();
    assert_eq!(s.tracks[0].layers[0].gain, 1.0);
}

#[test]
fn set_track_playback_mode_round_trip() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let track_id = s.tracks[0].id;
    assert_eq!(s.tracks[0].playback_mode, PlaybackMode::Sum);

    let after = h.commit(
        Edit::SetTrackPlaybackMode {
            track_id,
            from: PlaybackMode::Sum,
            to: PlaybackMode::SelectOne { active: None },
        },
        &mut s,
        0,
    );
    assert_eq!(
        s.tracks[0].playback_mode,
        PlaybackMode::SelectOne { active: None }
    );

    // Undo: back to Sum.
    h.checkout(h.root, &mut s).unwrap();
    assert_eq!(s.tracks[0].playback_mode, PlaybackMode::Sum);

    // Redo: back to SelectOne.
    h.checkout(after, &mut s).unwrap();
    assert_eq!(
        s.tracks[0].playback_mode,
        PlaybackMode::SelectOne { active: None }
    );
}

#[test]
fn add_track_round_trip() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let track = Track::new_with_mode(
        "track 5",
        TrackColor::from_palette_index(4),
        s.default_playback_mode,
    );
    let id = track.id;

    let added = h.commit(Edit::AddTrack { track }, &mut s, 0);
    assert_eq!(s.tracks.len(), 5);
    assert_eq!(s.tracks.last().map(|t| t.id), Some(id));

    h.checkout(h.root, &mut s).unwrap();
    assert_eq!(s.tracks.len(), 4);
    assert!(s.track(id).is_none());

    h.checkout(added, &mut s).unwrap();
    assert_eq!(s.tracks.len(), 5);
    assert_eq!(s.tracks.last().map(|t| t.id), Some(id));
}

#[test]
fn select_active_layer_round_trip() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let track_id = s.tracks[0].id;

    // Switch the track into SelectOne mode first.
    h.commit(
        Edit::SetTrackPlaybackMode {
            track_id,
            from: PlaybackMode::Sum,
            to: PlaybackMode::SelectOne { active: None },
        },
        &mut s,
        0,
    );
    let before_select = h.head;

    let after_select = h.commit(
        Edit::SelectActiveLayer {
            track_id,
            from: None,
            to: Some(2),
        },
        &mut s,
        0,
    );
    assert_eq!(
        s.tracks[0].playback_mode,
        PlaybackMode::SelectOne { active: Some(2) }
    );

    // Undo: back to None.
    h.checkout(before_select, &mut s).unwrap();
    assert_eq!(
        s.tracks[0].playback_mode,
        PlaybackMode::SelectOne { active: None }
    );

    // Redo: back to Some(2).
    h.checkout(after_select, &mut s).unwrap();
    assert_eq!(
        s.tracks[0].playback_mode,
        PlaybackMode::SelectOne { active: Some(2) }
    );
}

#[test]
fn select_active_layer_on_sum_track_is_noop() {
    // SelectActiveLayer applied to a Sum-mode track does nothing
    // (documented behavior); the track stays in Sum mode.
    let mut s = Session::new_default();
    let mut h = History::new();
    let track_id = s.tracks[0].id;

    h.commit(
        Edit::SelectActiveLayer {
            track_id,
            from: None,
            to: Some(2),
        },
        &mut s,
        0,
    );
    assert_eq!(s.tracks[0].playback_mode, PlaybackMode::Sum);
}

#[test]
fn unknown_target_yields_unknown_node() {
    let mut s = Session::new_default();
    let mut h = History::new();
    h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 90.0,
        },
        &mut s,
        0,
    );
    let bogus = NodeId::new();
    assert_eq!(
        h.checkout(bogus, &mut s),
        Err(HistoryError::UnknownNode(bogus))
    );
}
