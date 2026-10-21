//! The existing output worker, with its transition step exposed to fault tests.
use super::{ClockCmd, OutputState, PedalOutcome, pedal::Request};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, TryRecvError},
};
use std::time::{Duration, Instant};
use woodshed_audio::MidiOut;

trait Connection {
    fn ready(&self) -> bool;
    fn generation(&self) -> Option<u64>;
    fn status(&self) -> Option<String>;
    fn send(&mut self, bytes: &[u8], generation: Option<u64>) -> Result<(), String>;
    fn close(self) -> Result<(), String>;
}
impl Connection for MidiOut {
    fn ready(&self) -> bool {
        self.is_ready()
    }
    fn generation(&self) -> Option<u64> {
        self.connection_generation()
    }
    fn status(&self) -> Option<String> {
        self.transport_status()
    }
    fn send(&mut self, bytes: &[u8], generation: Option<u64>) -> Result<(), String> {
        self.send_raw_for_generation(bytes, generation)
            .map_err(|e| e.to_string())
    }
    fn close(self) -> Result<(), String> {
        self.close().map_err(|e| e.to_string())
    }
}

#[derive(Default)]
struct Clock {
    enabled: bool,
    playing: bool,
    bpm: f32,
    transport: bool,
    armed: bool,
    generation: Option<u64>,
}
impl Clock {
    fn record_send<P: Connection>(
        out: &mut P,
        state: &mut OutputState,
        bytes: &[u8],
        generation: Option<u64>,
    ) -> bool {
        state.attempted += 1;
        match out.send(bytes, generation) {
            Ok(()) => {
                state.accepted += 1;
                true
            },
            Err(error) => {
                state.failed += 1;
                state.failure(format!("MIDI send refused/uncertain: {error}"));
                false
            },
        }
    }
    fn step<P: Connection>(&mut self, out: &mut P, state: &mut OutputState) {
        let ready = out.ready();
        let generation = out.generation();
        if self.generation != generation || state.ready != ready {
            super::diagnostics::record(
                "writer-lifetime",
                None,
                Some(state.revision),
                generation,
                &[],
                || {
                    format!(
                        "ready={ready}; previous_generation={:?}; previous_ready={}",
                        self.generation, state.ready
                    )
                },
            );
        }
        if generation != self.generation || !ready {
            // A replacement can occur entirely between polls. Readiness alone
            // cannot establish that this is the subscriber we armed for.
            self.armed = false;
            self.transport = false;
        }
        self.generation = generation;
        if state.ready && !ready {
            state.disconnects += 1;
            state.failure("MIDI receiver disconnected; clock paused and no messages retained. Prior OS acceptance is not a device acknowledgement".into());
        }
        state.ready = ready;
        state.generation = generation;
        if !self.playing || !self.enabled {
            self.armed = true;
        }
        let want = ready && self.enabled && self.playing && self.armed;
        if want != self.transport {
            if ready {
                let success = Self::record_send(
                    out,
                    state,
                    if want { &[0xfa] } else { &[0xfc] },
                    self.generation,
                );
                if !success {
                    self.armed = false;
                    self.transport = false;
                    self.publish(out, state);
                    return;
                }
            }
            self.transport = want;
        }
        if want && !Self::record_send(out, state, &[0xf8], self.generation) {
            self.armed = false;
            self.transport = false;
        }
        self.publish(out, state);
    }
    fn publish<P: Connection>(&self, out: &P, state: &mut OutputState) {
        state.rearm_required = self.enabled && self.playing && !self.armed;
        let mut status = out.status().unwrap_or_else(|| "OS MIDI output open".into());
        if state.rearm_required {
            status.push_str(" · Clock paused; stop/start or disable/re-enable to rearm");
        }
        status.push_str(&format!(
            " · OS accepted {}; refused/uncertain {}",
            state.accepted, state.failed
        ));
        if let Some(failure) = &state.last_failure {
            status.push_str(&format!(" · Last failure: {failure}"));
        }
        state.append_pedal_failure(&mut status);
        state.status = Some(status);
    }
    fn retire<P: Connection>(&mut self, mut out: P, state: &mut OutputState) {
        // Stop only the same ready connection; never send it to a replacement.
        if self.transport && out.ready() && out.generation() == self.generation {
            Self::record_send(&mut out, state, &[0xfc], self.generation);
        }
        if let Err(error) = out.close() {
            state.failure(format!("MIDI close outcome: {error}"));
        }
        self.transport = false;
        self.armed = false;
        self.generation = None;
        state.connected = None;
        state.revision = 0;
        state.ready = false;
        state.generation = None;
        state.status = Some(state.closed_status());
    }
}

