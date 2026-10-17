//! Shared, device-free conversion of explicit event coordinates to frames.

/// Relative event position. Beats use the caller's already-rounded beat clock;
/// seconds use the output sample rate and never depend on tempo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventTime {
    Beats(f64),
    Seconds(f64),
}

fn rounded_frames(value: f64, scale: f64) -> Result<u64, &'static str> {
    if !value.is_finite() || value < 0.0 || !scale.is_finite() || scale <= 0.0 {
        return Err("event time and frame scale must be finite and nonnegative");
    }
    // Preserve the established Beats multiplication and rounding order.
    let frames = value * scale;
    if !frames.is_finite() || frames < 0.0 || frames >= u64::MAX as f64 {
        return Err("event frame is not representable");
    }
    let rounded = frames.round();
    if rounded >= u64::MAX as f64 {
        return Err("event frame is not representable");
    }
    Ok(rounded as u64)
}

impl EventTime {
    /// Round once to the nearest output frame. Zero is a valid onset. Resource
    /// budgets, event ordering and segment bounds belong to the caller.
    pub fn to_frames(self, beat_frames: u64, rate: u32) -> Result<u64, &'static str> {
        match self {
            Self::Beats(beats) => rounded_frames(beats, beat_frames as f64),
            Self::Seconds(seconds) => rounded_frames(seconds, rate as f64),
        }
    }
}

/// Admit an absolute duration and round once to output frames. Empty,
/// sub-frame and unrepresentable durations are refused, never clamped.
pub fn seconds_to_frames(seconds: f64, rate: u32) -> Result<u64, &'static str> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("duration seconds must be finite and positive");
    }
    let frames = EventTime::Seconds(seconds).to_frames(0, rate)?;
    if frames == 0 {
        return Err("duration is shorter than one rounded output frame");
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_are_absolute_and_beats_preserve_the_old_expression() {
        for rate in [1_000, 44_100, 48_000, 384_000] {
            for seconds in [0.0005, 0.0137, 0.5, 2.1, 123.456] {
                assert_eq!(
                    seconds_to_frames(seconds, rate).unwrap(),
                    (seconds * rate as f64).round() as u64
                );
                for beat in [1, 7, 19_314, 24_000] {
                    assert_eq!(
                        EventTime::Seconds(seconds).to_frames(beat, rate).unwrap(),
                        (seconds * rate as f64).round() as u64
                    );
                    assert_eq!(
                        EventTime::Beats(seconds).to_frames(beat, rate).unwrap(),
                        (seconds * beat as f64).round() as u64
                    );
                }
            }
        }
        assert_eq!(EventTime::Seconds(0.0).to_frames(0, 48_000), Ok(0));
    }

    #[test]
    fn invalid_values_and_float_integer_boundary_are_refused() {
        for value in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
            assert!(EventTime::Seconds(value).to_frames(1, 48_000).is_err());
            assert!(EventTime::Beats(value).to_frames(48_000, 48_000).is_err());
            assert!(seconds_to_frames(value, 48_000).is_err());
        }
        assert!(seconds_to_frames(0.0, 48_000).is_err());
        assert!(seconds_to_frames(0.000001, 1_000).is_err());
        assert!(seconds_to_frames(1.0, 0).is_err());
        assert!(EventTime::Beats(1.0).to_frames(0, 48_000).is_err());
        assert!(EventTime::Seconds(u64::MAX as f64).to_frames(1, 1).is_err());
    }
}
