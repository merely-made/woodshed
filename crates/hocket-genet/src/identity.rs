// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Host-local Hocket identity, persisted outside portable projects.

use std::path::{Path, PathBuf};

use personae::bootstrap::{self, Unlock};
use personae::roster;
use personae::vault::IdentityStorage;
use personae::{
    DerivedKeyAttestation, Ed25519Keypair, Ed25519PublicKey, IdentityError, IdentityProvider,
    InMemoryProvider, SealedIdentityProvider, SealedRecordStorage, load_or_create_auto_unlock_root,
};
use serde_json::Value;

const IDENTITY_RECORD: &str = "hocket/local-identity.json";

/// Pre-rename record name, when the product was Strophe (renamed 2026-07-14).
/// A sealed record's path is bound into its AEAD associated data, so the
/// identity cannot simply be moved to the new name: it has to be unsealed under
/// the old name and re-sealed under the new one. Without that, a musician who
/// already has an identity would silently become a new person with a new
/// fingerprint, and any hand-off envelope they had signed would no longer trace
/// back to them.
const LEGACY_IDENTITY_RECORD: &str = "strophe/local-identity.json";

/// Where this identity lives, and whether it is the family persona.
///
/// Hocket is the one application in the family whose durable public key is
/// routinely already in the world: the contact token a musician pastes to a
/// peer IS that key, and hand-off envelopes name it as their signer. So wiring
/// to the shared vault can never quietly mint a different one, and this says
/// which of the three real situations the user is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityHome {
    /// In the shared vault as `profile`, and it is the persona the rest of the
    /// family opens. Adopted with its key intact, so tokens already shared
    /// still resolve.
    Family { profile: String, protection: String },
    /// Hocket's own key, deliberately kept apart: the family persona is a
    /// different identity, and moving onto it would change the fingerprint
    /// peers already hold. Nothing rotates behind the user's back.
    Apart { family_profile: String },
    /// No shared vault on this machine. The pre-vault sealed record, as before.
    Local { reason: String },
}

impl IdentityHome {
    /// The one-line reading for the circle.
    pub fn summary(&self) -> String {
        match self {
            // The persona, not the protection. personae's description names the
            // backend AND its full path, which is right for a CLI line and
            // swamps a rail: it wrapped to seven lines in the circle. The
            // string is still on the variant for a detail view that has room.
            Self::Family { profile, .. } => {
                format!("Persona {profile}, shared across your Merely apps.")
            }
            Self::Apart { family_profile } => format!(
                "Hocket's own identity. Your persona {family_profile} is a different key, and moving to it would change the contact token you have already shared."
            ),
            Self::Local { reason } => {
                format!("Hocket's own identity, on this machine only ({reason}).")
            }
        }
    }
}

/// What the user agreed to when they joined a family persona.
///
/// Records the **key**, not just the profile name. Consent is to a specific
/// identity: if the family persona later points at a different key, this does
/// not carry over and Hocket asks again rather than rotating a second time on
/// the strength of a decision made about somebody else.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct FamilyConsent {
    profile: String,
    /// The master public key the user accepted, as a contact token.
    public_key: String,
}

/// Where the consent record lives, under Hocket's own data root. Not secret:
/// it names a public key and a profile id, both already visible to anyone who
/// can list the vault.
fn consent_path(data_root: &Path) -> PathBuf {
    data_root.join("family-persona.json")
}

fn load_consent(data_root: &Path) -> Option<FamilyConsent> {
    let bytes = std::fs::read(consent_path(data_root)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_consent(data_root: &Path, consent: &FamilyConsent) -> Result<(), IdentityError> {
    let path = consent_path(data_root);
    let write = || -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(consent).unwrap_or_default())?;
        std::fs::rename(tmp, &path)
    };
    write().map_err(|error| IdentityError::Backend(format!("record family persona: {error}")))
}

