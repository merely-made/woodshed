use super::*;

impl StageState {
    /// Explainable immediate neighbors of the current catalog material. The
    /// graph owns relations; core maps stable catalog identity back into the
    /// selection handles the product already uses.
    pub fn related_material(&self, limit: usize) -> Vec<RelatedSuggestion> {
        let selected_id = match self.lens {
            Lens::Scales => woodshed_graph::scale_id(self.scale().name),
            Lens::Chords => woodshed_graph::chord_id(self.chord().name),
            Lens::Arpeggios => woodshed_graph::arpeggio_id(self.arpeggio_chord().name),
            Lens::Progressions => {
                let Some(idx) = self.progression_idx else {
                    return Vec::new();
                };
                woodshed_graph::progression_id(self.progressions()[idx].name)
            },
            Lens::Exercises => woodshed_graph::exercise_id(self.exercise().name),
        };

        woodshed_graph::related_neighbors(&selected_id, limit)
            .into_iter()
            .filter_map(|item| {
                use woodshed_graph::CatalogKind;
                let (kind, target) = match item.kind {
                    CatalogKind::Scale => (
                        "Scale",
                        RelatedTarget::Scale(
                            self.scales().iter().position(|x| x.name == item.name)?,
                        ),
                    ),
                    CatalogKind::Chord => (
                        "Chord",
                        RelatedTarget::Chord(
                            self.chords().iter().position(|x| x.name == item.name)?,
                        ),
                    ),
                    CatalogKind::Arpeggio => (
                        "Arpeggio",
                        RelatedTarget::Arpeggio(
                            self.chords().iter().position(|x| x.name == item.name)?,
                        ),
                    ),
                    CatalogKind::Progression => (
                        "Progression",
                        RelatedTarget::Progression(
                            self.progressions()
                                .iter()
                                .position(|x| x.name == item.name)?,
                        ),
                    ),
                    CatalogKind::Exercise => (
                        "Exercise",
                        RelatedTarget::Exercise(
                            self.exercises().iter().position(|x| x.name == item.name)?,
                        ),
                    ),
                };
                Some(RelatedSuggestion {
                    title: item.name,
                    kind,
                    relations: item.relations,
                    score: item.score,
                    target,
                })
            })
            .collect()
    }

    pub fn catalog_id(&self) -> Option<String> {
        match self.lens {
            Lens::Scales => Some(woodshed_graph::scale_id(self.scale().name)),
            Lens::Chords => Some(woodshed_graph::chord_id(self.chord().name)),
            Lens::Arpeggios => Some(woodshed_graph::arpeggio_id(self.arpeggio_chord().name)),
            Lens::Progressions => self
                .progression_idx
                .map(|idx| woodshed_graph::progression_id(self.progressions()[idx].name)),
            Lens::Exercises => Some(woodshed_graph::exercise_id(self.exercise().name)),
        }
    }

    pub fn related_material_with_history(
        &self,
        history: &history::PracticeHistory,
        limit: usize,
    ) -> Vec<RelatedSuggestion> {
        let Some(selected_id) = self.catalog_id() else {
            return Vec::new();
        };
        let mut ranked: Vec<(usize, usize, RelatedSuggestion)> = self
            .related_material(usize::MAX)
            .into_iter()
            .enumerate()
            .map(|(stable_order, mut suggestion)| {
                let target_id = match suggestion.target {
                    RelatedTarget::Scale(idx) => woodshed_graph::scale_id(self.scales()[idx].name),
                    RelatedTarget::Chord(idx) => woodshed_graph::chord_id(self.chords()[idx].name),
                    RelatedTarget::Arpeggio(idx) => {
                        woodshed_graph::arpeggio_id(self.chords()[idx].name)
                    },
                    RelatedTarget::Progression(idx) => {
                        woodshed_graph::progression_id(self.progressions()[idx].name)
                    },
                    RelatedTarget::Exercise(idx) => {
                        woodshed_graph::exercise_id(self.exercises()[idx].name)
                    },
                };
                let count = history.related_transition_count(&selected_id, &target_id);
                if count > 0 {
                    // Evidence is an additional relation, not a replacement.
                    // The old boundary overwrote the theory reason here, which
                    // is exactly the erasure P4b removes: a pair that is both
                    // diatonic and practiced-after is both, and says so.
                    let explanation = if count == 1 {
                        "Previously practiced from here".to_string()
                    } else {
                        format!("Practiced from here {count} times")
                    };
                    suggestion.relations.insert(
                        0,
                        woodshed_graph::MaterialRelation::evidence_with_count(
                            selected_id.clone(),
                            target_id.clone(),
                            woodshed_graph::RelationKind::PracticedAfter,
                            (90 + count.min(10)) as u16,
                            explanation,
                            mere::PracticeEvidenceBasis::StatedFromReason,
                            u64::try_from(count).expect("practice reason count must fit u64"),
                        ),
                    );
                }
                (count, stable_order, suggestion)
            })
            .collect();
        ranked.sort_by_key(|(count, stable_order, _)| (std::cmp::Reverse(*count), *stable_order));
        ranked
            .into_iter()
            .take(limit)
            .map(|(_, _, suggestion)| suggestion)
            .collect()
    }

