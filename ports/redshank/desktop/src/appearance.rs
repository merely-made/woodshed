//! Application appearance over Tabard's choice store and authoring workshop.
//!
//! Appearance never changes playback, capture, feed, annotation or listener
//! authority. The applied presentation is held until an explicit saved choice
//! succeeds, even when the editor saves that same definition identity.

use std::path::PathBuf;

use tabard::theme::choice::{
    FileThemeChoiceStore, InMemoryThemeChoiceStore, ThemeChoice, ThemeChoiceStore,
};
use tabard::theme::registry::{Mode, ThemeSource};
use tabard::theme::seed::default_mode_for_def;
use tabard::{ResolvedThemeChoice, ThemePresentation, resolve_theme_choice};
use tabard_workshop::WorkshopState;

pub const PRODUCT_DEFAULT: &str = "redshank:default";
pub const MODES: [Mode; 4] = [Mode::Light, Mode::Dark, Mode::HcLight, Mode::HcDark];

pub struct AppearanceState {
    pub workshop: WorkshopState,
    pub open: bool,
    pub editor_open: bool,
    pub close_app: bool,
    pub authoring_available: bool,
    pub notice: Option<String>,
    store: Box<dyn ThemeChoiceStore>,
    selection_path: Option<PathBuf>,
    selection_error: Option<String>,
    library_error: Option<String>,
    applied: Option<ResolvedThemeChoice>,
    installed_sheet: Option<String>,
    pending_choice: Option<ThemeChoice>,
}

impl Default for AppearanceState {
    fn default() -> Self {
        Self {
            workshop: WorkshopState::in_memory(),
            open: false,
            editor_open: false,
            close_app: false,
            authoring_available: false,
            notice: None,
            store: Box::new(InMemoryThemeChoiceStore::new(ThemeChoice::new(
                PRODUCT_DEFAULT,
                None,
            ))),
            selection_path: None,
            selection_error: None,
            library_error: None,
            applied: None,
            installed_sheet: None,
            pending_choice: None,
        }
    }
}

impl AppearanceState {
    pub fn load(selection: PathBuf, library: PathBuf, protected_files: Vec<PathBuf>) -> Self {
        let mut state = Self {
            selection_path: Some(selection.clone()),
            ..Self::default()
        };
        match WorkshopState::load(library) {
            Ok(workshop) => {
                state.workshop = workshop;
                state.authoring_available = true;
            },
            Err(error) => {
                let message = format!(
                    "The theme library could not be opened: {error}. Appearance authoring is unavailable until it can be read."
                );
                state.library_error = Some(message);
            },
        }
        let mut protected = protected_files;
        protected.push(selection.clone());
        state.workshop.set_protected_export_paths(protected);
        // Missing preferences retain the exact existing Redshank appearance.
        // The shared file store is installed for the first explicit selection.
        let existing = selection.exists();
        match FileThemeChoiceStore::load_strict(selection) {
            Ok(store) => {
                if existing && store.choice().theme_id != PRODUCT_DEFAULT {
                    match resolve_theme_choice(state.workshop.registry(), store.choice()) {
                        Ok(resolved) => state.applied = Some(resolved),
                        Err(error) => {
                            state.notice = Some(format!(
                                "The saved appearance could not be rendered: {error}. Redshank defaults remain active."
                            ))
                        },
                    }
                }
                state.store = Box::new(store);
            },
            Err(error) => {
                let message = format!(
                    "Appearance preferences could not be read: {error}. The existing file is preserved; repair it before saving an appearance choice."
                );
                state.selection_error = Some(message);
            },
        }
        state
    }

    pub fn protect_media(&mut self, paths: Vec<PathBuf>) {
        let mut paths = paths;
        if let Some(path) = &self.selection_path {
            paths.push(path.clone());
        }
        self.workshop.set_protected_export_paths(paths);
    }

    pub fn applied(&self) -> Option<&ResolvedThemeChoice> {
        self.applied.as_ref()
    }

    pub fn active_id(&self) -> &str {
        self.applied.as_ref().map_or(PRODUCT_DEFAULT, |resolved| {
            resolved.resolved.theme_id.as_str()
        })
    }

