use cambium::{
    HoverEvent, HoverPhase, OpenedLinkEvent, clickable, el,
    graph_canvas_swatch_with_focus_and_drag_and_relations, map_state, on_hover, text, text_field,
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

use super::{PersonalRelationIntent, RELATED_LIMIT, UiChild, UiState, related_swatch};

fn claims_source(ui: &UiState) -> LinkSource {
    let personal = ui.personal_relations.as_ref().map_or_else(
        || "unavailable".to_owned(),
        |source| source.source_id().to_string(),
    );
    LinkSource::Available {
        binding: scenograph::SourceBinding {
            authority: "Woodshed".into(),
            domain: "Woodshed relation-claims composite; generated Catalog/Computed/Evidence are current, and the session personal journal is identified separately when present".into(),
            resource: format!("woodshed-claims:generated-current;personal-source:{personal}"),
        },
        revision: LinkSourceRevision::Unavailable {
            reason: "the composite reading has no shared immutable public revision".into(),
        },
    }
}

fn claims_link_reading(ui: &UiState) -> Option<LinkPairReading> {
    let (from, to) = ui.personal_link_pair.as_ref()?;
    let key = LinkKey::new(LinkStratum::Surface, from.clone(), to.clone());
    let mut grouped = std::collections::BTreeMap::<(String, String), Vec<LinkAssertion>>::new();

    // Generated reasons remain source-owned read-only claims. Their stable
    // IDs and directed endpoints come directly from the current mere snapshot;
    // never convert them into Chartulary EdgeIds or infer write authority.
    let generated = super::related_mere_snapshot(ui);
    let mut generated_by_id: std::collections::BTreeMap<
        woodshed_core::mere::GeneratedClaimId,
        &woodshed_core::mere::MaterialRelation,
    > = std::collections::BTreeMap::new();
    for relation in generated.relations.iter().filter(|relation| {
        LinkKey::new(
            LinkStratum::Surface,
            relation.source.clone(),
            relation.target.clone(),
        ) == key
    }) {
        if let Some(previous) = generated_by_id.get(&relation.claim_id) {
            if previous.provenance != relation.provenance {
                // Conflicting source metadata for one stable claim identity is
                // ambiguous; refuse to expose an arbitrary inverse alias.
                return None;
            }
            continue;
        }
        generated_by_id.insert(relation.claim_id.clone(), relation);
    }
    for relation in generated_by_id.values() {
        let provenance = &relation.provenance;
        let owner = relation.claim_id.owner.as_str();
        let evidence = provenance
            .evidence
            .iter()
            .map(generated_evidence_token)
            .collect();
        let assertion = LinkAssertion {
            id: format!("generated:{}", relation.claim_id.qualified_key()),
            predicate: provenance.original_kind.predicate_iri(),
            recognized_kind: Some(provenance.original_kind.label().to_owned()),
            label: Some(provenance.original_explanation.clone()),
            scope: LinkGraphScope::Custom(format!("urn:woodshed:generated-claim:{owner}")),
            asserter: None,
            first_asserted_ms: None,
            asserted_at_ms: None,
            withdrawn: None,
            basis: LinkAssertionBasis {
                note: None,
                method: Some(LinkAssertionMethod {
                    iri: provenance.method.iri.to_owned(),
                    version: Some(provenance.method.version.to_string()),
                }),
                evidence,
            },
        };
        grouped
            .entry((
                provenance.original_from.clone(),
                provenance.original_to.clone(),
            ))
            .or_default()
            .push(assertion);
    }

    if let Some(source) = &ui.personal_relations {
        let records = if ui.personal_link_history {
            source.retained_assertions().collect::<Vec<_>>()
        } else {
            source.held_assertions().collect::<Vec<_>>()
        };
        for record in records {
            if LinkKey::new(LinkStratum::Surface, record.from.clone(), record.to.clone()) != key {
                continue;
            }
            let id = serde_json::to_string(&record.id).ok()?;
            let predicate = match &record.edge.class {
                RelationClass::App { family, kind }
                    if family == "woodshed" && *kind == woodshed_core::mere::RELATED =>
                {
                    woodshed_core::mere::RELATED_PREDICATE_IRI.to_owned()
                },
                RelationClass::App { family, kind } => format!("app:{family}:{kind}"),
                RelationClass::Semantic(_) => {
                    record.edge.class.predicate().unwrap_or("").to_owned()
                },
            };
            let assertion = LinkAssertion {
                id: format!("personal:{id}"),
                predicate,
                recognized_kind: None,
                label: record.edge.label.clone(),
                scope: LinkGraphScope::Custom(source.source_id().to_string()),
                asserter: record
                    .asserter
                    .as_ref()
                    .map(|actor| actor.as_str().to_owned()),
                first_asserted_ms: record.span.first_asserted_ms,
                asserted_at_ms: record.span.last_asserted_ms,
                withdrawn: record
                    .span
                    .withdrawn
                    .as_ref()
                    .map(|withdrawn| LinkWithdrawal {
                        at_ms: withdrawn.at_ms,
                        by: Some(withdrawn.by.as_str().to_owned()),
                    }),
                basis: LinkAssertionBasis {
                    note: record.basis.note.clone(),
                    method: record
                        .basis
                        .method
                        .as_ref()
                        .map(|method| LinkAssertionMethod {
                            iri: method.iri.clone(),
                            version: method.version.clone(),
                        }),
                    evidence: record.basis.evidence.clone(),
                },
            };
            grouped
                .entry((record.from.clone(), record.to.clone()))
                .or_default()
                .push(assertion);
        }
    }
    for assertions in grouped.values_mut() {
        assertions.sort_by(|left, right| left.id.cmp(&right.id));
    }
    let buckets = grouped
        .into_iter()
        .map(|((source_from, source_to), assertions)| LinkBucket {
            displayed_from: source_from.clone(),
            displayed_to: source_to.clone(),
            source_from: LinkEndpoint {
                id: source_from.clone(),
                address: Some(source_from),
            },
            source_to: LinkEndpoint {
                id: source_to.clone(),
                address: Some(source_to),
            },
            assertions,
            legacy_members: Vec::new(),
            legacy_summaries: Vec::new(),
        })
        .collect();
    let source = claims_source(ui);
    Some(LinkPairReading {
        source,
        links: vec![LinkReading { key, buckets }],
        coverage: LinkCoverage::default(),
    })
}

fn generated_evidence_token(evidence: &GeneratedEvidenceRef) -> String {
    let components = match evidence {
        GeneratedEvidenceRef::CatalogRelation { from, to, kind } => vec![
            "catalog-relation".to_owned(),
            from.clone(),
            to.clone(),
            kind.predicate_iri(),
        ],
        GeneratedEvidenceRef::CatalogMaterial { id } => {
            vec!["catalog-material".to_owned(), id.clone()]
        },
        GeneratedEvidenceRef::ProgressionAdjacency {
            progression_id,
            left_role_ordinal,
            right_role_ordinal,
        } => vec![
            "progression-adjacency".to_owned(),
            progression_id.clone(),
            left_role_ordinal.to_string(),
            right_role_ordinal.to_string(),
        ],
        GeneratedEvidenceRef::PracticeTransitionAggregate {
            from,
            to,
            basis,
            count,
        } => vec![
            "practice-transition-aggregate".to_owned(),
            from.clone(),
            to.clone(),
            match basis {
                PracticeEvidenceBasis::WalkedPath => "walked-path",
                PracticeEvidenceBasis::StatedFromReason => "stated-from-reason",
                PracticeEvidenceBasis::Unspecified => "unspecified",
            }
            .to_owned(),
            (*count).map_or_else(|| "unknown".to_owned(), |count| count.to_string()),
        ],
    };
    let body = components
        .iter()
        .map(|part| format!("{}:{part}", part.len()))
        .collect::<String>();
    format!("urn:woodshed:evidence:v1:{body}")
}

fn sealed_actor(ui: &UiState) -> Option<Author> {
    match ui.seal.as_ref() {
        Some(crate::persona::PracticeSeal::Sealed { persona, .. })
            if ui.practice_saved && !persona.trim().is_empty() && persona == persona.trim() =>
        {
            Some(Author::new(persona.clone()))
        },
        _ => None,
    }
}

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
                let snapshot = super::related_mere_snapshot(ui);
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
                        ui.request(crate::audio::AudioRequest::PreviewPhrase {
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
                    ui.request(crate::audio::AudioRequest::PreviewPhrase {
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
                            expected_source_id: source.source_id().clone(),
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
                            expected_source_id: source.source_id().clone(),
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
    let mut stage = ui.stage.clone();
    if !stage.select_catalog_id(catalog_id) {
        return None;
    }
    let card = stage.card_from_lens()?;
    let (pitches, duration, strum) = stage.card_sounding_pitches_at_tempo(&card, ui.transport.bpm);
    (!pitches.is_empty()).then_some((pitches, duration, strum))
}

pub(super) fn opened_claims_link(ui: &UiState) -> UiChild {
    let Some(reading) = claims_link_reading(ui) else {
        return Box::new(el("div", ()));
    };
    let Some((from, to)) = ui.personal_link_pair.as_ref() else {
        return Box::new(el("div", ()));
    };
    let source_revision = ui
        .personal_relations
        .as_ref()
        .map_or(0, |source| source.runtime_revision());
    let actor = sealed_actor(ui);
    let actor_name = actor.as_ref().map(|actor| actor.as_str().to_owned());
    let mut children: Vec<UiChild> = vec![
        Box::new(el("h3", "Claims")),
        Box::new(el(
            "p",
            "Generated catalog, computed, and practice-evidence reasons are read-only. Personal reasons are authored separately under the sealed persona.",
        )),
        Box::new(el("p", format!("{} → {}", from, to))),
        Box::new(
            clickable(
                el(
                    "button",
                    if ui.personal_link_history {
                        "Show held claims"
                    } else {
                        "Show retained history"
                    },
                ),
                |ui: &mut UiState, _| ui.personal_link_history = !ui.personal_link_history,
            )
            .attr("class", "personal-link-history-toggle"),
        ),
    ];
    let can_write = ui.practice_saved && actor.is_some() && ui.personal_relations.is_some();
    if can_write {
        children.push(Box::new(map_state(
            text_field(&ui.personal_relation_note),
            |ui: &mut UiState| &mut ui.personal_relation_note,
        )));
        let from_for_assert = from.clone();
        let to_for_assert = to.clone();
        children.push(Box::new(clickable(
            el("button", "Add personal reason").attr("class", "personal-add"),
            move |ui: &mut UiState, _| {
                if !ui.practice_saved || sealed_actor(ui).is_none() {
                    ui.personal_relation_notice = Some(
                        "Choose a sealed persona before creating a personal assertion.".into(),
                    );
                    return;
                }
                let Some(source) = ui.personal_relations.as_ref() else {
                    ui.personal_relation_notice =
                        Some("The personal assertion source has not been saved yet.".into());
                    return;
                };
                let expected_revision = source.runtime_revision();
                ui.refresh_event_time();
                let note = ui.personal_relation_note.text().trim().to_owned();
                ui.personal_relation_intents
                    .push(PersonalRelationIntent::Assert {
                        expected_source_id: source.source_id().clone(),
                        expected_revision,
                        from: from_for_assert.clone(),
                        to: to_for_assert.clone(),
                        note: (!note.is_empty()).then_some(note),
                        at_ms: ui.now_ms,
                    });
                ui.personal_relation_note = cambium::TextInput::new(String::new());
                ui.personal_relation_notice = Some("Saving personal assertion…".into());
            },
        )));
    } else {
        let reason = if !ui.practice_saved {
            "Personal writing is available only in a saved session."
        } else if actor.is_none() {
            "Personal writing requires a non-empty sealed persona."
        } else {
            "The personal assertion source is unavailable or still loading; generated reasons remain readable."
        };
        children.push(Box::new(el("p", reason)));
    }
    if let Some(notice) = &ui.personal_relation_notice {
        children.push(Box::new(el("p", notice.clone()).attr("role", "status")));
    }

    let key = LinkKey::new(LinkStratum::Surface, from.clone(), to.clone());
    let swatch = Swatch {
        id: format!(
            "woodshed-claims-link-{}",
            ui.personal_relations
                .as_ref()
                .map_or("unavailable", |source| source.source_id().as_str())
        ),
        scope: Scope::Link(key),
        mode: SwatchMode::Projection {
            definition_id: "opened-link".into(),
            variant_id: None,
        },
    };
    let layout = ui
        .personal_link_layout
        .clone()
        .unwrap_or_else(|| OpenedLinkLayout::default_for(&reading));
    let history = ui.personal_link_history;
    children.push(Box::new(cambium::opened_link_swatch(
        &swatch,
        &reading,
        &layout,
        scenograph::link::OpenedLinkOptions { now_ms: ui.now_ms },
        history,
        move |_source, _key, bucket, assertion| {
            let mut actions = Vec::new();
            if assertion.withdrawn.is_none() {
                actions.push(cambium::LinkMemberAction {
                    id: "audition".into(),
                    label: format!(
                        "Audition {} → {}",
                        bucket.displayed_from, bucket.displayed_to
                    ),
                });
                if actor_name
                    .as_deref()
                    .is_some_and(|actor| assertion.asserter.as_deref() == Some(actor))
                {
                    actions.push(cambium::LinkMemberAction {
                        id: "confirm".into(),
                        label: "Confirm this assertion".into(),
                    });
                    actions.push(cambium::LinkMemberAction {
                        id: "withdraw".into(),
                        label: "Withdraw this assertion".into(),
                    });
                }
            }
            actions
        },
        move |ui: &mut UiState, event| queue_relation_action(ui, event, source_revision),
    )));
    Box::new(el("section", children).attr("class", "claims-link-inspection"))
}

fn activate_related_relation(ui: &mut UiState, relation_id: String) {
    ui.related_relation = (ui.related_relation.as_deref() != Some(relation_id.as_str()))
        .then_some(relation_id.clone());
    if ui.related_relation.as_deref() != Some(relation_id.as_str()) {
        return;
    }
    if let Some(claim) = related_mere_snapshot(ui)
        .relations
        .into_iter()
        .find(|claim| claim.claim_id.qualified_key() == relation_id)
    {
        ui.personal_link_pair =
            Some((claim.provenance.original_from, claim.provenance.original_to));
        ui.personal_link_history = false;
        ui.personal_link_layout = None;
        ui.personal_relation_notice = None;
    }
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
                                    el("div", text("Stage")).attr("class", "related-stage"),
                                    move |ui: &mut UiState, _| {
                                        let from_id = ui.stage.catalog_id();
                                        ui.stage.select_related(target);
                                        ui.stage_current(from_id);
                                    },
                                ),
                                clickable(
                                    el("div", text("Claims")).attr("class", "related-claims"),
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
mod tests {
    use super::*;
    use woodshed_core::personal_relations::{
        AssertionBasis, PersonalRelationSource, PersonalRelationSourceId,
    };

    #[test]
    fn exact_assertion_audition_queues_frozen_before_then_after_without_touching_set() {
        let from = "chord:Major".to_string();
        let to = "chord:Minor".to_string();
        let actor = Author::new("personae:alice");
        let mut source = PersonalRelationSource::empty(
            PersonalRelationSourceId::from_random_uuid_bytes([5; 16]),
        );
        let id = source
            .assert_relation(
                &actor,
                0,
                &from,
                &to,
                Relation::new(RelationClass::app("woodshed", 3)).with_label("related"),
                Some(100),
                AssertionBasis::default(),
            )
            .unwrap();
        let revision = source.runtime_revision();
        let mut ui = UiState::new();
        ui.personal_relations = Some(source);
        ui.personal_link_pair = Some((from.clone(), to.clone()));
        ui.seal = Some(crate::persona::PracticeSeal::Sealed {
            persona: actor.as_str().to_string(),
            protection: "test".into(),
        });
        let set_before = serde_json::to_value(&ui.set).unwrap();
        let stage_material_before = ui.stage.catalog_id();
        let reading = claims_link_reading(&ui).unwrap();
        let expected_source = reading.source.clone();
        let key = LinkKey::new(LinkStratum::Surface, from.clone(), to.clone());
        let target = cambium::LinkMemberTarget {
            source: expected_source,
            link: key,
            from: from.clone(),
            to: to.clone(),
            assertion_id: format!("personal:{}", serde_json::to_string(&id).unwrap()),
        };

        queue_relation_action(
            &mut ui,
            OpenedLinkEvent::Member {
                target,
                action: "audition".into(),
            },
            revision,
        );

        let before_expected = endpoint_preview(&ui, &from).unwrap().0;
        let after_expected = endpoint_preview(&ui, &to).unwrap().0;
        assert!(matches!(
            ui.audio_requests.as_slice(),
            [woodshed_core::audio::AudioRequest::PreviewPhrase { before, after, .. }]
                if before == &before_expected && after == &after_expected
        ));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), set_before);
        assert_eq!(ui.stage.catalog_id(), stage_material_before);
        assert!(ui.personal_relation_intents.is_empty());
    }

    #[test]
    fn claims_reading_keeps_generated_and_personal_reasons_with_distinct_owners() {
        let mut ui = UiState::new();
        ui.app_settings.stage.related.graph_scope =
            woodshed_core::settings::RelatedGraphScope::Mere;
        let generated = related_mere_snapshot(&ui).relations[0].clone();
        let from = generated.provenance.original_from.clone();
        let to = generated.provenance.original_to.clone();
        let actor = Author::new("personae:alice");
        let mut source = PersonalRelationSource::empty(
            PersonalRelationSourceId::from_random_uuid_bytes([9; 16]),
        );
        let personal_id = source
            .assert_relation(
                &actor,
                0,
                &from,
                &to,
                Relation::new(RelationClass::app("woodshed", 3)).with_label("related"),
                Some(10),
                AssertionBasis {
                    note: Some("personal interpretation".into()),
                    ..AssertionBasis::default()
                },
            )
            .unwrap();
        ui.personal_relations = Some(source);
        ui.personal_link_pair = Some((from.clone(), to.clone()));

        let reading = claims_link_reading(&ui).unwrap();
        let assertions = reading
            .links
            .iter()
            .flat_map(|link| link.buckets.iter())
            .flat_map(|bucket| bucket.assertions.iter())
            .collect::<Vec<_>>();
        let generated_claim = assertions
            .iter()
            .find(|assertion| {
                assertion.id == format!("generated:{}", generated.claim_id.qualified_key())
            })
            .expect("generated reason is part of the pair reading");
        assert_eq!(
            generated_claim.scope,
            LinkGraphScope::Custom(format!(
                "urn:woodshed:generated-claim:{}",
                generated.claim_id.owner.as_str()
            ))
        );
        assert_eq!(generated_claim.asserter, None);
        assert_eq!(generated_claim.asserted_at_ms, None);
        assert_eq!(generated_claim.withdrawn, None);
        assert_eq!(
            generated_claim.predicate,
            generated.provenance.original_kind.predicate_iri()
        );
        assert_eq!(
            generated_claim.label.as_deref(),
            Some(generated.provenance.original_explanation.as_str())
        );
        assert_eq!(
            generated_claim
                .basis
                .method
                .as_ref()
                .map(|method| method.iri.as_str()),
            Some(generated.provenance.method.iri)
        );
        assert_eq!(
            generated_claim.basis.evidence,
            generated
                .provenance
                .evidence
                .iter()
                .map(generated_evidence_token)
                .collect::<Vec<_>>()
        );

        let personal = assertions
            .iter()
            .find(|assertion| {
                assertion.id == format!("personal:{}", serde_json::to_string(&personal_id).unwrap())
            })
            .expect("personal reason remains independently addressable");
        assert_eq!(
            personal.basis.note.as_deref(),
            Some("personal interpretation")
        );
        assert_eq!(personal.asserter.as_deref(), Some("personae:alice"));
        assert_eq!(
            personal.predicate,
            woodshed_core::mere::RELATED_PREDICATE_IRI
        );
        assert_ne!(generated_claim.id, personal.id);
    }

    #[test]
    fn claims_source_discloses_personal_owner_unavailable_while_generated_reading_remains_current()
    {
        let mut ui = UiState::new();
        ui.personal_link_pair = Some(("chord:Major".into(), "chord:Major 7".into()));

        let reading = claims_link_reading(&ui).unwrap();
        let LinkSource::Available { binding, revision } = &reading.source else {
            panic!("the generated catalog projection is still available");
        };
        assert!(binding.resource.contains("personal-source:unavailable"));
        assert!(
            binding
                .domain
                .contains("identified separately when present")
        );
        assert!(matches!(
            revision,
            LinkSourceRevision::Unavailable { reason }
                if reason.contains("no shared immutable public revision")
        ));
    }

    #[test]
    fn generated_inverse_alias_auditions_original_assertion_direction() {
        let mut ui = UiState::new();
        ui.app_settings.stage.related.graph_scope =
            woodshed_core::settings::RelatedGraphScope::Mere;
        let snapshot = related_mere_snapshot(&ui);
        let generated = snapshot
            .relations
            .iter()
            .find(|relation| {
                snapshot.relations.iter().any(|candidate| {
                    candidate.claim_id == relation.claim_id
                        && candidate.source == relation.target
                        && candidate.target == relation.source
                })
            })
            .unwrap()
            .clone();
        let original_from = generated.provenance.original_from.clone();
        let original_to = generated.provenance.original_to.clone();
        let reverse_display = snapshot
            .relations
            .iter()
            .find(|candidate| {
                candidate.claim_id == generated.claim_id
                    && candidate.source == original_to
                    && candidate.target == original_from
            })
            .unwrap();
        assert_ne!(reverse_display.source, original_from);
        ui.personal_link_pair = Some((
            reverse_display.source.clone(),
            reverse_display.target.clone(),
        ));
        let reading = claims_link_reading(&ui).unwrap();
        let target = cambium::LinkMemberTarget {
            source: reading.source,
            link: LinkKey::new(
                LinkStratum::Surface,
                reverse_display.source.clone(),
                reverse_display.target.clone(),
            ),
            from: original_from.clone(),
            to: original_to.clone(),
            assertion_id: format!("generated:{}", generated.claim_id.qualified_key()),
        };

        queue_relation_action(
            &mut ui,
            OpenedLinkEvent::Member {
                target,
                action: "audition".into(),
            },
            0,
        );

        let before = endpoint_preview(&ui, &original_from).unwrap().0;
        let after = endpoint_preview(&ui, &original_to).unwrap().0;
        assert!(matches!(
            ui.audio_requests.as_slice(),
            [woodshed_core::audio::AudioRequest::PreviewPhrase { before: queued_before, after: queued_after, .. }]
                if queued_before == &before && queued_after == &after
        ));
        assert!(ui.personal_relation_intents.is_empty());
    }

    #[test]
    fn selecting_related_inverse_claim_opens_original_directed_pair() {
        let mut ui = UiState::new();
        ui.app_settings.stage.related.graph_scope =
            woodshed_core::settings::RelatedGraphScope::Mere;
        let snapshot = related_mere_snapshot(&ui);
        let inverse = snapshot
            .relations
            .iter()
            .find(|relation| {
                relation.source != relation.provenance.original_from
                    && relation.target == relation.provenance.original_from
                    && snapshot.relations.iter().any(|candidate| {
                        candidate.claim_id == relation.claim_id
                            && candidate.source == relation.provenance.original_from
                            && candidate.target == relation.provenance.original_to
                    })
            })
            .expect("the related catalog exposes an inverse display alias")
            .clone();
        let expected = (
            inverse.provenance.original_from.clone(),
            inverse.provenance.original_to.clone(),
        );
        let set_before = serde_json::to_value(&ui.set).unwrap();

        activate_related_relation(&mut ui, inverse.claim_id.qualified_key());

        assert_eq!(ui.personal_link_pair, Some(expected));
        assert_eq!(serde_json::to_value(&ui.set).unwrap(), set_before);
    }
}