/// A durable host identity whose secret is held in memory only while Hocket runs.
pub struct LocalIdentity {
    provider: SealedIdentityProvider,
    /// The identity Hocket speaks as, when that is a family persona rather
    /// than its own record. `None` is the ordinary case: `provider` answers
    /// everything. An `InMemoryProvider` rather than a bare keypair because
    /// attestation is a provider concern and personae keeps its own.
    speaking: Option<InMemoryProvider>,
    home: IdentityHome,
    /// Where the consent record is written. Empty for an identity opened
    /// without a data root (tests that only exercise the sealed record).
    data_root: PathBuf,
}

impl LocalIdentity {
    /// Load or create the identity under Hocket's platform data directory,
    /// then place it in the shared vault when that can be done without
    /// changing anybody's key.
    pub fn open_default() -> Result<Self, IdentityError> {
        let data_root = default_data_root()?;
        adopt_legacy_data_root(&data_root)?;
        let unlock_path = data_root.join("personae/auto-unlock-root.json");
        let root_key = load_or_create_auto_unlock_root(unlock_path)?.ok_or_else(|| {
            IdentityError::Backend(
                "OS-protected automatic identity unlock is unavailable on this platform"
                    .to_string(),
            )
        })?;
        let mut identity = Self::open_with_root(&data_root.join("personae/records"), root_key)?;
        identity.data_root = data_root;
        identity.settle(&bootstrap::default_vault_dir(), Unlock::from_env());
        Ok(identity)
    }

    fn open_with_root(records_root: &Path, root_key: [u8; 32]) -> Result<Self, IdentityError> {
        let records = SealedRecordStorage::open_with_key(records_root, root_key);
        adopt_legacy_record(&records)?;
        let provider = SealedIdentityProvider::load_or_create(&records, IDENTITY_RECORD)?;
        Ok(Self {
            provider,
            speaking: None,
            home: IdentityHome::Local {
                reason: "the shared vault was not consulted".into(),
            },
            data_root: PathBuf::new(),
        })
    }

    /// Where this identity stands relative to the shared vault.
    ///
    /// **Never rotates and never overwrites.** The only outcome that writes is
    /// the one that cannot lose anything: when the family persona does not
    /// exist yet, Hocket's existing key is adopted into it, so the user gains a
    /// shared persona without their fingerprint moving. When it does exist and
    /// holds a different key, the vault is left exactly as found and Hocket
    /// keeps its own identity, because following the family choice there would
    /// silently make the user a different person to every peer holding their
    /// token. That switch is theirs to make, knowing the cost.
    /// Settle where this identity lives, adopting a joined persona's key when
    /// the user has consented to it.
    ///
    /// One vault open answers everything: which persona the family resolves
    /// to, whether it already holds this application's key, and whether the
    /// user agreed to speak as it. The unlock is used here and nowhere else,
    /// so a caller that named a passphrase vault is not silently re-read
    /// against the OS ladder half way through.
    fn settle(&mut self, vault_dir: &Path, unlock: Unlock) {
        self.speaking = None;
        let opened = match bootstrap::open_storage(vault_dir, unlock) {
            Ok(opened) => opened,
            Err(error) => {
                self.home = IdentityHome::Local {
                    reason: format!("no shared vault here: {error}"),
                };
                return;
            }
        };
        let family = match roster::resolve_profile(&*opened.storage, vault_dir) {
            Ok(id) => id,
            Err(error) => {
                self.home = IdentityHome::Local {
                    reason: format!("the vault would not say which persona: {error}"),
                };
                return;
            }
        };
        // Hocket's own record key, deliberately, not the one it speaks as:
        // this decides whether the vault already holds *this application's*
        // identity.
        let mine = self.provider.master_public_key().to_bytes();
        match opened.storage.load_profile(&family) {
            Ok(profile) if profile.master.public_key().to_bytes() == mine => {
                self.home = IdentityHome::Family {
                    profile: family.0,
                    protection: opened.description,
                };
            }
            Ok(profile) => {
                let joined = InMemoryProvider::from_seed(profile.master.to_seed());
                let token = encode_contact_token(&joined.master_public_key());
                // Consent is to a key, not to a name: a family persona that has
                // since moved to a different identity does not carry the old
                // agreement forward, so Hocket goes back to asking.
                let agreed = load_consent(&self.data_root)
                    .is_some_and(|c| c.profile == family.0 && c.public_key == token);
                if agreed {
                    self.speaking = Some(joined);
                    self.home = IdentityHome::Family {
                        profile: family.0,
                        protection: opened.description,
                    };
                } else {
                    self.home = IdentityHome::Apart {
                        family_profile: family.0,
                    };
                }
            }
            // Absent: adopt into it, key intact.
            Err(_) => {
                self.home = match roster::import_profile(
                    &*opened.storage,
                    &family,
                    "Hocket",
                    self.provider.master_keypair().clone(),
                ) {
                    Ok(_) => IdentityHome::Family {
                        profile: family.0,
                        protection: opened.description,
                    },
                    Err(error) => IdentityHome::Local {
                        reason: format!("could not place this identity in the vault: {error}"),
                    },
                };
            }
        }
    }

