//! woodshed-graph — woodshed's theory catalog as a chartulary graph.
//!
//! The first real consumer of the chartulary substrate (G3). It reads woodshed's
//! *existing* theory catalog (scales, chords, progressions, exercises) and relates
//! it as a content-addressed container graph, then demonstrates practice lineage
//! over stemma. No data is invented: the relations are read from or computed over
//! the real catalog.
//!
//! - **Nodes** are theory objects. Each is a [`Container`] whose id is
//!   `kind:name` (`chord:Major 7`, `scale:Major`), titled by its name and tagged
//!   with its kind and category.
//! - **Edges** are woodshed's own relation family (the app-private "inner ring",
//!   [`RelationClass::app`]):
//!   - a progression `Contains` a chord (read from the progression's roles), and
//!   - a chord `FitsInScale` a scale (computed: the chord's pitch classes are a
//!     subset of the scale's).
//!
//! Practice lineage lives in stemma, keyed by the same node ids: a practice
//! session is an `Owner` whose visits to theory objects branch and are recorded.
//! The spine does not auto-feed stemma; the consumer records visits with its own
//! "engagement" semantics, keyed by the shared identity. That is how the two
//! layers integrate (substrate plan open question 6, settled here: consumer-side).

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use portolan::{Author, Container, GraphLog, LogId, Relation, RelationClass};
use woodshedding::{chord, exercise, progression, scale};

/// woodshed's app-private relation family name.
pub const FAMILY: &str = "woodshed";
/// A progression contains a chord (kind 0 of the woodshed family).
pub const CONTAINS: u16 = 0;
/// A chord fits within a scale (kind 1 of the woodshed family).
pub const FITS_IN_SCALE: u16 = 1;
/// An arpeggio is the sequential realization of a chord formula.
pub const REALIZES: u16 = 2;
/// A user-authored personal relation. Source provenance lives in the
/// assertion owner; this app-private predicate only says the endpoints are
/// related and carries no generated/catalog truth.
pub const RELATED: u16 = 3;
/// Stable predicate IRI for the app-private personal `related` relation.
pub const RELATED_PREDICATE_IRI: &str = "urn:woodshed:relation:related";

/// Catalog kind carried by a related-material result. This deliberately names
/// catalog identities rather than a configured practice Card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CatalogKind {
    Scale,
    Chord,
    Arpeggio,
    Progression,
    Exercise,
}

impl CatalogKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scale => "Scale",
            Self::Chord => "Chord",
            Self::Arpeggio => "Arpeggio",
            Self::Progression => "Progression",
            Self::Exercise => "Exercise",
        }
    }
}

/// One material in the catalog graph, projected without its chartulary key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogMaterial {
    pub id: String,
    pub name: String,
    pub kind: CatalogKind,
}

/// Who says so. A relation's authority travels with it, because a filter that
/// hides one layer must not be able to delete another's facts, and a suggestion
/// must never read as catalog truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationAuthority {
    /// Authored in the catalog and read back (a progression's roles).
    Catalog,
    /// Derived from catalog truth by a deterministic rule. Reproducible from
    /// the formulas alone, with no history and no inference.
    Computed,
    /// Observed practice. Produced by the consumer that owns the history, not
    /// by this crate; the kind exists here so one record type spans the layers.
    Evidence,
}

const CATALOG_RELATION_METHOD: GeneratedMethod = GeneratedMethod {
    iri: "urn:woodshed:method:catalog-progression-membership",
    version: 1,
};
const CATALOG_ADJACENCY_METHOD: GeneratedMethod = GeneratedMethod {
    iri: "urn:woodshed:method:catalog-progression-adjacency",
    version: 1,
};
const COMPUTED_RELATION_METHOD: GeneratedMethod = GeneratedMethod {
    iri: "urn:woodshed:method:computed-formula-relations",
    version: 1,
};
const PRACTICE_TRANSITION_METHOD: GeneratedMethod = GeneratedMethod {
    iri: "urn:woodshed:method:practice-relation-aggregate",
    version: 1,
};
const PRACTICE_PATH_METHOD: GeneratedMethod = GeneratedMethod {
    iri: "urn:woodshed:method:practice-path-transition-aggregate",
    version: 1,
};
const PRACTICE_REASON_METHOD: GeneratedMethod = GeneratedMethod {
    iri: "urn:woodshed:method:practice-stated-source-aggregate",
    version: 1,
};

