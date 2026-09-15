//! The notes tab body: episode header, timeline strip, clusters and spans.
//! Lane S2 owns this file.

use super::{controls, rows};
use crate::{
    CompactCommand, FullView, NoteSummary, NotesFilter, RedshankSurfaceState,
    RepresentationSummary, cluster_key, format_date, format_time, note_clusters, percent,
};
use cambium::{button, el, text};

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
    for group in note_clusters(notes) {
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

/// One cluster. A shut cluster keeps its header and count chip and drops its
/// rows; the header's collapse/expand link is the only control that opens it.
fn cluster_block(state: &RedshankSurfaceState, group: &[&NoteSummary]) -> FullView {
    let key = cluster_key(group);
    let expanded = state.cluster_expanded(key) || group.len() == 1;
    let last = group.last().map(|note| note.offset_ms).unwrap_or_default();
    let at = format_time(group[0].offset_ms);
    let head = (group.len() > 1).then(|| {
        let word = if expanded { "collapse" } else { "expand" };
        let link = button(word, move |state: &mut RedshankSurfaceState, _| {
            state.request(CompactCommand::ToggleCluster(key));
        })
        .attr("class", "rs-notes-collapse")
        .attr("aria-label", format!("{word} cluster at {at}"))
        .attr("aria-expanded", if expanded { "true" } else { "false" });
        el(
            "div",
            (
                rows::micro(format!(
                    "CLUSTER · {at}–{} · {} NOTES",
                    format_time(last),
                    group.len()
                )),
                Box::new(link) as FullView,
            ),
        )
        .attr("class", "rs-notes-cluster-head")
    });
    let shown: Vec<FullView> = if expanded {
        group
            .iter()
            .take(CLUSTER_ROWS)
            .map(|note| rows::note_row(state, note))
            .collect()
    } else {
        Vec::new()
    };
    let more = (expanded && group.len() > CLUSTER_ROWS).then(|| {
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

/// The sentence the representation card prints: what was heard, when, whether
/// the bytes still match. Each branch says only what the receipt supports.
pub fn representation_line(summary: &RepresentationSummary) -> String {
    let heard = match summary.retrieved_at_ms {
        Some(at) => format!("the copy heard on {}", format_date(at)),
        None => "the copy you heard".to_owned(),
    };
    match (&summary.short_digest, summary.matches) {
        (Some(digest), Some(true)) => {
            format!(
                "Notes target {heard} ({digest}). The cached object matches; anchors are exact."
            )
        },
        (Some(digest), Some(false)) => {
            format!(
                "Notes target {heard} ({digest}). The cached object has changed; anchors drifted."
            )
        },
        (Some(digest), None) => {
            format!("Notes target {heard} ({digest}). No note kept a digest, so drift is unproven.")
        },
        (None, _) => format!("Notes target {heard}. No digest was kept, so a change would pass."),
    }
}

/// The REPRESENTATION card, from the selected item's projected receipt.
fn representation(state: &RedshankSurfaceState, notes: &[&NoteSummary]) -> Option<FullView> {
    let item = state.selected_item()?;
    if notes.is_empty() {
        return None;
    }
    let body = match &item.representation {
        Some(summary) => representation_line(summary),
        None => format!(
            "Notes target the copy of {} you heard. It is not cached, so no receipt was kept.",
            item.title
        ),
    };
    Some(Box::new(
        el(
            "div",
            (
                rows::micro("REPRESENTATION"),
                el("p", text(body)).attr("class", "rs-notes-representation-body"),
            ),
        )
        .attr("class", "rs-card rs-notes-representation"),
    ))
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let notes = visible(state);
    let groups = note_clusters(&notes);
    let list: Vec<FullView> = if groups.is_empty() {
        vec![rows::empty_row("No notes for this episode yet.")]
    } else {
        groups
            .iter()
            .map(|group| cluster_block(state, group))
            .collect()
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
    use crate::tabs::tests_support::{act, click, commands, markup, playing, runner};
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
            pinned: false,
            representation: Some(crate::RepresentationSummary {
                retrieved_at_ms: Some(1_757_635_200_000),
                short_digest: Some("c41f9ab2".into()),
                matches: Some(true),
            }),
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
        let mut state = notes_state();
        state.seed_expanded_clusters();
        let markup = markup(state, panel);
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
    fn the_representation_card_reads_the_projected_receipt() {
        let markup = markup(notes_state(), panel);
        assert!(markup.contains("REPRESENTATION"));
        assert!(markup.contains("Notes target the copy heard on Sep 12 (c41f9ab2)."));
        assert!(markup.contains("The cached object matches; anchors are exact."));
    }

    /// Each honest variant, straight off the summary.
    #[test]
    fn the_representation_card_says_only_what_the_receipt_supports() {
        let drift = representation_line(&RepresentationSummary {
            retrieved_at_ms: Some(1_757_635_200_000),
            short_digest: Some("c41f9ab2".into()),
            matches: Some(false),
        });
        assert!(drift.contains("has changed"));
        let no_digest = representation_line(&RepresentationSummary {
            retrieved_at_ms: Some(1_757_635_200_000),
            short_digest: None,
            matches: None,
        });
        assert!(no_digest.contains("No digest was kept"));
        let undated = representation_line(&RepresentationSummary {
            retrieved_at_ms: None,
            short_digest: Some("c41f9ab2".into()),
            matches: None,
        });
        assert!(undated.contains("the copy you heard"));
        let mut state = notes_state();
        state.items[0].representation = None;
        assert!(markup(state, panel).contains("It is not cached, so no receipt was kept."));
    }

    /// Clusters are shut until the playhead's is seeded, and the header's own
    /// link is what opens and shuts one.
    #[test]
    fn cluster_collapse_keeps_the_header_and_drops_the_rows() {
        let mut runner = runner(notes_state(), panel);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("CLUSTER"));
        assert!(!markup.contains("note point-0"));
        assert!(markup.contains("expand cluster at 4:10"));
        let opened = act(&mut runner, "expand cluster at 4:10");
        assert_eq!(opened, [CompactCommand::ToggleCluster(250_000)]);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("note point-0"));
        assert!(markup.contains("collapse cluster at 4:10"));
        act(&mut runner, "collapse cluster at 4:10");
        assert!(
            !runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("note point-0")
        );
    }

    /// The default: the cluster the playhead sits in, and no other.
    #[test]
    fn the_playhead_cluster_is_the_one_seeded_open() {
        let mut state = notes_state();
        state.notes.push(note("far-one", 900_000, None));
        state.compact.now_playing.as_mut().unwrap().position_ms = 900_000;
        state.seed_expanded_clusters();
        assert_eq!(state.expanded_clusters, vec![900_000]);
        assert!(!state.cluster_expanded(250_000));
    }

    #[test]
    fn an_empty_episode_shows_the_quiet_row() {
        let mut state = notes_state();
        state.notes.clear();
        assert!(markup(state, panel).contains("No notes for this episode yet."));
    }
}
