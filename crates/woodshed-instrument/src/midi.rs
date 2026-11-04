//! MIDI input/output.
//!
//! Two layers:
//!
//! - **Pure helpers** ([`MidiClockSync`], event parsing) — testable
//!   without hardware. These encode the algorithms that matter
//!   (BPM derivation from clock ticks, byte-stream → typed events).
//! - **I/O wrappers** ([`MidiOut`], [`MidiIn`]) — thin adapters over
//!   `midir` ports, with an optional standard BLE peripheral variant on the
//!   same output writer. Open a named connection, send/receive bytes, hand
//!   parsed events to a callback.
//!
//! See `design_docs/2026-05-15_midi_design.md` for the full design
//! note covering transport ownership and what's out of scope.
//!
//! # Why we don't trigger sounds from MIDI Note In
//!
//! Woodshed is a practice tool, not a synth. Note On / Off events
//! from a controller are visible via [`MidiEvent::NoteOn`] /
//! [`MidiEvent::NoteOff`] for app-level use (e.g. a future "tap-tempo
//! by MIDI pedal" feature), but the audio engine itself doesn't
//! consume them.

#[cfg(feature = "ble-midi")]
pub mod ble;
pub mod diagnostics;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use midir::{
    Ignore, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};

/// Errors that can come out of the MIDI layer.
#[derive(Debug)]
pub enum MidiError {
    /// Failed to construct the MIDI client (driver missing, OS
    /// permissions, etc.).
    Init(midir::InitError),
    /// Couldn't find a port matching the requested name. The string
    /// inside is the requested name for debugging.
    PortNotFound(String),
    Connect(String),
    Send(midir::SendError),
    #[cfg(feature = "ble-midi")]
    Bluetooth(ble::Error),
}

impl std::fmt::Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Init(e) => write!(f, "midi init: {e}"),
            Self::PortNotFound(name) => write!(f, "midi port not found: {name}"),
            Self::Connect(s) => write!(f, "midi connect: {s}"),
            Self::Send(e) => write!(f, "midi send: {e}"),
            #[cfg(feature = "ble-midi")]
            Self::Bluetooth(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for MidiError {}

impl From<midir::InitError> for MidiError {
    fn from(e: midir::InitError) -> Self {
        Self::Init(e)
    }
}

impl From<midir::SendError> for MidiError {
    fn from(e: midir::SendError) -> Self {
        Self::Send(e)
    }
}

// === Constants for MIDI System Realtime / Channel messages we care about ===

/// MIDI realtime: clock tick. 24 per quarter note.
pub const CLOCK_TICK: u8 = 0xF8;
/// MIDI realtime: start playback from beat 1.
pub const TRANSPORT_START: u8 = 0xFA;
/// MIDI realtime: continue playback from current position.
pub const TRANSPORT_CONTINUE: u8 = 0xFB;
/// MIDI realtime: stop playback.
pub const TRANSPORT_STOP: u8 = 0xFC;

// === Typed events ===

/// Parsed MIDI events the rest of the app cares about. Channel
/// messages carry a 0-based channel (0..16) — different from the
/// 1-based notation common in DAW UIs.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MidiEvent {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    Clock,
    Start,
    Continue,
    Stop,
}

/// Parse a single MIDI message (a raw byte slice straight from
/// `midir`'s callback). Returns `None` for messages we don't model.
///
/// Realtime System messages can be interleaved into the middle of a
/// channel message stream in MIDI 1.0; `midir` already splits them
/// for us, so each callback delivers exactly one logical message.
pub fn parse_message(bytes: &[u8]) -> Option<MidiEvent> {
    if bytes.is_empty() {
        return None;
    }
    let status = bytes[0];
    // System Realtime messages: single byte, no data.
    match status {
        CLOCK_TICK => return Some(MidiEvent::Clock),
        TRANSPORT_START => return Some(MidiEvent::Start),
        TRANSPORT_CONTINUE => return Some(MidiEvent::Continue),
        TRANSPORT_STOP => return Some(MidiEvent::Stop),
        _ => {},
    }
    // Channel messages: 0x80..=0xEF with channel in low nibble.
    if !(0x80..=0xEF).contains(&status) {
        return None;
    }
    let channel = status & 0x0F;
    let kind = status & 0xF0;
    match kind {
        0x80 => {
            // Note Off
            let note = *bytes.get(1)?;
            let velocity = *bytes.get(2)?;
            if note > 127 || velocity > 127 {
                return None;
            }
            Some(MidiEvent::NoteOff {
                channel,
                note,
                velocity,
            })
        },
        0x90 => {
            // Note On — by convention, velocity 0 = Note Off.
            let note = *bytes.get(1)?;
            let velocity = *bytes.get(2)?;
            if note > 127 || velocity > 127 {
                return None;
            }
            if velocity == 0 {
                Some(MidiEvent::NoteOff {
                    channel,
                    note,
                    velocity,
                })
            } else {
                Some(MidiEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                })
            }
        },
        0xB0 => {
            // Control Change
            let controller = *bytes.get(1)?;
            let value = *bytes.get(2)?;
            if controller > 127 || value > 127 {
                return None;
            }
            Some(MidiEvent::ControlChange {
                channel,
                controller,
                value,
            })
        },
        _ => None,
    }
}