    /// Join the family persona: speak as it from now on, and remember that the
    /// user agreed to this key.
    ///
    /// **This is the rotation.** The contact token changes, so every peer
    /// holding the old one can no longer address a hand-off here until it is
    /// re-shared. Callers must have said so before calling. Consent is written
    /// before the switch, so an identity that cannot record the decision does
    /// not quietly make it.
    pub fn join_family(&mut self, vault_dir: &Path) -> Result<(), IdentityError> {
        self.join_family_with(vault_dir, Unlock::from_env())
    }

    /// [`Self::join_family`] naming its unlock, for tests on the portable
    /// passphrase vault every platform has.
    fn join_family_with(
        &mut self,
        vault_dir: &Path,
        unlock: Unlock,
    ) -> Result<(), IdentityError> {
        let IdentityHome::Apart { family_profile } = self.home.clone() else {
            return Err(IdentityError::Backend(
                "this identity is already the family persona, or there is no vault".into(),
            ));
        };
        let opened = bootstrap::open_storage(vault_dir, unlock)?;
        let profile = opened
            .storage
            .load_profile(&personae::vault::ProfileId(family_profile.clone()))?;
        let joined = InMemoryProvider::from_seed(profile.master.to_seed());
        save_consent(
            &self.data_root,
            &FamilyConsent {
                profile: family_profile.clone(),
                public_key: encode_contact_token(&joined.master_public_key()),
            },
        )?;
        self.speaking = Some(joined);
        self.home = IdentityHome::Family {
            profile: family_profile,
            protection: opened.description,
        };
        Ok(())
    }

    /// Who this identity derives from: a joined family persona when there is
    /// one, Hocket's own record otherwise.
    fn speaker(&self) -> &dyn IdentityProvider {
        match &self.speaking {
            Some(joined) => joined,
            None => &self.provider,
        }
    }

    /// The family persona this identity could join, and by joining, become.
    /// `None` when there is nothing to join.
    pub fn family_to_join(&self) -> Option<&str> {
        match &self.home {
            IdentityHome::Apart { family_profile } => Some(family_profile.as_str()),
            _ => None,
        }
    }

    /// Where this identity lives, for the circle to report.
    pub fn home(&self) -> &IdentityHome {
        &self.home
    }

    /// Short display fingerprint of the public key. This is not an address.
    pub fn fingerprint(&self) -> String {
        // Through the speaker, not the record: after joining a family persona
        // the fingerprint on screen must be the one peers will actually see.
        self.speaker()
            .master_public_key()
            .to_bytes()
            .iter()
            .take(6)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The whole public key as a copyable contact token: 64 lowercase hex
    /// characters. Unlike [`fingerprint`](Self::fingerprint), this is the full
    /// key, so a peer can address a hand-off back to this identity. A friendlier
    /// checksummed encoding is a later refinement.
    pub fn contact_token(&self) -> String {
        encode_contact_token(&self.speaker().master_public_key())
    }
}

/// Encode a public key as a contact token: 64 lowercase hex characters.
pub fn encode_contact_token(key: &Ed25519PublicKey) -> String {
    key.to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Parse a contact token (as pasted, whitespace tolerated) back into a public
/// key. Errors carry a human-facing reason, since a mistyped or truncated token
/// is the common failure a peer needs told about.
pub fn parse_contact_token(token: &str) -> Result<Ed25519PublicKey, String> {
    let cleaned: String = token.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() != 64 {
        return Err(format!(
            "a contact token is 64 hex characters; this is {}",
            cleaned.len()
        ));
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in cleaned.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| "token has invalid text".to_string())?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| "a contact token must be hexadecimal".to_string())?;
    }
    Ed25519PublicKey::from_bytes(&bytes).map_err(|_| "not a valid identity key".to_string())
}

