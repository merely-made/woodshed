//! Woodshed's desktop application.
//!
//! There is no native host in this crate any more. `cambium-genet-winit-host`
//! owns the winit lifecycle, the genet surface, the retained layout, the paint
//! pass, hit testing, pointer/keyboard/IME/wheel routing, the overlay-scrollbar
//! fade, and the AccessKit install-before-show lifecycle — all of it extracted
//! from this file, which woodshed was the donor for. Woodshed is now its first
//! consumer.
//!
//! What is left here is woodshed: which views to render, which state they run
//! over, the audio and MIDI seams, persistence, the custom-paint leaves, and
//! the self-drive lane. It reaches the host through seven plain closures
//! ([`HostHooks`]) with its own state in their captured environment.
//!
//! | hook | woodshed's half |
//! |------|-----------------|
//! | `frame` | advance the transports and the tuner ([`drive`]), refresh the leaves ([`leaves`]) |
//! | `after_dispatch` | push state through the backend, MIDI, chrome, and persistence seams ([`sync`]) |
//! | `after_frame` | pump the scenario one step ([`scenario`]) |
//! | `after_wake` | service output, preview completions and accepted transport status |
//! | `close_request` | finish capture storage and WAV work, then honor appearance changes |
//! | `focused_text` | the focused product or shared workshop text field ([`text`]) |
//! | `key_intercept` | Escape closes an open dropdown |

mod appearance;
mod arrangement;
mod audio;
mod capture_persistence;
mod drive;
mod fonts;
mod fold_receipt;
#[cfg(test)]
mod layout_tests;
mod leaves;
mod midi;
mod music_export;
mod persona;
mod scenario;
mod session;
mod shared;
mod storage;
mod sync;
mod text;
mod wav_export;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cambium::{clickable, el, text as text_node, title_bar};
use cambium_genet_winit_host::{
    CaptionLabels, HostHooks, HostOptions, Init, Key, KeyPress, NamedKey, Runner, WindowCommands,
    WindowFrame, platform_caption_controls, run,
};
use woodshed_core::audio::AudioBackend as _;
use woodshed_core::midi::MidiBackend as _;
use woodshed_core::settings::WindowSettings;
use woodshed_views::stage::{UiChild, UiState, ViewportClass, stage_root};

use crate::audio::CpalBackend;
use crate::shared::Shared;
use crate::sync::{Ctx, Logic};

/// The shared title bar owns native insets, drag regions and caption actions.
fn desktop_chrome(commands: &WindowCommands) -> UiChild {
    title_bar(
        Box::new(
            el("span", text_node("W"))
                .attr("class", "woodshed-mark")
                .attr("aria-hidden", "true"),
        ),
        Box::new(el("span", text_node("Woodshed")).attr("class", "chrome-title")),
        Box::new(String::new()),
        platform_caption_controls(commands, &CaptionLabels::default()),
    )
}

