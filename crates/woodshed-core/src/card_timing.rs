//! Pure admission of authored Card time, shared by dwell and ordered previews.
//! Device-rate PCM budgets remain the audio host's responsibility.
use std::time::Duration;
use woodshedding::rehearsal::{Hold, Timing};

pub type CardPreview = (Vec<f32>, f32, f32);
pub type CardPreviewResult = Result<CardPreview, CardTimingError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTimingError {
    InvalidTempo,
    InvalidSeconds,
    UnrepresentableDuration,
}
impl std::fmt::Display for CardTimingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidTempo => "Card tempo must be finite and positive",
            Self::InvalidSeconds => "Card seconds must be finite and positive",
            Self::UnrepresentableDuration => "Card duration cannot be represented by this clock",
        })
    }
}
impl std::error::Error for CardTimingError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyTimingMinimum {
    HalfSecond,
    OneBar,
    OneRep,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedCardTiming {
    pub effective_bpm: f32,
    /// None is an admitted Manual instruction, never a failed conversion.
    pub dwell: Option<Duration>,
    pub legacy_minimum: Option<LegacyTimingMinimum>,
}
fn duration(seconds: f32) -> Result<Duration, CardTimingError> {
    let value = Duration::try_from_secs_f32(seconds)
        .map_err(|_| CardTimingError::UnrepresentableDuration)?;
    if value.is_zero() {
        return Err(CardTimingError::UnrepresentableDuration);
    }
    Ok(value)
}
pub fn resolve_card_timing(
    timing: &Timing,
    fallback_bpm: f32,
) -> Result<ResolvedCardTiming, CardTimingError> {
    let bpm = timing.bpm.unwrap_or(fallback_bpm);
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(CardTimingError::InvalidTempo);
    }
    let (seconds, legacy_minimum) = match timing.hold {
        Hold::Manual => (None, None),
        Hold::Bars(n) => (
            Some(240.0 / bpm * f32::from(n.max(1))),
            (n == 0).then_some(LegacyTimingMinimum::OneBar),
        ),
        Hold::Reps(n) => (
            Some(240.0 / bpm * f32::from(n.max(1))),
            (n == 0).then_some(LegacyTimingMinimum::OneRep),
        ),
        Hold::Seconds(s) => {
            if !s.is_finite() || s <= 0.0 {
                return Err(CardTimingError::InvalidSeconds);
            }
            (
                Some(s.max(0.5)),
                (s < 0.5).then_some(LegacyTimingMinimum::HalfSecond),
            )
        },
    };
    Ok(ResolvedCardTiming {
        effective_bpm: bpm,
        dwell: seconds.map(duration).transpose()?,
        legacy_minimum,
    })
}
impl ResolvedCardTiming {
    /// One complete ordered traversal, retaining its note order/repetitions.
    pub fn ordered_preview(self, visits: usize) -> Result<(f32, f32), CardTimingError> {
        if visits == 0 {
            return Ok((0.0, 0.0));
        }
        let step = self.dwell.map_or(60.0 / self.effective_bpm, |d| {
            d.as_secs_f32() / visits as f32
        });
        let seconds = self.dwell.map_or_else(
            || (visits.saturating_sub(1) as f32 * step) + 0.4,
            |d| d.as_secs_f32(),
        );
        duration(seconds)?;
        let strum_ms = step * 1000.0;
        if !strum_ms.is_finite() {
            return Err(CardTimingError::UnrepresentableDuration);
        }
        Ok((seconds, strum_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_clock_and_fallback_are_distinct_and_never_rewritten() {
        let mut timing = Timing {
            bpm: None,
            hold: Hold::Bars(1),
        };
        for bpm in [20.0, 400.0] {
            let inherited = resolve_card_timing(&timing, bpm).unwrap();
            timing.bpm = Some(bpm);
            let before = serde_json::to_value(&timing).unwrap();
            let authored = resolve_card_timing(&timing, f32::NAN).unwrap();
            assert_eq!(authored, inherited);
            assert!((authored.dwell.unwrap().as_secs_f32() - 240.0 / bpm).abs() < 0.00001);
            let (seconds, strum) = authored.ordered_preview(4).unwrap();
            assert!((seconds - 240.0 / bpm).abs() < 0.00001);
            assert!((strum - seconds * 250.0).abs() < 0.00001);
            assert_eq!(serde_json::to_value(&timing).unwrap(), before);
            timing.bpm = None;
        }
    }
    #[test]
    fn manual_and_invalid_or_unrepresentable_are_distinct() {
        let mut timing = Timing {
            bpm: Some(120.0),
            hold: Hold::Manual,
        };
        assert_eq!(resolve_card_timing(&timing, 0.0).unwrap().dwell, None);
        for bpm in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            timing.bpm = Some(bpm);
            assert_eq!(
                resolve_card_timing(&timing, 120.0),
                Err(CardTimingError::InvalidTempo)
            );
        }
        timing.bpm = None;
        for fallback in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                resolve_card_timing(&timing, fallback),
                Err(CardTimingError::InvalidTempo)
            );
        }
        timing.hold = Hold::Bars(1);
        for bpm in [f32::MAX, f32::MIN_POSITIVE] {
            timing.bpm = Some(bpm);
            assert_eq!(
                resolve_card_timing(&timing, 120.0),
                Err(CardTimingError::UnrepresentableDuration)
            );
        }
        timing.bpm = Some(120.0);
        for s in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            timing.hold = Hold::Seconds(s);
            assert_eq!(
                resolve_card_timing(&timing, 120.0),
                Err(CardTimingError::InvalidSeconds)
            );
        }
        timing.hold = Hold::Seconds(1e30);
        assert_eq!(
            resolve_card_timing(&timing, 120.0),
            Err(CardTimingError::UnrepresentableDuration)
        );
    }
    #[test]
    fn legacy_minima_are_explicit_and_preserve_authored_values() {
        for (hold, minimum, seconds) in [
            (Hold::Seconds(0.1), LegacyTimingMinimum::HalfSecond, 0.5),
            (Hold::Bars(0), LegacyTimingMinimum::OneBar, 2.0),
            (Hold::Reps(0), LegacyTimingMinimum::OneRep, 2.0),
        ] {
            let timing = Timing { bpm: None, hold };
            let before = serde_json::to_value(&timing).unwrap();
            let resolved = resolve_card_timing(&timing, 120.0).unwrap();
            assert_eq!(resolved.legacy_minimum, Some(minimum));
            assert_eq!(resolved.dwell.unwrap().as_secs_f32(), seconds);
            assert_eq!(serde_json::to_value(&timing).unwrap(), before);
        }
    }
}
