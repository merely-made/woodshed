use super::*;

pub fn screen(ui: &UiState) -> UiChild {
    if ui.music_editor.legacy && ui.set.music.is_none() && ui.music_draft().is_none() {
        return super::super::arrangement_editor::screen(ui);
    }
    let mut children: Vec<UiChild> = vec![Box::new(el(
        "h2",
        text(format!("Arrangement · {}", ui.working_sets.active_name)),
    ))];
    if let Some(notice) = &ui.music_editor.notice {
        children.push(Box::new(
            el("p", text(notice.clone()))
                .attr("role", "status")
                .attr("class", "music-notice"),
        ));
    }
    let Some(d) = ui.music_draft() else {
        if ui.set.music.is_some() {
            children.push(action(
                "Export preserved MusicXML",
                "music-export",
                UiState::export_music,
            ));
        } else {
            children.push(Box::new(el("p",text("Write music from this Set, or start a rhythmic exercise. Chart, tab and score follow the same music."))));
            children.push(action(
                if ui
                    .set
                    .progression_arrangement
                    .as_ref()
                    .is_some_and(|a| !a.alignment.time_signatures.is_empty())
                {
                    "Write saved arrangement as music"
                } else {
                    "Write each Card as a 4/4 bar"
                },
                "music-from-set",
                UiState::write_set_music,
            ));
            children.push(Box::new(el("h3", text("New rhythmic exercise"))));
            children.push(Box::new(
                el(
                    "div",
                    vec![
                        action("Start in 4/4", "music-new-four", |ui| {
                            ui.new_music_exercise(4, 4)
                        }),
                        action("Start in 3/4", "music-new-three", |ui| {
                            ui.new_music_exercise(3, 4)
                        }),
                        action("Start in 6/8", "music-new-six", |ui| {
                            ui.new_music_exercise(6, 8)
                        }),
                    ],
                )
                .attr("class", "music-commands"),
            ));
            if !ui.set.cards.is_empty()
                || ui.set.progression_arrangement.is_some()
                || !ui.arrangement_editor.drafts.is_empty()
            {
                children.push(action(
                    "Open existing Card alignment",
                    "music-legacy",
                    |ui| ui.music_editor.legacy = true,
                ));
            }
        }
        return Box::new(el("section", children).attr("class", "music-editor"));
    };
    let mut toolbar: Vec<UiChild> = vec![];
    for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
        toolbar.push(Box::new(clickable(
            el("button", text(view_label(view)))
                .attr("class", "t-btn music-view")
                .attr("aria-pressed", (d.view == view).to_string())
                .attr("data-music-view", view_label(view)),
            move |ui: &mut UiState, _| ui.set_music_view(view),
        )));
    }
    match ui.music_editor.practice.status() {
        practice::PracticeStatus::Playing => {
            toolbar.push(action(
                "Pause",
                "music-practice-pause",
                UiState::pause_music_practice,
            ));
            toolbar.push(action(
                "Stop",
                "music-practice-stop",
                UiState::stop_music_practice,
            ));
        },
        practice::PracticeStatus::Paused => {
            toolbar.push(action(
                "Resume",
                "music-practice-resume",
                UiState::resume_music_practice,
            ));
            toolbar.push(action(
                "Stop",
                "music-practice-stop",
                UiState::stop_music_practice,
            ));
        },
        _ => toolbar.push(action(
            "Practice arrangement",
            "music-practice-play",
            UiState::practice_music,
        )),
    }
    toolbar.push(action("Undo", "music-undo", UiState::music_undo));
    toolbar.push(action("Redo", "music-redo", UiState::music_redo));
    toolbar.push(action("Save", "music-save", UiState::save_music));
    toolbar.push(action(
        "Export MusicXML…",
        "music-export",
        UiState::export_music,
    ));
    children.push(Box::new(el("div", toolbar).attr("class", "music-commands")));
    let mut practice_controls: Vec<UiChild> = vec![
        action("Slower", "music-practice-slower", |ui| {
            ui.set_music_practice_tempo((ui.music_practice_tempo() - 4.0).clamp(1.0, 400.0))
        }),
        Box::new(el(
            "span",
            text(format!(
                "Practice {} BPM · written {} BPM",
                ui.music_practice_tempo(),
                d.score.tempo
            )),
        )),
        action("Faster", "music-practice-faster", |ui| {
            ui.set_music_practice_tempo((ui.music_practice_tempo() + 4.0).clamp(1.0, 400.0))
        }),
        action(
            "Match written tempo",
            "music-practice-match-tempo",
            UiState::match_music_tempo,
        ),
    ];
    for (label, mode) in [
        ("Off", woodshedding::rehearsal::LoopMode::Off),
        ("Arrangement", woodshedding::rehearsal::LoopMode::All),
        ("Bar", woodshedding::rehearsal::LoopMode::Card),
    ] {
        practice_controls.push(Box::new(clickable(
            el("button", text(format!("Repeat: {label}")))
                .attr("class", format!("t-btn music-repeat music-repeat-{label}"))
                .attr("aria-pressed", (ui.set.loop_mode == mode).to_string()),
            move |ui: &mut UiState, _| ui.set.loop_mode = mode,
        )));
    }
    if ui.music_editor.practice.pending_update() {
        practice_controls.push(Box::new(
            el("span", text("Edits queued for next bar")).attr("role", "status"),
        ));
    }
    children.push(Box::new(
        el("div", practice_controls).attr("class", "music-commands"),
    ));
    let mut follow_controls = vec![action(
        if ui.music_editor.follow_practice {
            "Edit selection"
        } else {
            "Follow playback"
        },
        "music-follow-playback",
        |ui| ui.music_editor.follow_practice = !ui.music_editor.follow_practice,
    )];
    if ui.music_editor.follow_practice {
        follow_controls.push(action("Whole arrangement", "music-follow-all", |ui| {
            ui.music_editor.follow_range = false
        }));
        follow_controls.push(action("Selected bar", "music-follow-selected", |ui| {
            if let Some(bar) = ui.music_draft().map(|d| d.selected.0) {
                ui.music_editor.follow_first.selected = bar;
                ui.music_editor.follow_last.selected = bar;
                ui.music_editor.follow_range = true;
            }
        }));
        let options = (1..=d.score.measures.len())
            .map(|b| format!("Bar {b}"))
            .collect::<Vec<_>>();
        follow_controls.push(Box::new(el("span", text("From"))));
        follow_controls.push(Box::new(map_state(
            select(
                &ui.music_editor.follow_first,
                &options.iter().map(String::as_str).collect::<Vec<_>>(),
            ),
            |ui: &mut UiState| &mut ui.music_editor.follow_first,
        )));
        follow_controls.push(Box::new(el("span", text("through"))));
        follow_controls.push(Box::new(map_state(
            select(
                &ui.music_editor.follow_last,
                &options.iter().map(String::as_str).collect::<Vec<_>>(),
            ),
            |ui: &mut UiState| &mut ui.music_editor.follow_last,
        )));
        follow_controls.push(action("Use bars", "music-follow-range", |ui| {
            ui.music_editor.follow_range = true
        }));
    }
    children.push(Box::new(
        el("div", follow_controls).attr("class", "music-commands"),
    ));
    let (b, n) = d.selected;
    let measure = &d.score.measures[b];
    let event = &measure.events[n];
    children.push(Box::new(
        el(
            "div",
            vec![
                Box::new(el("h3", text(d.score.title.clone()))) as UiChild,
                Box::new(
                    el(
                        "span",
                        text(format!(
                            "{} · {} strings · {}/{} · {} BPM · {}",
                            d.score.instrument,
                            d.score.tuning.len(),
                            measure.meter.beats,
                            measure.meter.beat_type,
                            d.score.tempo,
                            if d.dirty() { "Draft" } else { "Saved" }
                        )),
                    )
                    .attr("class", "music-quiet"),
                ),
            ],
        )
        .attr("class", "music-heading"),
    ));
    if let Some(leaf) = notation_leaf(ui) {
        let size = leaf.intrinsic_size();
        let mut score: Vec<UiChild> = vec![Box::new(custom_leaf::<UiState, ()>(
            crate::notation_leaf::NOTATION_LEAF_KEY,
            size.width.ceil() as u32,
            size.height.ceil() as u32,
        ))];
        for label in leaf.labels() {
            score.push(Box::new(
                el("span", text(label.text))
                    .attr("class", "music-score-label")
                    .attr(
                        "style",
                        format!("position:absolute;left:{}px;top:{}px", label.x, label.y),
                    ),
            ));
        }
        for (bar, note, (x, y, w, h)) in leaf.event_rects() {
            if ui.music_preview_active() {
                continue;
            }
            let e = &d.score.measures[bar].events[note];
            let pitches = e
                .notes
                .iter()
                .filter_map(|p| d.score.pitch(*p).ok())
                .map(note_name)
                .collect::<Vec<_>>()
                .join(" ");
            let name = format!(
                "Bar {}, {}, {}",
                bar + 1,
                if e.rest { "rest" } else { &pitches },
                e.value.label()
            );
            score.push(Box::new(clickable(
                el("button", ())
                    .attr("class", "music-note")
                    .attr("aria-label", name)
                    .attr("aria-pressed", (d.selected == (bar, note)).to_string())
                    .attr("data-music-bar", bar.to_string())
                    .attr("data-music-note", note.to_string())
                    .attr(
                        "style",
                        format!("position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px"),
                    ),
                move |ui: &mut UiState, _| ui.select_music(bar, note),
            )));
        }
        children.push(Box::new(
            el("div", score).attr("class", "music-sheet").attr(
                "style",
                format!(
                    "position:relative;width:{}px;height:{}px",
                    size.width, size.height
                ),
            ),
        ));
    }
    let pitches = event
        .notes
        .iter()
        .filter_map(|p| d.score.pitch(*p).ok())
        .map(note_name)
        .collect::<Vec<_>>()
        .join(" · ");
    children.push(Box::new(el(
        "h3",
        text(format!(
            "Bar {} · {} · {}",
            b + 1,
            if event.rest { "Rest" } else { &pitches },
            event.value.label()
        )),
    )));
    let mut commands = vec![action(
        if event.rest { "Make note" } else { "Make rest" },
        "music-rest",
        UiState::music_rest,
    )];
    if event.value == NoteValue::Quarter {
        commands.push(action("Split into eighths", "music-eighths", |ui| {
            ui.music_split(false)
        }));
        commands.push(action("Make triplet", "music-triplet", |ui| {
            ui.music_split(true)
        }));
    }
    if event.notes.len() == 1 && !event.rest {
        commands.push(action("Pitch −", "music-pitch-down", |ui| {
            ui.music_step_pitch(-1)
        }));
        commands.push(action("Pitch +", "music-pitch-up", |ui| {
            ui.music_step_pitch(1)
        }));
    }
    commands.push(action(
        "Sustain for bar",
        "music-sustain",
        UiState::music_sustain,
    ));
    commands.push(action(
        "Listen to selection",
        "music-listen",
        UiState::music_listen,
    ));
    children.push(Box::new(
        el("div", commands).attr("class", "music-commands"),
    ));
    children.push(action(
        if measure.harmony.is_some() {
            "Change written chord…"
        } else {
            "Add chord symbol…"
        },
        "music-change-chord",
        UiState::open_music_chord,
    ));
    if ui.music_editor.choose_chord {
        let roots = [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
        ];
        children.push(Box::new(
            el(
                "div",
                vec![
                    Box::new(map_state(
                        select(&ui.music_editor.chord_root, &roots),
                        |ui: &mut UiState| &mut ui.music_editor.chord_root,
                    )) as UiChild,
                    Box::new(map_state(
                        select(&ui.music_editor.chord_kind, &QUALITY_LABELS),
                        |ui: &mut UiState| &mut ui.music_editor.chord_kind,
                    )),
                    action(
                        "Use written chord",
                        "music-use-chord",
                        UiState::apply_music_chord,
                    ),
                    action("Cancel", "music-cancel-chord", |ui| {
                        ui.music_editor.choose_chord = false
                    }),
                ],
            )
            .attr("class", "music-commands"),
        ));
    }
    if let Some(symbol) = measure.harmony.as_ref() {
        let mut chords = vec![
            Box::new(el(
                "span",
                text(format!("Written chord: {}", symbol.label())),
            )) as UiChild,
            action("Previous voicing", "music-voicing-prev", |ui| {
                ui.music_voicing(-1)
            }),
            action("Next voicing", "music-voicing-next", |ui| {
                ui.music_voicing(1)
            }),
            action("Quarter notes ↑", "music-pick-up", |ui| {
                ui.preview_music_pattern(pattern::PickingPattern::QuarterAscending)
            }),
            action("Quarter notes ↓", "music-pick-down", |ui| {
                ui.preview_music_pattern(pattern::PickingPattern::QuarterDescending)
            }),
            action("Eighth notes ↑↓", "music-pick-eighths", |ui| {
                ui.preview_music_pattern(pattern::PickingPattern::EighthUpDown)
            }),
        ];
        if d.score.chord_mismatch(b) {
            chords.push(Box::new(el("p",text("These notes differ from the written chord. Keep the symbol, or choose a suggestion below.")).attr("class","music-mismatch").attr("role","status")));
            for suggestion in d.score.chord_suggestions(b) {
                chords.push(action(
                    format!("Use {}", suggestion.label()),
                    "music-chord-suggestion",
                    move |ui| ui.music_symbol(suggestion.clone()),
                ));
            }
        }
        children.push(Box::new(el("div", chords).attr("class", "music-commands")));
    }
    if let Some(preview) = ui.music_editor.picking_preview.as_ref() {
        children.push(Box::new(
            el(
                "p",
                text({
                    let m = &preview.score.measures[preview.selected.0];
                    let picks = m
                        .events
                        .iter()
                        .flat_map(|e| e.notes.iter())
                        .filter_map(|n| preview.score.pitch(*n).ok())
                        .map(note_name)
                        .collect::<Vec<_>>()
                        .join(" → ");
                    let used = preview
                        .source_voicing
                        .iter()
                        .filter(|n| m.events.iter().any(|e| e.notes.contains(n)))
                        .count();
                    format!(
                        "{} · {used} of {} chord tones · {picks}",
                        preview.pattern.label(),
                        preview.source_voicing.len()
                    )
                }),
            )
            .attr("role", "status")
            .attr("class", "music-pattern-summary")
            .attr("aria-label", preview.summary()),
        ));
        children.push(Box::new(
            el(
                "div",
                vec![
                    action(
                        "Apply picking pattern",
                        "music-pick-apply",
                        UiState::apply_music_pattern,
                    ),
                    action(
                        "Cancel preview",
                        "music-pick-cancel",
                        UiState::cancel_music_pattern,
                    ),
                ],
            )
            .attr("class", "music-commands"),
        ));
    }
    children.push(action(
        "Remove selected note",
        "music-remove-tone",
        UiState::music_remove_tone,
    ));
    if ui.music_editor.follow_practice {
        let score = ui.music_editor.practice.applied_score().unwrap_or(&d.score);
        let end = score.measures.len().saturating_sub(1);
        let (first, last) = if ui.music_editor.follow_range {
            let first = ui.music_editor.follow_first.selected.min(end);
            (
                first,
                ui.music_editor.follow_last.selected.min(end).max(first),
            )
        } else {
            (0, end)
        };
        children.push(playback_board::board(
            ui,
            score,
            ui.music_editor
                .practice
                .playhead()
                .map(|p| (p.bar_index, p.event_index)),
            first..=last,
        ));
    } else {
        children.push(board(ui, d));
    }
    children.push(Box::new(
        el(
            "div",
            vec![
                action("Written tempo −", "music-slower", |ui| ui.music_tempo(-4)),
                action("Written tempo +", "music-faster", |ui| ui.music_tempo(4)),
                action("Add bar", "music-add-bar", UiState::music_add_bar),
            ],
        )
        .attr("class", "music-commands"),
    ));
    Box::new(on_key(
        el("section", children).attr("class", "music-editor"),
        UiState::music_key,
    ))
}
