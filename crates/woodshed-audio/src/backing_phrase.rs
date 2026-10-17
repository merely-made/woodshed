//! Transient ordered phrase backing, prepared before audio reaches the callback.

use crate::{ChordRender, SampleBuffer, chord_audio::render_chord};

/// One grouped chord event with an explicit beat or absolute-seconds position.
#[derive(Clone, Debug, PartialEq)]
pub struct BackingEvent {
    pub time: audio_primitives::timing::EventTime,
    pub pitches_hz: Vec<f32>,
    pub strum_seconds: f64,
}

/// Ordered backing events projected into one bar.
#[derive(Clone, Debug, PartialEq)]
pub struct BackingPhrase {
    pub events: Vec<BackingEvent>,
}

const MAX_EVENTS: usize = 4096;
const MAX_TOTAL_PITCHES: usize = 4096;
const MAX_GROUP_PITCHES: usize = 256;
/// Caps the sum of rendered pitch/frame iterations independently of PCM size.
pub(crate) const MAX_RENDER_WORK: u64 = 67_108_864;

pub(crate) struct PhrasePreflight {
    pub positions: Vec<(usize, usize)>,
    pub work_frames: u64,
}

pub(crate) fn preflight(
    phrase: &BackingPhrase,
    duration_frames: usize,
    beat_frames: u64,
    beats_per_bar: u64,
    rate: u32,
) -> Result<PhrasePreflight, &'static str> {
    if phrase.events.len() > MAX_EVENTS {
        return Err("backing phrase exceeds 4096 events");
    }
    let mut total_pitches = 0usize;
    let mut total_work = 0u64;
    let mut starts = Vec::with_capacity(phrase.events.len());
    let mut prior_frame = None;
    for event in &phrase.events {
        if let audio_primitives::timing::EventTime::Beats(beat) = event.time {
            if !beat.is_finite() || beat < 0.0 || beat >= beats_per_bar as f64 {
                return Err("backing phrase beats must be finite and inside the bar");
            }
        }
        let onset = usize::try_from(event.time.to_frames(beat_frames, rate)?)
            .map_err(|_| "backing phrase onset frame is not representable")?;
        if onset >= duration_frames || prior_frame.is_some_and(|prior| onset <= prior) {
            return Err(
                "backing phrase onset frames must be distinct, ordered, and inside the bar",
            );
        }
        prior_frame = Some(onset);
        if event.pitches_hz.is_empty() || event.pitches_hz.len() > MAX_GROUP_PITCHES {
            return Err("backing phrase groups need 1..=256 pitches");
        }
        total_pitches = total_pitches
            .checked_add(event.pitches_hz.len())
            .ok_or("backing phrase pitch count overflow")?;
        if total_pitches > MAX_TOTAL_PITCHES {
            return Err("backing phrase exceeds 4096 total pitches");
        }
        if !event.strum_seconds.is_finite() || event.strum_seconds < 0.0 {
            return Err("backing phrase strum spacing must be finite and nonnegative");
        }
        let strum = usize::try_from(
            audio_primitives::timing::EventTime::Seconds(event.strum_seconds)
                .to_frames(beat_frames, rate)?,
        )
        .map_err(|_| "backing phrase strum frame is not representable")?;
        if event.pitches_hz.len() > 1 && event.strum_seconds > 0.0 && strum == 0 {
            return Err("backing phrase strum offsets must be distinct ordered frames");
        }
        let remaining = duration_frames - onset;
        let last_strum = strum
            .checked_mul(event.pitches_hz.len().saturating_sub(1))
            .ok_or("backing phrase strum offsets overflow")?;
        if last_strum >= remaining {
            return Err("backing phrase strum offsets must fit inside the bar");
        }
        let work = (event.pitches_hz.len() as u64)
            .checked_mul(remaining as u64)
            .ok_or("backing phrase render work overflow")?;
        total_work = total_work
            .checked_add(work)
            .ok_or("backing phrase render work overflow")?;
        if total_work > MAX_RENDER_WORK {
            return Err("backing phrase exceeds render-work bound");
        }
        for &frequency in &event.pitches_hz {
            if !frequency.is_finite()
                || frequency <= 0.0
                || frequency as f64
                    * (remaining as f64 / rate as f64)
                    * 3.0
                    * std::f32::consts::TAU as f64
                    > f32::MAX as f64 / 2.0
            {
                return Err("backing phrase pitch cannot render finite PCM");
            }
        }
        starts.push((onset, strum));
    }
    Ok(PhrasePreflight {
        positions: starts,
        work_frames: total_work,
    })
}

