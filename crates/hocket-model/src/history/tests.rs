// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;
use crate::ids::MediaRef;

fn malformed_histories() -> Vec<(&'static str, History)> {
    let mut session = Session::new_default();
    let mut base = History::new();
    let first = base.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 90.0,
        },
        &mut session,
        1,
    );
    let second = base.commit(
        Edit::SetBpm {
            from: 90.0,
            to: 80.0,
        },
        &mut session,
        2,
    );
    let mut cases = Vec::new();
    for case in [
        "self cycle",
        "two node cycle",
        "missing root",
        "missing head",
        "root parent",
        "root edit",
        "extra root",
        "key mismatch",
        "missing parent",
        "non-root genesis",
        "disconnected cycle",
    ] {
        let mut h = base.clone();
        match case {
            "self cycle" => h.nodes.get_mut(&second).unwrap().parent = Some(second),
            "two node cycle" => h.nodes.get_mut(&first).unwrap().parent = Some(second),
            "missing root" => h.root = NodeId::new(),
            "missing head" => h.head = NodeId::new(),
            "root parent" => h.nodes.get_mut(&h.root).unwrap().parent = Some(second),
            "root edit" => {
                h.nodes.get_mut(&h.root).unwrap().edit = Edit::SetBpm {
                    from: 120.0,
                    to: 1.0,
                }
            },
            "extra root" => h.nodes.get_mut(&first).unwrap().parent = None,
            "key mismatch" => h.nodes.get_mut(&first).unwrap().id = NodeId::new(),
            "missing parent" => h.nodes.get_mut(&first).unwrap().parent = Some(NodeId::new()),
            "non-root genesis" => h.nodes.get_mut(&first).unwrap().edit = Edit::Genesis,
            "disconnected cycle" => {
                let id = NodeId::new();
                h.nodes.insert(
                    id,
                    HistoryNode {
                        id,
                        parent: Some(id),
                        edit: Edit::SetBpm {
                            from: 120.0,
                            to: 1.0,
                        },
                        timestamp_ms: 3,
                    },
                );
            },
            _ => unreachable!(),
        }
        cases.push((case, h));
    }
    cases
}

#[test]
fn malformed_topology_is_refused_before_checkout_or_integration_mutation() {
    for (case, mut malformed) in malformed_histories() {
        assert!(malformed.validate().is_err(), "{case}");
        let bundle = crate::ProjectBundle::new(Session::new_default(), malformed.clone());
        let mut bytes = Vec::new();
        ciborium::into_writer(&bundle, &mut bytes).unwrap();
        assert!(
            matches!(
                crate::ProjectBundle::from_bytes(&bytes),
                Err(crate::PersistenceError::History(_))
            ),
            "{case}"
        );
        let before = malformed.clone();
        let mut session = Session::new_default();
        let before_session = session.clone();
        if case == "disconnected cycle" {
            // Unrelated corruption is refused by admission/integration;
            // checkout validates only the paths it will traverse.
            malformed.checkout(malformed.head, &mut session).unwrap();
            let cyclic = malformed
                .nodes
                .values()
                .find(|node| node.parent == Some(node.id))
                .unwrap()
                .id;
            assert!(malformed.checkout(cyclic, &mut session).is_err());
            malformed.head = cyclic;
            assert!(malformed.checkout(cyclic, &mut session).is_err());
            malformed.head = before.head;
        } else {
            assert!(
                malformed.checkout(malformed.head, &mut session).is_err(),
                "{case}"
            );
            assert!(
                malformed.checkout(malformed.root, &mut session).is_err(),
                "{case}"
            );
        }
        assert_eq!(session, before_session, "{case}");
        assert_eq!(malformed, before, "{case}");
        // Same root exercises admission rather than DifferentRoot refusal.
        let mut local = History {
            root: malformed.root,
            head: malformed.root,
            nodes: BTreeMap::from([(
                malformed.root,
                HistoryNode {
                    id: malformed.root,
                    parent: None,
                    edit: Edit::Genesis,
                    timestamp_ms: 0,
                },
            )]),
        };
        let local_before = local.clone();
        assert!(local.integrate(&malformed).is_err(), "{case}");
        assert_eq!(local, local_before, "{case}");
        assert!(malformed.integrate(&local).is_err(), "{case}");
        assert_eq!(malformed, before, "{case}");
    }
}

