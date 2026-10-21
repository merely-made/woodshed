//! Pure selected-shape inventory and identity checks shared by notation and rehearsal.
use crate::{
    chord::catalog as chord_catalog,
    fretboard::{ChordVoicing, Fretboard},
    pitch::{Pitch, Spelling},
    rehearsal::{Card, Material, Set},
    tuning::{Instrument, Tuning, catalog as tuning_catalog},
};

/// Explicit inherited instrument context. Card-specific setup still takes precedence.
#[derive(Clone, Debug)]
pub struct ShapeContext {
    pub tuning: Tuning,
    pub fret_start: u8,
    pub fret_count: u8,
}
/// The bounded enumeration policy used for persisted chord-shape selections.
/// A changed policy must not silently reinterpret a saved ordinal.
pub const CARD_SHAPE_PROFILE: &str = "root-bass/v1";
const MAX_CARD_SHAPE_COMBINATIONS: usize = 50_000;
const MAX_CARD_SHAPES: usize = 128;
pub const CARD_SHAPE_LIMITED_PROFILE: &str = "root-bass/position4/v1";
const MAX_COMPLETE_SHAPE_SPAN: u8 = 4;

pub fn shape_fingerprint(voicing: &ChordVoicing) -> String {
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

impl CardShapeGeometry {
    /// An unavailable retained setup must not borrow the live neck geometry.
    pub const EMPTY: Self = Self {
        string_count: 0,
        physical_fret_start: 0,
        physical_fret_end: 0,
        capo: 0,
    };
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
pub struct CardShapeCandidates {
    pub geometry: CardShapeGeometry,
    pub tuning_name: String,
    pub instrument: Instrument,
    pub setup: ResolvedShapeSetup,
    pub voicings: Vec<ChordVoicing>,
    pub limited_inventory: bool,
    pub profile: &'static str,
}

impl ShapeContext {
    fn tuning(&self) -> Tuning {
        self.tuning.clone()
    }
    /// Recheck a saved arrangement's musical catalog and inherited setup.
    pub fn from_saved(set: &Set) -> Result<Self, String> {
        let saved = set
            .progression_arrangement
            .as_ref()
            .ok_or("This Set has no saved arrangement")?;
        if saved.version != 1
            || saved.root_pc >= 12
            || saved.fret_start > saved.fret_end
            || saved.fret_end == 0
            || saved.fret_end > 30
        {
            return Err("Unsupported or invalid saved arrangement setup".into());
        }
        let progression = crate::progression::catalog()
            .iter()
            .find(|p| p.name == saved.progression)
            .ok_or("Saved progression is unavailable")?;
        if progression.roles != saved.roles {
            return Err("Saved progression roles have changed; realign explicitly".into());
        }
        let instrument = Instrument::from_identity(&saved.instrument)
            .ok_or("Saved instrument is unavailable")?;
        let tuning =
            Tuning::find_for(&saved.tuning, instrument).ok_or("Saved tuning is unavailable")?;
        if tuning.strings.iter().map(|p| p.midi()).collect::<Vec<_>>() != saved.open_midi {
            return Err("Saved tuning pitches have changed; review setup explicitly".into());
        }
        Ok(Self {
            tuning,
            fret_start: saved.fret_start,
            fret_count: saved.fret_end,
        })
    }
    pub fn resolve_card_tuning(&self, card: &Card) -> Result<Tuning, CardShapeUnavailable> {
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

    pub fn inventory(&self, card: &Card) -> Result<CardShapeCandidates, CardShapeUnavailable> {
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

    pub fn selected_card_shape(&self, card: &Card) -> CardShapeStatus {
        if !matches!(card.material, Material::Chord { .. }) {
            return CardShapeStatus::NotChord;
        }
        if card.setting.voicing_idx.is_none() {
            return CardShapeStatus::Unselected;
        }
        match self.inventory(card) {
            Ok(inventory) => inventory.selected(card),
            Err(error) => CardShapeStatus::Unavailable(error),
        }
    }
}
impl CardShapeCandidates {
    pub fn selected(self, card: &Card) -> CardShapeStatus {
        if !matches!(card.material, Material::Chord { .. }) {
            return CardShapeStatus::NotChord;
        }
        let Some(index) = card.setting.voicing_idx else {
            return CardShapeStatus::Unselected;
        };
        let candidates = self;
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
}
