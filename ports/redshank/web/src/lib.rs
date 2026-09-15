//! Redshank in a browser.
//!
//! The claim is narrow and worth stating exactly: the views here are
//! `redshank_surfaces::surface`, the same function the sovereign desktop
//! draws, with the same `redshank_surfaces::sheet()`, rendered by the same
//! Cambium and genet-layout and netrender onto a canvas instead of a window.
//! Nothing about the interface is reimplemented in JavaScript.
//!
//! What differs is everything below the surface. The desktop's `Desktop`
//! owns a Symphonia decoder on a Firewheel output, a JSON store, a file
//! picker, a CPAL microphone, and a Rustls fetcher. The browser has none of
//! them in this stack yet. So this host is an **honest half**: it seeds an
//! in-memory fixture, applies every command that is pure state, and answers
//! the rest with a notice naming the command, rather than pretending.
//!
//! | command class | here |
//! |---|---|
//! | navigation, layout, scene, filters, settings | applied |
//! | queue order, seek, rate, volume, text notes | applied, in memory |
//! | Play/Pause/Replay | toggles `TransportState`, no clock advances |
//! | voice capture | shows the recording row; no microphone, nothing saved |
//! | files, network, cache, export | refused with a notice |
//!
//! See `web/README.md` for the build, bindgen, and serve recipe.
#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use cambium_genet_web_host::mount;
use cambium_rootstock::{AppCtx, HostFont, HostHooks, HostOptions, Init};
use redshank_model::{
    AnnotationId, ItemId, ListenerSettings, RepresentationReceipt, ThemeMode, ThemeSeed,
    TimedTarget,
};
use redshank_surfaces::{
    CompactCommand, CompactPlayerState, FONTS, Face, FeedRow, FullView, ItemRow,
    ListeningSessionRow, Mode, NoteMarker, NoteSummary, NoteSummaryBody, NowPlaying, Recording,
    RedshankSurfaceState, RepresentationSummary, Seed, SourceKind, TextCapture, TransportState,
    VoiceNotePreview, sheet, surface,
};
use wasm_bindgen::prelude::*;

/// The canvas this looks for when none is named.
const DEFAULT_CANVAS_ID: &str = "redshank";

type Logic = fn(&RedshankSurfaceState) -> FullView;
type Context<'a> = AppCtx<'a, RedshankSurfaceState, Logic, FullView>;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

const FEED_URL: &str = "https://example.invalid/marsh-hours.xml";
const LOCAL_ID: &str = "local-mudflat-take";

fn item_id(raw: &str) -> ItemId {
    ItemId(raw.to_owned())
}

fn episode(index: usize, title: &str, position_ms: u64, completed: bool) -> ItemRow {
    ItemRow {
        id: item_id(&format!("marsh-{index}")),
        title: title.to_owned(),
        feed_url: Some(FEED_URL.to_owned()),
        feed_title: Some("Marsh Hours".to_owned()),
        face: Face::Tag("mp3".to_owned()),
        source: if index <= 2 {
            SourceKind::Offline
        } else {
            SourceKind::Cloud
        },
        duration_ms: Some(2_820_000 + index as u64 * 120_000),
        position_ms,
        completed,
        published: Some(format!("2026-0{}-1{}", 4 + index / 3, index % 3 + 1)),
        cached_bytes: (index <= 2).then_some(41_943_040),
        note_count: if index == 1 { 3 } else { 0 },
        unavailable: (index == 5).then(|| "Enclosure returned 404".to_owned()),
        pinned: index == 1,
        representation: (index <= 2).then(|| RepresentationSummary {
            retrieved_at_ms: Some(1_757_635_200_000),
            short_digest: Some("c41f9ab2".to_owned()),
            matches: Some(true),
        }),
    }
}

/// The anchor a browser-captured note freezes against. No representation
/// receipt is claimed: nothing was fetched, so none is honest.
fn anchor(item: &ItemId, offset_ms: u64) -> TimedTarget {
    TimedTarget {
        item_id: item.clone(),
        offset_ms,
        end_offset_ms: None,
        pressed_offset_ms: Some(offset_ms),
        representation: RepresentationReceipt::default(),
    }
}

