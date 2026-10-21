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

#[test]
fn source_reconstruction_reuses_the_pair_checked_selection() {
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
        let ids: Vec<_> = set.cards.iter().map(|c| c.id).collect();
        let pair = (ids[count - 1], ids[0]); // Explicit, reversed and possibly nonadjacent.
        COUNTS.set([0; 3]);
        disclose_comparison(WorkingSetId(1), "source", &set, &ids, pair).unwrap();
        let expected = passage_projection_json(WorkingSetId(1), "source", &set, &ids).unwrap();
        assert_eq!(COUNTS.get(), [2, 1, count - 1]);
        COUNTS.set([0; 3]);
        let actual =
            comparison_passage_projection_json(WorkingSetId(1), "source", &set, &ids, pair)
                .unwrap();
        assert_eq!(COUNTS.get(), [1, 1, count - 1]);
        assert_eq!(actual, expected);
    }
}

#[test]
fn source_passage_preserves_the_comparison_before_passage_error_order() {
    use super::*;
    use woodshedding::pitch::PitchClass;
    use woodshedding::rehearsal::LoopMode;
    let mut set = Set::from_cards(
        (0..65).map(|_| {
            KeyedCatalogRef {
                formula_id: "chord:Major".into(),
                root: PitchClass::new(0),
            }
            .to_card()
            .unwrap()
        }),
        LoopMode::Off,
    );
    let ids: Vec<_> = set.cards.iter().map(|c| c.id).collect();
    let cases = [
        (WorkingSetId(0), "v1", ids.clone(), (ids[0], ids[1])),
        (WorkingSetId(1), "", ids.clone(), (ids[0], ids[1])),
        (WorkingSetId(1), "v1", ids.clone(), (ids[0], CardId(999))),
        (
            WorkingSetId(1),
            "v1",
            vec![ids[0], CardId(999)],
            (ids[0], ids[1]),
        ),
        (
            WorkingSetId(1),
            "v1",
            vec![ids[0], ids[0]],
            (ids[0], ids[0]),
        ),
        (WorkingSetId(1), "v1", ids.clone(), (ids[0], ids[1])),
    ];
    for (owner, revision, selected, pair) in cases {
        let expected = disclose_comparison(owner, revision, &set, &selected, pair)
            .and_then(|_| passage_projection_json(owner, revision, &set, &selected));
        assert_eq!(
            comparison_passage_projection_json(owner, revision, &set, &selected, pair),
            expected
        );
    }
    set.cards[64].material = woodshedding::rehearsal::Material::Chord {
        name: "uninstalled".into(),
        root: PitchClass::new(0),
    };
    assert_eq!(
        comparison_passage_projection_json(WorkingSetId(1), "v1", &set, &ids, (ids[0], ids[1])),
        Err(DisclosureError::UnsupportedMaterial(ids[64]))
    );
}
