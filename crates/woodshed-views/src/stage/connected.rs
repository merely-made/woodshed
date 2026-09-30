//! Cross-catalog discovery anchored to a selected authored occurrence.
//! Inspection and audition never author material; staging re-resolves the
//! source so an edit or removal cannot silently stage a stale preview.

use cambium::{clickable, el, text};
use woodshed_core::audio::AudioRequest;
use woodshed_core::history::{EngagementKind, ObservationProvenance, catalog_id_for_card};
use woodshedding::rehearsal::CardId;

use super::{UiChild, UiState};

impl UiState {
    pub(super) fn is_inspected_arpeggio(
        &self,
        subject: &woodshed_core::harmony::KeyedCatalogRef,
    ) -> bool {
        let Some(source) = self.arpeggio_source else {
            return false;
        };
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            // An occurrence-bound action must fail closed after removal;
            // silently falling back to a formula would change what is heard.
            return subject.formula_id.starts_with("arpeggio:");
        };
        woodshed_core::harmony::KeyedCatalogRef::from_material(&card.material).is_some_and(
            |mut keyed| {
                keyed.formula_id = keyed.formula_id.replacen("chord:", "arpeggio:", 1);
                &keyed == subject
            },
        )
    }

    pub fn inspect_chord_arpeggio(&mut self, source: CardId) -> bool {
        self.refresh_event_time();
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.discovery_notice = Some("The source Card is no longer in the Set.".into());
            return false;
        };
        self.arpeggio_source = Some(source);
        match self.stage.chord_arpeggio_discovery(card) {
            Ok(discovery) => {
                self.context_disclosed.insert(discovery.subject);
                self.discovery_notice = None;
                true
            },
            Err(reason) => {
                self.discovery_notice = Some(reason.to_string());
                false
            },
        }
    }

    pub fn hear_chord_arpeggio(&mut self) {
        self.refresh_event_time();
        let Some(source) = self.arpeggio_source else {
            return;
        };
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.discovery_notice = Some("The source Card is no longer in the Set.".into());
            return;
        };
        match self.stage.chord_arpeggio_discovery(card) {
            Ok(discovery) => {
                let (pitches, duration_s, strum_s) = self
                    .stage
                    .card_sounding_pitches_at_tempo(&discovery.preview, self.transport.bpm);
                if pitches.is_empty() {
                    self.discovery_notice = Some("This realization has no sounding notes. Choose an available shape or change the note selection.".into());
                    return;
                }
                self.practice_history.record_observation(
                    self.now_ms,
                    discovery.subject.formula_id.clone(),
                    EngagementKind::Previewed,
                    catalog_id_for_card(card),
                    None,
                    ObservationProvenance::capture(&discovery.preview).ok(),
                );
                self.request(AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s,
                });
                self.discovery_notice = None;
            },
            Err(reason) => self.discovery_notice = Some(reason.to_string()),
        }
    }

    pub fn stage_chord_arpeggio(&mut self) -> Option<CardId> {
        self.refresh_event_time();
        let source = self.arpeggio_source?;
        let from = self
            .set
            .cards
            .iter()
            .find(|card| card.id == source)
            .and_then(catalog_id_for_card);
        match self.stage.stage_chord_arpeggio(&mut self.set, source) {
            Ok(id) => {
                if let Some(card) = self.set.cards.iter().find(|card| card.id == id) {
                    self.practice_history.record_observation(
                        self.now_ms,
                        catalog_id_for_card(card).unwrap_or_default(),
                        EngagementKind::Staged,
                        from,
                        None,
                        ObservationProvenance::capture(card).ok(),
                    );
                }
                self.discovery_notice = None;
                Some(id)
            },
            Err(reason) => {
                self.discovery_notice = Some(reason.to_string());
                None
            },
        }
    }
}

