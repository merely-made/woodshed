#![forbid(unsafe_code)]

mod headed_receipt;
mod session;
mod voice;

use cambium_genet_winit_host::{
    AppCtx, CloseDisposition, FocusedTextSlot, HostHooks, HostOptions, Init, Key, KeyPress,
    NamedKey, Runner, WindowFrame, run,
};
use headed_receipt::HeadedReceipt;
use layout_dom_api::LayoutDom;
use redshank_cache::EpisodeCache;
use redshank_feed::FeedImport;
use redshank_model::{
    Annotation, AnnotationId, AudioCaptureHost, CaptureAnchor, CapturePlaybackBehavior, ItemId,
    LibraryItem, MediaSource, NoteBody, RedshankModel,
};
use redshank_playback::{PlaybackCommand, PlaybackRuntime, PlaybackState};
use redshank_storage::{JsonDirectoryStore, ModelStore};
use redshank_surfaces::{
    COMPACT_SHEET, CompactCommand, RedshankSurfaceState, TextCapture, TransportState, surface,
};
use session::Session;
use std::{
    cell::RefCell,
    io::Read,
    path::PathBuf,
    rc::Rc,
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use ureq::ResponseExt;
use voice::LocalVoiceCapture;

type Logic = fn(&RedshankSurfaceState) -> redshank_surfaces::FullView;
type AppRunner = Runner<RedshankSurfaceState, Logic, redshank_surfaces::FullView>;
type Context<'a> = AppCtx<'a, RedshankSurfaceState, Logic, redshank_surfaces::FullView>;

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
    PathBuf::from("redshank-data")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

const MAX_FEED_BYTES: u64 = 4 * 1024 * 1024;

fn fetch_and_import_feed(requested_url: &str) -> Result<FeedImport, String> {
    let mut response = ureq::get(requested_url)
        .header(
            "Accept",
            "application/rss+xml, application/atom+xml, application/xml, text/xml",
        )
        .call()
        .map_err(|error| format!("Could not fetch podcast feed: {error}"))?;
    let final_url = response.get_uri().to_string();
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_FEED_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read podcast feed: {error}"))?;
    if bytes.len() as u64 > MAX_FEED_BYTES {
        return Err("Podcast feed is larger than the 4 MiB standalone limit".into());
    }
    let body =
        String::from_utf8(bytes).map_err(|_| "Podcast feed is not valid UTF-8 XML".to_owned())?;
    redshank_feed::import(&body, &final_url, now_ms())
        .map_err(|error| format!("Could not import podcast feed: {error}"))
}

enum IoReply {
    Saved {
        revision: u64,
        result: Result<(), String>,
    },
    Opened(Option<PathBuf>),
    Cached {
        id: ItemId,
        result: Result<MediaSource, String>,
    },
    FeedFetched {
        requested_url: String,
        result: Result<FeedImport, String>,
    },
    CacheRemoved {
        id: ItemId,
        result: Result<bool, String>,
    },
    VoiceRemoved(Result<bool, String>),
}

struct SaveRequest {
    revision: u64,
    model: RedshankModel,
}

/// Serial persistence is kept off the UI and audio threads. At most one save
/// is in flight; the next request contains the latest complete model.
struct Persistence {
    requests: mpsc::Sender<SaveRequest>,
    replies: mpsc::Receiver<IoReply>,
    reply_sender: mpsc::Sender<IoReply>,
    revision: u64,
    durable: u64,
    in_flight: Option<u64>,
    failed: bool,
}

impl Persistence {
    fn start(store: JsonDirectoryStore) -> Self {
        let (requests, receiver) = mpsc::channel::<SaveRequest>();
        let (reply_sender, replies) = mpsc::channel();
        let worker_reply = reply_sender.clone();
        thread::Builder::new()
            .name("redshank-storage".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let result = store
                        .save(&request.model)
                        .map_err(|error| format!("Could not save listener state: {error:?}"));
                    if worker_reply
                        .send(IoReply::Saved {
                            revision: request.revision,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("spawn Redshank storage worker");
        Self {
            requests,
            replies,
            reply_sender,
            revision: 0,
            durable: 0,
            in_flight: None,
            failed: false,
        }
    }

    fn changed(&mut self) {
        self.revision += 1;
        self.failed = false;
    }

    fn flush(&mut self, model: &RedshankModel) -> Result<(), String> {
        if self.revision == self.durable || self.in_flight.is_some() || self.failed {
            return Ok(());
        }
        self.requests
            .send(SaveRequest {
                revision: self.revision,
                model: model.clone(),
            })
            .map_err(|_| {
                "The storage worker stopped; listener changes remain unsaved".to_owned()
            })?;
        self.in_flight = Some(self.revision);
        Ok(())
    }

    fn acknowledge(&mut self, revision: u64, result: &Result<(), String>) {
        self.in_flight = None;
        match result {
            Ok(()) => {
                self.durable = revision;
                self.failed = false;
            },
            Err(_) => self.failed = true,
        }
    }
}

struct SavedDraft {
    revision: u64,
    id: AnnotationId,
    anchor: CaptureAnchor,
    text: String,
}

struct PendingCacheRemoval {
    revision: u64,
    id: ItemId,
    path: PathBuf,
}

struct VoiceSession {
    anchor: CaptureAnchor,
    resume_playback: bool,
}

struct PendingVoiceRemoval {
    revision: u64,
    blob_id: String,
}

struct Desktop {
    session: Session,
    runtime: PlaybackRuntime,
    persistence: Persistence,
    dialog_pending: bool,
    cache_pending: Option<ItemId>,
    cache_removals: Vec<PendingCacheRemoval>,
    cache_removals_in_flight: usize,
    voice_capture: LocalVoiceCapture,
    voice_session: Option<VoiceSession>,
    voice_save_revision: Option<u64>,
    voice_removals: Vec<PendingVoiceRemoval>,
    voice_removals_in_flight: usize,
    feed_pending: Option<String>,
    data_root: PathBuf,
    closing: bool,
    saved_draft: Option<SavedDraft>,
    receipt: Option<HeadedReceipt>,
    last_progress: Instant,
    last_projection: Instant,
}

impl Desktop {
    fn schedule_durable_cache_removals(&mut self, revision: u64, state: &mut RedshankSurfaceState) {
        let mut waiting = Vec::new();
        for removal in self.cache_removals.drain(..) {
            if removal.revision > revision {
                waiting.push(removal);
                continue;
            }
            let still_shared = self.session.model.library.values().any(|item| {
                matches!(item.source(), MediaSource::Cached { path, .. } if std::path::Path::new(path) == removal.path.as_path())
            });
            if still_shared {
                state.notice =
                    Some("Offline download removed; shared cache object retained".into());
                continue;
            }
            let cache = EpisodeCache::new(
                self.data_root.join("cache"),
                self.session.model.settings.cache_budget_bytes,
            );
            let sender = self.persistence.reply_sender.clone();
            let id = removal.id;
            let path = removal.path;
            match thread::Builder::new()
                .name("redshank-cache-remove".into())
                .spawn(move || {
                    let result = cache
                        .remove_cached_path(&path)
                        .map_err(|error| error.to_string());
                    let _ = sender.send(IoReply::CacheRemoved { id, result });
                }) {
                Ok(_) => self.cache_removals_in_flight += 1,
                Err(error) => {
                    state.notice = Some(format!("Could not start cache removal: {error}"));
                },
            }
        }
        self.cache_removals = waiting;
    }

    fn schedule_durable_voice_removals(&mut self, revision: u64, state: &mut RedshankSurfaceState) {
        let mut waiting = Vec::new();
        for removal in self.voice_removals.drain(..) {
            if removal.revision > revision {
                waiting.push(removal);
                continue;
            }
            let still_shared = self.session.model.annotations.values().any(|note| {
                matches!(&note.body, NoteBody::Audio { blob_id, .. } if blob_id == &removal.blob_id)
            });
            if still_shared {
                continue;
            }
            let sender = self.persistence.reply_sender.clone();
            let data_root = self.data_root.clone();
            let blob_id = removal.blob_id;
            match thread::Builder::new()
                .name("redshank-voice-remove".into())
                .spawn(move || {
                    let result = LocalVoiceCapture::remove_blob(&data_root, &blob_id);
                    let _ = sender.send(IoReply::VoiceRemoved(result));
                }) {
                Ok(_) => self.voice_removals_in_flight += 1,
                Err(error) => {
                    state.notice = Some(format!("Could not start voice-note removal: {error}"));
                },
            }
        }
        self.voice_removals = waiting;
    }

    fn next_annotation_id(&self) -> AnnotationId {
        let mut suffix = self.session.model.annotations.len();
        loop {
            let id = AnnotationId(format!("note:{}:{suffix}", now_ms()));
            if !self.session.model.annotations.contains_key(&id) {
                return id;
            }
            suffix += 1;
        }
    }

    fn fetch_feed(
        &mut self,
        state: &mut RedshankSurfaceState,
        requested_url: String,
    ) -> Result<(), String> {
        if self.feed_pending.is_some() {
            return Err("Another podcast feed is still refreshing".into());
        }
        if !requested_url.starts_with("http://") && !requested_url.starts_with("https://") {
            return Err("Podcast subscriptions require an HTTP or HTTPS feed URL".into());
        }
        let sender = self.persistence.reply_sender.clone();
        let worker_url = requested_url.clone();
        thread::Builder::new()
            .name("redshank-feed".into())
            .spawn(move || {
                let result = fetch_and_import_feed(&worker_url);
                let _ = sender.send(IoReply::FeedFetched {
                    requested_url: worker_url,
                    result,
                });
            })
            .map_err(|error| format!("Could not start feed refresh: {error}"))?;
        self.feed_pending = Some(requested_url);
        state.notice = Some("Refreshing podcast feed…".into());
        Ok(())
    }

    fn send(&self, command: PlaybackCommand) -> Result<(), String> {
        self.runtime
            .command(command)
            .map_err(|error| error.to_string())
    }

    fn select(&mut self, id: ItemId) -> Result<(), String> {
        for command in self
            .session
            .select(id, &self.runtime.snapshot(), now_ms())?
        {
            self.send(command)?;
        }
        self.persistence.changed();
        Ok(())
    }

    fn open(&mut self, path: PathBuf) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or("This file path cannot be stored as Unicode")?
            .to_owned();
        let id = ItemId(format!("local:{path}"));
        if !self.session.model.library.contains_key(&id) {
            let title = PathBuf::from(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Local audio")
                .to_owned();
            self.session
                .model
                .add_item(LibraryItem::LocalAudio {
                    id: id.clone(),
                    title,
                    source: MediaSource::Local { path },
                })
                .map_err(|e| format!("Could not add file: {e:?}"))?;
        }
        self.session
            .model
            .enqueue(&id)
            .map_err(|e| format!("Could not queue file: {e:?}"))?;
        self.select(id)
    }

    fn open_url(&mut self, url: String) -> Result<(), String> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Remote audio URLs must use HTTP or HTTPS".into());
        }
        let id = ItemId(format!("remote:{url}"));
        if !self.session.model.library.contains_key(&id) {
            let title = url
                .split(['?', '#'])
                .next()
                .and_then(|value| value.rsplit('/').find(|part| !part.is_empty()))
                .unwrap_or("Remote audio")
                .to_owned();
            self.session
                .model
                .add_item(LibraryItem::DirectAudio {
                    id: id.clone(),
                    title,
                    source: MediaSource::Enclosure { url },
                })
                .map_err(|e| format!("Could not add URL: {e:?}"))?;
        }
        self.session
            .model
            .enqueue(&id)
            .map_err(|e| format!("Could not queue URL: {e:?}"))?;
        self.select(id)
    }

    fn begin_note(&mut self, state: &mut RedshankSurfaceState) -> Result<(), String> {
        if state.text_capture.is_some() {
            return Err("Save or cancel the current note first".into());
        }
        let anchor = self.session.capture(&self.runtime.snapshot())?;
        match self.session.model.settings.capture_playback {
            CapturePlaybackBehavior::Pause => self.send(PlaybackCommand::Pause)?,
            CapturePlaybackBehavior::Continue => {},
            CapturePlaybackBehavior::Duck => return Err(
                "Ducking is not available in this local playback slice; choose Pause or Continue"
                    .into(),
            ),
        }
        state.editing_note = None;
        state.text_capture = Some(TextCapture {
            anchor,
            draft: String::new(),
        });
        state.set_text_draft("");
        Ok(())
    }

    fn save_note(
        &mut self,
        state: &mut RedshankSurfaceState,
        anchor: CaptureAnchor,
        text: String,
    ) -> Result<(), String> {
        if text.trim().is_empty() {
            return Err("Write a note before saving".into());
        }
        if state
            .text_capture
            .as_ref()
            .is_none_or(|capture| capture.anchor != anchor)
        {
            return Err("The note's capture target changed; its draft was retained".into());
        }
        let id = state
            .editing_note
            .clone()
            .unwrap_or_else(|| self.next_annotation_id());
        if let Some(existing) = self.session.model.annotations.get(&id) {
            if existing.target != anchor {
                return Err("The edited note does not match this capture target".into());
            }
            self.session
                .model
                .update_text_annotation(&id, text.clone())
                .map_err(|e| format!("Could not edit note: {e:?}"))?;
        } else {
            self.session
                .model
                .add_text_annotation(id.clone(), anchor.clone(), text.clone(), now_ms())
                .map_err(|e| format!("Could not add note: {e:?}"))?;
        }
        state.editing_note = Some(id.clone());
        self.persistence.changed();
        self.saved_draft = Some(SavedDraft {
            revision: self.persistence.revision,
            id,
            anchor,
            text,
        });
        state.notice = Some("Saving note…".into());
        Ok(())
    }

    fn begin_voice_note(&mut self, state: &mut RedshankSurfaceState) -> Result<(), String> {
        if state.text_capture.is_some() {
            return Err("Save or cancel the current text note first".into());
        }
        if self.voice_session.is_some() {
            return Err("A voice note is already recording".into());
        }
        let snapshot = self.runtime.snapshot();
        let anchor = self.session.capture(&snapshot)?;
        let resume_playback = match self.session.model.settings.capture_playback {
            CapturePlaybackBehavior::Pause => {
                let was_playing = snapshot.state == PlaybackState::Playing;
                self.send(PlaybackCommand::Pause)?;
                was_playing
            },
            CapturePlaybackBehavior::Continue => false,
            CapturePlaybackBehavior::Duck => {
                return Err(
                    "Ducking is not available yet; choose Pause or Continue in Settings".into(),
                );
            },
        };
        if let Err(error) = self.voice_capture.begin_capture() {
            if resume_playback {
                let _ = self.send(PlaybackCommand::Play);
            }
            return Err(error);
        }
        self.voice_session = Some(VoiceSession {
            anchor,
            resume_playback,
        });
        state.compact.voice_capture_active = true;
        state.notice = Some("Recording voice note… release to save".into());
        Ok(())
    }

    fn finish_voice_note(&mut self, state: &mut RedshankSurfaceState) -> Result<(), String> {
        let capture = self
            .voice_session
            .take()
            .ok_or("No voice note is recording")?;
        state.compact.voice_capture_active = false;
        let result = self.voice_capture.finish_capture().and_then(|body| {
            let id = self.next_annotation_id();
            let blob_id = match &body {
                NoteBody::Audio { blob_id, .. } => blob_id.clone(),
                NoteBody::Text { .. } => return Err("Microphone returned a text note".into()),
            };
            if let Err(error) = self.session.model.add_annotation(Annotation {
                id,
                target: capture.anchor,
                body,
                created_at_ms: now_ms(),
            }) {
                let _ = LocalVoiceCapture::remove_blob(&self.data_root, &blob_id);
                return Err(format!("Could not add voice note: {error:?}"));
            }
            self.persistence.changed();
            self.voice_save_revision = Some(self.persistence.revision);
            state.notice = Some("Saving voice note…".into());
            Ok(())
        });
        if capture.resume_playback
            && let Err(error) = self.send(PlaybackCommand::Play)
        {
            return Err(match result {
                Ok(()) => error,
                Err(capture_error) => {
                    format!("{capture_error}; playback could not resume: {error}")
                },
            });
        }
        result
    }

    fn open_note_target(&mut self, note: &Annotation) -> Result<(), String> {
        if self.session.selected.as_ref() != Some(&note.target.item_id) {
            self.select(note.target.item_id.clone())?;
        }
        self.send(PlaybackCommand::Seek(note.target.offset_ms))
    }

    fn command(
        &mut self,
        state: &mut RedshankSurfaceState,
        command: CompactCommand,
    ) -> Result<(), String> {
        match command {
            CompactCommand::Play
            | CompactCommand::Pause
            | CompactCommand::SkipBackward(_)
            | CompactCommand::SkipForward(_) => {
                let snap = self.runtime.snapshot();
                self.session.capture(&snap)?;
                let command = match command {
                    CompactCommand::Play => {
                        if snap.state == PlaybackState::Ended {
                            self.send(PlaybackCommand::Seek(0))?;
                        }
                        PlaybackCommand::Play
                    },
                    CompactCommand::Pause => PlaybackCommand::Pause,
                    CompactCommand::SkipBackward(ms) => {
                        PlaybackCommand::Seek(snap.position_ms.saturating_sub(ms))
                    },
                    CompactCommand::SkipForward(ms) => {
                        PlaybackCommand::Seek(snap.position_ms.saturating_add(ms))
                    },
                    _ => unreachable!(),
                };
                self.send(command)?;
            },
            CompactCommand::SelectItem(id) => self.select(id)?,
            CompactCommand::OpenLocalFile => {
                if !self.dialog_pending {
                    let sender = self.persistence.reply_sender.clone();
                    thread::Builder::new()
                        .name("redshank-file-picker".into())
                        .spawn(move || {
                            let path = rfd::FileDialog::new()
                                .add_filter("Audio", &["mp3", "m4a", "aac"])
                                .pick_file();
                            let _ = sender.send(IoReply::Opened(path));
                        })
                        .map_err(|error| format!("Could not open file picker: {error}"))?;
                    self.dialog_pending = true;
                }
            },
            CompactCommand::CacheItem(id) => {
                if self.cache_pending.is_some() {
                    return Err("Another episode download is still running".into());
                }
                let url = self
                    .session
                    .model
                    .library
                    .get(&id)
                    .ok_or("That library item no longer exists")?
                    .source()
                    .enclosure_url()
                    .ok_or("Only remote audio can be downloaded")?
                    .to_owned();
                if self
                    .session
                    .model
                    .library
                    .get(&id)
                    .is_some_and(|item| item.source().is_cached())
                {
                    return Err("That recording is already available offline".into());
                }
                let cache = EpisodeCache::new(
                    self.data_root.join("cache"),
                    self.session.model.settings.cache_budget_bytes,
                );
                let sender = self.persistence.reply_sender.clone();
                let request_id = id.clone();
                thread::Builder::new()
                    .name("redshank-cache".into())
                    .spawn(move || {
                        let result = cache.cache_url(&url).map_err(|error| error.to_string());
                        let _ = sender.send(IoReply::Cached {
                            id: request_id,
                            result,
                        });
                    })
                    .map_err(|error| format!("Could not start episode download: {error}"))?;
                self.cache_pending = Some(id);
                state.notice = Some("Downloading episode for offline listening…".into());
            },
            CompactCommand::RemoveCachedItem(id) => {
                if self.cache_pending.as_ref() == Some(&id)
                    || self.cache_removals.iter().any(|removal| removal.id == id)
                {
                    return Err("That episode already has a cache operation running".into());
                }
                let item = self
                    .session
                    .model
                    .library
                    .get_mut(&id)
                    .ok_or("That library item no longer exists")?;
                let MediaSource::Cached {
                    path, origin_url, ..
                } = item.source().clone()
                else {
                    return Err("That recording is not available offline".into());
                };
                item.replace_source(MediaSource::Enclosure { url: origin_url });
                self.persistence.changed();
                self.cache_removals.push(PendingCacheRemoval {
                    revision: self.persistence.revision,
                    id,
                    path: PathBuf::from(path),
                });
                state.notice = Some("Saving cache removal…".into());
            },
            CompactCommand::Subscribe(url) => {
                if url.trim().is_empty() {
                    return Err("Enter a podcast feed URL first".into());
                }
                self.fetch_feed(state, url)?;
            },
            CompactCommand::RefreshSubscription(url) => self.fetch_feed(state, url)?,
            CompactCommand::BeginTextNote | CompactCommand::AddTextNote => {
                self.begin_note(state)?
            },
            CompactCommand::SaveTextNote { anchor, plain_text } => {
                self.save_note(state, anchor, plain_text)?
            },
            CompactCommand::BeginEditNote(id) => {
                if state.text_capture.is_some() {
                    return Err("Save or cancel the current note first".into());
                }
                let note = self
                    .session
                    .model
                    .annotations
                    .get(&id)
                    .ok_or("That note no longer exists")?
                    .clone();
                self.open_note_target(&note)?;
                let NoteBody::Text { plain_text } = &note.body else {
                    return Err("Only text notes can be edited here".into());
                };
                state.text_capture = Some(TextCapture {
                    anchor: note.target.clone(),
                    draft: plain_text.clone(),
                });
                state.set_text_draft(plain_text.clone());
                state.editing_note = Some(id);
            },
            CompactCommand::OpenVoiceNote(id) => {
                let note = self
                    .session
                    .model
                    .annotations
                    .get(&id)
                    .ok_or("That voice note no longer exists")?
                    .clone();
                if !matches!(note.body, NoteBody::Audio { .. }) {
                    return Err("That is not a voice note".into());
                }
                self.open_note_target(&note)?;
                state.notice = Some("Moved playback to the voice note's anchor".into());
            },
            CompactCommand::EditNote { id, plain_text } => {
                if state.editing_note.as_ref() != Some(&id) {
                    return Err("Open that note for editing first".into());
                }
                let anchor = state
                    .text_capture
                    .as_ref()
                    .ok_or("The note editor is closed")?
                    .anchor
                    .clone();
                self.save_note(state, anchor, plain_text)?;
            },
            CompactCommand::CancelTextNote => {
                state.text_capture = None;
                state.editing_note = None;
                state.set_text_draft("");
            },
            CompactCommand::Enqueue(id) => {
                self.session
                    .model
                    .enqueue(&id)
                    .map_err(|e| format!("Could not queue item: {e:?}"))?;
                self.persistence.changed();
            },
            CompactCommand::Dequeue(id) => {
                self.session
                    .model
                    .dequeue(&id)
                    .map_err(|e| format!("Could not remove queued item: {e:?}"))?;
                self.persistence.changed();
            },
            CompactCommand::RemoveLibraryItem(id) => {
                let was_selected = self.session.selected.as_ref() == Some(&id);
                let removed_voice_blobs: Vec<_> = self
                    .session
                    .model
                    .annotations_for_item(&id)
                    .into_iter()
                    .filter_map(|note| match &note.body {
                        NoteBody::Audio { blob_id, .. } => Some(blob_id.clone()),
                        NoteBody::Text { .. } => None,
                    })
                    .collect();
                let item = self
                    .session
                    .model
                    .remove_item(&id)
                    .map_err(|e| format!("Could not remove library item: {e:?}"))?;
                if was_selected {
                    self.session.selected = None;
                    let _ = self.send(PlaybackCommand::Stop);
                }
                if state
                    .text_capture
                    .as_ref()
                    .is_some_and(|capture| capture.anchor.item_id == id)
                {
                    state.text_capture = None;
                    state.editing_note = None;
                    state.set_text_draft("");
                }
                let cached_path = match item.source() {
                    MediaSource::Cached { path, .. } => Some(PathBuf::from(path)),
                    _ => None,
                };
                self.persistence.changed();
                self.voice_removals
                    .extend(
                        removed_voice_blobs
                            .into_iter()
                            .map(|blob_id| PendingVoiceRemoval {
                                revision: self.persistence.revision,
                                blob_id,
                            }),
                    );
                if let Some(path) = cached_path {
                    self.cache_removals.push(PendingCacheRemoval {
                        revision: self.persistence.revision,
                        id,
                        path,
                    });
                }
                state.notice = Some("Removed from library".into());
            },
            CompactCommand::MoveQueue { from, to } => {
                self.session
                    .model
                    .reorder_queue(from, to)
                    .map_err(|e| format!("Could not move queued item: {e:?}"))?;
                self.persistence.changed();
            },
            CompactCommand::DeleteNote(id) => {
                if state.editing_note.as_ref() == Some(&id) {
                    return Err("Close the note editor before deleting this note".into());
                }
                let voice_blob =
                    self.session
                        .model
                        .annotations
                        .get(&id)
                        .and_then(|note| match &note.body {
                            NoteBody::Audio { blob_id, .. } => Some(blob_id.clone()),
                            NoteBody::Text { .. } => None,
                        });
                self.session
                    .model
                    .delete_annotation(&id)
                    .map_err(|e| format!("Could not delete note: {e:?}"))?;
                self.persistence.changed();
                if let Some(blob_id) = voice_blob {
                    self.voice_removals.push(PendingVoiceRemoval {
                        revision: self.persistence.revision,
                        blob_id,
                    });
                }
            },
            CompactCommand::UpdateSettings(settings) => {
                self.session.model.settings = settings;
                self.persistence.changed();
            },
            CompactCommand::BeginVoiceNote => self.begin_voice_note(state)?,
            CompactCommand::FinishVoiceNote => self.finish_voice_note(state)?,
        }
        Ok(())
    }

    fn dispatch(&mut self, state: &mut RedshankSurfaceState) {
        let commands: Vec<_> = state.drain_commands().collect();
        for command in commands {
            if let Err(error) = self.command(state, command) {
                state.notice = Some(error);
            }
        }
        self.session.project(state, &self.runtime.snapshot());
        if let Err(error) = self.persistence.flush(&self.session.model) {
            state.notice = Some(error);
        }
    }

    fn poll(&mut self, state: &mut RedshankSurfaceState) {
        while let Ok(reply) = self.persistence.replies.try_recv() {
            match reply {
                IoReply::Opened(path) => {
                    self.dialog_pending = false;
                    if let Some(path) = path
                        && let Err(error) = self.open(path)
                    {
                        state.notice = Some(error);
                    }
                },
                IoReply::Cached { id, result } => {
                    self.cache_pending = None;
                    match result {
                        Err(error) => state.notice = Some(error),
                        Ok(source) => {
                            let origin = source.enclosure_url().map(str::to_owned);
                            let Some(item) = self.session.model.library.get_mut(&id) else {
                                state.notice = Some(
                                    "The download finished after its library item was removed"
                                        .into(),
                                );
                                continue;
                            };
                            if item.source().enclosure_url() != origin.as_deref() {
                                state.notice = Some(
                                    "The recording source changed while its download was running"
                                        .into(),
                                );
                                continue;
                            }
                            item.replace_source(source);
                            self.persistence.changed();
                            if self.session.selected.as_ref() == Some(&id)
                                && let Err(error) = self.select(id)
                            {
                                state.notice = Some(error);
                                continue;
                            }
                            state.notice = Some("Episode is available offline".into());
                        },
                    }
                },
                IoReply::FeedFetched {
                    requested_url,
                    result,
                } => {
                    if self.feed_pending.as_deref() == Some(requested_url.as_str()) {
                        self.feed_pending = None;
                    }
                    match result {
                        Err(error) => state.notice = Some(error),
                        Ok(imported) => {
                            let title = imported.subscription.title.clone();
                            let mut added = 0_usize;
                            self.session
                                .model
                                .upsert_subscription(imported.subscription);
                            for episode in imported.episodes {
                                match self.session.model.upsert_feed_item(episode) {
                                    Ok(true) => added += 1,
                                    Ok(false) => {},
                                    Err(error) => {
                                        state.notice =
                                            Some(format!("Could not merge feed: {error:?}"));
                                        continue;
                                    },
                                }
                            }
                            self.persistence.changed();
                            state.feed_url_editor = Default::default();
                            state.notice = Some(format!("Refreshed {title}; {added} new episodes"));
                        },
                    }
                },
                IoReply::CacheRemoved { id, result } => {
                    self.cache_removals_in_flight = self.cache_removals_in_flight.saturating_sub(1);
                    state.notice = Some(match result {
                        Ok(true) => format!("Removed offline download for {}", id.0),
                        Ok(false) => format!("Offline download for {} was already absent", id.0),
                        Err(error) => error,
                    });
                },
                IoReply::VoiceRemoved(result) => {
                    self.voice_removals_in_flight = self.voice_removals_in_flight.saturating_sub(1);
                    if let Err(error) = result {
                        state.notice = Some(error);
                    }
                },
                IoReply::Saved { revision, result } => {
                    self.persistence.acknowledge(revision, &result);
                    match result {
                        Err(error) => {
                            state.notice = Some(error);
                            self.closing = false;
                        },
                        Ok(()) => {
                            self.schedule_durable_cache_removals(revision, state);
                            self.schedule_durable_voice_removals(revision, state);
                            if self
                                .voice_save_revision
                                .is_some_and(|voice_revision| voice_revision <= revision)
                            {
                                self.voice_save_revision = None;
                                state.notice = Some("Voice note saved".into());
                            }
                            if self
                                .saved_draft
                                .as_ref()
                                .is_some_and(|draft| draft.revision <= revision)
                            {
                                let draft = self.saved_draft.take().expect("saved draft");
                                if state.editing_note.as_ref() == Some(&draft.id)
                                    && state
                                        .text_capture
                                        .as_ref()
                                        .is_some_and(|c| c.anchor == draft.anchor)
                                    && state.text_editor.text() == draft.text
                                {
                                    state.text_capture = None;
                                    state.editing_note = None;
                                    state.set_text_draft("");
                                    state.notice = Some("Note saved".into());
                                } else if self.closing && state.text_capture.is_some() {
                                    self.closing = false;
                                    state.notice = Some(
                                        "The draft changed while saving; it is still open".into(),
                                    );
                                }
                            }
                        },
                    }
                },
            }
        }
        if let Some(error) = self.voice_capture.active_error() {
            let _ = self.voice_capture.cancel_capture();
            let capture = self.voice_session.take();
            state.compact.voice_capture_active = false;
            if capture.is_some_and(|capture| capture.resume_playback) {
                let _ = self.send(PlaybackCommand::Play);
            }
            state.notice = Some(error);
        }
        let snapshot = self.runtime.snapshot();
        if snapshot.state != PlaybackState::Playing
            || self.last_progress.elapsed() >= Duration::from_secs(5)
        {
            self.last_progress = Instant::now();
            if self.session.record_progress(&snapshot, now_ms()) {
                self.persistence.changed();
            }
        }
        self.session.project(state, &snapshot);
        if let Err(error) = self.persistence.flush(&self.session.model) {
            state.notice = Some(error);
            self.persistence.failed = true;
            self.closing = false;
        }
    }

    fn close(&mut self, state: &mut RedshankSurfaceState) -> Result<(), String> {
        self.send(PlaybackCommand::Pause)?;
        if self.voice_session.take().is_some() {
            self.voice_capture.cancel_capture()?;
            state.compact.voice_capture_active = false;
        }
        if let Some(capture) = &state.text_capture {
            let text = state.text_editor.text().to_owned();
            if !text.trim().is_empty() {
                self.save_note(state, capture.anchor.clone(), text)?;
            }
        }
        // Persist queue order independently; the most recently selected item is
        // available in the library and each item retains its own progress.
        if self
            .session
            .record_progress(&self.runtime.snapshot(), now_ms())
        {
            self.persistence.changed();
        }
        self.persistence.failed = false;
        self.persistence.flush(&self.session.model)?;
        self.closing = true;
        Ok(())
    }
}

fn focused_text_class(runner: &AppRunner) -> Option<String> {
    let node = runner.focus()?;
    let dom = runner.dom();
    let dom = dom.borrow();
    let name = dom.element_name(node)?;
    if name.local.as_ref() != "textarea" {
        return None;
    }
    let parent = dom.parent(node)?;
    dom.attribute(
        parent,
        &layout_dom_api::Namespace::from(""),
        &layout_dom_api::LocalName::from("class"),
    )
    .map(str::to_owned)
}

fn focused_text(runner: &AppRunner) -> Option<FocusedTextSlot<RedshankSurfaceState>> {
    let node = runner.focus()?;
    if focused_text_class(runner).as_deref() == Some("redshank-feed-url") {
        return Some(FocusedTextSlot {
            node,
            get: Box::new(|state| &state.feed_url_editor),
            get_mut: Box::new(|state| &mut state.feed_url_editor),
        });
    }
    Some(FocusedTextSlot {
        node,
        get: Box::new(|state| &state.text_editor),
        get_mut: Box::new(|state| &mut state.text_editor),
    })
}

fn key_intercept(runner: &mut AppRunner, key: &KeyPress) -> bool {
    let note_editor_focused = focused_text_class(runner).as_deref() == Some("redshank-text-editor");
    let state = runner.state();
    let command = if key.modifiers.is_command_chord() {
        match &key.key {
            Key::Character(c) if c.eq_ignore_ascii_case("o") => Some(CompactCommand::OpenLocalFile),
            Key::Named(NamedKey::Enter) if note_editor_focused => {
                state.text_capture.as_ref().map(|capture| {
                    if let Some(id) = &state.editing_note {
                        CompactCommand::EditNote {
                            id: id.clone(),
                            plain_text: state.text_editor.text().into(),
                        }
                    } else {
                        CompactCommand::SaveTextNote {
                            anchor: capture.anchor.clone(),
                            plain_text: state.text_editor.text().into(),
                        }
                    }
                })
            },
            _ => None,
        }
    } else if focused_text(runner).is_some() || key.modifiers.alt {
        None
    } else if matches!(
        state.compact.transport,
        TransportState::Playing | TransportState::Paused
    ) {
        match &key.key {
            Key::Named(NamedKey::Space) => {
                Some(if state.compact.transport == TransportState::Playing {
                    CompactCommand::Pause
                } else {
                    CompactCommand::Play
                })
            },
            Key::Named(NamedKey::ArrowLeft) => Some(CompactCommand::SkipBackward(
                state.settings.skip_backward_ms,
            )),
            Key::Named(NamedKey::ArrowRight) => {
                Some(CompactCommand::SkipForward(state.settings.skip_forward_ms))
            },
            Key::Character(c) if c.eq_ignore_ascii_case("n") => Some(CompactCommand::BeginTextNote),
            Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                Some(if state.compact.voice_capture_active {
                    CompactCommand::FinishVoiceNote
                } else {
                    CompactCommand::BeginVoiceNote
                })
            },
            _ => None,
        }
    } else {
        None
    };
    if let Some(command) = command {
        runner.update(|state| state.request(command));
        true
    } else {
        false
    }
}

fn hooks(
    desktop: Rc<RefCell<Desktop>>,
) -> HostHooks<RedshankSurfaceState, Logic, redshank_surfaces::FullView> {
    let frame = Rc::clone(&desktop);
    let dispatch = Rc::clone(&desktop);
    let wake = Rc::clone(&desktop);
    HostHooks {
        frame: Box::new(move |ctx: &mut Context<'_>| {
            let mut desktop = frame.borrow_mut();
            if desktop.last_projection.elapsed() >= Duration::from_millis(100) || desktop.closing {
                desktop.last_projection = Instant::now();
                let mut next = ctx.runner.state().clone();
                desktop.dispatch(&mut next);
                desktop.poll(&mut next);
                if &next != ctx.runner.state() {
                    ctx.runner.update(|state| *state = next);
                }
            }
            if let Some(mut receipt) = desktop.receipt.take() {
                let mut next = ctx.runner.state().clone();
                if let Err(error) = receipt.drive(&mut desktop, &mut next) {
                    receipt.fail(error);
                    *ctx.close = true;
                }
                if &next != ctx.runner.state() {
                    ctx.runner.update(|state| *state = next);
                }
                desktop.receipt = Some(receipt);
            }
            if desktop.closing
                && desktop.persistence.durable == desktop.persistence.revision
                && desktop.cache_removals.is_empty()
                && desktop.cache_removals_in_flight == 0
                && desktop.voice_removals.is_empty()
                && desktop.voice_removals_in_flight == 0
                && desktop
                    .receipt
                    .as_ref()
                    .is_none_or(|receipt| !receipt.active())
            {
                *ctx.close = true;
            }
            // Worker changes need polling until playback and persistence settle.
            let snap = desktop.runtime.snapshot();
            desktop.closing
                || desktop.dialog_pending
                || desktop.cache_pending.is_some()
                || !desktop.cache_removals.is_empty()
                || desktop.cache_removals_in_flight > 0
                || desktop.voice_session.is_some()
                || !desktop.voice_removals.is_empty()
                || desktop.voice_removals_in_flight > 0
                || desktop.feed_pending.is_some()
                || desktop.persistence.in_flight.is_some()
                || desktop.receipt.as_ref().is_some_and(HeadedReceipt::active)
                || !desktop.session.matches(&snap) && desktop.session.selected.is_some()
                || matches!(snap.state, PlaybackState::Loading | PlaybackState::Playing)
        }),
        after_dispatch: Box::new(move |ctx: &mut Context<'_>| {
            ctx.runner
                .update(|state| dispatch.borrow_mut().dispatch(state));
        }),
        after_frame: Box::new(|_| {}),
        after_wake: Box::new(move |ctx| {
            let mut desktop = wake.borrow_mut();
            let mut next = ctx.runner.state().clone();
            desktop.poll(&mut next);
            if &next != ctx.runner.state() {
                ctx.runner.update(|state| *state = next);
            }
        }),
        close_request: Box::new(move |ctx, _| {
            ctx.runner.update(|state| {
                if let Err(error) = desktop.borrow_mut().close(state) {
                    state.notice = Some(error);
                }
            });
            CloseDisposition::KeepVisible
        }),
        focused_text: Box::new(focused_text),
        key_intercept: Box::new(key_intercept),
    }
}

