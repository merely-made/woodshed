//! Emit a portable Set attachment for the Graphshell editable-bar example.
//! The cards and saved arrangement are authored through Woodshed's real APIs.
use woodshed_core::{
    Lens, ROOT_NAMES, StageState, saved_arrangements::save_progression_arrangement,
};
use woodshedding::rehearsal::{Set, arrangement::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stage = StageState::default();
    stage.lens = Lens::Progressions;
    stage.root_idx = ROOT_NAMES
        .iter()
        .position(|name| *name == "C")
        .ok_or("C root unavailable")?;
    let progression = stage
        .progressions()
        .iter()
        .position(|p| p.name == "ii-V-I (Jazz)")
        .ok_or("ii-V-I (Jazz) progression unavailable")?;
    stage.select_progression(progression);

    let mut set = Set::default();
    for role in 0..3 {
        stage.progression_expand(role);
        let mut card = stage
            .card_from_lens()
            .ok_or("Progression card unavailable")?;
        stage.select_card_shape_index(&mut card, 0)?;
        set.push(card);
    }
    // A second C card preserves a separate identity and a varied authored
    // voicing, matching the existing saved-arrangement integration fixture.
    set.duplicate(2);
    stage.select_card_shape_index(&mut set.cards[3], 1)?;

    let arrangement = ProgressionArrangement {
        name: "Saved practice passage".into(),
        extent: 64,
        role_spans: vec![
            Span {
                start: 0,
                length: 16,
            },
            Span {
                start: 16,
                length: 16,
            },
            Span {
                start: 32,
                length: 32,
            },
        ],
        cards: set
            .cards
            .iter()
            .enumerate()
            .map(|(i, card)| CardAlignment {
                card: card.id,
                role: Some(i.min(2)),
                placement: Some(Placement {
                    lane: 0,
                    span: Span {
                        start: [0, 16, 32, 48][i],
                        length: [16, 8, 16, 16][i],
                    },
                }),
            })
            .collect(),
        rests: vec![ArrangementRest {
            key: "breath".into(),
            role: None,
            placement: Placement {
                lane: 0,
                span: Span {
                    start: 24,
                    length: 8,
                },
            },
        }],
        time_signatures: vec![TimeSignatureChange {
            start: 0,
            numerator: 4,
            denominator: 4,
        }],
        bar_groups: vec![],
    };
    save_progression_arrangement(&stage, &mut set, arrangement)?;

    let output = serde_json::json!({ "version": 1, "set": set });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
