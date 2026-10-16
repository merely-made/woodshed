// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The persisted owner's personal relation assertions.
//!
//! This is one session-wide, owner-held assertion ledger. Catalog relations
//! remain read-only in `woodshed-graph`; only this source accepts personal
//! writes. Its journal is the authority for assertion identity and lifecycle.

use std::fmt;

pub use chartulary::{AssertionBasis, Author, EdgeId, Relation, RelationClass};
use chartulary::{AssertionRecord, Batch, CommitError, Container, EditSpec, GraphLog};
use muniment::Journal;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

/// Stable identity of the personal assertion source owned by one persisted
/// Woodshed session. This is distinct from the authenticated actor and from
/// the in-process GraphLog revision.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PersonalRelationSourceId(String);

impl PersonalRelationSourceId {
    /// Construct the canonical UUID string from random bytes supplied by the
    /// trusted host. The host owns randomness and persists this value once.
    pub fn from_random_uuid_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let value = format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15]
        );
        Self(value)
    }

    /// The stable source identifier as a canonical UUID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for PersonalRelationSourceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PersonalRelationSourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if is_canonical_uuid4(&value) {
            Ok(Self(value))
        } else {
            Err(D::Error::custom(
                "personal relation source ID must be a canonical UUIDv4",
            ))
        }
    }
}

fn is_canonical_uuid4(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|&index| bytes[index] == b'-')
        && bytes.iter().enumerate().all(|(index, byte)| {
            [8, 13, 18, 23].contains(&index)
                || byte.is_ascii_digit()
                || (b'a'..=b'f').contains(byte)
        })
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

impl fmt::Display for PersonalRelationSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The journal-backed persisted form. A missing value in an old session means
/// no owner ledger was recorded; the host must mint and save a source ID
/// before enabling writes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedPersonalRelationJournal {
    pub source_id: PersonalRelationSourceId,
    pub journal: Journal<Batch<Container, Relation>>,
}

/// A single session's personal relation source.
pub struct PersonalRelationSource {
    source_id: PersonalRelationSourceId,
    graph: GraphLog<Container, Relation>,
}

/// Why a personal relation action was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersonalRelationError {
    /// The action was based on an earlier in-process source generation.
    StaleRevision { expected: u64, actual: u64 },
    /// The endpoint is not present in the current Woodshed theory catalog.
    UnknownEndpoint(String),
    /// The exact retained assertion does not exist.
    UnknownAssertion(EdgeId),
    /// The assertion is no longer held and cannot be changed or revived.
    NotHeld(EdgeId),
    /// The supplied endpoints do not match the stable assertion identity.
    EndpointMismatch(EdgeId),
    /// The authenticated actor does not own this assertion.
    NotOwner(EdgeId),
    /// Chartulary refused the atomic journal batch.
    Commit(String),
}

impl fmt::Display for PersonalRelationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleRevision { expected, actual } => {
                write!(
                    f,
                    "stale personal relation action: expected {expected}, found {actual}"
                )
            },
            Self::UnknownEndpoint(id) => write!(f, "unknown catalog endpoint: {id}"),
            Self::UnknownAssertion(id) => write!(f, "unknown personal assertion: {id:?}"),
            Self::NotHeld(id) => write!(f, "personal assertion is no longer held: {id:?}"),
            Self::EndpointMismatch(id) => write!(f, "personal assertion endpoints changed: {id:?}"),
            Self::NotOwner(id) => write!(f, "authenticated actor does not own assertion: {id:?}"),
            Self::Commit(reason) => write!(f, "personal relation journal refused action: {reason}"),
        }
    }
}

impl std::error::Error for PersonalRelationError {}

impl PersonalRelationSource {
    /// Start a new empty personal source. Its ID must be generated by the
    /// trusted host and persisted before this source is used for mutation.
    pub fn empty(source_id: PersonalRelationSourceId) -> Self {
        Self {
            source_id,
            graph: GraphLog::new(),
        }
    }

    /// Reopen a persisted source by replaying its journal.
    pub fn restore(persisted: PersistedPersonalRelationJournal) -> Self {
        Self {
            source_id: persisted.source_id,
            graph: GraphLog::replay(persisted.journal),
        }
    }

    /// The source's stable identity, unchanged by reopen or edits.
    pub fn source_id(&self) -> &PersonalRelationSourceId {
        &self.source_id
    }

    /// In-process generation used only to reject stale local actions. This is
    /// not a public or durable source revision.
    pub fn runtime_revision(&self) -> u64 {
        self.graph.revision()
    }

    /// Every retained personal assertion in stable EdgeId order.
    pub fn retained_assertions(&self) -> impl Iterator<Item = &AssertionRecord<String, Relation>> {
        self.graph.assertions()
    }

