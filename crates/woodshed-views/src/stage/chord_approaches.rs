//! Chromatic approach recipes are anchored to an explicit adjacent chord pair.
//! The exercise is appended independently; inspection never edits the passage.

use cambium::{clickable, el, text};
use woodshed_core::audio::AudioRequest;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::history::{EngagementKind, ObservationProvenance, catalog_id_for_card};
use woodshedding::rehearsal::{ApproachDirection, Card, CardId, MarkMode, Material};

use super::{UiChild, UiState};

impl UiState {
    pub fn current_chord_approach_pair(&self) -> Option<(CardId, CardId)> {
        let index = self.selected_card_index()?;
        let source = self.set.cards.get(index)?;
        let target = self.set.cards.get(index + 1)?;
        (matches!(source.material, Material::Chord { .. })
            && matches!(target.material, Material::Chord { .. }))
        .then_some((source.id, target.id))
    }

    pub(super) fn is_inspected_chord_approach(&self, subject: &KeyedCatalogRef) -> bool {
        self.approach_source.is_some()
            && self.approach_target.is_some()
            && self
                .approach_subject
                .as_ref()
                .is_some_and(|inspected| inspected == subject)
    }

    pub fn inspect_chord_approach(
        &mut self,
        source: CardId,
        target: CardId,
        direction: ApproachDirection,
    ) -> bool {
        self.refresh_event_time();
        self.approach_source = Some(source);
        self.approach_target = Some(target);
        self.approach_direction = Some(direction);
        self.approach_subject = self
            .set
            .cards
            .iter()
            .find(|card| card.id == target)
            .and_then(|card| {
                let Material::Chord { name, root } = &card.material else {
                    return None;
                };
                KeyedCatalogRef::from_material(&Material::ChordApproach {
                    name: name.clone(),
                    root: *root,
                    direction,
                })
            });
        match self
            .stage
            .chord_approach_discovery(&self.set, source, target, direction)
        {
            Ok(discovery) => {
                if let Some(subject) = KeyedCatalogRef::from_material(&discovery.preview.material) {
                    self.context_disclosed.insert(subject);
                }
                self.approach_notice = None;
                true
            },
            Err(reason) => {
                self.approach_notice = Some(reason.to_string());
                false
            },
        }
    }

    pub fn hear_chord_approach(&mut self) {
        self.refresh_event_time();
        let (Some(source), Some(target), Some(direction)) = (
            self.approach_source,
            self.approach_target,
            self.approach_direction,
        ) else {
            return;
        };
        match self
            .stage
            .chord_approach_discovery(&self.set, source, target, direction)
        {
            Ok(discovery) => {
                let card = discovery.preview;
                self.queue_approach_preview(
                    &card,
                    self.set
                        .cards
                        .iter()
                        .find(|card| card.id == target)
                        .and_then(catalog_id_for_card),
                );
                self.approach_notice = None;
            },
            Err(reason) => self.approach_notice = Some(reason.to_string()),
        }
    }

    fn queue_approach_preview(&mut self, card: &Card, from: Option<String>) {
        let (pitches, duration_s, strum_s) = self
            .stage
            .card_sounding_pitches_at_tempo(card, self.transport.bpm);
        if pitches.is_empty() {
            return;
        }
        if let Some(catalog) = catalog_id_for_card(card) {
            self.practice_history.record_observation(
                self.now_ms,
                catalog,
                EngagementKind::Previewed,
                from,
                None,
                ObservationProvenance::capture(card).ok(),
            );
        }
        self.request(AudioRequest::PreviewPitches {
            pitches,
            duration_s,
            strum_s,
        });
    }

