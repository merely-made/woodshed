// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::{UiChild, UiState};
use cambium::{button, clickable, el, text};
use mere_ontology::{Profile, TermKind};
use std::path::Path;
use woodshed_core::{ProgressionBoard, ProgressionCard};

pub(super) const CHORD: &str = "http://w3id.org/polifonia/ontology/roman-chord/Chord";
pub(super) const HAS_QUALITY: &str = "http://w3id.org/polifonia/ontology/roman-chord/hasQuality";
pub(super) const QUALITY: &str = "http://w3id.org/polifonia/ontology/roman-chord/Quality";

#[derive(Default)]
pub(super) struct ChordTerms<'a> {
    chord: bool,
    quality_label: Option<&'a str>,
}

/// Resolve Woodshed's active lens from its bundled and local subject packs.
pub fn load(directory: Option<&Path>) -> Result<Profile, String> {
    mere_ontology::load(
        include_str!("../../ontology/lens.toml"),
        &[include_str!("../../ontology/roman-chord.toml")],
        directory,
    )
}

impl<'a> ChordTerms<'a> {
    pub(super) fn from_profile(profile: &'a Profile) -> Self {
        let has = |iri, kind| {
            profile
                .terms
                .entry(iri)
                .is_some_and(|entry| entry.kind == kind)
        };
        let chord = has(CHORD, TermKind::Class);
        Self {
            chord,
            quality_label: profile
                .terms
                .entry(HAS_QUALITY)
                .filter(|term| {
                    chord && has(QUALITY, TermKind::Class) && term.kind == TermKind::Predicate
                })
                .map(|term| term.display_label("en")),
        }
    }

    pub(super) fn progression_controls(
        &self,
        board: &ProgressionBoard,
        filter: Option<(&'static str, &'static str)>,
    ) -> UiChild {
        let selected = filter
            .filter(|(name, _)| *name == board.name && self.quality_label.is_some())
            .map(|(_, quality)| quality);
        let mut controls: Vec<UiChild> = Vec::new();
        if let Some(label) = self.quality_label {
            let mut counts = std::collections::BTreeMap::new();
            for card in &board.cards {
                *counts.entry(card.quality).or_insert(0usize) += 1;
            }
            let name = board.name;
            let mut options: Vec<UiChild> = vec![Box::new(el("span", text(label.to_owned())))];
            for (quality, count) in std::iter::once((None, board.cards.len()))
                .chain(counts.into_iter().map(|(q, n)| (Some(q), n)))
            {
                let title = quality.unwrap_or("All positions");
                let active = selected == quality;
                options.push(Box::new(
                    button(format!("{title} ({count})"), move |ui: &mut UiState, _| {
                        ui.progression_quality = quality.map(|q| (name, q));
                    })
                    .attr("class", if active { "seg active" } else { "seg" })
                    .attr("aria-pressed", if active { "true" } else { "false" }),
                ));
            }
            controls.push(Box::new(
                el("div", options)
                    .attr("class", "prog-quality-filter")
                    .attr("role", "group")
                    .attr("aria-label", label.to_owned()),
            ));
        }
        let cards: Vec<UiChild> = board
            .cards
            .iter()
            .enumerate()
            .filter(|(_, card)| selected.is_none_or(|q| card.quality == q))
            .map(|(index, card)| self.card(index, card, selected.is_some()))
            .collect();
        controls.push(Box::new(el("div", cards).attr("class", "prog-cards")));
        Box::new(el("div", controls))
    }

