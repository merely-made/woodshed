#![forbid(unsafe_code)]

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(not(windows))]
use std::fs::File;

use redshank_model::RedshankModel;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Encode(serde_json::Error),
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Encode(error)
    }
}

pub trait ModelStore {
    fn load(&self) -> Result<Option<RedshankModel>, StoreError>;
    fn save(&self, model: &RedshankModel) -> Result<(), StoreError>;
}

/// A crash-tolerant local store made from immutable numbered generations.
/// Pending or corrupt newer files are ignored, leaving the latest valid prior
/// generation readable.
pub struct JsonDirectoryStore {
    root: PathBuf,
    #[cfg(test)]
    fail_before_publish: bool,
}

impl JsonDirectoryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            #[cfg(test)]
            fail_before_publish: false,
        }
    }

    fn generations(&self) -> Result<Vec<(u64, PathBuf)>, StoreError> {
        let mut generations = Vec::new();
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(generations),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(number) = generation_number(name, ".json") else {
                continue;
            };
            generations.push((number, path));
        }
        generations.sort_unstable_by_key(|(number, _)| *number);
        Ok(generations)
    }

    fn next_generation(&self) -> Result<u64, StoreError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(1),
            Err(error) => return Err(error.into()),
        };
        let mut latest = 0;
        for entry in entries {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let number = generation_number(&name, ".json")
                .or_else(|| generation_number(&name, ".json.pending"));
            latest = latest.max(number.unwrap_or(0));
        }
        Ok(latest.saturating_add(1))
    }

    fn generation_path(&self, generation: u64) -> PathBuf {
        self.root.join(format!("state-{generation:020}.json"))
    }
}

fn generation_number(name: &str, suffix: &str) -> Option<u64> {
    name.strip_prefix("state-")?
        .strip_suffix(suffix)?
        .parse()
        .ok()
}

impl ModelStore for JsonDirectoryStore {
    fn load(&self) -> Result<Option<RedshankModel>, StoreError> {
        for (_, path) in self.generations()?.into_iter().rev() {
            let bytes = match fs::read(path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if let Ok(model) = serde_json::from_slice(&bytes) {
                return Ok(Some(model));
            }
        }
        Ok(None)
    }

    fn save(&self, model: &RedshankModel) -> Result<(), StoreError> {
        fs::create_dir_all(&self.root)?;
        let generation = self.next_generation()?;
        let final_path = self.generation_path(generation);
        let pending_path = pending_path(&final_path);
        let encoded = serde_json::to_vec_pretty(model)?;
        let mut pending = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&pending_path)?;
        pending.write_all(&encoded)?;
        pending.sync_all()?;
        drop(pending);

        #[cfg(test)]
        if self.fail_before_publish {
            return Err(StoreError::Io(io::Error::other(
                "injected pre-publish failure",
            )));
        }

        fs::rename(&pending_path, &final_path)?;
        sync_directory(&self.root)?;
        Ok(())
    }
}