/// Stable source-owned identity for one generated relation reason. Inverse
/// display rows share this identity; independent reasons carry different keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneratedClaimId {
    pub owner: GeneratedClaimOwner,
    pub key: String,
}

impl GeneratedClaimId {
    /// An unambiguous, source-qualified token for portable consumers.
    pub fn qualified_key(&self) -> String {
        format!("{}:{}", self.owner.as_str(), self.key)
    }

    fn new(owner: GeneratedClaimOwner, components: &[&str]) -> Self {
        let key = components
            .iter()
            .map(|component| format!("{}:{component}", component.len()))
            .collect::<String>();
        Self { owner, key }
    }

    fn for_relation(source: &str, target: &str, kind: RelationKind) -> Self {
        let owner = match kind.authority() {
            RelationAuthority::Catalog => GeneratedClaimOwner::Catalog,
            RelationAuthority::Computed => GeneratedClaimOwner::Computed,
            RelationAuthority::Evidence => GeneratedClaimOwner::Evidence,
        };
        Self::new(owner, &[kind.stable_key(), source, target])
    }
}

/// The relation vocabulary, root-independent by construction.
///
/// Every kind here relates catalog *formulas*, which is why none of them names
/// a key: `Major 7` extends `Major` whatever the tonic. The keyed relations the
/// plan also wants (diatonic in, borrowed from, dominant of, resolves to,
/// relative/parallel) are contextual — they exist only once a tonic is chosen —
/// so they belong to the keyed-instance layer, beside first-class pitch-class
/// identities, rather than being faked at formula level here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    /// A progression calls for this chord.
    ContainsMaterial,
    /// This chord is called for by that progression.
    AppearsIn,
    /// This chord's tones are a subset of that scale's.
    FitsInScale,
    /// That scale admits this chord.
    ScaleAdmits,
    /// An arpeggio is the sequential realization of a chord formula.
    Realizes,
    /// A chord formula is realized by that arpeggio.
    RealizedBy,
    /// This chord's tones are a strict subset of that chord's (Major -> Major 7).
    ExtendedBy,
    /// This chord strictly contains that chord's tones.
    Extends,
    /// Same tone count, one tone moved: an alteration of the same colour.
    Alters,
    /// The two share pitch classes; the count rides on the record.
    SharesTones,
    /// Symmetric voice-leading distance; the distance rides on the record.
    VoiceLeadsTo,
    /// One scale is a rotation of the other's interval set.
    ModeOf,
    /// Same degree count, differing by exactly one tone.
    ScaleNeighbor,
    /// Two chords sit next to each other inside a catalog progression.
    UsedTogether,
    /// Observed: this was practiced before that (consumer-supplied).
    PracticedBefore,
    /// Observed: this was practiced after that (consumer-supplied).
    PracticedAfter,
}

