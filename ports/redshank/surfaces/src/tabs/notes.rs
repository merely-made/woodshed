//! The notes tab body: episode header, timeline strip, clusters and spans.
//! Lane S2 owns this file.

use super::{controls, rows};
use crate::{
    CompactCommand, FullView, NoteSummary, NotesFilter, RedshankSurfaceState, format_time, percent,
};
use cambium::{el, text};

/// Notes within this many milliseconds of a cluster's first note join it.
const CLUSTER_MS: u64 = 10_000;
/// Rows a cluster shows before the "more in this cluster" line.
const CLUSTER_ROWS: usize = 4;

fn visible(state: &RedshankSurfaceState) -> Vec<&NoteSummary> {
    match (state.notes_filter, state.selected_item()) {
        (NotesFilter::ThisEpisode, Some(item)) => state
            .notes
            .iter()
            .filter(|note| note.item_id == item.id)
            .collect(),
        _ => state.notes.iter().collect(),
    }
}

/// Point notes in ascending order, grouped into clusters.
fn clusters<'a>(notes: &[&'a NoteSummary]) -> Vec<Vec<&'a NoteSummary>> {
    let mut points: Vec<&NoteSummary> = notes
        .iter()
        .copied()
        .filter(|note| note.end_offset_ms.is_none())
        .collect();
    points.sort_by_key(|note| note.offset_ms);
    let mut out: Vec<Vec<&NoteSummary>> = Vec::new();
    for note in points {
        match out.last_mut() {
            Some(group) if note.offset_ms.saturating_sub(group[0].offset_ms) <= CLUSTER_MS => {
                group.push(note)
            },
            _ => out.push(vec![note]),
        }
    }
    out
}

fn header(state: &RedshankSurfaceState) -> FullView {
    let item = state.selected_item();
    let facts = match item {
        Some(item) => {
            let mut parts = Vec::new();
            if let Some(feed) = &item.feed_title {
                parts.push(feed.to_uppercase());
            }
            parts.push(item.source.badge().to_owned());
            if let Some(published) = &item.published {
                parts.push(if item.completed {
                    format!("COMPLETED {}", published.to_uppercase())
                } else {
                    published.to_uppercase()
                });
            } else if item.completed {
                parts.push("COMPLETED".to_owned());
            }
            parts.join(" · ")
        },
        None => "NO EPISODE SELECTED".to_owned(),
    };
    let title = item
        .map(|item| item.title.clone())
        .unwrap_or_else(|| "Nothing selected".into());
    let default_face = crate::Face::default();
    let face = item.map(|item| &item.face).unwrap_or(&default_face);
    let filter = controls::segment(
        "Notes filter",
        vec![
            (
                "this episode".to_owned(),
                state.notes_filter == NotesFilter::ThisEpisode,
                vec![CompactCommand::SetNotesFilter(NotesFilter::ThisEpisode)],
            ),
            (
                "all notes".to_owned(),
                state.notes_filter == NotesFilter::AllNotes,
                vec![CompactCommand::SetNotesFilter(NotesFilter::AllNotes)],
            ),
        ],
    );
    let export = rows::action(
        "Export W3C",
        "Export annotations as W3C Web Annotation JSON",
        "rs-row-action",
        CompactCommand::ExportAnnotations,
    );
    Box::new(
        el(
            "div",
            (
                rows::face(face, ""),
                el(
                    "div",
                    (
                        rows::micro(facts),
                        el("div", text(title)).attr("class", "rs-notes-title"),
                    ),
                )
                .attr("class", "rs-notes-identity"),
                filter,
                export,
            ),
        )
        .attr("class", "rs-notes-head"),
    )
}

/// Ticks, cluster chips, and span washes over the item's duration.
fn strip(state: &RedshankSurfaceState, notes: &[&NoteSummary]) -> FullView {
    let duration = state
        .selected_item()
        .and_then(|item| item.duration_ms)
        .or_else(|| {
            notes
                .iter()
                .map(|note| note.offset_ms)
                .max()
                .map(|max| max + 1)
        })
        .unwrap_or(1);
    let mut parts: Vec<FullView> = vec![Box::new(
        el("span", text("")).attr("class", "rs-notes-track"),
    )];
    for group in clusters(notes) {
        let left = percent(group[0].offset_ms, Some(duration));
        for note in &group {
            parts.push(Box::new(
                el("span", text("")).attr("class", "rs-notes-tick").attr(
                    "style",
                    format!("left:{:.1}%", percent(note.offset_ms, Some(duration))),
                ),
            ));
        }
        if group.len() > 1 {
            parts.push(Box::new(
                el("span", rows::badge(group.len().to_string()))
                    .attr("class", "rs-notes-chip")
                    .attr("style", format!("left:{left:.1}%")),
            ));
        }
    }
    for note in notes.iter().filter(|note| note.end_offset_ms.is_some()) {
        let end = note.end_offset_ms.unwrap_or(note.offset_ms);
        let left = percent(note.offset_ms, Some(duration));
        let width = (percent(end, Some(duration)) - left).max(0.5);
        parts.push(Box::new(
            el("span", text(""))
                .attr("class", "rs-notes-wash")
                .attr("style", format!("left:{left:.1}%;width:{width:.1}%")),
        ));
        parts.push(Box::new(
            el(
                "span",
                text(format!(
                    "span {}–{}",
                    format_time(note.offset_ms),
                    format_time(end)
                )),
            )
            .attr("class", "rs-notes-wash-label")
            .attr("style", format!("left:{left:.1}%")),
        ));
    }
    parts.push(Box::new(
        el("span", text("0:00"))
            .attr("class", "rs-notes-end")
            .attr("style", "left:0"),
    ));
    parts.push(Box::new(
        el("span", text(format_time(duration)))
            .attr("class", "rs-notes-end")
            .attr("style", "right:0"),
    ));
    Box::new(
        el("div", parts)
            .attr("class", "rs-notes-strip")
            .attr("aria-label", "Note timeline"),
    )
}

