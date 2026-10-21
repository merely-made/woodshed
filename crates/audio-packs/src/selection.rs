// SPDX-License-Identifier: MPL-2.0
//! Retained selection inside an application's existing document. The pattern
//! is the existing sequencer data; the snapshot supplies immutable assets.
use crate::{LoadedPack, PackError, PackSnapshot};
use audio_pattern::{SequencerPattern, Sound, Step, midi::MidiExport};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSelection {
    format: u32,
    snapshot: Arc<PackSnapshot>,
    pattern_id: String,
    pattern: SequencerPattern,
    /// Inactive drafts are retained when switching patterns; each pattern has
    /// one authored value, either here or in the selected slot above.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    inactive_patterns: BTreeMap<String, SequencerPattern>,
    #[serde(skip)]
    loaded: OnceLock<Arc<LoadedPack>>,
}

impl PartialEq for PackSelection {
    fn eq(&self, other: &Self) -> bool {
        self.format == other.format
            && self.pattern_id == other.pattern_id
            && self.pattern == other.pattern
            && self.inactive_patterns == other.inactive_patterns
            && (Arc::ptr_eq(&self.snapshot, &other.snapshot) || self.snapshot == other.snapshot)
    }
}

#[derive(Clone, Debug)]
pub enum PackEdit {
    CycleStep {
        track: usize,
        step: usize,
    },
    MuteTrack(usize),
    Gain {
        track: usize,
        delta: f32,
    },
    /// Musical step positions, never screen coordinates. Shared scene bindings
    /// submit the same edit after source/revision and quantization checks.
    MoveHit {
        track: usize,
        from: usize,
        to: usize,
    },
}

