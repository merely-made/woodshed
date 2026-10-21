// SPDX-License-Identifier: MPL-2.0
use crate::PackError;
use audio_pattern::{SequencerPattern, Sound, midi::MidiVoice};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CAPABILITIES: [&str; 3] = ["sample.mono.v1", "step.one_bar.v1", "midi.smf0.v1"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackDefinition {
    pub format: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub license: String,
    pub requires: Vec<String>,
    pub parameters: BTreeMap<String, ParameterMetadata>,
    pub voices: BTreeMap<String, SampleVoice>,
    /// The existing pattern schema is authoritative, including its sound fields.
    pub patterns: BTreeMap<String, SequencerPattern>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleVoice {
    pub sample: String,
    pub attribution: String,
    pub midi: MidiVoice,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterMetadata {
    pub label: String,
    pub unit: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

impl PackDefinition {
    pub(crate) fn validate(&self) -> Result<(), PackError> {
        if self.format != 1
            || !valid_id(&self.id)
            || self.version.trim().is_empty()
            || self.version.len() > 64
            || self.name.trim().is_empty()
            || self.name.len() > 128
            || self.license.trim().is_empty()
            || self.license.len() > 128
        {
            return Err(PackError(
                "Unsupported pack format or invalid identity/license metadata".into(),
            ));
        }
        let required: BTreeSet<_> = self.requires.iter().map(String::as_str).collect();
        if required.len() != self.requires.len() || required != BTreeSet::from(CAPABILITIES) {
            return Err(PackError(format!(
                "Pack requires unsupported or incomplete capabilities; supported: {}",
                CAPABILITIES.join(", ")
            )));
        }
        if self.voices.is_empty()
            || self.voices.len() > 64
            || self.patterns.is_empty()
            || self.patterns.len() > 64
            || self.parameters.len() != 3
        {
            return Err(PackError(
                "Pack voice, pattern or parameter count exceeds supported bounds".into(),
            ));
        }
        for (key, unit, lower, upper) in [
            ("gain", "linear", 0.0, 4.0),
            ("accent_gain", "linear", 0.0, 4.0),
            ("pitch_semitones", "semitones", -24.0, 24.0),
        ] {
            let p = self
                .parameters
                .get(key)
                .ok_or_else(|| PackError(format!("Missing parameter metadata: {key}")))?;
            if p.label.trim().is_empty()
                || p.label.len() > 80
                || p.unit != unit
                || !p.min.is_finite()
                || !p.max.is_finite()
                || !p.default.is_finite()
                || p.min < lower
                || p.max > upper
                || p.min > p.default
                || p.default > p.max
            {
                return Err(PackError(format!("Invalid parameter metadata: {key}")));
            }
        }
        let mut notes = BTreeSet::new();
        for (id, voice) in &self.voices {
            if !valid_id(id)
                || !pack_path(&voice.sample)
                || !voice.sample.ends_with(".wav")
                || voice.attribution.trim().is_empty()
                || voice.attribution.len() > 512
            {
                return Err(PackError(format!("Invalid sample declaration: {id}")));
            }
            voice.midi.validate()?;
            if !notes.insert((voice.midi.channel, voice.midi.note)) {
                return Err(PackError(
                    "Kit voices need distinct MIDI channel/note pairs".into(),
                ));
            }
        }
        for (name, pattern) in &self.patterns {
            if !valid_id(name) {
                return Err(PackError("Invalid pattern identity".into()));
            }
            pattern.validate()?;
            let mut names = BTreeSet::new();
            for track in &pattern.tracks {
                if !names.insert(&track.name) {
                    return Err(PackError(
                        "Track names must be unique within a pattern".into(),
                    ));
                }
                let Sound::Sample {
                    id,
                    gain,
                    accent_gain,
                    pitch_semitones,
                    ..
                } = &track.sound
                else {
                    return Err(PackError(
                        "This pack version supports sample voices only".into(),
                    ));
                };
                if !self.voices.contains_key(id) {
                    return Err(PackError(format!("Unknown sample voice: {id}")));
                }
                for (key, value) in [
                    ("gain", gain),
                    ("accent_gain", accent_gain),
                    ("pitch_semitones", pitch_semitones),
                ] {
                    let p = &self.parameters[key];
                    if *value < p.min || *value > p.max {
                        return Err(PackError(format!(
                            "Track parameter lies outside declared range: {key}"
                        )));
                    }
                }
            }
            // Admission includes MIDI-map validity, even if the first use is audio.
            audio_pattern::midi::export_midi(pattern, &self.mappings(pattern))?;
        }
        Ok(())
    }

    pub(crate) fn mappings(&self, pattern: &SequencerPattern) -> Vec<Option<MidiVoice>> {
        pattern
            .tracks
            .iter()
            .map(|track| {
                track
                    .sound
                    .sample_id()
                    .and_then(|id| self.voices.get(id))
                    .map(|voice| voice.midi)
            })
            .collect()
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id != "."
        && id != ".."
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

/// Portable relative file spelling. Interpret the same references on all hosts.
pub(crate) fn pack_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|part| {
            if part.is_empty() || matches!(part, "." | "..") || part.ends_with(['.', ' ']) {
                return false;
            }
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
                && !part
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        })
}