impl RelationKind {
    fn stable_key(self) -> &'static str {
        match self {
            Self::ContainsMaterial => "contains-material",
            Self::AppearsIn => "appears-in",
            Self::FitsInScale => "fits-in-scale",
            Self::ScaleAdmits => "scale-admits",
            Self::Realizes => "realizes",
            Self::RealizedBy => "realized-by",
            Self::ExtendedBy => "extended-by",
            Self::Extends => "extends",
            Self::Alters => "alters",
            Self::SharesTones => "shares-tones",
            Self::VoiceLeadsTo => "voice-leads-to",
            Self::ModeOf => "mode-of",
            Self::ScaleNeighbor => "scale-neighbor",
            Self::UsedTogether => "used-together",
            Self::PracticedBefore => "practiced-before",
            Self::PracticedAfter => "practiced-after",
        }
    }

    /// Stable source-owned predicate identifier for this relation kind.
    /// This is protocol vocabulary; use [`Self::label`] only for display.
    pub fn predicate_iri(self) -> String {
        format!("urn:woodshed:relation:{}", self.stable_key())
    }

    /// Short human name, for an edge explanation or a relation filter.
    pub fn label(self) -> &'static str {
        match self {
            Self::ContainsMaterial => "contains",
            Self::AppearsIn => "appears in",
            Self::FitsInScale => "fits in scale",
            Self::ScaleAdmits => "admits",
            Self::Realizes => "realizes",
            Self::RealizedBy => "realized by",
            Self::ExtendedBy => "extended by",
            Self::Extends => "extends",
            Self::Alters => "alters",
            Self::SharesTones => "shares tones",
            Self::VoiceLeadsTo => "voice-leads to",
            Self::ModeOf => "mode of",
            Self::ScaleNeighbor => "one tone apart",
            Self::UsedTogether => "used together",
            Self::PracticedBefore => "practiced before",
            Self::PracticedAfter => "practiced after",
        }
    }

    /// Which layer owns it. The Set layer is not here: Set order is the Set's
    /// own truth, not a catalog relation.
    pub fn authority(self) -> RelationAuthority {
        match self {
            Self::ContainsMaterial | Self::AppearsIn | Self::UsedTogether => {
                RelationAuthority::Catalog
            },
            Self::PracticedBefore | Self::PracticedAfter => RelationAuthority::Evidence,
            _ => RelationAuthority::Computed,
        }
    }

    /// The kind naming the same fact from the other end. Symmetric kinds are
    /// their own inverse, which is what lets one computation record both
    /// directions without inventing a second relation.
    pub fn inverse(self) -> Self {
        match self {
            Self::ContainsMaterial => Self::AppearsIn,
            Self::AppearsIn => Self::ContainsMaterial,
            Self::FitsInScale => Self::ScaleAdmits,
            Self::ScaleAdmits => Self::FitsInScale,
            Self::Realizes => Self::RealizedBy,
            Self::RealizedBy => Self::Realizes,
            Self::ExtendedBy => Self::Extends,
            Self::Extends => Self::ExtendedBy,
            Self::PracticedBefore => Self::PracticedAfter,
            Self::PracticedAfter => Self::PracticedBefore,
            symmetric => symmetric,
        }
    }

    pub fn is_symmetric(self) -> bool {
        self.inverse() == self
    }

    /// Every deterministic kind this crate can currently derive, for a relation
    /// filter that wants to name its families rather than count them.
    pub const DETERMINISTIC: [Self; 14] = [
        Self::ContainsMaterial,
        Self::AppearsIn,
        Self::FitsInScale,
        Self::ScaleAdmits,
        Self::Realizes,
        Self::RealizedBy,
        Self::ExtendedBy,
        Self::Extends,
        Self::Alters,
        Self::SharesTones,
        Self::VoiceLeadsTo,
        Self::ModeOf,
        Self::ScaleNeighbor,
        Self::UsedTogether,
    ];
}

/// One typed relation between two catalog materials.
///
/// This replaces the old flattened `{ reason, score }` pair. The difference
/// that matters: several of these may connect the same pair, and ranking
/// chooses what to show first without deleting the rest.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRelation {
    /// Stable identity owned by the catalog or computing rule, never a view
    /// counter or a Chartulary assertion ID.
    pub claim_id: GeneratedClaimId,
    pub source: String,
    pub target: String,
    pub kind: RelationKind,
    /// This relation's own strength in `0..=100`. Not a ranking of the
    /// neighbor: one pair's several relations each carry their own.
    pub weight: u16,
    /// Continuous distance where the relation has one (voice-leading motion).
    pub distance: Option<f32>,
    /// Shared pitch-class count where the relation is about tones.
    pub shared_tones: Option<usize>,
    /// Why, in the player's words.
    pub explanation: String,
    pub authority: RelationAuthority,
    /// Original generated assertion direction and typed source evidence.
    pub provenance: GeneratedClaimProvenance,
}

