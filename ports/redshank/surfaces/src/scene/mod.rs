//! Feed-node projections for the Mere tab and the phone feed page. Lane S3 owns this directory.
//!
//! One structure, three projections: the chain (1g wide, 2c phone), the orrery
//! (2f), and the trail (2g). The scene header selects among them; the header,
//! the summary, and the legend live here because all three share them.
//!
//! Hosts must append [`SCENE_CSS`] to the Redshank stylesheet.

pub mod card;
pub mod chain;
pub mod geometry;
pub mod orrery;
pub mod trail;

use crate::{CompactCommand, FullView, RedshankSurfaceState, Scene, format_bytes, format_span};
use cambium::{PointerClick, button, el, text};

/// Whether the scene shows every feed at once (the Mere overview) or one feed.
///
/// `current_feed()` falls back to the first feed, so it is `None` only for an
/// empty library; the overview is keyed off an unset `selected_feed` instead.
pub fn is_overview(state: &RedshankSurfaceState) -> bool {
    state.selected_feed.is_none()
}

/// `1,260`.
fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The header's right-hand fact: the whole library, one feed, or the week.
fn summary(state: &RedshankSurfaceState) -> String {
    if state.scene == Scene::Trail {
        let listened: u64 = state
            .sessions
            .iter()
            .map(|session| session.stop_ms.saturating_sub(session.start_ms))
            .sum();
        return format!("{} this week", format_span(listened));
    }
    if let Some(feed) = state
        .selected_feed
        .as_deref()
        .and_then(|url| state.feed(url))
    {
        return format!(
            "{} ep · {} unplayed · {}",
            feed.episode_count,
            feed.unplayed_count,
            format_bytes(feed.offline_bytes),
        );
    }
    let episodes: usize = state.feeds.iter().map(|feed| feed.episode_count).sum();
    let notes: usize = state.items.iter().map(|item| item.note_count).sum();
    format!(
        "{} feeds · {} episodes · {} notes",
        state.feeds.len(),
        grouped(episodes),
        grouped(notes),
    )
}

fn scene_segment(state: &RedshankSurfaceState) -> FullView {
    let buttons: Vec<FullView> = Scene::ALL
        .iter()
        .map(|scene| {
            let scene = *scene;
            let class = if state.scene == scene {
                "rs-segment-option rs-segment-item rs-segment-on"
            } else {
                "rs-segment-option rs-segment-item"
            };
            Box::new(
                button(
                    scene.label(),
                    move |state: &mut RedshankSurfaceState, _: PointerClick| {
                        state.scene = scene;
                        state.request(CompactCommand::SelectScene(scene));
                    },
                )
                .attr("class", class)
                .attr("aria-label", scene.label())
                .attr(
                    "aria-pressed",
                    if state.scene == scene {
                        "true"
                    } else {
                        "false"
                    },
                ),
            ) as FullView
        })
        .collect();
    Box::new(
        el("div", buttons)
            .attr("class", "rs-segment")
            .attr("role", "group"),
    )
}

fn header(state: &RedshankSurfaceState) -> FullView {
    let mut row: Vec<FullView> = vec![
        Box::new(el("span", text("SCENE")).attr("class", "rs-micro")),
        scene_segment(state),
        Box::new(el("span", text(summary(state))).attr("class", "rs-scene-summary")),
    ];
    if let Some(feed) = state.current_feed() {
        let url = feed.feed_url.clone();
        row.push(Box::new(
            button(
                "Refresh",
                move |state: &mut RedshankSurfaceState, _: PointerClick| {
                    state.request(CompactCommand::RefreshSubscription(url.clone()));
                },
            )
            .attr("class", "rs-scene-refresh")
            .attr("aria-label", "Refresh"),
        ));
    }
    Box::new(el("div", row).attr("class", "rs-scene-head"))
}

/// listened / in progress / now / note, for the wide chain.
fn legend() -> FullView {
    let entries: Vec<FullView> = [
        ("rs-node rs-node-listened", "listened"),
        ("rs-node rs-node-progress", "in progress"),
        ("rs-node rs-node-now", "now"),
        ("rs-gnode", "note"),
    ]
    .into_iter()
    .map(|(class, word)| {
        Box::new(
            el(
                "span",
                (
                    Box::new(el("span", ()).attr("class", class)) as FullView,
                    Box::new(el("span", text(word.to_owned()))) as FullView,
                ),
            )
            .attr("class", "rs-chain-legend-item"),
        ) as FullView
    })
    .collect();
    Box::new(
        el("div", entries)
            .attr("class", "rs-chain-legend")
            .attr("aria-label", "Legend"),
    )
}