    /// Currently held personal assertions.
    pub fn held_assertions(&self) -> impl Iterator<Item = &AssertionRecord<String, Relation>> {
        self.graph
            .assertions()
            .filter(|record| record.span.is_held())
    }

    /// Snapshot source identity and append-only journal for session persistence.
    pub fn persisted(&self) -> PersistedPersonalRelationJournal {
        PersistedPersonalRelationJournal {
            source_id: self.source_id.clone(),
            journal: self.graph.log().clone(),
        }
    }

    /// Assert a directed personal relation. All action inputs are explicit;
    /// the caller supplies the authenticated actor and source record time.
    pub fn assert_relation(
        &mut self,
        actor: &Author,
        expected_revision: u64,
        from: &str,
        to: &str,
        relation: Relation,
        at_ms: Option<u64>,
        basis: AssertionBasis,
    ) -> Result<EdgeId, PersonalRelationError> {
        self.check_revision(expected_revision)?;
        let from_node = catalog_node(from)
            .ok_or_else(|| PersonalRelationError::UnknownEndpoint(from.to_owned()))?;
        let to_node = catalog_node(to)
            .ok_or_else(|| PersonalRelationError::UnknownEndpoint(to.to_owned()))?;

        let mut edits = Vec::new();
        if self.graph.graph().key_of(&from.to_owned()).is_none() {
            edits.push(EditSpec::InsertNode(from_node));
        }
        if from != to && self.graph.graph().key_of(&to.to_owned()).is_none() {
            edits.push(EditSpec::InsertNode(to_node));
        }
        edits.push(EditSpec::Assert {
            from: from.to_owned(),
            to: to.to_owned(),
            edge: relation,
            asserter: Some(actor.clone()),
            at_ms,
            basis,
        });
        let committed = self
            .graph
            .commit_batch(actor.clone(), expected_revision, edits)
            .map_err(commit_error)?;
        committed
            .edges
            .last()
            .copied()
            .ok_or_else(|| PersonalRelationError::Commit("assertion ID was not returned".into()))
    }

    /// Confirm an exact held assertion owned by the authenticated actor.
    pub fn confirm_exact(
        &mut self,
        actor: &Author,
        expected_revision: u64,
        id: EdgeId,
        expected_from: &str,
        expected_to: &str,
        at_ms: Option<u64>,
    ) -> Result<(), PersonalRelationError> {
        self.check_revision(expected_revision)?;
        self.check_owned_held(actor, id, expected_from, expected_to)?;
        if !self.graph.confirm_assertion(actor, id, at_ms) {
            return Err(PersonalRelationError::NotHeld(id));
        }
        Ok(())
    }

    /// Withdraw an exact held assertion owned by the authenticated actor.
    pub fn withdraw_exact(
        &mut self,
        actor: &Author,
        expected_revision: u64,
        id: EdgeId,
        expected_from: &str,
        expected_to: &str,
        at_ms: Option<u64>,
    ) -> Result<(), PersonalRelationError> {
        self.check_revision(expected_revision)?;
        self.check_owned_held(actor, id, expected_from, expected_to)?;
        if !self.graph.withdraw_assertion(actor, id, at_ms) {
            return Err(PersonalRelationError::NotHeld(id));
        }
        Ok(())
    }

    fn check_revision(&self, expected: u64) -> Result<(), PersonalRelationError> {
        let actual = self.graph.revision();
        if expected == actual {
            Ok(())
        } else {
            Err(PersonalRelationError::StaleRevision { expected, actual })
        }
    }

    fn check_owned_held(
        &self,
        actor: &Author,
        id: EdgeId,
        expected_from: &str,
        expected_to: &str,
    ) -> Result<(), PersonalRelationError> {
        let record = self
            .graph
            .assertion(id)
            .ok_or(PersonalRelationError::UnknownAssertion(id))?;
        if record.from != expected_from || record.to != expected_to {
            return Err(PersonalRelationError::EndpointMismatch(id));
        }
        if !record.span.is_held() {
            return Err(PersonalRelationError::NotHeld(id));
        }
        if record.asserter.as_ref() != Some(actor) {
            return Err(PersonalRelationError::NotOwner(id));
        }
        Ok(())
    }
}

fn catalog_node(id: &str) -> Option<Container> {
    woodshed_graph::catalog_materials()
        .into_iter()
        .find(|material| material.id == id)
        .map(|material| {
            Container::new(material.id)
                .with_title(material.name)
                .with_tag(material.kind.label())
        })
}

