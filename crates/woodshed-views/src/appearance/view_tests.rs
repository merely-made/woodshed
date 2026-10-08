//! Authoring and appearance selection through mounted, focusable controls.

use super::*;
use crate::stage::{UiChild, UiState};
use cambium_genet_winit_host::{Harness, HostHooks, Init, KeyPress, NamedKey};
use taproot::Selector;

type Host = Harness<UiState, fn(&UiState) -> UiChild, UiChild>;

fn root(ui: &UiState) -> UiChild {
    if ui.appearance.workshop_open {
        workshop_screen(ui)
    } else {
        appearance_page(ui)
    }
}

fn mount(ui: UiState) -> Host {
    let mut hooks: HostHooks<UiState, fn(&UiState) -> UiChild, UiChild> = HostHooks::inert();
    hooks.after_dispatch = Box::new(|ctx| {
        *ctx.set_sheet = Some(appearance_stylesheet(ctx.runner.state()));
    });
    let mut host = Harness::with_hooks(
        Init {
            sheet: appearance_stylesheet(&ui),
            state: ui,
            logic: root as fn(&UiState) -> UiChild,
            fonts: vec![],
            images: vec![],
        },
        hooks,
    );
    host.layout_at(1280.0, 960.0);
    host
}

fn action(name: &str) -> Selector {
    Selector::role("button").with_attr("data-action", name)
}
fn theme(id: &str) -> Selector {
    Selector::role("button").with_attr("data-appearance-theme", id)
}
fn mode(key: &str) -> Selector {
    Selector::role("button").with_attr("data-appearance-mode", key)
}

#[track_caller]
fn click(host: &mut Host, selector: Selector) {
    assert!(
        host.click_on(&selector),
        "Control must be mounted and painted: {selector:?}"
    );
}

#[test]
fn appearance_settings_offer_named_themes_modes_and_keyboard_focus_without_authoring() {
    let mut host = mount(UiState::new());
    assert!(host.with_dom(|dom| taproot::matching(dom, &action("edit-appearance")).is_empty()));
    for builtin in ThemeMode::ALL {
        click(&mut host, theme(builtin.theme_id()));
        assert_eq!(host.state().app_settings.appearance.theme, builtin.label());
        assert_eq!(
            host.state()
                .appearance
                .resolve(&host.state().app_settings.appearance)
                .theme
                .id,
            builtin.theme_id()
        );
    }
    for profile in CANONICAL_MODES {
        click(&mut host, mode(&profile.as_key()));
        assert_eq!(
            host.state()
                .appearance
                .resolve(&host.state().app_settings.appearance)
                .mode,
            profile
        );
    }
    host.press_key(&KeyPress::named(NamedKey::Tab));
    let focused = host
        .focus()
        .expect("Appearance buttons participate in keyboard focus");
    assert!(
        host.with_dom(|dom| taproot::matching(dom, &Selector::role("button")).contains(&focused))
    );
}

#[test]
fn authoring_copy_preview_close_cancel_save_and_apply_follow_real_controls() {
    let mut ui = UiState::new();
    ui.appearance_authoring_available = true;
    let mut host = mount(ui);
    click(&mut host, theme("woodshed:ember"));
    let prior = host.state().app_settings.appearance.clone();
    click(&mut host, action("edit-appearance"));
    assert!(host.state().appearance.workshop_open);
    let copy_id = host.state().appearance.workshop.draft_theme().id.clone();
    assert_ne!(copy_id, "woodshed:ember");
    click(
        &mut host,
        Selector::role("button").with_attr("data-mode", "hc_light"),
    );
    assert_eq!(host.state().app_settings.appearance, prior);
    click(&mut host, action("apply-to-woodshed"));
    assert_eq!(host.state().app_settings.appearance, prior);
    assert!(
        host.state()
            .appearance_notice
            .as_ref()
            .unwrap()
            .contains("Save")
    );
    click(&mut host, action("back-to-woodshed"));
    assert!(host.state().appearance.workshop.close_requested());
    assert!(host.state().appearance.workshop_open);
    click(&mut host, action("cancel-close"));
    assert!(!host.state().appearance.workshop.close_requested());
    assert!(host.state().appearance.workshop_open);
    click(&mut host, action("save"));
    click(&mut host, action("apply-to-woodshed"));
    let applied = host
        .state()
        .app_settings
        .appearance
        .theme_choice
        .as_ref()
        .unwrap();
    assert_eq!(applied.theme_id, copy_id);
    assert_eq!(applied.theme_mode, Some(Mode::HcLight));
    assert_eq!(host.state().app_settings.appearance.theme, "Ember");
    click(&mut host, action("back-to-woodshed"));
    assert!(!host.state().appearance.workshop_open);
    assert!(host.with_dom(|dom| !taproot::matching(dom, &theme(&copy_id)).is_empty()));
    click(&mut host, theme("woodshed:slate"));
    click(&mut host, theme(&copy_id));
    assert_eq!(
        host.state()
            .appearance
            .resolve(&host.state().app_settings.appearance)
            .theme
            .id,
        copy_id
    );
}

#[test]
fn workshop_sheet_replaces_authored_app_css_and_returns_when_editor_closes() {
    let mut ui = UiState::new();
    ui.appearance_authoring_available = true;
    let mut authored = Theme::new("theme:sheet", "Sheet", ThemeMode::Slate.seeds());
    authored.mode_sheets.insert(
        "dark".into(),
        vec![".theme-editor { background: #f00000; }".into()],
    );
    ui.appearance
        .workshop
        .import_theme_json(&tabard::portable::theme_json(&authored).unwrap());
    ui.appearance.workshop.save();
    ui.appearance
        .select(
            &mut ui.app_settings.appearance,
            &authored.id,
            Some(Mode::Dark),
        )
        .unwrap();
    assert!(appearance_stylesheet(&ui).contains(".theme-editor { background: #f00000; }"));
    let mut host = mount(ui);
    click(&mut host, action("edit-appearance"));
    assert!(
        !appearance_stylesheet(host.state()).contains(".theme-editor { background: #f00000; }")
    );
    click(&mut host, action("back-to-woodshed"));
    assert!(appearance_stylesheet(host.state()).contains(".theme-editor { background: #f00000; }"));
}