// === Pure clock-sync helper ===

/// Derive BPM from a stream of incoming MIDI Clock tick timestamps.
///
/// MIDI Clock is 24 ticks per quarter note. We average inter-tick
/// intervals over the most recent N ticks and smooth across calls
/// with a small exponential moving average so a host that jitters
/// by a few ms doesn't make our reported BPM oscillate.
#[derive(Clone, Debug)]
pub struct MidiClockSync {
    /// Ticks retained for averaging. ~24 = one beat of history.
    history: VecDeque<Instant>,
    /// History cap.
    capacity: usize,
    /// Exponential smoothing factor (0 = no smoothing, 1 = freeze).
    /// 0.3 = lean toward new measurements but soak some jitter.
    smoothing: f32,
    /// Most recent smoothed BPM estimate, `None` until enough ticks.
    smoothed_bpm: Option<f32>,
}

impl Default for MidiClockSync {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(24),
            capacity: 24,
            smoothing: 0.3,
            smoothed_bpm: None,
        }
    }
}

impl MidiClockSync {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop all history. Use when leaving / re-entering clock-slave
    /// mode.
    pub fn reset(&mut self) {
        self.history.clear();
        self.smoothed_bpm = None;
    }

    /// Record one incoming MIDI clock tick at the given wall-clock
    /// instant. Updates the smoothed BPM estimate as a side effect.
    pub fn record_tick(&mut self, at: Instant) {
        if self.history.len() >= self.capacity {
            self.history.pop_front();
        }
        self.history.push_back(at);
        self.recompute();
    }

    fn recompute(&mut self) {
        if self.history.len() < 2 {
            return;
        }
        // Mean of inter-tick intervals.
        let intervals: Vec<f32> = self
            .history
            .iter()
            .zip(self.history.iter().skip(1))
            .map(|(a, b)| b.duration_since(*a).as_secs_f32())
            .collect();
        let mean_secs_per_tick = intervals.iter().sum::<f32>() / intervals.len() as f32;
        if mean_secs_per_tick < 1e-6 {
            return;
        }
        // 24 ticks per quarter note.
        let secs_per_beat = mean_secs_per_tick * 24.0;
        let raw_bpm = (60.0 / secs_per_beat).clamp(40.0, 240.0);
        self.smoothed_bpm = Some(match self.smoothed_bpm {
            None => raw_bpm,
            Some(prev) => prev + (raw_bpm - prev) * (1.0 - self.smoothing),
        });
    }

    /// Current smoothed BPM estimate, if there's enough history.
    pub fn estimated_bpm(&self) -> Option<f32> {
        self.smoothed_bpm
    }

    /// Number of ticks currently retained.
    pub fn tick_count(&self) -> usize {
        self.history.len()
    }
}

// === I/O ===

/// List the names of available MIDI input ports. Returns an empty
/// vec if no MIDI subsystem is available.
pub fn list_input_ports() -> Result<Vec<String>, MidiError> {
    let input = MidiInput::new("woodshed-list")?;
    let ports = input.ports();
    let mut names = Vec::with_capacity(ports.len());
    for p in &ports {
        match input.port_name(p) {
            Ok(name) => names.push(name),
            Err(_) => names.push(String::from("<unnamed>")),
        }
    }
    Ok(names)
}

/// List the names of available MIDI output ports.
pub fn list_output_ports() -> Result<Vec<String>, MidiError> {
    let output = match MidiOutput::new("woodshed-list") {
        Ok(output) => output,
        Err(error) => {
            // Native BLE is independent of the OS MIDI-port service (notably
            // ALSA sequencer availability). Keep its explicit choices reachable.
            #[cfg(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ))]
            {
                let _ = error;
                return Ok(vec![
                    ble::OUTPUT_NAME.into(),
                    ble::COMPAT_OUTPUT_NAME.into(),
                ]);
            }
            #[cfg(not(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            )))]
            {
                return Err(error.into());
            }
        },
    };
    let ports = output.ports();
    let mut names = Vec::with_capacity(ports.len());
    for p in &ports {
        match output.port_name(p) {
            Ok(name) => names.push(name),
            Err(_) => names.push(String::from("<unnamed>")),
        }
    }
    #[cfg(all(
        feature = "ble-midi",
        any(target_os = "macos", target_os = "linux", target_os = "windows")
    ))]
    for label in [ble::OUTPUT_NAME, ble::COMPAT_OUTPUT_NAME] {
        if !names.iter().any(|name| name == label) {
            names.push(label.into());
        }
    }
    Ok(names)
}

/// Open a MIDI output port by name. Mostly a convenience that hides
/// `midir`'s `port-by-index` API.
pub struct MidiOut {
    conn: OutputConnection,
}

enum OutputConnection {
    Port(PortOutput),
    #[cfg(all(
        feature = "ble-midi",
        any(target_os = "macos", target_os = "linux", target_os = "windows")
    ))]
    Bluetooth(ble::Output),
}

