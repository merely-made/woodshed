use cambium::{HoverEvent, HoverPhase, clickable, custom_leaf, el, on_hover, text};
use woodshed_core::step_set;
use woodshedding::rehearsal::{LoopMode, MarkMode, Recipe};

use super::{UiChild, UiState};
use crate::fretboard_leaf::REHEARSAL_FRETBOARD_LEAF_KEY;

fn recipe_line(recipe: &Recipe) -> String {
    match recipe {
        Recipe::Progression { name, .. } => format!("from {name}"),
        Recipe::Exercise { name } => format!("from {name}"),
        Recipe::PracticeSet { name } => format!("from {name}"),
        Recipe::Song { name, bar } => format!("from {name} · bar {bar}"),
    }
}

impl UiState {
    /// Select a Card in the visible Set. A running managed Set receives a seek
    /// command so the next clock observation cannot overwrite a local cursor
    /// change; an inspected Set remains independent of a background runner.
    pub fn select_rehearsal_card(&mut self, index: usize) {
        if index >= self.set.cards.len() {
            return;
        }
        if self.practice_clock_managed && self.is_current_set_rehearsing() {
            self.queue_practice_clock_command(super::PracticeClockCommand::Seek(index));
            return;
        }
        if self.is_current_set_rehearsing() {
            self.finish_rehearsal_observation(woodshed_core::history::EngagementKind::Rehearsed);
        }
        self.set.cursor = index;
        if self.is_current_set_rehearsing() {
            self.record_rehearsal_cursor();
        }
    }

    pub fn previous_rehearsal_card(&mut self) {
        if self.set.cards.is_empty() {
            return;
        }
        if self.practice_clock_managed && self.is_current_set_rehearsing() {
            let set = self.rehearsal_set();
            let current = set.cursor.min(set.cards.len() - 1);
            let target = if current > 0 {
                current - 1
            } else if set.loop_mode == LoopMode::All {
                set.cards.len() - 1
            } else {
                current
            };
            self.queue_practice_clock_command(super::PracticeClockCommand::Seek(target));
            return;
        }
        if self.is_current_set_rehearsing() {
            self.finish_rehearsal_observation(woodshed_core::history::EngagementKind::Rehearsed);
        }
        step_set(&mut self.set, -1);
        if self.is_current_set_rehearsing() {
            self.record_rehearsal_cursor();
        }
    }

    pub fn next_rehearsal_card(&mut self) {
        if self.practice_clock_managed && self.is_current_set_rehearsing() {
            self.advance_rehearsal_cursor();
            return;
        }
        if self.is_current_set_rehearsing() {
            self.complete_rehearsal_cursor();
        }
        let advanced = step_set(&mut self.set, 1);
        if self.is_current_set_rehearsing() {
            if advanced {
                self.record_rehearsal_cursor();
            } else {
                self.stop_rehearsal();
            }
        }
    }

    pub fn cycle_set_loop_mode(&mut self) {
        self.set.loop_mode = match self.set.loop_mode {
            LoopMode::Off => LoopMode::All,
            LoopMode::All => LoopMode::Card,
            LoopMode::Card => LoopMode::Off,
        };
    }
}

fn progress_text(ui: &UiState, card: &woodshedding::rehearsal::Card) -> Option<(String, f32)> {
    let projection = ui.practice_clock_projection.as_ref()?;
    let owner = ui.rehearsal_owner?;
    if !ui.rehearsal_running
        || projection.epoch != ui.rehearsal_epoch
        || projection.owner != owner
        || owner != ui.working_sets.active_id
        || projection.card_id != card.id
        || projection.sample_rate == 0
        || ui.validate_live_clock_source().is_err()
    {
        return None;
    }
    let lowered = ui
        .rehearsal_clock_program
        .as_ref()?
        .cards
        .get(projection.card_index)?;
    if lowered.card.id != card.id {
        return None;
    }
    use woodshed_core::PracticeHold;
    match lowered.hold {
        PracticeHold::Beats(total) => {
            let beat = projection.beat_ticks as f64 / projection.ticks_per_beat.max(1) as f64;
            Some((
                format!("{beat:.1} / {total:.1} beats"),
                (beat / total).clamp(0.0, 1.0) as f32,
            ))
        },
        PracticeHold::Seconds(total) => {
            let elapsed = projection.card_frames as f64 / projection.sample_rate as f64;
            let fraction = elapsed / total;
            Some((
                format!("{elapsed:.1} / {total:.1}s"),
                fraction.clamp(0.0, 1.0) as f32,
            ))
        },
        PracticeHold::Manual { .. }
            if projection.status == super::PracticeClockStatus::WaitingManual =>
        {
            Some(("Phrase complete · waiting for Next".into(), 1.0))
        },
        PracticeHold::Manual { .. } => {
            let total = lowered.events.len();
            let started = projection
                .event_ordinal
                .map_or(0, |ordinal| ordinal as usize + 1)
                .min(total);
            let label = if projection.status == super::PracticeClockStatus::Paused {
                "Paused · Manual · Resume or Next".into()
            } else {
                format!("Manual · events {started}/{total} · Next advances")
            };
            Some((
                label,
                if total == 0 {
                    0.0
                } else {
                    started as f32 / total as f32
                },
            ))
        },
    }
}

