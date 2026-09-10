// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Content-addressed media storage.
//!
//! Captured audio buffers are addressed by their BLAKE3 hash. The model
//! crate (`hocket-model`) stores `MediaRef` values; the actual `f32`
//! buffers live here (or, later, in a filesystem / Moothold-backed
//! store implementing the same trait).
//!
//! For Feature Target 3a, only the in-memory store exists. Disk
//! backing arrives with Feature Target 8 (local persistence).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use hocket_model::MediaRef;

/// A media buffer: mono `f32` samples plus the rate they were captured
/// at. (Multi-channel support is deferred until the engine grows past
/// mono input.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Trait for storing and retrieving media buffers by content-address.
pub trait MediaStore {
    /// Insert a buffer. Returns the `MediaRef` that addresses it. If
    /// the buffer is already stored under the same ref, this is a
    /// no-op (idempotent — same bytes always produce the same ref).
    fn put(&mut self, samples: &[f32], sample_rate: u32) -> MediaRef;

    /// Retrieve a buffer by reference.
    fn get(&self, reference: &MediaRef) -> Option<&MediaBuffer>;

    /// Number of distinct buffers currently stored.
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory store. Keyed by `MediaRef`; `BTreeMap` for deterministic
/// iteration order (useful for tests).
#[derive(Default, Clone, Debug)]
pub struct InMemoryStore {
    buffers: BTreeMap<MediaRef, MediaBuffer>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MediaStore for InMemoryStore {
    fn put(&mut self, samples: &[f32], sample_rate: u32) -> MediaRef {
        let media_ref = hash_buffer(samples, sample_rate);
        self.buffers
            .entry(media_ref)
            .or_insert_with(|| MediaBuffer {
                samples: samples.to_vec(),
                sample_rate,
            });
        media_ref
    }

    fn get(&self, reference: &MediaRef) -> Option<&MediaBuffer> {
        self.buffers.get(reference)
    }

    fn len(&self) -> usize {
        self.buffers.len()
    }
}

/// BLAKE3 hash of `(sample_rate_le_bytes, samples_as_le_bytes)`.
///
/// Sample rate is included in the hash because the same `f32` sequence
/// at 44.1 kHz vs 48 kHz is different audio; content-addressing should
/// distinguish them.
pub fn hash_buffer(samples: &[f32], sample_rate: u32) -> MediaRef {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&sample_rate.to_le_bytes());
    // Hash samples as raw little-endian f32 bytes. `as_bytes` lookalike
    // via a reinterpret would be faster but `to_le_bytes` is endian-safe
    // and the perf cost (a per-sample 4-byte copy) is negligible at the
    // sample rates we work with.
    for s in samples {
        hasher.update(&s.to_le_bytes());
    }
    let hash = hasher.finalize();
    MediaRef(*hash.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_returns_stable_ref_for_same_bytes() {
        let mut store = InMemoryStore::new();
        let r1 = store.put(&[0.1, 0.2, 0.3], 48_000);
        let r2 = store.put(&[0.1, 0.2, 0.3], 48_000);
        assert_eq!(r1, r2);
        // Idempotent: same ref → store contains exactly one entry.
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn put_different_bytes_yields_different_refs() {
        let mut store = InMemoryStore::new();
        let r1 = store.put(&[0.1, 0.2, 0.3], 48_000);
        let r2 = store.put(&[0.1, 0.2, 0.4], 48_000);
        assert_ne!(r1, r2);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn put_same_samples_different_rate_yields_different_ref() {
        let mut store = InMemoryStore::new();
        let r1 = store.put(&[0.1, 0.2, 0.3], 44_100);
        let r2 = store.put(&[0.1, 0.2, 0.3], 48_000);
        assert_ne!(r1, r2);
    }

    #[test]
    fn get_round_trips_buffer() {
        let mut store = InMemoryStore::new();
        let r = store.put(&[0.5, -0.5, 0.25], 48_000);
        let buf = store.get(&r).unwrap();
        assert_eq!(buf.samples, vec![0.5, -0.5, 0.25]);
        assert_eq!(buf.sample_rate, 48_000);
    }

    #[test]
    fn hash_buffer_matches_put_ref() {
        let mut store = InMemoryStore::new();
        let samples = vec![0.1_f32, 0.2, 0.3];
        let direct = hash_buffer(&samples, 48_000);
        let via_put = store.put(&samples, 48_000);
        assert_eq!(direct, via_put);
    }
}
