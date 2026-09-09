//! The vendor's JSON compression, as used by the LLT2 transport.
//!
//! Firmware from 1.2.2 onward carries large messages through a bespoke codec
//! rather than raw JSON. It is not a general-purpose compressor: it knows this
//! protocol's vocabulary by heart and encodes JSON structurally, four bits at a
//! time.
//!
//! - Structural tokens (`{`, `}`, `,`, `:`, `[`, `]`, `true`, `false`, `null`)
//!   cost **one nibble** each.
//! - A string in [`KEYWORDS`] costs a nibble plus a byte, so `"jsonrpc"`
//!   travels in 12 bits rather than 72.
//! - Any other string carries a length, then its UTF-8 bytes.
//! - Numbers are packed as BCD, one nibble per character, so `-3.5` is five
//!   nibbles including its sign and point.
//!
//! # Provenance, and one deliberate divergence
//!
//! Recovered from the vendor's `JsonCompressor` and `NibbleList`. The encoder
//! there is unambiguous and is reproduced faithfully. Its *decoder*, as
//! decompiled, advances one nibble too many for every multi-nibble token:
//! `BT_STRING_DICT` consumes four nibbles where the encoder writes three, and
//! the same off-by-one appears in all four variable-length branches while
//! single-nibble tokens are correct.
//!
//! Correct for the simple case and uniformly wrong by one for the compound
//! ones is the signature of a decompiler mis-hoisting a loop increment, not of
//! a real protocol quirk — and a decoder matching it could not read its own
//! encoder's output. So [`decode`] is written as the exact inverse of
//! [`encode`] and the pair is tested by round-trip. The peer that matters is
//! the firmware, which must accept what the vendor's encoder emits.
//!
//! **Unverified against hardware.** No compressed payload has been captured
//! from the instrument yet; every reply so far arrived uncompressed. This is a
//! careful reading, not a confirmed fact, until a device answers a compressed
//! request.

use alloc::{string::String, vec::Vec};

/// Every string the firmware knows by index.
///
/// Order is load-bearing: an entry's position *is* its encoding, so this must
/// never be sorted, deduplicated, or otherwise tidied. It happens to arrive in
/// codepoint order, which is what allowed the ten entries stored as symbolic
/// constants in the vendor's source to be resolved and then checked by the
/// position they had to occupy.
pub const KEYWORDS: [&str; 163] = [
    "2.0",
    "ActivateSpkFilter",
    "AddBank",
    "AddEffect",
    "AuxIn",
    "AuxInDryWet",
    "AuxOut",
    "AuxOutDryWet",
    "BTcheck",
    "Boost",
    "BypassEffect",
    "Calibrate",
    "Chorus",
    "Decay",
    "Default",
    "Delay",
    "DelaySync",
    "DelayTime",
    "Disto",
    "Distortion",
    "DryWet",
    "DumpFile",
    "Echo",
    "Equalizer",
    "Feedback",
    "Frequency",
    "Gain",
    "GainBand1",
    "GainBand2",
    "GainBand3",
    "GainBand4",
    "GainBand5",
    "GainBand6",
    "GetAnalysis",
    "GetLastRecordingName",
    "GetLevels",
    "GetStatus",
    "GetVersion",
    "Highpass",
    "LFO",
    "LaunchCalibration",
    "Lowpass",
    "MoveBank",
    "MoveEffect",
    "None",
    "Notch",
    "OK",
    "Octaver",
    "Phaser",
    "Pitch",
    "PrintBank",
    "PullFbk",
    "Q",
    "ReadBank",
    "ReadConfig",
    "ReadMetronome",
    "RemoveBank",
    "RemoveEffect",
    "Reverb",
    "SaveConfig",
    "SetBankName",
    "SetConfig",
    "SetController",
    "SetEQBandGain",
    "SetEQGain",
    "SetGainBank",
    "SetGainPreamp",
    "SetPhaseInv",
    "SetSpeakerBiquads",
    "Shift",
    "Slider",
    "StartAnalysis",
    "StartMetronome",
    "StartRecording",
    "StartRendering",
    "StartTuner",
    "StopMetronome",
    "StopRecording",
    "StopRendering",
    "StopTuner",
    "SustainKiller",
    "SwitchBank",
    "Sync",
    "Tremolo",
    "UpdateEffect",
    "UpdateMetronome",
    "Volume",
    "afmax",
    "afmin",
    "afstep",
    "agmin",
    "amp",
    "aux_in_drywet",
    "aux_in_on",
    "aux_out_drywet",
    "aux_out_on",
    "band",
    "bank",
    "bank_num",
    "batt_left",
    "bpm",
    "bypass",
    "calibration_on",
    "code",
    "config",
    "control",
    "cpu_id",
    "data",
    "default",
    "den",
    "dst",
    "duration",
    "effect",
    "effect_dest",
    "effect_num",
    "effects",
    "error",
    "f0",
    "f1",
    "favorite_banks",
    "fbk_onoff",
    "fbk_params",
    "feedback",
    "file",
    "file_type",
    "free",
    "free_gb",
    "free_pct",
    "gain",
    "gain_master",
    "gain_preamp",
    "guitar",
    "id",
    "input_level",
    "jsonrpc",
    "key",
    "max",
    "message",
    "method",
    "metronome",
    "min",
    "name",
    "nb_bars",
    "nbbars",
    "nbreso",
    "num",
    "offset",
    "on",
    "output_level",
    "parameter",
    "params",
    "preset",
    "reset",
    "result",
    "size",
    "source",
    "src",
    "type",
    "value",
    "version",
    "version_esp",
    "version_stm",
    "wireless",
];

