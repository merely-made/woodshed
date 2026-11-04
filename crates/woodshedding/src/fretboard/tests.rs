use super::*;
use crate::chord::catalog as chord_catalog;
use crate::pitch::Spelling;
use crate::scale::catalog as scale_catalog;
use crate::tuning::{Instrument, Tuning};

fn standard_guitar() -> Fretboard {
    Fretboard::new(
        Tuning::find_for("Standard", Instrument::Guitar).unwrap(),
        22,
    )
}

fn nat(name: NoteName, octave: i8) -> Pitch {
    Pitch::natural(name, octave)
}

fn find_scale(name: &str) -> &'static ScaleFormula {
    scale_catalog().iter().find(|s| s.name == name).unwrap()
}

fn find_chord(name: &str) -> &'static ChordFormula {
    chord_catalog().iter().find(|c| c.name == name).unwrap()
}

#[test]
fn pitch_at_open_strings_matches_tuning() {
    let board = standard_guitar();
    for (i, &open) in board.tuning.strings.iter().enumerate() {
        assert_eq!(board.pitch_at(i, 0).midi(), open.midi());
    }
}

#[test]
fn pitch_at_fret_5_string_6_is_a2() {
    let board = standard_guitar();
    // Standard 6th string (index 0) is E2; fret 5 is A2 (MIDI 45).
    assert_eq!(board.pitch_at(0, 5).midi(), 45);
}

#[test]
fn c_major_scale_positions_cover_every_string() {
    let board = standard_guitar();
    let major = find_scale("Major");
    let positions = board
        .positions_for_scale(major, nat(NoteName::C, 4))
        .unwrap();
    for s in 0..board.tuning.strings.len() {
        assert!(
            positions.iter().any(|p| p.string_index == s),
            "string {s} has no scale positions"
        );
    }
}

#[test]
fn c_major_scale_positions_only_use_pitch_classes_in_scale() {
    let board = standard_guitar();
    let major = find_scale("Major");
    let positions = board
        .positions_for_scale(major, nat(NoteName::C, 4))
        .unwrap();
    // C major pitch classes: 0,2,4,5,7,9,11
    let allowed = [0u8, 2, 4, 5, 7, 9, 11];
    for p in &positions {
        assert!(allowed.contains(&p.pitch.pitch_class()));
    }
}

#[test]
fn d_major_scale_positions_use_sharp_spelling() {
    // D major has F# and C#. Fretboard positions should preserve those.
    let board = standard_guitar();
    let major = find_scale("Major");
    let positions = board
        .positions_for_scale(major, nat(NoteName::D, 4))
        .unwrap();
    let f_sharp_count = positions
        .iter()
        .filter(|p| p.pitch.name == NoteName::F && p.pitch.accidental == Accidental::Sharp)
        .count();
    assert!(
        f_sharp_count > 0,
        "expected F# spellings in D major scale positions"
    );
}

#[test]
fn b_flat_major_scale_positions_use_flat_spelling() {
    let board = standard_guitar();
    let major = find_scale("Major");
    let bb4 = Pitch::new(NoteName::B, Accidental::Flat, 4);
    let positions = board.positions_for_scale(major, bb4).unwrap();
    let e_flat_count = positions
        .iter()
        .filter(|p| p.pitch.name == NoteName::E && p.pitch.accidental == Accidental::Flat)
        .count();
    assert!(
        e_flat_count > 0,
        "expected Eb spellings in Bb major scale positions"
    );
}

#[test]
fn c_major_chord_positions_only_contain_chord_tones() {
    let board = standard_guitar();
    let major = find_chord("Major");
    let positions = board
        .positions_for_chord(major, nat(NoteName::C, 4))
        .unwrap();
    let allowed = [0u8, 4, 7]; // C E G
    for p in &positions {
        assert!(allowed.contains(&p.pitch.pitch_class()));
    }
}

#[test]
fn first_position_e_major_voicing_is_found() {
    // The classic open E major chord: 0-2-2-1-0-0
    let board = standard_guitar();
    let e_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings(e_major, nat(NoteName::E, 2), 0, 4)
        .unwrap();
    let target = vec![Some(0), Some(2), Some(2), Some(1), Some(0), Some(0)];
    let found = voicings.iter().any(|v| v.fret_pattern() == target);
    assert!(
        found,
        "expected open E major shape (0-2-2-1-0-0); voicings = {:?}",
        voicings
            .iter()
            .map(|v| v.fret_pattern())
            .collect::<Vec<_>>()
    );
}

