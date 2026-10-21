//! The desktop [`MidiBackend`]: `woodshed-audio`'s `midir` layer behind
//! the core's MIDI seam (audio-depth slice 13).
//!
//! Input is a connected `MidiIn` — its `midir` reader thread queues
//! events and the `MidiClockSync` derives BPM from incoming clock, which
//! the host polls to slave its transport. Output is a dedicated
//! output thread that emits 24-PPQN MIDI clock + Start/Stop and admitted
//! software pedal gestures; the thread owns the output
//! connection, so the `midir` handle never crosses a thread boundary.
//! The web host implements the same trait over Web MIDI.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use woodshed_audio::{MidiEvent, MidiIn, list_midi_input_ports, list_midi_output_ports};
use woodshed_core::midi::{
    ControllerBatch, MidiBackend, PedalAction, PedalBinding, PedalBindings, PedalOutcome,
    PedalReport, PedalTarget,
};

mod controller;
mod pedal;
mod stdio;
/// Commands to the existing MIDI output thread. The thread owns the output
/// connection (it calls `connect_by_name` itself), so no `midir` handle
/// crosses the channel — only these plain-data messages do.
mod worker;
use pedal::{Dispatcher, Request};
use woodshed_audio::midi::diagnostics;

enum ClockCmd {
    Connect {
        target: Option<String>,
        revision: u64,
    },
    Transport {
        enabled: bool,
        playing: bool,
        bpm: f32,
    },
    Pedal(Request),
    Stop,
}

#[derive(Clone, Default, serde::Serialize)]
pub(crate) struct OutputState {
    connected: Option<String>,
    revision: u64,
    status: Option<String>,
    error: Option<String>,
    ready: bool,
    generation: Option<u64>,
    rearm_required: bool,
    attempted: u64,
    accepted: u64,
    failed: u64,
    disconnects: u64,
    last_failure: Option<String>,
    /// Retain an incomplete gesture across later refusal/reconnect/status updates.
    pedal_failure: Option<PedalReport>,
}
impl OutputState {
    fn closed_status(&self) -> String {
        let mut status = "Output closed".to_string();
        if let Some(error) = &self.last_failure {
            status.push_str(&format!(" · Last failure: {error}"));
        }
        self.append_pedal_failure(&mut status);
        status
    }
    fn append_pedal_failure(&self, status: &mut String) {
        if let Some(report) = &self.pedal_failure {
            let summary = report.summary();
            if self.last_failure.as_deref() != Some(summary.as_str()) {
                status.push_str(&format!(" · Last uncertain pedal: {summary}"));
            }
        }
    }
    fn failure(&mut self, error: String) {
        self.status = Some(error.clone());
        self.error = Some(error.clone());
        self.last_failure = Some(error);
    }
}

pub struct MidiHost {
    diagnostics_cache: Option<(
        (bool, Option<u64>),
        woodshed_core::midi::MidiDiagnosticsStatus,
    )>,
    pedal: Dispatcher,
    input: Option<MidiIn>,
    requested_in: Option<String>,
    input_cursor: u64,
    input_invalidated: bool,
    controller: Arc<Mutex<controller::Controller>>,
    connected_in: Option<String>,
    connected_out: Option<String>,
    requested_out: Option<String>,
    output_state: Arc<Mutex<OutputState>>,
    selection_revision: Arc<AtomicU64>,
    output_notice: Option<String>,
    output_error: Option<String>,
    clock_tx: Sender<ClockCmd>,
    clock_thread: Option<JoinHandle<()>>,
    /// De-dup the per-tick clock-out pushes so a steady transport doesn't
    /// spam the channel every frame.
    last_out: (bool, bool, i32),
    error: Option<String>,
}

impl MidiHost {
    pub fn new() -> Self {
        let (tx, rx) = channel::<ClockCmd>();
        let output_state = Arc::new(Mutex::new(OutputState::default()));
        let worker_state = output_state.clone();
        let selection_revision = Arc::new(AtomicU64::new(0));
        let worker_selection = selection_revision.clone();
        let thread = std::thread::spawn(move || worker::run(rx, worker_state, worker_selection));
        let pedal = Dispatcher {
            tx: tx.clone(),
            state: output_state.clone(),
            selection: selection_revision.clone(),
            busy: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
            last: Arc::new(Mutex::new(None)),
            bindings: Arc::new(Mutex::new(PedalBindings::default())),
        };
        Self {
            diagnostics_cache: None,
            pedal,
            input: None,
            requested_in: None,
            input_cursor: 0,
            input_invalidated: false,
            controller: Default::default(),
            connected_in: None,
            connected_out: None,
            requested_out: None,
            output_state,
            selection_revision,
            output_notice: None,
            output_error: None,
            clock_tx: tx,
            clock_thread: Some(thread),
            last_out: (false, false, -1),
            error: None,
        }
    }
    /// Opt-in shell control shares this host's connection and admission gate.
    pub fn enable_pedal_stdio(&self) {
        stdio::start(self.pedal.clone());
    }
    pub fn set_pedal_bindings(&self, bindings: &PedalBindings) {
        let mut current = self.pedal.bindings.lock().unwrap();
        if *current != *bindings {
            *current = bindings.clone();
        }
    }
    pub(crate) fn output_snapshot(&self) -> OutputState {
        self.output_state.lock().unwrap().clone()
    }
}

