// SPDX-License-Identifier: MPL-2.0
//! A derived MIDI projection of the existing step pattern. This owns no clock,
//! device, file path or second musical document. Channels in authoring data are
//! 1–16; conversion to the wire's 0–15 happens here.
use crate::{PatternError, SequencerPattern, Step};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiVoice {
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    pub accent_velocity: u8,
    /// Fraction of one step, 1–100. Note endings never cross its next onset.
    pub gate_percent: u8,
}

impl MidiVoice {
    pub fn validate(self) -> Result<(), PatternError> {
        if !(1..=16).contains(&self.channel)
            || self.note > 127
            || !(1..=127).contains(&self.velocity)
            || !(1..=127).contains(&self.accent_velocity)
            || !(1..=100).contains(&self.gate_percent)
        {
            return Err(PatternError(
                "Invalid MIDI channel, note, velocity or gate".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MidiExport {
    pub bytes: Vec<u8>,
    pub ticks_per_quarter: u16,
    pub length_ticks: u32,
    pub notes: usize,
}

/// Export exactly one bar as a format-0 file, retaining the terminal boundary
/// even when its last steps are rests. Each entry maps the corresponding track;
/// only muted tracks may omit their mapping. Sample buffers are never read.
pub fn export_midi(
    pattern: &SequencerPattern,
    mappings: &[Option<MidiVoice>],
) -> Result<MidiExport, PatternError> {
    pattern.validate()?;
    if mappings.len() != pattern.tracks.len() {
        return Err(PatternError(
            "MIDI mappings must cover the pattern's tracks".into(),
        ));
    }
    let denominator = u32::from(pattern.time_signature.denominator);
    let divisions = u32::from(pattern.subdivision.divisions_per_beat);
    // Preserve PPQ 480 where it is exact. Increase it only when the authored
    // notated-beat subdivision needs a finer integral grid.
    let divisor = denominator * divisions;
    let needed = divisor / gcd(divisor, 4);
    let ppq = 480 / gcd(480, needed) * needed;
    if ppq > 0x7fff {
        return Err(PatternError(
            "Authored grid exceeds MIDI timing precision".into(),
        ));
    }
    let step_ticks = ppq * 4 / divisor;
    let length_ticks = step_ticks * pattern.steps_per_bar() as u32;
    let tempo = (60_000_000.0 / f64::from(pattern.bpm)).round() as u32;
    if tempo == 0 || tempo > 0xff_ffff {
        return Err(PatternError(
            "Tempo cannot be represented by a MIDI tempo event".into(),
        ));
    }
    let mut events = vec![
        (0, 0, TrackEventKind::Meta(MetaMessage::Tempo(tempo.into()))),
        (
            0,
            0,
            TrackEventKind::Meta(MetaMessage::TimeSignature(
                pattern.time_signature.numerator,
                pattern.time_signature.denominator.ilog2() as u8,
                24,
                8,
            )),
        ),
    ];
    let mut identities = BTreeSet::new();
    let mut notes = 0;
    for (track, voice) in pattern.tracks.iter().zip(mappings) {
        if let Some(voice) = voice {
            voice.validate()?;
        }
        if track.muted {
            continue;
        }
        let voice = voice
            .ok_or_else(|| PatternError(format!("Track {} has no MIDI mapping", track.name)))?;
        if !identities.insert((voice.channel, voice.note)) {
            return Err(PatternError(
                "Active tracks need distinct MIDI channel/note pairs".into(),
            ));
        }
        let gate = (step_ticks * u32::from(voice.gate_percent) / 100).max(1);
        for (index, step) in track.steps.iter().enumerate() {
            let Step::Active { accent } = step else {
                continue;
            };
            let tick = index as u32 * step_ticks;
            let channel = (voice.channel - 1).into();
            let key = voice.note.into();
            let vel = if *accent {
                voice.accent_velocity
            } else {
                voice.velocity
            }
            .into();
            events.push((
                tick,
                2,
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, vel },
                },
            ));
            events.push((
                tick + gate,
                1,
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOff { key, vel: 0.into() },
                },
            ));
            notes += 1;
        }
    }
    // Note-offs sort before note-ons at a shared boundary, and EndOfTrack last.
    events.push((
        length_ticks,
        3,
        TrackEventKind::Meta(MetaMessage::EndOfTrack),
    ));
    events.sort_by_key(|(tick, priority, _)| (*tick, *priority));
    let mut previous = 0;
    let track = events
        .into_iter()
        .map(|(tick, _, kind)| {
            let delta = tick - previous;
            previous = tick;
            TrackEvent {
                delta: delta.into(),
                kind,
            }
        })
        .collect();
    let smf = Smf {
        header: Header::new(Format::SingleTrack, Timing::Metrical((ppq as u16).into())),
        tracks: vec![track],
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .map_err(|e| PatternError(format!("MIDI encoding: {e}")))?;
    Ok(MidiExport {
        bytes,
        ticks_per_quarter: ppq as u16,
        length_ticks,
        notes,
    })
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
