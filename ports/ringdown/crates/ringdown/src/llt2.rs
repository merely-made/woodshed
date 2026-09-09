//! LLT2 — the transport this firmware actually uses.
//!
//! Where [`crate::llt`] wraps JSON text in JSON frames, LLT2 compresses the
//! message with [`crate::compress`] and carries the result in **binary** frames.
//! The vendor's client picks between them by firmware version: both the audio
//! and connectivity processors at 1.2.2 or newer means LLT2, anything older
//! means LLT. See [`selects_llt2`].
//!
//! # Frame layout
//!
//! A compressed message that fits one write is sent **bare** — no header at
//! all — exactly as LLT sends short messages unwrapped. Only when it exceeds
//! the write length is it split:
//!
//! ```text
//! byte 0      transfer type ('J' for a JSON message)
//! byte 1      reserved, always zero
//! byte 2      object id, low byte
//! byte 3      frame number, low byte      (1-based)
//! byte 4      frame number, high byte
//! bytes 5-8   total compressed length, 32-bit little-endian — FIRST FRAME ONLY
//! next 2      this frame's payload length, 16-bit little-endian
//! remainder   payload
//! ```
//!
//! So the header is **11 bytes on the first frame and 7 thereafter**, and each
//! frame carries `min(remaining, write_len - header)` bytes.
//!
//! The device acknowledges each frame with a six-byte reply whose first five
//! bytes echo the header and whose sixth is an [`crate::llt::LltCode`].
//!
//! # Provenance
//!
//! `sendMessage` defeated the decompiler entirely — it emits
//! "Method not decompiled" — so this was read from a second pass in fallback
//! mode, off the raw instruction stream. The layout above is transcribed from
//! the actual `add`/`shift`/`and` sequence rather than from readable Java, and
//! the receive side is corroborated by `onDeviceMessageReceived`, which
//! decompiled cleanly and checks exactly these five header bytes.
//!
//! **Unverified against hardware.** No LLT2 exchange has been performed with
//! the instrument yet.

use alloc::vec::Vec;

use crate::compress;
use crate::handshake::Version;
use crate::llt::LltCode;

/// Transfer type for a JSON message: ASCII `'J'`.
pub const TRANSFER_TYPE_JSON: u8 = 74;

/// Header length on the first frame, which carries the total size.
pub const FIRST_FRAME_HEADER: usize = 11;

/// Header length on every later frame.
pub const LATER_FRAME_HEADER: usize = 7;

/// Length of the device's acknowledgement.
pub const ACK_LEN: usize = 6;

/// The firmware version at which the vendor's client switches to LLT2.
pub const LLT2_MIN_VERSION: Version = Version {
    major: 1,
    minor: 2,
    patch: 2,
};

/// The largest compressed message the vendor's client will send.
pub const MAX_COMPRESSED_SIZE: usize = 16_384;

/// The largest uncompressed message the vendor's client will compress.
pub const MAX_UNCOMPRESSED_SIZE: usize = 131_072;

/// Whether a device with these firmware versions expects LLT2 rather than LLT.
///
/// **Both** processors must be new enough; the vendor's client requires it of
/// each independently, so a mixed pair falls back to the older transport.
pub fn selects_llt2(stm: Version, esp: Version) -> bool {
    stm >= LLT2_MIN_VERSION && esp >= LLT2_MIN_VERSION
}

/// What to write for one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outbound2 {
    /// The compressed message fits a single write. Send these bytes as they
    /// are — there is no frame header on this path.
    Bare(Vec<u8>),
    /// The message was split. Send each frame in order, waiting for
    /// [`LltCode::Ok`] before the next.
    Framed(Vec<Vec<u8>>),
}

impl Outbound2 {
    /// The writes to perform, in order.
    pub fn frames(&self) -> &[Vec<u8>] {
        match self {
            Outbound2::Bare(b) => core::slice::from_ref(b),
            Outbound2::Framed(v) => v.as_slice(),
        }
    }

    /// Whether the message needed splitting.
    pub fn is_framed(&self) -> bool {
        matches!(self, Outbound2::Framed(_))
    }
}

