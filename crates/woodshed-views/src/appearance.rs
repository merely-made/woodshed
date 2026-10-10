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
    /// Missing definitions or modes remain visible without damaging the saved
    /// request, so reinstalling the library can recover the user's selection.
    pub fallback_reason: Option<String>,
}

pub struct AppearanceState {
    pub workshop: WorkshopState,
    pub workshop_open: bool,
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
                theme,
                mode,
                palette,
                stylesheet,
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
                    stylesheet,
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
        settings.theme_choice = Some(ThemeChoice::new(theme.id, mode));
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
    let Some(candidate) = ui.appearance.pending.take() else {
        return false;
    };
    let mut settings = ui.app_settings.clone();
    settings.appearance = candidate.settings.clone();
    match persist(&settings) {
        Ok(()) => {
            ui.app_settings = settings;
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
        ThemePresentation::Derived(resolved) => {
            Ok((resolved.palette, stage_css(&resolved.palette), None))
        },
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
            let stylesheet = format!("{}\n{}", stage_css_from_roles(&palette), rules.join("\n"));
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
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tabard::library::ThemeLibraryStore;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "woodshed-appearance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn library(&self) -> PathBuf {
            self.0.join("themes.json")
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn legacy_themes_preserve_every_palette_role_and_stage_rule() {
        let state = AppearanceState::in_memory();
        for builtin in ThemeMode::ALL {
            let settings = AppearanceSettings {
                theme: builtin.label().into(),
                theme_choice: None,
            };
            let resolved = state.resolve(&settings);
            assert_eq!(resolved.theme.id, builtin.theme_id());
            assert_eq!(resolved.palette, tinct::derive_palette(&builtin.seeds()));
            assert_eq!(resolved.stylesheet, builtin.css());
            assert_eq!(resolved.mode.dark(), builtin.seeds().dark);
            assert!(!resolved.mode.high_contrast());
            assert!(resolved.fallback_reason.is_none());
        }
    }

    #[test]
    fn canonical_modes_use_shared_derivation_without_changing_legacy_seeds() {
        let state = AppearanceState::in_memory();
        let mut settings = AppearanceSettings::default();
        for mode in CANONICAL_MODES {
            state
                .select(&mut settings, "woodshed:slate", Some(mode.clone()))
                .unwrap();
            let resolved = state.resolve(&settings);
            assert_eq!(resolved.mode, mode);
            assert_eq!(resolved.theme.seeds, ThemeMode::Slate.seeds());
            assert_eq!(
                resolved.palette,
                resolved.theme.palette_for_mode(&mode).unwrap().palette
            );
        }
        let previous = settings.clone();
        assert!(
            state
                .select(&mut settings, "theme:missing", Some(Mode::Dark))
                .is_err()
        );
        assert_eq!(settings, previous);
        assert!(
            state
                .select(
                    &mut settings,
                    "woodshed:slate",
                    Some(Mode::Custom("missing".into()))
                )
                .is_err()
        );
        assert_eq!(settings, previous);
    }

    #[test]
    fn saved_builtin_copy_applies_and_restores_without_preview_activating_it() {
        let dir = TempDir::new();
        let mut state = AppearanceState::load(dir.library()).unwrap();
        let mut settings = AppearanceSettings {
            theme: "Ember".into(),
            theme_choice: None,
        };
        state.begin_edit(&settings).unwrap();
        let id = state.workshop.draft_theme().id.clone();
        assert_ne!(id, ThemeMode::Ember.theme_id());
        assert_eq!(state.workshop.draft_theme().source, ThemeSource::User);
        assert_eq!(state.workshop.draft_theme().seeds, ThemeMode::Ember.seeds());
        state.workshop.set_mode(Mode::HcLight);
        assert_eq!(
            state.resolve(&settings).theme.id,
            ThemeMode::Ember.theme_id()
        );
        assert!(state.apply_workshop(&mut settings).is_err());
        assert!(settings.theme_choice.is_none());
        state.workshop.save();
        state.apply_workshop(&mut settings).unwrap();
        assert_eq!(settings.theme, "Ember");
        let restored_settings: AppearanceSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        let restored = AppearanceState::load(dir.library()).unwrap();
        let appearance = restored.resolve(&restored_settings);
        assert_eq!(appearance.theme.id, id);
        assert_eq!(appearance.mode, Mode::HcLight);
        assert!(appearance.fallback_reason.is_none());
    }

    #[test]
    fn failed_library_save_cannot_apply_unsaved_theme_and_preserves_selection() {
        let dir = TempDir::new();
        let mut state = AppearanceState::load(dir.library()).unwrap();
        let mut settings = AppearanceSettings::default();
        state.begin_edit(&settings).unwrap();
        // The store must refuse an external writer, and no saved registration
        // becomes available for the application to select on that failure.
        fs::write(dir.library(), "external edit").unwrap();
        state.workshop.save();
        assert!(state.apply_workshop(&mut settings).is_err());
        assert_eq!(settings, AppearanceSettings::default());
        assert_eq!(fs::read_to_string(dir.library()).unwrap(), "external edit");
    }

    #[test]
    fn missing_choice_remains_recoverable_and_custom_css_uses_exact_sheet() {
        let dir = TempDir::new();
        let mut theme = Theme::new("theme:custom", "My theme", ThemeMode::Slate.seeds());
        theme.mode_sheets.insert(
            "hc_light".into(),
            vec![".root { background: #123456; }".into()],
        );
        theme.mode_sheets.insert(
            "custom:concert".into(),
            vec![".root { color: #abcdef; }".into()],
        );
        let mut library = ThemeLibraryStore::load(dir.library()).unwrap();
        library.save(&[theme.clone()]).unwrap();
        let state = AppearanceState::load(dir.library()).unwrap();
        let mut settings = AppearanceSettings::default();
        state
            .select(&mut settings, &theme.id, Some(Mode::HcLight))
            .unwrap();
        let resolved = state.resolve(&settings);
        assert!(
            resolved
                .stylesheet
                .ends_with(".root { background: #123456; }")
        );
        let mut pure = theme.clone();
        pure.mode_sheets.clear();
        assert_eq!(
            resolved.palette,
            pure.palette_for_mode(&Mode::HcLight).unwrap().palette
        );
        assert!(
            state
                .modes(&theme.id)
                .contains(&Mode::Custom("concert".into()))
        );
        state
            .select(
                &mut settings,
                &theme.id,
                Some(Mode::Custom("concert".into())),
            )
            .unwrap();
        assert!(
            state
                .resolve(&settings)
                .stylesheet
                .ends_with(".root { color: #abcdef; }")
        );
        settings.theme_choice = Some(ThemeChoice::new("theme:missing", Some(Mode::Dark)));
        let previous = settings.clone();
        assert!(state.resolve(&settings).fallback_reason.is_some());
        assert_eq!(settings, previous);
        library.save(&[]).unwrap();
        let restored = AppearanceState::load(dir.library()).unwrap();
        assert!(restored.resolve(&settings).fallback_reason.is_some());
    }

    #[test]
    fn product_builtin_collision_is_visible_at_load() {
        let dir = TempDir::new();
        let theme = Theme::new("woodshed:slate", "Collision", ThemeMode::Slate.seeds());
        ThemeLibraryStore::load(dir.library())
            .unwrap()
            .save(&[theme])
            .unwrap();
        assert!(AppearanceState::load(dir.library()).is_err());
    }
}
