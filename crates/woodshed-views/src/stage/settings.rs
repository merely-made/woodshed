use cambium::{clickable, el, map_state, select, text};
use woodshed_core::audio::{AudioRequest, CalibrationStatus};
use woodshed_core::midi::{PedalAction, PedalMessageKind};
use woodshedding::rehearsal::SetGraphEdgeKind;

use super::{BoardLayout, SettingsPage, UiChild, UiState};

/// MIDI device panel (audio-depth slice 13): port pickers, clock-slave /
/// clock-master toggles, and a live status + event readout. The host
/// realizes the selections through the `MidiBackend` seam.
fn midi_panel(ui: &UiState) -> UiChild {
    let in_opts: Vec<&str> = std::iter::once("None")
        .chain(ui.midi.input_ports.iter().map(|s| s.as_str()))
        .collect();
    let out_opts: Vec<&str> = std::iter::once("None")
        .chain(ui.midi.output_ports.iter().map(|s| s.as_str()))
        .collect();
    let input_dd = map_state(select(&ui.midi.input_dd, &in_opts), |ui: &mut UiState| {
        &mut ui.midi.input_dd
    });
    let output_dd = map_state(select(&ui.midi.output_dd, &out_opts), |ui: &mut UiState| {
        &mut ui.midi.output_dd
    });
    let slave_label = if ui.midi.clock_slave {
        "Sync to clock: on"
    } else {
        "Sync to clock: off"
    };
    let out_label = if ui.midi.clock_out {
        "Send clock: on"
    } else {
        "Send clock: off"
    };
    let clock = ui
        .midi
        .clock_bpm
        .map(|b| format!("clock {b:.1} bpm"))
        .unwrap_or_else(|| "no clock".to_string());
    let status = format!(
        "in: {} · out: {} · {clock}{}",
        ui.midi.connected_in.as_deref().unwrap_or("—"),
        ui.midi.connected_out.as_deref().unwrap_or("—"),
        ui.midi
            .output_notice
            .as_ref()
            .map_or(String::new(), |notice| format!(" · {notice}")),
    );
    let events_line = if ui.midi.events.is_empty() {
        "(no events)".to_string()
    } else {
        ui.midi.events.join("    ")
    };
    Box::new(el(
        "div",
        (
            el("div", text("MIDI")).attr("class", "settings-heading settings-gap"),
            el(
                "div",
                (
                    el("span", text("In")).attr("class", "header-label"),
                    input_dd,
                    el("span", ()).attr("class", "header-gap"),
                    el("span", text("Out")).attr("class", "header-label"),
                    output_dd,
                ),
            )
            .attr("class", "header-row")
            // Open menus overlap the following clock row; painting and hit
            // testing must keep these options above its controls.
            .attr("style", "position: relative; z-index: 20;"),
            el(
                "div",
                (
                    clickable(
                        el("div", text(slave_label)).attr("class", "t-btn"),
                        |ui: &mut UiState, _| ui.midi.clock_slave = !ui.midi.clock_slave,
                    ),
                    clickable(
                        el("div", text(out_label)).attr("class", "t-btn"),
                        |ui: &mut UiState, _| ui.midi.clock_out = !ui.midi.clock_out,
                    ),
                    clickable(
                        el("div", text("Refresh ports")).attr("class", "t-btn"),
                        |ui: &mut UiState, _| ui.midi.refresh_requested = true,
                    ),
                ),
            )
            .attr("class", "transport"),
            el("div", text(status)).attr("class", "settings-line"),
            el("div", text(ui.midi.error.clone().unwrap_or_default()))
                .attr("class", "settings-line")
                .attr("role", "status"),
            el("div", text(events_line)).attr("class", "settings-line midi-events"),
            pedal_panel(ui),
        ),
    ))
}

