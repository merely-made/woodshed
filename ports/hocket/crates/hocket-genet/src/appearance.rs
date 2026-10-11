// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Application appearance over Tabard's choice store and authoring workshop.
//!
//! Appearance never changes recorder session, history, audio, project or custody
//! authority. The applied presentation is held until an explicit saved choice
//! succeeds, even when the editor saves that same definition identity.

use std::path::PathBuf;

use tabard::theme::choice::{
    FileThemeChoiceStore, InMemoryThemeChoiceStore, ThemeChoice, ThemeChoiceStore,
};
use tabard::theme::registry::Mode;
use tabard::theme::seed::default_mode_for_def;
use tabard::{ResolvedThemeChoice, ThemePresentation, resolve_theme_choice};
use tabard_workshop::WorkshopState;

pub const PRODUCT_DEFAULT: &str = "hocket:default";
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
        if same_destination(&selection, &library)
            || protected_files
                .iter()
                .any(|path| same_destination(&selection, path) || same_destination(&library, path))
        {
            let message =
                "Appearance preferences and the theme library need separate paths from each other and application data.".to_string();
            state.selection_error = Some(message.clone());
            state.library_error = Some(message);
            return state;
        }
        match WorkshopState::load(library) {
            Ok(workshop) => {
                if workshop.registry().theme_def(PRODUCT_DEFAULT).is_some() {
                    state.library_error = Some("The theme library contains Hocket's reserved built-in identity. Repair the library before authoring.".into());
                } else {
                    state.workshop = workshop;
                    state.authoring_available = true;
                }
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
        // Missing preferences retain the exact existing Hocket appearance.
        // The shared file store is installed for the first explicit selection.
        let existing = selection.exists();
        match FileThemeChoiceStore::load_strict(selection) {
            Ok(store) => {
                if existing {
                    let resolution = if store.choice().theme_id == PRODUCT_DEFAULT {
                        if store.choice().theme_mode.is_some() {
                            product_resolution(store.choice()).map(Some)
                        } else {
                            Ok(None)
                        }
                    } else {
                        resolve_theme_choice(state.workshop.registry(), store.choice())
                            .map(Some)
                            .map_err(|error| error.to_string())
                    };
                    match resolution {
                        Ok(resolved) => state.applied = resolved,
                        Err(error) => {
                            state.notice = Some(format!(
                                "The saved appearance could not be rendered: {error}. Hocket defaults remain active."
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
            {
                let choice = ThemeChoice::new(PRODUCT_DEFAULT, mode);
                let resolved = if choice.theme_mode.is_some() {
                    Some(product_resolution(&choice)?)
                } else {
                    None
                };
                (choice, resolved)
            }
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
                        "Saved appearance applied to Hocket."
                    } else {
                        "Appearance applied for this session."
                    }
                    .into(),
                ),
                Err(error) => Some(error),
            };
        }
    }

    pub fn begin_edit(&mut self) -> Result<(), String> {
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
        let (theme, mode) = self.applied.as_ref().map_or_else(
            || (crate::theme::builtin(), Some(Mode::Dark)),
            |applied| {
                // Editing follows the durable definition. The installed
                // presentation remains a separate snapshot until Apply.
                let theme = self
                    .workshop
                    .registry()
                    .theme_def(&applied.resolved.theme_id)
                    .cloned()
                    .unwrap_or_else(|| applied.theme.clone());
                (theme, applied.resolved.theme_mode.clone())
            },
        );
        self.workshop.edit_definition(&theme, mode)?;
        self.workshop.cancel_close();
        self.editor_open = true;
        Ok(())
    }

    pub fn apply_workshop(&mut self) -> Result<(), String> {
        let choice = self.workshop.saved_choice()?;
        if choice.theme_id == PRODUCT_DEFAULT {
            return Err("This theme identity belongs to Hocket's built-in appearance. Make a new copy before applying it.".into());
        }
        self.request_select(&choice.theme_id, choice.theme_mode)
    }

    pub fn stylesheet(&self) -> String {
        if self.editor_open {
            return format!(
                "{}\n{}",
                tabard_workshop::workshop_stylesheet(),
                crate::appearance_view::EDITOR_CSS
            );
        }
        let base = format!(
            "{}\n{}\n{}",
            crate::theme::sheet(),
            cambium::TITLE_BAR_CSS,
            crate::appearance_view::APPEARANCE_CSS
        );
        match self.applied.as_ref().map(|applied| &applied.presentation) {
            None => base,
            Some(ThemePresentation::Derived(mode)) => format!(
                "{base}\n{}\n{}",
                mode.css_custom_properties(),
                crate::theme::role_css()
            ),
            Some(ThemePresentation::AuthoredStylesheet(rules)) => {
                format!(
                    "{base}\n{}\n{}\n{}",
                    crate::theme::default_tokens(),
                    crate::theme::role_css(),
                    rules.join("\n")
                )
            },
        }
    }

    pub fn take_stylesheet_change(&mut self) -> Option<String> {
        let sheet = self.stylesheet();
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
            diagnostics.push("Authored CSS controls Hocket's appearance. Omitted roles retain the warm recorder colors.".into());
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

fn same_destination(left: &std::path::Path, right: &std::path::Path) -> bool {
    fn identity(path: &std::path::Path) -> Option<PathBuf> {
        let mut ancestor = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(path)
        };
        let mut suffix = Vec::new();
        let mut result = loop {
            if let Ok(path) = ancestor.canonicalize() {
                break path;
            }
            suffix.push(
                ancestor
                    .components()
                    .next_back()?
                    .as_os_str()
                    .to_os_string(),
            );
            if !ancestor.pop() {
                return None;
            }
        };
        for segment in suffix.iter().rev() {
            match std::path::Path::new(segment).components().next()? {
                std::path::Component::CurDir => {},
                std::path::Component::ParentDir => {
                    result.pop();
                },
                _ => result.push(segment),
            }
        }
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            result = PathBuf::from(result.as_os_str().to_string_lossy().to_lowercase());
        }
        Some(result)
    }
    left == right
        || matches!((identity(left), identity(right)), (Some(left), Some(right)) if left == right)
}

/// The product's built-in keeps its seeds across canonical mode changes.
/// Tabard continues to own both exact-sheet and derived-mode presentation.
fn product_resolution(choice: &ThemeChoice) -> Result<ResolvedThemeChoice, String> {
    let theme = crate::theme::builtin();
    let mode = choice.theme_mode.clone().unwrap_or(Mode::Dark);
    let presentation = theme
        .presentation_for_mode(&mode)
        .map_err(|error| error.to_string())?;
    Ok(ResolvedThemeChoice {
        requested: choice.clone(),
        resolved: ThemeChoice::new(PRODUCT_DEFAULT, Some(mode)),
        theme,
        presentation,
        diagnostics: vec![],
    })
}

/// Device-local appearance is separate from update policy, projects and keys.
pub fn load() -> AppearanceState {
    let settings = crate::update::settings_path();
    let selection = std::env::var_os("HOCKET_APPEARANCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| settings.with_file_name("appearance.json"));
    let library = std::env::var_os("HOCKET_THEME_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| selection.with_file_name("themes.json"));
    let mut protected = vec![settings];
    protected.extend(crate::identity::protected_appearance_destinations());
    AppearanceState::load(selection, library, protected)
}

/// Explicit scenario-only device isolation; ordinary startup owns real audio.
pub fn device_free_receipt() -> bool {
    std::env::var("HOCKET_APPEARANCE_RECEIPT").as_deref() == Ok("1")
        && std::env::var_os("HOCKET_SCENARIO").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabard::theme::registry::THEME_ID_DEFAULT;
    use tabard::{Theme, library::ThemeLibraryStore};

    fn loaded(dir: &tempfile::TempDir) -> AppearanceState {
        AppearanceState::load(
            dir.path().join("choice.json"),
            dir.path().join("themes.json"),
            vec![],
        )
    }

    #[test]
    fn missing_preferences_keep_warm_default_without_writing_files() {
        let dir = tempfile::tempdir().unwrap();
        let state = loaded(&dir);
        assert_eq!(state.active_id(), PRODUCT_DEFAULT);
        assert!(state.stylesheet().contains("--ground: #17130e"));
        assert!(state.authoring_available);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn save_is_separate_from_apply_and_same_id_changes_hold_the_applied_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = loaded(&dir);
        state.begin_edit().unwrap();
        let id = state.workshop.draft_theme().id.clone();
        assert_ne!(id, PRODUCT_DEFAULT);
        assert!(state.apply_workshop().is_err());
        state.workshop.save();
        assert_eq!(state.active_id(), PRODUCT_DEFAULT);
        state.apply_workshop().unwrap();
        state.commit_requested();
        assert_eq!(state.active_id(), id);
        state.editor_open = false;
        let before = state.stylesheet();
        state.begin_edit().unwrap();
        assert_eq!(state.workshop.draft_theme().id, id);
        *state.workshop.text_field_mut("mode-sheet").unwrap() =
            cambium::TextInput::new(".app { background:#123456; }");
        state.workshop.apply_stylesheet();
        state.workshop.save();
        state.editor_open = false;
        assert_eq!(state.stylesheet(), before);
        state.begin_edit().unwrap();
        assert_eq!(state.workshop.draft_theme().id, id);
        assert!(!state.workshop.has_changes());
        state.apply_workshop().unwrap();
        state.commit_requested();
        state.editor_open = false;
        assert!(state.stylesheet().ends_with(".app { background:#123456; }"));
        assert_eq!(loaded(&dir).stylesheet(), state.stylesheet());
    }

    #[test]
    fn all_canonical_modes_use_shared_roles_without_erasing_recording_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = loaded(&dir);
        for mode in MODES {
            state.select(THEME_ID_DEFAULT, Some(mode.clone())).unwrap();
            assert_eq!(
                state.applied().unwrap().resolved.theme_mode.as_ref(),
                Some(&mode)
            );
            let css = state.stylesheet();
            assert!(css.contains("--tabard-color-bg:"));
            assert!(css.contains("--record: #e2493a"));
            assert!(css.contains("--voice-teal: #56b3a8"));
        }
        state.select(PRODUCT_DEFAULT, None).unwrap();
        assert_eq!(loaded(&dir).active_id(), PRODUCT_DEFAULT);
    }

    #[test]
    fn corrupt_library_and_preferences_remain_intact_and_block_writes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("themes.json"), "bad library").unwrap();
        std::fs::write(dir.path().join("choice.json"), "bad choice").unwrap();
        let mut state = loaded(&dir);
        assert!(!state.authoring_available);
        assert!(state.begin_edit().is_err());
        assert!(state.select(THEME_ID_DEFAULT, None).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("themes.json")).unwrap(),
            "bad library"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("choice.json")).unwrap(),
            "bad choice"
        );
        assert_eq!(state.diagnostics().len(), 2);
    }

    #[test]
    fn failed_choice_write_does_not_install_candidate_or_change_the_previous_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = loaded(&dir);
        state.select(THEME_ID_DEFAULT, Some(Mode::Dark)).unwrap();
        let before = state.stylesheet();
        std::fs::remove_file(dir.path().join("choice.json")).unwrap();
        std::fs::create_dir(dir.path().join("choice.json")).unwrap();
        assert!(state.select(THEME_ID_DEFAULT, Some(Mode::Light)).is_err());
        assert_eq!(state.stylesheet(), before);
    }

    #[test]
    fn missing_theme_request_survives_fallback_and_custom_mode_keeps_exact_css() {
        let dir = tempfile::tempdir().unwrap();
        let choice = ThemeChoice::new("user:missing", Some(Mode::Dark));
        std::fs::write(
            dir.path().join("choice.json"),
            serde_json::to_vec(&choice).unwrap(),
        )
        .unwrap();
        let state = loaded(&dir);
        assert!(!state.diagnostics().is_empty());
        assert_eq!(state.store.choice(), &choice);
        let mut theme = Theme::new("user:concert", "Concert", crate::theme::builtin().seeds);
        theme.mode_sheets.insert(
            "custom:concert".into(),
            vec![".app { color:#abcdef; }".into()],
        );
        ThemeLibraryStore::load(dir.path().join("themes.json"))
            .unwrap()
            .save(&[theme])
            .unwrap();
        let mut state = loaded(&dir);
        state
            .select("user:concert", Some(Mode::Custom("concert".into())))
            .unwrap();
        assert!(state.stylesheet().ends_with(".app { color:#abcdef; }"));
        assert_eq!(loaded(&dir).stylesheet(), state.stylesheet());
    }

    #[test]
    fn external_library_change_blocks_unsaved_theme_application() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = loaded(&dir);
        state.begin_edit().unwrap();
        std::fs::write(dir.path().join("themes.json"), "external edit").unwrap();
        state.workshop.save();
        assert!(state.apply_workshop().is_err());
        assert_eq!(state.active_id(), PRODUCT_DEFAULT);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("themes.json")).unwrap(),
            "external edit"
        );
    }
    #[test]
    fn library_and_choice_paths_cannot_share_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("appearance.json");
        let mut state = AppearanceState::load(path.clone(), path.clone(), vec![]);
        assert!(!state.authoring_available);
        assert!(state.select(THEME_ID_DEFAULT, None).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn exports_protect_selection_settings_and_private_data_before_replace() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("update-settings.json");
        let private = dir.path().join("custody");
        let identity = private.join("family-persona.json");
        let mut state = AppearanceState::load(
            dir.path().join("choice.json"),
            dir.path().join("themes.json"),
            vec![settings.clone()],
        );
        state
            .workshop
            .set_protected_export_directories(vec![private]);
        state.begin_edit().unwrap();
        for path in [dir.path().join("choice.json"), settings, identity] {
            state.workshop.request_export();
            let artifact = state.workshop.take_export().unwrap();
            state.workshop.complete_export(artifact, Some(path.clone()));
            assert!(state.workshop.status().contains("protected application"));
            assert!(!path.exists());
            state.workshop.replace_export();
            assert!(!path.exists());
        }
        assert!(state.workshop.has_changes());
    }
    #[test]
    fn hocket_warm_modes_keep_the_product_identity_and_restore_each_mode() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = loaded(&dir);
        for mode in MODES {
            state.select(PRODUCT_DEFAULT, Some(mode.clone())).unwrap();
            assert_eq!(state.active_id(), PRODUCT_DEFAULT);
            let restored = loaded(&dir);
            assert_eq!(
                restored.applied().unwrap().resolved.theme_mode.as_ref(),
                Some(&mode)
            );
            assert_eq!(restored.stylesheet(), state.stylesheet());
        }
    }
    #[cfg(unix)]
    #[test]
    fn missing_choice_and_library_files_cannot_alias_through_a_parent_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        let path = dir.path().join("appearance.json");
        let mut state = AppearanceState::load(path.clone(), alias.join("appearance.json"), vec![]);
        assert!(!state.authoring_available);
        assert!(state.select(THEME_ID_DEFAULT, None).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn appearance_stores_cannot_replace_application_files_or_nested_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let protected = dir.path().join("settings.json");
        std::fs::write(&protected, "keep application bytes").unwrap();
        for (selection, library) in [
            (protected.clone(), dir.path().join("themes.json")),
            (dir.path().join("choice.json"), protected.clone()),
        ] {
            let mut state = AppearanceState::load(selection, library, vec![protected.clone()]);
            assert!(!state.authoring_available);
            assert!(state.select(THEME_ID_DEFAULT, None).is_err());
            assert_eq!(
                std::fs::read_to_string(&protected).unwrap(),
                "keep application bytes"
            );
        }
        #[cfg(unix)]
        {
            let alias = dir.path().join("alias");
            std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
            let real = dir.path().join("absent/nested/choice.json");
            let aliased = alias.join("absent/nested/choice.json");
            assert!(same_destination(&real, &aliased));
            let state = AppearanceState::load(real, aliased, vec![]);
            assert!(!state.authoring_available);
        }
    }
}
