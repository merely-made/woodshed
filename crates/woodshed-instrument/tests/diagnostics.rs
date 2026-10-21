// SPDX-License-Identifier: MPL-2.0
//! A dedicated test process isolates the process-local recording opt-in from
//! unrelated transport tests running concurrently in the library test binary.
use woodshed_instrument::midi::diagnostics::*;

#[test]
fn runtime_opt_in_stop_retains_and_restart_appends_without_off_capture() {
    // Run in a fresh process without the explicit startup opt-in.
    assert_ne!(
        std::env::var("INSTRUMENT_MIDI_DIAGNOSTICS").as_deref(),
        Ok("1")
    );
    assert_eq!(revision(), (false, None));
    assert!(snapshot().is_none());
    record("off", None, None, None, &[], || {
        panic!("off must remain lazy")
    });
    set_recording(false);
    assert!(snapshot().is_none());
    set_recording(true);
    set_recording(true);
    record(
        "sample",
        Some(4),
        Some(2),
        Some(7),
        &[0xb0, 66, 127],
        || "first".into(),
    );
    set_recording(false);
    let stopped = snapshot().unwrap();
    assert!(!stopped.recording);
    assert_eq!(stopped.events.len(), 3);
    assert_eq!(stopped.events[0].stage, "recording-start");
    assert_eq!(stopped.events[2].stage, "recording-stop");
    set_recording(false);
    record("off-again", None, None, None, &[], || {
        panic!("stopped must remain lazy")
    });
    assert_eq!(snapshot().unwrap(), stopped);
    set_recording(true);
    record("resumed", None, None, None, &[], || "second".into());
    set_recording(false);
    let resumed = snapshot().unwrap();
    assert_eq!(resumed.events.len(), 6);
    assert_eq!(resumed.events[3].stage, "recording-start");
    assert_eq!(resumed.events[4].stage, "resumed");
    assert_eq!(resumed.events[5].sequence, 6);
    assert_eq!(resumed.overwritten, 0);
    // Stop while an already-admitted producer is constructing its detail.
    set_recording(true);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let producer = std::thread::spawn(move || {
        record("racing-stop", None, None, None, &[], || {
            entered_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            "must not append after stop".into()
        });
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    set_recording(false);
    let stopped = snapshot().unwrap();
    resume_tx.send(()).unwrap();
    producer.join().unwrap();
    assert_eq!(snapshot().unwrap(), stopped);
}
