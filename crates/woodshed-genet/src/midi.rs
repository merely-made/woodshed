//! The desktop [`MidiBackend`]: `woodshed-audio`'s `midir` layer behind
//! the core's MIDI seam (audio-depth slice 13).
//!
//! Input is a connected `MidiIn` — its `midir` reader thread queues
//! events and the `MidiClockSync` derives BPM from incoming clock, which
//! the host polls to slave its transport. Output is a dedicated
//! clock-generator thread that emits 24-PPQN MIDI clock + Start/Stop so
//! external gear slaves to Woodshed's tempo; the thread owns the output
//! connection, so the `midir` handle never crosses a thread boundary.
//! The web host implements the same trait over Web MIDI.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use woodshed_audio::{MidiEvent, MidiIn, list_midi_input_ports, list_midi_output_ports};
use woodshed_core::midi::MidiBackend;

mod midi_output;
use midi_output::{Ack, Command};

fn output_request_needed(
    accepted: Option<&str>,
    pending: Option<Option<&str>>,
    last_failed: Option<Option<&str>>,
    target: Option<&str>,
) -> bool {
    if pending == Some(target) {
        return false;
    }
    if pending.is_some() {
        return true;
    }
    accepted != target && last_failed != Some(target)
}

pub struct MidiHost {
    input: Option<MidiIn>,
    connected_in: Option<String>,
    output: OutputControl,
    clock_tx: Sender<Command>,
    _clock_thread: JoinHandle<()>,
    /// De-dup the per-tick clock-out pushes so a steady transport doesn't
    /// spam the channel every frame.
    last_out: (bool, bool, i32),
    input_error: Option<String>,
}

struct OutputControl {
    accepted: Option<String>,
    requested: Option<(u64, Option<String>)>,
    last_attempt: Option<Option<String>>,
    active_request: Option<u64>,
    next_request: u64,
    tx: Sender<Command>,
    ack_rx: Receiver<Ack>,
    error: Option<String>,
}

impl OutputControl {
    fn new(tx: Sender<Command>, ack_rx: Receiver<Ack>) -> Self {
        Self {
            accepted: None,
            requested: None,
            last_attempt: None,
            active_request: None,
            next_request: 0,
            tx,
            ack_rx,
            error: None,
        }
    }

    fn poll(&mut self) {
        while let Ok(ack) = self.ack_rx.try_recv() {
            let current = self
                .requested
                .as_ref()
                .is_some_and(|(id, _)| *id == ack.request_id);
            let active =
                self.active_request.is_some() && ack.connection_request_id == self.active_request;
            if current || active {
                self.accepted = ack.accepted;
                if current {
                    self.requested = None;
                    self.active_request = ack.connection_request_id;
                }
                if let Some(error) = ack.error {
                    self.error = Some(format!("midi out: {error}"));
                } else if current {
                    self.error = None;
                    self.last_attempt = None;
                }
            }
        }
    }

    fn connect(&mut self, port: Option<&str>) {
        let pending = self.requested.as_ref().map(|(_, target)| target.as_deref());
        let last_failed = self
            .last_attempt
            .as_ref()
            .filter(|_| self.requested.is_none())
            .map(|target| target.as_deref());
        if !output_request_needed(self.accepted.as_deref(), pending, last_failed, port) {
            return;
        }
        self.next_request = self.next_request.wrapping_add(1);
        let request_id = self.next_request;
        let target = port.map(str::to_string);
        self.requested = Some((request_id, target.clone()));
        self.last_attempt = Some(target.clone());
        if self
            .tx
            .send(Command::Connect { request_id, target })
            .is_err()
        {
            self.requested = None;
            self.error = Some("midi out: clock worker is unavailable".into());
        }
    }
}

impl MidiHost {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Command>();
        let (ack_tx, ack_rx) = channel::<Ack>();
        let thread = std::thread::spawn(move || midi_output::clock_out_loop(rx, ack_tx));
        Self {
            input: None,
            connected_in: None,
            output: OutputControl::new(tx.clone(), ack_rx),
            clock_tx: tx,
            _clock_thread: thread,
            last_out: (false, false, -1),
            input_error: None,
        }
    }
}

impl Default for MidiHost {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiBackend for MidiHost {
    fn poll(&mut self) {
        self.output.poll();
    }

    fn input_ports(&self) -> Vec<String> {
        list_midi_input_ports().unwrap_or_default()
    }

    fn output_ports(&self) -> Vec<String> {
        list_midi_output_ports().unwrap_or_default()
    }

    fn connect_input(&mut self, port: Option<&str>) {
        match port {
            None => {
                self.input = None;
                self.connected_in = None;
                self.input_error = None;
            },
            Some(name) => {
                if self.connected_in.as_deref() == Some(name) {
                    return;
                }
                match MidiIn::connect_by_name(name) {
                    Ok(m) => {
                        self.input = Some(m);
                        self.connected_in = Some(name.to_string());
                        self.input_error = None;
                    },
                    Err(e) => {
                        self.input = None;
                        self.connected_in = None;
                        self.input_error = Some(format!("midi in: {e}"));
                    },
                }
            },
        }
    }

    fn connect_output(&mut self, port: Option<&str>) {
        self.output.connect(port);
    }

    fn connected_input(&self) -> Option<&str> {
        self.connected_in.as_deref()
    }

    fn connected_output(&self) -> Option<&str> {
        self.output.accepted.as_deref()
    }

