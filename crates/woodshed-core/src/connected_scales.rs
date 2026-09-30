//! Rooted chord-to-scale formula containment, with explicit Set authoring.
//!
//! These queries describe catalog formulas. A selected chord shape and capo
//! are retained as instrument context, but do not reinterpret the material's
//! root or establish which scale fingering is suitable for a player.
//! Audition resolves the preview Card through the shared scale realization.
//! Its saved setup determines concert pitches and physical contacts; the
//! ascending traversal is not a fingering solver or synchronized note events.

use woodshedding::pitch::PitchClass;
use woodshedding::rehearsal::{Card, CardId, MarkMode, Material, Set, Touch};

use crate::{StageState, harmony::KeyedCatalogRef};

#[derive(Clone, Debug)]
pub struct ChordScaleDiscovery {
    pub source_card_id: CardId,
    pub subject: KeyedCatalogRef,
    pub label: String,
    pub explanation: String,
    pub preview: Card,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScaleDiscoveryUnavailable {
    NotChord,
    UnknownChord(String),
    NotScale(String),
    UnknownScale(String),
    DoesNotContainChord(KeyedCatalogRef),
    MissingSource(CardId),
}

impl std::fmt::Display for ScaleDiscoveryUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotChord => f.write_str("select a chord to discover containing scales"),
            Self::UnknownChord(name) => write!(f, "unknown chord: {name}"),
            Self::NotScale(id) => write!(f, "candidate is not a scale: {id}"),
            Self::UnknownScale(id) => write!(f, "unknown scale: {id}"),
            Self::DoesNotContainChord(subject) => write!(
                f,
                "{} does not contain all rooted chord formula tones",
                subject.label().unwrap_or_else(|| subject.wire_key())
            ),
            Self::MissingSource(_) => f.write_str("the source chord is no longer in the Set"),
        }
    }
}
impl std::error::Error for ScaleDiscoveryUnavailable {}

fn source_subject(source: &Card) -> Result<KeyedCatalogRef, ScaleDiscoveryUnavailable> {
    let Material::Chord { name, .. } = &source.material else {
        return Err(ScaleDiscoveryUnavailable::NotChord);
    };
    KeyedCatalogRef::from_material(&source.material)
        .ok_or_else(|| ScaleDiscoveryUnavailable::UnknownChord(name.clone()))
}

impl StageState {
    /// Enumerate exact containment across all known scale formulas and roots.
    /// The limit is a disclosure budget; it never changes the Set or source.
    /// Discovery prefers the same root, then the named Major/Minor scales,
    /// then fewer added tones and deterministic name/root order. The named
    /// preference aids a bounded list; it is not a musical quality judgment.
    pub fn chord_scale_discoveries(
        &self,
        source: &Card,
        limit: usize,
    ) -> Result<Vec<ChordScaleDiscovery>, ScaleDiscoveryUnavailable> {
        let chord = source_subject(source)?;
        let chord_tones = chord.pitch_classes().expect("validated catalog chord");
        let mut candidates = Vec::new();
        for formula in woodshedding::scale::catalog() {
            for root in 0..12 {
                let subject = KeyedCatalogRef {
                    formula_id: woodshed_graph::scale_id(formula.name),
                    root: PitchClass::new(root),
                };
                let Some(scale_tones) = subject.pitch_classes() else {
                    continue;
                };
                if chord_tones.is_subset(&scale_tones) {
                    candidates.push((
                        subject.root != chord.root,
                        !matches!(formula.name, "Major" | "Minor"),
                        scale_tones.len() - chord_tones.len(),
                        formula.name,
                        subject.root.value(),
                        subject,
                    ));
                }
            }
        }
        candidates.sort_by(|left, right| {
            (&left.0, &left.1, &left.2, &left.3, &left.4)
                .cmp(&(&right.0, &right.1, &right.2, &right.3, &right.4))
        });
        candidates
            .into_iter()
            .take(limit)
            .map(|(_, _, _, _, _, subject)| self.chord_scale_discovery(source, &subject))
            .collect()
    }

