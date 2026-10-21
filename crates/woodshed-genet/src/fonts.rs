//! Unmodified shared consumer faces, registered through native metadata.

use cambium_genet_winit_host::HostFont;

pub fn bundled_fonts() -> Vec<HostFont> {
    [
        include_bytes!("../../../assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/ibm-plex-mono/IBMPlexMono-Bold.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/ibm-plex-mono/IBMPlexMono-Italic.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/ibm-plex-mono/IBMPlexMono-BoldItalic.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/source-serif-4/SourceSerif4.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/source-serif-4/SourceSerif4-Italic.ttf").as_slice(),
    ]
    .into_iter()
    .map(|bytes| HostFont {
        family: None,
        bytes: bytes.to_vec(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium::{el, text};
    use cambium_genet_winit_host::{Harness, HostHooks, Init};
    use tabard::typography::{FontFamily, FontRole};
    use tabard::{Theme, ThemeFontStyle};
    use taproot::Selector;
    use woodshed_views::appearance::{appearance_stylesheet, commit_selection};
    use woodshed_views::stage::{UiChild, UiState};

    type Host = Harness<UiState, crate::sync::Logic, UiChild>;
    const PROBE: &str = ".font-probe { position:absolute;display:block;top:0;left:0;white-space:nowrap;font-weight:400;margin:0; } .font-probe.recipe-desc, .font-probe.fret-label, .font-probe.graph-canvas-swatch-label { width:auto; }";

    #[test]
    #[ignore = "requires an owned sealed font profile and live isolated Keeper"]
    fn sealed_profile_font_snapshot() {
        let state = std::path::PathBuf::from(
            std::env::var_os("WOODSHED_STATE").expect("owned state path required"),
        );
        assert!(state.is_file(), "existing owned profile required");
        let output = std::path::PathBuf::from(
            std::env::var_os("WOODSHED_FONT_SNAPSHOT").expect("owned snapshot output required"),
        );
        let (storage, _) = crate::storage::open_store_as(None);
        let mut ui = UiState::new();
        crate::session::restore(&storage, &mut ui);
        assert_eq!(
            ui.set.cards.len(),
            2,
            "synthetic two-Card qualification profile"
        );
        assert_eq!(
            ui.loop_captures.takes.len(),
            1,
            "retained raw take qualification profile"
        );
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .unwrap();
        serde_json::to_writer_pretty(
            &mut file,
            &serde_json::json!({
                "domain":ui.to_persisted(),
                "fonts":ui.appearance.choice(&ui.app_settings.appearance).fonts,
                "appearance":ui.app_settings.appearance,
                "metronome":ui.app_settings.metronome,
                "fretboard":ui.app_settings.fretboard
            }),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "writes only an explicitly owned isolated theme library"]
    fn prepare_font_theme_fixture() {
        let path = std::path::PathBuf::from(
            std::env::var_os("WOODSHED_THEME_LIBRARY").expect("owned library path required"),
        );
        assert!(!path.exists(), "fixture library must be new");
        let mut theme = Theme::new(
            "theme:woodshed-font-acceptance",
            "Woodshed Font Acceptance",
            woodshed_views::theme::ThemeMode::Slate.seeds(),
        );
        for (role, family, size, leading) in [
            (FontRole::Interface, "sans-serif", 14.0, 1.2),
            (FontRole::Reading, "Source Serif 4", 14.0, 1.4),
            (FontRole::Heading, "Source Serif 4", 16.0, 1.2),
            (FontRole::Label, "sans-serif", 11.0, 1.0),
        ] {
            theme
                .typography
                .set(
                    role,
                    ThemeFontStyle {
                        family: Some(family.into()),
                        size_px: Some(size),
                        line_height: Some(leading),
                    },
                )
                .unwrap();
        }
        tabard::library::ThemeLibraryStore::load(path)
            .unwrap()
            .save(&[theme])
            .unwrap();
    }

    fn width(host: &Host, class: &str) -> f32 {
        let node = host.with_dom(|dom| taproot::matching_classes(dom, &Selector::class(class))[0]);
        host.painted_rect(node).expect("painted glyph probe").2
    }

    #[test]
    fn fonts_shape_remeasure_and_keep_authored_precedence_with_fixed_overlay_geometry() {
        let mut ui = UiState::new();
        ui.appearance
            .set_available_fonts(cambium_genet_winit_host::available_font_families(
                &bundled_fonts(),
            ));
        assert!(
            ui.appearance
                .available_fonts
                .iter()
                .any(|name| name == "IBM Plex Mono")
        );
        let mut theme = Theme::new(
            "theme:woodshed-font-measure",
            "Font measure",
            woodshed_views::theme::ThemeMode::Slate.seeds(),
        );
        theme
            .typography
            .set(
                FontRole::Reading,
                ThemeFontStyle {
                    family: Some("Source Serif 4".into()),
                    size_px: Some(20.0),
                    line_height: Some(1.4),
                },
            )
            .unwrap();
        theme
            .typography
            .set(
                FontRole::Label,
                ThemeFontStyle {
                    family: Some("Source Serif 4".into()),
                    size_px: Some(24.0),
                    line_height: Some(1.6),
                },
            )
            .unwrap();
        ui.appearance
            .workshop
            .import_theme_json(&tabard::portable::theme_json(&theme).unwrap());
        ui.appearance.workshop.save();
        ui.appearance
            .select(&mut ui.app_settings.appearance, &theme.id, None)
            .unwrap();
        let mut choice = ui.appearance.choice(&ui.app_settings.appearance);
        choice.fonts.0.insert(
            FontRole::Reading,
            vec![FontFamily::new("IBM Plex Mono").unwrap()],
        );
        choice.fonts.0.insert(
            FontRole::Label,
            vec![FontFamily::new("IBM Plex Mono").unwrap()],
        );
        ui.app_settings.appearance.theme_choice = Some(choice);
        let domain = serde_json::to_value(ui.to_persisted()).unwrap();
        let mut hooks: HostHooks<UiState, crate::sync::Logic, UiChild> = HostHooks::inert();
        hooks.after_dispatch = Box::new(|ctx| {
            ctx.runner.update(|ui| {
                commit_selection(ui, |_| Ok(()));
            });
            *ctx.set_sheet = Some(format!(
                "{}\n{PROBE}",
                appearance_stylesheet(ctx.runner.state())
            ));
        });
        let mut host = Host::with_hooks(
            Init {
                sheet: format!("{}\n{PROBE}", appearance_stylesheet(&ui)),
                state: ui,
                logic: Box::new(|_| {
                    Box::new(el(
                        "div",
                        (
                            el("p", text("00000000"))
                                .attr("class", "font-probe recipe-desc mono-probe"),
                            el("span", text("00000000"))
                                .attr("class", "font-probe fret-label fret-probe"),
                            el("span", text("00000000"))
                                .attr("class", "font-probe graph-canvas-swatch-label graph-probe"),
                        ),
                    )) as UiChild
                }),
                fonts: bundled_fonts(),
                images: vec![],
            },
            hooks,
        );
        host.layout_at(1000.0, 700.0);
        assert!(
            (width(&host, "mono-probe") - 96.0).abs() < 1.0,
            "application family follows authored20px size and Plex600/1000em metrics"
        );
        let fret_before = width(&host, "fret-probe");
        assert!(
            (width(&host, "graph-probe") - 48.0).abs() < 1.0,
            "graph overlay retains10px shaping with chosen label family"
        );
        assert!(
            fret_before < 96.0,
            "positioned marker ignores authored label24px size"
        );
        host.update(|ui| {
            let mut choice = ui.appearance.choice(&ui.app_settings.appearance);
            choice.fonts.0.remove(&FontRole::Reading);
            ui.app_settings.appearance.theme_choice = Some(choice);
        });
        host.after_dispatch();
        host.relayout();
        assert!(
            (width(&host, "mono-probe") - 96.0).abs() > 1.0,
            "family change invalidates actual shaping/layout"
        );
        assert!((width(&host, "fret-probe") - fret_before).abs() < 0.1);
        host.layout_at(420.0, 800.0);
        assert_eq!(
            serde_json::to_value(host.state().to_persisted()).unwrap(),
            domain
        );
        let mut authored = theme;
        authored.id = "theme:woodshed-font-measure-authored".into();
        authored.mode_sheets.insert(
            "dark".into(),
            vec![".recipe-desc { font-family:'Source Serif 4' !important; }".into()],
        );
        host.update(|ui| {
            ui.appearance
                .workshop
                .import_theme_json(&tabard::portable::theme_json(&authored).unwrap());
            ui.appearance.workshop.save();
            let mut choice = ui.appearance.choice(&ui.app_settings.appearance);
            choice.fonts.0.insert(
                FontRole::Reading,
                vec![FontFamily::new("IBM Plex Mono").unwrap()],
            );
            ui.app_settings.appearance.theme_choice = Some(choice);
            ui.appearance
                .select(
                    &mut ui.app_settings.appearance,
                    &authored.id,
                    Some(tabard::theme::registry::Mode::Dark),
                )
                .unwrap();
            ui.appearance.reset_active();
        });
        host.after_dispatch();
        host.relayout();
        assert!(
            (width(&host, "mono-probe") - 96.0).abs() > 1.0,
            "authored family retains ordinary important precedence"
        );
    }

    #[test]
    fn appearance_buttons_use_requested_interface_family_and_authored_size() {
        let mut ui = UiState::new();
        ui.appearance
            .set_available_fonts(cambium_genet_winit_host::available_font_families(
                &bundled_fonts(),
            ));
        let mut theme = Theme::new(
            "theme:font-controls",
            "Font controls",
            woodshed_views::theme::ThemeMode::Slate.seeds(),
        );
        theme
            .typography
            .set(
                FontRole::Interface,
                ThemeFontStyle {
                    family: Some("Source Serif 4".into()),
                    size_px: Some(20.0),
                    line_height: Some(1.2),
                },
            )
            .unwrap();
        ui.appearance
            .workshop
            .import_theme_json(&tabard::portable::theme_json(&theme).unwrap());
        ui.appearance.workshop.save();
        ui.appearance
            .select(&mut ui.app_settings.appearance, &theme.id, None)
            .unwrap();
        let mut choice = ui.appearance.choice(&ui.app_settings.appearance);
        choice.fonts.0.insert(
            FontRole::Interface,
            vec![FontFamily::new("IBM Plex Mono").unwrap()],
        );
        ui.app_settings.appearance.theme_choice = Some(choice);
        let mut host = Host::with_hooks(
            Init {
                sheet: appearance_stylesheet(&ui),
                state: ui,
                logic: Box::new(woodshed_views::appearance::appearance_page),
                fonts: bundled_fonts(),
                images: vec![],
            },
            HostHooks::inert(),
        );
        host.layout_at(1000.0, 800.0);
        let projection = host.a11y_projection();
        let node = host.with_dom(|dom| {
            taproot::matching_with_projection(
                dom,
                &Selector::role("button").with_attr("data-appearance-theme", "woodshed:slate"),
                &projection,
            )[0]
        });
        let bounds = host.painted_rect(node).unwrap();
        assert!(
            (bounds.2 - 86.0).abs() < 1.0,
            "Slate: five20px PlexMono glyphs+24px padding+2px border; got {}",
            bounds.2
        );
        assert!(
            host.click_on(
                &Selector::role("button").with_attr("data-appearance-theme", "woodshed:slate")
            ),
            "remeasured control remains a live hit target"
        );
    }
}