#[test]
fn valid_deep_history_and_retained_branches_are_rooted() {
    let mut h = History::new();
    let mut s = Session::new_default();
    for i in 0..2048 {
        h.commit(
            Edit::SetBpm {
                from: s.bpm,
                to: 90.0,
            },
            &mut s,
            i,
        );
    }
    let retained = h.head;
    h.checkout(h.root, &mut s).unwrap();
    h.commit(
        Edit::SetBpm {
            from: s.bpm,
            to: 80.0,
        },
        &mut s,
        2049,
    );
    h.validate().unwrap();
    h.checkout(retained, &mut s).unwrap();
    assert_eq!(s.bpm, 90.0);
    assert_eq!(h.nodes.len(), 2050);
}

#[test]
fn new_history_has_only_root() {
    let h = History::new();
    assert_eq!(h.head, h.root);
    assert_eq!(h.nodes.len(), 1);
    assert!(matches!(h.nodes[&h.root].edit, Edit::Genesis));
    assert_eq!(h.nodes[&h.root].parent, None);
}

#[test]
fn commit_advances_head_and_applies() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let n1 = h.commit(
        Edit::SetBpm {
            from: s.bpm,
            to: 90.0,
        },
        &mut s,
        0,
    );
    assert_eq!(h.head, n1);
    assert_eq!(s.bpm, 90.0);
    assert_eq!(h.nodes.len(), 2);
}

#[test]
fn undo_redo_round_trips() {
    let mut s = Session::new_default();
    let mut h = History::new();
    assert!(!h.can_undo() && !h.can_redo());

    h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 90.0,
        },
        &mut s,
        0,
    );
    assert_eq!(s.bpm, 90.0);
    assert!(h.can_undo() && !h.can_redo());

    assert!(h.undo(&mut s));
    assert_eq!(s.bpm, 120.0);
    assert!(!h.can_undo() && h.can_redo());

    assert!(h.redo(&mut s));
    assert_eq!(s.bpm, 90.0);

    // Undo back to root, then a further undo is a no-op.
    assert!(h.undo(&mut s));
    assert_eq!(s.bpm, 120.0);
    assert!(!h.undo(&mut s)); // already at root
    assert!(h.can_redo()); // but the edit is still redoable
}

#[test]
fn checkout_back_inverts() {
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
    assert_eq!(s.bpm, 90.0);
    h.checkout(h.root, &mut s).unwrap();
    assert_eq!(s.bpm, 120.0);
    assert_eq!(h.head, h.root);
}

#[test]
fn checkout_forward_applies() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let n1 = h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 90.0,
        },
        &mut s,
        0,
    );
    h.checkout(h.root, &mut s).unwrap();
    assert_eq!(s.bpm, 120.0);
    h.checkout(n1, &mut s).unwrap();
    assert_eq!(s.bpm, 90.0);
}

#[test]
fn commit_after_checkout_back_retains_future_branch() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let _n1 = h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 90.0,
        },
        &mut s,
        0,
    );
    let n2 = h.commit(
        Edit::SetBpm {
            from: 90.0,
            to: 60.0,
        },
        &mut s,
        0,
    );
    assert_eq!(h.nodes.len(), 3);

    h.checkout(h.root, &mut s).unwrap();
    let _n3 = h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 100.0,
        },
        &mut s,
        0,
    );

    assert!(h.nodes.contains_key(&n2));
    assert_eq!(s.bpm, 100.0);
}

#[test]
fn checkout_moves_between_retained_sibling_branches() {
    let mut s = Session::new_default();
    let mut h = History::new();
    let root = h.root;
    let first = h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 90.0,
        },
        &mut s,
        1,
    );
    h.checkout(root, &mut s).unwrap();
    let second = h.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 100.0,
        },
        &mut s,
        2,
    );

    h.checkout(first, &mut s).unwrap();
    assert_eq!(s.bpm, 90.0);
    h.checkout(second, &mut s).unwrap();
    assert_eq!(s.bpm, 100.0);
    assert_eq!(h.nodes.len(), 3);
}

#[test]
fn integrate_retains_remote_branch_without_moving_the_local_head() {
    let mut shared_session = Session::new_default();
    let mut local = History::new();
    let shared = local.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 110.0,
        },
        &mut shared_session,
        1,
    );
    let mut remote_session = shared_session.clone();
    let mut remote = local.clone();
    let local_head = local.commit(
        Edit::SetBpm {
            from: 110.0,
            to: 90.0,
        },
        &mut shared_session,
        2,
    );
    let remote_head = remote.commit(
        Edit::SetBpm {
            from: 110.0,
            to: 100.0,
        },
        &mut remote_session,
        3,
    );

    assert_eq!(local.integrate(&remote).unwrap(), 1);
    assert_eq!(local.head, local_head);
    assert!(local.nodes.contains_key(&shared));
    assert!(local.nodes.contains_key(&remote_head));
    local.checkout(remote_head, &mut shared_session).unwrap();
    assert_eq!(shared_session.bpm, 100.0);
}

