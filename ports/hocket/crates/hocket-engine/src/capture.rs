// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Phrase capture state machine.
//!
//! `Capture` is fed input samples one at a time (or in batches via
//! `feed_slice`). When armed, it accumulates samples into a buffer
//! sized for one phrase (`bars_per_phrase × samples_per_bar`). On
//! completion, the buffer is drainable via `take_completed`.
//!
//! ## Feature Target 3a scope
//!
//! For FT3a, "arm" means "start recording with the very next sample."
//! Bar-aligned arming (start at the next bar boundary, after a
//! count-in) arrives in FT3b along with the cpal input stream and the
//! bar-phase synchronization between input and output streams.
//!
//! ## Feature Target 3a non-scope
//!
//! - No cpal input integration. Tests feed synthesized samples
//!   directly.
//! - No latency compensation. FT3c will integrate
//!   `woodshed_audio::calibration` and offset the buffer-write
//!   position by the measured input-output round-trip.
//! - No playback. FT3 only stores the captured buffer and commits the
//!   `Edit::CapturePhrase`. Playback of captured phrases is a
//!   follow-on target.

/// State of an in-progress capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureState {
    /// Not capturing. `feed` is a no-op.
    Idle,
    /// Recording in progress.
    Recording {
        samples_done: usize,
        samples_total: usize,
    },
    /// Capture finished; buffer ready to be drained via
    /// [`Capture::take_completed`]. After draining, state returns to
    /// `Idle`.
    Complete,
}

/// Capture state machine.
///
/// Construct with [`Capture::new`] specifying how many samples make up
/// one full phrase (`samples_per_phrase = bar_samples × bars_per_phrase`).
/// Call [`Capture::arm`] to start, then feed input samples via
/// [`Capture::feed`] or [`Capture::feed_slice`]. Poll [`Capture::state`]
/// or react to [`Capture::take_completed`] returning `Some`.
#[derive(Clone, Debug)]
pub struct Capture {
    state: CaptureState,
    buffer: Vec<f32>,
    samples_per_phrase: usize,
}

impl Capture {
    pub fn new(samples_per_phrase: usize) -> Self {
        Self {
            state: CaptureState::Idle,
            buffer: Vec::with_capacity(samples_per_phrase),
            samples_per_phrase,
        }
    }

    /// Length (in samples) of one phrase at this Capture's configuration.
    pub fn samples_per_phrase(&self) -> usize {
        self.samples_per_phrase
    }

    pub fn state(&self) -> &CaptureState {
        &self.state
    }

    /// Begin recording. Clears any prior buffer and moves to
    /// `Recording`. If already `Recording` or `Complete`, this resets
    /// to a fresh `Recording`.
    pub fn arm(&mut self) {
        self.buffer.clear();
        self.state = CaptureState::Recording {
            samples_done: 0,
            samples_total: self.samples_per_phrase,
        };
    }

    /// Abort an in-progress capture without producing a phrase.
    /// Resets to `Idle`.
    pub fn cancel(&mut self) {
        self.buffer.clear();
        self.state = CaptureState::Idle;
    }

    /// Feed one input sample. Advances the state machine. If this was
    /// the last sample of the phrase, state transitions to `Complete`.
    /// No-op if not currently `Recording`.
    pub fn feed(&mut self, input: f32) {
        let (done, total) = match self.state {
            CaptureState::Recording {
                samples_done,
                samples_total,
            } => (samples_done, samples_total),
            _ => return,
        };
        self.buffer.push(input);
        let new_done = done + 1;
        if new_done >= total {
            self.state = CaptureState::Complete;
        } else {
            self.state = CaptureState::Recording {
                samples_done: new_done,
                samples_total: total,
            };
        }
    }

    /// Feed many samples in a batch. Equivalent to calling `feed` for
    /// each, but cheaper. Stops feeding (with the remainder ignored)
    /// once the capture completes, so callers can pass an entire input
    /// buffer without worrying about overrun.
    pub fn feed_slice(&mut self, samples: &[f32]) {
        for &s in samples {
            if matches!(self.state, CaptureState::Recording { .. }) {
                self.feed(s);
            } else {
                return;
            }
        }
    }

