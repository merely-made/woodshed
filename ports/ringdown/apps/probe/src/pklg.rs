//! Decode a PacketLogger capture of the vendor app talking to the guitar.
//!
//! Apple's PacketLogger writes `.pklg`: a flat sequence of records, each a
//! length, a timestamp, a one-byte type, then the raw HCI packet. This reads
//! that, reassembles the ACL fragments into L2CAP PDUs, keeps the ATT ones,
//! and turns every write to the guitar and every notification from it back
//! into the JSON the app and the firmware exchanged, using the same codec the
//! driver speaks. No Wireshark step, no export format to get right.
//!
//! The point of the exercise is one object: what the app sends when it
//! places a bank, which this project has not been able to guess (client
//! surface plan, Phase D). So the output is the whole conversation in order,
//! with the bank and effect writes pretty-printed where they occur.
//!
//! Format facts, from Wireshark's `packetlogger.c` rather than from Apple,
//! who do not document it: the record length counts the timestamp and type
//! bytes; timestamps are seconds and microseconds; the byte order of the
//! header varies between macOS versions and is detected from the first
//! record. Types 0x02 and 0x03 are ACL data sent and received; the rest is
//! HCI commands, events, and log text, none of which carry the protocol.

use std::collections::HashMap;

use ringdown::{compress, llt, llt2};

/// Read a capture and print the conversation.
pub fn decode_file(path: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("could not read {path}: {e}"))?;
    let records = read_records(&bytes)?;
    println!("{path}: {} records, {} bytes", records.len(), bytes.len());

    let mut acl = AclReassembler::default();
    let mut att = AttDecoder::default();
    let t0 = records.first().map(|r| r.ts).unwrap_or(0.0);

    for record in &records {
        let sent = match record.kind {
            0x02 => true,
            0x03 => false,
            _ => continue,
        };
        for pdu in acl.push(sent, &record.data) {
            att.handle(record.ts - t0, sent, &pdu);
        }
    }
    att.summary();
    Ok(())
}

struct Record {
    ts: f64,
    kind: u8,
    data: Vec<u8>,
}

/// Split the file into records, detecting the header byte order from the
/// first one: a big-endian read of a small little-endian length is enormous,
/// and vice versa, so whichever yields a length that fits the file wins.
fn read_records(bytes: &[u8]) -> Result<Vec<Record>, String> {
    if bytes.len() < 13 {
        return Err(String::from("too short to be a PacketLogger file"));
    }
    let be = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let le = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let plausible = |n: usize| (9..=bytes.len() - 4).contains(&n);
    let big_endian = match (plausible(be), plausible(le)) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => be <= le,
        (false, false) => return Err(String::from("first record length fits neither byte order")),
    };
    let u32_at = |i: usize| -> u32 {
        let b = [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]];
        if big_endian {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        }
    };

    let mut records = Vec::new();
    let mut i = 0;
    while i + 13 <= bytes.len() {
        let len = u32_at(i) as usize;
        if len < 9 || i + 4 + len > bytes.len() {
            eprintln!(
                "  stopped at byte {i}: record length {len} does not fit; {} records read",
                records.len()
            );
            break;
        }
        let secs = u32_at(i + 4) as f64;
        let usecs = u32_at(i + 8) as f64;
        let kind = bytes[i + 12];
        let data = bytes[i + 13..i + 4 + len].to_vec();
        records.push(Record {
            ts: secs + usecs / 1_000_000.0,
            kind,
            data,
        });
        i += 4 + len;
    }
    Ok(records)
}

/// Rebuild L2CAP PDUs from ACL fragments, per connection and direction.
#[derive(Default)]
struct AclReassembler {
    partial: HashMap<(u16, bool), (usize, Vec<u8>)>,
}

