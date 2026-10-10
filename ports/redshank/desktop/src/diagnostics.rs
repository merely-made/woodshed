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
    capture_ids_bounded: bool,
}

#[derive(Clone)]
pub struct Diagnostics {
    enabled: bool,
    collector: Arc<Mutex<Collector>>,
}

impl Diagnostics {
    pub fn new(enabled: bool, run: RunId, limits: RetentionLimits) -> Self {
        let capture_ids_bounded = run.0.len() <= 128;
        Self {
            enabled,
            collector: Arc::new(Mutex::new(Collector {
                store: ObservationStore::new(run, SourceId::from("redshank.persistence"), limits),
                started: Instant::now(),
                failure: None,
                capture_ids_bounded,
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

    /// Admission context only. This neither reads records nor advances a
    /// receipt reader, and is not a causal reference or durability acknowledgement.
    fn capture_cut(&self) -> Result<Option<serde_json::Value>, String> {
        if !self.enabled {
            return Ok(None);
        }
        let collector = self
            .collector
            .try_lock()
            .map_err(|_| "capture diagnostic store unavailable")?;
        if let Some(failure) = collector.failure {
            return Err(failure.into());
        }
        if !collector.capture_ids_bounded {
            return Err("capture diagnostic identity exceeds 128 bytes".into());
        }
        let stats = collector.store.stats();
        Ok(Some(serde_json::json!({
            "run": stats.run.0,
            "source": stats.source.0,
            "generation": stats.generation,
            "next_sequence": stats.next_sequence,
        })))
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

/// Product-local capture identity and read-only owner handle. Each request
/// reuses this run; the callback samples the UI and persistence only at seal.
#[derive(Clone)]
pub(crate) struct CaptureContext {
    run: Result<String, &'static str>,
    desktop: std::rc::Rc<std::cell::RefCell<crate::Desktop>>,
}

impl CaptureContext {
    pub(crate) fn new(
        enabled: bool,
        desktop: std::rc::Rc<std::cell::RefCell<crate::Desktop>>,
    ) -> Option<Self> {
        if !enabled {
            return None;
        }
        static RUN: AtomicU64 = AtomicU64::new(0);
        let run = (|| {
            let ordinal = RUN
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map_err(|_| "capture run identity exhausted")?;
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "capture run clock unavailable")?
                .as_nanos();
            Ok(format!(
                "redshank-capture-{}-{time}-{ordinal}",
                std::process::id()
            ))
        })();
        Some(Self { run, desktop })
    }

    pub(crate) fn observer(&self) -> mesquite::CaptureObserver<crate::scenario::Product> {
        let capture = self.clone();
        mesquite::CaptureObserver {
            // Failure is propagated by the observer rather than silently
            // selecting legacy uncorrelated capture behavior.
            run: self
                .run
                .clone()
                .unwrap_or_else(|_| "redshank-capture-unavailable".into()),
            observe: Box::new(move |ctx, _presentation| capture.seal(&ctx.runner.state().surface)),
        }
    }

    pub(crate) fn seal(
        &self,
        state: &redshank_surfaces::RedshankSurfaceState,
    ) -> Result<mesquite::CaptureProjection, String> {
        self.run.as_ref().map_err(|error| (*error).to_owned())?;
        let desktop = self
            .desktop
            .try_borrow()
            .map_err(|_| "capture persistence owner unavailable")?;
        capture_projection(state, &desktop.persistence)
    }
}

/// Fixed schema: no text, paths, feed/item/note identities, raw failures or
/// editable content. Counts are scalar, so collection size does not grow this
/// projection. Persistence is owner-time state, distinct from rendered UI.
fn capture_projection(
    state: &redshank_surfaces::RedshankSurfaceState,
    persistence: &crate::Persistence,
) -> Result<mesquite::CaptureProjection, String> {
    use crate::scenario::{layout_name, mode_name, notes_filter_name, seed_name};
    let ui = serde_json::json!({
        "tab": state.active_tab.label(),
        "transport": state.compact.transport.micro_word(),
        "layout": layout_name(state.layout),
        "scene": state.scene.label(),
        "seed": seed_name(state.seed),
        "mode": mode_name(state.mode),
        "notes_filter": notes_filter_name(state.notes_filter),
        "queue_count": state.queue.len(),
        "note_count": state.notes.len(),
        "item_count": state.items.len(),
        "feed_count": state.feeds.len(),
        "session_count": state.sessions.len(),
        "recording": state.compact.recording.is_some(),
        "now_playing": state.compact.now_playing.is_some(),
        "text_capture": state.text_capture.is_some(),
        "notice_present": state.notice.is_some(),
        "transcript_present": state.transcript.is_some(),
    });
    let fields = ui
        .as_object()
        .expect("fixed UI object")
        .iter()
        .map(|(key, value)| {
            (
                key.replace('_', "-"),
                match value {
                    serde_json::Value::String(value) => value.clone(),
                    other => other.to_string(),
                },
            )
        })
        .collect();
    Ok(mesquite::CaptureProjection {
        fields,
        viewport: None,
        product: serde_json::json!({
            "schema_version": 1,
            "rendered_ui": ui,
            "persistence_at_seal": {
                "dirty_revision": persistence.revision,
                "durable_revision": persistence.durable,
                "in_flight_revision": persistence.in_flight,
                "failed": persistence.failed,
            },
            "diagnostic_admission_cut": persistence.diagnostics.capture_cut()?,
            "semantic_revision": null,
            "operation_cause": null,
        }),
    })
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

    #[test]
    fn capture_cut_is_admission_context_without_consuming_a_reader() {
        let diagnostics = Diagnostics::new(
            true,
            "capture-test".into(),
            RetentionLimits {
                max_records: 4,
                max_bytes: 4096,
                max_age: Duration::from_secs(300),
            },
        );
        let mut cursor = diagnostics.cursor().unwrap();
        diagnostics.record(SaveObservation::new(Phase::DispatchRequested, 7), None);
        let cut = diagnostics.capture_cut().unwrap().unwrap();
        assert_eq!(cut["run"], "capture-test");
        assert_eq!(cut["source"], "redshank.persistence");
        assert_eq!(cut["next_sequence"], 2);
        assert_eq!(cursor.next_sequence(), 1);
        assert_eq!(diagnostics.read(&mut cursor, 4).unwrap().records.len(), 1);
    }

    #[test]
    fn capture_cut_rejects_conflicts_and_unbounded_identity() {
        let limits = RetentionLimits {
            max_records: 4,
            max_bytes: 4096,
            max_age: Duration::from_secs(300),
        };
        let diagnostics = Diagnostics::new(true, "capture-test".into(), limits);
        let guard = diagnostics.collector.lock().unwrap();
        assert!(diagnostics.capture_cut().is_err());
        drop(guard);
        let oversized = Diagnostics::new(true, RunId("x".repeat(129)), limits);
        assert!(oversized.capture_cut().is_err());
        let disabled = Diagnostics::new(false, "disabled".into(), limits);
        assert_eq!(disabled.capture_cut().unwrap(), None);
    }
}
