//! Exercise the production transition hook with in-memory incoming custody.
use super::*;
use muniment::Backend;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use woodshed_core::storage::SessionStore;
use woodshed_views::persona::PracticeSeal;
use woodshed_views::stage::{UiChild, UiState};

fn fixture_root() -> std::path::PathBuf {
    // Every backend in this fixture is in-memory; the frozen root identifies
    // the installed binding without opening production filesystem paths.
    std::env::temp_dir().join("woodshed-persona-preservation-fixture")
}

fn fixture_binding(profile: &str) -> crate::storage::StoreBinding {
    crate::storage::StoreBinding::for_test(
        fixture_root(),
        if profile == "outgoing" { "a" } else { "b" }.repeat(64),
        personae::ProfileId(profile.into()),
    )
}

struct AdmissionOnlyBackend {
    inner: muniment::MemoryBackend,
    write_attempts: Arc<AtomicUsize>,
}

impl AdmissionOnlyBackend {
    fn refuse_write(&self) -> Result<(), muniment::StoreError> {
        self.write_attempts.fetch_add(1, Ordering::Relaxed);
        Err(muniment::StoreError::Backend(
            "incoming admission must not write".into(),
        ))
    }
}

#[async_trait::async_trait]
impl Backend for AdmissionOnlyBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, muniment::StoreError> {
        self.inner.get(key).await
    }

    async fn put(&self, _: &str, _: &[u8]) -> Result<(), muniment::StoreError> {
        self.refuse_write()
    }

    async fn delete(&self, _: &str) -> Result<(), muniment::StoreError> {
        self.refuse_write()
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, muniment::StoreError> {
        self.inner.list(prefix).await
    }

    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, muniment::StoreError> {
        self.inner.scan(start, end).await
    }

    async fn apply(&self, _: &[muniment::backend::WriteOp]) -> Result<(), muniment::StoreError> {
        self.refuse_write()
    }
}

fn stored_bytes(backend: &muniment::MemoryBackend) -> Vec<(String, Vec<u8>)> {
    let mut keys = ready(backend.list("")).unwrap();
    keys.sort();
    keys.into_iter()
        .map(|key| {
            let bytes = ready(backend.get(&key)).unwrap().unwrap();
            (key, bytes)
        })
        .collect()
}

fn inadmissible_candidate(
    fault: &str,
) -> (
    crate::storage::OpenedStore,
    muniment::MemoryBackend,
    Arc<AtomicUsize>,
) {
    use woodshed_core::loop_capture::{
        CAPTURE_VERSION, LoopCaptureCodec, LoopCaptureRef, PER_BEAT_CLOCK,
    };
    let bytes = muniment::MemoryBackend::default();
    let seed = SessionStore::new(bytes.clone());
    match fault {
        "session" => seed.try_save("not a session").unwrap(),
        "settings" => seed.try_save_settings("not settings").unwrap(),
        "workspace" => {
            let mut saved = UiState::new().to_persisted();
            saved.workspace_json = Some(r#"{"version":999,"future":"retained"}"#.into());
            seed.try_save(&serde_json::to_string(&saved).unwrap())
                .unwrap();
        },
        "missing-pcm" | "clock" => {
            let namespace = "0123456789abcdef0123456789abcdef";
            let (digest, frames) = seed
                .try_write_capture(namespace, 1_000, &vec![0.25; 2_000])
                .unwrap();
            let mut ui = UiState::new();
            ui.stage_current(None);
            ui.loop_captures.version = CAPTURE_VERSION;
            ui.loop_captures.namespace = Some(namespace.into());
            ui.loop_captures.takes.push(LoopCaptureRef {
                id: woodshed_core::loop_plan::LoopSegmentId {
                    set: ui.working_sets.active_id,
                    card: ui.set.cards[0].id,
                },
                bpm: if fault == "clock" { 60.0 } else { 120.0 },
                beats: 4,
                bars: 1,
                sample_rate: 1_000,
                frames,
                digest: if fault == "missing-pcm" {
                    "0".repeat(64)
                } else {
                    digest
                },
                codec: LoopCaptureCodec::RawF32V1,
                clock: Some(PER_BEAT_CLOCK.into()),
                seconds: None,
            });
            seed.try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
                .unwrap();
        },
        "owner" | "choice" => {},
        _ => panic!("unknown incoming fault {fault}"),
    }
    let write_attempts = Arc::new(AtomicUsize::new(0));
    let storage = SessionStore::new(Box::new(AdmissionOnlyBackend {
        inner: bytes.clone(),
        write_attempts: Arc::clone(&write_attempts),
    }) as crate::storage::HostBackend);
    let binding = match fault {
        "owner" => None,
        "choice" => Some(fixture_binding("another-persona")),
        _ => Some(fixture_binding("incoming")),
    };
    (
        crate::storage::OpenedStore {
            storage,
            seal: PracticeSeal::Sealed {
                persona: "incoming".into(),
                protection: "test".into(),
            },
            binding,
        },
        bytes,
        write_attempts,
    )
}

fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("memory store unexpectedly waited for external work"),
    }
}