    pub fn stage_chord_approach(&mut self) -> Option<CardId> {
        self.refresh_event_time();
        let (Some(source), Some(target), Some(direction)) = (
            self.approach_source,
            self.approach_target,
            self.approach_direction,
        ) else {
            return None;
        };
        let from = self
            .set
            .cards
            .iter()
            .find(|card| card.id == target)
            .and_then(catalog_id_for_card);
        match self
            .stage
            .stage_chord_approach(&mut self.set, source, target, direction)
        {
            Ok(id) => {
                self.record_staged_approach(id, from);
                self.approach_notice = None;
                Some(id)
            },
            Err(reason) => {
                self.approach_notice = Some(reason.to_string());
                None
            },
        }
    }

    fn record_staged_approach(&mut self, id: CardId, from: Option<String>) {
        if let Some(card) = self.set.cards.iter().find(|card| card.id == id) {
            if let Some(catalog) = catalog_id_for_card(card) {
                self.practice_history.record_observation(
                    self.now_ms,
                    catalog,
                    EngagementKind::Staged,
                    from,
                    None,
                    ObservationProvenance::capture(card).ok(),
                );
            }
        }
    }

    fn context_approach_preview(&mut self, subject: &KeyedCatalogRef) -> Option<Card> {
        let mut preview = subject.to_card()?;
        let configured = self.stage.card_from_lens()?;
        preview.setting = configured.setting;
        preview.timing = configured.timing;
        preview.setting.marked.clear();
        preview.setting.mark_mode = MarkMode::Off;
        match self.stage.chord_approach_realization(&preview) {
            Ok(_) => {
                self.approach_notice = None;
                Some(preview)
            },
            Err(reason) => {
                self.approach_notice = Some(reason.to_string());
                None
            },
        }
    }

    pub(super) fn hear_context_chord_approach(&mut self, subject: &KeyedCatalogRef) {
        self.refresh_event_time();
        if let Some(preview) = self.context_approach_preview(subject) {
            self.queue_approach_preview(&preview, self.stage.catalog_id());
        }
    }

    pub(super) fn stage_context_chord_approach(&mut self, subject: &KeyedCatalogRef) {
        self.refresh_event_time();
        let Some(preview) = self.context_approach_preview(subject) else {
            return;
        };
        self.set.push(preview);
        let id = self
            .set
            .cards
            .last()
            .expect("append assigns an occurrence")
            .id;
        self.record_staged_approach(id, self.stage.catalog_id());
    }
}

pub(super) fn context_detail(ui: &UiState, subject: &KeyedCatalogRef) -> Option<UiChild> {
    let Some(Material::ChordApproach { direction, .. }) = subject.to_material() else {
        return None;
    };
    let explanation = if ui.is_inspected_chord_approach(subject) {
        match (ui.approach_source, ui.approach_target) {
            (Some(source), Some(target)) => match ui
                .stage
                .chord_approach_discovery(&ui.set, source, target, direction)
            {
                Ok(discovery) => discovery.explanation,
                Err(reason) => reason.to_string(),
            },
            _ => "The inspected passage is no longer available.".into(),
        }
    } else {
        format!(
            "Standalone chromatic approach {} the chord's tones, using the current Stage instrument and tuning. No relation to an authored passage is asserted. Approach notes are transient semitone neighbors; this is not a claim of key compatibility.",
            direction.label().to_lowercase()
        )
    };
    Some(Box::new(
        el("div", text(explanation)).attr("class", "chord-approach-context"),
    ))
}

