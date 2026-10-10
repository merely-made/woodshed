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

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use directories::ProjectDirs;
use graphshell::native::custody::CustodyRefusal;
use graphshell::native::custody_client::CustodyClientError;
use graphshell::native::custody_identity::CustodyIdentity;
use muniment::backend::WriteOp;
use muniment::{Backend, StoreError};
use personae::ProfileId;
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
    fn path(&self, key: &str) -> Option<&PathBuf> {
        match key {
            "session" => self.session.as_ref(),
            "settings" => self.settings.as_ref(),
            _ => None,
        }
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
}

impl Default for FsBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for FsBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(path) = self.path(key) else {
            return Ok(None);
        };
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::Backend(error.to_string())),
        }
    }

    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let Some(path) = self.path(key) else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, bytes).map_err(|error| StoreError::Backend(error.to_string()))
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let Some(path) = self.path(key) else {
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
            if key.starts_with(prefix) && self.path(key).is_some_and(|path| path.exists()) {
                keys.push(key.to_string());
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

    /// Two files, written in order.
    ///
    /// Not atomic, and it does not pretend to be: this host has a fixed two-slot
    /// key space and nothing here writes a pair that must land together. A
    /// backend whose consumers need real batches wants redb, which muniment
    /// already ships.
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
