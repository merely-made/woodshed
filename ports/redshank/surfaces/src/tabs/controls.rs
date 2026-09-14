//! Stepper, Segment, Toggle, and Readout as CSS on native buttons.
//!
//! Cambium ships no such controls; when the shared library grows them these
//! builders are deleted and the callers adopt them. Lane S2 owns this file.

use crate::{CompactCommand, FullView, RedshankSurfaceState};
use cambium::{button, el, text};

/// `−` value unit `+`, with the value exposed as `aria-valuenow`.
pub fn stepper(
    label: &str,
    value: i64,
    unit: &str,
    decrease: CompactCommand,
    increase: CompactCommand,
) -> FullView {
    let step = |glyph: &'static str, word: &'static str, command: CompactCommand| {
        Box::new(
            button(glyph, move |state: &mut RedshankSurfaceState, _| {
                state.request(command.clone());
            })
            .attr("class", "rs-stepper-step")
            .attr("aria-label", format!("{word} {label}")),
        ) as FullView
    };
    Box::new(
        el(
            "div",
            (
                step("−", "Decrease", decrease),
                el("span", text(format!("{value} {unit}"))).attr("class", "rs-stepper-value"),
                step("+", "Increase", increase),
            ),
        )
        .attr("class", "rs-stepper")
        .attr("role", "group")
        .attr("aria-label", label.to_owned())
        .attr("aria-valuenow", value.to_string())
        .attr("aria-valuetext", format!("{value} {unit}")),
    )
}

/// A bordered option strip; the selected option carries `rs-segment-on`. An
/// option may emit more than one command (Rate sets the backend and the store).
pub fn segment(label: &str, options: Vec<(String, bool, Vec<CompactCommand>)>) -> FullView {
    let buttons: Vec<FullView> = options
        .into_iter()
        .map(|(option, selected, commands)| {
            let aria = format!("{label}: {option}");
            Box::new(
                button(option, move |state: &mut RedshankSurfaceState, _| {
                    for command in &commands {
                        state.request(command.clone());
                    }
                })
                .attr(
                    "class",
                    if selected {
                        "rs-segment-option rs-segment-on"
                    } else {
                        "rs-segment-option"
                    },
                )
                .attr("role", "radio")
                .attr("aria-label", aria)
                .attr("aria-checked", if selected { "true" } else { "false" }),
            ) as FullView
        })
        .collect();
    Box::new(
        el("div", buttons)
            .attr("class", "rs-segment")
            .attr("role", "radiogroup")
            .attr("aria-label", label.to_owned()),
    )
}

/// A switch that flips its field.
pub fn toggle(label: &str, on: bool, command: CompactCommand) -> FullView {
    Box::new(
        button(
            if on { "on" } else { "off" },
            move |state: &mut RedshankSurfaceState, _| {
                state.request(command.clone());
            },
        )
        .attr(
            "class",
            if on {
                "rs-toggle rs-toggle-on"
            } else {
                "rs-toggle"
            },
        )
        .attr("role", "switch")
        .attr("aria-label", label.to_owned())
        .attr("aria-checked", if on { "true" } else { "false" }),
    )
}

/// A value the surface reports but does not set.
pub fn readout(label: &str, value: impl Into<String>, unit: &str) -> FullView {
    Box::new(
        el(
            "div",
            (
                el("span", text(value.into())).attr("class", "rs-readout-value"),
                el("span", text(unit.to_owned())).attr("class", "rs-readout-unit"),
            ),
        )
        .attr("class", "rs-readout")
        .attr("role", "status")
        .attr("aria-label", label.to_owned()),
    )
}

/// One label/control settings row.
pub fn row(label: &str, hint: Option<&str>, control: FullView) -> FullView {
    let label_cell = el(
        "div",
        (
            el("span", text(label.to_owned())).attr("class", "rs-settings-label"),
            hint.map(|hint| el("span", text(hint.to_owned())).attr("class", "rs-settings-hint")),
        ),
    )
    .attr("class", "rs-settings-labels");
    Box::new(el("div", (label_cell, control)).attr("class", "rs-row rs-settings-row"))
}