/// Uses the existing buttons/steppers and durable settings, with no I/O in views.
fn pedal_panel(ui: &UiState) -> UiChild {
    let rows: Vec<UiChild> = PedalAction::ALL
        .into_iter()
        .map(|action| {
            let binding = ui.app_settings.audio_midi.midi_pedal.get(action);
            let enabled = binding.enabled;
            let can_trigger = ui.midi.output_ready && binding.messages().is_ok();
            let mut controls: Vec<UiChild> = vec![
                Box::new(el("span", text(action.label())).attr("class", "header-label")),
                Box::new(clickable(
                    el("div", text(if enabled { "Enabled" } else { "Disabled" }))
                        .attr("class", "t-btn")
                        .attr("aria-label", format!("{} binding enabled", action.label())),
                    move |ui: &mut UiState, _| {
                        ui.midi.disarm_inputs();
                        let binding = ui.app_settings.audio_midi.midi_pedal.get_mut(action);
                        binding.enabled = !binding.enabled;
                    },
                )),
                Box::new(clickable(
                    el("div", text(binding.kind.label()))
                        .attr("class", "t-btn")
                        .attr("aria-label", format!("{} message type", action.label())),
                    move |ui: &mut UiState, _| {
                        ui.midi.disarm_inputs();
                        let binding = ui.app_settings.audio_midi.midi_pedal.get_mut(action);
                        binding.kind = binding.kind.next();
                        if !binding.kind.supports_release() {
                            binding.release = None;
                        }
                    },
                )),
                pedal_number(ui, action, PedalField::Channel),
                pedal_number(ui, action, PedalField::Number),
            ];
            if binding.kind != PedalMessageKind::ProgramChange {
                controls.push(pedal_number(ui, action, PedalField::Value));
            }
            if binding.kind.supports_release() {
                controls.push(Box::new(clickable(
                    el(
                        "div",
                        text(if binding.release.is_some() {
                            "Release: on (50 ms)"
                        } else {
                            "Release: off"
                        }),
                    )
                    .attr("class", "t-btn")
                    .attr("aria-label", format!("{} release", action.label())),
                    move |ui: &mut UiState, _| {
                        ui.midi.disarm_inputs();
                        let binding = ui.app_settings.audio_midi.midi_pedal.get_mut(action);
                        binding.release = if binding.release.is_some() {
                            None
                        } else {
                            Some(0)
                        };
                    },
                )));
                if binding.release.is_some() {
                    controls.push(pedal_number(ui, action, PedalField::Release));
                }
            }
            let mut trigger = el("button", text(format!("Trigger {}", action.label())))
                .attr("class", "t-btn t-hear")
                .attr("type", "button")
                .attr("data-midi-pedal-trigger", "true");
            if !can_trigger {
                trigger = trigger.attr("disabled", "").attr("aria-disabled", "true");
            }
            controls.push(Box::new(clickable(trigger, move |ui: &mut UiState, _| {
                if ui.midi.output_ready
                    && ui
                        .app_settings
                        .audio_midi
                        .midi_pedal
                        .get(action)
                        .messages()
                        .is_ok()
                    && ui.midi.pedal_request.is_none()
                {
                    ui.midi.pedal_request = Some(action);
                    ui.midi.pedal_request_target = None;
                }
            })));
            Box::new(
                el("div", controls)
                    .attr("class", "transport")
                    .attr("style", "flex-wrap: wrap;"),
            ) as UiChild
        })
        .collect();
    let report = ui
        .midi
        .pedal_report
        .as_ref()
        .map(|report| report.summary())
        .unwrap_or_else(|| "No pedal action sent".into());
    Box::new(el("div", (
        el("div", text("Software pedal")).attr("class", "settings-heading settings-gap"),
        el("div", text("Match each binding to your instrument's MIDI menu, then enable it. Channel 1–16; number/value 0–127. Configuration changes send nothing."))
            .attr("class", "settings-line"),
        el("div", rows),
        pedal_input_panel(ui),
        midi_diagnostics_panel(ui),
        el("div", text(report)).attr("class", "settings-line")
            .attr("aria-live", "polite"),
    )))
}

