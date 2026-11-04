use super::*;

/// Accumulates relation records per (source, target) pair before ranking, so a
/// pair related four ways arrives at ranking as four records, not one.
#[derive(Default)]
pub(super) struct RelationBuilder {
    pub(super) by_source: HashMap<String, HashMap<String, Vec<MaterialRelation>>>,
}

impl RelationBuilder {
    /// Record `relation` and its inverse, so both ends see the pair.
    pub(super) fn add(&mut self, relation: MaterialRelation) {
        let inverse = MaterialRelation {
            source: relation.target.clone(),
            target: relation.source.clone(),
            kind: relation.kind.inverse(),
            explanation: relation.explanation.clone(),
            ..relation.clone()
        };
        for record in [relation, inverse] {
            self.by_source
                .entry(record.source.clone())
                .or_default()
                .entry(record.target.clone())
                .or_default()
                .push(record);
        }
    }

    /// Record a relation whose two directions read differently.
    pub(super) fn add_directed(
        &mut self,
        source: &str,
        target: &str,
        kind: RelationKind,
        weight: u16,
        forward: String,
        reverse: String,
    ) {
        let forward_record = MaterialRelation::new(source, target, kind, weight, forward);
        let reverse_record = MaterialRelation {
            claim_id: forward_record.claim_id.clone(),
            provenance: forward_record.provenance.clone(),
            source: target.to_owned(),
            target: source.to_owned(),
            kind: kind.inverse(),
            weight,
            distance: None,
            shared_tones: None,
            explanation: reverse,
            authority: kind.authority(),
        };
        for record in [forward_record, reverse_record] {
            self.by_source
                .entry(record.source.clone())
                .or_default()
                .entry(record.target.clone())
                .or_default()
                .push(record);
        }
    }

    pub(super) fn finish(
        self,
        names: &HashMap<String, String>,
        kinds: &HashMap<String, CatalogKind>,
    ) -> HashMap<String, Vec<RelatedNeighbor>> {
        let mut index: HashMap<String, Vec<RelatedNeighbor>> = HashMap::new();
        for (source, targets) in self.by_source {
            let mut neighbors: Vec<RelatedNeighbor> = targets
                .into_iter()
                .filter_map(|(target, mut relations)| {
                    let kind = *kinds.get(&target)?;
                    relations.sort_by(|a, b| {
                        (std::cmp::Reverse(a.weight), a.kind, &a.claim_id).cmp(&(
                            std::cmp::Reverse(b.weight),
                            b.kind,
                            &b.claim_id,
                        ))
                    });
                    relations.dedup_by(|a, b| a.claim_id == b.claim_id);
                    let best = relations.first().map(|r| r.weight).unwrap_or_default();
                    // Multiplicity is evidence of relevance, so it lifts the
                    // rank a little. It never merges the relations: the bonus
                    // is capped precisely so it cannot become a reason to keep
                    // only one of them.
                    let bonus = ((relations.len() as u16).saturating_sub(1) * 2).min(8);
                    Some(RelatedNeighbor {
                        name: names
                            .get(&target)
                            .cloned()
                            .unwrap_or_else(|| target.clone()),
                        id: target,
                        kind,
                        score: best.saturating_add(bonus).min(100),
                        relations,
                    })
                })
                .collect();
            neighbors.sort_by(|a, b| {
                (std::cmp::Reverse(a.score), a.kind, a.name.as_str()).cmp(&(
                    std::cmp::Reverse(b.score),
                    b.kind,
                    b.name.as_str(),
                ))
            });
            index.insert(source, neighbors);
        }
        index
    }
}

