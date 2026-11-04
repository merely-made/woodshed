//! Woodshed's appearance adapter over Tabard definitions and its shared editor.
//!
//! Definitions live in the authored library; the application's durable choice
//! lives in `AppearanceSettings`. Workshop previews never select app appearance.

use std::cell::RefCell;
use std::path::PathBuf;
use std::{collections::BTreeSet, io};

use tabard::theme::choice::ThemeChoice;
use tabard::theme::registry::{Mode, ThemeSource};
use tabard::{Theme, ThemePresentation};
use tabard_workshop::WorkshopState;
use tinct::Palette;
use woodshed_core::settings::AppearanceSettings;

use crate::theme::{ThemeMode, stage_css, stage_css_from_roles};

pub mod fonts;
mod view;
pub use view::{appearance_page, appearance_stylesheet, workshop_screen};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod view_tests;

pub const CANONICAL_MODES: [Mode; 4] = [Mode::Light, Mode::Dark, Mode::HcLight, Mode::HcDark];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppearanceOption {
    pub id: String,
    pub name: String,
    pub built_in: bool,
}

#[derive(Clone)]
pub struct AppearanceResolution {
    pub theme: Theme,
    pub mode: Mode,
    pub palette: Palette,
    pub stylesheet: String,
    /// Applied theme snapshot without application-owned family variables.
    /// A font-only Apply must not activate a same-ID library edit.
    base_stylesheet: String,
    /// Missing definitions or modes remain visible without damaging the saved
    /// request, so reinstalling the library can recover the user's selection.
    pub fallback_reason: Option<String>,
}

pub struct AppearanceState {
    pub workshop: WorkshopState,
    pub workshop_open: bool,
    pub font_editor: tabard_workshop::fonts::FontRoleEditor,
    pub available_fonts: Vec<String>,
    active: RefCell<Option<(AppearanceSettings, AppearanceResolution)>>,
    pending: Option<AppearanceSelection>,
}

struct AppearanceSelection {
    settings: AppearanceSettings,
    presentation: AppearanceResolution,
}

impl Default for AppearanceState {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl AppearanceState {
    pub fn in_memory() -> Self {
        Self {
            workshop: WorkshopState::in_memory(),
            workshop_open: false,
            font_editor: tabard_workshop::fonts::FontRoleEditor::new(Default::default()),
            available_fonts: Vec::new(),
            active: RefCell::new(None),
            pending: None,
        }
    }

    pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
        let workshop = WorkshopState::load(path)?;
        // Woodshed's named built-ins are additional to Tabard's shared ones.
        // A portable library cannot override these identities in this host.
        for theme in workshop.registry().list() {
            if theme.source == ThemeSource::User && ThemeMode::from_theme_id(&theme.id).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Authored theme {} collides with a Woodshed built-in",
                        theme.id
                    ),
                ));
            }
        }
        Ok(Self {
            workshop,
            workshop_open: false,
            font_editor: tabard_workshop::fonts::FontRoleEditor::new(Default::default()),
            available_fonts: Vec::new(),
            active: RefCell::new(None),
            pending: None,
        })
    }

    pub fn options(&self) -> Vec<AppearanceOption> {
        let mut options: Vec<_> = ThemeMode::ALL
            .into_iter()
            .map(|theme| AppearanceOption {
                id: theme.theme_id().into(),
                name: theme.label().into(),
                built_in: true,
            })
            .collect();
        options.extend(
            self.workshop
                .registry()
                .list()
                .into_iter()
                .filter(|theme| {
                    theme.source == ThemeSource::User
                        && ThemeMode::from_theme_id(&theme.id).is_none()
                })
                .map(|theme| AppearanceOption {
                    id: theme.id.clone(),
                    name: theme.name.clone(),
                    built_in: false,
                }),
        );
        options
    }

    fn definition(&self, id: &str) -> Option<Theme> {
        if let Some(builtin) = ThemeMode::from_theme_id(id) {
            return Some(builtin.definition());
        }
        self.workshop
            .registry()
            .theme_def(id)
            .filter(|theme| theme.source == ThemeSource::User)
            .cloned()
    }

    pub fn choice(&self, settings: &AppearanceSettings) -> ThemeChoice {
        settings.theme_choice.clone().unwrap_or_else(|| {
            let theme = ThemeMode::from_name(&settings.theme).unwrap_or_default();
            ThemeChoice::new(theme.theme_id(), None)
        })
    }

    pub fn resolve(&self, settings: &AppearanceSettings) -> AppearanceResolution {
        if let Some((choice, presentation)) = &*self.active.borrow() {
            if choice == settings {
                return presentation.clone();
            }
        }
        let presentation = self.resolve_current(settings);
        *self.active.borrow_mut() = Some((settings.clone(), presentation.clone()));
        presentation
    }

    /// A restored persona/settings context deliberately reopens its selected
    /// definition. Saving library edits alone never invalidates active paint.
    pub fn reset_active(&mut self) {
        *self.active.get_mut() = None;
        self.pending = None;
    }

    pub fn reset_font_editor(&mut self, settings: &AppearanceSettings) {
        self.font_editor = tabard_workshop::fonts::FontRoleEditor::new(self.choice(settings).fonts);
        self.font_editor
            .set_available_families(self.available_fonts.clone());
    }

    pub fn set_available_fonts(&mut self, families: Vec<String>) {
        self.font_editor.set_available_families(families.clone());
        self.workshop.set_installed_font_families(families.clone());
        self.available_fonts = families;
        self.reset_active();
    }

    fn resolve_current(&self, settings: &AppearanceSettings) -> AppearanceResolution {
        let choice = self.choice(settings);
        let mut reason = None;
        let theme = self.definition(&choice.theme_id).unwrap_or_else(|| {
            reason = Some(format!(
                "Theme {} is unavailable; using {}.",
                choice.theme_id, settings.theme
            ));
            ThemeMode::from_name(&settings.theme)
                .unwrap_or_default()
                .definition()
        });
        let mut mode = choice
            .theme_mode
            .clone()
            .unwrap_or_else(|| default_mode(&theme));
        if matches!(mode, Mode::Custom(_)) && theme.mode_sheet(&mode).is_none() {
            reason = Some(format!(
                "Mode {} is unavailable; using the theme default.",
                mode.label()
            ));
            mode = default_mode(&theme);
        }
        match render(&theme, &mode) {
            Ok((palette, stylesheet, presentation_notice)) => AppearanceResolution {
                theme: theme.clone(),
                mode,
                palette,
                stylesheet: fonts::with_requests(
                    &stylesheet,
                    &theme,
                    &choice.fonts,
                    &self.available_fonts,
                ),
                base_stylesheet: stylesheet,
                fallback_reason: match (reason, presentation_notice) {
                    (Some(reason), Some(notice)) => Some(format!("{reason} {notice}")),
                    (reason, notice) => reason.or_else(|| notice.map(str::to_owned)),
                },
            },
            Err(error) => {
                let theme = ThemeMode::Slate.definition();
                let mode = default_mode(&theme);
                let palette = theme.palette();
                let stylesheet = stage_css(&palette);
                AppearanceResolution {
                    theme,
                    mode,
                    palette,
                    stylesheet: stylesheet.clone(),
                    base_stylesheet: stylesheet,
                    fallback_reason: Some(format!(
                        "Could not render selected appearance: {error}; using Slate."
                    )),
                }
            },
        }
    }

    /// Select only registered, admissible definitions. No editor preview or
    /// failed selection mutates the application setting.
    pub fn select(
        &self,
        settings: &mut AppearanceSettings,
        id: &str,
        mode: Option<Mode>,
    ) -> Result<(), String> {
        let theme = self
            .definition(id)
            .ok_or_else(|| format!("Theme {id} is unavailable"))?;
        let selected_mode = mode.clone().unwrap_or_else(|| default_mode(&theme));
        render(&theme, &selected_mode)?;
        if let Some(builtin) = ThemeMode::from_theme_id(&theme.id) {
            settings.theme = builtin.label().into();
        }
        let fonts = self.choice(settings).fonts;
        let mut choice = ThemeChoice::new(theme.id, mode);
        choice.fonts = fonts;
        settings.theme_choice = Some(choice);
        Ok(())
    }

    /// Open the chosen definition in the existing editor. Product built-ins
    /// import through Tabard's protected copy boundary, keeping their seeds.
    pub fn begin_edit(&mut self, settings: &AppearanceSettings) -> Result<(), String> {
        if self.workshop.has_changes() || self.workshop.has_pending_fields() {
            self.workshop_open = true;
            return Err(
                "Save or discard the current workshop changes before opening another theme.".into(),
            );
        }
        let resolved = self.resolve(settings);
        let theme = self
            .definition(&self.choice(settings).theme_id)
            .unwrap_or(resolved.theme);
        let mode = if matches!(resolved.mode, Mode::Custom(_))
            && theme.mode_sheet(&resolved.mode).is_none()
        {
            default_mode(&theme)
        } else {
            resolved.mode
        };
        self.workshop.edit_definition(&theme, Some(mode))?;
        self.workshop.cancel_close();
        self.workshop_open = true;
        Ok(())
    }

    /// Apply the saved definition and selected preview mode explicitly. The
    /// editor owns library transactions; this adapter owns only app selection.
    pub fn apply_workshop(&self, settings: &mut AppearanceSettings) -> Result<(), String> {
        let choice = self.workshop.saved_choice()?;
        self.select(settings, &choice.theme_id, choice.theme_mode)
    }

    pub fn request_selection(
        &mut self,
        settings: &AppearanceSettings,
        id: &str,
        mode: Option<Mode>,
    ) -> Result<(), String> {
        self.pending = None;
        let mut candidate = settings.clone();
        self.select(&mut candidate, id, mode)?;
        self.pending = Some(AppearanceSelection {
            presentation: self.resolve_current(&candidate),
            settings: candidate,
        });
        Ok(())
    }

    pub fn request_workshop_selection(
        &mut self,
        settings: &AppearanceSettings,
    ) -> Result<(), String> {
        self.pending = None;
        let choice = self.workshop.saved_choice()?;
        self.request_selection(settings, &choice.theme_id, choice.theme_mode)
    }

    /// Additional attached modes remain available alongside all four canonical
    /// profiles. Empty entries do not create a mode users cannot render.
    pub fn modes(&self, id: &str) -> Vec<Mode> {
        let mut modes = CANONICAL_MODES.to_vec();
        let active = self.active.borrow();
        let definition = active
            .as_ref()
            .filter(|(_, presentation)| presentation.theme.id == id)
            .map(|(_, presentation)| presentation.theme.clone())
            .or_else(|| self.definition(id));
        if let Some(theme) = definition {
            let mut keys = BTreeSet::new();
            for (key, rules) in &theme.mode_sheets {
                if !rules.is_empty() && keys.insert(key.clone()) {
                    if let Some(mode @ Mode::Custom(_)) = Mode::from_key(key) {
                        modes.push(mode);
                    }
                }
            }
        }
        modes
    }
}

