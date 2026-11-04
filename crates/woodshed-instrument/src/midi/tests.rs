use super::*;
use std::time::Duration;

// === Parse tests ===

#[test]
fn pedal_input_sequence_consumes_identical_timestamps_once_and_detects_overflow() {
    let mut state = MidiInState::default();
    let at = Instant::now();
    let event = MidiEvent::ControlChange {
        channel: 0,
        controller: 67,
        value: 127,
    };
    state.push(at, event);
    state.push(at, event);
    assert_eq!(state.events_since(0).unwrap().len(), 2);
    let cursor = state.received_events;
    assert!(state.events_since(cursor).unwrap().is_empty());
    state.push(at, event);
    assert_eq!(state.events_since(cursor).unwrap().len(), 1);
    for _ in 0..MidiInState::CAPACITY {
        state.push(at, event);
    }
    assert!(state.events_since(cursor).is_none());
}
#[test]
fn pedal_input_rejects_truncated_and_non_data_bytes() {
    for bytes in [
        vec![0x90, 67],
        vec![0x80, 67],
        vec![0xb0, 67],
        vec![0x90, 200, 127],
        vec![0xb0, 67, 255],
    ] {
        assert!(parse_message(&bytes).is_none());
    }
}

#[test]
fn parse_note_on_with_velocity() {
    let bytes = [0x91, 60, 96]; // channel 1, middle C, vel 96
    let ev = parse_message(&bytes).unwrap();
    assert_eq!(
        ev,
        MidiEvent::NoteOn {
            channel: 1,
            note: 60,
            velocity: 96
        }
    );
}

#[test]
fn parse_note_on_velocity_zero_is_note_off() {
    let bytes = [0x90, 60, 0];
    let ev = parse_message(&bytes).unwrap();
    assert_eq!(
        ev,
        MidiEvent::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0
        }
    );
}

#[test]
fn parse_explicit_note_off() {
    let bytes = [0x82, 64, 64];
    let ev = parse_message(&bytes).unwrap();
    assert_eq!(
        ev,
        MidiEvent::NoteOff {
            channel: 2,
            note: 64,
            velocity: 64
        }
    );
}

#[test]
fn parse_control_change() {
    let bytes = [0xB0, 7, 100]; // ch 0, volume, value 100
    let ev = parse_message(&bytes).unwrap();
    assert_eq!(
        ev,
        MidiEvent::ControlChange {
            channel: 0,
            controller: 7,
            value: 100
        }
    );
}

#[test]
fn parse_realtime_messages() {
    assert_eq!(parse_message(&[CLOCK_TICK]), Some(MidiEvent::Clock));
    assert_eq!(parse_message(&[TRANSPORT_START]), Some(MidiEvent::Start));
    assert_eq!(parse_message(&[TRANSPORT_STOP]), Some(MidiEvent::Stop));
    assert_eq!(
        parse_message(&[TRANSPORT_CONTINUE]),
        Some(MidiEvent::Continue)
    );
}

#[test]
fn parse_unknown_returns_none() {
    // Program Change is 0xC0..=0xCF; we don't model it.
    assert!(parse_message(&[0xC0, 5]).is_none());
    assert!(parse_message(&[]).is_none());
}

// === Clock sync tests ===

/// Build a sequence of ticks spaced `interval_ms` apart starting
/// from `now`. Returns the corresponding Instant series.
fn tick_series(start: Instant, count: usize, interval_ms: u64) -> Vec<Instant> {
    (0..count)
        .map(|i| start + Duration::from_millis(interval_ms * i as u64))
        .collect()
}

#[test]
fn clock_sync_120_bpm_from_simulated_ticks() {
    // 120 BPM, 24 PPQN: secs_per_beat = 0.5s, secs_per_tick =
    // 0.5 / 24 ≈ 20.833 ms. Use that as interval.
    let mut sync = MidiClockSync::new();
    let start = Instant::now();
    // Use fractional milliseconds via micros.
    let interval = Duration::from_micros(20_833);
    for i in 0..48 {
        sync.record_tick(start + interval * i);
    }
    let bpm = sync.estimated_bpm().unwrap();
    assert!((bpm - 120.0).abs() < 1.0, "got {bpm}");
}

#[test]
fn clock_sync_60_bpm_from_simulated_ticks() {
    // 60 BPM: secs_per_beat = 1s, secs_per_tick = 1/24 ≈ 41.667ms.
    let mut sync = MidiClockSync::new();
    let start = Instant::now();
    let interval = Duration::from_micros(41_667);
    for i in 0..48 {
        sync.record_tick(start + interval * i);
    }
    let bpm = sync.estimated_bpm().unwrap();
    assert!((bpm - 60.0).abs() < 1.0, "got {bpm}");
}

#[test]
fn clock_sync_returns_none_without_ticks() {
    let sync = MidiClockSync::new();
    assert!(sync.estimated_bpm().is_none());
}

#[test]
fn clock_sync_reset_clears_state() {
    let mut sync = MidiClockSync::new();
    let ticks = tick_series(Instant::now(), 24, 20);
    for t in ticks {
        sync.record_tick(t);
    }
    assert!(sync.estimated_bpm().is_some());
    sync.reset();
    assert!(sync.estimated_bpm().is_none());
    assert_eq!(sync.tick_count(), 0);
}

#[test]
fn list_ports_does_not_panic() {
    // Don't assert anything about port counts — the test
    // environment may or may not have MIDI. We just verify the
    // call doesn't panic.
    let _ = list_input_ports();
    let _ = list_output_ports();
}
