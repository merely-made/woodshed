//! Keyed musical facts used by comparison and contextual Stage projections.
//!
//! Catalog graph identities name formulas only. A sounded chord or scale also
//! needs its tonic, so this module carries that second, stable identity without
//! assigning spatial meaning or changing catalog relations.

use std::collections::BTreeSet;

use woodshedding::chord::catalog as chord_catalog;
use woodshedding::pitch::{Pitch, PitchClass, Spelling};
use woodshedding::rehearsal::{
    ApproachDirection, ArpeggioDirection, Card, CardId, Material, ScalePattern, Touch,
};
use woodshedding::scale::catalog as scale_catalog;

pub use woodshedding::pitch_class_set::{PitchClassMotion, PitchClassMove, PitchSetComparison};

/// A catalog chord or scale formula at one tonic.
///
/// `formula_id` is the existing stable catalog id (`chord:Major`,
/// `scale:Dorian`); [`Self::wire_key`] adds the tonic without turning a
/// keyed realization into a new catalog formula.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyedCatalogRef {
    pub formula_id: String,
    pub root: PitchClass,
}

impl KeyedCatalogRef {
    /// Preserve the sequential catalog identity carried by chord + touch.
    pub fn from_card(card: &Card) -> Option<Self> {
        let mut keyed = Self::from_material(&card.material)?;
        if matches!(card.material, Material::Chord { .. })
            && matches!(card.touch, Touch::Arpeggiate { .. })
        {
            keyed.formula_id = keyed.formula_id.replacen("chord:", "arpeggio:", 1);
        }
        Some(keyed)
    }

    /// Catalog preview with the correct articulation; occurrence assigned by Set.
    pub fn to_card(&self) -> Option<Card> {
        Some(Card {
            id: CardId::UNASSIGNED,
            label: self.label()?,
            material: self.to_material()?,
            setting: Default::default(),
            touch: if self.formula_id.starts_with("arpeggio:") {
                Touch::Arpeggiate {
                    direction: ArpeggioDirection::UpDown,
                    inversion: 0,
                }
            } else if self.formula_id.starts_with("scale-pattern:")
                || self.formula_id.starts_with("chord-approach:")
            {
                Touch::Walk
            } else {
                Touch::Block
            },
            timing: Default::default(),
            from: None,
        })
    }

    /// Name a keyed chord or scale material, rejecting paths, riffs, and
    /// materials whose formula no longer exists in the catalog.
    pub fn from_material(material: &Material) -> Option<Self> {
        match material {
            Material::ChordApproach {
                name,
                root,
                direction,
            } if chord_catalog().iter().any(|formula| formula.name == name) => Some(Self {
                formula_id: woodshed_graph::chord_approach_id(name, *direction),
                root: *root,
            }),
            Material::Chord { name, root }
                if chord_catalog().iter().any(|formula| formula.name == name) =>
            {
                Some(Self {
                    formula_id: woodshed_graph::chord_id(name),
                    root: *root,
                })
            },
            Material::Scale { name, root }
                if scale_catalog().iter().any(|formula| formula.name == name) =>
            {
                Some(Self {
                    formula_id: woodshed_graph::scale_id(name),
                    root: *root,
                })
            },
            Material::ScalePattern {
                name,
                root,
                pattern,
            } if scale_catalog().iter().any(|formula| formula.name == name) => Some(Self {
                formula_id: woodshed_graph::scale_pattern_id(name, *pattern),
                root: *root,
            }),
            _ => None,
        }
    }

