//! Bounded, disposable whole-Set playback plans.
//!
//! The caller supplies opaque tokens and already-lowered events. This module
//! neither imports product Card types nor owns practice history.

use super::{MAX_PROGRAM_CARDS, MAX_PROGRAM_EVENTS, MAX_PROGRAM_PCM_FRAMES, OutputError, err};
use crate::SampleBuffer;
use firewheel::collector::ArcGc;

/// One beat is divided into 960 integer ticks in a prepared program.
pub const PROGRAM_TICKS_PER_BEAT: u64 = 960;
/// Alias for consumers that want the tick scale under a short name.
pub const BEAT_TICKS: u64 = PROGRAM_TICKS_PER_BEAT;
pub const MAX_PROGRAM_TONE_FRAMES: usize = 65_536;
pub const MAX_PROGRAM_TONES: usize = 256;
/// Short periodic table used only when no integer-cycle loop meets the normal
/// frame and pitch-error bounds. Playback advances through it with a fixed
/// point phase accumulator and linear interpolation.
pub const FALLBACK_PERIODIC_WAVE_FRAMES: usize = 4096;
pub(super) const FALLBACK_PHASE_INDEX_BITS: u32 = 12;
pub(super) const FALLBACK_PHASE_FRACTION_BITS: u32 = 52;
const _: () = assert!(FALLBACK_PERIODIC_WAVE_FRAMES == 1usize << FALLBACK_PHASE_INDEX_BITS);
const _: () = assert!(FALLBACK_PHASE_INDEX_BITS + FALLBACK_PHASE_FRACTION_BITS == 64);

/// Caller-owned ephemeral identity. It is not persisted or interpreted here.
pub type ProgramToken = u64;

/// A complete disposable schedule. A caller may include refusal sentinels for
/// cards that cannot currently be prepared, allowing the audio boundary to
/// report them in sequence rather than silently skipping them.
#[derive(Clone, Debug)]
pub struct ProgramPlan {
    pub cards: Vec<ProgramCard>,
    pub start_index: usize,
    pub loop_mode: ProgramLoop,
}