fn note(id: &str, offset_ms: u64, body: NoteSummaryBody) -> NoteSummary {
    NoteSummary {
        id: AnnotationId(id.to_owned()),
        item_id: item_id(LOCAL_ID),
        offset_ms,
        end_offset_ms: None,
        body,
        private: true,
    }
}

/// A library with enough in it that every tab has something to draw.
fn fixture(seed: Seed, mode: Mode) -> RedshankSurfaceState {
    let local = ItemRow {
        id: item_id(LOCAL_ID),
        title: "Mudflat take 3".to_owned(),
        feed_url: None,
        feed_title: None,
        face: Face::Tag("wav".to_owned()),
        source: SourceKind::Local,
        duration_ms: Some(121_000),
        position_ms: 84_000,
        completed: false,
        published: None,
        cached_bytes: None,
        note_count: 3,
        unavailable: None,
        pinned: false,
        representation: None,
    };

    let episodes = [
        episode(0, "The tide is a clock", 2_820_000, true),
        episode(1, "Wader counts, and who does them", 640_000, false),
        episode(2, "Saltmarsh, in three revisions", 0, false),
        episode(3, "A ringing station in February", 0, false),
        episode(4, "What the estuary forgets", 0, false),
        episode(5, "Night passage", 0, false),
    ];

    let notes = vec![
        note(
            "web-note-1",
            41_000,
            NoteSummaryBody::Text("The room tone changes here — second mic?".to_owned()),
        ),
        note(
            "web-note-2",
            67_500,
            NoteSummaryBody::Text("Keep this phrasing for the cold open.".to_owned()),
        ),
        note(
            "web-note-3",
            96_200,
            NoteSummaryBody::Voice {
                duration_ms: 7_400,
                preview: VoiceNotePreview::Idle,
            },
        ),
    ];

    let markers: Vec<NoteMarker> = notes.iter().map(NoteSummary::marker).collect();

    // Both state structs keep a private command queue, so they are seeded by
    // mutation rather than struct-update syntax.
    let mut compact = CompactPlayerState::default();
    // The browser host has no audio. Paused is the truthful resting state:
    // there is a position and a duration, and nothing is advancing them.
    compact.transport = TransportState::Paused;
    compact.now_playing = Some(NowPlaying {
        item_id: local.id.clone(),
        title: local.title.clone(),
        feed_title: None,
        face: local.face.clone(),
        source: SourceKind::Local,
        position_ms: 84_000,
        duration_ms: Some(121_000),
        resumed_from_ms: Some(72_000),
        buffered_percent: 100,
        markers,
    });
    // The recording row is reachable so the design can be read at that state;
    // nothing is captured, and Finish says so.
    compact.voice_capture_available = true;

    let sessions = vec![
        ListeningSessionRow {
            item_id: item_id(LOCAL_ID),
            title: "Mudflat take 3".to_owned(),
            start_ms: 0,
            stop_ms: 84_000,
            duration_ms: Some(121_000),
            day_label: "TODAY".to_owned(),
            wall_clock: "09:14".to_owned(),
            note_offsets_ms: vec![41_000, 67_500, 96_200],
            completed: false,
            active: true,
        },
        ListeningSessionRow {
            item_id: item_id("marsh-1"),
            title: "Wader counts, and who does them".to_owned(),
            start_ms: 0,
            stop_ms: 640_000,
            duration_ms: Some(2_940_000),
            day_label: "TODAY".to_owned(),
            wall_clock: "07:52".to_owned(),
            note_offsets_ms: vec![210_000, 488_000],
            completed: false,
            active: false,
        },
        ListeningSessionRow {
            item_id: item_id("marsh-0"),
            title: "The tide is a clock".to_owned(),
            start_ms: 1_250_000,
            stop_ms: 2_820_000,
            duration_ms: Some(2_820_000),
            day_label: "YESTERDAY".to_owned(),
            wall_clock: "21:06".to_owned(),
            note_offsets_ms: vec![1_760_000],
            completed: true,
            active: false,
        },
    ];

    let settings = ListenerSettings {
        seed: match seed {
            Seed::Wetland => ThemeSeed::Wetland,
            Seed::BrandShell => ThemeSeed::BrandShell,
        },
        mode: match mode {
            Mode::Dark => ThemeMode::Dark,
            Mode::Light => ThemeMode::Light,
            Mode::HcDark => ThemeMode::HcDark,
            Mode::HcLight => ThemeMode::HcLight,
        },
        ..ListenerSettings::default()
    };

    let mut items = vec![local];
    items.extend(episodes.iter().cloned());

    let mut state = RedshankSurfaceState::default();
    state.seed = seed;
    state.mode = mode;
    state.compact = compact;
    state.items = items;
    state.feeds = vec![FeedRow {
        feed_url: FEED_URL.to_owned(),
        title: "Marsh Hours".to_owned(),
        subtitle: Some("Field recordings from the estuary".to_owned()),
        face: Face::Tag("MH".to_owned()),
        episode_count: 6,
        unplayed_count: 4,
        offline_bytes: 125_829_120,
        last_refreshed_ms: Some(1_789_000_000_000),
        failure: None,
    }];
    state.selected_feed = Some(FEED_URL.to_owned());
    state.queue = vec![item_id("marsh-1"), item_id("marsh-2"), item_id("marsh-3")];
    state.notes = notes;
    state.sessions = sessions;
    state.settings = settings;
    state.cache_used_bytes = 125_829_120;
    state
}

