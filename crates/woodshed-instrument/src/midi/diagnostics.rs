//! Opt-in observations, never an admission, writer, retry or device acknowledgement.
//! Runtime opt-in; INSTRUMENT_MIDI_DIAGNOSTICS=1 is an explicit startup opt-in.
//! Stopping retains the bounded history. No hot-path file I/O.
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub const CAPACITY: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Event {
    pub sequence: u64,
    pub elapsed_micros: u64,
    pub stage: &'static str,
    pub request_id: Option<u64>,
    pub revision: Option<u64>,
    pub generation: Option<u64>,
    pub bytes: Vec<u8>,
    pub detail: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub recording: bool,
    pub capacity: usize,
    pub overwritten: u64,
    pub events: Vec<Event>,
}

struct Journal {
    start: Instant,
    sequence: u64,
    events: VecDeque<Event>,
    capacity: usize,
    overwritten: u64,
}
impl Journal {
    fn new(capacity: usize) -> Self {
        Self {
            start: Instant::now(),
            sequence: 0,
            events: VecDeque::new(),
            capacity,
            overwritten: 0,
        }
    }
    fn push(&mut self, mut event: Event) {
        self.sequence = self.sequence.saturating_add(1);
        event.sequence = self.sequence;
        event.elapsed_micros = self.start.elapsed().as_micros().min(u64::MAX as u128) as u64;
        event.truncated = event.bytes.len() > 16 || event.detail.chars().count() > 256;
        event.bytes.truncate(16);
        event.detail = event.detail.chars().take(256).collect();
        if self.events.len() == self.capacity {
            self.events.pop_front();
            self.overwritten = self.overwritten.saturating_add(1);
        }
        self.events.push_back(event);
    }
    fn boundary(&mut self, recording: bool, detail: &str) {
        self.push(Event {
            sequence: 0,
            elapsed_micros: 0,
            stage: if recording {
                "recording-start"
            } else {
                "recording-stop"
            },
            request_id: None,
            revision: None,
            generation: None,
            bytes: Vec::new(),
            detail: detail.into(),
            truncated: false,
        });
    }
    fn snapshot(&self, recording: bool) -> Snapshot {
        Snapshot {
            recording,
            capacity: self.capacity,
            overwritten: self.overwritten,
            events: self.events.iter().cloned().collect(),
        }
    }
}

struct Recorder {
    recording: AtomicBool,
    journal: Mutex<Option<Journal>>,
}
fn recorder() -> &'static Recorder {
    static RECORDER: OnceLock<Recorder> = OnceLock::new();
    RECORDER.get_or_init(|| {
        let recording = std::env::var("INSTRUMENT_MIDI_DIAGNOSTICS").as_deref() == Ok("1");
        let journal = recording.then(|| {
            let mut journal = Journal::new(CAPACITY);
            journal.boundary(true, "Explicit startup opt-in");
            journal
        });
        Recorder {
            recording: AtomicBool::new(recording),
            journal: Mutex::new(journal),
        }
    })
}

/// Session-local opt-in. Stop and retain history; never persist the preference.
pub fn set_recording(recording: bool) {
    let recorder = recorder();
    if let Ok(mut journal) = recorder.journal.lock() {
        if recorder.recording.load(Ordering::Acquire) == recording {
            return;
        }
        if recording && journal.is_none() {
            *journal = Some(Journal::new(CAPACITY));
        }
        if let Some(journal) = journal.as_mut() {
            journal.boundary(recording, "Desktop session recording control");
        }
        recorder.recording.store(recording, Ordering::Release);
    }
}

/// Cheap readout key so desktop polling need not copy an unchanged history.
pub fn revision() -> (bool, Option<u64>) {
    let recorder = recorder();
    let journal = recorder.journal.lock().ok();
    (
        recorder.recording.load(Ordering::Acquire),
        journal.and_then(|journal| journal.as_ref().map(|journal| journal.sequence)),
    )
}

/// Disabled by default; lazy detail construction and bounded memory when enabled.
/// Losing diagnostic access cannot change the musical operation's outcome.
pub fn record(
    stage: &'static str,
    request_id: Option<u64>,
    revision: Option<u64>,
    generation: Option<u64>,
    bytes: &[u8],
    detail: impl FnOnce() -> String,
) {
    let recorder = recorder();
    if recorder.recording.load(Ordering::Acquire) {
        let detail = detail();
        if let Ok(mut journal) = recorder.journal.lock() {
            if !recorder.recording.load(Ordering::Acquire) {
                return;
            }
            let Some(journal) = journal.as_mut() else {
                return;
            };
            journal.push(Event {
                sequence: 0,
                elapsed_micros: 0,
                stage,
                request_id,
                revision,
                generation,
                bytes: bytes.to_vec(),
                detail,
                truncated: false,
            });
        }
    }
}

/// Read-only copy; it never drains, changes arming or alters a send deadline.
pub fn snapshot() -> Option<Snapshot> {
    let recorder = recorder();
    let journal = recorder.journal.lock().ok()?;
    journal
        .as_ref()
        .map(|journal| journal.snapshot(recorder.recording.load(Ordering::Acquire)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> Event {
        Event {
            sequence: 0,
            elapsed_micros: 0,
            stage: "test",
            request_id: Some(4),
            revision: Some(2),
            generation: Some(7),
            bytes: vec![0xb0, 66, 127],
            detail: "observed, not acknowledged".into(),
            truncated: false,
        }
    }
    #[test]
    fn journal_is_bounded_sequenced_and_snapshot_does_not_drain() {
        let mut journal = Journal::new(2);
        for _ in 0..4 {
            journal.push(event());
        }
        let snapshot = journal.snapshot(true);
        assert_eq!(snapshot.overwritten, 2);
        assert_eq!(
            snapshot
                .events
                .iter()
                .map(|e| e.sequence)
                .collect::<Vec<_>>(),
            [3, 4]
        );
        assert_eq!(journal.snapshot(true).events.len(), 2);
        assert_eq!(snapshot.events[1].bytes, [0xb0, 66, 127]);
        assert!(!snapshot.events[1].truncated);
        assert!(snapshot.events[0].elapsed_micros <= snapshot.events[1].elapsed_micros);
    }
    #[test]
    fn oversized_diagnostics_are_explicitly_truncated_without_utf8_damage() {
        let mut journal = Journal::new(1);
        let mut entry = event();
        entry.bytes = vec![0; 32];
        entry.detail = "é".repeat(300);
        journal.push(entry);
        let snapshot = journal.snapshot(true);
        let entry = &snapshot.events[0];
        assert!(entry.truncated);
        assert_eq!(entry.bytes.len(), 16);
        assert_eq!(entry.detail.chars().count(), 256);
    }
}
