use woodshedding::rehearsal::{LoopMode, Set};

#[test]
fn card_loop_roundtrips_without_trapping_explicit_set_navigation() {
    let mut set = Set::default();
    let mut stage = woodshed_core::StageState::new();
    stage.set_lens(woodshed_core::Lens::Chords);
    let template = stage.card_from_lens().unwrap();
    set.push(template.clone());
    set.push(template);
    set.loop_mode = LoopMode::Card;
    let encoded = serde_json::to_string(&set).unwrap();
    let mut loaded: Set = serde_json::from_str(&encoded).unwrap();
    assert_eq!(loaded.loop_mode, LoopMode::Card);
    assert!(woodshed_core::step_set(&mut loaded, 1));
    assert_eq!(loaded.cursor, 1);
    assert!(!woodshed_core::step_set(&mut loaded, 1));
    assert_eq!(loaded.cursor, 1);
    assert!(woodshed_core::step_set(&mut loaded, -1));
    assert_eq!(loaded.cursor, 0);
}