/// The scene the state selects, over the current feed.
pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let mut children: Vec<FullView> = vec![header(state)];
    match state.scene {
        Scene::Chain => {
            children.push(chain::panel(state));
            if is_overview(state) {
                children.push(legend());
            }
        },
        Scene::Orrery => children.push(orrery::panel(state)),
        Scene::Trail => children.push(trail::panel(state)),
    }
    Box::new(
        el("section", children)
            .attr("class", "rs-scene")
            .attr("aria-label", "Scene"),
    )
}

/// Scene-local rules. The shell appends this to the Redshank stylesheet;
/// only `--t-*`, `--font-*`, `--space-*`, and `--text-*` tokens appear here.
pub const SCENE_CSS: &str = r#"
.rs-scene { display: flex; flex-direction: column; gap: var(--space-12); padding: var(--space-12); }
.rs-scene-head { display: flex; align-items: center; gap: var(--space-12); }
.rs-scene-summary { flex: 1; font-family: var(--font-mono); font-size: var(--text-label);
  color: var(--t-text-dim); overflow: hidden; white-space: nowrap; }
.rs-scene-refresh { font-size: var(--text-label); color: var(--t-text); background: var(--t-bg);
  border: 0; padding: var(--space-4) var(--space-8); box-shadow: inset 0 0 0 1px var(--t-surface-2); }

/* Nodes. Sizes are scene facts, so they are scoped to the scene. */
.rs-scene .rs-node { display: block; width: 20px; height: 20px; background: var(--t-surface);
  box-shadow: inset 0 0 0 1px var(--t-surface-2); }
.rs-scene .rs-node-listened { background: var(--t-text-dim); }
.rs-scene .rs-node-progress { background: color-mix(in srgb, var(--t-secondary) 55%, transparent); }
.rs-scene .rs-node-now { width: 24px; height: 24px; background: var(--t-tertiary);
  box-shadow: inset 0 0 0 2px var(--t-tertiary); }
.rs-scene .rs-node-next { background: transparent; border: 1px dashed var(--t-text-dim); box-shadow: none; }
.rs-scene .rs-gnode { display: block; width: 10px; height: 10px; border-radius: 50%;
  background: var(--t-text); }
.rs-scene .rs-gnode-voice { background: transparent; box-shadow: inset 0 0 0 2px var(--t-text); }
.rs-scene .rs-face { display: block; width: 40px; height: 40px; background: var(--t-surface);
  font-family: var(--font-mono); font-size: var(--text-label); color: var(--t-text-dim);
  overflow: hidden; white-space: nowrap; }

/* Chain, wide (1g): one relative row per feed, connectors as 1px boxes. */
.rs-chain { display: flex; flex-direction: column; gap: var(--space-8); }
.rs-chain-row { position: relative; height: 108px; }
.rs-chain-node { position: absolute; transform: translate(-50%, -50%); }
.rs-chain-line { position: absolute; height: 1px; background: var(--t-surface-2); z-index: 0; }
.rs-chain-line-next { background: transparent; border-top: 1px dashed var(--t-text-dim); }
.rs-chain-drop { position: absolute; width: 1px; background: var(--t-surface-2); z-index: 0; }
.rs-chain-label { position: absolute; transform: translate(-50%, 0); font-family: var(--font-mono);
  font-size: var(--text-label); color: var(--t-text-dim); white-space: nowrap;
  display: flex; align-items: center; }
.rs-scene .rs-pin-mark { display: block; width: 6px; height: 6px; background: var(--t-text);
  flex: none; margin-right: var(--space-4); }
.rs-chain-title, .rs-card-title { display: flex; align-items: center; }
.rs-chain-feed { position: absolute; left: 0; top: 8px; }
.rs-chain-feed-title { position: absolute; left: 0; top: 56px; font-size: var(--text-label);
  color: var(--t-text); white-space: nowrap; overflow: hidden; width: 80px; }

