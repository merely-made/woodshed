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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePlaybackBehavior {
    Pause,
    Duck,
    Continue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ListenerSettings {
    pub capture_playback: CapturePlaybackBehavior,
    pub skip_forward_ms: u64,
    pub skip_backward_ms: u64,
    #[serde(default = "default_cache_budget_bytes")]
    pub cache_budget_bytes: u64,
}

pub const fn default_cache_budget_bytes() -> u64 {
    2 * 1024 * 1024 * 1024
}

impl Default for ListenerSettings {
    fn default() -> Self {
        Self {
            capture_playback: CapturePlaybackBehavior::Pause,
            skip_forward_ms: 30_000,
            skip_backward_ms: 15_000,
            cache_budget_bytes: default_cache_budget_bytes(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Progress {
    pub position_ms: u64,
    pub completed: bool,
    pub updated_at_ms: u64,
}

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
                representation: RepresentationReceipt::default(),
            },
            body: NoteBody::Text {
                plain_text: "remember this".into(),
            },
            created_at_ms: 10,
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

    #[test]
    fn text_edit_keeps_the_original_capture_anchor() {
        let mut model = RedshankModel::default();
        let item = ItemId("a".into());
        model.add_item(local_item("a")).unwrap();
        let anchor = CaptureAnchor {
            item_id: item,
            offset_ms: 12_345,
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
