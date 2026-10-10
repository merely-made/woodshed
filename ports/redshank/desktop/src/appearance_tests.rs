//! Mounted appearance acceptance with no playback or capture authority.
use super::*;
use appearance::{AppearanceState, DesktopState};
use cambium_genet_winit_host::{Harness, inert_hooks};
use tabard::{Theme, ThemePresentation, library::ThemeLibraryStore, theme::registry::Mode};
use taproot::Selector;

type Host = Harness<DesktopState, Logic, Child>;

fn saved_theme(library: &std::path::Path, authored: bool) -> Theme {
    let mut theme = Theme::new(
        "user:redshank-test",
        "Saved listening theme",
        redshank_surfaces::theme::seeds(redshank_surfaces::Seed::Wetland),
    );
    if authored {
        theme.mode_sheets.insert(
            "dark".into(),
            vec![":root { --tabard-color-bg:#123456; --tabard-color-text:#f1f2f3; }".into()],
        );
        theme.mode_sheets.insert(
            "custom:garden".into(),
            vec![":root { --tabard-color-bg:#203020; }".into()],
        );
    }
    ThemeLibraryStore::load(library)
        .unwrap()
        .save(&[theme.clone()])
        .unwrap();
    theme
}

fn launch(selection: &std::path::Path, library: &std::path::Path) -> Host {
    let mut state = DesktopState {
        surface: Default::default(),
        appearance: AppearanceState::load(selection.into(), library.into(), Vec::new()),
    };
    let sheet = state
        .appearance
        .take_stylesheet_change(&state.surface)
        .unwrap();
    let mut hooks = inert_hooks();
    hooks.after_dispatch =
        Box::new(|ctx| appearance_host::after_dispatch_with_exporter(ctx, |_| None));
    hooks.focused_text = Box::new(focused_text);
    hooks.key_intercept = Box::new(key_intercept);
    let mut host = Host::with_hooks(
        Init {
            state,
            logic: appearance::root as Logic,
            sheet,
            fonts: plex_fonts(),
            images: Vec::new(),
        },
        hooks,
    );
    host.layout_at(1200.0, 900.0);
    host
}

fn click(host: &mut Host, selector: Selector) {
    assert!(
        host.click_on(&selector),
        "actual mounted appearance control"
    );
    host.after_dispatch();
    host.relayout();
}

fn rendered_color(host: &Host, class: &str, property: &str, expected: tinct::Srgb) {
    let node = host.with_dom(|dom| taproot::matching(dom, &Selector::class(class))[0]);
    let actual = host
        .computed_value(node, property)
        .unwrap()
        .to_ascii_lowercase()
        .replace(' ', "");
    let hex = tinct::color_to_hex(expected).to_ascii_lowercase();
    let rgb = format!("rgb({},{},{})", expected.r, expected.g, expected.b);
    let rgba = format!("rgba({},{},{},1)", expected.r, expected.g, expected.b);
    assert!([hex, rgb, rgba].contains(&actual), "{property}: {actual}");
}

