use super::*;

/// The catalog/rule owner that gives a generated reason its stable namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeneratedClaimOwner {
    /// Authored in the Woodshed theory catalog.
    Catalog,
    /// Reproducibly derived by a named Woodshed rule family.
    Computed,
    /// Projected from the session's retained practice history.
    Evidence,
}

impl GeneratedClaimOwner {
    /// Stable wire token for this relation source namespace.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Computed => "computed",
            Self::Evidence => "evidence",
        }
    }
}

/// Source-owned method used to produce a generated claim. The version changes
/// only when the meaning of the named extraction or computation changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneratedMethod {
    pub iri: &'static str,
    pub version: u16,
}

/// Exact source material available to explain a generated claim. These are
/// references, not copied catalog payloads or claims of authorship.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeneratedEvidenceRef {
    /// A catalog relation from the source graph.
    CatalogRelation {
        from: String,
        to: String,
        kind: RelationKind,
    },
    /// Catalog formulas used as the inputs to a deterministic comparison.
    CatalogMaterial { id: String },
    /// The exact adjacent roles in a named progression that support a
    /// UsedTogether claim.
    ProgressionAdjacency {
        progression_id: String,
        left_role_ordinal: usize,
        right_role_ordinal: usize,
    },
    /// An aggregate of retained practice transitions. `None` means the
    /// projecting caller did not supply a count; no event identity is implied.
    PracticeTransitionAggregate {
        from: String,
        to: String,
        basis: PracticeEvidenceBasis,
        count: Option<u64>,
    },
}

/// Which practice-history aggregate supports an observed relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PracticeEvidenceBasis {
    /// The retained history recorded adjacent visits along a walked path.
    WalkedPath,
    /// The practitioner explicitly staged one material from another.
    StatedFromReason,
    /// A caller used the legacy evidence constructor without identifying its
    /// history projection semantics.
    Unspecified,
}

/// Typed provenance retained with one generated reason. The `original_*`
/// fields describe its source assertion even when a reverse display row swaps
/// `MaterialRelation.source`, `.target`, and `.kind`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedClaimProvenance {
    pub original_from: String,
    pub original_to: String,
    pub original_kind: RelationKind,
    pub original_explanation: String,
    pub method: GeneratedMethod,
    pub evidence: Vec<GeneratedEvidenceRef>,
}

pub(super) fn provenance_for(
    source: &str,
    target: &str,
    kind: RelationKind,
    explanation: &str,
    evidence: Option<Vec<GeneratedEvidenceRef>>,
) -> GeneratedClaimProvenance {
    let (method, evidence) = if let Some(evidence) = evidence {
        let method = if kind == RelationKind::UsedTogether {
            CATALOG_ADJACENCY_METHOD
        } else {
            CATALOG_RELATION_METHOD
        };
        (method, evidence)
    } else {
        match kind.authority() {
            RelationAuthority::Catalog => (
                CATALOG_RELATION_METHOD,
                vec![GeneratedEvidenceRef::CatalogRelation {
                    from: source.to_owned(),
                    to: target.to_owned(),
                    kind,
                }],
            ),
            RelationAuthority::Computed => (
                COMPUTED_RELATION_METHOD,
                vec![
                    GeneratedEvidenceRef::CatalogMaterial {
                        id: source.to_owned(),
                    },
                    GeneratedEvidenceRef::CatalogMaterial {
                        id: target.to_owned(),
                    },
                ],
            ),
            RelationAuthority::Evidence => (
                PRACTICE_TRANSITION_METHOD,
                vec![GeneratedEvidenceRef::PracticeTransitionAggregate {
                    from: source.to_owned(),
                    to: target.to_owned(),
                    basis: PracticeEvidenceBasis::Unspecified,
                    count: None,
                }],
            ),
        }
    };
    GeneratedClaimProvenance {
        original_from: source.to_owned(),
        original_to: target.to_owned(),
        original_kind: kind,
        original_explanation: explanation.to_owned(),
        method,
        evidence,
    }
}