#[test]
fn voicings_satisfy_root_in_bass() {
    let board = standard_guitar();
    let e_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings(e_major, nat(NoteName::E, 2), 0, 4)
        .unwrap();
    for v in &voicings {
        let lowest = v
            .strings
            .iter()
            .find_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .unwrap();
        assert_eq!(lowest, nat(NoteName::E, 2).pitch_class());
    }
}

#[test]
fn root_bass_uses_sounding_order_for_reentrant_tunings() {
    let board = Fretboard::new(
        Tuning::find_for("Standard (high-G)", Instrument::Ukulele).unwrap(),
        15,
    );
    let c_major = find_chord("Major");
    let root = nat(NoteName::C, 4);
    let voicings = board.find_chord_voicings(c_major, root, 0, 4).unwrap();
    assert!(
        !voicings.is_empty(),
        "a C-major root-bass shape fits high-G uke"
    );
    for voicing in voicings {
        let actual_bass = voicing
            .strings
            .iter()
            .filter_map(|play| play.pitch())
            .min_by_key(|pitch| pitch.midi())
            .expect("played strings")
            .pitch_class();
        assert_eq!(actual_bass, root.pitch_class(), "{voicing:?}");
    }
}

#[test]
fn voicings_contain_all_chord_tones() {
    let board = standard_guitar();
    let e_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings(e_major, nat(NoteName::E, 2), 0, 4)
        .unwrap();
    for v in &voicings {
        let mut pcs: Vec<u8> = v
            .strings
            .iter()
            .filter_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .collect();
        pcs.sort_unstable();
        pcs.dedup();
        for pc in &[4u8, 8, 11] {
            assert!(pcs.contains(pc), "voicing missing pc {pc}: {:?}", v);
        }
    }
}

#[test]
fn voicings_respect_fret_span() {
    let board = standard_guitar();
    let g_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings(g_major, nat(NoteName::G, 2), 3, 4)
        .unwrap();
    for v in &voicings {
        assert!(v.fret_span() <= 4, "span exceeds 4: {:?}", v);
    }
}

#[test]
fn fretboard_works_with_alternative_tunings() {
    // Drop D should let us find a D power chord shape across the
    // bottom three strings (0-0-0) at standard hand position.
    let drop_d = Tuning::find_for("Drop D", Instrument::Guitar).unwrap();
    let board = Fretboard::new(drop_d, 22);
    let major = find_chord("Major");
    let positions = board
        .positions_for_chord(major, nat(NoteName::D, 2))
        .unwrap();
    // The lowest string is D2 (open); fret 0 should yield a D position.
    assert!(positions.iter().any(|p| p.string_index == 0 && p.fret == 0));
}

#[test]
fn ukulele_c_major_positions_exist() {
    let uke = Tuning::find_for("Standard (high-G)", Instrument::Ukulele).unwrap();
    let board = Fretboard::new(uke, 18);
    let major = find_chord("Major");
    let positions = board
        .positions_for_chord(major, nat(NoteName::C, 4))
        .unwrap();
    assert!(!positions.is_empty());
}

#[test]
fn pitch_at_uses_sharps_by_default() {
    let board = standard_guitar();
    // String 6 fret 1 is F (= MIDI 41 = F)
    assert_eq!(board.pitch_at(0, 1), Pitch::natural(NoteName::F, 2));
    // String 6 fret 2 is F# (sharps spelling)
    let fs2 = board.pitch_at(0, 2);
    assert_eq!(fs2, Pitch::from_midi(42, Spelling::Sharps));
}

#[test]
fn voicing_count_is_finite_and_reasonable() {
    // Sanity check: E major in window 0..4 should produce a small
    // double-digit number of voicings, not thousands.
    let board = standard_guitar();
    let e_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings(e_major, nat(NoteName::E, 2), 0, 4)
        .unwrap();
    assert!(!voicings.is_empty());
    assert!(
        voicings.len() < 200,
        "too many voicings: {}",
        voicings.len()
    );
}

#[test]
fn bounded_search_marks_a_result_limited_prefix_incomplete() {
    let board = standard_guitar();
    let major = find_chord("Major");
    let search = board
        .find_chord_voicings_bounded(major, nat(NoteName::E, 2), 0, 4, 50_000, 1)
        .unwrap();
    assert_eq!(search.voicings.len(), 1);
    assert!(
        board
            .find_chord_voicings_bounded(major, nat(NoteName::E, 2), 0, 4, 50_000, 0)
            .unwrap()
            .voicings
            .is_empty()
    );
    assert!(
        !search.complete,
        "a caller-visible result cap must never claim a complete inventory"
    );
}

