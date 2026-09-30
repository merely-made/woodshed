//! The per-frame drive: everything that advances because time passed.
//!
//! The tuner's reading, the song's bar, the rehearsal dwell clock, the arpeggio
//! and exercise step clocks, incoming MIDI, and the latency calibration poll.
//! Returns whether any of them wants another frame — the host keeps frames
//! coming while it does, and sleeps when it does not.

use woodshed_core::audio::{AudioBackend, CalibrationStatus};
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
    ui.tuner.enabled
        || ui.song_playing
        || ui.rehearsal_running
        || ui.stage.arpeggio_playing
        || ui.stage.exercise_playing
        || ui.stage.scale_run_playing
        || ui.calib_active
        || (shared.midi.connected_input().is_some()
            && (ui.midi.clock_slave || ui.section == woodshed_core::storage::AppSection::Settings))
}

/// Advance the live clocks against `ui`. Returns `true` while something is
/// animating.
pub fn frame(shared: &mut Shared, ui: &mut UiState) -> bool {
    let mut animating = false;
    // Poll the MIDI seam (immutable) before borrowing the backend.
    let midi_in_connected = shared.midi.connected_input().is_some();
    let midi_clock_bpm = shared.midi.clock_bpm();
    let midi_events = shared.midi.recent_events();
    // Refresh frame observations from the host clock. Interaction handlers
    // independently sample the injected event clock, since an idle view may
    // have no intervening frames for a long pause.
    ui.now_ms = wall_time_ms();

    refresh_rehearsal_clock(
        &mut shared.last_rehearsal_step,
        &mut shared.last_rehearsal_instruction,
        ui,
    );

    let Some(backend) = shared.backend.as_mut() else {
        return false;
    };
    let now = std::time::Instant::now();

    if ui.tuner.enabled {
        ui.tuner.reading = backend.tuner_reading();
        animating = true;
    }
    if ui.song_playing {
        if let Some(bar) = backend.song_bar() {
            ui.song_bar_live = bar;
        }
        ui.song_recording = backend.song_recording();
        ui.song_loop_bars = backend.song_loop_bars();
        animating = true;
    }
    animating |= rehearsal_dwell(&mut shared.last_rehearsal_step, ui, backend, now);
    animating |= transport_steps(&mut shared.last_arp_step, ui, backend, now);

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
            let bpm = bpm.clamp(30.0, 300.0);
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
    animating
}

/// The clock-out master values this frame, for the MIDI seam. Read after
/// [`frame`], because the transport may have moved during it.
pub fn clock_out(ui: &UiState) -> (bool, bool, f32) {
    (ui.midi.clock_out, ui.transport.playing, ui.transport.bpm)
}

/// An occurrence edit, cursor change, tempo change or resume starts a fresh
/// dwell. The old instruction's elapsed time must not advance its replacement.
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

/// The rehearsal set's dwell transport: hold each card for its own dwell, then
/// advance and voice what you land on.
fn rehearsal_dwell(
    last: &mut Option<std::time::Instant>,
    ui: &mut UiState,
    backend: &mut impl AudioBackend,
    now: std::time::Instant,
) -> bool {
    if !ui.rehearsal_running || ui.rehearsal_set().cards.is_empty() {
        *last = None;
        return false;
    }
    let Some(dwell) = ui
        .rehearsal_card()
        .and_then(|card| woodshed_core::card_dwell(card, ui.transport.bpm))
    else {
        // Manual card: the dwell transport waits here.
        *last = None;
        return true;
    };
    match last {
        Some(t) if now.duration_since(*t) >= dwell => {
            if ui.advance_rehearsal_cursor() {
                // Landed on a new card — voice its material ("hear it as you
                // land").
                if let Some((pitches, d, strum)) = ui.rehearsal_sounding_pitches() {
                    if !pitches.is_empty() {
                        backend.preview_pitches(&pitches, d, strum);
                    }
                }
            } else {
                // End of set, loop off: stop.
                ui.stop_rehearsal();
            }
            *last = Some(now);
        },
        None => *last = Some(now),
        _ => {},
    }
    true
}

/// The arpeggio / exercise / scale-run step clock: one step per beat at the
/// transport bpm, sonified as it lands.
fn transport_steps(
    last: &mut Option<std::time::Instant>,
    ui: &mut UiState,
    backend: &mut crate::audio::CpalBackend,
    now: std::time::Instant,
) -> bool {
    let stepping =
        ui.stage.arpeggio_playing || ui.stage.exercise_playing || ui.stage.scale_run_playing;
    if !stepping {
        *last = None;
        return false;
    }
    let beat = std::time::Duration::from_secs_f32(60.0 / ui.transport.bpm.max(30.0));
    match last {
        Some(t) if now.duration_since(*t) >= beat => {
            // Sonify the step we land on — the arpeggio climbs audibly, the
            // exercise plays its notes.
            let note_secs = beat.as_secs_f32() * 0.85;
            if ui.stage.arpeggio_playing {
                ui.stage.arpeggio_advance();
                if let Some(freq) = ui.stage.arpeggio_current_pitch_hz() {
                    backend.preview_note(freq, note_secs);
                }
            }
            if ui.stage.exercise_playing {
                ui.stage.exercise_advance();
                if let Some(freq) = ui.stage.exercise_current_pitch_hz() {
                    backend.preview_note(freq, note_secs);
                }
            }
            if ui.stage.scale_run_playing {
                if let Some(freq) = ui.stage.scale_run_tick() {
                    backend.preview_note(freq, note_secs);
                }
            }
            *last = Some(now);
        },
        None => *last = Some(now),
        _ => {},
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordedAudio {
        previews: Vec<(Vec<f32>, f32, f32)>,
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
            card.setting.tuning = None;
            card.setting.mark_mode = MarkMode::Solo;
            card.setting.marked = vec![(0, 0)];
        }
        let owner = ui.working_sets.active_id;
        ui.toggle_rehearsal();
        let expected_sound = ui.rehearsal_sounding_pitches().unwrap();
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
        assert_eq!(ui.rehearsal_sounding_pitches().unwrap(), expected_sound);
        assert_ne!(
            ui.stage
                .card_sounding_pitches_at_tempo(ui.rehearsal_card().unwrap(), ui.transport.bpm),
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
        assert!(rehearsal_dwell(
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
