//! Woodshed's appearance adapter over Tabard definitions and its shared editor.
//!
//! Definitions live in the authored library; the application's durable choice
//! lives in `AppearanceSettings`. Workshop previews never select app appearance.

use std::path::PathBuf;
use std::{collections::BTreeSet, io};

use tabard::Theme;
use tabard::theme::choice::ThemeChoice;
use tabard::theme::registry::{Mode, ThemeSource};
use tabard_workshop::WorkshopState;
use tinct::Palette;
use woodshed_core::settings::AppearanceSettings;

use crate::theme::{ThemeMode, stage_css};

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
            Ok((palette, stylesheet)) => AppearanceResolution {
                theme,
                mode,
                palette,
                stylesheet,
                fallback_reason: reason,
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
        let choice = self.choice(settings);
        let theme = self
            .definition(&choice.theme_id)
            .ok_or_else(|| format!("Theme {} is unavailable", choice.theme_id))?;
        if theme.source == ThemeSource::User {
            self.workshop.select_theme(&theme.id);
        } else {
            let json = tabard::portable::theme_json(&theme).map_err(|error| error.to_string())?;
            self.workshop.import_theme_json(&json);
        }
        if let Some(mode) = choice.theme_mode {
            self.workshop.set_mode(mode);
        }
        self.workshop_open = true;
        Ok(())
    }

    /// Apply the saved definition and selected preview mode explicitly. The
    /// editor owns library transactions; this adapter owns only app selection.
    pub fn apply_workshop(&self, settings: &mut AppearanceSettings) -> Result<(), String> {
        if self.workshop.has_changes() || self.workshop.has_pending_fields() {
            return Err("Save the theme in the workshop before using it in Woodshed.".into());
        }
        let draft = self.workshop.draft_theme();
        if draft.source != ThemeSource::User
            || ThemeMode::from_theme_id(&draft.id).is_some()
            || self.workshop.registry().theme_def(&draft.id) != Some(draft)
        {
            return Err("Save an authored copy before using it in Woodshed.".into());
        }
        self.select(settings, &draft.id, Some(self.workshop.mode().clone()))
    }

    /// Additional attached modes remain available alongside all four canonical
    /// profiles. Empty entries do not create a mode users cannot render.
    pub fn modes(&self, id: &str) -> Vec<Mode> {
        let mut modes = CANONICAL_MODES.to_vec();
        if let Some(theme) = self.definition(id) {
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

fn default_mode(theme: &Theme) -> Mode {
    Mode::from_flags(theme.seeds.dark, theme.high_contrast)
}

fn render(theme: &Theme, mode: &Mode) -> Result<(Palette, String), String> {
    if matches!(mode, Mode::Custom(_)) && theme.mode_sheet(mode).is_none() {
        return Err(format!("Mode {} has no authored stylesheet", mode.label()));
    }
    let mut derived = theme.clone();
    derived.mode_sheets.clear();
    let profile = if matches!(mode, Mode::Custom(_)) {
        Mode::Dark
    } else {
        mode.clone()
    };
    let palette = derived
        .palette_for_mode(&profile)
        .map_err(|error| error.to_string())?
        .palette;
    let mut stylesheet = stage_css(&palette);
    if let Some(rules) = theme.mode_sheet(mode) {
        stylesheet.push('\n');
        stylesheet.push_str(&rules.join("\n"));
    }
    Ok((palette, stylesheet))
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
