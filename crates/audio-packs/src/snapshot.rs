// SPDX-License-Identifier: MPL-2.0
use crate::{MAX_ASSET, MAX_SOURCE, MAX_TOTAL, PackDefinition, PackError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestFormat {
    Toml,
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackIdentity {
    pub id: String,
    pub version: String,
    /// BLAKE3 over the versioned exact-source/asset envelope, independent of paths
    /// on the installing computer. Different authored encodings have different IDs.
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSnapshot {
    format: u32,
    pub(crate) manifest_format: ManifestFormat,
    pub(crate) source: Vec<u8>,
    pub(crate) assets: BTreeMap<String, Vec<u8>>,
    pub(crate) identity: PackIdentity,
}

impl PackSnapshot {
    pub(crate) fn new(
        manifest_format: ManifestFormat,
        source: Vec<u8>,
        assets: BTreeMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            format: 1,
            manifest_format,
            source,
            assets,
            identity: PackIdentity {
                id: String::new(),
                version: String::new(),
                digest: String::new(),
            },
        }
    }
    pub fn identity(&self) -> &PackIdentity {
        &self.identity
    }

    pub fn to_json(&self) -> Result<Vec<u8>, PackError> {
        serde_json::to_vec(self).map_err(|e| PackError(format!("Snapshot encoding: {e}")))
    }

    /// Decode only. LoadedPack::from_snapshot must admit the result before use.
    pub fn from_json(bytes: &[u8]) -> Result<Self, PackError> {
        if bytes.len() > 80 * 1024 * 1024 {
            return Err(PackError("Snapshot exceeds resource bound".into()));
        }
        let snapshot: Self = serde_json::from_slice(bytes)
            .map_err(|e| PackError(format!("Snapshot decoding: {e}")))?;
        snapshot.validate_bounds()?;
        Ok(snapshot)
    }

    pub(crate) fn validate_bounds(&self) -> Result<(), PackError> {
        if self.format != 1
            || self.source.is_empty()
            || self.source.len() > MAX_SOURCE
            || self.assets.len() > 64
        {
            return Err(PackError(
                "Unsupported snapshot version or resource count".into(),
            ));
        }
        let mut total = 0usize;
        for (path, bytes) in &self.assets {
            if !crate::definition::pack_path(path) || bytes.len() > MAX_ASSET {
                return Err(PackError("Invalid snapshot asset or resource size".into()));
            }
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| PackError("Snapshot size overflow".into()))?;
            if total > MAX_TOTAL {
                return Err(PackError(
                    "Snapshot assets exceed aggregate resource bound".into(),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn parse_definition(&self) -> Result<PackDefinition, PackError> {
        if self.source.len() > MAX_SOURCE {
            return Err(PackError("Manifest exceeds resource bound".into()));
        }
        let text = std::str::from_utf8(&self.source)
            .map_err(|e| PackError(format!("Manifest encoding: {e}")))?;
        match self.manifest_format {
            ManifestFormat::Toml => {
                toml::from_str(text).map_err(|e| PackError(format!("TOML manifest: {e}")))
            },
            ManifestFormat::Json => {
                serde_json::from_str(text).map_err(|e| PackError(format!("JSON manifest: {e}")))
            },
        }
    }

    pub(crate) fn computed_identity(&self, definition: &PackDefinition) -> PackIdentity {
        let mut hash = blake3::Hasher::new();
        hash.update(b"woodshed-audio-pack-snapshot-v1\0");
        hash.update(&[match self.manifest_format {
            ManifestFormat::Toml => 0,
            ManifestFormat::Json => 1,
        }]);
        add_bytes(&mut hash, &self.source);
        for (path, bytes) in &self.assets {
            add_bytes(&mut hash, path.as_bytes());
            add_bytes(&mut hash, bytes);
        }
        PackIdentity {
            id: definition.id.clone(),
            version: definition.version.clone(),
            digest: hash.finalize().to_hex().to_string(),
        }
    }
}

fn add_bytes(hash: &mut blake3::Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