    fn card(&self, index: usize, chord: &ProgressionCard, show_position: bool) -> UiChild {
        const LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
        let class = if chord.is_expanded {
            "prog-card prog-card-active"
        } else {
            "prog-card"
        };
        let mut numeral = el("div", text(chord.numeral.clone())).attr("class", "prog-numeral");
        if self.chord {
            numeral = numeral.attr("property", LABEL);
        }
        let mut fields: Vec<UiChild> = vec![
            Box::new(numeral),
            Box::new(el("div", text(chord.chord_label.clone())).attr("class", "prog-chord")),
        ];
        if show_position {
            fields.push(Box::new(
                el("span", text(format!("Position {}", index + 1))).attr("class", "prog-position"),
            ));
        }
        if chord.is_expanded && self.quality_label.is_some() {
            fields.push(Box::new(
                el(
                    "div",
                    el("span", text(chord.quality)).attr("property", LABEL),
                )
                .attr("class", "prog-quality")
                .attr("property", HAS_QUALITY)
                .attr("typeof", QUALITY),
            ));
        }
        let mut card = el("div", fields)
            .attr("class", class)
            .attr("data-position", (index + 1).to_string());
        if self.chord {
            card = card.attr("typeof", CHORD);
        }
        Box::new(clickable(card, move |ui: &mut UiState, _| {
            ui.stage.progression_expand(index)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::{UiState, progression_board_view};
    use cambium_genet_winit_host::Harness;
    use taproot::Selector;

    #[test]
    fn quality_filter_counts_occurrences_and_opens_the_original_position() {
        let mut ui = UiState::new();
        let custom = include_str!("../../ontology/roman-chord.toml")
            .replace("Chord quality", "Chord colour");
        ui.ontology =
            mere_ontology::load(include_str!("../../ontology/lens.toml"), &[&custom], None);
        let index = ui
            .stage
            .progressions()
            .iter()
            .position(|p| p.name == "I-V-vi-IV")
            .unwrap();
        ui.stage.lens = woodshed_core::Lens::Progressions;
        ui.stage.select_progression(index);
        let board = ui.stage.progression_board().unwrap();
        let major = board.cards[0].quality;
        let minor = board.cards[2].quality;
        assert_ne!(major, minor);
        let mut authored = ui.stage.card_from_lens().unwrap();
        authored.label = "Keep my edited name".into();
        ui.set.push(authored.clone());
        ui.set.push(authored);
        let original_set = serde_json::to_value(&ui.set).unwrap();
        let mut host = Harness::new(crate::theme::slate_stage_css(), ui, progression_board_view);
        host.layout_at(1200.0, 800.0);
        let group = Selector::class("prog-quality-filter").with_attr("aria-label", "Chord colour");
        host.with_surfaces(|s| assert_eq!(taproot::matching_classes(s[0].dom, &group).len(), 1));
        let major_button = Selector::role("button").containing(&format!("{major} (3)"));
        assert!(host.click_on(&major_button));
        assert_eq!(host.state().stage.progression_expanded, 0);
        host.with_surfaces(|s| {
            assert_eq!(
                taproot::matching_classes(s[0].dom, &Selector::class("prog-card")).len(),
                3
            );
            assert!(
                taproot::matching_classes(
                    s[0].dom,
                    &Selector::class("prog-card").with_attr("data-position", "3")
                )
                .is_empty()
            );
        });
        host.with_surfaces(|s| {
            assert_eq!(
                taproot::matching_classes(
                    s[0].dom,
                    &Selector::class("prog-position").containing("Position 4")
                )
                .len(),
                1
            )
        });
        assert!(host.click_on(&Selector::class("prog-card").with_attr("data-position", "4")));
        assert_eq!(host.state().stage.progression_expanded, 3);
        assert!(host.click_on(&Selector::role("button").containing("All positions (4)")));
        host.with_surfaces(|s| {
            assert_eq!(
                taproot::matching_classes(s[0].dom, &Selector::class("prog-card")).len(),
                4
            )
        });
        assert_eq!(host.state().stage.progression_expanded, 3);
        assert!(host.click_on(&major_button));
        host.update(|ui| {
            let blues = ui
                .stage
                .progressions()
                .iter()
                .position(|p| p.name == "12-Bar Blues")
                .unwrap();
            ui.stage.select_progression(blues);
        });
        host.with_surfaces(|s| {
            let all = Selector::class("seg")
                .with_attr("aria-pressed", "true")
                .containing("All positions (12)");
            assert_eq!(taproot::matching_classes(s[0].dom, &all).len(), 1);
            let dominant = Selector::class("seg").containing("Dominant 7 (12)");
            assert_eq!(taproot::matching_classes(s[0].dom, &dominant).len(), 1);
        });
        assert_eq!(
            serde_json::to_value(&host.state().set).unwrap(),
            original_set
        );
    }

    #[test]
    fn selected_roman_chord_terms_type_the_live_progression_cards() {
        let mut ui = UiState::new();
        ui.ontology = load(None);
        ui.stage.select_progression(0);
        let cards = ui.stage.progression_board().unwrap().cards;
        let mut host = Harness::new(crate::theme::slate_stage_css(), ui, progression_board_view);
        let selector = Selector::class("prog-card").with_attr("typeof", CHORD);
        host.with_surfaces(|surfaces| {
            let quality = Selector::class("prog-quality").with_attr("property", HAS_QUALITY);
            assert_eq!(
                taproot::matching_classes(surfaces[0].dom, &quality).len(),
                1
            );
            assert_eq!(
                taproot::matching_classes(surfaces[0].dom, &selector).len(),
                cards.len()
            );
        });
        let bare = mere_ontology::load(
            r#"
            id = "urn:woodshed:test:bare"
            version = "0.1.0"
            kind = ["lens"]
            licence = "MPL-2.0"
            [lens]
        "#,
            &[],
            None,
        )
        .unwrap();
        host.update(|ui| ui.ontology = Ok(bare));
        host.with_surfaces(|surfaces| {
            assert!(taproot::matching_classes(surfaces[0].dom, &selector).is_empty());
            assert!(
                taproot::matching_classes(surfaces[0].dom, &Selector::class("prog-quality-filter"))
                    .is_empty()
            );
            assert_eq!(
                taproot::matching_classes(surfaces[0].dom, &Selector::class("prog-card")).len(),
                cards.len()
            );
        });
    }
}
