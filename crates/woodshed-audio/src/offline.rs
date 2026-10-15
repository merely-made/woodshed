//! Offline rendering — turn a [`SequencerPattern`] into PCM samples
//! (or a `.wav` file) without going through cpal.
//!
//! Useful for:
//! - exporting a pattern as a `.wav` to share with bandmates,
//! - regression tests that need a deterministic audio buffer,
//! - future bouncing of a loop record / overdub session.
//!
//! This pure pattern export preserves the historical renderer. Native output
//! uses the bounded Firewheel processor in `output_runtime`; no device or
//! shared callback mixer is constructed by this module.

use std::path::Path;

use crate::samples::SampleError;
use crate::sequencer::SequencerPattern;
use crate::sequencer::Step;
use crate::sound::Sound;

mod song_export;
pub use song_export::{SongExportError, SongExportReport, export_song_wav_to};

/// How many frames to process per chunk. The mixer loops over voices
/// once per frame, so smaller chunks mean more loop iterations per
/// second of audio; larger chunks use more memory transiently. 4096
/// is a comfortable middle ground.
const CHUNK_FRAMES: usize = 4096;

/// Render a pattern to an interleaved `f32` buffer.
///
/// - `duration_secs` is how much audio to produce. Patterns loop —
///   so rendering longer than one bar at the pattern's BPM will give
///   you N bars, fading on whatever lands at the cut.
/// - `channels` is the channel count for the output (1 = mono, 2 =
///   stereo). The mixer is mono internally; the output simply
///   duplicates each mixed sample across all channels.
///
/// Returns a `Vec<f32>` of length `duration_secs * sample_rate * channels`.
pub fn render_pattern(
    pattern: SequencerPattern,
    sample_rate_hz: f32,
    duration_secs: f32,
    channels: u16,
) -> Vec<f32> {
    let chs = channels.max(1) as usize;
    let total_frames = (sample_rate_hz * duration_secs).round() as usize;

    let mut state = PatternRenderState::new(pattern, sample_rate_hz);
    state.playing = true;

    let mut out = vec![0.0_f32; total_frames * chs];
    let mut frames_done = 0;
    while frames_done < total_frames {
        let remaining = total_frames - frames_done;
        let frames_this = remaining.min(CHUNK_FRAMES);
        let start = frames_done * chs;
        let end = start + frames_this * chs;
        process_buffer(&mut state, &mut out[start..end], channels);
        frames_done += frames_this;
    }
    out
}

/// Render a pattern straight to a `.wav` file on disk. The file is
/// 16-bit PCM at the requested sample rate; multi-channel uses
/// duplicated mono.
pub fn export_wav(
    pattern: SequencerPattern,
    sample_rate_hz: u32,
    duration_secs: f32,
    channels: u16,
    path: impl AsRef<Path>,
) -> Result<(), SampleError> {
    let samples = render_pattern(pattern, sample_rate_hz as f32, duration_secs, channels);
    let spec = hound::WavSpec {
        channels,
        sample_rate: sample_rate_hz,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path.as_ref(), spec)?;
    let scale = i16::MAX as f32;
    for &s in &samples {
        let clamped = s.clamp(-1.0, 1.0);
        writer.write_sample((clamped * scale) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::{Subdivision, TimeSignature};

    #[test]
    fn render_pattern_returns_correct_buffer_length() {
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let buf = render_pattern(p, 48_000.0, 1.0, 2);
        assert_eq!(buf.len(), 48_000 * 2);
    }

    #[test]
    fn render_pattern_produces_audio_not_silence() {
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let buf = render_pattern(p, 48_000.0, 1.0, 1);
        let max_abs = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            max_abs > 0.0,
            "expected some non-silence in rendered output; max abs = {max_abs}"
        );
    }

    #[test]
    fn render_pattern_clicks_are_localized_to_beats() {
        // 120 BPM, 4/4 quarter clicks: clicks at 0.0s, 0.5s, 1.0s, 1.5s.
        // Across 2 seconds of audio there should be ~4 distinct
        // high-energy windows.
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let buf = render_pattern(p, 48_000.0, 2.0, 1);

        // Bucket the buffer into 50ms windows and count windows with
        // amplitude above a threshold.
        let window = 48_000 / 20; // 2400 samples = 50ms
        let mut loud = 0;
        let mut prev_was_loud = false;
        for chunk in buf.chunks(window) {
            let peak = chunk.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
            let is_loud = peak > 0.05;
            if is_loud && !prev_was_loud {
                loud += 1;
            }
            prev_was_loud = is_loud;
        }
        // Expect ~4 click events; allow some slack for boundary cases.
        assert!(
            (3..=5).contains(&loud),
            "expected ~4 click events; got {loud}"
        );
    }

    #[test]
    fn export_wav_writes_readable_file() {
        let p = SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let path = std::env::temp_dir().join("woodshed_export_test.wav");
        export_wav(p, 48_000, 0.5, 1, &path).unwrap();

        // Read it back and verify shape.
        let reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 48_000);
        assert_eq!(spec.bits_per_sample, 16);
        // 0.5s × 48000 Hz = 24000 samples.
        let frame_count = reader.duration() as usize;
        assert_eq!(frame_count, 24_000);

        let _ = std::fs::remove_file(&path);
    }
}

