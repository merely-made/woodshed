use cambium::{PointerClick, button, el, lens, text};
use tabard_workshop::{WorkshopState, workshop_stylesheet, workshop_view};

use crate::stage::{UiChild, UiState};
use crate::theme::{apply_accessibility, text_scale_factor};

const APPEARANCE_CSS: &str = "
.appearance-options, .appearance-modes { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0; }
.appearance-options button, .appearance-modes button { font-family: sans-serif; font-size: 13px; padding: 9px 12px; border: 1px solid currentColor; border-radius: 6px; cursor: pointer; }
.appearance-options button[aria-pressed=\"false\"], .appearance-modes button[aria-pressed=\"false\"] { background: transparent; }
.appearance-options button:focus-visible, .appearance-modes button:focus-visible, .appearance-edit:focus-visible { outline: 2px solid currentColor; outline-offset: 3px; }
.appearance-notice { margin: 10px 0; font-size: 13px; line-height: 1.5; }
";

// Workshop rules replace the product sheet while the authoring screen is
// mounted. Only the host's outer frame is retained: product selectors and
// authored mode CSS cannot reach the neutral editor chrome.
const EMBEDDING_CSS: &str = "
.desktop-frame { display: flex; flex-direction: column; height: 100%; min-height: 0; overflow: hidden; }
.desktop-frame > .cambium-title-bar, .desktop-frame > .chrome { flex-shrink: 0; background: #ffffff; color: #253247; border-bottom: 1px solid #d4dae3; }
.woodshed-workshop { display: flex; flex: 1; flex-direction: column; min-height: 0; }
.woodshed-workshop > .tabard-workshop { flex: 1; min-height: 0; }
.woodshed-workshop-controls { display: flex; align-items: center; gap: 12px; padding: 10px 18px; background: #ffffff; border-bottom: 1px solid #dde1e7; }
.woodshed-workshop-controls p { font-size: 12px; color: #53637a; }
.woodshed-workshop > .appearance-notice { padding: 8px 18px; margin: 0; background: #fff4d8; color: #493d25; }
";

pub fn appearance_stylesheet(ui: &UiState) -> String {
    let css = if ui.appearance.workshop_open {
        format!("{}\n{}", workshop_stylesheet(), EMBEDDING_CSS)
    } else {
        format!(
            "{}\n{}",
            ui.appearance
                .resolve(&ui.app_settings.appearance)
                .stylesheet,
            APPEARANCE_CSS
        )
    };
    apply_accessibility(
        css,
        ui.app_settings.accessibility.reduce_motion,
        text_scale_factor(&ui.app_settings.accessibility.text_scale),
    )
}

pub fn appearance_page(ui: &UiState) -> UiChild {
    let resolved = ui.appearance.resolve(&ui.app_settings.appearance);
    let options: Vec<UiChild> = ui
        .appearance
        .options()
        .into_iter()
        .map(|option| {
            let active = option.id == resolved.theme.id;
            let id = option.id.clone();
            Box::new(
                button(option.name, move |ui: &mut UiState, _: PointerClick| {
                    ui.appearance_notice = ui
                        .appearance
                        .request_selection(&ui.app_settings.appearance, &id, None)
                        .err();
                })
                .attr("data-appearance-theme", option.id)
                .attr("aria-pressed", active.to_string())
                .attr(
                    "class",
                    if active {
                        "side-item side-active"
                    } else {
                        "side-item"
                    },
                ),
            ) as UiChild
        })
        .collect();
    let modes: Vec<UiChild> = ui
        .appearance
        .modes(&resolved.theme.id)
        .into_iter()
        .map(|mode| {
            let active = mode == resolved.mode;
            let key = mode.as_key();
            Box::new(
                button(mode.label(), move |ui: &mut UiState, _: PointerClick| {
                    let choice = ui.appearance.resolve(&ui.app_settings.appearance);
                    ui.appearance_notice = ui
                        .appearance
                        .request_selection(
                            &ui.app_settings.appearance,
                            &choice.theme.id,
                            Some(mode.clone()),
                        )
                        .err();
                })
                .attr("data-appearance-mode", key)
                .attr("aria-pressed", active.to_string())
                .attr(
                    "class",
                    if active {
                        "side-item side-active"
                    } else {
                        "side-item"
                    },
                ),
            ) as UiChild
        })
        .collect();
    let mut content: Vec<UiChild> = vec![
        Box::new(el("h2", text("Appearance")).attr("class", "settings-heading")),
        Box::new(el("p", text("Theme")).attr("class", "settings-line")),
        Box::new(
            el("div", options)
                .attr("class", "appearance-options")
                .attr("aria-label", "Theme"),
        ),
        Box::new(el("p", text("Presentation mode")).attr("class", "settings-line")),
        Box::new(
            el("div", modes)
                .attr("class", "appearance-modes")
                .attr("aria-label", "Presentation mode"),
        ),
    ];
    for notice in [
        resolved.fallback_reason.as_ref(),
        ui.appearance_notice.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        content.push(Box::new(
            el("p", text(notice.clone()))
                .attr("class", "appearance-notice")
                .attr("role", "status"),
        ));
    }
    if ui.appearance_authoring_available {
        content.push(Box::new(
            button("Edit appearance…", |ui: &mut UiState, _: PointerClick| {
                ui.appearance_notice = ui.appearance.begin_edit(&ui.app_settings.appearance).err();
            })
            .attr("data-action", "edit-appearance")
            .attr("class", "t-btn appearance-edit"),
        ));
        content.push(Box::new(
            el(
                "p",
                text("Edit a user copy in Tabard, save it, then choose Apply to Woodshed."),
            )
            .attr("class", "settings-line"),
        ));
    }
    Box::new(
        el("section", content)
            .attr("class", "board settings-page")
            .attr("aria-label", "Appearance settings"),
    )
}

pub fn workshop_screen(ui: &UiState) -> UiChild {
    let mut content: Vec<UiChild> = vec![Box::new(el("div", (
        button("Back to Woodshed", |ui: &mut UiState, _: PointerClick| {
            ui.appearance_notice = None;
            ui.appearance_close_app = false;
            if ui.appearance.workshop.request_close() {
                ui.appearance.workshop.cancel_close();
                ui.appearance.workshop_open = false;
            }
        }).attr("data-action", "back-to-woodshed"),
        button("Apply to Woodshed", |ui: &mut UiState, _: PointerClick| {
            ui.appearance_notice = ui.appearance
                .request_workshop_selection(&ui.app_settings.appearance).err();
        }).attr("data-action", "apply-to-woodshed").attr("class", "primary-button"),
        el("p", text("Previewing and editing here leaves your Woodshed appearance selected until you apply.")),
    )).attr("class", "woodshed-workshop-controls"))];
    if let Some(notice) = &ui.appearance_notice {
        content.push(Box::new(
            el("p", text(notice.clone()))
                .attr("class", "appearance-notice")
                .attr("role", "status"),
        ));
    }
    content.push(Box::new(lens(
        |state: &mut WorkshopState| workshop_view(state),
        |ui: &mut UiState| &mut ui.appearance.workshop,
    )));
    Box::new(
        el("div", content)
            .attr("class", "woodshed-workshop")
            .attr("data-surface", "woodshed.appearance-workshop.v1"),
    )
}
