//! The listen tab body: Up next beside Notes. Lane S2 owns this file.

use super::rows;
use crate::{
    CompactCommand, FullView, ItemRow, NoteSummary, RedshankSurfaceState, format_span, format_time,
};
use cambium::{TextInput, button, el, lens, text, textarea};
use redshank_model::ItemId;

/// Time left in the queue, for the `3 · 1h 42m` summary.
fn remaining_ms(state: &RedshankSurfaceState) -> u64 {
    state
        .queue
        .iter()
        .filter_map(|id| state.item(id))
        .map(|item| match item.duration_ms {
            Some(duration) if !item.completed => duration.saturating_sub(item.position_ms),
            _ => 0,
        })
        .sum()
}

fn queue_row(state: &RedshankSurfaceState, index: usize, id: &ItemId) -> FullView {
    let item = state.item(id);
    let title = item
        .map(|item| item.title.clone())
        .unwrap_or_else(|| id.0.clone());
    let playing = state
        .compact
        .now_playing
        .as_ref()
        .is_some_and(|now| &now.item_id == id);
    let listened = item.map(ItemRow::listened).unwrap_or(0.0);
    let completed = item.is_some_and(|item| item.completed);
    let source = item.map(|item| item.source).unwrap_or_default();
    let default_face = crate::Face::default();
    let face = item.map(|item| &item.face).unwrap_or(&default_face);

    let select_id = id.clone();
    let select_title = title.clone();
    let title_button = Box::new(
        button(title.clone(), move |state: &mut RedshankSurfaceState, _| {
            state.request(CompactCommand::SelectItem(select_id.clone()));
        })
        .attr("class", "rs-listen-title")
        .attr("aria-label", format!("Select {select_title}")),
    ) as FullView;

    let last = index + 1 == state.queue.len();
    let up_title = title.clone();
    let up = Box::new(
        button("Up", move |state: &mut RedshankSurfaceState, _| {
            if index > 0 {
                state.request(CompactCommand::MoveQueue {
                    from: index,
                    to: index - 1,
                });
            }
        })
        .attr("class", "rs-row-action")
        .attr("aria-label", format!("Move {up_title} up"))
        .attr("aria-disabled", if index == 0 { "true" } else { "false" })
        .attr("tabindex", if index == 0 { "-1" } else { "0" }),
    ) as FullView;
    let down_title = title.clone();
    let down = Box::new(
        button("Down", move |state: &mut RedshankSurfaceState, _| {
            if index + 1 < state.queue.len() {
                state.request(CompactCommand::MoveQueue {
                    from: index,
                    to: index + 1,
                });
            }
        })
        .attr("class", "rs-row-action")
        .attr("aria-label", format!("Move {down_title} down"))
        .attr("aria-disabled", if last { "true" } else { "false" })
        .attr("tabindex", if last { "-1" } else { "0" }),
    ) as FullView;
    let remove = rows::action(
        "Remove",
        format!("Remove {title} from queue"),
        "rs-row-action",
        CompactCommand::Dequeue(id.clone()),
    );
    let offline = if item.is_some_and(|item| item.cached_bytes.is_some()) {
        rows::action(
            "Remove download",
            format!("Remove offline download for {title}"),
            "rs-row-action",
            CompactCommand::RemoveCachedItem(id.clone()),
        )
    } else {
        rows::action(
            "Download",
            format!("Download {title} for offline listening"),
            "rs-row-action",
            CompactCommand::CacheItem(id.clone()),
        )
    };

    Box::new(
        el(
            "div",
            (
                rows::face(face, "rs-listen-face"),
                el(
                    "div",
                    (
                        title_button,
                        rows::bar(listened, completed, "rs-listen-bar"),
                    ),
                )
                .attr("class", "rs-listen-cell"),
                rows::micro(source.badge()),
                el("span", (up, down, remove, offline)).attr("class", "rs-row-actions"),
            ),
        )
        .attr(
            "class",
            if playing {
                "rs-row rs-row-active rs-listen-row"
            } else {
                "rs-row rs-listen-row"
            },
        ),
    )
}

/// `Note for {title} at m:ss`, or `m:ss → m:ss` for a span.
fn capture_label(state: &RedshankSurfaceState, capture: &crate::TextCapture) -> String {
    let title = state
        .item(&capture.anchor.item_id)
        .map(|item| item.title.clone())
        .unwrap_or_else(|| capture.anchor.item_id.0.clone());
    match capture.end_offset_ms {
        Some(end) => format!(
            "Note for {title} at {} → {}",
            format_time(capture.anchor.offset_ms),
            format_time(end)
        ),
        None => format!(
            "Note for {title} at {}",
            format_time(capture.anchor.offset_ms)
        ),
    }
}