/* Chain, vertical (2c): node column plus an episode card. */
.rs-chain-vertical { display: grid; grid-template-columns: 28px 1fr; gap: var(--space-8); align-items: center; }
.rs-chain-cell { display: grid; place-items: center; }
.rs-chain-card { display: flex; align-items: center; gap: var(--space-8);
  padding: var(--space-8); background: var(--t-surface); }
.rs-chain-card-active { box-shadow: inset 0 0 0 2px var(--t-secondary); }
.rs-chain-title { font-size: var(--text-ui-13); color: var(--t-text); overflow: hidden; white-space: nowrap; }
.rs-chain-meta { font-family: var(--font-mono); font-size: var(--text-label); color: var(--t-text-dim);
  overflow: hidden; white-space: nowrap; }
.rs-chain-action { border: 0; background: var(--t-bg); color: var(--t-text);
  font-size: var(--text-label); padding: var(--space-4) var(--space-8); }
.rs-chain-note { display: flex; gap: var(--space-8); border: 0; background: transparent;
  color: var(--t-text-dim); font-size: var(--text-label); text-align: left;
  overflow: hidden; white-space: nowrap; }
.rs-chain-note-time { font-family: var(--font-mono); color: var(--t-secondary); }
.rs-chain-legend { display: flex; gap: var(--space-12); font-family: var(--font-mono);
  font-size: var(--text-label); color: var(--t-text-dim); }
.rs-chain-legend-item { display: flex; align-items: center; gap: var(--space-4); }

/* Orrery (2f): a fixed relative box, everything placed from the centre. */
.rs-orrery { position: relative; height: 380px; }
.rs-orrery-ring { position: absolute; left: 50%; top: 190px; width: 280px; height: 280px;
  transform: translate(-50%, -50%); border: 1px dashed var(--t-surface-2); border-radius: 50%; }
.rs-orrery-orbit { position: absolute; width: 56px; height: 56px; transform: translate(-50%, -50%);
  border: 1px solid color-mix(in srgb, var(--t-tertiary) 40%, transparent); border-radius: 50%; }
.rs-orrery-node { position: absolute; transform: translate(-50%, -50%); }
.rs-orrery-label { position: absolute; transform: translate(0, -50%); font-family: var(--font-mono);
  font-size: var(--text-label); color: var(--t-text-dim); white-space: nowrap; }
.rs-orrery-centre { position: absolute; left: 50%; top: 190px; transform: translate(-50%, -50%);
  display: grid; justify-items: center; gap: var(--space-4); }
.rs-orrery-caption { font-family: var(--font-mono); font-size: var(--text-label); color: var(--t-text-dim);
  white-space: nowrap; }
.rs-orrery-older { background: color-mix(in srgb, var(--t-text-dim) 35%, transparent); }

/* Trail (2g): a washed card per session, sectioned by day. */
.rs-trail { display: flex; flex-direction: column; gap: var(--space-8); }
.rs-trail-day { display: flex; align-items: center; gap: var(--space-8); font-family: var(--font-mono);
  font-size: var(--text-label); color: var(--t-text-dim); }
.rs-trail-rule { flex: 1; height: 1px; background: var(--t-surface-2); }
.rs-trail-card { position: relative; padding: var(--space-8); background: var(--t-surface); overflow: hidden; }
.rs-trail-wash { position: absolute; top: 0; bottom: 0; z-index: 0; }
.rs-trail-wash-active { background: color-mix(in srgb, var(--t-secondary) 30%, transparent); }
.rs-trail-wash-done { background: color-mix(in srgb, var(--t-text-dim) 25%, transparent); }
.rs-trail-head { position: absolute; top: 0; bottom: 0; width: 2px; background: var(--t-tertiary); z-index: 1; }
.rs-trail-dot { position: absolute; bottom: 3px; width: 6px; height: 6px; border-radius: 50%;
  background: var(--t-text); transform: translate(-50%, 0); z-index: 1; }
.rs-trail-body { position: relative; display: flex; align-items: center; gap: var(--space-8); z-index: 2; }
.rs-trail-title { flex: 1; font-size: var(--text-ui-13); color: var(--t-text);
  overflow: hidden; white-space: nowrap; }
.rs-trail-meta { font-family: var(--font-mono); font-size: var(--text-label); color: var(--t-text-dim);
  overflow: hidden; white-space: nowrap; }
