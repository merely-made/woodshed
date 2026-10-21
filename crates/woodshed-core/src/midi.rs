//! The MIDI seam (genet-host plan; audio-depth slice 13).
//!
//! The core describes what the app wants from MIDI in neutral data; a
//! host supplies a [`MidiBackend`] that realizes it — `midir` on
//! desktop, Web MIDI in the browser. The core never touches a MIDI API,
//! so the same intent (which ports, slave-to-clock, send-clock) drives
//! both. The BPM-from-clock math and byte parsing live in
//! `woodshed-audio`'s `midi` module (tested there); this is only the
//! boundary the host implements. Pedal preparation and encoding are portable
//! `woodshed-instrument::pedal` data, re-exported here without native drivers.

pub use woodshed_instrument::pedal::{
    Action as PedalAction, Binding as PedalBinding, Bindings as PedalBindings,
    MessageKind as PedalMessageKind, Outcome as PedalOutcome, Report as PedalReport,
};

pub use woodshed_instrument::pedal::input::{
    ControllerBatch, ControllerBinding, ControllerEdges, ControllerEvent, ControllerKind,
    Mappings as PedalInputMappings, Shortcut as PedalShortcut, Target as PedalTarget,
};

/// Transient diagnostics readout; capture preference/history never enter settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MidiDiagnosticsStatus {
    pub available: bool,
    pub recording: bool,
    pub history_available: bool,
    pub retained_events: usize,
    pub capacity: usize,
    pub overwritten: u64,
    pub truncated_events: usize,
    pub recent_events: Vec<String>,
}

/// The host-supplied MIDI realization. Implementations should be
/// tolerant of a missing subsystem: construct in a degraded state and
/// report through [`error`](Self::error) rather than failing the app.
pub trait MidiBackend {
    fn set_diagnostics_recording(&mut self, _recording: bool) {}
    fn diagnostics_status(&mut self) -> MidiDiagnosticsStatus {
        MidiDiagnosticsStatus::default()
    }
    /// Freeze the same observation returned by the desktop scripting Status path.
    fn diagnostics_export(&self) -> Result<Vec<u8>, String> {
        Err("MIDI diagnostics are unavailable in this host".into())
    }
    /// Drain host completion facts before projecting accepted connection state.
    fn poll(&mut self) {}
    /// Keep host frames running while asynchronous MIDI work needs observation.
    fn needs_service(&self) -> bool {
        false
    }

    /// Available input port names (a fresh scan each call — ports come
    /// and go as gear is plugged in).
    fn input_ports(&self) -> Vec<String>;
    /// Available output port names (fresh scan).
    fn output_ports(&self) -> Vec<String>;
    /// Connect the named input port; `None` disconnects. Once connected,
    /// incoming MIDI clock feeds [`clock_bpm`](Self::clock_bpm) and other
    /// events feed [`recent_events`](Self::recent_events).
    /// Deduplicate requested intent; explicit None must retire an old request
    /// even after the confirmed endpoint has disappeared. Reopen is explicit.
    fn connect_input(&mut self, port: Option<&str>);
    /// Connect the named output port; `None` disconnects. The clock-out
    /// master (see [`set_clock_out`](Self::set_clock_out)) drives it. Repeated
    /// requests are idempotent, including pending/refused targets. A new target
    /// supersedes an outstanding request even if it equals the accepted port.
    /// Explicit pedal gestures share this same output owner.
    fn connect_output(&mut self, port: Option<&str>);
    /// The connected input port name, if any.
    fn connected_input(&self) -> Option<&str>;
    /// The accepted output connection, if any; a pending request is not acceptance.
    fn connected_output(&self) -> Option<&str>;
    /// BPM derived from incoming MIDI clock — `None` when the input is
    /// disconnected or not receiving clock. The host polls this to slave
    /// its transport tempo.
    fn clock_bpm(&self) -> Option<f32>;
    /// Recent incoming events as short display strings (newest last),
    /// for a live readout. Clock ticks are summarized out.
    fn recent_events(&self) -> Vec<String>;
    /// Drive the clock-out master. While `enabled && playing` and an
    /// output is connected, emit 24-PPQN MIDI clock at `bpm` and send
    /// Start / Stop on the play-state transitions so external gear
    /// slaves to Woodshed's tempo. Idempotent; call every dispatch/tick.
    fn set_clock_out(&mut self, enabled: bool, playing: bool, bpm: f32);
    /// One explicit action, never retained for a future connection. Hosts which
    /// do not implement pedal output must refuse rather than silently accept.
    fn trigger_pedal(&mut self, action: PedalAction, _binding: PedalBinding) -> PedalReport {
        PedalReport::refused(
            0,
            action,
            "Software pedal output is unavailable in this host",
        )
    }
    /// Consume each fresh input event once; display history remains independent.
    fn pedal_input_events(&mut self) -> ControllerBatch {
        ControllerBatch::default()
    }
    /// Prepare the existing input owner's live mapping gate. Learning/history
    /// polling does not authorize actions. Arming and target identity are live.
    fn configure_pedal_inputs(
        &mut self,
        _bindings: &PedalBindings,
        _armed: bool,
        _target: Option<PedalTarget>,
    ) {
    }
    /// Actual receiver identity, independent of the pending-action slot.
    fn pedal_target(&self) -> Option<PedalTarget> {
        None
    }
    fn trigger_pedal_for_target(
        &mut self,
        action: PedalAction,
        binding: PedalBinding,
        target: PedalTarget,
    ) -> PedalReport {
        if self.pedal_target() == Some(target) {
            self.trigger_pedal(action, binding)
        } else {
            PedalReport::refused(0, action, "Input target changed; action cancelled")
        }
    }
    fn pedal_report(&self) -> Option<PedalReport> {
        None
    }
    fn output_ready(&self) -> bool {
        false
    }
    /// A subsystem/connection failure to surface in the UI, if any.
    fn error(&self) -> Option<&str>;
    /// Output transport readiness/advertising status, when the host can observe it.
    fn output_status(&self) -> Option<&str> {
        None
    }
}
