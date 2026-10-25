//! Audio engine for Woodshed.
//!
//! ## Output path
//! - [`sequencer`] — pure data: patterns, tracks, steps, time signatures,
//!   subdivisions. Serde-serializable for preset persistence.
//! - [`sound`] — sample-by-sample synthesis: [`Sound::Click`] (sine
//!   burst + envelope) and [`Sound::Sample`] (loaded WAV with gain +
//!   pitch shift).
//! - [`samples`] — [`SampleBank`] for loading WAV files into runtime
//!   buffers.
//! - [`output_runtime`] — one Firewheel graph/device clock with bounded
//!   processor-owned click, preview, backing and recording lanes.
//! - [`engine`] — shared native input operation errors.
//!
//! ## Input path
//! - [`input`] — [`InputEngine`] owns one cpal input stream; analyzers
//!   plug in via [`InputEngineBuilder`] and the [`input::Analyzer`] trait.
//! - [`input::PitchAnalyzer`] — drives the tuner.
//! - [`onset`] — [`onset::OnsetAnalyzer`] for transient detection, plus
//!   the pure [`OnsetDetector`] + [`estimate_bpm`] for audio-tap-tempo.
//!
//! ## Cross-cutting
//! - [`calibration`] — measures input-to-output round-trip latency by
//!   correlating known click times against detected onsets. Reusable
//!   widget shape; pure DSP helper + a driver session struct.
//! - [`offline`] — render a pattern to a `Vec<f32>` or `.wav` without
//!   going through cpal.
//! - [`midi`] — MIDI in/out, clock-slave BPM derivation.
//! - [`looper`] — bar-aligned audio loop record + overdub.

pub mod backing_phrase;
pub mod calibration;
pub mod capture_feed;
pub mod chord_audio;
pub mod engine;
pub mod input;
pub mod looper;
pub use woodshed_instrument::midi;
pub mod offline;
pub mod onset;
pub mod output_runtime;
// These modules have one shared owner; keep the runtime's public audio facade.
pub use audio_packs::samples;
pub use audio_pattern::{sequencer, sound};
pub mod song;

pub use backing_phrase::{BackingEvent, BackingPhrase};
pub use calibration::{CalibrationOutcome, CalibrationSession, estimate_latency_from_pairs};
pub use chord_audio::{ChordRender, render_chord};
pub use engine::AudioError;
pub use input::{
    Analyzer, DetectedNote, DetectedNoteName, DetectorKind, InputEngine, InputEngineBuilder,
    InputHealth, LooperCaptureAnalyzer, LooperCaptureHandle, PitchAnalyzer, TunerEngine,
    TunerHandle, TunerSnapshot,
};
pub use looper::Looper;
pub use midi::{
    MidiClockSync, MidiError, MidiEvent, MidiIn, MidiInState, MidiOut,
    list_input_ports as list_midi_input_ports, list_output_ports as list_midi_output_ports,
};
pub use offline::{
    SongExportError, SongExportReport, export_song_wav_to, export_wav, render_pattern,
};
pub use onset::{
    DetectedOnset, OnsetAnalyzer, OnsetDetector, OnsetHandle, OnsetSnapshot, estimate_bpm,
};
pub use output_runtime::{OutputError, OutputRuntime, OutputSnapshot};
pub use samples::{SampleBank, SampleError};
pub use sequencer::{SequencerPattern, Step, Subdivision, TimeSignature, Track};
pub use song::{Bar, ChordRef, PendingChange, Song, SongCursor, SongError};
pub use sound::{SampleBuffer, Sound};
