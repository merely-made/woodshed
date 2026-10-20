use std::{path::PathBuf, process::Command};
use woodshedding::{notation::MusicDocument, timing::ArrangementSchedule};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn musicxml_triplets_rests_and_disposable_tempo_survive_file_to_timing() {
    let path = fixture("rhythm.musicxml");
    let before = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_musicxml-timing"))
        .arg(&path)
        .arg("73.5")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("written 96 BPM | timing 73.5 BPM | 960 ticks/quarter"));
    assert!(text.contains("event 5 | onset 2880 | duration 960 | rest"));
    assert!(text.contains("event 2 | onset 320 | duration 320 | MIDI"));
    assert!(text.contains("event 3 | onset 640 | duration 320 | MIDI"));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn progression_keeps_chord_contacts_simultaneous_and_defaults_to_written_tempo() {
    let output = Command::new(env!("CARGO_BIN_EXE_musicxml-timing"))
        .arg(fixture("progression.musicxml"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("written 96 BPM | timing 96 BPM"));
    assert_eq!(text.matches("bar ").count(), 4);
    let events: Vec<_> = text.lines().filter(|l| l.contains("event ")).collect();
    assert_eq!(events.len(), 4);
    for event in events {
        assert!(event.contains("onset 0 | duration 3840"));
        assert!(event.matches("MIDI ").count() >= 4);
    }
}

#[test]
fn invalid_tempo_is_a_failure_without_partial_timing_output() {
    for bpm in ["NaN", "0", "401", "not-a-tempo"] {
        let output = Command::new(env!("CARGO_BIN_EXE_musicxml-timing"))
            .arg(fixture("rhythm.musicxml"))
            .arg(bpm)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn parsed_musicxml_can_be_timed_beyond_audio_engine_capacity() {
    let document = MusicDocument {
        musicxml: std::fs::read_to_string(fixture("progression.musicxml")).unwrap(),
    };
    let mut score = document.score().unwrap();
    score.measures = vec![score.measures[0].clone(); 257];
    let document = MusicDocument::from_score(&score).unwrap();
    let reparsed = document.score().unwrap();
    let timing = ArrangementSchedule::from_score(&reparsed, 96.0).unwrap();
    assert_eq!(timing.measures.len(), 257);
    assert_eq!(timing.measures[256].duration_ticks, 3840);
    assert_eq!(reparsed, score);
}
