#[test]
fn midi_pedal_real_controls_suppress_key_repeat_and_refuse_disabled_or_disconnected() {
    use woodshed_core::midi::PedalAction;
    let mut ui = UiState::new();
    ui.section = woodshed_core::storage::AppSection::Settings;
    ui.app_settings.page = woodshed_core::settings::SettingsPage::AudioMidi;
    ui.midi.output_ready = true;
    ui.app_settings.audio_midi.midi_pedal.effect_next.enabled = true;
    ui.app_settings
        .audio_midi
        .midi_pedal
        .effect_previous
        .enabled = true;
    let mut host = cambium_genet_winit_host::Harness::with_command_init(
        move |commands| {
            let commands = commands.clone();
            Init {
                sheet: appearance::stylesheet(&ui),
                state: ui,
                logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands, WindowFrame::Host))
                    as Logic,
                fonts: vec![],
                images: vec![],
            }
        },
        HostHooks {
            key_intercept: Box::new(escape_policy),
            ..cambium_genet_winit_host::inert_hooks()
        },
        HostOptions::default(),
    );
    host.layout_at(1180.0, 1050.0);
    for action in [PedalAction::EffectNext, PedalAction::EffectPrevious] {
        host.update(|ui| ui.midi.pedal_request = None);
        assert!(host.click_on(
            &taproot::Selector::class("t-btn").containing(format!("Trigger {}", action.label()))
        ));
        assert_eq!(host.state().midi.pedal_request, Some(action));
        for key in [NamedKey::Enter, NamedKey::Space] {
            host.update(|ui| ui.midi.pedal_request = None);
            let mut press = KeyPress::new(Key::Named(key));
            press.repeat = true;
            for _ in 0..8 {
                host.press_key(&press);
                assert_eq!(host.state().midi.pedal_request, None);
            }
            press.repeat = false;
            host.press_key(&press);
            assert_eq!(host.state().midi.pedal_request, Some(action));
        }
    }
    host.update(|ui| {
        ui.midi.pedal_request = None;
        ui.app_settings
            .audio_midi
            .midi_pedal
            .effect_previous
            .enabled = false;
    });
    host.press_key(&KeyPress::new(Key::Named(NamedKey::Enter)));
    assert!(host.state().midi.pedal_request.is_none());
    host.update(|ui| {
        ui.app_settings
            .audio_midi
            .midi_pedal
            .effect_previous
            .enabled = true;
        ui.midi.output_ready = false;
    });
    host.press_key(&KeyPress::new(Key::Named(NamedKey::Space)));
    assert!(host.state().midi.pedal_request.is_none());
}

/// These layout/persistence fixtures model an accepted host operation;
/// actual runtime refusal and bound command routing have separate tests.
pub(crate) fn acknowledge_card_preview_fixture(ui: &mut woodshed_views::stage::UiState) {
    let requests = ui.audio_requests.clone();
    for request in requests {
        if let woodshed_core::audio::AudioRequest::CardPreview(request) = request {
            if request.run_epoch.is_some() {
                ui.finish_card_preview_admission(&request, Ok(()));
            }
        }
    }
}

use super::*;

#[derive(Clone)]
pub(super) struct SaveFailureBackend {
    pub(super) inner: muniment::MemoryBackend,
    pub(super) refusing: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait::async_trait]
impl muniment::Backend for SaveFailureBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, muniment::StoreError> {
        self.inner.get(key).await
    }
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), muniment::StoreError> {
        if self.refusing.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(muniment::StoreError::Backend("test write refused".into()));
        }
        self.inner.put(key, bytes).await
    }
    async fn delete(&self, key: &str) -> Result<(), muniment::StoreError> {
        self.inner.delete(key).await
    }
    async fn list(&self, prefix: &str) -> Result<Vec<String>, muniment::StoreError> {
        self.inner.list(prefix).await
    }
    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, muniment::StoreError> {
        self.inner.scan(start, end).await
    }
    async fn apply(&self, ops: &[muniment::backend::WriteOp]) -> Result<(), muniment::StoreError> {
        self.inner.apply(ops).await
    }
}

