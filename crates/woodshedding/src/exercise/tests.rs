use super::*;
use crate::tuning::{Instrument, Tuning};

fn standard_guitar() -> Tuning {
    Tuning::find_for("Standard", Instrument::Guitar).unwrap()
}

fn find(name: &str) -> &'static Exercise {
    catalog().iter().find(|e| e.name == name).unwrap()
}

#[test]
fn chromatic_ascending_only_produces_24_notes_on_six_strings() {
    let ex = find("Chromatic 1-2-3-4");
    let params = ExerciseParams {
        starting_fret: 1,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 8,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    // 6 strings × 4 frets = 24 notes
    assert_eq!(steps.len(), 24);
}

#[test]
fn chromatic_both_directions_produces_48_notes() {
    let ex = find("Chromatic 1-2-3-4");
    let steps = ex.generate(&standard_guitar(), &ExerciseParams::default());
    assert_eq!(steps.len(), 48);
}

#[test]
fn chromatic_starts_at_specified_fret() {
    let ex = find("Chromatic 1-2-3-4");
    let params = ExerciseParams {
        starting_fret: 5,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 8,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    assert_eq!(steps[0].fret, 5);
    assert_eq!(steps[0].finger, 1);
    assert_eq!(steps[3].fret, 8);
    assert_eq!(steps[3].finger, 4);
}

#[test]
fn spider_1324_uses_finger_order_1_3_2_4() {
    let ex = find("Spider 1-3-2-4");
    let params = ExerciseParams {
        starting_fret: 1,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 8,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    // First four steps are first string with fingers 1, 3, 2, 4
    let fingers: Vec<u8> = steps.iter().take(4).map(|s| s.finger).collect();
    assert_eq!(fingers, vec![1, 3, 2, 4]);
}

#[test]
fn trill_alternates_two_frets() {
    let ex = find("Trill 1-2");
    let params = ExerciseParams {
        starting_fret: 5,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 4,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    // First string: 4 reps × 2 notes = 8 steps alternating 5, 6, 5, 6, ...
    let first_string: Vec<u8> = steps.iter().take(8).map(|s| s.fret).collect();
    assert_eq!(first_string, vec![5, 6, 5, 6, 5, 6, 5, 6]);
}

#[test]
fn ladder_produces_diagonal_pattern() {
    let ex = find("Ladder");
    let params = ExerciseParams {
        starting_fret: 1,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 8,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    assert_eq!(steps.len(), 6);
    // Each step is on consecutive string + consecutive fret
    for (i, step) in steps.iter().enumerate() {
        assert_eq!(step.string_index, i);
        assert_eq!(step.fret, 1 + i as u8);
    }
}

#[test]
fn x_pattern_alternates_outer_strings() {
    let ex = find("X-Pattern");
    let steps = ex.generate(&standard_guitar(), &ExerciseParams::default());
    // First two steps should be string 0 then string 5 (outer pair)
    assert_eq!(steps[0].string_index, 0);
    assert_eq!(steps[1].string_index, 5);
}

#[test]
fn all_exercises_generate_at_least_one_step() {
    let g = standard_guitar();
    for ex in catalog() {
        let steps = ex.generate(&g, &ExerciseParams::default());
        assert!(!steps.is_empty(), "exercise {} produced no steps", ex.name);
    }
}

#[test]
fn exercises_work_with_arbitrary_string_counts() {
    // 4-string bass should work with chromatic.
    let bass = Tuning::find_for("Standard 4", Instrument::Bass).unwrap();
    let ex = find("Chromatic 1-2-3-4");
    let steps = ex.generate(&bass, &ExerciseParams::default());
    // 4 strings × 4 frets × 2 directions = 32 steps
    assert_eq!(steps.len(), 32);
}

#[test]
fn catalog_in_category_filters() {
    let chromatic_count = catalog_in_category(ExerciseCategory::Chromatic).count();
    assert_eq!(chromatic_count, 1);
    let trill_count = catalog_in_category(ExerciseCategory::Trill).count();
    assert_eq!(trill_count, 1);
}

#[test]
fn pentatonic_box_shift_produces_two_boxes() {
    let ex = find("Pentatonic Box Shift");
    let params = ExerciseParams {
        starting_fret: 5,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 8,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    // 6 strings × 2 notes per box × 2 boxes = 24 steps ascending
    assert_eq!(steps.len(), 24);
    // First box should contain frets at and around starting_fret.
    assert!(steps.iter().take(12).any(|s| s.fret == 5));
    // Second box shifted by 3 frets.
    assert!(steps.iter().skip(12).any(|s| s.fret == 8));
}

#[test]
fn two_string_climb_uses_only_two_strings() {
    let ex = find("Two-String Climb");
    let params = ExerciseParams {
        starting_fret: 1,
        direction: ExerciseDirection::Ascending,
        trill_repeats: 8,
    };
    let steps = ex.generate(&standard_guitar(), &params);
    // All steps should be on string 0 or 1.
    for s in &steps {
        assert!(s.string_index < 2, "step on string {}", s.string_index);
    }
    // 4 reps × 4 notes per rep = 16
    assert_eq!(steps.len(), 16);
}