#[derive(Clone, Debug)]
pub struct ProgramCard {
    pub token: ProgramToken,
    pub bpm: f64,
    pub end: ProgramEnd,
    pub events: Vec<ProgramEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramEnd {
    /// End after this many beat ticks. Events in this card use beat ticks.
    BeatTicks(u64),
    /// End after this many output frames. Events in this card use frames.
    DeviceFrames(u64),
    /// Play for the specified duration, then wait for an explicit advance.
    Manual {
        preview_ticks: u64,
        tail_frames: u64,
    },
    /// A known unsupported card. It waits for an explicit advance at its turn.
    Refusal(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramDuration {
    BeatTicks(u64),
    DeviceFrames(u64),
    UntilCardEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramLoop {
    Once,
    Set,
    Card,
}

#[derive(Clone, Debug)]
pub struct ProgramEvent {
    pub token: ProgramToken,
    pub onset: ProgramPosition,
    /// Fixed output-frame offset for grouped pitches, used for tight chord
    /// voicing while the group remains positioned on the shared beat clock.
    pub onset_offset_frames: u64,
    pub gate: ProgramDuration,
    pub frequency_hz: f64,
    pub amplitude: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProgramPosition {
    BeatTicks(u64),
    DeviceFrames(u64),
}

/// A generation-like runtime-local command fence. It is not a durable ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgramEpoch(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub enum ProgramStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
    WaitingManual,
    Refused,
    Complete,
    WaitingObserver,
}

/// Coherent logical position published with the device-frame observation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgramSnapshot {
    /// Last boundary-applied replacement, or zero for the initial program.
    pub revision: u64,
    /// Current prepared segment count, including after a replacement.
    pub card_count: usize,
    pub epoch: ProgramEpoch,
    pub device_frame: u64,
    pub card_index: usize,
    pub card_token: Option<ProgramToken>,
    pub event_index: usize,
    pub event_token: Option<ProgramToken>,
    pub beat_ticks: u64,
    /// IEEE-754 bits for the current prepared card's applied BPM. Zero means
    /// the program has no current card (stopped before admission).
    pub bpm_bits: u64,
    /// Beat ticks between configured click subdivisions; zero means no click
    /// pattern is installed. This is the actual shared program-clock division.
    pub click_subdivision_ticks: u64,
    pub card_frames: u64,
    pub active_frames: u64,
    pub status: ProgramStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramTransitionKind {
    Started,
    Ended(ProgramEndReason),
    Refused(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramEndReason {
    Completed,
    Seek,
    Stop,
}

/// Card-level transition. Event position is available in ProgramSnapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramTransition {
    pub epoch: ProgramEpoch,
    pub card_token: ProgramToken,
    pub device_frame: u64,
    pub active_frames: u64,
    pub kind: ProgramTransitionKind,
}

#[derive(Debug)]
pub struct PreparedProgram {
    pub(super) rate: u32,
    pub(super) start_index: usize,
    pub(super) loop_mode: ProgramLoop,
    pub(super) cards: Vec<PreparedCard>,
    pub(super) tones: Vec<ArcGc<PreparedTone>>,
    pub(super) refusal_reasons: Vec<Option<String>>,
}

#[derive(Debug)]
pub(super) struct PreparedCard {
    pub token: ProgramToken,
    pub bpm: f64,
    pub end: ProgramEnd,
    pub events: Vec<PreparedEvent>,
}

#[derive(Debug)]
pub(super) struct PreparedEvent {
    pub token: ProgramToken,
    pub onset: ProgramPosition,
    pub onset_offset_frames: u64,
    pub gate: ProgramDuration,
    pub tone: usize,
    pub amplitude: f32,
}

#[derive(Debug)]
pub(super) struct PreparedTone {
    pub pcm: SampleBuffer,
    pub requested_frequency_hz: f64,
    pub actual_frequency_hz: f64,
    /// Present only for the short fractional-phase table. The ordinary
    /// integer-cycle PCM path keeps its exact one-frame stepping behavior.
    pub phase_step_q64: Option<u64>,
    #[cfg(test)]
    pub cycles: usize,
}

impl PreparedProgram {
    pub(super) fn prepare(plan: ProgramPlan, rate: u32) -> Result<Self, OutputError> {
        if plan.cards.is_empty() || plan.cards.len() > MAX_PROGRAM_CARDS {
            return Err(err("program needs 1..=256 cards"));
        }
        if plan.start_index >= plan.cards.len() {
            return Err(err("program start index is outside the card list"));
        }

        let mut tones = Vec::new();
        let mut tone_keys: Vec<u64> = Vec::new();
        let mut cards = Vec::with_capacity(plan.cards.len());
        let mut refusal_reasons = Vec::with_capacity(plan.cards.len());
        let mut event_count = 0usize;
        let mut pcm_frames = 0usize;
        for card in plan.cards {
            if !card.bpm.is_finite() || card.bpm <= 0.0 {
                return Err(err("card tempo must be finite and positive"));
            }
            let ticks_q32 = card.bpm * PROGRAM_TICKS_PER_BEAT as f64 * (u32::MAX as f64 + 1.0)
                / (60.0 * rate as f64);
            if !ticks_q32.is_finite() || ticks_q32 < 1.0 || ticks_q32 > u64::MAX as f64 {
                cards.push(PreparedCard {
                    token: card.token,
                    bpm: card.bpm,
                    end: ProgramEnd::Refusal(2),
                    events: Vec::new(),
                });
                refusal_reasons.push(Some(
                    "card tempo is outside the representable output clock range".to_owned(),
                ));
                continue;
            }
            let (domain, length) = match card.end {
                ProgramEnd::BeatTicks(ticks) => (0u8, ticks),
                ProgramEnd::DeviceFrames(frames) => (1u8, frames),
                ProgramEnd::Manual { preview_ticks, .. } => (0u8, preview_ticks),
                ProgramEnd::Refusal(_) => (2u8, 0),
            };
            if domain == 2 && !card.events.is_empty() {
                return Err(err("refusal sentinel cannot contain events"));
            }
            if domain != 2
                && length == 0
                && !matches!(card.end, ProgramEnd::Manual { preview_ticks: 0, tail_frames } if tail_frames > 0)
            {
                return Err(err("program card duration must be positive"));
            }
            if domain == 2 {
                cards.push(PreparedCard {
                    token: card.token,
                    bpm: card.bpm,
                    end: card.end,
                    events: Vec::new(),
                });
                refusal_reasons.push(Some("caller-declared unsupported card".to_owned()));
                continue;
            }
            if matches!(card.end, ProgramEnd::Manual { tail_frames: 0, .. }) {
                return Err(err(
                    "manual card tail must contain at least one output frame",
                ));
            }
            event_count = event_count
                .checked_add(card.events.len())
                .ok_or_else(|| err("program event count overflow"))?;
            if event_count > MAX_PROGRAM_EVENTS {
                return Err(err("program exceeds 4096 events"));
            }
            let mut events = Vec::with_capacity(card.events.len());
            let mut previous = None;
            let mut card_refusal = None;
            for event in card.events {
                if event.onset_offset_frames > MAX_PROGRAM_PCM_FRAMES as u64 {
                    return Err(err("event intra-group offset exceeds 16777216 frames"));
                }
                let (event_domain, onset) = match event.onset {
                    ProgramPosition::BeatTicks(ticks) => (0u8, ticks),
                    ProgramPosition::DeviceFrames(frames) => (1u8, frames),
                };
                let (gate_domain, gate) = match event.gate {
                    ProgramDuration::BeatTicks(ticks) => (0u8, ticks),
                    ProgramDuration::DeviceFrames(frames) => (1u8, frames),
                    ProgramDuration::UntilCardEnd => (domain, 0),
                };
                if event_domain != domain || gate_domain != domain {
                    return Err(err(
                        "event position and gate must match its card clock domain",
                    ));
                }
                let offset_units = if domain == 0 {
                    (event.onset_offset_frames as f64 * card.bpm * PROGRAM_TICKS_PER_BEAT as f64
                        / (60.0 * rate as f64))
                        .ceil() as u64
                } else {
                    event.onset_offset_frames
                };
                let effective_onset = onset
                    .checked_add(offset_units)
                    .ok_or_else(|| err("event offset exceeds the card clock domain"))?;
                let effective_gate = if event.gate == ProgramDuration::UntilCardEnd {
                    length.checked_sub(effective_onset).unwrap_or(0)
                } else {
                    gate
                };
                let manual_final_tick = matches!(card.end, ProgramEnd::Manual { preview_ticks, .. } if onset == preview_ticks && event.gate == ProgramDuration::UntilCardEnd);
                if manual_final_tick
                    && matches!(card.end, ProgramEnd::Manual { tail_frames, .. } if event.onset_offset_frames >= tail_frames)
                {
                    return Err(err(
                        "manual final event offset must fit within its fixed tail",
                    ));
                }
                if (effective_onset >= length && !manual_final_tick)
                    || (effective_gate == 0 && !manual_final_tick)
                    || (!manual_final_tick
                        && effective_onset
                            .checked_add(effective_gate)
                            .is_none_or(|end| end > length))
                {
                    return Err(err("event timing must be positive and fit within its card"));
                }
                let order = (onset, event.onset_offset_frames);
                if previous.is_some_and(|p| order < p) {
                    return Err(err("program events must be ordered by onset"));
                }
                previous = Some(order);
                if !event.frequency_hz.is_finite()
                    || event.frequency_hz <= 0.0
                    || event.frequency_hz >= rate as f64 * 0.5
                {
                    card_refusal = Some(
                        "program event frequency is nonpositive or outside Nyquist".to_owned(),
                    );
                    break;
                }
                if !event.amplitude.is_finite() || !(0.0..=1.0).contains(&event.amplitude) {
                    return Err(err(
                        "program event amplitude must be finite and within 0..=1",
                    ));
                }
                let key = event.frequency_hz.to_bits();
                let tone = if let Some(index) = tone_keys.iter().position(|k| *k == key) {
                    index
                } else {
                    if tones.len() == MAX_PROGRAM_TONES {
                        card_refusal = Some("program exceeds 256 distinct tones".to_owned());
                        break;
                    }
                    let tone = match prepare_tone(event.frequency_hz, rate) {
                        Ok(tone) => tone,
                        Err(problem) => {
                            card_refusal = Some(problem.0);
                            break;
                        },
                    };
                    if (tone.actual_frequency_hz - event.frequency_hz).abs() / event.frequency_hz
                        > 1.0e-6
                    {
                        card_refusal =
                            Some("program tone exceeds the bounded frequency error".to_owned());
                        break;
                    }
                    pcm_frames = pcm_frames
                        .checked_add(tone.pcm.len())
                        .ok_or_else(|| err("program PCM bound overflow"))?;
                    if pcm_frames > MAX_PROGRAM_PCM_FRAMES {
                        pcm_frames -= tone.pcm.len();
                        card_refusal = Some("program PCM exceeds 16777216 frames".to_owned());
                        break;
                    }
                    tone_keys.push(key);
                    tones.push(ArcGc::new(tone));
                    tones.len() - 1
                };
                events.push(PreparedEvent {
                    token: event.token,
                    onset: event.onset,
                    onset_offset_frames: event.onset_offset_frames,
                    gate: event.gate,
                    tone,
                    amplitude: event.amplitude,
                });
            }
            if let Some(reason) = card_refusal {
                cards.push(PreparedCard {
                    token: card.token,
                    bpm: card.bpm,
                    end: ProgramEnd::Refusal(1),
                    events: Vec::new(),
                });
                refusal_reasons.push(Some(reason));
            } else {
                validate_polyphony(&events, card.bpm, rate)?;
                cards.push(PreparedCard {
                    token: card.token,
                    bpm: card.bpm,
                    end: card.end,
                    events,
                });
                refusal_reasons.push(None);
            }
        }
        Ok(Self {
            rate,
            start_index: plan.start_index,
            loop_mode: plan.loop_mode,
            cards,
            tones,
            refusal_reasons,
        })
    }
}

impl PreparedProgram {
    /// Control-side explanation for a card converted into a refusal sentinel.
    pub fn refusal_reason(&self, card_index: usize) -> Option<&str> {
        self.refusal_reasons
            .get(card_index)
            .and_then(Option::as_deref)
    }
}

fn validate_polyphony(events: &[PreparedEvent], bpm: f64, rate: u32) -> Result<(), OutputError> {
    let mut bounds = Vec::with_capacity(events.len() * 2);
    for event in events {
        let (start, end, domain) = match (event.onset, event.gate) {
            (ProgramPosition::BeatTicks(start), ProgramDuration::BeatTicks(gate)) => {
                let offset =
                    (event.onset_offset_frames as f64 * bpm * PROGRAM_TICKS_PER_BEAT as f64
                        / (60.0 * rate as f64))
                        .ceil() as u64;
                let start = start.saturating_add(offset);
                (start, start.saturating_add(gate), 0u8)
            },
            (ProgramPosition::BeatTicks(start), ProgramDuration::UntilCardEnd) => {
                let offset =
                    (event.onset_offset_frames as f64 * bpm * PROGRAM_TICKS_PER_BEAT as f64
                        / (60.0 * rate as f64))
                        .ceil() as u64;
                (start.saturating_add(offset), u64::MAX, 0u8)
            },
            (ProgramPosition::DeviceFrames(start), ProgramDuration::DeviceFrames(gate)) => {
                let start = start.saturating_add(event.onset_offset_frames);
                (start, start.saturating_add(gate), 1u8)
            },
            (ProgramPosition::DeviceFrames(start), ProgramDuration::UntilCardEnd) => (
                start.saturating_add(event.onset_offset_frames),
                u64::MAX,
                1u8,
            ),
            _ => return Err(err("event gate clock domain differs from onset")),
        };
        let tail = if domain == 0 {
            (bpm * 0.25 * PROGRAM_TICKS_PER_BEAT as f64 / 60.0).ceil() as u64
        } else {
            (rate as f64 * 0.25).ceil() as u64
        };
        bounds.push((start, 1i8));
        bounds.push((end.saturating_add(tail), -1i8));
    }
    bounds.sort_unstable_by_key(|(at, delta)| (*at, *delta));
    let mut active = 0i32;
    for (_, delta) in bounds {
        active += i32::from(delta);
        if active > super::MAX_PROGRAM_VOICES as i32 {
            return Err(err("program exceeds 256 overlapping prepared voices"));
        }
    }
    Ok(())
}

fn prepare_tone(frequency: f64, rate: u32) -> Result<PreparedTone, OutputError> {
    // Search the smallest integer-cycle period. This work is control-side and
    // bounded by 65,536 candidates per distinct tone.
    let mut selected = None;
    for frames in 1..=MAX_PROGRAM_TONE_FRAMES {
        let cycles = (frequency * frames as f64 / rate as f64).round();
        if cycles < 1.0 {
            continue;
        }
        let actual = cycles * rate as f64 / frames as f64;
        if (actual - frequency).abs() / frequency <= 1.0e-6 {
            selected = Some((frames, cycles as usize, actual));
            break;
        }
    }
    let Some((frames, cycles, actual_frequency_hz)) = selected else {
        return prepare_fractional_tone(frequency, rate);
    };
    let mut pcm = Vec::with_capacity(frames);
    for frame in 0..frames {
        let phase = std::f64::consts::TAU * cycles as f64 * frame as f64 / frames as f64;
        // Preserve Woodshed's preview tone. Only preparation evaluates these
        // harmonics; the callback reads native-rate periodic PCM.
        pcm.push((phase.sin() + (phase * 2.0).sin() * 0.25 + (phase * 3.0).sin() * 0.12) as f32);
    }
    if pcm.iter().any(|sample| !sample.is_finite()) {
        return Err(err("program tone synthesis produced a nonfinite sample"));
    }
    Ok(PreparedTone {
        pcm: SampleBuffer::new(pcm, rate),
        requested_frequency_hz: frequency,
        actual_frequency_hz,
        phase_step_q64: None,
        #[cfg(test)]
        cycles,
    })
}

fn prepare_fractional_tone(frequency: f64, rate: u32) -> Result<PreparedTone, OutputError> {
    // The callback cannot synthesize or allocate, so prepare one cycle of the
    // existing Woodshed waveform here and move through it at the requested
    // output rate. A rounded Q64 step has much finer resolution than the
    // existing 1 ppm admission limit; keep that limit as an explicit guard.
    let phase_step_q64 = (frequency / rate as f64 * 18_446_744_073_709_551_616.0).round() as u64;
    let actual_frequency_hz = phase_step_q64 as f64 / 18_446_744_073_709_551_616.0
        * rate as f64;
    if phase_step_q64 == 0
        || !actual_frequency_hz.is_finite()
        || (actual_frequency_hz - frequency).abs() / frequency > 1.0e-6
    {
        return Err(err("program tone exceeds the bounded frequency error"));
    }

    let mut pcm = Vec::with_capacity(FALLBACK_PERIODIC_WAVE_FRAMES);
    for frame in 0..FALLBACK_PERIODIC_WAVE_FRAMES {
        let phase = std::f64::consts::TAU * frame as f64 / FALLBACK_PERIODIC_WAVE_FRAMES as f64;
        pcm.push((phase.sin() + (phase * 2.0).sin() * 0.25 + (phase * 3.0).sin() * 0.12) as f32);
    }
    if pcm.iter().any(|sample| !sample.is_finite()) {
        return Err(err("program tone synthesis produced a nonfinite sample"));
    }
    Ok(PreparedTone {
        pcm: SampleBuffer::new(pcm, rate),
        requested_frequency_hz: frequency,
        actual_frequency_hz,
        phase_step_q64: Some(phase_step_q64),
        #[cfg(test)]
        cycles: 0,
    })
}
