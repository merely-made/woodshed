use cambium::{clickable, el, text};
use woodshed_core::audio::AudioRequest;
use woodshed_core::loop_plan::{LoopPlan, LoopReadiness, LoopSegmentId, LooperSource};
use woodshed_core::song::{SECTION_LABELS, song_from_progression};
use woodshed_core::working_sets::WorkingSetId;

use super::{LoopExportRequest, UiChild, UiState};

fn song_deck(ui: &UiState) -> UiChild {
    let from_prog_label = match ui.stage.progression_idx {
        Some(_) => "From progression",
        None => "From progression (pick one on Stage first)",
    };
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text(if ui.song_playing { "Stop" } else { "Play" }))
                        .attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        if !ui.song.is_empty() {
                            ui.song_playing = !ui.song_playing;
                        }
                    },
                ),
                clickable(
                    el("div", text("Rewind")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.request(AudioRequest::SongRewind);
                        ui.song_bar_live = 0;
                    },
                ),
                clickable(
                    el("div", text(if ui.song.one_shot { "Once" } else { "Loop" }))
                        .attr("class", "t-btn"),
                    |ui: &mut UiState, _| ui.song.one_shot = !ui.song.one_shot,
                ),
                clickable(
                    el(
                        "div",
                        text(if ui.song.click {
                            "Click: on"
                        } else {
                            "Click: off"
                        }),
                    )
                    .attr("class", "t-btn"),
                    |ui: &mut UiState, _| ui.song.click = !ui.song.click,
                ),
                clickable(
                    el("div", text(from_prog_label)).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        if let Some(doc) = song_from_progression(&ui.stage, ui.transport.bpm) {
                            ui.song = doc;
                            ui.song_playing = false;
                            ui.song_bar_live = 0;
                            ui.song_edit_cursor = 0;
                            ui.request(AudioRequest::SongRewind);
                        }
                    },
                ),
                el(
                    "div",
                    text(if ui.song.name.is_empty() {
                        format!("{} bars", ui.song.bars.len())
                    } else {
                        format!("{} · {} bars", ui.song.name, ui.song.bars.len())
                    }),
                )
                .attr("class", "t-readout"),
            ),
        )
        .attr("class", "transport"),
    )
}

fn song_bar_ops(_ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text("+ Bar")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.song_edit_cursor = ui.song.add_bar_after(ui.song_edit_cursor);
                    },
                ),
                clickable(
                    el("div", text("Dup")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.song_edit_cursor = ui.song.duplicate(ui.song_edit_cursor);
                    },
                ),
                clickable(
                    el("div", text("Remove")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.song_edit_cursor = ui.song.remove(ui.song_edit_cursor);
                    },
                ),
                clickable(
                    el("div", text("◀ Move")).attr("class", "t-btn t-narrow"),
                    |ui: &mut UiState, _| {
                        ui.song_edit_cursor = ui.song.move_bar(ui.song_edit_cursor, -1);
                    },
                ),
                clickable(
                    el("div", text("Move ▶")).attr("class", "t-btn t-narrow"),
                    |ui: &mut UiState, _| {
                        ui.song_edit_cursor = ui.song.move_bar(ui.song_edit_cursor, 1);
                    },
                ),
            ),
        )
        .attr("class", "transport"),
    )
}

/// Looper controls (audio-depth slice 15): record your part into the
/// selected bar as the song loops past it, overdub or replace, and clear.
/// The engine records live input into the bar's audio buffer; these drive
/// it through the audio seam.
fn song_loop_ops(ui: &UiState) -> UiChild {
    let rec_label = if ui.song_recording {
        "● Recording / armed"
    } else {
        "● Rec bar"
    };
    let rec_class = if ui.song_recording {
        "t-btn rec-on"
    } else {
        "t-btn"
    };
    let mode_label = if ui.song_record_replace {
        "Replace"
    } else {
        "Overdub"
    };
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text(rec_label)).attr("class", rec_class),
                    |ui: &mut UiState, _| ui.request(AudioRequest::SongRecordToggle),
                ),
                clickable(
                    el("div", text(mode_label)).attr("class", "t-btn"),
                    |ui: &mut UiState, _| ui.song_record_replace = !ui.song_record_replace,
                ),
                clickable(
                    el("div", text("Clear loop")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| ui.request(AudioRequest::SongClearLoop),
                ),
                el(
                    "div",
                    text(
                        "play the song, then Rec to capture your part into \
                         the selected bar as it loops past",
                    ),
                )
                .attr("class", "t-readout"),
            ),
        )
        .attr("class", "transport"),
    )
}

