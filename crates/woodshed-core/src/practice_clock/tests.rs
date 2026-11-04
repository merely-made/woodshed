use super::*;
use woodshedding::rehearsal::{FretWindow, Recipe, ScalePattern, Setting};

fn stage() -> StageState {
    StageState::new()
}

fn set(cards: Vec<Card>) -> Set {
    let mut set = Set::default();
    set.cards = cards;
    set
}

fn path_card(stage: &StageState, positions: Vec<(usize, u8)>) -> Card {
    Card {
        id: CardId(7),
        label: "Repeated path".into(),
        material: Material::Path {
            positions,
            root: PitchClass::new(9),
        },
        setting: Setting {
            instrument: stage.tuning().instrument.to_string(),
            tuning: Some(stage.tuning().name),
            ..Setting::default()
        },
        touch: Touch::Walk,
        timing: Timing::default(),
        from: None,
    }
}

#[test]
fn set_lowering_preserves_ordered_repeated_physical_visits_and_seconds() {
    let mut stage = stage();
    let card = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
    let expected = stage.card_voicing(&card).unwrap().0;
    stage.set_tuning(1);
    let mut card = card;
    card.timing.hold = Hold::Seconds(3.0);
    let set = set(vec![card]);
    let program = stage.practice_program(&set, 120.0);
    let lowered = &program.cards[0];
    assert!(lowered.unavailable.is_none());
    assert_eq!(lowered.hold, PracticeHold::Seconds(3.0));
    assert_eq!(lowered.events.len(), 3);
    assert_eq!(
        lowered
            .events
            .iter()
            .map(|event| event.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(lowered.events[0].contacts, vec![(0, 1)]);
    assert_eq!(lowered.events[2].contacts, vec![(0, 1)]);
    assert_eq!(lowered.events[0].pitches, lowered.events[2].pitches);
    assert_eq!(
        lowered
            .events
            .iter()
            .map(|event| event.pitches[0])
            .collect::<Vec<_>>(),
        expected,
        "generic preview and managed lowering use the same retained Path contacts"
    );
    assert!(
        lowered
            .events
            .iter()
            .all(|event| event.strum_seconds == 0.0)
    );
    assert!((lowered.events[1].at - 1.0).abs() < 1e-9);
}

#[test]
fn unavailable_saved_path_contacts_refuse_display_sound_and_managed_lowering() {
    let stage = stage();
    let card = path_card(&stage, vec![(usize::MAX, u8::MAX)]);
    assert!(stage.dots_for_card(&card).is_empty());
    assert!(stage.card_voicing(&card).unwrap().0.is_empty());
    let program = stage.practice_program(&set(vec![card]), 120.0);
    assert!(program.cards[0].unavailable.is_some());
    assert!(program.cards[0].events.is_empty());
}

#[test]
fn path_solo_marks_share_full_ordered_contacts_outside_viewport() {
    let mut stage = stage();
    let mut card = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
    card.setting.mark_mode = MarkMode::Solo;
    card.setting.marked = vec![(0, 1), (0, 2)];
    stage.fret_start = 8;
    stage.fret_count = 12;

    assert!(
        stage.dots_for_card(&card).is_empty(),
        "viewport clips dots only"
    );
    let generic = stage.card_sounding_pitches(&card).unwrap().0;
    let program = stage.practice_program(&set(vec![card]), 120.0);
    let managed = program.cards[0]
        .events
        .iter()
        .flat_map(|event| event.pitches.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(generic, managed);
    assert_eq!(
        generic.len(),
        3,
        "the authored repeated contact remains repeated"
    );
    assert_eq!(program.cards[0].events[0].contacts, vec![(0, 1)]);
    assert_eq!(program.cards[0].events[2].contacts, vec![(0, 1)]);
}

#[test]
fn path_mute_marks_share_full_ordered_contacts_outside_viewport() {
    let mut stage = stage();
    let mut card = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
    card.setting.mark_mode = MarkMode::Mute;
    card.setting.marked = vec![(0, 2)];
    stage.fret_start = 8;
    stage.fret_count = 12;

    assert!(
        stage.dots_for_card(&card).is_empty(),
        "viewport clips dots only"
    );
    let generic = stage.card_sounding_pitches(&card).unwrap().0;
    let program = stage.practice_program(&set(vec![card]), 120.0);
    let managed = program.cards[0]
        .events
        .iter()
        .flat_map(|event| event.pitches.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(generic, managed);
    assert_eq!(generic.len(), 2);
    assert_eq!(program.cards[0].events[0].contacts, vec![(0, 1)]);
    assert_eq!(program.cards[0].events[1].contacts, vec![(0, 1)]);
}

#[test]
fn path_contact_setup_supports_legacy_inheritance_and_refuses_unknown_identity() {
    let stage = stage();
    let mut inherited = path_card(&stage, vec![(0, 1), (0, 2), (0, 1)]);
    inherited.setting.instrument.clear();
    inherited.setting.tuning = None;
    let expected = stage.card_voicing(&inherited).unwrap().0;
    let program = stage.practice_program(&set(vec![inherited.clone()]), 120.0);
    assert_eq!(
        program.cards[0]
            .events
            .iter()
            .map(|event| event.pitches[0])
            .collect::<Vec<_>>(),
        expected
    );

    inherited.setting.instrument = "unknown instrument".into();
    inherited.setting.tuning = Some("unknown tuning".into());
    assert!(stage.card_voicing(&inherited).unwrap().0.is_empty());
    let refused = stage.practice_program(&set(vec![inherited]), 120.0);
    assert!(refused.cards[0].unavailable.is_some());
    assert!(refused.cards[0].events.is_empty());
}

#[test]
fn equal_pitch_path_visits_keep_each_authored_contact() {
    let stage = stage();
    let board = Fretboard::new(stage.tuning(), stage.fret_count);
    let mut equal_pitch_contacts = None;
    'outer: for left_string in 0..stage.string_count() {
        for left_fret in 0..=stage.fret_count {
            for right_string in left_string + 1..stage.string_count() {
                for right_fret in 0..=stage.fret_count {
                    if board.pitch_at(left_string, left_fret).midi()
                        == board.pitch_at(right_string, right_fret).midi()
                    {
                        equal_pitch_contacts =
                            Some(((left_string, left_fret), (right_string, right_fret)));
                        break 'outer;
                    }
                }
            }
        }
    }
    let (first, second) = equal_pitch_contacts.expect("tuning has alternate equal-pitch contacts");
    let lowered = stage
        .practice_program(
            &set(vec![path_card(&stage, vec![first, second, first])]),
            120.0,
        )
        .cards
        .remove(0);
    assert!(lowered.unavailable.is_none(), "{:?}", lowered.unavailable);
    assert_eq!(lowered.events.len(), 3);
    assert_eq!(lowered.events[0].contacts, vec![first]);
    assert_eq!(lowered.events[1].contacts, vec![second]);
    assert_eq!(lowered.events[2].contacts, vec![first]);
    assert_eq!(lowered.events[0].pitches, lowered.events[1].pitches);
    assert_eq!(lowered.events[0].ordinal, 0);
    assert_eq!(lowered.events[1].ordinal, 1);
    assert_eq!(lowered.events[2].ordinal, 2);
}

#[test]
fn manual_phrase_and_beat_holds_have_distinct_units() {
    let stage = stage();
    let path = path_card(&stage, vec![(0, 1), (0, 2), (0, 3)]);
    let mut set = set(vec![path.clone()]);
    let manual = stage.practice_program(&set, 120.0).cards.remove(0);
    assert!(
        matches!(manual.hold, PracticeHold::Manual { phrase_beats: 2.0, tail_seconds } if (tail_seconds - 0.4).abs() < 1e-6)
    );
    assert_eq!(
        manual
            .events
            .iter()
            .map(|event| event.at)
            .collect::<Vec<_>>(),
        vec![0.0, 1.0, 2.0]
    );
    let slower_manual = stage.practice_program(&set, 72.0).cards.remove(0);
    assert!(
        matches!(slower_manual.hold, PracticeHold::Manual { tail_seconds, .. } if tail_seconds == 0.4)
    );

    set.cards[0].timing.hold = Hold::Bars(2);
    let bars = stage.practice_program(&set, 120.0).cards.remove(0);
    assert_eq!(bars.hold, PracticeHold::Beats(8.0));
    assert_eq!(
        bars.events.iter().map(|event| event.at).collect::<Vec<_>>(),
        vec![0.0, 8.0 / 3.0, 16.0 / 3.0]
    );
}

#[test]
fn scale_pattern_repeats_are_events_and_marked_contacts_are_retained() {
    let mut stage = stage();
    let major = stage
        .scales()
        .iter()
        .position(|scale| scale.name == "Major")
        .unwrap();
    stage.select_scale(major);
    let mut card = stage.card_from_lens().expect("default scale lens card");
    card.material = Material::ScalePattern {
        name: stage.scale().name.into(),
        root: PitchClass::new(9),
        pattern: ScalePattern::Thirds,
    };
    card.touch = Touch::Walk;
    let realization = stage.scale_card_realization(&card).unwrap();
    assert!(realization.notes.len() >= 4);
    let chosen = realization.notes[0].clone();
    card.setting.marked = vec![(chosen.string_index, chosen.physical_fret)];
    card.setting.mark_mode = MarkMode::Solo;
    let lowered = stage
        .practice_program(&set(vec![card]), 120.0)
        .cards
        .remove(0);
    assert!(lowered.unavailable.is_none());
    assert!(!lowered.events.is_empty());
    assert!(
        lowered
            .events
            .iter()
            .all(|event| event.contacts == vec![(chosen.string_index, chosen.physical_fret)])
    );
}

#[test]
fn chord_block_and_chord_approach_keep_realized_physical_contacts() {
    let mut stage = stage();
    stage.set_lens(crate::Lens::Chords);
    let chord = stage.card_from_lens().unwrap();
    let block = stage
        .practice_program(&set(vec![chord.clone()]), 120.0)
        .cards
        .remove(0);
    assert!(block.unavailable.is_none());
    assert_eq!(block.events.len(), 1, "Block is one grouped chord event");
    assert_eq!(block.events[0].strum_seconds, 0.018);
    assert!(block.events[0].pitches.len() >= 3);
    assert_eq!(
        block.events[0].contacts.len(),
        block.events[0].pitches.len()
    );

    let Material::Chord { name, root } = chord.material else {
        panic!("chord lens card")
    };
    let approach = Card {
        id: CardId(9),
        label: "Approach".into(),
        material: Material::ChordApproach {
            name,
            root,
            direction: woodshedding::rehearsal::ApproachDirection::Below,
        },
        setting: chord.setting,
        touch: Touch::Walk,
        timing: Timing::default(),
        from: None,
    };
    let lowered = stage
        .practice_program(&set(vec![approach]), 120.0)
        .cards
        .remove(0);
    assert!(
        lowered.unavailable.is_none(),
        "{}",
        lowered.unavailable.unwrap_or_default()
    );
    assert!(lowered.events.len() >= 2);
    assert!(
        lowered
            .events
            .iter()
            .all(|event| event.pitches.len() == 1 && !event.contacts.is_empty())
    );
}

#[test]
fn live_arpeggio_and_scale_run_lower_their_actual_cursor_paths() {
    let mut stage = stage();
    let arpeggio = stage.live_clock_card(LiveSequenceKind::Arpeggio, 100.0);
    assert!(arpeggio.unavailable.is_none());
    assert!(arpeggio.events.len() >= 3);
    assert!(
        arpeggio
            .events
            .iter()
            .all(|event| event.pitches.len() == 1 && event.contacts.len() == 1)
    );

    stage.set_lens(crate::Lens::Scales);
    let scale_run = stage.live_clock_card(LiveSequenceKind::ScaleRun, 100.0);
    assert!(scale_run.unavailable.is_none());
    assert!(!scale_run.events.is_empty());
    assert!(
        scale_run
            .events
            .iter()
            .all(|event| event.contacts.len() == 1)
    );
    assert!(
        stage
            .live_clock_card(LiveSequenceKind::ScaleRun, f32::NAN)
            .unavailable
            .is_some()
    );
}

#[test]
fn live_both_exercise_keeps_repeated_turnaround_event_identity() {
    let mut stage = stage();
    let index = stage
        .exercises()
        .iter()
        .position(|exercise| exercise.name == "Chromatic 1-2-3-4")
        .unwrap();
    stage.select_exercise(index);
    let card = stage.live_clock_card(LiveSequenceKind::Exercise, 120.0);
    assert!(card.unavailable.is_none());
    let repeated = card
        .events
        .windows(2)
        .find(|pair| pair[0].contacts == pair[1].contacts)
        .unwrap();
    assert_ne!(repeated[0].ordinal, repeated[1].ordinal);
    assert_eq!(repeated[0].pitches, repeated[1].pitches);
}

#[test]
fn invalid_setup_is_retained_as_unavailable_and_riff_silence_is_intentional() {
    let stage = stage();
    let mut scale = stage.card_from_lens().unwrap();
    scale.setting.fret_window = Some(FretWindow { start: 30, span: 2 });
    let riff = Card {
        id: CardId(8),
        label: "Chromatic 1-2-3-4".into(),
        material: Material::Riff {
            name: "Chromatic 1-2-3-4".into(),
        },
        setting: scale.setting.clone(),
        touch: Touch::Block,
        timing: Timing::default(),
        from: Some(Recipe::Exercise {
            name: "Chromatic 1-2-3-4".into(),
        }),
    };
    let program = stage.practice_program(&set(vec![scale, riff]), 120.0);
    assert_eq!(program.cards.len(), 2);
    assert!(program.cards[0].unavailable.is_some());
    assert!(program.cards[1].unavailable.is_none());
    assert!(program.cards[1].events.is_empty());
}
