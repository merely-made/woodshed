//! Family A bottom dock: identity, seek track, transport, capture. Lane S1 owns this.
//!
//! One dock, fixed height. The identity row, the seek track, and the
//! transport-and-capture row keep their heights in every transport state;
//! recording, buffering, completed, and unavailable swap the row's contents in
//! place. The heights live in `redshank.css`, not here.

use crate::{
    CompactCommand, CompactPlayerState, CompactView, Face, SourceKind, TransportState, format_time,
    percent,
};
use cambium::{PointerButton, PointerPhase, button_with, el, on_pointer, text};

/// Rate cycle: 80 → 100 → 120 → 150 → 80.
fn next_rate(rate: u16) -> u16 {
    match rate {
        r if r < 100 => 100,
        r if r < 120 => 120,
        r if r < 150 => 150,
        _ => 80,
    }
}

/// Volume cycle: 100 → 80 → 60 → 40 → 20 → 100.
fn next_volume(volume: u8) -> u8 {
    match volume {
        v if v > 80 => 80,
        v if v > 60 => 60,
        v if v > 40 => 40,
        v if v > 20 => 20,
        _ => 100,
    }
}

/// A transport or capture button: label and shortcut adjacent so a screen
/// reader and the markup tests read the same pair.
pub(crate) fn control(
    children: Vec<CompactView>,
    class: &str,
    label: String,
    shortcut: Option<&str>,
    enabled: bool,
    command: CompactCommand,
) -> CompactView {
    let class = if enabled {
        class.to_owned()
    } else {
        format!("{class} rs-btn-off")
    };
    let mut view = button_with(children, move |state: &mut CompactPlayerState, _| {
        if enabled {
            state.request(command.clone());
        }
    })
    .attr("class", class)
    .attr("aria-label", label);
    if let Some(shortcut) = shortcut {
        view = view.attr("aria-keyshortcuts", shortcut);
    }
    Box::new(
        view.attr("aria-disabled", if enabled { "false" } else { "true" })
            .attr("tabindex", if enabled { "0" } else { "-1" }),
    )
}

pub(crate) fn label(body: impl Into<String>) -> CompactView {
    Box::new(text(body.into()))
}

pub(crate) fn span(class: &str, body: impl Into<String>) -> CompactView {
    Box::new(el("span", text(body.into())).attr("class", class))
}

/// `LOCAL FILE · LOCAL · PLAYING · RESUMED FROM 1:21`.
pub(crate) fn microlabel(state: &CompactPlayerState) -> String {
    let Some(now) = &state.now_playing else {
        return format!("NO SOURCE · {}", state.transport.micro_word());
    };
    // Without a show, the source kind is the head word, so no badge repeats it.
    let mut line = match &now.feed_title {
        Some(feed) => format!(
            "{} · {} · {}",
            feed.to_uppercase(),
            now.source.badge(),
            state.transport.micro_word()
        ),
        None => {
            let head = match now.source {
                SourceKind::Local => "LOCAL FILE",
                SourceKind::Cloud => "DIRECT URL",
                SourceKind::Offline => "OFFLINE COPY",
                SourceKind::HostBlob => "HOST AUDIO",
            };
            format!("{head} · {}", state.transport.micro_word())
        },
    };
    if let Some(resumed) = now.resumed_from_ms {
        line.push_str(&format!(" · RESUMED FROM {}", format_time(resumed)));
    }
    line
}

pub(crate) fn face_with(state: &CompactPlayerState, class: &str) -> CompactView {
    match state.now_playing.as_ref().map(|now| &now.face) {
        Some(Face::Artwork(url)) => Box::new(
            el("div", ())
                .attr("class", class)
                .attr("style", format!("background-image: url({url});")),
        ),
        Some(Face::Tag(tag)) => Box::new(el("div", text(tag.clone())).attr("class", class)),
        None => Box::new(el("div", ()).attr("class", class)),
    }
}

