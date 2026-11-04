//! The per-frame drive: everything that advances because time passed.
//!
//! The tuner's reading, the song's bar, the rehearsal dwell clock, the arpeggio
//! and exercise step clocks, incoming MIDI, and the latency calibration poll.
//! Returns whether any of them wants another frame — the host keeps frames
//! coming while it does, and sleeps when it does not.

use woodshed_core::audio::{AudioBackend, CalibrationStatus, TRANSPORT_MAX_BPM, TRANSPORT_MIN_BPM};
use woodshed_core::midi::MidiBackend as _;
use woodshed_views::stage::UiState;

use crate::shared::Shared;

/// The host supplies event timestamps even while the view is idle between
/// frames. Portable UI tests retain their deterministic supplied timestamps.
pub fn wall_time_ms() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|since| since.as_millis() as u64)
}

/// Whether a graph drag still needs the full time-driven state pass.
///
/// Pointer dispatch has already rebuilt the retained tree for the moved node.
/// A static practice surface can therefore paint that result directly. Live
/// audio, transport, calibration, and MIDI surfaces still need their ordinary
/// frame updates while the pointer is held.
pub fn requires_live_frame(shared: &Shared, ui: &UiState) -> bool {
    (ui.midi.diagnostics.recording && ui.section == woodshed_core::storage::AppSection::Settings)
        || ui.tuner.enabled
        || ui.song_playing
        || ui.rehearsal_running
        || ui.stage.arpeggio_playing
        || ui.stage.exercise_playing
        || ui.stage.scale_run_playing
        || ui.calib_active
        || shared.midi.needs_service()
        || shared
            .backend
            .as_ref()
            .is_some_and(crate::audio::CpalBackend::needs_service)
        || (shared.midi.connected_input().is_some()
            && (ui.midi.clock_slave || ui.section == woodshed_core::storage::AppSection::Settings))
}

/// Observations come from the accepted runtime, including after a one-shot
/// stops. Reading them does not admit or replace any product source.
pub struct AudioObservation {
    playing: bool,
    bar: Option<usize>,
    capture_busy: bool,
    loops: Vec<bool>,
    error: Option<String>,
    availability: woodshed_core::audio::AudioAvailability,
    recorder: woodshed_views::stage::RecorderObservation,
}

impl AudioObservation {
    pub fn changed(&self, ui: &UiState) -> bool {
        self.playing != ui.song_playing
            || self.bar.is_some_and(|bar| bar != ui.song_bar_live)
            || self.capture_busy != ui.song_recording
            || self.loops != ui.song_loop_bars
            || self.error != ui.audio_error
            || self.availability != ui.audio_availability
            || self.recorder != ui.recorder_observation
    }

    pub fn apply(self, ui: &mut UiState) {
        ui.song_playing = self.playing;
        if let Some(bar) = self.bar {
            ui.song_bar_live = bar;
        }
        ui.song_recording = self.capture_busy;
        ui.song_loop_bars = self.loops;
        ui.audio_error = self.error;
        ui.audio_availability = self.availability;
        if !ui.audio_availability.tuner_active {
            ui.tuner.reading = None;
        }
        ui.recorder_observation = self.recorder;
    }
}

/// Service the context even if every UI transport is stopped. Completed PCM
/// reaches its current owner before a subsequent dispatch can replace backing.
pub fn poll_audio(shared: &mut Shared) -> Option<AudioObservation> {
    let backend = shared.backend.as_mut()?;
    let mut observation = observe_audio(backend);
    observation.recorder = backend.recorder_observation(shared.last_song.as_ref());
    Some(observation)
}

fn observe_audio(backend: &mut crate::audio::CpalBackend) -> AudioObservation {
    backend.service();
    AudioObservation {
        playing: backend.observe_song_playing(),
        bar: backend.song_bar(),
        capture_busy: backend.loop_capture_busy(),
        loops: backend.song_loop_bars(),
        error: backend.error().map(str::to_owned),
        availability: backend.availability(),
        recorder: backend.recorder_observation(None),
    }
}

