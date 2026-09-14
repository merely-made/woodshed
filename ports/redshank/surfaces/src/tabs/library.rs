//! The library tab body: subscription roster, episode list, quiet field.
//! Lane S2 owns this file.

use super::rows;
use crate::{
    CompactCommand, FeedRow, FullView, ItemRow, RedshankSurfaceState, Scene, SourceKind,
    TransportState, format_bytes, format_time, scene,
};
use cambium::{TextInput, button, el, lens, text, textarea};

fn feed_detail(feed: &FeedRow) -> String {
    if let Some(failure) = &feed.failure {
        return format!("{failure} · retry");
    }
    let offline = if feed.offline_bytes == 0 {
        "nothing offline".to_owned()
    } else {
        format!("{} offline", format_bytes(feed.offline_bytes))
    };
    format!(
        "{} episodes · {} unplayed · {offline}",
        feed.episode_count, feed.unplayed_count
    )
}

fn feed_row(state: &RedshankSurfaceState, feed: &FeedRow) -> FullView {
    let selected = state
        .current_feed()
        .is_some_and(|current| current.feed_url == feed.feed_url);
    let url = feed.feed_url.clone();
    let title = feed.title.clone();
    let select_title = title.clone();
    let select = Box::new(
        button(title.clone(), move |state: &mut RedshankSurfaceState, _| {
            state.request(CompactCommand::SelectFeed(Some(url.clone())));
        })
        .attr("class", "rs-library-title")
        .attr("aria-label", format!("Select {select_title}")),
    ) as FullView;
    let detail_command = if feed.failure.is_some() {
        Some(CompactCommand::RefreshSubscription(feed.feed_url.clone()))
    } else {
        None
    };
    let detail: FullView = match detail_command {
        Some(command) => rows::action(
            feed_detail(feed),
            format!("Retry refresh of {title}"),
            "rs-library-detail",
            command,
        ),
        None => Box::new(el("div", text(feed_detail(feed))).attr("class", "rs-library-detail")),
    };
    let refresh = rows::action(
        "Refresh",
        format!("Refresh {title}"),
        "rs-row-action",
        CompactCommand::RefreshSubscription(feed.feed_url.clone()),
    );
    let unplayed = (feed.unplayed_count > 0).then(|| rows::badge(feed.unplayed_count.to_string()));
    Box::new(
        el(
            "div",
            (
                el("span", (rows::face(&feed.face, ""), unplayed)).attr("class", "rs-library-face"),
                el("div", (select, detail)).attr("class", "rs-library-cell"),
                el("span", refresh).attr("class", "rs-row-actions"),
            ),
        )
        .attr(
            "class",
            if selected {
                "rs-row rs-row-active rs-library-row"
            } else {
                "rs-row rs-library-row"
            },
        ),
    )
}

/// The `Local audio` row: everything that hangs off no feed.
fn local_row(state: &RedshankSurfaceState) -> FullView {
    let local: Vec<&ItemRow> = state
        .items
        .iter()
        .filter(|item| item.feed_url.is_none())
        .collect();
    let placeholders = local
        .iter()
        .filter(|item| item.source == SourceKind::Cloud)
        .count();
    let selected = state.selected_feed.is_none() && state.current_feed().is_none();
    let select = Box::new(
        button("Local audio", |state: &mut RedshankSurfaceState, _| {
            state.request(CompactCommand::SelectFeed(None));
        })
        .attr("class", "rs-library-title")
        .attr("aria-label", "Select local audio"),
    ) as FullView;
    Box::new(
        el(
            "div",
            (
                rows::face(&crate::Face::Tag(local.len().to_string()), ""),
                el(
                    "div",
                    (
                        select,
                        el(
                            "div",
                            text(format!(
                                "{} files · {placeholders} cloud placeholder",
                                local.len()
                            )),
                        )
                        .attr("class", "rs-library-detail"),
                    ),
                )
                .attr("class", "rs-library-cell"),
            ),
        )
        .attr(
            "class",
            if selected {
                "rs-row rs-row-active rs-library-row"
            } else {
                "rs-row rs-library-row"
            },
        ),
    )
}

/// The quiet subscribe field. The wrapper keeps `redshank-feed-url`: the
/// desktop's focused-text hook keys on that class.
fn subscribe_field() -> FullView {
    let field = Box::new(
        el(
            "div",
            Box::new(lens(
                |input: &mut TextInput| textarea(input),
                |state: &mut RedshankSurfaceState| &mut state.feed_url_editor,
            )) as FullView,
        )
        .attr("class", "redshank-feed-url"),
    ) as FullView;
    let subscribe = Box::new(
        button("Subscribe", |state: &mut RedshankSurfaceState, _| {
            let url = state.feed_url_editor.text().trim().to_owned();
            state.request(CompactCommand::Subscribe(url));
        })
        .attr("class", "rs-row-action")
        .attr("aria-label", "Subscribe to podcast feed"),
    ) as FullView;
    Box::new(el("div", (field, subscribe)).attr("class", "rs-library-field"))
}