#[test]
fn four_modes_authored_roles_and_reopen_preserve_listener_state_and_queue() {
    let dir = tempfile::tempdir().unwrap();
    let selection = dir.path().join("appearance.json");
    let library = dir.path().join("themes.json");
    let theme = saved_theme(&library, true);
    let mut host = launch(&selection, &library);
    let original = host.state().surface.clone();
    rendered_color(
        &host,
        "rs-app",
        "background-color",
        redshank_surfaces::theme::palette(original.seed, original.mode).bg,
    );
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "toggle-appearance"),
    );
    click(
        &mut host,
        Selector::role("button").with_attr("data-appearance-theme", &theme.id),
    );
    for mode in [Mode::Light, Mode::Dark, Mode::HcLight, Mode::HcDark] {
        click(
            &mut host,
            Selector::role("button").with_attr("data-appearance-mode", &mode.as_key()),
        );
        let expected = if mode == Mode::Dark {
            tinct::Srgb::rgb(0x12, 0x34, 0x56)
        } else {
            theme.palette_for_mode(&mode).unwrap().palette.bg
        };
        rendered_color(&host, "rs-app", "background-color", expected);
        assert_eq!(host.state().surface, original);
        host.update(|state| assert_eq!(state.surface.drain_commands().count(), 0));
        let reopened = launch(&selection, &library);
        rendered_color(&reopened, "rs-app", "background-color", expected);
        assert_eq!(
            reopened
                .state()
                .appearance
                .applied()
                .unwrap()
                .resolved
                .theme_mode,
            Some(mode)
        );
    }
    click(
        &mut host,
        Selector::role("button").with_attr("data-appearance-mode", "custom:garden"),
    );
    rendered_color(
        &host,
        "rs-app",
        "background-color",
        tinct::Srgb::rgb(0x20, 0x30, 0x20),
    );
    let reopened = launch(&selection, &library);
    rendered_color(
        &reopened,
        "rs-app",
        "background-color",
        tinct::Srgb::rgb(0x20, 0x30, 0x20),
    );
    click(
        &mut host,
        Selector::role("button").with_attr("data-appearance-theme", appearance::PRODUCT_DEFAULT),
    );
    rendered_color(
        &host,
        "rs-app",
        "background-color",
        redshank_surfaces::theme::palette(original.seed, original.mode).bg,
    );
    assert_eq!(host.state().surface, original);
}

#[test]
fn corrupt_stores_and_failed_choice_writes_preserve_owned_bytes_and_presentation() {
    let dir = tempfile::tempdir().unwrap();
    let selection = dir.path().join("appearance.json");
    let library = dir.path().join("themes.json");
    std::fs::write(&selection, b"broken choice").unwrap();
    std::fs::write(&library, b"broken library").unwrap();
    let mut state = AppearanceState::load(selection.clone(), library.clone(), Vec::new());
    assert!(!state.authoring_available);
    assert!(
        state
            .diagnostics()
            .iter()
            .any(|text| text.contains("preserved"))
    );
    assert!(state.select(appearance::PRODUCT_DEFAULT, None).is_err());
    assert_eq!(std::fs::read(&selection).unwrap(), b"broken choice");
    assert_eq!(std::fs::read(&library).unwrap(), b"broken library");
    let mut host = launch(&selection, &library);
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "toggle-appearance"),
    );
    click(
        &mut host,
        Selector::role("button").with_attr("data-appearance-theme", appearance::PRODUCT_DEFAULT),
    );
    let messages = host.state().appearance.diagnostics().join(" ");
    assert!(messages.contains("preferences could not be read"));
    assert!(messages.contains("theme library could not be opened"));
    assert!(host.with_dom(|dom| {
        taproot::matching(
            dom,
            &Selector::role("button").with_attr("data-action", "edit-appearance"),
        )
        .is_empty()
    }));
    assert_eq!(std::fs::read(&selection).unwrap(), b"broken choice");
    assert_eq!(std::fs::read(&library).unwrap(), b"broken library");
    std::fs::remove_file(&selection).unwrap();
    std::fs::remove_file(&library).unwrap();
    let theme = saved_theme(&library, false);
    let mut state = AppearanceState::load(selection.clone(), library, Vec::new());
    state.select(&theme.id, Some(Mode::Light)).unwrap();
    let prior = state.applied().unwrap().clone();
    std::fs::remove_file(&selection).unwrap();
    std::fs::create_dir(&selection).unwrap();
    assert!(state.select(&theme.id, Some(Mode::Dark)).is_err());
    assert_eq!(state.applied().unwrap().resolved, prior.resolved);
    assert!(selection.is_dir());
}

