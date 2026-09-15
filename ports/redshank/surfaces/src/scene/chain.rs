//! The chain projection. Lane S3 owns this file.
//!
//! Two structures, chosen by whether a feed is selected:
//!
//! - overview (1g): one horizontal row per feed, oldest episode left, with the
//!   connectors drawn as absolutely positioned 1px boxes;
//! - one feed (2c): a two-column grid, newest episode at the top, each episode
//!   carrying a row card and its notes beneath it.

use super::{card, geometry};
use crate::{
    CompactCommand, Face, FeedRow, FullView, ItemRow, NoteKind, NoteSummary, RedshankSurfaceState,
    format_time,
};
use cambium::{PointerClick, button_with, el, text};

/// Horizontal geometry, in pixels.
const FIRST_X: f32 = 120.0;
const STEP_X: f32 = 64.0;
const NODE_Y: f32 = 40.0;
const GNODE_Y: f32 = 88.0;

fn face(face: &Face, unplayed: usize) -> FullView {
    let label = match face {
        Face::Artwork(url) => url.clone(),
        Face::Tag(tag) => tag.clone(),
    };
    let mut children: Vec<FullView> = vec![Box::new(el("span", text(label)))];
    if unplayed > 0 {
        children.push(Box::new(
            el("span", text(unplayed.to_string())).attr("class", "rs-badge"),
        ));
    }
    Box::new(el("span", children).attr("class", "rs-face"))
}

/// One episode square. `extra` adds a scene-local class and `style` places it;
/// both carry on the same element the aria-label does, so tests can read the
/// geometry straight off the control.
pub(super) fn episode_node(
    item: &ItemRow,
    now: Option<&redshank_model::ItemId>,
    extra: &str,
    style: &str,
) -> FullView {
    let id = item.id.clone();
    let label = item.title.clone();
    Box::new(
        button_with(
            (),
            move |state: &mut RedshankSurfaceState, _: PointerClick| {
                state.request(CompactCommand::SelectItem(id.clone()));
            },
        )
        .attr(
            "class",
            geometry::node_class(geometry::node_state(item, now), extra),
        )
        .attr("style", style.to_owned())
        .attr("aria-label", label),
    )
}

pub(super) fn note_gnode(note: &NoteSummary, extra: &str, style: &str) -> FullView {
    let id = note.id.clone();
    let voice = note.kind() == NoteKind::Voice;
    let label = format!("Note at {}", format_time(note.offset_ms));
    let class = if voice {
        format!("rs-gnode rs-gnode-voice{extra}")
    } else {
        format!("rs-gnode{extra}")
    };
    Box::new(
        button_with(
            (),
            move |state: &mut RedshankSurfaceState, _: PointerClick| {
                state.request(CompactCommand::OpenNote(id.clone()));
            },
        )
        .attr("class", class)
        .attr("style", style.to_owned())
        .attr("aria-label", label),
    )
}

fn line(left: f32, width: f32, top: f32, dashed: bool) -> FullView {
    let class = if dashed {
        "rs-chain-line rs-chain-line-next"
    } else {
        "rs-chain-line"
    };
    Box::new(el("span", ()).attr("class", class).attr(
        "style",
        format!("left:{left}px;top:{top}px;width:{width}px;"),
    ))
}

/// A chain label. A pinned episode carries the small filled square first, so
/// the Mere overview marks what the listener kept without re-ordering the
/// chronology the chain exists to show.
fn label_at(x: f32, y: f32, pinned: bool, words: String) -> FullView {
    Box::new(
        el("span", (crate::tabs::rows::pin_mark(pinned), text(words)))
            .attr("class", "rs-chain-label")
            .attr("style", format!("left:{x}px;top:{y}px;")),
    )
}