fn open_local() -> FullView {
    Box::new(
        el(
            "div",
            (
                Box::new(
                    button("Open local file", |state: &mut RedshankSurfaceState, _| {
                        state.request(CompactCommand::OpenLocalFile);
                    })
                    .attr("class", "rs-library-open")
                    .attr("aria-label", "Open local file")
                    .attr("aria-keyshortcuts", "Control+O"),
                ) as FullView,
                rows::kbd("Ctrl O"),
            ),
        )
        .attr("class", "rs-library-open-row"),
    )
}

fn status_word(item: &ItemRow) -> String {
    if item.completed {
        "completed".to_owned()
    } else if item.note_count > 0 {
        format!("{} notes", item.note_count)
    } else if item.position_ms > 0 {
        format!("resumes at {}", format_time(item.position_ms))
    } else {
        "unplayed".to_owned()
    }
}

/// Play / Resume / Replay / Playing — exactly one primary per row.
fn primary(state: &RedshankSurfaceState, item: &ItemRow) -> FullView {
    let current = state
        .compact
        .now_playing
        .as_ref()
        .is_some_and(|now| now.item_id == item.id);
    let title = item.title.clone();
    if item.unavailable.is_some() {
        return rows::action(
            "Retry",
            format!("Retry {title}"),
            "rs-library-primary",
            CompactCommand::RetryItem(item.id.clone()),
        );
    }
    if current && state.compact.transport == TransportState::Playing {
        return rows::inert(
            "Playing",
            format!("{title} is playing"),
            "rs-library-primary",
        );
    }
    if item.completed {
        let command = if current {
            CompactCommand::Replay
        } else {
            CompactCommand::SelectItem(item.id.clone())
        };
        return rows::action(
            "Replay",
            format!("Replay {title}"),
            "rs-library-primary",
            command,
        );
    }
    if item.position_ms > 0 {
        return rows::action(
            "Resume",
            format!("Resume {title}"),
            "rs-library-primary",
            CompactCommand::SelectItem(item.id.clone()),
        );
    }
    rows::action(
        "Play",
        format!("Play {title}"),
        "rs-library-primary",
        CompactCommand::SelectItem(item.id.clone()),
    )
}

fn episode_row(state: &RedshankSurfaceState, item: &ItemRow) -> FullView {
    let current = state
        .compact
        .now_playing
        .as_ref()
        .is_some_and(|now| now.item_id == item.id);
    let buffering = current && state.compact.transport == TransportState::Buffering;
    let badge_word = if buffering {
        "BUFFERING"
    } else {
        item.source.badge()
    };
    let title = item.title.clone();
    let facts = format!(
        "{} · {} · {}",
        item.published.clone().unwrap_or_else(|| "undated".into()),
        item.duration_ms
            .map(format_time)
            .unwrap_or_else(|| "—".into()),
        status_word(item)
    );
    let mut secondary = vec![rows::action(
        "Add to queue",
        format!("Add {title} to queue"),
        "rs-row-action",
        CompactCommand::Enqueue(item.id.clone()),
    )];
    secondary.push(if item.cached_bytes.is_some() {
        rows::action(
            "Remove download",
            format!("Remove offline download for {title}"),
            "rs-row-action",
            CompactCommand::RemoveCachedItem(item.id.clone()),
        )
    } else {
        rows::action(
            "Download",
            format!("Download {title} for offline listening"),
            "rs-row-action",
            CompactCommand::CacheItem(item.id.clone()),
        )
    });
    secondary.push(rows::action(
        "Remove",
        format!("Remove {title} from library, including its notes"),
        "rs-row-action",
        CompactCommand::RemoveLibraryItem(item.id.clone()),
    ));
    let unavailable = item.unavailable.as_ref().map(|message| {
        el("div", text(message.clone()))
            .attr("class", "rs-library-detail")
            .attr("role", "status")
    });
    Box::new(
        el(
            "div",
            (
                el(
                    "div",
                    (
                        el("div", text(title)).attr("class", "rs-library-title"),
                        el("div", text(facts)).attr("class", "rs-library-detail"),
                        unavailable,
                    ),
                )
                .attr("class", "rs-library-cell"),
                rows::micro(badge_word),
                rows::bar(item.listened(), item.completed, "rs-library-progress"),
                primary(state, item),
                el("span", secondary).attr("class", "rs-row-actions"),
            ),
        )
        .attr(
            "class",
            if current {
                "rs-row rs-row-active rs-library-episode"
            } else {
                "rs-row rs-library-episode"
            },
        ),
    )
}

