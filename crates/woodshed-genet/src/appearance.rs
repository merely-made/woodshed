//! Native bindings for Woodshed's shared Tabard appearance editor.

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;

use cambium_genet_winit_host::{CloseDisposition, SceneProducer, choose_save_path};
use cambium_rootstock::{ProducerRole, ProducerSemantics};
use directories::ProjectDirs;
use tabard_workshop::{
    ExportArtifact, ExportFormat, GRAPH_LEAF_KEY, PreviewScene, READER_LEAF_KEY, ReaderSpecimen,
    STYLESHEET_LEAF_KEY, StylesheetSpecimen,
};
use woodshed_views::appearance::{AppearanceState, appearance_stylesheet};
use woodshed_views::stage::UiState;

use crate::sync::Ctx;

const READER_RASTER_KEY: u64 = 0x7461_6261_7264_7264;
const STYLESHEET_RASTER_KEY: u64 = 0x7461_6261_7264_6373;

const HOST_CHROME_CSS: &str = "
.desktop-frame { padding: 0; }
.desktop-frame > .root { padding: 8px 16px 16px; }
.desktop-frame > .cambium-title-bar { --titlebar-padding: 16px; --titlebar-min-height: 42px; }
.woodshed-mark { font-weight: 700; padding: 3px 6px; border: 1px solid currentColor; border-radius: 4px; }
";

fn library_path(
    theme_override: Option<OsString>,
    state_override: Option<OsString>,
    config: Option<PathBuf>,
) -> Option<PathBuf> {
    theme_override
        .map(PathBuf::from)
        .or_else(|| state_override.map(|path| PathBuf::from(path).with_extension("themes.json")))
        .or_else(|| config.map(|path| path.join("themes.json")))
}

/// The isolated visual lane keeps the production host, views and stores while
/// avoiding audio device activation. Ordinary startup always retains hardware.
pub fn device_free_receipt() -> bool {
    std::env::var("WOODSHED_APPEARANCE_RECEIPT").as_deref() == Ok("1")
        && std::env::var_os("WOODSHED_SCENARIO").is_some()
}

pub fn load_library(ui: &mut UiState) {
    let path = library_path(
        std::env::var_os("WOODSHED_THEME_LIBRARY"),
        std::env::var_os("WOODSHED_STATE"),
        ProjectDirs::from("dev", "Woodshed", "Woodshed")
            .map(|dirs| dirs.config_dir().to_path_buf()),
    );
    ui.appearance_authoring_available = false;
    let Some(path) = path else {
        ui.appearance_notice = Some("Appearance authoring is unavailable because no application configuration directory was found.".into());
        return;
    };
    load_library_at(ui, path);
}

fn load_library_at(ui: &mut UiState, path: PathBuf) {
    ui.appearance_authoring_available = false;
    match AppearanceState::load(&path) {
        Ok(mut appearance) => {
            appearance.workshop.set_protected_export_paths(
                crate::storage::FsBackend::new().protected_export_paths(),
            );
            ui.appearance = appearance;
            ui.appearance_authoring_available = true;
            ui.appearance_notice = None;
        },
        Err(error) => {
            ui.appearance_notice = Some(format!(
                "Could not open theme library {}: {error}. Appearance authoring is unavailable until the library can be read.",
                path.display()
            ));
        },
    }
}

pub fn stylesheet(ui: &UiState) -> String {
    format!(
        "{}\n{}\n{}",
        appearance_stylesheet(ui),
        cambium::TITLE_BAR_CSS,
        HOST_CHROME_CSS
    )
}

pub fn after_dispatch(ctx: &mut Ctx<'_>) {
    after_dispatch_with_exporter(ctx, |artifact| {
        let extension = if artifact.format == ExportFormat::Css {
            "css"
        } else {
            "json"
        };
        choose_save_path("Export theme", &artifact.suggested_name, &[extension])
    });
}

fn after_dispatch_with_exporter(
    ctx: &mut Ctx<'_>,
    mut destination: impl FnMut(&ExportArtifact) -> Option<PathBuf>,
) {
    if !ctx.runner.state().appearance.workshop_open {
        return;
    }
    let mut export = None;
    let mut close_app = false;
    ctx.runner.update(|ui| {
        ui.appearance.workshop.sync_controls();
        export = ui.appearance.workshop.take_export();
        if ui.appearance.workshop.exit_requested() {
            if ui.appearance.workshop.has_changes() {
                ui.appearance.workshop.discard();
            }
            ui.appearance.workshop.cancel_close();
            ui.appearance.workshop_open = false;
            close_app = std::mem::take(&mut ui.appearance_close_app);
        } else if !ui.appearance.workshop.close_requested() {
            // Cancelling a native close returns to the ordinary embedded
            // editor. A later Back/Discard action must not close Woodshed.
            ui.appearance_close_app = false;
        }
    });
    if let Some(artifact) = export {
        let path = destination(&artifact);
        ctx.runner
            .update(|ui| ui.appearance.workshop.complete_export(artifact, path));
    }
    if close_app {
        *ctx.close = true;
    }
}