fn desktop_root(ui: &UiState, commands: &WindowCommands) -> UiChild {
    let retry = commands.clone();
    let discard = commands.clone();
    let decision = ui.practice_close_pending.then(|| {
        el(
            "section",
            (
                el("h3", text_node("Practice is not confirmed saved")),
                el(
                    "p",
                    text_node(
                        ui.practice_save_notice
                            .clone()
                            .unwrap_or_else(|| "Current changes remain in this window.".into()),
                    ),
                ),
                el(
                    "div",
                    (
                        clickable(
                            el("button", text_node("Retry save"))
                                .attr("class", "t-btn practice-close-retry"),
                            move |ui: &mut UiState, _| {
                                ui.practice_close_pending = false;
                                retry.close();
                            },
                        ),
                        clickable(
                            el("button", text_node("Discard unsaved practice and close"))
                                .attr("class", "t-btn practice-close-discard"),
                            move |ui: &mut UiState, _| {
                                ui.practice_discard_close = true;
                                discard.close();
                            },
                        ),
                        clickable(
                            el("button", text_node("Cancel"))
                                .attr("class", "t-btn practice-close-cancel"),
                            |ui: &mut UiState, _| {
                                ui.practice_close_pending = false;
                                ui.practice_discard_close = false;
                            },
                        ),
                    ),
                )
                .attr("class", "transport"),
            ),
        )
        .attr("class", "practice-close-panel")
        .attr("role", "alert")
    });
    let persona_decision = ui.pending_persona_switch.as_ref().map(|pending| {
        use woodshed_views::persona::SwitchDecision;
        el("section", (
            el("h3", text_node("Keep outgoing practice before switching persona")),
            el("p", text_node(pending.notice.clone())),
            clickable(el("button", text_node("Retry save and switch")).attr("class", "t-btn persona-save-retry"),
                |ui: &mut UiState, _| if let Some(pending) = &mut ui.pending_persona_switch { pending.decision = Some(SwitchDecision::Retry); }),
            clickable(el("button", text_node("Discard unsaved changes and switch")).attr("class", "t-btn persona-save-discard"),
                |ui: &mut UiState, _| if let Some(pending) = &mut ui.pending_persona_switch { pending.decision = Some(SwitchDecision::Discard); }),
            clickable(el("button", text_node("Cancel switch")).attr("class", "t-btn persona-save-cancel"),
                |ui: &mut UiState, _| if let Some(pending) = &mut ui.pending_persona_switch { pending.decision = Some(SwitchDecision::Cancel); }),
        )).attr("class", "practice-close-panel").attr("role", "alert")
    });
    let draft_close = commands.clone();
    let draft_decision = ui.arrangement_editor.close_pending.then(|| {
        el(
            "section",
            (
                el("h3", text_node("Arrangement drafts are not saved")),
                el("p", text_node("Keep editing to save each draft, or explicitly discard the drafts before closing.")),
                clickable(
                    el("button", text_node("Keep editing"))
                        .attr("class", "t-btn arrangement-close-cancel"),
                    |ui: &mut UiState, _| ui.arrangement_editor.close_pending = false,
                ),
                clickable(
                    el("button", text_node("Discard arrangement drafts and close"))
                        .attr("class", "t-btn arrangement-close-discard"),
                    move |ui: &mut UiState, _| {
                        ui.arrangement_editor.drafts.clear();
                        ui.music_editor.drafts.clear();
                        ui.arrangement_editor.close_pending = false;
                        draft_close.close();
                    },
                ),
            ),
        )
        .attr("class", "practice-close-panel")
        .attr("role", "alert")
    });
    Box::new(
        el(
            "div",
            (
                desktop_chrome(commands),
                decision,
                persona_decision,
                draft_decision,
                stage_root(ui),
            ),
        )
        .attr("class", "desktop-frame"),
    )
}