/// Advance the live clocks against `ui`. Returns `true` while something is
/// animating.
pub fn frame(shared: &mut Shared, ui: &mut UiState) -> bool {
    if !crate::capture_persistence::session_current(shared, ui) {
        if let Some(backend) = shared.backend.as_mut() {
            backend.service();
            ui.audio_availability = backend.availability();
            if !ui.audio_availability.tuner_active {
                ui.tuner.reading = None;
            }
            return backend.needs_service();
        }
        ui.audio_availability = shared
            .backend
            .as_ref()
            .map(crate::audio::CpalBackend::availability)
            .unwrap_or_default();
        if !ui.audio_availability.tuner_active {
            ui.tuner.reading = None;
        }
        return false;
    }
    let mut animating = crate::sync::poll_midi_state(ui, &mut shared.midi);
    let previous_history = ui.practice_history.len();
    let previous_cursor = ui.rehearsal_set().cursor;
    if let Some(observation) = poll_audio(shared) {
        observation.apply(ui);
    }
    crate::capture_persistence::persist_if_changed(shared, ui);
    // Poll the MIDI seam (immutable) before borrowing the backend.
    let midi_in_connected = shared.midi.connected_input().is_some();
    let midi_clock_bpm = shared.midi.clock_bpm();
    let midi_events = shared.midi.recent_events();
    ui.midi.diagnostics = shared.midi.diagnostics_status();
    animating |=
        ui.midi.diagnostics.recording && ui.section == woodshed_core::storage::AppSection::Settings;
    // Refresh frame observations from the host clock. Interaction handlers
    // independently sample the injected event clock, since an idle view may
    // have no intervening frames for a long pause.
    ui.now_ms = wall_time_ms();

    let Some(backend) = shared.backend.as_mut() else {
        ui.audio_availability = Default::default();
        ui.tuner.reading = None;
        if ui.practice_clock_managed && ui.rehearsal_running {
            ui.stop_rehearsal();
            ui.card_timing_notice =
                Some("Audio output is unavailable. Run requires an active output clock.".into());
        }
        return animating;
    };

    if ui.tuner.enabled {
        ui.tuner.reading = backend.tuner_reading();
        animating = true;
    }
    animating |= ui.song_playing || backend.needs_service();
    animating |= backend.sync_practice_clock(ui);

    // MIDI: reflect polled state; slave the transport to incoming clock.
    ui.midi.clock_bpm = midi_clock_bpm;
    ui.midi.events = midi_events;
    if midi_in_connected
        && (ui.midi.clock_slave || ui.section == woodshed_core::storage::AppSection::Settings)
    {
        animating = true;
    }
    if midi_in_connected && ui.midi.clock_slave {
        if let Some(bpm) = midi_clock_bpm {
            let bpm = bpm.clamp(TRANSPORT_MIN_BPM, TRANSPORT_MAX_BPM);
            if (ui.transport.bpm - bpm).abs() > 0.3 {
                ui.set_bpm(bpm);
                backend.set_metronome(ui.transport);
            }
        }
    }
    // Latency calibration: poll while a run is active; drop out of active on any
    // terminal status.
    if ui.calib_active {
        let status = backend.calibration_poll();
        ui.calib_status = status;
        animating = true;
        if !matches!(status, CalibrationStatus::Running { .. }) {
            ui.calib_active = false;
        }
    }
    ui.latency_ms = backend.latency_ms();
    // Track the neck-window settings + instrument into the stage before the
    // leaf/dots read fret_start/fret_count this frame.
    ui.sync_neck();
    // Two-way the card-rename buffer: adopt the selected card's label when the
    // selection moves, else commit what was typed.
    ui.sync_card_rename();
    // Device-originated Card starts/completions can arrive without an input
    // dispatch. Publish their history/cursor through the ordinary guarded save
    // path, including the final completion after the user presses Next.
    if ui.practice_history.len() != previous_history || ui.rehearsal_set().cursor != previous_cursor
    {
        crate::capture_persistence::persist(shared, ui);
    }
    animating
}

/// The clock-out master values this frame, for the MIDI seam. Read after
/// [`frame`], because the transport may have moved during it.
pub fn clock_out(ui: &UiState) -> (bool, bool, f32) {
    (ui.midi.clock_out, ui.transport.playing, ui.transport.bpm)
}

