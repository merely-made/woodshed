use super::*;

/// The shared title bar owns native insets, drag regions and caption actions.
pub(super) fn desktop_chrome(commands: &WindowCommands, frame: WindowFrame) -> UiChild {
    if frame == WindowFrame::Host {
        return Box::new(String::new());
    }
    title_bar(
        Box::new(
            el("span", text_node("W"))
                .attr("class", "woodshed-mark")
                .attr("aria-hidden", "true"),
        ),
        Box::new(el("span", text_node("Woodshed")).attr("class", "chrome-title")),
        Box::new(String::new()),
        caption_controls_for_frame(commands, &CaptionLabels::default(), frame),
    )
}

pub(super) fn desktop_root(ui: &UiState, commands: &WindowCommands, frame: WindowFrame) -> UiChild {
    let retry = commands.clone();
    let discard = commands.clone();
    let decision = ui.practice_close_pending.then(|| {
        el(
            "section",
            (
                el("h3", text_node("Practice is not confirmed saved")),
                el(
                    "p",
                    text_node(
                        ui.practice_save_notice
                            .clone()
                            .unwrap_or_else(|| "Current changes remain in this window.".into()),
                    ),
                ),
                el(
                    "div",
                    (
                        clickable(
                            el("button", text_node("Retry save"))
                                .attr("class", "t-btn practice-close-retry"),
                            move |ui: &mut UiState, _| {
                                ui.practice_close_pending = false;
                                retry.close();
                            },
                        ),
                        clickable(
                            el("button", text_node("Discard unsaved practice and close"))
                                .attr("class", "t-btn practice-close-discard"),
                            move |ui: &mut UiState, _| {
                                ui.practice_discard_close = true;
                                discard.close();
                            },
                        ),
                        clickable(
                            el("button", text_node("Cancel"))
                                .attr("class", "t-btn practice-close-cancel"),
                            |ui: &mut UiState, _| {
                                ui.practice_close_pending = false;
                                ui.practice_discard_close = false;
                            },
                        ),
                    ),
                )
                .attr("class", "transport"),
            ),
        )
        .attr("class", "practice-close-panel")
        .attr("role", "alert")
    });
    let persona_decision = ui.pending_persona_switch.as_ref().map(|pending| {
        use woodshed_views::persona::SwitchDecision;
        el(
            "section",
            (
                el(
                    "h3",
                    text_node("Keep outgoing practice before switching persona"),
                ),
                el("p", text_node(pending.notice.clone())),
                clickable(
                    el("button", text_node("Retry save and switch"))
                        .attr("class", "t-btn persona-save-retry"),
                    |ui: &mut UiState, _| {
                        if let Some(pending) = &mut ui.pending_persona_switch {
                            pending.decision = Some(SwitchDecision::Retry);
                        }
                    },
                ),
                clickable(
                    el("button", text_node("Discard unsaved changes and switch"))
                        .attr("class", "t-btn persona-save-discard"),
                    |ui: &mut UiState, _| {
                        if let Some(pending) = &mut ui.pending_persona_switch {
                            pending.decision = Some(SwitchDecision::Discard);
                        }
                    },
                ),
                clickable(
                    el("button", text_node("Cancel switch"))
                        .attr("class", "t-btn persona-save-cancel"),
                    |ui: &mut UiState, _| {
                        if let Some(pending) = &mut ui.pending_persona_switch {
                            pending.decision = Some(SwitchDecision::Cancel);
                        }
                    },
                ),
            ),
        )
        .attr("class", "practice-close-panel")
        .attr("role", "alert")
    });
    let draft_close = commands.clone();
    let draft_decision = ui.arrangement_editor.close_pending.then(|| {
        el(
            "section",
            (
                el("h3", text_node("Arrangement drafts are not saved")),
                el("p", text_node("Keep editing to save each draft, or explicitly discard the drafts before closing.")),
                clickable(
                    el("button", text_node("Keep editing"))
                        .attr("class", "t-btn arrangement-close-cancel"),
                    |ui: &mut UiState, _| ui.arrangement_editor.close_pending = false,
                ),
                clickable(
                    el("button", text_node("Discard arrangement drafts and close"))
                        .attr("class", "t-btn arrangement-close-discard"),
                    move |ui: &mut UiState, _| {
                        ui.arrangement_editor.drafts.clear();
                        ui.music_editor.drafts.clear();
                        ui.arrangement_editor.close_pending = false;
                        draft_close.close();
                    },
                ),
            ),
        )
        .attr("class", "practice-close-panel")
        .attr("role", "alert")
    });
    Box::new(
        el(
            "div",
            (
                desktop_chrome(commands, frame),
                decision,
                persona_decision,
                draft_decision,
                stage_root(ui),
            ),
        )
        .attr("class", "desktop-frame"),
    )
}
