// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Portable signed, transport-neutral Hocket session hand-off.
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

use hocket_model::{MediaRef, ProjectBundle, SessionId};
use insigne::DerivedKeyAttestation;
use personae::{Ed25519PublicKey, Ed25519Signature, IdentityProvider};
use serde::{Deserialize, Serialize};

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
    Manifest(hocket_model::PersistenceError),
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
    ConflictingPhrase(hocket_model::PhraseId),
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
            Self::Manifest(error) => write!(f, "handoff manifest failed: {error}"),
            Self::Encode(error) => write!(f, "handoff encoding failed: {error}"),
            Self::Decode(error) => write!(f, "handoff decoding failed: {error}"),
            Self::Identity(error) => write!(f, "handoff identity failed: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported handoff format version {version}")
            },
            Self::RecipientMismatch => f.write_str("handoff is addressed to another recipient"),
            Self::InvalidSender => f.write_str("handoff sender key is invalid"),
            Self::InvalidSenderAttestation => {
                f.write_str("handoff sender key is not authorized by its durable identity")
            },
            Self::InvalidSignature => f.write_str("handoff signature does not verify"),
            Self::SessionMismatch => f.write_str("handoff belongs to another session"),
            Self::SnapshotMismatch => {
                f.write_str("handoff manifest does not match its history head")
            },
            Self::ConflictingPhrase(id) => {
                write!(f, "handoff phrase {id} conflicts with local data")
            },
            Self::History(error) => write!(f, "handoff history failed: {error}"),
            Self::MissingMedia(references) => {
                write!(f, "handoff is missing {} media blob(s)", references.len())
            },
            Self::MediaHashMismatch { expected, actual } => {
                write!(
                    f,
                    "handoff media hash mismatch: expected {expected}, found {actual}"
                )
            },
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
        bundle.validate().map_err(HandoffError::Manifest)?;
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
        if sender.derived() != &signer.public_key().to_bytes() {
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
        let checked = self
            .sender
            .check(&salt)
            .map_err(|_| HandoffError::InvalidSenderAttestation)?;
        let sender = Ed25519PublicKey::from_bytes(checked.derived())
            .map_err(|_| HandoffError::InvalidSender)?;
        let sender_identity = Ed25519PublicKey::from_bytes(checked.master())
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

        self.payload
            .bundle
            .validate()
            .map_err(HandoffError::Manifest)?;
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
    /// Both append-only phrase pools are retained, including captured phrases
    /// on inactive branches. An existing phrase identity cannot change contents.
    pub fn accept_branch(
        self,
        local_bundle: &mut ProjectBundle,
        local_media: &mut InMemoryStore,
    ) -> Result<BranchAcceptance, HandoffError> {
        local_bundle.validate().map_err(HandoffError::Manifest)?;
        self.bundle.validate().map_err(HandoffError::Manifest)?;
        if local_bundle.session.id != self.bundle.session.id {
            return Err(HandoffError::SessionMismatch);
        }
        // Checkout changes active layers, but intentionally never removes
        // captured phrases. Compare against the incoming snapshot with the
        // retained local pool included, rather than requiring it to forget
        // local captures the sender has not seen.
        let mut expected_session = self.bundle.session.clone();
        for (id, phrase) in &local_bundle.session.phrases {
            if let Some(incoming) = expected_session.phrases.get(id) {
                if incoming != phrase {
                    return Err(HandoffError::ConflictingPhrase(*id));
                }
            } else {
                expected_session.phrases.insert(*id, phrase.clone());
            }
        }
        let accepted_head = self.bundle.history.head;
        let mut history = local_bundle.history.clone();
        let integrated_nodes = history.integrate(&self.bundle.history)?;
        let mut session = local_bundle.session.clone();
        history.checkout(accepted_head, &mut session)?;
        // Incoming captures on retained, inactive branches are not visited by
        // checkout. Admit those immutable phrases only when the incoming pool
        // matches its capture edits. Missing or changed historical phrases must
        // still fail snapshot validation, even when the local pool has them.
        for node in self.bundle.history.nodes.values() {
            if let hocket_model::Edit::AppendLayer { phrase, .. } = &node.edit {
                if self.bundle.session.phrases.get(&phrase.id) != Some(phrase) {
                    return Err(HandoffError::SnapshotMismatch);
                }
                if let Some(existing) = session.phrases.get(&phrase.id) {
                    if existing != phrase {
                        return Err(HandoffError::ConflictingPhrase(phrase.id));
                    }
                } else {
                    session.phrases.insert(phrase.id, phrase.clone());
                }
            }
        }
        if session != expected_session {
            return Err(HandoffError::SnapshotMismatch);
        }
        let mut media = local_media.clone();
        let mut imported_media = 0;
        for reference in referenced_media(&self.bundle) {
            let buffer = self
                .media
                .get(&reference)
                .ok_or_else(|| HandoffError::MissingMedia(BTreeSet::from([reference])))?;
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
mod tests;
