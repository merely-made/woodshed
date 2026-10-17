// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket's Genet desktop host.
//!
//! The shared `cambium-genet-winit-host` owns the winit lifecycle, retained
//! Livery/Buckram layout, paint, input routing, and accessibility projection.
//! Hocket supplies its state, views, custom leaves, workers, and scenario
//! policy through the host hooks below.

mod appearance;
#[cfg(test)]
mod appearance_tests;
mod appearance_view;
mod identity;
mod leaves;
mod project_io;
mod recorder_audio;
mod scenario;
mod state;
mod theme;
mod update;
mod view;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use armillary::Wake;
use cambium_genet_winit_host::{
    AppCtx, CloseDisposition, FocusedTextSlot, HostHooks, HostOptions, HostWake, HostWindow, Init,
    Runner, WindowCommands, WindowFrame, run,
};

use identity::LocalIdentity;
use project_io::{ProjectReply, spawn_project_worker};
use state::AppState;
use view::{Child, root};

type Logic = fn(&AppState) -> Child;
type AppRunner = Runner<AppState, Logic, Child>;

/// The app-owned side of the host boundary. Worker handles move into
/// [`AppState`]; only their UI-thread receivers and the waveform cache stay
/// here, where the host hooks can service them.
struct Runtime {
    project_updates: Option<Receiver<ProjectReply>>,
    update_statuses: Option<Receiver<update::UpdateStatus>>,
    waveform_cache: leaves::WaveformCache,
    scenario: Option<scenario::Run>,
    previews: tabard_workshop::native_host::PreviewBindings,
}

impl Runtime {
    fn new(scenario: Option<scenario::Run>) -> Self {
        Self {
            project_updates: None,
            update_statuses: None,
            waveform_cache: leaves::WaveformCache::new(),
            scenario,
            previews: Default::default(),
        }
    }
}

fn boot_state(
    runtime: &Rc<RefCell<Runtime>>,
    _window: &dyn HostWindow,
    commands: &WindowCommands,
    host_wake: &HostWake,
) -> Init<AppState, Logic> {
    let wake: Wake = host_wake.callback();
    let (project_worker, project_updates) = spawn_project_worker(wake.clone());
    let (update_worker, update_statuses) =
        update::worker::spawn_update_worker(wake, build_transport());
    let update_settings =
        update::UpdateSettingsProvider::load_or_default(update::settings_path()).settings();
    if update_settings.policy.checks() {
        update_worker.command(update::worker::UpdateCommand::Check {
            settings: update_settings.clone(),
            user_asked: false,
        });
    }
    {
        let mut runtime = runtime.borrow_mut();
        runtime.project_updates = Some(project_updates);
        runtime.update_statuses = Some(update_statuses);
    }
    let identity = LocalIdentity::open_default().map_err(|error| error.to_string());
    let mut state = AppState::new(
        project_worker,
        update_worker,
        update_settings,
        identity,
        appearance::device_free_receipt(),
    );
    state.appearance = appearance::load();
    state.window_commands = commands.clone();
    state.protect_appearance_exports();
    let sheet = state.appearance.take_stylesheet_change().unwrap();
    Init {
        state,
        logic: root as Logic,
        sheet,
        fonts: Vec::new(),
        images: Vec::new(),
    }
}

fn drain_worker_updates(ctx: &mut AppCtx<'_, AppState, Logic, Child>, runtime: &Runtime) {
    while let Some(update) = runtime
        .project_updates
        .as_ref()
        .and_then(|updates| updates.try_recv().ok())
    {
        ctx.runner
            .update(|state| state.apply_project_update(update));
    }
    while let Some(status) = runtime
        .update_statuses
        .as_ref()
        .and_then(|statuses| statuses.try_recv().ok())
    {
        ctx.runner.update(|state| state.apply_update_status(status));
    }
}

fn hooks(runtime: &Rc<RefCell<Runtime>>) -> HostHooks<AppState, Logic, Child> {
    let frame_runtime = runtime.clone();
    let scenario_runtime = runtime.clone();
    let dispatch_runtime = runtime.clone();
    let close_runtime = runtime.clone();
    HostHooks {
        // Firewheel's meter/capture promotion and Hocket's custom leaves are
        // live frame work, so retain the original 60 fps tick policy.
        frame: Box::new(move |ctx| {
            let mut runtime = frame_runtime.borrow_mut();
            drain_worker_updates(ctx, &runtime);
            ctx.runner.update(AppState::tick);
            if ctx.runner.state().appearance.editor_open {
                runtime.previews.register(
                    &ctx.runner.state().appearance.workshop,
                    ctx.leaves,
                    ctx.producers,
                );
            } else {
                leaves::reconcile(ctx.leaves, &mut runtime.waveform_cache, ctx.runner.state());
            }
            true
        }),
        after_dispatch: Box::new(move |ctx| {
            // Accept recorder actions before another input can snapshot a
            // project for Save/Export; the frame hook still services live audio.
            ctx.runner.update(AppState::tick);
            appearance_dispatch(ctx);
            guard_exit(ctx, &dispatch_runtime);
        }),
        after_frame: Box::new(move |ctx| {
            {
                let mut runtime = scenario_runtime.borrow_mut();
                if let Some(mut run) = runtime.scenario.take() {
                    scenario::drive(&mut run, ctx);
                    runtime.scenario = Some(run);
                }
            }
            guard_exit(ctx, &scenario_runtime);
        }),
        after_wake: Box::new(|_ctx| {}),
        close_request: Box::new(move |ctx, _request| {
            let mut allow = false;
            ctx.runner.update(|state| {
                allow = close_admission(state);
            });
            if allow && !defer_scenario_close(&close_runtime) {
                let mut restarting = false;
                ctx.runner.update(|state| restarting = state.apply_restart_after_close_guard());
                if restarting { CloseDisposition::KeepVisible } else { CloseDisposition::Exit }
            } else {
                CloseDisposition::KeepVisible
            }
        }),
        focused_text: Box::new(|runner: &AppRunner| {
            if !runner.state().appearance.editor_open {
                return None;
            }
            let node = runner.focus()?;
            let field = tabard_workshop::native_host::focused_field(
                &runner.dom(),
                node,
                &runner.state().appearance.workshop,
            )?;
            let mutable = field.clone();
            Some(FocusedTextSlot {
                node,
                get: Box::new(move |state| {
                    state
                        .appearance
                        .workshop
                        .text_field(&field)
                        .expect("shared field")
                }),
                get_mut: Box::new(move |state| {
                    state
                        .appearance
                        .workshop
                        .text_field_mut(&mutable)
                        .expect("shared field")
                }),
            })
        }),
        key_intercept: Box::new(|_runner, _press| false),
    }
}

