#![forbid(unsafe_code)]

//! Reusable Redshank listening surfaces.
//!
//! These views own presentation and interaction only. A host drains
//! [`CompactCommand`] values and applies them through Redshank's player,
//! library, and annotation adapters. The compact Player/Capture surface
//! deliberately has no library, storage, network, or audio-device authority.
//!
//! Module map (see `design_docs/2026-09-13_redshank_gui_implementation_plan.md`):
//!
//! - [`theme`]: tinct-derived seeds, tokens, and the one stylesheet;
//! - [`shell`]: header, destinations, layout switch, work area, phone bar;
//! - [`dock`]: the family A bottom dock (identity, seek track, transport, capture);
//! - [`rail`]: the family B left transport rail, same facts and commands;
//! - [`tabs`]: Listen, Library, Notes, Settings bodies;
//! - [`scene`]: the feed-node projections (chain, orrery, trail) for the Mere tab.

pub mod dock;
pub mod rail;
pub mod scene;
pub mod shell;
pub mod tabs;
pub mod theme;

use cambium::{AnyView, GenetCtx, GenetElement, TextInput};
use redshank_model::{
    AnnotationId, CaptureAnchor, FeedSubscription, ItemId, LibraryItem, ListenerSettings,
};

pub use theme::{FONTS, sheet};

pub type CompactView = Box<dyn AnyView<CompactPlayerState, (), GenetCtx, GenetElement>>;
pub type FullView = Box<dyn AnyView<RedshankSurfaceState, (), GenetCtx, GenetElement>>;

/// Re-exported so hosts that mount the compact surface alone keep one import.
pub use dock::{capture_surface, compact_surface, player_surface};

// ---------------------------------------------------------------------------
// Transport and now-playing facts
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportState {
    Empty,
    Paused,
    Playing,
    Buffering,
    /// The selected item played to its end; the primary action is Replay.
    Completed,
    Unavailable(String),
}

impl TransportState {
    pub fn spoken_status(&self) -> &str {
        match self {
            Self::Empty => "Nothing loaded",
            Self::Paused => "Paused",
            Self::Playing => "Playing",
            Self::Buffering => "Buffering",
            Self::Completed => "Completed",
            Self::Unavailable(message) => message,
        }
    }

    /// The tracked microlabel word the dock prints beside the source.
    pub fn micro_word(&self) -> &str {
        match self {
            Self::Empty => "IDLE",
            Self::Paused => "PAUSED",
            Self::Playing => "PLAYING",
            Self::Buffering => "BUFFERING",
            Self::Completed => "COMPLETED",
            Self::Unavailable(_) => "UNAVAILABLE",
        }
    }
}

/// Where an item's bytes come from, for the LOCAL / CLOUD / OFFLINE badges.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceKind {
    #[default]
    Local,
    Cloud,
    Offline,
    HostBlob,
}

impl SourceKind {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Local => "LOCAL",
            Self::Cloud => "CLOUD",
            Self::Offline => "OFFLINE",
            Self::HostBlob => "HOST",
        }
    }

    pub fn from_item(item: &LibraryItem) -> Self {
        match item.source() {
            redshank_model::MediaSource::Local { .. } => Self::Local,
            redshank_model::MediaSource::Enclosure { .. } => Self::Cloud,
            redshank_model::MediaSource::Cached { .. } => Self::Offline,
            redshank_model::MediaSource::HostBlob { .. } => Self::HostBlob,
        }
    }
}

/// What an item or feed shows as its square face.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Face {
    /// Artwork by URL; hosts that cannot load it fall back to `Tag`.
    Artwork(String),
    /// A short format or count tag such as `m4a` or `3`.
    Tag(String),
}

impl Default for Face {
    fn default() -> Self {
        Self::Tag(String::new())
    }
}

/// A note marker on a seek track or timeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteMarker {
    pub id: AnnotationId,
    pub offset_ms: u64,
    /// `Some` for a span annotation.
    pub end_offset_ms: Option<u64>,
    pub kind: NoteKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoteKind {
    Text,
    Voice,
}

