//! Input-to-output round-trip latency calibration.
//!
//! What it measures:
//!
//! ```text
//!     ┌──────────┐  output  ┌────────┐         ┌─────┐  input  ┌────────┐
//!     │ App emits│  latency │Speaker │ travel  │ Mic │ latency │ Onset  │
//!     │  click   │ ───────► │        │ ──────► │     │ ──────► │detector│
//!     │ at t=T0  │          │        │  (~ms)  │     │         │  T1    │
//!     └──────────┘          └────────┘         └─────┘         └────────┘
//! ```
//!
//! The intended measurement is `T1 - T0`, the round-trip perceived as "lag" between
//! a metronome click and the moment their hit gets registered. Both
//! the onset-timing-feedback widget and the loop recorder need this
//! number to display anything honest:
//!
//! - Without it, "you're 30ms late" might be entirely the system
//!   latency, not the player.
//! - For loop record, samples captured "in time" actually land in
//!   the buffer one round-trip late; without compensation the loop
//!   sounds noticeably behind on playback.
//!
//! The live session preserves the existing approximate control-time click
//! anchor. Command dispatch, output callback scheduling, input callback
//! timestamping, and physical propagation all contribute to the result;
//! this is not a committed sample-clock calibration.
//!
//! # API shape
//!
//! Two layers:
//!
//! - [`estimate_latency_from_pairs`] — pure DSP. Given a list of
//!   expected click instants and detected onset instants, returns the
//!   median latency. Trivially testable, no audio dependency.
//! - [`CalibrationSession`] — drives a live calibration run. The UI
//!   passes its click controls, calls [`CalibrationSession::start`]
//!   when the user clicks "Calibrate," then polls [`Self::poll`]
//!   each tick. When [`Self::poll`] returns a terminal
//!   [`CalibrationOutcome`], the UI shows the result and lets the
//!   user accept / discard.
//!
//! The session does *not* own the audio engines — the UI passes in
//! the relevant handles so calibration composes with whatever
//! transport the user is otherwise running.

use std::time::{Duration, Instant};

use crate::onset::OnsetHandle;
use crate::sequencer::{SequencerPattern, Subdivision, TimeSignature};

// Pure pairing / median-latency DSP (and the `MATCH_WINDOW` it keys
// off) now lives in `audio_primitives::calibration`, shared with
// Strophe. Re-exported so existing
// `woodshed_audio::{estimate_latency_from_pairs, MATCH_WINDOW}` paths
// resolve unchanged. The live session driver below builds on it.
pub use audio_primitives::calibration::{
    MATCH_WINDOW, count_matches, estimate_latency_and_count, estimate_latency_from_pairs,
};

/// BPM the calibration metronome runs at. 60 BPM = one click per
/// second — slow enough to leave plenty of "guard space" between
/// clicks so a detected onset is unambiguously the click that
/// preceded it, not one further back.
pub const CALIBRATION_BPM: f32 = 60.0;

/// How many clicks to play during a calibration run. 6 is enough to
/// get a stable median while keeping the experience under 10 seconds.
pub const CALIBRATION_CLICKS: usize = 6;

// ============================================================
// Session driver
// ============================================================

/// Click controls supplied by the application's one output runtime.
///
/// This is a control-thread seam; the session owns neither a stream nor a
/// concrete engine. Expected instants retain the existing approximate
/// control-time anchor, not a claim about physical callback/device timing.
pub trait ClickControl {
    type Error;
    fn set_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), Self::Error>;
    fn play_click(&mut self) -> Result<(), Self::Error>;
    fn stop_click(&mut self) -> Result<(), Self::Error>;

    /// Admit pattern and playback together, preserving prior output when
    /// the operation is rejected. Bounded runtimes use one atomic command.
    fn start_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), Self::Error>;
}

/// Phase the calibration session is in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CalibrationPhase {
    /// Hasn't started yet. UI shows the "Calibrate" button.
    Idle,
    /// Playing clicks and listening. UI shows progress.
    Running,
    /// Run finished. UI shows the result (success or failure) and
    /// offers Accept / Retry.
    Finished,
}