/// Per-bar chord / tempo / meter / section editor for the cursor bar.
fn song_bar_editor(ui: &UiState) -> UiChild {
    let cursor = ui
        .song_edit_cursor
        .min(ui.song.bars.len().saturating_sub(1));
    let bar = &ui.song.bars[cursor];
    let root_label = if bar.formula_name.is_empty() {
        "silent".to_string()
    } else {
        bar.root_name()
    };
    let chord_label = if bar.formula_name.is_empty() {
        "—".to_string()
    } else {
        bar.formula_name.clone()
    };
    let section_label = if bar.label.is_empty() {
        "(no section)".to_string()
    } else {
        bar.label.clone()
    };
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text(format!("Root: {root_label}"))).attr("class", "t-btn"),
                    move |ui: &mut UiState, _| {
                        ui.song.bars[cursor].cycle_root();
                    },
                ),
                clickable(
                    el("div", text(format!("Chord: {chord_label}"))).attr("class", "t-btn"),
                    move |ui: &mut UiState, _| {
                        ui.song.bars[cursor].cycle_formula();
                    },
                ),
                clickable(
                    el("div", text("silent")).attr("class", "t-btn"),
                    move |ui: &mut UiState, _| {
                        ui.song.bars[cursor].toggle_silent();
                    },
                ),
                clickable(
                    el("div", text("-")).attr("class", "t-btn t-narrow"),
                    move |ui: &mut UiState, _| ui.song.bars[cursor].nudge_bpm(-5.0),
                ),
                el("div", text(format!("{:.0} bpm", bar.bpm))).attr("class", "t-readout"),
                clickable(
                    el("div", text("+")).attr("class", "t-btn t-narrow"),
                    move |ui: &mut UiState, _| ui.song.bars[cursor].nudge_bpm(5.0),
                ),
                clickable(
                    el("div", text(format!("{}/4", bar.beats))).attr("class", "t-btn"),
                    move |ui: &mut UiState, _| ui.song.bars[cursor].cycle_beats(),
                ),
                clickable(
                    el("div", text(format!("x{}", bar.length))).attr("class", "t-btn"),
                    move |ui: &mut UiState, _| ui.song.bars[cursor].cycle_length(),
                ),
                clickable(
                    el("div", text(section_label)).attr("class", "t-btn"),
                    move |ui: &mut UiState, _| {
                        let cur = &ui.song.bars[cursor].label;
                        let i = SECTION_LABELS
                            .iter()
                            .position(|s| *s == cur.as_str())
                            .unwrap_or(0);
                        let next = SECTION_LABELS[(i + 1) % SECTION_LABELS.len()];
                        ui.song.bars[cursor].label = next.to_string();
                    },
                ),
            ),
        )
        .attr("class", "transport"),
    )
}

