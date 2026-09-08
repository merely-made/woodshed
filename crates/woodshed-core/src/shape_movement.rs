//! Exact physical-contact movement between two selected chord shapes.
//!
//! This module reports neck geometry only. It neither assigns fingers nor
//! treats a marked sounding note as a released physical contact.

use crate::StageState;
use crate::card_shapes::{
    CardShapeStatus, CardShapeUnavailable, ResolvedCardShape, ResolvedShapeSetup,
};
use woodshedding::fretboard::StringPlay;
use woodshedding::rehearsal::Card;

/// A sounding string's shape contact. `Open` is explicit, including when a
/// capo supplies the concert pitch; it is not a player-fretted contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeContact {
    Open { string_index: usize },
    Fretted { string_index: usize, fret: u8 },
}

impl ShapeContact {
    pub fn string_index(&self) -> usize {
        match self {
            Self::Open { string_index } | Self::Fretted { string_index, .. } => *string_index,
        }
    }
}

/// One string's physical-contact change. Open-to-fretted and
/// fretted-to-open transitions are respectively `Added` and `Dropped`; they
/// have no invented travel cost. Muted strings have no contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeStringMovement {
    Held {
        contact: ShapeContact,
    },
    Moved {
        string_index: usize,
        from_fret: u8,
        to_fret: u8,
        fret_travel: u8,
    },
    Added {
        contact: ShapeContact,
    },
    Dropped {
        contact: ShapeContact,
    },
}

impl ShapeStringMovement {
    pub fn string_index(&self) -> usize {
        match self {
            Self::Held { contact } | Self::Added { contact } | Self::Dropped { contact } => {
                contact.string_index()
            },
            Self::Moved { string_index, .. } => *string_index,
        }
    }
}

/// Separate, directly observable physical facts about two selected shapes.
/// `position_shift` is right-lowest-fretted minus left-lowest-fretted in
/// physical fret numbers. It is absent if either shape has no fretted contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeMovement {
    pub per_string: Vec<ShapeStringMovement>,
    pub total_matched_fret_travel: u16,
    pub left_fretted_span: Option<u8>,
    pub right_fretted_span: Option<u8>,
    pub position_shift: Option<i16>,
}

/// Why two cards cannot be compared as selected shapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeMovementUnavailable {
    LeftNotChord,
    RightNotChord,
    LeftUnselected,
    RightUnselected,
    LeftUnavailable(CardShapeUnavailable),
    RightUnavailable(CardShapeUnavailable),
    SetupMismatch {
        left: ResolvedShapeSetup,
        right: ResolvedShapeSetup,
    },
}

fn selected(
    state: &StageState,
    card: &Card,
    left: bool,
) -> Result<ResolvedCardShape, ShapeMovementUnavailable> {
    match state.selected_card_shape(card) {
        CardShapeStatus::Available(shape) => Ok(shape),
        CardShapeStatus::NotChord if left => Err(ShapeMovementUnavailable::LeftNotChord),
        CardShapeStatus::NotChord => Err(ShapeMovementUnavailable::RightNotChord),
        CardShapeStatus::Unselected if left => Err(ShapeMovementUnavailable::LeftUnselected),
        CardShapeStatus::Unselected => Err(ShapeMovementUnavailable::RightUnselected),
        CardShapeStatus::Unavailable(reason) if left => {
            Err(ShapeMovementUnavailable::LeftUnavailable(reason))
        },
        CardShapeStatus::Unavailable(reason) => {
            Err(ShapeMovementUnavailable::RightUnavailable(reason))
        },
    }
}

fn contact(shape: &ResolvedCardShape, string_index: usize) -> Option<ShapeContact> {
    match shape.voicing.strings.get(string_index)? {
        StringPlay::Muted => None,
        StringPlay::Played { fret: 0, .. } => Some(ShapeContact::Open { string_index }),
        StringPlay::Played { fret, .. } => Some(ShapeContact::Fretted {
            string_index,
            fret: fret.saturating_add(shape.geometry.capo),
        }),
    }
}

fn fretted_extent(shape: &ResolvedCardShape) -> Option<(u8, u8)> {
    let frets: Vec<u8> = (0..shape.voicing.strings.len())
        .filter_map(|string_index| match contact(shape, string_index) {
            Some(ShapeContact::Fretted { fret, .. }) => Some(fret),
            Some(ShapeContact::Open { .. }) | None => None,
        })
        .collect();
    let lowest = frets.iter().copied().min()?;
    let highest = frets.iter().copied().max()?;
    Some((lowest, highest - lowest))
}