fn pending_path(final_path: &Path) -> PathBuf {
    let mut pending = final_path.as_os_str().to_owned();
    pending.push(".pending");
    PathBuf::from(pending)
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> Result<(), StoreError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

// Windows does not permit opening a directory with `File::open`. The pending
// file is still flushed before its same-directory rename, and immutable older
// generations remain available if publication is interrupted.
#[cfg(windows)]
fn sync_directory(_path: &Path) -> Result<(), StoreError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use redshank_model::{
        Annotation, AnnotationId, ItemId, LibraryItem, MediaSource, NoteBody, Progress,
        RepresentationReceipt, TimedTarget,
    };
    use tempfile::tempdir;

    use super::*;

    fn populated_model() -> RedshankModel {
        let mut model = RedshankModel::default();
        let local = ItemId("local".into());
        let direct = ItemId("direct".into());
        let episode = ItemId("episode".into());
        model
            .add_item(LibraryItem::LocalAudio {
                id: local.clone(),
                title: "Field recording".into(),
                source: MediaSource::Local {
                    path: "field.flac".into(),
                },
            })
            .unwrap();
        model
            .add_item(LibraryItem::DirectAudio {
                id: direct.clone(),
                title: "Direct recording".into(),
                source: MediaSource::Enclosure {
                    url: "https://cdn.example.test/direct.mp3".into(),
                },
            })
            .unwrap();
        model
            .add_item(LibraryItem::FeedEpisode {
                id: episode.clone(),
                feed_url: "https://example.test/feed.xml".into(),
                guid: "episode-7".into(),
                title: "Episode seven".into(),
                source: MediaSource::Enclosure {
                    url: "https://cdn.example.test/7.mp3".into(),
                },
            })
            .unwrap();
        model.enqueue(&episode).unwrap();
        model.selected_item = Some(episode.clone());
        model
            .set_progress(
                &episode,
                Progress {
                    position_ms: 91_000,
                    completed: false,
                    updated_at_ms: 500,
                },
            )
            .unwrap();
        let receipt = RepresentationReceipt {
            requested_url: Some("https://cdn.example.test/7.mp3".into()),
            final_url: Some("https://edge.example.test/7-a.mp3".into()),
            media_type: Some("audio/mpeg".into()),
            byte_length: Some(123_456),
            etag: Some("episode-7-a".into()),
            retrieved_at_ms: Some(450),
            complete_digest: Some("blake3:abc".into()),
            ..RepresentationReceipt::default()
        };
        model
            .library
            .get_mut(&direct)
            .unwrap()
            .replace_source(MediaSource::Cached {
                path: "cache/abc.audio".into(),
                origin_url: "https://cdn.example.test/direct.mp3".into(),
                representation: Box::new(receipt.clone()),
            });
        for (id, body) in [
            (
                "text-note",
                NoteBody::Text {
                    plain_text: "follow this argument".into(),
                },
            ),
            (
                "audio-note",
                NoteBody::Audio {
                    blob_id: "voice-1".into(),
                    media_type: "audio/wav".into(),
                    duration_ms: 2_400,
                },
            ),
        ] {
            model
                .add_annotation(Annotation {
                    id: AnnotationId(id.into()),
                    target: TimedTarget {
                        item_id: episode.clone(),
                        offset_ms: 90_500,
                        representation: receipt.clone(),
                    },
                    body,
                    created_at_ms: 600,
                })
                .unwrap();
        }
        model
    }

    #[test]
    fn earlier_schema_one_without_selection_keeps_progress_and_notes() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let mut expected = populated_model();
        expected.selected_item = None;
        let mut document = serde_json::to_value(&expected).unwrap();
        document.as_object_mut().unwrap().remove("selected_item");
        fs::write(
            store.generation_path(1),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        assert_eq!(store.load().unwrap(), Some(expected));
    }

    #[test]
    fn library_progress_queue_and_annotations_survive_restart() {
        let directory = tempdir().unwrap();
        let expected = populated_model();
        JsonDirectoryStore::new(directory.path())
            .save(&expected)
            .unwrap();

        let reopened = JsonDirectoryStore::new(directory.path())
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(reopened, expected);
    }

    #[test]
    fn failed_publish_leaves_the_previous_generation_readable() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let previous = populated_model();
        store.save(&previous).unwrap();

        let mut replacement = previous.clone();
        replacement.settings.skip_forward_ms = 60_000;
        let failing_store = JsonDirectoryStore {
            root: directory.path().into(),
            fail_before_publish: true,
        };
        assert!(failing_store.save(&replacement).is_err());

        assert_eq!(store.load().unwrap(), Some(previous));
        store.save(&replacement).unwrap();
        assert_eq!(store.load().unwrap(), Some(replacement));
    }

    #[test]
    fn corrupt_newer_generation_falls_back_to_the_latest_valid_state() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let expected = populated_model();
        store.save(&expected).unwrap();
        fs::write(store.generation_path(2), b"not json").unwrap();

        assert_eq!(store.load().unwrap(), Some(expected));
    }

    #[test]
    fn schema_one_without_selected_item_still_loads() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("state-00000000000000000001.json"),
            br#"{"schema_version":1,"library":{},"queue":[],"progress":{},"annotations":{},"settings":{"capture_playback":"pause","skip_forward_ms":30000,"skip_backward_ms":15000}}"#,
        )
        .unwrap();
        let loaded = JsonDirectoryStore::new(directory.path())
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(loaded.selected_item, None);
    }
}
