//! The desktop backend: files in the config dir the xilem app uses
//! (`ProjectDirs dev/Woodshed/Woodshed`), under their own filenames so the two
//! apps never clobber each other during the migration.
//!
//! A [`muniment::Backend`] rather than woodshed's own storage trait, so the
//! store, the sealing, and the slot naming above it are all muniment's and this
//! file is only the platform half: which directory, which filename per slot.
//! The web host realizes the same trait over OPFS.
//!
//! **The sealing key comes from djinn** (dramatis DR-C). Woodshed opens no
//! vault: it calls djinn over Graphshell's custody route, which releases the
//! one key Woodshed seals practice with (D11). With djinn absent or Locked the
//! identity is pending (D12): already-saved practice that was never sealed
//! still reads, and nothing is written, in the clear or otherwise.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use async_trait::async_trait;
use directories::ProjectDirs;
use graphshell_endpoint::door::custody::CustodyRefusal;
use graphshell_endpoint::door::custody_client::CustodyClientError;
use graphshell_endpoint::door::custody_identity::CustodyIdentity;
use muniment::backend::WriteOp;
use muniment::{Backend, StoreError};
use personae::ProfileId;
use woodshed_core::loop_capture::{MAX_PCM_BYTES, pcm_key, validate_namespace};
use woodshed_core::sealed_backend::SealedBackend;
use woodshed_core::storage::SessionStore;
use woodshed_views::persona::PracticeSeal;

/// The name Woodshed is admitted to djinn's custody route under.
pub const CUSTODY_APP: &str = "woodshed";

/// The practice store, sealed to the persona djinn speaks as.
///
/// `None` keeps djinn's own choice, which is the startup path on every machine
/// that needs no asking. `Some` is the answer to the persona gate: djinn
/// switches to it (and remembers it for the family) before the store opens.
pub fn open_store_as(profile: Option<&ProfileId>) -> (SessionStore<HostBackend>, PracticeSeal) {
    let (backend, seal) = open_backend(profile);
    (SessionStore::new(backend), seal)
}

/// The store woodshed practices over, decided at startup.
pub type HostBackend = Box<dyn Backend + Send + Sync>;

/// Reach djinn as Woodshed, on `profile` when one was chosen. The error is the
/// pending reason, in words for Settings.
fn connect(profile: Option<&ProfileId>) -> Result<CustodyIdentity, String> {
    let identity = CustodyIdentity::connect(CUSTODY_APP).map_err(pending)?;
    match profile {
        Some(id) if identity.profile() != Some(id) => {
            identity.choose_profile(id.clone()).map_err(pending)?;
            // The provider answers for the persona it connected as.
            CustodyIdentity::connect(CUSTODY_APP).map_err(pending)
        },
        _ => Ok(identity),
    }
}

/// Why the identity is pending, in words for Settings.
fn pending(error: CustodyClientError) -> String {
    eprintln!("[woodshed] identity pending ({error}); practice is not saved");
    match error {
        CustodyClientError::Refused(CustodyRefusal::Locked) => "the vault is locked".into(),
        CustodyClientError::Absent(_) => "djinn is not running".into(),
        other => format!("djinn refused: {other}"),
    }
}

/// The backend, plus what is protecting what it writes.
///
/// The seal is returned rather than only logged: Settings offers to switch a
/// persona, and until this it could not name the one in force. Every branch
/// answers, so "pending" always arrives with its reason attached.
fn open_backend(profile: Option<&ProfileId>) -> (HostBackend, PracticeSeal) {
    let identity = match connect(profile) {
        Ok(identity) => identity,
        Err(reason) => {
            return (
                Box::new(PendingBackend::new(FsBackend::new())),
                PracticeSeal::Pending { reason },
            );
        },
    };
    let persona = identity
        .profile()
        .map(|profile| profile.0.clone())
        .unwrap_or_default();
    match SealedBackend::for_provider(FsBackend::new(), &identity) {
        // Adopting plaintext is the migration: a session written before sealing
        // was switched on is read once as it stands, and the next save seals it.
        Ok(sealed) => {
            let protection = identity.protection();
            eprintln!("[woodshed] practice sealed to persona {persona:?} ({protection})");
            (
                Box::new(Custodied {
                    inner: sealed.adopting_plaintext(),
                    identity,
                }),
                PracticeSeal::Sealed {
                    persona,
                    protection,
                },
            )
        },
        Err(error) => {
            eprintln!(
                "[woodshed] djinn released no sealing key for persona {persona:?}: {error}; \
                 practice is not saved"
            );
            (
                Box::new(PendingBackend::new(FsBackend::new())),
                PracticeSeal::Pending {
                    reason: format!("djinn released no sealing key for {persona:?}"),
                },
            )
        },
    }
}

