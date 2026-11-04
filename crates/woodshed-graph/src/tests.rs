use super::catalog::relation;
use super::*;
use crate::relations::RelationBuilder;

#[test]
fn the_graph_holds_the_whole_catalog() {
    let graph = build_catalog_graph("woodshed:catalog");
    let expected = scale::catalog().len()
        + chord::catalog().len() * 2
        + progression::catalog().len()
        + exercise::catalog().len();
    assert_eq!(graph.graph().node_count(), expected);
    // A well-known object is present, found by its stable id.
    assert!(graph.graph().get(&chord_id("Major 7")).is_some());
    assert!(graph.graph().get(&arpeggio_id("Major 7")).is_some());
    assert!(graph.graph().get(&scale_id("Major")).is_some());
}

#[test]
fn a_progression_contains_its_chords() {
    let graph = build_catalog_graph("woodshed:catalog");
    let g = graph.graph();
    // ii-V-I calls for a minor 7, a dominant 7, and a major 7.
    let prog = g
        .key_of(&progression_id("ii-V-I (Jazz)"))
        .expect("the ii-V-I progression is in the catalog");
    let contains = RelationClass::app(FAMILY, CONTAINS);
    let chords: BTreeSet<String> = g
        .out_edges_of_class(prog, &contains)
        .map(|(_, target)| g.node(target).unwrap().id.clone())
        .collect();
    assert!(chords.contains(&chord_id("Major 7")), "I is a major 7");
    assert!(
        chords.contains(&chord_id("Dominant 7")),
        "V is a dominant 7"
    );
    assert!(chords.contains(&chord_id("Minor 7")), "ii is a minor 7");
}

#[test]
fn a_major_seventh_fits_the_major_scale() {
    let graph = build_catalog_graph("woodshed:catalog");
    let g = graph.graph();
    let cmaj7 = g.key_of(&chord_id("Major 7")).unwrap();
    let fits = RelationClass::app(FAMILY, FITS_IN_SCALE);
    let scales: BTreeSet<String> = g
        .out_edges_of_class(cmaj7, &fits)
        .map(|(_, target)| g.node(target).unwrap().id.clone())
        .collect();
    // 1-3-5-7 sits inside the major scale.
    assert!(
        scales.contains(&scale_id("Major")),
        "Major 7 fits the Major scale"
    );
}

#[test]
fn tags_partition_the_catalog_by_kind() {
    let graph = build_catalog_graph("woodshed:catalog");
    let g = graph.graph();
    assert_eq!(g.nodes_tagged("chord").count(), chord::catalog().len());
    assert_eq!(g.nodes_tagged("scale").count(), scale::catalog().len());
}

#[test]
fn a_user_forks_the_catalog_to_add_a_personal_chord() {
    let catalog = build_catalog_graph("woodshed:catalog");
    let before = catalog.graph().node_count();

    // A user forks the shared catalog into their own graph and adds a chord.
    let mut mine = catalog.fork(LogId::new("mark:private"));
    let author = Author::new("mark");
    mine.insert_node(
        &author,
        Container::new(chord_id("Mark's Cluster"))
            .with_title("Mark's Cluster")
            .with_tag("chord"),
    );
    mine.connect(
        &author,
        &chord_id("Mark's Cluster"),
        &scale_id("Chromatic"),
        relation(FITS_IN_SCALE),
    );

    // The fork has the addition and knows it descends from the catalog.
    assert!(mine.graph().get(&chord_id("Mark's Cluster")).is_some());
    let provenance = mine.provenance().expect("a fork carries provenance");
    assert_eq!(provenance.source, Some(LogId::new("woodshed:catalog")));

    // The shared catalog is untouched.
    assert_eq!(catalog.graph().node_count(), before);
    assert!(catalog.graph().get(&chord_id("Mark's Cluster")).is_none());
}

#[test]
fn the_catalog_projects_names_but_keeps_private_relations_private() {
    let graph = build_catalog_graph("woodshed:catalog");
    let quads = chartulary::rdf::to_quads(graph.graph());

    // woodshed's relations (Contains, FitsInScale) are its private family, so
    // every projected triple is a curated literal; not one music relation
    // leaks into RDF.
    assert!(
        quads
            .iter()
            .all(|q| q.predicate == chartulary::rdf::SCHEMA_NAME
                || q.predicate == chartulary::rdf::SCHEMA_KEYWORDS),
        "only schema.org literals project; the app ring stays private"
    );
    // A known chord's title reaches RDF as schema:name.
    assert!(quads.iter().any(|q| q.predicate == chartulary::rdf::SCHEMA_NAME
        && matches!(&q.object, chartulary::rdf::Term::Literal { value, .. } if value == "Major 7")));
}