impl NoteKind {
    pub fn micro_word(self) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Voice => "VOICE",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NowPlaying {
    pub item_id: ItemId,
    pub title: String,
    /// Show title for a feed episode; `None` for local and direct audio.
    pub feed_title: Option<String>,
    pub face: Face,
    pub source: SourceKind,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    /// Saved progress the host resumed from, when it did.
    pub resumed_from_ms: Option<u64>,
    /// Percent of the source available for seeking (`100` for local files).
    pub buffered_percent: u8,
    pub markers: Vec<NoteMarker>,
}

/// An active voice recording, as the dock presents it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recording {
    pub elapsed_ms: u64,
    /// The frozen anchor in source time.
    pub anchor_ms: u64,
    /// Whether playback paused for the capture (the microlabel says so).
    pub paused_for_capture: bool,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceTab {
    #[default]
    Listen,
    Library,
    Notes,
    Mere,
    Settings,
}

impl SurfaceTab {
    pub const ALL: [Self; 5] = [
        Self::Listen,
        Self::Library,
        Self::Notes,
        Self::Mere,
        Self::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Listen => "Listen",
            Self::Library => "Library",
            Self::Notes => "Notes",
            Self::Mere => "Mere",
            Self::Settings => "Settings",
        }
    }

    pub fn panel_id(self) -> &'static str {
        match self {
            Self::Listen => "redshank-listen-panel",
            Self::Library => "redshank-library-panel",
            Self::Notes => "redshank-notes-panel",
            Self::Mere => "redshank-mere-panel",
            Self::Settings => "redshank-settings-panel",
        }
    }
}

/// Which dock family the host selected from width and aspect.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layout {
    /// Family A: bottom dock, phone bar under it below 600px.
    #[default]
    Dock,
    /// Family B: left transport rail; tabs at the top of the work area.
    Rail,
}

/// Which projection of the feed-node structure a scene shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scene {
    #[default]
    Chain,
    Orrery,
    Trail,
}

impl Scene {
    pub const ALL: [Self; 3] = [Self::Chain, Self::Orrery, Self::Trail];

    pub fn label(self) -> &'static str {
        match self {
            Self::Chain => "chain",
            Self::Orrery => "orrery",
            Self::Trail => "trail",
        }
    }
}

/// Which notes the Notes tab lists.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotesFilter {
    #[default]
    ThisEpisode,
    AllNotes,
}

/// Which section the phone-width Listen panel shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ListenPane {
    #[default]
    Queue,
    Notes,
}

/// The one row whose overflow menu is open. One at a time, by construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuTarget {
    Queue(ItemId),
    Note(AnnotationId),
    Feed(String),
    Episode(ItemId),
}

/// What the Notes representation card can honestly say about an item's bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepresentationSummary {
    pub retrieved_at_ms: Option<u64>,
    /// The first eight hex characters of the complete digest.
    pub short_digest: Option<String>,
    /// Whether the notes' frozen digests still equal the item's; `None` when
    /// one side never recorded one.
    pub matches: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactCommand {
    Play,
    Pause,
    /// Restart a completed item from zero.
    Replay,
    Seek(u64),
    SkipBackward(u64),
    SkipForward(u64),
    /// Playback rate in percent (`100` is real time).
    SetRate(u16),
    /// Output volume in percent.
    SetVolume(u8),
    AddTextNote,
    BeginVoiceNote,
    FinishVoiceNote,
    CancelVoiceNote,
    OpenVoiceNote(AnnotationId),
    PlayVoiceNote(AnnotationId),
    StopVoiceNote(AnnotationId),
    BeginTextNote,
    CancelTextNote,
    SaveTextNote {
        anchor: CaptureAnchor,
        plain_text: String,
    },
    SaveSpanNote {
        anchor: CaptureAnchor,
        end_offset_ms: u64,
        plain_text: String,
    },
    /// Seek to a note's anchor and play (Open at source).
    OpenNote(AnnotationId),
    /// Play a span annotation from its start to its end.
    PlaySpan(AnnotationId),
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
    /// Retry loading an unavailable or failed item.
    RetryItem(ItemId),
    /// Pin an item into the Mere projection.
    PinItem(ItemId),
    Subscribe(String),
    RefreshSubscription(String),
    /// Choose which feed the Library episode list and the scenes show.
    SelectFeed(Option<String>),
    SelectTab(SurfaceTab),
    SelectScene(Scene),
    SetLayout(Layout),
    SetNotesFilter(NotesFilter),
    /// Open one row's overflow menu, or close the open one with `None`.
    ToggleMenu(Option<MenuTarget>),
    /// Open or shut one note cluster, keyed by its first offset bucket.
    ToggleCluster(u64),
    /// Which Listen section the phone shows.
    SelectListenPane(ListenPane),
    /// Serialize the current item's annotations as W3C Web Annotation JSON.
    ExportAnnotations,
    UpdateSettings(ListenerSettings),
}

