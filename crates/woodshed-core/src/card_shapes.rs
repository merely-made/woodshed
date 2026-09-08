//! Selected chord-shape resolution for rehearsal cards.

use super::*;

/// The bounded enumeration policy used for persisted chord-shape selections.
/// A changed policy must not silently reinterpret a saved ordinal.
pub const CARD_SHAPE_PROFILE: &str = "root-bass/v1";
const MAX_CARD_SHAPE_COMBINATIONS: usize = 50_000;
const MAX_CARD_SHAPES: usize = 128;
const CARD_SHAPE_LIMITED_PROFILE: &str = "root-bass/position4/v1";
const MAX_COMPLETE_SHAPE_SPAN: u8 = 4;

fn shape_fingerprint(voicing: &ChordVoicing) -> String {
    voicing
        .fret_pattern()
        .into_iter()
        .map(|fret| fret.map_or_else(|| "x".to_string(), |fret| fret.to_string()))
        .collect::<Vec<_>>()
        .join("-")
}

/// Why a card whose `voicing_idx` selects a shape cannot resolve it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardShapeUnavailable {
    NotChord,
    UnknownInstrument(String),
    UnknownTuning(String),
    AmbiguousLegacyTuning(String),
    NoInstrumentDefault(String),
    WindowBelowCapo,
    EmptyWindow,
    SearchLimit,
    NoShapes,
    IndexOutOfRange { selected: usize, count: usize },
    ProfileChanged { saved: String },
    FingerprintChanged { saved: String, found: String },
}

impl std::fmt::Display for CardShapeUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotChord => f.write_str("only chord cards have selected shapes"),
            Self::UnknownInstrument(value) => write!(f, "unknown instrument: {value}"),
            Self::UnknownTuning(value) => write!(f, "unknown tuning: {value}"),
            Self::AmbiguousLegacyTuning(value) => {
                write!(f, "ambiguous legacy tuning: {value}")
            },
            Self::NoInstrumentDefault(value) => write!(f, "no default tuning for {value}"),
            Self::WindowBelowCapo => f.write_str("the selected window is below the capo"),
            Self::EmptyWindow => f.write_str("the selected fret window has no playable frets"),
            Self::SearchLimit => f.write_str("pin a narrower fret window to browse shapes"),
            Self::NoShapes => f.write_str("no root-bass shape found in this search"),
            Self::IndexOutOfRange { selected, count } => {
                write!(
                    f,
                    "shape {} is unavailable; this setup has {count}",
                    selected.saturating_add(1)
                )
            },
            Self::ProfileChanged { .. } => f.write_str("saved shape uses an older search profile"),
            Self::FingerprintChanged { .. } => {
                f.write_str("saved shape no longer matches its fret pattern")
            },
        }
    }
}

impl std::error::Error for CardShapeUnavailable {}

/// The card-specific neck geometry a selected shape uses. Frets are physical
/// nut-relative coordinates; `capo` is retained so a view can distinguish the
/// capoed open position from the nut's open string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardShapeGeometry {
    pub string_count: usize,
    pub physical_fret_start: u8,
    pub physical_fret_end: u8,
    pub capo: u8,
}

/// The exact effective setup used to resolve a selected shape. The concert
/// open-string MIDI values protect comparison from treating two differently
/// pitched tunings with the same display name as interchangeable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedShapeSetup {
    pub instrument: Instrument,
    pub tuning_name: String,
    pub capo: u8,
    pub concert_open_midi: Vec<i32>,
}

/// A selected shape after resolving card setup. `voicing.strings` keeps frets
/// relative to the capo; dots and `physical_positions` use nut-relative frets.
#[derive(Clone, Debug)]
pub struct ResolvedCardShape {
    pub index: usize,
    pub count: usize,
    pub fingerprint: String,
    pub profile: &'static str,
    /// True when an unpinned wide neck was sampled through stable four-fret
    /// windows rather than exhaustively enumerated as one giant inventory.
    pub limited_inventory: bool,
    pub tuning_name: String,
    pub instrument: Instrument,
    pub setup: ResolvedShapeSetup,
    pub geometry: CardShapeGeometry,
    pub voicing: ChordVoicing,
}

impl ResolvedCardShape {
    pub fn physical_positions(&self) -> Vec<(usize, u8)> {
        self.voicing
            .strings
            .iter()
            .enumerate()
            .filter_map(|(string_index, play)| {
                play.fret()
                    .map(|fret| (string_index, fret.saturating_add(self.geometry.capo)))
            })
            .collect()
    }

    pub fn concert_pitches(&self) -> Vec<Pitch> {
        self.voicing
            .strings
            .iter()
            .filter_map(|play| play.pitch())
            .collect()
    }
}