// ---------------------------------------------------------------------------
// Command application
// ---------------------------------------------------------------------------

/// A monotonic suffix for notes captured in the page. Single-threaded, but
/// the atomic keeps the type honest without a cell dance.
static NEXT_NOTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(100);

/// Say what the browser host cannot do, naming the command.
fn refuse(state: &mut RedshankSurfaceState, what: &str) {
    state.notice = Some(format!("Not available in the browser host yet: {what}"));
}

fn move_queue(queue: &mut Vec<ItemId>, from: usize, to: usize) {
    if from < queue.len() && to < queue.len() {
        let id = queue.remove(from);
        queue.insert(to, id);
    }
}

/// Apply one command, or refuse it.
fn apply(state: &mut RedshankSurfaceState, command: CompactCommand) {
    use CompactCommand as C;
    match command {
        // Navigation and presentation: pure state, all of it applied.
        C::SelectTab(tab) => state.active_tab = tab,
        C::SelectScene(scene) => state.scene = scene,
        C::SelectFeed(url) => state.selected_feed = url,
        C::SetLayout(layout) => state.layout = layout,
        C::SetNotesFilter(filter) => state.notes_filter = filter,
        C::ToggleMenu(_) | C::ToggleCluster(_) | C::SelectListenPane(_) => {
            state.apply_presentation(&command);
        },
        C::UpdateSettings(settings) => {
            state.seed = match settings.seed {
                ThemeSeed::Wetland => Seed::Wetland,
                ThemeSeed::BrandShell => Seed::BrandShell,
            };
            state.mode = match settings.mode {
                ThemeMode::Dark => Mode::Dark,
                ThemeMode::Light => Mode::Light,
                ThemeMode::HcDark => Mode::HcDark,
                ThemeMode::HcLight => Mode::HcLight,
            };
            state.compact.skip_backward_ms = settings.skip_backward_ms;
            state.compact.skip_forward_ms = settings.skip_forward_ms;
            state.settings = settings;
        },

        // The queue is a list of ids, which is a thing a browser can hold.
        C::Enqueue(id) => {
            if !state.queue.contains(&id) {
                state.queue.push(id);
            }
        },
        C::Dequeue(id) => state.queue.retain(|queued| queued != &id),
        C::MoveQueue { from, to } => move_queue(&mut state.queue, from, to),

        // Transport, without a clock. Position moves only where a command
        // moves it; nothing here pretends time is passing.
        C::Seek(offset) => seek_to(state, offset),
        C::SkipBackward(delta) => {
            let at = position(state).saturating_sub(delta);
            seek_to(state, at);
        },
        C::SkipForward(delta) => {
            let at = position(state) + delta;
            seek_to(state, at);
        },
        C::Play => {
            if state.compact.now_playing.is_some() {
                state.compact.transport = TransportState::Playing;
                state.notice = Some(
                    "No audio output in the browser host: the position does not advance."
                        .to_owned(),
                );
            }
        },
        C::Pause => {
            if state.compact.now_playing.is_some() {
                state.compact.transport = TransportState::Paused;
            }
        },
        C::Replay => {
            seek_to(state, 0);
            state.compact.transport = TransportState::Paused;
        },
        C::SetRate(percent) => {
            state.compact.rate_percent = percent;
            state.settings.playback_rate_percent = percent;
        },
        C::SetVolume(percent) => {
            state.compact.volume_percent = percent;
            state.settings.volume_percent = percent;
        },

        // Text capture round-trips entirely in memory.
        C::BeginTextNote | C::AddTextNote => {
            let at = position(state);
            let id = state
                .compact
                .now_playing
                .as_ref()
                .map(|playing| playing.item_id.clone())
                .unwrap_or_else(|| item_id(LOCAL_ID));
            state.text_capture = Some(TextCapture {
                anchor: anchor(&id, at),
                draft: String::new(),
                end_offset_ms: None,
            });
            state.set_text_draft("");
        },
        C::CancelTextNote => {
            state.text_capture = None;
            state.set_text_draft("");
        },
        C::SaveTextNote { anchor, plain_text } => save_note(state, anchor, None, plain_text),
        C::SaveSpanNote {
            anchor,
            end_offset_ms,
            plain_text,
        } => save_note(state, anchor, Some(end_offset_ms), plain_text),
        C::EditNote { id, plain_text } => {
            if let Some(existing) = state.notes.iter_mut().find(|note| note.id == id) {
                existing.body = NoteSummaryBody::Text(plain_text);
            }
            state.editing_note = None;
            state.text_capture = None;
            state.set_text_draft("");
        },
        C::BeginEditNote(id) => {
            if let Some(NoteSummaryBody::Text(text)) = state
                .notes
                .iter()
                .find(|note| note.id == id)
                .map(|note| note.body.clone())
            {
                state.set_text_draft(text);
            }
            state.editing_note = Some(id);
        },
        C::DeleteNote(id) => {
            state.notes.retain(|note| note.id != id);
            if let Some(playing) = state.compact.now_playing.as_mut() {
                playing.markers.retain(|marker| marker.id != id);
            }
            if state.editing_note.as_ref() == Some(&id) {
                state.editing_note = None;
            }
        },

        // Voice capture shows its row and saves nothing: there is no
        // microphone adapter in this stack for the browser yet.
        C::BeginVoiceNote => {
            let at = position(state);
            state.compact.voice_capture_active = true;
            state.compact.recording = Some(Recording {
                elapsed_ms: 3_200,
                anchor_ms: at,
                paused_for_capture: false,
            });
        },
        C::FinishVoiceNote => {
            state.compact.voice_capture_active = false;
            state.compact.recording = None;
            refuse(state, "FinishVoiceNote (no microphone or blob store)");
        },
        C::CancelVoiceNote => {
            state.compact.voice_capture_active = false;
            state.compact.recording = None;
        },

        // Selecting an item is a projection the browser can do from rows.
        C::SelectItem(id) => select_item(state, &id),
        C::OpenNote(id) | C::PlaySpan(id) => {
            if let Some(offset) = state
                .notes
                .iter()
                .find(|note| note.id == id)
                .map(|note| note.offset_ms)
            {
                seek_to(state, offset);
            }
        },

        // Everything below needs audio, files, network, or a store.
        C::OpenVoiceNote(_) | C::PlayVoiceNote(_) | C::StopVoiceNote(_) => {
            refuse(state, "voice note audition (no audio output)")
        },
        C::OpenLocalFile => refuse(state, "OpenLocalFile (no file picker)"),
        C::CacheItem(_) => refuse(state, "Download for offline listening (no cache)"),
        C::RemoveCachedItem(_) => refuse(state, "Remove offline download (no cache)"),
        C::RemoveLibraryItem(_) => refuse(state, "RemoveLibraryItem (no durable store)"),
        C::RetryItem(_) => refuse(state, "RetryItem (no network fetcher)"),
        // A pin is a row fact, so the page can hold it; nothing outlives the tab.
        C::PinItem(id) => {
            if let Some(row) = state.items.iter_mut().find(|row| row.id == id) {
                row.pinned = !row.pinned;
            }
        },
        C::Subscribe(_) => refuse(state, "Subscribe (no feed fetcher)"),
        C::RefreshSubscription(_) => refuse(state, "RefreshSubscription (no feed fetcher)"),
        C::ExportAnnotations => refuse(state, "ExportAnnotations (no file writer)"),
    }
}