impl Default for MidiHost {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiBackend for MidiHost {
    fn set_diagnostics_recording(&mut self, recording: bool) {
        diagnostics::set_recording(recording);
    }
    fn diagnostics_status(&mut self) -> woodshed_core::midi::MidiDiagnosticsStatus {
        let revision = diagnostics::revision();
        if let Some((old, status)) = &self.diagnostics_cache {
            if *old == revision {
                return status.clone();
            }
        }
        let snapshot = diagnostics::snapshot();
        let mut status = woodshed_core::midi::MidiDiagnosticsStatus {
            available: true,
            capacity: diagnostics::CAPACITY,
            ..Default::default()
        };
        if let Some(snapshot) = snapshot {
            status.recording = snapshot.recording;
            status.history_available = true;
            status.retained_events = snapshot.events.len();
            status.capacity = snapshot.capacity;
            status.overwritten = snapshot.overwritten;
            status.truncated_events = snapshot
                .events
                .iter()
                .filter(|event| event.truncated)
                .count();
            status.recent_events = snapshot
                .events
                .iter()
                .rev()
                .take(12)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|event| {
                    format!(
                        "#{} · {:.3} ms · {} · request {} · rev {:?} / gen {:?} · {:?} · {}{}",
                        event.sequence,
                        event.elapsed_micros as f64 / 1000.0,
                        event.stage,
                        event.request_id.map_or("—".into(), |id| id.to_string()),
                        event.revision,
                        event.generation,
                        event.bytes,
                        event.detail,
                        if event.truncated { " [truncated]" } else { "" }
                    )
                })
                .collect();
        }
        self.diagnostics_cache = Some((revision, status.clone()));
        status
    }
    fn diagnostics_export(&self) -> Result<Vec<u8>, String> {
        let status = self.pedal.status();
        if status["diagnostics"].is_null() {
            return Err("Record MIDI diagnostics before exporting a history".into());
        }
        serde_json::to_vec_pretty(
            &serde_json::json!({"format":"woodshed-midi-diagnostics","version":1,"status":status}),
        )
        .map_err(|error| error.to_string())
    }

    fn input_ports(&self) -> Vec<String> {
        list_midi_input_ports().unwrap_or_default()
    }

    fn output_ports(&self) -> Vec<String> {
        list_midi_output_ports().unwrap_or_default()
    }

    fn connect_input(&mut self, port: Option<&str>) {
        if self.requested_in.as_deref() == port {
            return;
        }
        self.requested_in = port.map(str::to_owned);
        self.input_cursor = 0;
        self.input_invalidated = true;
        let epoch = self
            .controller
            .lock()
            .unwrap()
            .replace_source(port.is_some());
        match port {
            None => {
                self.input = None;
                self.connected_in = None;
            },
            Some(name) => {
                if self.connected_in.as_deref() == Some(name) {
                    return;
                }
                let controller = self.controller.clone();
                let dispatcher = self.pedal.clone();
                match MidiIn::connect_with_handler(name, move |event| {
                    controller
                        .lock()
                        .unwrap()
                        .receive(epoch, event, &dispatcher);
                }) {
                    Ok(m) => {
                        self.input = Some(m);
                        self.connected_in = Some(name.to_string());
                    },
                    Err(e) => {
                        self.controller.lock().unwrap().replace_source(false);
                        self.input = None;
                        self.connected_in = None;
                        self.error = Some(format!("midi in: {e}"));
                    },
                }
            },
        }
    }

    fn connect_output(&mut self, port: Option<&str>) {
        if self.requested_out.as_deref() == port {
            return;
        }
        self.requested_out = port.map(str::to_string);
        self.controller.lock().unwrap().invalidate();
        self.output_notice = port.map(|_| "Opening MIDI output…".into());
        let revision = self.selection_revision.fetch_add(1, Ordering::AcqRel) + 1;
        if self
            .clock_tx
            .send(ClockCmd::Connect {
                target: port.map(str::to_string),
                revision,
            })
            .is_err()
        {
            self.output_state
                .lock()
                .unwrap()
                .failure("MIDI output worker stopped; command was not accepted".into());
        }
    }

