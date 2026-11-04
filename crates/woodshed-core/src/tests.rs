use super::*;

#[test]
fn default_state_resolves_dots() {
    let s = StageState::new();
    let dots = s.dots();
    assert!(!dots.is_empty(), "minor pentatonic on standard tuning");
    assert!(dots.iter().any(|d| d.is_root), "root positions present");
    assert!(dots.iter().all(|d| d.fret <= s.fret_count));
    assert!(dots.iter().all(|d| d.string_index < s.string_count()));
}

#[test]
fn select_scale_changes_dots() {
    let mut s = StageState::new();
    let before = s.dots().len();
    let major = s
        .scales()
        .iter()
        .position(|f| f.name == "Major")
        .expect("Major in catalog");
    s.select_scale(major);
    assert_eq!(s.scale().name, "Major");
    assert_ne!(
        s.dots().len(),
        before,
        "major has more tones than pentatonic"
    );
}

#[test]
fn chord_lens_resolves_chord_tones() {
    let mut s = StageState::new();
    s.set_lens(Lens::Chords);
    let dots = s.dots();
    assert!(!dots.is_empty(), "chord tones on standard tuning");
    assert!(dots.iter().any(|d| d.is_root));
}

#[test]
fn root_change_transposes() {
    let mut s = StageState::new();
    let a_dots: Vec<_> = s.dots().iter().map(|d| (d.string_index, d.fret)).collect();
    s.set_root(3); // C
    let c_dots: Vec<_> = s.dots().iter().map(|d| (d.string_index, d.fret)).collect();
    assert_ne!(a_dots, c_dots, "different root, different positions");
    assert_eq!(s.root_name(), "C");
}

#[test]
fn fret_window_filters_card_dots() {
    let mut s = StageState::new();
    s.set_lens(Lens::Scales);
    let mut card = s.card_from_lens().unwrap();
    let all = s.dots_for_card(&card).len();
    card.setting.fret_window = Some(woodshedding::rehearsal::FretWindow { start: 5, span: 4 });
    let windowed = s.dots_for_card(&card);
    assert!(!windowed.is_empty());
    assert!(windowed.len() < all, "window narrows the position set");
    assert!(windowed.iter().all(|d| d.fret >= 5 && d.fret <= 9));
}

#[test]
fn retained_positive_tempo_is_shared_by_dwell_and_ordered_preview_outside_ui_range() {
    use woodshedding::rehearsal::{ArpeggioDirection, Hold, Touch};
    let mut stage = StageState::new();
    stage.set_lens(Lens::Chords);
    let mut card = stage.card_from_lens().unwrap();
    card.timing.hold = Hold::Bars(1);
    card.touch = Touch::Arpeggiate {
        direction: ArpeggioDirection::Up,
        inversion: 0,
    };
    for bpm in [20.0, 400.0] {
        card.timing.bpm = Some(bpm);
        let retained = serde_json::to_value(&card).unwrap();
        let dwell = card_dwell(&card, 120.0).unwrap().unwrap().as_secs_f32();
        let (_, duration, _) = stage.card_sounding_pitches_at_tempo(&card, 120.0).unwrap();
        assert!((dwell - 240.0 / bpm).abs() < 0.0001);
        assert!((duration - dwell).abs() < 0.0001);
        assert_eq!(serde_json::to_value(&card).unwrap(), retained);
        let mut scale = StageState::new().card_from_lens().unwrap();
        scale.touch = Touch::Walk;
        scale.timing = card.timing.clone();
        let (_, scale_duration, _) = stage.card_sounding_pitches_at_tempo(&scale, 90.0).unwrap();
        assert!((scale_duration - dwell).abs() < 0.0001);
    }
}

#[test]
fn card_dwell_follows_hold() {
    use woodshedding::rehearsal::Hold;
    let s = StageState::new();
    let mut card = s.card_from_lens().unwrap();
    assert!(
        card_dwell(&card, 120.0).unwrap().is_none(),
        "manual by default"
    );
    card.timing.hold = Hold::Bars(2);
    let d = card_dwell(&card, 120.0).unwrap().unwrap();
    assert!((d.as_secs_f32() - 4.0).abs() < 0.01, "2 bars at 120 = 4s");
    card.timing.bpm = Some(60.0);
    let d = card_dwell(&card, 120.0).unwrap().unwrap();
    assert!((d.as_secs_f32() - 8.0).abs() < 0.01, "card bpm wins");
}

