use cambium::{
    HoverEvent, HoverPhase, OpenedLinkEvent, clickable, el,
    graph_canvas_swatch_with_focus_and_drag_and_relations, map_state, on_hover, text,
    text_field_typed,
};
use scenograph::link::{
    LinkAssertion, LinkAssertionBasis, LinkAssertionMethod, LinkBucket, LinkCoverage, LinkEndpoint,
    LinkGraphScope, LinkKey, LinkPairReading, LinkReading, LinkSource, LinkSourceRevision,
    LinkStratum, LinkWithdrawal, OpenedLinkLayout,
};
use scenograph::swatch::{Scope, Swatch, SwatchMode};
use woodshed_core::RelatedSuggestion;
use woodshed_core::mere::{GeneratedEvidenceRef, PracticeEvidenceBasis};
use woodshed_core::personal_relations::{Author, EdgeId, RelationClass};

use super::{
    PersonalRelationIntent, RELATED_LIMIT, UiChild, UiState, related_mere_snapshot, related_swatch,
};

fn queue_relation_action(ui: &mut UiState, event: OpenedLinkEvent, expected_revision: u64) {
    match event {
        OpenedLinkEvent::Layout(layout) => ui.personal_link_layout = Some(layout),
        OpenedLinkEvent::Member { target, action } => {
            let Some((from, to)) = ui.personal_link_pair.clone() else {
                ui.personal_relation_notice =
                    Some("Open a link pair before acting on an assertion.".into());
                return;
            };
            let expected_key = LinkKey::new(LinkStratum::Surface, from.clone(), to.clone());
            if target.source != claims_source(ui) || target.link != expected_key {
                ui.personal_relation_notice =
                    Some("This assertion reading is stale; reopen the link before acting.".into());
                return;
            }
            if let Some(claim_id) = target.assertion_id.strip_prefix("generated:") {
                let snapshot = super::opened_claims_mere_snapshot(ui, &target.from);
                let exact = snapshot.relations.iter().find(|relation| {
                    relation.claim_id.qualified_key() == claim_id
                        && relation.provenance.original_from == target.from
                        && relation.provenance.original_to == target.to
                });
                match (exact, action.as_str()) {
                    (Some(_), "audition") => {
                        let Some((before, before_duration, before_strum)) =
                            endpoint_preview(ui, &target.from)
                        else {
                            ui.personal_relation_notice = Some(
                                "The source endpoint cannot be auditioned from the current catalog."
                                    .into(),
                            );
                            return;
                        };
                        let Some((after, after_duration, after_strum)) =
                            endpoint_preview(ui, &target.to)
                        else {
                            ui.personal_relation_notice = Some(
                                "The target endpoint cannot be auditioned from the current catalog."
                                    .into(),
                            );
                            return;
                        };
                        ui.request(woodshed_core::audio::AudioRequest::PreviewPhrase {
                            before,
                            after,
                            chord_duration_s: before_duration.max(after_duration),
                            gap_s: 0.22,
                            strum_s: before_strum.max(after_strum),
                        });
                        ui.personal_relation_notice = Some(format!(
                            "Audition queued in assertion direction: {} → {}",
                            target.from, target.to
                        ));
                    },
                    _ => {
                        ui.personal_relation_notice = Some(
                            "Generated catalog, computed, and evidence claims are read-only; only current exact claims can be auditioned.".into(),
                        );
                    },
                }
                return;
            }
            let Some(source) = ui.personal_relations.as_ref() else {
                ui.personal_relation_notice =
                    Some("The personal assertion source is unavailable.".into());
                return;
            };
            if source.runtime_revision() != expected_revision {
                ui.personal_relation_notice =
                    Some("This assertion reading is stale; reopen the link before acting.".into());
                return;
            }
            let expected_source_id = source.source_id().clone();
            let Some(raw_id) = target.assertion_id.strip_prefix("personal:") else {
                ui.personal_relation_notice =
                    Some("The assertion handle has an unknown source namespace.".into());
                return;
            };
            let id: EdgeId = match serde_json::from_str(raw_id) {
                Ok(id) => id,
                Err(error) => {
                    ui.personal_relation_notice =
                        Some(format!("The assertion handle could not be read: {error}"));
                    return;
                },
            };
            let Some(record) = source.retained_assertions().find(|record| {
                record.id == id && record.from == target.from && record.to == target.to
            }) else {
                ui.personal_relation_notice =
                    Some("The exact source assertion is no longer present.".into());
                return;
            };
            if record.from != target.from || record.to != target.to {
                ui.personal_relation_notice = Some("The exact assertion direction changed.".into());
                return;
            }
            let currently_held = record.span.is_held();
            let actor = sealed_actor(ui);
            let owned = actor
                .as_ref()
                .is_some_and(|actor| record.asserter.as_ref() == Some(actor));
            match action.as_str() {
                "audition" if currently_held => {
                    let Some((before, before_duration, before_strum)) =
                        endpoint_preview(ui, &target.from)
                    else {
                        ui.personal_relation_notice = Some(
                            "The source endpoint cannot be auditioned from the current catalog."
                                .into(),
                        );
                        return;
                    };
                    let Some((after, after_duration, after_strum)) =
                        endpoint_preview(ui, &target.to)
                    else {
                        ui.personal_relation_notice = Some(
                            "The target endpoint cannot be auditioned from the current catalog."
                                .into(),
                        );
                        return;
                    };
                    ui.request(woodshed_core::audio::AudioRequest::PreviewPhrase {
                        before,
                        after,
                        chord_duration_s: before_duration.max(after_duration),
                        gap_s: 0.22,
                        strum_s: before_strum.max(after_strum),
                    });
                    ui.personal_relation_notice = Some(format!(
                        "Audition queued in assertion direction: {} → {}",
                        target.from, target.to
                    ));
                },
                "confirm" if currently_held && owned => {
                    ui.refresh_event_time();
                    ui.personal_relation_intents
                        .push(PersonalRelationIntent::Confirm {
                            expected_source_id: expected_source_id.clone(),
                            expected_revision,
                            id: raw_id.to_owned(),
                            from: target.from,
                            to: target.to,
                            at_ms: ui.now_ms,
                        });
                    ui.personal_relation_notice = Some("Saving confirmation…".into());
                },
                "withdraw" if currently_held && owned => {
                    ui.refresh_event_time();
                    ui.personal_relation_intents
                        .push(PersonalRelationIntent::Withdraw {
                            expected_source_id: expected_source_id.clone(),
                            expected_revision,
                            id: raw_id.to_owned(),
                            from: target.from,
                            to: target.to,
                            at_ms: ui.now_ms,
                        });
                    ui.personal_relation_notice = Some("Saving withdrawal…".into());
                },
                _ => {
                    ui.personal_relation_notice =
                        Some("That action is not available for this exact assertion.".into())
                },
            }
        },
    }
}

