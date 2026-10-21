// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Save / load support for sessions.
//!
//! Sessions serialize via [ciborium](https://docs.rs/ciborium) as CBOR
//! (RFC 8949). CBOR is self-describing and cross-language, so a saved
//! project is inspectable without Hocket, and it is the serialization
//! Moothold uses — the same bytes persist locally and travel on a
//! hand-off. This replaced postcard at the FT8 serialization migration
//! (2026-07-14); postcard had been the FT2 choice (compact, but Rust-only
//! and not self-describing).
//!
//! Because `Session` and `History` use `BTreeMap` collections, ciborium
//! emits map entries in sorted key order with definite lengths, so encoded
//! output is deterministic — the same logical state encodes to the same
//! bytes every time. That determinism is a prerequisite for
//! content-addressing the project bundle itself later. If a future field
//! introduces a non-`BTreeMap` map or a float that needs shortest-form
//! reduction, revisit against RFC 8949 §4.2 canonical rules.
//!
//! Encoding is not carried in `format_version`: that field tracks the
//! payload *schema*, which this migration did not change, and it cannot be
//! read without first knowing the encoding. Pre-CBOR postcard bundles are
//! simply not decodable, which is a clean break — no such file exists on
//! disk (DOC_POLICY section 3).

use serde::{Deserialize, Serialize};

use crate::history::{History, HistoryError};
use crate::session::Session;

/// On-disk representation of a saved project. Session + history.
///
/// Media buffers are *not* part of this bundle. They live in a
/// content-addressed store keyed by `MediaRef` and travel separately
/// (e.g. via Moothold blob storage).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectBundle {
    /// Schema version for the manifest payload. A future incompatible schema
    /// gets a new version rather than being decoded under false assumptions.
    pub format_version: u16,
    pub session: Session,
    pub history: History,
    /// Local editable kit draft; handoff playback uses rendered recorder media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_pack: Option<audio_packs::PackSelection>,
}

#[derive(Debug)]
pub enum PersistenceError {
    History(HistoryError),
    Encode(String),
    Decode(String),
    UnsupportedVersion(u16),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::History(error) => write!(f, "invalid project history: {error}"),
            Self::Encode(e) => write!(f, "encode failed: {e}"),
            Self::Decode(e) => write!(f, "decode failed: {e}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported project format version {version}")
            }
        }
    }
}

impl std::error::Error for PersistenceError {}

impl ProjectBundle {
    /// Current serialized project-manifest schema.
    pub const FORMAT_VERSION: u16 = 1;

    pub fn new(session: Session, history: History) -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            session,
            history,
            audio_pack: None,
        }
    }

    /// Check schema and retained history topology without changing the bundle.
    /// This does not replay edits or change snapshot equality semantics.
    pub fn validate(&self) -> Result<(), PersistenceError> {
        if self.format_version != Self::FORMAT_VERSION {
            return Err(PersistenceError::UnsupportedVersion(self.format_version));
        }
        if let Some(pack) = &self.audio_pack { pack.validate().map_err(|e| PersistenceError::Decode(format!("audio pack: {e}")))?; }
        self.history.validate().map_err(PersistenceError::History)
    }

    /// Encode to CBOR bytes. Deterministic given equal input because all
    /// collections are `BTreeMap` (sorted keys, definite lengths).
    pub fn to_bytes(&self) -> Result<Vec<u8>, PersistenceError> {
        self.validate()?;
        let mut bytes = Vec::new();
        ciborium::into_writer(self, &mut bytes)
            .map_err(|error| PersistenceError::Encode(error.to_string()))?;
        Ok(bytes)
    }

    /// Decode from CBOR bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PersistenceError> {
        let bundle: Self = ciborium::from_reader(bytes)
            .map_err(|error| PersistenceError::Decode(error.to_string()))?;
        bundle.validate()?;
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Edit;
    use crate::ids::MediaRef;
    use crate::phrase::{Layer, Phrase};

    #[test]
    fn decode_and_encode_refuse_invalid_retained_history() {
        let mut bundle = ProjectBundle::new(Session::new_default(), History::new());
        bundle.history.head = crate::NodeId::new();
        // Serialize directly to represent hostile durable input.
        let mut bytes = Vec::new();
        ciborium::into_writer(&bundle, &mut bytes).unwrap();
        assert!(matches!(ProjectBundle::from_bytes(&bytes), Err(PersistenceError::History(_))));
        assert!(matches!(bundle.to_bytes(), Err(PersistenceError::History(_))));
    }

    #[test]
    fn round_trip_preserves_inactive_capture_pool_and_branches() {
        let mut session = Session::new_default();
        let mut history = History::new();
        let phrase = Phrase::new(MediaRef([7; 32]), 4, 120.0, 1);
        history.commit(Edit::AppendLayer { track_id: session.tracks[0].id,
            layer: Layer::new(phrase.id), phrase }, &mut session, 1);
        history.checkout(history.root, &mut session).unwrap();
        history.commit(Edit::SetBpm { from: 120.0, to: 90.0 }, &mut session, 2);
        let bundle = ProjectBundle::new(session, history);
        assert_eq!(ProjectBundle::from_bytes(&bundle.to_bytes().unwrap()).unwrap(), bundle);
        assert_eq!(bundle.session.phrases.len(), 1);
        assert_eq!(bundle.history.nodes.len(), 3);
    }

    /// FT2 validation criterion: build a session with multiple layers
    /// across multiple tracks, save, reload — bit-identical.
    #[test]
    fn session_with_layers_round_trips() {
        let mut session = Session::new_default();
        let mut history = History::new();

        // Append two layers to track 0, one layer to track 1.
        for (track_idx, layer_count) in &[(0_usize, 2_usize), (1, 1)] {
            let track_id = session.tracks[*track_idx].id;
            for i in 0..*layer_count {
                let phrase = Phrase::new(
                    MediaRef([(*track_idx * 4 + i) as u8; 32]),
                    4,
                    120.0,
                    1_000_000 + *track_idx as u64 * 1000 + i as u64,
                );
                let layer = Layer::new(phrase.id);
                history.commit(
                    Edit::AppendLayer {
                        track_id,
                        phrase,
                        layer,
                    },
                    &mut session,
                    0,
                );
            }
        }

        let bundle = ProjectBundle::new(session.clone(), history.clone());
        let bytes = bundle.to_bytes().unwrap();
        let restored = ProjectBundle::from_bytes(&bytes).unwrap();

        assert_eq!(bundle, restored, "round-trip should be bit-identical");
        assert_eq!(restored.session.phrases.len(), 3);
        assert_eq!(restored.session.tracks[0].layers.len(), 2);
        assert_eq!(restored.session.tracks[1].layers.len(), 1);
        assert_eq!(restored.history.nodes.len(), 4); // root + 3 appends
    }

    /// Bytes are deterministic — same input always produces same output.
    #[test]
    fn encoding_is_deterministic_for_equal_input() {
        let session = Session::new_default();
        let history = History::new();
        let b1 = ProjectBundle::new(session.clone(), history.clone());
        let b2 = ProjectBundle::new(session, history);
        assert_eq!(b1.to_bytes().unwrap(), b2.to_bytes().unwrap());
    }

    #[test]
    fn rejects_unknown_format_version() {
        let mut bundle = ProjectBundle::new(Session::new_default(), History::new());
        bundle.format_version += 1;
        let mut bytes = Vec::new();
        ciborium::into_writer(&bundle, &mut bytes).unwrap();
        assert_eq!(
            ProjectBundle::from_bytes(&bytes).unwrap_err().to_string(),
            "unsupported project format version 2"
        );
    }
}
