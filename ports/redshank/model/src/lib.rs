#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AnnotationId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SavedTranscript {
    pub resource: FeedTranscript,
    pub final_url: String,
    pub retrieved_at_ms: u64,
    pub source: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeedResource {
    pub url: String,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeedTranscript {
    pub url: String,
    pub media_type: Option<String>,
    pub language: Option<String>,
    pub relation: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeedEpisodeFacts {
    pub published: Option<String>,
    pub summary: Option<String>,
    pub duration: Option<String>,
    pub artwork: Option<String>,
    pub enclosure_media_type: Option<String>,
    pub enclosure_byte_length: Option<u64>,
    pub chapters: Vec<FeedResource>,
    pub transcripts: Vec<FeedTranscript>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeedSubscription {
    pub feed_url: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub link: Option<String>,
    pub language: Option<String>,
    pub artwork: Option<String>,
    pub last_refreshed_ms: Option<u64>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCandidate {
    pub path: String,
    pub item_ids: Vec<ItemId>,
    pub byte_length: Option<u64>,
    pub last_used_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaSource {
    Local {
        path: String,
    },
    Enclosure {
        url: String,
    },
    Cached {
        path: String,
        origin_url: String,
        representation: Box<RepresentationReceipt>,
    },
    HostBlob {
        id: String,
    },
}

impl MediaSource {
    pub fn enclosure_url(&self) -> Option<&str> {
        match self {
            Self::Enclosure { url } => Some(url),
            Self::Cached { origin_url, .. } => Some(origin_url),
            Self::Local { .. } | Self::HostBlob { .. } => None,
        }
    }

    pub fn is_cached(&self) -> bool {
        matches!(self, Self::Cached { .. })
    }

    pub fn cached_representation(&self) -> Option<&RepresentationReceipt> {
        match self {
            Self::Cached { representation, .. } => Some(representation.as_ref()),
            Self::Local { .. } | Self::Enclosure { .. } | Self::HostBlob { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryItem {
    LocalAudio {
        id: ItemId,
        title: String,
        source: MediaSource,
    },
    DirectAudio {
        id: ItemId,
        title: String,
        source: MediaSource,
    },
    FeedEpisode {
        id: ItemId,
        feed_url: String,
        guid: String,
        title: String,
        source: MediaSource,
        #[serde(default)]
        facts: Box<FeedEpisodeFacts>,
    },
}

impl LibraryItem {
    pub fn id(&self) -> &ItemId {
        match self {
            Self::LocalAudio { id, .. }
            | Self::DirectAudio { id, .. }
            | Self::FeedEpisode { id, .. } => id,
        }
    }

    pub fn source(&self) -> &MediaSource {
        match self {
            Self::LocalAudio { source, .. }
            | Self::DirectAudio { source, .. }
            | Self::FeedEpisode { source, .. } => source,
        }
    }

    pub fn replace_source(&mut self, source: MediaSource) {
        match self {
            Self::LocalAudio {
                source: current, ..
            }
            | Self::DirectAudio {
                source: current, ..
            }
            | Self::FeedEpisode {
                source: current, ..
            } => *current = source,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            Self::LocalAudio { title, .. }
            | Self::DirectAudio { title, .. }
            | Self::FeedEpisode { title, .. } => title,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepresentationReceipt {
    pub requested_url: Option<String>,
    pub final_url: Option<String>,
    pub media_type: Option<String>,
    pub byte_length: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub retrieved_at_ms: Option<u64>,
    pub complete_digest: Option<String>,
}

/// Evidence about bytes, not about whether two recordings sound alike.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RepresentationIdentity {
    Same,
    Different,
    #[default]
    Unproven,
}

impl RepresentationReceipt {
    /// Full digests take precedence over server validators. Strong ETags are
    /// scoped to the final resource URL; different tags need not mean different
    /// bytes (RFC 9110, 8.8.1). Dates and equal lengths cannot establish identity.
    pub fn compare(&self, other: &Self) -> RepresentationIdentity {
        use RepresentationIdentity::{Different, Same, Unproven};
        if let (Some(a), Some(b)) = (&self.complete_digest, &other.complete_digest)
            && !a.is_empty()
            && !b.is_empty()
        {
            return if a == b { Same } else { Different };
        }
        if let (Some(a), Some(b)) = (self.byte_length, other.byte_length)
            && a != b
        {
            return Different;
        }
        if let (Some(a), Some(b)) = (&self.final_url, &other.final_url)
            && !a.is_empty()
            && a == b
            && let (Some(a), Some(b)) = (&self.etag, &other.etag)
            && strong_etag(a)
            && a == b
        {
            return Same;
        }
        Unproven
    }
}

fn strong_etag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    bytes.len() >= 2
        && bytes[0] == b'"'
        && bytes[bytes.len() - 1] == b'"'
        && bytes[1..bytes.len() - 1]
            .iter()
            .all(|b| *b == 0x21 || (0x23..=0x7e).contains(b) || *b >= 0x80)
}

/// Portable evidence for the version-one audio aligner. This data-only mirror
/// keeps the durable model independent of the DSP implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioFingerprint {
    pub version: u16,
    pub frame_ms: u16,
    /// Anchor relative to the first stored frame, including its subframe tail.
    pub anchor_offset_ms: u32,
    pub frames: Vec<[u8; 16]>,
}

impl AudioFingerprint {
    pub fn reference_duration_ms(&self) -> u64 {
        self.frames.len() as u64 * u64::from(self.frame_ms)
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.version != 1 || self.frame_ms != 100 || !(30..=600).contains(&self.frames.len()) {
            return Err(ModelError::InvalidAlignment(
                "Unsupported or incomplete audio fingerprint",
            ));
        }
        let covered = self.reference_duration_ms();
        let anchor = u64::from(self.anchor_offset_ms);
        if !(covered..covered + 100).contains(&anchor) {
            return Err(ModelError::InvalidAlignment(
                "Audio fingerprint does not reach the anchor",
            ));
        }
        Ok(())
    }
}

/// An estimated point on complete destination bytes, with the exact original
/// target that authorized its computation. Similarity is not a probability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedPosition {
    pub original_target: TimedTarget,
    pub destination: RepresentationReceipt,
    pub offset_ms: u64,
    pub algorithm_version: u32,
    pub confidence_per_mille: u16,
    pub runner_up_per_mille: u16,
    pub reference_duration_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimedTarget {
    pub item_id: ItemId,
    pub offset_ms: u64,
    /// `Some` for a span target; the annotation covers `offset_ms..end_offset_ms`.
    #[serde(default)]
    pub end_offset_ms: Option<u64>,
    /// The raw press offset before the reaction offset was subtracted.
    #[serde(default)]
    pub pressed_offset_ms: Option<u64>,
    pub representation: RepresentationReceipt,
    /// Source-time audio evidence frozen with the original target. Old notes
    /// remain readable and retain their original timestamps without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<AudioFingerprint>,
}

/// A host-frozen target used while capturing a text or voice note.
pub type CaptureAnchor = TimedTarget;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteBody {
    Text {
        plain_text: String,
    },
    Audio {
        blob_id: String,
        media_type: String,
        duration_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Annotation {
    pub id: AnnotationId,
    pub target: TimedTarget,
    pub body: NoteBody,
    pub created_at_ms: u64,
    #[serde(default)]
    pub privacy: NotePrivacy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePlaybackBehavior {
    Pause,
    Duck,
    Continue,
}

/// Whether a note is the listener's alone or may be shared on export.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotePrivacy {
    #[default]
    Private,
    Shareable,
}

/// How often subscriptions refresh themselves. The host owns the clock.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshSchedule {
    #[default]
    Manual,
    Hourly,
    Daily,
}

/// Where a completed item reopens: from the beginning, or from where the
/// listener stopped.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeCompleted {
    #[default]
    Start,
    Saved,
}

/// Which palette seed the surfaces derive their roles from.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSeed {
    #[default]
    Wetland,
    BrandShell,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    HcDark,
    HcLight,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ListenerSettings {
    pub capture_playback: CapturePlaybackBehavior,
    pub skip_forward_ms: u64,
    pub skip_backward_ms: u64,
    #[serde(default = "default_cache_budget_bytes")]
    pub cache_budget_bytes: u64,
    /// Requested playback rate in percent; the backend reports the effective one.
    #[serde(default = "default_playback_rate_percent")]
    pub playback_rate_percent: u16,
    #[serde(default = "default_volume_percent")]
    pub volume_percent: u8,
    /// Subtracted from a capture press so the anchor lands before the reaction.
    #[serde(default)]
    pub reaction_offset_ms: u64,
    #[serde(default = "default_resume_after_capture")]
    pub resume_after_capture: bool,
    /// While a capture ducks playback, the volume is scaled to this percent.
    #[serde(default = "default_duck_volume_percent")]
    pub duck_volume_percent: u8,
    #[serde(default)]
    pub note_privacy: NotePrivacy,
    #[serde(default)]
    pub refresh_schedule: RefreshSchedule,
    /// Where a completed item reopens when it is selected again.
    #[serde(default)]
    pub resume_completed_from: ResumeCompleted,
    #[serde(default)]
    pub auto_download: bool,
    #[serde(default)]
    pub auto_reclaim: bool,
    #[serde(default)]
    pub seed: ThemeSeed,
    #[serde(default)]
    pub mode: ThemeMode,
    /// Audio preceding a note retained as a fingerprint; zero disables capture.
    #[serde(default = "default_alignment_window_ms")]
    pub alignment_window_ms: u64,
    #[serde(default = "default_alignment_min_confidence_per_mille")]
    pub alignment_min_confidence_per_mille: u16,
    /// Maximum decoded source duration searched by an explicit realignment.
    #[serde(default = "default_alignment_max_search_ms")]
    pub alignment_max_search_ms: u64,
}

pub const fn default_alignment_window_ms() -> u64 {
    20_000
}

pub const fn default_alignment_min_confidence_per_mille() -> u16 {
    940
}

pub const fn default_alignment_max_search_ms() -> u64 {
    6 * 60 * 60 * 1_000
}

pub const fn default_cache_budget_bytes() -> u64 {
    2 * 1024 * 1024 * 1024
}

pub const fn default_playback_rate_percent() -> u16 {
    100
}

pub const fn default_volume_percent() -> u8 {
    80
}

pub const fn default_resume_after_capture() -> bool {
    true
}

pub const fn default_duck_volume_percent() -> u8 {
    10
}

impl Default for ListenerSettings {
    fn default() -> Self {
        Self {
            capture_playback: CapturePlaybackBehavior::Pause,
            skip_forward_ms: 30_000,
            skip_backward_ms: 15_000,
            cache_budget_bytes: default_cache_budget_bytes(),
            playback_rate_percent: default_playback_rate_percent(),
            volume_percent: default_volume_percent(),
            reaction_offset_ms: 0,
            resume_after_capture: default_resume_after_capture(),
            duck_volume_percent: default_duck_volume_percent(),
            note_privacy: NotePrivacy::default(),
            refresh_schedule: RefreshSchedule::default(),
            resume_completed_from: ResumeCompleted::default(),
            auto_download: false,
            auto_reclaim: false,
            seed: ThemeSeed::default(),
            mode: ThemeMode::default(),
            alignment_window_ms: default_alignment_window_ms(),
            alignment_min_confidence_per_mille: default_alignment_min_confidence_per_mille(),
            alignment_max_search_ms: default_alignment_max_search_ms(),
        }
    }
}

impl ListenerSettings {
    pub fn validate_alignment_settings(&self) -> Result<(), ModelError> {
        if self.alignment_window_ms != 0 && !(5_000..=60_000).contains(&self.alignment_window_ms) {
            return Err(ModelError::InvalidAlignment(
                "Fingerprint window must be disabled or 5 to 60 seconds",
            ));
        }
        if !(800..=990).contains(&self.alignment_min_confidence_per_mille) {
            return Err(ModelError::InvalidAlignment(
                "Minimum similarity must be 800 to 990 per mille",
            ));
        }
        if !(60_000..=86_400_000).contains(&self.alignment_max_search_ms) {
            return Err(ModelError::InvalidAlignment(
                "Alignment search must be 1 minute to 24 hours",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Progress {
    pub position_ms: u64,
    pub completed: bool,
    pub updated_at_ms: u64,
}

/// One stretch of listening, as the trail projection reads it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ListeningSession {
    pub item_id: ItemId,
    /// Source offsets the session covered.
    pub start_ms: u64,
    pub stop_ms: u64,
    /// Wall clock, host-supplied.
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub completed: bool,
}

/// The newest sessions the model keeps; older ones are dropped on record.
pub const MAX_LISTENING_SESSIONS: usize = 500;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RedshankModel {
    /// Explicitly saved transcript sources; retained offline in the model store.
    #[serde(default)]
    pub transcripts: BTreeMap<ItemId, SavedTranscript>,
    pub schema_version: u32,
    pub library: BTreeMap<ItemId, LibraryItem>,
    #[serde(default)]
    pub subscriptions: BTreeMap<String, FeedSubscription>,
    pub queue: Vec<ItemId>,
    #[serde(default)]
    pub selected_item: Option<ItemId>,
    pub progress: BTreeMap<ItemId, Progress>,
    pub annotations: BTreeMap<AnnotationId, Annotation>,
    /// Estimated point positions on a different complete recording. These
    /// never replace the original target or assert a remapped span end.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub derived_positions: BTreeMap<AnnotationId, DerivedPosition>,
    #[serde(default)]
    pub listening_sessions: Vec<ListeningSession>,
    /// Items the listener kept to hand; the Mere projections mark them.
    #[serde(default)]
    pub pinned: BTreeSet<ItemId>,
    pub settings: ListenerSettings,
}

impl Default for RedshankModel {
    fn default() -> Self {
        Self {
            transcripts: BTreeMap::new(),
            schema_version: 1,
            library: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            queue: Vec::new(),
            selected_item: None,
            progress: BTreeMap::new(),
            annotations: BTreeMap::new(),
            derived_positions: BTreeMap::new(),
            listening_sessions: Vec::new(),
            pinned: BTreeSet::new(),
            settings: ListenerSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    DuplicateItem(ItemId),
    MissingItem(ItemId),
    DuplicateAnnotation(AnnotationId),
    MissingAnnotation(AnnotationId),
    InvalidQueuePosition(usize),
    StaleAlignment(AnnotationId),
    InvalidAlignment(&'static str),
}

impl RedshankModel {
    pub fn add_item(&mut self, item: LibraryItem) -> Result<(), ModelError> {
        let id = item.id().clone();
        if self.library.contains_key(&id) {
            return Err(ModelError::DuplicateItem(id));
        }
        self.library.insert(id, item);
        Ok(())
    }

    pub fn upsert_subscription(&mut self, subscription: FeedSubscription) {
        self.subscriptions
            .insert(subscription.feed_url.clone(), subscription);
    }

    /// Merge refreshed feed metadata while retaining a completed local download
    /// when it still represents the same enclosure URL.
    pub fn upsert_feed_item(&mut self, mut item: LibraryItem) -> Result<bool, ModelError> {
        let LibraryItem::FeedEpisode { id, source, .. } = &item else {
            return Err(ModelError::MissingItem(item.id().clone()));
        };
        let id = id.clone();
        let incoming_url = source.enclosure_url().map(str::to_owned);
        let inserted = !self.library.contains_key(&id);
        if let Some(existing) = self.library.get(&id)
            && existing.source().is_cached()
            && existing.source().enclosure_url() == incoming_url.as_deref()
        {
            item.replace_source(existing.source().clone());
        }
        self.library.insert(id, item);
        Ok(inserted)
    }

    /// Unique cached objects from least to most recently used. Shared objects
    /// take the newest use of any referring item, then paths break ties.
    pub fn cache_eviction_order(&self) -> Vec<CacheCandidate> {
        let mut candidates = BTreeMap::<String, CacheCandidate>::new();
        for (id, item) in &self.library {
            let MediaSource::Cached {
                path,
                representation,
                ..
            } = item.source()
            else {
                continue;
            };
            let used = self
                .progress
                .get(id)
                .map(|progress| progress.updated_at_ms)
                .or(representation.retrieved_at_ms)
                .unwrap_or(0);
            let candidate = candidates.entry(path.clone()).or_insert(CacheCandidate {
                path: path.clone(),
                item_ids: Vec::new(),
                byte_length: representation.byte_length,
                last_used_ms: used,
            });
            candidate.item_ids.push(id.clone());
            candidate.last_used_ms = candidate.last_used_ms.max(used);
            candidate.byte_length = candidate.byte_length.or(representation.byte_length);
        }
        let mut candidates: Vec<_> = candidates.into_values().collect();
        candidates.sort_by(|left, right| {
            (left.last_used_ms, &left.path).cmp(&(right.last_used_ms, &right.path))
        });
        candidates
    }

    pub fn enqueue(&mut self, id: &ItemId) -> Result<(), ModelError> {
        self.require_item(id)?;
        if !self.queue.contains(id) {
            self.queue.push(id.clone());
        }
        Ok(())
    }

    pub fn dequeue(&mut self, id: &ItemId) -> Result<(), ModelError> {
        let Some(index) = self.queue.iter().position(|candidate| candidate == id) else {
            return Err(ModelError::MissingItem(id.clone()));
        };
        self.queue.remove(index);
        Ok(())
    }

    pub fn reorder_queue(&mut self, from: usize, to: usize) -> Result<(), ModelError> {
        if from >= self.queue.len() || to >= self.queue.len() {
            return Err(ModelError::InvalidQueuePosition(from.max(to)));
        }
        let item = self.queue.remove(from);
        self.queue.insert(to, item);
        Ok(())
    }

    pub fn remove_item(&mut self, id: &ItemId) -> Result<LibraryItem, ModelError> {
        let item = self
            .library
            .remove(id)
            .ok_or_else(|| ModelError::MissingItem(id.clone()))?;
        self.queue.retain(|queued| queued != id);
        if self.selected_item.as_ref() == Some(id) {
            self.selected_item = None;
        }
        self.progress.remove(id);
        self.pinned.remove(id);
        self.transcripts.remove(id);
        self.annotations
            .retain(|_, note| note.target.item_id != *id);
        self.derived_positions
            .retain(|id, _| self.annotations.contains_key(id));
        Ok(item)
    }

    pub fn is_pinned(&self, id: &ItemId) -> bool {
        self.pinned.contains(id)
    }

    /// Pin or unpin one item; returns its new pinned state.
    pub fn toggle_pin(&mut self, id: &ItemId) -> Result<bool, ModelError> {
        self.require_item(id)?;
        if self.pinned.remove(id) {
            return Ok(false);
        }
        self.pinned.insert(id.clone());
        Ok(true)
    }

    pub fn set_progress(&mut self, id: &ItemId, progress: Progress) -> Result<(), ModelError> {
        self.require_item(id)?;
        self.progress.insert(id.clone(), progress);
        Ok(())
    }

    pub fn add_annotation(&mut self, annotation: Annotation) -> Result<(), ModelError> {
        self.require_item(&annotation.target.item_id)?;
        let id = annotation.id.clone();
        if self.annotations.contains_key(&id) {
            return Err(ModelError::DuplicateAnnotation(id));
        }
        self.annotations.insert(id, annotation);
        Ok(())
    }

    pub fn add_text_annotation(
        &mut self,
        id: AnnotationId,
        anchor: CaptureAnchor,
        plain_text: String,
        created_at_ms: u64,
    ) -> Result<(), ModelError> {
        self.add_annotation(Annotation {
            id,
            target: anchor,
            body: NoteBody::Text { plain_text },
            created_at_ms,
            privacy: self.settings.note_privacy,
        })
    }

    /// A text note covering `anchor.offset_ms..end_offset_ms` of one item.
    pub fn add_span_annotation(
        &mut self,
        id: AnnotationId,
        anchor: CaptureAnchor,
        end_offset_ms: u64,
        plain_text: String,
        created_at_ms: u64,
    ) -> Result<(), ModelError> {
        let mut target = anchor;
        target.end_offset_ms = Some(end_offset_ms.max(target.offset_ms));
        self.add_annotation(Annotation {
            id,
            target,
            body: NoteBody::Text { plain_text },
            created_at_ms,
            privacy: self.settings.note_privacy,
        })
    }

    /// Append a listening session, keeping only the newest
    /// [`MAX_LISTENING_SESSIONS`].
    pub fn record_listening_session(&mut self, session: ListeningSession) {
        self.listening_sessions.push(session);
        let overflow = self
            .listening_sessions
            .len()
            .saturating_sub(MAX_LISTENING_SESSIONS);
        if overflow > 0 {
            self.listening_sessions.drain(..overflow);
        }
    }

    /// The IRI an exported annotation targets. The model's source is always
    /// an IRI (a `SpecificResource`'s `source` must be one); a local path
    /// becomes a `file:` URL, and anything with no URL of its own is named
    /// by its library identity under the port's own scheme.
    fn annotation_source(&self, note: &Annotation) -> String {
        if let Some(url) = note.target.representation.final_url.as_deref().or(note
            .target
            .representation
            .requested_url
            .as_deref())
        {
            return url.to_owned();
        }
        let item = &note.target.item_id;
        match self.library.get(item).map(LibraryItem::source) {
            Some(MediaSource::Local { path }) => {
                local_file_iri(path).unwrap_or_else(|| item_iri(item))
            },
            Some(MediaSource::Enclosure { url }) => url.clone(),
            Some(MediaSource::Cached {
                origin_url,
                representation,
                ..
            }) => representation
                .final_url
                .clone()
                .unwrap_or_else(|| origin_url.clone()),
            Some(MediaSource::HostBlob { id }) => format!("blob:{id}"),
            None => item_iri(item),
        }
    }

    /// One item's shareable annotations as a W3C Web Annotation page. Pure:
    /// no I/O. Private notes are the listener's alone and stay out; a note
    /// is made shareable per note before it leaves.
    pub fn export_annotations(&self, item: &ItemId) -> serde_json::Value {
        let items: Vec<_> = self
            .annotations_for_item(item)
            .into_iter()
            .filter(|note| note.privacy == NotePrivacy::Shareable)
            .map(|note| self.export_annotation(note))
            .collect();
        serde_json::json!({
            "@context": "http://www.w3.org/ns/anno.jsonld",
            "type": "AnnotationPage",
            "items": items,
        })
    }

    /// The same export as pretty JSON text, for hosts without serde_json.
    pub fn export_annotations_json(&self, item: &ItemId) -> String {
        serde_json::to_string_pretty(&self.export_annotations(item))
            .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"))
    }

    fn export_annotation(&self, note: &Annotation) -> serde_json::Value {
        let body = match &note.body {
            NoteBody::Text { plain_text } => serde_json::json!({
                "type": "TextualBody",
                "value": plain_text,
                "format": "text/plain",
            }),
            NoteBody::Audio {
                blob_id,
                media_type,
                duration_ms,
            } => serde_json::json!({
                "type": "Audio",
                "id": blob_id,
                "format": media_type,
                "duration": format!("PT{:.3}S", *duration_ms as f64 / 1000.0),
            }),
        };
        serde_json::json!({
            "@context": "http://www.w3.org/ns/anno.jsonld",
            "type": "Annotation",
            "id": note_iri(&note.id),
            "created": rfc3339_utc(note.created_at_ms),
            "motivation": "commenting",
            "body": body,
            "target": {
                "source": self.annotation_source(note),
                "selector": {
                    "type": "FragmentSelector",
                    "conformsTo": "http://www.w3.org/TR/media-frags/",
                    "value": fragment_value(note.target.offset_ms, note.target.end_offset_ms),
                },
            },
        })
    }

    /// Keep a note to the listener, or let the export carry it.
    pub fn set_annotation_privacy(
        &mut self,
        id: &AnnotationId,
        privacy: NotePrivacy,
    ) -> Result<(), ModelError> {
        self.annotations
            .get_mut(id)
            .map(|annotation| annotation.privacy = privacy)
            .ok_or_else(|| ModelError::MissingAnnotation(id.clone()))
    }

    pub fn update_text_annotation(
        &mut self,
        id: &AnnotationId,
        plain_text: String,
    ) -> Result<(), ModelError> {
        let annotation = self
            .annotations
            .get_mut(id)
            .ok_or_else(|| ModelError::MissingAnnotation(id.clone()))?;
        match &mut annotation.body {
            NoteBody::Text { plain_text: text } => *text = plain_text,
            NoteBody::Audio { .. } => return Err(ModelError::MissingAnnotation(id.clone())),
        }
        Ok(())
    }

    /// Admit a completed alignment only while its frozen source target still
    /// exists unchanged. The host separately verifies that the destination is
    /// still its downloaded current copy when the asynchronous reply arrives.
    pub fn store_derived_position(
        &mut self,
        id: &AnnotationId,
        position: DerivedPosition,
    ) -> Result<(), ModelError> {
        self.validate_derived_position(id, &position)?;
        self.derived_positions.insert(id.clone(), position);
        Ok(())
    }

    /// Recheck durable evidence before using a saved position. Deserialization
    /// does not pass through the producer's admission method, and the listener
    /// may have raised the similarity threshold since the result was saved.
    pub fn validate_derived_position(
        &self,
        id: &AnnotationId,
        position: &DerivedPosition,
    ) -> Result<(), ModelError> {
        let note = self
            .annotations
            .get(id)
            .ok_or_else(|| ModelError::MissingAnnotation(id.clone()))?;
        if note.target != position.original_target {
            return Err(ModelError::StaleAlignment(id.clone()));
        }
        self.settings.validate_alignment_settings()?;
        let fingerprint = note
            .target
            .fingerprint
            .as_ref()
            .ok_or(ModelError::InvalidAlignment(
                "The original note has no audio fingerprint",
            ))?;
        fingerprint.validate()?;
        let digest = position
            .destination
            .complete_digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("blake3:"));
        if !digest.is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(ModelError::InvalidAlignment(
                "A complete destination BLAKE3 digest is required",
            ));
        }
        if position.algorithm_version != 1
            || position.reference_duration_ms != fingerprint.reference_duration_ms()
            || u64::from(fingerprint.anchor_offset_ms) > note.target.offset_ms
            || position.offset_ms < u64::from(fingerprint.anchor_offset_ms)
            || position.offset_ms > self.settings.alignment_max_search_ms
        {
            return Err(ModelError::InvalidAlignment(
                "Invalid alignment version, position, or reference interval",
            ));
        }
        if position.confidence_per_mille > 1_000
            || position.runner_up_per_mille > 1_000
            || position.confidence_per_mille < self.settings.alignment_min_confidence_per_mille
            || position
                .confidence_per_mille
                .saturating_sub(position.runner_up_per_mille)
                < 40
        {
            return Err(ModelError::InvalidAlignment(
                "Alignment similarity is insufficient or ambiguous",
            ));
        }
        Ok(())
    }

    pub fn annotations_for_item(&self, id: &ItemId) -> Vec<&Annotation> {
        let mut notes: Vec<_> = self
            .annotations
            .values()
            .filter(|note| note.target.item_id == *id)
            .collect();
        notes.sort_by_key(|note| (&note.target.offset_ms, &note.id));
        notes
    }

    pub fn delete_annotation(&mut self, id: &AnnotationId) -> Result<(), ModelError> {
        self.annotations
            .remove(id)
            .ok_or_else(|| ModelError::MissingAnnotation(id.clone()))?;
        self.derived_positions.remove(id);
        Ok(())
    }

    fn require_item(&self, id: &ItemId) -> Result<(), ModelError> {
        self.library
            .contains_key(id)
            .then_some(())
            .ok_or_else(|| ModelError::MissingItem(id.clone()))
    }
}

/// A media-fragment temporal value, `t=start` or `t=start,end`, in seconds.
/// A note's IRI on export: the same name Turnstone gives its graph node.
pub fn note_iri(id: &AnnotationId) -> String {
    format!("mere://redshank/note/{}", id.0)
}

/// A library item's IRI, for a source that has no URL of its own.
pub fn item_iri(id: &ItemId) -> String {
    format!("mere://redshank/item/{}", id.0)
}

/// An absolute local path as a `file:` URL. A relative path has no URL, and
/// a host with no file system (the browser) has none for any path.
fn local_file_iri(path: &str) -> Option<String> {
    #[cfg(any(unix, windows))]
    {
        url::Url::from_file_path(path).ok().map(String::from)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        None
    }
}

/// Milliseconds since the Unix epoch as an RFC 3339 UTC date-time, which is
/// the `xsd:dateTime` the Web Annotation model wants for `created`. Pure
/// proleptic-Gregorian arithmetic (Howard Hinnant's civil-from-days), so the
/// model stays free of a calendar dependency.
pub fn rfc3339_utc(ms: u64) -> String {
    let seconds = ms / 1_000;
    let millis = ms % 1_000;
    let days = (seconds / 86_400) as i64;
    let of_day = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60
    )
}

fn fragment_value(start_ms: u64, end_ms: Option<u64>) -> String {
    let seconds = |milliseconds: u64| format!("{:.3}", milliseconds as f64 / 1000.0);
    match end_ms {
        Some(end) => format!("t={},{}", seconds(start_ms), seconds(end)),
        None => format!("t={}", seconds(start_ms)),
    }
}

pub trait AudioCaptureHost {
    type Error;

    fn begin_capture(&mut self) -> Result<(), Self::Error>;
    fn finish_capture(&mut self) -> Result<NoteBody, Self::Error>;
    fn cancel_capture(&mut self) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_item(id: &str) -> LibraryItem {
        LibraryItem::LocalAudio {
            id: ItemId(id.into()),
            title: format!("Local {id}"),
            source: MediaSource::Local {
                path: format!("{id}.mp3"),
            },
        }
    }

    fn feed_item(id: &str, url: &str) -> LibraryItem {
        LibraryItem::FeedEpisode {
            id: ItemId(id.into()),
            feed_url: "https://example.test/feed.xml".into(),
            guid: id.into(),
            title: format!("Episode {id}"),
            source: MediaSource::Enclosure { url: url.into() },
            facts: Box::default(),
        }
    }

    #[test]
    fn feed_refresh_preserves_matching_cached_representation() {
        let mut model = RedshankModel::default();
        let id = ItemId("episode".into());
        let url = "https://example.test/episode.mp3";
        let mut original = feed_item(&id.0, url);
        original.replace_source(MediaSource::Cached {
            path: "cache/episode.audio".into(),
            origin_url: url.into(),
            representation: Box::new(RepresentationReceipt {
                complete_digest: Some("blake3:complete".into()),
                ..Default::default()
            }),
        });
        assert!(model.upsert_feed_item(original).unwrap());
        let mut refreshed = feed_item(&id.0, url);
        if let LibraryItem::FeedEpisode { title, .. } = &mut refreshed {
            *title = "Updated title".into();
        }
        assert!(!model.upsert_feed_item(refreshed).unwrap());
        let current = &model.library[&id];
        assert_eq!(current.title(), "Updated title");
        assert!(current.source().is_cached());
        assert_eq!(
            current
                .source()
                .cached_representation()
                .and_then(|receipt| receipt.complete_digest.as_deref()),
            Some("blake3:complete")
        );
    }

    #[test]
    fn cache_eviction_order_deduplicates_objects_and_uses_newest_reference() {
        let mut model = RedshankModel::default();
        for (id, path, updated) in [
            ("old", "cache/a.audio", 10),
            ("shared-a", "cache/shared.audio", 20),
            ("shared-b", "cache/shared.audio", 40),
            ("new", "cache/z.audio", 30),
        ] {
            let mut item = feed_item(id, &format!("https://example.test/{id}.mp3"));
            item.replace_source(MediaSource::Cached {
                path: path.into(),
                origin_url: format!("https://example.test/{id}.mp3"),
                representation: Box::new(RepresentationReceipt {
                    byte_length: Some(100),
                    retrieved_at_ms: Some(1),
                    ..Default::default()
                }),
            });
            let item_id = item.id().clone();
            model.add_item(item).unwrap();
            model
                .set_progress(
                    &item_id,
                    Progress {
                        position_ms: 1,
                        completed: false,
                        updated_at_ms: updated,
                    },
                )
                .unwrap();
        }
        let candidates = model.cache_eviction_order();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.path.as_str(), candidate.last_used_ms))
                .collect::<Vec<_>>(),
            [
                ("cache/a.audio", 10),
                ("cache/z.audio", 30),
                ("cache/shared.audio", 40),
            ]
        );
        assert_eq!(candidates[2].item_ids.len(), 2);
    }

    #[test]
    fn queue_and_progress_mutations_are_deterministic() {
        let mut model = RedshankModel::default();
        let first = ItemId("first".into());
        let second = ItemId("second".into());
        model.add_item(local_item("first")).unwrap();
        model.add_item(local_item("second")).unwrap();
        model.enqueue(&first).unwrap();
        model.enqueue(&first).unwrap();
        model.enqueue(&second).unwrap();
        assert_eq!(model.queue, [first.clone(), second]);

        model
            .set_progress(
                &first,
                Progress {
                    position_ms: 42_000,
                    completed: false,
                    updated_at_ms: 100,
                },
            )
            .unwrap();
        assert_eq!(model.progress[&first].position_ms, 42_000);
        model.dequeue(&first).unwrap();
        assert_eq!(model.queue, [ItemId("second".into())]);
    }

    #[test]
    fn annotations_require_a_library_target() {
        let mut model = RedshankModel::default();
        let annotation = Annotation {
            id: AnnotationId("note".into()),
            target: TimedTarget {
                item_id: ItemId("missing".into()),
                offset_ms: 12_000,
                end_offset_ms: None,
                pressed_offset_ms: None,
                fingerprint: None,
                representation: RepresentationReceipt::default(),
            },
            body: NoteBody::Text {
                plain_text: "remember this".into(),
            },
            created_at_ms: 10,
            privacy: NotePrivacy::Private,
        };
        assert_eq!(
            model.add_annotation(annotation),
            Err(ModelError::MissingItem(ItemId("missing".into())))
        );
    }

    #[test]
    fn queue_reorder_and_item_removal_preserve_invariants() {
        let mut model = RedshankModel::default();
        for id in ["a", "b", "c"] {
            model.add_item(local_item(id)).unwrap();
            model.enqueue(&ItemId(id.into())).unwrap();
        }
        model.reorder_queue(0, 2).unwrap();
        assert_eq!(
            model.queue,
            [ItemId("b".into()), ItemId("c".into()), ItemId("a".into())]
        );
        model.remove_item(&ItemId("c".into())).unwrap();
        assert_eq!(model.queue, [ItemId("b".into()), ItemId("a".into())]);
        assert!(!model.library.contains_key(&ItemId("c".into())));
    }

    #[test]
    fn selected_item_is_optional_and_clears_when_removed() {
        let mut model = RedshankModel::default();
        let id = ItemId("a".into());
        model.add_item(local_item("a")).unwrap();
        model.selected_item = Some(id.clone());
        model.remove_item(&id).unwrap();
        assert_eq!(model.selected_item, None);
    }

    /// A verbatim document written by the 2026-09-13 schema, before this
    /// change added settings, targets, privacy, and listening sessions.
    const STORED_2026_09_13: &str = r#"{
      "schema_version": 1,
      "library": {
        "local:a.mp3": {
          "kind": "local_audio",
          "id": "local:a.mp3",
          "title": "A",
          "source": { "kind": "local", "path": "a.mp3" }
        }
      },
      "subscriptions": {},
      "queue": ["local:a.mp3"],
      "selected_item": "local:a.mp3",
      "progress": {
        "local:a.mp3": { "position_ms": 42000, "completed": false, "updated_at_ms": 7 }
      },
      "annotations": {
        "note:1": {
          "id": "note:1",
          "target": {
            "item_id": "local:a.mp3",
            "offset_ms": 12000,
            "representation": {
              "requested_url": null,
              "final_url": null,
              "media_type": null,
              "byte_length": 9644,
              "etag": null,
              "last_modified": null,
              "retrieved_at_ms": null,
              "complete_digest": "blake3:fixed"
            }
          },
          "body": { "kind": "text", "plain_text": "remember this" },
          "created_at_ms": 11
        }
      },
      "settings": {
        "capture_playback": "pause",
        "skip_forward_ms": 30000,
        "skip_backward_ms": 15000,
        "cache_budget_bytes": 2147483648
      }
    }"#;

    #[test]
    fn a_stored_2026_09_13_document_loads_unchanged() {
        let model: RedshankModel = serde_json::from_str(STORED_2026_09_13).unwrap();
        assert_eq!(model.queue, [ItemId("local:a.mp3".into())]);
        assert_eq!(
            model.progress[&ItemId("local:a.mp3".into())].position_ms,
            42_000
        );
        assert!(model.listening_sessions.is_empty());
        let note = &model.annotations[&AnnotationId("note:1".into())];
        assert_eq!(note.target.offset_ms, 12_000);
        assert_eq!(note.target.end_offset_ms, None);
        assert_eq!(note.target.pressed_offset_ms, None);
        assert_eq!(note.privacy, NotePrivacy::Private);
        assert_eq!(model.settings, ListenerSettings::default());
    }

    #[test]
    fn span_annotations_round_trip_through_json() {
        let mut model = RedshankModel::default();
        model.add_item(local_item("a")).unwrap();
        model.settings.note_privacy = NotePrivacy::Shareable;
        model
            .add_span_annotation(
                AnnotationId("span".into()),
                CaptureAnchor {
                    item_id: ItemId("a".into()),
                    offset_ms: 1_000,
                    end_offset_ms: None,
                    pressed_offset_ms: Some(1_400),
                    fingerprint: None,
                    representation: RepresentationReceipt::default(),
                },
                4_500,
                "a span".into(),
                9,
            )
            .unwrap();
        let encoded = serde_json::to_string(&model).unwrap();
        let restored: RedshankModel = serde_json::from_str(&encoded).unwrap();
        let note = &restored.annotations[&AnnotationId("span".into())];
        assert_eq!(note.target.end_offset_ms, Some(4_500));
        assert_eq!(note.target.pressed_offset_ms, Some(1_400));
        assert_eq!(note.privacy, NotePrivacy::Shareable);
    }

    #[test]
    fn listening_sessions_keep_only_the_newest_five_hundred() {
        let mut model = RedshankModel::default();
        for index in 0..MAX_LISTENING_SESSIONS as u64 + 10 {
            model.record_listening_session(ListeningSession {
                item_id: ItemId("a".into()),
                start_ms: index,
                stop_ms: index + 1,
                started_at_ms: index,
                ended_at_ms: index + 1,
                completed: false,
            });
        }
        assert_eq!(model.listening_sessions.len(), MAX_LISTENING_SESSIONS);
        assert_eq!(model.listening_sessions[0].start_ms, 10);
    }

    #[test]
    fn exported_annotations_are_web_annotations_with_media_fragments() {
        let mut model = RedshankModel::default();
        let id = ItemId("a".into());
        model
            .add_item(LibraryItem::DirectAudio {
                id: id.clone(),
                title: "A".into(),
                source: MediaSource::Enclosure {
                    url: "https://example.test/a.mp3".into(),
                },
            })
            .unwrap();
        let anchor = CaptureAnchor {
            item_id: id.clone(),
            offset_ms: 12_500,
            end_offset_ms: None,
            pressed_offset_ms: None,
            fingerprint: None,
            representation: RepresentationReceipt::default(),
        };
        model
            .add_text_annotation(AnnotationId("n1".into()), anchor.clone(), "point".into(), 1)
            .unwrap();
        model
            .add_span_annotation(
                AnnotationId("n2".into()),
                CaptureAnchor {
                    offset_ms: 30_000,
                    ..anchor
                },
                45_250,
                "span".into(),
                2,
            )
            .unwrap();
        // New notes take the settings' privacy, Private by default, and a
        // private note is the listener's alone: it is not exported.
        assert!(
            model.export_annotations(&id)["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        for note in ["n1", "n2"] {
            model
                .set_annotation_privacy(&AnnotationId(note.into()), NotePrivacy::Shareable)
                .unwrap();
        }
        let export = model.export_annotations(&id);
        assert_eq!(export["@context"], "http://www.w3.org/ns/anno.jsonld");
        assert_eq!(export["type"], "AnnotationPage");
        let items = export["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "Annotation");
        assert_eq!(items[0]["id"], "mere://redshank/note/n1");
        assert_eq!(items[0]["created"], "1970-01-01T00:00:00.001Z");
        assert!(items[0].get("audience").is_none());
        assert_eq!(items[0]["body"]["type"], "TextualBody");
        assert_eq!(items[0]["body"]["value"], "point");
        assert_eq!(items[0]["target"]["source"], "https://example.test/a.mp3");
        assert_eq!(items[0]["target"]["selector"]["type"], "FragmentSelector");
        assert_eq!(items[0]["target"]["selector"]["value"], "t=12.500");
        assert_eq!(items[1]["target"]["selector"]["value"], "t=30.000,45.250");
    }

    #[test]
    fn created_is_an_rfc3339_utc_date_time() {
        // Values cross-checked against Python's datetime.fromtimestamp(utc).
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339_utc(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339_utc(1_758_900_000_000), "2025-09-26T15:20:00.000Z");
        assert_eq!(rfc3339_utc(1_735_689_599_999), "2024-12-31T23:59:59.999Z");
        assert_eq!(rfc3339_utc(4_102_444_800_000), "2100-01-01T00:00:00.000Z");
    }

    #[test]
    fn a_local_path_exports_as_a_file_url_and_a_relative_one_as_the_item_iri() {
        let mut model = RedshankModel::default();
        model.settings.note_privacy = NotePrivacy::Shareable;
        let absolute = if cfg!(windows) {
            r"C:\audio\talk one.mp3"
        } else {
            "/audio/talk one.mp3"
        };
        model
            .add_item(LibraryItem::LocalAudio {
                id: ItemId("abs".into()),
                title: "Absolute".into(),
                source: MediaSource::Local {
                    path: absolute.into(),
                },
            })
            .unwrap();
        model.add_item(local_item("rel")).unwrap();
        for item in ["abs", "rel"] {
            let anchor = CaptureAnchor {
                item_id: ItemId(item.into()),
                offset_ms: 1,
                end_offset_ms: None,
                pressed_offset_ms: None,
                fingerprint: None,
                representation: RepresentationReceipt::default(),
            };
            model
                .add_text_annotation(AnnotationId(item.into()), anchor, item.into(), 1)
                .unwrap();
        }
        let source = |item: &str| {
            model.export_annotations(&ItemId(item.into()))["items"][0]["target"]["source"].clone()
        };
        let expected_absolute = if cfg!(windows) {
            "file:///C:/audio/talk%20one.mp3"
        } else {
            "file:///audio/talk%20one.mp3"
        };
        assert_eq!(source("abs"), expected_absolute);
        assert_eq!(source("rel"), "mere://redshank/item/rel");
    }

    #[test]
    fn exported_voice_bodies_carry_their_blob_identity() {
        let mut model = RedshankModel::default();
        let id = ItemId("a".into());
        model.add_item(local_item("a")).unwrap();
        model
            .add_annotation(Annotation {
                id: AnnotationId("voice".into()),
                target: CaptureAnchor {
                    item_id: id.clone(),
                    offset_ms: 500,
                    end_offset_ms: None,
                    pressed_offset_ms: None,
                    fingerprint: None,
                    representation: RepresentationReceipt::default(),
                },
                body: NoteBody::Audio {
                    blob_id: "voice:abc".into(),
                    media_type: "audio/wav".into(),
                    duration_ms: 1_500,
                },
                created_at_ms: 3,
                privacy: NotePrivacy::Shareable,
            })
            .unwrap();
        let export = model.export_annotations(&id);
        let body = &export["items"][0]["body"];
        assert_eq!(body["id"], "voice:abc");
        assert_eq!(body["format"], "audio/wav");
        assert_eq!(body["duration"], "PT1.500S");
        assert_eq!(
            export["items"][0]["target"]["source"],
            "mere://redshank/item/a"
        );
    }

    #[test]
    fn text_edit_keeps_the_original_capture_anchor() {
        let mut model = RedshankModel::default();
        let item = ItemId("a".into());
        model.add_item(local_item("a")).unwrap();
        let anchor = CaptureAnchor {
            item_id: item,
            offset_ms: 12_345,
            end_offset_ms: None,
            pressed_offset_ms: None,
            fingerprint: None,
            representation: RepresentationReceipt {
                complete_digest: Some("blake3:fixed".into()),
                ..RepresentationReceipt::default()
            },
        };
        model
            .add_text_annotation(AnnotationId("n".into()), anchor.clone(), "first".into(), 1)
            .unwrap();
        model
            .update_text_annotation(&AnnotationId("n".into()), "edited".into())
            .unwrap();
        let note = model.annotations.get(&AnnotationId("n".into())).unwrap();
        assert_eq!(note.target, anchor);
        assert_eq!(
            note.body,
            NoteBody::Text {
                plain_text: "edited".into()
            }
        );
    }

    #[test]
    fn pinning_toggles_survives_a_round_trip_and_ends_with_the_item() {
        let mut model = RedshankModel::default();
        let id = ItemId("a".into());
        model.add_item(local_item("a")).unwrap();
        assert!(model.toggle_pin(&id).unwrap());
        assert!(model.is_pinned(&id));
        let stored = serde_json::to_string(&model).unwrap();
        let loaded: RedshankModel = serde_json::from_str(&stored).unwrap();
        assert!(loaded.is_pinned(&id));
        assert!(!model.toggle_pin(&id).unwrap());
        assert!(model.toggle_pin(&id).unwrap());
        model.remove_item(&id).unwrap();
        assert!(model.pinned.is_empty());
        assert!(model.toggle_pin(&id).is_err());
    }

    /// A store written before this pass has neither key; both default.
    #[test]
    fn a_store_without_pins_or_resume_policy_still_loads() {
        let stored = r#"{"schema_version":1,"library":{},"queue":[],"progress":{},
            "annotations":{},"settings":{"capture_playback":"pause",
            "skip_forward_ms":30000,"skip_backward_ms":15000}}"#;
        let model: RedshankModel = serde_json::from_str(stored).unwrap();
        assert!(model.pinned.is_empty());
        assert_eq!(model.settings.resume_completed_from, ResumeCompleted::Start);
    }
}
