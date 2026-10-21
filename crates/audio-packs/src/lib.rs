// SPDX-License-Identifier: MPL-2.0
//! One admission path for bundled, directory and retained data-defined kits.
//! All parsing/decoding happens on the caller's control side. The result is
//! immutable source custody plus prepared samples, with no audio device or UI.
#![forbid(unsafe_code)]

mod definition;
mod selection;
pub mod files;
pub mod samples;
mod snapshot;

pub use audio_pattern::{sequencer, sound};
pub use selection::{PackEdit, PackSelection};
pub use definition::{CAPABILITIES, PackDefinition, ParameterMetadata, SampleVoice};
pub use samples::{SampleBank, SampleError};
pub use snapshot::{ManifestFormat, PackIdentity, PackSnapshot};

use audio_pattern::{SequencerPattern, midi::MidiExport};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

pub(crate) const MAX_SOURCE: usize = 1_048_576;
pub(crate) const MAX_ASSET: usize = 4_194_304;
pub(crate) const MAX_TOTAL: usize = 16_777_216;
const MAX_DECODED_FRAMES: usize = 4_194_304;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackError(pub String);
impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PackError {}
impl From<std::io::Error> for PackError {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}
impl From<audio_pattern::PatternError> for PackError {
    fn from(e: audio_pattern::PatternError) -> Self {
        Self(e.0)
    }
}

/// A successfully admitted immutable definition and its resolved sample bank.
/// Prepare a replacement first, then swap the owning product's current value.
#[derive(Debug)]
pub struct LoadedPack {
    snapshot: PackSnapshot,
    definition: PackDefinition,
    bank: SampleBank,
}

impl LoadedPack {
    pub fn definition(&self) -> &PackDefinition {
        &self.definition
    }
    pub fn identity(&self) -> &PackIdentity {
        self.snapshot.identity()
    }
    pub fn snapshot(&self) -> &PackSnapshot {
        &self.snapshot
    }

    /// Produce a hydrated copy of the one authoritative authored pattern.
    pub fn pattern(&self, id: &str) -> Result<SequencerPattern, PackError> {
        let mut pattern = self.authored_pattern(id)?.clone();
        let resolved = self.bank.rehydrate_pattern(&mut pattern);
        if resolved != pattern.tracks.len() {
            return Err(PackError("Pack sample resolution is incomplete".into()));
        }
        Ok(pattern)
    }

    pub fn midi(&self, id: &str) -> Result<MidiExport, PackError> {
        let pattern = self.authored_pattern(id)?;
        Ok(audio_pattern::midi::export_midi(
            pattern,
            &self.definition.mappings(pattern),
        )?)
    }

    fn authored_pattern(&self, id: &str) -> Result<&SequencerPattern, PackError> {
        self.definition
            .patterns
            .get(id)
            .ok_or_else(|| PackError(format!("Unknown pattern: {id}")))
    }

    /// Revalidate retained bytes and identity, then decode. Installation paths
    /// and later edits/uninstallations cannot change this captured definition.
    pub fn from_snapshot(snapshot: PackSnapshot) -> Result<Self, PackError> {
        snapshot.validate_bounds()?;
        let definition = snapshot.parse_definition()?;
        definition.validate()?;
        if snapshot.identity != snapshot.computed_identity(&definition) {
            return Err(PackError(
                "Pack snapshot content does not match its retained identity".into(),
            ));
        }
        let expected: BTreeSet<_> = definition
            .voices
            .values()
            .map(|v| v.sample.as_str())
            .collect();
        if expected != snapshot.assets.keys().map(String::as_str).collect() {
            return Err(PackError(
                "Pack snapshot has missing or undeclared assets".into(),
            ));
        }
        let mut decoded = BTreeMap::new();
        let mut frames = 0usize;
        for (path, bytes) in &snapshot.assets {
            let sample = samples::decode_pack_wav(bytes, MAX_DECODED_FRAMES - frames)
                .map_err(|e| PackError(format!("Sample {path}: {e}")))?;
            frames += sample.len();
            decoded.insert(path, sample);
        }
        let mut bank = SampleBank::new();
        for (id, voice) in &definition.voices {
            bank.insert(id, decoded[&voice.sample].clone());
        }
        Ok(Self {
            snapshot,
            definition,
            bank,
        })
    }

    /// A manually installed directory is portable across desktop hosts. Exactly
    /// one manifest format is allowed, and every captured asset stays inside it.
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, PackError> {
        let root = path.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(PackError("Pack location must be a directory".into()));
        }
        let toml = root.join("audio-pack.toml").exists();
        let json = root.join("audio-pack.json").exists();
        let (name, format) = match (toml, json) {
            (true, false) => ("audio-pack.toml", ManifestFormat::Toml),
            (false, true) => ("audio-pack.json", ManifestFormat::Json),
            _ => {
                return Err(PackError(
                    "A pack needs exactly one audio-pack.toml or audio-pack.json".into(),
                ));
            },
        };
        let source = read_asset(&root, name, MAX_SOURCE)?;
        let mut snapshot = PackSnapshot::new(format, source, BTreeMap::new());
        let definition = snapshot.parse_definition()?;
        definition.validate()?;
        let mut total = 0usize;
        for voice in definition.voices.values() {
            if snapshot.assets.contains_key(&voice.sample) {
                continue;
            }
            let bytes = read_asset(&root, &voice.sample, MAX_ASSET.min(MAX_TOTAL - total))?;
            total += bytes.len();
            snapshot.assets.insert(voice.sample.clone(), bytes);
        }
        snapshot.identity = snapshot.computed_identity(&definition);
        Self::from_snapshot(snapshot)
    }
}

/// Embedded resources enter the same snapshot admission/decoding path as disk.
pub fn bundled_drum_kit() -> Result<LoadedPack, PackError> {
    let source = include_bytes!("../../../assets/audio-packs/basic-drums/audio-pack.json").to_vec();
    let assets = BTreeMap::from([
        (
            "samples/kick.wav".into(),
            include_bytes!("../../../assets/audio-packs/basic-drums/samples/kick.wav").to_vec(),
        ),
        (
            "samples/snare.wav".into(),
            include_bytes!("../../../assets/audio-packs/basic-drums/samples/snare.wav").to_vec(),
        ),
        (
            "samples/hat.wav".into(),
            include_bytes!("../../../assets/audio-packs/basic-drums/samples/hat.wav").to_vec(),
        ),
    ]);
    let mut snapshot = PackSnapshot::new(ManifestFormat::Json, source, assets);
    let definition = snapshot.parse_definition()?;
    definition.validate()?;
    snapshot.identity = snapshot.computed_identity(&definition);
    LoadedPack::from_snapshot(snapshot)
}

fn read_asset(root: &Path, relative: &str, limit: usize) -> Result<Vec<u8>, PackError> {
    if !definition::pack_path(relative) {
        return Err(PackError(format!("Invalid pack-relative path: {relative}")));
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| PackError(format!("Missing pack file {relative}: {e}")))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(PackError(format!(
            "Pack file escapes its directory or is not a file: {relative}"
        )));
    }
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(PackError(format!(
            "Pack file exceeds resource bound: {relative}"
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(PackError(format!(
            "Pack file exceeds resource bound: {relative}"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
