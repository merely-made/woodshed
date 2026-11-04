use super::*;

/// Shared retained-Card fretboard used in Stage and Rehearsal.
/// It never substitutes the live catalog selection for the saved instruction.
pub(in super::super) fn card_board(ui: &UiState) -> UiChild {
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
                    Box::new(el("div", super::super::note_card(d, string_count)).attr(
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
                    super::super::board_viewport_style(geom.orientation, w, h, ui.viewport_h),
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
