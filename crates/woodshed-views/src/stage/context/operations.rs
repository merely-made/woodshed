use super::*;

impl UiState {
    /// Focus a catalog material from the context graph. Selection updates the
    /// board so the surrounding graph can be recomputed, but this never adds a
    /// card to the Set.
    pub fn focus_context_catalog(&mut self, material: KeyedCatalogRef) -> bool {
        if material.formula_id.starts_with("arpeggio:") {
            let source = self.current_card().and_then(|card| {
                KeyedCatalogRef::from_material(&card.material).and_then(|mut keyed| {
                    keyed.formula_id = keyed.formula_id.replacen("chord:", "arpeggio:", 1);
                    (keyed == material).then_some(card.id)
                })
            });
            if let Some(source) = source {
                self.inspect_chord_arpeggio(source);
            }
        }
        if let Some(Material::ChordApproach {
            name,
            root,
            direction,
        }) = material.to_material()
        {
            let pair = self.current_chord_approach_pair().filter(|(_, target)| {
                self.set.cards.iter().find(|card| card.id == *target).is_some_and(|card|
                    matches!(&card.material, Material::Chord { name: target_name, root: target_root } if *target_name == name && *target_root == root))
            });
            if let Some((source, target)) = pair {
                self.inspect_chord_approach(source, target, direction);
            }
        }
        if let Some(Material::ScalePattern {
            name,
            root,
            pattern,
        }) = material.to_material()
        {
            let source = self.current_card().and_then(|card| match &card.material {
                Material::Scale {
                    name: source_name,
                    root: source_root,
                } if *source_name == name && *source_root == root => Some(card.id),
                _ => None,
            });
            if let Some(source) = source {
                self.inspect_scale_pattern(source, pattern);
            }
        }
        if !select_keyed_material(&mut self.stage, &material) {
            return false;
        }
        self.root_dd.selected = self.stage.root_idx;
        self.context_disclosed.insert(material.clone());
        self.context_focus = Some(material);
        self.related_hover = None;
        self.related_relation = None;
        true
    }

    /// Hear the currently focused context material. Audition is intentionally
    /// separate from both focus and Add to Set.
    pub fn audition_context_focus(&mut self) {
        if let Some(material) = self.context_focus.clone() {
            if self.is_inspected_chord_approach(&material) {
                self.hear_chord_approach();
                return;
            }
            if self.is_inspected_scale_pattern(&material) {
                self.hear_scale_pattern();
                return;
            }
            if self.is_inspected_arpeggio(&material) {
                self.hear_chord_arpeggio();
                return;
            }
            if !select_keyed_material(&mut self.stage, &material) {
                return;
            }
            self.root_dd.selected = self.stage.root_idx;
            if material.formula_id.starts_with("chord-approach:") {
                self.hear_context_chord_approach(&material);
                return;
            }
            if material.formula_id.starts_with("scale-pattern:") {
                self.hear_context_scale_pattern(&material);
                return;
            }
            let (pitches, duration_s, strum_s) = if material.formula_id.starts_with("arpeggio:") {
                let Some(card) = self.stage.card_from_lens() else {
                    return;
                };
                match self
                    .stage
                    .card_sounding_pitches_at_tempo(&card, self.transport.bpm)
                {
                    Ok(preview) => {
                        self.card_timing_notice = None;
                        preview
                    },
                    Err(error) => {
                        self.card_timing_notice = Some(error.to_string());
                        return;
                    },
                }
            } else {
                self.stage.voicing_preview()
            };
            if !pitches.is_empty() {
                let provenance = self.stage.card_from_lens().and_then(|card| {
                    ObservationProvenance::capture_in_set(&card, self.working_sets.active_id).ok()
                });
                self.request_observed_preview(
                    (pitches, duration_s, strum_s),
                    Some(material.formula_id.clone()),
                    None,
                    provenance,
                );
            }
        }
    }

