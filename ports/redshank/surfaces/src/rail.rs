//! Family B left transport rail. Lane S1 owns this.
//!
//! The same facts and the same commands as the dock, stacked: identity, a
//! vertical seek track that takes the flexible middle, transport, capture.

use crate::dock::{
    control, face_with, identity_title, label, microlabel, primary, span, text_note_button,
    voice_note_button,
};
use crate::{CompactCommand, CompactPlayerState, CompactView, format_time, percent};
use cambium::{el, text};

/// The vertical track: played extent from the top, ticks across it, the ember
/// head at the position.
fn seek(state: &CompactPlayerState) -> CompactView {
    let now = state.now_playing.as_ref();
    let duration = now.and_then(|now| now.duration_ms);
    let position = now.map(|now| now.position_ms).unwrap_or(0);
    let played = percent(position, duration);
    let buffered = now.map(|now| now.buffered_percent).unwrap_or(0) as f32;

    let mut layers: Vec<CompactView> = vec![
        Box::new(
            el("span", ())
                .attr("class", "rs-seek-buffered")
                .attr("style", format!("height: {buffered}%;")),
        ),
        Box::new(
            el("span", ())
                .attr("class", "rs-seek-played")
                .attr("style", format!("height: {played}%;")),
        ),
    ];
    for marker in now.map(|now| now.markers.as_slice()).unwrap_or(&[]) {
        let start = percent(marker.offset_ms, duration);
        match marker.end_offset_ms {
            Some(end) => {
                let height = (percent(end, duration) - start).max(0.5);
                layers.push(Box::new(
                    el("span", ())
                        .attr("class", "rs-seek-span")
                        .attr("style", format!("top: {start}%; height: {height}%;")),
                ));
            },
            None => layers.push(Box::new(
                el("span", ())
                    .attr("class", "rs-seek-tick")
                    .attr("style", format!("top: {start}%;")),
            )),
        }
    }
    layers.push(Box::new(
        el("span", ())
            .attr("class", "rs-seek-head")
            .attr("style", format!("top: {played}%;")),
    ));

    let track = el("div", layers)
        .attr("class", "rs-rail-seek")
        .attr("role", "slider")
        .attr("aria-label", "Seek")
        .attr("aria-valuemin", "0")
        .attr("aria-valuemax", duration.unwrap_or(0).to_string())
        .attr("aria-valuenow", position.to_string())
        .attr(
            "aria-valuetext",
            match duration {
                Some(duration) => format!("{} of {}", format_time(position), format_time(duration)),
                None => format_time(position),
            },
        );
    let scale: Vec<CompactView> = vec![
        Box::new(text("0:00")),
        span("rs-time", format_time(position)),
        Box::new(text(
            duration.map(format_time).unwrap_or("--:--".to_owned()),
        )),
    ];
    Box::new(
        el(
            "div",
            vec![
                Box::new(track) as CompactView,
                Box::new(el("div", scale).attr("class", "rs-rail-scale")),
            ],
        )
        .attr("class", "rs-rail-seek-row"),
    )
}

/// The recording card: a word, the danger colour, the frozen ember anchor, and
/// the two ways out.
fn recording_card(state: &CompactPlayerState) -> Vec<CompactView> {
    let Some(recording) = &state.recording else {
        return Vec::new();
    };
    let anchor: Vec<CompactView> = vec![
        label("anchor frozen at "),
        span("rs-ember", format_time(recording.anchor_ms)),
        label(if recording.paused_for_capture {
            " · playback paused"
        } else {
            " · playback continues"
        }),
    ];
    let head: Vec<CompactView> = vec![
        Box::new(el("span", ()).attr("class", "rs-rec-dot")),
        label("RECORDING"),
    ];
    let actions: Vec<CompactView> = vec![
        control(
            vec![label("Finish")],
            "rs-btn rs-btn-primary rs-btn-wide",
            "Finish voice note".into(),
            Some("R"),
            true,
            CompactCommand::FinishVoiceNote,
        ),
        control(
            vec![label("Cancel")],
            "rs-btn",
            "Cancel voice note".into(),
            Some("Escape"),
            true,
            CompactCommand::CancelVoiceNote,
        ),
    ];
    vec![Box::new(
        el(
            "div",
            vec![
                Box::new(el("span", head).attr("class", "rs-rec-word")) as CompactView,
                span("rs-rec-elapsed", format_time(recording.elapsed_ms)),
                Box::new(el("span", anchor).attr("class", "rs-rec-anchor")),
                Box::new(el("div", actions).attr("class", "rs-rail-pair")),
            ],
        )
        .attr("class", "rs-rail-recording")
        .attr("role", "status")
        .attr("aria-label", "Recording voice note"),
    )]
}

