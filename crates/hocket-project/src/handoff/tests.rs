// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use hocket_model::{Edit, History, Layer, Phrase, Session};
use personae::{IdentityProvider, InMemoryProvider};

use super::*;

fn bundle_with_media(store: &mut InMemoryStore) -> ProjectBundle {
    let mut session = Session::new_default();
    let mut history = History::new();
    let reference = store.put(&[0.25, -0.5, 0.75], 48_000);
    let phrase = Phrase::new(reference, session.bars_per_phrase, session.bpm, 1);
    let layer = Layer::new(phrase.id);
    history.commit(
        Edit::AppendLayer {
            track_id: session.tracks[0].id,
            phrase,
            layer,
        },
        &mut session,
        1,
    );
    ProjectBundle::new(session, history)
}

fn corrupt_history(bundle: &mut ProjectBundle, case: &str) {
    let h = &mut bundle.history;
    let head = h.head;
    match case {
        "self cycle" => h.nodes.get_mut(&head).unwrap().parent = Some(head),
        "two node cycle" => {
            let id = hocket_model::NodeId::new();
            h.nodes.insert(
                id,
                hocket_model::HistoryNode {
                    id,
                    parent: Some(head),
                    edit: Edit::SetBpm {
                        from: 120.0,
                        to: 90.0,
                    },
                    timestamp_ms: 4,
                },
            );
            h.nodes.get_mut(&head).unwrap().parent = Some(id);
        },
        "missing root" => h.root = hocket_model::NodeId::new(),
        "missing head" => h.head = hocket_model::NodeId::new(),
        "invalid root" => h.nodes.get_mut(&h.root).unwrap().parent = Some(head),
        "disconnected" => h.nodes.get_mut(&head).unwrap().parent = None,
        "key mismatch" => h.nodes.get_mut(&head).unwrap().id = hocket_model::NodeId::new(),
        _ => unreachable!(),
    }
}

#[test]
fn malformed_graphs_are_refused_by_create_receive_and_accept_without_mutation() {
    for case in [
        "self cycle",
        "two node cycle",
        "missing root",
        "missing head",
        "invalid root",
        "disconnected",
        "key mismatch",
    ] {
        let sender = InMemoryProvider::from_seed([1; 32]);
        let recipient = InMemoryProvider::from_seed([2; 32]);
        let (mut local, mut local_media, incoming, incoming_media) = divergent_captures();
        let mut envelope = HandoffEnvelope::create(
            &incoming,
            &incoming_media,
            recipient.master_public_key(),
            &sender,
        )
        .unwrap();
        corrupt_history(&mut envelope.payload.bundle, case);
        assert!(
            matches!(
                HandoffEnvelope::create(
                    &envelope.payload.bundle,
                    &incoming_media,
                    recipient.master_public_key(),
                    &sender
                ),
                Err(HandoffError::Manifest(_))
            ),
            "{case}"
        );
        // Sign the malformed payload as an authenticated hostile peer.
        // Admission must fail for topology, after signature verification.
        let unsigned = UnsignedHandoff {
            format_version: envelope.format_version,
            session_id: envelope.session_id,
            sender: &envelope.sender,
            recipient: envelope.recipient,
            payload: &envelope.payload,
        };
        envelope.signature = sender
            .derive_keypair(&handoff_salt(envelope.session_id))
            .unwrap()
            .sign(&cbor_bytes(&unsigned).unwrap())
            .to_bytes()
            .to_vec();
        let decoded = HandoffEnvelope::from_bytes(&envelope.to_bytes().unwrap()).unwrap();
        assert!(
            matches!(
                decoded.receive(recipient.master_public_key()),
                Err(HandoffError::Manifest(_))
            ),
            "{case}"
        );
        // Public ReceivedHandoff construction must not bypass validation.
        let received = ReceivedHandoff {
            sender: sender.master_public_key(),
            bundle: envelope.payload.bundle,
            media: incoming_media,
        };
        let before = local.clone();
        let before_media = local_media.clone();
        assert!(
            matches!(
                received.accept_branch(&mut local, &mut local_media),
                Err(HandoffError::Manifest(_))
            ),
            "{case}"
        );
        assert_eq!(local, before, "{case}");
        assert_eq!(local_media.len(), before_media.len(), "{case}");
        for phrase in before.session.phrases.values() {
            assert_eq!(
                local_media.get(&phrase.media),
                before_media.get(&phrase.media),
                "{case}"
            );
        }
    }
}