pub(super) fn build_relation_index() -> HashMap<String, Vec<RelatedNeighbor>> {
    let log = build_catalog_graph("woodshed:catalog:related");
    let graph = log.graph();
    let mut names: HashMap<String, String> = HashMap::new();
    let mut kinds: HashMap<String, CatalogKind> = HashMap::new();
    for (_, node) in graph.nodes() {
        names.insert(
            node.id.clone(),
            if node.title.is_empty() {
                node.id.clone()
            } else {
                node.title.clone()
            },
        );
        if let Some(kind) = kind_of(node) {
            kinds.insert(node.id.clone(), kind);
        }
    }

    let mut builder = RelationBuilder::default();

    // The authored and computed edges the catalog graph already carries.
    for (source, source_node) in graph.nodes() {
        let source_name = names.get(&source_node.id).cloned().unwrap_or_default();
        for (_, target, edge) in graph.out_edges(source) {
            let Some(target_node) = graph.node(target) else {
                continue;
            };
            let target_name = names.get(&target_node.id).cloned().unwrap_or_default();
            let (kind, weight, forward, reverse) = match &edge.class {
                class if class == &RelationClass::app(FAMILY, CONTAINS) => (
                    RelationKind::ContainsMaterial,
                    96,
                    format!("Calls for {target_name}"),
                    format!("Appears in {source_name}"),
                ),
                class if class == &RelationClass::app(FAMILY, FITS_IN_SCALE) => (
                    RelationKind::FitsInScale,
                    92,
                    format!("Fits {target_name}"),
                    format!("Admits {source_name}"),
                ),
                class if class == &RelationClass::app(FAMILY, REALIZES) => (
                    RelationKind::Realizes,
                    100,
                    format!("Arpeggiates {target_name}"),
                    format!("Practice {source_name} as an arpeggio"),
                ),
                _ => continue,
            };
            builder.add_directed(
                &source_node.id,
                &target_node.id,
                kind,
                weight,
                forward,
                reverse,
            );
        }
    }

    add_chord_relations(&mut builder);
    add_scale_relations(&mut builder);
    add_progression_adjacency(&mut builder);

    builder.finish(&names, &kinds)
}

fn circular_distance(a: i32, b: i32) -> i32 {
    let d = (a - b).abs().rem_euclid(12);
    d.min(12 - d)
}

fn voice_leading_distance(a: &BTreeSet<i32>, b: &BTreeSet<i32>) -> f32 {
    let one_way = |from: &BTreeSet<i32>, to: &BTreeSet<i32>| -> f32 {
        from.iter()
            .map(|tone| {
                to.iter()
                    .map(|other| circular_distance(*tone, *other))
                    .min()
                    .unwrap_or(6)
            })
            .sum::<i32>() as f32
            / from.len().max(1) as f32
    };
    (one_way(a, b) + one_way(b, a)) * 0.5
}

/// Chord-to-chord relations. One pair may earn several of these at once, which
/// is the whole point: `Major 7` both extends `Major` and shares three of its
/// tones, and a player is owed both facts rather than the higher-scoring one.
fn add_chord_relations(builder: &mut RelationBuilder) {
    let chords = chord::catalog();
    for (i, source) in chords.iter().enumerate() {
        let source_pcs = pitch_classes(source.intervals);
        for target in chords.iter().skip(i + 1) {
            let target_pcs = pitch_classes(target.intervals);
            let shared = source_pcs.intersection(&target_pcs).count();
            let motion = voice_leading_distance(&source_pcs, &target_pcs);
            let (from_id, to_id) = (chord_id(source.name), chord_id(target.name));

            // Containment: strictly more tones over the same core.
            if source_pcs.len() < target_pcs.len() && source_pcs.is_subset(&target_pcs) {
                let added = target_pcs.len() - source_pcs.len();
                builder.add_directed(
                    &from_id,
                    &to_id,
                    RelationKind::ExtendedBy,
                    90,
                    format!("{} extends it (+{added})", target.name),
                    format!("Extends {} (+{added})", source.name),
                );
            } else if target_pcs.len() < source_pcs.len() && target_pcs.is_subset(&source_pcs) {
                let added = source_pcs.len() - target_pcs.len();
                builder.add_directed(
                    &to_id,
                    &from_id,
                    RelationKind::ExtendedBy,
                    90,
                    format!("{} extends it (+{added})", source.name),
                    format!("Extends {} (+{added})", target.name),
                );
            }

            // Alteration: same size, one tone moved.
            if source_pcs.len() == target_pcs.len() && source_pcs.len().saturating_sub(shared) == 1
            {
                builder.add(
                    MaterialRelation::new(
                        &from_id,
                        &to_id,
                        RelationKind::Alters,
                        76,
                        "One tone apart".to_string(),
                    )
                    .with_shared_tones(shared),
                );
            }

            // Shared tones and voice-leading are distinct facts about the same
            // pair, so they are distinct records, not one blended score.
            if shared > 0 {
                let weight = (44 + shared * 11).min(88) as u16;
                builder.add(
                    MaterialRelation::new(
                        &from_id,
                        &to_id,
                        RelationKind::SharesTones,
                        weight,
                        format!("Shares {shared} tone{}", if shared == 1 { "" } else { "s" }),
                    )
                    .with_shared_tones(shared),
                );
            }
            if motion <= 1.5 {
                let weight = (86.0 - motion * 20.0).round().clamp(0.0, 86.0) as u16;
                builder.add(
                    MaterialRelation::new(
                        &from_id,
                        &to_id,
                        RelationKind::VoiceLeadsTo,
                        weight,
                        format!("Voice-leading {motion:.1}"),
                    )
                    .with_distance(motion),
                );
            }
        }
    }
}