pub fn rail_surface(state: &CompactPlayerState) -> CompactView {
    let enabled = state.transport_enabled();
    let back = state.skip_backward_ms;
    let forward = state.skip_forward_ms;
    let identity: Vec<CompactView> = vec![
        span("rs-micro", microlabel(state)),
        span("rs-rail-title", identity_title(state)),
    ];
    let head: Vec<CompactView> = vec![
        Box::new(
            el("span", ())
                .attr("class", "rs-mark")
                .attr("aria-label", "Merely"),
        ),
        span("rs-wordmark", "Redshank"),
    ];
    let pair: Vec<CompactView> = vec![
        control(
            vec![label(format!("−{}", back / 1_000))],
            "rs-btn rs-btn-mono",
            "Skip backward".into(),
            Some("ArrowLeft"),
            enabled,
            CompactCommand::SkipBackward(back),
        ),
        control(
            vec![label(format!("+{}", forward / 1_000))],
            "rs-btn rs-btn-mono",
            "Skip forward".into(),
            Some("ArrowRight"),
            enabled,
            CompactCommand::SkipForward(forward),
        ),
    ];

    let mut children: Vec<CompactView> = vec![
        Box::new(el("div", head).attr("class", "rs-rail-head")),
        face_with(state, "rs-rail-face"),
        Box::new(el("div", identity).attr("class", "rs-identity-text")),
    ];
    children.extend(recording_card(state));
    children.push(seek(state));
    children.push(primary(state));
    children.push(Box::new(el("div", pair).attr("class", "rs-rail-pair")));
    children.push(text_note_button(state));
    children.push(voice_note_button(state));

    Box::new(
        el("aside", children)
            .attr("class", "rs-rail")
            .attr("role", "region")
            .attr("aria-label", "Player"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Face, NowPlaying, Recording, SourceKind, TransportState};
    use cambium::{DomHandle, GenetAppRunner, PointerClick};
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use redshank_model::ItemId;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Logic = fn(&CompactPlayerState) -> CompactView;
    type Runner = GenetAppRunner<CompactPlayerState, Logic, CompactView, ()>;

    fn state() -> CompactPlayerState {
        CompactPlayerState {
            transport: TransportState::Playing,
            now_playing: Some(NowPlaying {
                item_id: ItemId("episode-42".into()),
                title: "Wetland".into(),
                feed_title: None,
                face: Face::Tag("m4a".into()),
                source: SourceKind::Local,
                position_ms: 84_000,
                duration_ms: Some(121_000),
                resumed_from_ms: None,
                buffered_percent: 100,
                markers: Vec::new(),
            }),
            ..CompactPlayerState::default()
        }
    }

    fn runner() -> Runner {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        Runner::new(dom, rail_surface as Logic, state())
    }

    fn node_with_label(dom: &ScriptedDom, root: NodeId, label: &str) -> NodeId {
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

    #[test]
    fn rail_carries_the_same_facts_and_commands() {
        let mut runner = runner();
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("class=\"rs-rail\""));
        assert!(markup.contains("class=\"rs-rail-seek\""));
        assert!(markup.contains("LOCAL FILE · PLAYING"));
        assert!(markup.contains("Wetland"));
        let pause = node_with_label(&runner.dom().borrow(), runner.root(), "Pause");
        runner.dispatch_click(pause, PointerClick::at((1.0, 1.0)));
        let note = node_with_label(&runner.dom().borrow(), runner.root(), "Add text note");
        runner.dispatch_click(note, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [CompactCommand::Pause, CompactCommand::AddTextNote]
        );
    }

    #[test]
    fn recording_rail_shows_the_frozen_anchor_and_both_exits() {
        let mut runner = runner();
        runner.update(|state| {
            state.recording = Some(Recording {
                elapsed_ms: 4_000,
                anchor_ms: 84_000,
                paused_for_capture: true,
            });
        });
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("RECORDING"));
        assert!(markup.contains("anchor frozen at "));
        let finish = node_with_label(&runner.dom().borrow(), runner.root(), "Finish voice note");
        runner.dispatch_click(finish, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::FinishVoiceNote]);
    }
}