fn position(state: &RedshankSurfaceState) -> u64 {
    state
        .compact
        .now_playing
        .as_ref()
        .map_or(0, |playing| playing.position_ms)
}

fn seek_to(state: &mut RedshankSurfaceState, offset_ms: u64) {
    let Some(playing) = state.compact.now_playing.as_mut() else {
        return;
    };
    let clamped = playing
        .duration_ms
        .map_or(offset_ms, |duration| offset_ms.min(duration));
    playing.position_ms = clamped;
    let id = playing.item_id.clone();
    if let Some(row) = state.items.iter_mut().find(|row| row.id == id) {
        row.position_ms = clamped;
    }
}

fn select_item(state: &mut RedshankSurfaceState, id: &ItemId) {
    let Some(row) = state.items.iter().find(|row| &row.id == id).cloned() else {
        return;
    };
    let markers = state
        .notes
        .iter()
        .filter(|note| &note.item_id == id)
        .map(NoteSummary::marker)
        .collect();
    state.compact.transport = match &row.unavailable {
        Some(message) => TransportState::Unavailable(message.clone()),
        None if row.completed => TransportState::Completed,
        None => TransportState::Paused,
    };
    state.compact.now_playing = Some(NowPlaying {
        item_id: row.id.clone(),
        title: row.title.clone(),
        feed_title: row.feed_title.clone(),
        face: row.face.clone(),
        source: row.source,
        position_ms: row.position_ms,
        duration_ms: row.duration_ms,
        resumed_from_ms: (row.position_ms > 0).then_some(row.position_ms),
        buffered_percent: match row.source {
            SourceKind::Cloud => 42,
            _ => 100,
        },
        markers,
    });
    state.seed_expanded_clusters();
}