pub(super) fn save_close_harness(
    storage: woodshed_core::storage::SessionStore<storage::HostBackend>,
) -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, Logic, UiChild>,
) {
    save_close_harness_with_frame_exit(storage, false)
}

fn save_close_harness_with_frame_exit(
    storage: woodshed_core::storage::SessionStore<storage::HostBackend>,
    mut request_frame_exit: bool,
) -> (
    Rc<RefCell<Shared>>,
    cambium_genet_winit_host::Harness<UiState, Logic, UiChild>,
) {
    let seal = woodshed_views::persona::PracticeSeal::Sealed {
        persona: "test".into(),
        protection: "test".into(),
    };
    let shared = Shared::with_store(Some(storage), Some(seal.clone()), None);
    let mut ui = UiState::new();
    ui.seal = Some(seal);
    ui.stage_current(None);
    let mut host_hooks = hooks(&shared);
    let mut frame = host_hooks.after_frame;
    host_hooks.after_frame = Box::new(move |ctx| {
        if std::mem::take(&mut request_frame_exit) {
            *ctx.close = true;
        }
        frame(ctx);
    });
    let mut host = cambium_genet_winit_host::Harness::with_command_init(
        move |commands| {
            let commands = commands.clone();
            Init {
                sheet: appearance::stylesheet(&ui),
                state: ui,
                logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands, WindowFrame::Host))
                    as Logic,
                fonts: Vec::new(),
                images: Vec::new(),
            }
        },
        host_hooks,
        HostOptions::default(),
    );
    host.layout_at(1100.0, 900.0);
    host.after_dispatch();
    (shared, host)
}

#[test]
fn frame_requested_exit_keeps_quarantined_work_until_explicit_discard() {
    let store = woodshed_core::storage::SessionStore::new(Box::new(
        muniment::MemoryBackend::default(),
    ) as storage::HostBackend);
    let original = serde_json::to_string(&UiState::new().to_persisted()).unwrap();
    store.try_save(&original).unwrap();
    store.refuse_writes("retained input is quarantined");
    let (shared, mut host) = save_close_harness_with_frame_exit(store, true);
    let cards = serde_json::to_value(&host.state().set.cards).unwrap();
    host.after_frame();
    host.relayout();
    assert!(!host.close_requested());
    assert!(host.state().practice_close_pending);
    assert_eq!(
        serde_json::to_value(&host.state().set.cards).unwrap(),
        cards
    );
    assert_eq!(
        shared.borrow().storage.as_ref().unwrap().load().unwrap(),
        original
    );
    assert!(host.click_on(&taproot::Selector::class("practice-close-retry")));
    assert!(!host.close_requested());
    assert_eq!(
        serde_json::to_value(&host.state().set.cards).unwrap(),
        cards
    );
    assert!(host.click_on(&taproot::Selector::class("practice-close-discard")));
    assert!(host.close_requested());
    assert_eq!(
        shared.borrow().storage.as_ref().unwrap().load().unwrap(),
        original
    );
}

#[test]
fn quarantined_close_can_cancel_retry_and_explicitly_discard_without_writing() {
    let store = woodshed_core::storage::SessionStore::new(Box::new(
        muniment::MemoryBackend::default(),
    ) as storage::HostBackend);
    let original = serde_json::to_string(&UiState::new().to_persisted()).unwrap();
    store.try_save(&original).unwrap();
    store.refuse_writes("original admission is uncertain");
    let (shared, mut host) = save_close_harness(store);
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    host.relayout();
    assert!(!host.close_requested());
    assert!(host.state().practice_close_pending);
    assert!(host.click_on(&taproot::Selector::class("practice-close-cancel")));
    assert!(!host.state().practice_close_pending);
    assert!(!host.close_requested());
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    host.relayout();
    assert!(host.click_on(&taproot::Selector::class("practice-close-retry")));
    assert!(!host.close_requested());
    assert!(host.state().practice_close_pending);
    assert!(host.click_on(&taproot::Selector::class("practice-close-discard")));
    assert!(host.close_requested());
    assert_eq!(
        shared.borrow().storage.as_ref().unwrap().load().as_deref(),
        Some(original.as_str())
    );
}