    pub fn modes(&self) -> Vec<Mode> {
        let mut modes = MODES.to_vec();
        if let Some(theme) = self.workshop.registry().theme_def(self.active_id()) {
            for (key, rules) in &theme.mode_sheets {
                if !rules.is_empty() {
                    if let Some(mode @ Mode::Custom(_)) = Mode::from_key(key) {
                        if !modes.contains(&mode) {
                            modes.push(mode);
                        }
                    }
                }
            }
        }
        modes
    }

    fn candidate(
        &self,
        id: &str,
        mode: Option<Mode>,
    ) -> Result<(ThemeChoice, Option<ResolvedThemeChoice>), String> {
        if let Some(error) = &self.selection_error {
            return Err(error.clone());
        }
        let prepared = if id == PRODUCT_DEFAULT {
            (ThemeChoice::new(PRODUCT_DEFAULT, None), None)
        } else {
            let theme = self
                .workshop
                .registry()
                .theme_def(id)
                .ok_or_else(|| format!("Theme {id} is unavailable."))?;
            let mode = mode.unwrap_or_else(|| default_mode_for_def(theme));
            theme
                .presentation_for_mode(&mode)
                .map_err(|error| error.to_string())?;
            let choice = ThemeChoice::new(id, Some(mode));
            let resolved = resolve_theme_choice(self.workshop.registry(), &choice)
                .map_err(|error| error.to_string())?;
            (choice, Some(resolved))
        };
        Ok(prepared)
    }

    pub fn select(&mut self, id: &str, mode: Option<Mode>) -> Result<(), String> {
        let (choice, candidate) = self.candidate(id, mode)?;
        if let Some(parent) = self.selection_path.as_ref().and_then(|path| path.parent()) {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Appearance preferences were not saved: {error}"))?;
        }
        self.store
            .set_choice(choice)
            .map_err(|error| format!("Appearance preferences were not saved: {error}"))?;
        self.applied = candidate;
        self.notice = None;
        Ok(())
    }

    /// Views request choices; the native dispatch tail owns I/O and publishes
    /// the selected presentation only after the shared store accepts it.
    pub fn request_select(&mut self, id: &str, mode: Option<Mode>) -> Result<(), String> {
        let (choice, _) = self.candidate(id, mode)?;
        self.pending_choice = Some(choice);
        Ok(())
    }

    pub fn commit_requested(&mut self) {
        if let Some(choice) = self.pending_choice.take() {
            self.notice = match self.select(&choice.theme_id, choice.theme_mode) {
                Ok(()) => Some(
                    if self.store.is_persistent() {
                        "Saved appearance applied to Redshank."
                    } else {
                        "Appearance applied for this session."
                    }
                    .into(),
                ),
                Err(error) => Some(error),
            };
        }
    }

    pub fn begin_edit(
        &mut self,
        surface: &redshank_surfaces::RedshankSurfaceState,
    ) -> Result<(), String> {
        if !self.authoring_available {
            return Err(
                "Appearance authoring is unavailable until the theme library can be read.".into(),
            );
        }
        if self.editor_open {
            return Ok(());
        }
        if self.workshop.has_changes() || self.workshop.has_pending_fields() {
            self.editor_open = true;
            return Err(
                "Save or discard the current workshop changes before opening another theme.".into(),
            );
        }
        let (theme, mode) = if let Some(applied) = &self.applied {
            (applied.theme.clone(), applied.resolved.theme_mode.clone())
        } else {
            let mut theme = tabard::Theme::new(
                PRODUCT_DEFAULT,
                "Redshank",
                redshank_surfaces::theme::seeds(surface.seed),
            );
            theme.source = ThemeSource::BuiltIn;
            if surface.seed == redshank_surfaces::Seed::Wetland {
                theme.mode_sheets.insert(
                    "dark".into(),
                    vec![redshank_surfaces::theme::tabard_role_css(
                        surface.seed,
                        redshank_surfaces::Mode::Dark,
                    )],
                );
            }
            let mode = match surface.mode {
                redshank_surfaces::Mode::Light => Mode::Light,
                redshank_surfaces::Mode::Dark => Mode::Dark,
                redshank_surfaces::Mode::HcLight => Mode::HcLight,
                redshank_surfaces::Mode::HcDark => Mode::HcDark,
            };
            (theme, Some(mode))
        };
        self.workshop.edit_definition(&theme, mode)?;
        self.workshop.cancel_close();
        self.editor_open = true;
        Ok(())
    }

