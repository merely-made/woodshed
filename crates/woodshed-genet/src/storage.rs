//! Persona-owned desktop persistence. The application root contains separate
//! cryptographic-owner stores; local profile labels only route custody calls.
//! Normal opens authenticate an immutable sealed owner record before exposing
//! session, settings or capture slots. Pending custody sees an empty read-only
//! memory backend and never reads a singleton or another persona's files.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use async_trait::async_trait;
use directories::ProjectDirs;
use graphshell_endpoint::door::custody::CustodyRefusal;
use graphshell_endpoint::door::custody_client::CustodyClientError;
use graphshell_endpoint::door::custody_identity::CustodyIdentity;
use muniment::backend::WriteOp;
use muniment::{Backend, MemoryBackend, StoreError};
use personae::{IdentityProvider, ProfileId};
use serde::{Deserialize, Serialize};
use woodshed_core::loop_capture::{MAX_PCM_BYTES, pcm_key, validate_namespace};
use woodshed_core::sealed_backend::SealedBackend;
use woodshed_core::storage::SessionStore;
use woodshed_views::persona::PracticeSeal;

/// The name Woodshed is admitted to djinn's custody route under.
pub const CUSTODY_APP: &str = "woodshed";

/// A successful custody/store open retains the explicit candidate provider.
/// Its family choice is committed by the caller only after content admission.
pub struct OpenedStore {
    pub storage: SessionStore<HostBackend>,
    pub seal: PracticeSeal,
    pub binding: Option<StoreBinding>,
}

/// The store woodshed practices over, decided at startup.
pub type HostBackend = Box<dyn Backend + Send + Sync>;

#[derive(Clone)]
pub struct StoreBinding {
    root: PathBuf,
    owner: String,
    authority: BindingAuthority,
}

#[derive(Clone)]
enum BindingAuthority {
    Custody(Arc<CustodyIdentity>),
    #[cfg(test)]
    Test {
        profile: ProfileId,
    },
}

impl StoreBinding {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn directory(&self) -> PathBuf {
        persona_directory(&self.root, &self.owner)
    }

    /// Change the family choice using the already-admitted candidate provider.
    /// A different target requires a different explicit custody connection.
    pub fn choose_profile(&self, profile: &ProfileId) -> Result<(), String> {
        match &self.authority {
            BindingAuthority::Custody(identity) => {
                if identity.profile() != Some(profile) {
                    return Err(
                        "The admitted persona differs from the requested family choice.".into(),
                    );
                }
                self.validate_choice_identity(identity, profile)?;
                identity.choose_profile(profile.clone()).map_err(|error| {
                    format!("The family persona choice could not be confirmed ({}). The family choice may already have changed; reopen or retry the selected persona.", pending(error))
                })?;
                self.validate_choice_identity(identity, profile).map_err(|error| {
                    format!("The family persona choice was acknowledged, but its identity could not be confirmed ({error}). The family choice may already have changed; reopen or retry the selected persona.")
                })
            },
            #[cfg(test)]
            BindingAuthority::Test { profile: connected } => {
                if connected == profile {
                    Ok(())
                } else {
                    Err("The admitted persona differs from the requested family choice.".into())
                }
            },
        }
    }

    fn validate_choice_identity(
        &self,
        identity: &CustodyIdentity,
        profile: &ProfileId,
    ) -> Result<(), String> {
        let status = identity.status().map_err(pending)?;
        admit_choice_identity(
            ChoiceIdentity {
                profile: status.profile,
                unlocked: status.lock == dramatis::view::VaultLockView::Unlocked
                    && !identity.is_locked(),
                public_key: status.persona_public_key,
            },
            profile,
            identity.master_public_key().to_bytes(),
            &self.owner,
        )
    }

    /// Local labels can be reused for a new seed. Reconnect explicitly before
    /// deciding that selecting a profile is a no-op for the installed store.
    pub fn matches_current(&self, profile: &ProfileId) -> Result<bool, String> {
        match &self.authority {
            BindingAuthority::Custody(_) => {
                let identity = connect(Some(profile))?;
                validate_profile(&identity, Some(profile))?;
                Ok(owner_for_provider(&identity) == self.owner)
            },
            #[cfg(test)]
            BindingAuthority::Test { profile: connected } => Ok(connected == profile),
        }
    }

