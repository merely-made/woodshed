//! Row, face, badge, and note-row builders shared by the tab bodies.
//!
//! Lane S2 owns this file. Every builder emits only contract classes
//! (`rs-row`, `rs-face`, `rs-badge`, `rs-micro`, `rs-kbd`, `rs-card`) plus
//! tab-local classes the tab sheets style.

use crate::{
    CompactCommand, Face, FullView, MenuTarget, NoteKind, NoteSummary, NoteSummaryBody,
    RedshankSurfaceState, VoiceNotePreview, format_time,
};
use cambium::{button, button_with, el, text};

/// The canvas draws the trigger as a vertical ellipsis. U+22EE is not in the
/// fallback faces this host has (the first receipt printed tofu), so the three
/// dots are drawn as boxes instead.
fn more_dots() -> FullView {
    let dots: Vec<FullView> = (0..3)
        .map(|_| Box::new(el("span", text("")).attr("class", "rs-more-dot")) as FullView)
        .collect();
    Box::new(el("span", dots).attr("class", "rs-more-dots"))
}

/// Tracked mono microlabel.
pub fn micro(value: impl Into<String>) -> FullView {
    Box::new(el("span", text(value.into())).attr("class", "rs-micro"))
}

/// Count or state badge.
pub fn badge(value: impl Into<String>) -> FullView {
    Box::new(el("span", text(value.into())).attr("class", "rs-badge"))
}

/// A keyboard hint chip.
pub fn kbd(value: impl Into<String>) -> FullView {
    Box::new(el("span", text(value.into())).attr("class", "rs-kbd"))
}

/// The square face. Artwork carries its url as `data-artwork`; hosts that can
/// load it paint it, and the tag text is the fallback the DOM always shows.
pub fn face(face: &Face, extra: &str) -> FullView {
    let class = if extra.is_empty() {
        "rs-face".to_owned()
    } else {
        format!("rs-face {extra}")
    };
    match face {
        Face::Artwork(url) => Box::new(
            el("span", text(""))
                .attr("class", class)
                .attr("data-artwork", url.clone()),
        ),
        Face::Tag(tag) => Box::new(el("span", text(tag.clone())).attr("class", class)),
    }
}

/// A section head: tracked word on the left, mono summary on the right.
pub fn section_head(word: &str, summary: impl Into<String>) -> FullView {
    Box::new(
        el("div", (micro(word.to_owned()), micro(summary.into()))).attr("class", "rs-section-head"),
    )
}

/// A button that emits one command.
pub fn action(
    label: impl Into<String>,
    aria: impl Into<String>,
    class: &'static str,
    command: CompactCommand,
) -> FullView {
    let aria = aria.into();
    Box::new(
        button(label.into(), move |state: &mut RedshankSurfaceState, _| {
            state.request(command.clone());
        })
        .attr("class", class)
        .attr("aria-label", aria),
    )
}

/// A button that is present but refuses its command (the "Playing" primary).
pub fn inert(label: impl Into<String>, aria: impl Into<String>, class: &'static str) -> FullView {
    Box::new(
        button(label.into(), |_: &mut RedshankSurfaceState, _| {})
            .attr("class", class)
            .attr("aria-label", aria.into())
            .attr("aria-disabled", "true")
            .attr("tabindex", "-1"),
    )
}

/// The quiet dashed row an empty section shows.
pub fn empty_row(message: &str) -> FullView {
    Box::new(el("p", text(message.to_owned())).attr("class", "rs-row rs-empty"))
}

/// A 3px progress bar; `dim` paints a completed item in the dim role.
pub fn bar(fraction: f32, dim: bool, class: &'static str) -> FullView {
    let width = (fraction * 100.0).clamp(0.0, 100.0);
    let fill = el("span", text(""))
        .attr(
            "class",
            if dim {
                "rs-bar-fill rs-bar-done"
            } else {
                "rs-bar-fill"
            },
        )
        .attr("style", format!("width:{width:.0}%"));
    Box::new(el("div", fill).attr("class", class))
}

/// `2 text · 1 voice`.
pub fn note_mix(notes: &[NoteSummary]) -> String {
    let voice = notes
        .iter()
        .filter(|note| note.kind() == NoteKind::Voice)
        .count();
    format!("{} text · {voice} voice", notes.len() - voice)
}

fn voice_status(duration_ms: u64, preview: &VoiceNotePreview) -> String {
    let duration = format_time(duration_ms);
    match preview {
        VoiceNotePreview::Idle => format!("{duration} · your voice"),
        VoiceNotePreview::Loading => format!("{duration} · loading"),
        VoiceNotePreview::Playing { position_ms } => {
            format!("{} / {duration}", format_time(*position_ms))
        },
        VoiceNotePreview::Ended => format!("{duration} · finished"),
        VoiceNotePreview::Unavailable(message) => format!("{duration} · {message}"),
    }
}

