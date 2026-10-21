//! The per-frame drive: everything that advances because time passed.
//!
//! The tuner's reading, the song's bar, the rehearsal dwell clock, the arpeggio
//! and exercise step clocks, incoming MIDI, and the latency calibration poll.
//! Returns whether any of them wants another frame — the host keeps frames
//! coming while it does, and sleeps when it does not.

use woodshed_core::audio::{
    AudioBackend, CalibrationStatus, TRANSPORT_MAX_BPM, TRANSPORT_MIN_BPM,
};
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
    recorder: woodshed_views::stage::RecorderObservation,
}

impl AudioObservation {
    pub fn changed(&self, ui: &UiState) -> bool {
        self.playing != ui.song_playing
            || self.bar.is_some_and(|bar| bar != ui.song_bar_live)
            || self.capture_busy != ui.song_recording
            || self.loops != ui.song_loop_bars
            || self.error != ui.audio_error
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
        recorder: backend.recorder_observation(None),
    }
}

/// Advance the live clocks against `ui`. Returns `true` while something is
/// animating.
pub fn frame(shared: &mut Shared, ui: &mut UiState) -> bool {
    if !crate::capture_persistence::session_current(shared, ui) {
        if let Some(backend) = shared.backend.as_mut() {
            backend.service();
            return backend.needs_service();
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
mod tests {
    use super::*;

    #[test]
    fn accepted_one_shot_status_stops_the_ui_without_restarting_or_retargeting_backing() {
        let mut ui = UiState::new();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.set.cards[0].timing.hold = woodshedding::rehearsal::Hold::Bars(1);
        ui.looper.inherited_bpm = 300.0;
        ui.looper.beats = 1;
        ui.looper.one_shot = true;
        ui.looper.click = false;
        ui.loop_working_set(ui.working_sets.active_id);
        let plan = ui.requested_loop_plan().unwrap();
        ui.loop_admitted = Some(plan.clone());
        let payload = serde_json::to_value(ui.to_persisted()).unwrap();
        let mut backend = crate::audio::CpalBackend::offline_for_test();
        backend.set_loop_plan(&plan).unwrap();
        backend.set_song_transport(true);
        // Before a graph block, queued Play remains the accepted control fact.
        observe_audio(&mut backend).apply(&mut ui);
        assert!(ui.song_playing);
        for _ in 0..3 {
            backend.process_for_test(4096);
        }
        // An unrelated dispatch may arrive before any wake/frame publication.
        // Its stale true is not an explicit new Play request.
        backend.set_song_transport(ui.song_playing);
        assert!(!backend.song_playing());
        let stopped = observe_audio(&mut backend);
        assert!(stopped.changed(&ui));
        stopped.apply(&mut ui);
        assert!(!ui.song_playing);
        // Ordinary dispatch now sees false rather than reissuing stale Play.
        backend.set_song_transport(ui.song_playing);
        backend.process_for_test(256);
        assert!(!backend.song_playing());
        // After stopped status has reached the UI, a new Play is authoritative.
        backend.set_song_transport(true);
        assert!(backend.song_playing());
        backend.process_for_test(256);
        assert!(backend.song_playing());
        assert_eq!(ui.loop_admitted, Some(plan));
        assert_eq!(serde_json::to_value(ui.to_persisted()).unwrap(), payload);
        assert!(backend.capture_source_rate().is_none());
    }

    #[derive(Default)]
    struct RecordedAudio {
        previews: Vec<(Vec<f32>, f32, f32)>,
        admissions: usize,
        refuse_on_admission: Option<usize>,
    }

    impl CardPreviewAdmission for RecordedAudio {
        fn admit_card(
            &mut self,
            preview: &woodshed_core::card_timing::CardPreview,
        ) -> Result<(), String> {
            self.admissions += 1;
            if self.refuse_on_admission == Some(self.admissions) {
                return Err("PCM frame budget exceeded".into());
            }
            self.preview_pitches(&preview.0, preview.1, preview.2);
            Ok(())
        }
    }
    impl AudioBackend for RecordedAudio {
        fn set_metronome(&mut self, _: woodshed_core::audio::TransportState) {}
        fn set_tuner_enabled(&mut self, _: bool) {}
        fn tuner_reading(&self) -> Option<woodshed_core::audio::TunerReading> {
            None
        }
        fn set_song(&mut self, _: &woodshed_core::song::SongDoc) {}
        fn set_song_transport(&mut self, _: bool) {}
        fn song_rewind(&mut self) {}
        fn song_bar(&self) -> Option<usize> {
            None
        }
        fn error(&self) -> Option<&str> {
            None
        }
        fn preview_pitches(&mut self, pitches: &[f32], duration: f32, strum: f32) {
            self.previews.push((pitches.to_vec(), duration, strum));
        }
    }

    #[test]
    fn background_dwell_keeps_its_owner_clock_cursor_and_inherited_sound() {
        use woodshedding::rehearsal::{Hold, MarkMode, Set};
        let mut ui = UiState::new();
        ui.now_ms = Some(1_000);
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.stage_current(None);
        for card in &mut ui.set.cards {
            card.timing.hold = Hold::Seconds(0.5);
            // Both absent identities inherit the runner's Stage setup. An
            // explicit Guitar with no tuning selects that instrument's default.
            card.setting.instrument.clear();
            card.setting.tuning = None;
            card.setting.mark_mode = MarkMode::Solo;
            card.setting.marked = vec![(0, 0)];
        }
        let owner = ui.working_sets.active_id;
        ui.toggle_rehearsal();
        crate::tests::acknowledge_card_preview_fixture(&mut ui);
        let expected_sound = ui.rehearsal_sounding_pitches().unwrap().unwrap();
        let observation_started = ui.card_started_ms;
        let observations = ui.practice_history.len();
        let began = std::time::Instant::now();
        let mut last = Some(began);
        let mut instruction = None;
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        last = Some(began);

        // A different editor owner may have the same local CardId; neither its
        // edits nor its visible setup may alter the running owner's dwell.
        let mut other = Set::default();
        let mut card = ui.set.cards[0].clone();
        card.label = "Foreground instruction".into();
        other.push(card);
        ui.working_sets.create(&mut ui.set, "Other Set", other);
        ui.set.cards[0].timing.bpm = Some(137.0);
        let drop_d = woodshed_core::tunings()
            .iter()
            .position(|t| t.name == "Drop D")
            .unwrap();
        ui.stage.set_tuning(drop_d);
        ui.tuning_dd.selected = drop_d;
        ui.now_ms = Some(1_500);
        ui.sync();
        assert_eq!(ui.card_started_ms, observation_started);
        assert_eq!(ui.practice_history.len(), observations);
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        assert_eq!(last, Some(began));
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert_eq!(
            ui.rehearsal_sounding_pitches().unwrap().unwrap(),
            expected_sound
        );
        assert_ne!(
            ui.stage
                .card_sounding_pitches_at_tempo(ui.rehearsal_card().unwrap(), ui.transport.bpm)
                .unwrap(),
            expected_sound,
        );

        ui.now_ms = Some(1_600);
        let mut backend = RecordedAudio::default();
        assert!(rehearsal_dwell(
            &mut last,
            &mut ui,
            &mut backend,
            began + std::time::Duration::from_millis(600),
        ));
        assert_eq!(ui.rehearsal_set().cursor, 1);
        assert_eq!(ui.set.cursor, 0);
        assert_eq!(ui.set.cards[0].label, "Foreground instruction");
        assert_eq!(backend.previews, vec![expected_sound]);
        assert!(ui.rehearsal_running);

        ui.now_ms = Some(2_200);
        assert!(!rehearsal_dwell(
            &mut last,
            &mut ui,
            &mut backend,
            began + std::time::Duration::from_millis(1200),
        ));
        assert!(!ui.rehearsal_running);
        assert_eq!(ui.set.cursor, 0);
        assert_eq!(backend.previews.len(), 1);
    }

    #[test]
    fn refused_next_card_stops_at_that_owner_card_without_credit_then_retry_is_admitted() {
        use woodshedding::rehearsal::Hold;

        let mut ui = UiState::new();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.stage_current(None);
        for card in &mut ui.set.cards {
            card.timing.hold = Hold::Seconds(0.5);
        }
        ui.now_ms = Some(1_000);
        let owner = ui.working_sets.active_id;
        let first = ui.set.cards[0].id;
        let rejected = ui.set.cards[1].id;
        let initial_history = ui.practice_history.len();
        let mut backend = RecordedAudio {
            refuse_on_admission: Some(2),
            ..RecordedAudio::default()
        };
        let began = std::time::Instant::now();
        let mut last = None;

        ui.toggle_rehearsal();
        assert!(rehearsal_dwell(&mut last, &mut ui, &mut backend, began));
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert_eq!(ui.rehearsal_set().cursor, 0);
        assert!(!ui.rehearsal_pending);
        assert_eq!(backend.admissions, 1);
        assert_eq!(ui.practice_history.len(), initial_history + 1);

        // Completing the admitted first Card advances to the second. Its
        // preview is refused, so the run stops on that Card and opens no
        // observation span or practice event for it.
        ui.now_ms = Some(1_600);
        assert!(!rehearsal_dwell(
            &mut last,
            &mut ui,
            &mut backend,
            began + std::time::Duration::from_millis(600),
        ));
        assert!(!ui.rehearsal_running);
        assert!(!ui.rehearsal_pending);
        assert_eq!(ui.rehearsal_owner, None);
        assert_eq!(ui.set.cursor, 1);
        assert_eq!(ui.set.cards[ui.set.cursor].id, rejected);
        assert_eq!(ui.practice_history.len(), initial_history + 2);
        assert!(ui.practice_history.recent(20).iter().any(|event| {
            event
                .provenance
                .as_ref()
                .is_some_and(|p| p.occurrence_id == first)
                && event.kind == woodshed_core::history::EngagementKind::Completed
                && event.provenance.as_ref().and_then(|p| p.working_set_id) == Some(owner)
        }));
        assert!(!ui.practice_history.recent(20).iter().any(|event| {
            event
                .provenance
                .as_ref()
                .is_some_and(|p| p.occurrence_id == rejected)
                && event.kind.is_practice()
        }));
        assert!(
            ui.card_timing_notice
                .as_deref()
                .unwrap()
                .contains("PCM frame budget exceeded")
        );

        // Retrying the same owner and current Card after the transient host
        // refusal succeeds; it does not skip ahead or inherit a foreign Set.
        ui.now_ms = Some(2_000);
        last = None;
        ui.toggle_rehearsal();
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert!(rehearsal_dwell(
            &mut last,
            &mut ui,
            &mut backend,
            began + std::time::Duration::from_secs(1),
        ));
        assert!(ui.rehearsal_running);
        assert!(!ui.rehearsal_pending);
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert_eq!(ui.set.cursor, 1);
        assert_eq!(ui.set.cards[ui.set.cursor].id, rejected);
        assert_eq!(backend.admissions, 3);
        let retried = ui
            .practice_history
            .recent(20)
            .into_iter()
            .find(|event| {
                event
                    .provenance
                    .as_ref()
                    .is_some_and(|p| p.occurrence_id == rejected)
                    && event.kind.is_practice()
            })
            .expect("successful retry opens an observation for the current Card");
        assert_eq!(
            retried.kind,
            woodshed_core::history::EngagementKind::Rehearsed
        );
        assert_eq!(retried.provenance.unwrap().working_set_id, Some(owner));
    }

    #[test]
    fn manual_hold_waits_without_advancing_or_repeating_admission() {
        use woodshedding::rehearsal::Hold;

        let mut ui = UiState::new();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.set.cards[0].timing.hold = Hold::Manual;
        ui.now_ms = Some(1_000);
        let owner = ui.working_sets.active_id;
        let card = ui.set.cards[0].id;
        let mut backend = RecordedAudio::default();
        let began = std::time::Instant::now();
        let mut last = None;

        ui.toggle_rehearsal();
        assert!(rehearsal_dwell(&mut last, &mut ui, &mut backend, began));
        let history_after_admission = ui.practice_history.len();
        ui.now_ms = Some(61_000);
        assert!(rehearsal_dwell(
            &mut last,
            &mut ui,
            &mut backend,
            began + std::time::Duration::from_secs(60),
        ));
        assert!(ui.rehearsal_running);
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert_eq!(ui.set.cursor, 0);
        assert_eq!(ui.set.cards[0].id, card);
        assert_eq!(backend.admissions, 1);
        assert_eq!(ui.practice_history.len(), history_after_admission);
        assert_eq!(last, None, "Manual has no timed dwell clock");
    }

    #[test]
    fn dwell_restarts_after_instruction_edits_cursor_changes_and_pause() {
        let mut ui = UiState::new();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.stage_current(None);
        ui.rehearsal_running = true;
        let mut last = None;
        let mut instruction = None;
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        let started = std::time::Instant::now();
        last = Some(started);
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        assert_eq!(last, Some(started));
        ui.set.cards[0].timing.bpm = Some(72.0);
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        assert!(last.is_none());
        last = Some(started);
        ui.set.cursor = 1;
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        assert!(last.is_none());
        last = Some(started);
        ui.transport.bpm = 96.0;
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        assert!(last.is_none());
        last = Some(started);
        ui.rehearsal_running = false;
        refresh_rehearsal_clock(&mut last, &mut instruction, &ui);
        assert!(last.is_none());
        assert!(instruction.is_none());
    }
}