fn midi_diagnostics_panel(ui: &UiState) -> UiChild {
    let status = &ui.midi.diagnostics;
    let mut toggle_button = el(
        "button",
        text(if status.recording {
            "Stop MIDI diagnostics"
        } else {
            "Record MIDI diagnostics"
        }),
    )
    .attr("class", "t-btn")
    .attr("aria-pressed", status.recording.to_string());
    if !status.available {
        toggle_button = toggle_button
            .attr("disabled", "")
            .attr("aria-disabled", "true");
    }
    let toggle = clickable(toggle_button, |ui: &mut UiState, _| {
        if ui.midi.diagnostics.available {
            ui.midi.diagnostics_record_request = Some(!ui.midi.diagnostics.recording);
        }
    });
    let mut export_button = el(
        "button",
        text(if ui.midi.diagnostics_export_busy {
            "Exporting diagnostics…"
        } else {
            "Export MIDI diagnostics…"
        }),
    )
    .attr("class", "t-btn");
    if !status.history_available || ui.midi.diagnostics_export_busy {
        export_button = export_button
            .attr("disabled", "")
            .attr("aria-disabled", "true");
    }
    let export = clickable(export_button, |ui: &mut UiState, _| {
        if ui.midi.diagnostics.history_available && !ui.midi.diagnostics_export_busy {
            ui.midi.diagnostics_export_requested = true;
        }
    });
    let show = clickable(
        el(
            "button",
            text(if ui.midi.diagnostics_show_history {
                "Hide latest events"
            } else {
                "Show latest events"
            }),
        )
        .attr("class", "t-btn")
        .attr(
            "aria-expanded",
            ui.midi.diagnostics_show_history.to_string(),
        ),
        |ui: &mut UiState, _| {
            ui.midi.diagnostics_show_history = !ui.midi.diagnostics_show_history;
        },
    );
    let latest: Vec<UiChild> = if ui.midi.diagnostics_show_history {
        status
            .recent_events
            .iter()
            .map(|entry| {
                Box::new(el("div", text(entry.clone())).attr("class", "settings-line")) as UiChild
            })
            .collect()
    } else {
        Vec::new()
    };
    Box::new(el("div", (
        el("div",text("MIDI diagnostic history")).attr("class","settings-heading settings-gap"),
        el("div",text("Recording is session-only and starts off by default. Stop retains the captured history; export saves a snapshot of the history and current MIDI status. Send results do not confirm instrument actions."))
            .attr("class","settings-line"),
        el("div", (toggle, export, show)).attr("class","transport").attr("style","flex-wrap: wrap;"),
        el("div",text(if !status.available { "MIDI diagnostics are unavailable in this host.".into() } else {format!("Recording: {} · retained: {} / {} · overwritten: {} · truncated: {}",if status.recording { "on" } else { "off" },status.retained_events,status.capacity,status.overwritten,status.truncated_events)}))
            .attr("class","settings-line").attr("aria-live","polite"),
        el("div",text(ui.midi.diagnostics_notice.clone().unwrap_or_default())).attr("class","settings-line").attr("aria-live","polite"),
        el("div",latest),
        el("div",text(if ui.midi.diagnostics_show_history { "Showing up to 12 latest events. Export includes the full retained history." } else { "" })).attr("class","settings-line"),
    )))
}

