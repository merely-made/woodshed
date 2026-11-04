use super::*;

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
