//! The Stage projection: one Set, as a `sceno` scene.
//!
//! Woodshed owns the musical facts and their typed relations; `sceno` owns
//! placement, footprints, representation, and routing. This module is the
//! adapter between the two, the analog of mere's `cartography` graph adapter,
//! and it is the only place in woodshed that knows the scene contract exists.
//!
//! Three properties are deliberate.
//!
//! **A staged card is an instance, not a source.** The material is interned
//! once; staging the same chord twice yields one [`SourceRef`] and two
//! [`ProjectedItem`]s. That is the contract's source-versus-instance
//! separation used as intended, and it is why a Set that drills one voicing
//! four times does not carry four copies of it.
//!
//! **A pair related for several reasons is several relations.** `sceno`
//! deliberately gave relations no channel map, because multi-edge is truth and
//! collapsing to one line is an experience setting. So a chord pair that both
//! shares tones and voice-leads emits two [`RoutedRelation`]s, each with its
//! own kind and weight, and a host chooses whether to fan or collapse them.
//!
//! **Relations here are formula-level, and therefore key-agnostic.**
//! `woodshed-graph` relates catalog formulas (`Major 7` extends `Major`
//! whatever the tonic). Staged cards carry roots, so the Stage layer is where
//! keyed relations (diatonic in, dominant of, resolves to) would eventually be
//! derived. The Set reading preserves those formula relations. The optional
//! musical context adds separate keyed shared-tone and chord-in-scale facts.

use std::collections::{BTreeMap, BTreeSet};

use sceno::{
    Footprint, InstanceId, ProjectedItem, Rect, Representation, RoutedRelation, Scene, Size2,
    SourceRef, Transform2, Vec2,
};
use scenotime::{RelationId, Revision, SceneEpoch, SceneSnapshot};
use woodshed_graph::{
    MaterialRelation, RelationAuthority, RelationKind, chord_id, exercise_id, relations_between,
    scale_id,
};
use woodshedding::rehearsal::{CardId, Material, Set};

use crate::arrangement::{GraphArrangement, arrange_graph};
use crate::harmony::KeyedCatalogRef;
use crate::settings::StageGraphReading;
use crate::stage_context::{
    StageContextOptions, StageNodeId, StageNodeKind, circle_of_fifths_context,
};

/// The adapter name every Stage source ref carries, so a viewer with no
/// woodshed access still knows which product minted the id.
pub const ADAPTER: &str = "woodshed.stage";

/// The relation kind for Set order. Namespaced because the scene's `kind` is
/// an open string shared with every other adapter.
pub const SEQUENCE_KIND: &str = "woodshed:sequence";

/// The emphasis channel practice recency rides on. `"heat"` is the name
/// mere's cartography already uses for freshness, and a host that shades by
/// heat needs no woodshed-specific code.
pub const RECENCY_CHANNEL: &str = "heat";

/// How the Stage projection is realized. Defaults render a legible lane
/// without a solver; a caller that runs one can overwrite the transforms.
#[derive(Clone, Debug)]
pub struct StageSceneOptions {
    /// Footprint stamped on each card item. The contract deleted `measure`,
    /// so the host's measured extent belongs here.
    pub card_size: Size2,
    /// Gap between card centres along the lane.
    pub spacing: f32,
    /// Per-occurrence practice recency in `0..=1`, emitted on
    /// [`RECENCY_CHANNEL`]. Supplied by the caller because this crate owns no
    /// history; the evidence layer is the consumer's.
    pub recency: BTreeMap<CardId, f32>,
    /// Which catalog relation families to draw. `None` draws every one.
    /// Filtering is a view operation: it drops projected relations and never
    /// touches Set truth.
    pub relation_kinds: Option<Vec<RelationKind>>,
    /// Whether Set order is drawn as relations.
    pub sequence: bool,
    /// Product-selected arrangement. It changes placement only; occurrence and
    /// relation identity are assigned after the arrangement is resolved.
    pub arrangement: GraphArrangement,
    /// Optional quiet keyed catalog context. It is derived from the Set and
    /// focus only; it cannot change authored cards or their sequence.
    pub context: Option<StageContextOptions>,
}

impl Default for StageSceneOptions {
    fn default() -> Self {
        Self {
            card_size: Size2::new(160.0, 96.0),
            spacing: 220.0,
            recency: BTreeMap::new(),
            relation_kinds: None,
            sequence: true,
            arrangement: GraphArrangement::Snake,
            context: None,
        }
    }
}

/// One Set projected into the scene contract, with the occurrence identities
/// the scene itself cannot carry.
///
/// `sceno` addresses items by position ([`InstanceId`] is an index), so the
/// mapping back to [`CardId`] rides here rather than inside the scene. That
/// keeps the scene product-free while leaving a woodshed host able to answer
/// "which card did I just click".
#[derive(Clone, Debug)]
pub struct StageGraphSnapshot {
    pub snapshot: SceneSnapshot,
    /// Staged occurrence identities in Set order. Kept for existing consumers;
    /// context items are addressed through [`Self::node_of`].
    pub cards: Vec<CardId>,
    /// Product meaning per dense scene instance, including quiet context.
    pub nodes: Vec<StageSceneNode>,
}

/// Product meaning recovered from a generic scene item.
#[derive(Clone, Debug)]
pub struct StageSceneNode {
    pub id: StageNodeId,
    pub label: String,
    pub kind: StageNodeKind,
    pub material: Option<Material>,
    pub keyed: Option<KeyedCatalogRef>,
    pub foreground: bool,
}

/// Generic relation disclosure for a scene containing catalog context. The
/// established [`StageRelationDetail`] remains the occurrence-only Set API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageSceneRelationDetail {
    pub reference: StageRelationRef,
    pub from: StageNodeId,
    pub to: StageNodeId,
    pub kind: String,
    pub label: String,
}

/// Epoch-qualified item identity used by Cambium callbacks. A stale event from
/// a previous dense projection cannot name a new occupant of the same slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StageInstanceRef {
    pub epoch: SceneEpoch,
    pub instance: InstanceId,
}

/// Epoch-qualified relation identity. Cambium currently carries relation ids as
/// strings, so [`Self::key`] is the lossless adapter at that boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StageRelationRef {
    pub epoch: SceneEpoch,
    pub relation: RelationId,
}

/// Stable, view-local identity for one derivable relation between staged
/// occurrences. Unlike [`StageRelationRef`], this survives a new dense scene
/// epoch: occurrence ids and the open relation kind carry the semantic key.
/// It is suitable for visibility preferences, never for changing relation
/// truth.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageRelationKey {
    pub from: CardId,
    pub to: CardId,
    pub kind: String,
}

impl StageRelationKey {
    /// A DOM/scenario-safe diagnostic key. Consumers should retain the typed
    /// value when they can; this string is only an adapter at text boundaries.
    pub fn wire_key(&self) -> String {
        format!("{}:{}:{}", self.from.0, self.to.0, self.kind)
    }
}

