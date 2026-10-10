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
//! | `after_wake` | no worker channel to drain yet |
//! | `close_request` | exit: Woodshed has no background operation to retain |
//! | `focused_text` | the focused product or shared workshop text field ([`text`]) |
//! | `key_intercept` | Escape closes an open dropdown |

mod appearance;
mod audio;
mod drive;
#[cfg(test)]
mod layout_tests;
mod leaves;
mod midi;
mod persona;
mod scenario;
mod session;
mod shared;
mod storage;
mod sync;
mod text;

use std::cell::RefCell;
use std::rc::Rc;

use cambium::{el, text as text_node, title_bar};
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
    Box::new(el("div", (desktop_chrome(commands), stage_root(ui))).attr("class", "desktop-frame"))
}

/// Build the starting state: the audio backend, the restored session, and the
/// application settings. Runs once, inside the host, after the window exists
/// but before the first frame.
fn boot_state(
    shared: &Rc<RefCell<Shared>>,
    window: &dyn cambium_genet_winit_host::HostWindow,
    commands: &WindowCommands,
) -> Init<UiState, Logic> {
    let mut shared = shared.borrow_mut();
    let backend = CpalBackend::new();
    let mut ui = UiState::new();
    ui.event_clock = Some(drive::wall_time_ms);
    let (size_w, size_h) = window.inner_size();
    let scale = window.scale_factor() as f32;
    ui.set_viewport_width(size_w as f32 / scale);
    ui.set_viewport_height(size_h as f32 / scale);
    ui.audio_error = backend.error().map(String::from);

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
    if matches!(ui.seal, Some(woodshed_views::persona::PracticeSeal::Pending { .. })) {
        ui.practice_saved = false;
    }
    appearance::load_library(&mut ui);
    shared.theme = ui.theme();
    shared.reduce_motion = ui.app_settings.accessibility.reduce_motion;
    shared.text_scale = ui.app_settings.accessibility.text_scale.clone();
    let sheet = appearance::stylesheet(&ui);
    shared.appearance_sheet = Some(sheet.clone());
    // Populate the MIDI port pickers with what's plugged in now.
    ui.midi.input_ports = shared.midi.input_ports();
    ui.midi.output_ports = shared.midi.output_ports();
    shared.backend = Some(backend);

    let commands = commands.clone();
    Init {
        state: ui,
        logic: Box::new(move |ui: &UiState| desktop_root(ui, &commands)) as Logic,
        sheet,
        fonts: Vec::new(),
        images: Vec::new(),
    }
}

/// Refresh the shared view's transient width band after a resize or DPI change.
/// Returns whether the retained root actually needed rebuilding.
fn sync_viewport(ctx: &mut Ctx<'_>) -> bool {
    let (width, height) = ctx.logical_size;
    let changed = {
        let ui = ctx.runner.state();
        ui.viewport != ViewportClass::for_width(width) || (ui.viewport_h - height).abs() >= 16.0
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
    }
}

fn hooks(shared: &Rc<RefCell<Shared>>) -> HostHooks<UiState, Logic, UiChild> {
    let mut lane = scenario::from_env(shared.clone());
    let frame_shared = shared.clone();
    let dispatch_shared = shared.clone();
    let after_frame_shared = shared.clone();
    let close_shared = shared.clone();
    let mut previews = appearance::PreviewBindings::default();
    HostHooks {
        frame: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = frame_shared.borrow_mut();
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
            shared.drag_frame_metrics.note_leaves(phase.elapsed());
            animating
        }),
        after_dispatch: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = dispatch_shared.borrow_mut();
            sync::after_dispatch(&mut shared, ctx);
        }),
        after_frame: Box::new(move |ctx: &mut Ctx<'_>| {
            let mut shared = after_frame_shared.borrow_mut();
            shared.drag_frame_metrics.finish(ctx.frame_profile);
            drop(shared);
            if let Some(lane) = &mut lane {
                scenario::drive(lane, ctx);
            }
        }),
        after_wake: Box::new(|_ctx| {}),
        close_request: Box::new(move |ctx, _request| {
            if let Some(geometry) = ctx.geometry {
                persist_window_geometry(&mut close_shared.borrow_mut(), ctx, geometry);
            }
            appearance::close_request(ctx)
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
        move |window, commands, _wake| boot_state(&init_shared, window, commands),
        hooks(&shared),
    )
    .expect("run app");
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let bar =
                h.with_dom(|dom| taproot::matching(dom, &Selector::class("cambium-title-bar"))[0]);
            let (x, y, w, _) = h.painted_rect(bar).unwrap();
            assert!(x.abs() < 1.0 && y.abs() < 1.0);
            assert!((w - width).abs() < 1.0, "titlebar {w} viewport {width}");
            let editor =
                h.with_dom(|dom| taproot::matching(dom, &Selector::class("woodshed-workshop"))[0]);
            let (_, top, _, _) = h.painted_rect(editor).unwrap();
            assert!(top >= 36.0);
        }
    }
}
