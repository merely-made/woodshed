//! Test-only operation accounting; no counters in normal builds.
use std::cell::Cell;
#[derive(Clone, Copy)]
pub(super) enum Event {
    Decode,
    Compile,
    Owner,
    Revision,
    Reconstruct,
    Install,
    Navigate,
}
thread_local! { static COUNTS: Cell<[usize; 7]> = const { Cell::new([0; 7]) }; }
pub(super) fn record(event: Event) {
    COUNTS.with(|c| {
        let mut n = c.get();
        n[event as usize] += 1;
        c.set(n);
    });
}

#[test]
fn successful_returns_reuse_the_reading_and_current_source() {
    use super::*;
    use woodshed_core::harmony::KeyedCatalogRef;
    use woodshedding::pitch::PitchClass;
    for member in [false, true] {
        let mut ui = UiState::new();
        for root in [0, 0, 9] {
            ui.set.push(
                KeyedCatalogRef {
                    formula_id: "chord:Major".into(),
                    root: PitchClass::new(root),
                }
                .to_card()
                .unwrap(),
            );
        }
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(false).unwrap();
        let reading = ui.relationship_reading().unwrap().unwrap();
        let key = relationship_link_keys(&reading).into_iter().next().unwrap();
        let pair = relationship_pair_reading(&reading, &key).unwrap();
        let bucket = &pair.links[0].buckets[0];
        let target = cambium::LinkMemberTarget {
            source: pair.source.clone(),
            link: key,
            from: bucket.source_from.id.clone(),
            to: bucket.source_to.id.clone(),
            assertion_id: bucket.assertions[0].id.clone(),
        };
        if !member {
            ui.select_relationship(Some(target.from.clone()), None)
                .unwrap();
        }
        COUNTS.set([0; 7]);
        if member {
            open_relationship_member(&mut ui, &target, "open-source-card").unwrap();
        } else {
            ui.open_relationship_source().unwrap();
        }
        assert_eq!(
            COUNTS.get(),
            if member {
                [1, 2, 1, 1, 1, 1, 1]
            } else {
                [1, 1, 1, 1, 1, 0, 1]
            }
        );
    }
}