/// The device's acknowledgement of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ack2 {
    /// Transfer type, echoed from the frame.
    pub transfer_type: u8,
    /// Object id, low byte.
    pub object_id: u8,
    /// Which frame is being acknowledged.
    pub frame: u16,
    /// The device's verdict.
    pub code: LltCode,
}

impl Ack2 {
    /// Parse an acknowledgement, returning `None` if these bytes are not one.
    ///
    /// Used to demultiplex: a compressed reply and an acknowledgement arrive on
    /// the same characteristic, and a compressed document always begins with
    /// the start nibble in the high half of its first byte, so a frame whose
    /// first byte is a transfer type cannot be confused with one.
    pub fn parse(data: &[u8]) -> Option<Ack2> {
        if data.len() < ACK_LEN {
            return None;
        }
        Some(Ack2 {
            transfer_type: data[0],
            object_id: data[2],
            frame: u16::from(data[3]) | (u16::from(data[4]) << 8),
            code: LltCode::try_from(data[5]).ok()?,
        })
    }

    /// Whether this acknowledges the given transfer and frame.
    pub fn answers(&self, object_id: u8, frame: u16) -> bool {
        self.object_id == object_id && self.frame == frame
    }
}

/// Things that can go wrong preparing an LLT2 message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Llt2Error {
    /// The JSON could not be compressed.
    Compress(compress::EncodeError),
    /// The compressed message exceeds what the vendor's client will send.
    TooLarge {
        /// Compressed size in bytes.
        size: usize,
        /// The limit that was exceeded.
        limit: usize,
    },
    /// The write length cannot hold a header plus at least one payload byte.
    WriteLenTooSmall {
        /// The smallest workable write length.
        needed: usize,
        /// What was offered.
        got: usize,
    },
}

impl core::fmt::Display for Llt2Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Llt2Error::Compress(e) => write!(f, "could not compress: {e}"),
            Llt2Error::TooLarge { size, limit } => {
                write!(
                    f,
                    "compressed message of {size} bytes exceeds the {limit}-byte limit"
                )
            }
            Llt2Error::WriteLenTooSmall { needed, got } => write!(
                f,
                "write length {got} cannot carry an LLT2 frame; need at least {needed}"
            ),
        }
    }
}

impl core::error::Error for Llt2Error {}

impl From<compress::EncodeError> for Llt2Error {
    fn from(e: compress::EncodeError) -> Self {
        Llt2Error::Compress(e)
    }
}

/// Compress `json` and prepare it for transmission.
///
/// `object_id` is the JSON-RPC `id`; only its low byte reaches the wire, which
/// is the device's own limit rather than a simplification made here.
pub fn prepare(json: &str, object_id: u8, write_len: usize) -> Result<Outbound2, Llt2Error> {
    if json.len() > MAX_UNCOMPRESSED_SIZE {
        return Err(Llt2Error::TooLarge {
            size: json.len(),
            limit: MAX_UNCOMPRESSED_SIZE,
        });
    }

    let compressed = compress::encode(json)?;
    if compressed.len() > MAX_COMPRESSED_SIZE {
        return Err(Llt2Error::TooLarge {
            size: compressed.len(),
            limit: MAX_COMPRESSED_SIZE,
        });
    }

    if compressed.len() <= write_len {
        return Ok(Outbound2::Bare(compressed));
    }

    // Every frame must carry at least one payload byte, and the first frame has
    // the largest header, so that is the binding constraint.
    let needed = FIRST_FRAME_HEADER + 1;
    if write_len < needed {
        return Err(Llt2Error::WriteLenTooSmall {
            needed,
            got: write_len,
        });
    }

    let total = compressed.len() as u32;
    let mut frames = Vec::new();
    let mut rest = compressed.as_slice();
    let mut frame_no: u16 = 1;

    while !rest.is_empty() {
        let header = if frame_no == 1 {
            FIRST_FRAME_HEADER
        } else {
            LATER_FRAME_HEADER
        };
        let take = core::cmp::min(rest.len(), write_len - header);

        let mut frame = Vec::with_capacity(header + take);
        frame.push(TRANSFER_TYPE_JSON);
        frame.push(0);
        frame.push(object_id);
        frame.push((frame_no & 0xff) as u8);
        frame.push((frame_no >> 8) as u8);
        if frame_no == 1 {
            frame.extend_from_slice(&total.to_le_bytes());
        }
        frame.push((take & 0xff) as u8);
        frame.push(((take >> 8) & 0xff) as u8);
        frame.extend_from_slice(&rest[..take]);

        frames.push(frame);
        rest = &rest[take..];
        frame_no += 1;
    }

    Ok(Outbound2::Framed(frames))
}

