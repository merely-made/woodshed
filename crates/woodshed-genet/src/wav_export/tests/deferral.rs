use super::*;

#[test]
fn installed_root_protection_is_frozen_and_covers_uncreated_case_aliases() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("PrivateRoot");
    let shared = Shared::with_store(None, None, None);
    shared.borrow_mut().data_root = Some(root.clone());
    shared.borrow_mut().theme_library = Some(directory.path().join("AuthoredThemes.json"));
    let protection = Protection::current(&shared.borrow());
    shared.borrow_mut().data_root = Some(directory.path().join("AnotherRoot"));
    assert!(
        protection
            .check(&directory.path().join("privateroot"))
            .is_err()
    );
    assert!(
        protection
            .check(&directory.path().join("authoredthemes.json"))
            .is_err()
    );
    assert!(
        protection
            .check(&directory.path().join("ordinary.wav"))
            .is_ok()
    );
    assert!(!root.exists());
}

#[test]
fn deferred_dialog_cancel_and_epoch_revocation_preserve_destinations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chosen.wav");
    let mut exporter = WavExport::default();
    let mut ui = UiState::new();
    start_dialog(
        None,
        &mut exporter,
        ui.audio_session.clone(),
        song(0.1),
        48_000,
        "take.wav".into(),
        Protection::default(),
        |_| None,
    )
    .unwrap();
    let job = exporter.job.take().unwrap();
    exporter.complete(&mut ui, job.receiver.recv().unwrap());
    assert!(
        ui.loop_export_notice
            .as_ref()
            .unwrap()
            .contains("cancelled")
    );
    assert!(!path.exists());

    fs::write(&path, b"keep").unwrap();
    let target = path.clone();
    let (entered, ready) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    start_dialog(
        None,
        &mut exporter,
        ui.audio_session.clone(),
        song(0.1),
        48_000,
        "take.wav".into(),
        Protection::default(),
        move |_| {
            entered.send(()).unwrap();
            blocked.recv().unwrap();
            Some(target)
        },
    )
    .unwrap();
    ready.recv().unwrap();
    exporter.revoke_session();
    let saved = ui.to_persisted();
    ui.apply_persisted(&saved, ui.app_settings.clone());
    release.send(()).unwrap();
    let job = exporter.job.take().unwrap();
    assert!(
        job.receiver
            .recv()
            .unwrap()
            .err()
            .unwrap()
            .contains("cancelled")
    );
    assert!(ui.loop_export_notice.is_none());
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn deferred_export_close_preserves_the_dirty_appearance_gate() {
    let dir = tempfile::tempdir().unwrap();
    let store = woodshed_core::storage::SessionStore::new(Box::new(
        muniment::MemoryBackend::default(),
    ) as crate::storage::HostBackend);
    let shared = Shared::with_store(Some(store), None, None);
    let mut ui = UiState::new();
    crate::capture_persistence::begin_session(&mut shared.borrow_mut(), &ui).unwrap();
    ui.appearance =
        woodshed_views::appearance::AppearanceState::load(dir.path().join("themes.json")).unwrap();
    ui.appearance
        .begin_edit(&ui.app_settings.appearance)
        .unwrap();
    ui.appearance.workshop.new_copy();
    assert!(ui.appearance.workshop.has_changes());
    let (sender, receiver) = mpsc::channel();
    shared.borrow_mut().wav_export.job = Some(Job {
        session: ui.audio_session.clone(),
        receiver,
        cancelled: Arc::new(Mutex::new(false)),
    });
    let close_shared = shared.clone();
    let wake_shared = shared.clone();
    let mut hooks: HostHooks<UiState, crate::sync::Logic, woodshed_views::stage::UiChild> =
        HostHooks::inert();
    hooks.close_request = Box::new(move |ctx, _| {
        close_request(&mut close_shared.borrow_mut(), ctx)
            .unwrap_or_else(|| crate::appearance::close_request(ctx))
    });
    hooks.after_wake = Box::new(move |ctx| poll(&mut wake_shared.borrow_mut(), ctx));
    let logic: crate::sync::Logic = Box::new(woodshed_views::appearance::workshop_screen);
    let mut host = Harness::with_hooks(
        Init {
            sheet: crate::appearance::stylesheet(&ui),
            state: ui,
            logic,
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    host.layout_at(1280.0, 960.0);
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    assert!(!host.close_requested());
    sender
        .send(Err("fixture worker completion".into()))
        .unwrap();
    (host.wake().callback())();
    assert!(host.process_wake());
    assert!(!host.close_requested());
    assert!(host.state().appearance.workshop.has_changes());
    assert!(host.state().appearance.workshop.close_requested());
    assert!(host.state().appearance_close_app);
    assert!(!shared.borrow().wav_export.close_when_finished);
}

#[test]
fn export_completion_rechecks_later_save_failure_before_closing() {
    let refusing = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let backend = crate::tests::SaveFailureBackend {
        inner: Default::default(),
        refusing: refusing.clone(),
    };
    let store =
        woodshed_core::storage::SessionStore::new(Box::new(backend) as crate::storage::HostBackend);
    let (shared, mut host) = crate::tests::save_close_harness(store);
    let (sender, receiver) = mpsc::channel();
    shared.borrow_mut().wav_export.job = Some(Job {
        session: host.state().audio_session.clone(),
        receiver,
        cancelled: Arc::new(Mutex::new(false)),
    });
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    assert!(!host.close_requested());
    refusing.store(true, std::sync::atomic::Ordering::Relaxed);
    host.update(|ui| ui.stage_current(None));
    host.after_dispatch();
    sender.send(Err("fixture completion".into())).unwrap();
    (host.wake().callback())();
    assert!(host.process_wake());
    assert!(!host.close_requested());
    assert!(host.state().practice_close_pending);
    assert_eq!(host.state().set.cards.len(), 2);
    let persisted = shared.borrow().storage.as_ref().unwrap().load().unwrap();
    assert_eq!(
        woodshed_core::storage::decode_session(&persisted)
            .unwrap()
            .session
            .set
            .cards
            .len(),
        1
    );
}

#[test]
fn explicit_discard_survives_export_deferral_without_later_autosave() {
    let refusing = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let backend = crate::tests::SaveFailureBackend {
        inner: Default::default(),
        refusing: refusing.clone(),
    };
    let store =
        woodshed_core::storage::SessionStore::new(Box::new(backend) as crate::storage::HostBackend);
    let (shared, mut host) = crate::tests::save_close_harness(store);
    let (sender, receiver) = mpsc::channel();
    shared.borrow_mut().wav_export.job = Some(Job {
        session: host.state().audio_session.clone(),
        receiver,
        cancelled: Arc::new(Mutex::new(false)),
    });
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    host.relayout();
    assert!(host.click_on(&taproot::Selector::class("practice-close-discard")));
    assert!(host.state().practice_discard_close);
    assert!(!host.close_requested());
    refusing.store(false, std::sync::atomic::Ordering::Relaxed);
    host.after_dispatch();
    assert!(shared.borrow().storage.as_ref().unwrap().load().is_none());
    sender.send(Err("fixture completion".into())).unwrap();
    (host.wake().callback())();
    assert!(host.process_wake());
    assert!(host.close_requested());
    assert!(shared.borrow().storage.as_ref().unwrap().load().is_none());
    assert!(
        shared
            .borrow()
            .storage
            .as_ref()
            .unwrap()
            .load_settings()
            .is_none()
    );
}

#[test]
fn native_chooser_cancellation_is_one_request_and_writes_nothing() {
    let shared = Shared::with_store(None, None, None);
    let mut ui = UiState::new();
    ui.song.bars.push(woodshed_core::song::SongBar::default());
    ui.select_app_section(woodshed_core::storage::AppSection::Looper);
    {
        let mut shared = shared.borrow_mut();
        let mut backend = crate::audio::CpalBackend::offline_for_test();
        backend.try_set_song(&ui.song).unwrap();
        shared.backend = Some(backend);
        shared.last_song = Some(ui.song.clone());
        shared.last_audio_session = Some(ui.audio_session.clone());
    }
    let chosen = Rc::new(Cell::new(0));
    let calls = chosen.clone();
    let binding = shared.clone();
    let mut hooks: HostHooks<UiState, crate::sync::Logic, woodshed_views::stage::UiChild> =
        HostHooks::inert();
    hooks.after_dispatch = Box::new(move |ctx| {
        after_dispatch_with_chooser(&mut binding.borrow_mut(), ctx, |_| {
            calls.set(calls.get() + 1);
            None
        });
    });
    let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
    let mut host = Harness::with_hooks(
        Init {
            sheet: crate::appearance::stylesheet(&ui),
            state: ui,
            logic,
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    host.layout_at(1280.0, 960.0);
    assert!(host.click_on(&Selector::class("t-btn").containing("Export WAV")));
    assert_eq!(chosen.get(), 1);
    assert!(
        host.state()
            .loop_export_notice
            .as_ref()
            .unwrap()
            .contains("cancelled")
    );
    assert!(!host.state().loop_export_busy);
    assert!(host.state().loop_export_requests.is_empty());
    assert!(shared.borrow().wav_export.job.is_none());
    assert!(shared.borrow().wav_export.pending.is_none());
}