/// The static bar cluster the design draws for a voice note.
fn voice_bars() -> FullView {
    const HEIGHTS: [u32; 9] = [6, 12, 9, 16, 7, 11, 5, 13, 8];
    let bars: Vec<FullView> = HEIGHTS
        .iter()
        .map(|height| {
            Box::new(
                el("span", text(""))
                    .attr("class", "rs-voice-bar")
                    .attr("style", format!("height:{height}px")),
            ) as FullView
        })
        .collect();
    Box::new(el("span", bars).attr("class", "rs-voice-bars"))
}

fn preview_control(id: &redshank_model::AnnotationId, preview: &VoiceNotePreview) -> FullView {
    let id = id.clone();
    let (label, command) = match preview {
        VoiceNotePreview::Idle => ("Play", CompactCommand::PlayVoiceNote(id)),
        VoiceNotePreview::Loading | VoiceNotePreview::Playing { .. } => {
            ("Stop", CompactCommand::StopVoiceNote(id))
        },
        VoiceNotePreview::Ended => ("Replay", CompactCommand::PlayVoiceNote(id)),
        VoiceNotePreview::Unavailable(_) => ("Retry", CompactCommand::PlayVoiceNote(id)),
    };
    action(
        label,
        format!("{label} voice note"),
        "rs-note-preview",
        command,
    )
}

/// The body column: text, or the voice bar cluster with its transport.
pub fn note_body(note: &NoteSummary) -> FullView {
    match &note.body {
        NoteSummaryBody::Text(body) => {
            Box::new(el("span", text(body.clone())).attr("class", "rs-note-text"))
        },
        NoteSummaryBody::Voice {
            duration_ms,
            preview,
        } => Box::new(
            el(
                "span",
                (
                    preview_control(&note.id, preview),
                    voice_bars(),
                    el("span", text(voice_status(*duration_ms, preview)))
                        .attr("class", "rs-note-voice-time"),
                ),
            )
            .attr("class", "rs-note-voice"),
        ),
    }
}

/// The type/anchor column: `TEXT` over `0:46`.
pub fn note_anchor(note: &NoteSummary) -> FullView {
    Box::new(
        el(
            "span",
            (
                micro(note.kind().micro_word()),
                el("span", text(format_time(note.offset_ms))).attr("class", "rs-note-time"),
            ),
        )
        .attr("class", "rs-note-anchor"),
    )
}

/// The overflow trigger and, when this row owns the open menu, its actions.
///
/// One menu at a time falls out of the state holding a single target: opening
/// one replaces the other. Cambium's `overlay_surface` wants a measured
/// trigger rect, panel size, and bounds, none of which a view in flow knows,
/// so the panel is an inline strip rather than a positioned popover.
pub fn overflow(
    state: &RedshankSurfaceState,
    target: MenuTarget,
    subject: &str,
    actions: Vec<FullView>,
) -> FullView {
    let open = state.menu_is_open(&target);
    let next = (!open).then_some(target);
    let trigger = Box::new(
        button_with(more_dots(), move |state: &mut RedshankSurfaceState, _| {
            state.request(CompactCommand::ToggleMenu(next.clone()));
        })
        .attr("class", "rs-row-more")
        .attr("aria-label", format!("More actions for {subject}"))
        .attr("aria-haspopup", "menu")
        .attr("aria-expanded", if open { "true" } else { "false" }),
    ) as FullView;
    let menu = open.then(|| {
        el("span", actions)
            .attr("class", "rs-row-actions")
            .attr("role", "menu")
            .attr("aria-label", format!("Actions for {subject}"))
    });
    Box::new(el("span", (trigger, menu)).attr("class", "rs-row-menu"))
}

/// Open at stays outside the menu; Edit (text only) and Delete go in it.
pub fn note_actions(state: &RedshankSurfaceState, note: &NoteSummary) -> FullView {
    let at = format_time(note.offset_ms);
    let open_at = action(
        format!("Open at {at}"),
        format!("Open note at {at}"),
        "rs-note-open",
        CompactCommand::OpenNote(note.id.clone()),
    );
    let mut actions = Vec::new();
    if note.kind() == NoteKind::Text {
        actions.push(action(
            "Edit",
            format!("Edit note at {at}"),
            "rs-row-action",
            CompactCommand::BeginEditNote(note.id.clone()),
        ));
    }
    actions.push(action(
        "Delete",
        format!("Delete note at {at}"),
        "rs-row-action",
        CompactCommand::DeleteNote(note.id.clone()),
    ));
    let menu = overflow(
        state,
        MenuTarget::Note(note.id.clone()),
        &format!("note at {at}"),
        actions,
    );
    Box::new(el("span", (open_at, menu)).attr("class", "rs-row-tail"))
}

/// One note row, as the Listen and Notes tabs both show it.
pub fn note_row(state: &RedshankSurfaceState, note: &NoteSummary) -> FullView {
    Box::new(
        el(
            "div",
            (
                note_anchor(note),
                note_body(note),
                note_actions(state, note),
            ),
        )
        .attr("class", "rs-row rs-note-row"),
    )
}

/// The small filled square a pinned item carries before its title.
pub fn pin_mark(pinned: bool) -> Option<FullView> {
    pinned.then(|| {
        Box::new(
            el("span", text(""))
                .attr("class", "rs-pin-mark")
                .attr("aria-label", "Pinned"),
        ) as FullView
    })
}
