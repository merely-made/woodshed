//! A scale formula supplies the grammar for a bounded practice recipe.
//! Inspect and Hear retain the source occurrence; only Stage authors a Card.

use cambium::{clickable, el, text};
use woodshed_core::audio::AudioRequest;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::history::{EngagementKind, ObservationProvenance, catalog_id_for_card};
use woodshedding::rehearsal::{CardId, Material, ScalePattern};

use super::{UiChild, UiState};

impl UiState {
    pub(super) fn is_inspected_scale_pattern(&self, subject: &KeyedCatalogRef) -> bool {
        if self.pattern_source.is_none() {
            return false;
        }
        self.pattern_subject
            .as_ref()
            .is_some_and(|inspected| inspected == subject)
    }

    /// Formula-only graph focus uses the configured Stage scale as its source,
    /// then applies the same readiness check as an occurrence-bound recipe.
    pub(super) fn context_scale_pattern_preview(
        &mut self,
        subject: &KeyedCatalogRef,
    ) -> Option<woodshedding::rehearsal::Card> {
        let Some(Material::ScalePattern { pattern, .. }) = subject.to_material() else {
            return None;
        };
        let source = self.stage.card_from_lens()?;
        match self.stage.scale_pattern_discovery(&source, pattern) {
            Ok(discovery) => {
                self.pattern_notice = None;
                Some(discovery.preview)
            },
            Err(reason) => {
                self.pattern_notice = Some(reason.to_string());
                None
            },
        }
    }

    pub(super) fn hear_context_scale_pattern(&mut self, subject: &KeyedCatalogRef) {
        self.refresh_event_time();
        let Some(card) = self.context_scale_pattern_preview(subject) else {
            return;
        };
        let (pitches, duration_s, strum_s) = self
            .stage
            .card_sounding_pitches_at_tempo(&card, self.transport.bpm);
        if pitches.is_empty() {
            return;
        }
        if let Some(catalog) = catalog_id_for_card(&card) {
            self.practice_history.record_observation(
                self.now_ms,
                catalog,
                EngagementKind::Previewed,
                self.stage.catalog_id(),
                None,
                ObservationProvenance::capture_in_set(&card, self.working_sets.active_id).ok(),
            );
        }
        self.request(AudioRequest::PreviewPitches {
            pitches,
            duration_s,
            strum_s,
        });
    }

    pub(super) fn stage_context_scale_pattern(&mut self, subject: &KeyedCatalogRef) {
        self.refresh_event_time();
        let Some(card) = self.context_scale_pattern_preview(subject) else {
            return;
        };
        self.set.push(card);
        if let Some(card) = self.set.cards.last() {
            if let Some(catalog) = catalog_id_for_card(card) {
                self.practice_history.record_observation(
                    self.now_ms,
                    catalog,
                    EngagementKind::Staged,
                    self.stage.catalog_id(),
                    None,
                    ObservationProvenance::capture_in_set(card, self.working_sets.active_id).ok(),
                );
            }
        }
    }

