//! View-side binding for the host-owned, device-clocked Set program.
//!
//! This module carries a disposable lowered source and projects host snapshots
//! into the current Card/event. It has no dependency on the audio runtime and
//! does not persist clock state.

use super::UiState;
use woodshed_core::PracticeProgram;
use woodshed_core::history::EngagementKind;
use woodshedding::rehearsal::{Card, CardId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeClockStatus {
    Stopped,
    Playing,
    Paused,
    WaitingManual,
    Refused,
    Complete,
    WaitingObserver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeCardEndReason {
    Completed,
    Seek,
    Stop,
}

/// A host-to-view clock projection. `card_index` is in the frozen playback
/// program's order, not necessarily the current Set's physical order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeClockObservation {
    pub epoch: u64,
    pub owner: woodshed_core::working_sets::WorkingSetId,
    pub card_index: usize,
    pub event_ordinal: Option<u32>,
    pub device_frame: u64,
    pub active_frames: u64,
    pub sample_rate: u32,
    pub beat_ticks: u64,
    /// Scale supplied by the audio host; views do not assume its tick unit.
    pub ticks_per_beat: u64,
    pub card_frames: u64,
    pub status: PracticeClockStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeClockProjection {
    pub epoch: u64,
    pub owner: woodshed_core::working_sets::WorkingSetId,
    pub card_id: CardId,
    pub card_index: usize,
    pub event_ordinal: Option<u32>,
    pub contacts: Vec<(usize, u8)>,
    pub device_frame: u64,
    pub active_frames: u64,
    pub sample_rate: u32,
    pub beat_ticks: u64,
    pub ticks_per_beat: u64,
    pub card_frames: u64,
    pub status: PracticeClockStatus,
}

/// Explicit user control of the active program. `Advance` is distinct from
/// `Seek`: advancing a Manual card is a completion, while moving the cursor is
/// an interruption of the current observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeClockCommand {
    Advance,
    Seek(usize),
}

impl UiState {
    /// Enable the device-clock managed Run/Pause path. Hosts call this on
    /// startup and after restoring a session; it deliberately is not persisted.
    pub fn enable_practice_clock(&mut self) {
        self.practice_clock_managed = true;
    }

    /// The exact disposable program frozen at Run. Card tokens are one-based
    /// indexes into this playback order; event tokens are one-based ordinals.
    pub fn practice_clock_program(&self) -> Option<PracticeProgram> {
        self.rehearsal_clock_program.clone()
    }

    /// Original Set index passed to the host as its starting cursor. The
    /// frozen program stays in source order; it is never rotated at Run.
    pub fn practice_clock_start_index(&self) -> usize {
        self.rehearsal_clock_start_cursor
    }

    pub fn practice_clock_loop_mode(&self) -> woodshedding::rehearsal::LoopMode {
        self.rehearsal_set().loop_mode
    }

    pub fn take_practice_clock_commands(&mut self) -> Vec<PracticeClockCommand> {
        std::mem::take(&mut self.practice_clock_commands)
    }

    /// Copy the frozen schedule and update its current source Card/BPM values.
    /// Event contacts and source order remain fixed; changed Cards or Set order
    /// require stopping the old epoch.
    pub fn live_practice_program_for_host(&self) -> Result<PracticeProgram, String> {
        self.validate_live_clock_source()?;
        let set = self.rehearsal_set();
        let mut live = self
            .rehearsal_clock_program
            .as_ref()
            .expect("validated frozen program")
            .clone();
        for (lowered, card) in live.cards.iter_mut().zip(&set.cards) {
            lowered.card = card.clone();
            lowered.bpm = card.timing.bpm.unwrap_or(self.transport.bpm);
        }
        Ok(live)
    }

    pub(super) fn validate_live_clock_source(&self) -> Result<(), String> {
        let owner = self.rehearsal_owner.ok_or_else(|| {
            "The rehearsal Set owner is unavailable; stop and start Run again.".to_string()
        })?;
        let source_order = &self.rehearsal_clock_source_order;
        let set = self.working_sets.get(owner, &self.set).ok_or_else(|| {
            "The rehearsal Set owner was removed; stop and start Run again.".to_string()
        })?;
        let live_order = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        if live_order != *source_order {
            return Err("The rehearsed Set order changed; stop and start Run again.".into());
        }
        let Some(frozen) = self.rehearsal_clock_program.as_ref() else {
            return Err("The frozen rehearsal program is unavailable.".into());
        };
        if self.rehearsal_stage.is_none() {
            return Err(
                "The frozen rehearsal setup is unavailable; stop and start Run again.".into(),
            );
        }
        if frozen.cards.len() != set.cards.len() {
            return Err("The rehearsed Set contents changed; stop and start Run again.".into());
        }
        for (lowered, card) in frozen.cards.iter().zip(&set.cards) {
            if !same_card_without_bpm(&lowered.card, card) {
                return Err(
                    "The rehearsed Card material changed; stop and start Run again.".into(),
                );
            }
        }
        Ok(())
    }

    pub fn queue_practice_clock_command(&mut self, command: PracticeClockCommand) {
        if self.practice_clock_managed && self.rehearsal_running {
            self.practice_clock_commands.push(command);
        }
    }

    /// Accept the program's Started transition only while the exact run
    /// source is still current. A device transition is not a durable source
    /// authority and cannot open a span for a stale or replaced Card.
    pub fn practice_card_started(&mut self, observation: PracticeClockObservation) -> bool {
        if !self.practice_clock_managed
            || !self.rehearsal_running
            || observation.sample_rate == 0
            || observation.ticks_per_beat == 0
            || !matches!(
                observation.status,
                PracticeClockStatus::Playing
                    | PracticeClockStatus::Paused
                    | PracticeClockStatus::WaitingManual
            )
            || !self.clock_observation_matches(&observation)
            || self.rehearsal_observed_card.is_some()
        {
            return false;
        }
        let Some(card) = self
            .rehearsal_clock_program
            .as_ref()
            .and_then(|program| program.cards.get(observation.card_index))
            .map(|card| card.card.clone())
        else {
            return false;
        };
        let Some(source_index) = self
            .rehearsal_set()
            .cards
            .iter()
            .position(|item| item.id == card.id)
        else {
            return false;
        };
        self.rehearsal_set_mut().cursor = source_index;
        self.rehearsal_pending = false;
        self.rehearsal_active_frames = observation.active_frames;
        self.rehearsal_sample_rate = Some(observation.sample_rate);
        self.card_started_ms = self.now_ms;
        self.rehearsal_observed_midi = None;
        self.rehearsal_observed_card = Some(card.clone());
        self.practice_clock_projection = Some(self.projection_for(&observation, card.id));
        self.card_timing_notice = None;
        self.open_practice_history(&card, observation.owner);
        true
    }

    /// Update device-frame progress and active contacts for the matching
    /// Started Card. Event ordinals, rather than pitches, distinguish repeated
    /// physical contacts and repeated notes.
    pub fn observe_practice_clock(&mut self, observation: PracticeClockObservation) -> bool {
        if !self.practice_clock_managed
            || observation.sample_rate == 0
            || observation.ticks_per_beat == 0
            || !self.clock_observation_matches(&observation)
        {
            return false;
        }
        let Some(card_id) = self
            .rehearsal_clock_program
            .as_ref()
            .and_then(|program| program.cards.get(observation.card_index))
            .map(|card| card.card.id)
        else {
            return false;
        };
        if self.rehearsal_observed_card.as_ref().map(|card| card.id) != Some(card_id) {
            return false;
        }
        self.rehearsal_active_frames = observation.active_frames;
        self.rehearsal_sample_rate = Some(observation.sample_rate);
        self.practice_clock_projection = Some(self.projection_for(&observation, card_id));
        true
    }

    pub fn practice_card_ended(
        &mut self,
        epoch: u64,
        owner: woodshed_core::working_sets::WorkingSetId,
        card_index: usize,
        reason: PracticeCardEndReason,
        active_frames: u64,
        sample_rate: u32,
    ) -> bool {
        if !self.practice_clock_managed
            || !self.rehearsal_running
            || self.rehearsal_epoch != epoch
            || self.rehearsal_owner != Some(owner)
            || self.working_sets.get(owner, &self.set).is_none()
            || sample_rate == 0
            || self
                .rehearsal_clock_program
                .as_ref()
                .and_then(|program| program.cards.get(card_index))
                .map(|card| card.card.id)
                != self.rehearsal_observed_card.as_ref().map(|card| card.id)
        {
            return false;
        }
        self.rehearsal_active_frames = active_frames;
        self.rehearsal_sample_rate = Some(sample_rate);
        let kind = if reason == PracticeCardEndReason::Completed {
            EngagementKind::Completed
        } else {
            EngagementKind::Rehearsed
        };
        self.finish_rehearsal_observation(kind);
        self.practice_clock_projection = None;
        self.rehearsal_pending = reason != PracticeCardEndReason::Stop;
        if reason == PracticeCardEndReason::Stop {
            self.rehearsal_running = false;
            self.rehearsal_paused = false;
            self.rehearsal_owner = None;
            self.rehearsal_stage = None;
            self.rehearsal_clock_program = None;
            self.rehearsal_clock_source_order.clear();
            self.rehearsal_observed_midi = None;
            self.rehearsal_active_frames = 0;
            self.rehearsal_sample_rate = None;
            self.practice_clock_commands.clear();
            self.stage_clock_event = None;
            self.stage_clock_contacts.clear();
            self.stage_clock_beat_ticks = 0;
        }
        true
    }

    pub fn practice_card_refused(
        &mut self,
        epoch: u64,
        owner: woodshed_core::working_sets::WorkingSetId,
        card_index: usize,
        reason: String,
    ) -> bool {
        if !self.practice_clock_managed
            || !self.rehearsal_running
            || self.rehearsal_epoch != epoch
            || self.rehearsal_owner != Some(owner)
            || self
                .rehearsal_clock_program
                .as_ref()
                .and_then(|program| program.cards.get(card_index))
                .is_none()
        {
            return false;
        }
        let Some(failed_card) = self
            .rehearsal_clock_program
            .as_ref()
            .and_then(|program| program.cards.get(card_index))
            .map(|card| card.card.clone())
        else {
            return false;
        };
        if card_index < self.rehearsal_set().cards.len() {
            self.rehearsal_set_mut().cursor = card_index;
        }
        // Only previously Started cards have an open span to close. The refused
        // card itself never gets an observation or a cursor completion.
        if self.rehearsal_observed_card.is_some() {
            self.finish_rehearsal_observation(EngagementKind::Rehearsed);
        }
        let prefix = format!("Set {} · Card {}:", owner.0, failed_card.id.0);
        let attributed = if reason.starts_with(&prefix) {
            reason
        } else {
            format!("{prefix} {reason}")
        };
        let attributed = if owner != self.working_sets.active_id
            && !attributed.contains("Select this Set to retry")
        {
            format!("{attributed} Select this Set to retry the Card.")
        } else {
            attributed
        };
        self.card_timing_notice = Some(attributed);
        self.rehearsal_running = false;
        self.rehearsal_pending = false;
        self.rehearsal_paused = false;
        self.rehearsal_epoch = self.rehearsal_epoch.wrapping_add(1);
        self.rehearsal_owner = None;
        self.rehearsal_stage = None;
        self.rehearsal_clock_program = None;
        self.rehearsal_clock_source_order.clear();
        self.rehearsal_observed_midi = None;
        self.rehearsal_active_frames = 0;
        self.rehearsal_sample_rate = None;
        self.practice_clock_projection = None;
        self.practice_clock_commands.clear();
        self.stage_clock_event = None;
        self.stage_clock_contacts.clear();
        self.stage_clock_beat_ticks = 0;
        true
    }

    fn clock_observation_matches(&self, observation: &PracticeClockObservation) -> bool {
        self.rehearsal_epoch == observation.epoch
            && self.rehearsal_owner == Some(observation.owner)
            && self
                .rehearsal_clock_program
                .as_ref()
                .and_then(|program| program.cards.get(observation.card_index))
                .is_some_and(|card| {
                    self.rehearsal_set()
                        .cards
                        .iter()
                        .any(|source| source.id == card.card.id)
                })
            && self.validate_live_clock_source().is_ok()
    }

    fn projection_for(
        &self,
        observation: &PracticeClockObservation,
        card_id: CardId,
    ) -> PracticeClockProjection {
        let contacts = observation
            .event_ordinal
            .and_then(|ordinal| {
                self.rehearsal_clock_program
                    .as_ref()?
                    .cards
                    .get(observation.card_index)?
                    .events
                    .iter()
                    .find(|event| event.ordinal == ordinal)
                    .map(|event| event.contacts.clone())
            })
            .unwrap_or_default();
        PracticeClockProjection {
            epoch: observation.epoch,
            owner: observation.owner,
            card_id,
            card_index: observation.card_index,
            event_ordinal: observation.event_ordinal,
            contacts,
            device_frame: observation.device_frame,
            active_frames: observation.active_frames,
            sample_rate: observation.sample_rate,
            beat_ticks: observation.beat_ticks,
            ticks_per_beat: observation.ticks_per_beat,
            card_frames: observation.card_frames,
            status: observation.status,
        }
    }

    fn open_practice_history(
        &mut self,
        card: &Card,
        owner: woodshed_core::working_sets::WorkingSetId,
    ) {
        let Some(subject) = woodshed_core::history::catalog_id_for_card(card) else {
            return;
        };
        let provenance =
            woodshed_core::history::ObservationProvenance::capture_in_set(card, owner).ok();
        self.practice_history.record_observation(
            self.now_ms,
            subject,
            woodshed_core::history::EngagementKind::Rehearsed,
            None,
            None,
            provenance,
        );
    }
}

impl UiState {
    fn begin_managed_rehearsal(&mut self) {
        self.rehearsal_epoch = self.rehearsal_epoch.wrapping_add(1);
        self.rehearsal_owner = Some(self.working_sets.active_id);
        self.capture_runner_stage();
        let set = self.rehearsal_set().clone();
        let ids = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        let cursor = set.cursor.min(set.cards.len().saturating_sub(1));
        let stage = self
            .rehearsal_stage
            .as_ref()
            .expect("captured runner Stage");
        self.rehearsal_clock_program = Some(stage.practice_program(&set, self.transport.bpm));
        self.rehearsal_clock_source_order = ids;
        self.rehearsal_clock_start_cursor = cursor;
        self.practice_clock_commands.clear();
        self.practice_clock_projection = None;
        self.rehearsal_active_frames = 0;
        self.rehearsal_sample_rate = None;
        // A clock snapshot reports only the current event, not a receipt for
        // every event that completed between UI polls.
        self.rehearsal_observed_midi = None;
        self.rehearsal_observed_card = None;
        self.card_started_ms = None;
        self.rehearsal_pending = true;
        self.rehearsal_paused = false;
        self.rehearsal_running = true;
    }

    pub(super) fn toggle_managed_rehearsal(&mut self) {
        if self.rehearsal_running && self.is_current_set_rehearsing() {
            if self.rehearsal_paused {
                self.rehearsal_paused = false;
            } else {
                self.rehearsal_paused = true;
            }
            return;
        }
        if self.rehearsal_running {
            self.stop_rehearsal();
        }
        if !self.set.cards.is_empty() {
            self.begin_managed_rehearsal();
        }
    }

    pub fn practice_program_finished(
        &mut self,
        epoch: u64,
        owner: woodshed_core::working_sets::WorkingSetId,
    ) -> bool {
        if !self.practice_clock_managed
            || !self.rehearsal_running
            || self.rehearsal_epoch != epoch
            || self.rehearsal_owner != Some(owner)
        {
            return false;
        }
        self.practice_clock_projection = None;
        self.practice_clock_commands.clear();
        self.rehearsal_clock_program = None;
        self.rehearsal_clock_source_order.clear();
        self.rehearsal_paused = false;
        self.rehearsal_pending = false;
        self.rehearsal_running = false;
        self.rehearsal_owner = None;
        self.rehearsal_stage = None;
        self.stage_clock_event = None;
        self.stage_clock_contacts.clear();
        self.stage_clock_beat_ticks = 0;
        true
    }
}

fn same_card_without_bpm(a: &Card, b: &Card) -> bool {
    fn normalized(card: &Card) -> Option<serde_json::Value> {
        let mut value = serde_json::to_value(card).ok()?;
        if let Some(timing) = value
            .get_mut("timing")
            .and_then(serde_json::Value::as_object_mut)
        {
            timing.remove("bpm");
        }
        Some(value)
    }
    matches!((normalized(a), normalized(b)), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::super::UiState;
    use super::*;
    use woodshedding::rehearsal::Set;
    use woodshedding::rehearsal::{Hold, Material, Timing};

    fn managed_ui() -> UiState {
        let mut ui = UiState::new();
        ui.practice_clock_managed = true;
        ui.stage_current(None);
        // These fixtures isolate Run admission from the preceding Stage action.
        ui.practice_history = Default::default();
        ui.set.cards[0].timing = Timing {
            bpm: None,
            hold: Hold::Bars(1),
        };
        ui
    }

    #[test]
    fn managed_run_freezes_exact_owner_program_and_tempo_retime_preserves_source() {
        let mut ui = managed_ui();
        let card_id = ui.set.cards[0].id;
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;
        assert!(ui.rehearsal_running && ui.rehearsal_pending);
        assert_eq!(
            ui.rehearsal_clock_program.as_ref().unwrap().cards[0]
                .card
                .id,
            card_id
        );
        assert!(ui.live_practice_program_for_host().is_ok());

        ui.transport.bpm = 400.0;
        assert!(ui.live_practice_program_for_host().is_ok());
        assert_eq!(ui.rehearsal_epoch, epoch);
        assert!(ui.rehearsal_clock_program.is_some());
    }

    #[test]
    fn started_only_opens_history_and_pause_resume_preserves_the_same_observation() {
        let mut ui = managed_ui();
        ui.now_ms = Some(100);
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;
        let owner = ui.rehearsal_owner.unwrap();
        let started = PracticeClockObservation {
            epoch,
            owner,
            card_index: 0,
            event_ordinal: Some(0),
            device_frame: 4800,
            active_frames: 0,
            sample_rate: 48_000,
            beat_ticks: 0,
            ticks_per_beat: 960,
            card_frames: 0,
            status: PracticeClockStatus::Playing,
        };
        assert!(ui.practice_history.is_empty());
        assert!(ui.practice_card_started(started.clone()));
        assert_eq!(ui.practice_history.len(), 1);
        let open = ui.rehearsal_observed_card.as_ref().unwrap().id;
        ui.toggle_rehearsal();
        assert!(ui.rehearsal_paused);
        assert_eq!(ui.rehearsal_epoch, epoch);
        assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, open);
        ui.toggle_rehearsal();
        assert!(!ui.rehearsal_paused);
        assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, open);
        assert!(!ui.practice_card_started(started));
    }

    #[test]
    fn pause_intent_during_startup_does_not_discard_a_real_started_transition() {
        let mut ui = managed_ui();
        ui.now_ms = Some(100);
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;
        let owner = ui.rehearsal_owner.unwrap();
        ui.toggle_rehearsal();
        assert!(ui.rehearsal_paused);
        let started = PracticeClockObservation {
            epoch,
            owner,
            card_index: 0,
            event_ordinal: None,
            device_frame: 4800,
            active_frames: 0,
            sample_rate: 48_000,
            beat_ticks: 0,
            ticks_per_beat: 960,
            card_frames: 0,
            status: PracticeClockStatus::Paused,
        };
        assert!(ui.practice_card_started(started));
        assert!(ui.rehearsal_paused);
        assert_eq!(ui.practice_history.len(), 1);
    }

    #[test]
    fn nonzero_start_cursor_keeps_original_program_order_and_card_tokens() {
        let mut ui = managed_ui();
        let first = ui.set.cards[0].clone();
        ui.set.push(first);
        let second_id = ui.set.cards[1].id;
        ui.set.cursor = 1;
        ui.toggle_rehearsal();
        let program = ui.practice_clock_program().unwrap();
        assert_eq!(program.cards[0].card.id, ui.set.cards[0].id);
        assert_eq!(program.cards[1].card.id, second_id);
        assert_eq!(ui.practice_clock_start_index(), 1);

        let owner = ui.rehearsal_owner.unwrap();
        let epoch = ui.rehearsal_epoch;
        assert!(ui.practice_card_started(PracticeClockObservation {
            epoch,
            owner,
            card_index: 1,
            event_ordinal: None,
            device_frame: 0,
            active_frames: 0,
            sample_rate: 48_000,
            beat_ticks: 0,
            ticks_per_beat: 960,
            card_frames: 0,
            status: PracticeClockStatus::Playing,
        }));
        assert_eq!(ui.rehearsal_set().cursor, 1);
        assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, second_id);
    }

    #[test]
    fn refusal_selects_the_failed_card_for_retry_without_crediting_it() {
        let mut ui = managed_ui();
        let first = ui.set.cards[0].clone();
        ui.set.push(first);
        let failed = ui.set.cards[1].id;
        let owner = ui.working_sets.active_id;
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;

        assert!(ui.practice_card_refused(epoch, owner, 1, "unsupported Card".into()));
        assert_eq!(ui.rehearsal_set().cursor, 1);
        assert!(!ui.rehearsal_running);
        assert!(ui.practice_history.is_empty());
        let notice = ui.card_timing_notice.as_deref().unwrap();
        assert!(notice.contains(&owner.0.to_string()));
        assert!(notice.contains(&failed.0.to_string()));

        ui.toggle_rehearsal();
        assert_eq!(ui.practice_clock_start_index(), 1);
        assert_eq!(
            ui.practice_clock_program().unwrap().cards[1].card.id,
            failed
        );
    }

    #[test]
    fn parked_owner_refusal_keeps_its_cursor_without_switching_sets() {
        let mut ui = managed_ui();
        let first = ui.set.cards[0].clone();
        ui.set.push(first);
        let failed = ui.set.cards[1].id;
        let owner = ui.working_sets.active_id;
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;
        ui.create_working_set_from("Other", Set::default());
        let selected = ui.working_sets.active_id;

        assert!(ui.practice_card_refused(epoch, owner, 1, "unsupported Card".into()));
        assert_eq!(ui.working_sets.active_id, selected);
        assert_eq!(ui.working_sets.get(owner, &ui.set).unwrap().cursor, 1);
        let notice = ui.card_timing_notice.as_deref().unwrap();
        assert!(notice.contains(&failed.0.to_string()));
        assert!(notice.contains("Select this Set to retry"));
    }

    #[test]
    fn frozen_background_owner_survives_exploration_of_another_set() {
        let mut ui = managed_ui();
        let owner = ui.working_sets.active_id;
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;
        let card_id = ui.rehearsal_clock_program.as_ref().unwrap().cards[0]
            .card
            .id;

        ui.create_working_set_from("Other", Set::default());
        ui.stage.root_idx = (ui.stage.root_idx + 1) % woodshed_core::ROOT_NAMES.len();
        assert_ne!(ui.working_sets.active_id, owner);
        assert!(ui.rehearsal_running);
        assert!(ui.live_practice_program_for_host().is_ok());
        assert!(ui.practice_card_started(PracticeClockObservation {
            epoch,
            owner,
            card_index: 0,
            event_ordinal: None,
            device_frame: 0,
            active_frames: 0,
            sample_rate: 48_000,
            beat_ticks: 0,
            ticks_per_beat: 960,
            card_frames: 0,
            status: PracticeClockStatus::Playing,
        }));
        assert_eq!(ui.rehearsal_observed_card.as_ref().unwrap().id, card_id);
        assert_eq!(ui.rehearsal_set().cursor, 0);
    }

    #[test]
    fn persisted_session_clears_clock_but_keeps_host_managed_mode() {
        let mut ui = managed_ui();
        ui.enable_practice_clock();
        ui.toggle_rehearsal();
        ui.rehearsal_paused = true;
        let saved = ui.to_persisted();
        let app_settings = ui.app_settings.clone();

        ui.apply_persisted(&saved, app_settings);

        assert!(ui.practice_clock_managed);
        assert!(!ui.rehearsal_running);
        assert!(!ui.rehearsal_paused);
        assert!(ui.rehearsal_clock_program.is_none());
        assert!(ui.practice_clock_projection.is_none());
        assert!(ui.practice_clock_commands.is_empty());
    }

    #[test]
    fn changed_card_source_refuses_stale_started_transition_without_crediting_it() {
        let mut ui = managed_ui();
        ui.toggle_rehearsal();
        let epoch = ui.rehearsal_epoch;
        let owner = ui.rehearsal_owner.unwrap();
        ui.set.cards[0].material = Material::Riff {
            name: "changed".into(),
        };
        let observation = PracticeClockObservation {
            epoch,
            owner,
            card_index: 0,
            event_ordinal: Some(0),
            device_frame: 10,
            active_frames: 0,
            sample_rate: 48_000,
            beat_ticks: 0,
            ticks_per_beat: 960,
            card_frames: 0,
            status: PracticeClockStatus::Playing,
        };
        assert!(!ui.practice_card_started(observation));
        assert!(ui.practice_history.is_empty());
    }
}