    /// Revalidate an explicitly chosen keyed scale against the current chord.
    /// Formula roots govern the relation, independently of selected-shape sound.
    pub fn chord_scale_discovery(
        &self,
        source: &Card,
        subject: &KeyedCatalogRef,
    ) -> Result<ChordScaleDiscovery, ScaleDiscoveryUnavailable> {
        let chord = source_subject(source)?;
        if !subject.formula_id.starts_with("scale:") {
            return Err(ScaleDiscoveryUnavailable::NotScale(
                subject.formula_id.clone(),
            ));
        }
        let material = subject
            .to_material()
            .ok_or_else(|| ScaleDiscoveryUnavailable::UnknownScale(subject.formula_id.clone()))?;
        let chord_tones = chord.pitch_classes().expect("validated catalog chord");
        let scale_tones = subject.pitch_classes().expect("validated catalog scale");
        if !chord_tones.is_subset(&scale_tones) {
            return Err(ScaleDiscoveryUnavailable::DoesNotContainChord(
                subject.clone(),
            ));
        }
        let label = subject.label().expect("validated catalog scale");
        let mut preview = source.clone();
        preview.id = CardId::UNASSIGNED;
        preview.label = label.clone();
        preview.material = material;
        preview.touch = Touch::Walk;
        preview.setting.voicing_idx = None;
        preview.setting.voicing_fingerprint = None;
        preview.setting.voicing_profile = None;
        preview.setting.marked.clear();
        preview.setting.mark_mode = MarkMode::Off;
        let explanation = format!(
            "{label} contains all {} tones of {} and adds {} pitch classes. Scale audition resolves the saved instrument, tuning, capo, and fret window. The ascending traversal does not prescribe a fingering. Setup and timing are retained; chord shape and marked notes are cleared.",
            chord_tones.len(),
            chord.label().expect("validated catalog chord"),
            scale_tones.len() - chord_tones.len(),
        );
        Ok(ChordScaleDiscovery {
            source_card_id: source.id,
            subject: subject.clone(),
            label,
            explanation,
            preview,
        })
    }

    /// Explicitly append a fresh occurrence after revalidating the live source.
    pub fn stage_chord_scale(
        &self,
        set: &mut Set,
        source_id: CardId,
        subject: &KeyedCatalogRef,
    ) -> Result<CardId, ScaleDiscoveryUnavailable> {
        let source = set
            .cards
            .iter()
            .find(|card| card.id == source_id)
            .ok_or(ScaleDiscoveryUnavailable::MissingSource(source_id))?;
        let discovery = self.chord_scale_discovery(source, subject)?;
        set.push(discovery.preview);
        Ok(set.cards.last().expect("push adds a card").id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::rehearsal::{FretWindow, Hold, Recipe, Setting, Timing};

    fn source(root: u8) -> Card {
        Card {
            id: CardId(7),
            label: "Source major chord".into(),
            material: Material::Chord {
                name: "Major".into(),
                root: PitchClass::new(root),
            },
            setting: Setting {
                instrument: "Ukulele".into(),
                tuning: Some("Standard".into()),
                capo: Some(2),
                fret_window: Some(FretWindow { start: 2, span: 4 }),
                voicing_idx: Some(12),
                voicing_fingerprint: Some("saved chord".into()),
                voicing_profile: Some("root-bass/v1".into()),
                marked: vec![(1, 3)],
                mark_mode: MarkMode::Solo,
            },
            touch: Touch::Block,
            timing: Timing {
                bpm: Some(83.0),
                hold: Hold::Bars(2),
            },
            from: Some(Recipe::Progression {
                name: "Recipe".into(),
                key: PitchClass::new(root),
            }),
        }
    }

    fn scale(name: &str, root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: woodshed_graph::scale_id(name),
            root: PitchClass::new(root),
        }
    }

    #[test]
    fn containment_is_rooted_and_does_not_infer_concert_or_same_name_membership() {
        let state = StageState::new();
        let card = source(0);
        assert!(
            state
                .chord_scale_discovery(&card, &scale("Major", 0))
                .is_ok()
        );
        assert!(
            state
                .chord_scale_discovery(&card, &scale("Major", 7))
                .is_ok()
        );
        assert!(matches!(
            state.chord_scale_discovery(&card, &scale("Major", 2)),
            Err(ScaleDiscoveryUnavailable::DoesNotContainChord(_))
        ));
        let d_major = source(2);
        assert!(
            state
                .chord_scale_discovery(&d_major, &scale("Major", 2))
                .is_ok()
        );
        assert!(
            state
                .chord_scale_discovery(&d_major, &scale("Major", 0))
                .is_err()
        );
    }

    #[test]
    fn preview_preserves_shared_context_but_resets_chord_local_selections() {
        let state = StageState::new();
        let card = source(0);
        let before = serde_json::to_value(&card).unwrap();
        let discovery = state
            .chord_scale_discovery(&card, &scale("Major", 0))
            .unwrap();
        let preview = serde_json::to_value(&discovery.preview).unwrap();
        assert_eq!(serde_json::to_value(&card).unwrap(), before);
        assert_eq!(discovery.preview.id, CardId::UNASSIGNED);
        assert_eq!(discovery.source_card_id, card.id);
        assert!(matches!(discovery.preview.touch, Touch::Walk));
        for field in ["instrument", "tuning", "capo", "fret_window"] {
            assert_eq!(preview["setting"][field], before["setting"][field]);
        }
        for field in ["voicing_idx", "voicing_fingerprint", "voicing_profile"] {
            assert!(preview["setting"][field].is_null());
        }
        assert!(discovery.preview.setting.marked.is_empty());
        assert_eq!(discovery.preview.setting.mark_mode, MarkMode::Off);
        assert_eq!(preview["timing"], before["timing"]);
        assert_eq!(preview["from"], before["from"]);
        assert!(discovery.explanation.contains("Scale audition"));
    }

    #[test]
    fn unknown_nonchord_and_non_scale_inputs_are_explicit() {
        let state = StageState::new();
        let mut card = source(0);
        assert!(matches!(
            state.chord_scale_discovery(
                &card,
                &KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(0)
                }
            ),
            Err(ScaleDiscoveryUnavailable::NotScale(_))
        ));
        assert!(matches!(
            state.chord_scale_discovery(&card, &scale("Missing", 0)),
            Err(ScaleDiscoveryUnavailable::UnknownScale(_))
        ));
        card.material = Material::Chord {
            name: "Missing".into(),
            root: PitchClass::new(0),
        };
        assert!(matches!(
            state.chord_scale_discoveries(&card, 0),
            Err(ScaleDiscoveryUnavailable::UnknownChord(_))
        ));
        card.material = Material::Scale {
            name: "Major".into(),
            root: PitchClass::new(0),
        };
        assert!(matches!(
            state.chord_scale_discoveries(&card, 3),
            Err(ScaleDiscoveryUnavailable::NotChord)
        ));
    }

