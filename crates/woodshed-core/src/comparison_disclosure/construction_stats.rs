//! Test-only, per-thread construction accounting; absent from product builds.
use std::cell::Cell;
thread_local! {
    static COUNTS: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
}
fn bump(index: usize) {
    COUNTS.with(|c| {
        let mut value = c.get();
        value[index] += 1;
        c.set(value);
    });
}
pub(super) fn selection() {
    bump(0);
}
pub(super) fn dataset() {
    bump(1);
}
pub(super) fn pair() {
    bump(2);
}

#[test]
fn passage_resolves_selection_and_constructs_occurrences_once() {
    use super::*;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::LoopMode;
    for count in [2, 3, 64] {
        let set = Set::from_cards(
            (0..count).map(|_| {
                KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(0),
                }
                .to_card()
                .unwrap()
            }),
            LoopMode::Off,
        );
        let selected: Vec<_> = set.cards.iter().map(|c| c.id).collect();
        COUNTS.set([0; 3]);
        let result =
            passage_projection_json(WorkingSetId(1), "construction", &set, &selected).unwrap();
        assert_eq!(
            result["dataset"]["occurrences"].as_array().unwrap().len(),
            count
        );
        assert_eq!(COUNTS.get(), [1, 1, count - 1], "width {count}");
    }
}