#[test]
fn a_practice_session_records_lineage_over_the_same_node_ids() {
    use chartulary::stemma::{EntryPrivacy, Stemma, TransitionKind};

    // The lineage is keyed by the SAME identities as the chartulary graph, so
    // the two layers integrate by shared id, not by coupling. The consumer
    // decides what a "visit" is (here, a practice engagement) and records it;
    // the graph spine does not auto-feed stemma.
    let mut lineage: Stemma<String, String, String, ()> = Stemma::new();
    let session = lineage.ensure_owner("practice:2026-07-08".to_string(), None);

    let drilled = [
        (chord_id("Major 7"), "Major 7"),
        (scale_id("Major"), "Major"),
        (chord_id("Minor 7"), "Minor 7"),
    ];
    for (at, (id, title)) in drilled.iter().enumerate() {
        let entry = lineage.resolve_or_create_entry(
            id.clone(),
            title.to_string(),
            at as u64,
            EntryPrivacy::LocalOnly,
        );
        lineage
            .visit_entry(session, entry, (), TransitionKind::LinkClick, at as u64)
            .expect("the session and entry exist");
    }

    assert_eq!(lineage.visit_count(), 3, "three practice engagements");
    assert_eq!(lineage.entry_count(), 3);
    // The lineage entry is the graph node, by identity.
    assert!(lineage.entry_id_by_key(&chord_id("Major 7")).is_some());
}

#[test]
fn a_neighbor_explains_both_directions() {
    let chord = related_neighbors(&chord_id("Major 7"), 64);
    let scale = chord
        .iter()
        .find(|n| n.id == scale_id("Major"))
        .expect("the chord fits the major scale");
    assert!(scale.kinds().contains(&RelationKind::FitsInScale));

    let progression = chord
        .iter()
        .find(|n| n.id == progression_id("ii-V-I (Jazz)"))
        .expect("the chord appears in the progression");
    assert!(progression.kinds().contains(&RelationKind::AppearsIn));

    // The same fact from the scale's end names the inverse kind.
    let from_scale = related_neighbors(&scale_id("Major"), 64);
    let back = from_scale
        .iter()
        .find(|n| n.id == chord_id("Major 7"))
        .expect("the scale admits the chord");
    assert!(back.kinds().contains(&RelationKind::ScaleAdmits));
}

#[test]
fn chord_and_arpeggio_are_distinct_related_identities() {
    let chord = related_neighbors(&chord_id("Minor 7"), 64);
    let arpeggio = chord
        .iter()
        .find(|n| n.id == arpeggio_id("Minor 7"))
        .expect("chord offers its arpeggio realization");
    assert_eq!(arpeggio.kind, CatalogKind::Arpeggio);
    assert!(arpeggio.kinds().contains(&RelationKind::RealizedBy));

    let reverse = related_neighbors(&arpeggio_id("Minor 7"), 64);
    let source = reverse
        .iter()
        .find(|n| n.id == chord_id("Minor 7"))
        .expect("the arpeggio points back at its chord");
    assert!(source.kinds().contains(&RelationKind::Realizes));
}

#[test]
fn one_pair_keeps_every_applicable_relation() {
    // Major and Major 7 are related several ways at once. Ranking picks the
    // order; it must not pick a winner and drop the rest.
    let relations = relations_between(&chord_id("Major"), &chord_id("Major 7"));
    let kinds: Vec<RelationKind> = relations.iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RelationKind::ExtendedBy), "{kinds:?}");
    assert!(kinds.contains(&RelationKind::SharesTones), "{kinds:?}");
    assert!(kinds.contains(&RelationKind::VoiceLeadsTo), "{kinds:?}");
    assert!(
        relations.len() >= 3,
        "several relations survive ranking: {kinds:?}"
    );

    // Each carries its own measurement, rather than one blended score.
    let shared = relations
        .iter()
        .find(|r| r.kind == RelationKind::SharesTones)
        .unwrap();
    assert_eq!(shared.shared_tones, Some(3));
    let motion = relations
        .iter()
        .find(|r| r.kind == RelationKind::VoiceLeadsTo)
        .unwrap();
    assert!(motion.distance.is_some());
}