fn adjacent_card_label(ui: &UiState, index: Option<usize>) -> String {
    index
        .and_then(|index| ui.set.cards.get(index))
        .map(|card| format!("{} · {}", card.material.tag(), card.label))
        .unwrap_or_else(|| "—".into())
}

fn adjacent_card_indices(
    cursor: usize,
    len: usize,
    mode: LoopMode,
) -> (Option<usize>, Option<usize>) {
    let previous = cursor
        .checked_sub(1)
        .or_else(|| (mode == LoopMode::All).then_some(len.saturating_sub(1)));
    let next = if cursor + 1 < len {
        Some(cursor + 1)
    } else {
        match mode {
            LoopMode::All => Some(0),
            LoopMode::Card | LoopMode::Off => None,
        }
    };
    (previous, next)
}

pub(super) fn screen(ui: &UiState) -> UiChild {
    if ui.set.cards.is_empty() {
        return Box::new(
            el(
                "div",
                el(
                    "div",
                    (
                        el("div", text(ui.working_sets.active_name.clone()))
                            .attr("class", "working-instance-name"),
                        el("div", text("The set is empty. Stage material to begin.")),
                    ),
                )
                .attr("class", "placeholder"),
            )
            .attr("class", "board"),
        );
    }
    let cursor = ui.set.cursor.min(ui.set.cards.len() - 1);
    let loop_label = match ui.set.loop_mode {
        LoopMode::Off => "Loop: off",
        LoopMode::All => "Loop: all",
        LoopMode::Card => "Loop: card",
    };
    let (previous, next) = adjacent_card_indices(cursor, ui.set.cards.len(), ui.set.loop_mode);
    let deck: UiChild = Box::new(
        el(
            "div",
            (
                el("div", text(ui.working_sets.active_name.clone()))
                    .attr("class", "working-instance-name"),
                (ui.rehearsal_running && !ui.is_current_set_rehearsing()).then(|| {
                    el(
                        "div",
                        text("Another Set is rehearsing. Run transfers rehearsal to this Set."),
                    )
                    .attr("class", "t-readout background-rehearsal-status")
                }),
                clickable(
                    el(
                        "button",
                        text(if ui.is_current_set_rehearsing() {
                            if ui.rehearsal_paused {
                                "Resume"
                            } else {
                                "Pause"
                            }
                        } else {
                            "Run"
                        }),
                    )
                    .attr("class", "t-btn rehearsal-run")
                    .attr(
                        "aria-label",
                        if ui.is_current_set_rehearsing() {
                            if ui.rehearsal_paused {
                                "Resume Rehearsal"
                            } else {
                                "Pause Rehearsal"
                            }
                        } else {
                            "Run Rehearsal"
                        },
                    ),
                    |ui: &mut UiState, _| {
                        ui.toggle_rehearsal();
                    },
                ),
                clickable(
                    el("button", text("Prev"))
                        .attr("class", "t-btn rehearsal-prev")
                        .attr("aria-label", "Previous Card"),
                    |ui: &mut UiState, _| ui.previous_rehearsal_card(),
                ),
                clickable(
                    el("button", text("Next"))
                        .attr("class", "t-btn rehearsal-next")
                        .attr("aria-label", "Next Card"),
                    |ui: &mut UiState, _| ui.next_rehearsal_card(),
                ),
                (ui.rehearsal_running && ui.is_current_set_rehearsing()).then(|| {
                    clickable(
                        el("button", text("Stop"))
                            .attr("class", "t-btn rehearsal-stop")
                            .attr("aria-label", "Stop Rehearsal"),
                        |ui: &mut UiState, _| ui.stop_rehearsal(),
                    )
                }),
                clickable(
                    el("button", text("♪ Hear selected Card"))
                        .attr("class", "t-btn t-hear rehearsal-hear")
                        .attr("aria-label", "Hear selected Card"),
                    |ui: &mut UiState, _| ui.request_card_preview(),
                ),
                clickable(
                    el("button", text(loop_label))
                        .attr("class", "t-btn rehearsal-loop-mode")
                        .attr("aria-label", format!("Set loop mode: {loop_label}")),
                    |ui: &mut UiState, _| ui.cycle_set_loop_mode(),
                ),
                clickable(
                    el("button", text("Remove"))
                        .attr("class", "t-btn rehearsal-remove")
                        .attr("aria-label", "Remove selected Card"),
                    |ui: &mut UiState, _| {
                        let idx = ui.set.cursor;
                        ui.set.remove(idx);
                    },
                ),
                el(
                    "div",
                    text(format!("card {}/{}", cursor + 1, ui.set.cards.len())),
                )
                .attr("class", "t-readout"),
            ),
        )
        .attr("class", "transport rehearsal-transport"),
    );
    let progress = progress_text(ui, &ui.set.cards[cursor]);
    let context = el(
        "div",
        (
            el(
                "div",
                text(format!("Previous · {}", adjacent_card_label(ui, previous))),
            )
            .attr("class", "rehearsal-adjacent"),
            el(
                "div",
                text(format!(
                    "Current · {}",
                    adjacent_card_label(ui, Some(cursor))
                )),
            )
            .attr("class", "rehearsal-current"),
            el(
                "div",
                text(format!("Next · {}", adjacent_card_label(ui, next))),
            )
            .attr("class", "rehearsal-adjacent"),
            progress.map(|(label, fraction)| {
                let accessible_label = label.clone();
                el(
                    "div",
                    (
                        el("div", text(label)).attr("class", "rehearsal-progress-label"),
                        el(
                            "div",
                            el("div", ())
                                .attr("class", "rehearsal-progress-fill")
                                .attr("style", format!("width: {:.1}%", fraction * 100.0)),
                        )
                        .attr("class", "rehearsal-progress-track")
                        .attr("role", "progressbar")
                        .attr("aria-valuemin", "0")
                        .attr("aria-valuemax", "100")
                        .attr("aria-valuenow", format!("{:.0}", fraction * 100.0))
                        .attr("aria-label", accessible_label),
                    ),
                )
                .attr("class", "rehearsal-progress")
                .attr("role", "status")
            }),
        ),
    )
    .attr("class", "rehearsal-context");
    // The measured filmstrip (redesign P5): every card with its tag,
    // provenance, and touch; played cards dim behind the cursor
    // (engine group opacity), the current card ringed; the strip is a
    // horizontal scroll container (engine element scroll).
    let films: Vec<UiChild> = ui
        .set
        .cards
        .iter()
        .enumerate()
        .map(|(i, card)| {
            let class = match i.cmp(&cursor) {
                std::cmp::Ordering::Less => "film-card film-played",
                std::cmp::Ordering::Equal => "film-card film-current",
                std::cmp::Ordering::Greater => "film-card",
            };
            let provenance = card
                .from
                .as_ref()
                .map(recipe_line)
                .unwrap_or_else(|| "hand-added".to_string());
            let touch = match &card.touch {
                woodshedding::rehearsal::Touch::Block => "block".to_string(),
                woodshedding::rehearsal::Touch::Arpeggiate { direction, .. } => {
                    format!("arpeggiate {}", direction.label())
                },
                woodshedding::rehearsal::Touch::Walk => "walk".to_string(),
            };
            Box::new(clickable(
                el(
                    "button",
                    (
                        el("div", text(card.material.tag())).attr("class", "film-tag"),
                        el("div", text(card.label.clone())).attr("class", "film-label"),
                        el("div", text(touch)).attr("class", "film-meta"),
                        el("div", text(provenance)).attr("class", "film-meta"),
                    ),
                )
                .attr("class", class)
                .attr(
                    "aria-label",
                    format!("Select Card {}: {}", i + 1, card.label),
                )
                .attr("aria-pressed", if i == cursor { "true" } else { "false" }),
                move |ui: &mut UiState, _| ui.select_rehearsal_card(i),
            )) as UiChild
        })
        .collect();
    // The same Card editor appears in Stage's Set tray and Rehearsal. Actions
    // mutate the one persisted Set through UiState helpers.
    let editor = el("div", super::set_tray::card_editor(ui)).attr("class", "card-details");
    let scale_status: Option<UiChild> = ui.rehearsal_scale_status().map(|status| match status {
        Ok(message) => {
            Box::new(el("div", text(message)).attr("class", "t-readout scale-setup-status"))
                as UiChild
        },
        Err(message) => Box::new(
            el("div", text(message))
                .attr("class", "scale-setup-unavailable")
                .attr("role", "status"),
        ) as UiChild,
    });
    let approach_status: Option<UiChild> =
        ui.rehearsal_approach_status().map(|status| match status {
            Ok(message) => {
                Box::new(el("div", text(message)).attr("class", "t-readout chord-approach-status"))
                    as UiChild
            },
            Err(message) => Box::new(
                el("div", text(message))
                    .attr("class", "chord-approach-unavailable")
                    .attr("role", "status"),
            ) as UiChild,
        });
    if ui
        .rehearsal_approach_status()
        .is_some_and(|status| status.is_err())
    {
        return Box::new(el(
            "div",
            (
                deck,
                context,
                el("div", films).attr("class", "filmstrip"),
                editor,
                approach_status,
            ),
        ));
    }
    if ui
        .rehearsal_scale_status()
        .is_some_and(|status| status.is_err())
    {
        return Box::new(el(
            "div",
            (
                deck,
                context,
                el("div", films).attr("class", "filmstrip"),
                editor,
                scale_status,
            ),
        ));
    }

    Box::new(el(
        "div",
        (
            deck,
            context,
            el("div", films).attr("class", "filmstrip"),
            editor,
            scale_status,
            approach_status,
            card_board(ui),
        ),
    ))
}

