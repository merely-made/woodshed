#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AnnotationId(pub String);

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
    #[serde(default)]
    pub note_privacy: NotePrivacy,
    #[serde(default)]
    pub refresh_schedule: RefreshSchedule,
    #[serde(default)]
    pub auto_download: bool,
    #[serde(default)]
    pub auto_reclaim: bool,
    #[serde(default)]
    pub seed: ThemeSeed,
    #[serde(default)]
    pub mode: ThemeMode,
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
            note_privacy: NotePrivacy::default(),
            refresh_schedule: RefreshSchedule::default(),
            auto_download: false,
            auto_reclaim: false,
            seed: ThemeSeed::default(),
            mode: ThemeMode::default(),
        }
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
    pub schema_version: u32,
    pub library: BTreeMap<ItemId, LibraryItem>,
    #[serde(default)]
    pub subscriptions: BTreeMap<String, FeedSubscription>,
    pub queue: Vec<ItemId>,
    #[serde(default)]
    pub selected_item: Option<ItemId>,
    pub progress: BTreeMap<ItemId, Progress>,
    pub annotations: BTreeMap<AnnotationId, Annotation>,
    #[serde(default)]
    pub listening_sessions: Vec<ListeningSession>,
    pub settings: ListenerSettings,
}

impl Default for RedshankModel {
    fn default() -> Self {
        Self {
            schema_version: 1,
            library: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            queue: Vec::new(),
            selected_item: None,
            progress: BTreeMap::new(),
            annotations: BTreeMap::new(),
            listening_sessions: Vec::new(),
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
        self.annotations
            .retain(|_, note| note.target.item_id != *id);
        Ok(item)
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

    /// The source URL or path an exported annotation targets.
    fn annotation_source(&self, note: &Annotation) -> String {
        if let Some(url) = note.target.representation.final_url.as_deref().or(note
            .target
            .representation
            .requested_url
            .as_deref())
        {
            return url.to_owned();
        }
        match self
            .library
            .get(&note.target.item_id)
            .map(LibraryItem::source)
        {
            Some(MediaSource::Local { path }) => path.clone(),
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
            None => note.target.item_id.0.clone(),
        }
    }

    /// One item's annotations as a W3C Web Annotation page. Pure: no I/O.
    pub fn export_annotations(&self, item: &ItemId) -> serde_json::Value {
        let items: Vec<_> = self
            .annotations_for_item(item)
            .into_iter()
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
            "id": note.id.0,
            "created": note.created_at_ms,
            "motivation": "commenting",
            "audience": match note.privacy {
                NotePrivacy::Private => "private",
                NotePrivacy::Shareable => "shareable",
            },
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
            .map(|_| ())
            .ok_or_else(|| ModelError::MissingAnnotation(id.clone()))
    }

    fn require_item(&self, id: &ItemId) -> Result<(), ModelError> {
        self.library
            .contains_key(id)
            .then_some(())
            .ok_or_else(|| ModelError::MissingItem(id.clone()))
    }
}

/// A media-fragment temporal value, `t=start` or `t=start,end`, in seconds.
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
        let export = model.export_annotations(&id);
        assert_eq!(export["@context"], "http://www.w3.org/ns/anno.jsonld");
        assert_eq!(export["type"], "AnnotationPage");
        let items = export["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "Annotation");
        assert_eq!(items[0]["body"]["type"], "TextualBody");
        assert_eq!(items[0]["body"]["value"], "point");
        assert_eq!(items[0]["target"]["source"], "https://example.test/a.mp3");
        assert_eq!(items[0]["target"]["selector"]["type"], "FragmentSelector");
        assert_eq!(items[0]["target"]["selector"]["value"], "t=12.500");
        assert_eq!(items[1]["target"]["selector"]["value"], "t=30.000,45.250");
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
                    representation: RepresentationReceipt::default(),
                },
                body: NoteBody::Audio {
                    blob_id: "voice:abc".into(),
                    media_type: "audio/wav".into(),
                    duration_ms: 1_500,
                },
                created_at_ms: 3,
                privacy: NotePrivacy::Private,
            })
            .unwrap();
        let export = model.export_annotations(&id);
        let body = &export["items"][0]["body"];
        assert_eq!(body["id"], "voice:abc");
        assert_eq!(body["format"], "audio/wav");
        assert_eq!(body["duration"], "PT1.500S");
        assert_eq!(export["items"][0]["target"]["source"], "a.mp3");
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
}