    #[cfg(test)]
    pub fn for_test(root: PathBuf, owner: String, profile: ProfileId) -> Self {
        Self {
            root,
            owner,
            authority: BindingAuthority::Test { profile },
        }
    }
}

/// Resolve the whole application root once. Obsolete exact-file overrides are
/// refused, including empty values, so isolation can never fall back silently.
pub fn configured_root() -> Result<PathBuf, String> {
    let obsolete: Vec<_> = ["WOODSHED_STATE", "WOODSHED_SETTINGS"]
        .into_iter()
        .filter(|name| std::env::var_os(name).is_some())
        .collect();
    if !obsolete.is_empty() {
        return Err(format!(
            "{} is no longer supported. Unset the exact-file overrides and set WOODSHED_DATA_DIR to an isolated application directory.",
            obsolete.join(" and ")
        ));
    }
    let root = match std::env::var_os("WOODSHED_DATA_DIR") {
        Some(path) if path.is_empty() => {
            return Err("WOODSHED_DATA_DIR is empty. Set it to an application directory or unset it to use the normal directory.".into());
        },
        Some(path) => PathBuf::from(path),
        None => ProjectDirs::from("dev", "Woodshed", "Woodshed")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .ok_or("No application directory is available. Set WOODSHED_DATA_DIR to a writable directory.")?,
    };
    absolute_root(&root)
}

fn absolute_root(root: &Path) -> Result<PathBuf, String> {
    if root.as_os_str().is_empty() {
        return Err("The Woodshed application directory is empty.".into());
    }
    crate::paths::absolute(root)
        .map_err(|error| format!("Cannot resolve the Woodshed application directory: {error}"))
}

pub fn open_store_as(profile: Option<&ProfileId>) -> OpenedStore {
    match configured_root() {
        Ok(root) => open_store_at(&root, profile),
        Err(reason) => pending_store(reason),
    }
}

/// Open the explicitly selected candidate without changing the family choice.
/// Failure exposes no disk-backed fallback, even while the vault is pending.
pub fn open_store_at(root: &Path, profile: Option<&ProfileId>) -> OpenedStore {
    match try_open_store(root, profile) {
        Ok(opened) => opened,
        Err(reason) => pending_store(reason),
    }
}

fn try_open_store(root: &Path, profile: Option<&ProfileId>) -> Result<OpenedStore, String> {
    let root = absolute_root(root)?;
    let identity = Arc::new(connect(profile)?);
    let persona = validate_profile(&identity, profile)?;
    let owner = owner_for_provider(identity.as_ref());
    let directory = persona_directory(&root, &owner);
    let sealed = SealedBackend::for_provider(FsBackend::at(&directory), identity.as_ref())
        .map_err(|error| format!("djinn released no sealing key for {persona:?}: {error}"))?;
    let backend = Custodied {
        inner: sealed,
        identity: Arc::clone(&identity),
    };
    initialize_owner(&backend, &directory, &owner)?;
    let protection = identity.protection();
    let storage = SessionStore::new(Box::new(backend) as HostBackend);
    let binding = StoreBinding {
        root,
        owner,
        authority: BindingAuthority::Custody(identity),
    };
    Ok(OpenedStore {
        storage,
        seal: PracticeSeal::Sealed {
            persona,
            protection,
        },
        binding: Some(binding),
    })
}

fn pending_store(reason: String) -> OpenedStore {
    OpenedStore {
        storage: SessionStore::new(Box::new(PendingBackend::new()) as HostBackend),
        seal: PracticeSeal::Pending { reason },
        binding: None,
    }
}

fn connect(profile: Option<&ProfileId>) -> Result<CustodyIdentity, String> {
    CustodyIdentity::connect_as(CUSTODY_APP, profile.cloned()).map_err(pending)
}

fn validate_profile(
    identity: &CustodyIdentity,
    selected: Option<&ProfileId>,
) -> Result<String, String> {
    let profile = identity
        .profile()
        .ok_or("djinn opened no canonical persona profile")?;
    if profile.0.trim().is_empty() || profile.0 != profile.0.trim() {
        return Err("djinn opened an empty or non-canonical persona profile".into());
    }
    if selected.is_some_and(|selected| selected != profile) {
        return Err("djinn opened a different persona from the selected profile".into());
    }
    Ok(profile.0.clone())
}