#[test]
fn saving_same_identity_keeps_applied_snapshot_until_explicit_apply() {
    let dir = tempfile::tempdir().unwrap();
    let selection = dir.path().join("appearance.json");
    let library = dir.path().join("themes.json");
    let theme = saved_theme(&library, false);
    let mut state = AppearanceState::load(selection.clone(), library.clone(), Vec::new());
    state.select(&theme.id, Some(Mode::Light)).unwrap();
    let original = state.applied().unwrap().theme.clone();
    state.begin_edit(&RedshankSurfaceState::default()).unwrap();
    *state.workshop.text_field_mut("seed-hex").unwrap() = cambium::TextInput::new("#e42b65");
    state.workshop.sync_controls();
    // The shared editor's hex command validates and stages the seed edit.
    state.workshop.apply_hex();
    state.workshop.save();
    assert_ne!(
        state
            .workshop
            .registry()
            .theme_def(&theme.id)
            .unwrap()
            .seeds,
        original.seeds
    );
    assert_eq!(state.applied().unwrap().theme.seeds, original.seeds);
    state.apply_workshop().unwrap();
    assert_eq!(
        state.applied().unwrap().theme.seeds,
        original.seeds,
        "view requests do not publish before I/O"
    );
    state.commit_requested();
    assert_ne!(state.applied().unwrap().theme.seeds, original.seeds);
    let reopened = AppearanceState::load(selection, library, Vec::new());
    assert_eq!(
        reopened.applied().unwrap().theme.seeds,
        state.applied().unwrap().theme.seeds
    );
}

#[test]
fn product_copy_preserves_endorsed_wetland_and_dirty_close_can_be_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = DesktopState {
        surface: Default::default(),
        appearance: AppearanceState::load(
            dir.path().join("appearance.json"),
            dir.path().join("themes.json"),
            Vec::new(),
        ),
    };
    state.appearance.begin_edit(&state.surface).unwrap();
    let presentation = state
        .appearance
        .workshop
        .draft_theme()
        .presentation_for_mode(&Mode::Dark)
        .unwrap();
    let ThemePresentation::AuthoredStylesheet(css) = presentation else {
        panic!("endorsed dark override remains authored");
    };
    assert!(css.join("\n").to_ascii_lowercase().contains("#101716"));
    assert!(css.join("\n").to_ascii_lowercase().contains("#edf2ef"));
    assert!(!appearance_host::prepare_close(&mut state));
    state.appearance.workshop.cancel_close();
    let exit =
        tabard_workshop::native_host::sync_and_export(&mut state.appearance.workshop, &mut |_| {
            None
        });
    assert!(!appearance_view::sync_editor(&mut state, exit));
    assert!(state.appearance.editor_open);
    assert!(!state.appearance.close_app);
    state.appearance.workshop.discard_and_close();
    let exit =
        tabard_workshop::native_host::sync_and_export(&mut state.appearance.workshop, &mut |_| {
            None
        });
    assert!(!appearance_view::sync_editor(&mut state, exit));
    assert!(!state.appearance.editor_open);
    assert_eq!(state.surface, RedshankSurfaceState::default());
}

#[test]
fn export_guards_preserve_choice_future_generations_and_open_media() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("listener");
    let selection = dir.path().join("appearance.json");
    let library = dir.path().join("themes.json");
    let media = dir.path().join("episode.flac");
    std::fs::write(&media, b"owned media").unwrap();
    let mut state = AppearanceState::load(selection.clone(), library, Vec::new());
    state
        .workshop
        .set_protected_export_directories(vec![data.clone()]);
    state.protect_media(vec![media.clone()]);
    for target in [
        selection.clone(),
        data.join("state-00000000000000000002.json"),
        media.clone(),
    ] {
        state.workshop.request_export();
        let artifact = state.workshop.take_export().unwrap();
        state.workshop.complete_export(artifact, Some(target));
        assert!(state.workshop.status().contains("protected"));
        assert!(state.workshop.replacement_path().is_none());
    }
    assert!(!selection.exists());
    assert!(!data.exists());
    assert_eq!(std::fs::read(media).unwrap(), b"owned media");
}