    pub fn related_target_id(&self, target: RelatedTarget) -> String {
        match target {
            RelatedTarget::Scale(idx) => woodshed_graph::scale_id(self.scales()[idx].name),
            RelatedTarget::Chord(idx) => woodshed_graph::chord_id(self.chords()[idx].name),
            RelatedTarget::Arpeggio(idx) => woodshed_graph::arpeggio_id(self.chords()[idx].name),
            RelatedTarget::Progression(idx) => {
                woodshed_graph::progression_id(self.progressions()[idx].name)
            },
            RelatedTarget::Exercise(idx) => woodshed_graph::exercise_id(self.exercises()[idx].name),
        }
    }

    pub fn related_target_for_id(&self, id: &str) -> Option<RelatedTarget> {
        let (kind, name) = id.split_once(':')?;
        match kind {
            "scale" => self
                .scales()
                .iter()
                .position(|item| item.name == name)
                .map(RelatedTarget::Scale),
            "chord" => self
                .chords()
                .iter()
                .position(|item| item.name == name)
                .map(RelatedTarget::Chord),
            "arpeggio" => self
                .chords()
                .iter()
                .position(|item| item.name == name)
                .map(RelatedTarget::Arpeggio),
            "progression" => self
                .progressions()
                .iter()
                .position(|item| item.name == name)
                .map(RelatedTarget::Progression),
            "exercise" => self
                .exercises()
                .iter()
                .position(|item| item.name == name)
                .map(RelatedTarget::Exercise),
            _ => None,
        }
    }

    pub fn select_catalog_id(&mut self, id: &str) -> bool {
        let Some(target) = self.related_target_for_id(id) else {
            return false;
        };
        self.select_related(target);
        true
    }

    pub fn related_material_configured(
        &self,
        history: &history::PracticeHistory,
        settings: &storage::RelatedSettings,
        limit: usize,
    ) -> Vec<RelatedSuggestion> {
        let suggestions = if settings.use_history {
            self.related_material_with_history(history, usize::MAX)
        } else {
            self.related_material(usize::MAX)
        };
        let kept: Vec<RelatedSuggestion> = suggestions
            .into_iter()
            .filter(|suggestion| {
                !settings
                    .dismissed_ids
                    .contains(&self.related_target_id(suggestion.target))
            })
            .collect();
        diversify(kept, limit)
    }

    pub fn neighborhood_snapshot(
        &self,
        history: &history::PracticeHistory,
        settings: &storage::RelatedSettings,
        limit: usize,
    ) -> NeighborhoodSnapshot {
        let Some(center_id) = self.catalog_id() else {
            return NeighborhoodSnapshot::default();
        };
        let center_title = center_id
            .split_once(':')
            .map(|(_, title)| title)
            .unwrap_or(center_id.as_str())
            .to_string();
        let mut nodes = vec![NeighborhoodNode {
            id: center_id,
            title: center_title,
            kind: self.lens.label(),
            score: 100,
        }];
        nodes.extend(
            self.related_material_configured(history, settings, limit)
                .into_iter()
                .map(|suggestion| NeighborhoodNode {
                    id: self.related_target_id(suggestion.target),
                    title: suggestion.title,
                    kind: suggestion.kind,
                    score: suggestion.score,
                }),
        );
        let edges = (1..nodes.len()).map(|idx| (0, idx as u16)).collect();
        NeighborhoodSnapshot { nodes, edges }
    }

    pub fn select_related(&mut self, target: RelatedTarget) {
        match target {
            RelatedTarget::Scale(idx) => {
                self.set_lens(Lens::Scales);
                self.select_scale(idx);
            },
            RelatedTarget::Chord(idx) => {
                self.set_lens(Lens::Chords);
                self.select_chord(idx);
            },
            RelatedTarget::Arpeggio(idx) => {
                self.set_lens(Lens::Arpeggios);
                self.select_arpeggio(idx);
            },
            RelatedTarget::Progression(idx) => {
                self.set_lens(Lens::Progressions);
                self.select_progression(idx);
            },
            RelatedTarget::Exercise(idx) => {
                self.set_lens(Lens::Exercises);
                self.select_exercise(idx);
            },
        }
    }
}