fn pending(ui: &mut UiState, decision: SwitchDecision) {
    ui.pending_persona_switch = Some(PendingSwitch {
        profile: personae::ProfileId("incoming".into()),
        decision: Some(decision),
        waiting: false,
        notice: String::new(),
    });
}

fn fixture() -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, crate::sync::Logic, UiChild>,
    Arc<AtomicBool>,
    Rc<Cell<usize>>,
    muniment::MemoryBackend,
) {
    fixture_with_pending_opens(0)
}

fn fixture_with_pending_opens(
    pending_opens: usize,
) -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, crate::sync::Logic, UiChild>,
    Arc<AtomicBool>,
    Rc<Cell<usize>>,
    muniment::MemoryBackend,
) {
    fixture_with_initial_seals(vec![
        PracticeSeal::Pending {
            reason: "incoming custody unavailable".into(),
        };
        pending_opens
    ])
}

fn fixture_with_initial_seals(
    initial_seals: Vec<PracticeSeal>,
) -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, crate::sync::Logic, UiChild>,
    Arc<AtomicBool>,
    Rc<Cell<usize>>,
    muniment::MemoryBackend,
) {
    fixture_with_mode(initial_seals, false)
}

fn fixture_with_mode(
    initial_seals: Vec<PracticeSeal>,
    selection: bool,
) -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, crate::sync::Logic, UiChild>,
    Arc<AtomicBool>,
    Rc<Cell<usize>>,
    muniment::MemoryBackend,
) {
    fixture_with_candidate(initial_seals, selection, None, true)
}

