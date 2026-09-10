//! The I/O seam: everything a platform must provide to carry the protocol.
//!
//! The rest of this crate is sans-io by design, and [`Link`] is the one place
//! that admits I/O exists — as a trait, so the admission stays abstract. A
//! platform implements four operations and gets the whole protocol stack above
//! it unchanged: framing, the codec, the JSON-RPC layer, the effect model.
//!
//! # Why this is only four methods
//!
//! Read against a working BLE client, the protocol needs far less of a
//! transport than a GATT API offers. It writes to one characteristic, reads
//! that same characteristic once at connect time for the version banner,
//! consumes notifications from another, and eventually hangs up. Discovery,
//! pairing, service enumeration and reconnection are *not* here: they differ so
//! much between platforms that abstracting them would produce a trait shaped
//! like whichever platform was written first.
//!
//! So a [`Link`] is a connection that already exists. Getting one is the
//! platform's business; carrying the protocol over it is this crate's.
//!
//! # Deliberately not `Send`
//!
//! The `async fn`s here carry no auto-trait bounds, which Rust warns about for
//! public traits because callers cannot then require `Send`. That is the
//! intended trade. `btleplug`'s futures are `Send`; Web Bluetooth's, reached
//! through `wasm-bindgen` on a single-threaded runtime, are not and cannot be.
//! Demanding `Send` here would compile fine today on desktop and lock the web
//! out permanently — the exact outcome this trait exists to prevent.
//!
//! A consumer that genuinely needs `Send` can bound its own generic on it.

use alloc::vec::Vec;
use core::time::Duration;

/// An open connection to an instrument.
///
/// Implementations are platform transports: `btleplug` on desktop,
/// CoreBluetooth on iOS and macOS, Web Bluetooth in a browser, the Android BLE
/// stack under JNI.
#[allow(
    async_fn_in_trait,
    reason = "Send is deliberately not required; see the module docs"
)]
pub trait Link {
    /// Whatever the platform's I/O fails with.
    type Error;

    /// Write one message to the request characteristic.
    ///
    /// `with_response` selects the GATT write type. The protocol's normal path
    /// is a write *with* response, because the instrument acknowledges frames
    /// and an unacknowledged write is not flow-controlled against it; the
    /// unacknowledged form exists for probing.
    ///
    /// Bytes are already framed and, where the transport calls for it,
    /// compressed. A link neither inspects nor alters them.
    async fn write(&self, bytes: &[u8], with_response: bool) -> Result<(), Self::Error>;

    /// Read the response characteristic directly.
    ///
    /// Not a notification: at connect time this characteristic holds a
    /// plain-text version banner rather than a protocol message, and reading it
    /// is how the transport generation is chosen.
    async fn read_response(&self) -> Result<Vec<u8>, Self::Error>;

    /// The next notification, or `None` if `within` elapses first.
    ///
    /// Timing belongs to the implementation because timers are as
    /// platform-specific as the radio: `tokio::time` on desktop, a
    /// `setTimeout` race in a browser. Returning `None` on expiry rather than
    /// an error keeps "nothing arrived" distinct from "the link broke", which
    /// this protocol needs — a silent method and a failed one are different
    /// findings.
    ///
    /// Bytes arrive exactly as the device sent them. A link must not lossily
    /// decode them to text: replies may be compressed binary, and
    /// `String::from_utf8_lossy` destroys them.
    async fn next_notification(&mut self, within: Duration) -> Option<Vec<u8>>;

    /// Hand the instrument back.
    ///
    /// Consuming, because the instrument serves one client at a time and a
    /// link that has been released cannot be used again. Releasing is a real
    /// exchange with the device, so it is fallible and asynchronous.
    async fn disconnect(self) -> Result<(), Self::Error>
    where
        Self: Sized;

    /// The largest write this link can carry, when the platform will say.
    ///
    /// Defaults to `None`, which is the honest answer on `btleplug` — it
    /// exposes no MTU accessor at all, not even to read what was negotiated.
    /// Web Bluetooth caps writes at a known size and can answer. A consumer
    /// with `None` falls back to its own assumption.
    fn write_len_hint(&self) -> Option<usize> {
        None
    }
}