/// One preservation path for native close, explicit retry/discard, and the
/// completion of another product close decision such as appearance.
fn prepare_close(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> cambium_genet_winit_host::CloseDisposition {
    use cambium_genet_winit_host::CloseDisposition;
    if ctx.runner.state().pending_persona_switch.is_some() {
        return CloseDisposition::KeepVisible;
    }
    if ctx.runner.state().arrangement_drafts_dirty() {
        ctx.runner
            .update(|ui| ui.arrangement_editor.close_pending = true);
        return CloseDisposition::KeepVisible;
    }
    if !ctx.runner.state().practice_discard_close {
        if let Some(geometry) = ctx.geometry {
            persist_window_geometry(shared, ctx, geometry);
        }
    }
    let mut pending = false;
    ctx.runner.update(|ui| {
        pending = capture_persistence::close_request(shared, ui);
    });
    if pending {
        return CloseDisposition::KeepVisible;
    }
    wav_export::close_request(shared, ctx).unwrap_or_else(|| appearance::close_request(ctx))
}

/// Every product-requested exit, including scenario completion after a frame,
/// passes the same preservation decision as a native window close.
fn guard_requested_close(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    if *ctx.close {
        *ctx.close = false;
        *ctx.close = prepare_close(shared, ctx) == cambium_genet_winit_host::CloseDisposition::Exit;
    }
}

/// Build the starting state: the audio backend, the restored session, and the
/// application settings. Runs once, inside the host, after the window exists
/// but before the first frame.
fn boot_state(
    shared: &Rc<RefCell<Shared>>,
    window: &dyn cambium_genet_winit_host::HostWindow,
    commands: &WindowCommands,
    wake: &cambium_genet_winit_host::HostWake,
) -> Init<UiState, Logic> {
    let mut shared = shared.borrow_mut();
    let device_free_receipt = appearance::device_free_receipt();
    let mut backend = (!device_free_receipt).then(CpalBackend::new);
    if let Some(backend) = &mut backend {
        backend.set_wake(wake.callback());
    }
    let mut ui = UiState::new();
    ui.enable_practice_clock();
    shared.wav_export.set_wake(wake.callback());
    shared.capture_persistence.set_wake(wake.callback());
    ui.event_clock = Some(drive::wall_time_ms);
    let (size_w, size_h) = window.inner_size();
    let scale = window.scale_factor() as f32;
    ui.set_viewport_width(size_w as f32 / scale);
    ui.set_viewport_height(size_h as f32 / scale);
    ui.audio_error = backend
        .as_ref()
        .and_then(|backend| backend.error().map(String::from));

    // Restore the artifact session and the separate application settings, when
    // there is a store to restore from. There is not, on a machine whose vault
    // holds several personas with none chosen: the gate goes up instead, and
    // `persona::after_dispatch` restores once the question is answered.
    match shared.storage.as_ref() {
        Some(storage) => session::restore(storage, &mut ui),
        None => persona::seed(&mut shared, &mut ui),
    }
    // Outside the match, deliberately. This lived inside `seed`, which only
    // runs on the gate path, so on every ordinary launch Settings reported no
    // persona while the store was sealed to one. One assignment, both paths,
    // and they cannot drift apart again.
    ui.seal = shared.seal.clone();
    // A pending identity saves nothing (D12), so the notice says so.
    if matches!(
        ui.seal,
        Some(woodshed_views::persona::PracticeSeal::Pending { .. })
    ) {
        ui.practice_saved = false;
    }
    if let Some(storage) = shared.storage.as_ref() {
        session::ensure_personal_relation_source(storage, &mut ui);
    }
    appearance::load_library(&mut ui);
    shared.theme = ui.theme();
    shared.reduce_motion = ui.app_settings.accessibility.reduce_motion;
    shared.text_scale = ui.app_settings.accessibility.text_scale.clone();
    let sheet = appearance::stylesheet(&ui);
    shared.appearance_sheet = Some(sheet.clone());
    // Populate the MIDI port pickers with what's plugged in now.
    if !device_free_receipt {
        ui.midi.input_ports = shared.midi.input_ports();
        ui.midi.output_ports = shared.midi.output_ports();
    }
    shared.backend = backend;

    let commands = commands.clone();
    Init {
        state: ui,
        logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands)) as Logic,
        sheet,
        fonts: fonts::bundled_fonts(),
        images: Vec::new(),
    }
}

/// Refresh the shared view's transient width band after a resize or DPI change.
/// Returns whether the retained root actually needed rebuilding.
fn sync_viewport(ctx: &mut Ctx<'_>) -> bool {
    let (width, height) = ctx.logical_size;
    let changed = {
        let ui = ctx.runner.state();
        ui.viewport != ViewportClass::for_width(width)
            || (ui.viewport_width - width).abs() >= 16.0
            || (ui.viewport_h - height).abs() >= 16.0
    };
    if !changed {
        return false;
    }
    ctx.runner.update(|ui| {
        // `|` not `||`: both must run, and either change needs a rebuild (the
        // height bounds a vertical board's scroll viewport).
        let _ = ui.set_viewport_width(width) | ui.set_viewport_height(height);
    });
    true
}

/// What Escape means, window-wide.
///
/// An intercept rather than a view handler because it is a policy, not a
/// control's behaviour: it runs before dispatch and does not care what has the
/// caret. Named rather than inline so a test drives the shipping decision
/// instead of a copy of it.
fn escape_policy(runner: &mut Runner<UiState, Logic, UiChild>, press: &KeyPress) -> bool {
    if !matches!(press.key, Key::Named(NamedKey::Escape)) {
        return false;
    }
    // While the persona gate is up, Escape is how you practise without a
    // persona, and it has to work on the first press. The gate's picker does
    // ask for the caret now, and would report its own Escape, but the policy
    // answers first and consumes it: declining is not a thing to make
    // conditional on a focus request having landed.
    if runner.state().persona.is_some() {
        runner.update(|ui| {
            if let Some(pick) = ui.persona.as_mut() {
                pick.record(woodshed_views::persona::dismissed());
            }
        });
        return true;
    }
    // A captured musical gesture must cancel even when focus has moved.
    if runner.state().music_drag_active() || runner.state().music_preview_active() {
        runner.update(|ui| {
            ui.cancel_music_drag();
            if ui.music_preview_active() {
                ui.cancel_music_pattern();
            }
        });
        return true;
    }
    // Otherwise it closes any open dropdown.
    runner.update(|ui| {
        ui.tuning_dd.open = false;
        ui.root_dd.open = false;
    });
    true
}