fn saved_song_screen(ui: &UiState) -> UiChild {
    let deck = song_deck(ui);
    if ui.song.is_empty() {
        return Box::new(el(
            "div",
            (
                deck,
                song_bar_ops(ui),
                el(
                    "div",
                    el(
                        "div",
                        text(
                            "No bars yet. '+ Bar' to start one, or pick a \
                             progression on Stage and 'From progression'.",
                        ),
                    )
                    .attr("class", "placeholder"),
                )
                .attr("class", "board"),
            ),
        ));
    }
    let play_cursor = ui.song_bar_live.min(ui.song.bars.len() - 1);
    let edit_cursor = ui.song_edit_cursor.min(ui.song.bars.len() - 1);
    let chips: Vec<UiChild> = ui
        .song
        .bars
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut class = String::from("bar-chip");
            if i == play_cursor && ui.song_playing {
                class.push_str(" bar-current");
            }
            if i == edit_cursor {
                class.push_str(" bar-edit");
            }
            let section = if b.label.is_empty() {
                String::new()
            } else {
                b.label.clone()
            };
            let chord = if b.chord_label.is_empty() {
                "·".to_string()
            } else {
                b.chord_label.clone()
            };
            let looped = ui.song_loop_bars.get(i).copied().unwrap_or(false);
            Box::new(clickable(
                el(
                    "div",
                    (
                        el("div", text(section)).attr("class", "bar-label"),
                        el("div", text(chord)).attr("class", "bar-chord"),
                        el(
                            "div",
                            text(format!(
                                "{:.0} · {}/4{}",
                                b.bpm,
                                b.beats,
                                if looped { " ⟳" } else { "" }
                            )),
                        )
                        .attr("class", "bar-meta"),
                    ),
                )
                .attr("class", class),
                move |ui: &mut UiState, _| {
                    ui.song_edit_cursor = i;
                },
            )) as UiChild
        })
        .collect();
    Box::new(el(
        "div",
        (
            deck,
            song_bar_ops(ui),
            song_loop_ops(ui),
            el(
                "div",
                (
                    el("div", chips).attr("class", "bar-lane"),
                    song_bar_editor(ui),
                    el(
                        "div",
                        text(format!(
                            "editing bar {}/{} — chords voice at each bar top; \
                             the click follows each bar's tempo",
                            edit_cursor + 1,
                            ui.song.bars.len()
                        )),
                    )
                    .attr("class", "scale-name"),
                ),
            )
            .attr("class", "board"),
        ),
    ))
}

impl UiState {
    /// Select an owner without copying Cards into the legacy backing song.
    pub fn loop_working_set(&mut self, owner: WorkingSetId) {
        if self.song_recording && self.looper.source != LooperSource::Set(owner) {
            self.loop_notice =
                Some("Stop recording / armed capture before changing backing source.".into());
            return;
        }
        if self.working_sets.get(owner, &self.set).is_some() {
            self.looper.source = LooperSource::Set(owner);
            self.loop_selected = None;
            self.loop_notice = None;
        }
    }

    pub fn requested_loop_plan(&self) -> Result<LoopPlan, Vec<LoopReadiness>> {
        let LooperSource::Set(owner) = self.looper.source else {
            return Err(vec![LoopReadiness {
                segment: None,
                reason: "Choose a working Set.".into(),
            }]);
        };
        let Some(set) = self.working_sets.get(owner, &self.set) else {
            return Err(vec![LoopReadiness {
                segment: None,
                reason: "The selected working Set is unavailable.".into(),
            }]);
        };
        let name = self
            .working_sets
            .summaries(&self.set)
            .into_iter()
            .find(|summary| summary.id == owner)
            .map(|summary| summary.name)
            .unwrap_or_default();
        LoopPlan::lower(owner, &name, set, &self.stage, &self.looper)
    }

    /// Navigation opens the owner/Card setup; it does not retarget rehearsal.
    pub fn open_loop_card(&mut self, id: LoopSegmentId) {
        if self.activate_working_set(id.set) {
            self.set.select_id(id.card);
        }
    }

    fn selected_loop_segment(&self) -> Option<LoopSegmentId> {
        let requested = self.requested_loop_plan().ok();
        let plan = self.loop_admitted.as_ref().or(requested.as_ref())?;
        self.loop_selected
            .filter(|id| plan.index_of(*id).is_some())
            .or_else(|| plan.segments.first().map(|segment| segment.id))
    }
}

fn source_controls(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text("Loop this Set")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        ui.loop_working_set(ui.working_sets.active_id);
                    },
                ),
                clickable(
                    el("div", text("Saved backing")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        if ui.song_recording {
                            ui.loop_notice = Some(
                                "Stop recording / armed capture before changing backing source."
                                    .into(),
                            );
                            return;
                        }
                        ui.looper.source = LooperSource::SavedSong;
                        ui.song_playing = false;
                        ui.loop_notice = None;
                    },
                ),
                el(
                    "div",
                    text(match ui.looper.source {
                        LooperSource::SavedSong => "Source: saved backing song".to_string(),
                        LooperSource::Set(owner) => format!(
                            "Source: working Set {} · explicit {}/4 clock",
                            owner.0, ui.looper.beats
                        ),
                    }),
                )
                .attr("class", "t-readout"),
            ),
        )
        .attr("class", "transport"),
    )
}