fn close_admission(state: &mut AppState) -> bool {
    if state.appearance.editor_open {
        state.appearance.close_app = true;
        if !state.appearance.workshop.request_close() {
            state.wait_for_appearance_close();
            return false;
        }
    }
    state.request_project_close()
}

/// Native, caption, appearance, save continuation and scenario exits all pass
/// the same project admission before Mesquite may interrupt/read back and exit.
fn guard_exit(ctx: &mut AppCtx<'_, AppState, Logic, Child>, runtime: &Rc<RefCell<Runtime>>) {
    if !*ctx.close && !ctx.runner.state().project_close_ready() {
        return;
    }
    let mut allow = false;
    ctx.runner.update(|state| allow = close_admission(state));
    *ctx.close = allow && !defer_scenario_close(runtime);
    if *ctx.close {
        ctx.runner.update(|state| {
            if state.apply_restart_after_close_guard() {
                *ctx.close = false;
            }
        });
    }
}

/// An accepted close still owes the scenario's pending pixels and outcome.
/// Mesquite marks unfinished scenarios interrupted and closes after readback.
/// Appearance cancellation reaches neither this function nor that interruption.
fn defer_scenario_close(runtime: &Rc<RefCell<Runtime>>) -> bool {
    let mut runtime = runtime.borrow_mut();
    let Some(lane) = runtime.scenario.as_mut().filter(|lane| !lane.finished()) else {
        return false;
    };
    lane.request_close();
    true
}

fn appearance_dispatch(ctx: &mut AppCtx<'_, AppState, Logic, Child>) {
    let mut close = false;
    let mut sheet = None;
    ctx.runner.update(|state| {
        if state.appearance.editor_open {
            state.protect_appearance_exports();
            let exit = tabard_workshop::native_host::sync_and_export(
                &mut state.appearance.workshop,
                &mut |artifact| {
                    let extension = if artifact.format == tabard_workshop::ExportFormat::Css {
                        "css"
                    } else {
                        "json"
                    };
                    cambium_genet_winit_host::choose_save_path(
                        "Export theme",
                        &artifact.suggested_name,
                        &[extension],
                    )
                },
            );
            close = appearance_view::sync_editor(state, exit);
        }
        state.appearance.commit_requested();
        sheet = state.appearance.take_stylesheet_change();
    });
    if let Some(sheet) = sheet {
        *ctx.set_sheet = Some(sheet);
    }
    if close {
        *ctx.close = true;
    }
}

/// The update transport this run uses.
///
/// Luggage is the family's Rust-native pipeline. `HOCKET_UPDATE_TRANSPORT`
/// keeps Velopack selectable while the two paths remain under evaluation.
fn build_transport() -> Box<dyn update::UpdateTransport> {
    match std::env::var("HOCKET_UPDATE_TRANSPORT").as_deref() {
        Ok("velopack") => Box::new(update::velopack_transport::VelopackTransport::new()),
        _ => Box::new(update::luggage_transport::LuggageTransport::new()),
    }
}

fn main() {
    // Velopack lifecycle hooks may restart or exit; do this before creating an
    // audio device or native window.
    velopack::VelopackApp::build().run();
    if std::env::args().any(|arg| arg == "--update-now") {
        let settings =
            update::UpdateSettingsProvider::load_or_default(update::settings_path()).settings();
        std::process::exit(update::cli::run_update_now(build_transport(), settings));
    }

    let runtime = Rc::new(RefCell::new(Runtime::new(scenario::load())));
    let init_runtime = runtime.clone();
    let options = HostOptions {
        title: "Hocket".into(),
        window_frame: WindowFrame::App,
        initial_logical_size: (1_180.0, 700.0),
        size_env: Some(("HOCKET_WIDTH".into(), "HOCKET_HEIGHT".into())),
        ..Default::default()
    };
    run(
        options,
        move |window, commands, wake| boot_state(&init_runtime, window, commands, wake),
        hooks(&runtime),
    )
    .expect("run app");
}
