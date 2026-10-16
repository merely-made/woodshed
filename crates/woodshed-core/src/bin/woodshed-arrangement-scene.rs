// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! A reproducible study built through the actual Set and saved-shape APIs.
use scenomise::template::{Placement, Span};
use std::io::Write;
use woodshed_core::{
    Lens, ROOT_NAMES, StageState,
    arrangement_scene::{
        ArrangementRest, CardAlignment, ProgressionArrangement, arrangement_scene,
    },
};
use woodshedding::rehearsal::Set;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 || !matches!(args[0].as_str(), "regular" | "varied") {
        return Err("usage: woodshed-arrangement-scene regular|varied OUTPUT.json".into());
    }
    let varied = args[0] == "varied";
    let mut stage = StageState::default();
    stage.lens = Lens::Progressions;
    stage.root_idx = ROOT_NAMES.iter().position(|s| *s == "C").unwrap();
    stage.select_progression(
        stage
            .progressions()
            .iter()
            .position(|p| p.name == "ii-V-I (Jazz)")
            .unwrap(),
    );
    let mut set = Set::default();
    for i in 0..3 {
        stage.progression_expand(i);
        let mut card = stage
            .card_from_lens()
            .ok_or("Progression card unavailable")?;
        stage.select_card_shape_index(&mut card, 0)?;
        set.push(card);
    }
    if varied {
        set.duplicate(2);
        stage.select_card_shape_index(&mut set.cards[3], 1)?;
    }
    let bindings = set
        .cards
        .iter()
        .enumerate()
        .map(|(i, c)| CardAlignment {
            card: c.id,
            role: Some(i.min(2)),
            placement: Some(Placement {
                lane: 0,
                span: Span {
                    start: [0, 16, 32, 48][i],
                    length: if varied {
                        [16, 8, 16, 16][i]
                    } else {
                        [16, 16, 32, 0][i]
                    },
                },
            }),
        })
        .collect();
    let arrangement = ProgressionArrangement {
        name: if varied {
            "Varied arrangement"
        } else {
            "Regular arrangement"
        }
        .into(),
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
        cards: bindings,
        rests: if varied {
            vec![ArrangementRest {
                key: "breath".into(),
                placement: Placement {
                    lane: 0,
                    span: Span {
                        start: 24,
                        length: 8,
                    },
                },
                role: None,
            }]
        } else {
            vec![]
        },
    };
    let doc = arrangement_scene(&stage, &set, &arrangement, "ii-V-I-study")?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?
        .write_all(&serde_json::to_vec_pretty(&doc)?)?;
    println!(
        "Exported {} with {} supplied voicings",
        arrangement.name,
        doc.linked.len()
    );
    Ok(())
}