pub fn close_request(ctx: &mut Ctx<'_>) -> CloseDisposition {
    let mut allow = true;
    ctx.runner.update(|ui| {
        if ui.appearance.workshop_open {
            ui.appearance_close_app = true;
            allow = ui.appearance.workshop.request_close();
        }
    });
    if allow {
        CloseDisposition::Exit
    } else {
        CloseDisposition::KeepVisible
    }
}

#[derive(Default)]
pub struct PreviewBindings {
    reader: Option<Rc<RefCell<SceneProducer<ReaderSpecimen>>>>,
    stylesheet: Option<Rc<RefCell<SceneProducer<StylesheetSpecimen>>>>,
}

impl PreviewBindings {
    pub fn sync(&mut self, ctx: &mut Ctx<'_>) {
        if !ctx.runner.state().appearance.workshop_open {
            return;
        }
        let state = &ctx.runner.state().appearance.workshop;
        ctx.leaves
            .insert(GRAPH_LEAF_KEY, Box::new(state.graph_leaf()));
        let reader = state.reader_preview();
        let producer = self.reader.get_or_insert_with(|| {
            Rc::new(RefCell::new(scene_producer(
                reader.clone(),
                READER_RASTER_KEY,
            )))
        });
        producer.borrow_mut().set_source(reader);
        if !ctx.producers.contains(READER_LEAF_KEY) {
            ctx.producers
                .register(READER_LEAF_KEY, producer.clone(), &[])
                .expect("Tabard reader uses its own bounded producer key");
        }
        let stylesheet = state.stylesheet_preview();
        let producer = self.stylesheet.get_or_insert_with(|| {
            Rc::new(RefCell::new(scene_producer(
                stylesheet.clone(),
                STYLESHEET_RASTER_KEY,
            )))
        });
        producer.borrow_mut().set_source(stylesheet);
        if !ctx.producers.contains(STYLESHEET_LEAF_KEY) {
            ctx.producers
                .register(STYLESHEET_LEAF_KEY, producer.clone(), &[])
                .expect("Tabard stylesheet uses its own bounded producer key");
        }
    }
}

