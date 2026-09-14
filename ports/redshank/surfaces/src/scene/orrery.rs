//! The orrery projection (2f). Lane S3 owns this file.
//!
//! The feed sits at the centre of a 280px ring; the newest episodes run
//! clockwise from twelve. Everything is placed absolutely inside a fixed
//! 360x380 field so no `calc` or layout measurement is needed.

use super::{card, chain, geometry};
use crate::{FullView, RedshankSurfaceState};
use cambium::{el, text};

const FIELD_W: f32 = 360.0;
const FIELD_H: f32 = 380.0;
const CENTRE_X: f32 = FIELD_W / 2.0;
const CENTRE_Y: f32 = 190.0;
const RADIUS: f32 = 140.0;
const ORBIT: f32 = 28.0;
/// At most this many episodes ride the ring; the rest collapse into `older`.
const SLOTS: usize = 8;

fn place(dx: f32, dy: f32) -> String {
    format!("left:{}px;top:{}px;", CENTRE_X + dx, CENTRE_Y + dy)
}

fn label(dx: f32, dy: f32, words: String) -> FullView {
    Box::new(
        el("span", text(words))
            .attr("class", "rs-orrery-label")
            .attr(
                "style",
                format!("left:{}px;top:{}px;", CENTRE_X + dx + 16.0, CENTRE_Y + dy),
            ),
    )
}

