use super::*;
use crate::pitch::{Accidental, NoteName};

fn nat(name: NoteName, octave: i8) -> Pitch {
    Pitch::natural(name, octave)
}

fn pitch(name: NoteName, acc: Accidental, octave: i8) -> Pitch {
    Pitch::new(name, acc, octave)
}

fn find(name: &str) -> &'static ChordFormula {
    catalog().iter().find(|c| c.name == name).unwrap()
}

#[test]
fn c_major_triad_is_c_e_g() {
    let major = find("Major");
    let c = major.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            nat(NoteName::E, 4),
            nat(NoteName::G, 4)
        ]
    );
}

#[test]
fn d_major_triad_includes_f_sharp() {
    let major = find("Major");
    let d = major.apply_to(nat(NoteName::D, 4)).unwrap();
    assert_eq!(
        d,
        vec![
            nat(NoteName::D, 4),
            pitch(NoteName::F, Accidental::Sharp, 4),
            nat(NoteName::A, 4),
        ]
    );
}

#[test]
fn c_minor_triad_is_c_eb_g() {
    let minor = find("Minor");
    let cm = minor.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        cm,
        vec![
            nat(NoteName::C, 4),
            pitch(NoteName::E, Accidental::Flat, 4),
            nat(NoteName::G, 4),
        ]
    );
}

#[test]
fn cmaj7_is_c_e_g_b() {
    let m7 = find("Major 7");
    let c = m7.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            nat(NoteName::E, 4),
            nat(NoteName::G, 4),
            nat(NoteName::B, 4),
        ]
    );
}

#[test]
fn c7_is_c_e_g_bb() {
    let dom7 = find("Dominant 7");
    let c = dom7.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            nat(NoteName::E, 4),
            nat(NoteName::G, 4),
            pitch(NoteName::B, Accidental::Flat, 4),
        ]
    );
}

#[test]
fn cm7b5_is_c_eb_gb_bb() {
    let hd = find("Half-Diminished 7");
    let c = hd.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            pitch(NoteName::E, Accidental::Flat, 4),
            pitch(NoteName::G, Accidental::Flat, 4),
            pitch(NoteName::B, Accidental::Flat, 4),
        ]
    );
}

#[test]
fn cdim7_uses_double_flat_for_seventh() {
    // C dim7: C Eb Gb Bbb
    let dim7 = find("Diminished 7");
    let c = dim7.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(c[3], pitch(NoteName::B, Accidental::DoubleFlat, 4));
}

#[test]
fn caug_is_c_e_g_sharp() {
    let aug = find("Augmented");
    let c = aug.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            nat(NoteName::E, 4),
            pitch(NoteName::G, Accidental::Sharp, 4),
        ]
    );
}

#[test]
fn csus4_is_c_f_g() {
    let s = find("Suspended 4th");
    let c = s.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            nat(NoteName::F, 4),
            nat(NoteName::G, 4)
        ]
    );
}

#[test]
fn c7sharp9_uses_d_sharp_not_e_flat() {
    // The defining feature of #9 spelling: it's an augmented 9 (D#),
    // not a minor 3rd compound (Eb), even though they sound identical.
    let alt = find("Dominant 7 #9");
    let c = alt.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(c[4], pitch(NoteName::D, Accidental::Sharp, 5));
}

#[test]
fn cmaj13_has_seven_notes() {
    let m13 = find("Major 13");
    assert_eq!(m13.note_count(), 7);
}

#[test]
fn quartal_triad_is_c_f_bb() {
    let q = find("Quartal Triad");
    let c = q.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(
        c,
        vec![
            nat(NoteName::C, 4),
            nat(NoteName::F, 4),
            pitch(NoteName::B, Accidental::Flat, 4),
        ]
    );
}

#[test]
fn quintal_triad_is_c_g_d() {
    let q = find("Quintal Triad");
    let c = q.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(c[1], nat(NoteName::G, 4));
    assert_eq!(c[2], nat(NoteName::D, 5));
}

#[test]
fn round_trip_recognize_major_triad() {
    let major = find("Major");
    let pitches = major.apply_to(nat(NoteName::E, 4)).unwrap();
    let recognized = recognize(&pitches);
    assert!(recognized.iter().any(|f| f.name == "Major"));
}

#[test]
fn round_trip_recognize_dominant_7() {
    let dom7 = find("Dominant 7");
    let pitches = dom7.apply_to(nat(NoteName::G, 3)).unwrap();
    let recognized = recognize(&pitches);
    assert!(recognized.iter().any(|f| f.name == "Dominant 7"));
}

#[test]
fn round_trip_recognize_half_diminished() {
    let hd = find("Half-Diminished 7");
    let pitches = hd.apply_to(nat(NoteName::B, 3)).unwrap();
    let recognized = recognize(&pitches);
    assert!(recognized.iter().any(|f| f.name == "Half-Diminished 7"));
}

#[test]
fn catalog_in_category_filters_correctly() {
    let triads = catalog_in_category(ChordCategory::Triad).count();
    assert_eq!(triads, 4); // major, minor, augmented, diminished
    let altered = catalog_in_category(ChordCategory::Altered).count();
    assert!(altered >= 6);
}

#[test]
fn all_catalog_chords_apply_cleanly_to_c() {
    for f in catalog() {
        let result = f.apply_to(nat(NoteName::C, 4));
        assert!(
            result.is_ok(),
            "chord {} failed to apply: {:?}",
            f.name,
            result
        );
    }
}
