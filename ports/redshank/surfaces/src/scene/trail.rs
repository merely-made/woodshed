//! The trail projection (2g). Lane S3 owns this file.
//!
//! Listening sessions newest first, sectioned by the host's day label. Each
//! card is washed from where the session started to where it stopped, with the
//! notes made in it sitting on the wash.

use crate::{
    CompactCommand, FullView, ListeningSessionRow, RedshankSurfaceState, format_span, format_time,
    percent,
};
use cambium::{PointerClick, button_with, el, text};

/// `0:00 → 12:40 · now · 3 notes · completed`.
fn meta_line(session: &ListeningSessionRow) -> String {
    let stop = match session.duration_ms {
        Some(duration) if session.completed && session.stop_ms >= duration => "end".to_owned(),
        _ => format_time(session.stop_ms),
    };
    let mut parts = vec![
        format!("{} → {}", format_time(session.start_ms), stop),
        if session.active {
            "now".to_owned()
        } else {
            session.wall_clock.clone()
        },
    ];
    match session.note_offsets_ms.len() {
        0 => {},
        1 => parts.push("1 note".to_owned()),
        many => parts.push(format!("{many} notes")),
    }
    if session.completed {
        parts.push("completed".to_owned());
    }
    parts.join(" · ")
}

fn action_word(session: &ListeningSessionRow) -> &'static str {
    if session.active {
        "Playing"
    } else if session.completed {
        "Replay"
    } else {
        "Resume"
    }
}

fn card(session: &ListeningSessionRow) -> FullView {
    let start = percent(session.start_ms, session.duration_ms);
    let stop = percent(session.stop_ms, session.duration_ms);
    let wash_class = if session.active {
        "rs-trail-wash rs-trail-wash-active"
    } else {
        "rs-trail-wash rs-trail-wash-done"
    };
    let mut parts: Vec<FullView> = vec![Box::new(el("span", ()).attr("class", wash_class).attr(
        "style",
        format!("left:{start:.1}%;width:{:.1}%;", (stop - start).max(0.0)),
    ))];
    if session.active {
        parts.push(Box::new(
            el("span", ())
                .attr("class", "rs-trail-head")
                .attr("style", format!("left:{stop:.1}%;")),
        ));
    }
    for offset in &session.note_offsets_ms {
        parts.push(Box::new(el("span", ()).attr("class", "rs-trail-dot").attr(
            "style",
            format!("left:{:.1}%;", percent(*offset, session.duration_ms)),
        )));
    }

    let id = session.item_id.clone();
    let word = action_word(session);
    let body: Vec<FullView> = vec![
        Box::new(
            el(
                "div",
                (
                    Box::new(el("div", text(session.title.clone())).attr("class", "rs-trail-title"))
                        as FullView,
                    Box::new(el("div", text(meta_line(session))).attr("class", "rs-trail-meta"))
                        as FullView,
                ),
            )
            .attr("class", "rs-trail-title"),
        ),
        Box::new(
            button_with(
                text(word),
                move |state: &mut RedshankSurfaceState, _: PointerClick| {
                    state.request(CompactCommand::SelectItem(id.clone()));
                },
            )
            .attr("class", "rs-trail-action")
            .attr("aria-label", format!("{word} {}", session.title)),
        ),
    ];
    parts.push(Box::new(el("div", body).attr("class", "rs-trail-body")));

    Box::new(
        el("div", parts)
            .attr("class", "rs-trail-card")
            .attr("aria-label", format!("Session {}", session.title)),
    )
}

fn day_head(label: &str, listened_ms: u64) -> FullView {
    let children: Vec<FullView> = vec![
        Box::new(el("span", text(label.to_owned()))),
        Box::new(el("span", ()).attr("class", "rs-trail-rule")),
        Box::new(el("span", text(format_span(listened_ms)))),
    ];
    Box::new(
        el("div", children)
            .attr("class", "rs-trail-day")
            .attr("aria-label", format!("Day {label}")),
    )
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let mut children: Vec<FullView> = Vec::new();
    let mut index = 0;
    // Sessions arrive newest first; a day is a run of equal labels.
    while index < state.sessions.len() {
        let label = state.sessions[index].day_label.clone();
        let day: Vec<&ListeningSessionRow> = state.sessions[index..]
            .iter()
            .take_while(|session| session.day_label == label)
            .collect();
        let listened: u64 = day
            .iter()
            .map(|session| session.stop_ms.saturating_sub(session.start_ms))
            .sum();
        index += day.len();
        children.push(day_head(&label, listened));
        children.extend(day.into_iter().map(card));
    }
    Box::new(
        el("div", children)
            .attr("class", "rs-trail")
            .attr("aria-label", "Trail"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_fixture::*;
    use crate::{CompactCommand, RedshankSurfaceState, Scene};
    use cambium::PointerClick;

    fn trail_state() -> RedshankSurfaceState {
        RedshankSurfaceState {
            scene: Scene::Trail,
            ..fixture()
        }
    }

    #[test]
    fn sessions_group_by_day_with_a_total_and_a_week_summary() {
        let runner = runner(trail_state());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert_eq!(markup.matches("rs-trail-card").count(), 6);
        assert_eq!(markup.matches("aria-label=\"Day ").count(), 3);
        assert!(markup.contains("aria-label=\"Day TODAY\""));
        // TODAY carries two sessions: 12:40 plus 17:01.
        assert_eq!(markup.matches("aria-label=\"Session 21").count(), 6);
        assert!(markup.contains("29m"));
        assert!(markup.contains("1h 10m this week"));
    }

    #[test]
    fn the_active_session_is_washed_to_its_playhead_and_carries_its_notes() {
        let runner = runner(trail_state());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("rs-trail-wash-active"));
        assert!(markup.contains("rs-trail-wash-done"));
        assert!(markup.contains("rs-trail-head"));
        assert_eq!(markup.matches("rs-trail-dot").count(), 4);
        assert!(markup.contains("0:00 → 12:40 · now · 3 notes"));
        assert!(markup.contains("7:30 → end · 9:12 pm · completed"));
    }

    #[test]
    fn every_session_action_emits_select_item() {
        let mut runner = runner(trail_state());
        for (label, id) in [
            ("Playing 213 · Four", "e213"),
            ("Resume 212 · Three", "e212"),
            ("Replay 211 · Two", "e211"),
        ] {
            let node = node_with_label(&runner.dom().borrow(), runner.root(), label);
            runner.dispatch_click(node, PointerClick::at((1.0, 1.0)));
            let mut commands = Vec::new();
            runner
                .update(|state: &mut RedshankSurfaceState| commands.extend(state.drain_commands()));
            assert_eq!(
                commands,
                [CompactCommand::SelectItem(item_id(id))],
                "{label}"
            );
        }
    }
}