fn fixture_with_candidate(
    initial_seals: Vec<PracticeSeal>,
    selection: bool,
    mut initial_candidate: Option<crate::storage::OpenedStore>,
    sync_tail: bool,
) -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, crate::sync::Logic, UiChild>,
    Arc<AtomicBool>,
    Rc<Cell<usize>>,
    muniment::MemoryBackend,
) {
    let refusing = Arc::new(AtomicBool::new(false));
    let bytes = muniment::MemoryBackend::default();
    let store = SessionStore::new(Box::new(crate::tests::SaveFailureBackend {
        inner: bytes.clone(),
        refusing: refusing.clone(),
    }) as crate::storage::HostBackend);
    let (shared, old_host) = crate::tests::save_close_harness(store);
    let saved = old_host.state().to_persisted();
    drop(old_host);
    let mut ui = UiState::new();
    ui.apply_persisted(&saved, Default::default());
    ui.seal = Some(PracticeSeal::Sealed {
        persona: "outgoing".into(),
        protection: "test".into(),
    });
    {
        let mut shared = shared.borrow_mut();
        shared.seal = ui.seal.clone();
        shared.data_root = Some(fixture_root());
        shared.store_binding = Some(fixture_binding("outgoing"));
    }
    crate::capture_persistence::begin_session(&mut shared.borrow_mut(), &ui).unwrap();
    let incoming = SessionStore::new(
        Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend
    );
    let opened = Rc::new(Cell::new(0));
    let open_count = opened.clone();
    let mut incoming = Some(crate::storage::OpenedStore {
        storage: incoming,
        seal: PracticeSeal::Sealed {
            persona: "incoming".into(),
            protection: "test".into(),
        },
        binding: Some(fixture_binding("incoming")),
    });
    let mut initial_seals = initial_seals.into_iter();
    let transition_shared = shared.clone();
    let mut hooks = crate::hooks(&shared);
    hooks.after_dispatch = Box::new(move |ctx| {
        let mut shared = transition_shared.borrow_mut();
        if selection {
            // Exercise the production picker settlement without the unrelated
            // dispatch tail's ordinary autosave/audio polling.
            after_dispatch(&mut shared, ctx);
            return;
        }
        resume_switch_with(&mut shared, ctx, |id| {
            assert_eq!(id.0, "incoming");
            open_count.set(open_count.get() + 1);
            if let Some(seal) = initial_seals.next() {
                let binding = match &seal {
                    PracticeSeal::Sealed { persona, .. } => Some(fixture_binding(persona)),
                    PracticeSeal::Pending { .. } => None,
                };
                return crate::storage::OpenedStore {
                    storage: SessionStore::new(
                        Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend
                    ),
                    seal,
                    binding,
                };
            }
            if let Some(candidate) = initial_candidate.take() {
                return candidate;
            }
            incoming.take().expect("one admitted switch")
        });
        if sync_tail {
            crate::sync::after_dispatch(&mut shared, ctx);
        }
    });
    let host = cambium_genet_winit_host::Harness::with_command_init(
        move |commands| {
            let commands = commands.clone();
            cambium_genet_winit_host::Init {
                sheet: crate::appearance::stylesheet(&ui),
                state: ui,
                logic: Box::new(move |ui: &UiState| {
                    crate::desktop_root(
                        ui,
                        &commands,
                        cambium_genet_winit_host::WindowFrame::Host,
                    )
                })
                    as crate::sync::Logic,
                fonts: Vec::new(),
                images: Vec::new(),
            }
        },
        hooks,
        cambium_genet_winit_host::HostOptions::default(),
    );
    (shared, host, refusing, opened, bytes)
}

fn choose(ui: &mut UiState, selected: &str, family_choice: &str) {
    let roster = Roster {
        entries: ["outgoing", "incoming"]
            .into_iter()
            .map(|id| dramatis::roster::RosterEntry {
                id: personae::ProfileId(id.into()),
                display_name: "Work".into(),
                slot_count: 1,
                chosen: id == family_choice,
            })
            .collect(),
        chosen: personae::ProfileId(family_choice.into()),
        description: "test vault".into(),
    };
    let mut pick = PersonaPick::switch(roster);
    pick.record(PickerEvent::Chose(personae::ProfileId(selected.into())));
    ui.persona = Some(pick);
}