/// The title the identity row shows: the item, the unavailable message, or the
/// empty state.
pub(crate) fn identity_title(state: &CompactPlayerState) -> String {
    if let TransportState::Unavailable(message) = &state.transport {
        return message.clone();
    }
    match &state.now_playing {
        Some(now) => now.title.clone(),
        None => "Nothing playing".to_owned(),
    }
}

fn identity_row(state: &CompactPlayerState) -> CompactView {
    let (position, duration) = match &state.now_playing {
        Some(now) => (
            format_time(now.position_ms),
            now.duration_ms.map(format_time).unwrap_or("--:--".into()),
        ),
        None => ("0:00".to_owned(), "--:--".to_owned()),
    };
    let text_block: Vec<CompactView> = vec![
        span("rs-micro", microlabel(state)),
        span("rs-title", identity_title(state)),
    ];
    let time: Vec<CompactView> = vec![
        label(position),
        span("rs-time-dim", format!(" / {duration}")),
    ];
    Box::new(
        el(
            "div",
            vec![
                face_with(state, "rs-face"),
                Box::new(el("div", text_block).attr("class", "rs-identity-text")) as CompactView,
                Box::new(el("div", time).attr("class", "rs-time")) as CompactView,
            ],
        )
        .attr("class", "rs-dock-identity"),
    )
}

/// The seek track: buffered extent, played extent, note ticks and span washes,
/// and the ember head. Pressing or dragging seeks to the pointer fraction.
fn seek_row(state: &CompactPlayerState) -> CompactView {
    let now = state.now_playing.as_ref();
    let duration = now.and_then(|now| now.duration_ms);
    let position = now.map(|now| now.position_ms).unwrap_or(0);
    let played = percent(position, duration);
    let buffered = now.map(|now| now.buffered_percent).unwrap_or(0) as f32;
    let seekable = state.transport_enabled() && duration.unwrap_or(0) > 0;

    let mut layers: Vec<CompactView> = vec![
        Box::new(el("span", ()).attr("class", "rs-seek-base")),
        Box::new(
            el("span", ())
                .attr("class", "rs-seek-buffered")
                .attr("style", format!("width: {buffered}%;")),
        ),
        Box::new(
            el("span", ())
                .attr("class", "rs-seek-played")
                .attr("style", format!("width: {played}%;")),
        ),
    ];
    for marker in now.map(|now| now.markers.as_slice()).unwrap_or(&[]) {
        let start = percent(marker.offset_ms, duration);
        match marker.end_offset_ms {
            Some(end) => {
                let width = (percent(end, duration) - start).max(0.5);
                layers.push(Box::new(
                    el("span", ())
                        .attr("class", "rs-seek-span")
                        .attr("style", format!("left: {start}%; width: {width}%;")),
                ));
            },
            None => layers.push(Box::new(
                el("span", ())
                    .attr("class", "rs-seek-tick")
                    .attr("style", format!("left: {start}%;"))
                    .attr("aria-label", marker.kind.micro_word()),
            )),
        }
    }
    layers.push(Box::new(
        el("span", ())
            .attr("class", "rs-seek-head")
            .attr("style", format!("left: {played}%;")),
    ));

    let track = el("div", layers)
        .attr("class", "rs-seek")
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
        )
        .attr("tabindex", if seekable { "0" } else { "-1" });
    let total = duration.unwrap_or(0);
    let track = on_pointer(track, move |state: &mut CompactPlayerState, event| {
        if !seekable || event.button != PointerButton::Primary || event.size.0 <= 0.0 {
            return;
        }
        if matches!(event.phase, PointerPhase::Down | PointerPhase::Move) {
            let fraction = (event.local.0 / event.size.0).clamp(0.0, 1.0) as f64;
            state.request(CompactCommand::Seek((fraction * total as f64) as u64));
            event.prop.prevent_default();
        }
    });
    Box::new(el("div", Box::new(track) as CompactView).attr("class", "rs-seek-row"))
}