fn episode_section(state: &RedshankSurfaceState) -> FullView {
    let feed = state.current_feed().cloned();
    let (head, items): (FullView, Vec<&ItemRow>) = match &feed {
        Some(feed) => {
            let refresh = rows::action(
                "Refresh",
                format!("Refresh {} episodes", feed.title),
                "rs-row-action",
                CompactCommand::RefreshSubscription(feed.feed_url.clone()),
            );
            let auto = super::settings::auto_download_toggle(state);
            let head = Box::new(
                el(
                    "div",
                    (
                        rows::face(&feed.face, ""),
                        el(
                            "div",
                            (
                                el("div", text(feed.title.clone()))
                                    .attr("class", "rs-library-title"),
                                el("div", text(feed.subtitle.clone().unwrap_or_default()))
                                    .attr("class", "rs-library-detail"),
                            ),
                        )
                        .attr("class", "rs-library-cell"),
                        refresh,
                        auto,
                    ),
                )
                .attr("class", "rs-library-head"),
            ) as FullView;
            let url = feed.feed_url.clone();
            let items = state
                .items
                .iter()
                .filter(|item| item.feed_url.as_deref() == Some(url.as_str()))
                .collect();
            (head, items)
        },
        None => {
            let head = Box::new(
                el("div", rows::section_head("LOCAL AUDIO", String::new()))
                    .attr("class", "rs-library-head"),
            ) as FullView;
            let items = state
                .items
                .iter()
                .filter(|item| item.feed_url.is_none())
                .collect();
            (head, items)
        },
    };
    let rows_out: Vec<FullView> = if items.is_empty() {
        vec![rows::empty_row("No episodes in this feed yet.")]
    } else {
        items.iter().map(|item| episode_row(state, item)).collect()
    };
    Box::new(
        el("section", (head, rows_out))
            .attr("class", "rs-library-episodes")
            .attr("aria-label", "Episodes"),
    )
}

