#![forbid(unsafe_code)]

//! Reusable Redshank listening controls.
//!
//! These views own presentation and interaction only. A host drains
//! [`CompactCommand`] values and applies them through Redshank's player and
//! annotation adapters. The compact surface deliberately has no library,
//! storage, network, or audio-device authority.

use cambium::{
    AnyView, GenetCtx, GenetElement, PointerButton, PointerPhase, TextInput, button, el, lens,
    on_pointer, text, textarea,
};
use redshank_model::{
    AnnotationId, CaptureAnchor, FeedSubscription, ItemId, LibraryItem, ListenerSettings,
};

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
    OpenVoiceNote(AnnotationId),
    PlayVoiceNote(AnnotationId),
    StopVoiceNote(AnnotationId),
    BeginTextNote,
    CancelTextNote,
    SaveTextNote {
        anchor: CaptureAnchor,
        plain_text: String,
    },
    SelectItem(ItemId),
    Enqueue(ItemId),
    Dequeue(ItemId),
    RemoveLibraryItem(ItemId),
    MoveQueue {
        from: usize,
        to: usize,
    },
    DeleteNote(AnnotationId),
    EditNote {
        id: AnnotationId,
        plain_text: String,
    },
    BeginEditNote(AnnotationId),
    OpenLocalFile,
    CacheItem(ItemId),
    RemoveCachedItem(ItemId),
    Subscribe(String),
    RefreshSubscription(String),
    UpdateSettings(ListenerSettings),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactPlayerState {
    pub transport: TransportState,
    pub now_playing: Option<NowPlaying>,
    pub skip_backward_ms: u64,
    pub skip_forward_ms: u64,
    pub voice_capture_available: bool,
    pub voice_capture_active: bool,
    commands: Vec<CompactCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCapture {
    /// This anchor is supplied by the host at BeginTextNote time.
    pub anchor: CaptureAnchor,
    pub draft: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteSummaryBody {
    Text(String),
    Voice {
        duration_ms: u64,
        preview: VoiceNotePreview,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum VoiceNotePreview {
    #[default]
    Idle,
    Loading,
    Playing {
        position_ms: u64,
    },
    Ended,
    Unavailable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteSummary {
    pub id: AnnotationId,
    pub offset_ms: u64,
    pub body: NoteSummaryBody,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceTab {
    #[default]
    Listen,
    Library,
    Notes,
    Settings,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct RedshankSurfaceState {
    pub active_tab: SurfaceTab,
    pub compact: CompactPlayerState,
    pub library: Vec<LibraryItem>,
    pub subscriptions: Vec<FeedSubscription>,
    pub queue: Vec<ItemId>,
    pub notes: Vec<NoteSummary>,
    pub settings: ListenerSettings,
    pub text_capture: Option<TextCapture>,
    pub text_editor: TextInput,
    pub feed_url_editor: TextInput,
    pub notice: Option<String>,
    pub editing_note: Option<AnnotationId>,
    commands: Vec<CompactCommand>,
}

impl RedshankSurfaceState {
    pub fn drain_commands(&mut self) -> impl Iterator<Item = CompactCommand> + '_ {
        let mut commands: Vec<_> = self.compact.drain_commands().collect();
        commands.append(&mut self.commands);
        commands.into_iter()
    }

    pub fn request(&mut self, command: CompactCommand) {
        self.commands.push(command);
    }

    pub fn set_text_draft(&mut self, draft: impl Into<String>) {
        let draft = draft.into();
        if let Some(capture) = &mut self.text_capture {
            capture.draft = draft.clone();
        }
        self.text_editor = TextInput::new(draft);
    }
}

impl Default for CompactPlayerState {
    fn default() -> Self {
        Self {
            transport: TransportState::Empty,
            now_playing: None,
            skip_backward_ms: 15_000,
            skip_forward_ms: 30_000,
            voice_capture_available: false,
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
            TransportState::Empty | TransportState::Buffering | TransportState::Unavailable(_)
        )
}

fn control(
    label: &'static str,
    shortcut: &'static str,
    command: CompactCommand,
    available: bool,
) -> CompactView {
    Box::new(
        button(label, move |state: &mut CompactPlayerState, _| {
            if available && enabled(state) {
                state.request(command.clone());
            }
        })
        .attr("class", "redshank-control")
        .attr("aria-label", label)
        .attr("aria-keyshortcuts", shortcut)
        .attr("aria-disabled", if available { "false" } else { "true" })
        .attr("tabindex", if available { "0" } else { "-1" }),
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
        },
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
                            enabled(state),
                        ),
                        control(play_label, "Space", play_command, enabled(state)),
                        control(
                            "Skip forward",
                            "ArrowRight",
                            CompactCommand::SkipForward(forward),
                            enabled(state),
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
    let voice_label = if state.voice_capture_active {
        "Release to save voice note"
    } else {
        "Hold to record voice note"
    };
    let voice_available = enabled(state) && state.voice_capture_available;
    let voice = on_pointer(
        el("button", text(voice_label))
            .attr(
                "class",
                if state.voice_capture_active {
                    "redshank-control redshank-recording"
                } else {
                    "redshank-control"
                },
            )
            .attr("aria-label", voice_label)
            .attr("aria-keyshortcuts", "R")
            .attr(
                "aria-pressed",
                if state.voice_capture_active {
                    "true"
                } else {
                    "false"
                },
            )
            .attr(
                "aria-disabled",
                if voice_available { "false" } else { "true" },
            )
            .attr("tabindex", if voice_available { "0" } else { "-1" }),
        move |state: &mut CompactPlayerState, event| {
            if event.button != PointerButton::Primary || !voice_available {
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
    );
    Box::new(
        el(
            "section",
            (
                control(
                    "Add text note",
                    "N",
                    CompactCommand::AddTextNote,
                    enabled(state),
                ),
                voice,
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

pub type FullView = Box<dyn AnyView<RedshankSurfaceState, (), GenetCtx, GenetElement>>;

fn full_control(label: &'static str, shortcut: &'static str, command: CompactCommand) -> FullView {
    Box::new(
        button(label, move |state: &mut RedshankSurfaceState, _| {
            state.request(command.clone());
        })
        .attr("class", "redshank-control")
        .attr("aria-label", label)
        .attr("aria-keyshortcuts", shortcut),
    )
}

fn tab_control(label: &'static str, tab: SurfaceTab, selected: bool) -> FullView {
    Box::new(
        button(label, move |state: &mut RedshankSurfaceState, _| {
            state.active_tab = tab;
        })
        .attr("class", "redshank-tab")
        .attr("role", "tab")
        .attr("aria-label", label)
        .attr("aria-selected", if selected { "true" } else { "false" })
        .attr(
            "aria-controls",
            match tab {
                SurfaceTab::Listen => "redshank-listen-panel",
                SurfaceTab::Library => "redshank-library-panel",
                SurfaceTab::Notes => "redshank-notes-panel",
                SurfaceTab::Settings => "redshank-settings-panel",
            },
        ),
    )
}

/// The complete reusable listener surface. It projects host-provided state and
/// emits commands; files, audio, clocks, and persistence remain host-owned.
pub fn surface(state: &RedshankSurfaceState) -> FullView {
    let active = state.active_tab;
    let subscriptions = state.subscriptions.iter().flat_map(|subscription| {
        let feed_url = subscription.feed_url.clone();
        let title = subscription.title.clone();
        let summary = Box::new(text(format!("Subscribed: {title}"))) as FullView;
        let refresh = Box::new(
            button(
                "Refresh feed",
                move |state: &mut RedshankSurfaceState, _| {
                    state.request(CompactCommand::RefreshSubscription(feed_url.clone()));
                },
            )
            .attr("aria-label", format!("Refresh {title}")),
        ) as FullView;
        [summary, refresh]
    });
    let subscribe = Box::new(
        button("Subscribe", |state: &mut RedshankSurfaceState, _| {
            state.request(CompactCommand::Subscribe(
                state.feed_url_editor.text().trim().to_owned(),
            ));
        })
        .attr("aria-label", "Subscribe to podcast feed"),
    ) as FullView;
    let library = state.library.iter().map(|item| {
        let id = item.id().clone();
        let title = item.title().to_owned();
        let select_title = title.clone();
        let select = Box::new(
            button(title, move |state: &mut RedshankSurfaceState, _| {
                state.request(CompactCommand::SelectItem(id.clone()));
            })
            .attr("class", "redshank-library-select")
            .attr("aria-label", format!("Select {select_title}")),
        ) as FullView;
        let enqueue_id = item.id().clone();
        let enqueue_title = item.title().to_owned();
        let enqueue = Box::new(
            button(
                "Add to queue",
                move |state: &mut RedshankSurfaceState, _| {
                    state.request(CompactCommand::Enqueue(enqueue_id.clone()));
                },
            )
            .attr("aria-label", format!("Add {} to queue", enqueue_title)),
        ) as FullView;
        let mut actions = vec![enqueue];
        if item.source().enclosure_url().is_some() && !item.source().is_cached() {
            let cache_id = item.id().clone();
            let cache_title = item.title().to_owned();
            actions.push(Box::new(
                button(
                    "Download for offline listening",
                    move |state: &mut RedshankSurfaceState, _| {
                        state.request(CompactCommand::CacheItem(cache_id.clone()));
                    },
                )
                .attr(
                    "aria-label",
                    format!("Download {} for offline listening", cache_title),
                ),
            ));
        } else if item.source().is_cached() {
            let remove_id = item.id().clone();
            let remove_title = item.title().to_owned();
            actions.push(Box::new(
                el("span", text("Available offline"))
                    .attr("role", "status")
                    .attr(
                        "aria-label",
                        format!("{} is available offline", item.title()),
                    ),
            ));
            actions.push(Box::new(
                button(
                    "Remove offline download",
                    move |state: &mut RedshankSurfaceState, _| {
                        state.request(CompactCommand::RemoveCachedItem(remove_id.clone()));
                    },
                )
                .attr(
                    "aria-label",
                    format!("Remove offline download for {remove_title}"),
                ),
            ));
        }
        let remove_id = item.id().clone();
        let remove_title = item.title().to_owned();
        actions.push(Box::new(
            button(
                "Remove from library",
                move |state: &mut RedshankSurfaceState, _| {
                    state.request(CompactCommand::RemoveLibraryItem(remove_id.clone()));
                },
            )
            .attr(
                "aria-label",
                format!("Remove {remove_title} from library, including its notes"),
            ),
        ));
        Box::new(
            el(
                "article",
                (
                    select,
                    el("div", actions).attr("class", "redshank-item-actions"),
                ),
            )
            .attr("class", "redshank-library-item"),
        ) as FullView
    });
    let queue = state.queue.iter().enumerate().map(|(index, id)| {
        let id = id.clone();
        let title = state
            .library
            .iter()
            .find(|item| item.id() == &id)
            .map(LibraryItem::title)
            .unwrap_or("Unknown item")
            .to_owned();
        let select_id = id.clone();
        let select = Box::new(
            button(
                format!("{}. {}", index + 1, title),
                move |state: &mut RedshankSurfaceState, _| {
                    state.request(CompactCommand::SelectItem(select_id.clone()));
                },
            )
            .attr("class", "redshank-queue-select")
            .attr("aria-label", format!("Select queued item {}", title)),
        ) as FullView;
        let can_move_up = index > 0;
        let up = Box::new(
            button("Up", move |state: &mut RedshankSurfaceState, _| {
                if index > 0 {
                    state.request(CompactCommand::MoveQueue {
                        from: index,
                        to: index - 1,
                    });
                }
            })
            .attr("aria-label", format!("Move {} up", title))
            .attr("aria-disabled", if can_move_up { "false" } else { "true" })
            .attr("tabindex", if can_move_up { "0" } else { "-1" }),
        ) as FullView;
        let down_title = title.clone();
        let can_move_down = index + 1 < state.queue.len();
        let down = Box::new(
            button("Down", move |state: &mut RedshankSurfaceState, _| {
                if index + 1 < state.queue.len() {
                    state.request(CompactCommand::MoveQueue {
                        from: index,
                        to: index + 1,
                    });
                }
            })
            .attr("aria-label", format!("Move {} down", down_title))
            .attr(
                "aria-disabled",
                if can_move_down { "false" } else { "true" },
            )
            .attr("tabindex", if can_move_down { "0" } else { "-1" }),
        ) as FullView;
        let remove_id = id.clone();
        let remove_title = title.clone();
        let remove = Box::new(
            button("Remove", move |state: &mut RedshankSurfaceState, _| {
                state.request(CompactCommand::Dequeue(remove_id.clone()));
            })
            .attr("aria-label", format!("Remove {} from queue", remove_title)),
        ) as FullView;
        Box::new(
            el(
                "article",
                (
                    select,
                    el("div", (up, down, remove)).attr("class", "redshank-item-actions"),
                ),
            )
            .attr("class", "redshank-queue-item"),
        ) as FullView
    });
    let notes = state.notes.iter().map(|note| {
        let id = note.id.clone();
        let delete = Box::new(
            button("Delete note", move |state: &mut RedshankSurfaceState, _| {
                state.request(CompactCommand::DeleteNote(id.clone()));
            })
            .attr("aria-label", "Delete note"),
        ) as FullView;
        let content = match &note.body {
            NoteSummaryBody::Text(body) => {
                let open_id = note.id.clone();
                Box::new(
                    button(
                        format!("{}  {}", format_time(note.offset_ms), body),
                        move |state: &mut RedshankSurfaceState, _| {
                            state.request(CompactCommand::BeginEditNote(open_id.clone()));
                        },
                    )
                    .attr("class", "redshank-note-edit")
                    .attr("aria-label", "Edit text note"),
                ) as FullView
            },
            NoteSummaryBody::Voice {
                duration_ms,
                preview,
            } => {
                let open_id = note.id.clone();
                let source_time = format_time(note.offset_ms);
                let source = Box::new(
                    button(
                        source_time.clone(),
                        move |state: &mut RedshankSurfaceState, _| {
                            state.request(CompactCommand::OpenVoiceNote(open_id.clone()));
                        },
                    )
                    .attr("class", "redshank-note-source")
                    .attr(
                        "aria-label",
                        format!("Go to voice note source at {source_time}"),
                    ),
                ) as FullView;
                let status = match preview {
                    VoiceNotePreview::Idle => format!("Voice note · {}", format_time(*duration_ms)),
                    VoiceNotePreview::Loading => {
                        format!("Voice note · {} · Loading", format_time(*duration_ms))
                    },
                    VoiceNotePreview::Playing { position_ms } => format!(
                        "Voice note · {} / {}",
                        format_time(*position_ms),
                        format_time(*duration_ms)
                    ),
                    VoiceNotePreview::Ended => {
                        format!("Voice note · {} · Finished", format_time(*duration_ms))
                    },
                    VoiceNotePreview::Unavailable(error) => {
                        format!("Voice note · {} · {error}", format_time(*duration_ms))
                    },
                };
                Box::new(
                    el(
                        "div",
                        (
                            source,
                            el("span", text(status)).attr("class", "redshank-note-body"),
                        ),
                    )
                    .attr("class", "redshank-note-content"),
                ) as FullView
            },
        };
        let preview = match &note.body {
            NoteSummaryBody::Text(_) => None,
            NoteSummaryBody::Voice { preview, .. } => {
                let preview_id = note.id.clone();
                let (label, command) = match preview {
                    VoiceNotePreview::Idle => ("Play", CompactCommand::PlayVoiceNote(preview_id)),
                    VoiceNotePreview::Loading => {
                        ("Stop", CompactCommand::StopVoiceNote(preview_id))
                    },
                    VoiceNotePreview::Playing { .. } => {
                        ("Stop", CompactCommand::StopVoiceNote(preview_id))
                    },
                    VoiceNotePreview::Ended => {
                        ("Replay", CompactCommand::PlayVoiceNote(preview_id))
                    },
                    VoiceNotePreview::Unavailable(_) => {
                        ("Retry", CompactCommand::PlayVoiceNote(preview_id))
                    },
                };
                Some(Box::new(
                    button(label, move |state: &mut RedshankSurfaceState, _| {
                        state.request(command.clone());
                    })
                    .attr("aria-label", format!("{label} voice note"))
                    .attr("aria-disabled", "false")
                    .attr("tabindex", "0"),
                ) as FullView)
            },
        };
        Box::new(
            el(
                "article",
                (
                    content,
                    el("div", (preview, delete)).attr("class", "redshank-item-actions"),
                ),
            )
            .attr("class", "redshank-note-item"),
        ) as FullView
    });
    let save = state.text_capture.as_ref().map(|capture| {
        let anchor = capture.anchor.clone();
        Box::new(
            button(
                "Save text note",
                move |state: &mut RedshankSurfaceState, _| {
                    let plain_text = state.text_editor.text().to_owned();
                    if let Some(id) = state.editing_note.clone() {
                        state.request(CompactCommand::EditNote { id, plain_text });
                    } else {
                        state.request(CompactCommand::SaveTextNote {
                            anchor: anchor.clone(),
                            plain_text,
                        });
                    }
                },
            )
            .attr("aria-label", "Save text note")
            .attr("aria-keyshortcuts", "Control+Enter"),
        ) as FullView
    });
    let save = save.into_iter();
    let editor: Option<FullView> = state.text_capture.as_ref().map(|_| {
        Box::new(lens(
            |input: &mut TextInput| textarea(input),
            |state: &mut RedshankSurfaceState| &mut state.text_editor,
        )) as FullView
    });
    let editor = editor.into_iter();
    let begin_text = (state.text_capture.is_none() && active == SurfaceTab::Notes).then(|| {
        Box::new(
            button("Add text note", |state: &mut RedshankSurfaceState, _| {
                state.request(CompactCommand::BeginTextNote);
            })
            .attr("aria-label", "Begin text note")
            .attr("aria-keyshortcuts", "N"),
        ) as FullView
    });
    let cancel_text = state.text_capture.as_ref().map(|_| {
        Box::new(
            button("Cancel", |state: &mut RedshankSurfaceState, _| {
                state.request(CompactCommand::CancelTextNote)
            })
            .attr("aria-label", "Cancel text note"),
        ) as FullView
    });
    let empty_queue = state
        .queue
        .is_empty()
        .then(|| el("p", text("Your queue is empty.")).attr("class", "redshank-empty-state"));
    let empty_notes = (state.notes.is_empty() && state.text_capture.is_none()).then(|| {
        el(
            "p",
            text("No notes for this episode yet. Add one from the player."),
        )
        .attr("class", "redshank-empty-state")
    });
    Box::new(
        el(
            "main",
            (
                el(
                    "header",
                    (
                        el("h1", text("Redshank")).attr("class", "redshank-brand"),
                        el(
                            "nav",
                            (
                                tab_control(
                                    "Listen",
                                    SurfaceTab::Listen,
                                    active == SurfaceTab::Listen,
                                ),
                                tab_control(
                                    "Library",
                                    SurfaceTab::Library,
                                    active == SurfaceTab::Library,
                                ),
                                tab_control(
                                    "Notes",
                                    SurfaceTab::Notes,
                                    active == SurfaceTab::Notes,
                                ),
                                tab_control(
                                    "Settings",
                                    SurfaceTab::Settings,
                                    active == SurfaceTab::Settings,
                                ),
                            ),
                        )
                        .attr("class", "redshank-tabs")
                        .attr("role", "tablist")
                        .attr("aria-label", "Redshank sections"),
                    ),
                )
                .attr("class", "redshank-header"),
                state.notice.as_ref().map(|notice| {
                    el("p", text(notice.clone()))
                        .attr("role", "status")
                        .attr("class", "redshank-notice")
                }),
                el(
                    "div",
                    (
                        el(
                            "section",
                            (
                                el("h2", text("Library")),
                                library.collect::<Vec<_>>(),
                                full_control(
                                    "Open local file",
                                    "Control+O",
                                    CompactCommand::OpenLocalFile,
                                ),
                            ),
                        )
                        .attr("id", "redshank-library-panel")
                        .attr("class", "redshank-panel redshank-library")
                        .attr("role", "tabpanel")
                        .attr("aria-label", "Library")
                        .attr(
                            "aria-hidden",
                            if active == SurfaceTab::Library {
                                "false"
                            } else {
                                "true"
                            },
                        ),
                        el(
                            "section",
                            (
                                el("h2", text("Podcast subscriptions")),
                                subscriptions.collect::<Vec<_>>(),
                                el(
                                    "div",
                                    Box::new(lens(
                                        |input: &mut TextInput| textarea(input),
                                        |state: &mut RedshankSurfaceState| {
                                            &mut state.feed_url_editor
                                        },
                                    )),
                                )
                                .attr("class", "redshank-feed-url"),
                                subscribe,
                            ),
                        )
                        .attr("class", "redshank-panel redshank-subscriptions")
                        .attr("role", "region")
                        .attr("aria-label", "Podcast subscriptions")
                        .attr(
                            "aria-hidden",
                            if active == SurfaceTab::Library {
                                "false"
                            } else {
                                "true"
                            },
                        ),
                        el(
                            "section",
                            (
                                el("h2", text("Up next")),
                                empty_queue,
                                queue.collect::<Vec<_>>(),
                            ),
                        )
                        .attr("id", "redshank-listen-panel")
                        .attr("class", "redshank-panel redshank-queue")
                        .attr("role", "tabpanel")
                        .attr("aria-label", "Up next")
                        .attr(
                            "aria-hidden",
                            if active == SurfaceTab::Listen {
                                "false"
                            } else {
                                "true"
                            },
                        ),
                        el(
                            "section",
                            (
                                el("h2", text("Notes")),
                                empty_notes,
                                notes.collect::<Vec<_>>(),
                                state.text_capture.as_ref().map(|capture| {
                                    let title = state
                                        .library
                                        .iter()
                                        .find(|item| item.id() == &capture.anchor.item_id)
                                        .map(LibraryItem::title)
                                        .unwrap_or(&capture.anchor.item_id.0);
                                    el(
                                        "p",
                                        text(format!(
                                            "Note for {title} at {}",
                                            format_time(capture.anchor.offset_ms)
                                        )),
                                    )
                                    .attr("aria-label", "Captured note target")
                                }),
                                el("div", editor.collect::<Vec<_>>())
                                    .attr("id", "redshank-text-editor")
                                    .attr("class", "redshank-text-editor"),
                                save.collect::<Vec<_>>(),
                                begin_text,
                                cancel_text,
                            ),
                        )
                        .attr("id", "redshank-notes-panel")
                        .attr("class", "redshank-panel redshank-notes")
                        .attr(
                            "role",
                            if active == SurfaceTab::Notes {
                                "tabpanel"
                            } else {
                                "region"
                            },
                        )
                        .attr("aria-label", "Notes")
                        .attr(
                            "aria-hidden",
                            if matches!(active, SurfaceTab::Listen | SurfaceTab::Notes) {
                                "false"
                            } else {
                                "true"
                            },
                        ),
                        el(
                            "section",
                            (
                                el("h2", text("Settings")),
                                text(format!(
                                    "Playback settings: back {} ms, forward {} ms, offline cache {} MiB",
                                    state.settings.skip_backward_ms,
                                    state.settings.skip_forward_ms,
                                    state.settings.cache_budget_bytes / (1024 * 1024)
                                )),
                                full_control(
                                    "Increase forward skip",
                                    "",
                                    CompactCommand::UpdateSettings(ListenerSettings {
                                        skip_backward_ms: state.settings.skip_backward_ms,
                                        skip_forward_ms: state
                                            .settings
                                            .skip_forward_ms
                                            .saturating_add(5_000),
                                        capture_playback: state.settings.capture_playback,
                                        cache_budget_bytes: state.settings.cache_budget_bytes,
                                    }),
                                ),
                                full_control(
                                    "Decrease forward skip",
                                    "",
                                    CompactCommand::UpdateSettings(ListenerSettings {
                                        skip_backward_ms: state.settings.skip_backward_ms,
                                        skip_forward_ms: state
                                            .settings
                                            .skip_forward_ms
                                            .saturating_sub(5_000),
                                        capture_playback: state.settings.capture_playback,
                                        cache_budget_bytes: state.settings.cache_budget_bytes,
                                    }),
                                ),
                                full_control(
                                    "Increase offline cache budget",
                                    "",
                                    CompactCommand::UpdateSettings(ListenerSettings {
                                        cache_budget_bytes: state
                                            .settings
                                            .cache_budget_bytes
                                            .saturating_add(256 * 1024 * 1024),
                                        ..state.settings.clone()
                                    }),
                                ),
                                full_control(
                                    "Decrease offline cache budget",
                                    "",
                                    CompactCommand::UpdateSettings(ListenerSettings {
                                        cache_budget_bytes: state
                                            .settings
                                            .cache_budget_bytes
                                            .saturating_sub(256 * 1024 * 1024),
                                        ..state.settings.clone()
                                    }),
                                ),
                                full_control(
                                    "Pause during capture",
                                    "",
                                    CompactCommand::UpdateSettings(ListenerSettings {
                                        capture_playback:
                                            redshank_model::CapturePlaybackBehavior::Pause,
                                        ..state.settings.clone()
                                    }),
                                ),
                                full_control(
                                    "Continue during capture",
                                    "",
                                    CompactCommand::UpdateSettings(ListenerSettings {
                                        capture_playback:
                                            redshank_model::CapturePlaybackBehavior::Continue,
                                        ..state.settings.clone()
                                    }),
                                ),
                            ),
                        )
                        .attr("id", "redshank-settings-panel")
                        .attr("class", "redshank-panel redshank-settings")
                        .attr("role", "tabpanel")
                        .attr("aria-label", "Settings")
                        .attr(
                            "aria-hidden",
                            if active == SurfaceTab::Settings {
                                "false"
                            } else {
                                "true"
                            },
                        ),
                    ),
                )
                .attr("class", "redshank-workspace"),
                el(
                    "section",
                    Box::new(lens(
                        |compact: &mut CompactPlayerState| compact_surface(compact),
                        |state: &mut RedshankSurfaceState| &mut state.compact,
                    )),
                )
                .attr("class", "redshank-dock")
                .attr("role", "region")
                .attr("aria-label", "Player"),
            ),
        )
        .attr(
            "class",
            match active {
                SurfaceTab::Listen => "redshank-surface redshank-listen-view",
                SurfaceTab::Library => "redshank-surface redshank-library-view",
                SurfaceTab::Notes => "redshank-surface redshank-notes-view",
                SurfaceTab::Settings => "redshank-surface redshank-settings-view",
            },
        ),
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
    type FullLogic = fn(&RedshankSurfaceState) -> FullView;
    type FullRunner = GenetAppRunner<RedshankSurfaceState, FullLogic, FullView, ()>;

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

    fn full_runner(state: RedshankSurfaceState) -> FullRunner {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        FullRunner::new(dom, surface, state)
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

    fn node_with_control(dom: &ScriptedDom, root: NodeId, control: &str) -> NodeId {
        let attribute = LocalName::from("aria-controls");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &attribute)
                .is_some_and(|value| value == control)
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing tab for {control}");
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
            cambium::PointerEvent::new(PointerPhase::Down, (1.0, 1.0), (10.0, 10.0)),
        );
        let mut commands = Vec::new();
        capture.update(|state| {
            commands.extend(state.drain_commands());
            state.voice_capture_active = true;
        });
        assert_eq!(commands, [CompactCommand::BeginVoiceNote]);

        capture.dispatch_pointer_up(cambium::PointerEvent::new(
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

    fn anchored_state() -> RedshankSurfaceState {
        let mut state = RedshankSurfaceState {
            compact: playing_state(),
            ..Default::default()
        };
        state.library.push(LibraryItem::LocalAudio {
            id: ItemId("episode-42".into()),
            title: "Wetland".into(),
            source: redshank_model::MediaSource::Local {
                path: "wetland.mp3".into(),
            },
        });
        state.queue = vec![ItemId("episode-42".into()), ItemId("second".into())];
        state.text_capture = Some(TextCapture {
            anchor: CaptureAnchor {
                item_id: ItemId("episode-42".into()),
                offset_ms: 62_000,
                representation: redshank_model::RepresentationReceipt {
                    complete_digest: Some("blake3:frozen".into()),
                    ..Default::default()
                },
            },
            draft: String::new(),
        });
        state.set_text_draft("typed after capture");
        state
    }

    #[test]
    fn full_surface_mounts_compact_player_and_queue_reorder_controls() {
        let mut runner = full_runner(anchored_state());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("aria-label=\"Player\""));
        assert!(markup.contains("aria-label=\"Pause\""));
        assert!(markup.contains("Move Wetland down"));
        let move_up = node_with_label(&runner.dom().borrow(), runner.root(), "Move Wetland up");
        runner.dispatch_click(move_up, PointerClick::at((1.0, 1.0)));
        let mut no_op = Vec::new();
        runner.update(|state| no_op.extend(state.drain_commands()));
        assert!(no_op.is_empty());
        let move_down = node_with_label(&runner.dom().borrow(), runner.root(), "Move Wetland down");
        runner.dispatch_click(move_down, PointerClick::at((1.0, 1.0)));
        let pause = node_with_label(&runner.dom().borrow(), runner.root(), "Pause");
        runner.dispatch_click(pause, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [
                CompactCommand::Pause,
                CompactCommand::MoveQueue { from: 0, to: 1 },
            ]
        );
    }

    #[test]
    fn full_surface_defaults_to_listen_and_tabs_switch_without_product_commands() {
        let mut runner = full_runner(anchored_state());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("role=\"tablist\""));
        assert!(markup.contains("aria-label=\"Listen\" aria-selected=\"true\""));
        assert!(markup.contains("id=\"redshank-listen-panel\""));
        let library = node_with_control(
            &runner.dom().borrow(),
            runner.root(),
            "redshank-library-panel",
        );
        runner.dispatch_click(library, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| {
            assert_eq!(state.active_tab, SurfaceTab::Library);
            commands.extend(state.drain_commands());
        });
        assert!(commands.is_empty());
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("aria-label=\"Library\" aria-selected=\"true\""));
    }

    #[test]
    fn listen_surface_uses_compact_rows_and_hides_idle_editor_actions() {
        let mut state = RedshankSurfaceState {
            compact: playing_state(),
            ..Default::default()
        };
        state.library.push(LibraryItem::LocalAudio {
            id: ItemId("episode-42".into()),
            title: "Wetland".into(),
            source: redshank_model::MediaSource::Local {
                path: "wetland.mp3".into(),
            },
        });
        state.queue.push(ItemId("episode-42".into()));
        let runner = full_runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("class=\"redshank-workspace\""));
        assert!(markup.contains("class=\"redshank-queue-item\""));
        assert!(markup.contains(">Up next<"));
        assert!(markup.contains("No notes for this episode yet"));
        assert!(!markup.contains("aria-label=\"Begin text note\""));
        assert!(!markup.contains("aria-label=\"Cancel text note\""));
        assert!(
            markup.find("class=\"redshank-workspace\"") < markup.find("class=\"redshank-dock\"")
        );
    }

    #[test]
    fn voice_note_row_reports_duration_and_opens_its_anchor() {
        let mut state = RedshankSurfaceState {
            compact: playing_state(),
            active_tab: SurfaceTab::Notes,
            ..Default::default()
        };
        let id = AnnotationId("voice-one".into());
        state.notes.push(NoteSummary {
            id: id.clone(),
            offset_ms: 62_000,
            body: NoteSummaryBody::Voice {
                duration_ms: 3_500,
                preview: VoiceNotePreview::Idle,
            },
        });
        let mut runner = full_runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("Voice note · 0:03"));
        let open = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Go to voice note source at 1:02",
        );
        runner.dispatch_click(open, PointerClick::at((1.0, 1.0)));
        let play = node_with_label(&runner.dom().borrow(), runner.root(), "Play voice note");
        runner.dispatch_click(play, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [
                CompactCommand::OpenVoiceNote(id.clone()),
                CompactCommand::PlayVoiceNote(id),
            ]
        );
    }

    #[test]
    fn playing_voice_note_row_reports_progress_and_can_stop() {
        let mut state = RedshankSurfaceState {
            compact: playing_state(),
            active_tab: SurfaceTab::Notes,
            ..Default::default()
        };
        let id = AnnotationId("voice-one".into());
        state.notes.push(NoteSummary {
            id: id.clone(),
            offset_ms: 62_000,
            body: NoteSummaryBody::Voice {
                duration_ms: 6_000,
                preview: VoiceNotePreview::Playing { position_ms: 2_000 },
            },
        });
        let mut runner = full_runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("Voice note · 0:02 / 0:06"));
        let stop = node_with_label(&runner.dom().borrow(), runner.root(), "Stop voice note");
        runner.dispatch_click(stop, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands, [CompactCommand::StopVoiceNote(id)]);
    }

    #[test]
    fn remote_library_item_exposes_an_offline_download_command() {
        let mut state = anchored_state();
        state.library.push(LibraryItem::DirectAudio {
            id: ItemId("remote".into()),
            title: "Remote episode".into(),
            source: redshank_model::MediaSource::Enclosure {
                url: "https://example.test/episode.mp3".into(),
            },
        });
        let mut runner = full_runner(state);
        let download = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Download Remote episode for offline listening",
        );
        runner.dispatch_click(download, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [CompactCommand::CacheItem(ItemId("remote".into()))]
        );
        runner.update(|state| {
            state.library[1].replace_source(redshank_model::MediaSource::Cached {
                path: "cache/episode.audio".into(),
                origin_url: "https://example.test/episode.mp3".into(),
                representation: Box::default(),
            });
        });
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("Remote episode is available offline")
        );
        let remove = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Remove offline download for Remote episode",
        );
        runner.dispatch_click(remove, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [CompactCommand::RemoveCachedItem(ItemId("remote".into()))]
        );
    }

    #[test]
    fn library_item_exposes_an_explicit_removal_command() {
        let mut runner = full_runner(anchored_state());
        let remove = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Remove Wetland from library, including its notes",
        );
        runner.dispatch_click(remove, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [CompactCommand::RemoveLibraryItem(ItemId(
                "episode-42".into()
            ))]
        );
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("class=\"redshank-library-item\"")
        );
    }

    #[test]
    fn subscription_controls_emit_entered_and_retained_feed_urls() {
        let mut state = anchored_state();
        state.feed_url_editor = TextInput::new("https://example.test/feed.xml");
        state.subscriptions.push(FeedSubscription {
            feed_url: "https://example.test/old.xml".into(),
            title: "Old Marsh".into(),
            ..Default::default()
        });
        let mut runner = full_runner(state);
        let subscribe = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Subscribe to podcast feed",
        );
        runner.dispatch_click(subscribe, PointerClick::at((1.0, 1.0)));
        let refresh = node_with_label(&runner.dom().borrow(), runner.root(), "Refresh Old Marsh");
        runner.dispatch_click(refresh, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(
            commands,
            [
                CompactCommand::Subscribe("https://example.test/feed.xml".into()),
                CompactCommand::RefreshSubscription("https://example.test/old.xml".into()),
            ]
        );
    }

    #[test]
    fn save_text_note_uses_editor_text_and_frozen_anchor() {
        let mut runner = full_runner(anchored_state());
        assert!(
            runner
                .dom()
                .borrow()
                .outer_html(runner.root())
                .contains("<textarea")
        );
        let save = node_with_label(&runner.dom().borrow(), runner.root(), "Save text note");
        runner.dispatch_click(save, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0], CompactCommand::SaveTextNote { plain_text, anchor }
            if plain_text == "typed after capture" && anchor.representation.complete_digest.as_deref() == Some("blake3:frozen"))
        );
    }

    #[test]
    fn buffering_disables_transport_and_settings_emit_changed_values() {
        let mut state = anchored_state();
        state.compact.transport = TransportState::Buffering;
        let mut runner = full_runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("aria-label=\"Play\""));
        assert!(markup.contains("aria-disabled=\"true\""));
        let play = node_with_label(&runner.dom().borrow(), runner.root(), "Play");
        runner.dispatch_click(play, PointerClick::at((1.0, 1.0)));
        let increase = node_with_label(
            &runner.dom().borrow(),
            runner.root(),
            "Increase forward skip",
        );
        runner.dispatch_click(increase, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| commands.extend(state.drain_commands()));
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0], CompactCommand::UpdateSettings(settings) if settings.skip_forward_ms == 35_000)
        );
    }
}