.rs-trail-action { border: 0; background: var(--t-bg); color: var(--t-text);
  font-size: var(--text-label); padding: var(--space-4) var(--space-8); }

/* Context card, shared by chain and orrery. */
.rs-scene .rs-card { display: flex; flex-direction: column; gap: var(--space-8); max-width: 320px;
  padding: var(--space-12); background: var(--t-surface); box-shadow: inset 0 0 0 1px var(--t-surface-2); }
.rs-card-title { font-size: var(--text-ui-13); color: var(--t-text); overflow: hidden; white-space: nowrap; }
.rs-card-guid, .rs-card-line { display: flex; justify-content: space-between; gap: var(--space-8);
  font-family: var(--font-mono); font-size: var(--text-label); color: var(--t-text-dim);
  overflow: hidden; white-space: nowrap; }
.rs-card-actions { display: flex; gap: var(--space-8); }
.rs-card-action { border: 0; background: var(--t-bg); color: var(--t-text);
  font-size: var(--text-label); padding: var(--space-4) var(--space-8); }
"#;

/// The one fixture the three projections are checked against: a feed of six
/// episodes (two completed, one in progress, one playing, two unplayed), three
/// notes on the playing episode, and six listening sessions over three days.
#[cfg(test)]
pub(super) mod test_fixture {
    use crate::{
        CompactPlayerState, Face, FeedRow, FullView, ItemRow, ListeningSessionRow, NoteSummary,
        NoteSummaryBody, NowPlaying, RedshankSurfaceState, SourceKind, TransportState,
    };
    use cambium::{DomHandle, GenetAppRunner};
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use redshank_model::{AnnotationId, ItemId};
    use std::cell::RefCell;
    use std::rc::Rc;

    pub const FEED: &str = "https://allusionist.example/feed";

    type FullLogic = fn(&RedshankSurfaceState) -> FullView;
    pub type Runner = GenetAppRunner<RedshankSurfaceState, FullLogic, FullView, ()>;

    pub fn item_id(raw: &str) -> ItemId {
        ItemId(raw.to_owned())
    }

    pub fn note_id(raw: &str) -> AnnotationId {
        AnnotationId(raw.to_owned())
    }

    fn episode(
        id: &str,
        title: &str,
        published: &str,
        position_ms: u64,
        completed: bool,
        notes: usize,
    ) -> ItemRow {
        ItemRow {
            id: item_id(id),
            title: title.to_owned(),
            feed_url: Some(FEED.to_owned()),
            feed_title: Some("The Allusionist".to_owned()),
            face: Face::Tag("mp3".into()),
            source: SourceKind::Cloud,
            duration_ms: Some(1_925_000),
            position_ms,
            completed,
            published: Some(published.to_owned()),
            cached_bytes: None,
            note_count: notes,
            unavailable: None,
            pinned: false,
            representation: None,
        }
    }