#[test]
fn same_persona_selection_keeps_dirty_practice_without_preservation_or_reset() {
    let (shared, mut host, refusing, _opened, bytes) = fixture_with_mode(vec![], true);
    let epoch = host.state().audio_session.clone();
    let seal = host.state().seal.clone();
    let saved = ready(bytes.get("session")).unwrap();
    refusing.store(true, Ordering::Relaxed);
    host.update(|ui| {
        ui.set.cards[0].label = "unsaved outgoing instruction".into();
        ui.practice_saved = false;
        ui.practice_save_notice = Some("existing unconfirmed save".into());
        ui.practice_close_pending = true;
        ui.practice_discard_close = true;
        ui.loop_export_replace = Some((17, "outgoing.wav".into()));
        ui.enable_practice_clock();
        ui.event_clock = Some(|| Some(123_456));
        choose(ui, "outgoing", "outgoing");
    });
    let state = serde_json::to_value(host.state().to_persisted()).unwrap();
    host.after_dispatch();
    assert!(host.state().persona.is_none());
    assert!(host.state().pending_persona_switch.is_none());
    assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert_eq!(host.state().seal, seal);
    assert_eq!(shared.borrow().seal, seal);
    assert_eq!(
        serde_json::to_value(host.state().to_persisted()).unwrap(),
        state
    );
    assert!(!host.state().practice_saved);
    assert_eq!(
        host.state().practice_save_notice.as_deref(),
        Some("existing unconfirmed save")
    );
    assert!(host.state().practice_close_pending);
    assert!(host.state().practice_discard_close);
    assert_eq!(
        host.state().loop_export_replace,
        Some((17, "outgoing.wav".into()))
    );
    assert!(host.state().practice_clock_managed);
    assert!(crate::capture_persistence::session_current(
        &shared.borrow(),
        host.state()
    ));
    assert_eq!(ready(bytes.get("session")).unwrap(), saved);
    assert!(
        shared
            .borrow()
            .storage
            .as_ref()
            .unwrap()
            .save_problem()
            .is_none()
    );
    host.update(UiState::refresh_event_time);
    assert_eq!(host.state().now_ms, Some(123_456));
}

#[test]
fn persona_selection_keeps_real_family_and_store_transitions_on_the_preservation_path() {
    for case in 0..8 {
        let (shared, mut host, refusing, _opened, _bytes) = fixture_with_mode(vec![], true);
        refusing.store(true, Ordering::Relaxed);
        let selected = if case == 0 { "incoming" } else { "outgoing" };
        let family_choice = if case <= 1 { "incoming" } else { "outgoing" };
        match case {
            2 => {
                shared.borrow_mut().seal = Some(PracticeSeal::Pending {
                    reason: "locked".into(),
                })
            },
            3 => shared.borrow_mut().seal = None,
            4 => shared.borrow_mut().storage = None,
            5 => shared.borrow_mut().store_binding = None,
            6 => shared.borrow_mut().data_root = Some(fixture_root().join("changed-root")),
            7 => {
                let mut shared = shared.borrow_mut();
                let binding = shared.store_binding.as_ref().unwrap();
                shared.store_binding = Some(crate::storage::StoreBinding::for_test(
                    binding.root().to_path_buf(),
                    binding.owner().into(),
                    personae::ProfileId("reused-local-profile".into()),
                ));
            },
            _ => {},
        }
        host.update(|ui| {
            ui.set.cards[0].label = "retain outgoing work".into();
            choose(ui, selected, family_choice);
        });
        let epoch = host.state().audio_session.clone();
        host.after_dispatch();
        let pending = host
            .state()
            .pending_persona_switch
            .as_ref()
            .expect("a real transition must preserve outgoing practice");
        assert_eq!(pending.profile.0, selected);
        assert!(pending.decision.is_none());
        assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
        assert_eq!(host.state().set.cards[0].label, "retain outgoing work");
    }
}