pub(super) fn run(
    rx: Receiver<ClockCmd>,
    output: Arc<Mutex<OutputState>>,
    selection: Arc<AtomicU64>,
) {
    run_with(rx, output, selection, |name| {
        MidiOut::connect_by_name(name).map_err(|e| e.to_string())
    });
}
fn run_with<P: Connection>(
    rx: Receiver<ClockCmd>,
    output: Arc<Mutex<OutputState>>,
    selection: Arc<AtomicU64>,
    mut open: impl FnMut(&str) -> Result<P, String>,
) {
    let mut conn: Option<P> = None;
    let mut connection_revision = 0;
    let mut clock = Clock {
        bpm: 120.0,
        ..Default::default()
    };
    let mut pedal: Option<Request> = None;
    loop {
        let mut shutdown = false;
        loop {
            match rx.try_recv() {
                Ok(ClockCmd::Pedal(mut request)) => {
                    if pedal.is_some() {
                        request.cancel("Pedal slot occupied; nothing retained");
                    } else {
                        pedal = Some(request);
                    }
                },
                Ok(ClockCmd::Connect { target, revision }) => {
                    if revision != selection.load(Ordering::Acquire) {
                        continue;
                    }
                    if let Some(previous) = conn.take() {
                        with_state(&output, |state| clock.retire(previous, state));
                    }
                    {
                        let mut state = output.lock().unwrap();
                        state.connected = None;
                        state.ready = false;
                        state.status = Some(if target.is_some() {
                            "Opening MIDI output…".into()
                        } else {
                            state.closed_status()
                        });
                    }
                    if let Some(name) = target {
                        // Do not hold the readout mutex through OS initialization.
                        let result = open(&name);
                        if revision != selection.load(Ordering::Acquire) {
                            if let Ok(previous) = result {
                                with_state(&output, |state| clock.retire(previous, state));
                            }
                            continue;
                        }
                        match result {
                            Ok(connection) => {
                                {
                                    let mut state = output.lock().unwrap();
                                    state.connected = Some(name);
                                    state.revision = revision;
                                }
                                connection_revision = revision;
                                conn = Some(connection);
                            },
                            Err(error) => {
                                output
                                    .lock()
                                    .unwrap()
                                    .failure(format!("MIDI open failed: {error}"));
                            },
                        }
                    }
                    output.lock().unwrap().settled_revision = revision;
                },
                Ok(ClockCmd::Transport {
                    enabled,
                    playing,
                    bpm,
                }) => {
                    clock.enabled = enabled;
                    clock.playing = playing;
                    clock.bpm = bpm;
                },
                Ok(ClockCmd::Stop) | Err(TryRecvError::Disconnected) => {
                    shutdown = true;
                    break;
                },
                Err(TryRecvError::Empty) => break,
            }
        }
        if shutdown {
            if let Some(mut request) = pedal.take() {
                request.cancel("Output closed; no replay");
            }
            if let Some(previous) = conn.take() {
                with_state(&output, |state| clock.retire(previous, state));
            }
            return;
        }
        if let Some(mut request) = pedal.take() {
            let done = match conn.as_mut() {
                Some(connection) => pedal_step(
                    &mut request,
                    connection,
                    &output,
                    &selection,
                    connection_revision,
                ),
                None => {
                    request.cancel("MIDI output closed; no replay");
                    true
                },
            };
            if !done {
                pedal = Some(request);
            }
        }
        if connection_revision == selection.load(Ordering::Acquire) {
            if let Some(connection) = conn.as_mut() {
                with_state(&output, |state| clock.step(connection, state));
            }
        }
        let interval = if clock.transport {
            60.0 / (clock.bpm.max(1.0) * 24.0)
        } else {
            0.02
        };
        let mut sleep = Duration::from_secs_f32(interval);
        if let Some(request) = &pedal {
            sleep = sleep.min(request.due.saturating_duration_since(Instant::now()));
        }
        std::thread::sleep(sleep);
    }
}

// Each message uses the same selected connection and subscriber generation.
// Release is scheduled in this worker, so the 50 ms gap does not block clock.
fn pedal_step<P: Connection>(
    request: &mut Request,
    out: &mut P,
    output: &Mutex<OutputState>,
    selection: &AtomicU64,
    connection_revision: u64,
) -> bool {
    if request.revision != selection.load(Ordering::Acquire)
        || request.revision != connection_revision
        || !out.ready()
        || request.generation != out.generation()
    {
        request.cancel("Receiver changed/disconnected; remaining messages cancelled, no replay");
        return true;
    }
    let now = Instant::now();
    if now > request.expires {
        request.cancel("Pedal deadline expired; remaining messages cancelled, no replay");
        return true;
    }
    if now < request.due {
        return false;
    }
    let index = request.report.accepted_messages;
    super::diagnostics::record(
        "writer-send-start",
        Some(request.report.id),
        Some(request.revision),
        request.generation,
        &request.messages[index],
        || {
            format!(
                "index={index}; action={:?}; due_late_us={}",
                request.report.action,
                now.saturating_duration_since(request.due).as_micros()
            )
        },
    );
    let mut success = false;
    let mut error = None;
    with_state(output, |state| {
        success = Clock::record_send(out, state, &request.messages[index], request.generation);
        error = state.last_failure.clone();
    });
    super::diagnostics::record(
        "writer-send-result",
        Some(request.report.id),
        Some(request.revision),
        request.generation,
        &request.messages[index],
        || format!("index={index}; os_accepted={success}; error={error:?}"),
    );
    if !success {
        request.finish(
            PedalOutcome::Uncertain,
            error.unwrap_or_else(|| "Writer outcome uncertain; no retry".into()),
        );
        return true;
    }
    request.report.accepted_messages += 1;
    if request.report.accepted_messages == request.messages.len() {
        request.finish(
            PedalOutcome::Accepted,
            "All messages accepted by OS; musical result unobserved",
        );
        true
    } else {
        request.due = Instant::now() + Duration::from_millis(50);
        request.expires = request.due + Duration::from_millis(100);
        false
    }
}

// OS work can take hundreds of milliseconds. Publish a new snapshot afterward,
// without holding the readout mutex and blocking the UI through that operation.
fn with_state(output: &Mutex<OutputState>, work: impl FnOnce(&mut OutputState)) {
    let mut state = output.lock().unwrap().clone();
    work(&mut state);
    *output.lock().unwrap() = state;
}

#[cfg(test)]
mod tests;