/// Human-facing disclosure for one routed Stage relation. The route remains
/// in Scenograph; this record restores the Woodshed meaning the product UI is
/// responsible for explaining.
#[derive(Clone, Debug, PartialEq)]
pub struct StageRelationDetail {
    pub reference: StageRelationRef,
    pub key: StageRelationKey,
    pub label: String,
    pub explanation: String,
    pub authority: &'static str,
    pub weight: u16,
}

impl StageRelationRef {
    pub fn key(self) -> String {
        format!("{}:{}", self.epoch.0, self.relation.0)
    }

    pub fn from_key(key: &str) -> Option<Self> {
        let (epoch, relation) = key.split_once(':')?;
        Some(Self {
            epoch: SceneEpoch(epoch.parse().ok()?),
            relation: RelationId(relation.parse().ok()?),
        })
    }
}

impl StageGraphSnapshot {
    pub fn epoch(&self) -> SceneEpoch {
        self.snapshot.epoch
    }

    pub fn card_of(&self, instance: InstanceId) -> Option<CardId> {
        match &self.nodes.get(instance.0 as usize)?.id {
            StageNodeId::Card(card) => Some(*card),
            StageNodeId::Catalog(_) => None,
        }
    }

    pub fn node_of(&self, instance: InstanceId) -> Option<&StageSceneNode> {
        self.nodes.get(instance.0 as usize)
    }

    pub fn card_of_ref(&self, reference: StageInstanceRef) -> Option<CardId> {
        (reference.epoch == self.epoch())
            .then(|| self.card_of(reference.instance))
            .flatten()
    }

    pub fn instance_of(&self, card: CardId) -> Option<InstanceId> {
        self.nodes
            .iter()
            .position(|node| node.id == StageNodeId::Card(card))
            .map(|i| InstanceId(i as u32))
    }

    pub fn instance_of_node(&self, node_id: &StageNodeId) -> Option<InstanceId> {
        self.nodes
            .iter()
            .position(|node| &node.id == node_id)
            .map(|i| InstanceId(i as u32))
    }

    pub fn instance_ref_of(&self, card: CardId) -> Option<StageInstanceRef> {
        Some(StageInstanceRef {
            epoch: self.epoch(),
            instance: self.instance_of(card)?,
        })
    }

    pub fn relation_ref(&self, relation: RelationId) -> Option<StageRelationRef> {
        self.snapshot.active_relation(relation)?;
        Some(StageRelationRef {
            epoch: self.epoch(),
            relation,
        })
    }

    pub fn relation(&self, reference: StageRelationRef) -> Option<&RoutedRelation> {
        (reference.epoch == self.epoch())
            .then(|| self.snapshot.active_relation(reference.relation))
            .flatten()
    }

    /// Stable semantic identity for a relation in this snapshot.
    pub fn relation_key(&self, reference: StageRelationRef) -> Option<StageRelationKey> {
        let relation = self.relation(reference)?;
        Some(StageRelationKey {
            from: self.card_of(relation.from)?,
            to: self.card_of(relation.to)?,
            kind: relation.kind.clone().unwrap_or_else(|| "relation".into()),
        })
    }

    /// Explain one relation using the owning Woodshed layer. Set sequence is
    /// authored Set truth; catalog relations recover their typed reason and
    /// authority from `woodshed-graph` rather than asking the scene to carry
    /// product vocabulary.
    pub fn relation_detail(
        &self,
        reference: StageRelationRef,
        set: &Set,
    ) -> Option<StageRelationDetail> {
        let key = self.relation_key(reference)?;
        if key.kind == SEQUENCE_KIND {
            return Some(StageRelationDetail {
                reference,
                key,
                label: "sequence".into(),
                explanation: "Earlier card precedes later card in this Set.".into(),
                authority: "Set",
                weight: 100,
            });
        }

        if key.kind.starts_with("woodshed:keyed-")
            || key.kind.starts_with("woodshed:tonnetz-")
            || key.kind == "woodshed:circle-of-fifths"
        {
            let detail = self.scene_relation_detail(reference)?;
            return Some(StageRelationDetail {
                reference,
                key,
                label: detail.label.clone(),
                explanation: detail.label,
                authority: "Computed",
                weight: (self.relation(reference)?.weight.unwrap_or(0.0) * 100.0).round() as u16,
            });
        }

        let reason = self
            .reasons(key.from, key.to, set)
            .into_iter()
            .find(|reason| relation_slug(reason.kind) == key.kind)?;
        Some(StageRelationDetail {
            reference,
            key,
            label: reason.kind.label().into(),
            explanation: reason.explanation,
            authority: match reason.authority {
                RelationAuthority::Catalog => "Catalog",
                RelationAuthority::Computed => "Computed",
                RelationAuthority::Evidence => "Evidence",
            },
            weight: reason.weight,
        })
    }

    pub fn items(&self) -> Vec<(StageInstanceRef, &ProjectedItem)> {
        self.snapshot
            .active_items_in_order()
            .into_iter()
            .map(|(instance, item)| {
                (
                    StageInstanceRef {
                        epoch: self.epoch(),
                        instance,
                    },
                    item,
                )
            })
            .collect()
    }

    pub fn relations(&self) -> Vec<(StageRelationRef, &RoutedRelation)> {
        self.snapshot
            .tables
            .relations
            .iter()
            .enumerate()
            .filter_map(|(index, relation)| {
                Some((
                    StageRelationRef {
                        epoch: self.epoch(),
                        relation: RelationId(index as u32),
                    },
                    relation.as_ref()?,
                ))
            })
            .collect()
    }

