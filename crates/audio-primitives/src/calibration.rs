//! Input-to-output round-trip latency estimation (pure DSP).
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
//! `T1 - T0` is the full round-trip the user perceives as "lag" between
//! a metronome click and the moment their hit gets registered. Any
//! honest timing-feedback or loop-record feature needs this number:
//! without it, "you're 30ms late" might be entirely system latency, and
//! loop captures land one round-trip behind the beat.
//!
//! This module is just the pure pairing math. The live driver — playing
//! the calibration clicks, polling progress, owning the engine handles
//! — is framework-specific and lives in the consuming crate (see
//! `woodshed-audio`'s `CalibrationSession`).

use std::time::{Duration, Instant};

/// Match window — onsets farther than this from an expected click are
/// treated as unrelated and dropped. 200 ms accommodates extreme
/// latency setups (Bluetooth audio routinely hits ~150 ms) without
/// matching random ambient noise.
pub const MATCH_WINDOW: Duration = Duration::from_millis(200);

/// Result of pairing expected clicks with detected onsets.
///
/// `matched_pairs` counts the same non-negative nearest pairs that can
/// contribute to `latency`. It remains available when the pair count is below
/// the requested minimum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatencyEstimate {
    pub latency: Option<Duration>,
    pub matched_pairs: usize,
}

/// Pair each click with its nearest in-window onset and calculate the median
/// from admitted (non-negative) offsets. The count and estimate are derived
/// from the same pairing pass.
pub fn estimate_latency_and_count(
    expected_clicks: &[Instant],
    detected_onsets: &[Instant],
    minimum_pairs: usize,
) -> LatencyEstimate {
    let mut deltas_ms: Vec<f32> = Vec::new();
    for &expected in expected_clicks {
        let mut best: Option<(f32, f32)> = None; // (abs_delta_ms, signed_delta_ms)
        for &onset in detected_onsets {
            let signed_delta_ms = if onset >= expected {
                onset.duration_since(expected).as_secs_f32() * 1000.0
            } else {
                -(expected.duration_since(onset).as_secs_f32() * 1000.0)
            };
            let abs = signed_delta_ms.abs();
            if abs > MATCH_WINDOW.as_secs_f32() * 1000.0 {
                continue;
            }
            match best {
                Some((a, _)) if a <= abs => {},
                _ => best = Some((abs, signed_delta_ms)),
            }
        }
        if let Some((_, signed)) = best {
            // An onset before its nearest click is the player anticipating
            // the beat, not a latency contribution.
            if signed >= 0.0 {
                deltas_ms.push(signed);
            }
        }
    }
    let matched_pairs = deltas_ms.len();
    if matched_pairs == 0 || matched_pairs < minimum_pairs {
        return LatencyEstimate {
            latency: None,
            matched_pairs,
        };
    }
    deltas_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_ms = deltas_ms[matched_pairs / 2];
    LatencyEstimate {
        latency: Some(Duration::from_secs_f32(median_ms / 1000.0)),
        matched_pairs,
    }
}

/// Given a sequence of expected click instants (in wall-clock time)
/// and a sequence of detected onset instants, pair each click with
/// the nearest onset that falls within [`MATCH_WINDOW`] and return
/// the median latency.
///
/// Returns `None` if fewer than `minimum_pairs` good matches survive
/// the window filter (the run was too noisy, or the user wasn't
/// actually playing).
pub fn estimate_latency_from_pairs(
    expected_clicks: &[Instant],
    detected_onsets: &[Instant],
    minimum_pairs: usize,
) -> Option<Duration> {
    estimate_latency_and_count(expected_clicks, detected_onsets, minimum_pairs).latency
}

