// SPDX-License-Identifier: MPL-2.0
use crate::{SequencerPattern, Sound};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternError(pub String);

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PatternError {}

impl SequencerPattern {
    /// Admission for one-bar authored patterns. Existing constructors and wire
    /// representations retain their semantics; untrusted consumers opt into this
    /// checked boundary before preparing audio or MIDI.
    pub fn validate(&self) -> Result<(), PatternError> {
        let invalid = |message: &str| PatternError(message.into());
        if !self.bpm.is_finite() || !(1.0..=1000.0).contains(&self.bpm) {
            return Err(invalid(
                "Pattern tempo must be finite and between 1 and 1000 BPM",
            ));
        }
        let meter = self.time_signature;
        if !(1..=32).contains(&meter.numerator)
            || !meter.denominator.is_power_of_two()
            || meter.denominator > 64
            || !(1..=32).contains(&self.subdivision.divisions_per_beat)
            || self.tracks.is_empty()
            || self.tracks.len() > 64
            || self.steps_per_bar() > 1024
        {
            return Err(invalid(
                "Pattern meter, subdivisions or track count exceed supported bounds",
            ));
        }
        for track in &self.tracks {
            if track.name.trim().is_empty()
                || track.name.len() > 128
                || track.steps.len() != self.steps_per_bar()
            {
                return Err(invalid(
                    "Every track needs a name and exactly one bar of steps",
                ));
            }
            match &track.sound {
                Sound::Sample {
                    id,
                    gain,
                    accent_gain,
                    pitch_semitones,
                    ..
                } => {
                    if id.is_empty()
                        || id.len() > 128
                        || !gain.is_finite()
                        || !(0.0..=4.0).contains(gain)
                        || !accent_gain.is_finite()
                        || !(0.0..=4.0).contains(accent_gain)
                        || !pitch_semitones.is_finite()
                        || !(-24.0..=24.0).contains(pitch_semitones)
                    {
                        return Err(invalid(
                            "Sample identity, gain or pitch exceeds supported bounds",
                        ));
                    }
                },
                Sound::Click {
                    frequency_hz,
                    accent_frequency_hz,
                    duration_seconds,
                    amplitude,
                } => {
                    if !frequency_hz.is_finite()
                        || !(1.0..=24_000.0).contains(frequency_hz)
                        || !accent_frequency_hz.is_finite()
                        || !(1.0..=24_000.0).contains(accent_frequency_hz)
                        || !duration_seconds.is_finite()
                        || !(0.0..=2.0).contains(duration_seconds)
                        || !amplitude.is_finite()
                        || !(0.0..=1.0).contains(amplitude)
                    {
                        return Err(invalid(
                            "Click frequency, duration or amplitude exceeds supported bounds",
                        ));
                    }
                },
            }
        }
        Ok(())
    }
}