/// One feed's horizontal row: face, chain, note gnodes, dashed next refresh.
fn row(state: &RedshankSurfaceState, feed: Option<&FeedRow>, url: Option<&str>) -> FullView {
    let now = geometry::now_playing(state);
    let episodes = geometry::episodes_oldest_first(state, url);
    let mut parts: Vec<FullView> = Vec::new();

    let (feed_face, title, unplayed, refreshed) = match feed {
        Some(feed) => (
            feed.face.clone(),
            feed.title.clone(),
            feed.unplayed_count,
            feed.last_refreshed_ms.is_some(),
        ),
        None => (
            Face::Tag("local".into()),
            "Local audio".to_owned(),
            0,
            false,
        ),
    };
    parts.push(Box::new(
        el("span", face(&feed_face, unplayed)).attr("class", "rs-chain-feed"),
    ));
    parts.push(Box::new(
        el("span", text(title)).attr("class", "rs-chain-feed-title"),
    ));

    for (index, item) in episodes.iter().enumerate() {
        let x = FIRST_X + index as f32 * STEP_X;
        let previous = if index == 0 { 44.0 } else { x - STEP_X };
        parts.push(line(previous, x - previous, NODE_Y, false));
        parts.push(episode_node(
            item,
            now,
            " rs-chain-node",
            &format!("left:{x}px;top:{NODE_Y}px;"),
        ));
        let mut words = geometry::short_label(item);
        if now == Some(&item.id) {
            words.push_str(" · now");
        }
        parts.push(label_at(x, NODE_Y + 16.0, item.pinned, words));

        let notes = geometry::notes_for(state, &item.id);
        if notes.is_empty() {
            continue;
        }
        parts.push(Box::new(
            el("span", ()).attr("class", "rs-chain-drop").attr(
                "style",
                format!("left:{x}px;top:56px;height:{}px;", GNODE_Y - 56.0),
            ),
        ));
        let spread = 16.0;
        let centre = (notes.len() as f32 - 1.0) / 2.0;
        for (at, note) in notes.iter().enumerate() {
            let gx = x + (at as f32 - centre) * spread;
            parts.push(note_gnode(
                note,
                " rs-chain-node",
                &format!("left:{gx}px;top:{GNODE_Y}px;"),
            ));
        }
    }

    // The state carries no publication schedule, so the dashed "next refresh"
    // link is drawn only where the feed has ever refreshed.
    if refreshed {
        let last = FIRST_X + (episodes.len().max(1) as f32 - 1.0) * STEP_X;
        let x = last + STEP_X;
        parts.push(line(last, STEP_X, NODE_Y, true));
        parts.push(Box::new(
            el("span", ())
                .attr("class", "rs-chain-node rs-node rs-node-next")
                .attr("style", format!("left:{x}px;top:{NODE_Y}px;")),
        ));
        parts.push(label_at(x, NODE_Y + 16.0, false, "next refresh".to_owned()));
    }

    Box::new(
        el("div", parts)
            .attr("class", "rs-chain-row")
            .attr("aria-label", format!("Chain {}", url.unwrap_or("local"))),
    )
}

/// The 1g overview: every feed as its own row, plus local audio.
fn overview(state: &RedshankSurfaceState) -> FullView {
    let mut rows: Vec<FullView> = state
        .feeds
        .iter()
        .map(|feed| row(state, Some(feed), Some(feed.feed_url.as_str())))
        .collect();
    if state.items.iter().any(|item| item.feed_url.is_none()) {
        rows.push(row(state, None, None));
    }
    Box::new(el("div", rows).attr("class", "rs-chain"))
}

