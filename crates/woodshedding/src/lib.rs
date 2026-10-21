//! Portable core for woodshedding: turning musical material into
//! playable practice.
//!
//! This crate follows the gerund-crate convention used across Mark's
//! repos: the gerund names the reusable operation core, not the app shell.
//! `woodshedding` is therefore not just a bag of "data structures"; it is
//! the pure operation layer that lets consumers identify musical material,
//! realize it on a stringed instrument, organize it into progressions or
//! exercises, and generate practice sets.
//!
//! Pure data + math. No I/O, no UI, no audio. Consumers (the Woodshed app,
//! future web frontend, future CLI) depend on this crate for the canonical
//! model of pitches, intervals, tunings, scales, chords, progressions,
//! exercises, fretboard mappings, voicings, and practice sets.
//!
//! [`notation`] reads and writes the editable fretted MusicXML profile.
//! [`timing`] derives exact note/rest schedules from that music, without
//! depending on an application or an output engine.

pub mod card_shapes;
pub mod chord;
pub mod exercise;
pub mod fretboard;
pub mod interval;
pub mod notation;
pub mod pitch;
pub mod pitch_class_set;
pub mod practice;
pub mod progression;
pub mod rehearsal;
pub mod scale;
pub mod scale_pattern;
pub mod timing;
pub mod tuning;