/// An occurrence edit, cursor change, tempo change or resume starts a fresh
/// dwell. The old instruction's elapsed time must not advance its replacement.
#[cfg(test)]
fn refresh_rehearsal_clock(
    last: &mut Option<std::time::Instant>,
    instruction: &mut Option<String>,
    ui: &UiState,
) {
    let current = if ui.rehearsal_running {
        ui.rehearsal_card().map(|card| {
            serde_json::to_string(&(
                ui.rehearsal_owner,
                card,
                ui.transport.bpm.to_bits(),
                ui.card_started_ms,
            ))
            .expect("Card and clock state are serializable")
        })
    } else {
        None
    };
    if current != *instruction {
        *last = None;
        *instruction = current;
    }
}

/// Explicit control-side preview admission; no fallback to the void audio API.
pub(crate) trait CardPreviewAdmission: AudioBackend {
    fn admit_card(
        &mut self,
        preview: &woodshed_core::card_timing::CardPreview,
    ) -> Result<(), String>;
    fn admit_audition(
        &mut self,
        _preview: &woodshed_core::card_timing::CardPreview,
    ) -> Result<(), String> {
        Err("This host cannot confirm audition admission.".into())
    }
}
impl CardPreviewAdmission for crate::audio::CpalBackend {
    fn admit_card(
        &mut self,
        preview: &woodshed_core::card_timing::CardPreview,
    ) -> Result<(), String> {
        self.admit_card_preview(preview)
    }
    fn admit_audition(
        &mut self,
        preview: &woodshed_core::card_timing::CardPreview,
    ) -> Result<(), String> {
        self.admit_audition_preview(preview)
    }
}

pub(crate) fn service_audition_request(
    ui: &mut UiState,
    backend: &mut impl CardPreviewAdmission,
    request: &woodshed_core::audio::AuditionRequest,
) {
    if ui.audition_is_current(request) {
        let result = backend.admit_audition(&request.preview);
        ui.finish_audition_admission(request, result);
    }
}
#[cfg(test)]
fn service_card_requests(ui: &mut UiState, backend: &mut impl CardPreviewAdmission) {
    // Consume only the leading audition/Card/cancel commands; other requests keep
    // their ordering and remain for the ordinary dispatch tail.
    loop {
        match ui.audio_requests.first() {
            Some(woodshed_core::audio::AudioRequest::Audition(_)) => {
                let woodshed_core::audio::AudioRequest::Audition(request) =
                    ui.audio_requests.remove(0)
                else {
                    unreachable!()
                };
                service_audition_request(ui, backend, &request);
            },
            Some(woodshed_core::audio::AudioRequest::CardPreview(_)) => {
                let woodshed_core::audio::AudioRequest::CardPreview(request) =
                    ui.audio_requests.remove(0)
                else {
                    unreachable!()
                };
                if ui.card_preview_is_current(&request) {
                    let result = backend.admit_card(&request.preview);
                    ui.finish_card_preview_admission(&request, result);
                }
            },
            Some(woodshed_core::audio::AudioRequest::CancelPreview) => {
                ui.audio_requests.remove(0);
                backend.cancel_preview();
            },
            _ => break,
        }
    }
}

/// The rehearsal set's dwell transport: hold each card for its own dwell, then
/// advance and voice what you land on.
#[cfg(test)]
fn rehearsal_dwell(
    last: &mut Option<std::time::Instant>,
    ui: &mut UiState,
    backend: &mut impl CardPreviewAdmission,
    now: std::time::Instant,
) -> bool {
    service_card_requests(ui, backend);
    if !ui.rehearsal_running || ui.rehearsal_set().cards.is_empty() {
        *last = None;
        return false;
    }
    if ui.rehearsal_pending {
        *last = None;
        return true;
    }
    let resolved = ui
        .rehearsal_card()
        .map(|card| woodshed_core::card_dwell(card, ui.transport.bpm));
    let dwell = match resolved {
        Some(Ok(Some(dwell))) => dwell,
        Some(Ok(None)) => {
            *last = None;
            return true;
        }, // admitted Manual
        Some(Err(error)) => {
            ui.stop_rehearsal();
            ui.card_timing_notice = Some(error.to_string());
            *last = None;
            return false;
        },
        None => {
            *last = None;
            return false;
        },
    };
    match last {
        Some(t) if now.duration_since(*t) >= dwell => {
            if ui.advance_rehearsal_cursor() {
                service_card_requests(ui, backend);
            } else {
                ui.stop_rehearsal();
            }
            *last = Some(now);
        },
        None => *last = Some(now),
        _ => {},
    }
    ui.rehearsal_running
}

#[cfg(test)]
#[path = "drive/tests.rs"]
mod tests;
