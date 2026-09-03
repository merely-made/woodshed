// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Signed, transport-neutral Hocket session hand-off.
//!
//! A hand-off is a complete immutable snapshot: project manifest plus every
//! referenced media buffer. The envelope addresses that snapshot to a recipient
//! and is signed by a session-scoped key derived through `personae`. The durable
//! sender identity attests that derived key. Murm, Iroh, a file attachment, or
//! an eventual invite flow can carry its CBOR bytes without changing the
//! application protocol; confidentiality remains the carrier's responsibility.
//!
//! The envelope, and the bytes the signature covers, serialize as CBOR via
//! ciborium (the FT8 serialization migration, 2026-07-14). Signing and
//! verification both re-serialize the same `UnsignedHandoff`; CBOR over these
//! `BTreeMap`/`Vec` types is deterministic, so the verifier reconstructs the
//! exact signed bytes.
//!
//! This module deliberately does not merge edits. `hocket-model::History`
//! retains and integrates divergent branches, but it does not yet synthesize a
//! reconciled head for conflicting concurrent edits.

use std::collections::{BTreeMap, BTreeSet};

use personae::{DerivedKeyAttestation, Ed25519PublicKey, Ed25519Signature, IdentityProvider};
use serde::{Deserialize, Serialize};
use hocket_model::{MediaRef, ProjectBundle, SessionId};

use crate::media::{InMemoryStore, MediaBuffer, MediaStore, hash_buffer};

/// A complete received hand-off, ready for a host to stage for review.
#[derive(Clone, Debug)]
pub struct ReceivedHandoff {
    /// Durable identity that authorized the session-scoped signing key.
    pub sender: Ed25519PublicKey,
    pub bundle: ProjectBundle,
    pub media: InMemoryStore,
}

/// Result of explicitly accepting an incoming branch as the active session
/// head. The local branch remains in the integrated history graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchAcceptance {
    pub integrated_nodes: usize,
    pub accepted_head: hocket_model::NodeId,
    pub imported_media: usize,
}

/// Versioned recipient-addressed session transfer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HandoffEnvelope {
    pub format_version: u16,
    pub session_id: SessionId,
    /// Master-certified session-scoped signing key.
    pub sender: DerivedKeyAttestation,
    /// Intended recipient's public key.
    pub recipient: [u8; 32],
    payload: HandoffPayload,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct HandoffPayload {
    bundle: ProjectBundle,
    media: BTreeMap<MediaRef, MediaBuffer>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct UnsignedHandoff<'a> {
    format_version: u16,
    session_id: SessionId,
    sender: &'a DerivedKeyAttestation,
    recipient: [u8; 32],
    payload: &'a HandoffPayload,
}

#[derive(Debug)]
pub enum HandoffError {
    Encode(String),
    Decode(String),
    Identity(personae::IdentityError),
    UnsupportedVersion(u16),
    RecipientMismatch,
    InvalidSender,
    InvalidSenderAttestation,
    InvalidSignature,
    SessionMismatch,
    SnapshotMismatch,
    History(hocket_model::HistoryError),
    MissingMedia(BTreeSet<MediaRef>),
    MediaHashMismatch {
        expected: MediaRef,
        actual: MediaRef,
    },
}

impl std::fmt::Display for HandoffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(error) => write!(f, "handoff encoding failed: {error}"),
            Self::Decode(error) => write!(f, "handoff decoding failed: {error}"),
            Self::Identity(error) => write!(f, "handoff identity failed: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported handoff format version {version}")
            }
            Self::RecipientMismatch => f.write_str("handoff is addressed to another recipient"),
            Self::InvalidSender => f.write_str("handoff sender key is invalid"),
            Self::InvalidSenderAttestation => {
                f.write_str("handoff sender key is not authorized by its durable identity")
            }
            Self::InvalidSignature => f.write_str("handoff signature does not verify"),
            Self::SessionMismatch => f.write_str("handoff belongs to another session"),
            Self::SnapshotMismatch => {
                f.write_str("handoff manifest does not match its history head")
            }
            Self::History(error) => write!(f, "handoff history failed: {error}"),
            Self::MissingMedia(references) => {
                write!(f, "handoff is missing {} media blob(s)", references.len())
            }
            Self::MediaHashMismatch { expected, actual } => {
                write!(
                    f,
                    "handoff media hash mismatch: expected {expected}, found {actual}"
                )
            }
        }
    }
}