/// Terminal outcome from [`CalibrationSession::poll`].
#[derive(Clone, Debug)]
pub enum CalibrationOutcome {
    /// Still running; nothing to display yet beyond a progress meter.
    InProgress {
        /// How many of the expected clicks have fired by now.
        clicks_fired: usize,
        /// Total expected clicks for this run.
        total_clicks: usize,
    },
    /// Calibration succeeded. Apply this latency to the app's settings.
    Success {
        latency: Duration,
        matched_pairs: usize,
        total_clicks: usize,
    },
    /// Not enough clicks matched — the user probably didn't play
    /// along, or the threshold was too high. The UI should offer to
    /// retry.
    InsufficientPairs {
        matched_pairs: usize,
        total_clicks: usize,
    },
    /// The audio engines aren't available (e.g. mic permission
    /// denied). UI should surface this and not try again.
    EngineUnavailable,
}

fn outcome_from_estimate(
    estimate: audio_primitives::calibration::LatencyEstimate,
    total_clicks: usize,
) -> CalibrationOutcome {
    match estimate.latency {
        Some(latency) => CalibrationOutcome::Success {
            latency,
            matched_pairs: estimate.matched_pairs,
            total_clicks,
        },
        None => CalibrationOutcome::InsufficientPairs {
            matched_pairs: estimate.matched_pairs,
            total_clicks,
        },
    }
}

/// Stateful driver for one calibration run.
///
/// Lifecycle:
/// ```text
///   Idle → start() → Running → poll() repeatedly → Finished
/// ```
///
/// The session doesn't take ownership of the engines — UIs pass them
/// in by reference at call sites. This keeps the session compatible
/// with any UI framework and avoids fighting with whatever transport
/// the user has running.
pub struct CalibrationSession {
    phase: CalibrationPhase,
    /// Control-thread wall-clock time immediately after requesting playback.
    start_at: Option<Instant>,
    /// Pre-computed expected instants for each click in the run.
    expected_clicks: Vec<Instant>,
    /// Configurable: how many clicks per run.
    total_clicks: usize,
    /// Configurable: BPM the calibration plays at.
    bpm: f32,
}

impl Default for CalibrationSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CalibrationSession {
    pub fn new() -> Self {
        Self {
            phase: CalibrationPhase::Idle,
            start_at: None,
            expected_clicks: Vec::new(),
            total_clicks: CALIBRATION_CLICKS,
            bpm: CALIBRATION_BPM,
        }
    }

    /// Override the click count for the run. Useful for tests; the
    /// shipping UI sticks to the constants.
    pub fn with_total_clicks(mut self, n: usize) -> Self {
        self.total_clicks = n.max(2);
        self
    }

    /// Override the calibration BPM.
    pub fn with_bpm(mut self, bpm: f32) -> Self {
        self.bpm = bpm.clamp(40.0, 240.0);
        self
    }

    pub fn phase(&self) -> CalibrationPhase {
        self.phase
    }

    /// Begin a calibration run. Resets the onset history (so prior
    /// playing doesn't pollute pairing), installs a metronome pattern
    /// on the output engine, and starts playback.
    ///
    /// The caller is responsible for ensuring the onset analyzer is
    /// enabled. The session enables it as a courtesy but the UI
    /// should leave it enabled until the run finishes.
    /// Rejected output commands return the control's error and leave no
    /// running session or expected timing anchor.
    pub fn start<C: ClickControl>(
        &mut self,
        engine: &mut C,
        onset: &OnsetHandle,
    ) -> Result<(), C::Error> {
        self.phase = CalibrationPhase::Idle;
        self.start_at = None;
        self.expected_clicks.clear();

        let pattern =
            SequencerPattern::metronome(self.bpm, TimeSignature::new(4, 4), Subdivision::QUARTER);
        engine.start_click_pattern(pattern)?;
        onset.reset();
        onset.set_enabled(true);

        let now = Instant::now();
        let secs_per_click = 60.0 / self.bpm;
        // Preserve the existing control-time latency policy. Dispatch and
        // callback scheduling contribute to this approximation; it is not
        // a committed output-sample anchor or physical clock measurement.
        self.expected_clicks = (0..self.total_clicks)
            .map(|i| now + Duration::from_secs_f32(secs_per_click * i as f32))
            .collect();
        self.start_at = Some(now);
        self.phase = CalibrationPhase::Running;
        Ok(())
    }