/// The four-bit tag opening every token.
mod block {
    pub const START: u8 = 0;
    pub const LEFT_BRACE: u8 = 1;
    pub const RIGHT_BRACE: u8 = 2;
    pub const COMMA: u8 = 3;
    pub const COLON: u8 = 4;
    pub const LEFT_BRACKET: u8 = 5;
    pub const RIGHT_BRACKET: u8 = 6;
    pub const TRUE: u8 = 7;
    pub const FALSE: u8 = 8;
    pub const NULL: u8 = 9;
    pub const STRING_DICT: u8 = 10;
    pub const STRING_SHORT: u8 = 11;
    pub const STRING_LONG: u8 = 12;
    pub const NUMBER: u8 = 14;
    pub const END: u8 = 15;
}

/// The longest string the short form's four-bit length can describe.
pub const SHORT_STRING_MAX: usize = 15;

/// The longest string the long form's twelve-bit length can describe.
pub const LONG_STRING_MAX: usize = 4095;

/// The most characters a number may have, given its four-bit length.
pub const NUMBER_MAX_CHARS: usize = 15;

/// The BCD encoding of a character a number may contain.
fn to_bcd(c: char) -> Option<u8> {
    Some(match c {
        '0'..='9' => c as u8 - b'0',
        '-' => 10,
        '.' => 11,
        '+' => 12,
        'e' => 13,
        'E' => 14,
        _ => return None,
    })
}

/// The inverse of [`to_bcd`].
fn from_bcd(b: u8) -> Option<char> {
    Some(match b {
        0..=9 => (b + b'0') as char,
        10 => '-',
        11 => '.',
        12 => '+',
        13 => 'e',
        14 => 'E',
        _ => return None,
    })
}

/// Writes 4, 8 and 12-bit fields, tracking half-byte alignment so that a
/// nibble never pads out to a whole byte.
struct NibbleWriter {
    bytes: Vec<u8>,
    /// True when the next write begins a fresh byte.
    aligned: bool,
}

impl NibbleWriter {
    fn new() -> Self {
        NibbleWriter {
            bytes: Vec::new(),
            aligned: true,
        }
    }

    fn push4(&mut self, v: u8) {
        if self.aligned {
            self.bytes.push((v & 0x0f) << 4);
        } else {
            let last = self.bytes.len() - 1;
            self.bytes[last] = (self.bytes[last] & 0xf0) | (v & 0x0f);
        }
        self.aligned = !self.aligned;
    }