impl AclReassembler {
    fn push(&mut self, sent: bool, acl: &[u8]) -> Vec<Vec<u8>> {
        if acl.len() < 4 {
            return Vec::new();
        }
        let hf = u16::from_le_bytes([acl[0], acl[1]]);
        let handle = hf & 0x0fff;
        let pb = (hf >> 12) & 0b11;
        let len = u16::from_le_bytes([acl[2], acl[3]]) as usize;
        let payload = &acl[4..acl.len().min(4 + len)];
        let key = (handle, sent);

        // 0b01 is a continuing fragment; anything else starts a PDU.
        if pb == 0b01 {
            if let Some((_, buf)) = self.partial.get_mut(&key) {
                buf.extend_from_slice(payload);
            }
        } else {
            if payload.len() < 4 {
                return Vec::new();
            }
            let l2cap_len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
            self.partial.insert(key, (4 + l2cap_len, payload.to_vec()));
        }

        let mut out = Vec::new();
        if let Some((expected, buf)) = self.partial.get(&key)
            && buf.len() >= *expected
        {
            let (_, buf) = self.partial.remove(&key).unwrap();
            let cid = u16::from_le_bytes([buf[2], buf[3]]);
            if cid == 0x0004 {
                out.push(buf[4..].to_vec());
            }
        }
        out
    }
}

/// Turn ATT PDUs into the protocol's messages.
#[derive(Default)]
struct AttDecoder {
    /// Handle → name, learned from service discovery when the capture has it.
    names: HashMap<u16, &'static str>,
    /// Prepared (long) writes awaiting an execute, per handle.
    prepared: HashMap<u16, Vec<u8>>,
    /// LLT2 frames awaiting the rest of their message, per (sent, object id).
    frames: HashMap<(bool, u8), Vec<Vec<u8>>>,
    writes: usize,
    notifications: usize,
    /// Every decoded JSON message with its direction, for the summary.
    messages: Vec<(bool, String)>,
}

impl AttDecoder {
    fn handle(&mut self, t: f64, sent: bool, pdu: &[u8]) {
        let Some(&op) = pdu.first() else { return };
        let u16_at = |i: usize| u16::from_le_bytes([pdu[i], pdu[i + 1]]);
        match op {
            0x02 if pdu.len() >= 3 => {
                println!("[{t:9.3}] {} MTU request {}", arrow(sent), u16_at(1))
            }
            0x03 if pdu.len() >= 3 => {
                println!("[{t:9.3}] {} MTU response {}", arrow(sent), u16_at(1))
            }
            // Read By Type Response: characteristic declarations give us the
            // handle each UUID lives at, so writes and notifications can be
            // named rather than numbered.
            0x09 if pdu.len() >= 2 => {
                let each = pdu[1] as usize;
                if each == 21 {
                    for entry in pdu[2..].chunks_exact(each) {
                        let value_handle = u16::from_le_bytes([entry[3], entry[4]]);
                        let mut uuid = entry[5..21].to_vec();
                        uuid.reverse();
                        let text = uuid_text(&uuid);
                        if text == ringdown::GUITAR_CHARACTERISTIC_REQUEST {
                            self.names.insert(value_handle, "request");
                        } else if text == ringdown::GUITAR_CHARACTERISTIC_RESPONSE {
                            self.names.insert(value_handle, "response");
                        }
                    }
                }
            }
            // Read Response: the banner, read once at connect time.
            0x0b => {
                let text = String::from_utf8_lossy(&pdu[1..]);
                println!(
                    "[{t:9.3}] {} read response {:?}",
                    arrow(sent),
                    text.trim_end()
                );
            }
            // Write Request / Write Command.
            0x12 | 0x52 if pdu.len() >= 3 => {
                let handle = u16_at(1);
                self.writes += 1;
                self.payload(t, sent, handle, &pdu[3..]);
            }
            // Prepare Write: a long write in pieces, flushed by Execute Write.
            0x18 if pdu.len() >= 5 => {
                let handle = u16_at(1);
                let offset = u16_at(3) as usize;
                let buf = self.prepared.entry(handle).or_default();
                if buf.len() < offset + pdu.len() - 5 {
                    buf.resize(offset + pdu.len() - 5, 0);
                }
                buf[offset..offset + pdu.len() - 5].copy_from_slice(&pdu[5..]);
            }
            0x1a if pdu.len() >= 2 => {
                let flush = pdu[1] == 0x01;
                let prepared: Vec<(u16, Vec<u8>)> = self.prepared.drain().collect();
                if flush {
                    for (handle, value) in prepared {
                        self.writes += 1;
                        self.payload(t, sent, handle, &value);
                    }
                }
            }
            // Notification / Indication.
            0x1b | 0x1d if pdu.len() >= 3 => {
                let handle = u16_at(1);
                self.notifications += 1;
                self.payload(t, sent, handle, &pdu[3..]);
            }
            _ => {}
        }
    }