#[test]
fn incoming_admission_failure_retains_outgoing_drafts_export_and_store_until_explicit_decision() {
    for (fault, reason) in [
        ("session", "session"),
        ("settings", "settings"),
        ("workspace", "workspace"),
        ("missing-pcm", "missing"),
        ("clock", "recorded clock"),
        ("owner", "no admitted owner"),
        ("choice", "differs"),
    ] {
        let (candidate, incoming_bytes, incoming_writes) = inadmissible_candidate(fault);
        let incoming_before = stored_bytes(&incoming_bytes);
        // Only the production transition hook runs here. Autosave and audio
        // polling are independent seams and must not obscure admission writes.
        let (shared, mut host, _refusing, opened, outgoing_bytes) =
            fixture_with_candidate(vec![], false, Some(candidate), false);
        let outgoing_before = stored_bytes(&outgoing_bytes);
        let epoch = host.state().audio_session.clone();
        let seal = host.state().seal.clone();
        let outgoing_owner = shared
            .borrow()
            .store_binding
            .as_ref()
            .unwrap()
            .owner()
            .to_owned();
        host.update(|ui| {
            ui.set.cards[0].label = "unsaved instruction before incoming admission".into();
            ui.stage.lens = woodshed_core::Lens::Progressions;
            ui.stage.select_progression(0);
            ui.start_arrangement();
            let owner = ui.working_sets.active_id;
            *ui.arrangement_editor.field_mut(owner, "name").unwrap() =
                cambium::TextInput::new("unfinished outgoing arrangement");
            ui.sync_arrangement_scene();
            assert!(ui.arrangement_drafts_dirty());
            ui.practice_saved = false;
            ui.practice_save_notice = Some("outgoing save still unconfirmed".into());
            ui.loop_export_replace = Some((17, "outgoing.wav".into()));
            ui.loop_export_busy = true;
            ui.loop_export_notice = Some("outgoing export decision".into());
            // Even a discard authorizes retirement only after the incoming
            // store is admitted. Its failure must retain these live drafts.
            pending(ui, SwitchDecision::Discard);
        });
        let outgoing_ui = serde_json::to_value(host.state().to_persisted()).unwrap();
        let outgoing_settings = host.state().app_settings.clone();
        let draft_owner = host.state().working_sets.active_id;
        host.after_dispatch();

        assert_eq!(opened.get(), 1, "{fault}");
        assert!(Arc::ptr_eq(&epoch, &host.state().audio_session), "{fault}");
        assert_eq!(host.state().seal, seal, "{fault}");
        assert_eq!(shared.borrow().seal, seal, "{fault}");
        assert_eq!(
            serde_json::to_value(host.state().to_persisted()).unwrap(),
            outgoing_ui,
            "{fault}"
        );
        assert_eq!(host.state().app_settings, outgoing_settings, "{fault}");
        assert!(host.state().arrangement_drafts_dirty(), "{fault}");
        assert_eq!(
            host.state()
                .arrangement_editor
                .field(draft_owner, "name")
                .unwrap()
                .text(),
            "unfinished outgoing arrangement",
            "{fault}"
        );
        assert!(!host.state().practice_saved, "{fault}");
        assert_eq!(
            host.state().practice_save_notice.as_deref(),
            Some("outgoing save still unconfirmed"),
            "{fault}"
        );
        assert_eq!(
            host.state().loop_export_replace,
            Some((17, "outgoing.wav".into())),
            "{fault}"
        );
        assert!(host.state().loop_export_busy, "{fault}");
        assert_eq!(
            host.state().loop_export_notice.as_deref(),
            Some("outgoing export decision"),
            "{fault}"
        );
        assert!(
            crate::capture_persistence::session_current(&shared.borrow(), host.state()),
            "{fault}"
        );
        assert_eq!(
            shared.borrow().store_binding.as_ref().unwrap().owner(),
            outgoing_owner,
            "{fault}"
        );
        assert_eq!(stored_bytes(&outgoing_bytes), outgoing_before, "{fault}");
        assert_eq!(stored_bytes(&incoming_bytes), incoming_before, "{fault}");
        assert_eq!(incoming_writes.load(Ordering::Relaxed), 0, "{fault}");
        let target = host.state().pending_persona_switch.as_ref().unwrap();
        assert_eq!(target.profile.0, "incoming", "{fault}");
        assert!(target.decision.is_none() && !target.waiting, "{fault}");
        assert!(target.notice.contains(reason), "{fault}: {}", target.notice);
        assert!(
            target.notice.contains("Outgoing practice remains active"),
            "{fault}"
        );

        host.after_dispatch();
        assert_eq!(opened.get(), 1, "{fault}: no implicit incoming retry");
        host.update(|ui| {
            ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Retry);
        });
        host.after_dispatch();
        assert_eq!(
            opened.get(),
            1,
            "{fault}: retained drafts still require preservation"
        );
        assert!(Arc::ptr_eq(&epoch, &host.state().audio_session), "{fault}");
        assert!(host.state().arrangement_drafts_dirty(), "{fault}");
        assert_eq!(stored_bytes(&outgoing_bytes), outgoing_before, "{fault}");
        assert_eq!(stored_bytes(&incoming_bytes), incoming_before, "{fault}");
        assert_eq!(incoming_writes.load(Ordering::Relaxed), 0, "{fault}");

        host.update(|ui| {
            ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Discard);
        });
        host.after_dispatch();
        assert_eq!(opened.get(), 2, "{fault}");
        assert!(host.state().pending_persona_switch.is_none(), "{fault}");
        assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session), "{fault}");
        assert!(host.state().set.cards.is_empty(), "{fault}");
        assert!(!host.state().arrangement_drafts_dirty(), "{fault}");
        assert!(host.state().loop_export_replace.is_none(), "{fault}");
        assert_eq!(stored_bytes(&outgoing_bytes), outgoing_before, "{fault}");
        assert_eq!(stored_bytes(&incoming_bytes), incoming_before, "{fault}");
        assert_eq!(incoming_writes.load(Ordering::Relaxed), 0, "{fault}");
    }
}

