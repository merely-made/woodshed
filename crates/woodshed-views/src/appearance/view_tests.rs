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
    mount_with_persistence(ui, true)
}

fn mount_with_persistence(ui: UiState, accept: bool) -> Host {
    let mut hooks: HostHooks<UiState, fn(&UiState) -> UiChild, UiChild> = HostHooks::inert();
    hooks.after_dispatch = Box::new(move |ctx| {
        ctx.runner.update(|ui| {
            commit_selection(ui, |_| {
                if accept {
                    Ok(())
                } else {
                    Err("Settings receipt rejected the write".into())
                }
            });
        });
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

#[test]
fn rejected_preference_write_keeps_the_applied_theme_and_practice_state() {
    let ui = UiState::new();
    let settings = ui.app_settings.clone();
    let practice = serde_json::to_value(ui.to_persisted()).unwrap();
    let sheet = appearance_stylesheet(&ui);
    let mut host = mount_with_persistence(ui, false);
    click(&mut host, theme("woodshed:ember"));
    assert_eq!(host.state().app_settings, settings);
    assert_eq!(
        serde_json::to_value(host.state().to_persisted()).unwrap(),
        practice
    );
    assert_eq!(appearance_stylesheet(host.state()), sheet);
    assert!(
        host.state()
            .appearance_notice
            .as_ref()
            .unwrap()
            .contains("rejected")
    );
    host.update(|ui| {
        assert!(!commit_selection(ui, |_| panic!(
            "Rejected requests must be drained"
        )));
    });
}

#[test]
fn saving_an_applied_identity_retains_active_css_until_explicit_apply() {
    let mut ui = UiState::new();
    ui.appearance_authoring_available = true;
    let mut authored = Theme::new(
        "theme:active-snapshot",
        "Snapshot",
        ThemeMode::Slate.seeds(),
    );
    let original = ":root { --tabard-color-bg: #112233; }";
    let revised = ":root { --tabard-color-bg: #445566; }";
    authored
        .mode_sheets
        .insert("dark".into(), vec![original.into()]);
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
    let initial = appearance_stylesheet(&ui);
    let choice = ui.app_settings.appearance.clone();
    let mut host = mount(ui);
    click(&mut host, action("edit-appearance"));
    host.update(|ui| {
        *ui.appearance.workshop.text_field_mut("mode-sheet").unwrap() =
            cambium::TextInput::new(revised);
        ui.appearance.workshop.sync_controls();
    });
    click(&mut host, action("save"));
    assert_eq!(host.state().app_settings.appearance, choice);
    assert!(
        host.state()
            .appearance
            .resolve(&choice)
            .stylesheet
            .contains(original)
    );
    assert!(
        !host
            .state()
            .appearance
            .resolve(&choice)
            .stylesheet
            .contains(revised)
    );
    click(&mut host, action("back-to-woodshed"));
    assert_eq!(appearance_stylesheet(host.state()), initial);
    click(&mut host, action("edit-appearance"));
    assert_eq!(
        host.state()
            .appearance
            .workshop
            .draft_theme()
            .mode_sheet(&Mode::Dark)
            .unwrap(),
        &[revised.to_string()]
    );
    click(&mut host, action("apply-to-woodshed"));
    click(&mut host, action("back-to-woodshed"));
    assert_eq!(host.state().app_settings.appearance, choice);
    assert!(appearance_stylesheet(host.state()).contains(revised));
    assert!(!appearance_stylesheet(host.state()).contains(original));
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

#[test]
fn authored_shared_roles_recolor_the_product_surface_and_reset_on_builtin_selection() {
    let mut ui = UiState::new();
    let mut authored = Theme::new("theme:role-sheet", "Role sheet", ThemeMode::Slate.seeds());
    authored.mode_sheets.insert(
        "dark".into(),
        vec![":root { --tabard-color-surface: #123456; --tabard-color-text: #f1f2f3; }".into()],
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
    let mut host = mount(ui);
    let product = host.with_dom(|dom| taproot::matching(dom, &Selector::class("board"))[0]);
    let background = host.computed_value(product, "background-color").unwrap();
    let background = background.to_ascii_lowercase().replace(' ', "");
    assert!(matches!(
        background.as_str(),
        "#123456" | "rgb(18,52,86)" | "rgba(18,52,86,1)"
    ));
    assert!(
        host.state()
            .appearance
            .resolve(&host.state().app_settings.appearance)
            .fallback_reason
            .unwrap()
            .contains("Typed instrument and graph paint")
    );
    click(&mut host, theme("woodshed:slate"));
    host.relayout();
    let product = host.with_dom(|dom| taproot::matching(dom, &Selector::class("board"))[0]);
    let restored = host.computed_value(product, "background-color").unwrap();
    assert_ne!(restored.to_ascii_lowercase().replace(' ', ""), background);
    assert_eq!(
        appearance_stylesheet(host.state()),
        appearance_stylesheet(&UiState::new())
    );
}