fn scene_producer<T: PreviewScene>(source: Rc<RefCell<T>>, raster_key: u64) -> SceneProducer<T> {
    SceneProducer::new(source, raster_key, T::frame, T::revision, |source| {
        Some(ProducerSemantics {
            role: Some(ProducerRole::Image),
            name: Some(source.accessible_name().to_owned()),
            children: Vec::new(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::Logic;
    use cambium_genet_winit_host::{CloseRequest, Harness, HostHooks, Init};
    use taproot::Selector;
    use woodshed_views::stage::UiChild;

    type Host = Harness<UiState, Logic, UiChild>;

    fn mount(ui: UiState, destination: Rc<RefCell<Option<PathBuf>>>) -> Host {
        let mut hooks: HostHooks<UiState, Logic, UiChild> = HostHooks::inert();
        hooks.after_dispatch = Box::new(move |ctx| {
            after_dispatch_with_exporter(ctx, |_| destination.borrow().clone());
            *ctx.set_sheet = Some(stylesheet(ctx.runner.state()));
        });
        hooks.close_request = Box::new(|ctx, _| close_request(ctx));
        let logic: Logic = Box::new(|ui| {
            if ui.appearance.workshop_open {
                woodshed_views::appearance::workshop_screen(ui)
            } else {
                woodshed_views::appearance::appearance_page(ui)
            }
        });
        let mut host = Harness::with_hooks(
            Init {
                sheet: stylesheet(&ui),
                state: ui,
                logic,
                fonts: Vec::new(),
                images: Vec::new(),
            },
            hooks,
        );
        host.layout_at(1280.0, 960.0);
        host
    }

    fn editing(path: PathBuf) -> UiState {
        let mut ui = UiState::new();
        load_library_at(&mut ui, path);
        assert!(ui.appearance_authoring_available);
        ui.appearance
            .begin_edit(&ui.app_settings.appearance)
            .unwrap();
        ui
    }

    #[track_caller]
    fn click(host: &mut Host, action: &str) {
        assert!(
            host.click_on(&Selector::role("button").with_attr("data-action", action)),
            "mounted action {action}"
        );
    }

    #[test]
    fn explicit_and_scenario_library_paths_preserve_private_profile_isolation() {
        let config = Some(PathBuf::from("/profile/config"));
        assert_eq!(
            library_path(
                Some("/explicit/library.json".into()),
                Some("/scenario/state.json".into()),
                config.clone()
            ),
            Some(PathBuf::from("/explicit/library.json"))
        );
        assert_eq!(
            library_path(None, Some("/scenario/state.json".into()), config.clone()),
            Some(PathBuf::from("/scenario/state.themes.json"))
        );
        assert_eq!(
            library_path(None, None, config),
            Some(PathBuf::from("/profile/config/themes.json"))
        );
        assert_eq!(library_path(None, None, None), None);
    }

    #[test]
    fn corrupt_library_disables_authoring_and_preserves_the_requested_appearance_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("themes.json");
        std::fs::write(&path, "corrupt library").unwrap();
        let mut ui = UiState::new();
        ui.set_theme(woodshed_views::theme::ThemeMode::Ember);
        let settings = ui.app_settings.appearance.clone();
        load_library_at(&mut ui, path.clone());
        assert!(!ui.appearance_authoring_available);
        assert_eq!(ui.app_settings.appearance, settings);
        assert!(
            ui.appearance_notice
                .as_ref()
                .unwrap()
                .contains("Could not open theme library")
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "corrupt library");
        let host = mount(ui, Rc::new(RefCell::new(None)));
        assert!(host.with_dom(|dom| {
            taproot::matching(
                dom,
                &Selector::role("button").with_attr("data-action", "edit-appearance"),
            )
            .is_empty()
        }));
    }

    #[test]
    fn cancelled_native_close_then_embedded_discard_does_not_exit_the_application() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = mount(
            editing(dir.path().join("themes.json")),
            Rc::new(RefCell::new(None)),
        );
        host.request_close(CloseRequest::Native);
        host.relayout();
        assert!(!host.close_requested());
        assert!(host.state().appearance_close_app);
        assert!(host.state().appearance.workshop.close_requested());
        click(&mut host, "cancel-close");
        assert!(!host.state().appearance_close_app);
        click(&mut host, "back-to-woodshed");
        click(&mut host, "discard-close");
        assert!(!host.close_requested());
        assert!(!host.state().appearance.workshop_open);
        assert!(!host.state().appearance.workshop.exit_requested());
        click(&mut host, "edit-appearance");
        click(&mut host, "back-to-woodshed");
        assert!(host.state().appearance.workshop.close_requested());
        assert!(!host.close_requested());
    }

    #[test]
    fn native_save_confirmation_persists_the_definition_then_closes_the_application() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("themes.json");
        let mut host = mount(editing(path.clone()), Rc::new(RefCell::new(None)));
        let id = host.state().appearance.workshop.draft_theme().id.clone();
        host.request_close(CloseRequest::Native);
        host.relayout();
        click(&mut host, "save-close");
        assert!(host.close_requested());
        assert!(!host.state().appearance.workshop_open);
        assert!(!host.state().appearance.workshop.exit_requested());
        let restored = AppearanceState::load(path).unwrap();
        assert!(restored.options().iter().any(|option| option.id == id));
    }

    #[test]
    fn export_destination_cancellation_and_collision_leave_library_unsaved() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("themes.json");
        let output = dir.path().join("export.json");
        let destination = Rc::new(RefCell::new(None));
        let mut host = mount(editing(library.clone()), destination.clone());
        host.update(|ui| ui.appearance.workshop.request_export());
        host.after_dispatch();
        assert!(
            host.state()
                .appearance
                .workshop
                .status()
                .contains("cancelled")
        );
        assert!(!output.exists());
        *destination.borrow_mut() = Some(output.clone());
        host.update(|ui| ui.appearance.workshop.request_export());
        host.after_dispatch();
        let original = std::fs::read(&output).unwrap();
        assert!(!library.exists());
        assert!(host.state().appearance.workshop.has_changes());
        host.update(|ui| ui.appearance.workshop.request_export());
        host.after_dispatch();
        assert_eq!(std::fs::read(output).unwrap(), original);
        assert!(!library.exists());
        assert!(host.state().appearance.workshop.has_changes());
    }

    #[test]
    fn native_export_rejects_existing_and_missing_application_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("themes.json");
        let preferences = dir.path().join("settings.json");
        let missing_session = dir.path().join("practice.json");
        std::fs::write(&preferences, b"owned preference bytes").unwrap();
        let destination = Rc::new(RefCell::new(Some(preferences.clone())));
        let mut ui = editing(library.clone());
        ui.appearance
            .workshop
            .set_protected_export_paths(vec![preferences.clone(), missing_session.clone()]);
        let mut host = mount(ui, destination.clone());
        for output in [&preferences, &missing_session] {
            *destination.borrow_mut() = Some(output.clone());
            host.update(|ui| ui.appearance.workshop.request_export());
            host.after_dispatch();
            assert!(
                host.state()
                    .appearance
                    .workshop
                    .status()
                    .contains("protected application")
            );
            host.update(|ui| ui.appearance.workshop.replace_export());
            host.after_dispatch();
            assert!(
                host.state()
                    .appearance
                    .workshop
                    .status()
                    .contains("protected application")
            );
            assert_eq!(
                std::fs::read(&preferences).unwrap(),
                b"owned preference bytes"
            );
            assert!(!missing_session.exists());
            assert!(!library.exists());
        }
    }

    #[test]
    fn current_native_appearance_scenarios_parse_with_the_shared_driver() {
        for scenario in [
            include_str!("../../../scenarios/tabard_appearance.scn"),
            include_str!("../../../scenarios/tabard_appearance_reopen.scn"),
        ] {
            taproot::Scenario::parse(scenario).expect("production appearance scenario grammar");
        }
    }
}