fn pedal_input_panel(ui: &UiState) -> UiChild {
    let rows: Vec<UiChild> = PedalAction::ALL.into_iter().map(|action| {
        let mapping = ui.app_settings.audio_midi.midi_pedal.inputs.get(action);
        let key = mapping.keyboard.as_ref().map(|key| key.label())
            .unwrap_or_else(|| "Unassigned".into());
        let controller = mapping.controller.map(|binding| binding.label())
            .unwrap_or_else(|| "Unassigned".into());
        Box::new(el("div", (
            el("span", text(format!("{} · Key: {key} · MIDI: {controller}", action.label())))
                .attr("class", "header-label"),
            clickable(
                el("button", text("Learn key")).attr("class", "t-btn")
                    .attr("aria-label", format!("{} learn keyboard", action.label())),
                move |ui: &mut UiState, _| {
                    ui.midi.disarm_inputs();
                    ui.midi.learn_key = Some(action);
                    ui.midi.input_notice = Some("Press a letter/digit or navigation key; Shift/Alt optional. Escape cancels. Ctrl/Cmd shortcuts remain with the app.".into());
                },
            ),
            clickable(
                el("button", text("Clear key")).attr("class", "t-btn")
                    .attr("aria-label", format!("{} clear keyboard", action.label())),
                move |ui: &mut UiState, _| {
                    ui.midi.disarm_inputs();
                    ui.app_settings.audio_midi.midi_pedal.inputs.get_mut(action).keyboard = None;
                    ui.midi.input_notice = Some("Keyboard mapping cleared; inputs disarmed".into());
                },
            ),
            clickable(
                el("button", text("Learn MIDI")).attr("class", "t-btn")
                    .attr("aria-label", format!("{} learn MIDI", action.label())),
                move |ui: &mut UiState, _| {
                    ui.midi.disarm_inputs();
                    if ui.midi.connected_in.is_none() {
                        ui.midi.input_notice = Some("Select a MIDI input first".into());
                        return;
                    }
                    ui.midi.learn_controller = Some(action);
                    ui.midi.learn_controller_flush = true;
                    ui.midi.input_notice = Some("Press a note or momentary CC control (64–127). Learning sends nothing; release it afterward.".into());
                },
            ),
            clickable(
                el("button", text("Clear MIDI")).attr("class", "t-btn")
                    .attr("aria-label", format!("{} clear MIDI", action.label())),
                move |ui: &mut UiState, _| {
                    ui.midi.disarm_inputs();
                    ui.app_settings.audio_midi.midi_pedal.inputs.get_mut(action).controller = None;
                    ui.midi.input_notice = Some("Controller mapping cleared; inputs disarmed".into());
                },
            ),
        )).attr("class", "transport").attr("style", "flex-wrap: wrap;")) as UiChild
    }).collect();
    let arm = clickable(
        el(
            "button",
            text(if ui.midi.inputs_armed {
                "Disarm pedal inputs"
            } else {
                "Arm pedal inputs"
            }),
        )
        .attr("class", "t-btn"),
        |ui: &mut UiState, _| {
            if ui.midi.inputs_armed {
                ui.midi.disarm_inputs();
                ui.midi.input_notice = Some("Pedal inputs disarmed".into());
                return;
            }
            let bindings = &ui.app_settings.audio_midi.midi_pedal;
            if bindings.inputs.conflict() {
                ui.midi.input_notice =
                    Some("Duplicate input mappings; assign distinct controls".into());
                return;
            }
            if ui.midi.pedal_target.is_none() {
                ui.midi.input_notice =
                    Some("Select a ready MIDI output before arming inputs".into());
                return;
            }
            if !PedalAction::ALL.into_iter().any(|action| {
                let mapping = bindings.inputs.get(action);
                (mapping.keyboard.as_ref().is_some_and(|key| key.valid())
                    || mapping.controller.is_some_and(|binding| binding.valid()))
                    && bindings.get(action).messages().is_ok()
            }) {
                ui.midi.input_notice =
                    Some("Configure an input mapping and enable its output binding first".into());
                return;
            }
            ui.midi.learn_key = None;
            ui.midi.learn_controller = None;
            ui.midi.inputs_armed = true;
            ui.midi.input_notice = Some(
                "Pedal inputs armed. Release a learned controller once before its first trigger."
                    .into(),
            );
        },
    );
    Box::new(el("div", (
        el("div", text("Keyboard and controller inputs"))
            .attr("class", "settings-heading settings-gap"),
        el("div", text("Mappings are saved; arming is live only. Notes use Note Off/velocity 0 to release. CC uses 0–63 to release and 64–127 to press. Held controls do not repeat."))
            .attr("class", "settings-line"),
        el("div", rows),
        el("div", (
            arm,
            clickable(
                el("button", text("Cancel learning")).attr("class", "t-btn"),
                |ui: &mut UiState, _| {
                    ui.midi.disarm_inputs();
                    ui.midi.input_notice = Some("Learning cancelled; inputs disarmed".into());
                },
            ),
        )).attr("class", "transport"),
        el("div", text(ui.midi.input_notice.clone()
            .unwrap_or_else(|| "Pedal inputs disarmed".into())))
            .attr("class", "settings-line").attr("aria-live", "polite"),
    )))
}