/// The text editor card, shown only while a capture is open.
fn editor(state: &RedshankSurfaceState) -> Option<FullView> {
    let capture = state.text_capture.as_ref()?;
    let anchor = capture.anchor.clone();
    let end_offset_ms = capture.end_offset_ms;
    let save = Box::new(
        button("Save", move |state: &mut RedshankSurfaceState, _| {
            let plain_text = state.text_editor.text().to_owned();
            if let Some(id) = state.editing_note.clone() {
                state.request(CompactCommand::EditNote { id, plain_text });
            } else if let Some(end_offset_ms) = end_offset_ms {
                state.request(CompactCommand::SaveSpanNote {
                    anchor: anchor.clone(),
                    end_offset_ms,
                    plain_text,
                });
            } else {
                state.request(CompactCommand::SaveTextNote {
                    anchor: anchor.clone(),
                    plain_text,
                });
            }
        })
        .attr("class", "rs-listen-save")
        .attr("aria-label", "Save text note")
        .attr("aria-keyshortcuts", "Control+Enter"),
    ) as FullView;
    let cancel = rows::action(
        "Cancel",
        "Cancel text note",
        "rs-row-action",
        CompactCommand::CancelTextNote,
    );
    let field = Box::new(
        el(
            "div",
            Box::new(lens(
                |input: &mut TextInput| textarea(input),
                |state: &mut RedshankSurfaceState| &mut state.text_editor,
            )) as FullView,
        )
        .attr("id", "redshank-text-editor")
        .attr("class", "rs-listen-editor-field"),
    ) as FullView;
    Some(Box::new(
        el(
            "div",
            (
                el("p", text(capture_label(state, capture)))
                    .attr("aria-label", "Captured note target"),
                field,
                el("div", (save, cancel)).attr("class", "rs-row-actions"),
            ),
        )
        .attr("class", "rs-card rs-listen-editor"),
    ))
}