fn cluster_block(group: &[&NoteSummary]) -> FullView {
    let head = (group.len() > 1).then(|| {
        let last = group.last().map(|note| note.offset_ms).unwrap_or_default();
        el(
            "div",
            rows::micro(format!(
                "CLUSTER · {}–{} · {} NOTES",
                format_time(group[0].offset_ms),
                format_time(last),
                group.len()
            )),
        )
        .attr("class", "rs-notes-cluster-head")
    });
    let shown: Vec<FullView> = group
        .iter()
        .take(CLUSTER_ROWS)
        .map(|note| rows::note_row(note))
        .collect();
    let more = (group.len() > CLUSTER_ROWS).then(|| {
        el(
            "p",
            text(format!(
                "{} more in this cluster",
                group.len() - CLUSTER_ROWS
            )),
        )
        .attr("class", "rs-notes-more")
    });
    Box::new(el("section", (head, shown, more)).attr("class", "rs-notes-cluster"))
}

fn span_card(note: &NoteSummary) -> FullView {
    let end = note.end_offset_ms.unwrap_or(note.offset_ms);
    let length = end.saturating_sub(note.offset_ms);
    let privacy = if note.private { "PRIVATE" } else { "SHAREABLE" };
    let kind = note.kind().micro_word();
    let play = rows::action(
        "Play span",
        format!("Play span at {}", format_time(note.offset_ms)),
        "rs-row-action",
        CompactCommand::PlaySpan(note.id.clone()),
    );
    let edit = rows::action(
        "Edit",
        format!("Edit span at {}", format_time(note.offset_ms)),
        "rs-row-action",
        CompactCommand::BeginEditNote(note.id.clone()),
    );
    let delete = rows::action(
        "Delete",
        format!("Delete span at {}", format_time(note.offset_ms)),
        "rs-row-action",
        CompactCommand::DeleteNote(note.id.clone()),
    );
    Box::new(
        el(
            "div",
            (
                el(
                    "div",
                    (
                        el(
                            "span",
                            text(format!(
                                "{} → {} · {}",
                                format_time(note.offset_ms),
                                format_time(end),
                                format_time(length)
                            )),
                        )
                        .attr("class", "rs-notes-span-range"),
                        rows::micro(format!("{kind} · {privacy}")),
                    ),
                )
                .attr("class", "rs-notes-span-head"),
                rows::note_body(note),
                el("div", (play, edit, delete)).attr("class", "rs-row-actions"),
            ),
        )
        .attr("class", "rs-card rs-notes-span"),
    )
}