impl MaterialRelation {
    fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        kind: RelationKind,
        weight: u16,
        explanation: impl Into<String>,
    ) -> Self {
        let source = source.into();
        let target = target.into();
        let explanation = explanation.into();
        Self {
            claim_id: GeneratedClaimId::for_relation(&source, &target, kind),
            provenance: provenance_for(&source, &target, kind, &explanation, None),
            source,
            target,
            kind,
            weight,
            distance: None,
            shared_tones: None,
            explanation,
            authority: kind.authority(),
        }
    }

    fn with_claim_components(
        source: impl Into<String>,
        target: impl Into<String>,
        kind: RelationKind,
        weight: u16,
        explanation: impl Into<String>,
        reason_components: &[&str],
        evidence: Vec<GeneratedEvidenceRef>,
    ) -> Self {
        let source = source.into();
        let target = target.into();
        let mut components: Vec<&str> = vec![kind.stable_key(), source.as_str(), target.as_str()];
        components.extend_from_slice(reason_components);
        let explanation = explanation.into();
        Self {
            claim_id: GeneratedClaimId::new(GeneratedClaimOwner::Catalog, &components),
            provenance: provenance_for(&source, &target, kind, &explanation, Some(evidence)),
            source,
            target,
            kind,
            weight,
            distance: None,
            shared_tones: None,
            explanation,
            authority: kind.authority(),
        }
    }

    /// Build an observed relation. Public because the evidence layer is the
    /// consumer's: this crate owns no history and must not pretend to. The kind
    /// must be an [`RelationAuthority::Evidence`] one, so an observation cannot
    /// enter the record set wearing catalog authority.
    pub fn evidence(
        source: impl Into<String>,
        target: impl Into<String>,
        kind: RelationKind,
        weight: u16,
        explanation: impl Into<String>,
    ) -> Self {
        debug_assert_eq!(
            kind.authority(),
            RelationAuthority::Evidence,
            "an observed relation must use an evidence kind"
        );
        let mut relation = Self {
            authority: RelationAuthority::Evidence,
            ..Self::new(source, target, kind, weight, explanation)
        };
        relation.claim_id.owner = GeneratedClaimOwner::Evidence;
        relation
    }

    /// Build an observed aggregate relation while retaining its exact available
    /// traversal count as typed evidence. The count is an aggregate, not an
    /// identity for any individual practice event.
    pub fn evidence_with_count(
        source: impl Into<String>,
        target: impl Into<String>,
        kind: RelationKind,
        weight: u16,
        explanation: impl Into<String>,
        basis: PracticeEvidenceBasis,
        count: u64,
    ) -> Self {
        let mut relation = Self::evidence(source, target, kind, weight, explanation);
        relation.provenance.method = match basis {
            PracticeEvidenceBasis::WalkedPath => PRACTICE_PATH_METHOD,
            PracticeEvidenceBasis::StatedFromReason => PRACTICE_REASON_METHOD,
            PracticeEvidenceBasis::Unspecified => PRACTICE_TRANSITION_METHOD,
        };
        relation.provenance.evidence = vec![GeneratedEvidenceRef::PracticeTransitionAggregate {
            from: relation.provenance.original_from.clone(),
            to: relation.provenance.original_to.clone(),
            basis,
            count: Some(count),
        }];
        relation
    }

    fn with_distance(mut self, distance: f32) -> Self {
        self.distance = Some(distance);
        self
    }

    fn with_shared_tones(mut self, shared: usize) -> Self {
        self.shared_tones = Some(shared);
        self
    }
}

/// One neighbor of a focused material, with **every** relation that connects
/// them. `score` orders neighbors for display; it never decides which relations
/// survive.
#[derive(Clone, Debug, PartialEq)]
pub struct RelatedNeighbor {
    pub id: String,
    pub name: String,
    pub kind: CatalogKind,
    /// Strongest first. Always at least one.
    pub relations: Vec<MaterialRelation>,
    /// Display rank in `0..=100`: the strongest relation, plus a small bonus
    /// for carrying several, so a pair related four ways outranks a pair
    /// related once at the same strength.
    pub score: u16,
}