    /// Reconstruct the catalog-backed material, rejecting malformed or stale
    /// formula identifiers rather than silently substituting another formula.
    pub fn to_material(&self) -> Option<Material> {
        let (kind, name) = self.formula_id.split_once(':')?;
        if kind == "chord-approach" {
            let (slug, name) = name.split_once(':')?;
            let direction = ApproachDirection::from_slug(slug)?;
            return chord_catalog()
                .iter()
                .any(|formula| formula.name == name)
                .then(|| Material::ChordApproach {
                    name: name.into(),
                    root: self.root,
                    direction,
                });
        }
        if kind == "scale-pattern" {
            let (slug, name) = name.split_once(':')?;
            let pattern = ScalePattern::from_slug(slug)?;
            return scale_catalog()
                .iter()
                .any(|formula| formula.name == name)
                .then(|| Material::ScalePattern {
                    name: name.into(),
                    root: self.root,
                    pattern,
                });
        }
        match kind {
            "chord" | "arpeggio" if chord_catalog().iter().any(|formula| formula.name == name) => {
                Some(Material::Chord {
                    name: name.to_string(),
                    root: self.root,
                })
            },
            "scale" if scale_catalog().iter().any(|formula| formula.name == name) => {
                Some(Material::Scale {
                    name: name.to_string(),
                    root: self.root,
                })
            },
            _ => None,
        }
    }

    /// A stable key for scene, view, and wire boundaries.
    pub fn wire_key(&self) -> String {
        format!("{}@pc:{}", self.formula_id, self.root.value())
    }

    /// A compact player-facing label using the deterministic sharp spelling.
    pub fn label(&self) -> Option<String> {
        let material = self.to_material()?;
        let name = match material {
            Material::Chord { name, .. } | Material::Scale { name, .. } => name,
            Material::ScalePattern { name, pattern, .. } => {
                format!("{name} in {}", pattern.label().to_lowercase())
            },
            Material::ChordApproach {
                name, direction, ..
            } => format!("{name} approach from {}", direction.slug()),
            Material::Riff { .. } | Material::Path { .. } => return None,
        };
        let root = Pitch::from_midi(60 + i32::from(self.root.value()), Spelling::Sharps);
        let suffix = if self.formula_id.starts_with("arpeggio:") {
            " arpeggio"
        } else {
            ""
        };
        Some(format!("{}{} {}{suffix}", root.name, root.accidental, name))
    }

    /// Unique sounding pitch classes for this keyed formula.
    pub fn pitch_classes(&self) -> Option<BTreeSet<PitchClass>> {
        keyed_pitch_classes(&self.to_material()?)
    }
}

/// Unique sounding pitch classes for a catalog-backed chord or scale.
///
/// The formula is deliberately resolved by name each time. A stale saved name
/// yields `None`, never the first catalog formula.
pub fn keyed_pitch_classes(material: &Material) -> Option<BTreeSet<PitchClass>> {
    if let Material::ChordApproach {
        name,
        root,
        direction,
    } = material
    {
        let target = keyed_pitch_classes(&Material::Chord {
            name: name.clone(),
            root: *root,
        })?;
        let mut recipe = target.clone();
        recipe.extend(target.iter().map(|pc| {
            PitchClass::new((i16::from(pc.value()) + direction.fret_delta()).rem_euclid(12) as u8)
        }));
        return Some(recipe);
    }
    let pitches = match material {
        Material::Chord { name, root } | Material::ChordApproach { name, root, .. } => {
            let formula = chord_catalog()
                .iter()
                .find(|formula| formula.name == name)?;
            formula.apply_to(root_pitch(*root)).ok()?
        },
        Material::Scale { name, root } | Material::ScalePattern { name, root, .. } => {
            let formula = scale_catalog()
                .iter()
                .find(|formula| formula.name == name)?;
            formula.apply_to(root_pitch(*root)).ok()?
        },
        Material::Riff { .. } | Material::Path { .. } => return None,
    };
    Some(
        pitches
            .into_iter()
            .map(|pitch| PitchClass::new(pitch.pitch_class() as u8))
            .collect(),
    )
}

fn root_pitch(root: PitchClass) -> Pitch {
    Pitch::from_midi(60 + i32::from(root.value()), Spelling::Sharps)
}

/// Compare two keyed catalog realizations. Unknown or unsupported material is
/// inapplicable and returns `None`.
pub fn compare_pitch_sets(
    left: &KeyedCatalogRef,
    right: &KeyedCatalogRef,
) -> Option<PitchSetComparison> {
    let left_set = left.pitch_classes()?;
    let right_set = right.pitch_classes()?;
    Some(woodshedding::pitch_class_set::compare(
        &left_set, &right_set,
    ))
}