fn episode_card(state: &RedshankSurfaceState, item: &ItemRow) -> FullView {
    let now = geometry::now_playing(state);
    let id = item.id.clone();
    let word = geometry::action_word(item, now);
    let body: Vec<FullView> = vec![
        Box::new(
            el(
                "div",
                (
                    crate::tabs::rows::pin_mark(item.pinned),
                    text(item.title.clone()),
                ),
            )
            .attr("class", "rs-chain-title"),
        ),
        Box::new(el("div", text(geometry::meta_line(item, now))).attr("class", "rs-chain-meta")),
    ];
    let action: FullView = Box::new(
        button_with(
            text(word),
            move |state: &mut RedshankSurfaceState, _: PointerClick| {
                state.request(CompactCommand::SelectItem(id.clone()));
            },
        )
        .attr("class", "rs-chain-action")
        .attr("aria-label", word),
    );
    let class = if now == Some(&item.id) {
        "rs-chain-card rs-chain-card-active"
    } else {
        "rs-chain-card"
    };
    Box::new(el("div", (Box::new(el("div", body)) as FullView, action)).attr("class", class))
}

fn note_row(note: &NoteSummary) -> FullView {
    let id = note.id.clone();
    let time = format_time(note.offset_ms);
    let label = format!("Note at {time}");
    let children: Vec<FullView> = vec![
        Box::new(el("span", text(time)).attr("class", "rs-chain-note-time")),
        Box::new(el("span", text(geometry::note_body(note)))),
    ];
    Box::new(
        button_with(
            children,
            move |state: &mut RedshankSurfaceState, _: PointerClick| {
                state.request(CompactCommand::OpenNote(id.clone()));
            },
        )
        .attr("class", "rs-chain-note")
        .attr("aria-label", label),
    )
}

fn cell(child: FullView) -> FullView {
    Box::new(el("div", child).attr("class", "rs-chain-cell"))
}

/// The 2c vertical chain for one feed: newest at the top.
fn vertical(state: &RedshankSurfaceState, feed: &FeedRow) -> FullView {
    let now = geometry::now_playing(state);
    let episodes = geometry::episodes(state, Some(feed.feed_url.as_str()));
    let mut cells: Vec<FullView> = Vec::new();

    if feed.last_refreshed_ms.is_some() {
        cells.push(cell(Box::new(
            el("span", ()).attr("class", "rs-node rs-node-next"),
        )));
        cells.push(Box::new(
            el("div", text("next refresh · published order")).attr("class", "rs-chain-meta"),
        ));
    }

    for item in episodes {
        cells.push(cell(episode_node(item, now, "", "")));
        cells.push(episode_card(state, item));
        for note in geometry::notes_for(state, &item.id) {
            cells.push(cell(note_gnode(note, "", "")));
            cells.push(note_row(note));
        }
    }

    Box::new(
        el("div", cells)
            .attr("class", "rs-chain-vertical")
            .attr("aria-label", format!("Chain {}", feed.feed_url)),
    )
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let mut children: Vec<FullView> = Vec::new();
    if super::is_overview(state) {
        children.push(overview(state));
    } else if let Some(feed) = state.current_feed() {
        children.push(vertical(state, feed));
    }
    if let Some(card) = card::connections_card(state) {
        children.push(card);
    }
    Box::new(el("div", children).attr("class", "rs-chain-panel"))
}

#[cfg(test)]
mod tests {
    use super::super::test_fixture::*;
    use crate::{CompactCommand, RedshankSurfaceState, Scene};
    use cambium::PointerClick;

    #[test]
    fn vertical_chain_marks_every_episode_state_and_hangs_its_notes() {
        let runner = runner(fixture());
        let markup = runner.dom().borrow().outer_html(runner.root());
        // Six episode squares plus the dashed next-refresh node.
        assert_eq!(markup.matches("class=\"rs-node").count(), 7);
        assert!(markup.contains("rs-node rs-node-now"));
        assert!(markup.contains("rs-node rs-node-progress"));
        assert!(markup.contains("rs-node rs-node-listened"));
        assert_eq!(markup.matches("class=\"rs-gnode").count(), 3);
        assert!(markup.contains("Playing"));
        assert!(markup.contains("Resume"));
        assert!(markup.contains("Replay"));
    }