impl RelatedNeighbor {
    /// The relation shown when there is room for one.
    pub fn primary(&self) -> &MaterialRelation {
        &self.relations[0]
    }

    /// The primary explanation, which is what a one-line row shows.
    pub fn reason(&self) -> &str {
        &self.primary().explanation
    }

    /// Every applicable relation kind, in display order.
    pub fn kinds(&self) -> Vec<RelationKind> {
        self.relations.iter().map(|r| r.kind).collect()
    }

    /// Every explanation, which is what selecting the edge should expose.
    pub fn explanations(&self) -> Vec<&str> {
        self.relations
            .iter()
            .map(|r| r.explanation.as_str())
            .collect()
    }
}

fn kind_of(node: &Container) -> Option<CatalogKind> {
    if node.tags.iter().any(|tag| tag == "scale") {
        Some(CatalogKind::Scale)
    } else if node.tags.iter().any(|tag| tag == "chord") {
        Some(CatalogKind::Chord)
    } else if node.tags.iter().any(|tag| tag == "arpeggio") {
        Some(CatalogKind::Arpeggio)
    } else if node.tags.iter().any(|tag| tag == "progression") {
        Some(CatalogKind::Progression)
    } else if node.tags.iter().any(|tag| tag == "exercise") {
        Some(CatalogKind::Exercise)
    } else {
        None
    }
}

/// A catalog object's display name, if it exists.
pub fn material_name(id: &str) -> Option<String> {
    node_names().get(id).cloned()
}

/// Every material in the catalog graph, in stable id order. This is the
/// whole-mere source; focused views should use [`related_neighbors`] instead.
pub fn catalog_materials() -> Vec<CatalogMaterial> {
    static MATERIALS: OnceLock<Vec<CatalogMaterial>> = OnceLock::new();
    MATERIALS
        .get_or_init(|| {
            let log = build_catalog_graph("woodshed:catalog:materials");
            let mut materials = log
                .graph()
                .nodes()
                .filter_map(|(_, node)| {
                    Some(CatalogMaterial {
                        id: node.id.clone(),
                        name: if node.title.is_empty() {
                            node.id.clone()
                        } else {
                            node.title.clone()
                        },
                        kind: kind_of(node)?,
                    })
                })
                .collect::<Vec<_>>();
            materials.sort_by(|a, b| a.id.cmp(&b.id));
            materials
        })
        .clone()
}

fn relation_index() -> &'static HashMap<String, Vec<RelatedNeighbor>> {
    static INDEX: OnceLock<HashMap<String, Vec<RelatedNeighbor>>> = OnceLock::new();
    INDEX.get_or_init(build_relation_index)
}

fn node_names() -> &'static HashMap<String, String> {
    static NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let log = build_catalog_graph("woodshed:catalog:names");
        log.graph()
            .nodes()
            .map(|(_, node)| {
                (
                    node.id.clone(),
                    if node.title.is_empty() {
                        node.id.clone()
                    } else {
                        node.title.clone()
                    },
                )
            })
            .collect()
    })
}

#[cfg(test)]
mod tests;

mod relations;

use relations::build_relation_index;

mod provenance;

use provenance::provenance_for;
pub use provenance::{
    GeneratedClaimOwner, GeneratedClaimProvenance, GeneratedEvidenceRef, GeneratedMethod,
    PracticeEvidenceBasis,
};

mod catalog;

use catalog::pitch_classes;
pub use catalog::{
    arpeggio_id, build_catalog_graph, chord_approach_exercise_id, chord_approach_id, chord_id,
    exercise_id, progression_id, scale_id, scale_pattern_exercise_id, scale_pattern_id,
};

mod queries;

pub use queries::{related_neighbors, relations_between};