impl IdentityProvider for LocalIdentity {
    fn master_public_key(&self) -> Ed25519PublicKey {
        self.speaker().master_public_key()
    }

    fn derive_keypair(&self, salt: &[u8]) -> Result<Ed25519Keypair, IdentityError> {
        self.speaker().derive_keypair(salt)
    }

    fn attest_derived_key(&self, salt: &[u8]) -> Result<DerivedKeyAttestation, IdentityError> {
        self.speaker().attest_derived_key(salt)
    }
}

fn default_data_root() -> Result<PathBuf, IdentityError> {
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(root).join("Hocket"));
    }
    if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(root).join("hocket"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/share/hocket"));
    }
    Err(IdentityError::Backend(
        "could not determine Hocket's local data directory".to_string(),
    ))
}

fn legacy_data_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        return Some(PathBuf::from(root).join("Strophe"));
    }
    if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(root).join("strophe"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share/strophe"))
}

/// Move a pre-rename data directory to the Hocket one, once.
///
/// The auto-unlock root is DPAPI-wrapped against the Windows user, not against
/// its path, so it survives the move. The sealed record's associated data is its
/// path *relative to the records root*, which the move also leaves intact.
fn adopt_legacy_data_root(data_root: &Path) -> Result<(), IdentityError> {
    if data_root.exists() {
        return Ok(());
    }
    let Some(legacy) = legacy_data_root() else {
        return Ok(());
    };
    if !legacy.exists() {
        return Ok(());
    }
    std::fs::rename(&legacy, data_root).map_err(|err| {
        IdentityError::Backend(format!(
            "move pre-rename identity {legacy:?} -> {data_root:?}: {err}"
        ))
    })
}

/// Re-seal a pre-rename identity record under the current record name, once.
///
/// Unsealing yields the record as opaque JSON, which is re-sealed verbatim under
/// the new name's associated data. The key material is unchanged, so the
/// fingerprint shown in the circle stays the same across the rename.
fn adopt_legacy_record(records: &SealedRecordStorage) -> Result<(), IdentityError> {
    if records.load_record::<Value>(IDENTITY_RECORD)?.is_some() {
        return Ok(());
    }
    let Some(record) = records.load_record::<Value>(LEGACY_IDENTITY_RECORD)? else {
        return Ok(());
    };
    records.save_record(IDENTITY_RECORD, &record)?;
    records.delete_record(LEGACY_IDENTITY_RECORD)
}

#[cfg(test)]
mod tests {
    use personae::IdentityProvider;

    use super::*;

    #[test]
    fn contact_token_round_trips_and_rejects_malformed() {
        use personae::InMemoryProvider;
        let key = InMemoryProvider::from_seed([9; 32]).master_public_key();
        let token = encode_contact_token(&key);
        assert_eq!(token.len(), 64);
        assert_eq!(parse_contact_token(&token).unwrap(), key);
        // Pasted tokens carry stray whitespace; tolerate it.
        assert_eq!(parse_contact_token(&format!("  {token}\n")).unwrap(), key);
        assert!(parse_contact_token("too short").is_err());
        assert!(parse_contact_token(&"z".repeat(64)).is_err());
    }