#[test]
fn retry_after_transient_write_failure_verifies_both_slots_before_closing() {
    let refusing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let backend = SaveFailureBackend {
        inner: Default::default(),
        refusing: refusing.clone(),
    };
    let store =
        woodshed_core::storage::SessionStore::new(Box::new(backend) as storage::HostBackend);
    let (shared, mut host) = save_close_harness(store);
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    host.relayout();
    assert!(!host.close_requested());
    assert!(!host.state().practice_saved);
    refusing.store(false, std::sync::atomic::Ordering::Relaxed);
    assert!(host.click_on(&taproot::Selector::class("practice-close-retry")));
    assert!(host.close_requested());
    assert!(host.state().practice_saved);
    let shared = shared.borrow();
    let store = shared.storage.as_ref().unwrap();
    assert!(store.save_problem().is_none());
    assert_eq!(
        woodshed_core::storage::decode_session(&store.load().unwrap())
            .unwrap()
            .session
            .set
            .cards
            .len(),
        1
    );
    assert!(store.load_settings().is_some());
}

#[test]
fn discarded_practice_stays_unwritten_until_appearance_close_finishes_or_is_cancelled() {
    for cancel in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let refusing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let backend = SaveFailureBackend {
            inner: Default::default(),
            refusing: refusing.clone(),
        };
        let store =
            woodshed_core::storage::SessionStore::new(Box::new(backend) as storage::HostBackend);
        let (shared, mut host) = save_close_harness(store);
        host.update(|ui| {
            ui.appearance = woodshed_views::appearance::AppearanceState::load(
                directory.path().join("themes.json"),
            )
            .unwrap();
            ui.appearance
                .begin_edit(&ui.app_settings.appearance)
                .unwrap();
            ui.appearance.workshop.new_copy();
        });
        host.request_close(cambium_genet_winit_host::CloseRequest::Native);
        host.relayout();
        assert!(host.click_on(&taproot::Selector::class("practice-close-discard")));
        assert!(!host.close_requested());
        assert!(host.state().practice_discard_close);
        assert!(host.state().appearance_close_app);
        refusing.store(false, std::sync::atomic::Ordering::Relaxed);
        host.after_dispatch();
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
        let action = if cancel {
            "cancel-close"
        } else {
            "discard-close"
        };
        assert!(host.click_on(&taproot::Selector::role("button").with_attr("data-action", action)));
        if cancel {
            assert!(!host.close_requested());
            assert!(!host.state().practice_discard_close);
            host.after_dispatch();
            assert!(shared.borrow().storage.as_ref().unwrap().load().is_some());
        } else {
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
    }
}

#[test]
fn window_geometry_conversion_preserves_every_axis() {
    let host = cambium_genet_winit_host::WindowGeometry {
        position: (120.5, 80.25),
        size: (900.0, 640.0),
        maximized: true,
    };
    assert_eq!(to_host_geometry(to_window_settings(host)), host);
}
fn appearance_harness() -> cambium_genet_winit_host::Harness<UiState, Logic, UiChild> {
    appearance_harness_for_frame(WindowFrame::App)
}