    /// Explain a context relation without forcing it into the occurrence-only
    /// `StageRelationKey` contract used by the existing Set inventory.
    pub fn scene_relation_detail(
        &self,
        reference: StageRelationRef,
    ) -> Option<StageSceneRelationDetail> {
        let relation = self.relation(reference)?;
        let from = self.node_of(relation.from)?.id.clone();
        let to = self.node_of(relation.to)?.id.clone();
        let kind = relation.kind.clone().unwrap_or_else(|| "relation".into());
        let mut label = match kind.as_str() {
            "woodshed:keyed-shared-tones" => {
                let left = self.node_of(relation.from)?.keyed.as_ref()?;
                let right = self.node_of(relation.to)?.keyed.as_ref()?;
                let comparison = crate::harmony::compare_pitch_sets(left, right)?;
                let names = comparison
                    .shared
                    .iter()
                    .map(|tone| {
                        let pitch = woodshedding::pitch::Pitch::from_midi(
                            60 + i32::from(tone.value()),
                            woodshedding::pitch::Spelling::Sharps,
                        );
                        format!("{}{}", pitch.name, pitch.accidental)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("shares tones: {names}")
            },
            "woodshed:keyed-diatonic" => "diatonic in".into(),
            "woodshed:circle-of-fifths" => "roots a fifth apart".into(),
            "woodshed:tonnetz-parallel" => "Parallel: keep root and fifth; change the third".into(),
            "woodshed:tonnetz-relative" => {
                "Relative: retain two tones; move one by a whole step".into()
            },
            "woodshed:tonnetz-leading-tone" => {
                "Leading-tone exchange: retain two tones; move one by a semitone".into()
            },
            SEQUENCE_KIND => "sequence".into(),
            _ => kind.clone(),
        };
        if kind.starts_with("woodshed:tonnetz-") {
            let a = crate::tonnetz::triangle(self.node_of(relation.from)?.keyed.as_ref()?)?;
            let b = crate::tonnetz::triangle(self.node_of(relation.to)?.keyed.as_ref()?)?;
            let shared_vertices = a
                .iter()
                .filter(|a| b.iter().any(|b| a.0 == b.0 && a.1 == b.1))
                .count();
            if shared_vertices < 2 {
                label.push_str(" (wraps across map boundary)");
            }
        }
        Some(StageSceneRelationDetail {
            reference,
            from,
            to,
            kind,
            label,
        })
    }

    /// Every reason two staged occurrences are related, in display order.
    ///
    /// This is what selecting a fanned edge should show: all applicable
    /// reasons with their own authorities, rather than the one that ranked
    /// highest. Returns empty for a pair with no catalog identity on either
    /// side (a hand-drawn Path relates to nothing yet).
    pub fn reasons(&self, from: CardId, to: CardId, set: &Set) -> Vec<MaterialRelation> {
        let material = |id: CardId| set.cards.iter().find(|c| c.id == id).map(|c| &c.material);
        match (
            material(from).and_then(catalog_id),
            material(to).and_then(catalog_id),
        ) {
            (Some(a), Some(b)) => relations_between(&a, &b),
            _ => Vec::new(),
        }
    }
}

/// The catalog identity a material points at, or `None` when it carries its
/// own content.
///
/// A hand-drawn [`Material::Path`] names no catalog formula, so it has no
/// catalog relations. That is a real property of the material rather than a
/// gap: the arranged notes *are* the content.
fn catalog_id(material: &Material) -> Option<String> {
    match material {
        Material::Scale { name, .. } => Some(scale_id(name)),
        Material::Chord { name, .. } => Some(chord_id(name)),
        Material::Riff { name } => Some(exercise_id(name)),
        Material::Path { .. } => None,
    }
}

/// The interned source id for a staged card.
///
/// Catalog materials share a source across occurrences, which is the point.
/// A Path carries its own content, so its source is its occurrence.
fn source_id(material: &Material, card: CardId) -> String {
    catalog_id(material).unwrap_or_else(|| format!("path:{}", card.0))
}

/// A stable wire slug per relation family.
///
/// Written out rather than derived from `RelationKind::label`, because that
/// label is display text a future edit may reword, and this string crosses a
/// wire to viewers that match on it.
fn relation_slug(kind: RelationKind) -> &'static str {
    match kind {
        RelationKind::ContainsMaterial => "woodshed:contains",
        RelationKind::AppearsIn => "woodshed:appears-in",
        RelationKind::FitsInScale => "woodshed:fits-in-scale",
        RelationKind::ScaleAdmits => "woodshed:admits",
        RelationKind::Realizes => "woodshed:realizes",
        RelationKind::RealizedBy => "woodshed:realized-by",
        RelationKind::ExtendedBy => "woodshed:extended-by",
        RelationKind::Extends => "woodshed:extends",
        RelationKind::Alters => "woodshed:alters",
        RelationKind::SharesTones => "woodshed:shares-tones",
        RelationKind::VoiceLeadsTo => "woodshed:voice-leads-to",
        RelationKind::ModeOf => "woodshed:mode-of",
        RelationKind::ScaleNeighbor => "woodshed:scale-neighbor",
        RelationKind::UsedTogether => "woodshed:used-together",
        RelationKind::PracticedBefore => "woodshed:practiced-before",
        RelationKind::PracticedAfter => "woodshed:practiced-after",
    }
}

/// Project a Set into the portable scene contract.
///
/// Placement is a lane along x in Set order, which is honest for material
/// whose order is authoritative and gives the scene real bounds without a
/// solver. A caller running `scenomise` may overwrite the transforms; the
/// relations and identities do not change when it does.
pub fn stage_scene(set: &Set, options: &StageSceneOptions) -> StageGraphSnapshot {
    let mut scene = Scene::new();
    let graph = set.graph();
    let mut cards = Vec::with_capacity(graph.nodes.len());
    let mut nodes = Vec::with_capacity(graph.nodes.len());
    let mut occurrence_offsets = BTreeMap::<KeyedCatalogRef, usize>::new();
    let graph_edges = graph
        .edges
        .iter()
        .filter_map(|edge| {
            Some((
                graph.nodes.iter().position(|node| node.id == edge.from)?,
                graph.nodes.iter().position(|node| node.id == edge.to)?,
            ))
        })
        .collect::<Vec<_>>();
    let positions = arrange_graph(
        options.arrangement,
        graph.nodes.len(),
        &graph_edges,
        if graph.nodes.is_empty() {
            None
        } else {
            Some(set.cursor.min(graph.nodes.len() - 1))
        },
    );
    let span = options.spacing * (graph.nodes.len() as f32).sqrt().ceil().max(1.0);

    // Items, in Set order, one per staged occurrence.
    for node in &graph.nodes {
        let Some(card) = set.cards.iter().find(|c| c.id == node.id) else {
            continue;
        };
        let source =
            scene.intern_source(SourceRef::new(ADAPTER, source_id(&card.material, card.id)));
        let channels = options
            .recency
            .get(&card.id)
            .map(|heat| vec![(RECENCY_CHANNEL.to_string(), *heat)])
            .unwrap_or_default();

        let keyed = KeyedCatalogRef::from_material(&card.material);
        let position = if options.context.is_some() {
            keyed.as_ref().and_then(|keyed| {
                let occurrence = occurrence_offsets.entry(keyed.clone()).or_default();
                let base = context_position(options.context.as_ref()?.reading, keyed)?;
                let offset = *occurrence as f32 * 18.0;
                *occurrence += 1;
                Some(Vec2::new(base.x + offset, base.y + offset))
            })
        } else {
            None
        }
        .unwrap_or_else(|| {
            Vec2::new(
                (positions[node.index].0 - 0.5) * span,
                (positions[node.index].1 - 0.5) * span,
            )
        });
        scene.items.push(ProjectedItem {
            source,
            space: Scene::WORLD,
            transform: Transform2::translation(position.x, position.y),
            footprint: Footprint::Rect {
                size: options.card_size,
            },
            representation: Representation::Card,
            layer: 0,
            visible: true,
            hit: None,
            channels,
        });
        cards.push(card.id);
        nodes.push(StageSceneNode {
            id: StageNodeId::Card(card.id),
            label: card.label.clone(),
            kind: keyed
                .as_ref()
                .map(keyed_kind)
                .unwrap_or(StageNodeKind::Card),
            material: Some(card.material.clone()),
            keyed,
            foreground: true,
        });
    }

    let instance = |id: CardId| {
        cards
            .iter()
            .position(|c| *c == id)
            .map(|i| InstanceId(i as u32))
    };
    // Set order, straight from the Set's own graph so sequence truth stays
    // woodshedding's.
    let mut relations = Vec::new();
    if options.sequence {
        for edge in &graph.edges {
            let (Some(from), Some(to)) = (instance(edge.from), instance(edge.to)) else {
                continue;
            };
            relations.push(RoutedRelation {
                from,
                to,
                space: Scene::WORLD,
                points: vec![item_centre(&scene, from), item_centre(&scene, to)],
                kind: Some(SEQUENCE_KIND.to_string()),
                weight: Some(1.0),
            });
        }
    }

    // Catalog reasons, fanned: one relation per reason per pair, never deduped
    // to the highest-ranked.
    for (i, from_card) in cards.iter().enumerate() {
        let Some(from_material) = set.cards.iter().find(|c| c.id == *from_card) else {
            continue;
        };
        let Some(from_id) = catalog_id(&from_material.material) else {
            continue;
        };
        for to_card in cards.iter().skip(i + 1) {
            let Some(to_material) = set.cards.iter().find(|c| c.id == *to_card) else {
                continue;
            };
            let Some(to_id) = catalog_id(&to_material.material) else {
                continue;
            };
            let (Some(from), Some(to)) = (instance(*from_card), instance(*to_card)) else {
                continue;
            };
            for relation in relations_between(&from_id, &to_id) {
                if let Some(kinds) = &options.relation_kinds {
                    if !kinds.contains(&relation.kind) {
                        continue;
                    }
                }
                relations.push(RoutedRelation {
                    from,
                    to,
                    space: Scene::WORLD,
                    points: vec![item_centre(&scene, from), item_centre(&scene, to)],
                    kind: Some(relation_slug(relation.kind).to_string()),
                    weight: Some(f32::from(relation.weight) / 100.0),
                });
            }
        }
    }

    // Quiet context is a separate keyed reading. Its positions derive from
    // tonic slots on a fixed circle around the current focus, rather than from
    // the number of disclosed nodes, so expanding breadth cannot renormalize
    // material already on screen.
    if let Some(context_options) = &options.context {
        let focused = context_options
            .focus
            .as_ref()
            .and_then(|focus| match focus {
                StageNodeId::Card(card) => set
                    .card(*card)
                    .and_then(|card| KeyedCatalogRef::from_material(&card.material))
                    .map(|keyed| (StageNodeId::Card(*card), keyed)),
                StageNodeId::Catalog(keyed) => Some((focus.clone(), keyed.clone())),
            })
            .or_else(|| {
                set.cursor_id().and_then(|card| {
                    set.card(card)
                        .and_then(|card| KeyedCatalogRef::from_material(&card.material))
                        .map(|keyed| (StageNodeId::Card(card), keyed))
                })
            });
        if let Some((focus_id, focus_keyed)) = focused {
            let omitted = nodes
                .iter()
                .filter_map(|node| node.keyed.clone())
                .collect::<BTreeSet<_>>();
            let query = match context_options.reading {
                StageGraphReading::Tonnetz => crate::tonnetz::context,
                _ => circle_of_fifths_context,
            };
            let context = query(
                &focus_keyed,
                context_options.node_limit,
                &omitted,
                &context_options.retained,
            );
            let mut context_instances = BTreeMap::new();
            for node in &context.nodes {
                let Some(point) = context_position(context_options.reading, &node.keyed) else {
                    continue;
                };
                let source =
                    scene.intern_source(SourceRef::new(ADAPTER, node.keyed.formula_id.clone()));
                let instance = InstanceId(scene.items.len() as u32);
                scene.items.push(ProjectedItem {
                    source,
                    space: Scene::WORLD,
                    transform: Transform2::translation(point.x, point.y),
                    footprint: Footprint::Rect {
                        size: options.card_size,
                    },
                    representation: Representation::Card,
                    layer: -1,
                    visible: true,
                    hit: None,
                    channels: Vec::new(),
                });
                context_instances.insert(node.id.clone(), instance);
                nodes.push(StageSceneNode {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    kind: node.kind,
                    material: node.keyed.to_material(),
                    keyed: Some(node.keyed.clone()),
                    foreground: false,
                });
            }
            for relation in context.relations {
                let from_id = if relation.from == StageNodeId::Catalog(focus_keyed.clone()) {
                    &focus_id
                } else {
                    &relation.from
                };
                let to_id = if relation.to == StageNodeId::Catalog(focus_keyed.clone()) {
                    &focus_id
                } else {
                    &relation.to
                };
                let from = instance_of_node_id(&nodes, from_id)
                    .or_else(|| context_instances.get(from_id).copied());
                let to = instance_of_node_id(&nodes, to_id)
                    .or_else(|| context_instances.get(to_id).copied());
                let (Some(from), Some(to)) = (from, to) else {
                    continue;
                };
                relations.push(RoutedRelation {
                    from,
                    to,
                    space: Scene::WORLD,
                    points: vec![item_centre(&scene, from), item_centre(&scene, to)],
                    kind: Some(relation.kind.slug().to_string()),
                    weight: Some((relation.shared_tones.max(1) as f32 / 12.0).min(1.0)),
                });
            }
        }
    }
    // Keyed relations are a reading over every displayed keyed realization,
    // including non-focused staged cards. Formula relations above remain
    // intact; this adds only the tonic-aware facts and deduplicates the routes
    // already emitted by the bounded context builder.
    if options.context.is_some() {
        let mut seen = BTreeSet::new();
        for route in &relations {
            if let Some(kind) = route.kind.as_deref().filter(|kind| {
                kind.starts_with("woodshed:keyed-") || *kind == "woodshed:circle-of-fifths"
            }) {
                if let (Some(from), Some(to)) = (
                    nodes.get(route.from.0 as usize),
                    nodes.get(route.to.0 as usize),
                ) {
                    let (from, to) = if kind == "woodshed:keyed-shared-tones"
                        || kind == "woodshed:circle-of-fifths"
                    {
                        canonical_endpoints(from.id.clone(), to.id.clone())
                    } else {
                        (from.id.clone(), to.id.clone())
                    };
                    seen.insert((from, to, kind.to_string()));
                }
            }
        }
        for left_index in 0..nodes.len() {
            for right_index in (left_index + 1)..nodes.len() {
                let (Some(left), Some(right)) = (
                    nodes[left_index].keyed.as_ref(),
                    nodes[right_index].keyed.as_ref(),
                ) else {
                    continue;
                };
                let (Some(left_set), Some(right_set)) =
                    (left.pitch_classes(), right.pitch_classes())
                else {
                    continue;
                };
                let shared = left_set.intersection(&right_set).count();
                let from = nodes[left_index].id.clone();
                let to = nodes[right_index].id.clone();
                let append = |relations: &mut Vec<RoutedRelation>, kind: &str, weight: f32| {
                    relations.push(RoutedRelation {
                        from: InstanceId(left_index as u32),
                        to: InstanceId(right_index as u32),
                        space: Scene::WORLD,
                        points: vec![
                            item_centre(&scene, InstanceId(left_index as u32)),
                            item_centre(&scene, InstanceId(right_index as u32)),
                        ],
                        kind: Some(kind.to_string()),
                        weight: Some(weight),
                    });
                };
                let shared_endpoints = canonical_endpoints(from.clone(), to.clone());
                if shared > 0
                    && seen.insert((
                        shared_endpoints.0,
                        shared_endpoints.1,
                        "woodshed:keyed-shared-tones".into(),
                    ))
                {
                    append(
                        &mut relations,
                        "woodshed:keyed-shared-tones",
                        shared as f32 / 12.0,
                    );
                }
                let tonic_delta = (left.root.value() + 12 - right.root.value()) % 12;
                let fifth_endpoints = canonical_endpoints(from.clone(), to.clone());
                if options
                    .context
                    .as_ref()
                    .is_some_and(|context| context.reading == StageGraphReading::CircleOfFifths)
                    && (tonic_delta == 5 || tonic_delta == 7)
                    && seen.insert((
                        fifth_endpoints.0,
                        fifth_endpoints.1,
                        "woodshed:circle-of-fifths".into(),
                    ))
                {
                    append(&mut relations, "woodshed:circle-of-fifths", 1.0);
                }
                if options
                    .context
                    .as_ref()
                    .is_some_and(|context| context.reading == StageGraphReading::Tonnetz)
                {
                    for (neighbor, kind) in crate::tonnetz::neighbors(left) {
                        if &neighbor == right {
                            append(&mut relations, kind, 1.0);
                        }
                    }
                }
                let (chord_index, scale_index) = match (keyed_kind(left), keyed_kind(right)) {
                    (StageNodeKind::Chord, StageNodeKind::Scale) => (left_index, right_index),
                    (StageNodeKind::Scale, StageNodeKind::Chord) => (right_index, left_index),
                    _ => continue,
                };
                let chord = nodes[chord_index]
                    .keyed
                    .as_ref()
                    .unwrap()
                    .pitch_classes()
                    .unwrap();
                let scale = nodes[scale_index]
                    .keyed
                    .as_ref()
                    .unwrap()
                    .pitch_classes()
                    .unwrap();
                let from = nodes[chord_index].id.clone();
                let to = nodes[scale_index].id.clone();
                if chord.is_subset(&scale)
                    && seen.insert((from, to, "woodshed:keyed-diatonic".into()))
                {
                    relations.push(RoutedRelation {
                        from: InstanceId(chord_index as u32),
                        to: InstanceId(scale_index as u32),
                        space: Scene::WORLD,
                        points: vec![
                            item_centre(&scene, InstanceId(chord_index as u32)),
                            item_centre(&scene, InstanceId(scale_index as u32)),
                        ],
                        kind: Some("woodshed:keyed-diatonic".into()),
                        weight: Some(chord.len() as f32 / 12.0),
                    });
                }
            }
        }
    }
    let mut emitted = BTreeSet::new();
    relations.retain(|route| {
        let Some(kind) = route.kind.as_deref() else {
            return true;
        };
        let symmetric = matches!(
            kind,
            "woodshed:keyed-shared-tones" | "woodshed:circle-of-fifths"
        );
        if !symmetric && kind != "woodshed:keyed-diatonic" {
            return true;
        }
        let (Some(from), Some(to)) = (
            nodes.get(route.from.0 as usize),
            nodes.get(route.to.0 as usize),
        ) else {
            return false;
        };
        let (from, to) = if symmetric {
            canonical_endpoints(from.id.clone(), to.id.clone())
        } else {
            (from.id.clone(), to.id.clone())
        };
        emitted.insert((from, to, kind.to_string()))
    });
    scene.relations = relations;
    let item_bounds = if options.context.is_some() {
        Rect::new(Vec2::new(-900.0, -900.0), Size2::new(1800.0, 1800.0))
    } else {
        lane_bounds(&scene, options.card_size)
    };
    let floor_padding = 48.0;
    let floor_bounds = Rect::new(
        Vec2::new(
            item_bounds.origin.x - floor_padding,
            item_bounds.origin.y - floor_padding,
        ),
        Size2::new(
            item_bounds.size.w + floor_padding * 2.0,
            item_bounds.size.h + floor_padding * 2.0,
        ),
    );
    let floor_source = scene.intern_source(SourceRef::new(ADAPTER, "stage-floor"));
    scene.backdrops.push(sceno::Backdrop {
        source: floor_source,
        space: Scene::WORLD,
        transform: Transform2::translation(
            floor_bounds.origin.x + floor_bounds.size.w * 0.5,
            floor_bounds.origin.y + floor_bounds.size.h * 0.5,
        ),
        footprint: Footprint::Rect {
            size: floor_bounds.size,
        },
        kind: "woodshed:stage-floor".into(),
        visible: true,
        collidable: false,
    });
    scene.bounds = floor_bounds;
    let epoch = dense_epoch(&scene, &cards);
    let snapshot = SceneSnapshot::from_dense(epoch, Revision(0), scene)
        .unwrap_or_else(|error| panic!("Woodshed produced an invalid Stage scene: {error:?}"));

    StageGraphSnapshot {
        snapshot,
        cards,
        nodes,
    }
}

fn keyed_kind(keyed: &KeyedCatalogRef) -> StageNodeKind {
    if keyed.formula_id.starts_with("scale:") {
        StageNodeKind::Scale
    } else {
        StageNodeKind::Chord
    }
}

fn canonical_endpoints(a: StageNodeId, b: StageNodeId) -> (StageNodeId, StageNodeId) {
    if a <= b { (a, b) } else { (b, a) }
}

fn instance_of_node_id(nodes: &[StageSceneNode], id: &StageNodeId) -> Option<InstanceId> {
    nodes
        .iter()
        .position(|node| &node.id == id)
        .map(|index| InstanceId(index as u32))
}

fn item_centre(scene: &Scene, instance: InstanceId) -> Vec2 {
    scene
        .items
        .get(instance.0 as usize)
        .map(|item| Vec2::new(item.transform.translate.x, item.transform.translate.y))
        .unwrap_or(Vec2::new(0.0, 0.0))
}

/// Fixed Circle-of-Fifths slots. Relative major/minor share a tonic position:
/// A minor aligns with C major, and their distinct formula ids remain visible.
fn context_position(reading: StageGraphReading, keyed: &KeyedCatalogRef) -> Option<Vec2> {
    match reading {
        StageGraphReading::Tonnetz => crate::tonnetz::position(keyed).map(|(x, y)| Vec2::new(x, y)),
        StageGraphReading::CircleOfFifths => Some(circle_context_position(keyed)),
        StageGraphReading::Set => None,
    }
}

fn circle_context_position(keyed: &KeyedCatalogRef) -> Vec2 {
    let relative_offset = if keyed.formula_id.contains("Minor") {
        3_i16
    } else {
        0
    };
    let pitch = (i16::from(keyed.root.value()) + relative_offset).rem_euclid(12) as f32;
    let fifth_steps = (pitch * 7.0).rem_euclid(12.0);
    let angle = fifth_steps / 12.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let radius = match keyed.formula_id.as_str() {
        "chord:Major" => 580.0,
        "chord:Minor" => 340.0,
        "scale:Major" => 820.0,
        "scale:Minor" => 460.0,
        "chord:Major 7" => 640.0,
        "chord:Dominant 7" => 700.0,
        "chord:Minor 7" => 760.0,
        _ => 820.0,
    };
    Vec2::new(angle.cos() * radius, angle.sin() * radius)
}

/// A dense projection starts a fresh, content-addressed epoch whenever any
/// table occupant or route changes. Woodshed does not emit diffs yet, so this
/// coarse lifetime is safer than reinterpreting an `InstanceId` after reorder.
fn dense_epoch(scene: &Scene, cards: &[CardId]) -> SceneEpoch {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut write = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    };
    for card in cards {
        write(&card.0.to_le_bytes());
    }
    for source in &scene.sources {
        write(&(source.adapter.len() as u64).to_le_bytes());
        write(source.adapter.as_bytes());
        write(&(source.id.len() as u64).to_le_bytes());
        write(source.id.as_bytes());
    }
    for item in &scene.items {
        write(&item.source.0.to_le_bytes());
        write(&item.transform.translate.x.to_bits().to_le_bytes());
        write(&item.transform.translate.y.to_bits().to_le_bytes());
        write(&item.transform.scale.to_bits().to_le_bytes());
        write(&item.transform.rotate.to_bits().to_le_bytes());
        if let Footprint::Rect { size } = &item.footprint {
            write(&size.w.to_bits().to_le_bytes());
            write(&size.h.to_bits().to_le_bytes());
        }
        for (channel, value) in &item.channels {
            write(&(channel.len() as u64).to_le_bytes());
            write(channel.as_bytes());
            write(&value.to_bits().to_le_bytes());
        }
    }
    for backdrop in &scene.backdrops {
        write(&backdrop.source.0.to_le_bytes());
        write(&backdrop.space.0.to_le_bytes());
        write(&backdrop.transform.translate.x.to_bits().to_le_bytes());
        write(&backdrop.transform.translate.y.to_bits().to_le_bytes());
        write(&backdrop.transform.scale.to_bits().to_le_bytes());
        write(&backdrop.transform.rotate.to_bits().to_le_bytes());
        write(&(backdrop.kind.len() as u64).to_le_bytes());
        write(backdrop.kind.as_bytes());
        write(&[backdrop.visible.into(), backdrop.collidable.into()]);
        if let Footprint::Rect { size } = &backdrop.footprint {
            write(&size.w.to_bits().to_le_bytes());
            write(&size.h.to_bits().to_le_bytes());
        }
    }
    for relation in &scene.relations {
        write(&relation.from.0.to_le_bytes());
        write(&relation.to.0.to_le_bytes());
        write(&relation.space.0.to_le_bytes());
        if let Some(kind) = &relation.kind {
            write(&(kind.len() as u64).to_le_bytes());
            write(kind.as_bytes());
        }
        if let Some(weight) = relation.weight {
            write(&weight.to_bits().to_le_bytes());
        }
        for point in &relation.points {
            write(&point.x.to_bits().to_le_bytes());
            write(&point.y.to_bits().to_le_bytes());
        }
    }
    SceneEpoch(hash.max(1))
}