    /// A scratch vault. `PERSONAE_PASSPHRASE` names the unlock so these run on
    /// every platform: the OS auto-unlock ladder is Windows-only, and a test
    /// that silently skips elsewhere would report pass with no evidence.
    fn scratch_vault(tag: &str) -> (tempfile::TempDir, Unlock) {
        let dir = tempfile::tempdir().unwrap();
        let _ = tag;
        (
            dir,
            Unlock::Passphrase(b"hocket-identity-test".to_vec().into()),
        )
    }

    fn vault_storage(dir: &Path, unlock: Unlock) -> Box<dyn IdentityStorage> {
        bootstrap::open_storage(dir, unlock).unwrap().storage
    }

    #[test]
    fn an_empty_vault_adopts_hocket_without_moving_the_fingerprint() {
        // The whole reason this is not an ordinary wiring job: the contact
        // token a musician has already pasted to a peer IS this key.
        let records = tempfile::tempdir().unwrap();
        let (vault, unlock) = scratch_vault("adopt");
        let identity = LocalIdentity::open_with_root(records.path(), [0x61; 32]).unwrap();
        let before = identity.master_public_key();

        let mut identity = identity;
        identity.settle(vault.path(), unlock);
        let home = identity.home().clone();

        assert!(matches!(home, IdentityHome::Family { .. }), "{home:?}");
        assert_eq!(
            identity.master_public_key(),
            before,
            "adoption must not mint a new identity"
        );
        // And the vault now holds exactly that key, so the other applications
        // open the same person.
        let storage = vault_storage(
            vault.path(),
            Unlock::Passphrase(b"hocket-identity-test".to_vec().into()),
        );
        let profile = storage
            .load_profile(&personae::vault::ProfileId("default".into()))
            .unwrap();
        assert_eq!(profile.master.public_key(), before);
    }

    #[test]
    fn a_vault_already_holding_somebody_else_is_left_alone() {
        // The catastrophic case: the family persona is turnstone's identity,
        // with denizen grants rooted on it. Adopting over it would destroy
        // them, and following it would silently make the musician a different
        // person to every peer.
        let records = tempfile::tempdir().unwrap();
        let (vault, unlock) = scratch_vault("apart");
        let storage = vault_storage(vault.path(), unlock);
        let theirs = personae::roster::create_profile(
            &*storage,
            &personae::vault::ProfileId("default".into()),
            "Somebody else",
        )
        .unwrap();
        let theirs_public = theirs.master.public_key();
        drop(storage);

        let mut identity = LocalIdentity::open_with_root(records.path(), [0x62; 32]).unwrap();
        let mine = identity.master_public_key();
        identity.settle(
            vault.path(),
            Unlock::Passphrase(b"hocket-identity-test".to_vec().into()),
        );
        let home = identity.home().clone();

        assert!(matches!(home, IdentityHome::Apart { .. }), "{home:?}");
        assert_eq!(identity.master_public_key(), mine, "nothing rotated");
        let storage = vault_storage(
            vault.path(),
            Unlock::Passphrase(b"hocket-identity-test".to_vec().into()),
        );
        assert_eq!(
            storage
                .load_profile(&personae::vault::ProfileId("default".into()))
                .unwrap()
                .master
                .public_key(),
            theirs_public,
            "the other persona survives untouched"
        );
    }

    #[test]
    fn a_second_run_recognises_the_persona_it_adopted() {
        // Adoption is once; the run after it must read as Family rather than
        // adopting again or reading as a stranger.
        let records = tempfile::tempdir().unwrap();
        let (vault, unlock) = scratch_vault("again");
        let mut identity = LocalIdentity::open_with_root(records.path(), [0x63; 32]).unwrap();
        identity.settle(vault.path(), unlock);
        assert!(matches!(identity.home(), IdentityHome::Family { .. }));

        let mut again = LocalIdentity::open_with_root(records.path(), [0x63; 32]).unwrap();
        again.settle(
            vault.path(),
            Unlock::Passphrase(b"hocket-identity-test".to_vec().into()),
        );
        let home = again.home().clone();
        assert!(matches!(home, IdentityHome::Family { .. }), "{home:?}");
    }

