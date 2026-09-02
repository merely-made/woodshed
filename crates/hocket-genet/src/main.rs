//! Hocket's Genet desktop host.
//!
//! The shared `cambium-genet-winit-host` owns the winit lifecycle, retained
//! Livery/Buckram layout, paint, input routing, and accessibility projection.
//! Hocket supplies its state, views, custom leaves, workers, and scenario
//! policy through the host hooks below.

mod identity;
mod leaves;
mod project_io;
mod scenario;
mod state;
mod theme;
mod update;
mod view;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use armillary::Wake;
use cambium_genet_winit_host::{
    AppCtx, CloseDisposition, HostHooks, HostOptions, HostPointer, HostWake, HostWindow, Init,
    Runner, Surface, WindowCommands, run,
};

use identity::LocalIdentity;
use project_io::{ProjectUpdate, spawn_project_worker};
use state::AppState;
use view::{Child, root};

type Logic = fn(&AppState) -> Child;
type AppRunner = Runner<AppState, Logic, Child>;

/// The app-owned side of the host boundary. Worker handles move into
/// [`AppState`]; only their UI-thread receivers and the waveform cache stay
/// here, where the host hooks can service them.
struct Runtime {
    project_updates: Option<Receiver<ProjectUpdate>>,
    update_statuses: Option<Receiver<update::UpdateStatus>>,
    waveform_cache: leaves::WaveformCache,
    scenario: Option<scenario::Run>,
}

impl Runtime {
    fn new(scenario: Option<scenario::Run>) -> Self {
        Self {
            project_updates: None,
            update_statuses: None,
            waveform_cache: leaves::WaveformCache::new(),
            scenario,
        }
    }
}

/// A temporary adapter between a host frame and `genet-probe`'s generic
/// scenario grammar. Pointer deliveries and captures go back through the
/// host, so the receipt uses the same layout and presented frame as a person.
struct ScenarioDriver<'a, 'b> {
    ctx: &'a mut AppCtx<'b, AppState, Logic, Child>,
    capture_dir: PathBuf,
}

impl genet_probe::Automatable for ScenarioDriver<'_, '_> {
    fn with_surfaces<R>(&self, f: impl FnOnce(&[genet_probe::ProbeSurface<'_>]) -> R) -> R {
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        let (width, height) = self.ctx.logical_size;
        let sheet = theme::sheet();
        let surfaces = [genet_probe::ProbeSurface {
            name: "hocket",
            dom: &dom,
            rect: [0.0, 0.0, width, height],
            sheet: &sheet,
        }];
        f(&surfaces)
    }

    fn snapshot(&self) -> genet_probe::ProbeSnapshot {
        genet_probe::ProbeSnapshot::default()
            .with_field("status", self.ctx.runner.state().project_status_label())
    }

    fn drain_events(&mut self) -> Vec<String> {
        Vec::new()
    }

    fn act(&mut self, _label: &str) -> bool {
        false
    }

    fn press(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Press(x, y));
    }

    fn moved(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Moved(x, y));
    }

    fn release(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Release(x, y));
    }
}

impl genet_probe::Driveable for ScenarioDriver<'_, '_> {
    fn capture(&mut self, name: &str) -> bool {
        let path = self.capture_dir.join(format!("{name}.png"));
        *self.ctx.capture = Some(Box::new(
            move |surface: &dyn Surface, view, width, height| {
                let _ = scenario::capture_frame(surface, view, width, height, &path);
            },
        ));
        true
    }
}

fn boot_state(
    runtime: &Rc<RefCell<Runtime>>,
    _window: &dyn HostWindow,
    _commands: &WindowCommands,
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
    Init {
        state: AppState::new(project_worker, update_worker, update_settings, identity),
        logic: root as Logic,
        sheet: theme::sheet(),
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
    HostHooks {
        // Firewheel's meter/capture promotion and Hocket's custom leaves are
        // live frame work, so retain the original 60 fps tick policy.
        frame: Box::new(move |ctx| {
            let mut runtime = frame_runtime.borrow_mut();
            drain_worker_updates(ctx, &runtime);
            ctx.runner.update(AppState::tick);
            leaves::reconcile(ctx.leaves, &mut runtime.waveform_cache, ctx.runner.state());
            true
        }),
        after_dispatch: Box::new(|_ctx| {}),
        after_frame: Box::new(move |ctx| {
            let mut runtime = scenario_runtime.borrow_mut();
            let Some(mut run) = runtime.scenario.take() else {
                return;
            };
            let capture_dir = run.dir.clone();
            let progress = {
                let mut driver = ScenarioDriver { ctx, capture_dir };
                let progress = run.scenario.tick(&mut driver);
                drop(driver);
                progress
            };
            match progress {
                genet_probe::Progress::Running => runtime.scenario = Some(run),
                genet_probe::Progress::Done => {
                    scenario::write_done(&run.dir, &run.scenario.finish());
                    *ctx.close = true;
                }
            }
        }),
        after_wake: Box::new(|_ctx| {}),
        close_request: Box::new(|_ctx, _request| CloseDisposition::Exit),
        focused_text: Box::new(|_runner: &AppRunner| None),
        key_intercept: Box::new(|_runner, _press| false),
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
