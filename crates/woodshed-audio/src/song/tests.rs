use super::*;

fn bar_at(bpm: f32, beats: u8) -> Bar {
    Bar::with_tempo(bpm, TimeSignature::new(beats, 4))
}

#[test]
fn new_song_has_one_default_bar() {
    let s = Song::new();
    assert_eq!(s.len(), 1);
    assert_eq!(s.cursor.bar_idx, 0);
    assert!(!s.playing);
}

#[test]
fn add_bar_appends_and_inherits_settings() {
    let mut s = Song::new();
    s.bar_mut(0).unwrap().bpm = 140.0;
    let idx = s.add_bar();
    assert_eq!(idx, 1);
    assert_eq!(s.len(), 2);
    // New bar should inherit the tempo, not snap back to default.
    assert_eq!(s.bar(1).unwrap().bpm, 140.0);
    // But audio buffer should NOT carry forward.
    assert!(s.bar(1).unwrap().audio_buffer.is_none());
}

#[test]
fn insert_bar_shifts_cursor_right_when_needed() {
    let mut s = Song::new();
    s.add_bar();
    s.add_bar(); // 3 bars
    s.cursor.bar_idx = 2;
    s.insert_bar(1, bar_at(100.0, 4)).unwrap();
    // Cursor was at 2; we inserted at 1; cursor should be at 3.
    assert_eq!(s.cursor.bar_idx, 3);
    assert_eq!(s.len(), 4);
}

#[test]
fn remove_bar_adjusts_cursor() {
    let mut s = Song::new();
    s.add_bar();
    s.add_bar(); // 3 bars
    s.cursor.bar_idx = 2;
    s.remove_bar(1).unwrap();
    // Removed bar 1, cursor was at 2 → now at 1.
    assert_eq!(s.cursor.bar_idx, 1);
    assert_eq!(s.len(), 2);
}

#[test]
fn remove_last_remaining_bar_errors() {
    let mut s = Song::new();
    assert!(matches!(
        s.remove_bar(0),
        Err(SongError::CannotRemoveLastBar)
    ));
}

#[test]
fn copy_paste_round_trips() {
    let mut s = Song::new();
    s.add_bar(); // 2 bars
    s.bar_mut(0).unwrap().bpm = 90.0;
    s.bar_mut(0).unwrap().label = "Verse".into();
    let clip = s.copy_bar(0).unwrap();
    s.paste_bar(1, clip).unwrap();
    assert_eq!(s.bar(1).unwrap().bpm, 90.0);
    assert_eq!(s.bar(1).unwrap().label, "Verse");
}

#[test]
fn move_bar_repositions_without_changing_count() {
    let mut s = Song::new();
    s.add_bar();
    s.add_bar(); // 3 bars
    s.bar_mut(0).unwrap().label = "A".into();
    s.bar_mut(1).unwrap().label = "B".into();
    s.bar_mut(2).unwrap().label = "C".into();
    s.move_bar(0, 2).unwrap();
    // Order should now be B, C, A.
    assert_eq!(s.bar(0).unwrap().label, "B");
    assert_eq!(s.bar(1).unwrap().label, "C");
    assert_eq!(s.bar(2).unwrap().label, "A");
    assert_eq!(s.len(), 3);
}

#[test]
fn pending_changes_apply_at_bar_boundary() {
    let mut s = Song::new();
    s.add_bar();
    s.add_bar(); // 3 bars
    s.playing = true;
    s.queue(PendingChange::SeekTo { bar_idx: 2 });
    assert_eq!(s.cursor.bar_idx, 0);

    // Cross a boundary.
    let bar_samples = s.current_bar().duration_samples(48_000.0);
    s.advance(bar_samples + 100, 48_000.0);

    // Cursor should have crossed to next bar AND applied SeekTo
    // moving us to bar 2. The SeekTo applies during the
    // on_bar_boundary call as we cross.
    assert_eq!(s.cursor.bar_idx, 2);
}