#[test]
fn practice_sets_fill_rehearsal_sets() {
    let catalog = woodshedding::practice::catalog();
    assert!(!catalog.is_empty());
    let ps = &catalog[0];
    let set = set_from_practice(ps);
    assert_eq!(set.cards.len(), ps.items.len());
    assert!(
        set.cards
            .iter()
            .all(|c| matches!(c.from, Some(Recipe::PracticeSet { .. })))
    );
    assert!(set.cards.iter().all(|c| c.setting.fret_window.is_some()));
    // Every card resolves on the board.
    let s = StageState::new();
    assert!(set.cards.iter().all(|c| !s.dots_for_card(c).is_empty()));
}

#[test]
fn all_lenses_implemented() {
    assert!(Lens::ALL.iter().all(|l| l.implemented()));
}

#[test]
fn exercise_board_steps_with_trail() {
    let mut s = StageState::new();
    s.set_lens(Lens::Exercises);
    let b0 = s.exercise_board();
    assert!(b0.total > 1, "exercise generates a sequence");
    assert_eq!(b0.dots.len(), 1, "cold start shows only the current step");
    for _ in 0..EXERCISE_TRAIL + 2 {
        s.exercise_advance();
    }
    let b = s.exercise_board();
    assert!(b.dots.len() > 1, "trail appears behind the cursor");
    assert!(b.dots.len() <= EXERCISE_TRAIL + 1);
    assert_eq!(b.dots.iter().filter(|d| d.recency == 0).count(), 1);
    // Fret nudge clamps and resets the cursor.
    s.exercise_nudge_fret(100);
    assert_eq!(s.exercise_starting_fret, s.fret_count - 3);
    assert_eq!(s.exercise_step_idx, 0);
    s.exercise_nudge_fret(-100);
    assert_eq!(s.exercise_starting_fret, 0);
}

#[test]
fn progression_board_materializes_in_key() {
    let mut s = StageState::new();
    s.set_lens(Lens::Progressions);
    assert!(s.progression_board().is_none(), "cold start prompts");
    s.select_progression(0);
    let b = s.progression_board().expect("board after selection");
    assert!(!b.cards.is_empty());
    assert_eq!(b.cards.iter().filter(|c| c.is_expanded).count(), 1);
    assert!(!b.dots.is_empty(), "expanded chord resolves tones");
    // Expanding another card moves the expansion and changes the label.
    if b.cards.len() > 1 {
        let first = b.expanded_label.clone();
        s.progression_expand(1);
        let b2 = s.progression_board().unwrap();
        assert!(b2.cards[1].is_expanded);
        assert_ne!(b2.expanded_label, first);
    }
}

#[test]
fn format_role_covers_common_shapes() {
    use woodshedding::progression::{ChordRole, DegreeAlteration, RoleQuality};
    assert_eq!(format_role(&ChordRole::new(1, RoleQuality::Major)), "I");
    assert_eq!(format_role(&ChordRole::new(2, RoleQuality::Minor7)), "iim7");
    assert_eq!(
        format_role(&ChordRole::new(5, RoleQuality::Dominant7)),
        "V7"
    );
    assert_eq!(
        format_role(&ChordRole::altered(
            7,
            DegreeAlteration::Flat,
            RoleQuality::Major
        )),
        "♭VII"
    );
}

#[test]
fn arpeggio_board_resolves_and_steps() {
    let mut s = StageState::new();
    s.set_lens(Lens::Arpeggios);
    let b0 = s.arpeggio_board();
    assert!(!b0.dots.is_empty());
    assert!(b0.walk_len >= 1);
    assert_eq!(b0.dots.iter().filter(|d| d.is_current).count(), 1);
    let cur0: Vec<_> = b0.dots.iter().map(|d| d.is_current).collect();
    s.arpeggio_advance();
    let b1 = s.arpeggio_board();
    let cur1: Vec<_> = b1.dots.iter().map(|d| d.is_current).collect();
    assert_ne!(cur0, cur1, "advance moves the highlight");
    s.arpeggio_cycle_inversion();
    let b2 = s.arpeggio_board();
    assert_eq!(b2.step, 0, "inversion change resets the transport");
}