// ---------------------------------------------------------------------------
// Compact state (Player + Capture only)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactPlayerState {
    pub transport: TransportState,
    pub now_playing: Option<NowPlaying>,
    pub skip_backward_ms: u64,
    pub skip_forward_ms: u64,
    /// Effective playback rate in percent, as the backend reports it.
    pub rate_percent: u16,
    pub volume_percent: u8,
    pub voice_capture_available: bool,
    pub voice_capture_active: bool,
    pub recording: Option<Recording>,
    commands: Vec<CompactCommand>,
}

impl Default for CompactPlayerState {
    fn default() -> Self {
        Self {
            transport: TransportState::Empty,
            now_playing: None,
            skip_backward_ms: 15_000,
            skip_forward_ms: 30_000,
            rate_percent: 100,
            volume_percent: 80,
            voice_capture_available: false,
            voice_capture_active: false,
            recording: None,
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

    /// Whether transport commands are accepted right now.
    pub fn transport_enabled(&self) -> bool {
        self.now_playing.is_some()
            && !matches!(
                self.transport,
                TransportState::Empty | TransportState::Buffering | TransportState::Unavailable(_)
            )
    }
}

// ---------------------------------------------------------------------------
// Full-surface rows
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCapture {
    /// This anchor is supplied by the host at BeginTextNote time.
    pub anchor: CaptureAnchor,
    pub draft: String,
    /// `Some` while the draft is a span ending here.
    pub end_offset_ms: Option<u64>,
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
    pub item_id: ItemId,
    pub offset_ms: u64,
    pub end_offset_ms: Option<u64>,
    pub body: NoteSummaryBody,
    pub private: bool,
}

impl NoteSummary {
    pub fn kind(&self) -> NoteKind {
        match self.body {
            NoteSummaryBody::Text(_) => NoteKind::Text,
            NoteSummaryBody::Voice { .. } => NoteKind::Voice,
        }
    }

    pub fn marker(&self) -> NoteMarker {
        NoteMarker {
            id: self.id.clone(),
            offset_ms: self.offset_ms,
            end_offset_ms: self.end_offset_ms,
            kind: self.kind(),
        }
    }
}

/// One library item as the queue, library, and scenes present it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemRow {
    pub id: ItemId,
    pub title: String,
    pub feed_url: Option<String>,
    pub feed_title: Option<String>,
    pub face: Face,
    pub source: SourceKind,
    pub duration_ms: Option<u64>,
    pub position_ms: u64,
    pub completed: bool,
    /// Publication date label from feed facts, when known.
    pub published: Option<String>,
    pub cached_bytes: Option<u64>,
    pub note_count: usize,
    /// The host could not load it and offers a retry.
    pub unavailable: Option<String>,
    /// The listener kept this one to hand.
    pub pinned: bool,
    /// The cached object's receipt, when the item has one.
    pub representation: Option<RepresentationSummary>,
}

impl ItemRow {
    /// Listened fraction in `0.0..=1.0`, `1.0` when completed.
    pub fn listened(&self) -> f32 {
        if self.completed {
            return 1.0;
        }
        match self.duration_ms {
            Some(duration) if duration > 0 => {
                (self.position_ms as f32 / duration as f32).clamp(0.0, 1.0)
            },
            _ => 0.0,
        }
    }
}

/// One subscription as the roster and scenes present it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedRow {
    pub feed_url: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub face: Face,
    pub episode_count: usize,
    pub unplayed_count: usize,
    pub offline_bytes: u64,
    pub last_refreshed_ms: Option<u64>,
    /// The last refresh failed with this message.
    pub failure: Option<String>,
}

