use cambium::{clickable, el, map_state, select, text};
use woodshed_core::audio::{AudioRequest, CalibrationStatus};
use woodshed_core::midi::{PedalAction, PedalMessageKind};
use woodshedding::rehearsal::SetGraphEdgeKind;

use super::{BoardLayout, SettingsPage, UiChild, UiState};

#[derive(Clone, Copy)]
enum PedalField {
    Channel,
    Number,
    Value,
    Release,
}

fn page_nav(ui: &UiState) -> UiChild {
    let items: Vec<UiChild> = SettingsPage::ALL
        .iter()
        .map(|&(page, label)| {
            let class = if page == ui.app_settings.page {
                "side-item side-active"
            } else {
                "side-item"
            };
            Box::new(clickable(
                el("div", text(label)).attr("class", class),
                move |ui: &mut UiState, _| ui.app_settings.page = page,
            )) as UiChild
        })
        .collect();
    Box::new(el("nav", items).attr("class", "side settings-nav settings-page-nav"))
}

/// The persona line: who is practising, and what protects the session.
///
/// Three real states, none of them guessed. A declined gate is checked first
/// because it outranks the seal: the store never opened, so a seal left on the
/// state would be from before the decline.
fn persona_line(ui: &UiState) -> String {
    if let Some(notice) = &ui.practice_save_notice {
        return notice.clone();
    }
    if !ui.practice_saved {
        return "No persona chosen. Nothing from this session is saved.".to_string();
    }
    match &ui.seal {
        Some(seal) => seal.summary(),
        // Only before a store opens, which the gate covers. Said rather than
        // left blank, so a state nobody predicted does not read as an answer.
        None => "The practice store has not reported a persona yet.".to_string(),
    }
}

fn general_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("General")).attr("class", "settings-heading"),
                el(
                    "div",
                    text(format!("Woodshed {} · desktop alpha", env!("CARGO_PKG_VERSION"))),
                )
                .attr("class", "settings-line"),
                el("div", text(ui.audio_output_status())).attr("class", "settings-line"),
                el("div", text(ui.audio_input_status())).attr("class", "settings-line"),
                ui.audio_error.as_ref().map(|reason| el("div", text(format!("Audio operation: {reason}"))).attr("class", "settings-line")),
                el(
                    "div",
                    text("Selections, Set, practice history, tempo, theme, and layout restore on launch."),
                )
                .attr("class", "settings-line"),
                el("div", text("Persona")).attr("class", "settings-heading settings-gap"),
                // Who is practising, as the store reported it opening. A page
                // offering to switch personas has to be able to name the one in
                // force; before this it described sealing in general and left the
                // actual answer only in the boot log.
                el("div", text(persona_line(ui))).attr("class", "settings-line"),
                el(
                    "div",
                    text("Switching swaps to that persona's own Set, history, and settings."),
                )
                .attr("class", "settings-line"),
                clickable(
                    el("div", text("Switch persona…")).attr("class", "t-btn"),
                    // The host answers this: reading the roster means opening
                    // the vault, which a view does not do.
                    |ui: &mut UiState, _| ui.persona_switch_requested = true,
                ),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn appearance_page(ui: &UiState) -> UiChild {
    crate::appearance::appearance_page(ui)
}