fn appearance_harness_for_frame(
    frame: WindowFrame,
) -> cambium_genet_winit_host::Harness<UiState, Logic, UiChild> {
    let mut ui = UiState::new();
    ui.appearance_authoring_available = true;
    ui.appearance
        .begin_edit(&ui.app_settings.appearance)
        .unwrap();
    cambium_genet_winit_host::Harness::with_command_init(
        move |commands| {
            let commands = commands.clone();
            let sheet = appearance::stylesheet(&ui);
            Init {
                state: ui,
                logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands, frame)) as Logic,
                sheet,
                fonts: vec![],
                images: vec![],
            }
        },
        HostHooks {
            focused_text: Box::new(text::focused_text),
            after_dispatch: Box::new(appearance::after_dispatch),
            close_request: Box::new(|ctx, _| appearance::close_request(ctx)),
            ..cambium_genet_winit_host::inert_hooks()
        },
        HostOptions {
            window_frame: frame,
            ..Default::default()
        },
    )
}

#[test]
fn native_workshop_routes_name_and_stylesheet_text_through_owned_model() {
    use taproot::Selector;
    let mut h = appearance_harness();
    h.layout_at(1180.0, 2000.0);
    assert!(h.click_on(&Selector::role("button").with_attr("data-action", "toggle-stylesheet")));
    let name = Selector::role("textbox").with_attr("data-field", "name");
    assert!(h.click_on(&name));
    let before = h.state().appearance.workshop.draft_theme().name.clone();
    h.key_injected(" integration");
    assert_ne!(h.state().appearance.workshop.draft_theme().name, before);
    assert!(
        h.state()
            .appearance
            .workshop
            .draft_theme()
            .name
            .contains("integration")
    );
    let sheet = Selector::role("textbox").with_attr("data-field", "mode-sheet");
    // The stylesheet inspector owns the textarea; focus routing must also
    // work for textarea nodes, rather than only Woodshed's input fields.
    assert!(h.click_on(&sheet));
    assert!(text::focused_text(h.runner()).is_some());
    h.key_injected("body { color: #abcdef; }");
    assert!(
        h.state()
            .appearance
            .workshop
            .text_field("mode-sheet")
            .unwrap()
            .text()
            .contains("#abcdef")
    );
}

#[test]
fn shared_title_bar_fills_viewport_and_keeps_editor_below_it() {
    use taproot::Selector;
    let mut h = appearance_harness();
    for (width, height) in [(1180.0, 800.0), (640.0, 800.0)] {
        h.layout_at(width, height);
        let bar = h.with_dom(|dom| {
            taproot::matching_classes(dom, &Selector::class("cambium-title-bar"))[0]
        });
        let (x, y, w, _) = h.painted_rect(bar).unwrap();
        assert!(x.abs() < 1.0 && y.abs() < 1.0);
        assert!((w - width).abs() < 1.0, "titlebar {w} viewport {width}");
        let editor = h.with_dom(|dom| {
            taproot::matching_classes(dom, &Selector::class("woodshed-workshop"))[0]
        });
        let (_, top, _, _) = h.painted_rect(editor).unwrap();
        assert!(top >= 36.0);
    }
}

#[test]
fn native_frame_owns_window_ornamentation_without_hiding_application_content() {
    use layout_dom_api::LayoutDom as _;
    use taproot::Selector;
    let mut h = appearance_harness_for_frame(WindowFrame::Host);
    h.layout_at(1180.0, 800.0);
    let direct_chrome = h.with_dom(|dom| {
        let frame = taproot::matching_classes(dom, &Selector::class("desktop-frame"))[0];
        let direct = |class: &str| {
            taproot::matching_classes(dom, &Selector::class(class))
                .into_iter()
                .filter(|node| dom.parent(*node) == Some(frame))
                .count()
        };
        (
            direct("cambium-title-bar"),
            direct("window-caption-controls"),
        )
    });
    assert_eq!(direct_chrome, (0, 0));
    let editor =
        h.with_dom(|dom| taproot::matching_classes(dom, &Selector::class("woodshed-workshop"))[0]);
    assert!(h.visible_rect(editor).is_some());
}