/// One listening session for the trail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListeningSessionRow {
    pub item_id: ItemId,
    pub title: String,
    pub start_ms: u64,
    pub stop_ms: u64,
    pub duration_ms: Option<u64>,
    /// `TODAY`, `YESTERDAY`, or a short date, supplied by the host clock.
    pub day_label: String,
    pub wall_clock: String,
    pub note_offsets_ms: Vec<u64>,
    pub completed: bool,
    pub active: bool,
}

/// Which mode and seed the sheet is scoped to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Seed {
    #[default]
    Wetland,
    BrandShell,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Dark,
    Light,
    HcDark,
    HcLight,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct RedshankSurfaceState {
    pub active_tab: SurfaceTab,
    pub layout: Layout,
    pub scene: Scene,
    pub notes_filter: NotesFilter,
    pub seed: Seed,
    pub mode: Mode,
    pub compact: CompactPlayerState,
    /// Every library item, projected. Kept for hosts that still need it.
    pub library: Vec<LibraryItem>,
    pub items: Vec<ItemRow>,
    pub subscriptions: Vec<FeedSubscription>,
    pub feeds: Vec<FeedRow>,
    /// The feed whose episodes the Library list and scenes show.
    pub selected_feed: Option<String>,
    pub queue: Vec<ItemId>,
    /// Notes for the selected item (or all items under `NotesFilter::AllNotes`).
    pub notes: Vec<NoteSummary>,
    pub sessions: Vec<ListeningSessionRow>,
    pub settings: ListenerSettings,
    /// Bytes the offline cache holds now, for the Storage readout.
    pub cache_used_bytes: u64,
    pub microphone_label: Option<String>,
    pub text_capture: Option<TextCapture>,
    pub text_editor: TextInput,
    pub feed_url_editor: TextInput,
    pub notice: Option<String>,
    pub editing_note: Option<AnnotationId>,
    /// The one row whose overflow menu is open.
    pub open_menu: Option<MenuTarget>,
    /// Note clusters the listener has open, by first-offset bucket. Hosts seed
    /// the playhead's cluster on selection; after that this list is the truth.
    pub expanded_clusters: Vec<u64>,
    /// Which Listen section the phone shows; both show at wide width.
    pub listen_pane: ListenPane,
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

    pub fn item(&self, id: &ItemId) -> Option<&ItemRow> {
        self.items.iter().find(|item| &item.id == id)
    }

    pub fn selected_item(&self) -> Option<&ItemRow> {
        let id = &self.compact.now_playing.as_ref()?.item_id;
        self.item(id)
    }

    pub fn feed(&self, url: &str) -> Option<&FeedRow> {
        self.feeds.iter().find(|feed| feed.feed_url == url)
    }

    /// The feed to show: the selected one, else the playing item's, else the first.
    pub fn current_feed(&self) -> Option<&FeedRow> {
        if let Some(url) = &self.selected_feed {
            return self.feed(url);
        }
        if let Some(url) = self
            .selected_item()
            .and_then(|item| item.feed_url.as_deref())
        {
            return self.feed(url);
        }
        self.feeds.first()
    }

    /// Notes on the item the dock holds, oldest first.
    fn selected_notes(&self) -> Vec<&NoteSummary> {
        let Some(id) = self.compact.now_playing.as_ref().map(|now| &now.item_id) else {
            return Vec::new();
        };
        self.notes
            .iter()
            .filter(|note| &note.item_id == id)
            .collect()
    }

    /// The cluster holding the playhead: the one hosts open on selection.
    pub fn playhead_cluster(&self) -> Option<u64> {
        let position = self.compact.now_playing.as_ref()?.position_ms;
        let groups = note_clusters(&self.selected_notes());
        groups
            .iter()
            .rev()
            .find(|group| group[0].offset_ms <= position)
            .or_else(|| groups.first())
            .map(|group| cluster_key(group))
    }

    /// Seed the default: the playhead's cluster open, every other one shut.
    pub fn seed_expanded_clusters(&mut self) {
        self.expanded_clusters = self.playhead_cluster().into_iter().collect();
    }

    pub fn cluster_expanded(&self, key: u64) -> bool {
        self.expanded_clusters.contains(&key)
    }

    pub fn menu_is_open(&self, target: &MenuTarget) -> bool {
        self.open_menu.as_ref() == Some(target)
    }

    /// Apply the presentation-only commands both hosts share. Returns whether
    /// the command was one of them, so a host can fall through to its own.
    pub fn apply_presentation(&mut self, command: &CompactCommand) -> bool {
        match command {
            CompactCommand::ToggleMenu(target) => self.open_menu = target.clone(),
            CompactCommand::ToggleCluster(key) => {
                match self.expanded_clusters.iter().position(|open| open == key) {
                    Some(at) => {
                        self.expanded_clusters.remove(at);
                    },
                    None => self.expanded_clusters.push(*key),
                }
            },
            CompactCommand::SelectListenPane(pane) => self.listen_pane = *pane,
            _ => return false,
        }
        true
    }

    /// Scope class for the root element.
    pub fn scope_class(&self) -> &'static str {
        match (self.seed, self.mode) {
            (Seed::Wetland, Mode::Dark) => "t-redshank",
            (Seed::Wetland, Mode::Light) => "t-redshank-light",
            (Seed::Wetland, Mode::HcDark) => "t-redshank-hc-dark",
            (Seed::Wetland, Mode::HcLight) => "t-redshank-hc-light",
            (Seed::BrandShell, Mode::Dark) => "t-dark",
            (Seed::BrandShell, Mode::Light) => "t-light",
            (Seed::BrandShell, Mode::HcDark) => "t-hc-dark",
            (Seed::BrandShell, Mode::HcLight) => "t-hc-light",
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Notes within this many milliseconds of a cluster's first note join it.
pub const CLUSTER_MS: u64 = 10_000;

/// One cluster's key: the offset its first note sits at.
pub fn cluster_key(group: &[&NoteSummary]) -> u64 {
    group.first().map_or(0, |note| note.offset_ms)
}

/// Point notes in ascending order, grouped into clusters. Span notes carry
/// their own card, so they stay out.
pub fn note_clusters<'a>(notes: &[&'a NoteSummary]) -> Vec<Vec<&'a NoteSummary>> {
    let mut points: Vec<&NoteSummary> = notes
        .iter()
        .copied()
        .filter(|note| note.end_offset_ms.is_none())
        .collect();
    points.sort_by_key(|note| note.offset_ms);
    let mut out: Vec<Vec<&NoteSummary>> = Vec::new();
    for note in points {
        match out.last_mut() {
            Some(group) if note.offset_ms.saturating_sub(group[0].offset_ms) <= CLUSTER_MS => {
                group.push(note)
            },
            _ => out.push(vec![note]),
        }
    }
    out
}