    fn connected_input(&self) -> Option<&str> {
        self.connected_in.as_deref()
    }

    fn connected_output(&self) -> Option<&str> {
        self.connected_out.as_deref()
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
        {
            let state = self.output_state.lock().unwrap();
            self.connected_out = state.connected.clone();
            self.output_error = state.error.clone();
            self.output_notice = state.status.clone().or_else(|| state.error.clone());
        }
        let key = (enabled, playing, bpm.round() as i32);
        if key == self.last_out {
            return;
        }
        self.last_out = key;
        if self
            .clock_tx
            .send(ClockCmd::Transport {
                enabled,
                playing,
                bpm,
            })
            .is_err()
        {
            self.output_state
                .lock()
                .unwrap()
                .failure("MIDI output worker stopped; transport intent was not accepted".into());
        }
    }

    fn trigger_pedal(&mut self, action: PedalAction, binding: PedalBinding) -> PedalReport {
        self.pedal.submit(action, binding).0
    }
    fn configure_pedal_inputs(
        &mut self,
        bindings: &PedalBindings,
        armed: bool,
        target: Option<PedalTarget>,
    ) {
        self.controller
            .lock()
            .unwrap()
            .configure(bindings, armed, target);
    }
    fn pedal_input_events(&mut self) -> ControllerBatch {
        let mut batch = ControllerBatch {
            invalidated: std::mem::take(&mut self.input_invalidated),
            ..Default::default()
        };
        if self.input.as_ref().is_some_and(|input| !input.is_ready()) {
            self.controller.lock().unwrap().replace_source(false);
            self.input = None;
            self.connected_in = None;
            batch.invalidated = true;
            batch.notice = Some(
                "MIDI controller disconnected; inputs disarmed. Reopen the input explicitly."
                    .into(),
            );
        }
        if let Some(input) = &self.input {
            let snapshot = input.snapshot();
            let events = snapshot.events_since(self.input_cursor);
            self.input_cursor = snapshot.received_events;
            if let Some(events) = events {
                for (_, event) in events {
                    // This history is preparation/display only. Live events
                    // already visited the single callback-side edge gate.
                    if let Some(event) = controller::event(event) {
                        batch.events.push(event);
                    }
                }
            } else {
                self.controller.lock().unwrap().invalidate();
                batch.invalidated = true;
                batch.notice =
                    Some("MIDI controller history overflow/reset; inputs disarmed".into());
            }
        }
        if self.controller.lock().unwrap().take_invalidation() {
            batch.invalidated = true;
            batch.notice.get_or_insert_with(|| {
                "Controller/receiver lifetime changed; inputs disarmed".into()
            });
        }
        if let Some(notice) = self.controller.lock().unwrap().take_notice() {
            batch.notice.get_or_insert(notice);
        }
        batch
    }
    fn pedal_target(&self) -> Option<PedalTarget> {
        let state = self.output_state.lock().unwrap();
        (state.ready && state.revision == self.selection_revision.load(Ordering::Acquire))
            .then_some(PedalTarget {
                revision: state.revision,
                generation: state.generation,
            })
    }
    fn trigger_pedal_for_target(
        &mut self,
        action: PedalAction,
        binding: PedalBinding,
        target: PedalTarget,
    ) -> PedalReport {
        let report = self
            .pedal
            .submit_expected(action, binding, Some((target.revision, target.generation)))
            .0;
        diagnostics::record(
            "desktop-decision",
            Some(report.id),
            Some(target.revision),
            target.generation,
            &[],
            || report.summary(),
        );
        report
    }
    fn pedal_report(&self) -> Option<PedalReport> {
        self.pedal.last.lock().unwrap().clone()
    }
    fn output_ready(&self) -> bool {
        let state = self.output_state.lock().unwrap();
        state.ready
            && state.revision == self.selection_revision.load(Ordering::Acquire)
            && !self.pedal.busy.load(Ordering::Acquire)
    }
    fn error(&self) -> Option<&str> {
        self.output_error.as_deref().or(self.error.as_deref())
    }

    fn output_status(&self) -> Option<&str> {
        self.output_notice.as_deref()
    }
}

impl Drop for MidiHost {
    fn drop(&mut self) {
        self.controller.lock().unwrap().replace_source(false);
        let _ = self.clock_tx.send(ClockCmd::Stop);
        if let Some(thread) = self.clock_thread.take() {
            let _ = thread.join();
        }
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