fn compare_resolved(left: &ResolvedCardShape, right: &ResolvedCardShape) -> ShapeMovement {
    let mut per_string = Vec::new();
    let mut total_matched_fret_travel = 0_u16;
    let strings = left.voicing.strings.len().max(right.voicing.strings.len());
    for string_index in 0..strings {
        let left_contact = contact(left, string_index);
        let right_contact = contact(right, string_index);
        let movement = match (left_contact, right_contact) {
            (Some(left), Some(right)) if left == right => {
                ShapeStringMovement::Held { contact: left }
            },
            (
                Some(ShapeContact::Fretted {
                    fret: from_fret, ..
                }),
                Some(ShapeContact::Fretted { fret: to_fret, .. }),
            ) => {
                let fret_travel = from_fret.abs_diff(to_fret);
                total_matched_fret_travel += u16::from(fret_travel);
                ShapeStringMovement::Moved {
                    string_index,
                    from_fret,
                    to_fret,
                    fret_travel,
                }
            },
            (Some(left), Some(right)) => {
                per_string.push(ShapeStringMovement::Dropped { contact: left });
                ShapeStringMovement::Added { contact: right }
            },
            (Some(contact), None) => ShapeStringMovement::Dropped { contact },
            (None, Some(contact)) => ShapeStringMovement::Added { contact },
            (None, None) => continue,
        };
        per_string.push(movement);
    }

    let left_extent = fretted_extent(left);
    let right_extent = fretted_extent(right);
    ShapeMovement {
        per_string,
        total_matched_fret_travel,
        left_fretted_span: left_extent.map(|(_, span)| span),
        right_fretted_span: right_extent.map(|(_, span)| span),
        position_shift: left_extent.zip(right_extent).map(
            |((left_lowest, _), (right_lowest, _))| {
                i16::from(right_lowest) - i16::from(left_lowest)
            },
        ),
    }
}

