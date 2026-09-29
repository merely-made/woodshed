// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::*;
use diagnostics::{Diagnostics, Phase};
use std::fs;

fn observed_store() -> Diagnostics {
    Diagnostics::new(
        true,
        "worker-fixture".into(),
        apparatus::RetentionLimits {
            max_records: 256,
            max_bytes: 262_144,
            max_age: Duration::from_secs(300),
        },
    )
}

fn receive(persistence: &Persistence) -> (u64, Result<(), String>, Option<apparatus::RecordRef>) {
    match persistence
        .replies
        .recv_timeout(Duration::from_secs(5))
        .expect("real storage worker reply")
    {
        IoReply::Saved {
            revision,
            result,
            execution,
        } => (revision, result, execution),
        _ => panic!("unexpected storage reply"),
    }
}

#[test]
fn real_worker_success_precedes_durable_ack_and_causal_links_follow_owners() {
    let directory = tempfile::tempdir().unwrap();
    let diagnostics = observed_store();
    let mut cursor = diagnostics.cursor().unwrap();
    let mut persistence = Persistence::start_with_diagnostics(
        JsonDirectoryStore::new(directory.path()),
        diagnostics.clone(),
    );
    persistence.changed();
    persistence.flush(&RedshankModel::default()).unwrap();
    let (revision, result, execution) = receive(&persistence);
    assert!(result.is_ok());
    assert_eq!(
        persistence.durable, 0,
        "execution does not acknowledge durability"
    );
    assert!(
        JsonDirectoryStore::new(directory.path())
            .load()
            .unwrap()
            .is_some()
    );
    let before = diagnostics.read(&mut cursor, 256).unwrap();
    let request = before
        .records
        .iter()
        .find(|r| r.payload.phase == Phase::DispatchRequested)
        .unwrap();
    let started = before
        .records
        .iter()
        .find(|r| r.payload.phase == Phase::ExecutionStarted)
        .unwrap();
    let finished = before
        .records
        .iter()
        .find(|r| r.payload.phase == Phase::ExecutionFinished)
        .unwrap();
    assert_eq!(
        started.envelope.metadata.cause.as_ref(),
        Some(&request.envelope.reference)
    );
    assert_eq!(
        finished.envelope.metadata.cause.as_ref(),
        Some(&started.envelope.reference)
    );
    assert_eq!(
        finished.payload.result,
        Some(diagnostics::ResultKind::Success)
    );
    assert!(
        !before
            .records
            .iter()
            .any(|r| r.payload.phase == Phase::SaveReplyHandled)
    );
    persistence.acknowledge_with_cause(revision, &result, execution);
    assert_eq!(persistence.durable, 1);
    let after = diagnostics.read(&mut cursor, 256).unwrap();
    assert_eq!(after.records.len(), 1);
    assert_eq!(after.records[0].payload.phase, Phase::SaveReplyHandled);
    assert_eq!(
        after.records[0].envelope.metadata.cause.as_ref(),
        Some(&finished.envelope.reference)
    );
}

#[test]
fn real_io_failure_and_retry_share_logical_revision_but_keep_distinct_requests() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("private-storage-path");
    fs::write(&root, b"blocks directory creation").unwrap();
    let diagnostics = observed_store();
    let mut cursor = diagnostics.cursor().unwrap();
    let mut persistence =
        Persistence::start_with_diagnostics(JsonDirectoryStore::new(&root), diagnostics.clone());
    persistence.changed();
    persistence.flush(&RedshankModel::default()).unwrap();
    let (revision, result, execution) = receive(&persistence);
    assert!(result.is_err());
    persistence.acknowledge_with_cause(revision, &result, execution);
    assert_eq!(persistence.durable, 0);
    assert_eq!(persistence.revision, 1);
    fs::remove_file(&root).unwrap();
    persistence.failed = false;
    persistence.flush(&RedshankModel::default()).unwrap();
    let (revision, result, execution) = receive(&persistence);
    assert!(result.is_ok());
    assert_eq!(persistence.durable, 0);
    persistence.acknowledge_with_cause(revision, &result, execution);
    assert_eq!(persistence.durable, 1);
    assert!(JsonDirectoryStore::new(&root).load().unwrap().is_some());
    let batch = diagnostics.read(&mut cursor, 256).unwrap();
    let requests: Vec<_> = batch
        .records
        .iter()
        .filter(|r| r.payload.phase == Phase::DispatchRequested)
        .collect();
    assert_eq!(requests.len(), 2);
    assert_ne!(
        requests[0].envelope.reference,
        requests[1].envelope.reference
    );
    assert_eq!(
        requests[0].envelope.metadata.operation,
        requests[1].envelope.metadata.operation
    );
    let results: Vec<_> = batch
        .records
        .iter()
        .filter(|r| r.payload.phase == Phase::ExecutionFinished)
        .map(|r| r.payload.result)
        .collect();
    assert_eq!(
        results,
        [
            Some(diagnostics::ResultKind::IoFailure),
            Some(diagnostics::ResultKind::Success)
        ]
    );
    let handled: Vec<_> = batch
        .records
        .iter()
        .filter(|r| r.payload.phase == Phase::SaveReplyHandled)
        .map(|r| (r.payload.result, r.payload.durable_revision))
        .collect();
    assert_eq!(
        handled,
        [
            (Some(diagnostics::ResultKind::Failure), Some(0)),
            (Some(diagnostics::ResultKind::Success), Some(1)),
        ]
    );
    let exported = serde_json::to_string(&batch).unwrap();
    assert!(!exported.contains("private-storage-path"));
    assert!(!exported.contains("Could not save"));
}

#[test]
fn coalesced_dirty_revisions_do_not_invent_save_requests_or_action_causes() {
    let directory = tempfile::tempdir().unwrap();
    let diagnostics = observed_store();
    let mut cursor = diagnostics.cursor().unwrap();
    let mut persistence = Persistence::start_with_diagnostics(
        JsonDirectoryStore::new(directory.path()),
        diagnostics.clone(),
    );
    persistence.changed();
    persistence.flush(&RedshankModel::default()).unwrap();
    persistence.changed();
    persistence.changed();
    persistence.flush(&RedshankModel::default()).unwrap();
    let (revision, result, execution) = receive(&persistence);
    assert_eq!(revision, 1);
    persistence.acknowledge_with_cause(revision, &result, execution);
    assert_eq!(persistence.durable, 1);
    assert_eq!(persistence.revision, 3);
    persistence.flush(&RedshankModel::default()).unwrap();
    let (revision, result, execution) = receive(&persistence);
    assert_eq!(revision, 3);
    persistence.acknowledge_with_cause(revision, &result, execution);
    let batch = diagnostics.read(&mut cursor, 256).unwrap();
    let dispatched: Vec<_> = batch
        .records
        .iter()
        .filter(|r| r.payload.phase == Phase::DispatchRequested)
        .map(|r| r.payload.revision)
        .collect();
    assert_eq!(dispatched, [1, 3]);
    let coalesced: Vec<_> = batch
        .records
        .iter()
        .filter(|r| {
            r.payload.phase == Phase::DirtyRevisionObserved
                && r.payload.in_flight_revision == Some(1)
        })
        .collect();
    assert_eq!(coalesced.len(), 2);
    assert!(coalesced.iter().all(|r| r.envelope.metadata.operation.is_none() && r.envelope.metadata.cause.is_none()));
}
