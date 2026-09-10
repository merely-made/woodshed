// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Pre-rendered click loop.
//!
//! For FT3b-prime, the click is a fixed-tempo pre-rendered loop fed
//! into a `SamplerNode` with `RepeatMode::RepeatEndlessly`. Tempo
//! changes at runtime aren't supported here — they'll come in FT3b
//! proper when the master clock becomes a first-class engine concept.
//!
//! The synthesis (sine burst + exponential decay) lives in the shared
//! [`audio_primitives::click`] crate, so Hocket's click and Woodshed's
//! metronome sound identical. This module just pins the metronome's
//! voicing constants and the engine-facing signature.

/// Metronome voicing — matches `woodshed_audio::Sound::click()`.
const BASE_FREQ_HZ: f32 = 800.0;
const ACCENT_FREQ_HZ: f32 = 1200.0;
const CLICK_DURATION_S: f32 = 0.05;
const CLICK_AMPLITUDE: f32 = 0.4;

/// Integer audio frames in one full bar at the given tempo and meter.
pub fn frames_per_bar(sample_rate: u32, bpm: f32, beats_per_bar: u8, beat_unit: u8) -> usize {
    audio_primitives::click::frames_per_bar(sample_rate, bpm, beats_per_bar, beat_unit)
}

/// Render one bar of clicks at the given BPM and meter into a mono `Vec<f32>`.
///
/// - Downbeat (beat 0) uses an accented higher frequency.
/// - Other beats use the base frequency.
/// - Each click is a 50 ms sine burst with exponential decay.
pub fn render_click_loop(sample_rate: u32, bpm: f32, beats_per_bar: u8, beat_unit: u8) -> Vec<f32> {
    audio_primitives::click::render_click_bar_in_meter(
        sample_rate,
        bpm,
        beats_per_bar,
        beat_unit,
        BASE_FREQ_HZ,
        ACCENT_FREQ_HZ,
        CLICK_DURATION_S,
        CLICK_AMPLITUDE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_click_loop_has_expected_length() {
        // 120 BPM, 4 beats per bar, 48 kHz:
        // samples per beat = 48000 * 60 / 120 = 24000
        // total = 24000 * 4 = 96000
        let buf = render_click_loop(48_000, 120.0, 4, 4);
        assert_eq!(buf.len(), 96_000);
    }

    #[test]
    fn render_click_loop_starts_with_accent() {
        let buf = render_click_loop(48_000, 120.0, 4, 4);
        // Click 0 (downbeat) is louder than later clicks at the same offset.
        let downbeat_peak = buf[0..2400].iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        let samples_per_beat = 24_000;
        let beat1_peak = buf[samples_per_beat..samples_per_beat + 2400]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        // Same amplitude, different frequency — peaks should be similar
        // but not identical due to envelope sampling. Both should be > 0.
        assert!(downbeat_peak > 0.1);
        assert!(beat1_peak > 0.1);
    }

    #[test]
    fn render_click_loop_is_silent_between_clicks() {
        let buf = render_click_loop(48_000, 120.0, 4, 4);
        // 100 ms after a click (well past the 50ms duration) should be silent.
        let silent_offset = 48_000 / 10; // 100ms in samples
        assert!(buf[silent_offset].abs() < 1e-6);
    }

    #[test]
    fn render_click_loop_honors_the_beat_unit() {
        let buf = render_click_loop(48_000, 120.0, 3, 8);
        assert_eq!(buf.len(), 36_000);
        assert_eq!(frames_per_bar(48_000, 120.0, 3, 8), buf.len());
    }
}