/// The sealed store, answering only while djinn keeps the vault unlocked: a
/// lock revokes the released key (D11), so reads and writes stop with it.
struct Custodied<B> {
    inner: B,
    identity: CustodyIdentity,
}

impl<B> Custodied<B> {
    fn unlocked(&self) -> Result<(), StoreError> {
        match self.identity.is_locked() {
            true => Err(StoreError::Backend(
                "identity pending: the vault locked".into(),
            )),
            false => Ok(()),
        }
    }
}

#[async_trait]
impl<B: Backend + Send + Sync> Backend for Custodied<B> {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.unlocked()?;
        self.inner.get(key).await
    }

    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
        self.unlocked()?;
        self.inner.put(key, bytes).await
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.unlocked()?;
        self.inner.delete(key).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.unlocked()?;
        self.inner.list(prefix).await
    }

    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
        self.unlocked()?;
        self.inner.scan(start, end).await
    }

    async fn apply(&self, ops: &[WriteOp]) -> Result<(), StoreError> {
        self.unlocked()?;
        self.inner.apply(ops).await
    }
}

/// The store while the identity is pending (D12): reads what is already on
/// disk (a sealed session simply does not decode), and writes nothing. A
/// write is dropped, not refused, because the dispatch tail saves every beat
/// and Settings already says practice is not saved; it is said once here.
struct PendingBackend<B> {
    inner: B,
    said: AtomicBool,
}

impl<B> PendingBackend<B> {
    fn new(inner: B) -> Self {
        Self {
            inner,
            said: AtomicBool::new(false),
        }
    }

    fn dropped(&self) -> Result<(), StoreError> {
        if !self.said.swap(true, Ordering::Relaxed) {
            eprintln!("[woodshed] identity pending: practice is not being saved");
        }
        Ok(())
    }
}

#[async_trait]
impl<B: Backend + Send + Sync> Backend for PendingBackend<B> {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.get(key).await
    }

    async fn put(&self, _key: &str, _bytes: &[u8]) -> Result<(), StoreError> {
        self.dropped()
    }

    async fn delete(&self, _key: &str) -> Result<(), StoreError> {
        self.dropped()
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.list(prefix).await
    }

    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
        self.inner.scan(start, end).await
    }

    async fn apply(&self, _ops: &[WriteOp]) -> Result<(), StoreError> {
        self.dropped()
    }
}

pub struct FsBackend {
    /// `None` when the platform exposes no config dir. Persistence is silently
    /// disabled, matching woodshed-xilem's posture: a machine without a config
    /// dir still practices, it just does not remember.
    session: Option<PathBuf>,
    settings: Option<PathBuf>,
}

impl FsBackend {
    pub fn new() -> Self {
        // `WOODSHED_STATE` points the session at another file. A scenario run
        // sets it to a scratch profile: without it, an automated run would read
        // and then overwrite the real practice session.
        let settings_override = std::env::var("WOODSHED_SETTINGS").ok();
        if let Ok(path) = std::env::var("WOODSHED_STATE") {
            let state_path = PathBuf::from(path);
            return Self {
                settings: settings_override
                    .map(PathBuf::from)
                    .or_else(|| Some(state_path.with_extension("settings.json"))),
                session: Some(state_path),
            };
        }
        let (session, default_settings) = ProjectDirs::from("dev", "Woodshed", "Woodshed")
            .map(|dirs| {
                (
                    dirs.config_dir().join("genet-state.json"),
                    dirs.config_dir().join("genet-settings.json"),
                )
            })
            .unzip();
        Self {
            session,
            settings: settings_override.map(PathBuf::from).or(default_settings),
        }
    }

    /// Which file a slot lives in. Slot names are muniment's; the mapping to
    /// filenames is this host's, which is why an unknown slot has no file rather
    /// than a derived one: a typo should lose data loudly, not write somewhere
    /// nobody looks.
    fn path(&self, key: &str) -> Result<Option<PathBuf>, StoreError> {
        match key {
            "session" => Ok(self.session.clone()),
            "settings" => Ok(self.settings.clone()),
            _ => {
                let (namespace, digest) = key
                    .strip_prefix("loop-pcm/")
                    .and_then(|key| key.split_once('/'))
                    .ok_or_else(|| StoreError::Backend("Unknown Woodshed storage key.".into()))?;
                pcm_key(namespace, digest).map_err(StoreError::Backend)?;
                Ok(self
                    .capture_dir()
                    .map(|directory| directory.join(namespace).join(format!("{digest}.bin"))))
            },
        }
    }

