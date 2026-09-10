// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The Rust-native [`UpdateTransport`], over luggage.
//!
//! Luggage is the family's fork of `cargo-packager-updater` (ruled
//! 2026-07-24: name luggage, home mere): signed manifests over pluggable
//! feeds, minisign + optional BLAKE3 verification, per-platform install via
//! the artifacts `cargo-packager` builds. The whole pipeline is Rust; no
//! .NET anywhere. This transport is the default; [`super::velopack_transport`]
//! stays selectable for the A/B until one retires.
//!
//! ## Configuration
//!
//! - `HOCKET_UPDATE_FEED` ([`super::FEED_ENV`]): the feed, in
//!   [`luggage::Feed::parse`] form — an HTTP(S) URL, a directory holding
//!   `luggage.json`, or `github:owner/repo`.
//! - `HOCKET_UPDATE_PUBKEY` ([`PUBKEY_ENV`]): the minisign public key
//!   releases are signed with. Refusing to run without it keeps "signed"
//!   a fact rather than a default-off option.

use std::sync::Mutex;

use luggage::{Config, Feed};

use super::{CheckOutcome, FEED_ENV, UpdateChannel, UpdateTransport, Unsupported};

/// Environment variable naming the minisign public key.
pub const PUBKEY_ENV: &str = "HOCKET_UPDATE_PUBKEY";

/// Luggage-backed transport.
pub struct LuggageTransport {
    feed: Option<String>,
    pubkey: Option<String>,
    installed: bool,
    /// The update found by the last check, kept so `download` and
    /// `apply_and_restart` act on exactly what was offered.
    found: Mutex<Option<luggage::Update>>,
    /// Downloaded, verified bytes staged for `apply_and_restart`.
    staged: Mutex<Option<Vec<u8>>>,
}

impl Default for LuggageTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl LuggageTransport {
    /// Inspect the environment once.
    pub fn new() -> Self {
        Self {
            feed: std::env::var(FEED_ENV).ok().filter(|v| !v.is_empty()),
            pubkey: std::env::var(PUBKEY_ENV).ok().filter(|v| !v.is_empty()),
            installed: is_installed(),
            found: Mutex::new(None),
            staged: Mutex::new(None),
        }
    }

    fn config(&self) -> Result<Config, String> {
        let feed = self
            .feed
            .as_deref()
            .ok_or_else(|| format!("{FEED_ENV} is not set"))?;
        let pubkey = self
            .pubkey
            .as_deref()
            .ok_or_else(|| format!("{PUBKEY_ENV} is not set"))?;
        Ok(Config {
            feeds: vec![Feed::parse(feed).map_err(|e| e.to_string())?],
            pubkey: pubkey.to_string(),
            windows: None,
            // Everything else from the secure defaults, which is how
            // `require_signed_manifest` stays true here. Spelling the rest out
            // is what let that field arrive unset; taking them from Default
            // means a new security control lands switched on rather than
            // breaking the build and tempting a hand-written `false`.
            ..Config::default()
        })
    }
}

/// Whether this looks like an installed build rather than a dev one.
///
/// Heuristic, honestly so: a cargo build runs from `target/debug` or
/// `target/release`, and offering an "update" there would launch an
/// installer the developer did not ask for. Anything else is presumed
/// installed; unlike the Velopack layout swap, the worst case is an
/// installer running against its own install location, not a corrupted
/// directory.
fn is_installed() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let path = exe.to_string_lossy().replace('\\', "/");
    // Cargo's target directory is configurable (this workspace uses a shared
    // `graphshell-target` directory), so the test-binary `debug/deps` shape
    // is the stable dev-build receipt rather than the literal folder name.
    !(path.contains("/target/debug/")
        || path.contains("/target/release/")
        || path.contains("/debug/deps/")
        || path.contains("/release/deps/"))
}

impl UpdateTransport for LuggageTransport {
    fn availability(&self) -> Result<(), Unsupported> {
        if !self.installed {
            return Err(Unsupported::NotInstalled);
        }
        if self.feed.is_none() || self.pubkey.is_none() {
            return Err(Unsupported::NoTransport);
        }
        Ok(())
    }

    fn current_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn check(&self, _channel: UpdateChannel) -> Result<CheckOutcome, String> {
        // Channel rides the feed for now: a beta feed is a different URL or
        // repo, same as the Velopack transport.
        let config = self.config()?;
        let current = self
            .current_version()
            .parse()
            .map_err(|e| format!("current version: {e}"))?;
        match luggage::check_update(current, config).map_err(|e| e.to_string())? {
            Some(update) => {
                let version = update.version.clone();
                *self.found.lock().unwrap() = Some(update);
                Ok(CheckOutcome::Update(version))
            }
            None => Ok(CheckOutcome::UpToDate),
        }
    }

    fn download(&self, version: &str) -> Result<(), String> {
        let found = self.found.lock().unwrap().clone();
        let Some(update) = found else {
            return Err("no update in hand; check first".to_string());
        };
        if update.version != version {
            // The offer changed between deciding and fetching; refuse rather
            // than quietly installing something else.
            return Err(format!(
                "the feed offered {}, not {version}; check again",
                update.version
            ));
        }
        // Verifies BLAKE3 (when the manifest carries it) then minisign.
        let bytes = update.download().map_err(|e| e.to_string())?;
        *self.staged.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn apply_and_restart(&self) -> Result<(), String> {
        let staged = self.staged.lock().unwrap().take();
        let Some(bytes) = staged else {
            return Err("nothing staged to apply".to_string());
        };
        let found = self.found.lock().unwrap().clone();
        let Some(update) = found else {
            return Err("no update in hand; check first".to_string());
        };
        // On Windows/NSIS this launches the installer (passive, relaunch)
        // and exits the process; on macOS/Linux it swaps in place.
        update.install(bytes).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transport(feed: Option<&str>, pubkey: Option<&str>, installed: bool) -> LuggageTransport {
        LuggageTransport {
            feed: feed.map(String::from),
            pubkey: pubkey.map(String::from),
            installed,
            found: Mutex::new(None),
            staged: Mutex::new(None),
        }
    }

    #[test]
    fn a_cargo_build_is_not_an_installed_build() {
        // The test binary itself runs from target/, so the heuristic must
        // say "dev build" right here.
        assert!(!is_installed());
        let t = transport(Some("github:merely-made/hocket"), Some("pk"), false);
        assert_eq!(t.availability(), Err(Unsupported::NotInstalled));
    }

    #[test]
    fn missing_feed_or_pubkey_means_no_transport() {
        let no_feed = transport(None, Some("pk"), true);
        assert_eq!(no_feed.availability(), Err(Unsupported::NoTransport));
        // An unsigned feed is not a transport: "signed" is not optional.
        let no_key = transport(Some("C:/feed"), None, true);
        assert_eq!(no_key.availability(), Err(Unsupported::NoTransport));
    }

    #[test]
    fn feed_and_pubkey_on_an_installed_build_is_available() {
        let t = transport(Some("github:merely-made/hocket"), Some("pk"), true);
        assert_eq!(t.availability(), Ok(()));
    }

    #[test]
    fn download_without_a_check_is_refused() {
        let t = transport(Some("C:/feed"), Some("pk"), true);
        let err = t.download("0.2.0").unwrap_err();
        assert!(err.contains("check first"), "got: {err}");
    }

    #[test]
    fn apply_without_staged_bytes_is_refused() {
        let t = transport(Some("C:/feed"), Some("pk"), true);
        let err = t.apply_and_restart().unwrap_err();
        assert!(err.contains("nothing staged"), "got: {err}");
    }
}