    fn payload(&mut self, t: f64, sent: bool, handle: u16, bytes: &[u8]) {
        let name = self
            .names
            .get(&handle)
            .map(|n| (*n).to_string())
            .unwrap_or_else(|| format!("0x{handle:04x}"));
        let tag = format!("[{t:9.3}] {} {name:>8} {:>4}B", arrow(sent), bytes.len());

        // A bare compressed message: the codec's start nibble is the test.
        if let Some(json) = compress::decode(bytes) {
            self.message(&tag, sent, json, "compressed");
            return;
        }
        // LLT2 binary: a six-byte ack, or a frame of a larger message.
        if bytes.first() == Some(&llt2::TRANSFER_TYPE_JSON) {
            if bytes.len() == llt2::ACK_LEN {
                match llt2::Ack2::parse(bytes) {
                    Some(ack) => println!(
                        "{tag}  llt2 ack: object {} frame {} code {:?}",
                        ack.object_id, ack.frame, ack.code
                    ),
                    None => println!("{tag}  llt2 ack? {}", hex(bytes)),
                }
                return;
            }
            if bytes.len() > 2 {
                let key = (sent, bytes[2]);
                let frames = self.frames.entry(key).or_default();
                frames.push(bytes.to_vec());
                let n = frames.len();
                if let Some(json) = llt2::reassemble(frames) {
                    self.frames.remove(&key);
                    self.message(&tag, sent, json, &format!("llt2, {n} frames"));
                } else {
                    println!("{tag}  llt2 frame {n} of object {}", bytes[2]);
                }
                return;
            }
        }
        // Plain text: LLT1 JSON, an LLT1 ack, or something new.
        if let Ok(text) = std::str::from_utf8(bytes) {
            if let Some(ack) = llt::Ack::parse(text) {
                println!(
                    "{tag}  llt ack: object {} frame {} code {:?}",
                    ack.object_id, ack.message_id, ack.code
                );
            } else {
                self.message(&tag, sent, text.to_string(), "plain");
            }
            return;
        }
        println!("{tag}  binary {}", hex(bytes));
    }

    fn message(&mut self, tag: &str, sent: bool, json: String, how: &str) {
        println!("{tag}  {how}: {json}");
        // The writes this capture exists for get a second, readable copy.
        if sent
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&json)
            && let Some(method) = value.get("method").and_then(|m| m.as_str())
            && matches!(
                method,
                "AddBank" | "SetConfig" | "AddEffect" | "UpdateEffect"
            )
            && let Ok(pretty) = serde_json::to_string_pretty(&value)
        {
            for line in pretty.lines() {
                println!("{:>26}{line}", "");
            }
        }
        self.messages.push((sent, json));
    }

    fn summary(&self) {
        println!(
            "\n{} writes, {} notifications, {} decoded messages",
            self.writes,
            self.notifications,
            self.messages.len()
        );
        let mut methods: Vec<String> = self
            .messages
            .iter()
            .filter(|(sent, _)| *sent)
            .filter_map(|(_, json)| serde_json::from_str::<serde_json::Value>(json).ok())
            .filter_map(|v| v.get("method").and_then(|m| m.as_str()).map(String::from))
            .collect();
        methods.sort();
        methods.dedup();
        if !methods.is_empty() {
            println!("methods the app sent: {}", methods.join(", "));
        }
        let pending: usize = self.frames.values().map(Vec::len).sum();
        if pending > 0 {
            println!("{pending} LLT2 frame(s) never completed a message");
        }
        if self.names.is_empty() {
            println!(
                "no service discovery in the capture, so handles are numbered; the guitar's \
                 request characteristic is the one every write goes to"
            );
        }
    }
}

fn arrow(sent: bool) -> &'static str {
    if sent { "->" } else { "<-" }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render 16 big-endian UUID bytes in the canonical hyphenated form.
