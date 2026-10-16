//! Control-side custody of completed Set takes and their sealed companions.
//!
//! A companion is immutable and verified before the session can refer to it.
//! Session replacement is the commit point; failed publication retains both
//! the previous session and the live take. Unavailable references are retained
//! until an explicit Clear, rather than silently saving an empty capture bank.

use std::sync::Arc;
use std::time::{Duration, Instant};

use woodshed_audio::SampleBuffer;
use woodshed_core::audio::AudioBackend;
use woodshed_core::loop_capture::{
    CAPTURE_VERSION, LoopCaptureCodec, LoopCaptureManifest, LoopCaptureRef,
};
use woodshed_core::loop_plan::LoopSegmentId;
use woodshed_core::storage::SessionStore;
use woodshed_views::persona::PracticeSeal;
use woodshed_views::stage::UiState;

use crate::audio::{CaptureSnapshot, CpalBackend};
use crate::shared::Shared;
use crate::storage::HostBackend;

mod codec_worker;
use codec_worker::{CodecWorker, PreparedTake};

#[derive(Default)]
pub struct CapturePersistence {
    manifest: LoopCaptureManifest,
    namespace: Option<String>,
    buffers: Vec<CaptureSnapshot>,
    unavailable: Vec<LoopCaptureRef>,
    blocked: Option<String>,
    last_attempt: Option<Instant>,
    cleared: bool,
    audio_absent: bool,
    worker: CodecWorker,
    awaiting_commit: bool,
}

fn same_take(a: &CaptureSnapshot, b: &CaptureSnapshot) -> bool {
    a.id == b.id
        && a.legacy_clock == b.legacy_clock
        && a.bpm == b.bpm
        && a.beats == b.beats
        && a.bars == b.bars
        && a.buffer.sample_rate_hz == b.buffer.sample_rate_hz
        && Arc::ptr_eq(&a.buffer.data, &b.buffer.data)
}