/// Decode a reply, whether it arrived bare or as a single compressed blob.
///
/// Returns `None` when the bytes are not a compressed document, which is how a
/// caller tells a reply from an acknowledgement.
pub fn decode_reply(data: &[u8]) -> Option<alloc::string::String> {
    compress::decode(data)
}

/// Reassemble a framed reply, should the device ever send one.
///
/// Not observed from hardware: the vendor's client cannot reassemble inbound
/// frames at all, so if the device does chunk a reply it must do so by a
/// mechanism its own app never exercises. Provided because the frame layout is
/// symmetric and reassembly is cheap, but treat a success here as a discovery
/// rather than a routine path.
pub fn reassemble(frames: &[Vec<u8>]) -> Option<alloc::string::String> {
    let mut body = Vec::new();
    for (i, frame) in frames.iter().enumerate() {
        let header = if i == 0 {
            FIRST_FRAME_HEADER
        } else {
            LATER_FRAME_HEADER
        };
        if frame.len() < header {
            return None;
        }
        let len = usize::from(frame[header - 2]) | (usize::from(frame[header - 1]) << 8);
        let payload = frame.get(header..header + len)?;
        body.extend_from_slice(payload);
    }
    compress::decode(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::String};

    fn long_json(pairs: usize) -> String {
        let mut s = String::from("{\"effects\":[");
        for i in 0..pairs {
            if i > 0 {
                s.push(',');
            }
            s.push_str("{\"type\":\"Reverb\",\"key\":\"DryWet\",\"value\":0.5}");
        }
        s.push_str("]}");
        s
    }

    #[test]
    fn version_gate_matches_the_vendors_rule() {
        let old = Version {
            major: 1,
            minor: 2,
            patch: 1,
        };
        let new = Version {
            major: 1,
            minor: 2,
            patch: 2,
        };
        let newer = Version {
            major: 1,
            minor: 3,
            patch: 0,
        };

        assert!(selects_llt2(new, new));
        assert!(selects_llt2(newer, new));
        // The reference instrument: STM 1.2.3, ESP 1.3.0.
        assert!(selects_llt2(
            Version {
                major: 1,
                minor: 2,
                patch: 3
            },
            Version {
                major: 1,
                minor: 3,
                patch: 0
            }
        ));

        // Both processors must qualify, not just one.
        assert!(!selects_llt2(old, new));
        assert!(!selects_llt2(new, old));
        assert!(!selects_llt2(old, old));
    }

    #[test]
    fn a_short_message_is_sent_bare() {
        let out = prepare("{\"id\":1}", 1, 514).unwrap();
        assert!(!out.is_framed());
        // Bare means bare: the bytes are the compressed document itself, with
        // no header in front of them.
        let bytes = &out.frames()[0];
        assert_eq!(compress::decode(bytes).unwrap(), "{\"id\":1}");
    }

    #[test]
    fn a_long_message_is_framed_and_reassembles() {
        let json = long_json(60);
        let out = prepare(&json, 7, 100).unwrap();
        assert!(out.is_framed());
        assert_eq!(reassemble(out.frames()).unwrap(), json);
    }

    #[test]
    fn every_frame_fits_the_write_length() {
        let json = long_json(80);
        for write_len in [20, 64, 100, 244, 514] {
            let out = prepare(&json, 3, write_len).unwrap();
            for frame in out.frames() {
                assert!(
                    frame.len() <= write_len,
                    "frame of {} exceeds write length {write_len}",
                    frame.len()
                );
            }
            assert_eq!(reassemble(out.frames()).unwrap(), json);
        }
    }

    #[test]
    fn the_first_frame_carries_the_total_and_later_ones_do_not() {
        let json = long_json(60);
        let out = prepare(&json, 9, 100).unwrap();
        let frames = out.frames();
        assert!(frames.len() > 2, "need several frames for this test");

        let compressed_len = compress::encode(&json).unwrap().len() as u32;
        let declared = u32::from_le_bytes([frames[0][5], frames[0][6], frames[0][7], frames[0][8]]);
        assert_eq!(declared, compressed_len);

        // Payload accounting proves the later frames use the shorter header:
        // if they did not, the reassembled body would be corrupt, which the
        // round-trip in the other tests already covers, so check the header
        // length arithmetic directly here.
        let first_payload = usize::from(frames[0][9]) | (usize::from(frames[0][10]) << 8);
        assert_eq!(frames[0].len(), FIRST_FRAME_HEADER + first_payload);
        let second_payload = usize::from(frames[1][5]) | (usize::from(frames[1][6]) << 8);
        assert_eq!(frames[1].len(), LATER_FRAME_HEADER + second_payload);
    }

    #[test]
    fn frames_are_numbered_from_one_and_carry_the_object_id() {
        let json = long_json(60);
        let out = prepare(&json, 0x2a, 100).unwrap();
        for (i, frame) in out.frames().iter().enumerate() {
            assert_eq!(frame[0], TRANSFER_TYPE_JSON);
            assert_eq!(frame[1], 0, "byte 1 is reserved and must be zero");
            assert_eq!(frame[2], 0x2a);
            let n = u16::from(frame[3]) | (u16::from(frame[4]) << 8);
            assert_eq!(n as usize, i + 1);
        }
    }

    #[test]
    fn a_write_length_too_small_for_a_header_is_refused() {
        let json = long_json(60);
        let err = prepare(&json, 1, FIRST_FRAME_HEADER).unwrap_err();
        assert!(matches!(err, Llt2Error::WriteLenTooSmall { .. }));
    }

    #[test]
    fn oversized_messages_are_refused_rather_than_sent() {
        let huge = format!("\"{}\"", "x".repeat(MAX_UNCOMPRESSED_SIZE + 10));
        assert!(matches!(
            prepare(&huge, 1, 514),
            Err(Llt2Error::TooLarge { .. })
        ));
    }

    #[test]
    fn acks_parse_and_shorter_frames_are_declined() {
        let raw = [TRANSFER_TYPE_JSON, 0, 0x2a, 0x02, 0x00, 1];
        let ack = Ack2::parse(&raw).unwrap();
        assert_eq!(ack.transfer_type, TRANSFER_TYPE_JSON);
        assert_eq!(ack.object_id, 0x2a);
        assert_eq!(ack.frame, 2);
        assert_eq!(ack.code, LltCode::Ok);
        assert!(ack.answers(0x2a, 2));
        assert!(!ack.answers(0x2a, 3));

        assert!(Ack2::parse(&raw[..5]).is_none());
        assert!(Ack2::parse(&[]).is_none());
    }

    #[test]
    fn a_high_frame_number_survives_both_bytes() {
        let raw = [TRANSFER_TYPE_JSON, 0, 1, 0x34, 0x12, 1];
        assert_eq!(Ack2::parse(&raw).unwrap().frame, 0x1234);
    }

    #[test]
    fn an_unknown_status_byte_is_not_an_ack() {
        let raw = [TRANSFER_TYPE_JSON, 0, 1, 1, 0, 99];
        assert!(Ack2::parse(&raw).is_none());
    }

    /// The two inbound shapes must not be confusable. A compressed document
    /// always starts with the start nibble in the high half of byte zero, so
    /// its first byte is below 0x10; a transfer type is `'J'`.
    #[test]
    fn a_compressed_reply_is_not_mistaken_for_an_ack() {
        let compressed = compress::encode("{\"id\":1,\"result\":true}").unwrap();
        assert!(
            compressed[0] < 0x10,
            "start nibble must be in the high half"
        );
        assert_ne!(compressed[0], TRANSFER_TYPE_JSON);
        assert!(decode_reply(&compressed).is_some());
    }
}
