// SPDX-License-Identifier: MPL-2.0
//! Instrument I/O boundary shared by the practice runtime and Ringdown.
//!
//! The default `midi` feature owns the existing MIDI port, message and clock
//! implementation. It has no dependency on the private HyVibe app protocol.
//! Private HyVibe connection/session code lives in `ringdown-ble::instrument`,
//! beside its transport, and is not a dependency of this crate.

#[cfg(feature = "midi")]
pub mod midi;

/// Portable, data-defined software pedal actions and MIDI bindings.
pub mod pedal;