pub(super) fn panel(ui: &UiState) -> UiChild {
    let unavailable = ui
        .current_card()
        .filter(|card| {
            matches!(
                card.material,
                woodshedding::rehearsal::Material::Chord { .. }
            )
        })
        .and_then(|card| ui.stage.chord_arpeggio_discovery(card).err())
        .map(|reason| {
            el(
                "div",
                text(format!("Arpeggio discovery unavailable: {reason}")),
            )
            .attr("role", "status")
        });
    let candidate = ui.current_card().and_then(|card| {
        ui.stage
            .chord_arpeggio_discovery(card)
            .ok()
            .map(|discovery| (card.id, discovery))
    });
    let discover: Option<UiChild> = candidate.map(|(id, discovery)| {
        Box::new(clickable(
            el("div", text(format!("Discover {}", discovery.label)))
                .attr("class", "t-btn")
                .attr("aria-label", "Inspect related arpeggio for selected Card"),
            move |ui: &mut UiState, _| {
                ui.inspect_chord_arpeggio(id);
            },
        )) as UiChild
    });
    let inspected = ui
        .arpeggio_source
        .and_then(|id| ui.set.cards.iter().find(|card| card.id == id))
        .and_then(|card| ui.stage.chord_arpeggio_discovery(card).ok());
    let detail: Option<UiChild> = inspected.map(|discovery| Box::new(el("div", (
        el("div", text(discovery.label)).attr("class", "stage-context-title"),
        el("div", text(discovery.explanation)),
        el("div", text(if discovery.formula_preview {
            "Formula preview: no selected instrument shape."
        } else { "Selected instrument shape: the same sounding pitches in sequence." })),
        el("div", text("Uses the source Card's instrument settings, selected shape and timing. Edit Touch and timing after staging.")),
        el("div", (
            clickable(el("div", text("Hear arpeggio")).attr("class", "t-btn"), |ui: &mut UiState, _| ui.hear_chord_arpeggio()),
            clickable(el("div", text("Stage arpeggio")).attr("class", "t-btn"), |ui: &mut UiState, _| { ui.stage_chord_arpeggio(); }),
            clickable(el("div", text("Close discovery")).attr("class", "t-btn"), |ui: &mut UiState, _| { ui.arpeggio_source = None; ui.discovery_notice = None; }),
        )).attr("class", "stage-context-actions"),
    ))) as UiChild);
    Box::new(
        el(
            "div",
            (
                discover,
                detail,
                unavailable,
                ui.discovery_notice
                    .as_ref()
                    .map(|notice| el("div", text(notice.clone())).attr("role", "status")),
            ),
        )
        .attr("class", "stage-context-panel"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::Touch;

    fn chord(ui: &mut UiState) -> CardId {
        assert!(ui.focus_context_catalog(KeyedCatalogRef {
            formula_id: "chord:Major 7".into(),
            root: PitchClass::new(0)
        }));
        ui.stage_current(None);
        ui.set.cards.last().unwrap().id
    }

    #[test]
    fn inspecting_and_hearing_preserve_source_and_explicit_staging_has_new_identity() {
        let mut ui = UiState::new();
        let source = chord(&mut ui);
        let before = serde_json::to_value(&ui.set.cards).unwrap();
        assert!(ui.inspect_chord_arpeggio(source));
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before);
        assert!(ui.audio_requests.is_empty());
        ui.hear_chord_arpeggio();
        assert!(
            matches!(ui.audio_requests.last(), Some(AudioRequest::PreviewPitches { strum_s, .. }) if *strum_s > 0.0)
        );
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before);
        let added = ui.stage_chord_arpeggio().unwrap();
        assert_ne!(source, added);
        assert_eq!(ui.set.cards.len(), 2);
        assert!(matches!(ui.set.cards[1].touch, Touch::Arpeggiate { .. }));
        assert_eq!(serde_json::to_value(&ui.set.cards[0]).unwrap(), before[0]);
    }

    #[test]
    fn ambient_focus_and_list_actions_share_occurrence_realization() {
        let mut ui = UiState::new();
        let source = chord(&mut ui);
        ui.set.cards[0].timing.bpm = Some(83.0);
        let subject = ui
            .stage
            .chord_arpeggio_discovery(&ui.set.cards[0])
            .unwrap()
            .subject;
        assert!(ui.focus_context_catalog(subject));
        assert_eq!(ui.arpeggio_source, Some(source));
        assert_eq!(ui.set.cards.len(), 1);
        ui.audition_context_focus();
        assert!(
            matches!(ui.audio_requests.last(), Some(AudioRequest::PreviewPitches { strum_s, .. }) if *strum_s > 0.0)
        );
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 2);
        assert_eq!(ui.set.cards[1].timing.bpm, Some(83.0));
    }

    #[test]
    fn inherited_tempo_is_shared_by_discovery_and_rehearsal_preview() {
        let mut ui = UiState::new();
        let source = chord(&mut ui);
        ui.set.cards[0].timing.bpm = None;
        assert!(ui.inspect_chord_arpeggio(source));
        ui.transport.bpm = 60.0;
        ui.hear_chord_arpeggio();
        let slow = match ui.audio_requests.pop().unwrap() {
            AudioRequest::PreviewPitches { strum_s, .. } => strum_s,
            request => panic!("unexpected request: {request:?}"),
        };
        ui.transport.bpm = 120.0;
        ui.hear_chord_arpeggio();
        let fast = match ui.audio_requests.pop().unwrap() {
            AudioRequest::PreviewPitches { strum_s, .. } => strum_s,
            request => panic!("unexpected request: {request:?}"),
        };
        assert!((slow - 2.0 * fast).abs() < 0.001);
        let added = ui.stage_chord_arpeggio().unwrap();
        ui.set.select_id(added);
        ui.section = woodshed_core::storage::AppSection::Rehearsal;
        assert!((ui.preview_voicing().2 - fast).abs() < 0.001);
        ui.toggle_rehearsal();
        assert!(
            matches!(ui.audio_requests.last(), Some(AudioRequest::PreviewPitches { strum_s, .. }) if (*strum_s - fast).abs() < 0.001)
        );
    }

    #[test]
    fn catalog_arpeggio_fallback_is_sequential_and_uses_transport_tempo() {
        let mut ui = UiState::new();
        assert!(ui.focus_context_catalog(KeyedCatalogRef {
            formula_id: "arpeggio:Major 7".into(),
            root: PitchClass::new(0)
        }));
        assert!(ui.arpeggio_source.is_none());
        ui.transport.bpm = 60.0;
        ui.audition_context_focus();
        assert!(
            matches!(ui.audio_requests.last(), Some(AudioRequest::PreviewPitches { strum_s, .. }) if *strum_s > 0.5)
        );
        assert!(ui.set.cards.is_empty());
    }

    #[test]
    fn interaction_clock_is_refreshed_at_staging_and_observation_boundaries() {
        let mut ui = UiState::new();
        ui.now_ms = Some(1);
        ui.event_clock = Some(|| Some(42_000));
        chord(&mut ui);
        assert_eq!(ui.practice_history.recent(1)[0].at_ms, Some(42_000));
        ui.now_ms = Some(2);
        ui.record_rehearsal_cursor();
        assert_eq!(ui.card_started_ms, Some(42_000));
        ui.event_clock = Some(|| Some(43_000));
        ui.complete_rehearsal_cursor();
        assert_eq!(ui.practice_history.recent(1)[0].practiced_ms, Some(1_000));
    }

    #[test]
    fn repeated_material_with_unavailable_shape_cannot_reuse_previous_occurrence() {
        let mut ui = UiState::new();
        let first = chord(&mut ui);
        assert!(ui.inspect_chord_arpeggio(first));
        let subject = ui
            .stage
            .chord_arpeggio_discovery(&ui.set.cards[0])
            .unwrap()
            .subject;
        let second = chord(&mut ui);
        ui.set.select_id(second);
        ui.set.cards[1].setting.voicing_idx = Some(usize::MAX);
        assert!(ui.focus_context_catalog(subject));
        assert_eq!(ui.arpeggio_source, Some(second));
        ui.audition_context_focus();
        assert!(ui.audio_requests.is_empty());
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 2);
        assert!(ui.discovery_notice.is_some());
    }

    #[test]
    fn removed_source_cannot_stage_or_audition_a_stale_preview() {
        let mut ui = UiState::new();
        let source = chord(&mut ui);
        assert!(ui.inspect_chord_arpeggio(source));
        ui.set.remove(0);
        ui.hear_chord_arpeggio();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_chord_arpeggio().is_none());
        assert!(ui.set.cards.is_empty());
        assert!(ui.discovery_notice.is_some());
    }

    #[test]
    fn discovery_resolves_edits_at_action_time() {
        let mut ui = UiState::new();
        let source = chord(&mut ui);
        assert!(ui.inspect_chord_arpeggio(source));
        ui.set.cards[0].timing.bpm = Some(83.0);
        ui.set.cards[0].setting.capo = Some(2);
        let added = ui.stage_chord_arpeggio().unwrap();
        let card = ui.set.cards.iter().find(|card| card.id == added).unwrap();
        assert_eq!(card.timing.bpm, Some(83.0));
        assert_eq!(card.setting.capo, Some(2));
    }

    #[test]
    fn pause_duration_is_excluded_and_edit_retains_old_instruction_provenance() {
        let mut ui = UiState::new();
        chord(&mut ui);
        let subject = catalog_id_for_card(&ui.set.cards[0]).unwrap();
        ui.now_ms = Some(1_000);
        ui.toggle_rehearsal();
        ui.now_ms = Some(3_000);
        ui.toggle_rehearsal();
        ui.now_ms = Some(10_000);
        ui.toggle_rehearsal();
        ui.now_ms = Some(11_000);
        ui.set.cards[0].timing.bpm = Some(83.0);
        ui.sync();
        assert_eq!(ui.practice_history.total_practiced_ms(&subject), 3_000);
        let recent = ui.practice_history.recent(20);
        let closed = recent
            .iter()
            .find(|event| event.practiced_ms == Some(1_000))
            .unwrap();
        assert_ne!(
            closed.provenance.as_ref().unwrap().card_snapshot["timing"]["bpm"],
            serde_json::json!(83.0)
        );
    }
}