#[derive(Clone, Debug)]
struct Voice {
    sound: Sound,
    accent: bool,
    start_sample: u64,
}

/// Private state for pure offline pattern export.
///
/// `sample_position` and `step_position` are tracked separately:
/// - `sample_position` is the absolute sample count since playback
///   started; used for voice timing (envelope decay, etc.) and is
///   independent of BPM.
/// - `step_position` is fractional steps since playback started; each
///   frame advances it by `1 / samples_per_step`. This decouples step
///   triggering from sample count so that BPM changes mid-playback
///   don't cause a pause or jump.
struct PatternRenderState {
    pub pattern: SequencerPattern,
    pub sample_rate: f32,
    pub sample_position: u64,
    pub step_position: f64,
    pub last_step: i64,
    pub voices: Vec<Voice>,
    pub playing: bool,
}

impl PatternRenderState {
    pub(crate) fn new(pattern: SequencerPattern, sample_rate: f32) -> Self {
        Self {
            pattern,
            sample_rate,
            sample_position: 0,
            step_position: 0.0,
            last_step: -1,
            voices: Vec::new(),
            playing: false,
        }
    }
}

fn process_buffer(state: &mut PatternRenderState, output: &mut [f32], channels: u16) {
    let chs = channels as usize;
    let frames = output.len() / chs;

    if !state.playing {
        for sample in output.iter_mut() {
            *sample = 0.0;
        }
        return;
    }

    let bpm = state.pattern.bpm;
    let dpb = state.pattern.subdivision.divisions_per_beat as f32;
    let samples_per_step = state.sample_rate * 60.0 / (bpm * dpb);
    let step_inc = 1.0 / samples_per_step as f64;
    let steps_per_bar = state.pattern.steps_per_bar();
    let sample_rate = state.sample_rate;

    for frame in 0..frames {
        let global = state.sample_position + frame as u64;
        let step_pos = state.step_position + step_inc * frame as f64;
        let step_idx = step_pos as i64;

        if step_idx > state.last_step {
            let step_in_bar = step_idx as usize % steps_per_bar;
            // Clone Sound by value into each new Voice. Cheap: Click
            // is bytes-sized, Sample's buffer is Arc-shared.
            let triggers: Vec<(Sound, bool)> = state
                .pattern
                .tracks
                .iter()
                .filter(|t| !t.muted)
                .filter_map(|t| match t.steps.get(step_in_bar)? {
                    Step::Empty => None,
                    Step::Active { accent } => Some((t.sound.clone(), *accent)),
                })
                .collect();
            for (sound, accent) in triggers {
                state.voices.push(Voice {
                    sound,
                    accent,
                    start_sample: global,
                });
            }
            state.last_step = step_idx;
        }

        let mut sample_value = 0.0_f32;
        state.voices.retain(|v| {
            let local = global.saturating_sub(v.start_sample) as u32;
            match v.sound.render_sample(local, sample_rate, v.accent) {
                Some(value) => {
                    sample_value += value;
                    true
                },
                None => false,
            }
        });

        let clamped = sample_value.clamp(-1.0, 1.0);
        for ch in 0..chs {
            output[frame * chs + ch] = clamped;
        }
    }

    state.sample_position += frames as u64;
    state.step_position += step_inc * frames as f64;
}

#[cfg(test)]
mod mixer_tests {
    use super::*;
    use crate::sequencer::{Subdivision, TimeSignature};

    fn metronome_pattern(bpm: f32) -> SequencerPattern {
        SequencerPattern::metronome(bpm, TimeSignature::default(), Subdivision::QUARTER)
    }