/// The phone feed page: the scene segment and the selected projection. The
/// wide sheet hides `rs-library-scene`; the phone query shows it.
fn scene_block(state: &RedshankSurfaceState) -> FullView {
    let options = Scene::ALL
        .iter()
        .map(|option| {
            let option = *option;
            (
                option.label().to_owned(),
                state.scene == option,
                vec![CompactCommand::SelectScene(option)],
            )
        })
        .collect();
    Box::new(
        el(
            "div",
            (
                super::controls::segment("Scene", options),
                scene::panel(state),
            ),
        )
        .attr("class", "rs-library-scene"),
    )
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    let feeds: Vec<FullView> = state
        .feeds
        .iter()
        .map(|feed| feed_row(state, feed))
        .collect();
    let roster = el(
        "section",
        (
            rows::section_head(
                "SUBSCRIPTIONS · NODES",
                format!("{} feeds", state.feeds.len()),
            ),
            feeds,
            local_row(state),
            subscribe_field(),
            open_local(),
        ),
    )
    .attr("class", "rs-library-feeds")
    .attr("aria-label", "Subscriptions");

    Box::new(
        el(
            "section",
            (roster, episode_section(state), scene_block(state)),
        )
        .attr("class", "rs-panel rs-library"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::tests_support::{click, commands, markup, playing, runner};
    use crate::{Face, ItemRow};
    use redshank_model::ItemId;

    fn feed() -> FeedRow {
        FeedRow {
            feed_url: "https://example.test/feed.xml".into(),
            title: "The Allusionist".into(),
            subtitle: Some("Helen Zaltzman".into()),
            face: Face::Tag("AL".into()),
            episode_count: 214,
            unplayed_count: 3,
            offline_bytes: 340 * 1024 * 1024,
            last_refreshed_ms: None,
            failure: None,
        }
    }

    fn episode(id: &str, title: &str) -> ItemRow {
        ItemRow {
            id: ItemId(id.into()),
            title: title.into(),
            feed_url: Some("https://example.test/feed.xml".into()),
            feed_title: Some("The Allusionist".into()),
            face: Face::Tag("mp3".into()),
            source: SourceKind::Cloud,
            duration_ms: Some(2_480_000),
            position_ms: 0,
            completed: false,
            published: Some("Sep 11".into()),
            cached_bytes: None,
            note_count: 0,
            unavailable: None,
        }
    }

    fn library() -> RedshankSurfaceState {
        let mut state = RedshankSurfaceState::default();
        state.feeds.push(feed());
        state.selected_feed = Some("https://example.test/feed.xml".into());
        state.items.push(episode("e-214", "214 · The backlog"));
        state.items.push(ItemRow {
            position_ms: 600_000,
            ..episode("e-212", "212 · Eponyms IV")
        });
        state.items.push(ItemRow {
            completed: true,
            ..episode("e-211", "211 · Bonus 2025")
        });
        state
    }

    #[test]
    fn roster_row_shows_counts_badge_and_offline_bytes() {
        let markup = markup(library(), panel);
        assert!(markup.contains("214 episodes · 3 unplayed · 340 MiB offline"));
        assert!(markup.contains("rs-badge"));
        assert!(markup.contains(">3<"));
        assert!(markup.contains("Local audio"));
    }

    #[test]
    fn refresh_failure_offers_a_retry_that_refreshes() {
        let mut state = library();
        state.feeds[0].failure = Some("refresh failed".into());
        let mut runner = runner(state, panel);
        click(&mut runner, "Retry refresh of The Allusionist");
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::RefreshSubscription(
                "https://example.test/feed.xml".into()
            )]
        );
    }

    #[test]
    fn selecting_a_feed_and_local_audio_emits_select_feed() {
        let mut runner = runner(library(), panel);
        click(&mut runner, "Select The Allusionist");
        click(&mut runner, "Select local audio");
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::SelectFeed(Some("https://example.test/feed.xml".into())),
                CompactCommand::SelectFeed(None),
            ]
        );
    }

    #[test]
    fn each_episode_has_exactly_one_primary_action() {
        let markup = markup(library(), panel);
        assert!(markup.contains("aria-label=\"Play 214 · The backlog\""));
        assert!(markup.contains("aria-label=\"Resume 212 · Eponyms IV\""));
        assert!(markup.contains("aria-label=\"Replay 211 · Bonus 2025\""));
        assert_eq!(markup.matches("rs-library-primary").count(), 3);
    }

    #[test]
    fn the_playing_item_reads_playing_and_refuses_its_command() {
        let mut state = library();
        state.compact = playing();
        state.items[0].id = ItemId("episode-42".into());
        let mut runner = runner(state, panel);
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("is playing")
        );
        click(&mut runner, "214 · The backlog is playing");
        assert!(commands(&mut runner).is_empty());
    }

    #[test]
    fn buffering_the_current_item_swaps_its_badge() {
        let mut state = library();
        state.compact = playing();
        state.compact.transport = TransportState::Buffering;
        state.items[0].id = ItemId("episode-42".into());
        assert!(markup(state, panel).contains(">BUFFERING<"));
    }

    #[test]
    fn episodes_download_then_remove_and_leave_the_library() {
        let mut runner = runner(library(), panel);
        click(
            &mut runner,
            "Download 214 · The backlog for offline listening",
        );
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::CacheItem(ItemId("e-214".into()))]
        );
        runner.update(|state| state.items[0].cached_bytes = Some(4096));
        click(&mut runner, "Remove offline download for 214 · The backlog");
        click(
            &mut runner,
            "Remove 214 · The backlog from library, including its notes",
        );
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::RemoveCachedItem(ItemId("e-214".into())),
                CompactCommand::RemoveLibraryItem(ItemId("e-214".into())),
            ]
        );
    }

    #[test]
    fn unavailable_episode_shows_its_message_and_retries() {
        let mut state = library();
        state.items[0].unavailable = Some("Cloud object is gone".into());
        let mut runner = runner(state, panel);
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("Cloud object is gone")
        );
        click(&mut runner, "Retry 214 · The backlog");
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::RetryItem(ItemId("e-214".into()))]
        );
    }

    #[test]
    fn the_quiet_field_subscribes_and_opens_local_files() {
        let mut state = library();
        state.feed_url_editor = TextInput::new("https://example.test/new.xml");
        let mut runner = runner(state, panel);
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("redshank-feed-url")
        );
        click(&mut runner, "Subscribe to podcast feed");
        click(&mut runner, "Open local file");
        assert_eq!(
            commands(&mut runner),
            [
                CompactCommand::Subscribe("https://example.test/new.xml".into()),
                CompactCommand::OpenLocalFile,
            ]
        );
    }

    #[test]
    fn the_phone_scene_block_selects_a_projection() {
        let mut runner = runner(library(), panel);
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("rs-library-scene")
        );
        click(&mut runner, "Scene: orrery");
        assert_eq!(
            commands(&mut runner),
            [CompactCommand::SelectScene(Scene::Orrery)]
        );
    }
}
