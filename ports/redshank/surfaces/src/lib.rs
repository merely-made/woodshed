#![forbid(unsafe_code)]

//! Reusable Redshank listening controls.
//!
//! These views own presentation and interaction only. A host drains
//! [`CompactCommand`] values and applies them through Redshank's player and
//! annotation adapters. The compact surface deliberately has no library,
//! storage, network, or audio-device authority.

use cambium::{AnyView, GenetCtx, GenetElement, button, el, text};
use redshank_model::ItemId;

pub const COMPACT_SHEET: &str = include_str!("compact.css");

pub type CompactView = Box<dyn AnyView<CompactPlayerState, (), GenetCtx, GenetElement>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportState {
    Empty,
    Paused,
    Playing,
    Buffering,
    Unavailable(String),
}

impl TransportState {
    fn spoken_status(&self) -> &str {
        match self {
            Self::Empty => "Nothing loaded",
            Self::Paused => "Paused",
            Self::Playing => "Playing",
            Self::Buffering => "Buffering",
            Self::Unavailable(message) => message,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NowPlaying {
    pub item_id: ItemId,
    pub title: String,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactCommand {
    Play,
    Pause,
    SkipBackward(u64),
    SkipForward(u64),
    AddTextNote,
    BeginVoiceNote,
    FinishVoiceNote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactPlayerState {
    pub transport: TransportState,
    pub now_playing: Option<NowPlaying>,
    pub skip_backward_ms: u64,
    pub skip_forward_ms: u64,
    pub voice_capture_active: bool,
    commands: Vec<CompactCommand>,
}

impl Default for CompactPlayerState {
    fn default() -> Self {
        Self {
            transport: TransportState::Empty,
            now_playing: None,
            skip_backward_ms: 15_000,
            skip_forward_ms: 30_000,
            voice_capture_active: false,
            commands: Vec::new(),
        }
    }
}

impl CompactPlayerState {
    pub fn drain_commands(&mut self) -> impl Iterator<Item = CompactCommand> + '_ {
        self.commands.drain(..)
    }

    pub fn request(&mut self, command: CompactCommand) {
        self.commands.push(command);
    }
}

fn format_time(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if hours == 0 {
        format!("{minutes}:{seconds:02}")
    } else {
        format!("{hours}:{minutes:02}:{seconds:02}")
    }
}

fn enabled(state: &CompactPlayerState) -> bool {
    state.now_playing.is_some()
        && !matches!(
            state.transport,
            TransportState::Empty | TransportState::Unavailable(_)
        )
}

fn control(label: &'static str, shortcut: &'static str, command: CompactCommand) -> CompactView {
    Box::new(
        button(label, move |state: &mut CompactPlayerState, _| {
            if enabled(state) {
                state.request(command.clone());
            }
        })
        .attr("class", "redshank-control")
        .attr("aria-label", label)
        .attr("aria-keyshortcuts", shortcut),
    )
}

pub fn player_surface(state: &CompactPlayerState) -> CompactView {
    let (title, time) = match &state.now_playing {
        Some(item) => {
            let duration = item
                .duration_ms
                .map(format_time)
                .unwrap_or_else(|| "unknown".into());
            (
                item.title.clone(),
                format!("{} / {duration}", format_time(item.position_ms)),
            )
        }
        None => ("Nothing playing".into(), "0:00 / unknown".into()),
    };
    let (play_label, play_command) = if state.transport == TransportState::Playing {
        ("Pause", CompactCommand::Pause)
    } else {
        ("Play", CompactCommand::Play)
    };
    let backward = state.skip_backward_ms;
    let forward = state.skip_forward_ms;

    Box::new(
        el(
            "section",
            (
                el("div", text(title)).attr("class", "redshank-title"),
                el("div", text(state.transport.spoken_status()))
                    .attr("class", "redshank-status")
                    .attr("role", "status"),
                el("div", text(time))
                    .attr("class", "redshank-time")
                    .attr("aria-label", "Playback position"),
                el(
                    "div",
                    (
                        control(
                            "Skip backward",
                            "ArrowLeft",
                            CompactCommand::SkipBackward(backward),
                        ),
                        control(play_label, "Space", play_command),
                        control(
                            "Skip forward",
                            "ArrowRight",
                            CompactCommand::SkipForward(forward),
                        ),
                    ),
                )
                .attr("class", "redshank-controls"),
            ),
        )
        .attr("class", "redshank-player")
        .attr("role", "region")
        .attr("aria-label", "Player"),
    )
}

pub fn capture_surface(state: &CompactPlayerState) -> CompactView {
    let (voice_label, voice_command) = if state.voice_capture_active {
        ("Finish voice note", CompactCommand::FinishVoiceNote)
    } else {
        ("Start voice note", CompactCommand::BeginVoiceNote)
    };
    Box::new(
        el(
            "section",
            (
                control("Add text note", "N", CompactCommand::AddTextNote),
                control(voice_label, "R", voice_command),
            ),
        )
        .attr("class", "redshank-capture")
        .attr("role", "region")
        .attr("aria-label", "Capture"),
    )
}

pub fn compact_surface(state: &CompactPlayerState) -> CompactView {
    Box::new(
        el("div", (player_surface(state), capture_surface(state)))
            .attr("class", "redshank-compact"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium::{DomHandle, GenetAppRunner, PointerClick};
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Logic = fn(&CompactPlayerState) -> CompactView;
    type Runner = GenetAppRunner<CompactPlayerState, Logic, CompactView, ()>;

    fn playing_state() -> CompactPlayerState {
        CompactPlayerState {
            transport: TransportState::Playing,
            now_playing: Some(NowPlaying {
                item_id: ItemId("episode-42".into()),
                title: "Wetland".into(),
                position_ms: 62_000,
                duration_ms: Some(3_723_000),
            }),
            ..CompactPlayerState::default()
        }
    }

    fn runner(logic: Logic) -> Runner {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        Runner::new(dom, logic, playing_state())
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
    fn compact_surface_has_semantic_player_and_capture_controls() {
        let runner = runner(compact_surface);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("aria-label=\"Player\""));
        assert!(markup.contains("aria-label=\"Capture\""));
        assert!(markup.contains("aria-label=\"Pause\" aria-keyshortcuts=\"Space\""));
        assert!(markup.contains("aria-label=\"Add text note\" aria-keyshortcuts=\"N\""));
        assert!(markup.contains("Wetland"));
        assert!(markup.contains("1:02 / 1:02:03"));
    }

    #[test]
    fn player_and_capture_mount_independently_and_emit_commands() {
        let player = runner(player_surface);
        assert!(
            !player
                .dom()
                .borrow()
                .outer_html(player.root())
                .contains("Capture")
        );

        let mut capture = runner(capture_surface);
        assert!(
            !capture
                .dom()
                .borrow()
                .outer_html(capture.root())
                .contains("Player")
        );
        let note = node_with_label(&capture.dom().borrow(), capture.root(), "Add text note");
        capture.dispatch_click(note, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        capture.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::AddTextNote]);
    }

    #[test]
    fn unavailable_transport_rejects_commands() {
        let mut runner = runner(compact_surface);
        runner.update(|state| {
            state.transport = TransportState::Unavailable("Output device unavailable".into());
        });
        let play = node_with_label(&runner.dom().borrow(), runner.root(), "Play");
        runner.dispatch_click(play, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert!(commands.is_empty());
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("Output device unavailable")
        );
    }
}