/// Drain the shared UI request through the composing host's settings policy.
/// No preference or applied presentation changes until persistence succeeds.
/// Returns whether a request was handled, so hosts avoid saving it twice.
pub fn commit_selection(
    ui: &mut crate::stage::UiState,
    persist: impl FnOnce(&woodshed_core::settings::AppSettings) -> Result<(), String>,
) -> bool {
    if let Some(fonts) = ui.appearance.font_editor.take_requested() {
        let mut presentation = ui.appearance.resolve(&ui.app_settings.appearance);
        let mut candidate = ui.app_settings.appearance.clone();
        let mut choice = ui.appearance.choice(&candidate);
        choice.fonts = fonts;
        presentation.stylesheet = fonts::with_requests(
            &presentation.base_stylesheet,
            &presentation.theme,
            &choice.fonts,
            &ui.appearance.available_fonts,
        );
        candidate.theme_choice = Some(choice);
        ui.appearance.pending = Some(AppearanceSelection {
            presentation,
            settings: candidate,
        });
    }
    let Some(candidate) = ui.appearance.pending.take() else {
        return false;
    };
    let mut settings = ui.app_settings.clone();
    settings.appearance = candidate.settings.clone();
    match persist(&settings) {
        Ok(()) => {
            ui.app_settings = settings;
            let fonts = ui.appearance.choice(&ui.app_settings.appearance).fonts;
            ui.appearance.font_editor.applied(fonts);
            *ui.appearance.active.get_mut() = Some((candidate.settings, candidate.presentation));
            ui.appearance_notice = Some("Selected appearance applied to Woodshed.".into());
        },
        Err(error) => {
            ui.appearance_notice = Some(format!(
                "Could not save appearance: {error}. Your selected appearance is unchanged."
            ));
        },
    }
    true
}

fn default_mode(theme: &Theme) -> Mode {
    tabard::theme::seed::default_mode_for_def(theme)
}

fn render(theme: &Theme, mode: &Mode) -> Result<(Palette, String, Option<&'static str>), String> {
    match theme
        .presentation_for_mode(mode)
        .map_err(|error| error.to_string())?
    {
        ThemePresentation::Derived(resolved) => Ok((
            resolved.palette,
            format!(
                "{}\n{}\n{}",
                stage_css(&resolved.palette),
                view::APPEARANCE_CSS,
                fonts::role_stylesheet(theme)
            ),
            None,
        )),
        ThemePresentation::AuthoredStylesheet(rules) => {
            // Existing typed paint leaves need explicit defaults. These seeds
            // supply fallback colors only; they are not the authored CSS's
            // computed palette. The product CSS consumes shared role variables
            // and then the exact sheet takes cascade authority.
            let mut fallback = theme.clone();
            fallback.mode_sheets.clear();
            let profile = if matches!(mode, Mode::Custom(_)) {
                Mode::from_flags(theme.seeds.dark, theme.high_contrast)
            } else {
                mode.clone()
            };
            let palette = fallback
                .palette_for_mode(&profile)
                .map_err(|error| error.to_string())?
                .palette;
            let stylesheet = format!(
                "{}\n{}\n{}\n{}",
                stage_css_from_roles(&palette),
                view::APPEARANCE_CSS,
                fonts::role_stylesheet(theme),
                rules.join("\n")
            );
            Ok((
                palette,
                stylesheet,
                Some(
                    "Authored CSS controls Woodshed's appearance. Typed instrument and graph paint retain explicit seed palette defaults.",
                ),
            ))
        },
    }
}

#[cfg(test)]
#[path = "appearance/tests.rs"]
mod tests;
