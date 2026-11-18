// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! wavicle — a pure-Rust WavPack v5 codec.
//!
//! A wavicle is Eddington's word for the quantum entity that is both wave and
//! particle. A lossless codec makes audio exactly that: a continuous wave to
//! the ear, discrete bits on disk, one identity in both observations.
//!
//! Scope: lossless mono/stereo decode and encode of the WavPack v5 stream —
//! 16/24/32-bit integer and bit-exact 32-bit float PCM. Pure Rust, no C,
//! wasm-capable. Out of scope: DSD, hybrid/lossy modes, correction files,
//! more than two channels, pre-4.0 legacy streams.
//!
//! **Status: round-trip codec, verified against the reference.** Both
//! directions, bit-exact and lossless: decode and encode of 16/24/32-bit
//! integer and 32-bit float, mono and stereo. Verified against the reference
//! `wvunpack`/`wavpack` over a corpus; float in both directions passes a
//! BLAKE3 round-trip identity check over the decoded f32 bytes. Both the
//! `decode` and `encode` features are on by default; [`encode_int`] and
//! [`encode_float`] use a fixed multi-term decorrelation cascade (files land within
//! a hair of the reference encoder's size). Inputs of any length are supported:
//! the encoder splits long data into independent blocks. The founding plan and
//! conformance-oracle method live in the repository's `design_docs/`.

#![forbid(unsafe_code)]

pub mod block;
pub mod error;
pub mod format;
pub mod metadata;

// Shared between the two directions (bit I/O, the median model, sample tables).
#[cfg(any(feature = "decode", feature = "encode"))]
pub mod bitstream;
#[cfg(any(feature = "decode", feature = "encode"))]
pub mod entropy;

#[cfg(any(feature = "decode", feature = "encode"))]
pub mod decorr;

#[cfg(any(feature = "decode", feature = "encode"))]
pub mod float;

#[cfg(feature = "decode")]
pub mod decode;

#[cfg(feature = "encode")]
pub mod encode;

pub use block::{Block, BlockHeader, Blocks, StreamInfo};
#[cfg(feature = "decode")]
pub use decode::{DecodedStream, decode_stream};
#[cfg(feature = "encode")]
pub use encode::{EncodeParams, encode_float, encode_int};
pub use error::{Error, Scope};
