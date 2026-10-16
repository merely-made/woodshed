// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Export a real Stage chord lens, using Woodshed's catalog and tuning model.
use std::io::Write;
use woodshed_core::{
    Lens, ROOT_NAMES, StageState, fretboard_scene::stage_fretboard_scene, tunings,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "usage: woodshed-fretboard-scene ROOT CHORD_NAME TUNING_INDEX FRETS OUTPUT.scene.json"
                .into(),
        );
    }
    let mut stage = StageState::default();
    stage.lens = Lens::Chords;
    stage.root_idx = ROOT_NAMES
        .iter()
        .position(|n| *n == args[0])
        .ok_or("Root is not in Woodshed's root catalog")?;
    stage.chord_idx = stage
        .chords()
        .iter()
        .position(|c| c.name == args[1])
        .ok_or("Chord is not in Woodshed's catalog")?;
    stage.tuning_idx = args[2].parse()?;
    if stage.tuning_idx >= tunings().len() {
        return Err("Tuning index is outside Woodshed's tuning catalog".into());
    }
    stage.fret_count = args[3].parse()?;
    let recipe = stage_fretboard_scene(&stage, "cli-stage")?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?
        .write_all(&serde_json::to_vec_pretty(&recipe)?)?;
    println!(
        "Exported current chord lens at revision {}",
        recipe.revision
    );
    Ok(())
}