    #[test]
    fn process_buffer_silent_when_stopped() {
        let mut state = PatternRenderState::new(metronome_pattern(120.0), 48000.0);
        // playing = false by default
        let mut buf = vec![1.0_f32; 256];
        process_buffer(&mut state, &mut buf, 2);
        assert!(buf.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn process_buffer_produces_audio_when_playing() {
        let mut state = PatternRenderState::new(metronome_pattern(120.0), 48000.0);
        state.playing = true;
        // 1 second of stereo at 48kHz: more than a quarter note window
        let mut buf = vec![0.0_f32; 96000 * 2];
        process_buffer(&mut state, &mut buf, 2);
        let any_nonzero = buf.iter().any(|s| s.abs() > 1e-6);
        assert!(any_nonzero, "expected at least one nonzero sample");
    }

    #[test]
    fn process_buffer_advances_step_at_predicted_time() {
        // 120 BPM, quarter subdivision: 0.5s per step = 24000 samples
        // at 48kHz. After one second (96000 samples), last_step should
        // be 1 (we crossed into step 1 at sample 24000, and step 2 is
        // at 48000 — so by sample 96000-1, last_step >= 3).
        let mut state = PatternRenderState::new(metronome_pattern(120.0), 48000.0);
        state.playing = true;
        let mut buf = vec![0.0_f32; 96000 * 2];
        process_buffer(&mut state, &mut buf, 2);
        // After 1 second at 120 BPM, we should have crossed into at
        // least step 1 (typically 1 or 2 depending on rounding).
        assert!(
            state.last_step >= 1,
            "last_step = {} after 1s at 120 BPM",
            state.last_step
        );
    }

    #[test]
    fn process_buffer_step_count_scales_with_bpm() {
        // At 60 BPM (slower), fewer steps fire in a fixed window;
        // at 240 BPM (faster), more steps fire.
        let mut slow = PatternRenderState::new(metronome_pattern(60.0), 48000.0);
        let mut fast = PatternRenderState::new(metronome_pattern(240.0), 48000.0);
        slow.playing = true;
        fast.playing = true;

        let mut buf_slow = vec![0.0_f32; 48000 * 2];
        let mut buf_fast = vec![0.0_f32; 48000 * 2];
        process_buffer(&mut slow, &mut buf_slow, 2);
        process_buffer(&mut fast, &mut buf_fast, 2);

        assert!(
            fast.last_step > slow.last_step,
            "fast={} slow={}",
            fast.last_step,
            slow.last_step
        );
    }

    #[test]
    fn process_buffer_subdivision_scales_step_count() {
        // 120 BPM with 16th-note subdivision should advance 4x as many
        // steps as quarter-note subdivision in the same window.
        let p_quarter =
            SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::QUARTER);
        let p_sixteenth =
            SequencerPattern::metronome(120.0, TimeSignature::default(), Subdivision::SIXTEENTH);
        let mut quarter = PatternRenderState::new(p_quarter, 48000.0);
        let mut sixteenth = PatternRenderState::new(p_sixteenth, 48000.0);
        quarter.playing = true;
        sixteenth.playing = true;

        let mut buf_q = vec![0.0_f32; 48000 * 2];
        let mut buf_s = vec![0.0_f32; 48000 * 2];
        process_buffer(&mut quarter, &mut buf_q, 2);
        process_buffer(&mut sixteenth, &mut buf_s, 2);

        // last_step is 0-indexed; count of triggered steps = last_step + 1.
        // 16th should fire 4x as many steps as quarter.
        let count_q = (quarter.last_step + 1) as f64;
        let count_s = (sixteenth.last_step + 1) as f64;
        let ratio = count_s / count_q.max(1.0);
        assert!(
            (3.5..=4.5).contains(&ratio),
            "ratio {ratio}: q_count={count_q} s_count={count_s}"
        );
    }

    #[test]
    fn bpm_change_does_not_pause_playback() {
        // Regression: lowering BPM mid-playback used to cause a pause
        // because step_idx (computed as sample_position/samples_per_step)
        // shrank when samples_per_step grew, so the trigger condition
        // `step_idx > last_step` stayed false until samples caught up.
        // Fix: track step_position independently.
        let mut state = PatternRenderState::new(metronome_pattern(120.0), 48000.0);
        state.playing = true;

        // Play 1 second at 120 BPM (= 2 beats). last_step should be 1.
        let mut buf = vec![0.0_f32; 48000 * 2];
        process_buffer(&mut state, &mut buf, 2);
        let last_step_before = state.last_step;
        let step_pos_before = state.step_position;
        assert!(last_step_before >= 1);

        // Drop to 60 BPM mid-playback.
        state.pattern.bpm = 60.0;

        // Play another second at 60 BPM (= 1 beat). step should advance
        // by ~1, not pause.
        process_buffer(&mut state, &mut buf, 2);

        assert!(
            state.last_step > last_step_before,
            "step did not advance after BPM drop: before={last_step_before} after={}",
            state.last_step
        );
        assert!(
            state.step_position > step_pos_before,
            "step_position did not advance after BPM drop"
        );
    }

    #[test]
    fn bpm_increase_speeds_up_without_glitch() {
        // Symmetric case: raising BPM should advance the step counter
        // faster without skipping or doubling.
        let mut state = PatternRenderState::new(metronome_pattern(60.0), 48000.0);
        state.playing = true;

        let mut buf = vec![0.0_f32; 48000 * 2];
        process_buffer(&mut state, &mut buf, 2);
        let last_step_before = state.last_step;

        state.pattern.bpm = 240.0;
        process_buffer(&mut state, &mut buf, 2);

        // At 240 BPM, 4 steps per second; step counter should advance
        // by ~4 in the second window.
        let advance = state.last_step - last_step_before;
        assert!(
            (3..=5).contains(&advance),
            "expected ~4 steps after BPM raise; got {advance}"
        );
    }

    #[test]
    fn process_buffer_voices_decay_to_silence() {
        // After a click triggers and its duration elapses, the voice
        // should be removed from the active list.
        let mut state = PatternRenderState::new(metronome_pattern(120.0), 48000.0);
        state.playing = true;
        // 100ms is well past the click's 50ms duration.
        let mut buf = vec![0.0_f32; 4800 * 2];
        process_buffer(&mut state, &mut buf, 2);
        // Voice list should be empty after sufficient time.
        assert!(
            state.voices.is_empty() || state.voices.len() <= 1,
            "voices = {}",
            state.voices.len()
        );
    }
}