fn notes_for_item(state: &RedshankSurfaceState) -> Vec<&NoteSummary> {
    match state.selected_item() {
        Some(item) => state
            .notes
            .iter()
            .filter(|note| note.item_id == item.id)
            .collect(),
        None => state.notes.iter().collect(),
    }
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let queue: Vec<FullView> = state
        .queue
        .iter()
        .enumerate()
        .map(|(index, id)| queue_row(state, index, id))
        .collect();
    let queue_body: Vec<FullView> = if queue.is_empty() {
        vec![rows::empty_row("Your queue is empty.")]
    } else {
        queue
    };
    let up_next = el(
        "section",
        (
            rows::section_head(
                "UP NEXT",
                format!(
                    "{} · {}",
                    state.queue.len(),
                    format_span(remaining_ms(state))
                ),
            ),
            queue_body,
        ),
    )
    .attr("class", "rs-listen-queue")
    .attr("aria-label", "Up next");

    let notes = notes_for_item(state);
    let owned: Vec<NoteSummary> = notes.into_iter().cloned().collect();
    let title = state
        .selected_item()
        .map(|item| item.title.to_uppercase())
        .unwrap_or_else(|| "NO EPISODE".into());
    let note_rows: Vec<FullView> = if owned.is_empty() {
        vec![rows::empty_row("No notes for this episode yet.")]
    } else {
        owned.iter().map(rows::note_row).collect()
    };
    let notes_section = el(
        "section",
        (
            rows::section_head(&format!("NOTES · {title}"), rows::note_mix(&owned)),
            note_rows,
            editor(state),
        ),
    )
    .attr("class", "rs-listen-notes")
    .attr("aria-label", "Notes");

    Box::new(el("section", (up_next, notes_section)).attr("class", "rs-panel rs-listen"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::tests_support::playing;
    use crate::tabs::tests_support::{click, commands, markup, runner};
    use crate::{Face, NoteSummaryBody, SourceKind, TextCapture, VoiceNotePreview};
    use redshank_model::{AnnotationId, CaptureAnchor};

    fn item(id: &str, title: &str) -> ItemRow {
        ItemRow {
            id: ItemId(id.into()),
            title: title.into(),
            feed_url: None,
            feed_title: None,
            face: Face::Tag("m4a".into()),
            source: SourceKind::Local,
            duration_ms: Some(120_000),
            position_ms: 80_400,
            completed: false,
            published: None,
            cached_bytes: None,
            note_count: 0,
            unavailable: None,
        }
    }

    fn queued() -> RedshankSurfaceState {
        let mut state = RedshankSurfaceState {
            compact: playing(),
            ..Default::default()
        };
        state.items.push(item("episode-42", "Wetland"));
        state.items.push(ItemRow {
            source: SourceKind::Cloud,
            position_ms: 0,
            ..item("second", "Episode 214")
        });
        state.queue = vec![ItemId("episode-42".into()), ItemId("second".into())];
        state
    }

    #[test]
    fn queue_rows_carry_badge_listened_width_and_the_active_ring() {
        let markup = markup(queued(), panel);
        assert!(markup.contains("rs-row-active"));
        assert!(markup.contains(">LOCAL<"));
        assert!(markup.contains(">CLOUD<"));
        assert!(markup.contains("width:67%"));
        assert!(markup.contains("2 · 2m"));
    }

    #[test]
    fn empty_queue_and_notes_show_quiet_rows() {
        let markup = markup(RedshankSurfaceState::default(), panel);
        assert!(markup.contains("Your queue is empty."));
        assert!(markup.contains("No notes for this episode yet."));
    }

    #[test]
    fn queue_rows_select_and_reorder() {
        let mut runner = runner(queued(), panel);
        click(&mut runner, "Move Wetland up");
        assert!(commands(&mut runner).is_empty());
        click(&mut runner, "Move Wetland down");
        click(&mut runner, "Select Wetland");
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::MoveQueue { from: 0, to: 1 },
                CompactCommand::SelectItem(ItemId("episode-42".into())),
            ]
        );
    }

    #[test]
    fn queue_row_offers_download_then_removal() {
        let mut runner = runner(queued(), panel);
        click(&mut runner, "Download Wetland for offline listening");
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::CacheItem(ItemId("episode-42".into()))]
        );
        runner.update(|state| state.items[0].cached_bytes = Some(1024));
        click(&mut runner, "Remove offline download for Wetland");
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::RemoveCachedItem(ItemId(
                "episode-42".into()
            ))]
        );
    }

    fn with_notes() -> RedshankSurfaceState {
        let mut state = queued();
        state.notes.push(NoteSummary {
            id: AnnotationId("text-one".into()),
            item_id: ItemId("episode-42".into()),
            offset_ms: 46_000,
            end_offset_ms: None,
            body: NoteSummaryBody::Text("so worried I can't speak".into()),
            private: true,
        });
        state.notes.push(NoteSummary {
            id: AnnotationId("voice-one".into()),
            item_id: ItemId("episode-42".into()),
            offset_ms: 76_000,
            end_offset_ms: None,
            body: NoteSummaryBody::Voice {
                duration_ms: 6_000,
                preview: VoiceNotePreview::Idle,
            },
            private: true,
        });
        state
    }

    #[test]
    fn note_rows_show_type_word_anchor_and_open_at() {
        let markup = markup(with_notes(), panel);
        assert!(markup.contains(">TEXT<"));
        assert!(markup.contains(">VOICE<"));
        assert!(markup.contains("Open at 0:46"));
        assert!(markup.contains("0:06 · your voice"));
        assert!(markup.contains("NOTES · WETLAND"));
        assert!(markup.contains("1 text · 1 voice"));
    }

    #[test]
    fn note_rows_open_edit_delete_and_play() {
        let mut runner = runner(with_notes(), panel);
        click(&mut runner, "Open note at 0:46");
        click(&mut runner, "Edit note at 0:46");
        click(&mut runner, "Delete note at 0:46");
        click(&mut runner, "Play voice note");
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::OpenNote(AnnotationId("text-one".into())),
                CompactCommand::BeginEditNote(AnnotationId("text-one".into())),
                CompactCommand::DeleteNote(AnnotationId("text-one".into())),
                CompactCommand::PlayVoiceNote(AnnotationId("voice-one".into())),
            ]
        );
    }

    #[test]
    fn playing_voice_note_reports_progress_and_stops() {
        let mut state = with_notes();
        state.notes[1].body = NoteSummaryBody::Voice {
            duration_ms: 6_000,
            preview: VoiceNotePreview::Playing { position_ms: 2_000 },
        };
        let mut runner = runner(state, panel);
        assert!(markup_of(&runner).contains("0:02 / 0:06"));
        click(&mut runner, "Stop voice note");
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::StopVoiceNote(AnnotationId(
                "voice-one".into()
            ))]
        );
    }

    fn markup_of(runner: &crate::tabs::tests_support::Runner) -> String {
        runner.dom().borrow().outer_html(runner.root())
    }

    fn capture_state(end_offset_ms: Option<u64>) -> RedshankSurfaceState {
        let mut state = queued();
        state.text_capture = Some(TextCapture {
            anchor: CaptureAnchor {
                item_id: ItemId("episode-42".into()),
                offset_ms: 62_000,
                end_offset_ms,
                pressed_offset_ms: None,
                representation: redshank_model::RepresentationReceipt {
                    complete_digest: Some("blake3:frozen".into()),
                    ..Default::default()
                },
            },
            draft: String::new(),
            end_offset_ms,
        });
        state.set_text_draft("typed after capture");
        state
    }

    #[test]
    fn editor_is_hidden_until_capture_then_saves_the_frozen_anchor() {
        assert!(!markup(queued(), panel).contains("<textarea"));
        let mut runner = runner(capture_state(None), panel);
        assert!(markup_of(&runner).contains("Note for Wetland at 1:02"));
        click(&mut runner, "Save text note");
        let saved = commands(&mut runner);
        assert!(matches!(
            &saved[0],
            CompactCommand::SaveTextNote { plain_text, anchor }
                if plain_text == "typed after capture"
                    && anchor.representation.complete_digest.as_deref() == Some("blake3:frozen")
        ));
    }

    #[test]
    fn span_capture_saves_a_span_note() {
        let mut runner = runner(capture_state(Some(90_000)), panel);
        assert!(markup_of(&runner).contains("1:02 → 1:30"));
        click(&mut runner, "Save text note");
        assert!(matches!(
            &commands(&mut runner)[0],
            CompactCommand::SaveSpanNote { end_offset_ms, .. } if *end_offset_ms == 90_000
        ));
    }
}
