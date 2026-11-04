use super::*;

pub(super) fn claims_source(ui: &UiState) -> LinkSource {
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

pub(super) fn claims_link_reading(ui: &UiState) -> Option<LinkPairReading> {
    let (from, to) = ui.personal_link_pair.as_ref()?;
    claims_link_reading_for_pair(ui, from, to)
}

pub(in super::super) fn claims_link_reading_for_pair(
    ui: &UiState,
    from: &str,
    to: &str,
) -> Option<LinkPairReading> {
    let key = LinkKey::new(LinkStratum::Surface, from, to);
    let mut grouped = std::collections::BTreeMap::<(String, String), Vec<LinkAssertion>>::new();

    // Generated reasons remain source-owned read-only claims. Their stable
    // IDs and directed endpoints come directly from the current mere snapshot;
    // never convert them into Chartulary EdgeIds or infer write authority.
    let generated = super::super::opened_claims_mere_snapshot(ui, from);
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

pub(super) fn generated_evidence_token(evidence: &GeneratedEvidenceRef) -> String {
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

pub(super) fn sealed_actor(ui: &UiState) -> Option<Author> {
    match ui.seal.as_ref() {
        Some(crate::persona::PracticeSeal::Sealed { persona, .. })
            if ui.practice_saved && !persona.trim().is_empty() && persona == persona.trim() =>
        {
            Some(Author::new(persona.clone()))
        },
        _ => None,
    }
}

pub(in super::super) fn opened_claims_link(ui: &UiState) -> UiChild {
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
        children.push(Box::new(
            el(
                "label",
                (
                    text("Personal reason note (optional)"),
                    map_state(
                        text_field_typed(&ui.personal_relation_note)
                            .attr("aria-label", "Personal reason note (optional)")
                            .attr("data-field", "personal-reason"),
                        |ui: &mut UiState| &mut ui.personal_relation_note,
                    ),
                ),
            )
            .attr("class", "claims-reason-field"),
        ));
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
                let Some((expected_revision, expected_source_id)) = ui
                    .personal_relations
                    .as_ref()
                    .map(|source| (source.runtime_revision(), source.source_id().clone()))
                else {
                    ui.personal_relation_notice =
                        Some("The personal assertion source has not been saved yet.".into());
                    return;
                };
                ui.refresh_event_time();
                let note = ui.personal_relation_note.text().trim().to_owned();
                ui.personal_relation_intents
                    .push(PersonalRelationIntent::Assert {
                        expected_source_id,
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
