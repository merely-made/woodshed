// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Hocket's identity, held by djinn.
//!
//! Hocket opens no vault, sealed record or DPAPI root (dramatis repo plan,
//! D5). It calls djinn over Graphshell's custody route as the `hocket` app.
//! Its own pre-DR-C identity, the key behind every contact token already
//! shared, is adopted by djinn with its fingerprint unchanged (D13): djinn
//! places it in the vault, as its own persona when the family persona is a
//! different key, and leaves a public forwarding note naming that persona
//! ([`MARKER`]). Hocket reads the note and speaks as the persona it names.
//!
//! With djinn absent or Locked the identity is pending (D12): there is no
//! contact token, nothing is signed, and Hocket never falls back to a key of
//! its own.

use std::path::{Path, PathBuf};

use graphshell::native::custody_identity::CustodyIdentity;
use insigne::DerivedKeyAttestation;
use personae::{Ed25519Keypair, Ed25519PublicKey, IdentityError, IdentityProvider, ProfileId};

/// The name Hocket is admitted to djinn's custody route under.
const CUSTODY_APP: &str = "hocket";

/// djinn's forwarding note in Hocket's data root: the persona holding
/// Hocket's adopted key, and that key. Public; written by djinn.
const MARKER: &str = "custody-profile.json";

/// The DPAPI root that marks a pre-DR-C identity djinn has to adopt. Hocket
/// only checks that it exists; it never opens it.
const LEGACY_ROOT: &str = "personae/auto-unlock-root.json";

#[derive(serde::Deserialize)]
struct CustodyNote {
    profile: String,
    public_key: String,
}

/// Where this identity lives, and whether it is the family persona.
///
/// Hocket is the one application in the family whose durable public key is
/// routinely already in the world: the contact token a musician pastes to a
/// peer IS that key, and hand-off envelopes name it as their signer. So
/// moving into custody never quietly changes it, and this says which of the
/// two real situations the user is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityHome {
    /// Speaking as the persona the rest of the family uses.
    Family { profile: String, protection: String },
    /// Hocket's own key, kept apart as persona `profile` because the family
    /// persona is a different key: moving onto it would change the
    /// fingerprint peers already hold. Nothing rotates behind the user's back.
    Apart {
        profile: String,
        family_profile: String,
    },
}