fn owner_for_provider(provider: &dyn IdentityProvider) -> String {
    owner_for_public_key(&provider.master_public_key().to_bytes())
}

fn owner_for_public_key(public_key: &[u8; 32]) -> String {
    blake3::derive_key("woodshed.persona-store.owner.v1", public_key)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Public facts read immediately around the mutable family-choice RPC. A local
/// profile string is insufficient when its seed can have changed since preflight.
struct ChoiceIdentity {
    profile: Option<ProfileId>,
    unlocked: bool,
    public_key: Option<[u8; 32]>,
}

fn admit_choice_identity(
    current: ChoiceIdentity,
    expected_profile: &ProfileId,
    retained_public_key: [u8; 32],
    retained_owner: &str,
) -> Result<(), String> {
    if !current.unlocked {
        return Err("The vault locked while the persona choice was being confirmed.".into());
    }
    if current.profile.as_ref() != Some(expected_profile) {
        return Err("The current custody profile differs from the admitted persona. Reopen the selected persona.".into());
    }
    let current_public_key = current
        .public_key
        .ok_or("Custody released no public identity for the selected persona.")?;
    if current_public_key != retained_public_key
        || owner_for_public_key(&current_public_key) != retained_owner
    {
        return Err("The selected profile now belongs to a different cryptographic persona. Reopen the selected persona.".into());
    }
    Ok(())
}

fn persona_directory(root: &Path, owner: &str) -> PathBuf {
    root.join("personas").join("v1").join(owner)
}

const OWNER_VERSION: u32 = 1;
const OWNER_PURPOSE: &str = "woodshed.persona-practice-store";
const MAX_OWNER_FILE_BYTES: usize = 4096;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerManifest {
    version: u32,
    purpose: String,
    owner: String,
}

fn admit_owner(bytes: &[u8], owner: &str) -> Result<(), String> {
    let manifest: OwnerManifest = serde_json::from_slice(bytes)
        .map_err(|error| format!("The persona-store owner record could not be decoded: {error}"))?;
    if manifest.version != OWNER_VERSION || manifest.purpose != OWNER_PURPOSE {
        return Err("The persona-store owner record has an unsupported version or purpose.".into());
    }
    if manifest.owner != owner {
        return Err(
            "The persona-store owner record belongs to another cryptographic persona.".into(),
        );
    }
    Ok(())
}

/// Only a genuinely empty directory can acquire an owner. Incomplete or foreign
/// files are never silently adopted. The owner slot publishes without clobbering.
fn initialize_owner<B: Backend + Sync>(
    backend: &B,
    directory: &Path,
    owner: &str,
) -> Result<(), String> {
    if let Some(bytes) = pollster::block_on(backend.get("owner")).map_err(|error| {
        format!("The persona-store owner record could not be authenticated: {error}")
    })? {
        return admit_owner(&bytes, owner);
    }
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Cannot create the persona-store directory: {error}"))?;
    if std::fs::read_dir(directory)
        .map_err(|error| format!("Cannot inspect the persona-store directory: {error}"))?
        .next()
        .transpose()
        .map_err(|error| format!("Cannot inspect the persona-store directory: {error}"))?
        .is_some()
    {
        // Another opener may have published the immutable record between the
        // first read and directory inspection. Authenticate that winner before
        // classifying this as an unowned partial directory.
        if let Some(bytes) = pollster::block_on(backend.get("owner")).map_err(|error| {
            format!("The persona-store owner record could not be authenticated: {error}")
        })? {
            return admit_owner(&bytes, owner);
        }
        return Err("The persona-store directory contains files without an authenticated owner record. Choose an empty WOODSHED_DATA_DIR or repair the incomplete store.".into());
    }
    let manifest = serde_json::to_vec(&OwnerManifest {
        version: OWNER_VERSION,
        purpose: OWNER_PURPOSE.into(),
        owner: owner.into(),
    })
    .map_err(|error| format!("Cannot encode the persona-store owner record: {error}"))?;
    let written = pollster::block_on(backend.put("owner", &manifest));
    // A concurrent opener may have won the immutable publication. Accept only
    // an authenticated readback with exactly this schema, purpose and owner.
    let readback = pollster::block_on(backend.get("owner")).map_err(|error| {
        format!("The persona-store owner record could not be verified: {error}")
    })?;
    match readback {
        Some(bytes) => admit_owner(&bytes, owner),
        None => Err(format!(
            "The persona-store owner record was not published: {}",
            written.err().map_or_else(
                || "readback found no record".into(),
                |error| error.to_string()
            )
        )),
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

/// The sealed store, answering only while djinn keeps the vault unlocked: a
/// lock revokes the released key (D11), so reads and writes stop with it.
struct Custodied<B> {
    inner: B,
    identity: Arc<CustodyIdentity>,
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

/// Pending custody has an empty memory view and refuses every mutation.
/// It cannot leak a legacy singleton or another persona's plaintext into UI.
struct PendingBackend {
    inner: MemoryBackend,
    said: AtomicBool,
}

impl PendingBackend {
    fn new() -> Self {
        Self {
            inner: MemoryBackend::new(),
            said: AtomicBool::new(false),
        }
    }

    fn dropped(&self) -> Result<(), StoreError> {
        if !self.said.swap(true, Ordering::Relaxed) {
            eprintln!("[woodshed] identity pending: practice is not being saved");
        }
        Err(StoreError::Backend(
            "Identity pending: this session is not being saved.".into(),
        ))
    }
}

#[async_trait]
impl Backend for PendingBackend {
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

/// The exact destinations for one resolved persona store. There is no default
/// constructor: callers must use the installed binding's directory.
pub struct FsBackend {
    directory: PathBuf,
}

impl FsBackend {
    pub fn at(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
        }
    }

    fn path(&self, key: &str) -> Result<Option<PathBuf>, StoreError> {
        match key {
            "owner" | "session" | "settings" => Ok(Some(self.directory.join(key))),
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
        Some(self.directory.join("loop-pcm"))
    }

    pub fn protected_export_paths(&self) -> Vec<PathBuf> {
        ["owner", "session", "settings"]
            .map(|name| self.directory.join(name))
            .into()
    }

    /// Protect the entire store, including unknown future slots and temporaries.
    pub fn protected_export_directories(&self) -> Vec<PathBuf> {
        vec![self.directory.clone()]
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
        if key.starts_with("loop-pcm/") || key == "owner" {
            let maximum = if key == "owner" {
                MAX_OWNER_FILE_BYTES
            } else {
                MAX_CAPTURE_FILE_BYTES
            };
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(StoreError::Backend(error.to_string())),
            };
            let length = file
                .metadata()
                .map_err(|error| StoreError::Backend(error.to_string()))?
                .len();
            if length > maximum as u64 {
                return Err(StoreError::Backend(format!(
                    "{} file exceeds its bounded byte budget.",
                    if key == "owner" {
                        "Owner record"
                    } else {
                        "Loop PCM"
                    }
                )));
            }
            let mut bytes = Vec::with_capacity(length as usize);
            file.take(maximum as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| StoreError::Backend(error.to_string()))?;
            if bytes.len() > maximum {
                return Err(StoreError::Backend(format!(
                    "{} file grew beyond its bounded byte budget.",
                    if key == "owner" {
                        "Owner record"
                    } else {
                        "Loop PCM"
                    }
                )));
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
        if key == "owner" && bytes.len() > MAX_OWNER_FILE_BYTES {
            return Err(StoreError::Backend(
                "Owner record exceeds its bounded byte budget.".into(),
            ));
        }
        atomic_write(&path, bytes, key.starts_with("loop-pcm/") || key == "owner")
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        if key == "owner" {
            return Err(StoreError::Backend(
                "The persona-store owner record is immutable.".into(),
            ));
        }
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
        for key in ["owner", "session", "settings"] {
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
        FsBackend::at(directory)
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
            seconds: None,
            clock: None,
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
                    .join("loop-pcm")
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
        std::fs::write(directory.path().join("loop-pcm"), b"directory collision").unwrap();
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
                .join("loop-pcm")
                .join(NAMESPACE)
                .join(format!("{digest}.bin")),
        )
        .unwrap();
        assert_eq!(raw.len(), encode_pcm(48_000, &[0.25]).unwrap().len() + 40);
        assert_ne!(&raw[..8], b"WSHPCM\0\0");
        let pending = SessionStore::new(PendingBackend::new());
        assert!(
            pending
                .try_write_capture(NAMESPACE, 48_000, &[0.5])
                .is_err()
        );
        assert!(pending.try_save("must remain unsaved").is_err());
        assert!(!directory.path().join("session").exists());
        assert_eq!(
            ready(pending.backend().list("loop-pcm/")).unwrap(),
            Vec::<String>::new()
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
            .join("loop-pcm")
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
mod persona_store_tests {
    use super::*;
    use personae::InMemoryProvider;
    use woodshed_views::stage::UiState;

    fn sealed_at(
        root: &Path,
        provider: &InMemoryProvider,
    ) -> (SealedBackend<FsBackend>, String, PathBuf) {
        let owner = owner_for_provider(provider);
        let directory = persona_directory(root, &owner);
        let backend = SealedBackend::for_provider(FsBackend::at(&directory), provider).unwrap();
        (backend, owner, directory)
    }

    fn open_fixture(
        root: &Path,
        provider: &InMemoryProvider,
    ) -> SessionStore<SealedBackend<FsBackend>> {
        let (backend, owner, directory) = sealed_at(root, provider);
        initialize_owner(&backend, &directory, &owner).unwrap();
        SessionStore::new(backend)
    }

    #[test]
    fn cryptographic_personas_isolate_session_settings_and_pcm_across_fresh_handles() {
        let root = tempfile::tempdir().unwrap();
        let first_provider = InMemoryProvider::from_seed([1; 32]);
        let second_provider = InMemoryProvider::from_seed([2; 32]);
        let first_owner = owner_for_provider(&first_provider);
        let second_owner = owner_for_provider(&second_provider);
        assert_ne!(first_owner, second_owner);
        // Both vaults can use this local label. It never participates in paths.
        let label = ProfileId("default".into());
        let first_binding =
            StoreBinding::for_test(root.path().into(), first_owner.clone(), label.clone());
        let second_binding =
            StoreBinding::for_test(root.path().into(), second_owner.clone(), label);
        assert_ne!(first_binding.directory(), second_binding.directory());
        let first = open_fixture(root.path(), &first_provider);
        let second = open_fixture(root.path(), &second_provider);
        first.try_save("first practice").unwrap();
        first.try_save_settings("first settings").unwrap();
        second.try_save("second practice").unwrap();
        second.try_save_settings("second settings").unwrap();
        let namespace = "0123456789abcdef0123456789abcdef";
        let first_pcm = [0.25, -0.0];
        let second_pcm = [0.75, 0.0];
        let (first_digest, _) = first
            .try_write_capture(namespace, 48_000, &first_pcm)
            .unwrap();
        let (second_digest, _) = second
            .try_write_capture(namespace, 48_000, &second_pcm)
            .unwrap();
        drop(first);
        drop(second);
        let first = open_fixture(root.path(), &InMemoryProvider::from_seed([1; 32]));
        let second = open_fixture(root.path(), &InMemoryProvider::from_seed([2; 32]));
        assert_eq!(first.try_load().unwrap().as_deref(), Some("first practice"));
        assert_eq!(
            second.try_load().unwrap().as_deref(),
            Some("second practice")
        );
        assert_eq!(first.load_settings().as_deref(), Some("first settings"));
        assert_eq!(second.load_settings().as_deref(), Some("second settings"));
        for (store, digest, expected) in [
            (&first, first_digest, first_pcm),
            (&second, second_digest, second_pcm),
        ] {
            let reference = woodshed_core::loop_capture::LoopCaptureRef {
                id: woodshed_core::loop_plan::LoopSegmentId {
                    set: woodshed_core::working_sets::WorkingSetId(3),
                    card: woodshedding::rehearsal::CardId(7),
                },
                bpm: 120.0,
                beats: 4,
                bars: 1,
                sample_rate: 48_000,
                frames: 2,
                digest,
                codec: woodshed_core::loop_capture::LoopCaptureCodec::LegacyRawF32V1,
                clock: None,
                seconds: None,
            };
            let decoded = store.try_read_capture(namespace, &reference).unwrap();
            assert_eq!(
                decoded
                    .iter()
                    .map(|sample| sample.to_bits())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|sample| sample.to_bits())
                    .collect::<Vec<_>>()
            );
        }
        let renamed_binding = StoreBinding::for_test(
            root.path().into(),
            owner_for_provider(&InMemoryProvider::from_seed([1; 32])),
            ProfileId("renamed".into()),
        );
        assert_eq!(first_binding.directory(), renamed_binding.directory());
        assert_eq!(
            std::fs::read_dir(root.path().join("personas/v1"))
                .unwrap()
                .count(),
            2
        );
        assert!(!root.path().join("genet-state.json").exists());
    }

    #[test]
    fn scope_navigation_payload_reopens_from_filesystem_and_corrupt_route_bytes_are_retained() {
        let root = tempfile::tempdir().unwrap();
        let provider = InMemoryProvider::from_seed([13; 32]);
        let store = open_fixture(root.path(), &provider);
        let ui = UiState::new();
        let saved = ui.to_persisted();
        let route = saved.scope_navigation_json.clone().unwrap();
        let bytes = serde_json::to_string(&saved).unwrap();
        store.try_save(&bytes).unwrap();
        drop(store);

        let reopened_store = open_fixture(root.path(), &provider);
        let reopened_bytes = reopened_store.try_load().unwrap().unwrap();
        assert_eq!(reopened_bytes, bytes);
        let reopened_session = woodshed_core::storage::decode_session(&reopened_bytes)
            .unwrap()
            .session;
        assert_eq!(
            reopened_session.scope_navigation_json.as_deref(),
            Some(route.as_str())
        );
        let mut reopened_ui = UiState::new();
        reopened_ui.apply_persisted(&reopened_session, Default::default());
        assert_eq!(
            reopened_ui.to_persisted().scope_navigation_json.as_deref(),
            Some(route.as_str())
        );

        let mut corrupt_session = reopened_session;
        corrupt_session.scope_navigation_json = Some("{".into());
        let corrupt_bytes = serde_json::to_string(&corrupt_session).unwrap();
        reopened_store.try_save(&corrupt_bytes).unwrap();
        drop(reopened_store);

        let fresh_store = open_fixture(root.path(), &provider);
        let retained_corrupt_bytes = fresh_store.try_load().unwrap().unwrap();
        let mut fresh_ui = UiState::new();
        let corrupt_loaded = woodshed_core::storage::decode_session(&retained_corrupt_bytes)
            .unwrap()
            .session;
        fresh_ui.apply_persisted(&corrupt_loaded, Default::default());
        assert_eq!(
            fresh_ui.to_persisted().scope_navigation_json.as_deref(),
            Some("{")
        );
        assert_eq!(
            fresh_store.try_load().unwrap().as_deref(),
            Some(corrupt_bytes.as_str())
        );
        assert_eq!(retained_corrupt_bytes, corrupt_bytes);
    }

    #[test]
    fn owner_initialization_authenticates_schema_identity_and_is_immutable() {
        let provider = InMemoryProvider::from_seed([3; 32]);
        for (version, purpose, foreign_owner) in [
            (2, OWNER_PURPOSE, false),
            (OWNER_VERSION, "another-purpose", false),
            (OWNER_VERSION, OWNER_PURPOSE, true),
        ] {
            let root = tempfile::tempdir().unwrap();
            let (backend, owner, directory) = sealed_at(root.path(), &provider);
            let bytes = serde_json::to_vec(&OwnerManifest {
                version,
                purpose: purpose.into(),
                owner: if foreign_owner {
                    "0".repeat(64)
                } else {
                    owner.clone()
                },
            })
            .unwrap();
            pollster::block_on(backend.put("owner", &bytes)).unwrap();
            let before = std::fs::read(directory.join("owner")).unwrap();
            assert!(initialize_owner(&backend, &directory, &owner).is_err());
            assert_eq!(std::fs::read(directory.join("owner")).unwrap(), before);
            assert!(!directory.join("session").exists());
        }
        let root = tempfile::tempdir().unwrap();
        let (backend, owner, directory) = sealed_at(root.path(), &provider);
        initialize_owner(&backend, &directory, &owner).unwrap();
        let original = std::fs::read(directory.join("owner")).unwrap();
        initialize_owner(&backend, &directory, &owner).unwrap();
        assert!(pollster::block_on(backend.put("owner", b"replacement")).is_err());
        assert!(pollster::block_on(backend.delete("owner")).is_err());
        assert_eq!(std::fs::read(directory.join("owner")).unwrap(), original);
        let foreign = SealedBackend::for_provider(
            FsBackend::at(&directory),
            &InMemoryProvider::from_seed([4; 32]),
        )
        .unwrap();
        assert!(initialize_owner(&foreign, &directory, &owner).is_err());
        assert_eq!(std::fs::read(directory.join("owner")).unwrap(), original);
    }

    #[test]
    fn incomplete_unowned_directories_and_plaintext_never_become_owned_stores() {
        let provider = InMemoryProvider::from_seed([5; 32]);
        for name in ["session", "settings", "loop-pcm", ".owner.interrupted.tmp"] {
            let root = tempfile::tempdir().unwrap();
            let (backend, owner, directory) = sealed_at(root.path(), &provider);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join(name), b"unowned bytes").unwrap();
            assert!(initialize_owner(&backend, &directory, &owner).is_err());
            assert!(!directory.join("owner").exists());
            assert_eq!(
                std::fs::read(directory.join(name)).unwrap(),
                b"unowned bytes"
            );
        }
        let root = tempfile::tempdir().unwrap();
        let (backend, owner, directory) = sealed_at(root.path(), &provider);
        std::fs::create_dir_all(&directory).unwrap();
        let plain = serde_json::to_vec(&OwnerManifest {
            version: OWNER_VERSION,
            purpose: OWNER_PURPOSE.into(),
            owner: owner.clone(),
        })
        .unwrap();
        std::fs::write(directory.join("owner"), &plain).unwrap();
        assert!(initialize_owner(&backend, &directory, &owner).is_err());
        assert_eq!(std::fs::read(directory.join("owner")).unwrap(), plain);
    }

    #[test]
    fn valid_owner_does_not_adopt_plaintext_session_or_settings() {
        let provider = InMemoryProvider::from_seed([6; 32]);
        let root = tempfile::tempdir().unwrap();
        let store = open_fixture(root.path(), &provider);
        let directory = persona_directory(root.path(), &owner_for_provider(&provider));
        std::fs::write(directory.join("session"), b"{}").unwrap();
        std::fs::write(directory.join("settings"), b"{}").unwrap();
        assert!(store.try_load().is_err());
        assert!(store.load_settings().is_none());
        assert!(store.write_refusal().is_some());
        assert!(store.try_save("default").is_err());
        assert!(store.try_save_settings("default").is_err());
        assert_eq!(std::fs::read(directory.join("session")).unwrap(), b"{}");
        assert_eq!(std::fs::read(directory.join("settings")).unwrap(), b"{}");
    }

    #[test]
    fn family_choice_requires_stable_unlocked_profile_and_cryptographic_identity() {
        let provider = InMemoryProvider::from_seed([11; 32]);
        let master = provider.master_public_key().to_bytes();
        let owner = owner_for_provider(&provider);
        let profile = ProfileId("default".into());
        let stable = || ChoiceIdentity {
            profile: Some(profile.clone()),
            unlocked: true,
            public_key: Some(master),
        };
        assert!(admit_choice_identity(stable(), &profile, master, &owner).is_ok());
        let changed_seed = InMemoryProvider::from_seed([12; 32])
            .master_public_key()
            .to_bytes();
        for current in [
            ChoiceIdentity {
                unlocked: false,
                ..stable()
            },
            ChoiceIdentity {
                profile: None,
                ..stable()
            },
            ChoiceIdentity {
                profile: Some(ProfileId("another-profile".into())),
                ..stable()
            },
            ChoiceIdentity {
                public_key: None,
                ..stable()
            },
            ChoiceIdentity {
                public_key: Some(changed_seed),
                ..stable()
            },
        ] {
            assert!(admit_choice_identity(current, &profile, master, &owner).is_err());
        }
        assert!(
            admit_choice_identity(stable(), &profile, master, &"0".repeat(64)).is_err(),
            "a current public key must also match the installed cryptographic owner"
        );
        assert!(
            admit_choice_identity(stable(), &profile, changed_seed, &owner).is_err(),
            "a matching binding owner cannot substitute for the retained provider identity"
        );
    }

    #[test]
    fn pending_store_is_empty_and_refuses_every_mutation() {
        let opened = pending_store("the vault is locked".into());
        assert!(matches!(opened.seal, PracticeSeal::Pending { .. }));
        assert!(opened.binding.is_none());
        assert!(opened.storage.try_load().unwrap().is_none());
        assert!(opened.storage.load_settings().is_none());
        assert!(
            pollster::block_on(opened.storage.backend().list(""))
                .unwrap()
                .is_empty()
        );
        assert!(opened.storage.try_save("must remain unsaved").is_err());
        assert!(pollster::block_on(opened.storage.backend().delete("session")).is_err());
        assert!(
            pollster::block_on(opened.storage.backend().apply(&[WriteOp::Put {
                key: "session".into(),
                value: vec![1]
            }]))
            .is_err()
        );
        assert!(opened.storage.try_load().unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn root_resolution_preserves_symlink_parent_semantics_for_missing_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let actual = directory.path().join("actual");
        std::fs::create_dir_all(actual.join("sub")).unwrap();
        std::os::unix::fs::symlink(actual.join("sub"), directory.path().join("link")).unwrap();
        let root = directory.path().join("link/../private");
        assert_eq!(
            absolute_root(&root).unwrap(),
            actual.canonicalize().unwrap().join("private")
        );
        assert!(
            !actual.join("private").exists(),
            "resolving isolation must not create storage"
        );
    }

    #[test]
    fn configured_root_freezes_relative_paths_and_refuses_obsolete_or_empty_overrides() {
        for mode in ["relative", "obsolete-state", "obsolete-settings", "empty"] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "storage::persona_store_tests::configured_root_worker",
                    "--nocapture",
                ])
                .env_remove("WOODSHED_STATE")
                .env_remove("WOODSHED_SETTINGS")
                .env_remove("WOODSHED_DATA_DIR")
                .env("WOODSHED_ROOT_TEST_MODE", mode);
            match mode {
                "relative" => {
                    command.env("WOODSHED_DATA_DIR", "isolated-root");
                },
                "obsolete-state" => {
                    command.env("WOODSHED_STATE", "");
                },
                "obsolete-settings" => {
                    command.env("WOODSHED_SETTINGS", "");
                },
                "empty" => {
                    command.env("WOODSHED_DATA_DIR", "");
                },
                _ => unreachable!(),
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "root test {mode} failed: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn configured_root_worker() {
        let Ok(mode) = std::env::var("WOODSHED_ROOT_TEST_MODE") else {
            return;
        };
        // Configuration is supplied before this isolated process starts; the
        // test runner's other threads never observe environment mutation.
        match mode.as_str() {
            "relative" => {
                let root = configured_root().unwrap();
                assert!(root.is_absolute());
                assert_eq!(
                    root,
                    std::env::current_dir()
                        .unwrap()
                        .canonicalize()
                        .unwrap()
                        .join("isolated-root")
                );
            },
            "obsolete-state" | "obsolete-settings" => {
                let name = if mode == "obsolete-state" {
                    "WOODSHED_STATE"
                } else {
                    "WOODSHED_SETTINGS"
                };
                let error = configured_root().unwrap_err();
                assert!(error.contains(name));
                assert!(error.contains("WOODSHED_DATA_DIR"));
            },
            "empty" => {
                assert!(configured_root().unwrap_err().contains("empty"));
            },
            _ => panic!("unknown root test mode"),
        }
    }
}

#[cfg(test)]
mod dr_c_receipts {
    use super::*;
    #[test]
    #[ignore = "run against an isolated absent/locked custody keeper with WOODSHED_DATA_DIR"]
    fn dr_c_woodshed_identity_stays_pending() {
        let mode = std::env::var("DR_C_RECEIPT_MODE").expect("receipt mode");
        assert!(mode == "absent" || mode == "locked");
        let root = configured_root().expect("isolated application root");
        assert!(std::env::var_os("WOODSHED_DATA_DIR").is_some());
        std::fs::create_dir_all(&root).unwrap();
        let legacy = root.join("genet-state.json");
        std::fs::write(&legacy, b"already-public practice").unwrap();
        let opened = open_store_at(&root, None);
        assert!(matches!(opened.seal, PracticeSeal::Pending { .. }));
        assert!(opened.binding.is_none());
        assert!(opened.storage.try_load().unwrap().is_none());
        assert!(
            opened
                .storage
                .try_save("must never be written in the clear")
                .is_err()
        );
        assert!(pollster::block_on(opened.storage.backend().delete("session")).is_err());
        assert_eq!(std::fs::read(&legacy).unwrap(), b"already-public practice");
        assert!(!root.join("personas").exists());
        let chosen = open_store_at(&root, Some(&ProfileId("default".into())));
        assert!(matches!(chosen.seal, PracticeSeal::Pending { .. }));
        assert!(chosen.binding.is_none());
    }
}