#[test]
fn mismatched_incoming_seal_retains_outgoing_work_and_selected_target_until_retry() {
    let (shared, mut host, _refusing, opened, _bytes) =
        fixture_with_initial_seals(vec![PracticeSeal::Sealed {
            persona: "another-persona".into(),
            protection: "test".into(),
        }]);
    let epoch = host.state().audio_session.clone();
    let seal = host.state().seal.clone();
    host.update(|ui| {
        ui.enable_practice_clock();
        ui.event_clock = Some(|| Some(123_456));
        ui.set.cards[0].label = "outgoing instruction retained across identity mismatch".into();
        ui.loop_export_replace = Some((17, "outgoing.wav".into()));
        pending(ui, SwitchDecision::Retry);
    });
    host.after_dispatch();
    assert_eq!(opened.get(), 1);
    assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert_eq!(host.state().seal, seal);
    assert_eq!(shared.borrow().seal, seal);
    assert_eq!(
        host.state().set.cards[0].label,
        "outgoing instruction retained across identity mismatch"
    );
    assert_eq!(
        host.state().loop_export_replace,
        Some((17, "outgoing.wav".into()))
    );
    assert!(crate::capture_persistence::session_current(
        &shared.borrow(),
        host.state()
    ));
    let outgoing = shared.borrow().storage.as_ref().unwrap().load().unwrap();
    assert_eq!(
        woodshed_core::storage::decode_session(&outgoing)
            .unwrap()
            .session
            .set
            .cards[0]
            .label,
        "outgoing instruction retained across identity mismatch"
    );
    let pending = host.state().pending_persona_switch.as_ref().unwrap();
    assert_eq!(pending.profile.0, "incoming");
    assert!(pending.decision.is_none());
    assert!(!pending.waiting);
    assert!(pending.notice.contains("another-persona"));
    assert!(pending.notice.contains("selected persona \"incoming\""));
    host.after_dispatch();
    assert_eq!(
        opened.get(),
        1,
        "an identity mismatch awaits an explicit decision"
    );
    host.update(|ui| {
        ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Retry);
    });
    host.after_dispatch();
    assert_eq!(opened.get(), 2);
    assert!(host.state().pending_persona_switch.is_none());
    assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert!(host.state().set.cards.is_empty());
    assert!(
        matches!(host.state().seal.as_ref(), Some(PracticeSeal::Sealed { persona, .. }) if persona == "incoming")
    );
    assert!(host.state().practice_clock_managed);
    host.update(UiState::refresh_event_time);
    assert_eq!(host.state().now_ms, Some(123_456));
}