impl IdentityHome {
    /// The one-line reading for the circle.
    pub fn summary(&self) -> String {
        match self {
            Self::Family { profile, .. } => {
                format!("Persona {profile}, shared across your Merely apps.")
            }
            Self::Apart { family_profile, .. } => format!(
                "Hocket's own identity. Your persona {family_profile} is a different key, and moving to it would change the contact token you have already shared."
            ),
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
/// it names a public key and a profile id.
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

fn load_note(data_root: &Path) -> Option<CustodyNote> {
    let bytes = std::fs::read(data_root.join(MARKER)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The pending answer: djinn is absent or Locked, or has not adopted the
/// identity yet (D12).
fn pending(reason: impl std::fmt::Display) -> IdentityError {
    IdentityError::Backend(format!("identity pending: {reason}"))
}

/// Hocket's identity, as djinn holds it.
pub struct LocalIdentity {
    speaker: CustodyIdentity,
    home: IdentityHome,
    data_root: PathBuf,
}

impl LocalIdentity {
    /// Reach Hocket's identity through djinn: the persona djinn adopted
    /// Hocket's own key into, or the family persona when Hocket had none or
    /// the user joined it. Pending while djinn is absent or Locked.
    pub fn open_default() -> Result<Self, IdentityError> {
        let data_root = default_data_root()?;
        adopt_legacy_data_root(&data_root)?;
        Self::open_at(data_root)
    }

    fn open_at(data_root: PathBuf) -> Result<Self, IdentityError> {
        // The status this asks for is also djinn's cue to adopt Hocket's own
        // record and write the note read below (D13).
        let family = CustodyIdentity::connect(CUSTODY_APP).map_err(pending)?;
        let family_profile = family
            .profile()
            .map(|profile| profile.0.clone())
            .unwrap_or_default();
        let protection = family.protection();
        let family_token = encode_contact_token(&family.master_public_key());
        let note = load_note(&data_root);
        let Some(note) = note else {
            if data_root.join(LEGACY_ROOT).is_file() {
                return Err(pending(
                    "djinn has not adopted Hocket's own identity yet; nothing is signed until it does",
                ));
            }
            // No identity of Hocket's own ever existed: the family persona.
            return Ok(Self {
                speaker: family,
                home: IdentityHome::Family {
                    profile: family_profile,
                    protection,
                },
                data_root,
            });
        };
        let joined = load_consent(&data_root)
            .is_some_and(|c| c.profile == family_profile && c.public_key == family_token);
        if note.public_key == family_token || joined {
            return Ok(Self {
                speaker: family,
                home: IdentityHome::Family {
                    profile: family_profile,
                    protection,
                },
                data_root,
            });
        }
        let own = CustodyIdentity::connect_as(CUSTODY_APP, Some(ProfileId(note.profile.clone())))
            .map_err(pending)?;
        if encode_contact_token(&own.master_public_key()) != note.public_key {
            return Err(pending(
                "djinn's note does not match the persona it names; nothing is signed",
            ));
        }
        Ok(Self {
            speaker: own,
            home: IdentityHome::Apart {
                profile: note.profile,
                family_profile,
            },
            data_root,
        })
    }

    /// Join the family persona: speak as it from now on, and remember that the
    /// user agreed to this key.
    ///
    /// **This is the rotation.** The contact token changes, so every peer
    /// holding the old one can no longer address a hand-off here until it is
    /// re-shared. Callers must have said so before calling. Consent is written
    /// before the switch, so an identity that cannot record the decision does
    /// not quietly make it.
    pub fn join_family(&mut self) -> Result<(), IdentityError> {
        let IdentityHome::Apart { family_profile, .. } = self.home.clone() else {
            return Err(IdentityError::Backend(
                "this identity is already the family persona".into(),
            ));
        };
        let family = CustodyIdentity::connect(CUSTODY_APP).map_err(pending)?;
        save_consent(
            &self.data_root,
            &FamilyConsent {
                profile: family_profile.clone(),
                public_key: encode_contact_token(&family.master_public_key()),
            },
        )?;
        let protection = family.protection();
        self.speaker = family;
        self.home = IdentityHome::Family {
            profile: family_profile,
            protection,
        };
        Ok(())
    }

    /// The family persona this identity could join, and by joining, become.
    /// `None` when there is nothing to join.
    pub fn family_to_join(&self) -> Option<&str> {
        match &self.home {
            IdentityHome::Apart { family_profile, .. } => Some(family_profile.as_str()),
            IdentityHome::Family { .. } => None,
        }
    }

    /// Where this identity lives, for the circle to report.
    pub fn home(&self) -> &IdentityHome {
        &self.home
    }

    /// Short display fingerprint of the public key. This is not an address.
    pub fn fingerprint(&self) -> String {
        self.speaker
            .master_public_key()
            .to_bytes()
            .iter()
            .take(6)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The whole public key as a copyable contact token: 64 lowercase hex
    /// characters. Unlike [`fingerprint`](Self::fingerprint), this is the full
    /// key, so a peer can address a hand-off back to this identity.
    pub fn contact_token(&self) -> String {
        encode_contact_token(&self.speaker.master_public_key())
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
        self.speaker.master_public_key()
    }

    /// The key djinn releases for `salt` (a hand-off's session signer).
    fn derive_keypair(&self, salt: &[u8]) -> Result<Ed25519Keypair, IdentityError> {
        self.speaker.derive_keypair(salt)
    }

    fn attest_derived_key(&self, salt: &[u8]) -> Result<DerivedKeyAttestation, IdentityError> {
        self.speaker.attest_derived_key(salt)
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

/// Move a pre-rename data directory to the Hocket one, once, so djinn finds
/// the identity it adopts where it looks. A move only: nothing is opened.
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

    #[test]
    fn every_home_says_which_situation_the_user_is_in() {
        let family = IdentityHome::Family {
            profile: "default".into(),
            protection: "AutoOs".into(),
        };
        assert!(family.summary().contains("default"));
        let apart = IdentityHome::Apart {
            profile: "hocket".into(),
            family_profile: "work".into(),
        };
        assert!(apart.summary().contains("work"));
        assert!(apart.summary().contains("contact token"));
    }

    #[test]
    fn a_pending_identity_names_why() {
        assert!(pending("djinn is not running").to_string().contains("pending"));
    }
}

#[cfg(test)]
mod dr_c_receipts {
    use super::*;
    #[test]
    #[ignore = "run mere/scripts/dr_c_receipts.py against the isolated receipt keeper"]
    fn dr_c_hocket_identity_stays_pending() {
        let mode = std::env::var("DR_C_RECEIPT_MODE").expect("receipt mode");
        assert!(mode == "absent" || mode == "locked");
        if mode == "locked" {
            let mut client = graphshell::native::custody_client::BlockingCustodyClient::open(
                graphshell::native::app_admission::AppId::new(CUSTODY_APP)).unwrap();
            assert_eq!(client.status().unwrap().lock, graphshell::identity::VaultLockView::Locked);
            assert!(!client.roster().unwrap().entries.is_empty(), "public persona roster remains readable");
        }
        let temp = tempfile::tempdir().unwrap();
        let public = temp.path().join("public-session");
        std::fs::write(&public, b"public session retained").unwrap();
        let result = LocalIdentity::open_at(temp.path().to_path_buf());
        assert!(matches!(result, Err(ref error) if error.to_string().contains("pending")), "Hocket must expose no contact token or signing identity");
        assert_eq!(std::fs::read(&public).unwrap(), b"public session retained");
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1, "no identity, adoption or consent is written by Hocket while pending");
        // Also exercise the previously-owned identity branch. These synthetic
        // bytes are never decoded by Hocket: only djinn may adopt them.
        let legacy = temp.path().join(LEGACY_ROOT);
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, b"opaque legacy root").unwrap();
        assert!(LocalIdentity::open_at(temp.path().to_path_buf()).is_err());
        assert_eq!(std::fs::read(legacy).unwrap(), b"opaque legacy root");
        assert!(!temp.path().join(MARKER).exists());
        assert!(!consent_path(temp.path()).exists());
    }
}