/// The representation card. `ItemRow` carries no receipt, so this states what
/// the surface can honestly say and no digest.
fn representation(state: &RedshankSurfaceState, notes: &[&NoteSummary]) -> Option<FullView> {
    let item = state.selected_item()?;
    (!notes.is_empty()).then(|| {
        Box::new(
            el(
                "div",
                (
                    rows::micro("REPRESENTATION"),
                    el(
                        "p",
                        text(format!(
                            "{} notes target the copy of {} you heard. The digest is held with each \
                             note's anchor and is not projected into this surface.",
                            notes.len(),
                            item.title
                        )),
                    )
                    .attr("class", "rs-notes-representation-body"),
                ),
            )
            .attr("class", "rs-card rs-notes-representation"),
        ) as FullView
    })
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let notes = visible(state);
    let groups = clusters(&notes);
    let list: Vec<FullView> = if groups.is_empty() {
        vec![rows::empty_row("No notes for this episode yet.")]
    } else {
        groups.iter().map(|group| cluster_block(group)).collect()
    };
    let spans: Vec<FullView> = notes
        .iter()
        .filter(|note| note.end_offset_ms.is_some())
        .map(|note| span_card(note))
        .collect();
    let end = state
        .selected_item()
        .and_then(|item| item.duration_ms)
        .map(format_time)
        .unwrap_or_else(|| "0:00".into());
    let add = rows::action(
        format!("Add text note at {end}"),
        format!("Add text note at {end}"),
        "rs-notes-add",
        CompactCommand::BeginTextNote,
    );
    let side = el("section", (spans, representation(state, &notes), add))
        .attr("class", "rs-notes-side")
        .attr("aria-label", "Spans and representation");
    let left = el("section", list)
        .attr("class", "rs-notes-list")
        .attr("aria-label", "Notes");
    Box::new(
        el(
            "section",
            (
                header(state),
                strip(state, &notes),
                el("div", (left, side)).attr("class", "rs-notes-grid"),
            ),
        )
        .attr("class", "rs-panel rs-notes"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::tests_support::{click, commands, markup, playing, runner};
    use crate::{Face, ItemRow, NoteSummaryBody, SourceKind};
    use redshank_model::{AnnotationId, ItemId};

    fn note(id: &str, offset_ms: u64, end_offset_ms: Option<u64>) -> NoteSummary {
        NoteSummary {
            id: AnnotationId(id.into()),
            item_id: ItemId("episode-42".into()),
            offset_ms,
            end_offset_ms,
            body: NoteSummaryBody::Text(format!("note {id}")),
            private: true,
        }
    }

    fn notes_state() -> RedshankSurfaceState {
        let mut state = RedshankSurfaceState {
            compact: playing(),
            ..Default::default()
        };
        state.items.push(ItemRow {
            id: ItemId("episode-42".into()),
            title: "The Evolution of Teeth".into(),
            feed_url: None,
            feed_title: Some("In Our Time".into()),
            face: Face::Tag("mp3".into()),
            source: SourceKind::Offline,
            duration_ms: Some(2_832_000),
            position_ms: 2_832_000,
            completed: true,
            published: Some("Sep 12".into()),
            cached_bytes: Some(4096),
            note_count: 11,
            unavailable: None,
        });
        for index in 0..10u64 {
            state.notes.push(note(
                &format!("point-{index}"),
                250_000 + index * 1_000,
                None,
            ));
        }
        state
            .notes
            .push(note("span-one", 1_080_000, Some(1_290_000)));
        state
    }

    #[test]
    fn the_header_reads_the_selected_item_facts() {
        let markup = markup(notes_state(), panel);
        assert!(markup.contains("IN OUR TIME · OFFLINE · COMPLETED SEP 12"));
        assert!(markup.contains("The Evolution of Teeth"));
        assert!(markup.contains("Add text note at 47:12"));
    }

    #[test]
    fn the_cluster_chip_counts_its_notes_and_the_list_truncates() {
        let markup = markup(notes_state(), panel);
        assert!(markup.contains("CLUSTER · 4:10–4:19 · 10 NOTES"));
        assert!(markup.contains("rs-notes-chip"));
        assert!(markup.contains(">10<"));
        assert!(markup.contains("6 more in this cluster"));
    }

    #[test]
    fn the_strip_places_ticks_and_a_span_wash() {
        let markup = markup(notes_state(), panel);
        assert!(markup.contains("rs-notes-tick"));
        assert!(markup.contains("rs-notes-wash"));
        assert!(markup.contains("span 18:00–21:30"));
        assert!(markup.contains(">47:12<"));
    }

    #[test]
    fn the_span_card_plays_edits_and_deletes() {
        let mut runner = runner(notes_state(), panel);
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("18:00 → 21:30 · 3:30")
        );
        click(&mut runner, "Play span at 18:00");
        click(&mut runner, "Edit span at 18:00");
        click(&mut runner, "Delete span at 18:00");
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::PlaySpan(AnnotationId("span-one".into())),
                CompactCommand::BeginEditNote(AnnotationId("span-one".into())),
                CompactCommand::DeleteNote(AnnotationId("span-one".into())),
            ]
        );
    }

    #[test]
    fn the_filter_segment_and_export_emit_commands() {
        let mut runner = runner(notes_state(), panel);
        click(&mut runner, "Notes filter: all notes");
        click(&mut runner, "Export annotations as W3C Web Annotation JSON");
        click(&mut runner, "Add text note at 47:12");
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::SetNotesFilter(NotesFilter::AllNotes),
                CompactCommand::ExportAnnotations,
                CompactCommand::BeginTextNote,
            ]
        );
    }

    #[test]
    fn the_representation_card_states_what_the_surface_knows() {
        let markup = markup(notes_state(), panel);
        assert!(markup.contains("REPRESENTATION"));
        assert!(markup.contains("11 notes target the copy of The Evolution of Teeth"));
    }

    #[test]
    fn an_empty_episode_shows_the_quiet_row() {
        let mut state = notes_state();
        state.notes.clear();
        assert!(markup(state, panel).contains("No notes for this episode yet."));
    }
}