/// `214 · cloud` or `212 · 44%` — the fact beside a ring node.
fn ring_words(item: &crate::ItemRow, now: Option<&redshank_model::ItemId>) -> String {
    let head = geometry::short_label(item);
    let tail = match geometry::node_state(item, now) {
        geometry::NodeState::Now => "now".to_owned(),
        geometry::NodeState::Listened => "listened".to_owned(),
        geometry::NodeState::Progress => {
            format!("{:.0}%", item.listened() * 100.0)
        },
        geometry::NodeState::Unplayed => item.source.badge().to_lowercase(),
    };
    format!("{head} · {tail}")
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let Some(feed) = state.current_feed() else {
        return Box::new(el("div", text("No feed")).attr("class", "rs-orrery"));
    };
    let now = geometry::now_playing(state);
    let episodes = geometry::episodes(state, Some(feed.feed_url.as_str()));
    let older = episodes.len() > SLOTS;
    let shown = if older {
        &episodes[..SLOTS - 1]
    } else {
        &episodes[..]
    };
    let count = shown.len() + usize::from(older);

    let mut field: Vec<FullView> = vec![Box::new(
        el("span", ())
            .attr("class", "rs-orrery-ring")
            .attr("style", format!("left:{CENTRE_X}px;")),
    )];

    // Centre: the feed face and the caption drawn from the facts that exist —
    // the episode count and the settings refresh schedule.
    let caption = format!(
        "{} ep · {} refresh",
        feed.episode_count,
        match state.settings.refresh_schedule {
            redshank_model::RefreshSchedule::Manual => "manual",
            redshank_model::RefreshSchedule::Hourly => "hourly",
            redshank_model::RefreshSchedule::Daily => "daily",
        }
    );
    let face_label = match &feed.face {
        crate::Face::Artwork(url) => url.clone(),
        crate::Face::Tag(tag) => tag.clone(),
    };
    field.push(Box::new(
        el(
            "div",
            (
                Box::new(
                    el(
                        "span",
                        (
                            Box::new(el("span", text(face_label))) as FullView,
                            Box::new(
                                el("span", text(feed.unplayed_count.to_string()))
                                    .attr("class", "rs-badge"),
                            ) as FullView,
                        ),
                    )
                    .attr("class", "rs-face"),
                ) as FullView,
                Box::new(el("div", text(caption)).attr("class", "rs-orrery-caption")) as FullView,
            ),
        )
        .attr("class", "rs-orrery-centre")
        .attr("style", format!("left:{CENTRE_X}px;")),
    ));

    for (index, item) in shown.iter().enumerate() {
        let (dx, dy) = geometry::ring_point(index, count, RADIUS);
        field.push(chain::episode_node(
            item,
            now,
            " rs-orrery-node",
            &place(dx, dy),
        ));
        field.push(label(dx, dy, ring_words(item, now)));

        // The playing episode carries its notes as satellites on a 56px orbit.
        if now == Some(&item.id) {
            field.push(Box::new(
                el("span", ())
                    .attr("class", "rs-orrery-orbit")
                    .attr("style", place(dx, dy)),
            ));
            let notes = geometry::notes_for(state, &item.id);
            for (at, note) in notes.iter().enumerate() {
                let (sx, sy) = geometry::ring_point(at, notes.len(), ORBIT);
                field.push(chain::note_gnode(
                    note,
                    " rs-orrery-node",
                    &place(dx + sx, dy + sy),
                ));
            }
        }
    }

    if older {
        let (dx, dy) = geometry::ring_point(count - 1, count, RADIUS);
        field.push(Box::new(
            el("span", ())
                .attr("class", "rs-node rs-orrery-node rs-orrery-older")
                .attr("style", place(dx, dy)),
        ));
        field.push(label(
            dx,
            dy,
            format!("{} older", episodes.len() - shown.len()),
        ));
    }

    // The dashed next node sits at twelve, just above the newest episode.
    if feed.last_refreshed_ms.is_some() {
        field.push(Box::new(
            el("span", ())
                .attr("class", "rs-node rs-node-next rs-orrery-node")
                .attr("style", place(0.0, -RADIUS - 28.0)),
        ));
        field.push(label(0.0, -RADIUS - 28.0, "next refresh".to_owned()));
    }

    let mut children: Vec<FullView> = vec![Box::new(
        el("div", field)
            .attr("class", "rs-orrery-field")
            .attr("style", format!("width:{FIELD_W}px;height:{FIELD_H}px;")),
    )];
    if let Some(card) = card::compact_card(state) {
        children.push(card);
    }
    Box::new(
        el("div", children)
            .attr("class", "rs-orrery")
            .attr("aria-label", format!("Orrery {}", feed.feed_url)),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_fixture::*;
    use crate::{CompactCommand, RedshankSurfaceState, Scene};
    use cambium::PointerClick;
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};

    fn orrery_state() -> RedshankSurfaceState {
        RedshankSurfaceState {
            scene: Scene::Orrery,
            ..fixture()
        }
    }

    /// Every ring node as `(aria-label, top in px)`.
    fn placed(dom: &ScriptedDom, root: NodeId) -> Vec<(String, f32)> {
        let class = LocalName::from("class");
        let style = LocalName::from("style");
        let aria = LocalName::from("aria-label");
        let empty = Namespace::from("");
        let mut out = Vec::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            let is_node = dom
                .attribute(node, &empty, &class)
                .is_some_and(|value| value.contains("rs-orrery-node"));
            if is_node {
                let style = dom.attribute(node, &empty, &style).unwrap_or_default();
                let top = style
                    .split(';')
                    .find_map(|part| part.trim().strip_prefix("top:"))
                    .and_then(|value| value.trim_end_matches("px").parse::<f32>().ok())
                    .expect("a placed node carries a top offset");
                let label = dom.attribute(node, &empty, &aria).unwrap_or_default();
                out.push((label.to_string(), top));
            }
            pending.extend(dom.dom_children(node));
        }
        out
    }

    #[test]
    fn the_ring_carries_every_episode_and_the_notes_as_satellites() {
        let runner = runner(orrery_state());
        let handle = runner.dom();
        let dom = handle.borrow();
        let nodes = placed(&dom, runner.root());
        let episodes = nodes
            .iter()
            .filter(|(label, _)| label.starts_with('2'))
            .count();
        assert_eq!(episodes, 6);
        let satellites = nodes
            .iter()
            .filter(|(label, _)| label.starts_with("Note at"))
            .count();
        assert_eq!(satellites, 3);
    }

    #[test]
    fn the_newest_episode_sits_at_twelve() {
        let runner = runner(orrery_state());
        let handle = runner.dom();
        let dom = handle.borrow();
        let nodes: Vec<(String, f32)> = placed(&dom, runner.root())
            .into_iter()
            .filter(|(label, _)| label.starts_with('2'))
            .collect();
        let newest = nodes
            .iter()
            .find(|(label, _)| label.starts_with("215"))
            .expect("newest episode");
        let smallest = nodes.iter().map(|(_, top)| *top).fold(f32::MAX, f32::min);
        assert_eq!(newest.1, smallest);
        assert_eq!(newest.1, 50.0);
    }

    #[test]
    fn the_centre_caption_uses_the_counts_and_the_refresh_schedule() {
        let runner = runner(orrery_state());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("214 ep · manual refresh"));
        assert!(markup.contains("rs-orrery-orbit"));
        assert!(markup.contains("rs-node-next"));
        assert!(markup.contains("213 · now"));
        assert!(markup.contains("212 · 44%"));
    }

    #[test]
    fn the_selected_card_opens_the_episode() {
        let mut runner = runner(orrery_state());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("SELECTED · CONNECTIONS"));
        assert!(markup.contains("← 212 · Three · → 214 · Five · notes 0:46 1:02 1:16"));
        let open = node_with_label(&runner.dom().borrow(), runner.root(), "Open");
        runner.dispatch_click(open, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state: &mut RedshankSurfaceState| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::SelectItem(item_id("e213"))]);
    }
}
