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
use djinn::door::custody::CustodyRefusal;
use djinn::door::custody_client::CustodyClientError;
use djinn::door::custody_identity::CustodyIdentity;
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

/// Personae's pinned XChaCha20-Poly1305 envelope adds a 24-byte nonce and
/// 16-byte tag. Bound encrypted files before SealedBackend allocates plaintext.
const MAX_CAPTURE_FILE_BYTES: usize = MAX_PCM_BYTES + 40;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[path = "storage/capture_tests.rs"]
mod capture_tests;

#[cfg(test)]
#[path = "storage/persona_store_tests.rs"]
mod persona_store_tests;

#[cfg(test)]
#[path = "storage/dr_c_receipts.rs"]
mod dr_c_receipts;

#[path = "storage/filesystem.rs"]
mod filesystem;

use filesystem::atomic_write;