    #[test]
    fn budget_is_deterministic_with_same_root_named_preference_then_added_tones() {
        let state = StageState::new();
        let card = source(0);
        let all = state.chord_scale_discoveries(&card, usize::MAX).unwrap();
        assert!(all.len() > 12);
        assert!(state.chord_scale_discoveries(&card, 0).unwrap().is_empty());
        let bounded = state.chord_scale_discoveries(&card, 5).unwrap();
        assert_eq!(
            bounded.iter().map(|item| &item.subject).collect::<Vec<_>>(),
            all.iter()
                .take(5)
                .map(|item| &item.subject)
                .collect::<Vec<_>>()
        );
        let key = |item: &ChordScaleDiscovery| {
            (
                item.subject.root != PitchClass::new(0),
                !matches!(
                    item.subject.formula_id.as_str(),
                    "scale:Major" | "scale:Minor"
                ),
                item.subject.pitch_classes().unwrap().len() - 3,
                item.subject.formula_id.clone(),
                item.subject.root.value(),
            )
        };
        assert!(all.windows(2).all(|pair| key(&pair[0]) <= key(&pair[1])));
    }

    #[test]
    fn bounded_major_seventh_list_includes_the_named_major_scale() {
        let state = StageState::new();
        let mut card = source(0);
        card.material = Material::Chord {
            name: "Major 7".into(),
            root: PitchClass::new(0),
        };
        let bounded = state.chord_scale_discoveries(&card, 4).unwrap();
        assert!(bounded.iter().any(|item| item.subject == scale("Major", 0)));
        assert_eq!(bounded[0].subject, scale("Major", 0));
    }

    #[test]
    fn staging_revalidates_source_and_reopening_preserves_distinct_occurrences() {
        let state = StageState::new();
        let mut set = Set::default();
        set.push(source(0));
        let source_id = set.cards[0].id;
        let chosen = scale("Major", 0);
        let first = state
            .stage_chord_scale(&mut set, source_id, &chosen)
            .unwrap();
        set.cards[0].timing.bpm = Some(97.0);
        let second = state
            .stage_chord_scale(&mut set, source_id, &chosen)
            .unwrap();
        assert_ne!(source_id, first);
        assert_ne!(first, second);
        assert_eq!(set.cards[1].timing.bpm, Some(83.0));
        assert_eq!(set.cards[2].timing.bpm, Some(97.0));
        set.cards[0].material = Material::Chord {
            name: "Major".into(),
            root: PitchClass::new(2),
        };
        assert!(matches!(
            state.stage_chord_scale(&mut set, source_id, &chosen),
            Err(ScaleDiscoveryUnavailable::DoesNotContainChord(_))
        ));
        assert_eq!(set.cards.len(), 3);
        set.remove(0);
        assert!(matches!(
            state.stage_chord_scale(&mut set, source_id, &chosen),
            Err(ScaleDiscoveryUnavailable::MissingSource(_))
        ));
        let reopened: Set = serde_json::from_str(&serde_json::to_string(&set).unwrap()).unwrap();
        assert_eq!(reopened.cards[0].id, first);
        assert_eq!(reopened.cards[1].id, second);
        assert_eq!(KeyedCatalogRef::from_card(&reopened.cards[0]), Some(chosen));
        assert!(matches!(
            reopened.cards[0].from,
            Some(Recipe::Progression { .. })
        ));
    }
}
