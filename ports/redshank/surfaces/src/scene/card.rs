//! The context card summoned beside the selected episode. Lane S3 owns this file.

use super::geometry;
use crate::{CompactCommand, FullView, ItemRow, RedshankSurfaceState, format_time};
use cambium::{PointerClick, button, el, text};

/// The selected episode's neighbours in its own feed, by date.
fn neighbours<'a>(
    state: &'a RedshankSurfaceState,
    item: &ItemRow,
) -> (Option<&'a ItemRow>, Option<&'a ItemRow>) {
    let chain = geometry::episodes_oldest_first(state, item.feed_url.as_deref());
    let at = chain.iter().position(|row| row.id == item.id);
    match at {
        Some(index) => (
            index.checked_sub(1).and_then(|i| chain.get(i).copied()),
            chain.get(index + 1).copied(),
        ),
        None => (None, None),
    }
}

fn labelled(value: String, label: &str) -> FullView {
    Box::new(
        el(
            "div",
            (
                Box::new(el("span", text(value))) as FullView,
                Box::new(el("span", text(label.to_owned())).attr("class", "rs-micro")) as FullView,
            ),
        )
        .attr("class", "rs-card-line"),
    )
}

fn action(label: &'static str, command: CompactCommand) -> FullView {
    Box::new(
        button(
            label,
            move |state: &mut RedshankSurfaceState, _: PointerClick| {
                state.request(command.clone());
            },
        )
        .attr("class", "rs-card-action")
        .attr("aria-label", label),
    )
}

/// The wide `CONNECTIONS` card: identity, neighbours, notes, representation,
/// and the three actions.
pub fn connections_card(state: &RedshankSurfaceState) -> Option<FullView> {
    let item = state.selected_item()?;
    let now = geometry::now_playing(state);
    let notes = geometry::notes_for(state, &item.id);
    let (previous, next) = neighbours(state, item);

    let mut lines: Vec<FullView> = vec![
        Box::new(el("div", text("CONNECTIONS")).attr("class", "rs-micro")),
        Box::new(el("div", text(item.title.clone())).attr("class", "rs-card-title")),
        Box::new(
            el(
                "div",
                text(format!(
                    "guid {} · {} · {}",
                    item.id.0,
                    item.duration_ms.map_or("—".to_owned(), format_time),
                    geometry::badge(item, now),
                )),
            )
            .attr("class", "rs-card-guid"),
        ),
    ];
    if let Some(previous) = previous {
        lines.push(labelled(format!("← {}", previous.title), "prev"));
    }
    if let Some(next) = next {
        lines.push(labelled(format!("→ {}", next.title), "next"));
    }
    if let Some(tally) = geometry::note_tally(&notes) {
        lines.push(labelled(tally, &geometry::note_times(&notes)));
    }
    lines.push(labelled(
        "representation".to_owned(),
        geometry::representation(item),
    ));

    let actions: Vec<FullView> = vec![
        action("Open", CompactCommand::SelectItem(item.id.clone())),
        action("Pin", CompactCommand::PinItem(item.id.clone())),
        action("Add to queue", CompactCommand::Enqueue(item.id.clone())),
    ];

    Some(Box::new(
        el(
            "aside",
            (
                Box::new(el("div", lines).attr("class", "rs-card-body")) as FullView,
                Box::new(el("div", actions).attr("class", "rs-card-actions")) as FullView,
            ),
        )
        .attr("class", "rs-card")
        .attr("aria-label", "Selected episode connections"),
    ))
}

/// The orrery's one-line variant: `SELECTED · CONNECTIONS`, title, neighbours
/// and note times on one row, and Open.
pub fn compact_card(state: &RedshankSurfaceState) -> Option<FullView> {
    let item = state.selected_item()?;
    let notes = geometry::notes_for(state, &item.id);
    let (previous, next) = neighbours(state, item);
    let mut connections: Vec<String> = Vec::new();
    if let Some(previous) = previous {
        connections.push(format!("← {}", previous.title));
    }
    if let Some(next) = next {
        connections.push(format!("→ {}", next.title));
    }
    if !notes.is_empty() {
        connections.push(format!("notes {}", geometry::note_times(&notes)));
    }

    let body: Vec<FullView> = vec![
        Box::new(el("div", text("SELECTED · CONNECTIONS")).attr("class", "rs-micro")),
        Box::new(el("div", text(item.title.clone())).attr("class", "rs-card-title")),
        Box::new(el("div", text(connections.join(" · "))).attr("class", "rs-card-guid")),
    ];

    Some(Box::new(
        el(
            "aside",
            (
                Box::new(el("div", body).attr("class", "rs-card-body")) as FullView,
                action("Open", CompactCommand::SelectItem(item.id.clone())),
            ),
        )
        .attr("class", "rs-card")
        .attr("aria-label", "Selected episode connections"),
    ))
}