struct PortOutput {
    connection: MidiOutputConnection,
    port: MidiOutputPort,
    id: String,
    inventory: MidiOutput,
}
impl PortOutput {
    fn available(&self) -> bool {
        // A successful OS send is not proof a destination still exists.
        // Retain the original endpoint identity; never substitute by name.
        self.port.id() == self.id
            && self
                .inventory
                .ports()
                .iter()
                .any(|port| port == &self.port && port.id() == self.id)
    }
    fn send(&mut self, bytes: &[u8]) -> Result<(), MidiError> {
        if !self.available() {
            return Err(MidiError::Connect(
                "MIDI destination disappeared; message not submitted. Reconnect explicitly".into(),
            ));
        }
        self.connection.send(bytes)?;
        Ok(())
    }
}

fn find_output_port(output: &MidiOutput, name: &str) -> Result<MidiOutputPort, MidiError> {
    for p in output.ports() {
        if let Ok(pn) = output.port_name(&p) {
            if pn == name {
                return Ok(p);
            }
        }
    }
    Err(MidiError::PortNotFound(name.to_string()))
}

fn find_input_port(input: &MidiInput, name: &str) -> Result<MidiInputPort, MidiError> {
    for p in input.ports() {
        if let Ok(pn) = input.port_name(&p) {
            if pn == name {
                return Ok(p);
            }
        }
    }
    Err(MidiError::PortNotFound(name.to_string()))
}

/// Bounded queue of recent MIDI events with a clock-sync tracker.
#[derive(Clone, Debug, Default)]
pub struct MidiInState {
    /// Monotonic event sequence for consumers; timestamps need not be unique.
    pub received_events: u64,
    pub events: VecDeque<(Instant, MidiEvent)>,
    pub clock_sync: MidiClockSync,
}

impl MidiInState {
    pub const CAPACITY: usize = 256;

    /// None means history was lost/reset. Consumers must discard stale intent.
    pub fn events_since(&self, cursor: u64) -> Option<Vec<(Instant, MidiEvent)>> {
        let count = self.received_events.checked_sub(cursor)?;
        if count > self.events.len() as u64 {
            return None;
        }
        Some(
            self.events
                .iter()
                .skip(self.events.len() - count as usize)
                .copied()
                .collect(),
        )
    }

    fn push(&mut self, at: Instant, event: MidiEvent) {
        self.received_events = self.received_events.saturating_add(1);
        if matches!(event, MidiEvent::Clock) {
            self.clock_sync.record_tick(at);
        }
        if self.events.len() >= Self::CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back((at, event));
    }
}

/// Open a MIDI input port by name. Events arrive on midir's reader
/// thread and are queued into shared state behind a mutex; UI / app
/// code polls [`Self::state`] to see what's come in.
pub struct MidiIn {
    state: Arc<Mutex<MidiInState>>,
    _conn: MidiInputConnection<()>,
    port: MidiInputPort,
    id: String,
    inventory: MidiInput,
}

impl MidiIn {
    pub fn connect_by_name(port_name: &str) -> Result<Self, MidiError> {
        Self::connect_with_handler(port_name, |_| {})
    }

    /// Observe parsed input on the existing reader callback, independently of
    /// UI/display polling. Keep handlers bounded; do not call a UI from here.
    pub fn connect_with_handler(
        port_name: &str,
        mut handler: impl FnMut(MidiEvent) + Send + 'static,
    ) -> Result<Self, MidiError> {
        let mut input = MidiInput::new("woodshed-in")?;
        input.ignore(Ignore::None);
        let port = find_input_port(&input, port_name)?;
        let inventory = MidiInput::new("woodshed-input-inventory")?;
        let id = port.id();
        let state = Arc::new(Mutex::new(MidiInState::default()));
        let state_for_callback = Arc::clone(&state);
        let conn = input
            .connect(
                &port,
                "woodshed-in",
                move |_timestamp_us, bytes, _| {
                    if let Some(event) = parse_message(bytes) {
                        state_for_callback
                            .lock()
                            .unwrap()
                            .push(Instant::now(), event);
                        handler(event);
                    }
                },
                (),
            )
            .map_err(|e| MidiError::Connect(format!("{e}")))?;
        Ok(Self {
            state,
            _conn: conn,
            port,
            id,
            inventory,
        })
    }

    /// Check the original endpoint; never replace it by friendly name.
    pub fn is_ready(&self) -> bool {
        self.port.id() == self.id
            && self
                .inventory
                .ports()
                .iter()
                .any(|p| p == &self.port && p.id() == self.id)
    }

    /// Snapshot the current shared state.
    pub fn snapshot(&self) -> MidiInState {
        self.state.lock().unwrap().clone()
    }

    /// Drop all queued events and reset the clock sync. Call when
    /// entering a fresh sync session.
    pub fn reset(&self) {
        let mut s = self.state.lock().unwrap();
        s.events.clear();
        s.clock_sync.reset();
    }
}

#[cfg(test)]
mod tests;

mod output;