impl StageState {
    /// Compare two selected shapes under the same exact resolved setup.
    ///
    /// This is shape/neck movement, not finger movement or an ergonomic score.
    /// Mark and mute settings do not change the selected physical contacts.
    pub fn compare_cards(
        &self,
        left: &Card,
        right: &Card,
    ) -> Result<ShapeMovement, ShapeMovementUnavailable> {
        let left = selected(self, left, true)?;
        let right = selected(self, right, false)?;
        if left.setup != right.setup {
            return Err(ShapeMovementUnavailable::SetupMismatch {
                left: left.setup,
                right: right.setup,
            });
        }

        Ok(compare_resolved(&left, &right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::{KeyedCatalogRef, compare_pitch_motion};
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::{FretWindow, MarkMode, Material, Setting, Timing, Touch};

    fn c_major_card() -> Card {
        Card {
            id: woodshedding::rehearsal::CardId::UNASSIGNED,
            label: "C Major".to_string(),
            material: Material::Chord {
                name: "Major".to_string(),
                root: PitchClass::new(0),
            },
            setting: Setting {
                fret_window: Some(FretWindow { start: 0, span: 4 }),
                ..Setting::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        }
    }

    #[test]
    fn same_chord_can_have_zero_pitch_motion_and_nonzero_neck_movement() {
        let state = StageState::new();
        let mut left = c_major_card();
        let mut right = c_major_card();
        left.setting.fret_window = Some(FretWindow { start: 0, span: 12 });
        right.setting.fret_window = Some(FretWindow { start: 0, span: 12 });
        state
            .select_card_shape_index(&mut left, 0)
            .expect("first C shape");

        let keyed = KeyedCatalogRef::from_material(&left.material).expect("C major identity");
        assert_eq!(
            compare_pitch_motion(&keyed, &keyed)
                .expect("equal pitch sets")
                .total_semitones,
            0
        );
        let count = state.card_shape_count(&left).expect("C shape inventory");
        let movement = (1..count).find_map(|index| {
            state.select_card_shape_index(&mut right, index).ok()?;
            let movement = state.compare_cards(&left, &right).ok()?;
            (movement.total_matched_fret_travel > 0).then_some(movement)
        });
        assert!(
            movement.is_some(),
            "two C-major shapes have identical pitch classes but different matched frets"
        );
    }

    #[test]
    fn comparison_rejects_different_effective_setups() {
        let state = StageState::new();
        let mut left = c_major_card();
        let mut right = c_major_card();
        right.setting.capo = Some(2);
        right.setting.fret_window = Some(FretWindow { start: 2, span: 4 });
        state
            .select_next_card_shape(&mut left)
            .expect("open C shape");
        state
            .select_next_card_shape(&mut right)
            .expect("capoed C shape");

        assert!(matches!(
            state.compare_cards(&left, &right),
            Err(ShapeMovementUnavailable::SetupMismatch { .. })
        ));
    }

    #[test]
    fn comparison_distinguishes_unselected_and_invalid_shapes() {
        let state = StageState::new();
        let left = c_major_card();
        let mut right = c_major_card();
        assert!(matches!(
            state.compare_cards(&left, &right),
            Err(ShapeMovementUnavailable::LeftUnselected)
        ));

        right.setting.voicing_idx = Some(usize::MAX);
        assert!(matches!(
            state.compare_cards(&right, &left),
            Err(ShapeMovementUnavailable::LeftUnavailable(
                CardShapeUnavailable::IndexOutOfRange { .. }
            ))
        ));
    }

    #[test]
    fn marks_do_not_change_selected_shape_contacts() {
        let state = StageState::new();
        let mut left = c_major_card();
        let mut right = c_major_card();
        state.select_next_card_shape(&mut left).expect("left shape");
        state
            .select_next_card_shape(&mut right)
            .expect("right shape");
        let before = state.compare_cards(&left, &right).expect("same setup");

        left.setting.mark_mode = MarkMode::Solo;
        left.setting.marked = vec![(0, 0)];
        right.setting.mark_mode = MarkMode::Mute;
        right.setting.marked = vec![(5, 3)];
        assert_eq!(state.compare_cards(&left, &right), Ok(before));
    }

    #[test]
    fn physical_contacts_report_exact_travel_span_shift_and_open_mute_changes() {
        use crate::card_shapes::CardShapeGeometry;
        use woodshedding::fretboard::ChordVoicing;
        use woodshedding::pitch::{Pitch, Spelling};

        let setup = ResolvedShapeSetup {
            instrument: woodshedding::tuning::Instrument::Guitar,
            tuning_name: "Fixture".to_string(),
            capo: 2,
            concert_open_midi: vec![42, 47, 52, 57],
        };
        let shape = |strings| ResolvedCardShape {
            index: 0,
            count: 1,
            fingerprint: "fixture".to_string(),
            profile: "fixture",
            limited_inventory: false,
            tuning_name: setup.tuning_name.clone(),
            instrument: setup.instrument,
            setup: setup.clone(),
            geometry: CardShapeGeometry {
                string_count: 4,
                physical_fret_start: 2,
                physical_fret_end: 8,
                capo: 2,
            },
            voicing: ChordVoicing { strings },
        };
        let played = |fret| StringPlay::Played {
            fret,
            pitch: Pitch::from_midi(48 + i32::from(fret), Spelling::Sharps),
            interval_from_root: None,
        };
        let left = shape(vec![played(0), played(2), played(4), StringPlay::Muted]);
        let right = shape(vec![played(0), played(4), StringPlay::Muted, played(1)]);

        assert_eq!(
            compare_resolved(&left, &right),
            ShapeMovement {
                per_string: vec![
                    ShapeStringMovement::Held {
                        contact: ShapeContact::Open { string_index: 0 },
                    },
                    ShapeStringMovement::Moved {
                        string_index: 1,
                        from_fret: 4,
                        to_fret: 6,
                        fret_travel: 2,
                    },
                    ShapeStringMovement::Dropped {
                        contact: ShapeContact::Fretted {
                            string_index: 2,
                            fret: 6,
                        },
                    },
                    ShapeStringMovement::Added {
                        contact: ShapeContact::Fretted {
                            string_index: 3,
                            fret: 3,
                        },
                    },
                ],
                total_matched_fret_travel: 2,
                left_fretted_span: Some(2),
                right_fretted_span: Some(3),
                position_shift: Some(-1),
            }
        );
    }
}