// === Inversions and slash chords ===

#[test]
fn any_chord_tone_bass_finds_inversions() {
    // C major = C E G. With AnyChordTone bass, we should find
    // voicings with E in bass (1st inversion) AND G in bass (2nd
    // inversion), in addition to root position.
    let board = standard_guitar();
    let c_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings_for_bass(
            c_major,
            nat(NoteName::C, 4),
            0,
            4,
            BassConstraint::AnyChordTone,
        )
        .unwrap();
    let bass_pcs: Vec<u8> = voicings
        .iter()
        .map(|v| {
            v.strings
                .iter()
                .find_map(|s| match s {
                    StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                    _ => None,
                })
                .unwrap()
        })
        .collect();
    // C=0, E=4, G=7
    assert!(bass_pcs.contains(&0), "expected root-position voicings");
    assert!(
        bass_pcs.contains(&4),
        "expected 1st-inversion voicings (E bass)"
    );
    assert!(
        bass_pcs.contains(&7),
        "expected 2nd-inversion voicings (G bass)"
    );
}

#[test]
fn slash_chord_with_chord_tone_bass_works() {
    // C/G — C major with G in the bass. Same notes as 2nd
    // inversion C, but the slash-chord notation is more explicit.
    let board = standard_guitar();
    let c_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings_for_bass(
            c_major,
            nat(NoteName::C, 4),
            0,
            4,
            BassConstraint::Pitch(7), // G = pitch class 7
        )
        .unwrap();
    assert!(!voicings.is_empty());
    for v in &voicings {
        let bass_pc = v
            .strings
            .iter()
            .find_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .unwrap();
        assert_eq!(bass_pc, 7, "bass must be G in C/G");
    }
}

#[test]
fn slash_chord_with_non_chord_tone_bass_works() {
    // Cmaj7/F# — Cmaj7 (C E G B) with F# in the bass. F# is NOT a
    // chord tone, but a slash chord allows arbitrary bass.
    let board = standard_guitar();
    let cmaj7 = find_chord("Major 7");
    let voicings = board
        .find_chord_voicings_for_bass(
            cmaj7,
            nat(NoteName::C, 4),
            0,
            5,
            BassConstraint::Pitch(6), // F# = pitch class 6
        )
        .unwrap();
    assert!(
        !voicings.is_empty(),
        "expected at least one Cmaj7/F# voicing"
    );
    for v in &voicings {
        let bass_pc = v
            .strings
            .iter()
            .find_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .unwrap();
        assert_eq!(bass_pc, 6, "bass must be F# (pc 6) in Cmaj7/F#");
        // All chord tones (C E G B = 0 4 7 11) must still appear.
        let mut pcs: Vec<u8> = v
            .strings
            .iter()
            .filter_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .collect();
        pcs.sort_unstable();
        pcs.dedup();
        for required in &[0u8, 4, 7, 11] {
            assert!(
                pcs.contains(required),
                "voicing missing chord tone {required}: {:?}",
                v
            );
        }
    }
}

#[test]
fn slash_chord_bass_uses_none_interval_for_non_chord_tone() {
    // The F# in Cmaj7/F# should have interval_from_root = None on
    // the StringPlay, since F# is not a Cmaj7 chord tone.
    let board = standard_guitar();
    let cmaj7 = find_chord("Major 7");
    let voicings = board
        .find_chord_voicings_for_bass(cmaj7, nat(NoteName::C, 4), 0, 5, BassConstraint::Pitch(6))
        .unwrap();
    let v = voicings.first().expect("at least one voicing");
    let bass_play = v.strings.iter().find(|s| s.is_played()).unwrap();
    match bass_play {
        StringPlay::Played {
            interval_from_root,
            pitch,
            ..
        } => {
            assert_eq!(pitch.pitch_class(), 6);
            assert_eq!(*interval_from_root, None);
        },
        _ => unreachable!(),
    }
}

#[test]
fn root_constraint_default_excludes_inversions() {
    // The original find_chord_voicings (defaults to Root) should
    // never produce voicings with non-root bass.
    let board = standard_guitar();
    let c_major = find_chord("Major");
    let voicings = board
        .find_chord_voicings(c_major, nat(NoteName::C, 4), 0, 4)
        .unwrap();
    for v in &voicings {
        let bass_pc = v
            .strings
            .iter()
            .find_map(|s| match s {
                StringPlay::Played { pitch, .. } => Some(pitch.pitch_class()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            bass_pc, 0,
            "Root constraint must produce only C-bass voicings"
        );
    }
}
