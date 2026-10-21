//! Explicit, separate recovery snapshots. The failed store is read-only here.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
};

use redshank_model::{MediaSource, NoteBody, RedshankModel};

use crate::{JsonDirectoryStore, ModelStore, StoreError, sync_directory};

impl JsonDirectoryStore {
    /// Save an independent recovery data directory without admitting any writes
    /// to this store. `destination` must not exist, its parent must exist, and
    /// it must be outside this store (including through a symlink).
    ///
    /// Local and cached library audio is copied and its paths are rewritten in
    /// the saved model. Desktop voice blobs retain their identities in the new
    /// directory. Network sources and inline transcripts/target evidence are
    /// preserved. Unresolvable host blobs or missing audio refuse publication.
    ///
    /// The model generation is published last. A failed copy may leave an
    /// incomplete destination, which is never reused or overwritten by this
    /// method. The returned canonical root can be opened independently with
    /// `REDSHANK_DATA_DIR`; this method does not change the active store.
    pub fn save_recovery_copy(
        &self,
        destination: &Path,
        model: &RedshankModel,
    ) -> Result<PathBuf, StoreError> {
        let root = fs::canonicalize(&self.root)?;
        let Some(Component::Normal(name)) = destination.components().next_back() else {
            return Err(invalid("Choose a new recovery directory name"));
        };
        let parent = fs::canonicalize(
            destination
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?;
        let destination = parent.join(name);
        if destination.starts_with(&root) {
            return Err(invalid(
                "The recovery copy must be outside the original Redshank data directory",
            ));
        }
        // Unlike create_dir_all, this refuses existing directories, files and
        // dangling symlinks. The newly owned directory is the only write root.
        fs::create_dir(&destination)?;
        sync_directory(&parent)?;

        let mut snapshot = model.clone();
        let mut media = BTreeMap::<(bool, PathBuf), PathBuf>::new();
        for item in snapshot.library.values_mut() {
            let source = match item.source() {
                MediaSource::Local { path } => {
                    let path = copy_media(path, false, &destination, &mut media)?;
                    MediaSource::Local { path }
                },
                MediaSource::Cached {
                    path,
                    origin_url,
                    representation,
                } => {
                    let path = copy_media(path, true, &destination, &mut media)?;
                    MediaSource::Cached {
                        path,
                        origin_url: origin_url.clone(),
                        representation: representation.clone(),
                    }
                },
                MediaSource::Enclosure { .. } => continue,
                MediaSource::HostBlob { .. } => {
                    return Err(invalid(
                        "This recovery copy contains host-only audio that the desktop cannot copy",
                    ));
                },
            };
            item.replace_source(source);
        }

        let mut voices = BTreeSet::new();
        for note in snapshot.annotations.values() {
            if let NoteBody::Audio { blob_id, .. } = &note.body {
                let digest = blob_id
                    .strip_prefix("voice:")
                    .filter(|digest| {
                        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
                    .ok_or_else(|| invalid("A voice note has an unsupported blob identity"))?;
                if voices.insert(digest) {
                    let voice = destination.join("voice");
                    if voices.len() == 1 {
                        fs::create_dir(&voice)?;
                    }
                    let name = format!("{digest}.wav");
                    copy_file(&root.join("voice").join(&name), &voice.join(name))?;
                }
            }
        }
        for directory in ["media", "cache", "voice"] {
            let path = destination.join(directory);
            if path.exists() {
                sync_directory(&path)?;
            }
        }
        // No published state exists before all the snapshot's media is flushed.
        JsonDirectoryStore::new(&destination).save(&snapshot)?;
        Ok(destination)
    }
}

fn invalid(message: &str) -> StoreError {
    io::Error::new(io::ErrorKind::InvalidInput, message).into()
}

fn copy_media(
    source: &str,
    cached: bool,
    destination: &Path,
    media: &mut BTreeMap<(bool, PathBuf), PathBuf>,
) -> Result<String, StoreError> {
    // Relative references have the same working-directory meaning as playback.
    // Resolve symlinks to deduplicate files without following directory trees.
    let source = fs::canonicalize(source)?;
    let key = (cached, source.clone());
    let target = if let Some(target) = media.get(&key) {
        target.clone()
    } else {
        let directory = destination.join(if cached { "cache" } else { "media" });
        if !directory.exists() {
            fs::create_dir(&directory)?;
        }
        let extension = if cached {
            "audio"
        } else {
            source
                .extension()
                .and_then(|extension| extension.to_str())
                .filter(|extension| {
                    !extension.is_empty()
                        && extension.len() <= 16
                        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
                })
                .unwrap_or("audio")
        };
        let target = directory.join(format!("recovery-{:08}.{extension}", media.len()));
        copy_file(&source, &target)?;
        media.insert(key, target.clone());
        target
    };
    target
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("The recovery audio path cannot be stored as Unicode"))
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), StoreError> {
    let mut input = File::open(source)?;
    let before = input.metadata()?;
    if !before.is_file() {
        return Err(invalid("A referenced audio path is not a regular file"));
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let count = io::copy(&mut input, &mut output)?;
    let after = input.metadata()?;
    if count != before.len()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(invalid(
            "Referenced audio changed while making the recovery copy",
        ));
    }
    output.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use redshank_model::{
        Annotation, AnnotationId, ItemId, LibraryItem, NotePrivacy, RepresentationReceipt,
        SavedTranscript, TimedTarget,
    };
    use tempfile::tempdir;

    use super::*;

    fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        fn visit(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(root).unwrap().into(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }
        visit(root, root, &mut files);
        files
    }

    fn failed_store(root: &Path) -> JsonDirectoryStore {
        fs::create_dir(root).unwrap();
        fs::write(root.join("state-00000000000000000001.json"), b"bad state").unwrap();
        fs::write(
            root.join("state-00000000000000000002.json.pending"),
            b"pending",
        )
        .unwrap();
        let store = JsonDirectoryStore::new(root);
        assert!(matches!(
            store.load(),
            Err(StoreError::NoValidGeneration { .. })
        ));
        store
    }

    fn local_model(path: &Path) -> RedshankModel {
        let mut model = RedshankModel::default();
        model
            .add_item(LibraryItem::LocalAudio {
                id: ItemId("local".into()),
                title: "Local recording".into(),
                source: MediaSource::Local {
                    path: path.to_str().unwrap().into(),
                },
            })
            .unwrap();
        model
    }

    #[test]
    fn recovery_copy_reopens_with_local_cache_voice_and_inline_evidence() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        let local = base.path().join("external.flac");
        fs::write(&local, b"local audio bytes").unwrap();
        fs::create_dir(root.join("cache")).unwrap();
        let cached = root.join("cache/source.audio");
        fs::write(&cached, b"cached representation bytes").unwrap();
        let digest = "a".repeat(64);
        fs::create_dir(root.join("voice")).unwrap();
        fs::write(
            root.join(format!("voice/{digest}.wav")),
            b"voice note bytes",
        )
        .unwrap();
        let mut model = local_model(&local);
        let local_id = ItemId("local".into());
        let cached_id = ItemId("cached".into());
        let representation = RepresentationReceipt {
            complete_digest: Some("blake3:cached-evidence".into()),
            final_url: Some("https://cdn.example.test/audio".into()),
            ..Default::default()
        };
        model
            .add_item(LibraryItem::DirectAudio {
                id: cached_id.clone(),
                title: "Offline recording".into(),
                source: MediaSource::Cached {
                    path: cached.to_str().unwrap().into(),
                    origin_url: "https://example.test/audio".into(),
                    representation: Box::new(representation.clone()),
                },
            })
            .unwrap();
        model
            .add_item(LibraryItem::DirectAudio {
                id: ItemId("remote".into()),
                title: "Remote recording".into(),
                source: MediaSource::Enclosure {
                    url: "https://example.test/remote".into(),
                },
            })
            .unwrap();
        for name in ["voice-one", "voice-two"] {
            model
                .add_annotation(Annotation {
                    id: AnnotationId(name.into()),
                    target: TimedTarget {
                        item_id: local_id.clone(),
                        offset_ms: 120,
                        end_offset_ms: None,
                        pressed_offset_ms: Some(160),
                        representation: representation.clone(),
                        fingerprint: None,
                    },
                    body: NoteBody::Audio {
                        blob_id: format!("voice:{digest}"),
                        media_type: "audio/wav".into(),
                        duration_ms: 2_000,
                    },
                    created_at_ms: 5,
                    privacy: NotePrivacy::Private,
                })
                .unwrap();
        }
        model.transcripts.insert(
            local_id.clone(),
            SavedTranscript {
                resource: Default::default(),
                final_url: "https://example.test/transcript".into(),
                retrieved_at_ms: 7,
                source: "WEBVTT\n\n00:01.000 --> 00:02.000\nRetained source".into(),
            },
        );
        model.enqueue(&cached_id).unwrap();
        model.selected_item = Some(local_id.clone());
        model.settings.volume_percent = 37;
        let before = inventory(&root);
        let destination = base.path().join("recovered");
        let copied_root = store.save_recovery_copy(&destination, &model).unwrap();
        assert_eq!(copied_root, fs::canonicalize(&destination).unwrap());
        assert_eq!(inventory(&root), before);
        let reopened = JsonDirectoryStore::new(&copied_root)
            .load()
            .unwrap()
            .unwrap();
        let mut expected = model.clone();
        for id in [&local_id, &cached_id] {
            let source = reopened.library[id].source().clone();
            let path = match &source {
                MediaSource::Local { path } => {
                    assert_eq!(
                        Path::new(path).parent(),
                        Some(copied_root.join("media").as_path())
                    );
                    assert_eq!(fs::read(path).unwrap(), b"local audio bytes");
                    path
                },
                MediaSource::Cached {
                    path,
                    origin_url,
                    representation: actual,
                } => {
                    assert_eq!(
                        Path::new(path).parent(),
                        Some(copied_root.join("cache").as_path())
                    );
                    assert_eq!(Path::new(path).extension().unwrap(), "audio");
                    assert_eq!(origin_url, "https://example.test/audio");
                    assert_eq!(actual.as_ref(), &representation);
                    assert_eq!(fs::read(path).unwrap(), b"cached representation bytes");
                    path
                },
                _ => panic!("expected copied audio"),
            };
            assert!(Path::new(path).is_absolute());
            expected.library.get_mut(id).unwrap().replace_source(source);
        }
        assert_eq!(reopened, expected);
        assert_eq!(
            fs::read(copied_root.join(format!("voice/{digest}.wav"))).unwrap(),
            b"voice note bytes"
        );
        assert_eq!(fs::read_dir(copied_root.join("voice")).unwrap().count(), 1);
        // A fresh root can be loaded after the original directory and external
        // local file are unavailable; every local reference belongs to the copy.
        fs::rename(&root, base.path().join("original-unavailable")).unwrap();
        fs::remove_file(local).unwrap();
        assert_eq!(
            JsonDirectoryStore::new(&copied_root).load().unwrap(),
            Some(reopened)
        );
    }

    #[test]
    fn recovery_copy_refuses_existing_and_original_destinations_without_writes() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        let existing = base.path().join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("sentinel"), b"keep").unwrap();
        let before = inventory(base.path());
        for target in [&root, &root.join("copy"), &existing] {
            assert!(
                store
                    .save_recovery_copy(target, &RedshankModel::default())
                    .is_err()
            );
        }
        assert_eq!(inventory(base.path()), before);
    }

    #[cfg(unix)]
    #[test]
    fn recovery_copy_refuses_destination_symlink_alias_of_original() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        let alias = base.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let before = inventory(&root);
        assert!(
            store
                .save_recovery_copy(&alias.join("copy"), &RedshankModel::default())
                .is_err()
        );
        assert_eq!(inventory(&root), before);
    }

    #[test]
    fn recovery_copy_missing_media_leaves_no_published_model_or_original_changes() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        let destination = base.path().join("incomplete");
        let before = inventory(&root);
        assert!(
            store
                .save_recovery_copy(&destination, &local_model(&base.path().join("missing.wav")))
                .is_err()
        );
        assert_eq!(inventory(&root), before);
        assert_eq!(JsonDirectoryStore::new(&destination).load().unwrap(), None);
        assert!(
            store
                .save_recovery_copy(&destination, &RedshankModel::default())
                .is_err()
        );
    }

    #[test]
    fn recovery_copy_refuses_directory_media_and_host_only_sources() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        assert!(
            store
                .save_recovery_copy(
                    &base.path().join("directory-copy"),
                    &local_model(base.path())
                )
                .is_err()
        );
        let mut model = local_model(base.path());
        model
            .library
            .get_mut(&ItemId("local".into()))
            .unwrap()
            .replace_source(MediaSource::HostBlob {
                id: "browser-blob".into(),
            });
        assert!(
            store
                .save_recovery_copy(&base.path().join("host-copy"), &model)
                .is_err()
        );
    }

    #[test]
    fn recovery_copy_deduplicates_local_references() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        let local = base.path().join("local.wav");
        fs::write(&local, b"shared audio").unwrap();
        let mut model = local_model(&local);
        model
            .add_item(LibraryItem::LocalAudio {
                id: ItemId("second".into()),
                title: "Same audio".into(),
                source: model.library[&ItemId("local".into())].source().clone(),
            })
            .unwrap();
        let destination = store
            .save_recovery_copy(&base.path().join("copy"), &model)
            .unwrap();
        assert_eq!(fs::read_dir(destination.join("media")).unwrap().count(), 1);
        let reopened = JsonDirectoryStore::new(destination)
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(
            reopened.library[&ItemId("local".into())].source(),
            reopened.library[&ItemId("second".into())].source()
        );
    }

    #[test]
    fn recovery_copy_refuses_missing_or_unsupported_voice_blobs() {
        let base = tempdir().unwrap();
        let root = base.path().join("failed");
        let store = failed_store(&root);
        let before = inventory(&root);
        for (index, blob_id) in [
            "voice:../escape".into(),
            "browser-blob".into(),
            format!("voice:{}", "b".repeat(64)),
        ]
        .into_iter()
        .enumerate()
        {
            let mut model = RedshankModel::default();
            // Voice annotations remain recoverable even if their target item
            // is no longer present in the library; the blob is still required.
            model.annotations.insert(
                AnnotationId("voice".into()),
                Annotation {
                    id: AnnotationId("voice".into()),
                    target: TimedTarget {
                        item_id: ItemId("removed-item".into()),
                        offset_ms: 120,
                        end_offset_ms: None,
                        pressed_offset_ms: None,
                        representation: Default::default(),
                        fingerprint: None,
                    },
                    body: NoteBody::Audio {
                        blob_id,
                        media_type: "audio/wav".into(),
                        duration_ms: 2_000,
                    },
                    created_at_ms: 5,
                    privacy: NotePrivacy::Private,
                },
            );
            let destination = base.path().join(format!("incomplete-{index}"));
            assert!(store.save_recovery_copy(&destination, &model).is_err());
            assert_eq!(JsonDirectoryStore::new(destination).load().unwrap(), None);
            assert_eq!(inventory(&root), before);
        }
    }
}