fn to_host_geometry(settings: WindowSettings) -> cambium_genet_winit_host::WindowGeometry {
    cambium_genet_winit_host::WindowGeometry {
        position: (settings.x, settings.y),
        size: (settings.width, settings.height),
        maximized: settings.maximized,
    }
}

fn to_window_settings(geometry: cambium_genet_winit_host::WindowGeometry) -> WindowSettings {
    WindowSettings {
        x: geometry.position.0,
        y: geometry.position.1,
        width: geometry.size.0,
        height: geometry.size.1,
        maximized: geometry.maximized,
    }
}

fn initial_window_geometry(
    shared: &Rc<RefCell<Shared>>,
) -> Option<cambium_genet_winit_host::WindowGeometry> {
    shared
        .borrow()
        .storage
        .as_ref()
        .and_then(session::load_settings)
        .and_then(|settings| settings.window)
        .map(to_host_geometry)
}

fn persist_window_geometry(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    geometry: cambium_genet_winit_host::WindowGeometry,
) {
    let mut json = None;
    ctx.runner.update(|ui| {
        ui.app_settings.window = Some(to_window_settings(geometry));
        json = serde_json::to_string(&ui.app_settings).ok();
    });
    if let (Some(storage), Some(json)) = (shared.storage.as_ref(), json) {
        storage.save_settings(&json);
        ctx.runner
            .update(|ui| session::refresh_save_status(storage, ui));
    }
}

fn hooks(shared: &Rc<RefCell<Shared>>) -> HostHooks<UiState, Logic, UiChild> {
    let mut lane = scenario::from_env(shared.clone());
    let frame_shared = shared.clone();
    let dispatch_shared = shared.clone();
    let after_frame_shared = shared.clone();
    let wake_shared = shared.clone();
    let close_shared = shared.clone();
    let capture_close_requested = Rc::new(Cell::new(false));
    let wake_close_requested = capture_close_requested.clone();
    let mut previews = appearance::PreviewBindings::default();
    let mut arrangements = arrangement::Bindings::default();
    HostHooks {
        frame: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = frame_shared.borrow_mut();
            if !capture_persistence::session_current(&shared, ctx.runner.state()) {
                sync::after_dispatch(&mut shared, ctx);
            }
            let drag_active = ctx.runner.state().set_graph_drag_active;
            shared.drag_frame_metrics.begin(drag_active);
            let phase = std::time::Instant::now();
            let viewport_rebuilt = sync_viewport(ctx);
            shared
                .drag_frame_metrics
                .note_viewport(phase.elapsed(), viewport_rebuilt);
            let mut animating = false;
            let phase = std::time::Instant::now();
            let drive_rebuilt =
                !drag_active || drive::requires_live_frame(&shared, ctx.runner.state());
            if drive_rebuilt {
                ctx.runner.update(|ui| {
                    // The native text seam can update a field between ordinary
                    // dispatch tails. Refresh derived scenes before binding them.
                    ui.sync_arrangement_scene();
                    animating = drive::frame(&mut shared, ui)
                        | ui.tick_overview_dynamics()
                        | ui.tick_overview_atmosphere();
                });
            }
            shared
                .drag_frame_metrics
                .note_drive(phase.elapsed(), drive_rebuilt);
            let (out_enabled, out_playing, out_bpm) = drive::clock_out(ctx.runner.state());
            shared.midi.set_clock_out(out_enabled, out_playing, out_bpm);
            let phase = std::time::Instant::now();
            if drag_active && !drive_rebuilt {
                leaves::sync_set_graph(&mut shared, ctx.runner.state(), ctx.leaves);
            } else {
                leaves::sync_all(&mut shared, ctx.runner.state(), ctx.leaves);
            }
            previews.sync(ctx);
            arrangements.sync(ctx);
            shared.drag_frame_metrics.note_leaves(phase.elapsed());
            animating
        }),
        after_dispatch: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = dispatch_shared.borrow_mut();
            if !ctx.runner.state().practice_discard_close {
                sync::after_dispatch(&mut shared, ctx);
            } else {
                // Appearance is a separate close guard. Its confirmation may
                // finish or cancel this transaction; neither may autosave the
                // explicitly discarded practice while the decision is pending.
                let appearance_was_closing = ctx.runner.state().appearance_close_app;
                appearance::after_dispatch(ctx);
                if appearance_was_closing && !*ctx.close && !ctx.runner.state().appearance_close_app
                {
                    ctx.runner.update(|ui| ui.practice_discard_close = false);
                }
            }
            guard_requested_close(&mut shared, ctx);
        }),
        after_frame: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = after_frame_shared.borrow_mut();
            shared.drag_frame_metrics.finish(ctx.frame_profile);
            drop(shared);
            if let Some(lane) = &mut lane {
                scenario::drive(lane, ctx);
            }
            guard_requested_close(&mut after_frame_shared.borrow_mut(), ctx);
        }),
        after_wake: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = wake_shared.borrow_mut();
            wav_export::poll(&mut shared, ctx);
            if *ctx.close {
                return;
            }
            if !capture_persistence::session_current(&shared, ctx.runner.state()) {
                sync::after_dispatch(&mut shared, ctx);
                if !capture_persistence::session_current(&shared, ctx.runner.state()) {
                    if let Some(backend) = shared.backend.as_mut() {
                        backend.service();
                    }
                    return;
                }
            }
            let observation = drive::poll_audio(&mut shared);
            if let Some(observation) = observation {
                if observation.changed(ctx.runner.state())
                    || capture_persistence::pending(&shared, ctx.runner.state())
                {
                    ctx.runner.update(|ui| {
                        observation.apply(ui);
                        capture_persistence::persist_if_changed(&mut shared, ui);
                    });
                }
            } else if capture_persistence::pending(&shared, ctx.runner.state()) {
                ctx.runner.update(|ui| {
                    capture_persistence::persist_if_changed(&mut shared, ui);
                });
            }
            persona::resume_switch(&mut shared, ctx);
            if wake_close_requested.get()
                && !shared.capture_persistence.preparation_busy()
                && wav_export::close_request(&mut shared, ctx).is_none()
            {
                let disposition = prepare_close(&mut shared, ctx);
                wake_close_requested.set(shared.capture_persistence.preparation_busy());
                *ctx.close = disposition == cambium_genet_winit_host::CloseDisposition::Exit;
            }
        }),
        close_request: Box::new(move |ctx, _request| {
            let mut shared = close_shared.borrow_mut();
            let disposition = prepare_close(&mut shared, ctx);
            capture_close_requested.set(shared.capture_persistence.preparation_busy());
            disposition
        }),
        focused_text: Box::new(text::focused_text),
        key_intercept: Box::new(escape_policy),
    }
}