    fn note(id: &str, offset_ms: u64, body: NoteSummaryBody) -> NoteSummary {
        NoteSummary {
            id: note_id(id),
            item_id: item_id("e213"),
            offset_ms,
            end_offset_ms: None,
            body,
            private: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn session(
        id: &str,
        title: &str,
        start_ms: u64,
        stop_ms: u64,
        day: &str,
        clock: &str,
        notes: Vec<u64>,
        completed: bool,
        active: bool,
    ) -> ListeningSessionRow {
        ListeningSessionRow {
            item_id: item_id(id),
            title: title.to_owned(),
            start_ms,
            stop_ms,
            duration_ms: Some(1_925_000),
            day_label: day.to_owned(),
            wall_clock: clock.to_owned(),
            note_offsets_ms: notes,
            completed,
            active,
        }
    }

    pub fn fixture() -> RedshankSurfaceState {
        let items = vec![
            episode("e210", "210 · One", "2026-08-14", 1_925_000, true, 0),
            episode("e211", "211 · Two", "2026-08-21", 1_925_000, true, 0),
            episode("e212", "212 · Three", "2026-08-28", 850_000, false, 0),
            episode("e213", "213 · Four", "2026-09-04", 760_000, false, 3),
            episode("e214", "214 · Five", "2026-09-11", 0, false, 0),
            episode("e215", "215 · Six", "2026-09-18", 0, false, 0),
            ItemRow {
                feed_url: None,
                feed_title: None,
                source: SourceKind::Local,
                ..episode("local1", "Heartilation", "2026-07-02", 0, false, 0)
            },
        ];
        let now = NowPlaying {
            item_id: item_id("e213"),
            title: "213 · Four".to_owned(),
            feed_title: Some("The Allusionist".to_owned()),
            face: Face::Tag("mp3".into()),
            source: SourceKind::Cloud,
            position_ms: 760_000,
            duration_ms: Some(1_925_000),
            resumed_from_ms: None,
            buffered_percent: 100,
            markers: Vec::new(),
        };
        RedshankSurfaceState {
            selected_feed: Some(FEED.to_owned()),
            compact: CompactPlayerState {
                transport: TransportState::Playing,
                now_playing: Some(now),
                ..CompactPlayerState::default()
            },
            feeds: vec![FeedRow {
                feed_url: FEED.to_owned(),
                title: "The Allusionist".to_owned(),
                subtitle: None,
                face: Face::Tag("art".into()),
                episode_count: 214,
                unplayed_count: 3,
                offline_bytes: 340 * 1024 * 1024,
                last_refreshed_ms: Some(1_757_000_000_000),
                failure: None,
            }],
            items,
            notes: vec![
                note("n1", 46_000, NoteSummaryBody::Text("worried".to_owned())),
                note(
                    "n2",
                    62_000,
                    NoteSummaryBody::Text("second verse".to_owned()),
                ),
                note(
                    "n3",
                    76_000,
                    NoteSummaryBody::Voice {
                        duration_ms: 6_000,
                        preview: crate::VoiceNotePreview::Idle,
                    },
                ),
            ],
            sessions: vec![
                session(
                    "e213",
                    "213 · Four",
                    0,
                    760_000,
                    "TODAY",
                    "now",
                    vec![46_000, 62_000, 76_000],
                    false,
                    true,
                ),
                session(
                    "e212",
                    "212 · Three",
                    0,
                    1_021_000,
                    "TODAY",
                    "8:10 am",
                    Vec::new(),
                    false,
                    false,
                ),
                session(
                    "e211",
                    "211 · Two",
                    450_000,
                    1_925_000,
                    "YESTERDAY",
                    "9:12 pm",
                    Vec::new(),
                    true,
                    false,
                ),
                session(
                    "e210",
                    "210 · One",
                    0,
                    450_000,
                    "YESTERDAY",
                    "6:05 pm",
                    Vec::new(),
                    false,
                    false,
                ),
                session(
                    "e214",
                    "214 · Five",
                    0,
                    300_000,
                    "SEP 10",
                    "1:00 pm",
                    vec![10_000],
                    false,
                    false,
                ),
                session(
                    "e215",
                    "215 · Six",
                    0,
                    200_000,
                    "SEP 10",
                    "2:00 pm",
                    Vec::new(),
                    false,
                    false,
                ),
            ],
            ..RedshankSurfaceState::default()
        }
    }

    /// A runner over the scene panel alone.
    pub fn runner(state: RedshankSurfaceState) -> Runner {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        Runner::new(dom, super::panel as FullLogic, state)
    }

    pub fn node_with_label(dom: &ScriptedDom, root: NodeId, label: &str) -> NodeId {
        let aria = LocalName::from("aria-label");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &aria)
                .is_some_and(|value| value == label)
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing control {label}");
    }
}

#[cfg(test)]
mod tests {
    use super::test_fixture::*;
    use crate::{RedshankSurfaceState, Scene};

    fn markup(scene: Scene) -> String {
        let runner = runner(RedshankSurfaceState { scene, ..fixture() });
        runner.dom().borrow().outer_html(runner.root())
    }

    #[test]
    fn the_three_projections_carry_the_same_six_episodes() {
        let chain = markup(Scene::Chain);
        let orrery = markup(Scene::Orrery);
        let trail = markup(Scene::Trail);
        assert_eq!(chain.matches("class=\"rs-chain-card").count(), 6);
        // Episode squares only: the dashed next node and the three note
        // satellites ride the same field.
        let squares = orrery.matches("class=\"rs-node").count();
        assert_eq!(squares - orrery.matches("rs-node-next").count(), 6);
        assert_eq!(orrery.matches("class=\"rs-gnode").count(), 3);
        assert_eq!(trail.matches("aria-label=\"Session ").count(), 6);
    }
}