/// View-facing selected-shape state. Unselected is intentional legacy card
/// behavior, while Unavailable is a selected shape that must not fall back to
/// a different one silently.
#[derive(Clone, Debug)]
pub enum CardShapeStatus {
    NotChord,
    Unselected,
    Available(ResolvedCardShape),
    Unavailable(CardShapeUnavailable),
}

#[derive(Clone, Debug)]
struct CardShapeCandidates {
    geometry: CardShapeGeometry,
    tuning_name: String,
    instrument: Instrument,
    setup: ResolvedShapeSetup,
    voicings: Vec<ChordVoicing>,
    limited_inventory: bool,
    profile: &'static str,
}

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

    /// Resolve a card's persisted setup without considering material or
    /// enumerating shapes. Empty instrument identity deliberately keeps the
    /// legacy live-or-unique-tuning behavior; explicit identity always wins.
    fn resolve_card_tuning(&self, card: &Card) -> Result<Tuning, CardShapeUnavailable> {
        let live = self.tuning();
        let explicit_instrument = if card.setting.instrument.is_empty() {
            None
        } else {
            Some(
                Instrument::from_identity(&card.setting.instrument).ok_or_else(|| {
                    CardShapeUnavailable::UnknownInstrument(card.setting.instrument.clone())
                })?,
            )
        };
        match (&card.setting.tuning, explicit_instrument) {
            (Some(name), Some(instrument)) => Tuning::find_for(name, instrument)
                .ok_or_else(|| CardShapeUnavailable::UnknownTuning(name.clone())),
            (None, Some(instrument)) => Tuning::default_for(instrument)
                .ok_or_else(|| CardShapeUnavailable::NoInstrumentDefault(instrument.to_string())),
            (Some(name), None) if live.name == *name => Ok(live),
            (Some(name), None) => {
                let matches: Vec<_> = tuning_catalog()
                    .iter()
                    .filter(|spec| spec.name == name)
                    .collect();
                match matches.as_slice() {
                    [spec] => Ok(Tuning::from_spec(spec)),
                    [] => Err(CardShapeUnavailable::UnknownTuning(name.clone())),
                    _ => Err(CardShapeUnavailable::AmbiguousLegacyTuning(name.clone())),
                }
            },
            (None, None) => Ok(live),
        }
    }

    fn compute_card_shape_candidates(
        &self,
        card: &Card,
    ) -> Result<CardShapeCandidates, CardShapeUnavailable> {
        let Material::Chord { name, root } = &card.material else {
            return Err(CardShapeUnavailable::NotChord);
        };
        let tuning = self.resolve_card_tuning(card)?;
        let capo = card.setting.capo.unwrap_or(0);
        let instrument_end = tuning.instrument.standard_fret_count();
        let mut physical_start = self.fret_start;
        let mut physical_end = self.fret_count.min(instrument_end);
        if let Some(window) = card.setting.fret_window {
            // A pinned Card position replaces the live viewport; only the
            // actual instrument range clips it.
            physical_start = window.start;
            physical_end = window.start.saturating_add(window.span).min(instrument_end);
        }
        if physical_end < capo {
            return Err(CardShapeUnavailable::WindowBelowCapo);
        }
        physical_start = physical_start.max(capo);
        if physical_start > physical_end {
            return Err(CardShapeUnavailable::EmptyWindow);
        }
        let geometry = CardShapeGeometry {
            string_count: tuning.string_count(),
            physical_fret_start: physical_start,
            physical_fret_end: physical_end,
            capo,
        };
        let relative_start = physical_start - capo;
        let relative_end = physical_end - capo;
        let formula = chord_catalog()
            .iter()
            .find(|formula| formula.name == name.as_str())
            .ok_or_else(|| CardShapeUnavailable::NoShapes)?;
        // Search the same physical shape after transposing both the open strings
        // and requested root. The returned string frets remain capo-relative,
        // while their pitches are the actual concert pitches.
        let shape_root = Pitch::from_midi(48 + root.value() as i32, Spelling::Sharps);
        let concert_root = Pitch::from_midi(shape_root.midi() + capo as i32, Spelling::Sharps);
        let concert_tuning = tuning.transposed(capo as i32, Spelling::Sharps);
        let setup = ResolvedShapeSetup {
            instrument: tuning.instrument,
            tuning_name: tuning.name.clone(),
            capo,
            concert_open_midi: concert_tuning
                .strings
                .iter()
                .map(|pitch| pitch.midi())
                .collect(),
        };
        let span = relative_end - relative_start;
        let mut limited_inventory = span > MAX_COMPLETE_SHAPE_SPAN;
        let windows: Vec<(u8, u8)> = if limited_inventory {
            let mut starts = Vec::new();
            let last_start = relative_end - MAX_COMPLETE_SHAPE_SPAN;
            let mut start = relative_start;
            while start < last_start {
                starts.push(start);
                start = start.saturating_add(MAX_COMPLETE_SHAPE_SPAN);
            }
            starts.push(last_start);
            starts.sort_unstable();
            starts.dedup();
            starts
                .into_iter()
                .map(|start| (start, MAX_COMPLETE_SHAPE_SPAN))
                .collect()
        } else {
            vec![(relative_start, span)]
        };
        let mut voicings = Vec::new();
        let mut fingerprints = std::collections::BTreeSet::new();
        for (window_start, window_span) in windows {
            let board = Fretboard::new(concert_tuning.clone(), window_start + window_span);
            let search = board
                .find_chord_voicings_bounded(
                    formula,
                    concert_root,
                    window_start,
                    window_span,
                    MAX_CARD_SHAPE_COMBINATIONS,
                    MAX_CARD_SHAPES,
                )
                .map_err(|_| CardShapeUnavailable::NoShapes)?;
            limited_inventory |= !search.complete;
            for voicing in search.voicings {
                let fingerprint = shape_fingerprint(&voicing);
                if fingerprints.insert(fingerprint) {
                    voicings.push(voicing);
                    if voicings.len() == MAX_CARD_SHAPES {
                        break;
                    }
                }
            }
            if voicings.len() == MAX_CARD_SHAPES {
                break;
            }
        }
        if voicings.is_empty() {
            return Err(CardShapeUnavailable::NoShapes);
        }
        let profile = if limited_inventory {
            CARD_SHAPE_LIMITED_PROFILE
        } else {
            CARD_SHAPE_PROFILE
        };
        Ok(CardShapeCandidates {
            geometry,
            tuning_name: tuning.name,
            instrument: tuning.instrument,
            setup,
            voicings,
            limited_inventory,
            profile,
        })
    }

    pub(crate) fn selected_card_shape(&self, card: &Card) -> CardShapeStatus {
        if !matches!(card.material, Material::Chord { .. }) {
            return CardShapeStatus::NotChord;
        }
        let Some(index) = card.setting.voicing_idx else {
            return CardShapeStatus::Unselected;
        };
        let candidates = match self.card_shape_candidates(card) {
            Ok(candidates) => candidates,
            Err(reason) => return CardShapeStatus::Unavailable(reason),
        };
        let count = candidates.voicings.len();
        let Some(voicing) = candidates.voicings.get(index).cloned() else {
            return CardShapeStatus::Unavailable(CardShapeUnavailable::IndexOutOfRange {
                selected: index,
                count,
            });
        };
        if let Some(profile) = &card.setting.voicing_profile {
            if profile != candidates.profile {
                return CardShapeStatus::Unavailable(CardShapeUnavailable::ProfileChanged {
                    saved: profile.clone(),
                });
            }
        }
        let fingerprint = shape_fingerprint(&voicing);
        if let Some(saved) = &card.setting.voicing_fingerprint {
            if saved != &fingerprint {
                return CardShapeStatus::Unavailable(CardShapeUnavailable::FingerprintChanged {
                    saved: saved.clone(),
                    found: fingerprint,
                });
            }
        }
        CardShapeStatus::Available(ResolvedCardShape {
            index,
            count,
            fingerprint,
            profile: candidates.profile,
            limited_inventory: candidates.limited_inventory,
            tuning_name: candidates.tuning_name,
            instrument: candidates.instrument,
            setup: candidates.setup,
            geometry: candidates.geometry,
            voicing,
        })
    }

    /// Current selected-shape state. Views can show `Unavailable` verbatim and
    /// must not replace it with an unrelated legacy shape.
    pub fn card_shape_status(&self, card: &Card) -> CardShapeStatus {
        self.selected_card_shape(card)
    }

    /// Geometry for a selected chord shape, including a selected-but-invalid
    /// ordinal when its setup itself resolves. `None` preserves the ordinary
    /// live-board geometry for unselected/non-chord cards.
    pub fn card_shape_geometry(&self, card: &Card) -> Option<CardShapeGeometry> {
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
        card.setting.voicing_fingerprint = Some(shape_fingerprint(voicing));
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
            state.card_voicing(&card).0.len(),
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
        let heard = state.card_voicing(&card).0;
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
        let solo = state.card_sounding_pitches(&card).0;
        assert_eq!(solo.len(), 1, "solo keeps the selected physical contact");
        assert!((solo[0] - target.frequency).abs() < 0.5);

        card.setting.mark_mode = MarkMode::Mute;
        let muted = state.card_sounding_pitches(&card).0;
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
