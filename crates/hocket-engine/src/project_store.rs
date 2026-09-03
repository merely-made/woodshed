// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Durable Hocket project storage over Muniment's host-supplied byte backend.
//!
//! With a [`ZipBackend`](muniment::ZipBackend) the keys here become the entry
//! names of a plain zip archive, so a saved `.hock` file opens in any unzip
//! tool: the manifest is `manifest.cbor` and each captured phrase is a lossless
//! WavPack `media/<hash>.wv` (via wavicle). WavPack is an open format real DAWs
//! read, so the layout stays importable without Hocket while compressing far
//! better than WAV; the store itself is backend-agnostic.
//!
//! The model's [`ProjectBundle`] is one mutable manifest. Captured media stays
//! immutable and content-addressed under its existing [`MediaRef`], which hashes
//! the capture sample rate and decoded samples together — the `.wv` file is just
//! a carrier, so the reference is verified against the *decoded* audio on load,
//! not the file bytes.

use std::collections::BTreeSet;

use muniment::{Backend, StoreError, WriteOp};
use hocket_model::{MediaRef, PersistenceError, ProjectBundle};

use crate::media::{InMemoryStore, MediaBuffer, MediaStore, hash_buffer};

/// The manifest entry name for one Hocket project archive.
pub const MANIFEST_KEY: &str = "manifest.cbor";
/// Directory prefix for content-addressed media entries: `media/<hash>.wv`.
const MEDIA_PREFIX: &str = "media/";
/// Human-readable provenance entry, so someone who unzips a `.hock` sees what
/// wrote it. Informational only: it is not read back on load.
const META_KEY: &str = "meta.json";

/// Project storage over a host-selected Muniment backend. A desktop host can
/// use Redb; a browser host can later provide OPFS through the same interface.
pub struct ProjectStore<B> {
    backend: B,
}

/// A manifest plus every media blob that was available at load time. Missing
/// blobs do not prevent opening the project: their layers remain in history but
/// stay silent until a peer, backup, or later import supplies the media.
#[derive(Clone, Debug)]
pub struct LoadedProject {
    pub bundle: ProjectBundle,
    pub media: InMemoryStore,
    pub missing_media: BTreeSet<MediaRef>,
}

#[derive(Debug)]
pub enum ProjectStoreError {
    Store(StoreError),
    Manifest(PersistenceError),
    MissingManifest,
    MissingMedia(BTreeSet<MediaRef>),
    InvalidMedia {
        reference: MediaRef,
        reason: &'static str,
    },
    MediaEncode(String),
    MediaHashMismatch {
        expected: MediaRef,
        actual: MediaRef,
    },
}

impl std::fmt::Display for ProjectStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(f, "storage failed: {error}"),
            Self::Manifest(error) => write!(f, "project manifest failed: {error}"),
            Self::MissingManifest => f.write_str("project manifest is missing"),
            Self::MissingMedia(references) => {
                write!(
                    f,
                    "project save is missing {} media blob(s)",
                    references.len()
                )
            }
            Self::InvalidMedia { reference, reason } => {
                write!(f, "media {reference} is invalid: {reason}")
            }
            Self::MediaEncode(error) => write!(f, "media encoding failed: {error}"),
            Self::MediaHashMismatch { expected, actual } => {
                write!(
                    f,
                    "media hash mismatch: expected {expected}, found {actual}"
                )
            }
        }
    }
}

impl std::error::Error for ProjectStoreError {}

impl From<StoreError> for ProjectStoreError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<PersistenceError> for ProjectStoreError {
    fn from(error: PersistenceError) -> Self {
        Self::Manifest(error)
    }
}