#[test]
fn acceptance_rejects_malformed_local_graph_without_mutation() {
    let (mut local, mut local_media, incoming, incoming_media) = divergent_captures();
    let received = receive_bundle(&incoming, &incoming_media);
    corrupt_history(&mut local, "self cycle");
    let before = local.clone();
    let before_media = local_media.clone();
    assert!(matches!(
        received.accept_branch(&mut local, &mut local_media),
        Err(HandoffError::Manifest(_))
    ));
    assert_eq!(local, before);
    assert_eq!(local_media.len(), before_media.len());
    for phrase in before.session.phrases.values() {
        assert_eq!(
            local_media.get(&phrase.media),
            before_media.get(&phrase.media)
        );
    }
}

#[test]
fn acceptance_rejects_missing_public_media_without_mutation() {
    let (mut local, mut local_media, incoming, _) = divergent_captures();
    let before = local.clone();
    let before_media = local_media.clone();
    let received = ReceivedHandoff {
        sender: InMemoryProvider::from_seed([1; 32]).master_public_key(),
        bundle: incoming,
        media: InMemoryStore::new(),
    };
    assert!(matches!(
        received.accept_branch(&mut local, &mut local_media),
        Err(HandoffError::MissingMedia(_))
    ));
    assert_eq!(local, before);
    assert_eq!(local_media.len(), before_media.len());
    for phrase in before.session.phrases.values() {
        assert_eq!(
            local_media.get(&phrase.media),
            before_media.get(&phrase.media)
        );
    }
}

#[test]
fn signed_handoff_round_trips_complete_project_media() {
    let sender = InMemoryProvider::from_seed([1; 32]);
    let recipient = InMemoryProvider::from_seed([2; 32]);
    let mut store = InMemoryStore::new();
    let bundle = bundle_with_media(&mut store);

    let envelope =
        HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender).unwrap();
    let received = HandoffEnvelope::from_bytes(&envelope.to_bytes().unwrap())
        .unwrap()
        .receive(recipient.master_public_key())
        .unwrap();
    assert_eq!(received.bundle, bundle);
    assert_eq!(received.media.len(), 1);
    assert_eq!(received.sender, sender.master_public_key());
}

#[test]
fn handoff_rejects_the_wrong_recipient_and_tampering() {
    let sender = InMemoryProvider::from_seed([1; 32]);
    let recipient = InMemoryProvider::from_seed([2; 32]);
    let other = InMemoryProvider::from_seed([3; 32]);
    let mut store = InMemoryStore::new();
    let bundle = bundle_with_media(&mut store);
    let envelope =
        HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender).unwrap();
    assert!(matches!(
        envelope.clone().receive(other.master_public_key()),
        Err(HandoffError::RecipientMismatch)
    ));
    let mut tampered = envelope;
    tampered.payload.bundle.session.bpm = 90.0;
    assert!(matches!(
        tampered.receive(recipient.master_public_key()),
        Err(HandoffError::InvalidSignature)
    ));

    let mut wrong_session =
        HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender).unwrap();
    wrong_session.session_id = SessionId::new();
    assert!(matches!(
        wrong_session.receive(recipient.master_public_key()),
        Err(HandoffError::InvalidSenderAttestation)
    ));
}

#[test]
fn handoff_requires_all_referenced_media() {
    let sender = InMemoryProvider::from_seed([1; 32]);
    let recipient = InMemoryProvider::from_seed([2; 32]);
    let mut complete = InMemoryStore::new();
    let bundle = bundle_with_media(&mut complete);
    assert!(matches!(
        HandoffEnvelope::create(
            &bundle,
            &InMemoryStore::new(),
            recipient.master_public_key(),
            &sender,
        ),
        Err(HandoffError::MissingMedia(_))
    ));
}