    pub fn apply_workshop(&mut self) -> Result<(), String> {
        let choice = self.workshop.saved_choice()?;
        if choice.theme_id == PRODUCT_DEFAULT {
            return Err("This theme identity belongs to Redshank's built-in appearance. Make a new copy before applying it.".into());
        }
        self.request_select(&choice.theme_id, choice.theme_mode)
    }

    pub fn stylesheet(&self, surface: &redshank_surfaces::RedshankSurfaceState) -> String {
        if self.editor_open {
            return format!(
                "{}\n{}",
                tabard_workshop::workshop_stylesheet(),
                crate::appearance_view::EDITOR_CSS
            );
        }
        let chrome = format!(
            "{}{}",
            cambium::TITLE_BAR_CSS,
            crate::appearance_view::APPEARANCE_CSS
        );
        let inherited = redshank_surfaces::theme::host_role_css(surface.seed, surface.mode)
            .replace(".rs-app", ".redshank-desktop");
        let base = format!("{}\n{inherited}", redshank_surfaces::sheet());
        let Some(applied) = &self.applied else {
            return format!("{base}\n{chrome}");
        };
        let roles = redshank_surfaces::theme::host_role_css(surface.seed, surface.mode);
        match &applied.presentation {
            ThemePresentation::Derived(mode) => format!(
                "{base}\n{}\n{roles}\n{chrome}",
                mode.css_custom_properties()
            ),
            ThemePresentation::AuthoredStylesheet(rules) => {
                format!("{base}\n{roles}\n{chrome}\n{}", rules.join("\n"))
            },
        }
    }

    /// Ordinary typing, graph gestures and worker events retain the host's
    /// layout and caret. Only a changed appearance/editor sheet reskins it.
    pub fn take_stylesheet_change(
        &mut self,
        surface: &redshank_surfaces::RedshankSurfaceState,
    ) -> Option<String> {
        let sheet = self.stylesheet(surface);
        if self.installed_sheet.as_ref() == Some(&sheet) {
            return None;
        }
        self.installed_sheet = Some(sheet.clone());
        Some(sheet)
    }

    pub fn diagnostics(&self) -> Vec<String> {
        let mut diagnostics: Vec<_> = self
            .applied
            .as_ref()
            .into_iter()
            .flat_map(|resolved| resolved.diagnostics.iter().map(ToString::to_string))
            .collect();
        if self.applied.as_ref().is_some_and(|resolved| {
            matches!(
                resolved.presentation,
                ThemePresentation::AuthoredStylesheet(_)
            )
        }) {
            diagnostics.push("Authored CSS controls Redshank's appearance. Omitted roles retain the selected legacy seed and mode colors.".into());
        }
        for diagnostic in [&self.selection_error, &self.library_error, &self.notice]
            .into_iter()
            .flatten()
        {
            if !diagnostics.contains(diagnostic) {
                diagnostics.push(diagnostic.clone());
            }
        }
        diagnostics
    }
}

#[derive(Default)]
pub struct DesktopState {
    pub surface: redshank_surfaces::RedshankSurfaceState,
    pub appearance: AppearanceState,
}
pub type Child =
    Box<dyn cambium::AnyView<DesktopState, (), cambium::GenetCtx, cambium::GenetElement>>;
pub type Logic = fn(&DesktopState) -> Child;

pub fn root(state: &DesktopState) -> Child {
    if state.appearance.editor_open {
        return crate::appearance_view::editor(state);
    }
    let mut children: Vec<Child> = vec![crate::appearance_view::chrome(state)];
    if state.appearance.open {
        children.push(crate::appearance_view::panel(state));
    }
    children.push(Box::new(cambium::lens(
        |surface: &mut redshank_surfaces::RedshankSurfaceState| redshank_surfaces::surface(surface),
        |state: &mut DesktopState| &mut state.surface,
    )));
    Box::new(cambium::el("div", children).attr("class", "redshank-desktop"))
}