pub(crate) fn render(
    phrase: &BackingPhrase,
    duration_frames: usize,
    beat_frames: u64,
    beats_per_bar: u64,
    rate: u32,
) -> Result<SampleBuffer, &'static str> {
    let positions = preflight(phrase, duration_frames, beat_frames, beats_per_bar, rate)?.positions;
    let total_pitches: usize = phrase
        .events
        .iter()
        .map(|event| event.pitches_hz.len())
        .sum();
    let mut output = vec![0.0f32; duration_frames];
    let voice_amplitude = 0.45 / (total_pitches.max(1) as f32).sqrt();
    for (event, (onset, strum_frames)) in phrase.events.iter().zip(positions) {
        for (voice_index, &pitch) in event.pitches_hz.iter().enumerate() {
            let voice_onset = onset + voice_index * strum_frames;
            let remaining = duration_frames - voice_onset;
            let duration_seconds = remaining as f32 / rate as f32;
            let pcm = render_chord(
                &ChordRender {
                    pitches_hz: vec![pitch],
                    duration_seconds,
                    amplitude: voice_amplitude,
                    attack_seconds: 0.015,
                    release_seconds: 0.25,
                    strum_offset_ms: 0.0,
                },
                rate,
            );
            for (dst, sample) in output[voice_onset..].iter_mut().zip(pcm.data.iter()) {
                *dst = (*dst + *sample).clamp(-1.0, 1.0);
            }
        }
    }
    Ok(SampleBuffer::new(output, rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_onsets_keep_exact_frames_across_tempo_and_reject_frame_collisions() {
        use audio_primitives::timing::EventTime;
        let phrase = BackingPhrase {
            events: vec![
                BackingEvent {
                    time: EventTime::Seconds(0.0137),
                    pitches_hz: vec![220.0],
                    strum_seconds: 0.0,
                },
                BackingEvent {
                    time: EventTime::Seconds(0.1031),
                    pitches_hz: vec![330.0],
                    strum_seconds: 0.0,
                },
            ],
        };
        for rate in [44_100, 48_000] {
            let frames = audio_primitives::timing::seconds_to_frames(0.137, rate).unwrap() as usize;
            let slow = render(&phrase, frames, 100_000, 1, rate).unwrap();
            let fast = render(&phrase, frames, 1, 1, rate).unwrap();
            assert_eq!(
                slow.data.iter().map(|s| s.to_bits()).collect::<Vec<_>>(),
                fast.data.iter().map(|s| s.to_bits()).collect::<Vec<_>>()
            );
            let onset = (0.0137 * rate as f64).round() as usize;
            assert!(slow.data[..onset].iter().all(|s| *s == 0.0));
            assert!(slow.data[onset..].iter().any(|s| *s != 0.0));
        }
        let mut collision = phrase.clone();
        collision.events[1].time = EventTime::Seconds(0.013700001);
        assert!(preflight(&collision, 1_000, 1, 1, 1_000).is_err());
        collision.events[1].time = EventTime::Seconds(1.0);
        assert!(preflight(&collision, 1_000, 1, 1, 1_000).is_err());
    }

    #[test]
    fn seconds_strum_and_render_work_are_bounded_before_render() {
        use audio_primitives::timing::EventTime;
        let mut phrase = BackingPhrase {
            events: vec![BackingEvent {
                time: EventTime::Seconds(0.09),
                pitches_hz: vec![220.0, 330.0],
                strum_seconds: 0.01,
            }],
        };
        assert!(preflight(&phrase, 100, 1, 1, 1_000).is_err());
        phrase.events[0].strum_seconds = 0.009;
        assert!(preflight(&phrase, 100, 1, 1, 1_000).is_ok());
        phrase.events[0].time = EventTime::Seconds(0.0);
        phrase.events[0].pitches_hz = vec![220.0; 256];
        assert!(preflight(&phrase, 1_000_000, 1, 1, 1_000).is_err());
    }

    #[test]
    fn phrase_is_bounded_to_exact_bar_frames_and_validates_ordered_onsets() {
        let phrase = BackingPhrase {
            events: vec![
                BackingEvent {
                    time: audio_primitives::timing::EventTime::Beats(0.5),
                    pitches_hz: vec![220.0],
                    strum_seconds: 0.0,
                },
                BackingEvent {
                    time: audio_primitives::timing::EventTime::Beats(2.0),
                    pitches_hz: vec![330.0],
                    strum_seconds: 0.0,
                },
            ],
        };
        for (rate, beat, beats) in [(48_000, 24_000, 3), (44_100, 18_900, 7)] {
            let frames = (beat * beats) as usize;
            let pcm = render(&phrase, frames, beat, beats, rate).unwrap();
            assert_eq!(pcm.len(), frames);
            assert!(
                pcm.data[..(beat as usize / 2)]
                    .iter()
                    .all(|sample| *sample == 0.0)
            );
            assert!(pcm.data.iter().any(|sample| *sample != 0.0));
        }
        let collision = BackingPhrase {
            events: vec![
                BackingEvent {
                    time: audio_primitives::timing::EventTime::Beats(0.0),
                    pitches_hz: vec![220.0],
                    strum_seconds: 0.0,
                },
                BackingEvent {
                    time: audio_primitives::timing::EventTime::Beats(0.00001),
                    pitches_hz: vec![330.0],
                    strum_seconds: 0.0,
                },
            ],
        };
        assert!(preflight(&collision, 100, 1, 4, 48_000).is_err());
    }

    #[test]
    fn phrase_rejects_unbounded_and_nonfinite_inputs_before_render() {
        let invalid = BackingPhrase {
            events: vec![BackingEvent {
                time: audio_primitives::timing::EventTime::Beats(f64::NAN),
                pitches_hz: vec![440.0],
                strum_seconds: 0.0,
            }],
        };
        assert!(preflight(&invalid, 100, 25, 4, 48_000).is_err());
        let too_many = BackingPhrase {
            events: (0..MAX_EVENTS + 1)
                .map(|i| BackingEvent {
                    time: audio_primitives::timing::EventTime::Beats(i as f64 / 5000.0),
                    pitches_hz: vec![440.0],
                    strum_seconds: 0.0,
                })
                .collect(),
        };
        assert!(preflight(&too_many, 100_000, 25, 4, 48_000).is_err());
    }

    #[test]
    fn phrase_rejects_render_work_before_pcm_allocation() {
        let phrase = BackingPhrase {
            events: (0..256)
                .map(|index| BackingEvent {
                    time: audio_primitives::timing::EventTime::Beats((300 + index) as f64),
                    pitches_hz: vec![220.0; 4],
                    strum_seconds: 0.0,
                })
                .collect(),
        };
        let layout = preflight(&phrase, 100_000, 100, 1000, 1000).unwrap();
        assert!(layout.work_frames < MAX_RENDER_WORK);
        assert!(preflight(&phrase, 100_000, 100, 1000, 1000).is_ok());
        let denser = BackingPhrase {
            events: phrase
                .events
                .iter()
                .map(|event| BackingEvent {
                    pitches_hz: vec![220.0; 16],
                    ..event.clone()
                })
                .collect(),
        };
        assert!(preflight(&denser, 100_000, 100, 1000, 1000).is_err());
    }

    #[test]
    fn fractional_rate_strum_rounding_preserves_the_integer_voice_onset() {
        for rate in [44_100, 48_000] {
            let spacing = (0.018_f64 * rate as f64).round() as usize;
            let plain = render(
                &BackingPhrase {
                    events: vec![BackingEvent {
                        time: audio_primitives::timing::EventTime::Beats(0.0),
                        pitches_hz: vec![173.3],
                        strum_seconds: 0.0,
                    }],
                },
                10_000,
                10_000,
                1,
                rate,
            )
            .unwrap();
            let strummed = render(
                &BackingPhrase {
                    events: vec![BackingEvent {
                        time: audio_primitives::timing::EventTime::Beats(0.0),
                        pitches_hz: vec![173.3, 261.6],
                        strum_seconds: 0.018,
                    }],
                },
                10_000,
                10_000,
                1,
                rate,
            )
            .unwrap();
            let first_voice_scale = 1.0 / 2.0_f32.sqrt();
            for frame in 0..spacing {
                assert!(
                    (strummed.data[frame] - plain.data[frame] * first_voice_scale).abs() < 1e-6
                );
            }
            assert_ne!(
                strummed.data[spacing + 1].to_bits(),
                (plain.data[spacing + 1] * first_voice_scale).to_bits(),
                "second voice must enter at its validated frame for {rate} Hz"
            );
        }
    }
}
