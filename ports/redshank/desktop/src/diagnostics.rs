// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! An opt-in bounded copy of persistence owner observations. No domain data,
//! paths, model text or raw errors enter this payload or its causal references.

use apparatus::{
    Admission, Batch, Cursor, ObservationMetadata, ObservationStore, OperationId, RecordRef,
    RetentionLimits, RunId, SourceId,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Phase {
    DirtyRevisionObserved,
    DispatchRequested,
    DispatchAccepted,
    DispatchRejected,
    ExecutionStarted,
    ExecutionFinished,
    SaveReplyHandled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ResultKind {
    Success,
    Failure,
    IoFailure,
    EncodeFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SaveObservation {
    pub phase: Phase,
    pub revision: u64,
    pub result: Option<ResultKind>,
    pub dirty_revision: Option<u64>,
    pub durable_revision: Option<u64>,
    pub in_flight_revision: Option<u64>,
}

impl SaveObservation {
    pub fn new(phase: Phase, revision: u64) -> Self {
        Self {
            phase,
            revision,
            result: None,
            dirty_revision: None,
            durable_revision: None,
            in_flight_revision: None,
        }
    }
}

struct Collector {
    store: ObservationStore<SaveObservation>,
    started: Instant,
    failure: Option<&'static str>,
}

#[derive(Clone)]
pub struct Diagnostics {
    enabled: bool,
    collector: Arc<Mutex<Collector>>,
}

impl Diagnostics {
    pub fn new(enabled: bool, run: RunId, limits: RetentionLimits) -> Self {
        Self {
            enabled,
            collector: Arc::new(Mutex::new(Collector {
                store: ObservationStore::new(run, SourceId::from("redshank.persistence"), limits),
                started: Instant::now(),
                failure: None,
            })),
        }
    }

    pub fn from_env() -> Self {
        fn setting(key: &str, fallback: usize) -> Result<usize, &'static str> {
            std::env::var(key).ok().map_or(Ok(fallback), |value| {
                value
                    .parse()
                    .map_err(|_| "invalid diagnostic retention setting")
            })
        }
        let enabled = std::env::var("REDSHANK_DIAGNOSTICS").ok().as_deref() == Some("1");
        let configured = (|| {
            Ok::<_, &'static str>(RetentionLimits {
                max_records: setting("REDSHANK_DIAGNOSTIC_RECORDS", 256)?,
                max_bytes: setting("REDSHANK_DIAGNOSTIC_BYTES", 262_144)?,
                max_age: Duration::from_secs(setting("REDSHANK_DIAGNOSTIC_AGE_SECS", 300)? as u64),
            })
        })();
        static RUN: AtomicU64 = AtomicU64::new(0);
        let run = RunId(format!(
            "redshank-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            RUN.fetch_add(1, Ordering::Relaxed)
        ));
        let diagnostics = Self::new(
            enabled,
            run,
            configured.unwrap_or(RetentionLimits {
                max_records: 0,
                max_bytes: 0,
                max_age: Duration::ZERO,
            }),
        );
        diagnostics.collector.lock().unwrap().failure = configured.err();
        diagnostics
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The returned reference identifies this occurrence even when retention
    /// rejects it; later readers see that omission as a gap.
    pub fn record(&self, payload: SaveObservation, cause: Option<RecordRef>) -> Option<RecordRef> {
        if !self.enabled {
            return None;
        }
        let mut collector = self.collector.lock().ok()?;
        let encoded = match serde_json::to_vec(&payload) {
            Ok(encoded) => encoded.len(),
            Err(_) => {
                collector.failure = Some("diagnostic payload encoding failed");
                return None;
            },
        };
        let operation = (payload.phase != Phase::DirtyRevisionObserved)
            .then(|| OperationId(format!("save:{}", payload.revision)));
        // Sample receipt time under the lock. Worker and desktop acquisition
        // order, not an earlier producer timestamp, defines store admission.
        let now = collector.started.elapsed();
        match collector.store.record(
            payload,
            encoded,
            ObservationMetadata {
                operation,
                cause,
                ..Default::default()
            },
            now,
        ) {
            Ok(Admission::Retained(reference) | Admission::Rejected { reference, .. }) => {
                Some(reference)
            },
            Err(_) => {
                collector.failure = Some("diagnostic record admission failed");
                None
            },
        }
    }

    pub fn cursor(&self) -> Result<Cursor, String> {
        self.collector
            .lock()
            .map(|collector| collector.store.cursor())
            .map_err(|_| "diagnostic store lock failed".into())
    }

    pub fn read(
        &self,
        cursor: &mut Cursor,
        max_records: usize,
    ) -> Result<Batch<SaveObservation>, String> {
        let mut collector = self
            .collector
            .lock()
            .map_err(|_| "diagnostic store lock failed")?;
        if let Some(failure) = collector.failure {
            return Err(failure.into());
        }
        let now = collector.started.elapsed();
        collector
            .store
            .read(cursor, now, max_records)
            .map_err(|_| "diagnostic observation read failed".into())
    }

    pub fn attachment(&self, cursor: &mut Cursor) -> Result<mesquite::DiagnosticBatch, String> {
        let limit = self
            .collector
            .lock()
            .map_err(|_| "diagnostic store lock failed")?
            .store
            .limits()
            .max_records
            .max(1);
        Ok(self.read(cursor, limit)?.map_payload(|payload| {
            serde_json::to_value(payload).expect("typed save observation serializes")
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_receipt_and_inspection_readers_report_loss() {
        let diagnostics = Diagnostics::new(
            true,
            "readers".into(),
            RetentionLimits {
                max_records: 1,
                max_bytes: 4096,
                max_age: Duration::from_secs(300),
            },
        );
        let mut first = diagnostics.cursor().unwrap();
        let mut second = diagnostics.cursor().unwrap();
        diagnostics.record(SaveObservation::new(Phase::DispatchRequested, 1), None);
        diagnostics.record(SaveObservation::new(Phase::ExecutionStarted, 1), None);
        let a = diagnostics.read(&mut first, 1).unwrap();
        let b = diagnostics.read(&mut second, 1).unwrap();
        assert_eq!(a.records, b.records);
        assert_eq!(a.gaps, b.gaps);
        assert_eq!(a.stats.loss.evicted, 1);
        assert_eq!(a.gaps[0].first_sequence, 1);
        assert_eq!(a.records[0].payload.phase, Phase::ExecutionStarted);
    }

    #[test]
    fn disabled_retention_reports_gaps_without_payloads() {
        let diagnostics = Diagnostics::new(
            true,
            "disabled".into(),
            RetentionLimits {
                max_records: 0,
                max_bytes: 4096,
                max_age: Duration::from_secs(300),
            },
        );
        let mut cursor = diagnostics.cursor().unwrap();
        assert!(
            diagnostics
                .record(SaveObservation::new(Phase::DispatchRequested, 1), None)
                .is_some()
        );
        let batch = diagnostics.read(&mut cursor, 1).unwrap();
        assert!(batch.records.is_empty());
        assert_eq!(batch.stats.loss.rejected_disabled, 1);
        assert_eq!(batch.gaps.len(), 1);
    }
}
