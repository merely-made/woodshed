//! Catalog and discovery audition history follows actual host admission.
use super::*;
use std::sync::Arc;
use woodshed_core::audio::{AuditionRequest, PreviewObservation};
use woodshed_core::card_timing::CardPreview;

impl UiState {
    pub(super) fn request_observed_preview(
        &mut self,
        preview: CardPreview,
        subject_id: Option<String>,
        from_id: Option<String>,
        provenance: Option<ObservationProvenance>,
    ) {
        let ticket = Arc::new(());
        let observation = subject_id.map(|subject_id| PreviewObservation {
            subject_id,
            from_id,
            provenance,
        });
        self.pending_auditions.push(ticket.clone());
        self.latest_audition = Some(ticket.clone());
        self.audition_notice = None;
        self.request(AudioRequest::Audition(AuditionRequest {
            session: self.audio_session.clone(),
            ticket,
            preview,
            observation,
        }));
    }

    pub fn audition_is_current(&self, request: &AuditionRequest) -> bool {
        Arc::ptr_eq(&self.audio_session, &request.session)
            && self
                .pending_auditions
                .iter()
                .any(|ticket| Arc::ptr_eq(ticket, &request.ticket))
    }

    /// Consume each current ticket once. A later focus/edit does not redirect
    /// sound or observation; replacing the practice session invalidates both.
    pub fn finish_audition_admission(
        &mut self,
        request: &AuditionRequest,
        outcome: Result<(), String>,
    ) {
        if !Arc::ptr_eq(&self.audio_session, &request.session) {
            return;
        }
        let Some(index) = self
            .pending_auditions
            .iter()
            .position(|ticket| Arc::ptr_eq(ticket, &request.ticket))
        else {
            return;
        };
        self.pending_auditions.remove(index);
        let latest = self
            .latest_audition
            .as_ref()
            .is_some_and(|ticket| Arc::ptr_eq(ticket, &request.ticket));
        if let Err(reason) = outcome {
            if latest {
                let source = request
                    .observation
                    .as_ref()
                    .map(|observation| {
                        observation
                            .provenance
                            .as_ref()
                            .and_then(|p| {
                                if p.occurrence_id == woodshedding::rehearsal::CardId::UNASSIGNED {
                                    return None;
                                }
                                p.working_set_id.map(|owner| {
                                    format!(
                                        "{} · Set {} · Card {}",
                                        observation.subject_id, owner.0, p.occurrence_id.0
                                    )
                                })
                            })
                            .unwrap_or_else(|| observation.subject_id.clone())
                    })
                    .unwrap_or_else(|| "Selected material".into());
                self.audition_notice = Some(format!("{source}: {reason}"));
            }
            return;
        }
        if latest {
            self.audition_notice = None;
        }
        if let Some(observation) = &request.observation {
            self.refresh_event_time();
            self.practice_history.record_observation(
                self.now_ms,
                observation.subject_id.clone(),
                EngagementKind::Previewed,
                observation.from_id.clone(),
                None,
                observation.provenance.clone(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queued(ui: &UiState) -> AuditionRequest {
        match ui.audio_requests.last().unwrap() {
            AudioRequest::Audition(request) => request.clone(),
            _ => panic!("frozen observed audition"),
        }
    }
    fn catalog() -> UiState {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Chords);
        ui.now_ms = Some(10);
        ui
    }

    #[test]
    fn accepted_catalog_observation_uses_frozen_source_and_admission_time_once() {
        let mut ui = catalog();
        let before = serde_json::to_value(&ui.set).unwrap();
        ui.request_preview();
        let request = queued(&ui);
        assert!(ui.practice_history.is_empty());
        let expected = request.observation.clone().unwrap();
        ui.stage.set_lens(Lens::Scales);
        ui.stage.root_idx = 7;
        ui.now_ms = Some(20);
        ui.finish_audition_admission(&request, Ok(()));
        let event = &ui.practice_history.recent(1)[0];
        assert_eq!(event.kind, EngagementKind::Previewed);
        assert_eq!(event.subject_id, expected.subject_id);
        assert_eq!(event.from_id, expected.from_id);
        assert_eq!(event.provenance, expected.provenance);
        assert_eq!(event.at_ms, Some(20));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), before);
        ui.finish_audition_admission(&request, Ok(()));
        ui.finish_audition_admission(&request, Err("duplicate failure".into()));
        assert_eq!(ui.practice_history.len(), 1);
        assert!(ui.audition_notice.is_none());
    }

    #[test]
    fn refusal_is_retryable_and_each_same_frame_hear_has_its_own_receipt() {
        let mut ui = catalog();
        ui.request_preview();
        let refused = queued(&ui);
        ui.finish_audition_admission(&refused, Err("Stop backing playback".into()));
        assert!(ui.practice_history.is_empty());
        assert!(
            ui.audition_notice
                .as_ref()
                .unwrap()
                .contains("Stop backing playback")
        );
        ui.request_preview();
        let first = queued(&ui);
        ui.request_preview();
        let second = queued(&ui);
        assert!(!Arc::ptr_eq(&first.ticket, &second.ticket));
        ui.finish_audition_admission(&first, Ok(()));
        ui.finish_audition_admission(&second, Ok(()));
        assert_eq!(ui.practice_history.len(), 2);
        assert!(ui.audition_notice.is_none());
        assert!(ui.pending_auditions.is_empty());
    }

    #[test]
    fn older_result_cannot_replace_the_latest_notice_and_session_load_invalidates_tickets() {
        let mut ui = catalog();
        ui.request_preview();
        let old = queued(&ui);
        ui.request_preview();
        let latest = queued(&ui);
        ui.finish_audition_admission(&latest, Err("latest refusal".into()));
        let notice = ui.audition_notice.clone();
        ui.finish_audition_admission(&old, Err("old refusal".into()));
        assert_eq!(ui.audition_notice, notice);
        ui.request_preview();
        let stale = queued(&ui);
        let persisted = ui.to_persisted();
        ui.apply_persisted(&persisted, ui.app_settings.clone());
        assert!(ui.audition_notice.is_none());
        assert!(ui.pending_auditions.is_empty());
        assert!(!ui.audition_is_current(&stale));
        ui.finish_audition_admission(&stale, Ok(()));
        ui.finish_audition_admission(&stale, Err("prior session".into()));
        assert!(ui.practice_history.is_empty());
        assert!(ui.audition_notice.is_none());
    }
}