    pub fn inspect_scale_pattern(&mut self, source: CardId, pattern: ScalePattern) -> bool {
        self.refresh_event_time();
        self.pattern_source = Some(source);
        self.scale_pattern = Some(pattern);
        self.pattern_subject = None;
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.pattern_notice = Some("The source scale Card is no longer in the Set.".into());
            return false;
        };
        if let Material::Scale { name, root } = &card.material {
            self.pattern_subject = KeyedCatalogRef::from_material(&Material::ScalePattern {
                name: name.clone(),
                root: *root,
                pattern,
            });
        }
        match self
            .current_card_stage()
            .scale_pattern_discovery(card, pattern)
        {
            Ok(discovery) => {
                if let Some(subject) = KeyedCatalogRef::from_material(&discovery.preview.material) {
                    self.context_disclosed.insert(subject);
                }
                self.pattern_notice = None;
                true
            },
            Err(reason) => {
                self.pattern_notice = Some(reason.to_string());
                false
            },
        }
    }

    pub fn hear_scale_pattern(&mut self) {
        self.refresh_event_time();
        let (Some(source), Some(pattern)) = (self.pattern_source, self.scale_pattern) else {
            return;
        };
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.pattern_notice = Some("The source scale Card is no longer in the Set.".into());
            return;
        };
        match self
            .current_card_stage()
            .scale_pattern_discovery(card, pattern)
        {
            Ok(discovery) => {
                let (pitches, duration_s, strum_s) = self
                    .current_card_stage()
                    .card_sounding_pitches_at_tempo(&discovery.preview, self.transport.bpm);
                if pitches.is_empty() {
                    self.pattern_notice = Some(
                        "This scale pattern has no playable notes in its saved window.".into(),
                    );
                    return;
                }
                if let Some(catalog) = catalog_id_for_card(&discovery.preview) {
                    self.practice_history.record_observation(
                        self.now_ms,
                        catalog,
                        EngagementKind::Previewed,
                        catalog_id_for_card(card),
                        None,
                        ObservationProvenance::capture_in_set(
                            &discovery.preview,
                            self.working_sets.active_id,
                        )
                        .ok(),
                    );
                }
                self.request(AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s,
                });
                self.pattern_notice = None;
            },
            Err(reason) => self.pattern_notice = Some(reason.to_string()),
        }
    }

    pub fn stage_scale_pattern(&mut self) -> Option<CardId> {
        self.refresh_event_time();
        let (Some(source), Some(pattern)) = (self.pattern_source, self.scale_pattern) else {
            return None;
        };
        let from = self
            .set
            .cards
            .iter()
            .find(|card| card.id == source)
            .and_then(catalog_id_for_card);
        let stage = if self.is_current_set_rehearsing() {
            self.rehearsal_stage.as_ref().unwrap_or(&self.stage)
        } else {
            &self.stage
        };
        match stage.stage_scale_pattern(&mut self.set, source, pattern) {
            Ok(id) => {
                if let Some(card) = self.set.cards.iter().find(|card| card.id == id) {
                    if let Some(catalog) = catalog_id_for_card(card) {
                        self.practice_history.record_observation(
                            self.now_ms,
                            catalog,
                            EngagementKind::Staged,
                            from,
                            None,
                            ObservationProvenance::capture_in_set(
                                card,
                                self.working_sets.active_id,
                            )
                            .ok(),
                        );
                    }
                }
                self.pattern_notice = None;
                Some(id)
            },
            Err(reason) => {
                self.pattern_notice = Some(reason.to_string());
                None
            },
        }
    }
}

pub(super) fn context_detail(ui: &UiState, subject: &KeyedCatalogRef) -> Option<UiChild> {
    let Some(Material::ScalePattern { pattern, .. }) = subject.to_material() else {
        return None;
    };
    let bound = ui.is_inspected_scale_pattern(subject);
    let source = if bound {
        ui.pattern_source
            .and_then(|id| ui.set.cards.iter().find(|card| card.id == id))
            .cloned()
    } else {
        ui.stage.card_from_lens()
    };
    let label = if bound {
        "Saved source Card setup"
    } else {
        "Current Stage instrument and tuning"
    };
    let stage = if bound {
        ui.current_card_stage()
    } else {
        &ui.stage
    };
    let description = source
        .as_ref()
        .map(|card| stage.scale_pattern_discovery(card, pattern));
    let explanation = match description {
        Some(Ok(discovery)) => discovery.explanation,
        Some(Err(reason)) => reason.to_string(),
        None => "The source scale Card is no longer available.".into(),
    };
    Some(Box::new(
        el(
            "div",
            (
                el("div", text(label)).attr("class", "stage-context-label"),
                el("div", text(explanation)),
            ),
        )
        .attr("class", "scale-pattern-context"),
    ))
}