impl CapturePersistence {
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.worker.set_wake(wake);
    }

    /// Includes a finished codec result awaiting custody publication. Failed
    /// publication keeps live PCM and this reservation until retry or reset.
    pub fn is_busy(&self) -> bool {
        self.worker.is_busy() || self.awaiting_commit
    }
    pub fn preparation_busy(&self) -> bool {
        self.worker.is_busy()
    }

    pub(crate) fn permit_export(
        &self,
        plan: Option<&woodshed_core::loop_plan::LoopPlan>,
    ) -> Result<(), String> {
        if let Some(error) = &self.blocked {
            if plan.is_some() {
                return Err(format!(
                    "Resolve the captured-loop catalog before exporting: {error}"
                ));
            }
        }
        if plan.is_some_and(|plan| {
            self.unavailable
                .iter()
                .any(|take| plan.index_of(take.id).is_some())
        }) {
            return Err("A captured loop in this backing is unavailable or damaged. Restore it or explicitly Clear stored loop before exporting WAV.".into());
        }
        Ok(())
    }

    fn restore(
        storage: Option<&SessionStore<HostBackend>>,
        manifest: &LoopCaptureManifest,
        backend: &mut CpalBackend,
    ) -> Result<Self, String> {
        let mut state = Self {
            manifest: manifest.clone(),
            namespace: manifest.namespace.clone(),
            ..Default::default()
        };
        if let Err(error) = manifest.validate() {
            state.blocked = Some(error);
            backend.restore_loop_captures(Vec::new())?;
            return Ok(state);
        }
        for reference in &manifest.takes {
            let samples = storage
                .ok_or_else(|| "Choose a persona to restore captured loops.".to_string())
                .and_then(|store| {
                    store.try_read_capture(
                        manifest
                            .namespace
                            .as_deref()
                            .expect("validated capture namespace"),
                        reference,
                    )
                });
            match samples {
                Ok(samples)
                    if woodshed_audio::Bar {
                        legacy_clock: reference.legacy_clock(),
                        bpm: reference.bpm,
                        time_signature: woodshed_audio::TimeSignature::new(reference.beats, 4),
                        length: reference.bars,
                        ..Default::default()
                    }
                    .duration_samples(reference.sample_rate as f32)
                        == reference.frames as u64 =>
                {
                    state.buffers.push(CaptureSnapshot {
                        legacy_clock: reference.legacy_clock(),
                        id: reference.id,
                        bpm: reference.bpm,
                        beats: reference.beats,
                        bars: reference.bars,
                        buffer: SampleBuffer::new(samples, reference.sample_rate),
                    })
                },
                Err(error) => {
                    eprintln!(
                        "[woodshed] unavailable Card loop {:?}: {error}",
                        reference.id
                    );
                    state.unavailable.push(reference.clone());
                },
                Ok(_) => {
                    eprintln!(
                        "[woodshed] unavailable Card loop {:?}: PCM length differs from captured clock",
                        reference.id
                    );
                    state.unavailable.push(reference.clone());
                },
            }
        }
        backend.restore_loop_captures(state.buffers.clone())?;
        Ok(state)
    }

    pub fn forget(&mut self, id: LoopSegmentId) {
        self.unavailable.retain(|reference| reference.id != id);
        self.buffers.retain(|take| take.id != id);
        self.cleared = true;
    }

    pub fn permit_record(
        &self,
        plan: &woodshed_core::loop_plan::LoopPlan,
        rate: u32,
        takes: Vec<CaptureSnapshot>,
    ) -> Result<(), String> {
        if self.blocked.is_some() {
            return Err("Resolve the unavailable captured-loop catalog before recording.".into());
        }
        if self
            .unavailable
            .iter()
            .any(|reference| plan.index_of(reference.id).is_some())
        {
            return Err("Clear the unavailable captured Card loop before recording this Set. Its saved reference is retained.".into());
        }
        let mut retained: std::collections::BTreeMap<_, _> = self
            .unavailable
            .iter()
            .map(|reference| (reference.id, reference.clone()))
            .collect();
        for take in takes {
            retained.insert(
                take.id,
                LoopCaptureRef {
                    clock: take.clock_field(),
                    id: take.id,
                    bpm: take.bpm,
                    beats: take.beats,
                    bars: take.bars,
                    sample_rate: take.buffer.sample_rate_hz,
                    frames: take.buffer.len(),
                    codec: LoopCaptureCodec::RawF32V1,
                    digest: "0".repeat(64),
                },
            );
        }
        // A take can continue through every loaded segment, so reserve the
        // entire plan rather than just the Card where recording starts.
        for segment in &plan.segments {
            let frames = woodshed_audio::Bar {
                bpm: segment.bpm,
                time_signature: woodshed_audio::TimeSignature::new(segment.beats, 4),
                length: segment.bars,
                ..Default::default()
            }
            .duration_samples(rate as f32) as usize;
            retained.insert(
                segment.id,
                LoopCaptureRef {
                    clock: Some(woodshed_core::loop_capture::PER_BEAT_CLOCK.into()),
                    id: segment.id,
                    bpm: segment.bpm,
                    beats: segment.beats,
                    bars: segment.bars,
                    sample_rate: rate,
                    frames,
                    codec: LoopCaptureCodec::RawF32V1,
                    digest: "0".repeat(64),
                },
            );
        }
        LoopCaptureManifest {
            namespace: Some("0".repeat(32)),
            takes: retained.into_values().collect(),
            ..Default::default()
        }
        .validate()
        .map_err(|error| format!("Clear retained Card loops before recording this Set: {error}"))
    }

    fn notice(&self) -> Option<String> {
        if let Some(error) = &self.blocked {
            return Some(format!(
                "Captured-loop references cannot be opened: {error} The saved session is retained."
            ));
        }
        if self.audio_absent && !self.manifest.takes.is_empty() {
            return Some("Captured Card loops are retained; audio output is unavailable.".into());
        }
        if !self.unavailable.is_empty() {
            return Some(format!(
                "{} captured Card loop(s) are unavailable or damaged. Their saved references are retained. Use Clear stored loop to remove a reference.",
                self.unavailable.len()
            ));
        }
        (!self.manifest.takes.is_empty())
            .then(|| format!("Card loops saved: {}", self.manifest.takes.len()))
    }

    fn needs_migration(&self) -> bool {
        // Only successfully hydrated legacy takes may be encoded. Unavailable
        // references retain their codec/address and can migrate if recovered
        // later, even when other valid takes already published a V2 catalog.
        self.buffers.iter().any(|take| {
            self.reference_for(take)
                .is_some_and(|reference| reference.codec == LoopCaptureCodec::LegacyRawF32V1)
        })
    }

    fn reference_for(&self, take: &CaptureSnapshot) -> Option<&LoopCaptureRef> {
        self.buffers
            .iter()
            .find(|old| same_take(old, take))
            .and_then(|_| {
                self.manifest
                    .takes
                    .iter()
                    .find(|reference| reference.id == take.id)
            })
    }

    fn changed(&self, takes: &[CaptureSnapshot]) -> bool {
        self.needs_migration()
            || self.cleared
            || self.buffers.len() != takes.len()
            || takes
                .iter()
                .any(|take| !self.buffers.iter().any(|old| same_take(old, take)))
    }

    fn retry_ready(&self) -> bool {
        self.last_attempt
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(1))
    }

    fn admit(&self, sealed: bool, takes: &[CaptureSnapshot]) -> Result<(), String> {
        if let Some(error) = &self.blocked {
            return Err(format!("{error} The saved session is retained."));
        }
        if (!takes.is_empty() || !self.manifest.is_empty()) && !sealed {
            return Err("Choose an unlocked persona to save captured loops. Live takes remain available in this session.".into());
        }
        if self
            .unavailable
            .iter()
            .any(|reference| takes.iter().any(|take| take.id == reference.id))
        {
            return Err(
                "Clear the unavailable Card loop before replacing its saved reference.".into(),
            );
        }
        // Decoded-frame admission must finish before any worker request.
        let mut manifest = LoopCaptureManifest {
            namespace: Some(self.namespace.clone().unwrap_or_else(|| "0".repeat(32))),
            takes: self.unavailable.clone(),
            ..Default::default()
        };
        for take in takes {
            manifest.takes.push(LoopCaptureRef {
                clock: take.clock_field(),
                id: take.id,
                bpm: take.bpm,
                beats: take.beats,
                bars: take.bars,
                sample_rate: take.buffer.sample_rate_hz,
                frames: take.buffer.len(),
                codec: LoopCaptureCodec::RawF32V1,
                digest: "0".repeat(64),
            });
        }
        manifest.validate()
    }

    fn required(&self, takes: &[CaptureSnapshot]) -> Vec<CaptureSnapshot> {
        takes
            .iter()
            .filter(|take| {
                self.reference_for(take)
                    .is_none_or(|reference| reference.codec == LoopCaptureCodec::LegacyRawF32V1)
            })
            .cloned()
            .collect()
    }

    fn persist_async(
        &mut self,
        storage: &SessionStore<HostBackend>,
        sealed: bool,
        takes: Vec<CaptureSnapshot>,
        ui: &mut UiState,
    ) -> Result<(), String> {
        // A failed policy-only rewrite may close with its verified legacy
        // references. New/replaced live PCM must remain available until its
        // companion and current session references are confirmed.
        self.awaiting_commit = takes.iter().any(|take| self.reference_for(take).is_none());
        if let Err(error) = self.admit(sealed, &takes) {
            // Drain a finished preparation even if the custody seal changed;
            // keep its cached bytes, but never publish under the rejected seal.
            let _ = self.worker.prepare(&ui.audio_session, &takes, Vec::new());
            self.last_attempt = Some(Instant::now());
            return Err(error);
        }
        let required = self.required(&takes);
        // Temporarily separate the lane/cache from the transaction borrow. A
        // default lane starts no thread, and the original is always restored.
        let mut worker = std::mem::take(&mut self.worker);
        let prepared = worker.prepare(&ui.audio_session, &takes, required);
        let result = match prepared {
            Ok(true) => {
                let result = self.persist_prepared(storage, sealed, takes, ui, worker.prepared());
                if result.is_ok() {
                    // Accepted explicit-codec references are now authoritative;
                    // only unconfirmed publication needs encoded retry storage.
                    worker.clear_prepared();
                }
                result
            },
            Ok(false) => {
                // Ordinary edits remain durable while the previous capture
                // manifest is the only fully verified set of references.
                let mut session = ui.to_persisted();
                session.loop_captures = self.manifest.clone();
                serde_json::to_string(&session).map_err(|error| error.to_string()).and_then(|json| {
                    storage.try_save(&json).map_err(|error| error.to_string())
                }).map(|()| {
                    ui.loop_capture_notice = Some("Preparing captured Card loops for storage. Live takes remain available.".into());
                })
            },
            Err(error) => Err(error),
        };
        self.worker = worker;
        if result.is_err() {
            self.last_attempt = Some(Instant::now());
        }
        result
    }

    // Exact CPU/fixture helper: production always uses the bounded worker.
    #[cfg(test)]
    fn persist(
        &mut self,
        storage: &SessionStore<HostBackend>,
        sealed: bool,
        takes: Vec<CaptureSnapshot>,
        ui: &mut UiState,
    ) -> Result<(), String> {
        self.admit(sealed, &takes)?;
        let prepared = self
            .required(&takes)
            .into_iter()
            .map(|take| {
                let encoded = woodshed_core::loop_capture::encode_capture(
                    take.buffer.sample_rate_hz,
                    &take.buffer.data,
                )?;
                Ok(PreparedTake {
                    take,
                    encoded: Arc::new(encoded),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.persist_prepared(storage, sealed, takes, ui, &prepared)
    }

    fn persist_prepared(
        &mut self,
        storage: &SessionStore<HostBackend>,
        sealed: bool,
        takes: Vec<CaptureSnapshot>,
        ui: &mut UiState,
        prepared: &[PreparedTake],
    ) -> Result<(), String> {
        self.last_attempt = Some(Instant::now());
        if let Some(error) = &self.blocked {
            return Err(format!("{error} The saved session is retained."));
        }
        let durable = !takes.is_empty() || !self.manifest.is_empty();
        if durable && !sealed {
            return Err("Choose an unlocked persona to save captured loops. Live takes remain available in this session.".into());
        }
        if self
            .unavailable
            .iter()
            .any(|reference| takes.iter().any(|take| take.id == reference.id))
        {
            return Err(
                "Clear the unavailable Card loop before replacing its saved reference.".into(),
            );
        }
        let mut manifest = LoopCaptureManifest {
            // Keep an entirely unavailable V1 catalog eligible for migration if
            // its companions can be restored later. An explicit raw codec records
            // that compression policy has already selected raw fallback.
            version: if self.manifest.version == 1
                && takes.is_empty()
                && !self.unavailable.is_empty()
            {
                1
            } else if self.manifest.version <= 2
                && takes.iter().all(|take| take.legacy_clock)
                && self.unavailable.iter().all(|reference| reference.legacy_clock())
            {
                2
            } else {
                CAPTURE_VERSION
            },
            namespace: self.namespace.clone(),
            takes: self
                .unavailable
                .iter()
                .filter(|reference| !takes.iter().any(|take| take.id == reference.id))
                .cloned()
                .collect(),
            ..Default::default()
        };
        if !takes.is_empty() && manifest.namespace.is_none() {
            let mut nonce = [0u8; 16];
            getrandom::fill(&mut nonce)
                .map_err(|error| format!("Cannot create captured-loop storage scope: {error}"))?;
            manifest.namespace = Some(nonce.iter().map(|byte| format!("{byte:02x}")).collect());
            self.namespace = manifest.namespace.clone();
        }
        for take in &takes {
            let old = self.reference_for(take);
            let digest = old.map_or_else(|| "0".repeat(64), |reference| reference.digest.clone());
            manifest.takes.push(LoopCaptureRef {
                clock: take.clock_field(),
                id: take.id,
                bpm: take.bpm,
                beats: take.beats,
                bars: take.bars,
                sample_rate: take.buffer.sample_rate_hz,
                frames: take.buffer.len(),
                codec: old.map_or(LoopCaptureCodec::RawF32V1, |reference| {
                    reference.codec.clone()
                }),
                digest,
            });
        }
        manifest.takes.sort_by_key(|reference| reference.id);
        if manifest.takes.is_empty() {
            manifest.namespace = None;
        }
        manifest.validate()?;
        // Catalog admission precedes any sample encoding or companion write.
        for take in &takes {
            if self
                .reference_for(take)
                .is_none_or(|reference| reference.codec == LoopCaptureCodec::LegacyRawF32V1)
            {
                let encoded = prepared
                    .iter()
                    .find(|prepared| same_take(&prepared.take, take))
                    .ok_or("Captured-loop codec preparation is not current.")?;
                let (codec, digest, frames) = storage.try_write_encoded_capture(
                    manifest
                        .namespace
                        .as_deref()
                        .expect("capture namespace prepared"),
                    take.buffer.sample_rate_hz,
                    take.buffer.len(),
                    &encoded.encoded,
                )?;
                let reference = manifest
                    .takes
                    .iter_mut()
                    .find(|reference| reference.id == take.id)
                    .expect("validated take reference");
                reference.codec = codec;
                reference.digest = digest;
                reference.frames = frames;
            }
        }
        let mut session = ui.to_persisted();
        session.loop_captures = manifest.clone();
        let json = serde_json::to_string(&session).map_err(|error| error.to_string())?;
        storage.try_save(&json).map_err(|error| error.to_string())?;
        self.unavailable
            .retain(|reference| !takes.iter().any(|take| take.id == reference.id));
        self.manifest = manifest.clone();
        self.namespace = manifest.namespace.clone();
        self.buffers = takes;
        self.cleared = false;
        self.awaiting_commit = false;
        self.worker.clear_prepared();
        ui.loop_captures = manifest;
        ui.loop_capture_notice = self.notice();
        Ok(())
    }
}

/// Reset before hydrating incoming session-local ids. Backpressure keeps the
/// incoming session unadmitted until the reset and restoration can finish.
pub fn begin_session(shared: &mut Shared, ui: &UiState) -> Result<Option<Option<String>>, String> {
    if shared
        .last_audio_session
        .as_ref()
        .is_some_and(|last| Arc::ptr_eq(last, &ui.audio_session))
        && !(shared.capture_persistence.audio_absent && shared.backend.is_some())
    {
        return Ok(None);
    }
    let mut state = if let Some(backend) = shared.backend.as_mut() {
        backend.clear_loop_session();
        CapturePersistence::restore(shared.storage.as_ref(), &ui.loop_captures, backend)?
    } else {
        CapturePersistence {
            manifest: ui.loop_captures.clone(),
            namespace: ui.loop_captures.namespace.clone(),
            unavailable: ui.loop_captures.takes.clone(),
            blocked: ui.loop_captures.validate().err(),
            audio_absent: true,
            ..Default::default()
        }
    };
    let notice = state.notice();
    state.worker = std::mem::take(&mut shared.capture_persistence.worker);
    state.worker.cancel_epoch();
    shared.capture_persistence = state;
    shared.last_song = None;
    shared.last_loop_plan = None;
    shared.last_audio_session = Some(ui.audio_session.clone());
    Ok(Some(notice))
}

pub fn session_current(shared: &Shared, ui: &UiState) -> bool {
    shared
        .last_audio_session
        .as_ref()
        .is_some_and(|last| Arc::ptr_eq(last, &ui.audio_session))
        && !(shared.capture_persistence.audio_absent && shared.backend.is_some())
}

pub fn pending(shared: &Shared, ui: &UiState) -> bool {
    shared
        .last_audio_session
        .as_ref()
        .is_some_and(|last| Arc::ptr_eq(last, &ui.audio_session))
        && (shared.capture_persistence.worker.completion_ready()
            || (!shared.capture_persistence.worker.is_busy()
                && shared.capture_persistence.retry_ready()
                && shared.backend.as_ref().is_some_and(|backend| {
                    shared
                        .capture_persistence
                        .changed(&backend.capture_snapshot())
                })))
}

/// Used by dispatch, and by completion service without another user action.
pub fn persist(shared: &mut Shared, ui: &mut UiState) {
    if ui.practice_discard_close {
        return;
    }
    if shared
        .last_audio_session
        .as_ref()
        .is_none_or(|last| !Arc::ptr_eq(last, &ui.audio_session))
    {
        return;
    }
    let Some(storage) = shared.storage.as_ref() else {
        return;
    };
    let takes = shared.backend.as_ref().map_or_else(
        || shared.capture_persistence.buffers.clone(),
        CpalBackend::capture_snapshot,
    );
    ui.loop_retained_ids = takes.iter().map(|take| take.id).collect();
    let sealed = matches!(shared.seal, Some(PracticeSeal::Sealed { .. }));
    if let Err(error) = shared
        .capture_persistence
        .persist_async(storage, sealed, takes, ui)
    {
        storage.session_unconfirmed(error.clone());
        ui.loop_capture_notice = Some(format!(
            "Practice save is unconfirmed: {error} Live changes remain available."
        ));
    }
    crate::session::refresh_save_status(storage, ui);
}

/// Queue current capture preparation and let the host defer native close until
/// publication completes. No join or codec execution runs on the UI thread.
pub fn close_request(shared: &mut Shared, ui: &mut UiState) -> bool {
    if ui.practice_discard_close {
        ui.practice_close_pending = false;
        return false;
    }
    persist(shared, ui);
    if let Some(storage) = shared.storage.as_ref() {
        if let Ok(json) = serde_json::to_string(&ui.app_settings) {
            storage.save_settings(&json);
        }
        crate::session::refresh_save_status(storage, ui);
    }
    if shared.capture_persistence.preparation_busy() {
        return true;
    }
    ui.practice_close_pending = !ui.practice_saved || ui.practice_save_notice.is_some();
    if ui.practice_close_pending && ui.practice_save_notice.is_none() {
        ui.practice_save_notice = Some("This practice session has not been saved. Choose an unlocked persona to retain it, or explicitly discard it to close.".into());
    }
    ui.practice_close_pending
}

pub fn persist_if_changed(shared: &mut Shared, ui: &mut UiState) {
    if pending(shared, ui) {
        persist(shared, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use muniment::backend::WriteOp;
    use muniment::{Backend, MemoryBackend, StoreError};
    use woodshed_core::sealed_backend::SealedBackend;
    use woodshedding::rehearsal::Hold;

    #[test]
    fn capture_native_scenarios_parse_with_shared_driver() {
        for scenario in [
            include_str!("../../../scenarios/mere_take_seed.scn"),
            include_str!("../../../scenarios/mere_folds.scn"),
            include_str!("../../../scenarios/mere_folds_reopen.scn"),
            include_str!("../../../scenarios/mere_take_reopen.scn"),
            include_str!("../../../scenarios/set_capture_seed.scn"),
            include_str!("../../../scenarios/set_capture_reopen.scn"),
            include_str!("../../../scenarios/set_capture_cleared_reopen.scn"),
            include_str!("../../../scenarios/set_capture_missing_reopen.scn"),
        ] {
            taproot::Scenario::parse(scenario).unwrap();
        }
    }

    fn fixture() -> (UiState, CaptureSnapshot) {
        let mut ui = UiState::new();
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        ui.stage_current(None);
        ui.stage_current(None);
        for card in &mut ui.set.cards {
            card.timing.hold = Hold::Bars(1);
        }
        ui.looper.beats = 1;
        ui.loop_working_set(ui.working_sets.active_id);
        let plan = ui.requested_loop_plan().unwrap();
        let segment = &plan.segments[0];
        let frames = (segment.duration_secs() * 48_000.0) as usize;
        let samples = (0..frames)
            .map(|frame| match frame % 4 {
                0 => 0.125,
                1 => -0.25,
                2 => -0.0,
                _ => 0.5,
            })
            .collect();
        let take = CaptureSnapshot {
            legacy_clock: false,
            id: segment.id,
            bpm: segment.bpm,
            beats: segment.beats,
            bars: segment.bars,
            buffer: SampleBuffer::new(samples, 48_000),
        };
        (ui, take)
    }

    const LEGACY_NAMESPACE: &str = "0123456789abcdef0123456789abcdef";

    fn seed_legacy(
        storage: &SessionStore<HostBackend>,
        ui: &mut UiState,
        takes: &[CaptureSnapshot],
    ) -> LoopCaptureManifest {
        let mut manifest = LoopCaptureManifest {
            version: 1,
            namespace: Some(LEGACY_NAMESPACE.into()),
            takes: Vec::new(),
        };
        for take in takes {
            let (digest, frames) = storage
                .try_write_capture(
                    LEGACY_NAMESPACE,
                    take.buffer.sample_rate_hz,
                    &take.buffer.data,
                )
                .unwrap();
            manifest.takes.push(LoopCaptureRef {
                clock: None,
                id: take.id,
                bpm: take.bpm,
                beats: take.beats,
                bars: take.bars,
                sample_rate: take.buffer.sample_rate_hz,
                frames,
                codec: LoopCaptureCodec::LegacyRawF32V1,
                digest,
            });
        }
        manifest.validate().unwrap();
        ui.loop_captures = manifest.clone();
        storage
            .try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
            .unwrap();
        manifest
    }

    fn bits(samples: &[f32]) -> Vec<u32> {
        samples.iter().map(|sample| sample.to_bits()).collect()
    }

    fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("memory fixture unexpectedly awaited external work"),
        }
    }

    #[test]
    fn hydrated_v1_capture_migrates_with_exact_sealed_readback_and_retains_old_blob() {
        let inner = MemoryBackend::default();
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(SealedBackend::new(inner.clone(), [53; 32])));
        let (mut ui, take) = fixture();
        let old = seed_legacy(&storage, &mut ui, &[take.clone()]);
        let old_key =
            woodshed_core::loop_capture::pcm_key(LEGACY_NAMESPACE, &old.takes[0].digest).unwrap();
        let old_blob = ready(inner.get(&old_key)).unwrap().unwrap();
        let mut backend = CpalBackend::offline_for_test();
        let mut state = CapturePersistence::restore(Some(&storage), &old, &mut backend).unwrap();
        assert!(state.needs_migration());
        assert!(state.changed(&backend.capture_snapshot()));
        state
            .persist(&storage, true, backend.capture_snapshot(), &mut ui)
            .unwrap();
        assert_eq!(ui.loop_captures.version, 2);
        assert_eq!(ui.loop_captures.namespace, old.namespace);
        let reference = &ui.loop_captures.takes[0];
        assert_eq!(reference.codec, LoopCaptureCodec::WavpackF32V1);
        assert_eq!(reference.id, old.takes[0].id);
        assert_eq!(reference.bpm, old.takes[0].bpm);
        assert_eq!(reference.beats, old.takes[0].beats);
        assert_eq!(reference.bars, old.takes[0].bars);
        assert_eq!(reference.sample_rate, old.takes[0].sample_rate);
        assert_eq!(reference.frames, old.takes[0].frames);
        assert_ne!(reference.digest, old.takes[0].digest);
        assert_eq!(
            bits(
                &storage
                    .try_read_capture(LEGACY_NAMESPACE, reference)
                    .unwrap()
            ),
            bits(&take.buffer.data)
        );
        assert_eq!(ready(inner.get(&old_key)).unwrap().unwrap(), old_blob);
        let reopened = woodshed_core::storage::decode_session(&storage.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(reopened.loop_captures, ui.loop_captures);
        assert!(!state.needs_migration());
        assert!(!state.changed(&backend.capture_snapshot()));
    }

    struct CountCompanions {
        inner: MemoryBackend,
        operations: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Backend for CountCompanions {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
            if key.starts_with("loop-pcm/") {
                self.operations
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            self.inner.get(key).await
        }
        async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
            if key.starts_with("loop-pcm/") {
                self.operations
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            self.inner.put(key, bytes).await
        }
        async fn delete(&self, key: &str) -> Result<(), StoreError> {
            self.inner.delete(key).await
        }
        async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
            self.inner.list(prefix).await
        }
        async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
            self.inner.scan(start, end).await
        }
        async fn apply(&self, _: &[WriteOp]) -> Result<(), StoreError> {
            panic!("capture persistence must not assume multi-file apply is atomic")
        }
    }

    #[test]
    fn raw_fallback_records_policy_once_and_does_not_touch_companions_again() {
        let operations = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(CountCompanions {
            inner: MemoryBackend::default(),
            operations: operations.clone(),
        }));
        let (mut ui, mut take) = fixture();
        // Full finite f32 bit entropy keeps this deterministic noise at raw
        // size. Explicit fallback must not be re-evaluated after reopening.
        let mut random = 0x1234_5678u32;
        let samples = (0..8192)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                let bits = if random & 0x7f80_0000 == 0x7f80_0000 {
                    random ^ 0x0080_0000
                } else {
                    random
                };
                f32::from_bits(bits)
            })
            .collect();
        take.bpm = 60.0 * 1000.0 / 8192.0;
        take.beats = 1;
        take.bars = 1;
        take.buffer = SampleBuffer::new(samples, 1000);
        let mut state = CapturePersistence::default();
        state
            .persist(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        assert_eq!(ui.loop_captures.version, CAPTURE_VERSION);
        assert_eq!(ui.loop_captures.takes[0].codec, LoopCaptureCodec::RawF32V1);
        let accepted = ui.loop_captures.clone();
        let before = operations.load(std::sync::atomic::Ordering::Relaxed);
        assert!(before > 0);
        ui.set.cards[1].label = "Current ordinary edit".into();
        state
            .persist(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        assert_eq!(
            operations.load(std::sync::atomic::Ordering::Relaxed),
            before
        );
        assert_eq!(ui.loop_captures, accepted);
        let mut backend = CpalBackend::offline_for_test();
        let mut reopened =
            CapturePersistence::restore(Some(&storage), &accepted, &mut backend).unwrap();
        assert!(!reopened.needs_migration());
        let after_restore = operations.load(std::sync::atomic::Ordering::Relaxed);
        reopened
            .persist(&storage, true, backend.capture_snapshot(), &mut ui)
            .unwrap();
        assert_eq!(
            operations.load(std::sync::atomic::Ordering::Relaxed),
            after_restore
        );
        assert_eq!(
            bits(&backend.capture_snapshot()[0].buffer.data),
            bits(&take.buffer.data)
        );
    }

    #[test]
    fn mixed_legacy_unavailable_reference_can_recover_and_migrate_inside_v2() {
        let inner = MemoryBackend::default();
        let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(inner.clone()));
        let (mut ui, first) = fixture();
        let mut second = first.clone();
        let plan = ui.requested_loop_plan().unwrap();
        second.id = plan.segments[1].id;
        let old = seed_legacy(&storage, &mut ui, &[first.clone(), second.clone()]);
        let missing_key =
            woodshed_core::loop_capture::pcm_key(LEGACY_NAMESPACE, &old.takes[1].digest).unwrap();
        // Different PCM is needed for distinct content addresses.
        assert_eq!(old.takes[0].digest, old.takes[1].digest);
        // Preserve the first take's bytes but publish a missing address for the
        // second owner; later recovery supplies that exact original raw blob.
        let mut mixed = old.clone();
        let mut samples = second.buffer.data.as_ref().clone();
        samples[0] = 0.0625;
        second.buffer = SampleBuffer::new(samples, 48_000);
        let raw = woodshed_core::loop_capture::encode_pcm(48_000, &second.buffer.data).unwrap();
        mixed.takes[1].digest = woodshed_core::loop_capture::pcm_digest(&raw);
        let recovery_key =
            woodshed_core::loop_capture::pcm_key(LEGACY_NAMESPACE, &mixed.takes[1].digest).unwrap();
        assert_ne!(recovery_key, missing_key);
        ui.loop_captures = mixed.clone();
        let mut backend = CpalBackend::offline_for_test();
        let mut state = CapturePersistence::restore(Some(&storage), &mixed, &mut backend).unwrap();
        assert_eq!(state.buffers.len(), 1);
        assert_eq!(state.unavailable, vec![mixed.takes[1].clone()]);
        state
            .persist(&storage, true, backend.capture_snapshot(), &mut ui)
            .unwrap();
        assert_eq!(ui.loop_captures.version, 2);
        let retained = ui
            .loop_captures
            .takes
            .iter()
            .find(|r| r.id == second.id)
            .unwrap();
        assert_eq!(retained, &mixed.takes[1]);
        ready(inner.put(&recovery_key, &raw)).unwrap();
        let mut recovered_backend = CpalBackend::offline_for_test();
        let mut recovered =
            CapturePersistence::restore(Some(&storage), &ui.loop_captures, &mut recovered_backend)
                .unwrap();
        assert!(recovered.needs_migration());
        assert!(recovered.unavailable.is_empty());
        recovered
            .persist(
                &storage,
                true,
                recovered_backend.capture_snapshot(),
                &mut ui,
            )
            .unwrap();
        assert!(
            ui.loop_captures
                .takes
                .iter()
                .all(|r| r.codec == LoopCaptureCodec::WavpackF32V1)
        );
        assert_eq!(
            bits(
                &storage
                    .try_read_capture(
                        LEGACY_NAMESPACE,
                        ui.loop_captures
                            .takes
                            .iter()
                            .find(|r| r.id == second.id)
                            .unwrap(),
                    )
                    .unwrap()
            ),
            bits(&second.buffer.data)
        );
        recovered_backend.clear_stored_capture(second.id).unwrap();
        recovered.forget(second.id);
        recovered
            .persist(
                &storage,
                true,
                recovered_backend.capture_snapshot(),
                &mut ui,
            )
            .unwrap();
        assert_eq!(ui.loop_captures.takes.len(), 1);
        assert_eq!(ui.loop_captures.takes[0].id, first.id);
    }

    fn completion_wake(state: &mut CapturePersistence) -> std::sync::mpsc::Receiver<()> {
        let (sender, receiver) = std::sync::mpsc::channel();
        state.set_wake(Arc::new(move || {
            let _ = sender.send(());
        }));
        receiver
    }

    fn await_completion(receiver: &std::sync::mpsc::Receiver<()>) {
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("bounded codec completion wake");
    }

    #[test]
    fn worker_prepares_without_blocking_authoring_and_publishes_current_edits_on_wake() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let (mut ui, take) = fixture();
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Mutex::new(release_rx);
        let control_thread = std::thread::current().id();
        let mut state = CapturePersistence::default();
        state.worker = CodecWorker::with_encoder(Arc::new(move |rate, samples| {
            assert_ne!(std::thread::current().id(), control_thread);
            started.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
            woodshed_core::loop_capture::encode_capture(rate, samples)
        }));
        let wakes = completion_wake(&mut state);
        state
            .persist_async(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        await_completion(&started_rx);
        assert!(state.is_busy());
        assert!(ui.loop_captures.is_empty());
        let provisional = woodshed_core::storage::decode_session(&storage.load().unwrap())
            .unwrap()
            .session;
        assert!(provisional.loop_captures.is_empty());
        ui.set.cards[1].label = "Latest edit while compression waits".into();
        state
            .persist_async(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        let ordinary = woodshed_core::storage::decode_session(&storage.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(ordinary.set.cards[1].label, ui.set.cards[1].label);
        assert!(ordinary.loop_captures.is_empty());
        release.send(()).unwrap();
        await_completion(&wakes);
        assert!(state.worker.completion_ready());
        state
            .persist_async(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        assert!(!state.is_busy());
        assert!(state.worker.prepared().is_empty());
        let published = woodshed_core::storage::decode_session(&storage.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(
            published.set.cards[1].label,
            "Latest edit while compression waits"
        );
        assert_eq!(published.loop_captures.takes.len(), 1);
        assert_eq!(
            bits(
                &storage
                    .try_read_capture(
                        published.loop_captures.namespace.as_deref().unwrap(),
                        &published.loop_captures.takes[0],
                    )
                    .unwrap()
            ),
            bits(&take.buffer.data)
        );
    }

    #[test]
    fn worker_cache_survives_session_failure_and_close_holds_only_new_live_pcm() {
        let inner = MemoryBackend::default();
        let good: SessionStore<HostBackend> = SessionStore::new(Box::new(inner.clone()));
        let failing: SessionStore<HostBackend> = SessionStore::new(Box::new(Failing {
            inner,
            key: "session",
            refuse_session_read: false,
        }));
        let (mut ui, take) = fixture();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let mut state = CapturePersistence::default();
        state.worker = CodecWorker::with_encoder(Arc::new(move |rate, samples| {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            woodshed_core::loop_capture::encode_capture(rate, samples)
        }));
        let wakes = completion_wake(&mut state);
        state
            .persist_async(&good, true, vec![take.clone()], &mut ui)
            .unwrap();
        let previous = good.load().unwrap();
        await_completion(&wakes);
        assert!(
            state
                .persist_async(&good, false, vec![take.clone()], &mut ui)
                .is_err()
        );
        assert!(!state.worker.is_busy());
        assert!(state.is_busy());
        assert_eq!(state.worker.prepared().len(), 1);
        assert_eq!(good.load().unwrap(), previous);
        assert!(
            state
                .persist_async(&failing, true, vec![take.clone()], &mut ui)
                .is_err()
        );
        assert!(state.is_busy());
        assert_eq!(state.worker.prepared().len(), 1);
        assert_eq!(good.load().unwrap(), previous);
        assert!(ui.loop_captures.is_empty());
        state
            .persist_async(&good, true, vec![take], &mut ui)
            .unwrap();
        assert!(!state.is_busy());
        assert!(state.worker.prepared().is_empty());
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);

        let (mut legacy_ui, legacy_take) = fixture();
        let legacy = seed_legacy(&good, &mut legacy_ui, &[legacy_take.clone()]);
        let mut backend = CpalBackend::offline_for_test();
        let mut migration =
            CapturePersistence::restore(Some(&good), &legacy, &mut backend).unwrap();
        let wakes = completion_wake(&mut migration);
        migration
            .persist_async(&good, true, backend.capture_snapshot(), &mut legacy_ui)
            .unwrap();
        assert!(migration.is_busy());
        await_completion(&wakes);
        assert!(
            migration
                .persist_async(&failing, true, backend.capture_snapshot(), &mut legacy_ui)
                .is_err()
        );
        assert!(!migration.is_busy());
        assert_eq!(migration.manifest, legacy);
        assert_eq!(
            bits(
                &good
                    .try_read_capture(LEGACY_NAMESPACE, &legacy.takes[0])
                    .unwrap()
            ),
            bits(&legacy_take.buffer.data)
        );
    }

    #[test]
    fn epoch_reset_reuses_one_lane_and_discards_old_prepared_results() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let shared = Shared::with_store(
            Some(storage),
            Some(PracticeSeal::Sealed {
                persona: "isolated".into(),
                protection: "fixture".into(),
            }),
            None,
        );
        let (mut old_ui, old_take) = fixture();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let thread_id = Arc::new(std::sync::Mutex::new(None));
        let observed_thread = thread_id.clone();
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Mutex::new(release_rx);
        let mut shared = shared.borrow_mut();
        shared.backend = Some(CpalBackend::offline_for_test());
        shared.capture_persistence.worker =
            CodecWorker::with_encoder(Arc::new(move |rate, samples| {
                let id = std::thread::current().id();
                let mut observed = observed_thread.lock().unwrap();
                if let Some(previous) = *observed {
                    assert_eq!(previous, id);
                } else {
                    *observed = Some(id);
                }
                drop(observed);
                if count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
                    started.send(()).unwrap();
                    release_rx.lock().unwrap().recv().unwrap();
                }
                woodshed_core::loop_capture::encode_capture(rate, samples)
            }));
        let wakes = completion_wake(&mut shared.capture_persistence);
        begin_session(&mut shared, &old_ui).unwrap();
        shared
            .backend
            .as_mut()
            .unwrap()
            .restore_loop_captures(vec![old_take.clone()])
            .unwrap();
        persist(&mut shared, &mut old_ui);
        await_completion(&started_rx);
        assert!(!pending(&shared, &old_ui));
        let (mut incoming, mut replacement) = fixture();
        replacement.buffer = SampleBuffer::new(vec![0.0625; old_take.buffer.len()], 48_000);
        begin_session(&mut shared, &incoming).unwrap();
        assert!(shared.capture_persistence.is_busy());
        shared
            .backend
            .as_mut()
            .unwrap()
            .restore_loop_captures(vec![replacement.clone()])
            .unwrap();
        persist(&mut shared, &mut incoming);
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(incoming.loop_captures.is_empty());
        release.send(()).unwrap();
        await_completion(&wakes);
        persist_if_changed(&mut shared, &mut incoming);
        await_completion(&wakes);
        shared.capture_persistence.last_attempt = Some(Instant::now());
        assert!(pending(&shared, &incoming));
        persist_if_changed(&mut shared, &mut incoming);
        assert!(!shared.capture_persistence.is_busy());
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 2);
        let reference = &incoming.loop_captures.takes[0];
        assert_eq!(
            bits(
                &shared
                    .storage
                    .as_ref()
                    .unwrap()
                    .try_read_capture(
                        incoming.loop_captures.namespace.as_deref().unwrap(),
                        reference,
                    )
                    .unwrap()
            ),
            bits(&replacement.buffer.data)
        );
        assert_ne!(bits(&replacement.buffer.data), bits(&old_take.buffer.data));
    }

    #[test]
    fn explicit_clear_while_preparing_cannot_republish_retired_pcm() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let shared = Shared::with_store(
            Some(storage),
            Some(PracticeSeal::Sealed {
                persona: "isolated".into(),
                protection: "fixture".into(),
            }),
            None,
        );
        let (mut ui, take) = fixture();
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Mutex::new(release_rx);
        let mut shared = shared.borrow_mut();
        shared.backend = Some(CpalBackend::offline_for_test());
        shared.capture_persistence.worker =
            CodecWorker::with_encoder(Arc::new(move |rate, samples| {
                started.send(()).unwrap();
                release_rx.lock().unwrap().recv().unwrap();
                woodshed_core::loop_capture::encode_capture(rate, samples)
            }));
        let wakes = completion_wake(&mut shared.capture_persistence);
        begin_session(&mut shared, &ui).unwrap();
        shared
            .backend
            .as_mut()
            .unwrap()
            .restore_loop_captures(vec![take.clone()])
            .unwrap();
        persist(&mut shared, &mut ui);
        await_completion(&started_rx);
        shared
            .backend
            .as_mut()
            .unwrap()
            .clear_stored_capture(take.id)
            .unwrap();
        shared.capture_persistence.forget(take.id);
        persist(&mut shared, &mut ui);
        assert!(ui.loop_captures.is_empty());
        assert!(shared.capture_persistence.worker.is_busy());
        release.send(()).unwrap();
        await_completion(&wakes);
        assert!(pending(&shared, &ui));
        persist_if_changed(&mut shared, &mut ui);
        assert!(!shared.capture_persistence.is_busy());
        assert!(shared.capture_persistence.worker.encoded(&take).is_none());
        assert!(ui.loop_captures.is_empty());
        let published = woodshed_core::storage::decode_session(
            &shared.storage.as_ref().unwrap().load().unwrap(),
        )
        .unwrap()
        .session;
        assert!(published.loop_captures.is_empty());
    }

    #[test]
    fn prepared_wrong_rate_or_frames_cannot_publish_or_touch_companion_storage() {
        for wrong_rate in [true, false] {
            let operations = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(CountCompanions {
                inner: MemoryBackend::default(),
                operations: operations.clone(),
            }));
            let (mut ui, take) = fixture();
            let mut state = CapturePersistence::default();
            state.worker = CodecWorker::with_encoder(Arc::new(move |rate, samples| {
                if wrong_rate {
                    woodshed_core::loop_capture::encode_capture(rate + 1, samples)
                } else {
                    woodshed_core::loop_capture::encode_capture(rate, &samples[..samples.len() - 1])
                }
            }));
            let wakes = completion_wake(&mut state);
            state
                .persist_async(&storage, true, vec![take.clone()], &mut ui)
                .unwrap();
            let previous = storage.load().unwrap();
            await_completion(&wakes);
            assert!(
                state
                    .persist_async(&storage, true, vec![take.clone()], &mut ui)
                    .is_err()
            );
            assert_eq!(operations.load(std::sync::atomic::Ordering::Relaxed), 0);
            assert_eq!(storage.load().unwrap(), previous);
            assert!(ui.loop_captures.is_empty());
            assert!(state.is_busy());
            assert_eq!(
                bits(&state.worker.prepared()[0].take.buffer.data),
                bits(&take.buffer.data)
            );
        }
    }

    #[test]
    fn mounted_main_close_waits_for_capture_worker_then_preserves_dirty_appearance_gate() {
        use cambium_genet_winit_host::{CloseRequest, Harness, Init};
        let directory = tempfile::tempdir().unwrap();
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let shared = Shared::with_store(
            Some(storage),
            Some(PracticeSeal::Sealed {
                persona: "isolated".into(),
                protection: "fixture".into(),
            }),
            None,
        );
        let (mut ui, take) = fixture();
        ui.appearance =
            woodshed_views::appearance::AppearanceState::load(directory.path().join("themes.json"))
                .unwrap();
        ui.appearance
            .begin_edit(&ui.app_settings.appearance)
            .unwrap();
        ui.appearance.workshop.new_copy();
        assert!(ui.appearance.workshop.has_changes());
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Mutex::new(release_rx);
        {
            let mut shared = shared.borrow_mut();
            shared.backend = Some(CpalBackend::offline_for_test());
            begin_session(&mut shared, &ui).unwrap();
            shared
                .backend
                .as_mut()
                .unwrap()
                .restore_loop_captures(vec![take.clone()])
                .unwrap();
            shared
                .backend
                .as_mut()
                .unwrap()
                .set_loop_plan(&ui.requested_loop_plan().unwrap())
                .unwrap();
            shared.capture_persistence.worker =
                CodecWorker::with_encoder(Arc::new(move |rate, samples| {
                    started.send(()).unwrap();
                    release_rx.lock().unwrap().recv().unwrap();
                    woodshed_core::loop_capture::encode_capture(rate, samples)
                }));
        }
        let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
        let mut host = Harness::with_hooks(
            Init {
                sheet: crate::appearance::stylesheet(&ui),
                state: ui,
                logic,
                fonts: Vec::new(),
                images: Vec::new(),
            },
            crate::hooks(&shared),
        );
        host.layout_at(1280.0, 960.0);
        let wake = host.wake().callback();
        let (completion, completed) = std::sync::mpsc::channel();
        shared
            .borrow_mut()
            .capture_persistence
            .set_wake(Arc::new(move || {
                wake();
                let _ = completion.send(());
            }));
        host.request_close(CloseRequest::Native);
        await_completion(&started_rx);
        assert!(!host.close_requested());
        assert!(shared.borrow().capture_persistence.is_busy());
        host.update(|ui| ui.set.cards[1].label = "Edit while native close waits".into());
        host.after_dispatch();
        host.layout_at(1280.0, 960.0);
        assert_eq!(
            host.state().set.cards[1].label,
            "Edit while native close waits"
        );
        assert!(!host.close_requested());
        assert!(host.state().loop_captures.is_empty());
        release.send(()).unwrap();
        await_completion(&completed);
        assert!(host.process_wake());
        assert!(!shared.borrow().capture_persistence.is_busy());
        assert!(!host.close_requested());
        assert!(host.state().appearance.workshop.has_changes());
        assert!(host.state().appearance.workshop.close_requested());
        assert!(host.state().appearance_close_app);
        let shared = shared.borrow();
        let store = shared.storage.as_ref().unwrap();
        let published = woodshed_core::storage::decode_session(&store.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(
            published.set.cards[1].label,
            "Edit while native close waits"
        );
        assert_eq!(published.loop_captures.takes.len(), 1);
        assert_eq!(
            bits(
                &store
                    .try_read_capture(
                        published.loop_captures.namespace.as_deref().unwrap(),
                        &published.loop_captures.takes[0],
                    )
                    .unwrap()
            ),
            bits(&take.buffer.data)
        );
    }

    #[test]
    fn worker_quota_rejection_precedes_thread_request_and_keeps_current_session() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let (mut ui, take) = fixture();
        storage.save("previous session");
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let mut state = CapturePersistence::default();
        state.worker = CodecWorker::with_encoder(Arc::new(move |rate, samples| {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            woodshed_core::loop_capture::encode_capture(rate, samples)
        }));
        let duplicate = vec![take.clone(), take];
        assert!(
            state
                .persist_async(&storage, true, duplicate, &mut ui)
                .is_err()
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert!(!state.worker.is_busy());
        assert_eq!(storage.load().unwrap(), "previous session");
    }

    #[test]
    fn wav_export_refuses_only_relevant_unavailable_set_captures() {
        let (ui, take) = fixture();
        let plan = ui.requested_loop_plan().unwrap();
        let mut state = CapturePersistence {
            unavailable: vec![LoopCaptureRef {
                clock: take.clock_field(),
                id: take.id,
                bpm: take.bpm,
                beats: take.beats,
                bars: take.bars,
                sample_rate: 48_000,
                frames: take.buffer.len(),
                codec: LoopCaptureCodec::RawF32V1,
                digest: "00".repeat(32),
            }],
            ..Default::default()
        };
        assert!(
            state
                .permit_export(Some(&plan))
                .unwrap_err()
                .contains("unavailable")
        );
        assert!(state.permit_export(None).is_ok());
        state.unavailable[0].id.card.0 += 1000;
        assert!(state.permit_export(Some(&plan)).is_ok());
        state.blocked = Some("invalid catalog".into());
        assert!(state.permit_export(Some(&plan)).is_err());
        assert!(state.permit_export(None).is_ok());
    }

    struct Failing {
        inner: MemoryBackend,
        key: &'static str,
        refuse_session_read: bool,
    }

    #[async_trait]
    impl Backend for Failing {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
            if self.refuse_session_read && key == "session" {
                return Err(StoreError::Backend(
                    "fixture post-commit verification refused".into(),
                ));
            }
            self.inner.get(key).await
        }
        async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
            if key.starts_with(self.key) {
                return Err(StoreError::Backend("fixture write refused".into()));
            }
            self.inner.put(key, bytes).await
        }
        async fn delete(&self, key: &str) -> Result<(), StoreError> {
            self.inner.delete(key).await
        }
        async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
            self.inner.list(prefix).await
        }
        async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
            self.inner.scan(start, end).await
        }
        async fn apply(&self, _: &[WriteOp]) -> Result<(), StoreError> {
            panic!("capture persistence must not assume multi-file apply is atomic")
        }
    }

    #[test]
    fn companion_or_session_failure_retains_previous_session_and_live_take() {
        for key in ["loop-pcm/", "session"] {
            let inner = MemoryBackend::default();
            let previous = r#"{"lens":"Chords","root_idx":3}"#;
            let good: SessionStore<HostBackend> = SessionStore::new(Box::new(inner.clone()));
            good.save(previous);
            let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(Failing {
                inner: inner.clone(),
                key,
                refuse_session_read: false,
            }));
            let (mut ui, take) = fixture();
            let mut state = CapturePersistence::default();
            assert!(
                state
                    .persist(&storage, true, vec![take.clone()], &mut ui)
                    .is_err()
            );
            assert_eq!(good.load().unwrap(), previous);
            assert!(ui.loop_captures.is_empty());
            assert!(state.buffers.is_empty());
            assert_eq!(take.buffer.data[0].to_bits(), 0.125f32.to_bits());
            state
                .persist(&good, true, vec![take.clone()], &mut ui)
                .unwrap();
            assert_eq!(ui.loop_captures.takes.len(), 1);
            assert!(Arc::ptr_eq(
                &state.buffers[0].buffer.data,
                &take.buffer.data
            ));
        }
    }

    #[test]
    fn failed_legacy_migration_preserves_previous_refs_namespace_and_pcm_until_retry() {
        for key in ["loop-pcm/", "session"] {
            let inner = MemoryBackend::default();
            let good: SessionStore<HostBackend> =
                SessionStore::new(Box::new(SealedBackend::new(inner.clone(), [59; 32])));
            let (mut ui, take) = fixture();
            let legacy = seed_legacy(&good, &mut ui, &[take.clone()]);
            let previous = good.load().unwrap();
            let mut backend = CpalBackend::offline_for_test();
            let mut state =
                CapturePersistence::restore(Some(&good), &legacy, &mut backend).unwrap();
            let hydrated = backend.capture_snapshot();
            assert!(
                state
                    .persist(&good, false, hydrated.clone(), &mut ui)
                    .is_err()
            );
            assert_eq!(good.load().unwrap(), previous);
            let failing: SessionStore<HostBackend> =
                SessionStore::new(Box::new(SealedBackend::new(
                    Failing {
                        inner: inner.clone(),
                        key,
                        refuse_session_read: false,
                    },
                    [59; 32],
                )));
            assert!(
                state
                    .persist(&failing, true, hydrated.clone(), &mut ui)
                    .is_err()
            );
            assert_eq!(good.load().unwrap(), previous);
            assert_eq!(ui.loop_captures, legacy);
            assert_eq!(state.manifest, legacy);
            assert_eq!(state.namespace.as_deref(), Some(LEGACY_NAMESPACE));
            assert!(state.needs_migration());
            assert!(Arc::ptr_eq(
                &state.buffers[0].buffer.data,
                &hydrated[0].buffer.data
            ));
            assert_eq!(
                bits(&backend.capture_snapshot()[0].buffer.data),
                bits(&take.buffer.data)
            );
            state.persist(&good, true, hydrated, &mut ui).unwrap();
            assert_eq!(
                ui.loop_captures.takes[0].codec,
                LoopCaptureCodec::WavpackF32V1
            );
            assert!(!state.needs_migration());
        }
    }

    #[test]
    fn legacy_migration_is_pending_on_wake_and_cannot_publish_into_another_epoch() {
        let inner = MemoryBackend::default();
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(SealedBackend::new(inner.clone(), [61; 32])));
        let (mut ui, take) = fixture();
        let legacy = seed_legacy(&storage, &mut ui, &[take]);
        let previous = storage.load().unwrap();
        let shared = Shared::with_store(
            Some(storage),
            Some(PracticeSeal::Sealed {
                persona: "isolated".into(),
                protection: "fixture".into(),
            }),
            None,
        );
        let mut shared = shared.borrow_mut();
        shared.backend = Some(CpalBackend::offline_for_test());
        let wakes = completion_wake(&mut shared.capture_persistence);
        begin_session(&mut shared, &ui).unwrap();
        assert!(pending(&shared, &ui));
        let mut incoming = UiState::new();
        persist(&mut shared, &mut incoming);
        assert_eq!(shared.storage.as_ref().unwrap().load().unwrap(), previous);
        assert_eq!(shared.capture_persistence.manifest, legacy);
        ui.set.cards[1].label = "Edit at legacy migration wake".into();
        crate::drive::frame(&mut shared, &mut ui);
        await_completion(&wakes);
        persist_if_changed(&mut shared, &mut ui);
        assert_eq!(ui.loop_captures.version, 2);
        assert!(!pending(&shared, &ui));
        let reopened = woodshed_core::storage::decode_session(
            &shared.storage.as_ref().unwrap().load().unwrap(),
        )
        .unwrap()
        .session;
        assert_eq!(reopened.set.cards[1].label, "Edit at legacy migration wake");
        assert_eq!(reopened.loop_captures, ui.loop_captures);
        begin_session(&mut shared, &incoming).unwrap();
        assert!(
            shared
                .backend
                .as_ref()
                .unwrap()
                .capture_snapshot()
                .is_empty()
        );
        assert!(!shared.capture_persistence.needs_migration());
    }

    #[test]
    fn unavailable_only_v1_catalog_is_retained_until_explicit_clear() {
        let inner = MemoryBackend::default();
        let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(inner.clone()));
        let (mut ui, take) = fixture();
        let legacy = seed_legacy(&storage, &mut ui, &[take.clone()]);
        let key = woodshed_core::loop_capture::pcm_key(LEGACY_NAMESPACE, &legacy.takes[0].digest)
            .unwrap();
        ready(inner.delete(&key)).unwrap();
        let mut backend = CpalBackend::offline_for_test();
        let mut state = CapturePersistence::restore(Some(&storage), &legacy, &mut backend).unwrap();
        assert!(!state.needs_migration());
        state
            .persist(&storage, true, backend.capture_snapshot(), &mut ui)
            .unwrap();
        assert_eq!(ui.loop_captures, legacy);
        backend.clear_stored_capture(take.id).unwrap();
        state.forget(take.id);
        state
            .persist(&storage, true, backend.capture_snapshot(), &mut ui)
            .unwrap();
        assert!(ui.loop_captures.is_empty());
    }

    #[test]
    fn pending_seal_cannot_publish_even_an_existing_companion() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let (mut ui, take) = fixture();
        let mut state = CapturePersistence::default();
        state
            .persist(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        let previous = storage.load().unwrap();
        ui.set.cards[0].label = "Unsaved edit".into();
        assert!(state.persist(&storage, false, vec![take], &mut ui).is_err());
        assert_eq!(storage.load().unwrap(), previous);
    }

    #[test]
    fn accepted_session_with_failed_readback_is_unconfirmed_without_rollback() {
        let inner = MemoryBackend::default();
        let raw: SessionStore<HostBackend> = SessionStore::new(Box::new(inner.clone()));
        let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(Failing {
            inner,
            key: "never-refuse-put",
            refuse_session_read: true,
        }));
        let (mut ui, take) = fixture();
        let mut state = CapturePersistence::default();
        assert!(
            state
                .persist(&storage, true, vec![take.clone()], &mut ui)
                .is_err()
        );
        assert!(ui.loop_captures.is_empty());
        // Publication may have committed even though its verification failed.
        let published = woodshed_core::storage::decode_session(&raw.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(published.loop_captures.takes.len(), 1);
        assert_eq!(
            raw.try_read_capture(
                published.loop_captures.namespace.as_deref().unwrap(),
                &published.loop_captures.takes[0]
            )
            .unwrap()[0],
            0.125
        );
        assert_eq!(take.buffer.data[0], 0.125);
    }

    fn host(
        shared: &std::rc::Rc<std::cell::RefCell<Shared>>,
        ui: UiState,
    ) -> cambium_genet_winit_host::Harness<
        UiState,
        crate::sync::Logic,
        woodshed_views::stage::UiChild,
    > {
        let dispatch = shared.clone();
        let mut hooks = cambium_genet_winit_host::HostHooks::inert();
        hooks.after_dispatch =
            Box::new(move |ctx| crate::sync::after_dispatch(&mut dispatch.borrow_mut(), ctx));
        let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
        let mut host = cambium_genet_winit_host::Harness::with_hooks(
            cambium_genet_winit_host::Init {
                sheet: crate::appearance::stylesheet(&ui),
                state: ui,
                logic,
                fonts: Vec::new(),
                images: Vec::new(),
            },
            hooks,
        );
        host.layout_at(1280.0, 960.0);
        host
    }

    #[test]
    fn absent_audio_still_saves_practice_and_retains_then_explicitly_clears_orphan_refs() {
        let inner = MemoryBackend::default();
        let store: SessionStore<HostBackend> =
            SessionStore::new(Box::new(SealedBackend::new(inner.clone(), [31; 32])));
        let (mut ui, take) = fixture();
        CapturePersistence::default()
            .persist(&store, true, vec![take.clone()], &mut ui)
            .unwrap();
        let original = ui.loop_captures.clone();
        ui.select_app_section(woodshed_core::storage::AppSection::Looper);
        let shared = Shared::with_store(
            Some(store),
            Some(PracticeSeal::Sealed {
                persona: "isolated".into(),
                protection: "fixture".into(),
            }),
            None,
        );
        let mut host = host(&shared, ui);
        host.update(|ui| {
            ui.set.cards[1].label = "Practice with no output".into();
            ui.set.remove(0);
        });
        host.after_dispatch();
        assert!(shared.borrow().backend.is_none());
        let mut reopened = UiState::new();
        crate::session::restore(shared.borrow().storage.as_ref().unwrap(), &mut reopened);
        assert_eq!(reopened.set.cards[0].label, "Practice with no output");
        assert_eq!(reopened.loop_captures, original);
        // The removed occurrence's reference has its own explicit Clear action.
        host.update(|ui| ui.loop_notice = Some("Prior capture refusal".into()));
        host.layout_at(1280.0, 960.0);
        assert!(host.click_on(
            &taproot::Selector::role("button").with_attr("data-action", "clear-stored-loop")
        ));
        host.after_dispatch();
        assert!(host.state().loop_notice.is_none());
        crate::session::restore(shared.borrow().storage.as_ref().unwrap(), &mut reopened);
        assert!(reopened.loop_captures.is_empty());
        assert_eq!(reopened.set.cards[0].label, "Practice with no output");
    }

    #[test]
    fn completion_service_saves_without_dispatch_using_current_practice_edits() {
        let store: SessionStore<HostBackend> = SessionStore::new(Box::new(SealedBackend::new(
            MemoryBackend::default(),
            [41; 32],
        )));
        let shared = Shared::with_store(
            Some(store),
            Some(PracticeSeal::Sealed {
                persona: "isolated".into(),
                protection: "fixture".into(),
            }),
            None,
        );
        let (mut ui, take) = fixture();
        let mut shared = shared.borrow_mut();
        shared.backend = Some(CpalBackend::offline_for_test());
        begin_session(&mut shared, &ui).unwrap();
        shared
            .backend
            .as_mut()
            .unwrap()
            .restore_loop_captures(vec![take])
            .unwrap();
        shared
            .backend
            .as_mut()
            .unwrap()
            .set_loop_plan(&ui.requested_loop_plan().unwrap())
            .unwrap();
        let wakes = completion_wake(&mut shared.capture_persistence);
        // A later practice edit must be captured at publication, not replaced
        // by a session JSON prepared when recording began.
        ui.set.cards[1].label = "Current edit at completion".into();
        assert!(pending(&shared, &ui));
        crate::drive::frame(&mut shared, &mut ui);
        await_completion(&wakes);
        persist_if_changed(&mut shared, &mut ui);
        let mut reopened = UiState::new();
        crate::session::restore(shared.storage.as_ref().unwrap(), &mut reopened);
        assert_eq!(reopened.loop_captures.takes.len(), 1);
        assert_eq!(reopened.set.cards[1].label, "Current edit at completion");
        assert!(!pending(&shared, &ui));
        assert!(
            shared
                .backend
                .as_ref()
                .unwrap()
                .capture_source_rate()
                .is_none()
        );
    }

    #[test]
    fn unready_incoming_epoch_never_observes_prior_playing_or_cursor() {
        let store: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let shared = Shared::with_store(Some(store), None, None);
        let (old, _) = fixture();
        let mut backend = CpalBackend::offline_for_test();
        backend
            .set_loop_plan(&old.requested_loop_plan().unwrap())
            .unwrap();
        backend.set_song_transport(true);
        for _ in 0..80 {
            backend.song_rewind();
        }
        let mut shared = shared.borrow_mut();
        shared.backend = Some(backend);
        shared.last_audio_session = Some(old.audio_session.clone());
        let mut incoming = UiState::new();
        assert!(begin_session(&mut shared, &incoming).is_err());
        crate::drive::frame(&mut shared, &mut incoming);
        assert!(!incoming.song_playing);
        assert!(!incoming.song_recording);
        assert_eq!(incoming.song_bar_live, 0);
        assert!(!session_current(&shared, &incoming));
        shared.backend.as_mut().unwrap().process_for_test(256);
        assert!(begin_session(&mut shared, &incoming).is_ok());
        assert!(session_current(&shared, &incoming));
        assert!(
            shared
                .backend
                .as_ref()
                .unwrap()
                .capture_snapshot()
                .is_empty()
        );
    }

    #[test]
    fn damaged_companion_reference_survives_saves_until_explicit_clear() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let (mut ui, take) = fixture();
        let mut state = CapturePersistence::default();
        state
            .persist(&storage, true, vec![take.clone()], &mut ui)
            .unwrap();
        let manifest = ui.loop_captures.clone();
        let key = woodshed_core::loop_capture::pcm_key(
            manifest.namespace.as_deref().unwrap(),
            &manifest.takes[0].digest,
        )
        .unwrap();
        // Use actual backend damage, not an empty-catalog substitute.
        let mut write = std::pin::pin!(storage.backend().put(&key, b"truncated"));
        use std::future::Future;
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(matches!(
            write.as_mut().poll(&mut context),
            std::task::Poll::Ready(Ok(()))
        ));
        let mut backend = CpalBackend::offline_for_test();
        backend.clear_loop_session();
        let mut restored =
            CapturePersistence::restore(Some(&storage), &manifest, &mut backend).unwrap();
        assert_eq!(restored.unavailable.len(), 1);
        assert!(backend.capture_snapshot().is_empty());
        assert!(
            restored
                .permit_record(&ui.requested_loop_plan().unwrap(), 48_000, Vec::new())
                .is_err()
        );
        assert!(
            restored
                .persist(&storage, true, vec![take.clone()], &mut ui)
                .is_err()
        );
        restored
            .persist(&storage, true, Vec::new(), &mut ui)
            .unwrap();
        assert_eq!(ui.loop_captures, manifest);
        assert!(
            ui.loop_capture_notice
                .as_ref()
                .unwrap()
                .contains("unavailable or damaged")
        );
        backend.clear_stored_capture(take.id).unwrap();
        restored.forget(take.id);
        restored
            .persist(&storage, true, Vec::new(), &mut ui)
            .unwrap();
        assert!(ui.loop_captures.is_empty());
        assert!(ui.loop_capture_notice.is_none());
    }

    #[test]
    fn malformed_manifest_never_overwrites_a_saved_session() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let (mut ui, _) = fixture();
        ui.loop_captures.version = 999;
        let previous = serde_json::to_string(&ui.to_persisted()).unwrap();
        storage.save(&previous);
        let mut backend = CpalBackend::offline_for_test();
        backend.clear_loop_session();
        let mut state =
            CapturePersistence::restore(Some(&storage), &ui.loop_captures, &mut backend).unwrap();
        assert!(state.blocked.is_some());
        assert!(state.persist(&storage, true, Vec::new(), &mut ui).is_err());
        assert_eq!(storage.load().unwrap(), previous);
    }

    #[test]
    fn future_codec_is_retained_unavailable_without_losing_authoring_and_can_be_cleared() {
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        let (mut ui, take) = fixture();
        let mut manifest = seed_legacy(&storage, &mut ui, &[take]);
        manifest.version = 2;
        manifest.takes[0].codec = LoopCaptureCodec::Unsupported("future-exact-f32-v9".into());
        ui.loop_captures = manifest.clone();
        ui.set.cards[0].label = "Authored session with future companion".into();
        let previous = serde_json::to_string(&ui.to_persisted()).unwrap();
        storage.save(&previous);
        let mut reopened = UiState::new();
        crate::session::restore(&storage, &mut reopened);
        assert_eq!(reopened.set.cards[0].label, ui.set.cards[0].label);
        assert_eq!(reopened.loop_captures, manifest);
        let mut backend = CpalBackend::offline_for_test();
        let mut state =
            CapturePersistence::restore(Some(&storage), &reopened.loop_captures, &mut backend)
                .unwrap();
        assert!(state.blocked.is_none());
        assert_eq!(state.unavailable, manifest.takes);
        reopened.set.cards[1].label = "Current ordinary edit".into();
        state
            .persist(&storage, true, Vec::new(), &mut reopened)
            .unwrap();
        let published = woodshed_core::storage::decode_session(&storage.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(published.set.cards[1].label, "Current ordinary edit");
        assert_eq!(published.loop_captures, manifest);
        assert!(backend.capture_snapshot().is_empty());
        assert!(
            state
                .permit_export(Some(&reopened.requested_loop_plan().unwrap()))
                .is_err()
        );
        assert!(
            state
                .permit_record(&reopened.requested_loop_plan().unwrap(), 48_000, Vec::new())
                .is_err()
        );
        backend.clear_stored_capture(manifest.takes[0].id).unwrap();
        state.forget(manifest.takes[0].id);
        state
            .persist(&storage, true, Vec::new(), &mut reopened)
            .unwrap();
        assert!(reopened.loop_captures.is_empty());
    }

    #[test]
    fn unavailable_inactive_references_reserve_catalog_budget_before_recording() {
        let (ui, take) = fixture();
        let retained = LoopCaptureRef {
            clock: None,
            id: LoopSegmentId {
                set: woodshed_core::working_sets::WorkingSetId(99),
                card: woodshedding::rehearsal::CardId(1),
            },
            bpm: 120.0,
            beats: 4,
            bars: 1,
            sample_rate: 48_000,
            frames: woodshed_core::loop_capture::MAX_CAPTURE_FRAMES,
            codec: LoopCaptureCodec::RawF32V1,
            digest: "0".repeat(64),
        };
        let state = CapturePersistence {
            unavailable: vec![retained],
            ..Default::default()
        };
        assert!(
            state
                .permit_record(&ui.requested_loop_plan().unwrap(), 48_000, vec![take])
                .unwrap_err()
                .contains("frame budget")
        );
    }

    #[test]
    fn exact_sealed_set_capture_survives_fresh_process_reorder_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        for mode in [
            "seed-legacy",
            "migrate",
            "reopen",
            "clear",
            "reopen-cleared",
        ] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "capture_persistence::tests::fresh_process_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
                .env("WOODSHED_CAPTURE_FIXTURE", mode)
                .status()
                .unwrap();
            assert!(status.success(), "fresh-process {mode}");
        }
    }

    /// CPU-only qualification of an explicitly copied Keeper profile. The
    /// caller owns the isolated vault/socket and all supplied filesystem paths.
    #[test]
    #[ignore = "requires an isolated unlocked Keeper and copied profile overrides"]
    fn sealed_profile_compression_probe() {
        use std::io::Write;
        use std::path::PathBuf;

        let state_path = PathBuf::from(
            std::env::var_os("WOODSHED_STATE").expect("explicit copied WOODSHED_STATE is required"),
        );
        let settings_path = PathBuf::from(
            std::env::var_os("WOODSHED_SETTINGS")
                .expect("explicit copied WOODSHED_SETTINGS is required"),
        );
        let output_dir = PathBuf::from(
            std::env::var_os("WOODSHED_CAPTURE_PROBE_DIR")
                .expect("owned WOODSHED_CAPTURE_PROBE_DIR is required"),
        );
        assert!(
            state_path.is_absolute() && settings_path.is_absolute() && output_dir.is_absolute()
        );
        assert!(state_path.is_file(), "copied profile session must exist");
        assert!(
            output_dir.is_dir(),
            "owned probe output directory must exist"
        );
        let mode = std::env::var("WOODSHED_CAPTURE_PROBE_MODE")
            .expect("WOODSHED_CAPTURE_PROBE_MODE must be inspect or migrate");
        assert!(matches!(mode.as_str(), "inspect" | "migrate"));
        let report_path = output_dir.join(format!("{mode}.json"));
        let mut report_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&report_path)
            .expect("probe reports never overwrite existing files");

        let (storage, seal) = crate::storage::open_store_as(None);
        assert!(
            matches!(seal, PracticeSeal::Sealed { .. }),
            "actual Keeper custody must be sealed"
        );
        let original_json = storage
            .load()
            .expect("copied sealed session must be readable");
        let original = woodshed_core::storage::decode_session(&original_json)
            .unwrap()
            .session;
        let mut ui = UiState::new();
        crate::session::restore(&storage, &mut ui);
        assert_eq!(ui.loop_captures, original.loop_captures);
        let shared = Shared::with_store(Some(storage), Some(seal), None);
        let mut shared = shared.borrow_mut();
        shared.backend = Some(CpalBackend::offline_for_test());
        begin_session(&mut shared, &ui)
            .expect("actual capture bank must hydrate through session reset");
        let plan_admission = ui
            .requested_loop_plan()
            .map(|plan| shared.backend.as_mut().unwrap().set_loop_plan(&plan));
        let native_stream_count = shared.backend.as_ref().unwrap().output_stream_count();
        assert_eq!(native_stream_count, 0);
        assert!(
            shared
                .backend
                .as_ref()
                .unwrap()
                .capture_source_rate()
                .is_none()
        );
        let before = shared.backend.as_ref().unwrap().capture_snapshot();
        let manifest_before = ui.loop_captures.clone();
        let unavailable_before = shared.capture_persistence.unavailable.clone();
        let mut persistence = std::mem::take(&mut shared.capture_persistence);
        let storage = shared.storage.take().unwrap();
        let backend = shared.backend.take().unwrap();
        drop(shared);

        fn captured_metadata(takes: &[CaptureSnapshot]) -> Vec<serde_json::Value> {
            takes
                .iter()
                .map(|take| {
                    let mut exact_bytes = Vec::with_capacity(take.buffer.len() * 4);
                    for sample in take.buffer.data.iter() {
                        exact_bytes.extend_from_slice(&sample.to_le_bytes());
                    }
                    serde_json::json!({
                        "id": take.id,
                        "bpm": take.bpm,
                        "beats": take.beats,
                        "bars": take.bars,
                        "sample_rate": take.buffer.sample_rate_hz,
                        "frames": take.buffer.len(),
                        "pcm_f32le_blake3": muniment::blob::Hash::of(&exact_bytes).to_hex(),
                    })
                })
                .collect()
        }

        let captures_before = captured_metadata(&before);
        let migration_needed_before = persistence.needs_migration();
        let started = Instant::now();
        if mode == "migrate" {
            let wakes = completion_wake(&mut persistence);
            persistence
                .persist_async(&storage, true, backend.capture_snapshot(), &mut ui)
                .expect("production capture preparation must be accepted");
            if persistence.worker.is_busy() {
                wakes
                    .recv_timeout(Duration::from_secs(30))
                    .expect("actual Keeper compression must signal readiness within 30 seconds");
                persistence
                    .persist_async(&storage, true, backend.capture_snapshot(), &mut ui)
                    .expect("prepared captures must publish through actual custody backend");
            }
            assert!(!persistence.is_busy());
            assert!(persistence.worker.prepared().is_empty());
            assert!(!persistence.needs_migration());
        }
        // This read traverses Custodied::get and rechecks the real unlocked
        // identity; a cached PracticeSeal flag alone cannot certify publication.
        let final_json = storage
            .load()
            .expect("actual custody must still permit sealed readback");
        let published = woodshed_core::storage::decode_session(&final_json)
            .unwrap()
            .session;
        let after = backend.capture_snapshot();
        let captures_after = captured_metadata(&after);
        assert_eq!(
            captures_before, captures_after,
            "captured float bits and clocks must remain exact"
        );
        assert_eq!(manifest_before.namespace, ui.loop_captures.namespace);
        assert_eq!(published.loop_captures, ui.loop_captures);
        if mode == "inspect" {
            assert_eq!(
                original_json, final_json,
                "inspect must not save the copied profile"
            );
        }
        let report = serde_json::json!({
            "schema": "woodshed-sealed-profile-compression-probe-v1",
            "mode": mode,
            "seal": "sealed",
            "native_stream_count": native_stream_count,
            "input_opened": false,
            "plan_admission": match plan_admission {
                Ok(Ok(())) => serde_json::json!({"admitted": true}),
                Ok(Err(error)) => serde_json::json!({"admitted": false, "reason": error}),
                Err(readiness) => serde_json::json!({
                    "admitted": false,
                    "readiness": format!("{readiness:?}"),
                }),
            },
            "migration_needed_before": migration_needed_before,
            "elapsed_ms": started.elapsed().as_millis(),
            "manifest_before": manifest_before,
            "manifest_after": ui.loop_captures,
            "unavailable_before": unavailable_before,
            "unavailable_after": persistence.unavailable,
            "captures_before": captures_before,
            "captures_after": captures_after,
            "session_before_blake3": muniment::blob::Hash::of(original_json.as_bytes()).to_hex(),
            "session_after_blake3": muniment::blob::Hash::of(final_json.as_bytes()).to_hex(),
        });
        serde_json::to_writer_pretty(&mut report_file, &report).unwrap();
        report_file.write_all(b"\n").unwrap();
        report_file.sync_all().unwrap();
        println!(
            "sealed profile {mode} probe wrote metadata-only report {}",
            report_path.display()
        );
    }

    #[test]
    #[ignore = "fresh-process child, invoked by the sealed capture test"]
    fn fresh_process_fixture() {
        let mode = std::env::var("WOODSHED_CAPTURE_FIXTURE").unwrap();
        let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(SealedBackend::new(
            crate::storage::FsBackend::new(),
            [37; 32],
        )));
        let (expected_ui, expected) = fixture();
        let mut ui = if matches!(mode.as_str(), "seed" | "seed-legacy") {
            expected_ui
        } else {
            let mut ui = UiState::new();
            crate::session::restore(&storage, &mut ui);
            ui
        };
        if mode == "seed-legacy" {
            seed_legacy(&storage, &mut ui, &[expected]);
            assert_eq!(ui.loop_captures.version, 1);
            assert_eq!(
                ui.loop_captures.takes[0].codec,
                LoopCaptureCodec::LegacyRawF32V1
            );
            println!("sealed V1 raw capture seed: exact frames, no input device");
            return;
        }
        let mut backend = CpalBackend::offline_for_test();
        backend.clear_loop_session();
        let mut state =
            CapturePersistence::restore(Some(&storage), &ui.loop_captures, &mut backend).unwrap();
        if mode == "migrate" {
            let legacy = ui.loop_captures.clone();
            assert_eq!(legacy.version, 1);
            assert!(state.needs_migration());
            state
                .persist(&storage, true, backend.capture_snapshot(), &mut ui)
                .unwrap();
            assert_eq!(ui.loop_captures.version, 2);
            assert_eq!(ui.loop_captures.namespace, legacy.namespace);
            assert_eq!(
                ui.loop_captures.takes[0].codec,
                LoopCaptureCodec::WavpackF32V1
            );
            assert_eq!(
                bits(
                    &storage
                        .try_read_capture(LEGACY_NAMESPACE, &legacy.takes[0],)
                        .unwrap()
                ),
                bits(&expected.buffer.data)
            );
            assert_eq!(
                bits(&backend.capture_snapshot()[0].buffer.data),
                bits(&expected.buffer.data)
            );
            println!("fresh process: V1 raw migrated to exact current WavPack; legacy blob retained");
        } else if mode == "seed" {
            backend
                .restore_loop_captures(vec![expected.clone()])
                .unwrap();
            backend
                .set_loop_plan(&ui.requested_loop_plan().unwrap())
                .unwrap();
            state
                .persist(&storage, true, backend.capture_snapshot(), &mut ui)
                .unwrap();
            let at_rest = std::fs::read(std::env::var("WOODSHED_STATE").unwrap()).unwrap();
            assert!(serde_json::from_slice::<serde_json::Value>(&at_rest).is_err());
            println!(
                "sealed capture seed: {} exact frames, no input device",
                expected.buffer.len()
            );
        } else if mode == "reopen-cleared" {
            assert!(ui.loop_captures.is_empty());
            assert!(backend.capture_snapshot().is_empty());
            backend
                .set_loop_plan(&ui.requested_loop_plan().unwrap())
                .unwrap();
            assert!(backend.song_loop_bars().iter().all(|present| !present));
            println!("fresh process: cleared capture remains absent");
        } else {
            assert_eq!(ui.loop_captures.takes.len(), 1);
            let restored = backend.capture_snapshot();
            assert_eq!(restored.len(), 1);
            assert_eq!(restored[0].id, expected.id);
            assert!(
                restored[0]
                    .buffer
                    .data
                    .iter()
                    .zip(expected.buffer.data.iter())
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            assert!(backend.capture_source_rate().is_none());
            if mode == "reopen" {
                ui.set.cards.reverse();
                backend
                    .set_loop_plan(&ui.requested_loop_plan().unwrap())
                    .unwrap();
                assert_eq!(backend.song_loop_bars(), vec![false, true]);
                state
                    .persist(&storage, true, backend.capture_snapshot(), &mut ui)
                    .unwrap();
                println!(
                    "fresh process: exact samples and Set/Card identity retained across reorder"
                );
            } else {
                assert_eq!(mode, "clear");
                // Clock edit is refused until explicit clear even after reopen.
                ui.looper.inherited_bpm += 5.0;
                assert!(
                    backend
                        .set_loop_plan(&ui.requested_loop_plan().unwrap())
                        .is_err()
                );
                backend.clear_stored_capture(expected.id).unwrap();
                state.forget(expected.id);
                backend
                    .set_loop_plan(&ui.requested_loop_plan().unwrap())
                    .unwrap();
                state
                    .persist(&storage, true, backend.capture_snapshot(), &mut ui)
                    .unwrap();
                assert!(ui.loop_captures.is_empty());
                println!("fresh process: clock refusal resolved by explicit clear and persisted");
            }
        }
    }
    #[test]
    fn sealed_mixed_clock_reopen_preserves_legacy_blob_and_uses_canonical_new_take() {
        let inner = MemoryBackend::default();
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(SealedBackend::new(inner.clone(), [61; 32])));
        let (mut ui, _) = fixture();
        ui.looper.beats = 5;
        ui.set.cards[0].timing.bpm = Some(113.5);
        ui.set.cards[0].timing.hold = Hold::Bars(2);
        ui.set.cards[1].timing.bpm = Some(137.0);
        ui.set.cards[1].timing.hold = Hold::Bars(1);
        ui.loop_working_set(ui.working_sets.active_id);
        let plan = ui.requested_loop_plan().unwrap();
        let samples: Vec<_> = (0..253_744)
            .map(|i| [0.125, -0.0, -0.25, 0.5][i % 4])
            .collect();
        let (codec, digest, frames) = storage
            .try_write_capture_compressed(LEGACY_NAMESPACE, 48_000, &samples)
            .unwrap();
        let original = LoopCaptureRef {
            id: plan.segments[0].id,
            bpm: 113.5,
            beats: 5,
            bars: 2,
            sample_rate: 48_000,
            frames,
            digest,
            codec,
            clock: None,
        };
        let old_key =
            woodshed_core::loop_capture::pcm_key(LEGACY_NAMESPACE, &original.digest).unwrap();
        let encrypted_blob = ready(inner.get(&old_key)).unwrap().unwrap();
        let old_manifest = LoopCaptureManifest {
            version: 2,
            namespace: Some(LEGACY_NAMESPACE.into()),
            takes: vec![original.clone()],
        };
        ui.loop_captures = old_manifest.clone();
        storage
            .try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
            .unwrap();
        let mut backend = CpalBackend::offline_for_test();
        let mut state =
            CapturePersistence::restore(Some(&storage), &old_manifest, &mut backend).unwrap();
        assert!(state.unavailable.is_empty());
        assert!(!state.changed(&backend.capture_snapshot()));
        let mut takes = backend.capture_snapshot();
        assert!(takes[0].legacy_clock);
        assert_eq!(bits(&takes[0].buffer.data), bits(&samples));
        let canonical_frames = woodshed_audio::Bar {
            bpm: 137.0,
            time_signature: woodshed_audio::TimeSignature::new(5, 4),
            ..Default::default()
        }
        .duration_samples(48_000.0) as usize;
        assert_eq!(canonical_frames, 105_110);
        takes.push(CaptureSnapshot {
            id: plan.segments[1].id,
            bpm: 137.0,
            beats: 5,
            bars: 1,
            legacy_clock: false,
            buffer: SampleBuffer::new(vec![0.0625; canonical_frames], 48_000),
        });
        backend.restore_loop_captures(takes).unwrap();
        backend.set_loop_plan(&plan).unwrap();
        let (backing, rate) = backend.export_backing().unwrap();
        assert!(backing.bars[0].legacy_clock);
        assert!(!backing.bars[1].legacy_clock);
        assert_eq!(backing.bars[0].duration_samples(rate as f32), 253_744);
        let mut wav = std::io::Cursor::new(Vec::new());
        let report = woodshed_audio::offline::export_song_wav_to(&backing, rate, &mut wav).unwrap();
        assert_eq!(report.frames, 253_744 + 105_110);
        state
            .persist(&storage, true, backend.capture_snapshot(), &mut ui)
            .unwrap();
        assert_eq!(ui.loop_captures.version, CAPTURE_VERSION);
        assert_eq!(ui.loop_captures.takes[0], original);
        assert_eq!(
            ui.loop_captures.takes[1].clock.as_deref(),
            Some(woodshed_core::loop_capture::PER_BEAT_CLOCK)
        );
        assert_eq!(ready(inner.get(&old_key)).unwrap().unwrap(), encrypted_blob);
        let reopened = woodshed_core::storage::decode_session(&storage.load().unwrap())
            .unwrap()
            .session;
        let mut fresh = CpalBackend::offline_for_test();
        let restored =
            CapturePersistence::restore(Some(&storage), &reopened.loop_captures, &mut fresh)
                .unwrap();
        assert!(restored.unavailable.is_empty());
        let snapshots = fresh.capture_snapshot();
        assert!(snapshots[0].legacy_clock);
        assert!(!snapshots[1].legacy_clock);
        assert_eq!(bits(&snapshots[0].buffer.data), bits(&samples));
        fresh.set_loop_plan(&plan).unwrap();
        let (again, _) = fresh.export_backing().unwrap();
        assert_eq!(again.bars[0].duration_samples(48_000.0), 253_744);
        assert!(
            fresh
                .loop_arm_record(plan.segments[0].id)
                .unwrap_err()
                .contains("legacy-clock")
        );
        assert!(fresh.capture_source_rate().is_none());
        assert_eq!(ready(inner.get(&old_key)).unwrap().unwrap(), encrypted_blob);
    }

    /// Read-only accepted fixture intake. Never opens a device, vault or the
    /// real practice store; only writes exports to a caller-owned new folder.
    #[test]
    #[ignore = "requires accepted fixture JSON/WAV and an owned output directory"]
    fn canonical_clock_gold_fixture_probe() {
        let fixture_path = std::env::var("WOODSHED_CLOCK_FIXTURE_JSON").unwrap();
        let gold_path = std::env::var("WOODSHED_CLOCK_GOLD_WAV").unwrap();
        let output = std::path::PathBuf::from(std::env::var("WOODSHED_CLOCK_RECEIPT_DIR").unwrap());
        assert!(output.is_absolute() && output.is_dir());
        let input: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture_path).unwrap()).unwrap();
        let domain = &input["domain"];
        let storage: SessionStore<HostBackend> =
            SessionStore::new(Box::new(MemoryBackend::default()));
        storage
            .try_save(&serde_json::to_string(domain).unwrap())
            .unwrap();
        let mut ui = UiState::new();
        crate::session::restore(&storage, &mut ui);
        let plan = ui.requested_loop_plan().unwrap();
        let reference = &ui.loop_captures.takes[0];
        assert_eq!(reference.frames, 96_000);
        assert_eq!(reference.sample_rate, 48_000);
        let samples: Vec<_> = (0..reference.frames)
            .map(|frame| [0.000125, -0.00025, -0.0, 0.0005][frame % 4])
            .collect();
        // This is the exact deterministic take from the original fixture,
        // checked against its content-address before exporting anything.
        let raw = woodshed_core::loop_capture::encode_pcm(reference.sample_rate, &samples).unwrap();
        assert_eq!(
            woodshed_core::loop_capture::pcm_digest(&raw),
            reference.digest
        );
        let snapshot = CaptureSnapshot {
            id: reference.id,
            bpm: reference.bpm,
            beats: reference.beats,
            bars: reference.bars,
            legacy_clock: true,
            buffer: SampleBuffer::new(samples, reference.sample_rate),
        };
        let mut backend = CpalBackend::offline_for_test();
        backend.restore_loop_captures(vec![snapshot]).unwrap();
        backend.set_loop_plan(&plan).unwrap();
        let (mut backing, rate) = backend.export_backing().unwrap();
        let gold = std::fs::read(gold_path).unwrap();
        for legacy in [true, false] {
            for bar in &mut backing.bars {
                bar.legacy_clock = legacy;
            }
            let mut destination = std::io::Cursor::new(Vec::new());
            let report =
                woodshed_audio::offline::export_song_wav_to(&backing, rate, &mut destination)
                    .unwrap();
            assert_eq!(report.frames, 192_000);
            let bytes = destination.into_inner();
            assert_eq!(
                bytes, gold,
                "accepted 120 BPM WAV differs under legacy={legacy}"
            );
            use std::io::Write;
            let path = output.join(if legacy {
                "legacy-gold.wav"
            } else {
                "canonical-gold.wav"
            });
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(path)
                .unwrap()
                .write_all(&bytes)
                .unwrap();
        }
        assert!(backend.capture_source_rate().is_none());
        assert_eq!(backend.output_stream_count(), 0);
        println!(
            "accepted WAV: legacy and canonical each byte-identical; 192000 frames, 48000 Hz; exact original 96000-frame take; no input/output devices"
        );
    }
}