    #[test]
    fn newest_episode_leads_the_vertical_chain() {
        let runner = runner(fixture());
        let markup = runner.dom().borrow().outer_html(runner.root());
        let newest = markup.find("215 · Six").expect("newest episode");
        let oldest = markup.find("210 · One").expect("oldest episode");
        assert!(newest < oldest);
    }

    #[test]
    fn overview_draws_one_row_per_feed_and_a_local_audio_row() {
        let mut state = fixture();
        state.selected_feed = None;
        let runner = runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("aria-label=\"Chain https://allusionist.example/feed\""));
        assert!(markup.contains("aria-label=\"Chain local\""));
        assert!(markup.contains("Local audio"));
        assert!(markup.contains("rs-chain-legend"));
        assert!(markup.contains("next refresh"));
    }

    #[test]
    fn episode_and_note_and_card_actions_emit_commands() {
        let mut runner = runner(fixture());
        for (label, expected) in [
            ("Playing", CompactCommand::SelectItem(item_id("e213"))),
            ("Note at 0:46", CompactCommand::OpenNote(note_id("n1"))),
            ("Open", CompactCommand::SelectItem(item_id("e213"))),
            ("Pin", CompactCommand::PinItem(item_id("e213"))),
            ("Add to queue", CompactCommand::Enqueue(item_id("e213"))),
        ] {
            let node = node_with_label(&runner.dom().borrow(), runner.root(), label);
            runner.dispatch_click(node, PointerClick::at((1.0, 1.0)));
            let mut commands = Vec::new();
            runner
                .update(|state: &mut RedshankSurfaceState| commands.extend(state.drain_commands()));
            assert_eq!(commands, [expected], "{label}");
        }
    }

    #[test]
    fn the_context_card_lists_the_neighbours_by_date() {
        let runner = runner(fixture());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("← 212 · Three"));
        assert!(markup.contains("→ 214 · Five"));
        assert!(markup.contains("3 notes · 2 text · 1 voice"));
        assert!(markup.contains("0:46 1:02 1:16"));
        assert!(markup.contains("cloud"));
    }

    #[test]
    fn the_scene_segment_emits_select_scene() {
        let mut runner = runner(fixture());
        let node = node_with_label(&runner.dom().borrow(), runner.root(), "orrery");
        runner.dispatch_click(node, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state: &mut RedshankSurfaceState| {
            assert_eq!(state.scene, Scene::Orrery);
            commands.extend(state.drain_commands());
        });
        assert_eq!(commands, [CompactCommand::SelectScene(Scene::Orrery)]);
    }

    #[test]
    fn refresh_emits_for_the_current_feed() {
        let mut runner = runner(fixture());
        let node = node_with_label(&runner.dom().borrow(), runner.root(), "Refresh");
        runner.dispatch_click(node, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state: &mut RedshankSurfaceState| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [CompactCommand::RefreshSubscription(
                "https://allusionist.example/feed".to_owned()
            )]
        );
    }

    #[test]
    fn the_header_summarises_one_feed_and_the_whole_library() {
        let one = runner(fixture());
        assert!(
            one.dom()
                .borrow()
                .outer_html(one.root())
                .contains("214 ep · 3 unplayed · 340 MiB")
        );
        let mut state = fixture();
        state.selected_feed = None;
        let all = runner(state);
        assert!(
            all.dom()
                .borrow()
                .outer_html(all.root())
                .contains("1 feeds · 214 episodes · 3 notes")
        );
    }

    /// A pin is a model fact the overview marks and the card can undo.
    #[test]
    fn a_pinned_episode_is_marked_and_the_card_offers_unpin() {
        let mut state = fixture();
        state.selected_feed = None;
        for row in &mut state.items {
            if row.id == item_id("e213") {
                row.pinned = true;
            }
        }
        let runner = runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        // One mark on the chain label, one on the connections card's title.
        assert_eq!(markup.matches("rs-pin-mark").count(), 2);
        assert!(markup.contains("Unpin"));
        assert!(!markup.contains("aria-label=\"Pin\""));
    }
}