fn set_screen(ui: &UiState) -> UiChild {
    let requested = ui.requested_loop_plan();
    let plan = ui
        .loop_admitted
        .as_ref()
        .or_else(|| requested.as_ref().ok());
    let selected = ui.selected_loop_segment();
    let chips: Vec<UiChild> = plan
        .into_iter()
        .flat_map(|plan| plan.segments.iter().enumerate())
        .map(|(index, segment)| {
            let id = segment.id;
            let mut class = "bar-chip".to_string();
            if Some(id) == selected {
                class.push_str(" bar-edit");
            }
            if index == ui.song_bar_live && ui.song_playing {
                class.push_str(" bar-current");
            }
            let looped = ui.song_loop_bars.get(index).copied().unwrap_or(false);
            Box::new(clickable(
                el(
                    "div",
                    (
                        el("div", text(segment.label.clone())).attr("class", "bar-chord"),
                        el(
                            "div",
                            text(format!(
                                "{} bpm · {}/4 × {}{}",
                                segment.bpm,
                                segment.beats,
                                segment.bars,
                                if looped { " ⟳" } else { "" }
                            )),
                        )
                        .attr("class", "bar-meta"),
                    ),
                )
                .attr("class", class),
                move |ui: &mut UiState, _| ui.loop_selected = Some(id),
            )) as UiChild
        })
        .collect();
    let issues: Vec<UiChild> = requested
        .as_ref()
        .err()
        .into_iter()
        .flatten()
        .map(|issue| {
            let id = issue.segment;
            let label = id
                .and_then(|id| ui.working_sets.get(id.set, &ui.set)?.card(id.card))
                .map(|card| card.label.as_str())
                .unwrap_or("Set");
            Box::new(clickable(
                el("div", text(format!("{label}: {}", issue.reason))).attr("class", "t-readout"),
                move |ui: &mut UiState, _| {
                    if let Some(id) = id {
                        ui.open_loop_card(id);
                    }
                },
            )) as UiChild
        })
        .collect();
    let loaded = ui
        .loop_admitted
        .as_ref()
        .map(|plan| {
            format!(
                "Loaded Set {}: {} · {} Cards",
                plan.owner.0,
                plan.name,
                plan.segments.len()
            )
        })
        .unwrap_or_else(|| "No Set backing admitted yet".into());
    Box::new(el("div", (
        el("div", (
            clickable(el("div", text(if ui.song_playing { "Stop" } else { "Play" })).attr("class", "t-btn"),
                |ui: &mut UiState, _| {
                    if ui.loop_admitted.is_some() || ui.requested_loop_plan().is_ok() { ui.song_playing = !ui.song_playing; }
                }),
            clickable(el("div", text("Rewind")).attr("class", "t-btn"), |ui: &mut UiState, _| ui.request(AudioRequest::SongRewind)),
            clickable(el("div", text(if ui.looper.one_shot { "Once" } else { "Loop" })).attr("class", "t-btn").attr("role", "button").attr("data-action", "toggle-set-one-shot"),
                |ui: &mut UiState, _| ui.looper.one_shot = !ui.looper.one_shot),
            clickable(el("div", text(if ui.looper.click { "Click: on" } else { "Click: off" })).attr("class", "t-btn"),
                |ui: &mut UiState, _| ui.looper.click = !ui.looper.click),
            el("div", text(loaded)).attr("class", "t-readout"),
        )).attr("class", "transport"),
        el("div", (
            clickable(el("div", text("Tempo −")).attr("class", "t-btn"), |ui: &mut UiState, _| ui.looper.inherited_bpm = (ui.looper.inherited_bpm - 5.0).max(1.0)),
            el("div", text(format!("Inherited: {} bpm", ui.looper.inherited_bpm))).attr("class", "t-readout"),
            clickable(el("div", text("Tempo +")).attr("class", "t-btn"), |ui: &mut UiState, _| ui.looper.inherited_bpm += 5.0),
            clickable(el("div", text(format!("Clock: {}/4", ui.looper.beats))).attr("class", "t-btn"), |ui: &mut UiState, _| {
                let meters = [2, 3, 4, 5, 7];
                let index = meters.iter().position(|beats| *beats == ui.looper.beats).unwrap_or(1);
                ui.looper.beats = meters[(index + 1) % meters.len()];
            }),
        )).attr("class", "transport"),
        el("div", (
            clickable(el("div", text(if ui.song_recording { "Stop recording / armed" } else { "Rec Card" })).attr("class", "t-btn"), |ui: &mut UiState, _| {
                if let Some(id) = ui.selected_loop_segment() { ui.request(AudioRequest::LoopRecordToggle(id)); }
            }),
            clickable(el("div", text(if ui.song_record_replace { "Replace" } else { "Overdub" })).attr("class", "t-btn"),
                |ui: &mut UiState, _| ui.song_record_replace = !ui.song_record_replace),
            clickable(el("div", text("Clear Card loop")).attr("class", "t-btn"), |ui: &mut UiState, _| {
                if let Some(id) = ui.selected_loop_segment() { ui.request(AudioRequest::LoopClear(id)); }
            }),
            clickable(el("div", text("Card setup")).attr("class", "t-btn"), |ui: &mut UiState, _| {
                if let Some(id) = ui.selected_loop_segment() { ui.open_loop_card(id); }
            }),
        )).attr("class", "transport"),
        el("div", issues).attr("class", "transport"),
        el("div", (
            el("div", chips).attr("class", "bar-lane"),
            el("div", text("Cards supply material and Bars holds. This clock supplies quarter-note meter and inherited tempo. Clear a recorded loop before changing its tempo or hold; captured PCM stays at its authored duration. Hear and rehearsal remain available for every Card.")).attr("class", "scale-name"),
        )).attr("class", "board"),
    )))
}

