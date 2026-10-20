// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Explicit progression alignment over the Set's persistent Card identities.
use crate::{StageState, format_role, fretboard_scene::selected_card_fretboard_scene};
use sceno::SourceRef;
use scenomise::{
    backdrops::{BACKDROP_SCENE_VERSION, SceneFacts, SceneInput},
    document::{LinkedScene, SceneDocument},
    template::{
        Placement, SequenceTemplate, Span, TemplateGroup, TemplateOccurrence, TemplateRelation,
        TemplateSlot, TemplatedSequence,
    },
};
use std::collections::HashSet;
use woodshedding::rehearsal::{Material, Set};

pub use woodshedding::rehearsal::arrangement::{
    ArrangementRest, CardAlignment, ProgressionArrangement,
};
fn scene_span(span: woodshedding::rehearsal::arrangement::Span) -> Span {
    Span {
        start: span.start,
        length: span.length,
    }
}
fn scene_placement(value: &woodshedding::rehearsal::arrangement::Placement) -> Placement {
    Placement {
        lane: value.lane,
        span: scene_span(value.span),
    }
}
/// Export the Set's retained musical context, independent of live navigation.
pub fn saved_arrangement_scene(set: &Set, reading_id: &str) -> Result<SceneDocument, String> {
    if reading_id.trim().is_empty() {
        return Err("A stable reading identity is required".into());
    }
    let stage = crate::saved_arrangements::saved_arrangement_stage(set)?;
    arrangement_scene(
        &stage,
        set,
        &set.progression_arrangement.as_ref().unwrap().alignment,
        reading_id,
    )
}
pub fn arrangement_scene(
    stage: &StageState,
    set: &Set,
    arrangement: &ProgressionArrangement,
    reading_id: &str,
) -> Result<SceneDocument, String> {
    let progression = stage
        .progression_idx
        .and_then(|i| stage.progressions().get(i))
        .ok_or("Select a progression")?;
    if arrangement.role_spans.len() != progression.roles.len()
        || set.cards.len() > 512
        || arrangement.rests.len() > 512
        || arrangement.cards.len() > 512
    {
        return Err(
            "Arrangement needs one expectation span per role and bounded source occurrences".into(),
        );
    }
    let derived_bars = woodshedding::rehearsal::arrangement::bars::bars(arrangement)?;
    let mut ids = HashSet::new();
    if set
        .cards
        .iter()
        .any(|c| !c.id.is_assigned() || !ids.insert(c.id))
    {
        return Err("Set must supply assigned, unique persistent CardIds".into());
    }
    let mut bound = HashSet::new();
    for binding in &arrangement.cards {
        if !ids.contains(&binding.card) || !bound.insert(binding.card) {
            return Err("Alignment must name one existing card at most once".into());
        }
    }
    let slot = |index: Option<usize>| -> Result<Option<String>, String> {
        index
            .map(|i| {
                if i < progression.roles.len() {
                    Ok(format!("role-{i}"))
                } else {
                    Err("Alignment names an absent role".into())
                }
            })
            .transpose()
    };
    let slots = progression
        .roles
        .iter()
        .zip(&arrangement.role_spans)
        .enumerate()
        .map(|(i, (role, span))| TemplateSlot {
            key: format!("role-{i}"),
            lane: 0,
            span: scene_span(*span),
            label: format_role(role),
            expectation: format!(
                "{} in {} major · source-declared expectation",
                format_role(role),
                stage.root_name()
            ),
        })
        .collect();
    let mut occurrences = Vec::new();
    let mut linked = Vec::new();
    for card in &set.cards {
        let Material::Chord { name, root } = &card.material else {
            return Err(
                "This adapter currently accepts chord cards; rests are explicit arrangement events"
                    .into(),
            );
        };
        let binding = arrangement.cards.iter().find(|b| b.card == card.id);
        let key = format!("card-{}", card.id.0);
        let capo = card.setting.capo.unwrap_or(0);
        occurrences.push(TemplateOccurrence {
            key: key.clone(),
            source: SourceRef {
                adapter: "woodshed.chord".into(),
                id: format!("{}:{name}:capo-{capo}", root.value()),
            },
            label: card.label.clone(),
            detail: format!(
                "{reading_id}#card={} · persistent Set occurrence · supplied alignment",
                card.id.0
            ),
            placement: binding.and_then(|b| b.placement.as_ref().map(scene_placement)),
            slot: slot(binding.and_then(|b| b.role))?,
        });
        if let Some(scene) =
            selected_card_fretboard_scene(stage, card, &format!("{reading_id}#card={}", card.id.0))?
        {
            linked.push(LinkedScene {
                occurrence: key,
                label: format!("Selected voicing · {}", card.label),
                scene,
            });
        }
    }
    for rest in &arrangement.rests {
        if rest.key.trim().is_empty() {
            return Err("Rest needs a stable occurrence identity".into());
        }
        occurrences.push(TemplateOccurrence {
            key: format!("rest-{}", rest.key),
            source: SourceRef {
                adapter: "woodshed.arrangement-rest".into(),
                id: format!("{reading_id}#rest={}", rest.key),
            },
            label: "Rest".into(),
            detail: "Explicit authored rest; no chord or voicing inferred".into(),
            placement: Some(scene_placement(&rest.placement)),
            slot: slot(rest.role)?,
        });
    }
    let relations = set
        .cards
        .windows(2)
        .map(|pair| TemplateRelation {
            from: format!("card-{}", pair[0].id.0),
            to: format!("card-{}", pair[1].id.0),
            kind: "woodshed.set-next".into(),
            label: "Next Set occurrence".into(),
            why: "Persistent Set order; spatial alignment and chord identity remain independent"
                .into(),
        })
        .collect();
    let mut groups = vec![TemplateGroup {
        key: "phrase".into(),
        lane: 0,
        span: Span {
            start: 0,
            length: arrangement.extent,
        },
        parent: None,
        label: progression.name.into(),
    }];
    if !arrangement.time_signatures.is_empty() {
        let mut bar_parents = vec!["phrase".to_owned(); derived_bars.len()];
        for (index, group) in arrangement.bar_groups.iter().enumerate() {
            let first = group.first as usize;
            let last = (group.first + group.count - 1) as usize;
            let start = derived_bars[first].start;
            let end = derived_bars[last].start + derived_bars[last].length;
            let key = format!("bar-group-{index}");
            groups.push(TemplateGroup {
                key: key.clone(),
                lane: 0,
                span: Span {
                    start,
                    length: end - start,
                },
                parent: Some("phrase".into()),
                label: format!("Bars {}–{}", group.first + 1, group.first + group.count),
            });
            for parent in &mut bar_parents[first..=last] {
                *parent = key.clone();
            }
        }
        for (index, bar) in derived_bars.iter().enumerate() {
            groups.push(TemplateGroup {
                key: format!("bar-{index}"),
                lane: 0,
                span: Span {
                    start: bar.start,
                    length: bar.length,
                },
                parent: Some(bar_parents[index].clone()),
                label: format!(
                    "Bar {}{}",
                    index + 1,
                    if bar.complete { "" } else { " · partial" }
                ),
            });
        }
    }
    let template = SequenceTemplate {
        lanes: vec![format!(
            "{} · expected roles below / supplied occurrences above",
            progression.name
        )],
        extent: arrangement.extent,
        ticks_per_unit: 4,
        unit_label: "quarter-note beat (4 ticks); supplied spans, no playback clock".into(),
        groups,
    };
    let facts = SceneFacts::TemplatedSequence(TemplatedSequence {
        template,
        slots,
        occurrences,
        relations,
    });
    let revision =
        blake3::hash(&serde_json::to_vec(&(&facts, &linked)).map_err(|e| e.to_string())?)
            .to_hex()
            .to_string();
    let doc = SceneDocument::new(
        SceneInput {
            version: BACKDROP_SCENE_VERSION,
            source: SourceRef {
                adapter: "woodshed.progression-arrangement".into(),
                id: reading_id.into(),
            },
            revision,
            title: format!(
                "{} · {} in {}",
                arrangement.name,
                progression.name,
                stage.root_name()
            ),
            facts,
        },
        linked,
    );
    doc.validate()?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lens, ROOT_NAMES};
    use woodshedding::rehearsal::CardId;
    use woodshedding::rehearsal::arrangement::{Placement, Span};
    fn fixture() -> (StageState, Set, ProgressionArrangement) {
        let mut stage = StageState::default();
        stage.lens = Lens::Progressions;
        stage.root_idx = ROOT_NAMES.iter().position(|s| *s == "C").unwrap();
        stage.select_progression(
            stage
                .progressions()
                .iter()
                .position(|p| p.name == "ii-V-I (Jazz)")
                .unwrap(),
        );
        let mut set = Set::default();
        for i in 0..3 {
            stage.progression_expand(i);
            let mut card = stage.card_from_lens().unwrap();
            stage.select_card_shape_index(&mut card, 0).unwrap();
            set.push(card);
        }
        set.duplicate(2);
        stage.select_card_shape_index(&mut set.cards[3], 1).unwrap();
        let cards = set
            .cards
            .iter()
            .enumerate()
            .map(|(i, c)| CardAlignment {
                card: c.id,
                placement: Some(Placement {
                    lane: 0,
                    span: Span {
                        start: [0, 16, 32, 48][i],
                        length: [16, 8, 16, 16][i],
                    },
                }),
                role: Some(i.min(2)),
            })
            .collect();
        (
            stage,
            set,
            ProgressionArrangement {
                name: "Varied".into(),
                extent: 64,
                role_spans: vec![
                    Span {
                        start: 0,
                        length: 16,
                    },
                    Span {
                        start: 16,
                        length: 16,
                    },
                    Span {
                        start: 32,
                        length: 32,
                    },
                ],
                cards,
                rests: vec![ArrangementRest {
                    key: "breath".into(),
                    placement: Placement {
                        lane: 0,
                        span: Span {
                            start: 24,
                            length: 8,
                        },
                    },
                    role: None,
                }],
                time_signatures: vec![],
                bar_groups: vec![],
            },
        )
    }
    #[test]
    fn real_arrangement_round_trips_as_editable_djot_without_changing_its_set() {
        let (stage, set, arrangement) = fixture();
        let before = serde_json::to_value(&set).unwrap();
        let document = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        let source = crate::arrangement_document::to_djot(&document).unwrap();
        assert_eq!(crate::arrangement_document::from_djot(&source).unwrap(), document);
        assert_eq!(serde_json::to_value(&set).unwrap(), before);
        let mut changed = document.clone(); changed.primary.title = "Edited independent arrangement".into();
        let next = crate::arrangement_document::to_djot(&changed).unwrap();
        assert_eq!(crate::arrangement_document::from_djot(&next).unwrap(), changed);
        assert_eq!(crate::arrangement_document::from_djot(&source).unwrap(), document);
    }

    #[test]
    fn repeated_tonics_keep_card_identity_and_different_saved_voicings() {
        let (stage, set, arrangement) = fixture();
        let before = serde_json::to_value(&set).unwrap();
        let doc = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        let SceneFacts::TemplatedSequence(s) = &doc.primary.facts else {
            unreachable!()
        };
        assert_eq!(s.occurrences[2].source, s.occurrences[3].source);
        assert_ne!(s.occurrences[2].key, s.occurrences[3].key);
        assert_ne!(doc.linked[2].scene.facts, doc.linked[3].scene.facts);
        assert_eq!(s.occurrences[4].label, "Rest");
        assert_eq!(serde_json::to_value(&set).unwrap(), before);
        assert_eq!(
            doc,
            SceneDocument::parse(&serde_json::to_string(&doc).unwrap()).unwrap()
        );
    }
    #[test]
    fn explicit_meter_exports_bars_and_authored_groups_under_the_phrase() {
        let (stage, set, mut arrangement) = fixture();
        arrangement.time_signatures = vec![woodshedding::rehearsal::arrangement::TimeSignatureChange {
            start: 0,
            numerator: 4,
            denominator: 4,
        }];
        arrangement.bar_groups = vec![woodshedding::rehearsal::arrangement::BarGroup {
            first: 0,
            count: 2,
        }];
        let doc = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        let SceneFacts::TemplatedSequence(sequence) = &doc.primary.facts else {
            unreachable!()
        };
        assert_eq!(sequence.template.groups.len(), 6);
        assert_eq!(sequence.template.groups[1].parent.as_deref(), Some("phrase"));
        assert_eq!(sequence.template.groups[2].parent.as_deref(), Some("bar-group-0"));
        assert_eq!(sequence.template.groups[4].parent.as_deref(), Some("phrase"));
    }
    #[test]
    fn reorder_insert_and_reopen_preserve_selection_keys_and_bindings() {
        let (stage, mut set, arrangement) = fixture();
        let doc = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        let id = set.cards[2].id;
        let key = format!("card-{}", id.0);
        set.move_card(2, -1);
        set.duplicate(0);
        let set: Set = serde_json::from_value(serde_json::to_value(&set).unwrap()).unwrap();
        let next = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        assert_eq!(
            doc.for_occurrence(Some(&key)),
            next.for_occurrence(Some(&key))
        );
        let SceneFacts::TemplatedSequence(s) = &next.primary.facts else {
            unreachable!()
        };
        assert!(s.occurrences.iter().any(|o| o.placement.is_none()));
        assert_eq!(s.slots.len(), 3);
    }
    #[test]
    fn missing_alignment_is_not_a_guessed_position_and_bad_source_is_refused() {
        let (stage, mut set, mut arrangement) = fixture();
        arrangement.cards.clear();
        let d = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        assert_eq!(d.primary.compile().unwrap().unplaced.len(), 4);
        set.cards[0].setting.voicing_fingerprint = Some("stale".into());
        assert!(arrangement_scene(&stage, &set, &arrangement, "study").is_err());
        set.cards[0].id = CardId::UNASSIGNED;
        assert!(arrangement_scene(&stage, &set, &arrangement, "study").is_err());
    }
    #[test]
    fn selected_capo_shape_keeps_physical_frets_and_concert_pitches_consistent() {
        let (stage, mut set, arrangement) = fixture();
        let card = &mut set.cards[0];
        card.setting.capo = Some(2);
        stage.select_card_shape_index(card, 0).unwrap();
        let d = arrangement_scene(&stage, &set, &arrangement, "study").unwrap();
        let SceneFacts::Fretboard(board) = &d.linked[0].scene.facts else {
            unreachable!()
        };
        assert!(board.positions.iter().all(|p| p.fret >= 2));
        d.validate().unwrap();
    }
}
