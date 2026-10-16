//! Host admission of frozen Card sound; durable timing remains in the Card.
use super::*;
use woodshed_core::audio::CardPreviewRequest;
use woodshed_core::card_timing::{CardPreview, LegacyTimingMinimum};
use woodshed_core::working_sets::WorkingSetId;
use woodshedding::rehearsal::Material;
impl UiState {
    pub(super) fn bound_card_preview(
        &self,
        card: &Card,
        owner: WorkingSetId,
        run_epoch: Option<u64>,
        preview: CardPreview,
    ) -> Result<CardPreviewRequest, String> {
        let stage = if run_epoch.is_some() {
            self.rehearsal_stage.as_ref().unwrap_or(&self.stage)
        } else {
            self.current_card_stage()
        };
        // Marked silence is admissible only after the underlying realization
        // resolves. Legacy preview tuples also use an empty vector for an
        // unavailable setup; that case must not open a successful observation.
        let unavailable =
            |reason: String| format!("Set {} · Card {}: {reason}", owner.0, card.id.0);
        match &card.material {
            Material::Scale { .. } | Material::ScalePattern { .. } => {
                stage
                    .scale_card_realization(card)
                    .map_err(|error| unavailable(error.to_string()))?;
            },
            Material::ChordApproach { .. } => {
                stage
                    .chord_approach_realization(card)
                    .map_err(|error| unavailable(error.to_string()))?;
            },
            Material::Chord { .. } => {
                if let woodshed_core::CardShapeStatus::Unavailable(reason) =
                    stage.card_shape_status(card)
                {
                    return Err(unavailable(reason.to_string()));
                }
            },
            _ => {},
        }
        let unmarked = stage
            .card_voicing(card)
            .map_err(|error| unavailable(error.to_string()))?;
        if unmarked.0.is_empty() && !matches!(card.material, Material::Riff { .. }) {
            return Err(unavailable(
                "The saved material has no available playable realization.".into(),
            ));
        }
        Ok(CardPreviewRequest {
            session: self.audio_session.clone(),
            owner,
            card_id: card.id,
            instruction: serde_json::to_value(card).map_err(|error| error.to_string())?,
            fallback_bpm_bits: self.transport.bpm.to_bits(),
            run_epoch,
            preview,
        })
    }
    pub fn card_preview_is_current(&self, request: &CardPreviewRequest) -> bool {
        if !std::sync::Arc::ptr_eq(&self.audio_session, &request.session) {
            return false;
        }
        let Some(card) = self
            .working_sets
            .get(request.owner, &self.set)
            .and_then(|set| set.cards.iter().find(|card| card.id == request.card_id))
        else {
            return false;
        };
        if serde_json::to_value(card).ok().as_ref() != Some(&request.instruction)
            || (card.timing.bpm.is_none()
                && self.transport.bpm.to_bits() != request.fallback_bpm_bits)
        {
            return false;
        }
        match request.run_epoch {
            Some(epoch) => {
                self.rehearsal_running
                    && self.rehearsal_pending
                    && self.rehearsal_epoch == epoch
                    && self.rehearsal_owner == Some(request.owner)
                    && self
                        .rehearsal_card()
                        .is_some_and(|card| card.id == request.card_id)
            },
            None => true,
        }
    }
    pub fn finish_card_preview_admission(
        &mut self,
        request: &CardPreviewRequest,
        outcome: Result<(), String>,
    ) {
        if !self.card_preview_is_current(request) {
            return;
        }
        if let Err(error) = outcome {
            let reason = format!(
                "Set {} · Card {}: {error}",
                request.owner.0, request.card_id.0
            );
            if request.run_epoch.is_some() {
                self.refuse_current_rehearsal(reason);
            } else {
                self.card_timing_notice = Some(reason);
            }
            return;
        }
        self.card_timing_notice = None;
        self.refresh_event_time();
        let Some(card) = self
            .working_sets
            .get(request.owner, &self.set)
            .and_then(|set| set.cards.iter().find(|card| card.id == request.card_id))
            .cloned()
        else {
            return;
        };
        let midi = request
            .preview
            .0
            .iter()
            .map(|hz| (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32)
            .collect();
        let kind = if request.run_epoch.is_some() {
            self.rehearsal_pending = false;
            self.card_started_ms = self.now_ms;
            self.rehearsal_observed_card = Some(card.clone());
            self.rehearsal_observed_midi = Some(midi);
            EngagementKind::Rehearsed
        } else {
            EngagementKind::Previewed
        };
        if let Some(subject) = catalog_id_for_card(&card) {
            let mut provenance = ObservationProvenance::capture_in_set(&card, request.owner).ok();
            if let Some(provenance) = &mut provenance {
                if request.run_epoch.is_some() {
                    provenance.presented_midi = self.rehearsal_observed_midi.clone();
                }
            }
            self.practice_history.record_observation(
                self.now_ms,
                subject,
                kind,
                None,
                None,
                provenance,
            );
        }
    }
    pub(super) fn refuse_current_rehearsal(&mut self, reason: String) {
        // No observation/span has opened for this refused instruction.
        self.rehearsal_running = false;
        self.rehearsal_pending = false;
        self.rehearsal_owner = None;
        self.rehearsal_stage = None;
        self.rehearsal_observed_card = None;
        self.rehearsal_observed_midi = None;
        self.card_started_ms = None;
        self.card_timing_notice = Some(reason);
        self.request(AudioRequest::CancelPreview);
    }
    pub(super) fn selected_timing_interpretation(&self) -> Option<String> {
        let card = self.current_card()?;
        match woodshed_core::resolve_card_timing(&card.timing, self.transport.bpm) {
            Err(error) => Some(format!("Card timing unavailable: {error}")),
            Ok(timing) => timing.legacy_minimum.map(|minimum| match minimum {
                LegacyTimingMinimum::HalfSecond => {
                    "Uses the legacy minimum of 0.5 seconds; saved timing is unchanged.".into()
                },
                LegacyTimingMinimum::OneBar => {
                    "Uses the legacy minimum of one bar; saved timing is unchanged.".into()
                },
                LegacyTimingMinimum::OneRep => {
                    "Uses the legacy minimum of one rep; saved timing is unchanged.".into()
                },
            }),
        }
    }
}

#[cfg(test)]
impl UiState {
    /// UI unit fixtures explicitly acknowledge the host seam; native/host tests
    /// exercise real admission rather than treating this as audio evidence.
    pub(crate) fn admit_previews_for_test(&mut self) {
        let requests = std::mem::take(&mut self.audio_requests);
        for request in requests {
            match request {
                AudioRequest::CardPreview(request) => {
                    self.finish_card_preview_admission(&request, Ok(()))
                },
                other => self.audio_requests.push(other),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::rehearsal::Hold;
    fn staged() -> UiState {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Chords);
        ui.stage_current(None);
        ui.set.cards[0].timing.hold = Hold::Bars(1);
        ui.now_ms = Some(42);
        ui
    }
    fn request(ui: &UiState) -> CardPreviewRequest {
        ui.audio_requests
            .iter()
            .rev()
            .find_map(|request| match request {
                AudioRequest::CardPreview(request) if request.run_epoch.is_some() => {
                    Some(request.clone())
                },
                _ => None,
            })
            .unwrap()
    }
    #[test]
    fn repeated_same_millisecond_run_supersedes_prior_success_and_failure() {
        let mut ui = staged();
        let history = ui.practice_history.len();
        ui.toggle_rehearsal();
        let old = request(&ui);
        assert!(ui.rehearsal_pending);
        assert!(ui.card_started_ms.is_none());
        assert_eq!(ui.practice_history.len(), history);
        ui.toggle_rehearsal(); // cancel pending start
        ui.toggle_rehearsal(); // same owner/Card and timestamp, distinct epoch
        let current = request(&ui);
        assert_ne!(old.run_epoch, current.run_epoch);
        ui.finish_card_preview_admission(&old, Err("old failure".into()));
        ui.finish_card_preview_admission(&old, Ok(()));
        assert!(ui.rehearsal_pending);
        assert!(ui.card_timing_notice.is_none());
        assert_eq!(ui.practice_history.len(), history);
        ui.finish_card_preview_admission(&current, Ok(()));
        assert!(!ui.rehearsal_pending);
        assert_eq!(ui.card_started_ms, Some(42));
        assert_eq!(ui.practice_history.len(), history + 1);
    }
    #[test]
    fn refused_timing_and_runtime_admission_do_not_credit_or_rewrite_the_card() {
        let mut ui = staged();
        ui.set.cards[0].timing.hold = Hold::Seconds(1e30);
        let before = serde_json::to_value(&ui.set).unwrap();
        let history = ui.practice_history.len();
        ui.toggle_rehearsal();
        assert!(!ui.rehearsal_running);
        assert!(
            ui.card_timing_notice
                .as_deref()
                .unwrap()
                .contains("Set 1 · Card 1")
        );
        assert_eq!(ui.practice_history.len(), history);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        ui.set.cards[0].timing.hold = Hold::Bars(1);
        ui.set.cards[0].timing.bpm = Some(400.0);
        ui.toggle_rehearsal();
        let req = request(&ui);
        ui.finish_card_preview_admission(&req, Err("PCM frame budget exceeded".into()));
        assert!(!ui.rehearsal_running);
        assert_eq!(ui.practice_history.len(), history);
        assert_eq!(ui.set.cards[0].timing.bpm, Some(400.0));
        ui.toggle_rehearsal();
        ui.finish_card_preview_admission(&request(&ui), Ok(()));
        assert!(ui.rehearsal_running);
        assert!(!ui.rehearsal_pending);
        assert!(ui.card_timing_notice.is_none());
    }
    #[test]
    fn session_owner_and_instruction_bindings_are_checked_and_load_clears_notices() {
        let mut ui = staged();
        ui.toggle_rehearsal();
        let req = request(&ui);
        ui.set.cards[0].timing.bpm = Some(400.0);
        assert!(!ui.card_preview_is_current(&req));
        ui.set.cards[0].timing.bpm = None;
        let mut different_session = req.clone();
        different_session.session = std::sync::Arc::new(());
        assert_ne!(
            different_session, req,
            "Arc unit contents are equal, custody is not"
        );
        assert!(!ui.card_preview_is_current(&different_session));
        let mut other = req.clone();
        other.owner = WorkingSetId(999);
        assert!(!ui.card_preview_is_current(&other));
        let stored = ui.to_persisted();
        ui.card_timing_notice = Some("old source failure".into());
        ui.apply_persisted(&stored, ui.app_settings.clone());
        assert!(ui.card_timing_notice.is_none());
        assert!(!ui.rehearsal_pending);
        assert!(!ui.card_preview_is_current(&req));
    }

    #[test]
    fn unavailable_realization_never_becomes_an_admitted_silent_card() {
        let mut ui = staged();
        ui.stage.set_lens(Lens::Scales);
        ui.set.cards.clear();
        ui.stage_current(None);
        ui.set.cards[0].setting.tuning = Some("Missing saved tuning".into());
        ui.set.cards[0].timing.hold = Hold::Bars(1);
        let before = serde_json::to_value(&ui.set).unwrap();
        let history = ui.practice_history.len();
        ui.audio_requests.clear();
        ui.request_card_preview();
        assert!(
            ui.card_timing_notice
                .as_deref()
                .unwrap()
                .contains("Missing saved tuning")
        );
        assert!(ui.audio_requests.is_empty());
        ui.toggle_rehearsal();
        assert!(!ui.rehearsal_running);
        assert!(!ui.rehearsal_pending);
        assert!(ui.card_started_ms.is_none());
        assert_eq!(ui.practice_history.len(), history);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        assert!(
            !ui.audio_requests
                .iter()
                .any(|request| matches!(request, AudioRequest::CardPreview(_)))
        );
    }

    #[test]
    fn resolved_muted_material_can_request_silence_without_changing_its_instruction() {
        use woodshedding::rehearsal::MarkMode;
        let mut ui = staged();
        // Mute the resolved contacts the Card actually presents on its board.
        ui.set.cards[0].setting.mark_mode = MarkMode::Mute;
        ui.set.cards[0].setting.marked = ui
            .stage
            .dots_for_card(&ui.set.cards[0])
            .iter()
            .map(|dot| (dot.string_index, dot.fret))
            .collect();
        let before = serde_json::to_value(&ui.set).unwrap();
        ui.toggle_rehearsal();
        let request = request(&ui);
        assert!(
            request.preview.0.is_empty(),
            "all resolved chord tones are muted"
        );
        assert!(
            ui.rehearsal_pending,
            "silence still awaits an output operation"
        );
        assert!(ui.card_started_ms.is_none());
        ui.finish_card_preview_admission(&request, Ok(()));
        assert!(!ui.rehearsal_pending);
        assert!(ui.rehearsal_running);
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
    }
}