    /// Cancel a running session without producing a result.
    pub fn cancel<C: ClickControl>(&mut self, engine: &mut C) -> Result<(), C::Error> {
        let stopped = engine.stop_click();
        self.phase = CalibrationPhase::Idle;
        self.start_at = None;
        self.expected_clicks.clear();
        stopped
    }

    /// Check progress. Call from a UI tick (~10-50 Hz is plenty —
    /// the click cadence is 1 Hz).
    ///
    /// Returns:
    /// - [`CalibrationOutcome::InProgress`] while clicks are still
    ///   firing. Use the included counts to draw a progress bar.
    /// - [`CalibrationOutcome::Success`] or
    ///   [`CalibrationOutcome::InsufficientPairs`] when the run is
    ///   over. The session transitions to [`CalibrationPhase::Finished`].
    /// - An error if the click cannot stop; this never reports success.
    pub fn poll<C: ClickControl>(
        &mut self,
        engine: &mut C,
        onset: &OnsetHandle,
    ) -> Result<CalibrationOutcome, C::Error> {
        self.poll_at(engine, onset, Instant::now())
    }

    fn poll_at<C: ClickControl>(
        &mut self,
        engine: &mut C,
        onset: &OnsetHandle,
        now: Instant,
    ) -> Result<CalibrationOutcome, C::Error> {
        if self.start_at.is_none() {
            return Ok(CalibrationOutcome::EngineUnavailable);
        }

        let clicks_fired = self.expected_clicks.iter().filter(|&&t| t <= now).count();

        // Give a settle window equal to the match window after the
        // last expected click so a late onset on the final click
        // still gets paired.
        let final_click = *self
            .expected_clicks
            .last()
            .expect("non-empty after start()");
        let done = now >= final_click + MATCH_WINDOW;

        if !done {
            return Ok(CalibrationOutcome::InProgress {
                clicks_fired,
                total_clicks: self.total_clicks,
            });
        }

        // Run is complete — stop the click and tally.
        self.phase = CalibrationPhase::Finished;
        if let Err(error) = engine.stop_click() {
            self.start_at = None;
            return Err(error);
        }

        let snapshot = onset.snapshot();
        let onset_instants: Vec<Instant> = snapshot.recent.iter().map(|o| o.at).collect();
        // Require at least half the clicks to have matched.
        let minimum = self.total_clicks / 2;
        let estimate = estimate_latency_and_count(&self.expected_clicks, &onset_instants, minimum);
        Ok(outcome_from_estimate(estimate, self.total_clicks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure pairing/median tests (estimate_latency_from_pairs,
    // count_matches) moved with the code to audio-primitives. What
    // remains here covers the session driver's own state machine.

    #[derive(Default)]
    struct FakeClick {
        pattern: Option<SequencerPattern>,
        plays: usize,
        stops: usize,
        fail_pattern: bool,
        fail_play: bool,
        fail_stop: bool,
    }

    impl ClickControl for FakeClick {
        type Error = &'static str;
        fn set_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), Self::Error> {
            if self.fail_pattern {
                return Err("pattern rejected");
            }
            self.pattern = Some(pattern);
            Ok(())
        }
        fn play_click(&mut self) -> Result<(), Self::Error> {
            if self.fail_play {
                return Err("play rejected");
            }
            self.plays += 1;
            Ok(())
        }
        fn stop_click(&mut self) -> Result<(), Self::Error> {
            if self.fail_stop {
                return Err("stop rejected");
            }
            self.stops += 1;
            Ok(())
        }
        fn start_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), Self::Error> {
            if self.fail_pattern {
                return Err("pattern rejected");
            }
            if self.fail_play {
                return Err("play rejected");
            }
            self.set_click_pattern(pattern)?;
            self.play_click()
        }
    }

    #[test]
    fn fake_click_drives_start_completion_and_retry_without_engine() {
        let mut click = FakeClick::default();
        let onset = crate::onset::OnsetAnalyzer::new().handle();
        let mut session = CalibrationSession::new()
            .with_total_clicks(2)
            .with_bpm(120.0);
        assert!(matches!(
            session.poll(&mut click, &onset).unwrap(),
            CalibrationOutcome::EngineUnavailable
        ));
        session.start(&mut click, &onset).unwrap();
        assert_eq!(session.phase(), CalibrationPhase::Running);
        assert!(onset.is_enabled());
        assert_eq!(click.plays, 1);
        let pattern = click.pattern.as_ref().unwrap();
        assert_eq!(pattern.bpm, 120.0);
        assert_eq!(pattern.time_signature, TimeSignature::new(4, 4));
        assert_eq!(pattern.subdivision, Subdivision::QUARTER);
        let start = session.start_at.unwrap();
        assert_eq!(
            session.expected_clicks,
            [start, start + Duration::from_millis(500)]
        );
        assert!(matches!(
            session.poll_at(&mut click, &onset, start).unwrap(),
            CalibrationOutcome::InProgress {
                clicks_fired: 1,
                total_clicks: 2
            }
        ));
        let finish = *session.expected_clicks.last().unwrap() + MATCH_WINDOW;
        assert!(matches!(
            session.poll_at(&mut click, &onset, finish).unwrap(),
            CalibrationOutcome::InsufficientPairs {
                matched_pairs: 0,
                total_clicks: 2
            }
        ));
        assert_eq!(click.stops, 1);
        assert_eq!(session.phase(), CalibrationPhase::Finished);
        session.start(&mut click, &onset).unwrap();
        assert_eq!(click.plays, 2);
        assert_eq!(session.phase(), CalibrationPhase::Running);
        session.cancel(&mut click).unwrap();
        assert_eq!(click.stops, 2);
        assert_eq!(session.phase(), CalibrationPhase::Idle);
        assert!(session.expected_clicks.is_empty());
        assert!(session.start_at.is_none());
    }

    #[test]
    fn rejected_click_commands_cannot_start_or_complete_calibration() {
        let onset = crate::onset::OnsetAnalyzer::new().handle();
        let mut session = CalibrationSession::new().with_total_clicks(2);
        let mut click = FakeClick {
            fail_pattern: true,
            ..FakeClick::default()
        };
        assert_eq!(session.start(&mut click, &onset), Err("pattern rejected"));
        assert_eq!(session.phase(), CalibrationPhase::Idle);
        assert!(!onset.is_enabled());
        assert!(session.start_at.is_none());
        click.fail_pattern = false;
        click.fail_play = true;
        assert_eq!(session.start(&mut click, &onset), Err("play rejected"));
        assert_eq!(session.phase(), CalibrationPhase::Idle);
        assert!(!onset.is_enabled());
        assert!(session.expected_clicks.is_empty());
        assert!(click.pattern.is_none());
        assert_eq!(click.plays, 0);
        click.fail_play = false;
        session.start(&mut click, &onset).unwrap();
        click.fail_stop = true;
        let finish = *session.expected_clicks.last().unwrap() + MATCH_WINDOW;
        assert!(matches!(
            session.poll_at(&mut click, &onset, finish),
            Err("stop rejected")
        ));
        assert_eq!(session.phase(), CalibrationPhase::Finished);
        assert!(session.start_at.is_none());
        assert_eq!(session.cancel(&mut click), Err("stop rejected"));
        assert_eq!(session.phase(), CalibrationPhase::Idle);
    }

    #[test]
    fn session_starts_idle() {
        let s = CalibrationSession::new();
        assert_eq!(s.phase(), CalibrationPhase::Idle);
    }

    #[test]
    fn session_with_total_clicks_clamps_minimum() {
        let s = CalibrationSession::new().with_total_clicks(0);
        assert!(s.total_clicks >= 2);
    }

    #[test]
    fn session_outcome_uses_pairing_result_support_for_display() {
        let click = Instant::now();
        let estimate = audio_primitives::calibration::estimate_latency_and_count(
            &[click, click + Duration::from_secs(1)],
            &[click - Duration::from_millis(20)],
            2,
        );
        assert!(matches!(
            outcome_from_estimate(estimate, 2),
            CalibrationOutcome::InsufficientPairs {
                matched_pairs: 0,
                total_clicks: 2
            }
        ));
    }
}