#[test]
fn pending_incoming_retains_outgoing_authority_and_target_until_explicit_retry() {
    let (shared, mut host, _refusing, opened, bytes) = fixture_with_pending_opens(1);
    let epoch = host.state().audio_session.clone();
    let seal = host.state().seal.clone();
    let card = serde_json::to_value(&host.state().set.cards[0]).unwrap();
    let outgoing = shared.borrow().storage.as_ref().unwrap().load();
    host.update(|ui| {
        ui.loop_export_replace = Some((17, "outgoing.wav".into()));
        pending(ui, SwitchDecision::Retry);
    });
    host.after_dispatch();
    assert_eq!(opened.get(), 1);
    assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert_eq!(host.state().seal, seal);
    assert_eq!(shared.borrow().seal, seal);
    assert_eq!(
        serde_json::to_value(&host.state().set.cards[0]).unwrap(),
        card
    );
    assert_eq!(shared.borrow().storage.as_ref().unwrap().load(), outgoing);
    assert_eq!(
        host.state().loop_export_replace,
        Some((17, "outgoing.wav".into()))
    );
    assert!(crate::capture_persistence::session_current(
        &shared.borrow(),
        host.state()
    ));
    let target = host.state().pending_persona_switch.as_ref().unwrap();
    assert_eq!(target.profile.0, "incoming");
    assert!(target.decision.is_none());
    assert!(!target.waiting);
    assert!(target.notice.contains("incoming custody unavailable"));
    assert!(target.notice.contains("Outgoing practice remains active"));
    assert!(
        ready(muniment::Backend::get(&bytes, "session"))
            .unwrap()
            .is_some()
    );
    host.after_dispatch();
    assert_eq!(
        opened.get(),
        1,
        "a pending opener must await an explicit decision"
    );
    host.update(|ui| {
        ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Retry);
    });
    host.after_dispatch();
    assert_eq!(opened.get(), 2);
    assert!(host.state().pending_persona_switch.is_none());
    assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert!(host.state().set.cards.is_empty());
    assert!(
        matches!(host.state().seal.as_ref(), Some(PracticeSeal::Sealed { persona, .. }) if persona == "incoming")
    );
}

#[test]
fn pending_incoming_after_discard_keeps_live_work_and_requires_another_explicit_decision() {
    for decision in [SwitchDecision::Cancel, SwitchDecision::Discard] {
        let (shared, mut host, refusing, opened, bytes) = fixture_with_pending_opens(1);
        let epoch = host.state().audio_session.clone();
        let seal = host.state().seal.clone();
        let prior = ready(muniment::Backend::get(&bytes, "session")).unwrap();
        refusing.store(true, Ordering::Relaxed);
        host.update(|ui| {
            ui.set.cards[0].label = "unsaved work while incoming is unavailable".into();
            pending(ui, SwitchDecision::Discard);
        });
        host.after_dispatch();
        assert_eq!(opened.get(), 1);
        assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
        assert_eq!(host.state().seal, seal);
        assert_eq!(shared.borrow().seal, seal);
        assert_eq!(
            host.state().set.cards[0].label,
            "unsaved work while incoming is unavailable"
        );
        assert!(host.state().pending_persona_switch.is_some());
        assert_eq!(
            ready(muniment::Backend::get(&bytes, "session")).unwrap(),
            prior
        );
        // Retry must still preserve the outgoing owner. An earlier Discard is
        // not a standing permission to lose edits made during incoming failure.
        host.update(|ui| {
            ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Retry);
        });
        host.after_dispatch();
        assert_eq!(
            opened.get(),
            1,
            "outgoing save refusal blocks an ordinary retry"
        );
        host.update(|ui| ui.pending_persona_switch.as_mut().unwrap().decision = Some(decision));
        host.after_dispatch();
        assert_eq!(
            ready(muniment::Backend::get(&bytes, "session")).unwrap(),
            prior
        );
        assert!(host.state().pending_persona_switch.is_none());
        if decision == SwitchDecision::Cancel {
            assert_eq!(opened.get(), 1);
            assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
            assert_eq!(
                host.state().set.cards[0].label,
                "unsaved work while incoming is unavailable"
            );
        } else {
            assert_eq!(opened.get(), 2);
            assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session));
            assert!(host.state().set.cards.is_empty());
        }
    }
}