#[derive(Clone, Copy)]
enum PedalField {
    Channel,
    Number,
    Value,
    Release,
}
fn pedal_number(ui: &UiState, action: PedalAction, field: PedalField) -> UiChild {
    let binding = ui.app_settings.audio_midi.midi_pedal.get(action);
    let (label, value, min, max) = match field {
        PedalField::Channel => ("Channel", binding.channel, 1, 16),
        PedalField::Number => ("Number", binding.number, 0, 127),
        PedalField::Value => ("Value", binding.value, 0, 127),
        PedalField::Release => ("Release value", binding.release.unwrap_or(0), 0, 127),
    };
    let step = |delta: i16| -> UiChild {
        Box::new(clickable(
            el("div", text(if delta < 0 { "−" } else { "+" }))
                .attr("class", "neck-step")
                .attr(
                    "aria-label",
                    format!(
                        "{} {} {}",
                        action.label(),
                        label,
                        if delta < 0 { "decrease" } else { "increase" }
                    ),
                ),
            move |ui: &mut UiState, _| {
                ui.midi.disarm_inputs();
                let binding = ui.app_settings.audio_midi.midi_pedal.get_mut(action);
                let value = match field {
                    PedalField::Channel => &mut binding.channel,
                    PedalField::Number => &mut binding.number,
                    PedalField::Value => &mut binding.value,
                    PedalField::Release => binding.release.get_or_insert(0),
                };
                *value = (i16::from(*value) + delta).clamp(min, max) as u8;
            },
        ))
    };
    Box::new(
        el(
            "div",
            (
                el("span", text(label)).attr("class", "neck-label"),
                step(-1),
                el("span", text(value.to_string())).attr("class", "neck-value"),
                step(1),
            ),
        )
        .attr("class", "neck-control"),
    )
}

