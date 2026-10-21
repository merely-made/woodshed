#![forbid(unsafe_code)]

mod recovery;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(not(windows))]
use std::fs::File;

use redshank_model::RedshankModel;

/// Keep the current published state and two valid recovery generations.
const RETAINED_GENERATIONS: usize = 3;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Encode(serde_json::Error),
    /// Published generations exist, but none can establish a listener model.
    NoValidGeneration { paths: Vec<PathBuf> },
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
    /// `None` means no published generations, never failed recovery.
    fn load(&self) -> Result<Option<RedshankModel>, StoreError>;
    fn save(&self, model: &RedshankModel) -> Result<(), StoreError>;
}

/// A crash-tolerant local store made from immutable numbered generations.
/// Pending or corrupt newer files are ignored, leaving the latest valid prior
/// generation readable. Successful publication retains the newest three valid
/// generations. Pending files are never pruned: another writer may still be
/// publishing them. Failed recovery refuses publication before any writes.
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

    fn load_with_reader(
        &self,
        mut read: impl FnMut(&Path) -> io::Result<Vec<u8>>,
    ) -> Result<Option<RedshankModel>, StoreError> {
        let generations = self.generations()?;
        if generations.is_empty() {
            return Ok(None);
        }
        for (_, path) in generations.iter().rev() {
            let bytes = match read(path) {
                Ok(bytes) => bytes,
                // Another writer may prune a listed generation before this
                // read. Keep trying older candidates, but retain the original
                // inventory so exhausting them cannot be mistaken for empty.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if let Ok(model) = serde_json::from_slice(&bytes) {
                return Ok(Some(model));
            }
        }
        Err(StoreError::NoValidGeneration {
            paths: generations.into_iter().map(|(_, path)| path).collect(),
        })
    }

    /// Only run after the replacement has been flushed and published. Count
    /// valid states rather than filenames so corruption cannot evict recovery
    /// data. A cleanup error is returned to the caller, but does not roll back
    /// the already-published replacement or remove retained recovery states.
    fn prune_generations(&self) -> Result<(), StoreError> {
        let mut retained = 0;
        let mut removed = false;
        for (_, path) in self.generations()?.into_iter().rev() {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if retained < RETAINED_GENERATIONS
                && serde_json::from_slice::<RedshankModel>(&bytes).is_ok()
            {
                retained += 1;
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => removed = true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(error.into()),
            }
        }
        if removed {
            sync_directory(&self.root)?;
        }
        Ok(())
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
        self.load_with_reader(|path| fs::read(path))
    }

    fn save(&self, model: &RedshankModel) -> Result<(), StoreError> {
        // Publication and pruning require an empty or recoverable store. In
        // particular, a default model must not replace failed recovery data.
        self.load()?;
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
        self.prune_generations()
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
        Annotation, AnnotationId, CaptureAnchor, ItemId, LibraryItem, ListeningSession,
        MediaSource, NoteBody, NotePrivacy, Progress, RepresentationReceipt, TimedTarget,
    };
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn explicitly_saved_transcript_survives_restart_and_item_removal() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let mut model = populated_model();
        let id = ItemId("episode".into());
        let saved = redshank_model::SavedTranscript {
            resource: redshank_model::FeedTranscript {
                url: "https://example.test/transcript.vtt".into(),
                media_type: Some("text/vtt".into()),
                language: Some("en".into()),
                relation: None,
            },
            final_url: "https://cdn.example.test/transcript.vtt".into(),
            retrieved_at_ms: 42,
            source: "WEBVTT\n\n00:01.000 --> 00:02.000\nPreserved source".into(),
        };
        model.transcripts.insert(id.clone(), saved.clone());
        store.save(&model).unwrap();
        let mut reopened = store.load().unwrap().unwrap();
        assert_eq!(reopened.transcripts[&id], saved);
        reopened.remove_item(&id).unwrap();
        assert!(!reopened.transcripts.contains_key(&id));
    }

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
                facts: Default::default(),
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
                        fingerprint: None,
                        item_id: episode.clone(),
                        offset_ms: 90_500,
                        end_offset_ms: None,
                        pressed_offset_ms: Some(92_000),
                        representation: receipt.clone(),
                    },
                    body,
                    created_at_ms: 600,
                    privacy: NotePrivacy::Private,
                })
                .unwrap();
        }
        model.settings.note_privacy = NotePrivacy::Shareable;
        model
            .add_span_annotation(
                AnnotationId("span-note".into()),
                CaptureAnchor {
                    fingerprint: None,
                    item_id: episode.clone(),
                    offset_ms: 100_000,
                    end_offset_ms: None,
                    pressed_offset_ms: None,
                    representation: receipt,
                },
                140_000,
                "the whole passage".into(),
                700,
            )
            .unwrap();
        model.record_listening_session(ListeningSession {
            item_id: episode,
            start_ms: 0,
            stop_ms: 91_000,
            started_at_ms: 400,
            ended_at_ms: 500,
            completed: false,
        });
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
    fn empty_and_missing_directories_have_no_published_state() {
        let directory = tempdir().unwrap();
        assert_eq!(
            JsonDirectoryStore::new(directory.path()).load().unwrap(),
            None
        );
        let missing = directory.path().join("not-created");
        assert_eq!(JsonDirectoryStore::new(&missing).load().unwrap(), None);
        assert!(!missing.exists());
    }

    #[test]
    fn all_invalid_generations_refuse_replacement_and_preserve_every_byte() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let fixtures: [&[u8]; 3] = [b"not json", b"{}", b"null"];
        for (index, bytes) in fixtures.iter().enumerate() {
            fs::write(store.generation_path(index as u64 + 1), bytes).unwrap();
        }
        let error = store.load().unwrap_err();
        let StoreError::NoValidGeneration { paths } = error else {
            panic!("all-invalid state must differ from empty and I/O failure")
        };
        assert_eq!(paths.len(), fixtures.len());
        assert!(matches!(
            store.save(&RedshankModel::default()),
            Err(StoreError::NoValidGeneration { .. })
        ));
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            fixtures.len()
        );
        for (index, bytes) in fixtures.iter().enumerate() {
            assert_eq!(
                fs::read(store.generation_path(index as u64 + 1)).unwrap(),
                *bytes
            );
        }
    }

    #[test]
    fn io_failure_refuses_load_and_save_without_publishing_or_pruning() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        // Deterministic even when tests run with permission to bypass chmod.
        fs::create_dir(store.generation_path(1)).unwrap();
        let evidence = store.generation_path(1).join("retained-evidence");
        fs::write(&evidence, b"original bytes").unwrap();
        assert!(matches!(store.load(), Err(StoreError::Io(_))));
        assert!(matches!(
            store.save(&RedshankModel::default()),
            Err(StoreError::Io(_))
        ));
        assert_eq!(fs::read(evidence).unwrap(), b"original bytes");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

        let not_a_directory = directory.path().join("store-file");
        fs::write(&not_a_directory, b"keep this file").unwrap();
        let inaccessible = JsonDirectoryStore::new(&not_a_directory);
        assert!(matches!(inaccessible.load(), Err(StoreError::Io(_))));
        assert!(matches!(
            inaccessible.save(&RedshankModel::default()),
            Err(StoreError::Io(_))
        ));
        assert_eq!(fs::read(not_a_directory).unwrap(), b"keep this file");
    }

    #[test]
    fn vanished_newest_candidate_falls_back_to_older_valid_generation() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let expected = populated_model();
        store.save(&expected).unwrap();
        let newest = store.generation_path(2);
        fs::write(&newest, b"listed before removal").unwrap();
        let mut reads = Vec::new();
        let loaded = store.load_with_reader(|path| {
            reads.push(path.to_path_buf());
            if path == newest.as_path() {
                fs::remove_file(path).unwrap();
            }
            fs::read(path)
        });
        assert_eq!(loaded.unwrap(), Some(expected.clone()));
        assert_eq!(reads, vec![newest, store.generation_path(1)]);
        assert_eq!(store.load().unwrap(), Some(expected));
    }

    #[test]
    fn vanished_and_corrupt_candidates_never_become_an_empty_restore() {
        for all_vanished in [false, true] {
            let directory = tempdir().unwrap();
            let store = JsonDirectoryStore::new(directory.path());
            let older = store.generation_path(1);
            let newest = store.generation_path(2);
            fs::write(&older, b"retained corrupt evidence").unwrap();
            fs::write(&newest, b"listed before removal").unwrap();
            let mut reads = Vec::new();
            let result = store.load_with_reader(|path| {
                reads.push(path.to_path_buf());
                if all_vanished || path == newest.as_path() {
                    fs::remove_file(path).unwrap();
                }
                fs::read(path)
            });
            let Err(StoreError::NoValidGeneration { paths }) = result else {
                panic!("exhausted listed candidates must fail recovery, never return empty")
            };
            assert_eq!(reads, vec![newest.clone(), older.clone()]);
            assert_eq!(paths, vec![older.clone(), newest]);
            if !all_vanished {
                assert_eq!(fs::read(older).unwrap(), b"retained corrupt evidence");
                assert!(matches!(
                    store.save(&RedshankModel::default()),
                    Err(StoreError::NoValidGeneration { .. })
                ));
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            }
        }
    }

    #[test]
    fn corrupt_newer_generation_falls_back_to_the_latest_valid_state() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let expected = populated_model();
        store.save(&expected).unwrap();
        fs::write(store.generation_path(2), b"not json").unwrap();
        fs::write(store.generation_path(3), b"{}").unwrap();

        assert_eq!(store.load().unwrap(), Some(expected));
        assert_eq!(fs::read(store.generation_path(2)).unwrap(), b"not json");
        assert_eq!(fs::read(store.generation_path(3)).unwrap(), b"{}");
    }

    #[test]
    fn repeated_saves_bound_disk_history_and_keep_two_recovery_states() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let mut model = populated_model();
        for offset in 1..=20 {
            model.settings.skip_forward_ms = offset * 1_000;
            store.save(&model).unwrap();
            assert!(store.generations().unwrap().len() <= RETAINED_GENERATIONS);
        }
        let generations = store.generations().unwrap();
        assert_eq!(
            generations
                .iter()
                .map(|(number, _)| *number)
                .collect::<Vec<_>>(),
            [18, 19, 20]
        );
        assert_eq!(store.load().unwrap(), Some(model.clone()));

        fs::write(&generations[2].1, b"corrupt latest").unwrap();
        model.settings.skip_forward_ms = 19_000;
        assert_eq!(
            JsonDirectoryStore::new(directory.path()).load().unwrap(),
            Some(model.clone())
        );
        fs::write(&generations[1].1, b"corrupt second latest").unwrap();
        model.settings.skip_forward_ms = 18_000;
        assert_eq!(
            JsonDirectoryStore::new(directory.path()).load().unwrap(),
            Some(model)
        );
    }

    #[test]
    fn pruning_counts_valid_states_and_leaves_pending_and_unrelated_files() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let mut model = populated_model();
        for offset in 1..=3 {
            model.settings.skip_forward_ms = offset * 1_000;
            store.save(&model).unwrap();
        }
        fs::write(store.generation_path(4), b"corrupt newer state").unwrap();
        let pending = pending_path(&store.generation_path(5));
        fs::write(&pending, b"unfinished publication").unwrap();
        let unrelated = directory.path().join("voice-note.wav");
        fs::write(&unrelated, b"owned media").unwrap();

        model.settings.skip_forward_ms = 6_000;
        store.save(&model).unwrap();
        assert_eq!(
            store
                .generations()
                .unwrap()
                .iter()
                .map(|(number, _)| *number)
                .collect::<Vec<_>>(),
            [2, 3, 6]
        );
        assert_eq!(fs::read(pending).unwrap(), b"unfinished publication");
        assert_eq!(fs::read(unrelated).unwrap(), b"owned media");
        fs::write(store.generation_path(6), b"corrupt replacement").unwrap();
        model.settings.skip_forward_ms = 3_000;
        assert_eq!(store.load().unwrap(), Some(model));
    }

    #[test]
    fn failed_publication_does_not_prune_retained_recovery_states() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        let mut previous = populated_model();
        for offset in 1..=3 {
            previous.settings.skip_forward_ms = offset * 1_000;
            store.save(&previous).unwrap();
        }
        let failing = JsonDirectoryStore {
            root: directory.path().into(),
            fail_before_publish: true,
        };
        let mut replacement = previous.clone();
        replacement.settings.skip_forward_ms = 60_000;
        assert!(failing.save(&replacement).is_err());
        assert_eq!(store.generations().unwrap().len(), RETAINED_GENERATIONS);
        assert_eq!(
            JsonDirectoryStore::new(directory.path()).load().unwrap(),
            Some(previous)
        );
    }

    #[test]
    fn cleanup_error_reports_failure_without_rolling_back_published_state() {
        let directory = tempdir().unwrap();
        let store = JsonDirectoryStore::new(directory.path());
        // An unreadable generation is an I/O error rather than JSON corruption.
        store.save(&populated_model()).unwrap();
        fs::create_dir(store.generation_path(0)).unwrap();
        let replacement = populated_model();
        assert!(store.save(&replacement).is_err());
        assert_eq!(store.load().unwrap(), Some(replacement));
        assert!(store.generation_path(0).is_dir());
        assert!(store.generation_path(2).is_file());
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