fn uuid_text(be: &[u8]) -> String {
    let h: Vec<String> = be.iter().map(|b| format!("{b:02x}")).collect();
    let s = h.concat();
    format!(
        "{}-{}-{}-{}-{}",
        &s[0..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One record, big-endian header, built by hand: the parser must return
    /// the payload and the timestamp, and stop cleanly at the end.
    fn record(kind: u8, data: &[u8], big_endian: bool) -> Vec<u8> {
        let len = (9 + data.len()) as u32;
        let mut out = Vec::new();
        for v in [len, 1_700_000_000, 250_000] {
            out.extend_from_slice(&if big_endian {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            });
        }
        out.push(kind);
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn records_parse_in_either_byte_order() {
        for be in [true, false] {
            let mut file = record(0x02, &[1, 2, 3], be);
            file.extend(record(0x03, &[9], be));
            let records = read_records(&file).unwrap();
            assert_eq!(records.len(), 2);
            assert_eq!(records[0].kind, 0x02);
            assert_eq!(records[0].data, [1, 2, 3]);
            assert!((records[0].ts - 1_700_000_000.25).abs() < 1e-6);
            assert_eq!(records[1].data, [9]);
        }
    }

    /// A write split across two ACL fragments comes out as one ATT PDU.
    #[test]
    fn acl_fragments_reassemble_into_one_att_pdu() {
        let att = [0x52u8, 0x0e, 0x00, b'{', b'}'];
        let l2cap_len = att.len() as u16;
        let mut first = vec![0x40, 0x20]; // handle 0x40, PB = 0b10 (start)
        first.extend_from_slice(&(4u16 + 2).to_le_bytes()); // ACL len: L2CAP hdr + 2
        first.extend_from_slice(&l2cap_len.to_le_bytes());
        first.extend_from_slice(&0x0004u16.to_le_bytes());
        first.extend_from_slice(&att[..2]);
        let mut second = vec![0x40, 0x10]; // PB = 0b01 (continuation)
        second.extend_from_slice(&3u16.to_le_bytes());
        second.extend_from_slice(&att[2..]);

        let mut r = AclReassembler::default();
        assert!(r.push(true, &first).is_empty());
        let out = r.push(true, &second);
        assert_eq!(out, vec![att.to_vec()]);
    }

    #[test]
    fn a_plain_json_write_is_reported_as_a_message() {
        let mut d = AttDecoder::default();
        let mut pdu = vec![0x52, 0x0e, 0x00];
        pdu.extend_from_slice(br#"{"jsonrpc":2.0,"id":1,"method":"GetStatus","params":{}}"#);
        d.handle(0.0, true, &pdu);
        assert_eq!(d.writes, 1);
        assert_eq!(d.messages.len(), 1);
        assert!(d.messages[0].1.contains("GetStatus"));
    }

    #[test]
    fn uuids_render_canonically() {
        let bytes: Vec<u8> = (0..16).map(|i| i as u8 * 0x11).collect();
        assert_eq!(uuid_text(&bytes), "00112233-4455-6677-8899-aabbccddeeff");
    }

    /// A compressed write, as the vendor app really sends one under LLT2,
    /// decodes back to the JSON it was made from.
    #[test]
    fn a_compressed_write_decodes_through_the_codec() {
        let json = r#"{"jsonrpc":2.0,"id":7,"method":"AddBank","params":{"bank_num":8,"bank":{"name":"x","effects":[]}}}"#;
        let compressed = compress::encode(json).unwrap();
        let mut pdu = vec![0x12, 0x0e, 0x00];
        pdu.extend_from_slice(&compressed);
        let mut d = AttDecoder::default();
        d.handle(1.5, true, &pdu);
        assert_eq!(d.messages.len(), 1);
        let back: serde_json::Value = serde_json::from_str(&d.messages[0].1).unwrap();
        assert_eq!(back["method"], "AddBank");
        assert_eq!(back["params"]["bank"]["name"], "x");
    }

    /// A six-byte LLT2 ack is reported as an ack, not as a message.
    #[test]
    fn an_llt2_ack_is_not_mistaken_for_a_message() {
        let mut pdu = vec![0x1b, 0x11, 0x00];
        pdu.extend_from_slice(&[llt2::TRANSFER_TYPE_JSON, 0, 0x2a, 0x01, 0x00, 1]);
        let mut d = AttDecoder::default();
        d.handle(2.0, false, &pdu);
        assert_eq!(d.notifications, 1);
        assert!(d.messages.is_empty());
    }
}