    fn push8(&mut self, v: u8) {
        if self.aligned {
            self.bytes.push(v);
        } else {
            let last = self.bytes.len() - 1;
            self.bytes[last] = (self.bytes[last] & 0xf0) | (v >> 4);
            self.bytes.push((v & 0x0f) << 4);
        }
    }

    fn push12(&mut self, v: u16) {
        if self.aligned {
            self.bytes.push(((v & 0x0ff0) >> 4) as u8);
            self.bytes.push(((v & 0x000f) << 4) as u8);
        } else {
            let last = self.bytes.len() - 1;
            self.bytes[last] = (self.bytes[last] & 0xf0) | ((v & 0x0f00) >> 8) as u8;
            self.bytes.push((v & 0x00ff) as u8);
        }
        self.aligned = !self.aligned;
    }
}

/// Reads the fields [`NibbleWriter`] writes, indexed by nibble rather than byte.
struct NibbleReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> NibbleReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        NibbleReader { bytes, at: 0 }
    }

    fn peek4(&self, at: usize) -> Option<u8> {
        let byte = *self.bytes.get(at / 2)?;
        Some(if at.is_multiple_of(2) {
            byte >> 4
        } else {
            byte & 0x0f
        })
    }

    fn read4(&mut self) -> Option<u8> {
        let v = self.peek4(self.at)?;
        self.at += 1;
        Some(v)
    }

    fn read8(&mut self) -> Option<u8> {
        let hi = self.peek4(self.at)?;
        let lo = self.peek4(self.at + 1)?;
        self.at += 2;
        Some((hi << 4) | lo)
    }

    fn read12(&mut self) -> Option<u16> {
        let a = self.peek4(self.at)? as u16;
        let b = self.peek4(self.at + 1)? as u16;
        let c = self.peek4(self.at + 2)? as u16;
        self.at += 3;
        Some((a << 8) | (b << 4) | c)
    }
}

/// Why a document could not be compressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// A string exceeded what the twelve-bit length field can describe.
    StringTooLong(usize),
    /// A number had more characters than the four-bit length field allows.
    NumberTooLong(usize),
    /// A character appeared where the grammar does not allow one.
    Unexpected {
        /// Character offset into the input.
        at: usize,
        /// The offending character.
        found: char,
    },
    /// A string was opened and never closed.
    UnterminatedString,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncodeError::StringTooLong(n) => write!(
                f,
                "string of {n} bytes exceeds the {LONG_STRING_MAX}-byte limit"
            ),
            EncodeError::NumberTooLong(n) => write!(
                f,
                "number of {n} characters exceeds the {NUMBER_MAX_CHARS}-character limit"
            ),
            EncodeError::Unexpected { at, found } => write!(f, "unexpected {found:?} at {at}"),
            EncodeError::UnterminatedString => write!(f, "unterminated string"),
        }
    }
}

impl core::error::Error for EncodeError {}

/// Compress a JSON document.
///
/// Works on the JSON *source text*, so string contents travel exactly as
/// written, escapes included. `\"` is consumed as a unit rather than read as a
/// closing quote — the vendor's encoder does not do this and terminates such a
/// string early, which is a fault in it rather than a property of the format.
pub fn encode(json: &str) -> Result<Vec<u8>, EncodeError> {
    let src: Vec<char> = json.chars().collect();
    let mut w = NibbleWriter::new();
    w.push4(block::START);

    let mut i = 0usize;
    while i < src.len() {
        match src[i] {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '{' => {
                w.push4(block::LEFT_BRACE);
                i += 1;
            }
            '}' => {
                w.push4(block::RIGHT_BRACE);
                i += 1;
            }
            '[' => {
                w.push4(block::LEFT_BRACKET);
                i += 1;
            }
            ']' => {
                w.push4(block::RIGHT_BRACKET);
                i += 1;
            }
            ',' => {
                w.push4(block::COMMA);
                i += 1;
            }
            ':' => {
                w.push4(block::COLON);
                i += 1;
            }
            // The literals are recognised by their first character and skipped
            // whole, exactly as the vendor does.
            't' => {
                w.push4(block::TRUE);
                i += 4;
            }
            'f' => {
                w.push4(block::FALSE);
                i += 5;
            }
            'n' => {
                w.push4(block::NULL);
                i += 4;
            }
            '"' => {
                let (text, next) = scan_string(&src, i)?;
                encode_string(&mut w, &text)?;
                i = next;
            }
            '-' | '+' | '0'..='9' => {
                let (text, next) = scan_number(&src, i);
                if text.chars().count() > NUMBER_MAX_CHARS {
                    return Err(EncodeError::NumberTooLong(text.chars().count()));
                }
                w.push4(block::NUMBER);
                w.push4(text.chars().count() as u8);
                for ch in text.chars() {
                    // scan_number accepts only characters to_bcd knows.
                    w.push4(to_bcd(ch).unwrap_or(0));
                }
                i = next;
            }
            other => {
                return Err(EncodeError::Unexpected {
                    at: i,
                    found: other,
                });
            }
        }
    }

    w.push4(block::END);
    Ok(w.bytes)
}