    #[test]
    fn no_vault_at_all_keeps_the_pre_vault_identity() {
        // A machine with no vault backend still runs, unchanged from before
        // this wiring existed.
        let records = tempfile::tempdir().unwrap();
        let closed = tempfile::tempdir().unwrap();
        let file = closed.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();

        let mut identity = LocalIdentity::open_with_root(records.path(), [0x64; 32]).unwrap();
        identity.settle(
            &file.join("vault"),
            Unlock::Passphrase(b"hocket-identity-test".to_vec().into()),
        );
        let home = identity.home().clone();
        assert!(matches!(home, IdentityHome::Local { .. }), "{home:?}");
    }

    #[test]
    fn every_home_says_which_situation_the_user_is_in() {
        let family = IdentityHome::Family {
            profile: "work".into(),
            protection: "DPAPI-wrapped root".into(),
        };
        assert!(family.summary().contains("work"));
        assert!(family.summary().contains("DPAPI"));

        let apart = IdentityHome::Apart {
            family_profile: "work".into(),
        };
        assert!(
            apart.summary().contains("contact token"),
            "the cost of switching is the point: {}",
            apart.summary()
        );

        let local = IdentityHome::Local {
            reason: "no vault".into(),
        };
        assert!(local.summary().contains("no vault"));
    }

    /// An identity opened against a scratch records root AND a data root, so
    /// the consent record has somewhere to live.
    fn identity_with_root(
        records: &Path,
        data_root: &Path,
        key: u8,
    ) -> LocalIdentity {
        let mut identity = LocalIdentity::open_with_root(records, [key; 32]).unwrap();
        identity.data_root = data_root.to_path_buf();
        identity
    }

    fn test_unlock() -> Unlock {
        Unlock::Passphrase(b"hocket-identity-test".to_vec().into())
    }

    /// A vault whose family persona is somebody else, so the identity under
    /// test lands Apart and has something to join.
    fn vault_with_stranger(dir: &Path) -> Ed25519PublicKey {
        let storage = bootstrap::open_storage(dir, test_unlock()).unwrap().storage;
        personae::roster::create_profile(
            &*storage,
            &personae::vault::ProfileId("default".into()),
            "Somebody else",
        )
        .unwrap()
        .master
        .public_key()
    }

    #[test]
    fn joining_the_family_persona_is_the_rotation_and_it_is_explicit() {
        // The switch surface's whole point: the token changes, and only
        // because the user asked.
        let records = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();
        let theirs = vault_with_stranger(vault.path());

        let mut identity = identity_with_root(records.path(), data.path(), 0x71);
        identity.settle(vault.path(), test_unlock());
        let before = identity.master_public_key();
        assert_eq!(identity.family_to_join(), Some("default"));
        assert_ne!(before, theirs, "not yet joined");

        identity.join_family_with(vault.path(), test_unlock()).unwrap();

        assert_eq!(
            identity.master_public_key(),
            theirs,
            "Hocket now speaks as the family persona"
        );
        assert_ne!(
            identity.master_public_key(),
            before,
            "which is a rotation: the old contact token no longer names this app"
        );
        assert!(identity.family_to_join().is_none(), "nothing left to join");
    }

    #[test]
    fn a_joined_persona_survives_a_restart_without_asking_again() {
        let records = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();
        let theirs = vault_with_stranger(vault.path());

        let mut identity = identity_with_root(records.path(), data.path(), 0x72);
        identity.settle(vault.path(), test_unlock());
        identity.join_family_with(vault.path(), test_unlock()).unwrap();

        let mut restarted = identity_with_root(records.path(), data.path(), 0x72);
        restarted.settle(vault.path(), test_unlock());
        assert!(
            matches!(restarted.home(), IdentityHome::Family { .. }),
            "{:?}",
            restarted.home()
        );
        assert_eq!(restarted.master_public_key(), theirs);
    }