/// World bounds covering every placed card.
fn lane_bounds(scene: &Scene, card: Size2) -> Rect {
    let mut bounds: Option<Rect> = None;
    for item in &scene.items {
        let origin = Vec2::new(
            item.transform.translate.x - card.w * 0.5,
            item.transform.translate.y - card.h * 0.5,
        );
        let rect = Rect::new(origin, card);
        bounds = Some(match bounds {
            Some(acc) => acc.union(rect),
            None => rect,
        });
    }
    bounds.unwrap_or(Rect::new(Vec2::new(0.0, 0.0), Size2::new(0.0, 0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::{Card, LoopMode, Setting, Timing, Touch};

    fn card(label: &str, material: Material) -> Card {
        Card {
            id: CardId::UNASSIGNED,
            label: label.to_string(),
            material,
            setting: Setting::default(),
            touch: Touch::default(),
            timing: Timing::default(),
            from: None,
        }
    }

    fn chord(name: &str) -> Material {
        Material::Chord {
            name: name.to_string(),
            root: PitchClass::new(0),
        }
    }

    fn set_of(materials: &[(&str, Material)]) -> Set {
        Set::from_cards(
            materials.iter().map(|(l, m)| card(l, m.clone())),
            LoopMode::Off,
        )
    }

    fn item_source_count(snapshot: &StageGraphSnapshot) -> usize {
        snapshot
            .snapshot
            .tables
            .items
            .iter()
            .flatten()
            .map(|item| item.source)
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    #[test]
    fn each_staged_occurrence_is_its_own_instance() {
        let set = set_of(&[("one", chord("Major")), ("two", chord("Major"))]);
        let snapshot = stage_scene(&set, &StageSceneOptions::default());

        assert_eq!(
            snapshot.snapshot.active_item_count(),
            2,
            "two occurrences, two items"
        );
        assert_eq!(
            item_source_count(&snapshot),
            1,
            "one material, interned once; the Stage floor is a backdrop source"
        );
        assert_eq!(
            snapshot.snapshot.active_item(InstanceId(0)).unwrap().source,
            snapshot.snapshot.active_item(InstanceId(1)).unwrap().source,
            "both instances point at the same source"
        );
        assert_ne!(
            snapshot.cards[0], snapshot.cards[1],
            "occurrence identities stay distinct"
        );
    }

    #[test]
    fn changed_source_content_starts_a_fresh_dense_epoch() {
        let mut set = set_of(&[("one", chord("Major"))]);
        let before = stage_scene(&set, &StageSceneOptions::default());
        set.cards[0].material = chord("Minor");
        let after = stage_scene(&set, &StageSceneOptions::default());

        assert_ne!(before.epoch(), after.epoch());
        assert_eq!(before.snapshot.revision, Revision(0));
        assert_eq!(after.snapshot.revision, Revision(0));
    }

    #[test]
    fn set_order_is_drawn_and_can_be_withheld() {
        let set = set_of(&[
            ("one", chord("Major")),
            ("two", chord("Minor")),
            ("three", chord("Major 7")),
        ]);
        let with = stage_scene(&set, &StageSceneOptions::default());
        let sequence = |s: &StageGraphSnapshot| {
            s.snapshot
                .tables
                .relations
                .iter()
                .flatten()
                .filter(|r| r.kind.as_deref() == Some(SEQUENCE_KIND))
                .count()
        };
        assert_eq!(sequence(&with), 2, "three cards, two Next edges");

        let without = stage_scene(
            &set,
            &StageSceneOptions {
                sequence: false,
                ..Default::default()
            },
        );
        assert_eq!(sequence(&without), 0);
        assert_eq!(
            without.snapshot.active_item_count(),
            3,
            "withholding a relation family never drops items"
        );
    }

    #[test]
    fn a_pair_related_for_several_reasons_fans_into_several_relations() {
        // Major and Major 7 both share tones and stand in an extension
        // relation, which is the multi-reason case the contract exists to
        // carry without a channel map.
        let set = set_of(&[("plain", chord("Major")), ("seventh", chord("Major 7"))]);
        let snapshot = stage_scene(&set, &StageSceneOptions::default());

        let catalog: Vec<&RoutedRelation> = snapshot
            .snapshot
            .tables
            .relations
            .iter()
            .flatten()
            .filter(|r| r.kind.as_deref() != Some(SEQUENCE_KIND))
            .collect();
        assert!(
            catalog.len() > 1,
            "expected a fan, got {:?}",
            catalog.iter().map(|r| &r.kind).collect::<Vec<_>>()
        );

        let kinds: Vec<&str> = catalog.iter().filter_map(|r| r.kind.as_deref()).collect();
        let unique = kinds.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            kinds.len(),
            "each reason appears once, not deduped to the best one"
        );
        assert!(catalog.iter().all(|r| r.from != r.to), "no self-relations");
    }

    #[test]
    fn selecting_a_pair_exposes_every_reason_with_its_authority() {
        let set = set_of(&[("plain", chord("Major")), ("seventh", chord("Major 7"))]);
        let snapshot = stage_scene(&set, &StageSceneOptions::default());
        let reasons = snapshot.reasons(snapshot.cards[0], snapshot.cards[1], &set);

        assert!(reasons.len() > 1, "the fan is readable back off the pair");
        assert!(
            reasons.iter().all(|r| !r.explanation.is_empty()),
            "every reason explains itself in the player's words"
        );
    }

    #[test]
    fn relation_inventory_keys_survive_dense_scene_epochs() {
        let set = set_of(&[("plain", chord("Major")), ("seventh", chord("Major 7"))]);
        let snake = stage_scene(&set, &StageSceneOptions::default());
        let circle = stage_scene(
            &set,
            &StageSceneOptions {
                arrangement: GraphArrangement::Circle,
                ..StageSceneOptions::default()
            },
        );
        assert_ne!(snake.epoch(), circle.epoch());

        let details = |snapshot: &StageGraphSnapshot| {
            snapshot
                .relations()
                .into_iter()
                .map(|(reference, _)| snapshot.relation_detail(reference, &set).unwrap())
                .collect::<Vec<_>>()
        };
        let snake_details = details(&snake);
        let circle_details = details(&circle);
        assert_eq!(
            snake_details
                .iter()
                .map(|detail| detail.key.clone())
                .collect::<Vec<_>>(),
            circle_details
                .iter()
                .map(|detail| detail.key.clone())
                .collect::<Vec<_>>(),
            "view-local visibility keys do not inherit dense table identity"
        );
        assert!(snake_details.iter().all(|detail| {
            !detail.label.is_empty()
                && !detail.explanation.is_empty()
                && !detail.authority.is_empty()
                && detail.weight > 0
        }));
        assert!(
            snake_details
                .iter()
                .any(|detail| detail.authority == "Set" && detail.key.kind == SEQUENCE_KIND)
        );
    }

    #[test]
    fn a_relation_filter_is_a_view_operation() {
        let set = set_of(&[("plain", chord("Major")), ("seventh", chord("Major 7"))]);
        let only_tones = stage_scene(
            &set,
            &StageSceneOptions {
                relation_kinds: Some(vec![RelationKind::SharesTones]),
                sequence: false,
                ..Default::default()
            },
        );
        assert!(
            only_tones
                .snapshot
                .tables
                .relations
                .iter()
                .flatten()
                .all(|r| r.kind.as_deref() == Some(relation_slug(RelationKind::SharesTones))),
            "only the requested family survives"
        );
        assert_eq!(
            only_tones.snapshot.active_item_count(),
            2,
            "Set truth is untouched"
        );
    }

    #[test]
    fn a_hand_drawn_path_is_placed_but_relates_to_nothing() {
        let set = set_of(&[
            (
                "drawn",
                Material::Path {
                    positions: vec![(0, 3), (1, 5)],
                    root: PitchClass::new(0),
                },
            ),
            ("chord", chord("Major")),
        ]);
        let snapshot = stage_scene(&set, &StageSceneOptions::default());

        assert_eq!(snapshot.snapshot.active_item_count(), 2);
        assert_eq!(
            item_source_count(&snapshot),
            2,
            "a path carries its own content, so the two items use two sources"
        );
        let catalog = snapshot
            .snapshot
            .tables
            .relations
            .iter()
            .flatten()
            .filter(|r| r.kind.as_deref() != Some(SEQUENCE_KIND))
            .count();
        assert_eq!(catalog, 0, "a path names no catalog formula");
    }

    #[test]
    fn circle_context_keeps_relative_pairs_and_extensions_in_distinct_slots() {
        let set = set_of(&[
            ("C major", chord("Major")),
            (
                "A minor",
                Material::Chord {
                    name: "Minor".into(),
                    root: PitchClass::new(9),
                },
            ),
        ]);
        let snapshot = stage_scene(
            &set,
            &StageSceneOptions {
                context: Some(StageContextOptions {
                    node_limit: 12,
                    ..StageContextOptions::default()
                }),
                ..StageSceneOptions::default()
            },
        );
        let positions = snapshot
            .snapshot
            .tables
            .items
            .iter()
            .flatten()
            .map(|item| {
                (
                    item.transform.translate.x.to_bits(),
                    item.transform.translate.y.to_bits(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(positions.len(), snapshot.snapshot.active_item_count());
    }

    #[test]
    fn circle_context_completes_the_twelve_major_fifth_links_through_a_staged_c() {
        let set = set_of(&[("C", chord("Major"))]);
        let snapshot = stage_scene(
            &set,
            &StageSceneOptions {
                context: Some(StageContextOptions {
                    node_limit: 36,
                    ..StageContextOptions::default()
                }),
                ..StageSceneOptions::default()
            },
        );
        let major = |instance: InstanceId| {
            snapshot
                .node_of(instance)
                .and_then(|node| node.keyed.as_ref())
                .is_some_and(|keyed| keyed.formula_id == "chord:Major")
        };
        let fifths = snapshot
            .relations()
            .into_iter()
            .filter(|(_, relation)| {
                relation.kind.as_deref() == Some("woodshed:circle-of-fifths")
                    && major(relation.from)
                    && major(relation.to)
            })
            .map(|(_, relation)| {
                canonical_endpoints(
                    snapshot.node_of(relation.from).unwrap().id.clone(),
                    snapshot.node_of(relation.to).unwrap().id.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(fifths.len(), 12, "expected the complete major-key circle");
        let c = StageNodeId::Card(set.cards[0].id);
        for root in [5, 7] {
            assert!(fifths.contains(&canonical_endpoints(
                c.clone(),
                StageNodeId::Catalog(KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(root),
                }),
            )));
        }
    }

    #[test]
    fn tonnetz_has_twenty_four_triads_and_thirty_six_transformations() {
        let set = set_of(&[("C", chord("Major"))]);
        let snapshot = stage_scene(
            &set,
            &StageSceneOptions {
                context: Some(StageContextOptions {
                    reading: StageGraphReading::Tonnetz,
                    node_limit: 36,
                    ..StageContextOptions::default()
                }),
                ..StageSceneOptions::default()
            },
        );
        assert_eq!(snapshot.nodes.len(), 24);
        assert_eq!(snapshot.cards.len(), 1);
        let transformations: Vec<_> = snapshot
            .relations()
            .into_iter()
            .filter(|(_, route)| {
                route
                    .kind
                    .as_deref()
                    .is_some_and(|kind| kind.starts_with("woodshed:tonnetz-"))
            })
            .collect();
        assert_eq!(transformations.len(), 36);
        let staged = snapshot.instance_of(set.cards[0].id).unwrap();
        let incident: Vec<_> = transformations
            .iter()
            .filter(|(_, route)| route.from == staged || route.to == staged)
            .collect();
        assert_eq!(incident.len(), 3);
        for (reference, _) in incident {
            assert!(
                !snapshot
                    .scene_relation_detail(*reference)
                    .unwrap()
                    .label
                    .contains("wraps")
            );
        }
        assert!(transformations.iter().any(|(reference, _)| {
            snapshot
                .scene_relation_detail(*reference)
                .unwrap()
                .label
                .contains("wraps")
        }));
        assert!(
            !snapshot
                .relations()
                .iter()
                .any(|(_, route)| route.kind.as_deref() == Some("woodshed:circle-of-fifths"))
        );
    }

    #[test]
    fn focused_catalog_context_has_no_duplicate_symmetric_shared_tone_routes() {
        let set = set_of(&[("C", chord("Major"))]);
        let focus = KeyedCatalogRef {
            formula_id: "chord:Major".into(),
            root: PitchClass::new(2),
        };
        let snapshot = stage_scene(
            &set,
            &StageSceneOptions {
                context: Some(StageContextOptions {
                    focus: Some(StageNodeId::Catalog(focus)),
                    node_limit: 24,
                    ..StageContextOptions::default()
                }),
                ..StageSceneOptions::default()
            },
        );
        let mut keys = BTreeSet::new();
        for (_, relation) in snapshot.relations() {
            if relation.kind.as_deref() != Some("woodshed:keyed-shared-tones") {
                continue;
            }
            let from = snapshot.node_of(relation.from).unwrap().id.clone();
            let to = snapshot.node_of(relation.to).unwrap().id.clone();
            assert!(keys.insert(canonical_endpoints(from, to)));
        }
    }

    #[test]
    fn recency_rides_the_scene_rather_than_a_host_side_lookup() {
        let set = set_of(&[("one", chord("Major")), ("two", chord("Minor"))]);
        let mut recency = BTreeMap::new();
        recency.insert(set.cards[0].id, 0.75);
        let snapshot = stage_scene(
            &set,
            &StageSceneOptions {
                recency,
                ..Default::default()
            },
        );

        assert_eq!(
            snapshot
                .snapshot
                .active_item(InstanceId(0))
                .unwrap()
                .channels,
            vec![(RECENCY_CHANNEL.to_string(), 0.75)],
            "a remote viewer shades from the scene, not from woodshed's store"
        );
        assert!(
            snapshot
                .snapshot
                .active_item(InstanceId(1))
                .unwrap()
                .channels
                .is_empty()
        );
    }

    #[test]
    fn instances_and_occurrences_map_both_ways() {
        let set = set_of(&[("one", chord("Major")), ("two", chord("Minor"))]);
        let snapshot = stage_scene(&set, &StageSceneOptions::default());

        for (i, card) in snapshot.cards.iter().enumerate() {
            let instance = InstanceId(i as u32);
            assert_eq!(snapshot.card_of(instance), Some(*card));
            assert_eq!(snapshot.instance_of(*card), Some(instance));
        }
        assert_eq!(snapshot.card_of(InstanceId(99)), None);
    }

    #[test]
    fn epoch_qualifies_instance_and_relation_table_ids() {
        let set = set_of(&[("one", chord("Major")), ("two", chord("Major 7"))]);
        let first = stage_scene(&set, &StageSceneOptions::default());
        let instance = first.instance_ref_of(first.cards[0]).unwrap();
        let relation = first.relations()[0].0;
        assert_eq!(first.card_of_ref(instance), Some(first.cards[0]));
        assert!(first.relation(relation).is_some());

        let rearranged = stage_scene(
            &set,
            &StageSceneOptions {
                arrangement: GraphArrangement::Circle,
                ..StageSceneOptions::default()
            },
        );
        assert_ne!(first.epoch(), rearranged.epoch());
        assert_eq!(rearranged.card_of_ref(instance), None);
        assert!(rearranged.relation(relation).is_none());
        assert_eq!(
            StageRelationRef::from_key(&relation.key()),
            Some(relation),
            "the Cambium string seam preserves SceneEpoch and RelationId"
        );
    }

    #[test]
    fn an_empty_set_projects_an_empty_scene_not_a_broken_one() {
        let snapshot = stage_scene(&Set::default(), &StageSceneOptions::default());
        assert_eq!(snapshot.snapshot.active_item_count(), 0);
        assert!(
            snapshot
                .snapshot
                .tables
                .relations
                .iter()
                .flatten()
                .next()
                .is_none()
        );
        assert_eq!(
            snapshot.snapshot.tables.spaces.iter().flatten().count(),
            1,
            "the world space still exists"
        );
    }

    #[test]
    fn the_lane_grows_with_the_set_and_every_card_is_inside_it() {
        let set = set_of(&[
            ("one", chord("Major")),
            ("two", chord("Minor")),
            ("three", chord("Major 7")),
        ]);
        let options = StageSceneOptions::default();
        let snapshot = stage_scene(&set, &options);
        let bounds = snapshot.snapshot.tables.bounds;

        assert!(bounds.size.w >= options.spacing * 2.0);
        for item in snapshot.snapshot.tables.items.iter().flatten() {
            let x = item.transform.translate.x;
            assert!(
                x >= bounds.origin.x && x <= bounds.origin.x + bounds.size.w,
                "card at {x} fell outside the framed bounds"
            );
        }
    }
}
