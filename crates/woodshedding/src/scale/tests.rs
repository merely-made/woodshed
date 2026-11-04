use super::*;
use crate::pitch::{Accidental, NoteName};

fn pitch(name: NoteName, acc: Accidental, octave: i8) -> Pitch {
    Pitch::new(name, acc, octave)
}

fn nat(name: NoteName, octave: i8) -> Pitch {
    Pitch::natural(name, octave)
}

fn find(name: &str) -> &'static ScaleFormula {
    catalog().iter().find(|s| s.name == name).unwrap()
}

#[test]
fn c_major_produces_c_d_e_f_g_a_b() {
    let major = find("Major");
    let scale = major.apply_to(nat(NoteName::C, 4)).unwrap();
    let expected = vec![
        nat(NoteName::C, 4),
        nat(NoteName::D, 4),
        nat(NoteName::E, 4),
        nat(NoteName::F, 4),
        nat(NoteName::G, 4),
        nat(NoteName::A, 4),
        nat(NoteName::B, 4),
    ];
    assert_eq!(scale, expected);
}

#[test]
fn d_major_produces_correct_sharps() {
    // D major: D E F# G A B C#
    let major = find("Major");
    let scale = major.apply_to(nat(NoteName::D, 4)).unwrap();
    let expected = vec![
        nat(NoteName::D, 4),
        nat(NoteName::E, 4),
        pitch(NoteName::F, Accidental::Sharp, 4),
        nat(NoteName::G, 4),
        nat(NoteName::A, 4),
        nat(NoteName::B, 4),
        pitch(NoteName::C, Accidental::Sharp, 5),
    ];
    assert_eq!(scale, expected);
}

#[test]
fn b_flat_major_produces_correct_flats() {
    // Bb major: Bb C D Eb F G A
    let major = find("Major");
    let bb4 = pitch(NoteName::B, Accidental::Flat, 4);
    let scale = major.apply_to(bb4).unwrap();
    let names: Vec<_> = scale.iter().map(|p| (p.name, p.accidental)).collect();
    assert_eq!(
        names,
        vec![
            (NoteName::B, Accidental::Flat),
            (NoteName::C, Accidental::Natural),
            (NoteName::D, Accidental::Natural),
            (NoteName::E, Accidental::Flat),
            (NoteName::F, Accidental::Natural),
            (NoteName::G, Accidental::Natural),
            (NoteName::A, Accidental::Natural),
        ]
    );
}

#[test]
fn c_minor_produces_c_d_eb_f_g_ab_bb() {
    let minor = find("Minor");
    let scale = minor.apply_to(nat(NoteName::C, 4)).unwrap();
    let expected = vec![
        nat(NoteName::C, 4),
        nat(NoteName::D, 4),
        pitch(NoteName::E, Accidental::Flat, 4),
        nat(NoteName::F, 4),
        nat(NoteName::G, 4),
        pitch(NoteName::A, Accidental::Flat, 4),
        pitch(NoteName::B, Accidental::Flat, 4),
    ];
    assert_eq!(scale, expected);
}

#[test]
fn lydian_has_sharp_four() {
    let lydian = find("Lydian");
    let c_lydian = lydian.apply_to(nat(NoteName::C, 4)).unwrap();
    // 4th degree should be F#
    assert_eq!(c_lydian[3], pitch(NoteName::F, Accidental::Sharp, 4));
}

#[test]
fn phrygian_dominant_is_flamenco_scale() {
    // E phrygian dominant: E F G# A B C D
    let pd = find("Phrygian Dominant");
    let scale = pd.apply_to(nat(NoteName::E, 4)).unwrap();
    let expected = vec![
        nat(NoteName::E, 4),
        nat(NoteName::F, 4),
        pitch(NoteName::G, Accidental::Sharp, 4),
        nat(NoteName::A, 4),
        nat(NoteName::B, 4),
        nat(NoteName::C, 5),
        nat(NoteName::D, 5),
    ];
    assert_eq!(scale, expected);
}

#[test]
fn altered_scale_pitches_match_jazz_convention() {
    // C altered: C Db Eb Fb Gb Ab Bb
    let altered = find("Altered");
    let scale = altered.apply_to(nat(NoteName::C, 4)).unwrap();
    let expected = vec![
        nat(NoteName::C, 4),
        pitch(NoteName::D, Accidental::Flat, 4),
        pitch(NoteName::E, Accidental::Flat, 4),
        pitch(NoteName::F, Accidental::Flat, 4),
        pitch(NoteName::G, Accidental::Flat, 4),
        pitch(NoteName::A, Accidental::Flat, 4),
        pitch(NoteName::B, Accidental::Flat, 4),
    ];
    assert_eq!(scale, expected);
}

#[test]
fn whole_tone_has_six_pitches() {
    let wt = find("Whole Tone");
    let scale = wt.apply_to(nat(NoteName::C, 4)).unwrap();
    assert_eq!(scale.len(), 6);
}

#[test]
fn diminished_scales_have_eight_pitches() {
    let wh = find("Diminished (whole-half)");
    let hw = find("Diminished (half-whole)");
    assert_eq!(wh.degree_count(), 8);
    assert_eq!(hw.degree_count(), 8);
}

#[test]
fn chromatic_has_twelve_pitches() {
    let chrom = find("Chromatic");
    assert_eq!(chrom.degree_count(), 12);
}

#[test]
fn pentatonic_has_five_pitches() {
    let pent = find("Major Pentatonic");
    assert_eq!(pent.degree_count(), 5);
}

#[test]
fn round_trip_recognize_c_major() {
    let major = find("Major");
    let pitches = major.apply_to(nat(NoteName::C, 4)).unwrap();
    let recognized = recognize(&pitches);
    assert!(recognized.iter().any(|f| f.name == "Major"));
}

#[test]
fn round_trip_recognize_d_dorian() {
    let dorian = find("Dorian");
    let pitches = dorian.apply_to(nat(NoteName::D, 4)).unwrap();
    let recognized = recognize(&pitches);
    assert!(recognized.iter().any(|f| f.name == "Dorian"));
}

#[test]
fn round_trip_recognize_minor_pentatonic() {
    let pent = find("Minor Pentatonic");
    let pitches = pent.apply_to(nat(NoteName::A, 4)).unwrap();
    let recognized = recognize(&pitches);
    assert!(recognized.iter().any(|f| f.name == "Minor Pentatonic"));
}

#[test]
fn catalog_in_category_filters_correctly() {
    let diatonic_count = catalog_in_category(ScaleCategory::Diatonic).count();
    assert_eq!(diatonic_count, 7); // 7 diatonic modes
    let pent_count = catalog_in_category(ScaleCategory::Pentatonic).count();
    assert_eq!(pent_count, 2);
}

#[test]
fn all_catalog_scales_apply_cleanly_to_c() {
    // Sanity: every formula produces a result for C4 root.
    for f in catalog() {
        let result = f.apply_to(nat(NoteName::C, 4));
        assert!(
            result.is_ok(),
            "scale {} failed to apply: {:?}",
            f.name,
            result
        );
    }
}
