//! Chord-to-scale discovery uses exact formula containment, with explicit
//! occurrence-bound inspection, audition and authoring actions.

use cambium::{clickable, el, text};
use woodshed_core::audio::AudioRequest;
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::history::{EngagementKind, ObservationProvenance, catalog_id_for_card};
use woodshedding::rehearsal::{CardId, Material};

use super::{UiChild, UiState};

const SCALE_CHOICES: usize = 4;
const MAX_SCALE_CHOICES: usize = 32;

impl UiState {
    pub fn explore_chord_scales(&mut self, source: CardId) -> bool {
        self.refresh_event_time();
        self.scale_source = Some(source);
        self.scale_subject = None;
        self.scale_limit = SCALE_CHOICES;
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.scale_notice = Some("The source Card is no longer in the Set.".into());
            return false;
        };
        match self
            .current_card_stage()
            .chord_scale_discoveries(card, SCALE_CHOICES)
        {
            Ok(_) => {
                self.scale_notice = None;
                true
            },
            Err(reason) => {
                self.scale_notice = Some(reason.to_string());
                false
            },
        }
    }

    pub fn show_more_chord_scales(&mut self) {
        self.scale_limit = (self.scale_limit + SCALE_CHOICES).min(MAX_SCALE_CHOICES);
    }

    pub fn inspect_chord_scale(&mut self, source: CardId, subject: KeyedCatalogRef) -> bool {
        self.refresh_event_time();
        self.scale_source = Some(source);
        self.scale_subject = Some(subject.clone());
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.scale_notice = Some("The source Card is no longer in the Set.".into());
            return false;
        };
        match self
            .current_card_stage()
            .chord_scale_discovery(card, &subject)
        {
            Ok(_) => {
                self.context_disclosed.insert(subject);
                self.scale_notice = None;
                true
            },
            Err(reason) => {
                self.scale_notice = Some(reason.to_string());
                false
            },
        }
    }

    pub fn hear_chord_scale(&mut self) {
        self.refresh_event_time();
        let (Some(source), Some(subject)) = (self.scale_source, self.scale_subject.as_ref()) else {
            return;
        };
        let Some(card) = self.set.cards.iter().find(|card| card.id == source) else {
            self.scale_notice = Some("The source Card is no longer in the Set.".into());
            return;
        };
        match self
            .current_card_stage()
            .chord_scale_discovery(card, subject)
        {
            Ok(discovery) => {
                let (pitches, duration_s, strum_s) = self
                    .current_card_stage()
                    .card_sounding_pitches_at_tempo(&discovery.preview, self.transport.bpm);
                if pitches.is_empty() {
                    self.scale_notice = Some("This scale formula has no sounding notes.".into());
                    return;
                }
                self.practice_history.record_observation(
                    self.now_ms,
                    discovery.subject.formula_id.clone(),
                    EngagementKind::Previewed,
                    catalog_id_for_card(card),
                    None,
                    ObservationProvenance::capture_in_set(
                        &discovery.preview,
                        self.working_sets.active_id,
                    )
                    .ok(),
                );
                self.request(AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s,
                });
                self.scale_notice = None;
            },
            Err(reason) => self.scale_notice = Some(reason.to_string()),
        }
    }

    pub fn stage_chord_scale(&mut self) -> Option<CardId> {
        self.refresh_event_time();
        let (Some(source), Some(subject)) = (self.scale_source, self.scale_subject.as_ref()) else {
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
        match stage.stage_chord_scale(&mut self.set, source, subject) {
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
                self.scale_notice = None;
                Some(id)
            },
            Err(reason) => {
                self.scale_notice = Some(reason.to_string());
                None
            },
        }
    }
}