fn main() {
    let receipt = HeadedReceipt::from_environment().expect("configure headed receipt");
    let data_root = data_directory();
    let store = JsonDirectoryStore::new(&data_root);
    let (model, notice) = match store.load() {
        Ok(model) => (model.unwrap_or_default(), None),
        Err(error) => (
            RedshankModel::default(),
            Some(format!("Could not restore listener state: {error:?}")),
        ),
    };
    let mut desktop = Desktop {
        session: Session::new(model),
        runtime: PlaybackRuntime::start(),
        persistence: Persistence::start(store),
        dialog_pending: false,
        cache_pending: None,
        cache_removals: Vec::new(),
        cache_removals_in_flight: 0,
        voice_capture: LocalVoiceCapture::new(&data_root),
        voice_session: None,
        voice_save_revision: None,
        voice_removals: Vec::new(),
        voice_removals_in_flight: 0,
        feed_pending: None,
        data_root,
        closing: false,
        saved_draft: None,
        receipt,
        last_progress: Instant::now(),
        last_projection: Instant::now(),
    };
    let mut state = RedshankSurfaceState::default();
    state.notice = notice;
    state.compact.voice_capture_available = LocalVoiceCapture::is_available();
    // An optional local path or direct URL supports associations and reproducible receipts.
    let initial = std::env::args_os().nth(1);
    let restored = desktop
        .session
        .model
        .selected_item
        .clone()
        .filter(|id| desktop.session.model.library.contains_key(id))
        .or_else(|| desktop.session.model.queue.first().cloned());
    let result = if let Some(source) = initial {
        match source.to_str() {
            Some(url) if url.starts_with("http://") || url.starts_with("https://") => {
                desktop.open_url(url.to_owned())
            },
            _ => desktop.open(PathBuf::from(source)),
        }
    } else if let Some(id) = restored {
        desktop.select(id)
    } else {
        Ok(())
    };
    if let Err(error) = result {
        state.notice = Some(error);
    }
    desktop
        .session
        .project(&mut state, &desktop.runtime.snapshot());
    let wake_runtime = desktop.runtime.clone();
    let desktop = Rc::new(RefCell::new(desktop));
    run(
        HostOptions {
            title: "Redshank".into(),
            initial_logical_size: (860.0, 680.0),
            window_frame: WindowFrame::Host,
            ..HostOptions::default()
        },
        move |_, _, wake| {
            wake_runtime.set_wake(wake.callback());
            wake.wake();
            Init {
                state,
                logic: surface as Logic,
                sheet: COMPACT_SHEET.into(),
            }
        },
        hooks(Rc::clone(&desktop)),
    )
    .expect("run Redshank");
    if let Some(receipt) = desktop.borrow().receipt.as_ref() {
        println!("{}", receipt.result().expect("complete headed receipt"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write as _, net::TcpListener};

    #[test]
    fn standalone_feed_fetch_retains_final_parser_facts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            let body = r#"<rss version="2.0"><channel><title>Local Feed</title><item><guid>one</guid><title>One</title><enclosure url="episode.mp3" type="audio/mpeg" length="3"/></item></channel></rss>"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/rss+xml\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let url = format!("http://{address}/feed.xml");
        let imported = fetch_and_import_feed(&url).unwrap();
        server.join().unwrap();
        assert_eq!(imported.subscription.title, "Local Feed");
        assert_eq!(imported.episodes.len(), 1);
        assert_eq!(
            imported.episodes[0].source().enclosure_url(),
            Some(format!("http://{address}/episode.mp3").as_str())
        );
    }

    #[test]
    fn cache_object_is_removed_only_after_its_model_revision_is_durable() {
        let directory = tempfile::tempdir().unwrap();
        let cache_root = directory.path().join("cache");
        fs::create_dir_all(&cache_root).unwrap();
        let cached_path = cache_root.join("complete.audio");
        fs::write(&cached_path, b"complete").unwrap();
        let id = ItemId("cached".into());
        let mut desktop = desktop(directory.path());
        desktop
            .session
            .model
            .add_item(LibraryItem::DirectAudio {
                id: id.clone(),
                title: "Cached".into(),
                source: MediaSource::Cached {
                    path: cached_path.to_string_lossy().into_owned(),
                    origin_url: "https://example.test/cached.mp3".into(),
                    representation: Box::default(),
                },
            })
            .unwrap();
        let mut state = RedshankSurfaceState::default();
        desktop
            .command(&mut state, CompactCommand::RemoveCachedItem(id.clone()))
            .unwrap();
        assert!(cached_path.exists());
        assert!(matches!(
            desktop.session.model.library[&id].source(),
            MediaSource::Enclosure { .. }
        ));

        desktop
            .persistence
            .reply_sender
            .send(IoReply::Saved {
                revision: desktop.persistence.revision,
                result: Err("disk full".into()),
            })
            .unwrap();
        desktop.poll(&mut state);
        assert!(cached_path.exists());
        assert_eq!(desktop.cache_removals.len(), 1);

        desktop
            .persistence
            .reply_sender
            .send(IoReply::Saved {
                revision: desktop.persistence.revision,
                result: Ok(()),
            })
            .unwrap();
        for _ in 0..100 {
            desktop.poll(&mut state);
            if desktop.cache_removals_in_flight == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!cached_path.exists());
        assert_eq!(desktop.cache_removals_in_flight, 0);
    }

    #[test]
    fn voice_blob_is_removed_only_after_note_deletion_is_durable() {
        let directory = tempfile::tempdir().unwrap();
        let digest = "a".repeat(64);
        let blob_id = format!("voice:{digest}");
        let voice_root = directory.path().join("voice");
        fs::create_dir_all(&voice_root).unwrap();
        let blob_path = voice_root.join(format!("{digest}.wav"));
        fs::write(&blob_path, b"voice").unwrap();
        let mut desktop = desktop(directory.path());
        let id = AnnotationId("voice-note".into());
        desktop
            .session
            .model
            .add_annotation(Annotation {
                id: id.clone(),
                target: CaptureAnchor {
                    item_id: ItemId("a".into()),
                    offset_ms: 321,
                    representation: Default::default(),
                },
                body: NoteBody::Audio {
                    blob_id,
                    media_type: "audio/wav".into(),
                    duration_ms: 500,
                },
                created_at_ms: 1,
            })
            .unwrap();
        let mut state = RedshankSurfaceState::default();
        desktop
            .command(&mut state, CompactCommand::DeleteNote(id))
            .unwrap();
        assert!(blob_path.exists());

        desktop
            .persistence
            .reply_sender
            .send(IoReply::Saved {
                revision: desktop.persistence.revision,
                result: Ok(()),
            })
            .unwrap();
        for _ in 0..100 {
            desktop.poll(&mut state);
            if desktop.voice_removals_in_flight == 0 && desktop.voice_removals.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!blob_path.exists());
    }

    #[test]
    fn removing_selected_library_item_clears_player_queue_and_editor() {
        let directory = tempfile::tempdir().unwrap();
        let mut desktop = desktop(directory.path());
        let id = ItemId("a".into());
        desktop.session.selected = Some(id.clone());
        desktop.session.model.selected_item = Some(id.clone());
        desktop.session.model.enqueue(&id).unwrap();
        let mut state = RedshankSurfaceState::default();
        state.text_capture = Some(TextCapture {
            anchor: CaptureAnchor {
                item_id: id.clone(),
                offset_ms: 321,
                representation: Default::default(),
            },
            draft: "draft".into(),
        });
        state.editing_note = Some(AnnotationId("draft".into()));
        state.set_text_draft("draft");

        desktop
            .command(&mut state, CompactCommand::RemoveLibraryItem(id.clone()))
            .unwrap();

        assert!(!desktop.session.model.library.contains_key(&id));
        assert!(desktop.session.model.queue.is_empty());
        assert_eq!(desktop.session.selected, None);
        assert_eq!(desktop.session.model.selected_item, None);
        assert!(state.text_capture.is_none());
        assert!(state.editing_note.is_none());
        assert_eq!(state.text_editor.text(), "");
        assert_eq!(state.notice.as_deref(), Some("Removed from library"));
    }

    #[test]
    fn host_routes_typing_to_editor_and_command_enter_to_note_save() {
        let directory = tempfile::tempdir().unwrap();
        let desktop = Rc::new(RefCell::new(desktop(directory.path())));
        let anchor = CaptureAnchor {
            item_id: ItemId("a".into()),
            offset_ms: 321,
            representation: Default::default(),
        };
        let mut state = RedshankSurfaceState::default();
        state.text_capture = Some(TextCapture {
            anchor: anchor.clone(),
            draft: String::new(),
        });
        let mut host = cambium_genet_winit_host::Harness::with_hooks(
            Init {
                state,
                logic: surface as Logic,
                sheet: COMPACT_SHEET.into(),
            },
            hooks(Rc::clone(&desktop)),
        );
        host.layout_at(860.0, 680.0);
        for _ in 0..40 {
            if focused_text_class(host.runner()).as_deref() == Some("redshank-text-editor") {
                break;
            }
            host.tab(true);
        }
        assert!(
            focused_text_class(host.runner()).as_deref() == Some("redshank-text-editor"),
            "keyboard must reach the note editor"
        );
        host.key_char("n");
        host.key_injected("ote");
        assert_eq!(host.state().text_editor.text(), "note");
        host.press_key(&KeyPress {
            key: Key::Named(NamedKey::Enter),
            text: None,
            modifiers: cambium_genet_winit_host::Modifiers {
                ctrl: true,
                ..Default::default()
            },
            repeat: false,
        });
        assert_eq!(desktop.borrow().session.model.annotations.len(), 1);
        let note = desktop
            .borrow()
            .session
            .model
            .annotations
            .values()
            .next()
            .unwrap()
            .clone();
        assert_eq!(note.target, anchor);
        assert_eq!(
            note.body,
            NoteBody::Text {
                plain_text: "note".into()
            }
        );
    }

    fn desktop(directory: &std::path::Path) -> Desktop {
        let mut model = RedshankModel::default();
        model
            .add_item(LibraryItem::LocalAudio {
                id: ItemId("a".into()),
                title: "A".into(),
                source: MediaSource::Local {
                    path: "a.mp3".into(),
                },
            })
            .unwrap();
        Desktop {
            session: Session::new(model),
            runtime: PlaybackRuntime::start(),
            persistence: Persistence::start(JsonDirectoryStore::new(directory)),
            dialog_pending: false,
            cache_pending: None,
            cache_removals: Vec::new(),
            cache_removals_in_flight: 0,
            voice_capture: LocalVoiceCapture::new(directory),
            voice_session: None,
            voice_save_revision: None,
            voice_removals: Vec::new(),
            voice_removals_in_flight: 0,
            feed_pending: None,
            data_root: directory.to_owned(),
            closing: false,
            saved_draft: None,
            receipt: None,
            last_progress: Instant::now(),
            last_projection: Instant::now(),
        }
    }

    #[test]
    fn note_draft_survives_failed_save_and_later_typing_survives_success() {
        let directory = tempfile::tempdir().unwrap();
        let mut desktop = desktop(directory.path());
        let mut state = RedshankSurfaceState::default();
        let anchor = CaptureAnchor {
            item_id: ItemId("a".into()),
            offset_ms: 123,
            representation: Default::default(),
        };
        state.text_capture = Some(TextCapture {
            anchor: anchor.clone(),
            draft: String::new(),
        });
        state.set_text_draft("first version");
        desktop
            .save_note(&mut state, anchor.clone(), "first version".into())
            .unwrap();
        let id = state.editing_note.clone().unwrap();
        desktop
            .persistence
            .reply_sender
            .send(IoReply::Saved {
                revision: 1,
                result: Err("disk full".into()),
            })
            .unwrap();
        desktop.poll(&mut state);
        assert_eq!(state.text_editor.text(), "first version");
        assert!(state.text_capture.is_some());
        assert_eq!(desktop.persistence.durable, 0);
        state.set_text_draft("second version");
        desktop.closing = true;
        desktop
            .persistence
            .reply_sender
            .send(IoReply::Saved {
                revision: 1,
                result: Ok(()),
            })
            .unwrap();
        desktop.poll(&mut state);
        assert_eq!(state.text_editor.text(), "second version");
        assert!(!desktop.closing);
        desktop
            .save_note(&mut state, anchor.clone(), "second version".into())
            .unwrap();
        assert_eq!(desktop.session.model.annotations.len(), 1);
        assert_eq!(desktop.session.model.annotations[&id].target, anchor);
        assert_eq!(
            desktop.session.model.annotations[&id].body,
            NoteBody::Text {
                plain_text: "second version".into()
            }
        );
    }

    #[test]
    fn failed_save_preserves_dirty_revision_and_can_retry() {
        let directory = tempfile::tempdir().unwrap();
        let mut persistence = Persistence::start(JsonDirectoryStore::new(directory.path()));
        persistence.changed();
        persistence.in_flight = Some(1);
        persistence.acknowledge(1, &Err("disk full".into()));
        assert_eq!(persistence.durable, 0);
        assert_eq!(persistence.revision, 1);
        persistence.failed = false;
        persistence.flush(&RedshankModel::default()).unwrap();
        let IoReply::Saved { revision, result } = persistence
            .replies
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        else {
            panic!("wrong reply")
        };
        persistence.acknowledge(revision, &result);
        assert_eq!(persistence.durable, 1);
        assert!(
            JsonDirectoryStore::new(directory.path())
                .load()
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn later_mutation_remains_dirty_after_earlier_save_ack() {
        let directory = tempfile::tempdir().unwrap();
        let mut persistence = Persistence::start(JsonDirectoryStore::new(directory.path()));
        persistence.changed();
        persistence.flush(&RedshankModel::default()).unwrap();
        persistence.changed();
        let IoReply::Saved { revision, result } = persistence
            .replies
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        else {
            panic!("wrong reply")
        };
        persistence.acknowledge(revision, &result);
        assert_eq!(persistence.durable, 1);
        assert_eq!(persistence.revision, 2);
    }
}
