pub mod recorder;
pub use recorder::{RecorderBuffer, RecorderObservation};

use cambium::{clickable, el, text};
use woodshed_core::audio::AudioRequest;
use woodshed_core::loop_plan::{LoopPlan, LoopReadiness, LoopSegmentId, LooperSource};
use woodshed_core::song::{SECTION_LABELS, song_from_progression};
use woodshed_core::working_sets::WorkingSetId;

use super::{LoopExportRequest, UiChild, UiState};

fn song_deck(ui: &UiState) -> UiChild {
    Box::new(
        el(
            "div",
            (
                clickable(
                    el("div", text(if ui.song.one_shot { "Once" } else { "Loop" }))
                        .attr("class", "t-btn"),
                    |ui: &mut UiState, _| ui.song.one_shot = !ui.song.one_shot,
                ),
                clickable(
                    el("div", text("From progression")).attr("class", "t-btn"),
                    |ui: &mut UiState, _| {
                        if ui.song_recording {
                            ui.loop_notice = Some(
                                "Stop recording / armed capture before replacing saved backing."
                                    .into(),
                            );
                            return;
                        }
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
                    text(format!("{} · {} bars", ui.song.name, ui.song.bars.len())),
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

/// Per-bar chord / tempo / meter / section editor for the cursor bar.
fn song_bar_editor(ui: &UiState) -> UiChild {
    let cursor = ui
        .song_edit_cursor
        .min(ui.song.bars.len().saturating_sub(1));
    let bar = &ui.song.bars[cursor];
    let root_label = if bar.is_silent() {
        "silent".to_string()
    } else {
        bar.root_name()
    };
    let chord_label = if bar.is_silent() {
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
    let editor: UiChild = if ui.song.is_empty() {
        Box::new(el("div", text("Add a bar or choose From progression.")))
    } else {
        song_bar_editor(ui)
    };
    Box::new(el(
        "div",
        (song_deck(ui), song_bar_ops(ui), editor, recorder_mount(ui)),
    ))
}

fn recorder_mount(ui: &UiState) -> UiChild {
    Box::new(
        el("div", recorder::surface(ui))
            .attr("class", "woodshed-recorder-mount")
            .attr(
                "style",
                format!(
                    "height: {}px; min-height: 340px;",
                    (ui.viewport_h - 400.0).clamp(340.0, 500.0)
                ),
            ),
    )
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

    /// Navigate from a retained take into the Looper. Only the requested
    /// backing and lane selection change; the take, its PCM and the admitted
    /// plan are left to the host. A missing owner is reported, not retargeted.
    pub fn open_loop_take(&mut self, id: LoopSegmentId) {
        self.select_app_section(woodshed_core::storage::AppSection::Looper);
        if self
            .working_sets
            .get(id.set, &self.set)
            .and_then(|set| set.card(id.card))
            .is_none()
        {
            self.loop_notice = Some(format!(
                "The take for Set {} Card {} has no current Card. It is kept and not reassigned; clear it below if it is no longer wanted.",
                id.set.0, id.card.0
            ));
            return;
        }
        // Refuses (and explains) while capture is armed for another source.
        self.loop_working_set(id.set);
        if self.looper.source != LooperSource::Set(id.set) {
            return;
        }
        self.loop_selected = Some(id);
        if self
            .loop_admitted
            .as_ref()
            .is_some_and(|plan| plan.index_of(id).is_some())
        {
            return;
        }
        self.loop_notice = Some(
            if self
                .requested_loop_plan()
                .is_ok_and(|plan| plan.index_of(id).is_some())
            {
                "Loading this take's Set. Its recorder lane appears once the backing is admitted."
            } else {
                "This take's Card is not in the Set's loop-ready backing. The take is kept; use Card setup to resolve it."
            }
            .into(),
        );
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
            clickable(el("div",text(if ui.looper.one_shot {"Once"}else{"Loop"})).attr("class","t-btn").attr("role","button").attr("data-action","toggle-set-one-shot"),|ui:&mut UiState,_| {if !ui.song_recording {ui.looper.one_shot=!ui.looper.one_shot;}}),
            clickable(el("div",text(format!("Clock: {}/4",ui.looper.beats))).attr("class","t-btn"),|ui:&mut UiState,_| {
                if ui.song_recording {ui.loop_notice=Some("Stop recording / armed capture before changing meter.".into());return;}
                let meters=[2,3,4,5,7];let index=meters.iter().position(|beats|*beats==ui.looper.beats).unwrap_or(1);ui.looper.beats=meters[(index+1)%meters.len()];
            }),
            clickable(el("div",text("Card setup")).attr("class","t-btn"),|ui:&mut UiState,_| {if let Some(id)=ui.selected_loop_segment(){ui.open_loop_card(id);}}),
            el("div",text(loaded)).attr("class","t-readout"),
        )).attr("class","transport"),
        el("div",issues).attr("class","transport"),
        recorder_mount(ui),
        el("div",text("Ordered Cards keep their own tempo and Bars or Seconds hold. Clear a captured mix before changing its clock. Stored material and WAV export stay with the owning Set/Card.")).attr("class","scale-name"),
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
            let session = ui.audio_session.clone();
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
                move |ui: &mut UiState, _| {
                    if std::sync::Arc::ptr_eq(&session, &ui.audio_session) {
                        ui.request(AudioRequest::LoopClear(id));
                    } else {
                        ui.loop_notice = Some(
                            "The capture session has changed. Use its current controls.".into(),
                        );
                    }
                },
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
#[path = "looper/tests.rs"]
mod tests;