    /// If state is `Complete`, drain the captured buffer and reset to
    /// `Idle`. Returns the captured samples in order. Returns `None`
    /// if the capture isn't complete.
    pub fn take_completed(&mut self) -> Option<Vec<f32>> {
        if !matches!(self.state, CaptureState::Complete) {
            return None;
        }
        let buf = std::mem::take(&mut self.buffer);
        self.state = CaptureState::Idle;
        Some(buf)
    }

    /// Progress as a fraction in `[0.0, 1.0]`. `0.0` if not recording.
    pub fn progress(&self) -> f32 {
        match self.state {
            CaptureState::Recording {
                samples_done,
                samples_total,
            } => {
                if samples_total == 0 {
                    1.0
                } else {
                    samples_done as f32 / samples_total as f32
                }
            }
            CaptureState::Complete => 1.0,
            CaptureState::Idle => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_capture_is_idle() {
        let c = Capture::new(100);
        assert_eq!(c.state(), &CaptureState::Idle);
        assert_eq!(c.progress(), 0.0);
    }

    #[test]
    fn feed_in_idle_is_noop() {
        let mut c = Capture::new(10);
        c.feed(0.5);
        assert_eq!(c.state(), &CaptureState::Idle);
        assert!(c.take_completed().is_none());
    }

    #[test]
    fn arm_then_feed_to_completion() {
        let mut c = Capture::new(4);
        c.arm();
        assert!(matches!(
            c.state(),
            CaptureState::Recording {
                samples_done: 0,
                ..
            }
        ));
        for v in [0.1, 0.2, 0.3, 0.4] {
            c.feed(v);
        }
        assert_eq!(c.state(), &CaptureState::Complete);
        let buf = c.take_completed().unwrap();
        assert_eq!(buf, vec![0.1, 0.2, 0.3, 0.4]);
        // After draining, back to Idle.
        assert_eq!(c.state(), &CaptureState::Idle);
    }

    #[test]
    fn feed_slice_batches_correctly() {
        let mut c = Capture::new(5);
        c.arm();
        c.feed_slice(&[0.1, 0.2, 0.3]);
        assert!(matches!(
            c.state(),
            CaptureState::Recording {
                samples_done: 3,
                samples_total: 5
            }
        ));
        c.feed_slice(&[0.4, 0.5, 0.6, 0.7]); // 0.6/0.7 should be ignored
        assert_eq!(c.state(), &CaptureState::Complete);
        let buf = c.take_completed().unwrap();
        assert_eq!(buf, vec![0.1, 0.2, 0.3, 0.4, 0.5]);
    }

    #[test]
    fn cancel_returns_to_idle() {
        let mut c = Capture::new(10);
        c.arm();
        c.feed_slice(&[0.1, 0.2, 0.3]);
        c.cancel();
        assert_eq!(c.state(), &CaptureState::Idle);
        assert!(c.take_completed().is_none());
    }

    #[test]
    fn rearm_clears_previous_buffer() {
        let mut c = Capture::new(3);
        c.arm();
        c.feed_slice(&[0.1, 0.2]);
        c.arm(); // re-arm mid-recording
        c.feed_slice(&[0.5, 0.6, 0.7]);
        assert_eq!(c.state(), &CaptureState::Complete);
        let buf = c.take_completed().unwrap();
        assert_eq!(buf, vec![0.5, 0.6, 0.7]);
    }

    #[test]
    fn progress_tracks_proportion() {
        let mut c = Capture::new(4);
        c.arm();
        assert_eq!(c.progress(), 0.0);
        c.feed(0.1);
        assert!((c.progress() - 0.25).abs() < 1e-6);
        c.feed(0.2);
        assert!((c.progress() - 0.5).abs() < 1e-6);
        c.feed_slice(&[0.3, 0.4]);
        assert_eq!(c.progress(), 1.0);
    }
}