fn main() {
    let shared = Shared::boot();
    let init_shared = shared.clone();
    let initial_geometry = initial_window_geometry(&shared);
    let options = HostOptions {
        title: "Woodshed".into(),
        // CSD: the app draws its own chrome (title row, window buttons, drag
        // surface); the host supplies the edge-resize grab margins and cursors.
        window_frame: WindowFrame::App,
        maximize_control_label: CaptionLabels::default().maximize,
        initial_logical_size: (1_100.0, 664.0),
        initial_geometry,
        // A scenario run asks for a deterministic window: a receipt captured at
        // a different size is a different layout.
        size_env: Some(("WOODSHED_WIDTH".into(), "WOODSHED_HEIGHT".into())),
        ..Default::default()
    };
    run(
        options,
        move |window, commands, wake| boot_state(&init_shared, window, commands, wake),
        hooks(&shared),
    )
    .expect("run app");
}

#[cfg(test)]
mod tests {
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
        async fn apply(
            &self,
            ops: &[muniment::backend::WriteOp],
        ) -> Result<(), muniment::StoreError> {
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
                    logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands)) as Logic,
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
            let store = woodshed_core::storage::SessionStore::new(
                Box::new(backend) as storage::HostBackend
            );
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
            assert!(
                host.click_on(&taproot::Selector::role("button").with_attr("data-action", action))
            );
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
                    logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands)) as Logic,
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
                window_frame: WindowFrame::App,
                ..Default::default()
            },
        )
    }

    #[test]
    fn native_workshop_routes_name_and_stylesheet_text_through_owned_model() {
        use taproot::Selector;
        let mut h = appearance_harness();
        h.layout_at(1180.0, 2000.0);
        assert!(
            h.click_on(&Selector::role("button").with_attr("data-action", "toggle-stylesheet"))
        );
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
}