#[test]
fn loop_wraps_at_end() {
    let mut s = Song::new();
    s.add_bar(); // 2 bars
    s.playing = true;
    let bar0_samples = s.bar(0).unwrap().duration_samples(48_000.0);
    let bar1_samples = s.bar(1).unwrap().duration_samples(48_000.0);
    s.advance(bar0_samples + bar1_samples + 100, 48_000.0);
    // After crossing both bars, looped back to bar 0 with ~100
    // samples in.
    assert_eq!(s.cursor.bar_idx, 0);
    assert!(s.cursor.sample_in_bar <= 200);
}

#[test]
fn one_shot_stops_at_end() {
    let mut s = Song::new();
    s.add_bar(); // 2 bars
    s.one_shot = true;
    s.playing = true;
    let total =
        s.bar(0).unwrap().duration_samples(48_000.0) + s.bar(1).unwrap().duration_samples(48_000.0);
    s.advance(total + 1000, 48_000.0);
    assert!(!s.playing, "one-shot song should stop at end");
}

#[test]
fn pending_clear_drops_unapplied() {
    let mut s = Song::new();
    s.add_bar();
    s.playing = true;
    s.queue(PendingChange::SeekTo { bar_idx: 1 });
    s.queue(PendingChange::StopRecording);
    assert_eq!(s.pending.len(), 2);
    s.clear_pending();
    assert!(s.pending.is_empty());
    // And the change should NOT apply on the next boundary.
    let bar_samples = s.current_bar().duration_samples(48_000.0);
    s.advance(bar_samples + 100, 48_000.0);
    // Just rolled into bar 1 normally (next bar), not by SeekTo.
    assert_eq!(s.cursor.bar_idx, 1);
}

#[test]
fn song_serializes_to_json() {
    let mut s = Song::new();
    s.name = "Test Song".into();
    s.add_bar();
    s.bar_mut(1).unwrap().bpm = 90.0;
    s.bar_mut(1).unwrap().chord_ref = Some(ChordRef {
        formula_name: "Minor".into(),
        root_freq_hz: 220.0,
        pitches_hz: vec![220.0, 261.63, 329.63],
        label: "Am".into(),
    });
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("Test Song"));
    assert!(json.contains("90"));
    let back: Song = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, s.name);
    assert_eq!(back.len(), 2);
    assert_eq!(back.bar(1).unwrap().bpm, 90.0);
    assert_eq!(back.bar(1).unwrap().chord_ref.as_ref().unwrap().label, "Am");
}

#[test]
fn bar_length_multiplies_duration() {
    let mut b = Bar::with_tempo(120.0, TimeSignature::new(4, 4));
    let one = b.duration_secs();
    b.length = 3;
    assert!((b.duration_secs() - one * 3.0).abs() < 1e-4);
}

#[test]
fn multi_measure_bar_advances_after_full_block() {
    let mut s = Song::new();
    s.add_bar(); // 2 bars
    s.bar_mut(0).unwrap().length = 2;
    s.playing = true;
    let block0 = s.bar(0).unwrap().duration_samples(48_000.0);
    // One measure in (half the block) shouldn't cross the boundary.
    s.advance((block0 / 2).saturating_sub(10), 48_000.0);
    assert_eq!(s.cursor.bar_idx, 0, "still inside the length-2 block");
    // Past the full block, we land in bar 1.
    s.advance(block0, 48_000.0);
    assert_eq!(s.cursor.bar_idx, 1);
}

#[test]
fn bar_length_serde_default_for_legacy() {
    // A Bar JSON predating the `length` field must read back as a
    // single-measure bar, not a zero-length one.
    let mut v = serde_json::to_value(Bar::default()).unwrap();
    v.as_object_mut().unwrap().remove("length");
    let b: Bar = serde_json::from_value(v).unwrap();
    assert_eq!(b.length, 1);
}

#[test]
fn attach_and_detach_audio_buffer() {
    let mut s = Song::new();
    let buf = SampleBuffer::new(vec![0.5; 100], 48_000);
    s.attach_buffer(0, buf).unwrap();
    assert!(s.bar(0).unwrap().audio_buffer.is_some());
    let removed = s.detach_buffer(0).unwrap();
    assert!(removed.is_some());
    assert!(s.bar(0).unwrap().audio_buffer.is_none());
}