#[cfg(test)]
#[path = "rehearsal_tests.rs"]
mod rehearsal_tests;

/// Shared retained-Card fretboard used in Stage and Rehearsal.
/// It never substitutes the live catalog selection for the saved instruction.
pub(super) fn card_board(ui: &UiState) -> UiChild {
    for status in [ui.rehearsal_scale_status(), ui.rehearsal_approach_status()] {
        if let Some(Err(reason)) = status {
            return Box::new(el("div", text(reason)).attr("role", "status"));
        }
    }
    // Current card's material on the big board — the same Sprigging paint leaf
    // the Stage board uses. Over it, one clickable label per note: click *marks*
    // the note (a neutral selection, an accent ring). The [Off · Solo · Mute]
    // control below sets what marks do (Solo plays only the marked, Mute
    // silences them), and the board dims whatever the mode excludes. This is the
    // Selection axis of touch, made interactive.
    let Some(card) = ui.current_card() else {
        return Box::new(el("div", ()));
    };
    let dot_list = ui.current_card_stage().dots_for_card(card);
    let geom = ui.rehearsal_board_geometry();
    let string_count = geom.string_count;
    let (w, h) = geom.size_u32();
    let (mw, mh) = geom.marker_size();
    let labels: Vec<UiChild> = dot_list
        .iter()
        .map(|d| {
            let (si, fret) = (d.string_index, d.fret);
            let (px, py) = geom.note_pos(si, fret);
            let lx = px - mw / 2.0;
            let ly = py - mh / 2.0;
            let mut class = String::from("fret-label");
            if ui.card_marked(si, fret) {
                class.push_str(" marked");
            }
            if ui.card_excluded(si, fret) {
                class.push_str(" excluded");
            }
            // The spoken note/role/place plus the mark state, so a screen-reader
            // user knows what a marker is and what marking did to it.
            let mut a11y = woodshed_core::marker_a11y_label(d, string_count);
            if ui.card_marked(si, fret) {
                a11y.push_str(", marked");
            }
            if ui.card_excluded(si, fret) {
                a11y.push_str(", muted");
            }
            // click marks the note; hover peeks its detail card (the resolved
            // "click marks, hover shows").
            Box::new(on_hover(
                clickable(
                    el("div", text(d.label.clone()))
                        .attr("class", class)
                        .attr("role", "button")
                        .attr("aria-label", a11y)
                        .attr(
                            "style",
                            format!(
                                "left:{lx:.1}px; top:{ly:.1}px; width:{mw:.1}px; height:{mh:.1}px"
                            ),
                        ),
                    move |ui: &mut UiState, _| ui.toggle_card_mark(si, fret),
                ),
                move |ui: &mut UiState, ev: HoverEvent| match ev.phase {
                    HoverPhase::Enter | HoverPhase::Move => ui.hover_peek = Some((si, fret)),
                    HoverPhase::Leave => {
                        if ui.hover_peek == Some((si, fret)) {
                            ui.hover_peek = None;
                        }
                    },
                },
            )) as UiChild
        })
        .collect();
    // The hovered marker's ephemeral detail card (reuses the Stage board's
    // card, which is position-free now that Stage flows its pinned cards in a
    // strip). The peek stays an overlay: this wrapper anchors it beside its
    // marker, flipped above for markers in the lower half so it stays on-board.
    let peek: Vec<UiChild> = ui
        .hover_peek
        .and_then(|(si, fret)| {
            dot_list
                .iter()
                .find(|d| d.string_index == si && d.fret == fret)
                .map(|d| {
                    const CARD_W: f32 = 140.0;
                    const CARD_H: f32 = 112.0;
                    let (cx, cy) = geom.note_pos(d.string_index, d.fret);
                    let (_mw, mh) = geom.marker_size();
                    let (_bw, bh) = geom.size();
                    let below = cy < bh / 2.0;
                    let top = if below {
                        cy + mh / 2.0 + 6.0
                    } else {
                        cy - mh / 2.0 - 6.0 - CARD_H
                    };
                    let left = (cx - CARD_W / 2.0).max(2.0);
                    Box::new(el("div", super::note_card(d, string_count)).attr(
                        "style",
                        format!("position:absolute; left:{left:.1}px; top:{top:.1}px;"),
                    )) as UiChild
                })
        })
        .into_iter()
        .collect();
    // The mode control: a small [Off · Solo · Mute] segmented toggle, not a loose
    // button row. Sets what the marked set does to this card.
    let mode = ui.card_mark_mode();
    let mark_count = card.setting.marked.len();
    let seg = |m: MarkMode| -> UiChild {
        let cls = if mode == m { "seg active" } else { "seg" };
        Box::new(clickable(
            el("div", text(m.label())).attr("class", cls),
            move |ui: &mut UiState, _| ui.set_card_mark_mode(m),
        )) as UiChild
    };
    Box::new(
        el(
            "div",
            (
                el(
                    "div",
                    el(
                        "div",
                        (
                            custom_leaf::<UiState, ()>(REHEARSAL_FRETBOARD_LEAF_KEY, w, h),
                            el("div", labels).attr("class", "label-layer"),
                            el("div", peek).attr("class", "card-layer"),
                        ),
                    )
                    .attr("class", "fretboard-stack")
                    .attr(
                        "aria-label",
                        format!("{} fretboard, {} notes", card.label, dot_list.len()),
                    )
                    .attr("style", format!("width:{w}px; height:{h}px")),
                )
                .attr("class", "board-viewport rehearsal-board-viewport")
                .attr(
                    "style",
                    super::board_viewport_style(geom.orientation, w, h, ui.viewport_h),
                ),
                el(
                    "div",
                    (
                        el(
                            "div",
                            (seg(MarkMode::Off), seg(MarkMode::Solo), seg(MarkMode::Mute)),
                        )
                        .attr("class", "mode-seg"),
                        el(
                            "div",
                            text(format!("Card {} · {}", ui.set.cursor + 1, card.label)),
                        )
                        .attr("class", "scale-name"),
                        (mark_count > 0).then(|| {
                            clickable(
                                el("div", text(format!("Clear {mark_count} marks")))
                                    .attr("class", "clear-pins"),
                                |ui: &mut UiState, _| ui.clear_card_marks(),
                            )
                        }),
                    ),
                )
                .attr("class", "board-caption"),
            ),
        )
        .attr("class", "board")
        .attr("data-selected-card", card.id.0.to_string()),
    )
}