fn export_controls(ui: &UiState) -> UiChild {
    let waiting = ui.loop_export_busy || !ui.loop_export_requests.is_empty();
    let choose_disabled = waiting || ui.loop_export_replace.is_some();
    let mut controls: Vec<UiChild> = vec![Box::new(clickable(
        el("div", text("Export WAV"))
            .attr("class", "t-btn")
            .attr("role", "button")
            .attr("data-action", "export-wav")
            .attr("aria-disabled", choose_disabled.to_string()),
        |ui: &mut UiState, _| {
            if !ui.loop_export_busy
                && ui.loop_export_requests.is_empty()
                && ui.loop_export_replace.is_none()
            {
                ui.loop_export_requests
                    .push(LoopExportRequest::ChooseDestination);
            }
        },
    ))];
    if let Some((token, filename)) = &ui.loop_export_replace {
        let token = *token;
        controls.push(Box::new(
            el("div", text(format!("WAV already exists: {filename}"))).attr("class", "t-readout"),
        ));
        for (label, action, request) in [
            (
                "Replace WAV",
                "replace-wav",
                LoopExportRequest::Replace(token),
            ),
            (
                "Cancel",
                "cancel-replace-wav",
                LoopExportRequest::CancelReplace(token),
            ),
        ] {
            controls.push(Box::new(clickable(
                el("div", text(label))
                    .attr("class", "t-btn")
                    .attr("role", "button")
                    .attr("data-action", action)
                    .attr("aria-disabled", waiting.to_string()),
                move |ui: &mut UiState, _| {
                    if !ui.loop_export_busy
                        && ui.loop_export_requests.is_empty()
                        && ui
                            .loop_export_replace
                            .as_ref()
                            .is_some_and(|(current, _)| *current == token)
                    {
                        ui.loop_export_requests.push(request);
                    }
                },
            )));
        }
    }
    let notice = ui.loop_export_notice.clone().unwrap_or_else(|| {
        if ui.loop_export_busy {
            "Export in progress.".into()
        } else {
            String::new()
        }
    });
    Box::new(el(
        "div",
        (
            el("div", controls).attr("class", "transport"),
            el(
                "div",
                text("One pass · backing + completed loops · click follows Looper"),
            )
            .attr("class", "t-readout"),
            el("div", text(notice))
                .attr("class", "t-readout")
                .attr("role", "status"),
        ),
    ))
}