fn instrument_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("Instrument")).attr("class", "settings-heading"),
                el(
                    "div",
                    text(format!(
                        "Fretted instrument · {} strings",
                        ui.stage.string_count()
                    )),
                )
                .attr("class", "settings-line"),
                el(
                    "div",
                    text("Instrument-family selection will live here; tuning already drives every material projection."),
                )
                .attr("class", "settings-line"),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn tuning_page(ui: &UiState) -> UiChild {
    let names: Vec<&str> = woodshed_core::tunings().iter().map(|t| t.name).collect();
    let picker = map_state(select(&ui.tuning_dd, &names), |ui: &mut UiState| {
        &mut ui.tuning_dd
    });
    Box::new(
        el(
            "div",
            (
                el("div", text("Tuning")).attr("class", "settings-heading"),
                el("div", picker).attr("class", "settings-options"),
                el(
                    "div",
                    text(
                        "This is the same tuning used by Stage, Fretboard, Rehearsal, and Looper.",
                    ),
                )
                .attr("class", "settings-line"),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn fretboard_page(ui: &UiState) -> UiChild {
    let layouts: Vec<UiChild> = BoardLayout::ALL
        .iter()
        .map(|&layout| {
            let class = if layout == ui.board_layout() {
                "side-item side-active"
            } else {
                "side-item"
            };
            Box::new(clickable(
                el("div", text(layout.label())).attr("class", class),
                move |ui: &mut UiState, _| ui.set_board_layout(layout),
            )) as UiChild
        })
        .collect();
    let markers: Vec<UiChild> = ["Sharp", "Rounded", "Circle", "Diamond"]
        .iter()
        .map(|&name| {
            let active = ui.app_settings.fretboard.marker_style.as_str() == name;
            let class = if active {
                "side-item side-active"
            } else {
                "side-item"
            };
            Box::new(clickable(
                el("div", text(name)).attr("class", class),
                move |ui: &mut UiState, _| {
                    ui.app_settings.fretboard.marker_style = name.to_string();
                },
            )) as UiChild
        })
        .collect();
    // Neck window: an adjustable fret range, not a preset list. From and To each
    // step by one to any value; Full lets the end auto-track the instrument's
    // whole neck. A wide range scrolls (the board fits its pane), so any range is
    // reachable.
    fn stepper(label: &'static str, delta: i32, start: bool) -> UiChild {
        Box::new(clickable(
            el("div", text(label)).attr("class", "neck-step"),
            move |ui: &mut UiState, _| {
                if start {
                    ui.nudge_neck_start(delta)
                } else {
                    ui.nudge_neck_end(delta)
                }
            },
        )) as UiChild
    }
    let full_class = if ui.neck_is_full() {
        "neck-full side-active"
    } else {
        "neck-full"
    };
    let neck_control: UiChild = Box::new(
        el(
            "div",
            (
                el("div", text("From")).attr("class", "neck-label"),
                stepper("\u{2212}", -1, true),
                el("div", text(ui.neck_from().to_string())).attr("class", "neck-value"),
                stepper("+", 1, true),
                el("div", text("To")).attr("class", "neck-label neck-label-gap"),
                stepper("\u{2212}", -1, false),
                el("div", text(ui.neck_to().to_string())).attr("class", "neck-value"),
                stepper("+", 1, false),
                clickable(
                    el("div", text("Full")).attr("class", full_class),
                    move |ui: &mut UiState, _| ui.set_neck_full(),
                ),
            ),
        )
        .attr("class", "neck-control"),
    );
    // Orientation: lay the neck out horizontally (frets left-to-right) or stand
    // it up vertically (nut at the top, low E on the left) like a chord diagram.
    let orient_chips: Vec<UiChild> = ["Horizontal", "Vertical"]
        .iter()
        .map(|&name| {
            let active = ui.app_settings.fretboard.orientation.as_str() == name;
            let class = if active {
                "side-item side-active"
            } else {
                "side-item"
            };
            Box::new(clickable(
                el("div", text(name)).attr("class", class),
                move |ui: &mut UiState, _| {
                    ui.app_settings.fretboard.orientation = name.to_string();
                },
            )) as UiChild
        })
        .collect();
    Box::new(
        el(
            "div",
            (
                el("div", text("Layout")).attr("class", "settings-heading"),
                el("div", layouts).attr("class", "settings-options"),
                el("div", text("Markers")).attr("class", "settings-heading settings-gap"),
                el("div", markers).attr("class", "settings-options"),
                el("div", text("Orientation")).attr("class", "settings-heading settings-gap"),
                el("div", orient_chips).attr("class", "settings-options"),
                el("div", text("Neck (frets)")).attr("class", "settings-heading settings-gap"),
                neck_control,
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn metronome_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("Metronome")).attr("class", "settings-heading"),
                el(
                    "div",
                    (
                        clickable(
                            el("div", text("-5")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.nudge_bpm(-5.0),
                        ),
                        el("div", text(format!("{:.0} bpm", ui.transport.bpm)))
                            .attr("class", "t-readout"),
                        clickable(
                            el("div", text("+5")).attr("class", "t-btn"),
                            |ui: &mut UiState, _| ui.nudge_bpm(5.0),
                        ),
                    ),
                )
                .attr("class", "transport"),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn tuner_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("Tuner")).attr("class", "settings-heading"),
                el("div", text(ui.tuner_status())).attr("class", "settings-line"),
                clickable(
                    el("div", text(ui.tuner_action_label())).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.tuner.enabled = !ui.tuner.enabled;
                        if !ui.tuner.enabled {
                            ui.tuner.reading = None;
                        }
                    },
                ),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn rehearsal_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("Rehearsal")).attr("class", "settings-heading"),
                el(
                    "div",
                    text(format!("Current Set: {} cards", ui.set.cards.len())),
                )
                .attr("class", "settings-line"),
                el(
                    "div",
                    text("Card timing and touch remain Card settings; runner defaults will live here."),
                )
                .attr("class", "settings-line"),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn looper_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("Looper")).attr("class", "settings-heading"),
                clickable(
                    el(
                        "div",
                        text(if ui.song_record_replace {
                            "Record: replace"
                        } else {
                            "Record: overdub"
                        }),
                    )
                    .attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.song_record_replace = !ui.song_record_replace;
                    },
                ),
                el(
                    "div",
                    text(
                        "Count-in, unresolved-Card timing, and WAV export defaults will live here.",
                    ),
                )
                .attr("class", "settings-line"),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn audio_midi_page(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                el("div", text("Audio and MIDI")).attr("class", "settings-heading"),
                midi_panel(ui),
                calibration_panel(ui),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

fn accessibility_page(ui: &UiState) -> UiChild {
    // Two-way toggle rendered as a pair of chips, so the active choice is always
    // visible rather than inferred from a checkbox state.
    fn toggle(on_now: bool, set_on: fn(&mut UiState, bool)) -> Vec<UiChild> {
        [("Off", false), ("On", true)]
            .into_iter()
            .map(|(label, val)| {
                let active = on_now == val;
                let class = if active {
                    "side-item side-active"
                } else {
                    "side-item"
                };
                Box::new(clickable(
                    el("div", text(label)).attr("class", class),
                    move |ui: &mut UiState, _| set_on(ui, val),
                )) as UiChild
            })
            .collect()
    }
    let motion = toggle(ui.app_settings.accessibility.reduce_motion, |ui, v| {
        ui.app_settings.accessibility.reduce_motion = v;
    });
    let root = toggle(ui.app_settings.accessibility.distinguish_root, |ui, v| {
        ui.app_settings.accessibility.distinguish_root = v;
    });
    let sizes: Vec<UiChild> = ["Normal", "Large", "Larger"]
        .iter()
        .map(|&name| {
            let active = ui.app_settings.accessibility.text_scale.as_str() == name;
            let class = if active {
                "side-item side-active"
            } else {
                "side-item"
            };
            Box::new(clickable(
                el("div", text(name)).attr("class", class),
                move |ui: &mut UiState, _| {
                    ui.app_settings.accessibility.text_scale = name.to_string();
                },
            )) as UiChild
        })
        .collect();
    Box::new(
        el(
            "div",
            (
                el("div", text("Accessibility")).attr("class", "settings-heading"),
                el("div", text("Reduce motion")).attr("class", "settings-heading settings-gap"),
                el("div", motion).attr("class", "settings-options"),
                el("div", text("Distinguish root by outline"))
                    .attr("class", "settings-heading settings-gap"),
                el("div", root).attr("class", "settings-options"),
                el("div", text("Text size")).attr("class", "settings-heading settings-gap"),
                el("div", sizes).attr("class", "settings-options"),
                el(
                    "div",
                    text("Keyboard focus is visible, and the fretboard is announced to screen readers with each marker's note, interval, string, and fret."),
                )
                .attr("class", "settings-line settings-gap"),
            ),
        )
        .attr("class", "board settings-page"),
    )
}

pub(super) fn screen(ui: &UiState) -> UiChild {
    let page = match ui.app_settings.page {
        SettingsPage::General => general_page(ui),
        SettingsPage::Appearance => appearance_page(ui),
        SettingsPage::Instrument => instrument_page(ui),
        SettingsPage::Tuning => tuning_page(ui),
        SettingsPage::Stage => stage_page(ui),
        SettingsPage::Fretboard => fretboard_page(ui),
        SettingsPage::Metronome => metronome_page(ui),
        SettingsPage::Tuner => tuner_page(ui),
        SettingsPage::Rehearsal => rehearsal_page(ui),
        SettingsPage::Looper => looper_page(ui),
        SettingsPage::AudioMidi => audio_midi_page(ui),
        SettingsPage::Accessibility => accessibility_page(ui),
    };
    Box::new(el("div", (page_nav(ui), page)).attr("class", "body settings-shell"))
}

#[cfg(test)]
#[path = "settings/tests.rs"]
mod tests;

#[path = "settings/midi.rs"]
mod midi;

use midi::{calibration_panel, midi_panel};

#[path = "settings/stage_page.rs"]
mod stage_page;

use stage_page::stage_page;
