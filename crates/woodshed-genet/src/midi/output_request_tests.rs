use super::*;
use std::time::{Duration, Instant};

#[test]
fn closed_worker_clears_accepted_connection_and_reports_failure() {
    let mut host = MidiHost::new();
    host.output_state.lock().unwrap().connected = Some("previous".into());
    host.output_state.lock().unwrap().ready = true;
    host.clock_tx.send(ClockCmd::Stop).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !host.clock_thread.as_ref().unwrap().is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    host.poll();
    assert!(host.connected_output().is_none());
    assert!(!host.output_ready());
    assert!(host.error().unwrap().contains("worker is unavailable"));
    assert!(!host.needs_service());
}

#[test]
fn production_channel_supersedes_pending_and_dedupes_failed_target() {
    let mut host = MidiHost::new();
    host.clock_tx.send(ClockCmd::Stop).unwrap();
    host.clock_thread.take().unwrap().join().unwrap();
    let (tx, rx) = channel();
    host.clock_tx = tx;
    host.requested_out = Some("A".into());
    host.output_state.lock().unwrap().connected = Some("A".into());
    host.connect_output(Some("B"));
    let ClockCmd::Connect {
        revision: first, ..
    } = rx.recv().unwrap()
    else {
        panic!()
    };
    host.output_state
        .lock()
        .unwrap()
        .failure("cannot open B".into());
    host.connect_output(Some("B"));
    assert!(rx.try_recv().is_err());
    host.connect_output(Some("A"));
    let ClockCmd::Connect {
        target,
        revision: second,
    } = rx.recv().unwrap()
    else {
        panic!()
    };
    assert_eq!(target.as_deref(), Some("A"));
    assert!(second > first);
    host.connect_output(Some("B"));
    let ClockCmd::Connect {
        revision: third, ..
    } = rx.recv().unwrap()
    else {
        panic!()
    };
    assert!(third > second);
    host.connect_output(None);
    let ClockCmd::Connect {
        target,
        revision: fourth,
    } = rx.recv().unwrap()
    else {
        panic!()
    };
    assert!(target.is_none());
    assert!(fourth > third);
    host.connect_output(None);
    assert!(rx.try_recv().is_err());
}
