// SPDX-License-Identifier: MPL-2.0
//! Shared controls over product-owned selections. No document, I/O or clock.
use audio_packs::{PackEdit, PackSelection};
use audio_pattern::{Sound, Step};
use cambium::{AnyView, GenetCtx, GenetElement, button, el, text};
use scenomise::{span_edit::SpanEdit, template::Span};

pub type Child<S> = Box<dyn AnyView<S, (), GenetCtx, GenetElement>>;
#[derive(Clone, Debug)]
pub enum PackAction {
    Bundled,
    Import,
    Clear,
    ExportMidi,
    AddLoop,
    Pattern { revision: String, id: String },
    Edit { revision: String, edit: PackEdit },
}

pub fn panel<S: 'static>(
    selection: Option<&PackSelection>,
    notice: &str,
    busy: bool,
    add_loop: bool,
    act: fn(&mut S, PackAction),
) -> Child<S> {
    let mut children: Vec<Child<S>> = vec![
        Box::new(el("h3", text("Drum kit"))),
        Box::new(
            el("p", text(notice))
                .attr("role", "status")
                .attr("data-pack-status", "true"),
        ),
    ];
    if busy {
        return Box::new(el("section", children).attr("aria-label", "Drum kit").attr("class", "audio-pack-panel").attr("style", "display:flex;flex-direction:column"));
    }
    let controls: Vec<Child<S>> = [
        ("Basic drums", PackAction::Bundled, "pack-bundled"),
        ("Load kit folder", PackAction::Import, "pack-import"),
        ("Clear kit", PackAction::Clear, "pack-clear"),
    ]
    .into_iter()
    .map(|(label, action, key)| {
        Box::new(
            button(label, move |s: &mut S, _: cambium::PointerClick| {
                act(s, action.clone())
            })
            .attr("data-action", key)
            .attr("class", "t-btn"),
        ) as Child<S>
    })
    .collect();
    children.push(Box::new(el("div", controls).attr("class", "transport")));
    if let Some(selection) = selection {
        let p = selection.pattern();
        children.push(Box::new(el(
            "p",
            text(format!(
                "{} · {} · {}/{} · {} steps per beat",
                selection.name().unwrap_or(&selection.identity().id),
                selection.pattern_id(),
                p.time_signature.numerator,
                p.time_signature.denominator,
                p.subdivision.divisions_per_beat
            )),
        )));
        children.push(Box::new(el("p", text("Click a step: rest → hit → accent → rest. Arrows move a hit one step; occupied destinations are refused."))));
        let revision = selection.revision();
        let choices: Vec<Child<S>> = selection
            .pattern_ids()
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                let action = PackAction::Pattern {
                    revision: revision.clone(),
                    id: id.clone(),
                };
                Box::new(
                    button(id, move |s: &mut S, _: cambium::PointerClick| {
                        act(s, action.clone())
                    })
                    .attr("class", "t-btn"),
                ) as Child<S>
            })
            .collect();
        children.push(Box::new(el("div", choices).attr("class", "transport")));
        for (track, t) in p.tracks.iter().enumerate() {
            let mut row: Vec<Child<S>> = vec![Box::new(el("span", text(&t.name)))];
            for (label, edit) in [
                (
                    if t.muted { "Unmute" } else { "Mute" },
                    PackEdit::MuteTrack(track),
                ),
                ("Sample level −", PackEdit::Gain { track, delta: -0.1 }),
                ("Sample level +", PackEdit::Gain { track, delta: 0.1 }),
            ] {
                let action = PackAction::Edit {
                    revision: revision.clone(),
                    edit,
                };
                row.push(Box::new(
                    button(label, move |s: &mut S, _: cambium::PointerClick| {
                        act(s, action.clone())
                    })
                    .attr("aria-label", format!("{label} {}", t.name))
                    .attr("class", "t-btn"),
                ));
            }
            if let Sound::Sample { gain, .. } = &t.sound {
                row.push(Box::new(el("span", text(format!("{gain:.1}")))));
            }
            children.push(Box::new(el("div", row).attr("class", "transport")));
            let steps: Vec<Child<S>> = t
                .steps
                .iter()
                .enumerate()
                .map(|(step, value)| {
                    let label = match value {
                        Step::Empty => "·",
                        Step::Active { accent: false } => "●",
                        _ => "◆",
                    };
                    let action = PackAction::Edit {
                        revision: revision.clone(),
                        edit: PackEdit::CycleStep { track, step },
                    };
                    let mut cell: Vec<Child<S>> = vec![Box::new(
                        button(
                            format!("{} {label}", step + 1),
                            move |s: &mut S, _: cambium::PointerClick| act(s, action.clone()),
                        )
                        .attr(
                            "aria-label",
                            format!("{} step {} {label}", t.name, step + 1),
                        )
                        .attr("data-action", format!("pack-step-{track}-{step}"))
                        .attr("class", "t-btn"),
                    )];
                    if *value != Step::Empty {
                        for (label, delta) in [("←", -1.0), ("→", 1.0)] {
                            // Mere proposes a quantized musical span; the owner
                            // checks revision, source track and destination again.
                            if let Ok(span) = SpanEdit::Move.apply(
                                Span {
                                    start: step as u32,
                                    length: 1,
                                },
                                delta,
                                t.steps.len() as u32,
                                1,
                            ) {
                                if span.start as usize == step {
                                    continue;
                                }
                                let action = PackAction::Edit {
                                    revision: revision.clone(),
                                    edit: PackEdit::MoveHit {
                                        track,
                                        from: step,
                                        to: span.start as usize,
                                    },
                                };
                                cell.push(Box::new(
                                    button(label, move |s: &mut S, _: cambium::PointerClick| {
                                        act(s, action.clone())
                                    })
                                    .attr(
                                        "aria-label",
                                        format!("Move {} step {} {label}", t.name, step + 1),
                                    )
                                    .attr("class", "t-btn"),
                                ));
                            }
                        }
                    }
                    Box::new(el("div", cell).attr("style", "display:flex;gap:2px;align-items:center;flex-shrink:0"))
                        as Child<S>
                })
                .collect();
            children.push(Box::new(el("div", steps).attr(
                "style",
                "display:flex;flex-wrap:wrap;gap:4px;margin-bottom:12px",
            )));
        }
        children.push(Box::new(
            button("Export MIDI", move |s: &mut S, _: cambium::PointerClick| {
                act(s, PackAction::ExportMidi)
            })
            .attr("data-action", "pack-export-midi")
            .attr("class", "t-btn"),
        ));
        if add_loop {
            children.push(Box::new(
                button(
                    "Add drum loop to armed track",
                    move |s: &mut S, _: cambium::PointerClick| act(s, PackAction::AddLoop),
                )
                .attr("data-action", "pack-add-loop")
                .attr("class", "t-btn"),
            ));
        }
    }
    Box::new(
        el("section", children)
            .attr("aria-label", "Drum kit")
            .attr("class", "audio-pack-panel")
            .attr("style", "display:flex;flex-direction:column"),
    )
}
