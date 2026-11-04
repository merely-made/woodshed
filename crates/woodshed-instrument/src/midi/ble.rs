// SPDX-License-Identifier: MPL-2.0
//! Standard short-message BLE-MIDI transport. Native GATT lives in the supplier.
use super::diagnostics;
use std::{fmt, time::Duration};

pub const SERVICE_UUID: &str = "03b80e5a-ede8-4b33-a751-6ce34ec4c700";
pub const IO_UUID: &str = "7772e5db-3868-4112-a1a9-f2669d106bf3";
/// A selectable transport in the existing output picker. No radio is opened by listing it.
pub const OUTPUT_NAME: &str = "Bluetooth MIDI pedal";
pub const COMPAT_OUTPUT_NAME: &str = "Bluetooth MIDI pedal (no encryption)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Encrypted,
    /// Explicit owner choice for receivers that did not negotiate encryption.
    UnencryptedCompatibility,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub security: Security,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Ringdown".into(),
            security: Security::Encrypted,
        }
    }
}

impl Config {
    fn validate(&self) -> Result<(), Error> {
        // Flags + 128-bit service + name must fit a legacy 31-byte advertisement.
        if self.name.is_empty()
            || self.name.len() > 8
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::Invalid(
                "BLE name must be 1-8 ASCII letters, digits, '-' or '_'".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Advertising,
    Subscribed,
    Unavailable,
    Closed,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub phase: Phase,
    pub security: Security,
    pub subscribers: usize,
    pub generation: u64,
    pub accepted_messages: u64,
    pub received_messages: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Error {
    Invalid(String),
    NotSubscribed,
    WouldBlock,
    Expired,
    /// The OS operation started but its completion was not observed. Do not replay.
    Unknown,
    Transport(String),
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(s) => write!(f, "invalid BLE-MIDI message/configuration: {s}"),
            Self::NotSubscribed => write!(f, "BLE-MIDI requires exactly one current subscriber"),
            Self::WouldBlock => write!(
                f,
                "BLE-MIDI notification queue is full; message was not queued for retry"
            ),
            Self::Expired => write!(f, "BLE-MIDI command expired before sending"),
            Self::Unknown => write!(
                f,
                "BLE-MIDI send outcome is unknown; do not replay automatically"
            ),
            Self::Transport(s) => write!(f, "BLE-MIDI transport: {s}"),
            Self::Unsupported => write!(
                f,
                "BLE-MIDI peripheral role is unavailable on this platform"
            ),
        }
    }
}
impl std::error::Error for Error {}

/// One complete short MIDI message per notification; no running status, SysEx
/// or stream splitting. Maximum packet is five bytes, below the ATT minimum.
pub fn encode_short_message(message: &[u8], elapsed_millis: u64) -> Result<Vec<u8>, Error> {
    let expected = match message.first().copied() {
        Some(0x80..=0xbf | 0xe0..=0xef | 0xf2) => 3,
        Some(0xc0..=0xdf | 0xf1 | 0xf3) => 2,
        Some(0xf6 | 0xf8 | 0xfa..=0xfc | 0xfe | 0xff) => 1,
        _ => {
            return Err(Error::Invalid(
                "expected a complete short MIDI message".into(),
            ));
        },
    };
    if message.len() != expected || message[1..].iter().any(|byte| *byte >= 0x80) {
        return Err(Error::Invalid(
            "incorrect message length or non-data byte".into(),
        ));
    }
    let timestamp = elapsed_millis & 0x1fff;
    let mut packet = Vec::with_capacity(expected + 2);
    packet.push(0x80 | ((timestamp >> 7) as u8 & 0x3f));
    packet.push(0x80 | (timestamp as u8 & 0x7f));
    packet.extend_from_slice(message);
    Ok(packet)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod native;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) use native::Output;