pub(super) fn panel(ui: &UiState) -> UiChild {
    if ui.scale_source.is_none()
        && ui.scale_notice.is_none()
        && !ui
            .current_card()
            .is_some_and(|card| matches!(card.material, Material::Chord { .. }))
    {
        return Box::new(el("div", ()));
    }

    let explore: Option<UiChild> = ui
        .current_card()
        .filter(|card| matches!(card.material, Material::Chord { .. }))
        .map(|card| {
            let source = card.id;
            Box::new(clickable(
                el("div", text("Explore compatible scales")).attr("class", "t-btn"),
                move |ui: &mut UiState, _| {
                    ui.explore_chord_scales(source);
                },
            )) as UiChild
        });
    let limit = ui.scale_limit.clamp(SCALE_CHOICES, MAX_SCALE_CHOICES);
    let source = ui
        .scale_source
        .and_then(|id| ui.set.cards.iter().find(|card| card.id == id));
    let current_unavailable = if ui.scale_notice.is_some() {
        None
    } else if ui.scale_source.is_some() && source.is_none() {
        Some("The source Card is no longer in the Set.".to_string())
    } else if let Some((card, subject)) = source.zip(ui.scale_subject.as_ref()) {
        ui.current_card_stage()
            .chord_scale_discovery(card, subject)
            .err()
            .map(|reason| reason.to_string())
    } else {
        source
            .and_then(|card| {
                ui.current_card_stage()
                    .chord_scale_discoveries(card, limit)
                    .err()
            })
            .map(|reason| reason.to_string())
    };
    let choices: Vec<UiChild> = source
        .and_then(|card| {
            ui.current_card_stage()
                .chord_scale_discoveries(card, limit)
                .ok()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|discovery| {
            let source = discovery.source_card_id;
            let subject = discovery.subject;
            Box::new(clickable(
                el("div", text(discovery.label)).attr("class", "t-btn scale-choice"),
                move |ui: &mut UiState, _| {
                    ui.inspect_chord_scale(source, subject.clone());
                },
            )) as UiChild
        })
        .collect();
    let detail: Option<UiChild> = source
        .zip(ui.scale_subject.as_ref())
        .and_then(|(card, subject)| {
            ui.current_card_stage()
                .chord_scale_discovery(card, subject)
                .ok()
        })
        .map(|discovery| {
            Box::new(el(
                "div",
                (
                    el("div", text(discovery.label)).attr("class", "stage-context-title"),
                    el("div", text("Formula containment")).attr("class", "stage-context-label"),
                    el("div", text(discovery.explanation)),
                    el(
                        "div",
                        (
                            clickable(
                                el("div", text("Hear scale")).attr("class", "t-btn"),
                                |ui: &mut UiState, _| ui.hear_chord_scale(),
                            ),
                            clickable(
                                el("div", text("Stage scale")).attr("class", "t-btn"),
                                |ui: &mut UiState, _| {
                                    ui.stage_chord_scale();
                                },
                            ),
                        ),
                    )
                    .attr("class", "stage-context-actions"),
                ),
            )) as UiChild
        });
    Box::new(
        el(
            "div",
            (
                explore,
                ui.scale_source.map(|_| {
                    el(
                        "div",
                        text(format!("Scale formulas · up to {limit} · same root first; Major/Minor before extensions")),
                    )
                }),
                el("div", choices),
                (ui.scale_source.is_some() && limit < MAX_SCALE_CHOICES).then(|| clickable(
                    el("div", text("Show more scales")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| ui.show_more_chord_scales(),
                )),
                detail,
                current_unavailable.map(|notice| el("div", text(notice)).attr("role", "status")),
                ui.scale_notice
                    .as_ref()
                    .map(|notice| el("div", text(notice.clone())).attr("role", "status")),
                ui.scale_source.map(|_| {
                    clickable(
                        el("div", text("Close scales")).attr("class", "t-btn"),
                        |ui: &mut UiState, _| {
                            ui.scale_source = None;
                            ui.scale_subject = None;
                            ui.scale_notice = None;
                            ui.scale_limit = SCALE_CHOICES;
                        },
                    )
                }),
            ),
        )
        .attr("class", "stage-context-panel connected-scales"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::MarkMode;

    fn keyed(formula: &str, root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: formula.into(),
            root: PitchClass::new(root),
        }
    }

    fn source(ui: &mut UiState) -> CardId {
        assert!(ui.focus_context_catalog(keyed("chord:Major 7", 0)));
        ui.stage_current(None);
        ui.set.cards.last().unwrap().id
    }

    #[test]
    fn inspection_and_hearing_are_interest_and_staging_creates_distinct_occurrences() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        let original = serde_json::to_value(&ui.set.cards).unwrap();
        assert!(ui.explore_chord_scales(id));
        let choices = ui
            .stage
            .chord_scale_discoveries(&ui.set.cards[0], SCALE_CHOICES)
            .unwrap();
        assert!(choices.len() <= SCALE_CHOICES);
        assert!(
            choices
                .iter()
                .any(|choice| choice.subject == keyed("scale:Major", 0)),
            "first choices expose the familiar same-root Major formula"
        );
        ui.show_more_chord_scales();
        let expanded = ui
            .stage
            .chord_scale_discoveries(&ui.set.cards[0], ui.scale_limit)
            .unwrap();
        assert!(expanded.len() > choices.len());
        assert!(
            choices
                .iter()
                .all(|choice| expanded.iter().any(|next| next.subject == choice.subject))
        );
        assert!(
            expanded
                .iter()
                .any(|choice| choice.subject == keyed("scale:Major", 0)),
            "progressive disclosure must expose C Major for the first flow"
        );
        assert!(ui.inspect_chord_scale(id, keyed("scale:Major", 0)));
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), original);
        assert!(ui.audio_requests.is_empty());
        ui.hear_chord_scale();
        assert!(
            matches!(ui.audio_requests.last(), Some(AudioRequest::PreviewPitches { pitches, strum_s, .. }) if pitches.len() >= 7 && *strum_s > 0.0)
        );
        assert_eq!(serde_json::to_value(&ui.set.cards).unwrap(), original);
        let first = ui.stage_chord_scale().unwrap();
        let second = ui.stage_chord_scale().unwrap();
        assert_ne!(id, first);
        assert_ne!(first, second);
        assert_eq!(ui.scale_source, Some(id));
        assert!(
            ui.practice_history
                .recent(20)
                .iter()
                .all(|event| !event.kind.is_practice())
        );
        assert_eq!(ui.practice_history.total_practiced_ms("scale:Major"), 0);
    }

    #[test]
    fn staged_scale_clears_chord_shape_and_marks_without_editing_the_chord() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        ui.set.cards[0].setting.voicing_idx = Some(0);
        ui.set.cards[0].setting.marked = vec![(0, 3)];
        ui.set.cards[0].setting.mark_mode = MarkMode::Solo;
        ui.set.cards[0].timing.bpm = Some(83.0);
        let original = serde_json::to_value(&ui.set.cards[0]).unwrap();
        assert!(ui.inspect_chord_scale(id, keyed("scale:Major", 0)));
        let added = ui.stage_chord_scale().unwrap();
        let card = ui.set.cards.iter().find(|card| card.id == added).unwrap();
        assert_eq!(card.setting.voicing_idx, None);
        assert!(card.setting.marked.is_empty());
        assert_eq!(card.setting.mark_mode, MarkMode::Off);
        assert_eq!(card.timing.bpm, Some(83.0));
        assert_eq!(serde_json::to_value(&ui.set.cards[0]).unwrap(), original);
    }

    #[test]
    fn an_edited_source_rechecks_containment_and_a_removed_source_fails_closed() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        assert!(ui.inspect_chord_scale(id, keyed("scale:Major", 0)));
        ui.set.cards[0].material = Material::Chord {
            root: PitchClass::new(1),
            name: "Major 7".into(),
        };
        ui.hear_chord_scale();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_chord_scale().is_none());
        assert_eq!(ui.set.cards.len(), 1);
        assert!(ui.scale_notice.is_some());
        ui.set.remove(0);
        ui.hear_chord_scale();
        assert!(ui.audio_requests.is_empty());
        assert!(ui.stage_chord_scale().is_none());
        assert!(ui.set.cards.is_empty());
    }

    #[test]
    fn selecting_another_occurrence_does_not_retarget_the_inspected_source() {
        let mut ui = UiState::new();
        let id = source(&mut ui);
        ui.set.cards[0].timing.bpm = Some(83.0);
        assert!(ui.inspect_chord_scale(id, keyed("scale:Major", 0)));
        let other = source(&mut ui);
        ui.set.select_id(other);
        ui.set.cards[1].timing.bpm = Some(141.0);
        let added = ui.stage_chord_scale().unwrap();
        assert_eq!(
            ui.set
                .cards
                .iter()
                .find(|card| card.id == added)
                .unwrap()
                .timing
                .bpm,
            Some(83.0)
        );
    }
}