/// The rotations of an interval set, as normalized pitch-class sets. Two scales
/// are modes of each other when one appears among the other's rotations.
fn rotations(pcs: &BTreeSet<i32>) -> Vec<BTreeSet<i32>> {
    pcs.iter()
        .map(|root| {
            pcs.iter()
                .map(|pc| (pc - root).rem_euclid(12))
                .collect::<BTreeSet<i32>>()
        })
        .collect()
}

/// Scale-to-scale relations: modes of one another, or one tone apart.
fn add_scale_relations(builder: &mut RelationBuilder) {
    let scales = scale::catalog();
    for (i, source) in scales.iter().enumerate() {
        let source_pcs = pitch_classes(source.intervals);
        for target in scales.iter().skip(i + 1) {
            let target_pcs = pitch_classes(target.intervals);
            let (from_id, to_id) = (scale_id(source.name), scale_id(target.name));

            if source_pcs.len() == target_pcs.len() && rotations(&source_pcs).contains(&target_pcs)
            {
                builder.add(MaterialRelation::new(
                    &from_id,
                    &to_id,
                    RelationKind::ModeOf,
                    88,
                    "Same tones, different centre".to_string(),
                ));
                continue;
            }

            let shared = source_pcs.intersection(&target_pcs).count();
            if source_pcs.len() == target_pcs.len() && source_pcs.len().saturating_sub(shared) == 1
            {
                builder.add(
                    MaterialRelation::new(
                        &from_id,
                        &to_id,
                        RelationKind::ScaleNeighbor,
                        70,
                        "One degree apart".to_string(),
                    )
                    .with_shared_tones(shared),
                );
            }
        }
    }
}

/// Chords the catalog itself puts next to each other. Authored, not computed:
/// this is a progression saying these two follow one another in practice, which
/// is a different claim from any tone-counting rule and stays a separate record.
fn add_progression_adjacency(builder: &mut RelationBuilder) {
    for progression in progression::catalog() {
        for (adjacency_index, pair) in progression.roles.windows(2).enumerate() {
            let (a, b) = (
                pair[0].quality.chord_formula_name(),
                pair[1].quality.chord_formula_name(),
            );
            if a == b {
                continue;
            }
            let progression_key = progression_id(progression.name);
            let adjacency_key = adjacency_index.to_string();
            builder.add(MaterialRelation::with_claim_components(
                chord_id(a),
                chord_id(b),
                RelationKind::UsedTogether,
                84,
                format!("Adjacent in {}", progression.name),
                &[&progression_key, &adjacency_key],
                vec![GeneratedEvidenceRef::ProgressionAdjacency {
                    progression_id: progression_key.clone(),
                    left_role_ordinal: adjacency_index,
                    right_role_ordinal: adjacency_index + 1,
                }],
            ));
        }
    }
}
