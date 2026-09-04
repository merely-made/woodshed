#![forbid(unsafe_code)]

use std::path::PathBuf;

use cambium_genet_winit_host::{
    AppCtx, CloseDisposition, HostHooks, HostOptions, Init, Runner, WindowFrame, run,
};
use redshank_model::{LibraryItem, RedshankModel};
use redshank_storage::{JsonDirectoryStore, ModelStore};
use redshank_surfaces::{
    COMPACT_SHEET, CompactCommand, CompactPlayerState, CompactView, NowPlaying, TransportState,
    compact_surface,
};

type Logic = fn(&CompactPlayerState) -> CompactView;

fn data_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("REDSHANK_DATA_DIR") {
        return path.into();
    }
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(path).join("Redshank");
    }
    if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(path).join("redshank");
    }
    if let Some(path) = std::env::var_os("HOME") {
        return PathBuf::from(path).join(".local/share/redshank");
    }
    std::env::temp_dir().join("redshank")
}

fn item_title(item: &LibraryItem) -> &str {
    match item {
        LibraryItem::LocalAudio { title, .. } | LibraryItem::FeedEpisode { title, .. } => title,
    }
}

fn compact_from_model(model: &RedshankModel) -> CompactPlayerState {
    let now_playing = model.queue.first().and_then(|id| {
        model.library.get(id).map(|item| NowPlaying {
            item_id: id.clone(),
            title: item_title(item).to_string(),
            position_ms: model
                .progress
                .get(id)
                .map(|progress| progress.position_ms)
                .unwrap_or(0),
            duration_ms: None,
        })
    });
    let mut state = CompactPlayerState::default();
    state.transport = if now_playing.is_some() {
        TransportState::Paused
    } else {
        TransportState::Empty
    };
    state.now_playing = now_playing;
    state.skip_backward_ms = model.settings.skip_backward_ms;
    state.skip_forward_ms = model.settings.skip_forward_ms;
    state
}

fn reject_unwired_commands(state: &mut CompactPlayerState) {
    let commands: Vec<_> = state.drain_commands().collect();
    if commands.is_empty() {
        return;
    }
    let names = commands
        .iter()
        .map(|command| match command {
            CompactCommand::Play => "play",
            CompactCommand::Pause => "pause",
            CompactCommand::SkipBackward(_) => "skip backward",
            CompactCommand::SkipForward(_) => "skip forward",
            CompactCommand::AddTextNote => "add text note",
            CompactCommand::BeginVoiceNote => "begin voice note",
            CompactCommand::FinishVoiceNote => "finish voice note",
        })
        .collect::<Vec<_>>()
        .join(", ");
    state.transport = TransportState::Unavailable(format!(
        "{names}: playback and capture adapters are not connected yet"
    ));
}

fn hooks() -> HostHooks<CompactPlayerState, Logic, CompactView> {
    HostHooks {
        frame: Box::new(|_ctx| false),
        after_dispatch: Box::new(
            |ctx: &mut AppCtx<'_, CompactPlayerState, Logic, CompactView>| {
                ctx.runner.update(reject_unwired_commands);
            },
        ),
        after_frame: Box::new(|_ctx| {}),
        after_wake: Box::new(|_ctx| {}),
        close_request: Box::new(|_ctx, _request| CloseDisposition::Exit),
        focused_text: Box::new(|_runner: &Runner<CompactPlayerState, Logic, CompactView>| None),
        key_intercept: Box::new(|_runner, _press| false),
    }
}

fn main() {
    let store = JsonDirectoryStore::new(data_directory());
    let model = match store.load() {
        Ok(Some(model)) => model,
        Ok(None) => RedshankModel::default(),
        Err(error) => {
            eprintln!("Redshank could not restore its library: {error:?}");
            RedshankModel::default()
        }
    };
    let state = compact_from_model(&model);
    run(
        HostOptions {
            title: "Redshank".into(),
            initial_logical_size: (720.0, 180.0),
            window_frame: WindowFrame::Host,
            size_env: Some(("REDSHANK_WIDTH".into(), "REDSHANK_HEIGHT".into())),
            ..HostOptions::default()
        },
        move |_window, _commands, _wake| Init {
            state,
            logic: compact_surface as Logic,
            sheet: COMPACT_SHEET.into(),
        },
        hooks(),
    )
    .expect("run Redshank");
}

#[cfg(test)]
mod tests {
    use super::*;
    use redshank_model::{ItemId, MediaSource, Progress};

    #[test]
    fn queued_item_and_progress_restore_into_the_compact_surface() {
        let id = ItemId("episode-7".into());
        let mut model = RedshankModel::default();
        model
            .add_item(LibraryItem::FeedEpisode {
                id: id.clone(),
                feed_url: "https://example.test/feed.xml".into(),
                guid: "seven".into(),
                title: "Episode seven".into(),
                source: MediaSource::Enclosure {
                    url: "https://cdn.example.test/seven.mp3".into(),
                },
            })
            .unwrap();
        model.enqueue(&id).unwrap();
        model
            .set_progress(
                &id,
                Progress {
                    position_ms: 91_000,
                    completed: false,
                    updated_at_ms: 500,
                },
            )
            .unwrap();

        let compact = compact_from_model(&model);
        assert_eq!(compact.transport, TransportState::Paused);
        assert_eq!(compact.now_playing.unwrap().position_ms, 91_000);
        assert_eq!(compact.skip_backward_ms, 15_000);
        assert_eq!(compact.skip_forward_ms, 30_000);
    }

    #[test]
    fn empty_library_boots_as_an_empty_player() {
        let compact = compact_from_model(&RedshankModel::default());
        assert_eq!(compact.transport, TransportState::Empty);
        assert!(compact.now_playing.is_none());
    }

    #[test]
    fn unwired_command_becomes_an_explicit_degraded_state() {
        let mut compact = CompactPlayerState::default();
        compact.transport = TransportState::Paused;
        compact.now_playing = Some(NowPlaying {
            item_id: redshank_model::ItemId("one".into()),
            title: "One".into(),
            position_ms: 0,
            duration_ms: None,
        });
        // The public surface produces this command through a click; the host
        // receipt only needs to prove its current adapter boundary is honest.
        compact.request(CompactCommand::Play);
        reject_unwired_commands(&mut compact);
        assert!(matches!(
            compact.transport,
            TransportState::Unavailable(ref message) if message.contains("not connected")
        ));
    }
}
