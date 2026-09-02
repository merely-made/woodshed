//! Sans-io client protocol for the HyVibe smart guitar.
//!
//! The guitar is driven by JSON-RPC 2.0 carried over a Bluetooth Low Energy
//! GATT service, with a chunking layer beneath it for messages that exceed the
//! negotiated MTU. This crate owns that protocol and nothing else: encoding and
//! decoding, the chunk and reassembly state machine, and the domain model of
//! banks, effects, and typed DSP parameters.
//!
//! It performs no I/O. Bytes go in, bytes and events come out, so the same core
//! serves a desktop Bluetooth shell, a replay harness over captured fixtures,
//! and — should it ever be wanted — an embedded target. Transport lives in a
//! separate crate.
//!
//! # The connection, in order
//!
//! 1. Discover [`GUITAR_SERVICE`].
//! 2. Set the request characteristic to write-with-response.
//! 3. Subscribe to notifications on the response characteristic.
//! 4. *Read* the response characteristic once for the version banner
//!    ([`handshake::Banner`]).
//! 5. Request an MTU of [`MTU_REQUEST`]; usable write length is MTU minus
//!    [`ATT_WRITE_OVERHEAD`].
//! 6. Exchange JSON-RPC, framed by [`llt`] when a message does not fit.
//!
//! # Status
//!
//! Both transports ([`llt`], [`llt2`] with [`compress`]), the banner
//! ([`handshake`]) and the JSON-RPC layer ([`rpc`]) are implemented, tested,
//! and **confirmed against a real instrument**. As of 2026-09-01 that covers
//! `GetStatus`, the metronome, bank selection and naming, and the effect
//! chain: `AddEffect`, `RemoveEffect`, `MoveEffect`, `bypass`, and every key
//! in [`effects::PARAMETER_KEYS`], all audible on a factory bank. The typed
//! model is [`effects`] (the closed vocabulary as [`effects::EffectKind`],
//! [`effects::Effect`] and the provisional [`effects::BankSpec`]) and the
//! planners in [`plan`], one per verified write, each naming the receipt that
//! proves it. Banks are addressed by grid index (see [`rpc::params::bank`])
//! and have no read-back, which is why [`profile`] exists: the client's own
//! record of what it sent, applied from the same [`plan::Edit`] the wire is
//! planned from.
//!
//! Everything here was recovered by static analysis of the vendor's Android
//! application and then exercised against hardware; a method not named above
//! is still a hypothesis. `ReadConfig` wedges the firmware and `AddBank` is
//! destructive to the profile. See
//! `design_docs/2026-08-27_ringdown_founding.md`, Findings H1–H38.
//!
//! # Interoperability
//!
//! Ringdown is an independent implementation, not affiliated with or endorsed
//! by HyVibe. It contains no vendor code: only the interface facts needed to
//! talk to an instrument its owner already has.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod compress;
pub mod crc32;
pub mod effects;
pub mod handshake;
pub mod link;
pub mod llt;
pub mod llt2;
pub mod loopfile;
pub mod plan;
pub mod profile;
pub mod rpc;

/// The guitar's GATT service.
pub const GUITAR_SERVICE: &str = "eb65b6c6-fec3-4ed1-a6fc-9eff755a4160";

/// The characteristic requests are written to.
///
/// Written with response, not without: the vendor's client sets the default
/// write type explicitly, and a write-without-response would not be flow
/// controlled against a device that acknowledges every frame.
pub const GUITAR_CHARACTERISTIC_REQUEST: &str = "eb65b6c6-fec3-4ed1-a6fc-9eff755a4161";

/// The characteristic responses arrive on, by notification.
///
/// Also read once during connection setup, where it yields the version banner
/// rather than a JSON message — see [`handshake`].
pub const GUITAR_CHARACTERISTIC_RESPONSE: &str = "eb65b6c6-fec3-4ed1-a6fc-9eff755a4162";

/// The MTU the client asks for on connect.
pub const MTU_REQUEST: u16 = 517;

/// Bytes of ATT overhead subtracted from the MTU to get the usable write
/// length.
pub const ATT_WRITE_OVERHEAD: u16 = 3;

/// The write length before any MTU negotiation has succeeded.
///
/// This is not merely a conservative default — it is what the write length
/// *is* until the peer agrees to more, and it is too small to carry an LLT
/// frame. A client that has not renegotiated cannot send a chunked message at
/// all, which [`llt::frame_message`] reports rather than silently truncating.
pub const DEFAULT_WRITE_LEN: usize = 20;

/// Usable write length for a negotiated MTU.
///
/// ```
/// assert_eq!(ringdown::write_len_for_mtu(517), 514);
/// assert_eq!(ringdown::write_len_for_mtu(23), 20);
/// ```
pub fn write_len_for_mtu(mtu: u16) -> usize {
    mtu.saturating_sub(ATT_WRITE_OVERHEAD) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiated_mtu_yields_the_expected_write_length() {
        assert_eq!(write_len_for_mtu(MTU_REQUEST), 514);
    }

    #[test]
    fn the_minimum_ble_mtu_yields_the_documented_default() {
        assert_eq!(write_len_for_mtu(23), DEFAULT_WRITE_LEN);
    }

    #[test]
    fn a_nonsensical_mtu_does_not_underflow() {
        assert_eq!(write_len_for_mtu(0), 0);
    }
}
