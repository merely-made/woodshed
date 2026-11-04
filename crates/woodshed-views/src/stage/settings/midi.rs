use super::*;

/// MIDI device panel (audio-depth slice 13): port pickers, clock-slave /
/// clock-master toggles, and a live status + event readout. The host
/// realizes the selections through the `MidiBackend` seam.
pub(super) fn midi_panel(ui: &UiState) -> UiChild {
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
pub(super) fn pedal_panel(ui: &UiState) -> UiChild {
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

pub(super) fn midi_diagnostics_panel(ui: &UiState) -> UiChild {
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

pub(super) fn pedal_input_panel(ui: &UiState) -> UiChild {
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

pub(super) fn pedal_number(ui: &UiState, action: PedalAction, field: PedalField) -> UiChild {
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
pub(super) fn calibration_panel(ui: &UiState) -> UiChild {
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