#[test]
fn accepting_a_same_root_handoff_retains_the_local_branch() {
    let sender = InMemoryProvider::from_seed([1; 32]);
    let recipient = InMemoryProvider::from_seed([2; 32]);
    let mut local_media = InMemoryStore::new();
    let mut local_bundle = bundle_with_media(&mut local_media);
    let mut remote_bundle = local_bundle.clone();
    let mut remote_media = local_media.clone();
    let remote_head = remote_bundle.history.commit(
        Edit::SetBpm {
            from: remote_bundle.session.bpm,
            to: 90.0,
        },
        &mut remote_bundle.session,
        2,
    );
    let new_media = remote_media.put(&[0.5, -0.5], 48_000);
    let phrase = Phrase::new(new_media, remote_bundle.session.bars_per_phrase, 90.0, 3);
    let layer = Layer::new(phrase.id);
    remote_bundle.history.commit(
        Edit::AppendLayer {
            track_id: remote_bundle.session.tracks[1].id,
            phrase,
            layer,
        },
        &mut remote_bundle.session,
        3,
    );

    let envelope = HandoffEnvelope::create(
        &remote_bundle,
        &remote_media,
        recipient.master_public_key(),
        &sender,
    )
    .unwrap();
    let received = envelope.receive(recipient.master_public_key()).unwrap();
    let report = received
        .accept_branch(&mut local_bundle, &mut local_media)
        .unwrap();

    assert_eq!(report.accepted_head, remote_bundle.history.head);
    assert!(report.integrated_nodes >= 2);
    assert_eq!(report.imported_media, 1);
    assert_eq!(local_bundle.session, remote_bundle.session);
    assert!(local_bundle.history.nodes.contains_key(&remote_head));
    assert_eq!(local_media.len(), 2);
}

fn append_capture(
    bundle: &mut ProjectBundle,
    media: &mut InMemoryStore,
    track: usize,
    samples: &[f32],
) {
    let reference = media.put(samples, 48_000);
    let phrase = Phrase::new(
        reference,
        bundle.session.bars_per_phrase,
        bundle.session.bpm,
        2,
    );
    let layer = Layer::new(phrase.id);
    bundle.history.commit(
        Edit::AppendLayer {
            track_id: bundle.session.tracks[track].id,
            phrase,
            layer,
        },
        &mut bundle.session,
        2,
    );
}

fn divergent_captures() -> (ProjectBundle, InMemoryStore, ProjectBundle, InMemoryStore) {
    let mut local_media = InMemoryStore::new();
    let mut local = bundle_with_media(&mut local_media);
    let mut incoming = local.clone();
    let mut incoming_media = local_media.clone();
    append_capture(&mut local, &mut local_media, 0, &[0.6, -0.2]);
    append_capture(&mut incoming, &mut incoming_media, 1, &[-0.3, 0.8]);
    (local, local_media, incoming, incoming_media)
}

fn receive_bundle(bundle: &ProjectBundle, media: &InMemoryStore) -> ReceivedHandoff {
    let sender = InMemoryProvider::from_seed([1; 32]);
    let recipient = InMemoryProvider::from_seed([2; 32]);
    let envelope =
        HandoffEnvelope::create(bundle, media, recipient.master_public_key(), &sender).unwrap();
    HandoffEnvelope::from_bytes(&envelope.to_bytes().unwrap())
        .unwrap()
        .receive(recipient.master_public_key())
        .unwrap()
}

