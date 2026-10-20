//! Selected chord-shape resolution for rehearsal cards.

use super::*;

pub use woodshedding::card_shapes::{
    CARD_SHAPE_PROFILE, CardShapeGeometry, CardShapeStatus, CardShapeUnavailable,
    ResolvedCardShape, ResolvedShapeSetup,
};
use woodshedding::card_shapes::{CardShapeCandidates, ShapeContext};

#[derive(Clone, Debug)]
pub(crate) struct CardShapeCache {
    key: String,
    candidates: Result<CardShapeCandidates, CardShapeUnavailable>,
}

impl StageState {
    fn card_shape_cache_key(&self, card: &Card) -> String {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            card.material,
            card.setting.instrument,
            card.setting.tuning,
            card.setting.capo,
            card.setting.fret_window,
            (self.tuning(), self.fret_start, self.fret_count)
        )
    }

    fn card_shape_candidates(
        &self,
        card: &Card,
    ) -> Result<CardShapeCandidates, CardShapeUnavailable> {
        let key = self.card_shape_cache_key(card);
        {
            let mut cache = self.card_shape_cache.borrow_mut();
            if let Some(index) = cache.iter().position(|entry| entry.key == key) {
                let entry = cache.remove(index);
                let candidates = entry.candidates.clone();
                cache.push(entry);
                return candidates;
            }
        }
        let candidates = self.compute_card_shape_candidates(card);
        let mut cache = self.card_shape_cache.borrow_mut();
        if cache.len() >= 2 {
            cache.remove(0);
        }
        cache.push(CardShapeCache {
            key,
            candidates: candidates.clone(),
        });
        candidates
    }

    fn shape_context(&self) -> ShapeContext {
        ShapeContext {
            tuning: self.tuning(),
            fret_start: self.fret_start,
            fret_count: self.fret_count,
        }
    }
    pub(crate) fn resolve_card_tuning(&self, card: &Card) -> Result<Tuning, CardShapeUnavailable> {
        self.shape_context().resolve_card_tuning(card)
    }
    fn compute_card_shape_candidates(
        &self,
        card: &Card,
    ) -> Result<CardShapeCandidates, CardShapeUnavailable> {
        self.shape_context().inventory(card)
    }
    pub(crate) fn selected_card_shape(&self, card: &Card) -> CardShapeStatus {
        // Retain the application's small inventory cache.
        if !matches!(card.material, Material::Chord { .. }) {
            return CardShapeStatus::NotChord;
        }
        if card.setting.voicing_idx.is_none() {
            return CardShapeStatus::Unselected;
        }
        match self.card_shape_candidates(card) {
            Ok(inventory) => inventory.selected(card),
            Err(error) => CardShapeStatus::Unavailable(error),
        }
    }

    /// Current selected-shape state. Views can show `Unavailable` verbatim and
    /// must not replace it with an unrelated legacy shape.
    pub fn card_shape_status(&self, card: &Card) -> CardShapeStatus {
        self.selected_card_shape(card)
    }

    /// Geometry for a retained Path or selected chord shape, including an
    /// invalid chord ordinal when its setup itself resolves. Other unselected
    /// material retains the ordinary live-board geometry.
    pub fn card_shape_geometry(&self, card: &Card) -> Option<CardShapeGeometry> {
        if matches!(card.material, Material::Path { .. }) {
            // Path notes already use this retained tuning resolver. The neck
            // must use its string count too; the live window clips display.
            return Some(match self.resolve_card_tuning(card) {
                Ok(tuning) => {
                    let limit = tuning.instrument.standard_fret_count();
                    let start = self.fret_start.min(limit);
                    CardShapeGeometry {
                        string_count: tuning.string_count(),
                        physical_fret_start: start,
                        physical_fret_end: self.fret_count.min(limit).max(start),
                        capo: card.setting.capo.unwrap_or(0),
                    }
                },
                Err(_) => CardShapeGeometry::EMPTY,
            });
        }
        if !matches!(card.material, Material::Chord { .. }) || card.setting.voicing_idx.is_none() {
            return None;
        }
        self.card_shape_candidates(card)
            .ok()
            .map(|candidates| candidates.geometry)
    }

    /// The selected card setup's physical fret extent. This resolves tuning
    /// identity only, so callers may constrain a card window without paying
    /// for shape enumeration.
    pub fn card_fret_limit(&self, card: &Card) -> Result<u8, CardShapeUnavailable> {
        Ok(self
            .resolve_card_tuning(card)?
            .instrument
            .standard_fret_count())
    }

    /// Number of candidates in the setup's deterministic inventory. Check
    /// [`CardShapeStatus::Available`] for `limited_inventory` before
    /// presenting this as a complete neck-wide inventory.
    pub fn card_shape_count(&self, card: &Card) -> Result<usize, CardShapeUnavailable> {
        Ok(self.card_shape_candidates(card)?.voicings.len())
    }

    /// Select the next inventory candidate, persisting both its ordinal and the
    /// fingerprint/profile needed to detect later search-policy drift.
    pub fn select_next_card_shape(
        &self,
        card: &mut Card,
    ) -> Result<CardShapeStatus, CardShapeUnavailable> {
        let candidates = self.card_shape_candidates(card)?;
        let count = candidates.voicings.len();
        let index = card
            .setting
            .voicing_idx
            .map(|index| (index % count).saturating_add(1) % count)
            .unwrap_or(0);
        self.select_card_shape_index(card, index)
    }

    /// Select the preceding inventory candidate. From the all-tone map this
    /// chooses the final candidate, matching cyclic UI navigation.
    pub fn select_previous_card_shape(
        &self,
        card: &mut Card,
    ) -> Result<CardShapeStatus, CardShapeUnavailable> {
        let candidates = self.card_shape_candidates(card)?;
        let count = candidates.voicings.len();
        let index = card.setting.voicing_idx.map_or(count - 1, |index| {
            (index % count).checked_sub(1).unwrap_or(count - 1)
        });
        self.select_card_shape_index(card, index)
    }

    /// Select one candidate by its current bounded enumeration ordinal.
    pub fn select_card_shape_index(
        &self,
        card: &mut Card,
        index: usize,
    ) -> Result<CardShapeStatus, CardShapeUnavailable> {
        let candidates = self.card_shape_candidates(card)?;
        let voicing =
            candidates
                .voicings
                .get(index)
                .ok_or(CardShapeUnavailable::IndexOutOfRange {
                    selected: index,
                    count: candidates.voicings.len(),
                })?;
        card.setting.voicing_idx = Some(index);
        card.setting.voicing_fingerprint =
            Some(woodshedding::card_shapes::shape_fingerprint(voicing));
        card.setting.voicing_profile = Some(candidates.profile.to_string());
        match self.selected_card_shape(card) {
            CardShapeStatus::Available(shape) => Ok(CardShapeStatus::Available(shape)),
            CardShapeStatus::Unavailable(reason) => Err(reason),
            CardShapeStatus::NotChord => Err(CardShapeUnavailable::NotChord),
            CardShapeStatus::Unselected => Err(CardShapeUnavailable::NoShapes),
        }
    }

    /// Return a chord card to its documented all-tone-map state.
    pub fn clear_card_shape(&self, card: &mut Card) {
        card.setting.voicing_idx = None;
        card.setting.voicing_fingerprint = None;
        card.setting.voicing_profile = None;
    }

    pub(crate) fn selected_card_shape_dots(&self, card: &Card) -> Option<Vec<FretDot>> {
        let CardShapeStatus::Available(shape) = self.selected_card_shape(card) else {
            return None;
        };
        Some(
            shape
                .voicing
                .strings
                .iter()
                .enumerate()
                .filter_map(|(string_index, play)| match play {
                    StringPlay::Played {
                        fret,
                        pitch,
                        interval_from_root,
                    } => {
                        let semis = interval_from_root.map(|interval| interval.semitones());
                        Some(FretDot {
                            string_index,
                            fret: fret.saturating_add(shape.geometry.capo),
                            is_root: semis == Some(0),
                            label: format!("{}{}", pitch.name, pitch.accidental),
                            octave: pitch.octave,
                            degree: semis.map(degree_label).unwrap_or("").to_string(),
                            interval_name: semis.map(interval_name).unwrap_or("").to_string(),
                            frequency: pitch.frequency() as f32,
                        })
                    },
                    StringPlay::Muted => None,
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::card_shapes::CARD_SHAPE_LIMITED_PROFILE;

    fn c_major_card() -> Card {
        Card {
            id: CardId::UNASSIGNED,
            label: "C Major".to_string(),
            material: Material::Chord {
                name: "Major".to_string(),
                root: PitchClass::new(0),
            },
            setting: Setting::default(),
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        }
    }

    #[test]
    fn unselected_chord_keeps_legacy_map_and_formula_preview() {
        let state = StageState::new();
        let card = c_major_card();
        assert!(matches!(
            state.card_shape_status(&card),
            CardShapeStatus::Unselected
        ));
        assert!(
            state.dots_for_card(&card).len() > 3,
            "all-tone chord map remains"
        );
        assert_eq!(
            state.card_voicing(&card).unwrap().0.len(),
            3,
            "formula preview remains"
        );
    }

    #[test]
    fn selected_shape_unifies_dots_and_concert_audition() {
        let state = StageState::new();
        let mut card = c_major_card();
        card.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        let status = state.select_next_card_shape(&mut card).expect("shape fits");
        let CardShapeStatus::Available(shape) = status else {
            panic!("selected shape is available")
        };
        assert!(!shape.limited_inventory);
        assert_eq!(
            card.setting.voicing_fingerprint.as_deref(),
            Some(shape.fingerprint.as_str())
        );
        assert_eq!(
            card.setting.voicing_profile.as_deref(),
            Some(CARD_SHAPE_PROFILE)
        );
        let dots = state.dots_for_card(&card);
        assert_eq!(
            dots.iter()
                .map(|dot| (dot.string_index, dot.fret))
                .collect::<Vec<_>>(),
            shape.physical_positions()
        );
        let heard = state.card_voicing(&card).unwrap().0;
        let expected: Vec<f32> = shape
            .concert_pitches()
            .into_iter()
            .map(|pitch| pitch.frequency() as f32)
            .collect();
        assert_eq!(heard, expected);
    }

    #[test]
    fn capo_keeps_the_c_shape_and_sounds_d_concert() {
        let state = StageState::new();
        let mut card = c_major_card();
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 0, span: 5 });
        let CardShapeStatus::Available(shape) = state
            .select_next_card_shape(&mut card)
            .expect("capoed shape")
        else {
            panic!("capoed shape is available")
        };
        assert_eq!(shape.geometry.capo, 2);
        assert!(
            shape
                .physical_positions()
                .iter()
                .all(|(_, fret)| *fret >= 2)
        );
        let pcs: std::collections::BTreeSet<u8> = shape
            .concert_pitches()
            .into_iter()
            .map(|pitch| pitch.pitch_class())
            .collect();
        assert_eq!(
            pcs,
            [2, 6, 9].into_iter().collect(),
            "C shape capo 2 sounds D"
        );
    }

    #[test]
    fn selected_capoed_shape_applies_marks_to_physical_contacts() {
        let state = StageState::new();
        let mut card = c_major_card();
        card.setting.capo = Some(2);
        card.setting.fret_window = Some(FretWindow { start: 0, span: 5 });
        state
            .select_next_card_shape(&mut card)
            .expect("capoed shape fits");
        let target = state
            .dots_for_card(&card)
            .into_iter()
            .next()
            .expect("selected shape has a sounding contact");
        card.setting.marked = vec![(target.string_index, target.fret)];

        card.setting.mark_mode = MarkMode::Solo;
        let solo = state.card_sounding_pitches(&card).unwrap().0;
        assert_eq!(solo.len(), 1, "solo keeps the selected physical contact");
        assert!((solo[0] - target.frequency).abs() < 0.5);

        card.setting.mark_mode = MarkMode::Mute;
        let muted = state.card_sounding_pitches(&card).unwrap().0;
        let target_pc = super::pc_from_hz(target.frequency);
        assert!(
            muted
                .iter()
                .all(|pitch| super::pc_from_hz(*pitch) != target_pc)
        );
    }

    #[test]
    fn candidate_cache_re_resolves_after_capo_change() {
        let state = StageState::new();
        let mut card = c_major_card();
        card.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        state
            .select_next_card_shape(&mut card)
            .expect("open shape fits");
        assert_eq!(
            state
                .card_shape_geometry(&card)
                .expect("selected geometry")
                .capo,
            0
        );

        card.setting.capo = Some(2);
        // The pinned physical 0..4 window now only leaves relative frets 0..2,
        // too short for this C shape. Moving the window with the capo restores
        // the original relative 0..4 inventory and must invalidate that miss.
        assert!(matches!(
            state.card_shape_status(&card),
            CardShapeStatus::Unavailable(CardShapeUnavailable::NoShapes)
        ));
        card.setting.fret_window = Some(FretWindow { start: 2, span: 4 });
        assert_eq!(
            state
                .card_shape_geometry(&card)
                .expect("capo change invalidates candidate cache")
                .capo,
            2
        );
        let heard_pcs: std::collections::BTreeSet<u8> = state
            .card_voicing(&card)
            .unwrap()
            .0
            .into_iter()
            .map(super::pc_from_hz)
            .collect();
        assert_eq!(heard_pcs, [2, 6, 9].into_iter().collect());
    }

    #[test]
    fn full_neck_next_uses_a_declared_limited_inventory() {
        let state = StageState::new();
        let mut card = c_major_card();
        let CardShapeStatus::Available(shape) = state
            .select_next_card_shape(&mut card)
            .expect("default neck has shapes")
        else {
            panic!("shape is available")
        };
        assert!(shape.limited_inventory);
        assert_eq!(shape.profile, CARD_SHAPE_LIMITED_PROFILE);
    }

    #[test]
    fn pinned_window_replaces_the_live_window() {
        let mut state = StageState::new();
        state.fret_start = 8;
        state.fret_count = 16;
        let mut card = c_major_card();
        card.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        let CardShapeStatus::Available(shape) = state
            .select_next_card_shape(&mut card)
            .expect("pinned open position resolves")
        else {
            panic!("shape is available")
        };
        assert_eq!(
            (
                shape.geometry.physical_fret_start,
                shape.geometry.physical_fret_end
            ),
            (0, 4)
        );
    }

    #[test]
    fn explicit_instrument_fret_limit_ignores_live_guitar_viewport() {
        let mut state = StageState::new();
        state.fret_count = 7;
        let mut card = c_major_card();
        card.setting.instrument = "Ukulele".to_string();

        assert_eq!(state.card_fret_limit(&card), Ok(15));

        card.setting.instrument = "Unknown lute".to_string();
        assert!(matches!(
            state.card_fret_limit(&card),
            Err(CardShapeUnavailable::UnknownInstrument(value)) if value == "Unknown lute"
        ));
    }

    #[test]
    fn stale_index_or_fingerprint_never_substitutes_another_shape() {
        let state = StageState::new();
        let mut card = c_major_card();
        card.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        card.setting.voicing_idx = Some(usize::MAX);
        assert!(matches!(
            state.card_shape_status(&card),
            CardShapeStatus::Unavailable(CardShapeUnavailable::IndexOutOfRange { .. })
        ));
        card.setting.voicing_idx = Some(0);
        card.setting.voicing_fingerprint = Some("x-x-x".to_string());
        assert!(matches!(
            state.card_shape_status(&card),
            CardShapeStatus::Unavailable(CardShapeUnavailable::FingerprintChanged { .. })
        ));
    }

    #[test]
    fn navigation_recovers_from_a_stale_persisted_ordinal() {
        let state = StageState::new();
        let mut card = c_major_card();
        card.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        let count = state.card_shape_count(&card).expect("shape inventory");

        card.setting.voicing_idx = Some(usize::MAX);
        assert!(matches!(
            state
                .select_previous_card_shape(&mut card)
                .expect("previous normalizes a stale ordinal"),
            CardShapeStatus::Available(_)
        ));
        assert!(card.setting.voicing_idx.expect("selected") < count);

        card.setting.voicing_idx = Some(usize::MAX);
        assert!(matches!(
            state
                .select_next_card_shape(&mut card)
                .expect("next normalizes a stale ordinal"),
            CardShapeStatus::Available(_)
        ));
        assert!(card.setting.voicing_idx.expect("selected") < count);
    }

    #[test]
    fn two_card_cache_retains_recent_comparison_setups_and_evicts_oldest() {
        let state = StageState::new();
        let mut first = c_major_card();
        first.setting.fret_window = Some(FretWindow { start: 0, span: 4 });
        let mut second = c_major_card();
        second.setting.fret_window = Some(FretWindow { start: 1, span: 4 });
        let mut third = c_major_card();
        third.setting.fret_window = Some(FretWindow { start: 2, span: 4 });

        let first_key = state.card_shape_cache_key(&first);
        let second_key = state.card_shape_cache_key(&second);
        let third_key = state.card_shape_cache_key(&third);
        let _ = state.card_shape_candidates(&first);
        let _ = state.card_shape_candidates(&second);
        assert_eq!(state.card_shape_cache.borrow().len(), 2);

        let _ = state.card_shape_candidates(&first);
        assert_eq!(
            state
                .card_shape_cache
                .borrow()
                .last()
                .map(|entry| &entry.key),
            Some(&first_key)
        );
        let _ = state.card_shape_candidates(&third);
        let keys = state
            .card_shape_cache
            .borrow()
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec![first_key, third_key]);
        assert!(!keys.contains(&second_key));
    }
}
