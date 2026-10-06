//! Export owner-derived facts for the shared explained-relationship recipe.
//! No reading, export, or compilation authors the source Set.

use woodshed_core::comparison_disclosure::{disclose, projection_json};
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::working_sets::WorkingSetId;
use woodshedding::pitch::PitchClass;
use woodshedding::rehearsal::{LoopMode, Set};

pub fn fixture_set() -> Set {
    Set::from_cards(
        [
            ("C Major", "Major", 0),
            ("C Major again", "Major", 0),
            ("A Minor", "Minor", 9),
        ]
        .map(|(label, formula, root)| {
            let mut card = KeyedCatalogRef {
                formula_id: format!("chord:{formula}"),
                root: PitchClass::new(root),
            }
            .to_card()
            .expect("fixture uses installed formulas");
            card.label = label.into();
            card
        }),
        LoopMode::Off,
    )
}

pub fn fixture_json() -> serde_json::Value {
    let set = fixture_set();
    let selected = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
    let facts = disclose(
        WorkingSetId(1),
        "woodshed-relationship-fixture-v1",
        &set,
        &selected,
        (selected[0], selected[2]),
    )
    .expect("fixture has an exact shared-tone relationship");
    projection_json(&facts)
}

fn main() {
    println!("{}", serde_json::to_string_pretty(&fixture_json()).unwrap());
}