impl<B: Backend> ProjectStore<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Save every referenced media blob and the manifest in one backend batch.
    /// An existing blob is reused only when it decodes to the exact referenced
    /// audio; a missing, corrupt, or externally-edited blob is overwritten with
    /// the known-good in-memory bytes rather than failing the save. Media no
    /// longer referenced (e.g. saving a different project over the file) is
    /// pruned, so discarded audio is not retained. Transactional backends make
    /// the whole batch all-or-nothing.
    pub async fn save(
        &self,
        bundle: &ProjectBundle,
        media: &impl MediaStore,
    ) -> Result<(), ProjectStoreError> {
        let references = referenced_media(bundle);
        let missing: BTreeSet<MediaRef> = references
            .iter()
            .filter(|reference| media.get(reference).is_none())
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(ProjectStoreError::MissingMedia(missing));
        }

        let wanted_keys: BTreeSet<String> =
            references.iter().map(|reference| media_key(*reference)).collect();

        let mut writes = Vec::with_capacity(references.len() + 1);
        for reference in &references {
            let reference = *reference;
            let buffer = media
                .get(&reference)
                .expect("missing references checked above");
            // The in-memory store is keyed by content hash, so a mismatch here
            // is an internal inconsistency, not recoverable user data.
            let actual = hash_buffer(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(ProjectStoreError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
            let key = media_key(reference);
            // Reuse the stored blob only if it is present and decodes to exactly
            // this audio. Otherwise overwrite it: we validated the in-memory
            // bytes above, so a corrupt or tampered-with `.wv` self-heals on
            // save instead of blocking every future save.
            let reusable = match self.backend.get(&key).await? {
                Some(existing) => decode_media(reference, &existing)
                    .map(|stored| hash_buffer(&stored.samples, stored.sample_rate) == reference)
                    .unwrap_or(false),
                None => false,
            };
            if !reusable {
                writes.push(WriteOp::Put {
                    key,
                    value: encode_media(buffer)?,
                });
            }
        }

        // Prune media entries the current project no longer references.
        for existing_key in self.backend.list(MEDIA_PREFIX).await? {
            if !wanted_keys.contains(&existing_key) {
                writes.push(WriteOp::Delete { key: existing_key });
            }
        }

        writes.push(WriteOp::Put {
            key: META_KEY.to_string(),
            value: meta_json(),
        });
        writes.push(WriteOp::Put {
            key: MANIFEST_KEY.to_string(),
            value: bundle.to_bytes()?,
        });
        self.backend.apply(&writes).await?;
        Ok(())
    }

    /// Load the manifest and every available blob. Missing blob keys are
    /// reported in [`LoadedProject::missing_media`] rather than failing open.
    pub async fn load(&self) -> Result<LoadedProject, ProjectStoreError> {
        let bytes = self
            .backend
            .get(MANIFEST_KEY)
            .await?
            .ok_or(ProjectStoreError::MissingManifest)?;
        let bundle = ProjectBundle::from_bytes(&bytes)?;
        let mut media = InMemoryStore::new();
        let mut missing_media = BTreeSet::new();

        for reference in referenced_media(&bundle) {
            let Some(bytes) = self.backend.get(&media_key(reference)).await? else {
                missing_media.insert(reference);
                continue;
            };
            let buffer = decode_media(reference, &bytes)?;
            let actual = media.put(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(ProjectStoreError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
        }

        Ok(LoadedProject {
            bundle,
            media,
            missing_media,
        })
    }
}

fn referenced_media(bundle: &ProjectBundle) -> BTreeSet<MediaRef> {
    bundle
        .session
        .phrases
        .values()
        .map(|phrase| phrase.media)
        .collect()
}

/// A small, human-readable provenance record for the archive. Hand-built (no
/// serde dependency) since the shape is fixed; kept valid JSON so any tool reads
/// it. `manifest_format` mirrors the CBOR manifest's schema version.
fn meta_json() -> Vec<u8> {
    format!(
        "{{\n  \"format\": \"hocket-project\",\n  \"manifest_format\": {},\n  \"generator\": \"hocket-engine {}\"\n}}\n",
        ProjectBundle::FORMAT_VERSION,
        env!("CARGO_PKG_VERSION"),
    )
    .into_bytes()
}

fn media_key(reference: MediaRef) -> String {
    let mut key = String::with_capacity(MEDIA_PREFIX.len() + 64 + 4);
    key.push_str(MEDIA_PREFIX);
    for byte in reference.0 {
        use std::fmt::Write as _;
        write!(&mut key, "{byte:02x}").expect("writing to a string cannot fail");
    }
    key.push_str(".wv");
    key
}

/// Encode one mono f32 phrase as lossless WavPack (`.wv`) via wavicle. WavPack
/// is an open format real DAWs read, so a `.hock` stays importable without
/// Hocket, and wavicle is pure Rust so this also works on the wasm host.
/// Handles any length and capture rate (no WAV 4 GiB or rate-table limit).
/// Identity still travels in the content-addressed file name, not the bytes.
fn encode_media(buffer: &MediaBuffer) -> Result<Vec<u8>, ProjectStoreError> {
    wavicle::encode_float(1, buffer.sample_rate, &buffer.samples)
        .map_err(|error| ProjectStoreError::MediaEncode(error.to_string()))
}

/// Decode a stored `.wv` phrase back to samples. The caller re-hashes the
/// result against its [`MediaRef`], so a codec or corruption mismatch is caught
/// there. wavicle enforces the block CRCs internally as hard errors.
fn decode_media(reference: MediaRef, bytes: &[u8]) -> Result<MediaBuffer, ProjectStoreError> {
    let decoded = wavicle::decode_stream(bytes).map_err(|_| ProjectStoreError::InvalidMedia {
        reference,
        reason: "unreadable WavPack",
    })?;
    if decoded.channels != 1 || !decoded.is_float {
        return Err(ProjectStoreError::InvalidMedia {
            reference,
            reason: "expected mono 32-bit float media",
        });
    }
    // wavicle returns float samples as their IEEE bit patterns in `i32`.
    let samples = decoded
        .samples
        .iter()
        .map(|&s| f32::from_bits(s as u32))
        .collect();
    Ok(MediaBuffer {
        samples,
        sample_rate: decoded.sample_rate,
    })
}

#[cfg(test)]
mod tests {
    use muniment::{Backend, MemoryBackend};
    use pollster::block_on;
    use hocket_model::{Edit, History, Layer, Phrase, Session};

    use super::*;

    fn bundle_with_one_layer(store: &mut InMemoryStore) -> ProjectBundle {
        let mut session = Session::new_default();
        let mut history = History::new();
        let media = store.put(&[0.25, -0.5, 0.75], 48_000);
        let phrase = Phrase::new(media, session.bars_per_phrase, session.bpm, 1);
        let layer = Layer::new(phrase.id);
        let track_id = session.tracks[0].id;
        history.commit(
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            },
            &mut session,
            1,
        );
        ProjectBundle::new(session, history)
    }

    #[test]
    fn save_and_load_round_trip_manifest_and_media() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);

            project.save(&bundle, &media).await.unwrap();
            let loaded = project.load().await.unwrap();

            assert_eq!(loaded.bundle, bundle);
            assert!(loaded.missing_media.is_empty());
            let reference = loaded.bundle.session.phrases.values().next().unwrap().media;
            assert_eq!(
                loaded.media.get(&reference).unwrap().samples,
                vec![0.25, -0.5, 0.75]
            );
            assert_eq!(backend.get(MANIFEST_KEY).await.unwrap().is_some(), true);
        });
    }

    /// The archive entries are human-meaningful file names, not opaque keys, so
    /// a saved `.hock` is inspectable. Locks the no-lock-in layout against drift.
    #[test]
    fn save_uses_human_friendly_entry_names() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);

            project.save(&bundle, &media).await.unwrap();

            let reference = bundle.session.phrases.values().next().unwrap().media;
            let mut keys = backend.list("").await.unwrap();
            keys.sort();
            // manifest.cbor, media/<hash>.wv, meta.json
            assert_eq!(
                keys,
                vec![
                    "manifest.cbor".to_string(),
                    media_key(reference),
                    "meta.json".to_string(),
                ]
            );
            assert!(media_key(reference).starts_with("media/"));
            assert!(media_key(reference).ends_with(".wv"));

            // The stored media entry is a real WavPack file (wvpk magic) that a
            // DAW or wvunpack can open.
            let wv = backend.get(&media_key(reference)).await.unwrap().unwrap();
            assert_eq!(&wv[..4], b"wvpk");

            // The provenance entry is human-readable JSON naming the format.
            let meta = backend.get("meta.json").await.unwrap().unwrap();
            let meta = String::from_utf8(meta).unwrap();
            assert!(meta.contains("\"format\": \"hocket-project\""), "meta.json: {meta}");
        });
    }

    #[test]
    fn save_rejects_manifest_that_references_missing_media() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend);
            let mut populated = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut populated);
            let empty = InMemoryStore::new();

            let error = project.save(&bundle, &empty).await.unwrap_err();
            assert!(matches!(error, ProjectStoreError::MissingMedia(_)));
        });
    }

    #[test]
    fn load_keeps_project_when_a_media_blob_is_missing() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);
            backend
                .put(MANIFEST_KEY, &bundle.to_bytes().unwrap())
                .await
                .unwrap();

            let loaded = project.load().await.unwrap();
            assert_eq!(loaded.bundle, bundle);
            assert_eq!(loaded.missing_media.len(), 1);
            assert!(loaded.media.is_empty());
        });
    }

    #[test]
    fn corrupt_media_is_rejected() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);
            let reference = bundle.session.phrases.values().next().unwrap().media;
            backend
                .put(MANIFEST_KEY, &bundle.to_bytes().unwrap())
                .await
                .unwrap();
            backend
                .put(&media_key(reference), b"not audio")
                .await
                .unwrap();

            let error = project.load().await.unwrap_err();
            assert!(matches!(error, ProjectStoreError::InvalidMedia { .. }));
        });
    }

    #[test]
    fn save_heals_a_corrupt_existing_media_blob() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);
            let reference = bundle.session.phrases.values().next().unwrap().media;
            // A corrupt/tampered existing blob for a still-good in-memory phrase.
            backend
                .put(&media_key(reference), b"not audio")
                .await
                .unwrap();

            // Save must not abort: it overwrites the bad blob with valid audio.
            project.save(&bundle, &media).await.unwrap();

            // The project now reopens cleanly with the healed media.
            let loaded = project.load().await.unwrap();
            assert!(loaded.missing_media.is_empty());
            assert_eq!(loaded.bundle, bundle);
        });
    }

    #[test]
    fn save_prunes_media_the_new_project_no_longer_references() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());

            // Save project A, then a different project B over the same backend.
            let mut media_a = InMemoryStore::new();
            let bundle_a = bundle_with_one_layer(&mut media_a);
            let reference_a = bundle_a.session.phrases.values().next().unwrap().media;
            project.save(&bundle_a, &media_a).await.unwrap();

            let mut media_b = InMemoryStore::new();
            let mut session = Session::new_default();
            let mut history = History::new();
            let media = media_b.put(&[0.1, -0.2, 0.3], 44_100);
            let phrase = Phrase::new(media, session.bars_per_phrase, session.bpm, 2);
            let layer = Layer::new(phrase.id);
            let track_id = session.tracks[0].id;
            history.commit(
                Edit::AppendLayer {
                    track_id,
                    phrase,
                    layer,
                },
                &mut session,
                2,
            );
            let bundle_b = ProjectBundle::new(session, history);
            project.save(&bundle_b, &media_b).await.unwrap();

            // Project A's media is gone, not retained as an orphan blob.
            assert!(backend.get(&media_key(reference_a)).await.unwrap().is_none());
            let media_keys = backend.list(MEDIA_PREFIX).await.unwrap();
            assert_eq!(media_keys.len(), 1, "only project B's media remains");
        });
    }
}
