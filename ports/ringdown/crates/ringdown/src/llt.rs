//! LLT — the chunking transport that sits beneath JSON-RPC.
//!
//! A Bluetooth write is bounded by the negotiated MTU, and several of the
//! guitar's messages are larger than that. LLT is the layer that deals with it:
//! a message that already fits is sent as-is, and a message that does not is
//! split into numbered frames, each acknowledged by the device before the next
//! is sent.
//!
//! Everything here is pure. Frames go in and out as strings; nothing in this
//! module knows what a Bluetooth connection is.
//!
//! # Wire shape
//!
//! Each frame is a JSON object with deliberately short keys, followed by a
//! newline:
//!
//! ```text
//! {"oid":7,"mid":1,"s":2048,"d":"{\"jsonrpc\":\"2.0\",\"id\":7,\"metho"}\n
//! ```
//!
//! | Key   | Meaning                                                   |
//! |-------|-----------------------------------------------------------|
//! | `oid` | Object id — mirrors the JSON-RPC `id` being carried        |
//! | `mid` | Frame sequence, 1-based                                    |
//! | `d`   | The payload slice                                          |
//! | `s`   | Total length of the unsplit message (optional)             |
//! | `n`   | Filename (optional; file transfer only, unused here)       |
//!
//! # Provenance
//!
//! Recovered by static analysis; unconfirmed against hardware until the Phase 1
//! control run. See `design_docs/2026-08-27_ringdown_founding.md`, Findings F3
//! and F5.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// The status the device reports for a frame it has received.
///
/// The device answers every frame with one of these. Only [`LltCode::Ok`] means
/// "keep going"; [`LltCode::Done`] ends a transfer, and everything else is a
/// failure that abandons it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum LltCode {
    /// The device gave up on the transfer.
    Abort,
    /// Frame accepted; send the next one.
    Ok,
    /// Transfer complete.
    Done,
    /// The device timed out waiting for a frame.
    Timeout,
    /// The device could not keep up.
    Overload,
    /// The frame was not valid.
    Malformed,
    /// A frame in the sequence never arrived.
    MissingChunk,
    /// Frame sequence number was not the one expected.
    WrongMid,
    /// Object id did not match the transfer in progress.
    WrongOid,
}

impl LltCode {
    /// Whether this code means the transfer may continue.
    pub fn is_continue(self) -> bool {
        matches!(self, LltCode::Ok)
    }

    /// Whether this code ends the transfer without an error.
    pub fn is_terminal_success(self) -> bool {
        matches!(self, LltCode::Done)
    }
}

impl From<LltCode> for u8 {
    fn from(code: LltCode) -> u8 {
        match code {
            LltCode::Abort => 0,
            LltCode::Ok => 1,
            LltCode::Done => 2,
            LltCode::Timeout => 3,
            LltCode::Overload => 4,
            LltCode::Malformed => 5,
            LltCode::MissingChunk => 6,
            LltCode::WrongMid => 7,
            LltCode::WrongOid => 8,
        }
    }
}

impl TryFrom<u8> for LltCode {
    type Error = LltError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => LltCode::Abort,
            1 => LltCode::Ok,
            2 => LltCode::Done,
            3 => LltCode::Timeout,
            4 => LltCode::Overload,
            5 => LltCode::Malformed,
            6 => LltCode::MissingChunk,
            7 => LltCode::WrongMid,
            8 => LltCode::WrongOid,
            other => return Err(LltError::UnknownCode(other)),
        })
    }
}

/// A single outbound frame, before serialization.
#[derive(Debug, Clone, Serialize)]
struct Frame<'a> {
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    filename: Option<&'a str>,
    #[serde(rename = "oid")]
    object_id: i64,
    #[serde(rename = "mid")]
    message_id: u32,
    #[serde(rename = "s", skip_serializing_if = "Option::is_none")]
    size: Option<usize>,
    #[serde(rename = "d")]
    data: &'a str,
}