#[test]
fn voicing_pitches_per_lens() {
    let mut s = StageState::new();
    s.set_lens(Lens::Scales);
    assert!(s.voicing_pitches().len() >= 5, "scale tones voiced");
    assert!(
        s.voicing_pitches().iter().all(|f| *f > 20.0 && *f < 5000.0),
        "frequencies in audible range"
    );
    s.set_lens(Lens::Chords);
    assert!(s.voicing_pitches().len() >= 3, "chord tones voiced");
    s.set_lens(Lens::Arpeggios);
    assert!(s.voicing_pitches().len() >= 3, "arpeggio tones voiced");
    s.set_lens(Lens::Exercises);
    assert!(
        s.voicing_pitches().is_empty(),
        "exercises don't voice a chord"
    );
}

#[test]
fn voicing_preview_shapes_scale_vs_chord() {
    let mut s = StageState::new();
    s.set_lens(Lens::Scales);
    let (p, dur, strum) = s.voicing_preview();
    assert!(!p.is_empty() && dur > 0.0);
    assert!(strum > 100.0, "scale cascades with a wide strum offset");
    s.set_lens(Lens::Chords);
    let (_p, _dur, strum) = s.voicing_preview();
    assert!(strum < 50.0, "chord strums tight");
}

#[test]
fn progression_voicing_after_selection() {
    let mut s = StageState::new();
    s.set_lens(Lens::Progressions);
    assert!(
        s.voicing_pitches().is_empty(),
        "cold start, nothing to voice"
    );
    s.select_progression(0);
    assert!(s.voicing_pitches().len() >= 2, "expanded chord voiced");
}

#[test]
fn arpeggio_step_pitch_tracks_transport() {
    let mut s = StageState::new();
    s.set_lens(Lens::Arpeggios);
    let p0 = s.arpeggio_current_pitch_hz().expect("a current tone");
    assert!(p0 > 20.0);
    let mut seen_change = false;
    for _ in 0..8 {
        s.arpeggio_advance();
        if let Some(p) = s.arpeggio_current_pitch_hz() {
            if (p - p0).abs() > 0.1 {
                seen_change = true;
            }
        }
    }
    assert!(seen_change, "arpeggio walk visits multiple pitches");
}

#[test]
fn card_voicing_matches_material() {
    let mut s = StageState::new();
    s.set_lens(Lens::Chords);
    let card = s.card_from_lens().unwrap();
    let (pitches, dur, strum) = s.card_voicing(&card).unwrap();
    assert!(pitches.len() >= 3, "chord card voices its tones");
    assert!(dur > 0.0 && strum < 50.0);
    // A riff (exercise) card doesn't voice.
    s.set_lens(Lens::Exercises);
    let riff = s.card_from_lens().unwrap();
    assert!(s.card_voicing(&riff).unwrap().0.is_empty());
}

#[test]
fn cards_staged_from_a_lens_store_the_active_instrument_identity() {
    let mut s = StageState::new();
    s.set_lens(Lens::Chords);
    let card = s.card_from_lens().expect("chord lens stages a card");
    assert_eq!(card.setting.instrument, s.tuning().instrument.to_string());
}

#[test]
fn card_sounding_pitches_respects_mark_mode() {
    let mut s = StageState::new();
    s.set_lens(Lens::Chords);
    let mut card = s.card_from_lens().unwrap();

    // Off (nothing marked): the whole voicing.
    let full = s.card_sounding_pitches(&card).unwrap().0;
    assert!(full.len() >= 3, "off plays the whole voicing");

    // Mark one board position.
    let dots = s.dots_for_card(&card);
    let target = dots.first().expect("chord card has positions");
    let target_pc = pc_from_hz(target.frequency);
    card.setting.marked = vec![(target.string_index, target.fret)];

    // Mute: the marked note's pitch class drops from the voicing.
    card.setting.mark_mode = MarkMode::Mute;
    let muted = s.card_sounding_pitches(&card).unwrap().0;
    assert!(
        muted.iter().all(|&f| pc_from_hz(f) != target_pc),
        "mute drops the marked note's pitch class"
    );
    assert!(muted.len() < full.len(), "mute removes at least one tone");

    // Solo: only the marked position sounds.
    card.setting.mark_mode = MarkMode::Solo;
    let solo = s.card_sounding_pitches(&card).unwrap().0;
    assert_eq!(solo.len(), 1, "solo plays only the marked position");
    assert!(
        (solo[0] - target.frequency).abs() < 0.5,
        "solo plays the marked pitch"
    );
}

#[test]
fn exercise_step_pitch_available() {
    let mut s = StageState::new();
    s.set_lens(Lens::Exercises);
    let p = s.exercise_current_pitch_hz().expect("exercise has notes");
    assert!(p > 20.0 && p < 5000.0);
}

#[path = "tests/navigation.rs"]
mod navigation;