pub(super) fn screen(ui: &UiState) -> UiChild {
    let retained_ids: std::collections::BTreeSet<_> = ui
        .loop_retained_ids
        .iter()
        .copied()
        .chain(ui.loop_captures.takes.iter().map(|take| take.id))
        .collect();
    let retained: Vec<UiChild> = retained_ids
        .into_iter()
        .map(|id| {
            Box::new(clickable(
                el(
                    "div",
                    text(format!(
                        "Clear stored loop: Set {} Card {}",
                        id.set.0, id.card.0
                    )),
                )
                .attr("class", "t-btn")
                .attr("role", "button")
                .attr("data-action", "clear-stored-loop"),
                move |ui: &mut UiState, _| ui.request(AudioRequest::LoopClear(id)),
            )) as UiChild
        })
        .collect();
    let body = match ui.looper.source {
        LooperSource::SavedSong => saved_song_screen(ui),
        LooperSource::Set(_) => set_screen(ui),
    };
    Box::new(el(
        "div",
        (
            source_controls(ui),
            export_controls(ui),
            el("div", text(ui.loop_notice.clone().unwrap_or_default())).attr("class", "t-readout"),
            el(
                "div",
                text(ui.loop_capture_notice.clone().unwrap_or_default()),
            )
            .attr("class", "t-readout"),
            el("div", retained).attr("class", "transport"),
            body,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::Lens;
    use woodshedding::rehearsal::{Hold, Set};

    fn ready_ui() -> UiState {
        let mut ui = UiState::new();
        ui.song.bars.push(Default::default());
        ui.stage.set_lens(Lens::Chords);
        let mut card = ui.stage.card_from_lens().unwrap();
        card.timing.hold = Hold::Bars(2);
        ui.set.push(card);
        ui.loop_working_set(ui.working_sets.active_id);
        ui
    }

    #[test]
    fn wav_export_controls_require_host_collision_token_and_reset_with_session() {
        use cambium_genet_winit_host::Harness;
        use taproot::Selector;

        let mut ui = ready_ui();
        ui.select_app_section(woodshed_core::storage::AppSection::Looper);
        ui.set_viewport_width(1100.0);
        ui.set_viewport_height(900.0);
        let logic: Box<dyn FnMut(&UiState) -> UiChild> = Box::new(super::super::stage_root);
        let mut h = Harness::new(crate::theme::slate_stage_css(), ui, logic);
        h.layout_at(1100.0, 900.0);
        let export = Selector::class("t-btn").containing("Export WAV");
        let replace = Selector::class("t-btn").containing("Replace WAV");
        assert!(!h.click_on(&replace));
        assert!(h.click_on(&export));
        let _ = h.click_on(&export);
        assert_eq!(
            h.state().loop_export_requests,
            [LoopExportRequest::ChooseDestination]
        );

        h.update(|ui| {
            ui.loop_export_requests.clear();
            ui.loop_export_busy = true;
            ui.loop_export_notice = Some("Choosing WAV destination…".into());
        });
        let _ = h.click_on(&export);
        assert!(h.state().loop_export_requests.is_empty());
        h.update(|ui| {
            ui.loop_export_busy = false;
            ui.loop_export_replace = Some((73, "practice.wav".into()));
            ui.loop_export_notice = Some("Choose Replace WAV to replace the existing file.".into());
        });
        let _ = h.click_on(&export);
        assert!(h.state().loop_export_requests.is_empty());
        assert!(h.click_on(&replace));
        assert_eq!(
            h.state().loop_export_requests,
            [LoopExportRequest::Replace(73)]
        );
        let _ = h.click_on(&replace);
        assert_eq!(h.state().loop_export_requests.len(), 1);

        h.update(|ui| {
            ui.loop_export_requests.clear();
            ui.loop_export_replace = Some((91, "second.wav".into()));
        });
        assert!(h.click_on(&Selector::class("t-btn").containing("Cancel")));
        assert_eq!(
            h.state().loop_export_requests,
            [LoopExportRequest::CancelReplace(91)]
        );
        h.update(|ui| {
            ui.loop_export_requests.clear();
            ui.loop_export_replace = None;
            ui.loop_export_notice = Some("Export cancelled.".into());
        });
        assert!(h.click_on(&export));
        assert_eq!(
            h.state().loop_export_requests,
            [LoopExportRequest::ChooseDestination]
        );

        h.update(|ui| {
            ui.loop_export_busy = true;
            ui.loop_export_replace = Some((117, "never-persisted.wav".into()));
            ui.loop_export_notice = Some("Write refused; previous WAV retained.".into());
            let persisted = ui.to_persisted();
            let json = serde_json::to_value(&persisted).unwrap();
            for key in [
                "loop_export_requests",
                "loop_export_busy",
                "loop_export_notice",
                "loop_export_replace",
            ] {
                assert!(json.get(key).is_none());
            }
            ui.apply_persisted(&persisted, Default::default());
        });
        assert!(h.state().loop_export_requests.is_empty());
        assert!(!h.state().loop_export_busy);
        assert!(h.state().loop_export_notice.is_none());
        assert!(h.state().loop_export_replace.is_none());
    }

    #[test]
    fn source_stays_with_its_set_while_exploration_and_rehearsal_choose_other_owners() {
        let mut ui = ready_ui();
        let owner = ui.working_sets.active_id;
        let plan = ui.requested_loop_plan().unwrap();
        let card = plan.segments[0].id;
        let mut other = Set::default();
        let mut copy = ui.set.cards[0].clone();
        copy.label = "other material".into();
        other.push(copy);
        let other_owner = ui.working_sets.create(&mut ui.set, "Other", other);
        ui.rehearsal_owner = Some(other_owner);
        assert_eq!(ui.requested_loop_plan().unwrap(), plan);
        ui.open_loop_card(card);
        assert_eq!(ui.working_sets.active_id, owner);
        assert_eq!(ui.set.cursor_id(), Some(card.card));
        assert_eq!(ui.rehearsal_owner, Some(other_owner));
        assert_eq!(ui.looper.source, LooperSource::Set(owner));
    }

    #[test]
    fn selection_and_queued_capture_keep_identity_across_reorder() {
        let mut ui = ready_ui();
        let mut second = ui.set.cards[0].clone();
        second.label = "second".into();
        ui.set.push(second);
        ui.loop_admitted = Some(ui.requested_loop_plan().unwrap());
        let id = ui.loop_admitted.as_ref().unwrap().segments[0].id;
        ui.loop_selected = Some(id);
        ui.request(AudioRequest::LoopClear(id));
        ui.set.cards.reverse();
        ui.loop_admitted = Some(ui.requested_loop_plan().unwrap());
        assert_eq!(ui.selected_loop_segment(), Some(id));
        assert_eq!(ui.loop_admitted.as_ref().unwrap().index_of(id), Some(1));
        assert_eq!(ui.audio_requests, vec![AudioRequest::LoopClear(id)]);
    }

    #[test]
    fn persisted_source_reopens_without_copying_or_revoicing_legacy_song() {
        let mut ui = ready_ui();
        ui.looper.beats = 7;
        ui.looper.inherited_bpm = 137.25;
        ui.song.bars[0].pitches_hz = vec![253.125, 381.777];
        ui.song.bars[0].beats = 3;
        ui.song.bars[0].length = 4;
        let expected_song = ui.song.clone();
        let expected_plan = ui.requested_loop_plan().unwrap();
        let json = serde_json::to_string(&ui.to_persisted()).unwrap();
        let session: woodshed_core::storage::PersistedSession =
            serde_json::from_str(&json).unwrap();
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, Default::default());
        assert_eq!(reopened.looper, ui.looper);
        assert_eq!(reopened.requested_loop_plan().unwrap(), expected_plan);
        assert_eq!(reopened.song, expected_song);
        assert!(reopened.loop_admitted.is_none());
        assert!(!reopened.song_playing);
        let mut old = serde_json::to_value(&session).unwrap();
        old.as_object_mut().unwrap().remove("looper");
        let legacy = serde_json::from_value(old).unwrap();
        reopened.apply_persisted(&legacy, Default::default());
        assert_eq!(reopened.looper.source, LooperSource::SavedSong);
        assert_eq!(reopened.song, expected_song);
        assert!(
            serde_json::to_value(reopened.to_persisted())
                .unwrap()
                .get("looper")
                .is_none()
        );
    }
}