/// The device's acknowledgement of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Ack {
    /// The object id this acknowledges — matches the `id` of the JSON-RPC
    /// message being carried. An ack whose object id does not match the
    /// transfer in progress belongs to something else and must be ignored.
    #[serde(rename = "oid")]
    pub object_id: i64,
    /// Which frame of the sequence is being acknowledged, 1-based.
    #[serde(rename = "mid")]
    pub message_id: u32,
    /// The device's verdict on that frame.
    #[serde(rename = "result")]
    pub code: LltCode,
}

impl Ack {
    /// Parse an acknowledgement, returning `None` if this is not one.
    ///
    /// Notifications on the response characteristic carry both LLT
    /// acknowledgements and complete JSON-RPC replies. A caller demultiplexes
    /// by trying this first: `None` means the bytes are something else, not
    /// that they are malformed.
    pub fn parse(text: &str) -> Option<Ack> {
        serde_json::from_str(text.trim()).ok()
    }
}

/// What to actually put on the wire for one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outbound {
    /// The message fits a single write. Send it unwrapped — LLT is not used.
    Direct(String),
    /// The message was split. Send each frame in order, waiting for
    /// [`LltCode::Ok`] before the next.
    Chunked(Vec<String>),
}

impl Outbound {
    /// The frames to write, in order, whether or not chunking occurred.
    pub fn frames(&self) -> &[String] {
        match self {
            Outbound::Direct(s) => core::slice::from_ref(s),
            Outbound::Chunked(v) => v.as_slice(),
        }
    }

    /// Whether the message needed chunking.
    pub fn is_chunked(&self) -> bool {
        matches!(self, Outbound::Chunked(_))
    }
}

/// Things that can go wrong framing or parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LltError {
    /// `max_write_len` leaves no room for a frame's own JSON wrapper, so no
    /// amount of splitting would ever fit. The usual cause is attempting to
    /// send before an MTU negotiation has succeeded.
    WriteLenTooSmall {
        /// The smallest write length that could carry a frame.
        needed: usize,
        /// The write length that was offered.
        got: usize,
    },
    /// A frame status byte outside the known range.
    UnknownCode(u8),
    /// Serialization failed, which for these types means a bug rather than bad
    /// input.
    Serialize,
}

impl core::fmt::Display for LltError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LltError::WriteLenTooSmall { needed, got } => write!(
                f,
                "max write length {got} is too small for an LLT frame; need at least {needed}"
            ),
            LltError::UnknownCode(c) => write!(f, "unknown LLT status code {c}"),
            LltError::Serialize => write!(f, "failed to serialize an LLT frame"),
        }
    }
}

impl core::error::Error for LltError {}

/// Frame `message` for transmission, splitting it only if it does not fit.
///
/// `object_id` must be the JSON-RPC `id` of the message being carried, since
/// the device matches acknowledgements against it. `max_write_len` is the
/// negotiated attribute write length — MTU minus three — and is a runtime
/// value, not a constant: it is 20 until an MTU negotiation succeeds.
///
/// # Why the frame sizes are found by shrinking
///
/// A frame's serialized length is not a function of its payload's length. The
/// payload is embedded in a JSON string, so a quote or a backslash inside it
/// becomes two characters, and a control character becomes six. A split
/// computed from an average therefore overshoots the MTU precisely on the
/// messages most likely to contain quotes — which, in a protocol whose payload
/// is itself JSON, is all of them. So each frame starts from an estimate and
/// shrinks until the *serialized* frame fits.
pub fn frame_message(
    message: &str,
    object_id: i64,
    max_write_len: usize,
) -> Result<Outbound, LltError> {
    if message.len() <= max_write_len {
        return Ok(Outbound::Direct(String::from(message)));
    }

    // What a frame costs before any payload: the JSON wrapper, worst-case
    // field values, and the trailing newline. Measured rather than guessed, so
    // it stays correct if the frame shape changes.
    let overhead = frame_overhead(object_id, message.len())?;
    // At least one payload character has to fit, and a character can escape to
    // six (""), so a frame that cannot hold that cannot make progress.
    let needed = overhead + 6;
    if max_write_len < needed {
        return Err(LltError::WriteLenTooSmall {
            needed,
            got: max_write_len,
        });
    }

    let budget = max_write_len - overhead;
    let total = message.len();
    let mut frames = Vec::new();
    let mut rest = message;
    let mut message_id: u32 = 1;

    while !rest.is_empty() {
        // Start from the budget in characters and shrink until the serialized
        // frame fits. Slicing is on character boundaries so multi-byte
        // characters are never split.
        let mut take = rest
            .char_indices()
            .nth(budget)
            .map_or(rest.len(), |(i, _)| i);

        let (encoded, consumed) = loop {
            let slice = &rest[..take];
            let frame = Frame {
                filename: None,
                object_id,
                message_id,
                size: Some(total),
                data: slice,
            };
            let mut encoded = serde_json::to_string(&frame).map_err(|_| LltError::Serialize)?;
            encoded.push('\n');

            if encoded.len() <= max_write_len {
                break (encoded, take);
            }

            // Too long: drop one character and try again. `take` is always on a
            // boundary, so stepping back to the previous one is well-defined.
            take = match rest[..take].char_indices().next_back() {
                Some((i, _)) => i,
                // Cannot shrink further: a single character does not fit, which
                // the `needed` check above should already have prevented.
                None => {
                    return Err(LltError::WriteLenTooSmall {
                        needed: encoded.len(),
                        got: max_write_len,
                    });
                }
            };
        };

        frames.push(encoded);
        rest = &rest[consumed..];
        message_id += 1;
    }

    Ok(Outbound::Chunked(frames))
}