/// Play / Pause / Replay / Retry — one slot, one primary action.
pub(crate) fn primary(state: &CompactPlayerState) -> CompactView {
    let enabled = state.transport_enabled();
    match &state.transport {
        TransportState::Completed => control(
            vec![label("Replay")],
            "rs-btn rs-btn-primary rs-btn-wide",
            "Replay".into(),
            Some("Space"),
            true,
            CompactCommand::Replay,
        ),
        TransportState::Unavailable(_) => {
            let item = state.now_playing.as_ref().map(|now| now.item_id.clone());
            match item {
                Some(item) => control(
                    vec![label("Retry")],
                    "rs-btn rs-btn-primary rs-btn-wide",
                    "Retry".into(),
                    None,
                    true,
                    CompactCommand::RetryItem(item),
                ),
                None => control(
                    vec![label("►")],
                    "rs-btn rs-btn-primary",
                    "Play".into(),
                    Some("Space"),
                    false,
                    CompactCommand::Play,
                ),
            }
        },
        TransportState::Playing => control(
            vec![label("❚❚")],
            "rs-btn rs-btn-primary",
            "Pause".into(),
            Some("Space"),
            enabled,
            CompactCommand::Pause,
        ),
        _ => control(
            vec![label("►")],
            "rs-btn rs-btn-primary",
            "Play".into(),
            Some("Space"),
            enabled,
            CompactCommand::Play,
        ),
    }
}

/// The transport half of the row: skips, primary, rate, volume, elastic gap.
fn transport_controls(state: &CompactPlayerState) -> Vec<CompactView> {
    let enabled = state.transport_enabled();
    let back = state.skip_backward_ms;
    let forward = state.skip_forward_ms;
    let rate = state.rate_percent;
    let volume = state.volume_percent;
    vec![
        control(
            vec![label(format!("−{}", back / 1_000))],
            "rs-btn rs-btn-mono",
            "Skip backward".into(),
            Some("ArrowLeft"),
            enabled,
            CompactCommand::SkipBackward(back),
        ),
        primary(state),
        control(
            vec![label(format!("+{}", forward / 1_000))],
            "rs-btn rs-btn-mono",
            "Skip forward".into(),
            Some("ArrowRight"),
            enabled,
            CompactCommand::SkipForward(forward),
        ),
        control(
            vec![label(format!("{:.1}×", rate as f32 / 100.0))],
            "rs-btn rs-btn-quiet rs-rate",
            format!("Playback rate {:.1} times", rate as f32 / 100.0),
            None,
            true,
            CompactCommand::SetRate(next_rate(rate)),
        ),
        control(
            vec![label(format!("vol {volume}"))],
            "rs-btn rs-btn-quiet rs-vol",
            format!("Volume {volume} percent"),
            None,
            true,
            CompactCommand::SetVolume(next_volume(volume)),
        ),
        Box::new(el("span", ()).attr("class", "rs-spacer")),
    ]
}

pub(crate) fn text_note_button(state: &CompactPlayerState) -> CompactView {
    let enabled = state.transport_enabled();
    control(
        vec![
            span("rs-cap-glyph", "T"),
            span("rs-cap-label", "Text note"),
            span("rs-cap-word", "text"),
            span("rs-kbd", "N"),
        ],
        "rs-btn rs-btn-capture rs-capture-control",
        "Add text note".into(),
        Some("N"),
        enabled,
        CompactCommand::AddTextNote,
    )
}

