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
mod desktop;
mod drive;
mod fold_receipt;
mod fonts;
#[cfg(test)]
mod layout_tests;
mod leaves;
mod midi;
mod midi_diagnostics;
mod music_export;
mod pack_io;
mod paths;
mod pedal_inputs;
mod persona;
#[cfg(test)]
mod persona_store_receipt;
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
    WindowFrame, caption_controls_for_frame, platform_window_frame, run,
};
use woodshed_core::audio::AudioBackend as _;
use woodshed_core::midi::MidiBackend as _;
use woodshed_core::settings::WindowSettings;
use woodshed_views::stage::{UiChild, UiState, ViewportClass, stage_root};

use crate::audio::CpalBackend;
use crate::shared::Shared;
use crate::sync::{Ctx, Logic};

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
    midi_diagnostics::close_request(shared, ctx)
        .or_else(|| pack_io::close_request(shared, ctx))
        .or_else(|| wav_export::close_request(shared, ctx))
        .unwrap_or_else(|| appearance::close_request(ctx))
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
    frame: WindowFrame,
) -> Init<UiState, Logic> {
    let mut shared = shared.borrow_mut();
    let music_practice_receipt = scenario::music_practice_receipt();
    let device_free_receipt = appearance::device_free_receipt() || music_practice_receipt;
    let mut backend = if music_practice_receipt {
        Some(CpalBackend::offline_for_receipt())
    } else {
        (!device_free_receipt).then(CpalBackend::new)
    };
    if let Some(backend) = &mut backend {
        backend.set_wake(wake.callback());
    }
    let mut ui = UiState::new();
    ui.enable_practice_clock();
    shared.wav_export.set_wake(wake.callback());
    shared.pack_io.set_wake(wake.callback());
    shared.midi_diagnostics.set_wake(wake.callback());
    shared.capture_persistence.set_wake(wake.callback());
    ui.event_clock = Some(drive::wall_time_ms);
    let (size_w, size_h) = window.inner_size();
    let scale = window.scale_factor() as f32;
    ui.set_viewport_width(size_w as f32 / scale);
    ui.set_viewport_height(size_h as f32 / scale);
    ui.audio_availability = backend
        .as_ref()
        .map(CpalBackend::availability)
        .unwrap_or_default();
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
    appearance::load_library(&mut ui, &shared);
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
        logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands, frame)) as Logic,
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
    if pedal_inputs::keyboard(runner, press) {
        return true;
    }
    // A held Enter/Space must not repeatedly fire a performance action.
    if press.repeat && matches!(press.key, Key::Named(NamedKey::Enter | NamedKey::Space)) {
        use layout_dom_api::{LayoutDom as _, LocalName, Namespace};
        if runner.focus().is_some_and(|node| {
            runner
                .dom()
                .borrow()
                .attribute(
                    node,
                    &Namespace::from(""),
                    &LocalName::from("data-midi-pedal-trigger"),
                )
                .is_some()
        }) {
            return true;
        }
    }
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
    let shared = shared.borrow();
    let root = shared.data_root.as_deref()?;
    match desktop::load(root) {
        Ok(settings) => settings.window.map(to_host_geometry),
        Err(error) => {
            eprintln!("[woodshed] desktop preferences could not open: {error}");
            None
        },
    }
}

fn persist_window_geometry(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    geometry: cambium_genet_winit_host::WindowGeometry,
) {
    let Some(root) = shared.data_root.as_deref() else {
        return;
    };
    let settings = desktop::DesktopSettings {
        window: Some(to_window_settings(geometry)),
    };
    if let Err(error) = desktop::save(root, &settings) {
        eprintln!("[woodshed] window placement was not saved: {error}");
        ctx.runner.update(|ui| {
            ui.appearance_notice = Some(format!("Window placement was not saved: {error}"))
        });
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
            let mut animating = pedal_inputs::poll(&mut shared, ctx);
            let phase = std::time::Instant::now();
            let drive_rebuilt =
                !drag_active || drive::requires_live_frame(&shared, ctx.runner.state());
            if drive_rebuilt {
                ctx.runner.update(|ui| {
                    // The native text seam can update a field between ordinary
                    // dispatch tails. Refresh derived scenes before binding them.
                    ui.sync_arrangement_scene();
                    animating |= drive::frame(&mut shared, ui)
                        | ui.tick_overview_dynamics()
                        | ui.tick_overview_atmosphere();
                });
            }
            shared
                .drag_frame_metrics
                .note_drive(phase.elapsed(), drive_rebuilt);
            let (out_enabled, out_playing, out_bpm) = drive::clock_out(ctx.runner.state());
            shared
                .midi
                .set_pedal_bindings(&ctx.runner.state().app_settings.audio_midi.midi_pedal);
            shared.midi.set_clock_out(out_enabled, out_playing, out_bpm);
            ctx.runner.update(|ui| {
                animating |= sync::poll_midi_state(ui, &mut shared.midi);
            });
            pedal_inputs::sync(&mut shared, ctx.runner.state());
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
        prepare_presentation: Box::new(|_, _| None),
        after_wake: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = wake_shared.borrow_mut();
            wav_export::poll(&mut shared, ctx);
            pack_io::poll(&mut shared, ctx);
            midi_diagnostics::poll(&mut shared, ctx);
            if *ctx.close {
                return;
            }
            if !capture_persistence::session_current(&shared, ctx.runner.state()) {
                sync::after_dispatch(&mut shared, ctx);
                if !capture_persistence::session_current(&shared, ctx.runner.state()) {
                    if let Some(backend) = shared.backend.as_mut() {
                        backend.service();
                    }
                    let availability = shared
                        .backend
                        .as_ref()
                        .map(CpalBackend::availability)
                        .unwrap_or_default();
                    if ctx.runner.state().audio_availability != availability {
                        ctx.runner.update(|ui| {
                            ui.audio_availability = availability.clone();
                            if !ui.audio_availability.tuner_active {
                                ui.tuner.reading = None;
                            }
                        });
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
                && midi_diagnostics::close_request(&mut shared, ctx).is_none()
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
    if std::env::args_os().any(|arg| arg == "--midi-pedal-stdio") {
        shared.borrow().midi.enable_pedal_stdio();
    }
    let init_shared = shared.clone();
    let initial_geometry = initial_window_geometry(&shared);
    let frame = platform_window_frame();
    let options = HostOptions {
        title: "Woodshed".into(),
        // Native frames own their title and caption ornamentation. Windows
        // keeps the shared client frame, including resize and Snap behavior.
        window_frame: frame,
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
        move |window, commands, wake| boot_state(&init_shared, window, commands, wake, frame),
        hooks(&shared),
    )
    .expect("run app");
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[path = "render.rs"]
mod render;

use render::desktop_root;