    #[test]
    fn consent_is_to_a_key_so_a_moved_family_persona_asks_again() {
        // The second-rotation trap: the user agreed to be *this* person, not
        // to follow whatever the family choice becomes. If the persona later
        // names a different key, Hocket goes back to Apart rather than
        // rotating again on the strength of the old decision.
        let records = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();
        vault_with_stranger(vault.path());

        let mut identity = identity_with_root(records.path(), data.path(), 0x73);
        identity.settle(vault.path(), test_unlock());
        identity.join_family_with(vault.path(), test_unlock()).unwrap();
        let joined = identity.master_public_key();

        // The family persona is replaced by a different key under the same id.
        let storage = bootstrap::open_storage(vault.path(), test_unlock())
            .unwrap()
            .storage;
        let replacement = personae::vault::Profile::new(
            personae::vault::ProfileId("default".into()),
            "Replaced",
            Ed25519Keypair::from_seed([0x99; 32]),
        );
        storage.save_profile(&replacement).unwrap();
        drop(storage);

        let mut after = identity_with_root(records.path(), data.path(), 0x73);
        after.settle(vault.path(), test_unlock());
        assert!(
            matches!(after.home(), IdentityHome::Apart { .. }),
            "consent does not carry to a different key: {:?}",
            after.home()
        );
        assert_ne!(after.master_public_key(), joined);
        assert_ne!(
            after.master_public_key(),
            replacement.master.public_key(),
            "and it certainly does not silently become the replacement"
        );
    }

    #[test]
    fn joining_is_refused_when_there_is_nothing_to_join() {
        let records = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();

        // An empty vault adopts Hocket, so it is already the family persona.
        let mut identity = identity_with_root(records.path(), data.path(), 0x74);
        identity.settle(vault.path(), test_unlock());
        assert!(matches!(identity.home(), IdentityHome::Family { .. }));
        assert!(identity.join_family_with(vault.path(), test_unlock()).is_err());
    }

    #[test]
    fn sealed_identity_is_stable_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let first = LocalIdentity::open_with_root(dir.path(), [0x45; 32]).unwrap();
        let first_public = first.master_public_key();
        drop(first);

        let second = LocalIdentity::open_with_root(dir.path(), [0x45; 32]).unwrap();
        assert_eq!(second.master_public_key(), first_public);
    }

    #[test]
    fn a_pre_rename_identity_keeps_its_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let records = SealedRecordStorage::open_with_key(dir.path(), [0x51; 32]);
        let legacy = SealedIdentityProvider::load_or_create(&records, LEGACY_IDENTITY_RECORD)
            .expect("seal a pre-rename identity");
        let legacy_public = legacy.master_public_key();
        drop(legacy);

        let adopted = LocalIdentity::open_with_root(dir.path(), [0x51; 32]).unwrap();

        assert_eq!(
            adopted.master_public_key(),
            legacy_public,
            "the rename must not mint a new identity"
        );
        assert!(dir.path().join("hocket/local-identity.json").exists());
        assert!(!dir.path().join("strophe/local-identity.json").exists());
    }

    #[test]
    fn a_pre_rename_identity_does_not_displace_a_current_one() {
        let dir = tempfile::tempdir().unwrap();
        let records = SealedRecordStorage::open_with_key(dir.path(), [0x52; 32]);
        let current = SealedIdentityProvider::load_or_create(&records, IDENTITY_RECORD).unwrap();
        let current_public = current.master_public_key();
        drop(current);
        SealedIdentityProvider::load_or_create(&records, LEGACY_IDENTITY_RECORD).unwrap();

        let opened = LocalIdentity::open_with_root(dir.path(), [0x52; 32]).unwrap();

        assert_eq!(opened.master_public_key(), current_public);
    }

    #[test]
    fn wrong_record_root_cannot_open_identity() {
        let dir = tempfile::tempdir().unwrap();
        LocalIdentity::open_with_root(dir.path(), [0x45; 32]).unwrap();

        let error = LocalIdentity::open_with_root(dir.path(), [0x46; 32])
            .err()
            .expect("wrong root should fail");
        assert!(error.to_string().contains("decrypt sealed record"));
    }
}