#[test]
fn integrate_accepts_fragments_using_local_ancestors_without_moving_head() {
    let mut session = Session::new_default();
    let mut local = History::new();
    local.commit(
        Edit::SetBpm {
            from: 120.0,
            to: 110.0,
        },
        &mut session,
        1,
    );
    let local_head = local.head;
    let mut remote = local.clone();
    let mut remote_session = session.clone();
    let first = remote.commit(
        Edit::SetBpm {
            from: 110.0,
            to: 90.0,
        },
        &mut remote_session,
        2,
    );
    let second = remote.commit(
        Edit::SetBpm {
            from: 90.0,
            to: 80.0,
        },
        &mut remote_session,
        3,
    );
    remote.nodes.retain(|id, _| *id == first || *id == second);
    assert!(remote.validate().is_err());
    // A graph fragment is not a valid durable snapshot.
    assert!(
        crate::ProjectBundle::new(remote_session, remote.clone())
            .to_bytes()
            .is_err()
    );
    assert_eq!(local.integrate(&remote).unwrap(), 2);
    assert_eq!(local.head, local_head);
    assert_eq!(session.bpm, 110.0);
    local.validate().unwrap();
    assert_eq!(local.integrate(&remote).unwrap(), 0);
    local.checkout(second, &mut session).unwrap();
    assert_eq!(session.bpm, 80.0);
    local.checkout(local_head, &mut session).unwrap();
    assert_eq!(session.bpm, 110.0);
}

#[test]
fn invalid_fragment_union_is_atomic_even_with_a_valid_new_node() {
    for case in [
        "self cycle",
        "two node cycle",
        "missing parent",
        "extra root",
        "key mismatch",
        "missing head",
        "conflict",
    ] {
        let mut local = History::new();
        let before = local.clone();
        let good = NodeId::new();
        let bad = NodeId::new();
        let mut fragment = History {
            root: local.root,
            head: good,
            nodes: BTreeMap::new(),
        };
        fragment.nodes.insert(
            good,
            HistoryNode {
                id: good,
                parent: Some(local.root),
                edit: Edit::SetBpm {
                    from: 120.0,
                    to: 90.0,
                },
                timestamp_ms: 1,
            },
        );
        fragment.nodes.insert(
            bad,
            HistoryNode {
                id: bad,
                parent: Some(good),
                edit: Edit::SetBpm {
                    from: 90.0,
                    to: 80.0,
                },
                timestamp_ms: 2,
            },
        );
        match case {
            "self cycle" => fragment.nodes.get_mut(&bad).unwrap().parent = Some(bad),
            "two node cycle" => fragment.nodes.get_mut(&good).unwrap().parent = Some(bad),
            "missing parent" => fragment.nodes.get_mut(&bad).unwrap().parent = Some(NodeId::new()),
            "extra root" => fragment.nodes.get_mut(&bad).unwrap().parent = None,
            "key mismatch" => fragment.nodes.get_mut(&bad).unwrap().id = NodeId::new(),
            "missing head" => fragment.head = NodeId::new(),
            "conflict" => {
                let mut root = local.nodes[&local.root].clone();
                root.timestamp_ms = 99;
                fragment.nodes.insert(local.root, root);
            },
            _ => unreachable!(),
        }
        assert!(local.integrate(&fragment).is_err(), "{case}");
        assert_eq!(local, before, "{case}");
    }
}

#[test]
fn integrate_rejects_an_incompatible_graph() {
    let mut local = History::new();
    let other_root = History::new();
    assert!(matches!(
        local.integrate(&other_root),
        Err(HistoryError::DifferentRoot { .. })
    ));

    let mut conflicting = local.clone();
    conflicting
        .nodes
        .get_mut(&conflicting.root)
        .unwrap()
        .timestamp_ms = 1;
    assert!(matches!(
        local.integrate(&conflicting),
        Err(HistoryError::ConflictingNode(_))
    ));

    let mut missing_parent = local.clone();
    let orphan = NodeId::new();
    let absent_parent = NodeId::new();
    missing_parent.nodes.insert(
        orphan,
        HistoryNode {
            id: orphan,
            parent: Some(absent_parent),
            edit: Edit::Genesis,
            timestamp_ms: 0,
        },
    );
    assert!(matches!(
        local.integrate(&missing_parent),
        Err(HistoryError::MissingParent {
            node,
            parent
        }) if node == orphan && parent == absent_parent
    ));
}

#[path = "tests/layers.rs"]
mod layers;