fn endpoint_preview(ui: &UiState, catalog_id: &str) -> Option<(Vec<f32>, f32, f32)> {
    // StageState intentionally has no Clone: its transient shape cache is
    // mutable interior state. Resolve the endpoint in a fresh snapshot carrying
    // the live tuning/root/window instead of cloning that cache.
    let mut stage = woodshed_core::StageState::new();
    stage.tuning_idx = ui.stage.tuning_idx;
    stage.root_idx = ui.stage.root_idx;
    stage.fret_count = ui.stage.fret_count;
    stage.fret_start = ui.stage.fret_start;
    if !stage.select_catalog_id(catalog_id) {
        return None;
    }
    let card = stage.card_from_lens()?;
    let (pitches, duration, strum) = stage
        .card_sounding_pitches_at_tempo(&card, ui.transport.bpm)
        .ok()?;
    (!pitches.is_empty()).then_some((pitches, duration, strum))
}

fn activate_related_relation(ui: &mut UiState, relation_id: String) {
    if ui.related_relation.as_deref() == Some(relation_id.as_str()) {
        ui.related_relation = None;
        ui.personal_link_pair = None;
        return;
    }
    let key = related_mere_snapshot(ui)
        .relations
        .into_iter()
        .map(|claim| {
            LinkKey::new(
                LinkStratum::Surface,
                claim.provenance.original_from,
                claim.provenance.original_to,
            )
        })
        .find(|key| key.stable_id() == relation_id);
    let Some(key) = key else { return };
    ui.related_relation = Some(relation_id);
    ui.personal_link_pair = Some((key.first, key.second));
    ui.personal_link_history = false;
    ui.personal_link_layout = None;
    ui.personal_relation_notice = None;
}