fn save_note(
    state: &mut RedshankSurfaceState,
    anchor: TimedTarget,
    end_offset_ms: Option<u64>,
    plain_text: String,
) {
    let serial = NEXT_NOTE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let note = NoteSummary {
        id: AnnotationId(format!("web-note-{serial}")),
        item_id: anchor.item_id.clone(),
        offset_ms: anchor.offset_ms,
        end_offset_ms,
        body: NoteSummaryBody::Text(plain_text),
        private: matches!(
            state.settings.note_privacy,
            redshank_model::NotePrivacy::Private
        ),
    };
    if let Some(playing) = state.compact.now_playing.as_mut()
        && playing.item_id == note.item_id
    {
        playing.markers.push(note.marker());
        playing.markers.sort_by_key(|marker| marker.offset_ms);
    }
    if let Some(row) = state.items.iter_mut().find(|row| row.id == note.item_id) {
        row.note_count += 1;
    }
    state.notes.push(note);
    state.notes.sort_by_key(|note| note.offset_ms);
    state.text_capture = None;
    state.editing_note = None;
    state.set_text_draft("");
    state.notice = Some("Saved in this page only: the browser host has no store.".to_owned());
}

// ---------------------------------------------------------------------------
// Layout and page query
// ---------------------------------------------------------------------------

