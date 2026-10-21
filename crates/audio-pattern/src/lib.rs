// SPDX-License-Identifier: MPL-2.0
//! Existing Woodshed patterns and sound kernels, independent of devices and UI.
//! File/asset loading belongs to `audio-packs`; native rendering remains in the
//! consuming runtime. MIDI export returns bytes without opening an output port.
#![forbid(unsafe_code)]

pub mod midi;
pub mod render;
pub mod sequencer;
pub mod sound;
mod validation;

pub use sequencer::{SequencerPattern, Step, Subdivision, TimeSignature, Track};
pub use sound::{SampleBuffer, Sound};
pub use validation::PatternError;
