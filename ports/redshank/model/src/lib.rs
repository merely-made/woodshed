#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AnnotationId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaSource {
    Local { path: String },
    Enclosure { url: String },
    HostBlob { id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryItem {
    LocalAudio {
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
    },
}

impl LibraryItem {
    pub fn id(&self) -> &ItemId {
        match self {
            Self::LocalAudio { id, .. } | Self::FeedEpisode { id, .. } => id,
        }
    }

    pub fn source(&self) -> &MediaSource {
        match self {
            Self::LocalAudio { source, .. } | Self::FeedEpisode { source, .. } => source,
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
}

impl Default for ListenerSettings {
    fn default() -> Self {
        Self {
            capture_playback: CapturePlaybackBehavior::Pause,
            skip_forward_ms: 30_000,
            skip_backward_ms: 15_000,
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
    pub queue: Vec<ItemId>,
    pub progress: BTreeMap<ItemId, Progress>,
    pub annotations: BTreeMap<AnnotationId, Annotation>,
    pub settings: ListenerSettings,
}

impl Default for RedshankModel {
    fn default() -> Self {
        Self {
            schema_version: 1,
            library: BTreeMap::new(),
            queue: Vec::new(),
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

pub trait PlaybackHost {
    type Error;

    fn load(&mut self, source: &MediaSource) -> Result<(), Self::Error>;
    fn play(&mut self) -> Result<(), Self::Error>;
    fn pause(&mut self) -> Result<(), Self::Error>;
    fn seek(&mut self, position_ms: u64) -> Result<(), Self::Error>;
    fn snapshot_position_ms(&mut self) -> Result<u64, Self::Error>;
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
}
