// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Export the current resolved scale/chord lens as a host-independent recipe.
use crate::{Lens, StageState};
use sceno::SourceRef;
use scenomise::backdrops::{
    BACKDROP_SCENE_VERSION, Fretboard, FrettedPosition, SceneFacts, SceneInput, ToneRole,
    TunedString,
};

pub fn stage_fretboard_scene(stage: &StageState, reading_id: &str) -> Result<SceneInput, String> {
    if !matches!(stage.lens, Lens::Scales | Lens::Chords) {
        return Err("Portable fretboard export requires the resolved Scale or Chord lens".into());
    }
    let tuning = stage.tuning();
    let material = match stage.lens {
        Lens::Scales => stage.scales().get(stage.scale_idx).map(|s| s.name),
        Lens::Chords => stage.chords().get(stage.chord_idx).map(|c| c.name),
        _ => None,
    }
    .ok_or("The selected material is absent from Woodshed's catalog")?;
    let title = format!("{} {} · {}", stage.root_name(), material, tuning.name);
    fretboard_from_dots(
        tuning,
        stage.fret_count,
        stage.dots(),
        stage.root().pitch_class(),
        reading_id,
        title,
    )
}

fn fretboard_from_dots(
    tuning: woodshedding::tuning::Tuning,
    frets: u8,
    dots: Vec<crate::FretDot>,
    root: u8,
    reading_id: &str,
    title: String,
) -> Result<SceneInput, String> {
    let count = tuning.strings.len();
    let strings = tuning
        .strings
        .iter()
        .rev()
        .map(|p| {
            Ok(TunedString {
                label: p.to_string(),
                open_midi: u8::try_from(p.midi())
                    .map_err(|_| "Tuning is outside the portable MIDI range")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut positions = Vec::new();
    for dot in dots {
        let open = tuning
            .strings
            .get(dot.string_index)
            .ok_or("Position names an absent string")?;
        let midi = open.midi() + i32::from(dot.fret);
        let pc = u8::try_from(midi.rem_euclid(12)).map_err(|_| "Invalid pitch class")?;
        let role = if pc == root {
            ToneRole::Root
        } else {
            match dot.degree.as_str() {
                "3" | "b3" | "#3" => ToneRole::Third,
                "5" | "b5" | "#5" => ToneRole::Fifth,
                _ => ToneRole::Other,
            }
        };
        positions.push(FrettedPosition {
            key: format!("string-{}/fret-{}", dot.string_index, dot.fret),
            source: SourceRef {
                adapter: "woodshed.pitch-class".into(),
                id: pc.to_string(),
            },
            string_index: u16::try_from(count - 1 - dot.string_index)
                .map_err(|_| "Too many strings")?,
            fret: u16::from(dot.fret),
            pitch_class: pc,
            label: dot.label,
            role,
        });
    }
    let facts = SceneFacts::Fretboard(Fretboard {
        strings,
        frets: u16::from(frets),
        positions,
    });
    let revision = blake3::hash(&serde_json::to_vec(&facts).map_err(|e| e.to_string())?)
        .to_hex()
        .to_string();
    let recipe = SceneInput {
        version: BACKDROP_SCENE_VERSION,
        source: SourceRef {
            adapter: "woodshed.fretboard".into(),
            id: reading_id.into(),
        },
        revision,
        title,
        facts,
    };
    recipe.compile().map_err(|e| e.to_string())?;
    Ok(recipe)
}

/// Export only an explicitly selected and currently admitted card shape.
/// Missing selection remains absent; a stale saved shape is a refusal.
pub fn selected_card_fretboard_scene(
    stage: &StageState,
    card: &woodshedding::rehearsal::Card,
    reading_id: &str,
) -> Result<Option<SceneInput>, String> {
    use crate::card_shapes::CardShapeStatus;
    use woodshedding::rehearsal::Material;
    let shape = match stage.card_shape_status(card) {
        CardShapeStatus::Unselected => return Ok(None),
        CardShapeStatus::Available(shape) => shape,
        CardShapeStatus::Unavailable(reason) => {
            return Err(format!("Selected card voicing refused: {reason}"));
        },
        CardShapeStatus::NotChord => {
            return Err("Only chord cards supply a selected fretboard voicing".into());
        },
    };
    let Material::Chord { root, .. } = &card.material else {
        unreachable!()
    };
    let tuning = stage.resolve_card_tuning(card).map_err(|e| e.to_string())?;
    let dots = stage
        .selected_card_shape_dots(card)
        .ok_or("Selected shape is no longer available")?;
    // The reading frames the supplied contacts, not the search inventory's
    // empty remaining neck. Coordinates remain physical nut-relative frets.
    let frets = dots.iter().map(|dot| dot.fret).max().unwrap_or(1).max(1);
    let title = format!(
        "{} · supplied voicing {} · capo {} · {}",
        card.label,
        shape.index + 1,
        shape.geometry.capo,
        shape.fingerprint
    );
    let mut recipe = fretboard_from_dots(
        tuning,
        frets,
        dots,
        (root.value() + shape.geometry.capo) % 12,
        reading_id,
        title,
    )?;
    recipe.source.adapter = "woodshed.card-voicing".into();
    recipe.revision = blake3::hash(
        &serde_json::to_vec(&(
            &recipe.facts,
            &card.setting,
            &shape.fingerprint,
            shape.profile,
        ))
        .map_err(|e| e.to_string())?,
    )
    .to_hex()
    .to_string();
    Ok(Some(recipe))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_tuning_and_repeated_pitch_occurrences_survive_an_independent_host() {
        let stage = StageState::default();
        let recipe = stage_fretboard_scene(&stage, "active-stage").unwrap();
        let SceneFacts::Fretboard(board) = &recipe.facts else {
            unreachable!()
        };
        let tuning = stage.tuning();
        assert_eq!(
            i32::from(board.strings[0].open_midi),
            tuning.strings.last().unwrap().midi()
        );
        for dot in stage.dots() {
            let position = board
                .positions
                .iter()
                .find(|p| p.key == format!("string-{}/fret-{}", dot.string_index, dot.fret))
                .unwrap();
            assert_eq!(
                position.pitch_class,
                ((tuning.strings[dot.string_index].midi() + i32::from(dot.fret)) % 12) as u8
            );
        }
        let reading = recipe.compile().unwrap();
        assert!(
            reading
                .scene
                .relations
                .iter()
                .any(|r| r.kind.as_deref() == Some("same-resource"))
        );
        assert_eq!(
            recipe,
            SceneInput::parse(&serde_json::to_string(&recipe).unwrap()).unwrap()
        );
    }
    #[test]
    fn a_changed_neck_changes_the_reading_but_export_does_not_edit_stage() {
        let mut stage = StageState::default();
        let before = stage.dots();
        let first = stage_fretboard_scene(&stage, "stage").unwrap();
        assert_eq!(stage.dots().len(), before.len());
        stage.fret_count = 5;
        assert_ne!(
            first.revision,
            stage_fretboard_scene(&stage, "stage").unwrap().revision
        );
        stage.lens = Lens::Exercises;
        assert!(stage_fretboard_scene(&stage, "stage").is_err());
    }
    #[test]
    fn a_diminished_fifth_keeps_its_musical_role() {
        let mut stage = StageState::default();
        stage.lens = Lens::Chords;
        stage.chord_idx = stage
            .chords()
            .iter()
            .position(|c| c.name == "Diminished")
            .unwrap();
        let root = stage.root().pitch_class();
        let input = stage_fretboard_scene(&stage, "diminished").unwrap();
        let SceneFacts::Fretboard(board) = input.facts else {
            unreachable!()
        };
        assert!(
            board
                .positions
                .iter()
                .any(|p| p.pitch_class == (root + 6) % 12 && p.role == ToneRole::Fifth)
        );
    }
}