#[test]
fn failed_save_keeps_outgoing_epoch_store_and_cards_until_explicit_retry() {
    let (shared, mut host, refusing, opened, bytes) = fixture();
    let epoch = host.state().audio_session.clone();
    let prior = ready(muniment::Backend::get(&bytes, "session")).unwrap();
    refusing.store(true, Ordering::Relaxed);
    host.update(|ui| {
        ui.set.cards[0].label = "unsaved outgoing instruction".into();
        pending(ui, SwitchDecision::Retry);
    });
    host.after_dispatch();
    assert_eq!(opened.get(), 0);
    assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert_eq!(
        host.state().set.cards[0].label,
        "unsaved outgoing instruction"
    );
    assert_eq!(
        ready(muniment::Backend::get(&bytes, "session")).unwrap(),
        prior
    );
    host.layout_at(1100.0, 900.0);
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    assert!(
        !host.close_requested(),
        "close cannot bypass the pending switch decision"
    );
    refusing.store(false, Ordering::Relaxed);
    assert!(host.click_on(&taproot::Selector::class("persona-save-retry")));
    assert_eq!(opened.get(), 1);
    assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert!(host.state().pending_persona_switch.is_none());
    let outgoing = String::from_utf8(
        ready(muniment::Backend::get(&bytes, "session"))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        woodshed_core::storage::decode_session(&outgoing)
            .unwrap()
            .session
            .set
            .cards[0]
            .label,
        "unsaved outgoing instruction"
    );
    assert!(
        shared
            .borrow()
            .storage
            .as_ref()
            .unwrap()
            .save_problem()
            .is_none()
    );
}

#[test]
fn cancel_and_discard_are_distinct_and_do_not_overwrite_outgoing_storage() {
    for decision in [SwitchDecision::Cancel, SwitchDecision::Discard] {
        let (_shared, mut host, refusing, opened, bytes) = fixture();
        let prior = ready(muniment::Backend::get(&bytes, "session")).unwrap();
        let epoch = host.state().audio_session.clone();
        refusing.store(true, Ordering::Relaxed);
        host.update(|ui| {
            ui.set.cards[0].label = "only in memory".into();
            pending(ui, SwitchDecision::Retry);
        });
        host.after_dispatch();
        host.update(|ui| ui.pending_persona_switch.as_mut().unwrap().decision = Some(decision));
        host.after_dispatch();
        assert_eq!(
            ready(muniment::Backend::get(&bytes, "session")).unwrap(),
            prior
        );
        if decision == SwitchDecision::Cancel {
            assert_eq!(opened.get(), 0);
            assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
            assert_eq!(host.state().set.cards[0].label, "only in memory");
        } else {
            assert_eq!(opened.get(), 1);
            assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session));
            assert!(host.state().set.cards.is_empty());
        }
    }
}

#[test]
fn quarantined_store_cannot_retry_into_a_switch_and_reset_retains_event_clock() {
    let (shared, mut host, _refusing, opened, _) = fixture();
    shared
        .borrow()
        .storage
        .as_ref()
        .unwrap()
        .refuse_writes("invalid retained original");
    host.update(|ui| pending(ui, SwitchDecision::Retry));
    host.after_dispatch();
    assert_eq!(opened.get(), 0);
    assert!(
        !host
            .state()
            .pending_persona_switch
            .as_ref()
            .unwrap()
            .waiting
    );
    let mut ui = UiState::new();
    ui.event_clock = Some(|| Some(123_456));
    reset_for_persona(&mut ui);
    ui.refresh_event_time();
    assert_eq!(ui.now_ms, Some(123_456));
}