/// The same rule the desktop applies: family B only for a landscape slab
/// wide enough to carry a rail and narrow enough to need one.
fn layout_for(width: f32, height: f32) -> redshank_surfaces::Layout {
    if (700.0..900.0).contains(&width) && width > height * 1.15 {
        redshank_surfaces::Layout::Rail
    } else {
        redshank_surfaces::Layout::Dock
    }
}

/// `?seed=brand&mode=light` on the page URL, for opening straight into a
/// scope class without clicking through Settings.
fn seed_and_mode_from_query() -> (Seed, Mode) {
    let search = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    let mut seed = Seed::Wetland;
    let mut mode = Mode::Dark;
    for pair in search.trim_start_matches('?').split('&') {
        match pair.split_once('=') {
            Some(("seed", "brand" | "dark" | "brandshell")) => seed = Seed::BrandShell,
            Some(("seed", "wetland" | "redshank")) => seed = Seed::Wetland,
            Some(("mode", "light")) => mode = Mode::Light,
            Some(("mode", "dark")) => mode = Mode::Dark,
            Some(("mode", "hc-light" | "hclight")) => mode = Mode::HcLight,
            Some(("mode", "hc-dark" | "hcdark")) => mode = Mode::HcDark,
            _ => {},
        }
    }
    (seed, mode)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Mount Redshank onto a canvas.
///
/// Returns once the application is up; the listeners and frame callback it
/// installed keep it running, which is how a browser application lives with
/// nothing holding it on the stack.
#[wasm_bindgen]
pub async fn start(canvas_id: Option<String>) -> Result<(), JsValue> {
    // Without this a wasm panic is an unhelpful "unreachable executed".
    console_error_panic_hook::set_once();

    let id = canvas_id.unwrap_or_else(|| DEFAULT_CANVAS_ID.to_owned());
    let canvas = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&id))
        .ok_or_else(|| JsValue::from_str(&format!("no element with id {id:?}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str(&format!("element {id:?} is not a canvas")))?;

    let (seed, mode) = seed_and_mode_from_query();
    let mut state = fixture(seed, mode);
    state.seed_expanded_clusters();
    state.notice =
        Some("Browser host: in-memory fixture, no audio, files, network, or store.".to_owned());

    let options = HostOptions {
        title: "Redshank".into(),
        // A tab has no frame to draw, so the desktop's client-side
        // decorations have nothing to decorate here.
        decorations: true,
        ..Default::default()
    };

    mount(
        canvas,
        options,
        move |_window, _commands, _wake| Init {
            state,
            logic: surface as Logic,
            sheet: sheet(),
            // The sheet names both Plex families; the browser has neither.
            fonts: FONTS
                .iter()
                .map(|(family, bytes)| HostFont {
                    family: Some((*family).to_owned()),
                    bytes: bytes.to_vec(),
                })
                .collect(),
            images: Vec::new(),
        },
        HostHooks {
            frame: Box::new(|ctx: &mut Context<'_>| {
                let (width, height) = ctx.logical_size;
                let layout = layout_for(width, height);
                if ctx.runner.state().layout != layout {
                    ctx.runner.update(|state| state.layout = layout);
                }
                // Nothing here advances on its own, so no further frame is owed.
                false
            }),
            after_dispatch: Box::new(|ctx: &mut Context<'_>| {
                ctx.runner.update(|state| {
                    let commands: Vec<_> = state.drain_commands().collect();
                    for command in commands {
                        if !matches!(command, CompactCommand::ToggleMenu(_)) {
                            state.open_menu = None;
                        }
                        apply(state, command);
                    }
                });
            }),
            after_frame: Box::new(|_ctx| {}),
            after_wake: Box::new(|_ctx| {}),
            // Nothing closes a tab from inside the application.
            close_request: Box::new(|_ctx, _request| {
                cambium_rootstock::CloseDisposition::KeepVisible
            }),
            focused_text: Box::new(|_runner| None),
            key_intercept: Box::new(|_runner, _press| false),
        },
    )
    .await
    .map_err(|error| JsValue::from_str(&error))?;

    Ok(())
}