#[test]
fn generated_reason_identity_survives_inverse_display_and_keeps_independent_reasons() {
    let forward = relations_between(&chord_id("Major"), &chord_id("Major 7"));
    let reverse = relations_between(&chord_id("Major 7"), &chord_id("Major"));
    for relation in &forward {
        let inverse = reverse
            .iter()
            .find(|candidate| candidate.claim_id == relation.claim_id)
            .expect("the inverse display row preserves the original reason ID");
        assert_eq!(inverse.claim_id.owner, relation.claim_id.owner);
        if relation.authority == RelationAuthority::Computed {
            assert!(relation.claim_id.qualified_key().starts_with("computed:"));
        }
    }

    let from = chord_id("Major");
    let to = chord_id("Major 7");
    let mut builder = RelationBuilder::default();
    for progression in ["progression:ii-V-I", "progression:turnaround"] {
        builder.add(MaterialRelation::with_claim_components(
            &from,
            &to,
            RelationKind::UsedTogether,
            84,
            "Adjacent in the progression".to_owned(),
            &[progression, "0"],
            vec![GeneratedEvidenceRef::ProgressionAdjacency {
                progression_id: progression.to_owned(),
                left_role_ordinal: 0,
                right_role_ordinal: 1,
            }],
        ));
    }
    let names = HashMap::from([
        (from.clone(), "Major".to_owned()),
        (to.clone(), "Major 7".to_owned()),
    ]);
    let kinds = HashMap::from([
        (from.clone(), CatalogKind::Chord),
        (to.clone(), CatalogKind::Chord),
    ]);
    let index = builder.finish(&names, &kinds);
    let reasons = &index[&from][0].relations;
    assert_eq!(reasons.len(), 2, "independent progression reasons survive");
    assert_ne!(reasons[0].claim_id, reasons[1].claim_id);
    assert!(
        reasons
            .iter()
            .all(|reason| reason.kind == RelationKind::UsedTogether)
    );
    let inverse_reasons = &index[&to][0].relations;
    assert_eq!(inverse_reasons.len(), 2);
    let mut evidence_progressions = Vec::new();
    for reason in reasons {
        assert_eq!(reason.provenance.original_kind, RelationKind::UsedTogether);
        assert_eq!(
            reason.provenance.method, CATALOG_ADJACENCY_METHOD,
            "catalog adjacency names its source method"
        );
        let [
            GeneratedEvidenceRef::ProgressionAdjacency {
                progression_id,
                left_role_ordinal,
                right_role_ordinal,
            },
        ] = reason.provenance.evidence.as_slice()
        else {
            panic!("one exact progression adjacency backs each reason");
        };
        evidence_progressions.push(progression_id.clone());
        assert_eq!((*left_role_ordinal, *right_role_ordinal), (0, 1));
        assert!(inverse_reasons.iter().any(|inverse| {
            inverse.claim_id == reason.claim_id && inverse.provenance == reason.provenance
        }));
    }
    evidence_progressions.sort();
    assert_eq!(
        evidence_progressions,
        ["progression:ii-V-I", "progression:turnaround"]
    );
}

#[test]
fn generated_provenance_keeps_original_direction_and_typed_computation_inputs() {
    let forward = relations_between(&chord_id("Major"), &chord_id("Major 7"));
    let reverse = relations_between(&chord_id("Major 7"), &chord_id("Major"));
    let relation = forward
        .iter()
        .find(|relation| relation.kind == RelationKind::SharesTones)
        .unwrap();
    let inverse = reverse
        .iter()
        .find(|candidate| candidate.claim_id == relation.claim_id)
        .unwrap();

    assert_eq!(relation.provenance.original_from, chord_id("Major"));
    assert_eq!(relation.provenance.original_to, chord_id("Major 7"));
    assert_eq!(relation.provenance.original_kind, RelationKind::SharesTones);
    assert_eq!(
        relation.provenance.original_explanation,
        relation.explanation
    );
    assert_eq!(relation.provenance.method, COMPUTED_RELATION_METHOD);
    assert_eq!(
        relation.provenance.evidence,
        vec![
            GeneratedEvidenceRef::CatalogMaterial {
                id: chord_id("Major"),
            },
            GeneratedEvidenceRef::CatalogMaterial {
                id: chord_id("Major 7"),
            },
        ]
    );
    assert_eq!(inverse.provenance, relation.provenance);
    assert_eq!(inverse.source, relation.target);
    assert_eq!(inverse.target, relation.source);
}

#[test]
fn observed_transition_provenance_names_the_aggregate_without_inventing_event_ids() {
    let undated = MaterialRelation::evidence(
        "scale:Dorian",
        "chord:Minor 7",
        RelationKind::PracticedAfter,
        91,
        "Practiced next 1 times",
    );
    assert_eq!(undated.provenance.method, PRACTICE_TRANSITION_METHOD);
    assert_eq!(
        undated.provenance.evidence,
        vec![GeneratedEvidenceRef::PracticeTransitionAggregate {
            from: "scale:Dorian".into(),
            to: "chord:Minor 7".into(),
            basis: PracticeEvidenceBasis::Unspecified,
            count: None,
        }]
    );
    assert_eq!(
        undated.provenance.method.iri,
        "urn:woodshed:method:practice-relation-aggregate"
    );

    let counted = MaterialRelation::evidence_with_count(
        "scale:Dorian",
        "chord:Minor 7",
        RelationKind::PracticedAfter,
        91,
        "Practiced next once",
        PracticeEvidenceBasis::WalkedPath,
        1,
    );
    assert_eq!(
        counted.provenance.method.iri,
        "urn:woodshed:method:practice-path-transition-aggregate"
    );
    assert_eq!(
        counted.provenance.evidence,
        vec![GeneratedEvidenceRef::PracticeTransitionAggregate {
            from: "scale:Dorian".into(),
            to: "chord:Minor 7".into(),
            basis: PracticeEvidenceBasis::WalkedPath,
            count: Some(1),
        }]
    );
}