#[test]
fn accepting_divergent_captures_retains_both_pools_and_local_checkout() {
    let (mut local, mut local_media, incoming, incoming_media) = divergent_captures();
    let before = local.clone();
    let before_media = local_media.clone();
    let local_head = local.history.head;
    let report = receive_bundle(&incoming, &incoming_media)
        .accept_branch(&mut local, &mut local_media)
        .unwrap();

    assert_eq!(report.accepted_head, incoming.history.head);
    assert_eq!(report.integrated_nodes, 1);
    assert_eq!(report.imported_media, 1);
    assert_eq!(local.history.head, incoming.history.head);
    assert_eq!(local.session.tracks, incoming.session.tracks);
    assert_eq!(local.session.phrases.len(), 3);
    assert_eq!(local_media.len(), 3);
    for (bundle, media) in [(&before, &before_media), (&incoming, &incoming_media)] {
        for (id, phrase) in &bundle.session.phrases {
            assert_eq!(local.session.phrases.get(id), Some(phrase));
            assert_eq!(local_media.get(&phrase.media), media.get(&phrase.media));
        }
        for (id, node) in &bundle.history.nodes {
            assert_eq!(local.history.nodes.get(id), Some(node));
        }
    }

    local
        .history
        .checkout(local_head, &mut local.session)
        .unwrap();
    assert_eq!(local.history.head, local_head);
    assert_eq!(local.session.tracks, before.session.tracks);
    assert_eq!(local.session.phrases.len(), 3);
    assert_eq!(local_media.len(), 3);
    assert!(local.history.nodes.contains_key(&incoming.history.head));
}

#[test]
fn acceptance_retains_incoming_captures_from_an_inactive_branch() {
    let (mut local, mut local_media, mut incoming, mut incoming_media) = divergent_captures();
    let retained_head = incoming.history.head;
    assert!(incoming.history.undo(&mut incoming.session));
    append_capture(&mut incoming, &mut incoming_media, 2, &[0.1, -0.7]);

    receive_bundle(&incoming, &incoming_media)
        .accept_branch(&mut local, &mut local_media)
        .unwrap();
    assert_eq!(local.session.tracks, incoming.session.tracks);
    assert_eq!(local.session.phrases.len(), 4);
    assert_eq!(local_media.len(), 4);
    local
        .history
        .checkout(retained_head, &mut local.session)
        .unwrap();
    assert_eq!(local.session.tracks[1].layers.len(), 1);
    assert!(local.session.tracks[2].layers.is_empty());
    for phrase in incoming.session.phrases.values() {
        assert_eq!(
            local_media.get(&phrase.media),
            incoming_media.get(&phrase.media)
        );
    }
}

#[test]
fn acceptance_rejects_malformed_snapshots_without_mutation() {
    for case in [
        "changed tempo",
        "missing historical phrase",
        "unrecorded phrase",
    ] {
        let (mut local, mut local_media, mut incoming, mut incoming_media) = divergent_captures();
        match case {
            "changed tempo" => incoming.session.bpm += 1.0,
            "missing historical phrase" => {
                let common = local.session.tracks[0].layers[0].phrase_id;
                incoming.session.phrases.remove(&common);
            },
            "unrecorded phrase" => {
                let media = incoming_media.put(&[0.9], 48_000);
                let phrase = Phrase::new(media, 4, 120.0, 3);
                incoming.session.phrases.insert(phrase.id, phrase);
            },
            _ => unreachable!(),
        }
        let before = local.clone();
        let before_media = local_media.clone();
        let result =
            receive_bundle(&incoming, &incoming_media).accept_branch(&mut local, &mut local_media);
        assert!(
            matches!(result, Err(HandoffError::SnapshotMismatch)),
            "{case}: {result:?}"
        );
        assert_eq!(local, before, "{case}");
        assert_eq!(local_media.len(), before_media.len(), "{case}");
        for phrase in before.session.phrases.values() {
            assert_eq!(
                local_media.get(&phrase.media),
                before_media.get(&phrase.media),
                "{case}"
            );
        }
    }
}

#[test]
fn acceptance_rejects_conflicting_phrase_identity_without_mutation() {
    let (mut local, mut local_media, mut incoming, incoming_media) = divergent_captures();
    let common = local.session.tracks[0].layers[0].phrase_id;
    incoming
        .session
        .phrases
        .get_mut(&common)
        .unwrap()
        .captured_at_ms += 1;
    let before = local.clone();
    let before_media = local_media.clone();

    let result =
        receive_bundle(&incoming, &incoming_media).accept_branch(&mut local, &mut local_media);
    assert!(matches!(result, Err(HandoffError::ConflictingPhrase(id)) if id == common));
    assert_eq!(local, before);
    assert_eq!(local_media.len(), before_media.len());
    for phrase in before.session.phrases.values() {
        assert_eq!(
            local_media.get(&phrase.media),
            before_media.get(&phrase.media)
        );
    }
}