/// The serialized size of a frame carrying no payload, plus its newline.
///
/// Computed from a real frame rather than counted by hand, so it cannot drift
/// away from [`Frame`]'s actual shape.
fn frame_overhead(object_id: i64, total: usize) -> Result<usize, LltError> {
    let probe = Frame {
        filename: None,
        object_id,
        message_id: u32::MAX,
        size: Some(total),
        data: "",
    };
    let encoded = serde_json::to_string(&probe).map_err(|_| LltError::Serialize)?;
    Ok(encoded.len() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    /// Reassemble frames the way the device would, so tests assert on the
    /// property that matters rather than on exact byte counts.
    fn reassemble(frames: &[String]) -> String {
        let mut out = String::new();
        for (i, frame) in frames.iter().enumerate() {
            let v: serde_json::Value = serde_json::from_str(frame.trim_end()).unwrap();
            assert_eq!(
                v["mid"].as_u64().unwrap(),
                (i + 1) as u64,
                "frame sequence must be 1-based and contiguous"
            );
            out.push_str(v["d"].as_str().unwrap());
        }
        out
    }

    #[test]
    fn short_messages_skip_llt_entirely() {
        let msg = r#"{"jsonrpc":"2.0","id":1,"method":"GetStatus"}"#;
        let out = frame_message(msg, 1, 514).unwrap();
        assert_eq!(out, Outbound::Direct(msg.to_string()));
        assert!(!out.is_chunked());
        assert_eq!(out.frames().len(), 1);
    }

    #[test]
    fn a_message_exactly_at_the_limit_is_not_chunked() {
        let msg = "x".repeat(64);
        assert!(!frame_message(&msg, 1, 64).unwrap().is_chunked());
        assert!(frame_message(&msg, 1, 63).unwrap().is_chunked());
    }

    #[test]
    fn chunks_reassemble_to_the_original() {
        let msg = "y".repeat(4000);
        let out = frame_message(&msg, 7, 514).unwrap();
        assert!(out.is_chunked());
        assert_eq!(reassemble(out.frames()), msg);
    }

    /// The regression this codec exists for: a payload that is mostly quotes
    /// and backslashes roughly doubles when embedded in a JSON string, so any
    /// split computed arithmetically from the payload length overruns the MTU.
    #[test]
    fn escaping_heavy_payloads_still_fit_every_frame() {
        let msg = r#"{"a":"\"\\\"","b":"\\\\","c":"quoted \"thing\" and \\slash\\"}"#.repeat(80);
        let max = 200;
        let out = frame_message(&msg, 3, max).unwrap();

        assert!(out.is_chunked());
        for frame in out.frames() {
            assert!(
                frame.len() <= max,
                "frame of {} bytes exceeds the {max}-byte write length: {frame}",
                frame.len()
            );
        }
        assert_eq!(reassemble(out.frames()), msg);
    }

    /// Control characters escape to six characters (``), the worst case.
    #[test]
    fn control_characters_still_fit_every_frame() {
        let msg = "\u{1}\u{2}\u{1f}".repeat(300);
        let max = 120;
        let out = frame_message(&msg, 4, max).unwrap();
        for frame in out.frames() {
            assert!(frame.len() <= max, "frame exceeded write length: {frame}");
        }
        assert_eq!(reassemble(out.frames()), msg);
    }

    /// Multi-byte characters must never be split across frames — a half
    /// character is not valid UTF-8 and would not survive serialization.
    #[test]
    fn multibyte_characters_are_never_split() {
        let msg = "é日🎸".repeat(400);
        let out = frame_message(&msg, 5, 150).unwrap();
        for frame in out.frames() {
            assert!(frame.len() <= 150);
            // Parsing would fail outright on a split character.
            let _: serde_json::Value = serde_json::from_str(frame.trim_end()).unwrap();
        }
        assert_eq!(reassemble(out.frames()), msg);
    }

    #[test]
    fn every_frame_carries_the_total_size() {
        let msg = "z".repeat(2000);
        let out = frame_message(&msg, 9, 300).unwrap();
        for frame in out.frames() {
            let v: serde_json::Value = serde_json::from_str(frame.trim_end()).unwrap();
            assert_eq!(v["s"].as_u64().unwrap(), 2000);
            assert_eq!(v["oid"].as_i64().unwrap(), 9);
        }
    }

    #[test]
    fn frames_end_with_a_newline() {
        let out = frame_message(&"q".repeat(1000), 1, 200).unwrap();
        for frame in out.frames() {
            assert!(frame.ends_with('\n'), "frame is not newline-terminated");
        }
    }

    /// The unnegotiated MTU floor. A 20-byte write cannot hold a frame wrapper
    /// plus a worst-case character, so chunking must refuse rather than loop or
    /// emit frames that will be rejected.
    #[test]
    fn refuses_a_write_length_that_could_never_fit() {
        let err = frame_message(&"a".repeat(500), 1, 20).unwrap_err();
        assert!(matches!(err, LltError::WriteLenTooSmall { .. }));
    }

    #[test]
    fn status_codes_round_trip() {
        for raw in 0u8..=8 {
            let code = LltCode::try_from(raw).unwrap();
            assert_eq!(u8::from(code), raw);
        }
        assert_eq!(LltCode::try_from(9), Err(LltError::UnknownCode(9)));
    }

    #[test]
    fn only_ok_continues_a_transfer() {
        assert!(LltCode::Ok.is_continue());
        assert!(!LltCode::Done.is_continue());
        assert!(LltCode::Done.is_terminal_success());
        for code in [
            LltCode::Abort,
            LltCode::Timeout,
            LltCode::Overload,
            LltCode::Malformed,
            LltCode::MissingChunk,
            LltCode::WrongMid,
            LltCode::WrongOid,
        ] {
            assert!(!code.is_continue());
            assert!(!code.is_terminal_success());
        }
    }

    #[test]
    fn acks_parse_and_non_acks_are_declined() {
        let ack = Ack::parse(r#"{"oid":7,"mid":2,"result":1}"#).unwrap();
        assert_eq!(
            ack,
            Ack {
                object_id: 7,
                message_id: 2,
                code: LltCode::Ok
            }
        );

        // A JSON-RPC reply arriving on the same characteristic is not an ack.
        assert!(Ack::parse(r#"{"jsonrpc":"2.0","id":7,"result":{"battLeft":0.9}}"#).is_none());
        // Neither is a status code outside the known range.
        assert!(Ack::parse(r#"{"oid":7,"mid":2,"result":42}"#).is_none());
        assert!(Ack::parse("not json at all").is_none());
    }
}