/// Latency-calibration panel (audio-depth slice 14): measure the
/// input→output round-trip lag by playing clicks the player taps along
/// to. The host drives the `CalibrationSession` through the audio seam.
fn calibration_panel(ui: &UiState) -> UiChild {
    let latency_line = match ui.latency_ms {
        Some(ms) => format!("Latency: {ms:.0} ms round-trip"),
        None => "Latency: uncalibrated".to_string(),
    };
    let start_btn = |label: &str| {
        Box::new(clickable(
            el("div", text(label.to_string())).attr("class", "t-btn"),
            |ui: &mut UiState, _| ui.request(AudioRequest::CalibrationStart),
        )) as UiChild
    };
    let readout = |s: String| Box::new(el("div", text(s)).attr("class", "t-readout")) as UiChild;
    let controls: Vec<UiChild> = match ui.calib_status {
        CalibrationStatus::Idle => vec![Box::new(clickable(
            el("div", text("Calibrate")).attr("class", "t-btn t-hear"),
            |ui: &mut UiState, _| ui.request(AudioRequest::CalibrationStart),
        )) as UiChild],
        CalibrationStatus::Running {
            clicks_fired,
            total,
        } => vec![
            readout(format!("Tap along… {clicks_fired}/{total}")),
            Box::new(clickable(
                el("div", text("Cancel")).attr("class", "t-btn"),
                |ui: &mut UiState, _| ui.request(AudioRequest::CalibrationCancel),
            )) as UiChild,
        ],
        CalibrationStatus::Success {
            latency_ms,
            matched,
            total,
        } => vec![
            readout(format!("{latency_ms:.0} ms · {matched}/{total} hits")),
            Box::new(clickable(
                el("div", text("Accept")).attr("class", "t-btn t-hear"),
                |ui: &mut UiState, _| ui.request(AudioRequest::CalibrationAccept),
            )) as UiChild,
            start_btn("Retry"),
        ],
        CalibrationStatus::Insufficient { matched, total } => vec![
            readout(format!("only {matched}/{total} hits caught")),
            start_btn("Retry"),
        ],
        CalibrationStatus::Unavailable => vec![readout("no input device".to_string())],
    };
    Box::new(el(
        "div",
        (
            el("div", text("Latency calibration")).attr("class", "settings-heading settings-gap"),
            el("div", text(latency_line)).attr("class", "settings-line"),
            el("div", controls).attr("class", "transport"),
            el(
                "div",
                text(
                    "Plays a lead of clicks — tap your guitar with each one \
                     so Woodshed can measure the round-trip lag.",
                ),
            )
            .attr("class", "settings-line midi-events"),
        ),
    ))
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

fn stage_page(ui: &UiState) -> UiChild {
    let history_label = if ui.app_settings.stage.related.use_history {
        "History ranking: on"
    } else {
        "History ranking: off"
    };
    let graph_label = if ui.app_settings.stage.related.show_neighborhood {
        "Neighborhood graph: on"
    } else {
        "Neighborhood graph: off"
    };
    let scope_label = format!(
        "Graph scope: {}",
        ui.app_settings.stage.related.graph_scope.label()
    );
    let depth_label = format!(
        "Selection depth: {}",
        ui.app_settings.stage.related.relation_depth
    );
    let swatch_arrangement = format!(
        "Mere arrangement: {}",
        ui.app_settings.stage.related.arrangement.label()
    );
    let set_arrangement = format!(
        "Set arrangement: {}",
        ui.app_settings.stage.set_arrangement.label()
    );
    let reading = format!(
        "Set reading: {}",
        ui.app_settings.stage.set_graph_reading.label()
    );
    let context = if ui.app_settings.stage.context.enabled {
        format!(
            "Circle context: on · {} nodes",
            ui.app_settings.stage.context.node_limit()
        )
    } else {
        "Circle context: off".to_string()
    };
    let (set_graph_width, set_graph_height) = ui.app_settings.stage.set_graph_size();
    let set_graph_size = format!("Set canvas: {set_graph_width} × {set_graph_height} · reset");
    let hidden_count = ui.app_settings.stage.related.dismissed_ids.len();
    // One entry per derivable relation family, so the Set graph's visible
    // relations are configured here rather than through a single switch that
    // stops describing the projection as families are added.
    let relation_items: Vec<UiChild> = SetGraphEdgeKind::ALL
        .into_iter()
        .map(|kind| {
            let state = if ui.app_settings.stage.shows_relation(kind) {
                "on"
            } else {
                "off"
            };
            Box::new(clickable(
                el(
                    "div",
                    text(format!(
                        "Set {} edges: {state}",
                        kind.label().to_lowercase()
                    )),
                )
                .attr("class", "side-item"),
                move |ui: &mut UiState, _| {
                    ui.app_settings.stage.toggle_relation(kind);
                    ui.set_graph_relation = None;
                },
            )) as UiChild
        })
        .collect();
    Box::new(
        el(
            "div",
            (
                el("div", text("Stage")).attr("class", "settings-heading"),
                clickable(
                    el("div", text(history_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.use_history = !related.use_history;
                    },
                ),
                clickable(
                    el("div", text(graph_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.show_neighborhood = !related.show_neighborhood;
                    },
                ),
                clickable(
                    el("div", text(scope_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.graph_scope = related.graph_scope.toggle();
                        ui.related_relation = None;
                    },
                ),
                clickable(
                    el("div", text(depth_label)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let depth = &mut ui.app_settings.stage.related.relation_depth;
                        *depth = if *depth >= 6 { 1 } else { *depth + 1 };
                        ui.related_relation = None;
                    },
                ),
                clickable(
                    el("div", text(swatch_arrangement)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let related = &mut ui.app_settings.stage.related;
                        related.arrangement = related.arrangement.next();
                    },
                ),
                clickable(
                    el("div", text(set_arrangement)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        ui.set_graph_arrangement(ui.app_settings.stage.set_arrangement.next());
                    },
                ),
                clickable(
                    el("div", text(reading)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let reading = ui.app_settings.stage.set_graph_reading.next();
                        ui.set_graph_reading(reading);
                    },
                ),
                clickable(
                    el("div", text(context)).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        ui.app_settings.stage.context.enabled =
                            !ui.app_settings.stage.context.enabled;
                    },
                ),
                clickable(
                    el("div", text("Context breadth: next")).attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        let limit = ui.app_settings.stage.context.node_limit();
                        ui.app_settings.stage.context.node_limit =
                            if limit >= 36 { 12 } else { limit + 12 };
                    },
                ),
                clickable(
                    el("div", text(set_graph_size)).attr("class", "side-item"),
                    |ui: &mut UiState, _| ui.app_settings.stage.reset_set_graph_size(),
                ),
                clickable(
                    el("div", text(format!("Restore hidden ({hidden_count})")))
                        .attr("class", "side-item"),
                    |ui: &mut UiState, _| {
                        ui.app_settings.stage.related.dismissed_ids.clear();
                    },
                ),
                el("div", relation_items),
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
mod tests {
    use super::*;
    use crate::persona::PracticeSeal;

    #[test]
    fn the_persona_line_names_who_is_practising_and_what_holds_the_key() {
        // The gap this closes: Settings offered to switch a persona it could
        // not name.
        let mut ui = UiState::new();
        ui.seal = Some(PracticeSeal::Sealed {
            persona: "work".into(),
            protection: "OS auto-unlock sealed records".into(),
        });
        let line = persona_line(&ui);
        assert!(line.contains("work"), "{line}");
        assert!(
            line.contains("OS auto-unlock"),
            "the backend's own words: {line}"
        );
    }

    #[test]
    fn a_pending_session_says_so_and_says_why() {
        let mut ui = UiState::new();
        ui.seal = Some(PracticeSeal::Pending {
            reason: "djinn is not running".into(),
        });
        let line = persona_line(&ui);
        assert!(line.contains("Identity pending"), "{line}");
        assert!(line.contains("not saved"), "never in the clear: {line}");
        assert!(
            line.contains("djinn is not running"),
            "a reason, because 'pending' alone is not actionable: {line}"
        );
    }

    #[test]
    fn a_declined_gate_outranks_any_seal_left_on_the_state() {
        // Declining opens no store, so a seal from before the decline would be
        // a stale claim that this session is being kept.
        let mut ui = UiState::new();
        ui.seal = Some(PracticeSeal::Sealed {
            persona: "work".into(),
            protection: "DPAPI".into(),
        });
        crate::persona::practise_unsaved(&mut ui);
        let line = persona_line(&ui);
        assert!(line.contains("No persona chosen"), "{line}");
        assert!(
            !line.contains("work"),
            "the stale persona must not survive: {line}"
        );
    }

    #[test]
    fn a_store_that_has_not_reported_yet_says_that_rather_than_nothing() {
        let ui = UiState::new();
        assert!(persona_line(&ui).contains("not reported"));
    }
}
