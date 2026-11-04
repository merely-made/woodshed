use super::*;

/// One practice item as a rehearsal card, stamped with its recipe (the
/// P6 "Fill set" conversion). The item's hand position pins the card's
/// fret window.
pub fn card_from_practice_item(item: &PracticeItem, set_name: &str) -> Card {
    let recipe = Some(Recipe::PracticeSet {
        name: set_name.to_string(),
    });
    let window = |position: u8| {
        Some(FretWindow {
            start: position,
            span: 4,
        })
    };
    match item {
        PracticeItem::Scale {
            formula,
            root,
            position,
        } => Card {
            id: CardId::UNASSIGNED,
            label: item.label(),
            material: Material::Scale {
                name: formula.name.to_string(),
                root: PitchClass::new(root.pitch_class()),
            },
            setting: Setting {
                fret_window: window(*position),
                ..Setting::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: recipe,
        },
        PracticeItem::Chord {
            formula,
            root,
            position,
        } => Card {
            id: CardId::UNASSIGNED,
            label: item.label(),
            material: Material::Chord {
                name: formula.name.to_string(),
                root: PitchClass::new(root.pitch_class()),
            },
            setting: Setting {
                fret_window: window(*position),
                ..Setting::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: recipe,
        },
        PracticeItem::Exercise {
            exercise,
            starting_fret,
        } => Card {
            id: CardId::UNASSIGNED,
            label: item.label(),
            material: Material::Riff {
                name: exercise.name.to_string(),
            },
            setting: Setting {
                fret_window: window(*starting_fret),
                ..Setting::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: recipe,
        },
    }
}

/// Materialize a practice set as a rehearsal [`Set`] (cursor at the top).
/// Each stamped card enters as its own occurrence, so a template that repeats
/// material yields distinct, separately editable cards.
pub fn set_from_practice(ps: &PracticeSet) -> Set {
    Set::from_cards(
        ps.items
            .iter()
            .map(|item| card_from_practice_item(item, &ps.name)),
        LoopMode::All,
    )
}

/// How long the rehearsal transport dwells on `card` before advancing.
/// `None` = manual (no auto-advance). The card's own bpm wins over the
/// transport's; `Reps` counts as bars (one rep per bar until per-rep
/// audio lands).
pub fn card_dwell(
    card: &Card,
    fallback_bpm: f32,
) -> Result<Option<std::time::Duration>, CardTimingError> {
    Ok(resolve_card_timing(&card.timing, fallback_bpm)?.dwell)
}

/// Step a rehearsal set's cursor by `dir`, honoring its loop mode.
/// Returns false when the step hit the end with looping off.
pub fn step_set(set: &mut Set, dir: i32) -> bool {
    if set.cards.is_empty() {
        return false;
    }
    let n = set.cards.len() as i32;
    let next = set.cursor as i32 + dir;
    match set.loop_mode {
        LoopMode::All => {
            set.cursor = next.rem_euclid(n) as usize;
            true
        },
        LoopMode::Off | LoopMode::Card => {
            if next < 0 || next >= n {
                false
            } else {
                set.cursor = next as usize;
                true
            }
        },
    }
}
