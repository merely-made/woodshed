//! Exercise the production transition hook with in-memory incoming custody.
use super::*;
use muniment::Backend;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use woodshed_core::storage::SessionStore;
use woodshed_views::persona::PracticeSeal;
use woodshed_views::stage::{UiChild, UiState};

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

fn fixture_with_pending_opens(pending_opens: usize) -> (
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

fn fixture_with_initial_seals(initial_seals: Vec<PracticeSeal>) -> (
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
    shared.borrow_mut().seal = ui.seal.clone();
    crate::capture_persistence::begin_session(&mut shared.borrow_mut(), &ui).unwrap();
    let incoming = SessionStore::new(
        Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend
    );
    let opened = Rc::new(Cell::new(0));
    let open_count = opened.clone();
    let mut incoming = Some(incoming);
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
                return (
                    SessionStore::new(
                        Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend
                    ),
                    seal,
                );
            }
            (
                incoming.take().expect("one admitted switch"),
                PracticeSeal::Sealed {
                    persona: "incoming".into(),
                    protection: "test".into(),
                },
            )
        });
        crate::sync::after_dispatch(&mut shared, ctx);
    });
    let host = cambium_genet_winit_host::Harness::with_command_init(
        move |commands| {
            let commands = commands.clone();
            cambium_genet_winit_host::Init {
                sheet: crate::appearance::stylesheet(&ui),
                state: ui,
                logic: Box::new(move |ui: &UiState| crate::desktop_root(ui, &commands))
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
    for case in 0..5 {
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
    assert_eq!(serde_json::to_value(&host.state().set.cards[0]).unwrap(), card);
    assert_eq!(shared.borrow().storage.as_ref().unwrap().load(), outgoing);
    assert_eq!(host.state().loop_export_replace, Some((17, "outgoing.wav".into())));
    assert!(crate::capture_persistence::session_current(&shared.borrow(), host.state()));
    let target = host.state().pending_persona_switch.as_ref().unwrap();
    assert_eq!(target.profile.0, "incoming");
    assert!(target.decision.is_none());
    assert!(!target.waiting);
    assert!(target.notice.contains("incoming custody unavailable"));
    assert!(target.notice.contains("Outgoing practice remains active"));
    assert!(ready(muniment::Backend::get(&bytes, "session")).unwrap().is_some());
    host.after_dispatch();
    assert_eq!(opened.get(), 1, "a pending opener must await an explicit decision");
    host.update(|ui| {
        ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Retry);
    });
    host.after_dispatch();
    assert_eq!(opened.get(), 2);
    assert!(host.state().pending_persona_switch.is_none());
    assert!(!Arc::ptr_eq(&epoch, &host.state().audio_session));
    assert!(host.state().set.cards.is_empty());
    assert!(matches!(host.state().seal.as_ref(), Some(PracticeSeal::Sealed { persona, .. }) if persona == "incoming"));
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
        assert_eq!(host.state().set.cards[0].label, "unsaved work while incoming is unavailable");
        assert!(host.state().pending_persona_switch.is_some());
        assert_eq!(ready(muniment::Backend::get(&bytes, "session")).unwrap(), prior);
        // Retry must still preserve the outgoing owner. An earlier Discard is
        // not a standing permission to lose edits made during incoming failure.
        host.update(|ui| {
            ui.pending_persona_switch.as_mut().unwrap().decision = Some(SwitchDecision::Retry);
        });
        host.after_dispatch();
        assert_eq!(opened.get(), 1, "outgoing save refusal blocks an ordinary retry");
        host.update(|ui| ui.pending_persona_switch.as_mut().unwrap().decision = Some(decision));
        host.after_dispatch();
        assert_eq!(ready(muniment::Backend::get(&bytes, "session")).unwrap(), prior);
        assert!(host.state().pending_persona_switch.is_none());
        if decision == SwitchDecision::Cancel {
            assert_eq!(opened.get(), 1);
            assert!(Arc::ptr_eq(&epoch, &host.state().audio_session));
            assert_eq!(host.state().set.cards[0].label, "unsaved work while incoming is unavailable");
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