/// Press begins the recording, release finishes it: the anchor freezes on the
/// press, so the command pair is a pointer pair, never a click.
pub(crate) fn voice_note_button(state: &CompactPlayerState) -> CompactView {
    let active = state.voice_capture_active;
    let available = state.transport_enabled() && state.voice_capture_available;
    let name = if active {
        "Release to save voice note"
    } else {
        "Hold to record voice note"
    };
    let class = if available {
        "rs-btn rs-btn-capture rs-capture-control"
    } else {
        "rs-btn rs-btn-capture rs-capture-control rs-btn-off"
    };
    let children: Vec<CompactView> = vec![
        span("rs-cap-glyph", "V"),
        span("rs-cap-label", "Voice note"),
        span("rs-cap-word", "hold"),
        span("rs-kbd", "R · hold"),
    ];
    let view = el("button", children)
        .attr("class", class)
        .attr("aria-label", name)
        .attr("aria-keyshortcuts", "R")
        .attr("aria-pressed", if active { "true" } else { "false" })
        .attr("aria-disabled", if available { "false" } else { "true" })
        .attr("tabindex", if available { "0" } else { "-1" });
    Box::new(on_pointer(
        view,
        move |state: &mut CompactPlayerState, event| {
            if event.button != PointerButton::Primary || !available {
                return;
            }
            match event.phase {
                PointerPhase::Down if !state.voice_capture_active => {
                    state.request(CompactCommand::BeginVoiceNote);
                    event.prop.prevent_default();
                },
                PointerPhase::Up if state.voice_capture_active => {
                    state.request(CompactCommand::FinishVoiceNote);
                    event.prop.prevent_default();
                },
                _ => {},
            }
        },
    ))
}

/// The capture pair, grouped so the dock's phone grid can flatten it.
fn capture_group(state: &CompactPlayerState) -> CompactView {
    Box::new(
        el(
            "span",
            vec![text_note_button(state), voice_note_button(state)],
        )
        .attr("class", "rs-capture")
        .attr("role", "group")
        .attr("aria-label", "Capture"),
    )
}

/// Recording replaces the transport row's contents, at the same height.
fn recording_controls(state: &CompactPlayerState) -> Vec<CompactView> {
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
    let card: Vec<CompactView> = vec![
        Box::new(el("span", ()).attr("class", "rs-rec-dot")),
        span("rs-rec-word", "RECORDING"),
        span("rs-rec-elapsed", format_time(recording.elapsed_ms)),
        Box::new(el("span", anchor).attr("class", "rs-rec-anchor")),
    ];
    vec![
        Box::new(
            el("div", card)
                .attr("class", "rs-recording")
                .attr("role", "status")
                .attr("aria-label", "Recording voice note"),
        ),
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
    ]
}

fn transport_row(state: &CompactPlayerState, with_capture: bool) -> CompactView {
    let mut children = if state.recording.is_some() {
        recording_controls(state)
    } else {
        let mut children = transport_controls(state);
        if with_capture {
            children.push(capture_group(state));
        }
        children
    };
    if children.is_empty() {
        children.push(Box::new(el("span", ()).attr("class", "rs-spacer")));
    }
    Box::new(el("div", children).attr("class", "rs-transport-row rs-transport"))
}

/// The player alone: identity, seek, transport. Hosts that supply their own
/// capture affordance mount this.
pub fn player_surface(state: &CompactPlayerState) -> CompactView {
    Box::new(
        el(
            "section",
            vec![
                identity_row(state),
                seek_row(state),
                transport_row(state, false),
            ],
        )
        .attr("class", "rs-player")
        .attr("role", "region")
        .attr("aria-label", "Player"),
    )
}

/// The capture pair alone, for hosts that mount it beside their own transport.
pub fn capture_surface(state: &CompactPlayerState) -> CompactView {
    Box::new(
        el(
            "section",
            vec![text_note_button(state), voice_note_button(state)],
        )
        .attr("class", "rs-capture")
        .attr("role", "region")
        .attr("aria-label", "Capture"),
    )
}

