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
        assert!(
            resolved.stylesheet.starts_with(&builtin.css()),
            "every legacy stage rule stays verbatim before optional role mappings"
        );
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