/// `m:ss` or `h:mm:ss`.
pub fn format_time(milliseconds: u64) -> String {
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

/// `1h 42m`, `41m`, or `0m` for summaries.
pub fn format_span(milliseconds: u64) -> String {
    let minutes = milliseconds / 60_000;
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

/// `340 MiB` style byte labels.
pub fn format_bytes(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= 1024 * MIB {
        format!("{:.1} GiB", bytes as f64 / (1024 * MIB) as f64)
    } else {
        format!("{} MiB", bytes / MIB)
    }
}

/// `Sep 12` from epoch milliseconds, for the representation card. Howard
/// Hinnant's civil-from-days, the only date arithmetic these surfaces do.
pub fn format_date(at_ms: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let z = (at_ms / 86_400_000) as i64 + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as usize;
    format!("{} {day}", MONTHS[(month - 1).min(11)])
}

/// The first eight hex characters of a `blake3:...` digest.
pub fn short_digest(digest: &str) -> Option<String> {
    let hex = digest.rsplit(':').next()?;
    (hex.len() >= 8).then(|| hex[..8].to_owned())
}

/// Position as a percentage of duration, for track geometry.
pub fn percent(position_ms: u64, duration_ms: Option<u64>) -> f32 {
    match duration_ms {
        Some(duration) if duration > 0 => {
            (position_ms as f32 / duration as f32 * 100.0).clamp(0.0, 100.0)
        },
        _ => 0.0,
    }
}

/// The complete reusable listener surface. It projects host-provided state and
/// emits commands; files, audio, clocks, and persistence remain host-owned.
pub fn surface(state: &RedshankSurfaceState) -> FullView {
    shell::surface(state)
}