/// Minimum octave-free pitch-class assignment for two keyed materials.
///
/// This does not assign registered voices or fingers; it is applicable only
/// when the concrete pitch-class sets have equal, nonzero cardinality.
pub fn compare_pitch_motion(
    left: &KeyedCatalogRef,
    right: &KeyedCatalogRef,
) -> Option<PitchClassMotion> {
    woodshedding::pitch_class_set::minimum_motion(&left.pitch_classes()?, &right.pitch_classes()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(name: &str, root: u8) -> Material {
        Material::Chord {
            name: name.to_string(),
            root: PitchClass::new(root),
        }
    }

    #[test]
    fn c_major_and_a_minor_keep_shared_and_exclusive_tones() {
        let c_major = KeyedCatalogRef::from_material(&chord("Major", 0)).unwrap();
        let a_minor = KeyedCatalogRef::from_material(&chord("Minor", 9)).unwrap();
        let comparison = compare_pitch_sets(&c_major, &a_minor).unwrap();

        assert_eq!(
            comparison.shared,
            BTreeSet::from([PitchClass::new(0), PitchClass::new(4)])
        );
        assert_eq!(comparison.left_only, BTreeSet::from([PitchClass::new(7)]));
        assert_eq!(comparison.right_only, BTreeSet::from([PitchClass::new(9)]));
    }

    #[test]
    fn same_formula_at_different_roots_has_distinct_key_and_pitch_set() {
        let c_major = KeyedCatalogRef::from_material(&chord("Major", 0)).unwrap();
        let d_major = KeyedCatalogRef::from_material(&chord("Major", 2)).unwrap();

        assert_ne!(c_major, d_major);
        assert_ne!(c_major.wire_key(), d_major.wire_key());
        assert_ne!(c_major.pitch_classes(), d_major.pitch_classes());
        assert!(
            compare_pitch_sets(&c_major, &d_major)
                .unwrap()
                .shared
                .is_empty()
        );
    }

    #[test]
    fn malformed_or_unknown_formula_is_rejected() {
        let unknown = chord("Not in catalog", 0);
        assert!(KeyedCatalogRef::from_material(&unknown).is_none());
        assert!(keyed_pitch_classes(&unknown).is_none());

        let malformed = KeyedCatalogRef {
            formula_id: "chord:Not in catalog".into(),
            root: PitchClass::new(0),
        };
        assert!(malformed.to_material().is_none());
        assert!(malformed.label().is_none());
        assert!(malformed.pitch_classes().is_none());
    }

    #[test]
    fn keyed_motion_uses_the_concrete_pitch_sets() {
        let c_major = KeyedCatalogRef::from_material(&chord("Major", 0)).unwrap();
        let e_minor = KeyedCatalogRef::from_material(&chord("Minor", 4)).unwrap();
        let motion = compare_pitch_motion(&c_major, &e_minor).unwrap();
        assert_eq!(motion.total_semitones, 1);
        assert_eq!(
            motion.held,
            BTreeSet::from([PitchClass::new(4), PitchClass::new(7)])
        );
    }

    #[test]
    fn arpeggio_identity_retains_root_tones_and_touch_without_new_material() {
        let reference = KeyedCatalogRef {
            formula_id: "arpeggio:Major 7".into(),
            root: PitchClass::new(2),
        };
        let card = reference.to_card().unwrap();
        assert!(matches!(card.material, Material::Chord { .. }));
        assert!(matches!(card.touch, Touch::Arpeggiate { .. }));
        assert_eq!(KeyedCatalogRef::from_card(&card), Some(reference.clone()));
        let chord = KeyedCatalogRef::from_material(&card.material).unwrap();
        assert_ne!(chord, reference);
        assert_eq!(chord.pitch_classes(), reference.pitch_classes());
        assert_eq!(reference.wire_key(), "arpeggio:Major 7@pc:2");
        assert!(reference.label().unwrap().ends_with(" arpeggio"));
    }
}