    fn capture_dir(&self) -> Option<PathBuf> {
        self.session
            .as_ref()
            .map(|path| path.with_extension("loop-pcm"))
    }

    /// Exact host-owned destinations; Tabard owns normalization and collision
    /// protection when its export dialog is embedded in this application.
    pub fn protected_export_paths(&self) -> Vec<PathBuf> {
        self.session
            .iter()
            .chain(self.settings.iter())
            .cloned()
            .collect()
    }

    pub fn protected_export_directories(&self) -> Vec<PathBuf> {
        self.capture_dir().into_iter().collect()
    }
}

impl Default for FsBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Personae's pinned XChaCha20-Poly1305 envelope adds a 24-byte nonce and
/// 16-byte tag. Bound encrypted files before SealedBackend allocates plaintext.
const MAX_CAPTURE_FILE_BYTES: usize = MAX_PCM_BYTES + 40;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Commit one file through a synced sibling: rename for mutable slots, a
/// no-clobber hard link for immutable blobs. Failed publication retains old data.
/// This does not make a set of companion/session files one atomic transaction.
fn atomic_write(path: &Path, bytes: &[u8], immutable: bool) -> Result<(), StoreError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| StoreError::Backend(error.to_string()))?;
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::Backend("Storage destination has no filename.".into()))?;
    for _ in 0..32 {
        let mut temporary_name = std::ffi::OsString::from(".");
        temporary_name.push(name);
        temporary_name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = parent.join(temporary_name);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StoreError::Backend(error.to_string())),
        };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            if immutable {
                std::fs::hard_link(&temporary, path)
            } else {
                std::fs::rename(&temporary, path)
            }
        })();
        let _ = std::fs::remove_file(&temporary);
        if result.is_ok() {
            // A publication already committed cannot be rolled back by a
            // directory-sync error. Match muniment's zip backend's best effort.
            #[cfg(unix)]
            if let Ok(directory) = std::fs::File::open(parent) {
                let _ = directory.sync_all();
            }
        }
        return result.map_err(|error| StoreError::Backend(error.to_string()));
    }
    Err(StoreError::Backend(
        "Could not reserve a sibling storage temporary file.".into(),
    ))
}