pub(super) fn panel(ui: &UiState) -> UiChild {
    let current = ui
        .current_card()
        .filter(|card| matches!(card.material, Material::Chord { .. }));
    let pair = ui.current_chord_approach_pair();
    // A lone chord has no passage relation to disclose. Keep unavailable or
    // inspected context visible, but don't displace the rehearsal board with
    // an empty relationship panel.
    if pair.is_none() && ui.approach_source.is_none() && ui.approach_notice.is_none() {
        return Box::new(el("div", ()));
    }
    let choices: Vec<UiChild> = pair
        .map(|(source, target)| {
            ApproachDirection::ALL
                .into_iter()
                .map(|direction| {
                    let (label, grammar) = match direction {
                        ApproachDirection::Below => (
                            "Inspect from below",
                            "One fret below → target tone on the same string.",
                        ),
                        ApproachDirection::Above => (
                            "Inspect from above",
                            "One fret above → target tone on the same string.",
                        ),
                    };
                    Box::new(el(
                        "div",
                        (
                            el("div", text(grammar)),
                            clickable(
                                el("div", text(label)).attr("class", "t-btn chord-approach-choice"),
                                move |ui: &mut UiState, _| {
                                    ui.inspect_chord_approach(source, target, direction);
                                },
                            ),
                        ),
                    )) as UiChild
                })
                .collect()
        })
        .unwrap_or_default();
    let inspected = match (
        ui.approach_source,
        ui.approach_target,
        ui.approach_direction,
    ) {
        (Some(source), Some(target), Some(direction)) => Some(
            ui.stage
                .chord_approach_discovery(&ui.set, source, target, direction),
        ),
        _ => None,
    };
    let unavailable = if ui.approach_notice.is_some() {
        None
    } else {
        inspected
            .as_ref()
            .and_then(|result| result.as_ref().err())
            .map(ToString::to_string)
    };
    let detail: Option<UiChild> = inspected.and_then(Result::ok).map(|discovery| {
        Box::new(el(
            "div",
            (
                el("div", text(discovery.label)).attr("class", "stage-context-title"),
                ui.set
                    .cards
                    .iter()
                    .find(|card| card.id == discovery.source_card_id)
                    .zip(
                        ui.set
                            .cards
                            .iter()
                            .find(|card| card.id == discovery.target_card_id),
                    )
                    .map(|(source, target)| {
                        el(
                            "div",
                            text(format!(
                                "Inspected passage: {} → {}",
                                source.label, target.label
                            )),
                        )
                    }),
                el("div", text(discovery.explanation)),
                el(
                    "div",
                    text("Append a standalone exercise; the passage keeps its authored order."),
                ),
                el(
                    "div",
                    (
                        clickable(
                            el("div", text("Hear approach")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.hear_chord_approach(),
                        ),
                        clickable(
                            el("div", text("Append approach exercise")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| {
                                ui.stage_chord_approach();
                            },
                        ),
                    ),
                )
                .attr("class", "stage-context-actions"),
            ),
        )) as UiChild
    });
    let passage = pair.and_then(|(source, target)| {
        let source = ui.set.cards.iter().find(|card| card.id == source)?;
        let target = ui.set.cards.iter().find(|card| card.id == target)?;
        Some(el("div", text(format!("{} → {} · approach the next chord's target tones, without inferring a harmonic key.", source.label, target.label))))
    });
    Box::new(el("div", (
        el("div", text("Approach next chord's tones")).attr("class", "stage-context-label"),
        passage,
        (current.is_some() && pair.is_none()).then(|| el("div", text("Select a chord followed immediately by another chord in the Set to inspect this passage."))),
        el("div", choices),
        detail,
        unavailable.map(|notice| el("div", text(notice)).attr("class", "chord-approach-unavailable").attr("role", "status")),
        ui.approach_notice.as_ref().map(|notice| el("div", text(notice.clone())).attr("class", "chord-approach-unavailable").attr("role", "status")),
        ui.approach_source.map(|_| clickable(el("div", text("Close approach")).attr("class", "t-btn"), |ui: &mut UiState, _| {
            ui.approach_source = None; ui.approach_target = None; ui.approach_direction = None;
            ui.approach_subject = None; ui.approach_notice = None;
        })),
    )).attr("class", "stage-context-panel chord-approaches"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::storage::AppSection;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::{FretWindow, Hold};

    fn passage(ui: &mut UiState) -> (CardId, CardId) {
        let source = KeyedCatalogRef {
            formula_id: "chord:Minor 7".into(),
            root: PitchClass::new(9),
        }
        .to_card()
        .unwrap();
        ui.set.push(source);
        let source_id = ui.set.cards.last().unwrap().id;
        let mut target = KeyedCatalogRef {
            formula_id: "chord:Major 7".into(),
            root: PitchClass::new(0),
        }
        .to_card()
        .unwrap();
        target.setting.instrument = "Ukulele".into();
        target.setting.tuning = Some("Standard (high-G)".into());
        target.setting.capo = Some(2);
        target.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
        target.timing.bpm = Some(90.0);
        target.timing.hold = Hold::Bars(1);
        ui.stage.select_next_card_shape(&mut target).unwrap();
        target.setting.marked = vec![(0, 3)];
        target.setting.mark_mode = MarkMode::Solo;
        ui.set.push(target);
        let target_id = ui.set.cards.last().unwrap().id;
        ui.set.select_id(source_id);
        (source_id, target_id)
    }

    #[test]
    fn inspection_hearing_and_appending_preserve_the_passage_and_target_setup() {
        let mut ui = UiState::new();
        let (source, target) = passage(&mut ui);
        let before = serde_json::to_value(&ui.set.cards).unwrap();
        assert!(ui.inspect_chord_approach(source, target, ApproachDirection::Below));
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before);
        assert!(ui.audio_requests.is_empty());
        ui.hear_chord_approach();
        let heard = match ui.audio_requests.last().unwrap() {
            AudioRequest::PreviewPitches {
                pitches,
                duration_s,
                strum_s,
            } => (pitches.clone(), *duration_s, *strum_s),
            request => panic!("unexpected request: {request:?}"),
        };
        assert!(heard.0.len() >= 2 && heard.0.len() % 2 == 0);
        for pair in heard.0.chunks_exact(2) {
            assert!((pair[1] / pair[0] - 2.0_f32.powf(1.0 / 12.0)).abs() < 0.0001);
        }
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before);
        let first = ui.stage_chord_approach().unwrap();
        let second = ui.stage_chord_approach().unwrap();
        assert_ne!(first, second);
        assert_ne!(first, target);
        assert_eq!(ui.set.cursor_id(), Some(source));
        assert_eq!(serde_json::to_value(&ui.set.cards[0]).unwrap(), before[0]);
        assert_eq!(serde_json::to_value(&ui.set.cards[1]).unwrap(), before[1]);
        let appended = ui.set.cards.iter().find(|card| card.id == first).unwrap();
        assert_eq!(appended.setting.instrument, "Ukulele");
        assert_eq!(appended.setting.capo, Some(2));
        assert_eq!(
            appended.setting.voicing_fingerprint,
            ui.set.cards[1].setting.voicing_fingerprint
        );
        assert!(appended.setting.marked.is_empty());
        assert_eq!(appended.setting.mark_mode, MarkMode::Off);
        ui.set.select_id(first);
        ui.section = AppSection::Rehearsal;
        assert_eq!(ui.preview_voicing(), heard);
        assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
        assert!(
            ui.rehearsal_approach_status()
                .unwrap()
                .unwrap()
                .contains("Ukulele / Standard (high-G)")
        );
        assert!(
            ui.practice_history
                .recent(20)
                .iter()
                .all(|event| !event.kind.is_practice())
        );
    }

    #[test]
    fn above_visits_descend_one_semitone_to_target_tones() {
        let mut ui = UiState::new();
        let (source, target) = passage(&mut ui);
        assert!(ui.inspect_chord_approach(source, target, ApproachDirection::Above));
        ui.hear_chord_approach();
        let AudioRequest::PreviewPitches { pitches, .. } = ui.audio_requests.last().unwrap() else {
            panic!("expected preview");
        };
        for pair in pitches.chunks_exact(2) {
            assert!((pair[0] / pair[1] - 2.0_f32.powf(1.0 / 12.0)).abs() < 0.0001);
        }
    }

    #[test]
    fn reordered_or_removed_pair_cannot_retarget_an_identical_next_chord() {
        let mut ui = UiState::new();
        let (source, target) = passage(&mut ui);
        assert!(ui.inspect_chord_approach(source, target, ApproachDirection::Below));
        let duplicate = ui.set.cards[1].clone();
        ui.set.push(duplicate);
        ui.set.cards.swap(1, 2);
        ui.hear_chord_approach();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_chord_approach().is_none());
        assert_eq!(ui.set.cards.len(), 3);
        assert!(ui.approach_notice.is_some());
        let index = ui
            .set
            .cards
            .iter()
            .position(|card| card.id == target)
            .unwrap();
        ui.set.remove(index);
        ui.hear_chord_approach();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_chord_approach().is_none());
    }

    #[test]
    fn source_type_and_target_shape_errors_fail_closed() {
        let mut ui = UiState::new();
        let (source, target) = passage(&mut ui);
        assert!(ui.inspect_chord_approach(source, target, ApproachDirection::Below));
        ui.set.cards[1].setting.voicing_fingerprint = Some("stale saved shape".into());
        ui.hear_chord_approach();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_chord_approach().is_none());
        assert_eq!(ui.set.cards.len(), 2);
        ui.set.cards[0].material = Material::Scale {
            name: "Major".into(),
            root: PitchClass::new(9),
        };
        assert!(!ui.inspect_chord_approach(source, target, ApproachDirection::Above));
        assert!(ui.approach_notice.is_some());
    }

    #[test]
    fn ambient_choice_binds_the_authored_pair_and_cannot_escape_stale_adjacency() {
        let mut ui = UiState::new();
        let (source, target) = passage(&mut ui);
        let discovery = ui
            .stage
            .chord_approach_discovery(&ui.set, source, target, ApproachDirection::Above)
            .unwrap();
        let subject = KeyedCatalogRef::from_material(&discovery.preview.material).unwrap();
        assert!(ui.focus_context_catalog(subject));
        assert_eq!(ui.approach_source, Some(source));
        assert_eq!(ui.approach_target, Some(target));
        ui.audition_context_focus();
        assert_eq!(ui.set.cards.len(), 2);
        ui.audio_requests.clear();
        ui.set.cards.swap(0, 1);
        ui.audition_context_focus();
        assert!(ui.audio_requests.is_empty());
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 2);
    }

    #[test]
    fn free_catalog_approach_uses_validated_stage_setup_without_a_passage_claim() {
        let mut ui = UiState::new();
        assert!(ui.focus_context_catalog(KeyedCatalogRef {
            formula_id: "chord-approach:below:Major 7".into(),
            root: PitchClass::new(0)
        }));
        assert!(ui.approach_source.is_none());
        ui.audition_context_focus();
        assert!(!ui.audio_requests.is_empty());
        assert!(ui.set.cards.is_empty());
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 1);
        assert_eq!(ui.set.cards[0].setting.instrument, "Guitar");
        assert!(matches!(
            ui.set.cards[0].material,
            Material::ChordApproach {
                direction: ApproachDirection::Below,
                ..
            }
        ));
    }
    #[test]
    fn invalid_appended_setup_has_no_working_rehearsal_geometry_or_notes() {
        let mut ui = UiState::new();
        let (source, target) = passage(&mut ui);
        assert!(ui.inspect_chord_approach(source, target, ApproachDirection::Below));
        let appended = ui.stage_chord_approach().unwrap();
        ui.set.select_id(appended);
        let index = ui
            .set
            .cards
            .iter()
            .position(|card| card.id == appended)
            .unwrap();
        ui.set.cards[index].setting.tuning = Some("Missing saved target tuning".into());
        ui.section = AppSection::Rehearsal;
        assert!(
            ui.rehearsal_approach_status()
                .unwrap()
                .unwrap_err()
                .contains("Missing saved target tuning")
        );
        assert_eq!(ui.rehearsal_board_geometry().string_count, 0);
        assert!(ui.preview_voicing().0.is_empty());
        assert!(ui.stage.dots_for_card(&ui.set.cards[index]).is_empty());
    }
}