/// The whole dock: one fixed height, contents swapped in place.
pub fn compact_surface(state: &CompactPlayerState) -> CompactView {
    Box::new(
        el(
            "div",
            vec![
                identity_row(state),
                seek_row(state),
                transport_row(state, true),
            ],
        )
        .attr("class", "rs-dock")
        .attr("role", "region")
        .attr("aria-label", "Player"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NowPlaying, Recording, SourceKind};
    use cambium::{DomHandle, GenetAppRunner, PointerClick, PointerEvent};
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use redshank_model::ItemId;
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
                feed_title: None,
                face: Face::Tag("m4a".into()),
                source: SourceKind::Local,
                position_ms: 62_000,
                duration_ms: Some(3_723_000),
                resumed_from_ms: None,
                buffered_percent: 100,
                markers: Vec::new(),
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
        assert!(markup.contains("aria-label=\"Hold to record voice note\""));
        assert!(markup.contains("aria-keyshortcuts=\"R\""));
        assert!(markup.contains("aria-disabled=\"true\""));
        assert!(markup.contains("aria-pressed=\"false\""));
        assert!(markup.contains("Wetland"));
        assert!(markup.contains("1:02"));
        assert!(markup.contains(" / 1:02:03"));
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

        let voice = node_with_label(
            &capture.dom().borrow(),
            capture.root(),
            "Hold to record voice note",
        );
        capture.dispatch_click(voice, PointerClick::at((1.0, 1.0)));
        capture.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::AddTextNote]);
    }

    #[test]
    fn voice_capture_uses_pointer_press_and_release() {
        let mut capture = runner(capture_surface);
        capture.update(|state| state.voice_capture_available = true);
        let voice = node_with_label(
            &capture.dom().borrow(),
            capture.root(),
            "Hold to record voice note",
        );
        capture.dispatch_pointer_down(
            voice,
            PointerEvent::new(PointerPhase::Down, (1.0, 1.0), (10.0, 10.0)),
        );
        let mut commands = Vec::new();
        capture.update(|state| {
            commands.extend(state.drain_commands());
            state.voice_capture_active = true;
        });
        assert_eq!(commands, [CompactCommand::BeginVoiceNote]);

        capture.dispatch_pointer_up(PointerEvent::new(
            PointerPhase::Up,
            (1.0, 1.0),
            (10.0, 10.0),
        ));
        capture.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [
                CompactCommand::BeginVoiceNote,
                CompactCommand::FinishVoiceNote
            ]
        );
    }

    /// Unavailable keeps the controls in place, rejects transport, shows the
    /// item-scoped message, and offers Retry.
    #[test]
    fn unavailable_transport_rejects_commands() {
        let mut runner = runner(compact_surface);
        runner.update(|state| {
            state.transport = TransportState::Unavailable("Output device unavailable".into());
        });
        let skip = node_with_label(&runner.dom().borrow(), runner.root(), "Skip backward");
        runner.dispatch_click(skip, PointerClick::at((1.0, 1.0)));
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
        let retry = node_with_label(&runner.dom().borrow(), runner.root(), "Retry");
        runner.dispatch_click(retry, PointerClick::at((1.0, 1.0)));
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [CompactCommand::RetryItem(ItemId("episode-42".into()))]
        );
    }

    #[test]
    fn completed_offers_replay() {
        let mut runner = runner(compact_surface);
        runner.update(|state| state.transport = TransportState::Completed);
        let replay = node_with_label(&runner.dom().borrow(), runner.root(), "Replay");
        runner.dispatch_click(replay, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::Replay]);
    }

    #[test]
    fn pressing_the_seek_track_seeks_to_the_pressed_fraction() {
        let mut runner = runner(compact_surface);
        let track = node_with_label(&runner.dom().borrow(), runner.root(), "Seek");
        runner.dispatch_pointer_down(
            track,
            PointerEvent::new(PointerPhase::Down, (25.0, 4.0), (100.0, 16.0)),
        );
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::Seek(930_750)]);
    }

    #[test]
    fn recording_swaps_the_transport_row_and_keeps_the_rows() {
        let mut runner = runner(compact_surface);
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
        assert!(markup.contains("rs-rec-dot"));
        assert_eq!(markup.matches("class=\"rs-transport-row").count(), 1);
        let cancel = node_with_label(&runner.dom().borrow(), runner.root(), "Cancel voice note");
        runner.dispatch_click(cancel, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::CancelVoiceNote]);
    }

    #[test]
    fn rate_and_volume_cycle() {
        assert_eq!(next_rate(100), 120);
        assert_eq!(next_rate(150), 80);
        assert_eq!(next_volume(80), 60);
        assert_eq!(next_volume(20), 100);
        let mut runner = runner(compact_surface);
        let rate = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Playback rate 1.0 times",
        );
        runner.dispatch_click(rate, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::SetRate(120)]);
    }
}
