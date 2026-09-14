//! Shared scene math: episode ordering, node state, ring placement, labels.
//!
//! Lane S3 owns this file. The three projections differ in geometry only; the
//! facts they read and the words they print come from here.

use crate::{ItemRow, NoteSummary, RedshankSurfaceState, SourceKind, format_time};
use redshank_model::ItemId;

/// Which of the four episode states a node paints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeState {
    Unplayed,
    Listened,
    Progress,
    Now,
}

impl NodeState {
    /// The `rs-node` state modifier, empty for the default.
    pub fn modifier(self) -> &'static str {
        match self {
            Self::Unplayed => "",
            Self::Listened => " rs-node-listened",
            Self::Progress => " rs-node-progress",
            Self::Now => " rs-node-now",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Unplayed => "unplayed",
            Self::Listened => "listened",
            Self::Progress => "in progress",
            Self::Now => "now",
        }
    }
}

/// The item the dock is playing, when there is one.
pub fn now_playing(state: &RedshankSurfaceState) -> Option<&ItemId> {
    state.compact.now_playing.as_ref().map(|now| &now.item_id)
}

pub fn node_state(item: &ItemRow, now: Option<&ItemId>) -> NodeState {
    if now == Some(&item.id) {
        NodeState::Now
    } else if item.completed {
        NodeState::Listened
    } else if item.position_ms > 0 {
        NodeState::Progress
    } else {
        NodeState::Unplayed
    }
}

/// The `rs-node` class for an episode, plus any scene-local extra.
pub fn node_class(state: NodeState, extra: &str) -> String {
    format!("rs-node{}{extra}", state.modifier())
}

/// One feed's episodes, newest first: `published` descending where present,
/// title descending otherwise. Reverse for the oldest-to-newest wide chain.
pub fn episodes<'a>(state: &'a RedshankSurfaceState, feed_url: Option<&str>) -> Vec<&'a ItemRow> {
    let mut rows: Vec<&ItemRow> = state
        .items
        .iter()
        .filter(|item| item.feed_url.as_deref() == feed_url)
        .collect();
    rows.sort_by(|a, b| {
        let left = (a.published.as_deref().unwrap_or(""), a.title.as_str());
        let right = (b.published.as_deref().unwrap_or(""), b.title.as_str());
        right.cmp(&left)
    });
    rows
}

/// Oldest first, the order the wide chain runs in.
pub fn episodes_oldest_first<'a>(
    state: &'a RedshankSurfaceState,
    feed_url: Option<&str>,
) -> Vec<&'a ItemRow> {
    let mut rows = episodes(state, feed_url);
    rows.reverse();
    rows
}

/// The notes hanging off one episode, in offset order.
pub fn notes_for<'a>(state: &'a RedshankSurfaceState, id: &ItemId) -> Vec<&'a NoteSummary> {
    let mut notes: Vec<&NoteSummary> = state
        .notes
        .iter()
        .filter(|note| &note.item_id == id)
        .collect();
    notes.sort_by_key(|note| note.offset_ms);
    notes
}

/// A point on a ring of `radius`, index `0` at twelve o'clock, running
/// clockwise. Returns an offset from the centre in pixels.
pub fn ring_point(index: usize, count: usize, radius: f32) -> (f32, f32) {
    let count = count.max(1);
    let angle = index as f32 / count as f32 * std::f32::consts::TAU;
    (radius * angle.sin(), -radius * angle.cos())
}

/// The short node label: the leading `213` of a `213 · Title`, else the
/// publication label, else the title. The state carries no episode number, so
/// this reads one off the title when the host wrote one there.
pub fn short_label(item: &ItemRow) -> String {
    if let Some((head, _)) = item.title.split_once(" · ")
        && head.len() <= 8
    {
        return head.to_owned();
    }
    item.published.clone().unwrap_or_else(|| item.title.clone())
}

/// The badge word beside an episode: `NOW` outranks the source kind.
pub fn badge(item: &ItemRow, now: Option<&ItemId>) -> &'static str {
    if now == Some(&item.id) {
        "NOW"
    } else {
        item.source.badge()
    }
}

/// `Sep 4 · 12:40 / 32:05 · NOW` — date, progress, badge.
pub fn meta_line(item: &ItemRow, now: Option<&ItemId>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(published) = &item.published {
        parts.push(published.clone());
    }
    let state = node_state(item, now);
    parts.push(match (state, item.duration_ms) {
        (NodeState::Listened, _) => "completed".to_owned(),
        (_, Some(duration)) if item.position_ms > 0 => {
            format!(
                "{} / {}",
                format_time(item.position_ms),
                format_time(duration)
            )
        },
        (_, Some(duration)) => format_time(duration),
        _ => "unplayed".to_owned(),
    });
    parts.push(badge(item, now).to_owned());
    parts.join(" · ")
}

/// The single primary action word for an episode row.
pub fn action_word(item: &ItemRow, now: Option<&ItemId>) -> &'static str {
    match node_state(item, now) {
        NodeState::Now => "Playing",
        NodeState::Listened => "Replay",
        NodeState::Progress => "Resume",
        NodeState::Unplayed => "Play",
    }
}

/// `3 notes · 2 text · 1 voice`, or `None` when the episode carries none.
pub fn note_tally(notes: &[&NoteSummary]) -> Option<String> {
    if notes.is_empty() {
        return None;
    }
    let voice = notes
        .iter()
        .filter(|note| note.kind() == crate::NoteKind::Voice)
        .count();
    let text = notes.len() - voice;
    let mut line = format!("{} notes", notes.len());
    if text > 0 {
        line.push_str(&format!(" · {text} text"));
    }
    if voice > 0 {
        line.push_str(&format!(" · {voice} voice"));
    }
    Some(line)
}

/// `0:46 1:02 1:16`.
pub fn note_times(notes: &[&NoteSummary]) -> String {
    notes
        .iter()
        .map(|note| format_time(note.offset_ms))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The representation line the context card prints. The state carries no
/// digest comparison, so the source kind stands in for it.
pub fn representation(item: &ItemRow) -> &'static str {
    match item.source {
        SourceKind::Offline => "cached · matches",
        SourceKind::Local => "local file",
        SourceKind::HostBlob => "host blob",
        SourceKind::Cloud => "cloud",
    }
}

/// One note's one-line body: the text preview, or `voice · 0:06`.
pub fn note_body(note: &NoteSummary) -> String {
    match &note.body {
        crate::NoteSummaryBody::Text(body) => body.clone(),
        crate::NoteSummaryBody::Voice { duration_ms, .. } => {
            format!("voice · {}", format_time(*duration_ms))
        },
    }
}
