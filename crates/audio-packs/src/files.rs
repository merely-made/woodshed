// SPDX-License-Identifier: MPL-2.0
//! Host-side file preparation shared by CLI and existing application workers.
use crate::{LoadedPack, PackError, PackSelection, PackSnapshot, bundled_drum_kit};
use std::{
    io::{Read, Write},
    path::Path,
};

pub fn load(path: &Path, pattern: Option<&str>) -> Result<PackSelection, PackError> {
    if path == Path::new("bundled") {
        return choose(bundled_drum_kit()?, pattern);
    }
    if path.is_dir() {
        return choose(LoadedPack::from_directory(path)?, pattern);
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(80 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 80 * 1024 * 1024 {
        return Err(PackError("Rhythm file exceeds resource bound".into()));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| PackError(e.to_string()))?;
    if value.get("snapshot").is_some() {
        let selection: PackSelection =
            serde_json::from_value(value).map_err(|e| PackError(e.to_string()))?;
        selection.validate()?;
        return match pattern {
            Some(id) if id != selection.pattern_id() => selection.select_pattern(id),
            _ => Ok(selection),
        };
    }
    choose(
        LoadedPack::from_snapshot(PackSnapshot::from_json(&bytes)?)?,
        pattern,
    )
}
fn choose(pack: LoadedPack, pattern: Option<&str>) -> Result<PackSelection, PackError> {
    let id = pattern
        .map(str::to_owned)
        .or_else(|| pack.definition().patterns.keys().next().cloned())
        .ok_or_else(|| PackError("Kit has no patterns".into()))?;
    PackSelection::new(pack, &id)
}
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), PackError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}
pub fn export_midi(selection: &PackSelection, path: &Path) -> Result<(), PackError> {
    write_new(path, &selection.midi()?.bytes)
}

/// Reused by the portable audio-pack command and Ringdown's offline branch.
pub fn run_cli(args: &[std::ffi::OsString]) -> Result<(), PackError> {
    if !(3..=4).contains(&args.len()) {
        return Err(PackError("Usage: <bundled|pack-directory|snapshot-or-rhythm.json> <pattern> <new-output.mid> [new-rhythm.json]".into()));
    }
    let id = args[1]
        .to_str()
        .ok_or_else(|| PackError("Pattern identity must be UTF-8".into()))?;
    let selection = load(Path::new(&args[0]), Some(id))?;
    let midi = selection.midi()?;
    let saved = args
        .get(3)
        .map(|_| serde_json::to_vec(&selection).map_err(|e| PackError(e.to_string())))
        .transpose()?;
    write_new(Path::new(&args[2]), &midi.bytes)?;
    if let (Some(path), Some(bytes)) = (args.get(3), saved) {
        write_new(Path::new(path), &bytes)
            .map_err(|e| PackError(format!("MIDI exported; rhythm was not saved: {e}")))?;
    }
    println!(
        "{} {} [{}]: {}, {} notes, PPQ {}, {} ticks",
        selection.identity().id,
        selection.identity().version,
        selection.identity().digest,
        selection.pattern_id(),
        midi.notes,
        midi.ticks_per_quarter,
        midi.length_ticks
    );
    Ok(())
}
