use super::*;
use crate::history::catalog_id_for_card;
use woodshedding::{
    rehearsal::{FretWindow, Hold, Setting, Timing},
    tuning::{Instrument, Tuning},
};

fn pair() -> Set {
    let mut set = Set::default();
    for (name, root) in [("Minor 7", 9), ("Major 7", 0)] {
        set.push(Card {
            id: CardId::UNASSIGNED,
            label: format!("{name} target"),
            material: Material::Chord {
                name: name.into(),
                root: PitchClass::new(root),
            },
            setting: Setting {
                instrument: "Ukulele".into(),
                tuning: Some("Standard (high-G)".into()),
                capo: Some(2),
                fret_window: Some(FretWindow { start: 2, span: 12 }),
                ..Default::default()
            },
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        });
    }
    set
}

#[test]
fn chromatic_pairs_are_actual_same_string_contacts_with_target_setup() {
    let state = StageState::new();
    let set = pair();
    let tuning = Tuning::find_for("Standard (high-G)", Instrument::Ukulele).unwrap();
    for direction in ApproachDirection::ALL {
        let discovery = state
            .chord_approach_discovery(&set, set.cards[0].id, set.cards[1].id, direction)
            .unwrap();
        let realized = state
            .chord_approach_realization(&discovery.preview)
            .unwrap();
        assert_eq!(realized.geometry.string_count, 4);
        assert_eq!(realized.concert_root, PitchClass::new(2));
        for pair in realized.notes.chunks_exact(2) {
            assert!(!pair[0].is_target && pair[1].is_target);
            assert_eq!(pair[0].string_index, pair[1].string_index);
            assert_eq!(
                i16::from(pair[0].physical_fret) - i16::from(pair[1].physical_fret),
                direction.fret_delta()
            );
            assert_eq!(
                pair[0].pitch.midi() - pair[1].pitch.midi(),
                i32::from(direction.fret_delta())
            );
            for note in pair {
                assert_eq!(
                    note.pitch.midi(),
                    tuning.strings[note.string_index].midi() + i32::from(note.physical_fret)
                );
                assert!(note.physical_fret >= 2 && note.physical_fret <= 14);
            }
        }
        let sounded = state.card_sounding_pitches(&discovery.preview).unwrap().0;
        assert_eq!(
            sounded,
            realized
                .notes
                .iter()
                .map(|note| note.pitch.frequency() as f32)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn selected_shape_is_retained_exactly_and_stale_shape_is_not_substituted() {
    let state = StageState::new();
    let mut set = pair();
    state.select_next_card_shape(&mut set.cards[1]).unwrap();
    let before = serde_json::to_value(&set).unwrap();
    let discovery = state
        .chord_approach_discovery(
            &set,
            set.cards[0].id,
            set.cards[1].id,
            ApproachDirection::Above,
        )
        .unwrap();
    assert_eq!(serde_json::to_value(&set).unwrap(), before);
    assert_eq!(
        discovery.preview.setting.voicing_fingerprint,
        set.cards[1].setting.voicing_fingerprint
    );
    assert_eq!(
        discovery.preview.setting.voicing_profile,
        set.cards[1].setting.voicing_profile
    );
    let shape = match state.card_shape_status(&set.cards[1]) {
        CardShapeStatus::Available(shape) => shape,
        _ => panic!("selected shape"),
    };
    let realized = state
        .chord_approach_realization(&discovery.preview)
        .unwrap();
    assert!(
        realized
            .notes
            .iter()
            .filter(|note| note.is_target)
            .all(|note| shape
                .physical_positions()
                .contains(&(note.string_index, note.physical_fret)))
    );
    set.cards[1].setting.voicing_fingerprint = Some("stale".into());
    assert!(matches!(
        state.chord_approach_discovery(
            &set,
            set.cards[0].id,
            set.cards[1].id,
            ApproachDirection::Above
        ),
        Err(ChordApproachDiscoveryUnavailable::Realization(
            ChordApproachUnavailable::Shape(_)
        ))
    ));
}

#[test]
fn explicit_pair_rejects_reorder_removal_replacement_and_invalid_setup() {
    let state = StageState::new();
    let mut set = pair();
    let (source, target) = (set.cards[0].id, set.cards[1].id);
    set.cards.swap(0, 1);
    assert_eq!(
        state
            .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
            .unwrap_err(),
        ChordApproachDiscoveryUnavailable::NotAdjacent
    );
    assert_eq!(set.cards.len(), 2);
    set.cards.swap(0, 1);
    set.cards[1].setting.tuning = Some("Missing".into());
    assert!(matches!(
        state.stage_chord_approach(&mut set, source, target, ApproachDirection::Below),
        Err(ChordApproachDiscoveryUnavailable::Realization(
            ChordApproachUnavailable::Setup(_)
        ))
    ));
    set.cards[1].material = Material::Scale {
        name: "Major".into(),
        root: PitchClass::new(0),
    };
    assert_eq!(
        state
            .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
            .unwrap_err(),
        ChordApproachDiscoveryUnavailable::NotChordPair
    );
    set.remove(1);
    assert!(matches!(
        state.stage_chord_approach(&mut set, source, target, ApproachDirection::Below),
        Err(ChordApproachDiscoveryUnavailable::MissingTarget(_))
    ));
}

#[test]
fn gaps_never_invent_partners_and_valid_alternative_contacts_are_used() {
    let state = StageState::new();
    let mut set = pair();
    set.cards[1].setting.fret_window = Some(FretWindow { start: 2, span: 0 });
    assert!(matches!(
        state.chord_approach_discovery(
            &set,
            set.cards[0].id,
            set.cards[1].id,
            ApproachDirection::Below
        ),
        Err(ChordApproachDiscoveryUnavailable::Realization(
            ChordApproachUnavailable::NoCompletePairs
        ))
    ));
    set.cards[1].setting.instrument = "Guitar".into();
    set.cards[1].setting.tuning = Some("Standard".into());
    set.cards[1].setting.capo = None;
    set.cards[1].setting.fret_window = Some(FretWindow { start: 0, span: 7 });
    let discovery = state
        .chord_approach_discovery(
            &set,
            set.cards[0].id,
            set.cards[1].id,
            ApproachDirection::Below,
        )
        .unwrap();
    let realized = state
        .chord_approach_realization(&discovery.preview)
        .unwrap();
    assert!(
        realized
            .notes
            .iter()
            .filter(|note| note.is_target)
            .all(|note| note.physical_fret > 0)
    );
    // E4's open top string cannot approach from below; the same pitch on
    // the B string at fret 5 provides a valid partner within this window.
    assert!(
        realized
            .notes
            .iter()
            .any(|note| note.is_target && note.pitch.midi() == 64 && note.physical_fret == 5)
    );
    assert!(
        realized
            .notes
            .iter()
            .any(|note| note.is_target && note.pitch.midi() == 52)
    );
}

#[test]
fn order_marks_and_timing_preserve_visits_and_fit_inherited_dwell() {
    let state = StageState::new();
    let mut set = pair();
    set.cards[1].timing.hold = Hold::Bars(1);
    let mut recipe = state
        .chord_approach_discovery(
            &set,
            set.cards[0].id,
            set.cards[1].id,
            ApproachDirection::Above,
        )
        .unwrap()
        .preview;
    let realized = state.chord_approach_realization(&recipe).unwrap();
    let baseline = state.card_sounding_pitches(&recipe).unwrap().0;
    assert!(baseline.windows(2).any(|pair| pair[0] > pair[1]));
    let target = &realized.notes[1];
    recipe.setting.marked = vec![(target.string_index, target.physical_fret)];
    recipe.setting.mark_mode = MarkMode::Solo;
    let expected = baseline
        .iter()
        .copied()
        .filter(|pitch| *pitch == target.pitch.frequency() as f32)
        .collect::<Vec<_>>();
    assert_eq!(state.card_sounding_pitches(&recipe).unwrap().0, expected);
    recipe.setting.mark_mode = MarkMode::Mute;
    let expected = baseline
        .iter()
        .copied()
        .filter(|pitch| crate::pc_from_hz(*pitch) != target.pitch.pitch_class())
        .collect::<Vec<_>>();
    assert_eq!(state.card_sounding_pitches(&recipe).unwrap().0, expected);
    recipe.setting.mark_mode = MarkMode::Off;
    let (pitches, duration, offset) = state.card_sounding_pitches_at_tempo(&recipe, 80.0).unwrap();
    assert!((duration - 3.0).abs() < 0.00001);
    assert!((pitches.len() - 1) as f32 * offset < duration * 1000.0);
}

#[test]
fn explicit_append_reopens_as_a_distinct_recipe_and_pitch_sets_include_approaches() {
    let state = StageState::new();
    let mut set = pair();
    let before = serde_json::to_value(&set.cards).unwrap();
    let (source, target) = (set.cards[0].id, set.cards[1].id);
    let first = state
        .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
        .unwrap();
    let second = state
        .stage_chord_approach(&mut set, source, target, ApproachDirection::Below)
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(serde_json::to_value(&set.cards[..2]).unwrap(), before);
    let reopened: Set = serde_json::from_str(&serde_json::to_string(&set).unwrap()).unwrap();
    assert_eq!(reopened.cards[2].id, first);
    assert_eq!(
        catalog_id_for_card(&reopened.cards[2]).as_deref(),
        Some("exercise:chord-approach-below/v1:Major 7")
    );
    let keyed = KeyedCatalogRef::from_card(&reopened.cards[2]).unwrap();
    assert_eq!(keyed.wire_key(), "chord-approach:below:Major 7@pc:0");
    let pcs = keyed.pitch_classes().unwrap();
    assert!(pcs.contains(&PitchClass::new(3)));
    assert!(pcs.contains(&PitchClass::new(6)));
    assert_eq!(
        state.card_sounding_pitches(&reopened.cards[2]).unwrap().0,
        state.card_sounding_pitches(&set.cards[2]).unwrap().0
    );
}