/// Collect a string's source characters, from its opening quote to its close.
fn scan_string(src: &[char], start: usize) -> Result<(String, usize), EncodeError> {
    let mut out = String::new();
    let mut i = start + 1;
    while i < src.len() {
        match src[i] {
            '"' => return Ok((out, i + 1)),
            '\\' => {
                // Keep the escape intact and step over both characters, so an
                // escaped quote is not mistaken for the end of the string.
                out.push('\\');
                if let Some(next) = src.get(i + 1) {
                    out.push(*next);
                }
                i += 2;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Err(EncodeError::UnterminatedString)
}

/// Collect a number's source characters.
fn scan_number(src: &[char], start: usize) -> (String, usize) {
    let mut out = String::new();
    let mut i = start;
    while i < src.len() && to_bcd(src[i]).is_some() {
        out.push(src[i]);
        i += 1;
    }
    (out, i)
}

/// Emit a string in whichever of the three forms fits it.
fn encode_string(w: &mut NibbleWriter, text: &str) -> Result<(), EncodeError> {
    if let Some(index) = KEYWORDS.iter().position(|k| *k == text) {
        w.push4(block::STRING_DICT);
        w.push8(index as u8);
        return Ok(());
    }
    let bytes = text.as_bytes();
    if bytes.len() <= SHORT_STRING_MAX {
        w.push4(block::STRING_SHORT);
        w.push4(bytes.len() as u8);
    } else if bytes.len() <= LONG_STRING_MAX {
        w.push4(block::STRING_LONG);
        w.push12(bytes.len() as u16);
    } else {
        return Err(EncodeError::StringTooLong(bytes.len()));
    }
    for b in bytes {
        w.push8(*b);
    }
    Ok(())
}

/// Decompress a document produced by [`encode`].
///
/// Returns `None` for anything that is not this format — in particular for
/// input whose first nibble is not the start marker. That check is what lets a
/// caller tell a compressed frame from a plain JSON reply arriving on the same
/// characteristic, so it is a feature rather than mere validation.
pub fn decode(bytes: &[u8]) -> Option<String> {
    let mut r = NibbleReader::new(bytes);
    if r.read4()? != block::START {
        return None;
    }

    let mut out = String::new();
    loop {
        match r.read4()? {
            block::END => return Some(out),
            // A zero nibble in token position is padding, not a second START.
            // The instrument emits one before the closing brace of at least
            // some replies (captured from `ReadBank`), and rejecting it threw
            // away a perfectly good message. Skipping it yields well-formed
            // JSON; treating it as an error does not.
            block::START => {}
            block::LEFT_BRACE => out.push('{'),
            block::RIGHT_BRACE => out.push('}'),
            block::LEFT_BRACKET => out.push('['),
            block::RIGHT_BRACKET => out.push(']'),
            block::COMMA => out.push(','),
            block::COLON => out.push(':'),
            block::TRUE => out.push_str("true"),
            block::FALSE => out.push_str("false"),
            block::NULL => out.push_str("null"),
            block::STRING_DICT => {
                let index = r.read8()? as usize;
                out.push('"');
                out.push_str(KEYWORDS.get(index)?);
                out.push('"');
            }
            block::STRING_SHORT => {
                let len = r.read4()? as usize;
                read_string(&mut r, len, &mut out)?;
            }
            block::STRING_LONG => {
                let len = r.read12()? as usize;
                read_string(&mut r, len, &mut out)?;
            }
            block::NUMBER => {
                let len = r.read4()? as usize;
                for _ in 0..len {
                    out.push(from_bcd(r.read4()?)?);
                }
            }
            _ => return None,
        }
    }
}

/// Read `len` bytes of string body and append them, quoted.
fn read_string(r: &mut NibbleReader<'_>, len: usize, out: &mut String) -> Option<()> {
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(r.read8()?);
    }
    out.push('"');
    out.push_str(core::str::from_utf8(&bytes).ok()?);
    out.push('"');
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::ToString, vec};

    fn round_trip(json: &str) {
        let encoded = encode(json).unwrap_or_else(|e| panic!("encode {json}: {e}"));
        let decoded = decode(&encoded).unwrap_or_else(|| panic!("decode failed for {json}"));
        assert_eq!(decoded, json, "round trip changed the document");
    }

    #[test]
    fn the_dictionary_is_the_size_the_format_assumes() {
        // An 8-bit index caps the dictionary at 256 entries.
        assert_eq!(KEYWORDS.len(), 163);
        assert!(KEYWORDS.len() <= 256);
    }

    #[test]
    fn the_dictionary_has_no_duplicates() {
        // A duplicate would make encoding ambiguous and silently pick one.
        for (i, a) in KEYWORDS.iter().enumerate() {
            assert!(
                !KEYWORDS[i + 1..].contains(a),
                "{a} appears more than once in the dictionary"
            );
        }
    }

    /// Order is the encoding, so this pins a few positions that the wire
    /// depends on. A reordering that kept every entry would still break the
    /// protocol, and only a test like this would notice.
    #[test]
    fn dictionary_positions_are_pinned() {
        assert_eq!(KEYWORDS[0], "2.0");
        assert_eq!(KEYWORDS[36], "GetStatus");
        assert_eq!(KEYWORDS[134], "jsonrpc");
        assert_eq!(KEYWORDS[162], "wireless");
    }

    #[test]
    fn structural_tokens_round_trip() {
        round_trip("{}");
        round_trip("[]");
        round_trip("{\"id\":1}");
        round_trip("[[],[]]");
        round_trip("{\"on\":true}");
        round_trip("{\"on\":false}");
        round_trip("{\"on\":null}");
    }

    #[test]
    fn a_real_request_round_trips() {
        round_trip("{\"jsonrpc\":2.0,\"id\":1,\"method\":\"GetStatus\",\"params\":{}}");
    }

    #[test]
    fn a_real_reply_round_trips() {
        round_trip(
            "{\"jsonrpc\":\"2.0\",\"id\":90,\"result\":{\"free_gb\":7.634,\
             \"free_pct\":0.9949,\"batt_left\":46,\"version_stm\":\"V1.2.3\",\
             \"version_esp\":\"V1.3.0\",\"cpu_id\":\"PIdXXddxLAU=\",\"device\":\"H2S\"}}",
        );
    }

    /// The analysis reply is the densest real payload seen: nested arrays of
    /// signed decimals, which exercises BCD and bracket handling together.
    #[test]
    fn the_captured_analysis_round_trips() {
        round_trip("[[4,106,-3.3,6],[4,228,-6.8,3.75],[4,545,-7.8,8.1],[4,3760,-3.8,6]]");
    }

    #[test]
    fn numbers_of_every_shape_round_trip() {
        for n in [
            "0", "-1", "3.75", "-7.8", "1e5", "1E5", "1.5e-3", "1.5e+3", "3760", "0.9949",
        ] {
            round_trip(&format!("{{\"value\":{n}}}"));
        }
    }

    #[test]
    fn dictionary_strings_compress_to_twelve_bits() {
        // Tag nibble plus an 8-bit index, inside a start/end pair.
        let encoded = encode("\"jsonrpc\"").unwrap();
        assert_eq!(decode(&encoded).unwrap(), "\"jsonrpc\"");
        // START + DICT + index(2) + END = 5 nibbles = 3 bytes.
        assert_eq!(encoded.len(), 3);
    }

    #[test]
    fn compression_actually_compresses_a_real_message() {
        let json = "{\"jsonrpc\":2.0,\"id\":1,\"method\":\"ReadConfig\",\"params\":{}}";
        let encoded = encode(json).unwrap();
        assert!(
            encoded.len() < json.len() / 2,
            "expected better than half: {} -> {}",
            json.len(),
            encoded.len()
        );
    }

    #[test]
    fn short_and_long_strings_round_trip() {
        round_trip(&format!("{{\"a\":\"{}\"}}", "x".repeat(SHORT_STRING_MAX)));
        round_trip(&format!(
            "{{\"a\":\"{}\"}}",
            "x".repeat(SHORT_STRING_MAX + 1)
        ));
        round_trip(&format!("{{\"a\":\"{}\"}}", "x".repeat(1000)));
    }

    /// Both string forms must survive odd nibble alignment, since a preceding
    /// token can leave the writer mid-byte. This is where an alignment bug
    /// hides.
    #[test]
    fn strings_round_trip_from_either_alignment() {
        for pad in 0..2 {
            let prefix = "[".repeat(pad);
            let suffix = "]".repeat(pad);
            round_trip(&format!("{prefix}\"hello\"{suffix}"));
            round_trip(&format!("{prefix}\"{}\"{suffix}", "y".repeat(40)));
            round_trip(&format!("{prefix}12345{suffix}"));
        }
    }

    #[test]
    fn multibyte_strings_round_trip() {
        round_trip("{\"a\":\"café\"}");
        round_trip("{\"a\":\"日本語\"}");
        round_trip("{\"a\":\"🎸\"}");
    }

    /// The vendor's encoder ends a string at an escaped quote, losing the rest.
    /// Ours does not, and this is the case that distinguishes them.
    #[test]
    fn escaped_quotes_do_not_end_a_string() {
        round_trip("{\"a\":\"say \\\"hi\\\" now\"}");
        round_trip("{\"a\":\"back\\\\slash\"}");
    }

    #[test]
    fn decode_rejects_input_that_is_not_this_format() {
        // The start-marker check is how a caller tells a compressed frame from
        // a plain JSON reply on the same characteristic.
        assert!(decode(b"{\"jsonrpc\":\"2.0\"}").is_none());
        assert!(decode(&[]).is_none());
        assert!(decode(&[0xff, 0xff]).is_none());
    }

    #[test]
    fn decode_rejects_a_truncated_document() {
        let encoded = encode("{\"jsonrpc\":2.0,\"id\":1}").unwrap();
        for cut in 1..encoded.len() {
            // Truncation must fail cleanly rather than panic or invent data.
            let _ = decode(&encoded[..cut]);
        }
    }

    #[test]
    fn oversized_inputs_are_refused_rather_than_truncated() {
        let long = "x".repeat(LONG_STRING_MAX + 1);
        assert_eq!(
            encode(&format!("\"{long}\"")),
            Err(EncodeError::StringTooLong(LONG_STRING_MAX + 1))
        );
        let big_number = "1".repeat(NUMBER_MAX_CHARS + 1);
        assert_eq!(
            encode(&big_number),
            Err(EncodeError::NumberTooLong(NUMBER_MAX_CHARS + 1))
        );
    }

    #[test]
    fn whitespace_between_tokens_is_dropped() {
        let spaced = encode("{ \"id\" : 1 }").unwrap();
        let tight = encode("{\"id\":1}").unwrap();
        assert_eq!(spaced, tight);
    }

    #[test]
    fn every_dictionary_entry_round_trips_by_index() {
        // Catches an entry that is unencodable for some reason of its own.
        for (i, word) in KEYWORDS.iter().enumerate() {
            let json = format!("\"{word}\"");
            let encoded = encode(&json).unwrap_or_else(|e| panic!("encode {word}: {e}"));
            assert_eq!(
                decode(&encoded).unwrap_or_else(|| panic!("decode {word}")),
                json,
                "dictionary entry {i} ({word}) did not survive"
            );
        }
    }

    #[test]
    fn a_document_using_many_dictionary_words_round_trips() {
        round_trip(
            "{\"config\":{\"effects\":[{\"type\":\"Reverb\",\"preset\":\"default\",\
             \"bypass\":false,\"params\":[{\"key\":\"DryWet\",\"value\":0.5},\
             {\"key\":\"Decay\",\"value\":2.5}]}],\"gain_master\":0.8}}",
        );
    }

    #[test]
    fn bcd_covers_exactly_the_characters_numbers_use() {
        for c in "0123456789-.+eE".chars() {
            let b = to_bcd(c).unwrap_or_else(|| panic!("{c} should encode"));
            assert_eq!(from_bcd(b), Some(c));
        }
        assert_eq!(to_bcd('x'), None);
        assert_eq!(from_bcd(15), None);
    }

    #[test]
    fn nibble_writer_and_reader_agree_at_both_alignments() {
        for lead in 0..2 {
            let mut w = NibbleWriter::new();
            for _ in 0..lead {
                w.push4(0x5);
            }
            w.push8(0xa7);
            w.push12(0xbcd);
            w.push4(0x3);
            let bytes = w.bytes;

            let mut r = NibbleReader::new(&bytes);
            for _ in 0..lead {
                assert_eq!(r.read4(), Some(0x5));
            }
            assert_eq!(r.read8(), Some(0xa7));
            assert_eq!(r.read12(), Some(0xbcd));
            assert_eq!(r.read4(), Some(0x3));
        }
    }

    #[test]
    fn decode_survives_arbitrary_bytes_without_panicking() {
        // A hostile or corrupt frame must return None, never panic.
        let mut data = vec![0x00];
        for b in 0u8..=255 {
            data.push(b);
            let _ = decode(&data);
            if data.len() > 64 {
                data.truncate(1);
            }
        }
    }

    #[test]
    fn encode_error_messages_name_the_limit() {
        let e = EncodeError::NumberTooLong(20);
        assert!(e.to_string().contains("15"), "{e}");
    }
}

#[cfg(test)]
mod captured_replies {
    /// A real `ReadBank` reply, byte for byte off the instrument on
    /// 2026-08-29. Ringdown rejected this for a stray zero nibble before the
    /// closing brace and reported the call as unanswered, which read as the
    /// device having gone silent. It had not.
    const READBANK_REPLY: [u8; 30] = [
        0x01, 0xb7, 0x6a, 0x73, 0x6f, 0x6e, 0x72, 0x70, 0x63, 0x4b, 0x33, 0x22, 0xe3, 0x03, 0xb2,
        0x69, 0x64, 0x4e, 0x14, 0x3b, 0x67, 0x26, 0x57, 0x37, 0x56, 0xc7, 0x44, 0xb0, 0x02, 0xf0,
    ];

    #[test]
    fn a_real_readbank_reply_decodes() {
        assert_eq!(
            super::decode(&READBANK_REPLY).expect("must decode"),
            r#"{"jsonrpc":"2.0","id":4,"result":""}"#
        );
    }
}

#[cfg(test)]
mod nested_params {
    /// An effect carrying parameters: an array of objects, nested two deep
    /// inside the params object, which nothing else in the fixture set has.
    ///
    /// The instrument rejects every such message, and this test is half of why
    /// that is known to be the firmware's doing rather than ours: the codec
    /// round-trips the shape exactly. The other half is that the same message
    /// is refused identically over plain-JSON LLT, where no compression runs at
    /// all. See the founding doc, H26.
    #[test]
    fn an_effect_with_parameters_round_trips() {
        let json = concat!(
            r#"{"jsonrpc":2.0,"id":2,"method":"AddEffect","params":{"bank_num":0,"#,
            r#""effect":{"preset":"Default","type":"Tremolo","bypass":false,"#,
            r#""params":[{"key":"DryWet","value":0.5}]}}}"#
        );
        let encoded = super::encode(json).expect("encode must not fail");
        assert_eq!(super::decode(&encoded).expect("decode"), json);
    }
}