/// Count the admitted non-negative nearest click-onset pairs within the match
/// window. This reports the same support used by the latency estimator.
pub fn count_matches(clicks: &[Instant], onsets: &[Instant]) -> usize {
    estimate_latency_and_count(clicks, onsets, 0).matched_pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn estimate_latency_finds_median() {
        let now = Instant::now();
        let clicks: Vec<Instant> = (0..4).map(|i| now + ms(1000 * i as u64)).collect();
        // Each onset is 50 ms after its click.
        let onsets: Vec<Instant> = clicks.iter().map(|&c| c + ms(50)).collect();
        let latency = estimate_latency_from_pairs(&clicks, &onsets, 3).unwrap();
        let ms_value = latency.as_secs_f32() * 1000.0;
        assert!((ms_value - 50.0).abs() < 5.0, "got {ms_value} ms");
    }

    #[test]
    fn estimate_latency_rejects_outliers_via_median() {
        let now = Instant::now();
        let clicks: Vec<Instant> = (0..5).map(|i| now + ms(1000 * i as u64)).collect();
        // Four clean 40ms onsets + one wildly late 150ms onset.
        let mut onsets: Vec<Instant> = clicks.iter().take(4).map(|&c| c + ms(40)).collect();
        onsets.push(clicks[4] + ms(150));
        let latency = estimate_latency_from_pairs(&clicks, &onsets, 3).unwrap();
        let ms_value = latency.as_secs_f32() * 1000.0;
        // Median of [40,40,40,40,150] is 40 — outlier doesn't move it.
        assert!((ms_value - 40.0).abs() < 5.0, "got {ms_value} ms");
    }

    #[test]
    fn estimate_latency_drops_onsets_outside_match_window() {
        let now = Instant::now();
        let clicks: Vec<Instant> = (0..4).map(|i| now + ms(1000 * i as u64)).collect();
        // Onsets 500 ms late — way past MATCH_WINDOW. Should yield
        // None because no pairs survive.
        let onsets: Vec<Instant> = clicks.iter().map(|&c| c + ms(500)).collect();
        assert!(estimate_latency_from_pairs(&clicks, &onsets, 2).is_none());
    }

    #[test]
    fn estimate_latency_returns_none_below_minimum_pairs() {
        let now = Instant::now();
        let clicks: Vec<Instant> = (0..6).map(|i| now + ms(1000 * i as u64)).collect();
        // Only one matching onset — should fail at minimum_pairs = 3.
        let onsets = vec![clicks[2] + ms(30)];
        assert!(estimate_latency_from_pairs(&clicks, &onsets, 3).is_none());
    }

    #[test]
    fn estimate_latency_filters_negative_offsets() {
        // User played 20 ms early on every click. We should reject
        // those (they're "user ahead of beat," not system latency).
        let now = Instant::now();
        let clicks: Vec<Instant> = (0..4).map(|i| now + ms(1000 * i as u64)).collect();
        let onsets: Vec<Instant> = clicks.iter().map(|&c| c - ms(20)).collect();
        // No positive deltas survive → minimum_pairs not met → None.
        assert!(estimate_latency_from_pairs(&clicks, &onsets, 2).is_none());
    }

    #[test]
    fn count_matches_includes_only_within_window() {
        let now = Instant::now();
        let clicks: Vec<Instant> = (0..4).map(|i| now + ms(1000 * i as u64)).collect();
        let onsets = vec![
            clicks[0] + ms(40), // in window
            clicks[1] + ms(60), // in window
            clicks[2] + ms(500), // out of window
                                // clicks[3] has no onset at all
        ];
        assert_eq!(count_matches(&clicks, &onsets), 2);
    }

    #[test]
    fn nearest_negative_pair_is_not_replaced_by_a_later_positive_pair() {
        let click = Instant::now();
        let onsets = [click - ms(20), click + ms(30)];
        let result = estimate_latency_and_count(&[click], &onsets, 0);
        assert_eq!(result.matched_pairs, 0);
        assert_eq!(result.latency, None);
        assert_eq!(count_matches(&[click], &onsets), 0);
    }

    #[test]
    fn zero_offset_and_match_window_boundary_are_admitted() {
        let now = Instant::now();
        let clicks = [now, now + ms(1000)];
        let onsets = [clicks[0], clicks[1] + MATCH_WINDOW];
        let result = estimate_latency_and_count(&clicks, &onsets, 2);
        assert_eq!(result.matched_pairs, 2);
        // Even-sized support keeps the existing upper-median behavior.
        let latency_ms = result.latency.unwrap().as_secs_f32() * 1000.0;
        assert!((latency_ms - MATCH_WINDOW.as_secs_f32() * 1000.0).abs() < 0.01);
    }

    #[test]
    fn pairing_result_reports_support_when_insufficient_or_empty() {
        let click = Instant::now();
        let one_pair = estimate_latency_and_count(&[click, click + ms(1000)], &[click], 2);
        assert_eq!(
            one_pair,
            LatencyEstimate {
                latency: None,
                matched_pairs: 1
            }
        );
        let no_pairs = estimate_latency_and_count(&[click], &[click + ms(500)], 1);
        assert_eq!(
            no_pairs,
            LatencyEstimate {
                latency: None,
                matched_pairs: 0
            }
        );
    }
}