pub(super) fn panel(ui: &UiState) -> UiChild {
    let source_card = ui
        .current_card()
        .filter(|card| matches!(card.material, Material::Scale { .. }));
    if source_card.is_none() && ui.pattern_source.is_none() && ui.pattern_notice.is_none() {
        return Box::new(el("div", ()));
    }
    let choices: Vec<UiChild> = source_card
        .map(|card| {
            ScalePattern::ALL
                .into_iter()
                .map(|pattern| {
                    let source = card.id;
                    let (label, reason) = match pattern {
                        ScalePattern::Thirds => (
                            "Inspect diatonic thirds",
                            "Pairs of scale degrees: 1–3, 2–4, 3–5…",
                        ),
                        ScalePattern::Fourths => (
                            "Inspect diatonic fourths",
                            "Pairs of scale degrees: 1–4, 2–5, 3–6…",
                        ),
                    };
                    Box::new(el(
                        "div",
                        (
                            el("div", text(reason)),
                            clickable(
                                el("div", text(label)).attr("class", "t-btn scale-pattern-choice"),
                                move |ui: &mut UiState, _| {
                                    ui.inspect_scale_pattern(source, pattern);
                                },
                            ),
                        ),
                    )) as UiChild
                })
                .collect()
        })
        .unwrap_or_default();
    let inspected = ui
        .pattern_source
        .and_then(|id| ui.set.cards.iter().find(|card| card.id == id));
    let discovery = inspected.zip(ui.scale_pattern).map(|(card, pattern)| {
        ui.current_card_stage()
            .scale_pattern_discovery(card, pattern)
    });
    let unavailable = if ui.pattern_notice.is_some() {
        None
    } else if ui.pattern_source.is_some() && inspected.is_none() {
        Some("The source scale Card is no longer in the Set.".into())
    } else {
        discovery
            .as_ref()
            .and_then(|result| result.as_ref().err())
            .map(ToString::to_string)
    };
    let detail: Option<UiChild> = discovery.and_then(Result::ok).map(|discovery| {
        Box::new(el(
            "div",
            (
                el("div", text(discovery.label)).attr("class", "stage-context-title"),
                inspected.map(|card| el("div", text(format!("From {}", card.label)))),
                el("div", text(discovery.explanation)),
                el(
                    "div",
                    (
                        clickable(
                            el("div", text("Hear pattern")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.hear_scale_pattern(),
                        ),
                        clickable(
                            el("div", text("Stage pattern")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| {
                                ui.stage_scale_pattern();
                            },
                        ),
                    ),
                )
                .attr("class", "stage-context-actions"),
            ),
        )) as UiChild
    });
    Box::new(el("div", (
        el("div", text("Related scale practice patterns")).attr("class", "stage-context-label"),
        source_card.map(|card| el("div", text(format!("{} supplies the scale degrees. These recipes pair notes within its saved instrument and fret window; seven-note formulas are supported.", card.label)))),
        el("div", choices),
        detail,
        unavailable.map(|notice| el("div", text(notice)).attr("class", "scale-pattern-unavailable").attr("role", "status")),
        ui.pattern_notice.as_ref().map(|notice| el("div", text(notice.clone())).attr("class", "scale-pattern-unavailable").attr("role", "status")),
        ui.pattern_source.map(|_| clickable(el("div", text("Close pattern")).attr("class", "t-btn"), |ui: &mut UiState, _| {
            ui.pattern_source = None; ui.scale_pattern = None; ui.pattern_notice = None;
        })),
    )).attr("class", "stage-context-panel scale-patterns"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::FretWindow;

    fn source(ui: &mut UiState) -> CardId {
        let mut card = KeyedCatalogRef {
            formula_id: "scale:Major".into(),
            root: PitchClass::new(0),
        }
        .to_card()
        .unwrap();
        card.setting.instrument = "Ukulele".into();
        card.setting.tuning = Some("Standard (high-G)".into());
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 2, span: 10 });
        card.timing.bpm = Some(90.0);
        ui.set.push(card);
        ui.set.cards.last().unwrap().id
    }

    #[test]
    fn pattern_inspection_and_hearing_preserve_source_and_staging_preserves_visit_order() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        let before = serde_json::to_value(&ui.set.cards).unwrap();
        assert!(ui.inspect_scale_pattern(id, ScalePattern::Thirds));
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before);
        assert!(ui.audio_requests.is_empty());
        ui.hear_scale_pattern();
        let heard = match ui.audio_requests.last().unwrap() {
            AudioRequest::PreviewPitches {
                pitches,
                duration_s,
                strum_s,
            } => (pitches.clone(), *duration_s, *strum_s),
            request => panic!("unexpected request: {request:?}"),
        };
        assert!(heard.2 > 0.0);
        assert!(heard.0.windows(2).any(|pair| pair[1] < pair[0]));
        assert!(
            heard
                .0
                .iter()
                .enumerate()
                .any(|(index, note)| heard.0[..index].contains(note))
        );
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), before);
        let first = ui.stage_scale_pattern().unwrap();
        let second = ui.stage_scale_pattern().unwrap();
        assert_ne!(first, id);
        assert_ne!(first, second);
        assert_eq!(serde_json::to_value(&ui.set.cards[0]).unwrap(), before[0]);
        ui.set.select_id(first);
        ui.section = woodshed_core::storage::AppSection::Rehearsal;
        assert_eq!(ui.preview_voicing(), heard);
        assert_eq!(ui.rehearsal_board_geometry().string_count, 4);
        assert!(
            ui.rehearsal_scale_status()
                .unwrap()
                .unwrap()
                .contains("concert D Major")
        );
        assert!(
            ui.practice_history
                .recent(20)
                .iter()
                .all(|event| !event.kind.is_practice())
        );
        assert!(
            ui.practice_history
                .recent(20)
                .iter()
                .all(|event| event.subject_id != "scale:Major")
        );
    }

    #[test]
    fn source_edits_revalidate_the_recipe_and_removal_never_uses_live_stage() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        assert!(ui.inspect_scale_pattern(id, ScalePattern::Fourths));
        ui.set.cards[0].material = Material::Scale {
            name: "Major Pentatonic".into(),
            root: PitchClass::new(0),
        };
        ui.hear_scale_pattern();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_scale_pattern().is_none());
        assert_eq!(ui.set.cards.len(), 1);
        assert!(ui.pattern_notice.is_some());
        ui.set.remove(0);
        ui.hear_scale_pattern();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_scale_pattern().is_none());
    }

    #[test]
    fn invalid_saved_setup_explains_failure_before_authoring() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        ui.set.cards[0].setting.tuning = Some("Missing saved tuning".into());
        assert!(!ui.inspect_scale_pattern(id, ScalePattern::Thirds));
        assert!(
            ui.pattern_notice
                .as_ref()
                .unwrap()
                .contains("Missing saved tuning")
        );
        ui.hear_scale_pattern();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_scale_pattern().is_none());
        assert_eq!(ui.set.cards.len(), 1);
    }

    #[test]
    fn ambient_pattern_focus_uses_the_selected_scale_occurrence() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        let discovery = ui
            .stage
            .scale_pattern_discovery(&ui.set.cards[0], ScalePattern::Fourths)
            .unwrap();
        let subject = KeyedCatalogRef::from_material(&discovery.preview.material).unwrap();
        assert!(ui.focus_context_catalog(subject));
        assert_eq!(ui.pattern_source, Some(id));
        assert_eq!(ui.scale_pattern, Some(ScalePattern::Fourths));
        ui.audition_context_focus();
        assert_eq!(ui.set.cards.len(), 1);
        // Closing the inspector cannot make a later context Stage action lose
        // the matching authored occurrence's saved setup when it refocuses.
        ui.pattern_source = None;
        ui.pattern_subject = None;
        ui.scale_pattern = None;
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 2);
        assert_eq!(ui.set.cards[1].setting.instrument, "Ukulele");
        assert!(matches!(
            ui.set.cards[1].material,
            Material::ScalePattern {
                pattern: ScalePattern::Fourths,
                ..
            }
        ));
    }
    #[test]
    fn stale_ambient_pattern_after_source_edit_cannot_fall_back_to_live_catalog() {
        let mut ui = UiState::new();
        source(&mut ui);
        let discovery = ui
            .stage
            .scale_pattern_discovery(&ui.set.cards[0], ScalePattern::Thirds)
            .unwrap();
        let subject = KeyedCatalogRef::from_material(&discovery.preview.material).unwrap();
        assert!(ui.focus_context_catalog(subject));
        ui.set.cards[0].material = Material::Scale {
            name: "Major Pentatonic".into(),
            root: PitchClass::new(0),
        };
        ui.audition_context_focus();
        assert!(ui.audio_requests.is_empty());
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 1);
        assert!(ui.pattern_notice.is_some());
    }

    #[test]
    fn free_catalog_pattern_is_validated_and_stages_the_recipe_in_current_stage_setup() {
        let mut ui = UiState::new();
        let subject = KeyedCatalogRef {
            formula_id: "scale-pattern:thirds:Major".into(),
            root: PitchClass::new(0),
        };
        assert!(ui.focus_context_catalog(subject));
        assert!(ui.pattern_source.is_none());
        ui.audition_context_focus();
        assert!(!ui.audio_requests.is_empty());
        assert!(ui.set.cards.is_empty());
        ui.add_context_focus_to_set();
        assert_eq!(ui.set.cards.len(), 1);
        assert_eq!(ui.set.cards[0].setting.instrument, "Guitar");
        assert!(matches!(
            ui.set.cards[0].material,
            Material::ScalePattern {
                pattern: ScalePattern::Thirds,
                ..
            }
        ));
    }
}