#[test]
fn relations_carry_the_authority_that_produced_them() {
    let contains = relations_between(&progression_id("ii-V-I (Jazz)"), &chord_id("Minor 7"))
        .into_iter()
        .find(|relation| relation.kind == RelationKind::ContainsMaterial)
        .expect("the progression's graph edge is retained as a catalog claim");
    assert_eq!(contains.provenance.method, CATALOG_RELATION_METHOD);
    assert_eq!(
        contains.provenance.evidence,
        vec![GeneratedEvidenceRef::CatalogRelation {
            from: progression_id("ii-V-I (Jazz)"),
            to: chord_id("Minor 7"),
            kind: RelationKind::ContainsMaterial,
        }]
    );

    // Authored by a progression...
    let together = relations_between(&chord_id("Minor 7"), &chord_id("Dominant 7"));
    let authored = together
        .iter()
        .find(|r| r.kind == RelationKind::UsedTogether)
        .expect("ii-V puts them next to each other");
    assert_eq!(authored.authority, RelationAuthority::Catalog);
    assert!(authored.explanation.starts_with("Adjacent in"));

    // ...versus derived from the formulas.
    let computed = relations_between(&chord_id("Major"), &chord_id("Major 7"));
    assert!(
        computed
            .iter()
            .filter(|r| r.kind != RelationKind::UsedTogether)
            .all(|r| r.authority == RelationAuthority::Computed)
    );
}

#[test]
fn relation_predicate_iris_are_stable_protocol_ids_not_display_labels() {
    assert_eq!(
        RelationKind::UsedTogether.predicate_iri(),
        "urn:woodshed:relation:used-together"
    );
    assert_eq!(
        RelationKind::PracticedAfter.predicate_iri(),
        "urn:woodshed:relation:practiced-after"
    );
    assert_ne!(
        RelationKind::PracticedAfter.predicate_iri(),
        RelationKind::PracticedAfter.label()
    );
    assert_eq!(
        RELATED_PREDICATE_IRI, "urn:woodshed:relation:related",
        "the personal app relation uses the same source-owned predicate namespace"
    );
    let mut predicates = RelationKind::DETERMINISTIC
        .into_iter()
        .chain([RelationKind::PracticedBefore, RelationKind::PracticedAfter])
        .map(RelationKind::predicate_iri)
        .collect::<Vec<_>>();
    predicates.sort();
    predicates.dedup();
    assert_eq!(predicates.len(), RelationKind::DETERMINISTIC.len() + 2);
}

#[test]
fn modes_of_one_scale_relate_as_modes() {
    let dorian = related_neighbors(&scale_id("Dorian"), 128);
    let major = dorian
        .iter()
        .find(|n| n.id == scale_id("Major"))
        .expect("Dorian is a rotation of the major scale");
    assert!(major.kinds().contains(&RelationKind::ModeOf));
    assert_eq!(major.primary().authority, RelationAuthority::Computed);
}

#[test]
fn ranking_prefers_multiplicity_without_merging_it() {
    let neighbors = related_neighbors(&chord_id("Major"), 128);
    let major_7 = neighbors
        .iter()
        .find(|n| n.id == chord_id("Major 7"))
        .expect("closely related chord");
    assert!(major_7.score > 60);
    assert!(major_7.relations.len() > 1, "multiplicity is retained");
    // The score reflects the strongest relation plus a bounded bonus, so it
    // can never exceed a scale of 100 or reorder by count alone.
    let best = major_7.relations.iter().map(|r| r.weight).max().unwrap();
    assert!(major_7.score >= best && major_7.score <= best.saturating_add(8));
    // Neighbors arrive strongest-first.
    assert!(neighbors.windows(2).all(|w| w[0].score >= w[1].score));
}

#[test]
fn deterministic_relations_need_no_history() {
    // Nothing in this crate reads practice history: every relation it
    // produces is Catalog or Computed. The Evidence kinds exist for the
    // consumer that owns the history to add to the same records.
    let neighbors = related_neighbors(&chord_id("Major 7"), 128);
    assert!(!neighbors.is_empty());
    assert!(neighbors.iter().flat_map(|n| &n.relations).all(|r| {
        r.authority == RelationAuthority::Catalog || r.authority == RelationAuthority::Computed
    }));
}
