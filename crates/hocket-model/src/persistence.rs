// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Save / load support for sessions.
//!
//! Sessions serialize as canonical CBOR::Core (RFC 8949 §4.2.1) through
//! cbor-core, the family's one codec (Mere engine split D209). CBOR is
//! self-describing and cross-language, so a saved
//! project is inspectable without Hocket, and it is the serialization
//! Moothold uses — the same bytes persist locally and travel on a
//! hand-off. This replaced postcard at the FT8 serialization migration
//! (2026-07-14); postcard had been the FT2 choice (compact, but Rust-only
//! and not self-describing).
//!
//! cbor-core sorts every map's keys, struct fields included, and writes
//! shortest integers, lengths and floats, so the same logical state encodes
//! to the same bytes every time. That determinism is a prerequisite for
//! content-addressing the project bundle itself later.
//!
//! Decoding is strict, with one exception for manifests saved before D209:
//! ciborium wrote struct fields in declaration order. Such a manifest is read
//! once through cbor-core's normalization of key order (and float width) and
//! written canonically at the next save.
//!
//! Encoding is not carried in `format_version`: that field tracks the
//! payload *schema*, which this migration did not change, and it cannot be
//! read without first knowing the encoding. Pre-CBOR postcard bundles are
//! simply not decodable, which is a clean break — no such file exists on
//! disk (DOC_POLICY section 3).

use cbor_core::{DecodeOptions, Strictness, Value};
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

    /// Encode to canonical CBOR bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PersistenceError> {
        self.validate()?;
        Value::serialized(self)
            .map(|value| value.encode())
            .map_err(|error| PersistenceError::Encode(error.to_string()))
    }

    /// Decode from canonical CBOR bytes, or from a manifest ciborium wrote.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PersistenceError> {
        let value = match DecodeOptions::new().decode(bytes) {
            Err(cbor_core::Error::NonDeterministic) => DecodeOptions::new()
                .strictness(LEGACY)
                .decode(bytes),
            decoded => decoded,
        }
        .map_err(|error| PersistenceError::Decode(error.to_string()))?;
        let bundle: Self = value
            .deserialized()
            .map_err(|error| PersistenceError::Decode(error.to_string()))?;
        bundle.validate()?;
        Ok(bundle)
    }
}

/// What a pre-D209 manifest may differ by: ciborium's key order and,
/// defensively, a float wider than it needs. Nothing else is normalized.
const LEGACY: Strictness = Strictness {
    allow_unsorted_map_keys: true,
    allow_non_shortest_floats: true,
    ..Strictness::STRICT
};

/// Raw bytes for a bundle, bypassing validation: hostile durable input.
#[cfg(test)]
pub(crate) fn unvalidated_bytes(bundle: &ProjectBundle) -> Vec<u8> {
    Value::serialized(bundle).unwrap().encode()
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
        let bytes = unvalidated_bytes(&bundle);
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
        let bytes = unvalidated_bytes(&bundle);
        assert_eq!(
            ProjectBundle::from_bytes(&bytes).unwrap_err().to_string(),
            "unsupported project format version 2"
        );
    }

    /// A manifest the pre-D209 code wrote with ciborium (captured 2026-11-25).
    fn ciborium_manifest() -> Vec<u8> {
        let hex: String = include_str!("../tests/fixtures/ciborium_manifest_v1.hex")
            .split_whitespace()
            .collect();
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn a_manifest_ciborium_wrote_is_read_once_and_saved_canonically() {
        let legacy = ciborium_manifest();
        // Struct fields in declaration order: strict decoding refuses it.
        assert_eq!(
            DecodeOptions::new().decode(&legacy).unwrap_err(),
            cbor_core::Error::NonDeterministic
        );
        let bundle = ProjectBundle::from_bytes(&legacy).unwrap();
        assert_eq!(bundle.session.phrases.len(), 1);
        assert_eq!(bundle.session.bpm, 92.5);
        let canonical = bundle.to_bytes().unwrap();
        assert_ne!(canonical, legacy);
        assert_eq!(canonical.len(), legacy.len(), "only the key order moved");
        assert!(DecodeOptions::new().decode(&canonical).is_ok(), "the rewrite is canonical");
        assert_eq!(ProjectBundle::from_bytes(&canonical).unwrap(), bundle);
    }

    #[test]
    fn other_non_canonical_manifests_are_refused() {
        let canonical = ProjectBundle::new(Session::new_default(), History::new()).to_bytes().unwrap();
        assert!(ProjectBundle::from_bytes(&canonical).is_ok(), "control");
        // The top-level map head widened to two bytes, which ciborium never wrote.
        assert!((0xa0..0xb8).contains(&canonical[0]), "a short map head");
        let mut wide = vec![0xb9, 0x00, canonical[0] & 0x1f];
        wide.extend_from_slice(&canonical[1..]);
        assert!(matches!(ProjectBundle::from_bytes(&wide), Err(PersistenceError::Decode(_))));
    }
}