#[async_trait]
impl Backend for FsBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(path) = self.path(key)? else {
            return Ok(None);
        };
        if key.starts_with("loop-pcm/") {
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(StoreError::Backend(error.to_string())),
            };
            let length = file
                .metadata()
                .map_err(|error| StoreError::Backend(error.to_string()))?
                .len();
            if length > MAX_CAPTURE_FILE_BYTES as u64 {
                return Err(StoreError::Backend(
                    "Loop PCM file exceeds its bounded byte budget.".into(),
                ));
            }
            let mut bytes = Vec::with_capacity(length as usize);
            file.take(MAX_CAPTURE_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| StoreError::Backend(error.to_string()))?;
            if bytes.len() > MAX_CAPTURE_FILE_BYTES {
                return Err(StoreError::Backend(
                    "Loop PCM file grew beyond its bounded byte budget.".into(),
                ));
            }
            return Ok(Some(bytes));
        }
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::Backend(error.to_string())),
        }
    }

    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let Some(path) = self.path(key)? else {
            return Ok(());
        };
        if key.starts_with("loop-pcm/") && bytes.len() > MAX_CAPTURE_FILE_BYTES {
            return Err(StoreError::Backend(
                "Loop PCM write exceeds its bounded byte budget.".into(),
            ));
        }
        atomic_write(&path, bytes, key.starts_with("loop-pcm/"))
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let Some(path) = self.path(key)? else {
            return Ok(());
        };
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StoreError::Backend(error.to_string())),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let mut keys = Vec::new();
        for key in ["session", "settings"] {
            if key.starts_with(prefix) && self.path(key)?.is_some_and(|path| path.exists()) {
                keys.push(key.to_string());
            }
        }
        if let Some(directory) = self.capture_dir() {
            let entries = match std::fs::read_dir(directory) {
                Ok(entries) => Some(entries),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(StoreError::Backend(error.to_string())),
            };
            if let Some(entries) = entries {
                for namespace in entries {
                    let namespace =
                        namespace.map_err(|error| StoreError::Backend(error.to_string()))?;
                    if !namespace
                        .file_type()
                        .map_err(|error| StoreError::Backend(error.to_string()))?
                        .is_dir()
                    {
                        continue;
                    }
                    let name = namespace.file_name();
                    let Some(name) = name.to_str() else {
                        continue;
                    };
                    if validate_namespace(name).is_err() {
                        continue;
                    }
                    for entry in std::fs::read_dir(namespace.path())
                        .map_err(|error| StoreError::Backend(error.to_string()))?
                    {
                        let entry =
                            entry.map_err(|error| StoreError::Backend(error.to_string()))?;
                        if !entry
                            .file_type()
                            .map_err(|error| StoreError::Backend(error.to_string()))?
                            .is_file()
                        {
                            continue;
                        }
                        let file = entry.file_name();
                        let Some(digest) = file.to_str().and_then(|file| file.strip_suffix(".bin"))
                        else {
                            continue;
                        };
                        if let Ok(key) = pcm_key(name, digest) {
                            if key.starts_with(prefix) {
                                keys.push(key);
                            }
                        }
                    }
                }
            }
        }
        Ok(keys)
    }

    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
        let mut keys = self.list("").await?;
        keys.retain(|key| key.as_str() >= start && key.as_str() < end);
        keys.sort();
        Ok(keys)
    }

    /// Files written in order, each individually replaced atomically.
    ///
    /// This is not a multi-file transaction. Capture persistence writes and
    /// verifies blobs before committing its session reference separately.
    async fn apply(&self, ops: &[WriteOp]) -> Result<(), StoreError> {
        for op in ops {
            match op {
                WriteOp::Put { key, value } => self.put(key, value).await?,
                WriteOp::Delete { key } => self.delete(key).await?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use woodshed_core::loop_capture::{LoopCaptureCodec, LoopCaptureRef, encode_pcm, pcm_digest};
    use woodshed_core::storage::PersistedSession;
    const NAMESPACE: &str = "0123456789abcdef0123456789abcdef";
    const OTHER_NAMESPACE: &str = "fedcba9876543210fedcba9876543210";

    fn backend(directory: &Path) -> FsBackend {
        FsBackend {
            session: Some(directory.join("practice.json")),
            settings: Some(directory.join("settings.json")),
        }
    }

    fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => {
                panic!("local filesystem backend unexpectedly awaited external work")
            },
        }
    }

    fn reference(digest: String, frames: usize) -> LoopCaptureRef {
        LoopCaptureRef {
            id: woodshed_core::loop_plan::LoopSegmentId {
                set: woodshed_core::working_sets::WorkingSetId(3),
                card: woodshedding::rehearsal::CardId(7),
            },
            bpm: 83.75,
            beats: 7,
            bars: 3,
            sample_rate: 48_000,
            frames,
            codec: LoopCaptureCodec::LegacyRawF32V1,
            digest,
        }
    }

    #[test]
    fn filesystem_companion_reopens_exact_pcm_after_session_reference_commit() {
        let directory = tempfile::tempdir().unwrap();
        let store = SessionStore::new(backend(directory.path()));
        let samples = [0.0, -0.0, 0.25, -1.5];
        let (digest, frames) = store
            .try_write_capture(NAMESPACE, 48_000, &samples)
            .unwrap();
        let mut session = PersistedSession::default();
        session
            .loop_captures
            .takes
            .push(reference(digest.clone(), frames));
        session.loop_captures.namespace = Some(NAMESPACE.into());
        store
            .try_save(&serde_json::to_string(&session).unwrap())
            .unwrap();
        let reopened = SessionStore::new(backend(directory.path()));
        let persisted = woodshed_core::storage::decode_session(&reopened.load().unwrap())
            .unwrap()
            .session;
        assert_eq!(persisted.loop_captures, session.loop_captures);
        let decoded = reopened
            .try_read_capture(NAMESPACE, &persisted.loop_captures.takes[0])
            .unwrap();
        assert_eq!(
            decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            samples.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(
            std::fs::read(
                directory
                    .path()
                    .join("practice.loop-pcm")
                    .join(NAMESPACE)
                    .join(format!("{digest}.bin"))
            )
            .unwrap(),
            encode_pcm(48_000, &samples).unwrap()
        );
        assert_eq!(
            ready(reopened.backend().list("loop-pcm/")).unwrap(),
            vec![pcm_key(NAMESPACE, &digest).unwrap()]
        );
        assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        reopened.save_settings("first");
        reopened.save_settings("second");
        assert_eq!(reopened.load_settings().as_deref(), Some("second"));
    }

    #[test]
    fn filesystem_rejects_unknown_and_path_shaped_keys_without_touching_session() {
        let directory = tempfile::tempdir().unwrap();
        let backend = backend(directory.path());
        ready(backend.put("session", b"original")).unwrap();
        for key in [
            "unknown".into(),
            "loop-pcm/../practice.json".into(),
            format!("loop-pcm/{}", "A".repeat(64)),
            format!("loop-pcm/{}.bin", "a".repeat(64)),
            format!("loop-pcm/{NAMESPACE}/{}.bin", "a".repeat(64)),
            format!("loop-pcm/{NAMESPACE}/{}/../session", "a".repeat(64)),
        ] {
            assert!(ready(backend.get(&key)).is_err());
            assert!(ready(backend.put(&key, b"overwrite")).is_err());
            assert!(ready(backend.delete(&key)).is_err());
        }
        assert_eq!(ready(backend.get("session")).unwrap().unwrap(), b"original");
    }

    #[test]
    fn filesystem_bounds_reads_and_reports_corrupt_or_truncated_companions() {
        let directory = tempfile::tempdir().unwrap();
        let backend = backend(directory.path());
        let bytes = encode_pcm(48_000, &[0.25]).unwrap();
        let reference = reference(pcm_digest(&bytes), 1);
        let path = backend
            .path(&pcm_key(NAMESPACE, &reference.digest).unwrap())
            .unwrap()
            .unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_CAPTURE_FILE_BYTES as u64 + 1).unwrap();
        let store = SessionStore::new(backend);
        assert!(
            store
                .try_read_capture(NAMESPACE, &reference)
                .unwrap_err()
                .contains("byte budget")
        );
        std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
        assert!(store.try_read_capture(NAMESPACE, &reference).is_err());
        let mut corrupt = bytes;
        *corrupt.last_mut().unwrap() ^= 1;
        std::fs::write(&path, &corrupt).unwrap();
        assert!(
            store
                .try_read_capture(NAMESPACE, &reference)
                .unwrap_err()
                .contains("integrity")
        );
        assert!(store.try_write_capture(NAMESPACE, 48_000, &[0.25]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), corrupt);
        let (digest, frames) = store
            .try_write_capture(OTHER_NAMESPACE, 48_000, &[0.25])
            .unwrap();
        assert_eq!(
            store
                .try_read_capture(OTHER_NAMESPACE, &self::reference(digest, frames))
                .unwrap(),
            vec![0.25]
        );
        assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    }

    #[test]
    fn failed_companion_write_retains_previous_session_and_failed_rename_cleans_temp() {
        let directory = tempfile::tempdir().unwrap();
        let store = SessionStore::new(backend(directory.path()));
        store.try_save("previous session").unwrap();
        std::fs::write(
            directory.path().join("practice.loop-pcm"),
            b"directory collision",
        )
        .unwrap();
        assert!(store.try_write_capture(NAMESPACE, 48_000, &[0.25]).is_err());
        assert_eq!(store.load().as_deref(), Some("previous session"));
        let occupied = directory.path().join("occupied.json");
        std::fs::create_dir(&occupied).unwrap();
        std::fs::write(occupied.join("previous"), b"retained").unwrap();
        assert!(atomic_write(&occupied, b"replacement", false).is_err());
        assert_eq!(
            std::fs::read(occupied.join("previous")).unwrap(),
            b"retained"
        );
        assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn sealed_filesystem_blobs_and_pending_write_drop_keep_custody_semantics() {
        let directory = tempfile::tempdir().unwrap();
        let store = SessionStore::new(SealedBackend::new(backend(directory.path()), [3; 32]));
        let (digest, frames) = store.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap();
        assert_eq!(
            store
                .try_read_capture(NAMESPACE, &reference(digest.clone(), frames))
                .unwrap(),
            vec![0.25]
        );
        let raw = std::fs::read(
            directory
                .path()
                .join("practice.loop-pcm")
                .join(NAMESPACE)
                .join(format!("{digest}.bin")),
        )
        .unwrap();
        assert_eq!(raw.len(), encode_pcm(48_000, &[0.25]).unwrap().len() + 40);
        assert_ne!(&raw[..8], b"WSHPCM\0\0");
        let pending = SessionStore::new(PendingBackend::new(backend(directory.path())));
        assert!(
            pending
                .try_write_capture(NAMESPACE, 48_000, &[0.5])
                .is_err()
        );
        assert!(pending.try_save("must remain unsaved").is_err());
        assert!(!directory.path().join("practice.json").exists());
        assert_eq!(
            ready(pending.backend().list("loop-pcm/")).unwrap(),
            vec![pcm_key(NAMESPACE, &digest).unwrap()]
        );
    }

    #[test]
    fn two_personas_share_pcm_only_through_distinct_artifacts_and_cannot_clobber_ciphertext() {
        let directory = tempfile::tempdir().unwrap();
        let first = SessionStore::new(SealedBackend::new(backend(directory.path()), [1; 32]));
        let second = SessionStore::new(SealedBackend::new(backend(directory.path()), [2; 32]));
        let (digest, frames) = first.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap();
        let path = directory
            .path()
            .join("practice.loop-pcm")
            .join(NAMESPACE)
            .join(format!("{digest}.bin"));
        let original = std::fs::read(&path).unwrap();
        assert_eq!(
            first.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap(),
            (digest.clone(), frames)
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "identical readable PCM reuses its existing sealed blob"
        );
        assert!(
            second
                .try_write_capture(NAMESPACE, 48_000, &[0.25])
                .is_err()
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "inaccessible ciphertext is never overwritten"
        );
        let other = second
            .try_write_capture(OTHER_NAMESPACE, 48_000, &[0.25])
            .unwrap();
        assert_eq!(other, (digest.clone(), frames));
        let reference = reference(digest.clone(), frames);
        assert_eq!(
            first.try_read_capture(NAMESPACE, &reference).unwrap(),
            vec![0.25]
        );
        assert_eq!(
            second
                .try_read_capture(OTHER_NAMESPACE, &reference)
                .unwrap(),
            vec![0.25]
        );
        assert!(second.try_read_capture(NAMESPACE, &reference).is_err());
        assert!(first.try_read_capture(OTHER_NAMESPACE, &reference).is_err());
        // Even a directly addressed write must preserve the immutable blob.
        assert!(
            ready(backend(directory.path()).put(
                &pcm_key(NAMESPACE, &digest).unwrap(),
                b"corrupt replacement"
            ))
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

#[cfg(test)]
mod dr_c_receipts {
    use super::*;
    #[test]
    #[ignore = "run mere/scripts/dr_c_receipts.py against the isolated receipt keeper"]
    fn dr_c_woodshed_identity_stays_pending() {
        let mode = std::env::var("DR_C_RECEIPT_MODE").expect("receipt mode");
        assert!(mode == "absent" || mode == "locked");
        if mode == "locked" {
            let mut client = graphshell_endpoint::door::custody_client::BlockingCustodyClient::open(
                graphshell_endpoint::door::app_admission::AppId::new(CUSTODY_APP),
            )
            .unwrap();
            assert_eq!(
                client.status().unwrap().lock,
                graphshell_endpoint::door::identity::VaultLockView::Locked
            );
            assert!(!client.roster().unwrap().entries.is_empty());
        }
        let state = PathBuf::from(std::env::var("WOODSHED_STATE").expect("isolated state file"));
        std::fs::write(&state, b"already-public practice").unwrap();
        let (backend, seal) = open_backend(None);
        assert!(
            matches!(seal, PracticeSeal::Pending { .. }),
            "Woodshed must have no sealing-key fallback"
        );
        let mut reads_and_writes = std::pin::pin!(async {
            assert_eq!(
                backend.get("session").await.unwrap().unwrap(),
                b"already-public practice"
            );
            backend
                .put("session", b"must never be written in the clear")
                .await
                .unwrap();
            backend.delete("session").await.unwrap();
            backend
                .apply(&[WriteOp::Put {
                    key: "session".into(),
                    value: b"another forbidden cleartext write".to_vec(),
                }])
                .await
                .unwrap();
        });
        use std::future::Future;
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(
            reads_and_writes.as_mut().poll(&mut context).is_ready(),
            "the local file backend completes without an async runtime"
        );
        assert_eq!(
            std::fs::read(&state).unwrap(),
            b"already-public practice",
            "pending must leave public bytes unchanged"
        );
        let (_, chosen) = open_backend(Some(&ProfileId("default".into())));
        assert!(matches!(chosen, PracticeSeal::Pending { .. }));
    }
}