    fn clock_bpm(&self) -> Option<f32> {
        self.input
            .as_ref()
            .and_then(|m| m.snapshot().clock_sync.estimated_bpm())
    }

    fn recent_events(&self) -> Vec<String> {
        let Some(m) = self.input.as_ref() else {
            return Vec::new();
        };
        let snap = m.snapshot();
        // Newest-first, drop the clock spam, keep the last handful, then
        // restore chronological (newest last) order.
        let mut recent: Vec<String> = snap
            .events
            .iter()
            .rev()
            .filter(|(_, e)| !matches!(e, MidiEvent::Clock))
            .take(6)
            .map(|(_, e)| fmt_event(e))
            .collect();
        recent.reverse();
        recent
    }

    fn set_clock_out(&mut self, enabled: bool, playing: bool, bpm: f32) {
        let key = (enabled, playing, bpm.round() as i32);
        if key == self.last_out {
            return;
        }
        self.last_out = key;
        let _ = self.clock_tx.send(Command::Enabled(enabled));
        let _ = self.clock_tx.send(Command::Playing(playing));
        let _ = self.clock_tx.send(Command::Bpm(bpm));
    }

    fn error(&self) -> Option<&str> {
        self.output.error.as_deref().or(self.input_error.as_deref())
    }
}

#[cfg(test)]
mod output_request_tests {
    use super::{Ack, Command, OutputControl, output_request_needed};
    use std::sync::mpsc::channel;

    fn control() -> (
        OutputControl,
        std::sync::mpsc::Receiver<Command>,
        std::sync::mpsc::Sender<Ack>,
    ) {
        let (command_tx, command_rx) = channel();
        let (ack_tx, ack_rx) = channel();
        (OutputControl::new(command_tx, ack_rx), command_rx, ack_tx)
    }

    fn request_id(command: Command) -> u64 {
        match command {
            Command::Connect { request_id, .. } => request_id,
            _ => panic!("expected output connect request"),
        }
    }

    #[test]
    fn superseding_pending_target_restores_accepted_or_disconnect_choice() {
        assert!(output_request_needed(
            Some("A"),
            Some(Some("B")),
            None,
            Some("A")
        ));
        assert!(output_request_needed(None, Some(Some("B")), None, None));
        assert!(!output_request_needed(
            Some("A"),
            Some(Some("B")),
            None,
            Some("B")
        ));
        assert!(!output_request_needed(
            Some("A"),
            None,
            Some(Some("B")),
            Some("B")
        ));
    }

    #[test]
    fn production_channel_supersedes_pending_with_currently_accepted_target() {
        let (mut output, commands, acks) = control();
        output.connect(Some("A"));
        let first_id = request_id(commands.recv().unwrap());
        acks.send(Ack {
            request_id: first_id,
            connection_request_id: Some(first_id),
            accepted: Some("A".into()),
            error: None,
        })
        .unwrap();
        output.poll();

        output.connect(Some("B"));
        let b_id = request_id(commands.recv().unwrap());
        output.connect(Some("A"));
        let a_id = request_id(commands.recv().unwrap());
        assert!(a_id > b_id);

        acks.send(Ack {
            request_id: b_id,
            connection_request_id: Some(b_id),
            accepted: Some("B".into()),
            error: None,
        })
        .unwrap();
        acks.send(Ack {
            request_id: a_id,
            connection_request_id: Some(a_id),
            accepted: Some("A".into()),
            error: None,
        })
        .unwrap();
        output.poll();
        assert_eq!(output.accepted.as_deref(), Some("A"));
        assert!(output.requested.is_none());
    }

    #[test]
    fn production_channel_supersedes_pending_with_disconnect_and_ignores_stale_ack() {
        let (mut output, commands, acks) = control();
        output.connect(Some("A"));
        let first_id = request_id(commands.recv().unwrap());
        acks.send(Ack {
            request_id: first_id,
            connection_request_id: Some(first_id),
            accepted: Some("A".into()),
            error: None,
        })
        .unwrap();
        output.poll();

        output.connect(Some("B"));
        let b_id = request_id(commands.recv().unwrap());
        output.connect(None);
        let disconnect_id = request_id(commands.recv().unwrap());
        acks.send(Ack {
            request_id: b_id,
            connection_request_id: Some(b_id),
            accepted: Some("B".into()),
            error: None,
        })
        .unwrap();
        acks.send(Ack {
            request_id: disconnect_id,
            connection_request_id: None,
            accepted: None,
            error: None,
        })
        .unwrap();
        output.poll();
        assert_eq!(output.accepted, None);
        assert!(output.requested.is_none());
    }
}

impl Drop for MidiHost {
    fn drop(&mut self) {
        let _ = self.clock_tx.send(Command::Stop);
    }
}

fn fmt_event(e: &MidiEvent) -> String {
    match e {
        MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        } => {
            format!("NoteOn ch{} {} v{}", channel + 1, note, velocity)
        },
        MidiEvent::NoteOff { channel, note, .. } => {
            format!("NoteOff ch{} {}", channel + 1, note)
        },
        MidiEvent::ControlChange {
            channel,
            controller,
            value,
        } => {
            format!("CC ch{} #{}={}", channel + 1, controller, value)
        },
        MidiEvent::Clock => "Clock".to_string(),
        MidiEvent::Start => "Start".to_string(),
        MidiEvent::Continue => "Continue".to_string(),
        MidiEvent::Stop => "Stop".to_string(),
    }
}
