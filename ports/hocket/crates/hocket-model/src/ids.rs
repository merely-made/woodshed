// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Identifier newtypes.
//!
//! All ids are UUIDs (v4 random) to keep collisions across distributed
//! peers impossible without coordination. The `MediaRef` is a 32-byte
//! BLAKE3-shaped digest; the model only stores it — content hashing
//! happens in the engine when audio is captured.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
            Serialize, Deserialize,
        )]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a fresh random id.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_newtype!(SessionId);
id_newtype!(TrackId);
id_newtype!(PhraseId);
id_newtype!(NodeId);

/// A content-addressed reference to a media buffer (typically a
/// captured audio phrase). The bytes here are a digest — BLAKE3-shaped
/// in practice — produced by the engine when audio is captured.
///
/// The model does not interpret the digest; it just stores it. Two
/// `MediaRef`s with the same bytes refer to the same media. The actual
/// audio buffer lives in a separate content-addressed store
/// (filesystem, Moothold blob store, etc.) keyed by these bytes.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
    Serialize, Deserialize,
)]
pub struct MediaRef(pub [u8; 32]);

impl MediaRef {
    /// All-zero ref — useful as a sentinel in tests, not valid media.
    pub const ZERO: Self = Self([0; 32]);
}

impl std::fmt::Display for MediaRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // First 8 bytes as hex is enough for log lines.
        for byte in &self.0[..8] {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, "…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_per_call() {
        let a = TrackId::new();
        let b = TrackId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn media_ref_zero_constant() {
        assert_eq!(MediaRef::ZERO.0, [0u8; 32]);
    }

    #[test]
    fn media_ref_display_is_short_hex() {
        let r = MediaRef([0xAB; 32]);
        let s = format!("{r}");
        assert!(s.starts_with("abababababababab"));
        assert!(s.ends_with('…'));
    }
}