fn commit_error(error: CommitError<String>) -> PersonalRelationError {
    PersonalRelationError::Commit(format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use chartulary::{AssertionMethod, RelationClass};

    use super::*;

    fn source() -> PersonalRelationSource {
        PersonalRelationSource::empty(PersonalRelationSourceId::from_random_uuid_bytes([7; 16]))
    }

    fn relation(label: &str) -> Relation {
        Relation::new(RelationClass::app("woodshed", woodshed_graph::RELATED)).with_label(label)
    }

    fn basis(note: &str) -> AssertionBasis {
        AssertionBasis {
            note: Some(note.into()),
            method: Some(AssertionMethod {
                iri: "urn:woodshed:test-method".into(),
                version: Some("1".into()),
            }),
            evidence: vec!["capture:test".into()],
        }
    }

    #[test]
    fn independent_personal_assertions_keep_distinct_ids_and_authors_through_reopen() {
        let mut source = source();
        let actor_a = Author::new("persona:alice");
        let actor_b = Author::new("persona:bob");
        let first = source
            .assert_relation(
                &actor_a,
                source.runtime_revision(),
                "chord:Major",
                "scale:Major",
                relation("fits my practice"),
                Some(10),
                basis("independent reason A"),
            )
            .unwrap();
        let second = source
            .assert_relation(
                &actor_b,
                source.runtime_revision(),
                "chord:Major",
                "scale:Major",
                relation("fits my practice"),
                Some(20),
                basis("independent reason B"),
            )
            .unwrap();
        assert_ne!(first, second);
        let source_id = source.source_id().clone();
        let persisted = source.persisted();
        let encoded = serde_json::to_string(&persisted).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        let restored = PersonalRelationSource::restore(decoded);
        assert_eq!(restored.source_id(), &source_id);
        let rows = restored.retained_assertions().collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].asserter.as_ref(), Some(&actor_a));
        assert_eq!(rows[1].asserter.as_ref(), Some(&actor_b));
        assert_eq!(rows[0].basis.note.as_deref(), Some("independent reason A"));
        assert_eq!(rows[1].basis.note.as_deref(), Some("independent reason B"));
    }

    #[test]
    fn source_id_is_a_canonical_uuid4_and_roundtrips() {
        let source_id = PersonalRelationSourceId::from_random_uuid_bytes([0; 16]);
        assert_eq!(source_id.as_str(), "00000000-0000-4000-8000-000000000000");
        let encoded = serde_json::to_string(&source_id).unwrap();
        assert_eq!(
            serde_json::from_str::<PersonalRelationSourceId>(&encoded).unwrap(),
            source_id
        );
        assert!(serde_json::from_str::<PersonalRelationSourceId>("\"missing\"").is_err());
        assert!(
            serde_json::from_str::<PersonalRelationSourceId>(
                "\"00000000-0000-4000-8000-00000000000A\""
            )
            .is_err()
        );
    }

    #[test]
    fn only_authenticated_owner_can_confirm_or_withdraw_exact_held_assertion() {
        let mut source = source();
        let actor = Author::new("persona:alice");
        let other = Author::new("persona:bob");
        let id = source
            .assert_relation(
                &actor,
                0,
                "chord:Major",
                "scale:Major",
                relation("personal"),
                None,
                basis("source time is unknown"),
            )
            .unwrap();
        assert!(matches!(
            source.withdraw_exact(&other, 1, id, "chord:Major", "scale:Major", Some(30)),
            Err(PersonalRelationError::NotOwner(found)) if found == id
        ));
        assert!(matches!(
            source.withdraw_exact(&actor, 1, id, "scale:Major", "chord:Major", Some(30)),
            Err(PersonalRelationError::EndpointMismatch(found)) if found == id
        ));
        source
            .confirm_exact(&actor, 1, id, "chord:Major", "scale:Major", Some(20))
            .unwrap();
        source
            .withdraw_exact(&actor, 2, id, "chord:Major", "scale:Major", Some(30))
            .unwrap();
        assert!(matches!(
            source.confirm_exact(&actor, 3, id, "chord:Major", "scale:Major", Some(40)),
            Err(PersonalRelationError::NotHeld(found)) if found == id
        ));
        let record = source.retained_assertions().next().unwrap();
        assert_eq!(record.span.first_asserted_ms, None);
        assert_eq!(record.span.last_asserted_ms, Some(20));
        assert_eq!(record.span.withdrawn.as_ref().unwrap().at_ms, Some(30));
    }

    #[test]
    fn stale_local_action_and_unknown_catalog_endpoint_are_refused() {
        let mut source = source();
        let actor = Author::new("persona:alice");
        assert!(matches!(
            source.assert_relation(
                &actor,
                1,
                "chord:Major",
                "scale:Major",
                relation("personal"),
                Some(1),
                basis("stale"),
            ),
            Err(PersonalRelationError::StaleRevision {
                expected: 1,
                actual: 0
            })
        ));
        assert!(matches!(
            source.assert_relation(
                &actor,
                0,
                "not-a-catalog-item",
                "scale:Major",
                relation("personal"),
                Some(1),
                basis("unknown endpoint"),
            ),
            Err(PersonalRelationError::UnknownEndpoint(_))
        ));
        assert_eq!(source.runtime_revision(), 0);
    }
}