impl std::error::Error for HandoffError {}

impl From<personae::IdentityError> for HandoffError {
    fn from(error: personae::IdentityError) -> Self {
        Self::Identity(error)
    }
}

impl From<hocket_model::HistoryError> for HandoffError {
    fn from(error: hocket_model::HistoryError) -> Self {
        Self::History(error)
    }
}

impl HandoffEnvelope {
    pub const FORMAT_VERSION: u16 = 2;

    /// Build a signed complete snapshot for `recipient`.
    pub fn create(
        bundle: &ProjectBundle,
        media: &impl MediaStore,
        recipient: Ed25519PublicKey,
        identity: &impl IdentityProvider,
    ) -> Result<Self, HandoffError> {
        let references = referenced_media(bundle);
        let missing: BTreeSet<MediaRef> = references
            .iter()
            .filter(|reference| media.get(reference).is_none())
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(HandoffError::MissingMedia(missing));
        }
        let mut handoff_media = BTreeMap::new();
        for reference in references {
            let buffer = media
                .get(&reference)
                .expect("missing media checked before handoff construction");
            let actual = hash_buffer(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(HandoffError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
            handoff_media.insert(reference, buffer.clone());
        }
        let payload = HandoffPayload {
            bundle: bundle.clone(),
            media: handoff_media,
        };
        let salt = handoff_salt(bundle.session.id);
        let signer = identity.derive_keypair(&salt)?;
        let sender = identity.attest_derived_key(&salt)?;
        if sender.derived_public_key()? != signer.public_key() {
            return Err(HandoffError::InvalidSenderAttestation);
        }
        let recipient = recipient.to_bytes();
        let unsigned = UnsignedHandoff {
            format_version: Self::FORMAT_VERSION,
            session_id: bundle.session.id,
            sender: &sender,
            recipient,
            payload: &payload,
        };
        let signature = signer.sign(&cbor_bytes(&unsigned)?).to_bytes().to_vec();
        Ok(Self {
            format_version: Self::FORMAT_VERSION,
            session_id: bundle.session.id,
            sender,
            recipient,
            payload,
            signature,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, HandoffError> {
        cbor_bytes(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HandoffError> {
        let envelope: Self = ciborium::from_reader(bytes)
            .map_err(|error| HandoffError::Decode(error.to_string()))?;
        if envelope.format_version != Self::FORMAT_VERSION {
            return Err(HandoffError::UnsupportedVersion(envelope.format_version));
        }
        Ok(envelope)
    }

    /// Authenticate the envelope, check its address, and materialize its
    /// complete project snapshot. Address matching is not proof of private-key
    /// possession; the carrier supplies access control and confidentiality.
    pub fn receive(
        self,
        expected_recipient: Ed25519PublicKey,
    ) -> Result<ReceivedHandoff, HandoffError> {
        if self.recipient != expected_recipient.to_bytes() {
            return Err(HandoffError::RecipientMismatch);
        }
        let salt = handoff_salt(self.session_id);
        if !self.sender.verify(&salt) {
            return Err(HandoffError::InvalidSenderAttestation);
        }
        let sender = self
            .sender
            .derived_public_key()
            .map_err(|_| HandoffError::InvalidSender)?;
        let sender_identity = self
            .sender
            .master_public_key()
            .map_err(|_| HandoffError::InvalidSender)?;
        let unsigned = UnsignedHandoff {
            format_version: self.format_version,
            session_id: self.session_id,
            sender: &self.sender,
            recipient: self.recipient,
            payload: &self.payload,
        };
        let signed_bytes = cbor_bytes(&unsigned)?;
        let signature = Ed25519Signature::from_bytes(
            self.signature
                .as_slice()
                .try_into()
                .map_err(|_| HandoffError::InvalidSignature)?,
        );
        if !sender.verify(&signed_bytes, &signature) {
            return Err(HandoffError::InvalidSignature);
        }
        if self.payload.bundle.session.id != self.session_id {
            return Err(HandoffError::InvalidSignature);
        }

        let references = referenced_media(&self.payload.bundle);
        let missing: BTreeSet<MediaRef> = references
            .iter()
            .filter(|reference| !self.payload.media.contains_key(reference))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(HandoffError::MissingMedia(missing));
        }
        let mut media = InMemoryStore::new();
        for reference in references {
            let buffer = &self.payload.media[&reference];
            let actual = media.put(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(HandoffError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
        }
        Ok(ReceivedHandoff {
            sender: sender_identity,
            bundle: self.payload.bundle,
            media,
        })
    }
}

impl ReceivedHandoff {
    /// Accept this snapshot's head into a same-session local bundle.
    ///
    /// The operation is transactional in memory: local bundle and media stay
    /// unchanged unless graph integration, checkout, manifest validation, and
    /// media validation all succeed. This chooses the incoming branch as active
    /// but preserves the prior local branch for later checkout; it does not
    /// synthesize a semantic merge of concurrent edits.
    pub fn accept_branch(
        self,
        local_bundle: &mut ProjectBundle,
        local_media: &mut InMemoryStore,
    ) -> Result<BranchAcceptance, HandoffError> {
        if local_bundle.session.id != self.bundle.session.id {
            return Err(HandoffError::SessionMismatch);
        }
        let accepted_head = self.bundle.history.head;
        let mut history = local_bundle.history.clone();
        let integrated_nodes = history.integrate(&self.bundle.history)?;
        let mut session = local_bundle.session.clone();
        history.checkout(accepted_head, &mut session)?;
        if session != self.bundle.session {
            return Err(HandoffError::SnapshotMismatch);
        }
        let mut media = local_media.clone();
        let mut imported_media = 0;
        for reference in referenced_media(&self.bundle) {
            let buffer = self
                .media
                .get(&reference)
                .expect("received handoff validates all referenced media");
            let actual = media.put(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(HandoffError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
            if local_media.get(&reference).is_none() {
                imported_media += 1;
            }
        }
        local_bundle.session = session;
        local_bundle.history = history;
        *local_media = media;
        Ok(BranchAcceptance {
            integrated_nodes,
            accepted_head,
            imported_media,
        })
    }
}

/// Serialize a value to CBOR. Used for both the on-wire envelope and the exact
/// bytes the signature covers, so signing and verification stay byte-identical.
fn cbor_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, HandoffError> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes)
        .map_err(|error| HandoffError::Encode(error.to_string()))?;
    Ok(bytes)
}

fn handoff_salt(session_id: SessionId) -> Vec<u8> {
    let mut salt = b"hocket/handoff/v2/".to_vec();
    salt.extend_from_slice(session_id.0.as_bytes());
    salt
}

fn referenced_media(bundle: &ProjectBundle) -> BTreeSet<MediaRef> {
    bundle
        .session
        .phrases
        .values()
        .map(|phrase| phrase.media)
        .collect()
}

#[cfg(test)]
mod tests {
    use personae::{IdentityProvider, InMemoryProvider};
    use hocket_model::{Edit, History, Layer, Phrase, Session};

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

    #[test]
    fn signed_handoff_round_trips_complete_project_media() {
        let sender = InMemoryProvider::from_seed([1; 32]);
        let recipient = InMemoryProvider::from_seed([2; 32]);
        let mut store = InMemoryStore::new();
        let bundle = bundle_with_media(&mut store);

        let envelope =
            HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender)
                .unwrap();
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
            HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender)
                .unwrap();
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
            HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender)
                .unwrap();
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
}
