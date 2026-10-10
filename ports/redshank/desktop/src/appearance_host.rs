//! Native seams for the embedded shared editor, over Redshank's existing host.

use crate::appearance::{Child, DesktopState, Logic};
use cambium_genet_winit_host::{AppCtx, choose_save_path};

pub type Context<'a> = AppCtx<'a, DesktopState, Logic, Child>;

#[derive(Default)]
pub struct PreviewBindings(tabard_workshop::native_host::PreviewBindings);

impl PreviewBindings {
    pub fn frame(&mut self, ctx: &mut Context<'_>) {
        if ctx.runner.state().appearance.editor_open {
            self.0.register(
                &ctx.runner.state().appearance.workshop,
                ctx.leaves,
                ctx.producers,
            );
        }
    }
}

pub fn after_dispatch(ctx: &mut Context<'_>) {
    after_dispatch_with_exporter(ctx, |artifact| {
        choose_save_path(
            "Export theme",
            &artifact.suggested_name,
            &[if artifact.format == tabard_workshop::ExportFormat::Css {
                "css"
            } else {
                "json"
            }],
        )
    });
}

pub fn after_dispatch_with_exporter(
    ctx: &mut Context<'_>,
    mut destination: impl FnMut(&tabard_workshop::ExportArtifact) -> Option<std::path::PathBuf>,
) {
    let mut close = false;
    ctx.runner.update(|state| {
        state.appearance.commit_requested();
        let exit = state.appearance.editor_open
            && tabard_workshop::native_host::sync_and_export(
                &mut state.appearance.workshop,
                &mut destination,
            );
        close = crate::appearance_view::sync_editor(state, exit);
    });
    refresh_stylesheet(ctx);
    if close {
        ctx.runner.update(|state| state.appearance.close_app = true);
    }
}

/// The workshop resolves its own dirty-close decision before the listener
/// host starts its existing asynchronous shutdown.
pub fn prepare_close(state: &mut DesktopState) -> bool {
    if state.appearance.editor_open {
        state.appearance.close_app = true;
        if !state.appearance.workshop.request_close() {
            return false;
        }
        state.appearance.workshop.cancel_close();
        state.appearance.editor_open = false;
        state.appearance.close_app = false;
    }
    true
}

pub fn refresh_stylesheet(ctx: &mut Context<'_>) {
    let mut sheet = None;
    ctx.runner
        .update(|state| sheet = state.appearance.take_stylesheet_change(&state.surface));
    if let Some(sheet) = sheet {
        *ctx.set_sheet = Some(sheet);
    }
}

pub fn load(state: &mut DesktopState, data_root: &std::path::Path) {
    use directories::{BaseDirs, ProjectDirs};
    let selection = std::env::var_os("REDSHANK_APPEARANCE_STORE")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            ProjectDirs::from("made", "mere", "redshank")
                .map(|dirs| dirs.config_dir().join("appearance.json"))
        });
    let library = std::env::var_os("REDSHANK_THEME_LIBRARY")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            BaseDirs::new().map(|dirs| dirs.data_local_dir().join("mere/tabard/themes.json"))
        });
    if let (Some(selection), Some(library)) = (selection, library) {
        let protected = std::fs::read_dir(data_root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        state.appearance = crate::appearance::AppearanceState::load(selection, library, protected);
        state
            .appearance
            .workshop
            .set_protected_export_directories(vec![data_root.to_path_buf()]);
    } else {
        state.appearance.notice = Some("No application storage directory is available. Appearance changes are session-only and authoring is unavailable.".into());
    }
}