    /// Add the focused context material to the Set, preserving the existing
    /// provenance path used by the Related panel's explicit Stage action.
    pub fn add_context_focus_to_set(&mut self) {
        let Some(material) = self.context_focus.clone() else {
            return;
        };
        if self.is_inspected_chord_approach(&material) {
            self.stage_chord_approach();
            return;
        }
        if self.is_inspected_scale_pattern(&material) {
            self.stage_scale_pattern();
            return;
        }
        if self.is_inspected_arpeggio(&material) {
            self.stage_chord_arpeggio();
            return;
        }
        let from_id = self.stage.catalog_id();
        if self.focus_context_catalog(material.clone()) {
            if self.is_inspected_chord_approach(&material) {
                self.stage_chord_approach();
            } else if material.formula_id.starts_with("chord-approach:") {
                self.stage_context_chord_approach(&material);
            } else if self.is_inspected_scale_pattern(&material) {
                self.stage_scale_pattern();
            } else if material.formula_id.starts_with("scale-pattern:") {
                self.stage_context_scale_pattern(&material);
            } else {
                self.stage_current(from_id);
            }
        }
    }

    /// Clear only the context focus. The current board selection and Set stay
    /// intact, which makes this safe for a small dismiss affordance.
    pub fn clear_context_focus(&mut self) {
        self.context_focus = None;
    }

    pub fn show_more_context(&mut self) {
        let snapshot = super::super::set_graph_snapshot(self);
        let shown = snapshot
            .nodes
            .iter()
            .filter(|node| !node.foreground)
            .count();
        self.context_disclosed.extend(
            snapshot
                .nodes
                .iter()
                .filter_map(|node| (!node.foreground).then(|| node.keyed.clone()).flatten()),
        );
        self.app_settings.stage.context.node_limit = self
            .app_settings
            .stage
            .context
            .node_limit()
            .max(shown + 8)
            .min(36);
    }

    /// Disclose the next reading-owned candidates without changing Set
    /// membership or auditioning anything. Existing positions remain the
    /// scene's responsibility; these keys only widen the read model.
    pub fn show_nearby_context(&mut self) {
        let Some(focus) = self.context_focus.clone().or_else(|| {
            self.set
                .cursor_id()
                .and_then(|id| self.set.cards.iter().find(|card| card.id == id))
                .and_then(|card| KeyedCatalogRef::from_material(&card.material))
        }) else {
            return;
        };
        let authored = self
            .set
            .cards
            .iter()
            .filter_map(|card| KeyedCatalogRef::from_material(&card.material))
            .collect::<std::collections::BTreeSet<_>>();
        let candidates = woodshed_core::stage_candidates::ranked_candidates(
            self.app_settings.stage.set_graph_reading,
            &focus,
            &authored,
        );
        let displayed = super::super::set_graph_snapshot(self)
            .nodes
            .iter()
            .filter(|node| !node.foreground)
            .filter_map(|node| node.keyed.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let limit = self.app_settings.stage.context.node_limit();
        let free = limit.saturating_sub(displayed.len());
        let incoming: Vec<_> = candidates
            .into_iter()
            .filter(|candidate| !displayed.contains(&candidate.keyed))
            .take(if free == 0 { 6 } else { free.min(6) })
            .map(|candidate| candidate.keyed)
            .collect();
        if incoming.is_empty() {
            return;
        }
        let mut retained = std::collections::BTreeSet::new();
        if let Some(focus) = self
            .context_focus
            .as_ref()
            .filter(|focus| !authored.contains(*focus))
        {
            retained.insert(focus.clone());
        }
        // Retain manually placed context if a reading supplies it. Fixed musical
        // readings normally have no such overrides.
        retained.extend(
            displayed
                .iter()
                .filter(|key| self.context_positions.contains_key(&key.wire_key()))
                .cloned(),
        );
        for candidate in incoming {
            if retained.len() == limit {
                break;
            }
            retained.insert(candidate);
        }
        for existing in displayed {
            if retained.len() == limit {
                break;
            }
            retained.insert(existing);
        }
        self.context_disclosed = retained;
    }
}