/// The relation kinds beyond the one the row already shows, so multiplicity is
/// visible rather than silently ranked away. Empty when there is only one.
fn also_label(item: &RelatedSuggestion) -> String {
    if item.relation_count() < 2 {
        return String::new();
    }
    let others: Vec<&str> = item
        .relations
        .iter()
        .skip(1)
        .map(|relation| relation.kind.label())
        .collect();
    format!("also {}", others.join(" · "))
}

pub(super) fn panel(ui: &UiState) -> UiChild {
    let suggestions = ui.stage.related_material_configured(
        &ui.practice_history,
        &ui.app_settings.stage.related,
        RELATED_LIMIT,
    );

    // The suggestions as a structured pane: one row each, columns for kind /
    // name+why / actions. Hovering a row (or its graph node) highlights the
    // other through `related_hover`; the copy navigates, Stage stages, × hides.
    let rows: Vec<UiChild> = suggestions
        .iter()
        .map(|item| {
            let target = item.target;
            let dismiss_id = ui.stage.related_target_id(target);
            let row_class = if ui.related_hover == Some(target) {
                "related-row hovered"
            } else {
                "related-row"
            };
            Box::new(on_hover(
                el(
                    "div",
                    (
                        clickable(
                            el(
                                "div",
                                (
                                    el("div", text(item.kind)).attr(
                                        "class",
                                        format!("related-kind kind-{}", item.kind.to_lowercase()),
                                    ),
                                    el(
                                        "div",
                                        (
                                            el("div", text(item.title.clone()))
                                                .attr("class", "related-name"),
                                            el("div", text(item.reason().to_string()))
                                                .attr("class", "related-why"),
                                            // A pair related several ways says
                                            // so. The row shows one reason for
                                            // room, not because the others were
                                            // ranked away.
                                            el("div", text(also_label(item)))
                                                .attr("class", "related-also"),
                                        ),
                                    )
                                    .attr("class", "related-copy-text"),
                                ),
                            )
                            .attr("class", "related-copy"),
                            move |ui: &mut UiState, _| {
                                ui.stage.select_related(target);
                            },
                        ),
                        el(
                            "div",
                            (
                                clickable(
                                    el(
                                        "div",
                                        text(format!("Stage at end · {}", ui.set.cards.len() + 1)),
                                    )
                                    .attr("class", "related-stage")
                                    .attr("data-target", dismiss_id.clone())
                                    .attr(
                                        "aria-label",
                                        format!(
                                            "Stage {} at end as Card {}",
                                            item.title,
                                            ui.set.cards.len() + 1
                                        ),
                                    ),
                                    move |ui: &mut UiState, _| {
                                        let from_id = ui.stage.catalog_id();
                                        ui.stage.select_related(target);
                                        ui.stage_current(from_id);
                                    },
                                ),
                                clickable(
                                    el("div", text("Claims"))
                                        .attr("class", "related-claims")
                                        .attr("data-target", dismiss_id.clone())
                                        .attr(
                                            "aria-label",
                                            format!("Explain relation to {}", item.title),
                                        ),
                                    move |ui: &mut UiState, _| {
                                        if let Some(from) = ui.stage.catalog_id() {
                                            let to = ui.stage.related_target_id(target);
                                            ui.personal_link_pair = Some((from, to));
                                            ui.personal_link_history = false;
                                            ui.personal_link_layout = None;
                                            ui.personal_relation_notice = None;
                                        }
                                    },
                                ),
                                clickable(
                                    el("div", text("×")).attr("class", "related-hide"),
                                    move |ui: &mut UiState, _| {
                                        let related = &mut ui.app_settings.stage.related;
                                        if !related.dismissed_ids.contains(&dismiss_id) {
                                            related.dismissed_ids.push(dismiss_id.clone());
                                        }
                                    },
                                ),
                            ),
                        )
                        .attr("class", "related-actions"),
                    ),
                )
                .attr("class", row_class),
                move |ui: &mut UiState, ev: HoverEvent| match ev.phase {
                    HoverPhase::Enter | HoverPhase::Move => ui.related_hover = Some(target),
                    HoverPhase::Leave => {
                        if ui.related_hover == Some(target) {
                            ui.related_hover = None;
                        }
                    },
                },
            )) as UiChild
        })
        .collect();

    let pane: UiChild = if rows.is_empty() {
        Box::new(
            el(
                "div",
                text("Choose material with catalog relations to see suggestions."),
            )
            .attr("class", "related-empty"),
        )
    } else {
        Box::new(el("div", rows).attr("class", "related-list"))
    };

    // The interactive graph swatch: click a node to navigate, hover to link it to
    // its row, Expand to grow the canvas. Node click/hover route through the same
    // dispatch the pane uses, so the two stay in sync.
    let graph: UiChild = if ui.workspace.active_panel()
        == Some(crate::workspace::WorkspacePanel::Related)
        && ui.app_settings.stage.related.show_neighborhood
    {
        let swatch = related_swatch(ui);
        Box::new(
            el(
                "div",
                (
                    graph_canvas_swatch_with_focus_and_drag_and_relations(
                        &swatch,
                        |ui: &mut UiState, id: String| {
                            ui.stage.select_catalog_id(&id);
                        },
                        |ui: &mut UiState, id: Option<String>| {
                            ui.related_hover = id
                                .as_deref()
                                .and_then(|id| ui.stage.related_target_for_id(id));
                        },
                        |_ui: &mut UiState, _focus: Option<String>| {},
                        |_ui: &mut UiState, _drag| {},
                        activate_related_relation,
                        |_ui: &mut UiState, _relation| {},
                        |ui: &mut UiState| {
                            ui.related_expanded = !ui.related_expanded;
                        },
                        |_ui: &mut UiState, _event: cambium::PointerEvent| {},
                        |ui: &mut UiState, event: cambium::WheelEvent| {
                            super::canvas_camera::apply_wheel(
                                ui,
                                super::canvas_camera::CanvasSurface::Related,
                                &event,
                            );
                        },
                    ),
                    el(
                        "div",
                        super::canvas_camera::camera_controls(
                            super::canvas_camera::CanvasSurface::Related,
                            "related-camera",
                        ),
                    )
                    .attr("class", "related-camera-controls"),
                ),
            )
            .attr("class", "related-graph-col"),
        )
    } else {
        Box::new(el("div", ()))
    };

    let recent: Vec<UiChild> = ui
        .practice_history
        .recent(4)
        .into_iter()
        .map(|event| {
            let title = event
                .subject_id
                .split_once(':')
                .map(|(_, title)| title)
                .unwrap_or(event.subject_id.as_str());
            Box::new(
                el(
                    "div",
                    (
                        el("span", text(event.kind.label())).attr("class", "history-kind"),
                        el("span", text(title.to_string())).attr("class", "history-title"),
                    ),
                )
                .attr("class", "history-item"),
            ) as UiChild
        })
        .collect();
    let history: UiChild = if recent.is_empty() {
        Box::new(el("div", ()).attr("class", "history-empty"))
    } else {
        Box::new(
            el(
                "div",
                (
                    el("div", text("Recent")).attr("class", "history-heading"),
                    el("div", recent).attr("class", "history-list"),
                ),
            )
            .attr("class", "related-history"),
        )
    };

    Box::new(
        el(
            "aside",
            (
                el("div", text("Related")).attr("class", "related-heading"),
                el("div", text("What might I stage next?")).attr("class", "related-subtitle"),
                el(
                    "div",
                    (
                        graph,
                        el("div", (pane, opened_claims_link(ui))).attr("class", "related-pane-col"),
                    ),
                )
                .attr("class", "related-body"),
                history,
            ),
        )
        .attr("class", "related-panel"),
    )
}

#[cfg(test)]
#[path = "related/tests.rs"]
mod tests;

#[path = "related/claims.rs"]
mod claims;

#[cfg(test)]
use claims::{claims_link_reading, generated_evidence_token};
use claims::{claims_source, sealed_actor};

pub(super) use claims::{claims_link_reading_for_pair, opened_claims_link};
