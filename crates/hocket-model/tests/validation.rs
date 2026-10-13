// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Feature Target 2 validation tests (updated for layered model).
//!
//! Exercises the plan's "done conditions" for the model crate:
//! 100-commit history scrub timing, and the divergent-shape merge
//! test that demonstrates CRDT-readiness without requiring a full
//! merge implementation.

use std::time::Instant;

use hocket_model::{
    Edit, History, Layer, MediaRef, NodeId, Phrase, Session,
};

/// FT2 validation: scrubbing across 100 commits completes in <16ms.
///
/// Builds a 100-commit history (alternating BPM / layer / track-state
/// edits) then performs 100 round-trip scrubs (head → root → head)
/// and asserts the total elapsed time stays under one 60 fps frame
/// budget.
#[test]
fn one_hundred_commit_scrub_under_sixteen_ms() {
    let mut session = Session::new_default();
    let mut history = History::new();

    let track_ids: Vec<_> = session.tracks.iter().map(|t| t.id).collect();
    let mut nodes: Vec<NodeId> = vec![history.root];
    let mut current_bpm = session.bpm;

    for i in 0..100 {
        let edit = match i % 4 {
            0 => {
                let to = current_bpm - 0.5;
                let e = Edit::SetBpm { from: current_bpm, to };
                current_bpm = to;
                e
            }
            1 => {
                let track_id = track_ids[i % track_ids.len()];
                Edit::MuteTrack {
                    track_id,
                    from: false,
                    to: true,
                }
            }
            2 => {
                let track_id = track_ids[i % track_ids.len()];
                let phrase = Phrase::new(MediaRef([i as u8; 32]), 4, current_bpm, i as u64);
                let layer = Layer::new(phrase.id);
                Edit::AppendLayer {
                    track_id,
                    phrase,
                    layer,
                }
            }
            3 => {
                let track_id = track_ids[i % track_ids.len()];
                Edit::ArmTrack {
                    track_id,
                    from: false,
                    to: true,
                }
            }
            _ => unreachable!(),
        };
        let id = history.commit(edit, &mut session, i as u64);
        nodes.push(id);
    }

    let head = *nodes.last().unwrap();
    let root = history.root;

    // Time 100 round-trip scrubs: head → root → head, fifty times.
    let start = Instant::now();
    for _ in 0..50 {
        history.checkout(root, &mut session).unwrap();
        history.checkout(head, &mut session).unwrap();
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 16,
        "100-commit scrub took {elapsed:?}, target <16ms (one 60fps frame)"
    );
}

/// FT2 validation: divergent histories from a common ancestor can be
/// unioned deterministically — the node-set union is order-independent
/// because every NodeId is globally unique (UUID v4).
///
/// This does NOT implement merge semantics. It demonstrates that the
/// data shape supports CRDT-style merge: no id collisions between
/// branches, parent pointers preserved, encoding stays deterministic.
#[test]
fn divergent_histories_union_deterministically() {
    // Start with a shared baseline.
    let mut shared_session = Session::new_default();
    let mut shared_history = History::new();
    let n_shared = shared_history.commit(
        Edit::SetBpm {
            from: shared_session.bpm,
            to: 100.0,
        },
        &mut shared_session,
        100,
    );

    // Peer A diverges.
    let mut session_a = shared_session.clone();
    let mut history_a = shared_history.clone();
    let track_a = session_a.tracks[0].id;
    let n_a = history_a.commit(
        Edit::RenameTrack {
            track_id: track_a,
            from: session_a.tracks[0].name.clone(),
            to: "drums".to_string(),
        },
        &mut session_a,
        200,
    );

    // Peer B diverges differently.
    let mut session_b = shared_session.clone();
    let mut history_b = shared_history.clone();
    let track_b = session_b.tracks[1].id;
    let n_b = history_b.commit(
        Edit::MuteTrack {
            track_id: track_b,
            from: false,
            to: true,
        },
        &mut session_b,
        300,
    );

    // Property 1: NodeId sets share the common ancestors and root,
    // but the divergent leaves don't collide.
    assert!(history_a.nodes.contains_key(&n_shared));
    assert!(history_b.nodes.contains_key(&n_shared));
    assert!(history_a.nodes.contains_key(&n_a));
    assert!(history_b.nodes.contains_key(&n_b));
    assert!(!history_a.nodes.contains_key(&n_b));
    assert!(!history_b.nodes.contains_key(&n_a));

    // Property 2: union of the two node maps is order-independent.
    let union_a_then_b: std::collections::BTreeMap<_, _> = history_a
        .nodes
        .iter()
        .chain(history_b.nodes.iter())
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    let union_b_then_a: std::collections::BTreeMap<_, _> = history_b
        .nodes
        .iter()
        .chain(history_a.nodes.iter())
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    assert_eq!(union_a_then_b, union_b_then_a);

    // Property 3: parent pointers in each branch trace back to the
    // shared ancestor.
    let a_chain = ancestor_chain(&history_a, n_a);
    let b_chain = ancestor_chain(&history_b, n_b);
    assert!(a_chain.contains(&n_shared));
    assert!(b_chain.contains(&n_shared));
    assert!(a_chain.contains(&history_a.root));
    assert!(b_chain.contains(&history_b.root));
    assert_eq!(history_a.root, history_b.root);
}

fn ancestor_chain(h: &History, start: NodeId) -> Vec<NodeId> {
    let mut chain = Vec::new();
    let mut cur = Some(start);
    while let Some(id) = cur {
        chain.push(id);
        cur = h.nodes.get(&id).and_then(|n| n.parent);
    }
    chain
}
