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
use woodshed_core::loop_capture::{LoopCaptureManifest, LoopCaptureRef};
use woodshed_core::loop_plan::LoopSegmentId;
use woodshed_core::storage::SessionStore;
use woodshed_views::persona::PracticeSeal;
use woodshed_views::stage::UiState;

use crate::audio::{CaptureSnapshot, CpalBackend};
use crate::shared::Shared;
use crate::storage::HostBackend;

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
}

fn same_take(a: &CaptureSnapshot, b: &CaptureSnapshot) -> bool {
    a.id == b.id
        && a.bpm == b.bpm
        && a.beats == b.beats
        && a.bars == b.bars
        && a.buffer.sample_rate_hz == b.buffer.sample_rate_hz
        && Arc::ptr_eq(&a.buffer.data, &b.buffer.data)
}

impl CapturePersistence {
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
                        bpm: reference.bpm,
                        time_signature: woodshed_audio::TimeSignature::new(reference.beats, 4),
                        length: reference.bars,
                        ..Default::default()
                    }
                    .duration_samples(reference.sample_rate as f32)
                        == reference.frames as u64 =>
                {
                    state.buffers.push(CaptureSnapshot {
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
                    id: take.id,
                    bpm: take.bpm,
                    beats: take.beats,
                    bars: take.bars,
                    sample_rate: take.buffer.sample_rate_hz,
                    frames: take.buffer.len(),
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
                    id: segment.id,
                    bpm: segment.bpm,
                    beats: segment.beats,
                    bars: segment.bars,
                    sample_rate: rate,
                    frames,
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

    fn changed(&self, takes: &[CaptureSnapshot]) -> bool {
        self.cleared
            || self.buffers.len() != takes.len()
            || takes
                .iter()
                .any(|take| !self.buffers.iter().any(|old| same_take(old, take)))
    }

    fn retry_ready(&self) -> bool {
        self.last_attempt
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(1))
    }

    fn persist(
        &mut self,
        storage: &SessionStore<HostBackend>,
        sealed: bool,
        takes: Vec<CaptureSnapshot>,
        ui: &mut UiState,
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
            let old = self
                .buffers
                .iter()
                .find(|old| same_take(old, take))
                .and_then(|_| {
                    self.manifest
                        .takes
                        .iter()
                        .find(|reference| reference.id == take.id)
                });
            let digest = old.map_or_else(|| "0".repeat(64), |reference| reference.digest.clone());
            manifest.takes.push(LoopCaptureRef {
                id: take.id,
                bpm: take.bpm,
                beats: take.beats,
                bars: take.bars,
                sample_rate: take.buffer.sample_rate_hz,
                frames: take.buffer.len(),
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
            if !self.buffers.iter().any(|old| same_take(old, take)) {
                let digest = storage
                    .try_write_capture(
                        manifest
                            .namespace
                            .as_deref()
                            .expect("capture namespace prepared"),
                        take.buffer.sample_rate_hz,
                        &take.buffer.data,
                    )?
                    .0;
                manifest
                    .takes
                    .iter_mut()
                    .find(|reference| reference.id == take.id)
                    .expect("validated take reference")
                    .digest = digest;
            }
        }
        let mut session = ui.to_persisted();
        session.loop_captures = manifest.clone();
        let json = serde_json::to_string(&session).map_err(|error| error.to_string())?;
        if durable {
            storage.try_save(&json).map_err(|error| error.to_string())?;
        } else {
            // Preserve ordinary practice's existing best-effort unsaved policy.
            storage.save(&json);
        }
        self.unavailable
            .retain(|reference| !takes.iter().any(|take| take.id == reference.id));
        self.manifest = manifest.clone();
        self.namespace = manifest.namespace.clone();
        self.buffers = takes;
        self.cleared = false;
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
    let state = if let Some(backend) = shared.backend.as_mut() {
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
        && shared.capture_persistence.retry_ready()
        && shared.backend.as_ref().is_some_and(|backend| {
            shared
                .capture_persistence
                .changed(&backend.capture_snapshot())
        })
}

/// Used by dispatch, and by completion service without another user action.
pub fn persist(shared: &mut Shared, ui: &mut UiState) {
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
        .persist(storage, sealed, takes, ui)
    {
        ui.loop_capture_notice = Some(format!(
            "Captured-loop save is unconfirmed: {error} Live takes remain available."
        ));
    }
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
            id: segment.id,
            bpm: segment.bpm,
            beats: segment.beats,
            bars: segment.bars,
            buffer: SampleBuffer::new(samples, 48_000),
        };
        (ui, take)
    }

    #[test]
    fn wav_export_refuses_only_relevant_unavailable_set_captures() {
        let (ui, take) = fixture();
        let plan = ui.requested_loop_plan().unwrap();
        let mut state = CapturePersistence {
            unavailable: vec![LoopCaptureRef {
                id: take.id,
                bpm: take.bpm,
                beats: take.beats,
                bars: take.bars,
                sample_rate: 48_000,
                frames: take.buffer.len(),
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
        // A later practice edit must be captured at publication, not replaced
        // by a session JSON prepared when recording began.
        ui.set.cards[1].label = "Current edit at completion".into();
        assert!(pending(&shared, &ui));
        crate::drive::frame(&mut shared, &mut ui);
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
    fn unavailable_inactive_references_reserve_catalog_budget_before_recording() {
        let (ui, take) = fixture();
        let retained = LoopCaptureRef {
            id: LoopSegmentId {
                set: woodshed_core::working_sets::WorkingSetId(99),
                card: woodshedding::rehearsal::CardId(1),
            },
            bpm: 120.0,
            beats: 4,
            bars: 1,
            sample_rate: 48_000,
            frames: woodshed_core::loop_capture::MAX_CAPTURE_FRAMES,
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
        for mode in ["seed", "reopen", "clear", "reopen-cleared"] {
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

    #[test]
    #[ignore = "fresh-process child, invoked by the sealed capture test"]
    fn fresh_process_fixture() {
        let mode = std::env::var("WOODSHED_CAPTURE_FIXTURE").unwrap();
        let storage: SessionStore<HostBackend> = SessionStore::new(Box::new(SealedBackend::new(
            crate::storage::FsBackend::new(),
            [37; 32],
        )));
        let (expected_ui, expected) = fixture();
        let mut ui = if mode == "seed" {
            expected_ui
        } else {
            let mut ui = UiState::new();
            crate::session::restore(&storage, &mut ui);
            ui
        };
        let mut backend = CpalBackend::offline_for_test();
        backend.clear_loop_session();
        let mut state =
            CapturePersistence::restore(Some(&storage), &ui.loop_captures, &mut backend).unwrap();
        if mode == "seed" {
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
}