#[test]
fn native_appearance_scenarios_use_the_existing_shared_lane() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scenarios");
    let fixture = ThemeLibraryStore::load(root.join("fixtures/tabard_role_library.json")).unwrap();
    let theme = fixture
        .themes()
        .iter()
        .find(|theme| theme.id == "user:role-fixture")
        .unwrap();
    for mode in [Mode::Dark, Mode::Custom("garden".into())] {
        assert!(matches!(
            theme.presentation_for_mode(&mode).unwrap(),
            ThemePresentation::AuthoredStylesheet(_)
        ));
    }
    for name in [
        "tabard_authoring.scn",
        "tabard_reopen.scn",
        "tabard_authored_roles.scn",
        "tabard_authored_reopen.scn",
    ] {
        taproot::Scenario::parse(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap();
    }
}

#[test]
fn editor_native_text_route_does_not_edit_listener_note_or_queue_commands() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = launch(
        &dir.path().join("appearance.json"),
        &dir.path().join("themes.json"),
    );
    let original = host.state().surface.clone();
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "toggle-appearance"),
    );
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "edit-appearance"),
    );
    assert!(host.click_on(&Selector::role("textbox").with_attr("data-field", "name")));
    host.set_modifiers(cambium_genet_winit_host::Modifiers {
        ctrl: true,
        ..Default::default()
    });
    host.key_char("a");
    host.set_modifiers(Default::default());
    host.key_injected("Listening-night");
    host.after_dispatch();
    assert_eq!(
        host.state().appearance.workshop.draft_theme().name,
        "Listening-night"
    );
    assert_eq!(host.state().surface, original);
    host.update(|state| assert_eq!(state.surface.drain_commands().count(), 0));
}

#[test]
fn native_listener_feed_route_survives_application_chrome() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = launch(
        &dir.path().join("appearance.json"),
        &dir.path().join("themes.json"),
    );
    host.update(|state| state.surface.active_tab = redshank_surfaces::SurfaceTab::Library);
    host.relayout();
    for _ in 0..40 {
        if feed_field_focused(host.runner()) {
            break;
        }
        host.tab(true);
    }
    assert!(
        feed_field_focused(host.runner()),
        "the original subscribe field remains reachable"
    );
    host.key_injected("https://example.invalid/feed.xml");
    host.after_dispatch();
    assert_eq!(
        host.state().surface.feed_url_editor.text(),
        "https://example.invalid/feed.xml"
    );
    assert_eq!(host.state().surface.text_editor.text(), "");
    host.update(|state| assert_eq!(state.surface.drain_commands().count(), 0));
}

#[test]
fn narrow_editor_keeps_parent_actions_visible_and_saves_through_shared_controls() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = launch(
        &dir.path().join("appearance.json"),
        &dir.path().join("themes.json"),
    );
    host.layout_at(420.0, 900.0);
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "toggle-appearance"),
    );
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "edit-appearance"),
    );
    for action in ["back-to-redshank", "apply-to-redshank"] {
        let selector = Selector::role("button").with_attr("data-action", action);
        let node = host.with_dom(|dom| taproot::matching(dom, &selector)[0]);
        let (x, y, width, height) = host.painted_rect(node).unwrap();
        assert!(width > 0.0 && height > 0.0);
        assert!(x >= 0.0 && x + width <= 420.5, "{action}: {x}+{width}");
        assert!(y >= 0.0 && y + height <= 900.5, "{action}: {y}+{height}");
    }
    click(&mut host, Selector::role("button").containing("Save theme"));
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "apply-to-redshank"),
    );
    assert_eq!(host.state().appearance.active_id(), "theme:copy-1");
    click(
        &mut host,
        Selector::role("button").with_attr("data-action", "back-to-redshank"),
    );
    assert!(!host.state().appearance.editor_open);
    assert_eq!(host.state().surface, RedshankSurfaceState::default());
    host.update(|state| assert_eq!(state.surface.drain_commands().count(), 0));
}
