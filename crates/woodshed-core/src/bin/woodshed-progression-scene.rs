// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
use std::io::Write;
use woodshed_core::{
    ROOT_NAMES, StageState, progression_scene::progression_scene, song::song_from_progression,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err("usage: woodshed-progression-scene ROOT PROGRESSION_NAME BPM BLOCK_LENGTHS_COMMA_SEPARATED OUTPUT.json".into());
    }
    let mut stage = StageState::default();
    stage.root_idx = ROOT_NAMES
        .iter()
        .position(|s| *s == args[0])
        .ok_or("Unknown root")?;
    let i = stage
        .progressions()
        .iter()
        .position(|p| p.name == args[1])
        .ok_or("Unknown progression")?;
    stage.select_progression(i);
    let bpm = args[2].parse()?;
    let mut song = song_from_progression(&stage, bpm).ok_or("Progression cannot be realized")?;
    let lengths = args[3]
        .split(',')
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>()?;
    if lengths.len() != song.bars.len() {
        return Err("Supply one block length per progression role".into());
    }
    for (bar, length) in song.bars.iter_mut().zip(lengths) {
        bar.length = length;
    }
    let recipe = progression_scene(&stage, &song, "exported-song")?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?
        .write_all(&serde_json::to_vec_pretty(&recipe)?)?;
    println!(
        "Exported progression roles and arranged song blocks at revision {}",
        recipe.revision
    );
    Ok(())
}