impl PackSelection {
    pub fn new(pack: LoadedPack, pattern_id: &str) -> Result<Self, PackError> {
        let pattern = pack.authored_pattern(pattern_id)?.clone();
        let snapshot = Arc::new(pack.snapshot.clone());
        let value = Self {
            format: 1,
            snapshot,
            pattern_id: pattern_id.into(),
            pattern,
            inactive_patterns: BTreeMap::new(),
            loaded: OnceLock::from(Arc::new(pack)),
        };
        value.validate()?;
        Ok(value)
    }
    pub fn pattern(&self) -> &SequencerPattern {
        &self.pattern
    }
    pub fn pattern_id(&self) -> &str {
        &self.pattern_id
    }
    pub fn identity(&self) -> &crate::PackIdentity {
        self.snapshot.identity()
    }
    pub fn name(&self) -> Result<&str, PackError> {
        Ok(&self.loaded()?.definition.name)
    }
    fn loaded(&self) -> Result<&Arc<LoadedPack>, PackError> {
        if self.loaded.get().is_none() {
            let pack = LoadedPack::from_snapshot((*self.snapshot).clone())?;
            let _ = self.loaded.set(Arc::new(pack));
        }
        Ok(self.loaded.get().expect("prepared above"))
    }
    /// Revalidate even deserialized selections; cached PCM is never authority.
    pub fn validate(&self) -> Result<(), PackError> {
        if self.format != 1 {
            return Err(PackError("Unsupported retained rhythm format".into()));
        }
        let pack = self.loaded()?;
        pack.authored_pattern(&self.pattern_id)?;
        let mut definition = pack.definition.clone();
        if self.inactive_patterns.contains_key(&self.pattern_id)
            || self.inactive_patterns.len() >= definition.patterns.len()
        {
            return Err(PackError("Invalid inactive pattern drafts".into()));
        }
        for (id, pattern) in &self.inactive_patterns {
            if !definition.patterns.contains_key(id) {
                return Err(PackError("Unknown inactive pattern draft".into()));
            }
            definition.patterns.insert(id.clone(), pattern.clone());
        }
        definition
            .patterns
            .insert(self.pattern_id.clone(), self.pattern.clone());
        definition.validate()
    }
    pub fn revision(&self) -> String {
        let mut hash = blake3::Hasher::new();
        hash.update(self.identity().digest.as_bytes());
        hash.update(self.pattern_id.as_bytes());
        hash.update(&serde_json::to_vec(&self.pattern).expect("admitted finite pattern"));
        hash.finalize().to_hex().to_string()
    }
    pub fn select_pattern(&self, id: &str) -> Result<Self, PackError> {
        if id == self.pattern_id {
            return Ok(self.clone());
        }
        let original = self.loaded()?.authored_pattern(id)?.clone();
        let mut next = self.clone();
        next.inactive_patterns
            .insert(self.pattern_id.clone(), self.pattern.clone());
        next.pattern = next.inactive_patterns.remove(id).unwrap_or(original);
        next.pattern_id = id.into();
        next.validate()?;
        Ok(next)
    }
    pub fn pattern_ids(&self) -> Result<Vec<String>, PackError> {
        Ok(self.loaded()?.definition.patterns.keys().cloned().collect())
    }
    pub fn at_tempo(&self, bpm: f32) -> Result<Self, PackError> {
        let mut next = self.clone();
        next.pattern.bpm = bpm;
        next.validate()?;
        Ok(next)
    }
    /// A stale view/scene cannot redirect a positional edit after replacement.
    pub fn edit(&self, revision: &str, edit: PackEdit) -> Result<Self, PackError> {
        if revision != self.revision() {
            return Err(PackError("Rhythm changed; retry the edit.".into()));
        }
        let mut next = self.clone();
        match edit {
            PackEdit::CycleStep { track, step } => {
                let value = next
                    .pattern
                    .tracks
                    .get_mut(track)
                    .and_then(|t| t.steps.get_mut(step))
                    .ok_or_else(|| PackError("Step is unavailable".into()))?;
                *value = match *value {
                    Step::Empty => Step::Active { accent: false },
                    Step::Active { accent: false } => Step::Active { accent: true },
                    _ => Step::Empty,
                };
            },
            PackEdit::MuteTrack(track) => {
                let t = next
                    .pattern
                    .tracks
                    .get_mut(track)
                    .ok_or_else(|| PackError("Track is unavailable".into()))?;
                t.muted = !t.muted;
            },
            PackEdit::Gain { track, delta } => {
                if !delta.is_finite() {
                    return Err(PackError("Level change must be finite".into()));
                }
                let t = next
                    .pattern
                    .tracks
                    .get_mut(track)
                    .ok_or_else(|| PackError("Track is unavailable".into()))?;
                if let Sound::Sample { gain, .. } = &mut t.sound {
                    *gain += delta;
                }
            },
            PackEdit::MoveHit { track, from, to } => {
                let t = next
                    .pattern
                    .tracks
                    .get_mut(track)
                    .ok_or_else(|| PackError("Track is unavailable".into()))?;
                if from >= t.steps.len()
                    || to >= t.steps.len()
                    || t.steps[from] == Step::Empty
                    || (from != to && t.steps[to] != Step::Empty)
                {
                    return Err(PackError(
                        "Move needs an existing hit and an empty destination".into(),
                    ));
                }
                t.steps.swap(from, to);
            },
        }
        next.validate()?;
        Ok(next)
    }
    pub fn hydrated_pattern(&self) -> Result<SequencerPattern, PackError> {
        self.validate()?;
        let mut pattern = self.pattern.clone();
        if self.loaded()?.bank.rehydrate_pattern(&mut pattern) != pattern.tracks.len() {
            return Err(PackError("Pack sample resolution is incomplete".into()));
        }
        Ok(pattern)
    }
    pub fn midi(&self) -> Result<MidiExport, PackError> {
        self.validate()?;
        Ok(audio_pattern::midi::export_midi(
            &self.pattern,
            &self.loaded()?.definition.mappings(&self.pattern),
        )?)
    }
    pub fn render_bar(&self, rate: u32) -> Result<Vec<f32>, PackError> {
        self.render_bars(rate, 1)
    }
    pub fn render_bars(&self, rate: u32, bars: u8) -> Result<Vec<f32>, PackError> {
        if !(8_000..=192_000).contains(&rate) || bars == 0 {
            return Err(PackError("Unsupported render rate or bar count".into()));
        }
        let p = self.hydrated_pattern()?;
        let frames = audio_pattern::render::frames_for_bar(&p, rate)?
            .checked_mul(usize::from(bars))
            .filter(|n| *n <= 4_194_304)
            .ok_or_else(|| PackError("Drum layer exceeds render resource bound".into()))?;
        Ok(audio_pattern::render::render_frames(
            p,
            rate as f32,
            frames,
            1,
        ))
    }
}
