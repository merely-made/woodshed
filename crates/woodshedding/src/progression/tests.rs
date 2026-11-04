use super::*;
use crate::pitch::{Accidental, NoteName};
use crate::scale::catalog as scale_catalog;

fn major_scale() -> &'static ScaleFormula {
    scale_catalog().iter().find(|s| s.name == "Major").unwrap()
}

fn nat(n: NoteName, o: i8) -> Pitch {
    Pitch::natural(n, o)
}

#[test]
fn ii_v_i_in_c_produces_dm7_g7_cmaj7() {
    let prog = catalog()
        .iter()
        .find(|p| p.name == "ii-V-I (Jazz)")
        .unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::C, 4), major_scale())
        .unwrap();
    assert_eq!(chords.len(), 3);

    // Dm7
    assert_eq!(chords[0].formula.name, "Minor 7");
    assert_eq!(chords[0].root, nat(NoteName::D, 4));

    // G7
    assert_eq!(chords[1].formula.name, "Dominant 7");
    assert_eq!(chords[1].root, nat(NoteName::G, 4));

    // Cmaj7 — degree 1 is the same C as the key root.
    assert_eq!(chords[2].formula.name, "Major 7");
    assert_eq!(chords[2].root, nat(NoteName::C, 4));
}

#[test]
fn ii_v_i_dm7_pitches_are_d_f_a_c() {
    let prog = catalog()
        .iter()
        .find(|p| p.name == "ii-V-I (Jazz)")
        .unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::C, 4), major_scale())
        .unwrap();
    let dm7 = &chords[0];
    let names: Vec<NoteName> = dm7.pitches.iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        vec![NoteName::D, NoteName::F, NoteName::A, NoteName::C]
    );
}

#[test]
fn i_iv_v_in_c_produces_c_f_g() {
    let prog = catalog().iter().find(|p| p.name == "I-IV-V").unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::C, 4), major_scale())
        .unwrap();
    let roots: Vec<NoteName> = chords.iter().map(|c| c.root.name).collect();
    assert_eq!(roots, vec![NoteName::C, NoteName::F, NoteName::G]);
    for chord in &chords {
        assert_eq!(chord.formula.name, "Major");
    }
}

#[test]
fn i_iv_v_in_d_produces_d_g_a() {
    let prog = catalog().iter().find(|p| p.name == "I-IV-V").unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::D, 4), major_scale())
        .unwrap();
    let roots: Vec<NoteName> = chords.iter().map(|c| c.root.name).collect();
    assert_eq!(roots, vec![NoteName::D, NoteName::G, NoteName::A]);
}

#[test]
fn pop_i_v_vi_iv_in_g_produces_g_d_em_c() {
    let prog = catalog().iter().find(|p| p.name == "I-V-vi-IV").unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::G, 3), major_scale())
        .unwrap();
    assert_eq!(chords[0].root, nat(NoteName::G, 3));
    assert_eq!(chords[0].formula.name, "Major");
    assert_eq!(chords[1].root, nat(NoteName::D, 4));
    assert_eq!(chords[1].formula.name, "Major");
    assert_eq!(chords[2].root, nat(NoteName::E, 4));
    assert_eq!(chords[2].formula.name, "Minor");
    assert_eq!(chords[3].root, nat(NoteName::C, 4));
    assert_eq!(chords[3].formula.name, "Major");
}

#[test]
fn andalusian_uses_flat_alterations() {
    // i-bVII-bVI-V in A minor: Am, G, F, E
    // (flat 7 of A is G, flat 6 of A is F)
    // Note: we apply this in the major-scale frame here. The
    // alterations carry the bVII and bVI semantics.
    let prog = catalog()
        .iter()
        .find(|p| p.name == "Andalusian Cadence")
        .unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::A, 3), major_scale())
        .unwrap();
    // i = Am
    assert_eq!(chords[0].root.name, NoteName::A);
    assert_eq!(chords[0].formula.name, "Minor");
    // bVII: A major scale's 7th degree is G#; flat → G
    assert_eq!(chords[1].root.name, NoteName::G);
    assert_eq!(chords[1].root.accidental, Accidental::Natural);
    // bVI: 6th of A major is F#; flat → F
    assert_eq!(chords[2].root.name, NoteName::F);
    assert_eq!(chords[2].root.accidental, Accidental::Natural);
    // V: 5th of A major = E
    assert_eq!(chords[3].root.name, NoteName::E);
    assert_eq!(chords[3].formula.name, "Major");
}

#[test]
fn twelve_bar_blues_has_twelve_chords() {
    let prog = catalog().iter().find(|p| p.name == "12-Bar Blues").unwrap();
    let chords = prog
        .apply_in_key(nat(NoteName::E, 3), major_scale())
        .unwrap();
    assert_eq!(chords.len(), 12);
    // All chords should be Dominant 7
    for c in &chords {
        assert_eq!(c.formula.name, "Dominant 7");
    }
}

#[test]
fn role_quality_chord_formula_lookup_works_for_every_variant() {
    // Every RoleQuality should map to a real chord in the catalog.
    use RoleQuality::*;
    for q in [
        Major,
        Minor,
        Diminished,
        Augmented,
        Sus2,
        Sus4,
        Major6,
        Minor6,
        Major7,
        Minor7,
        Dominant7,
        MinorMajor7,
        HalfDiminished7,
        Diminished7,
        Major9,
        Minor9,
        Dominant9,
    ] {
        assert!(
            q.chord_formula().is_some(),
            "RoleQuality::{q:?} → name {:?} not found in chord catalog",
            q.chord_formula_name()
        );
    }
}

#[test]
fn catalog_in_category_filters() {
    let jazz_count = catalog_in_category(ProgressionCategory::Jazz).count();
    assert!(jazz_count >= 2);
    let blues_count = catalog_in_category(ProgressionCategory::Blues).count();
    assert!(blues_count >= 2);
}

#[test]
fn out_of_range_degree_returns_error() {
    let major = major_scale();
    // Degree 8 is out of range for a 7-note major scale.
    static FAKE_ROLES: &[ChordRole] = &[ChordRole::new(8, RoleQuality::Major)];
    let fake = Progression {
        name: "fake",
        description: "",
        category: ProgressionCategory::Custom,
        roles: FAKE_ROLES,
    };
    let err = fake.apply_in_key(nat(NoteName::C, 4), major).unwrap_err();
    assert!(matches!(err, ProgressionError::DegreeOutOfRange { .. }));
}
