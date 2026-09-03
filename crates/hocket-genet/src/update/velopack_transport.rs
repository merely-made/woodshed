// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The desktop [`UpdateTransport`], over Velopack.
//!
//! Velopack is the transport the family chose for desktop after the
//! 2026-07-22 pressure test (see mere's auto-update brief): one mechanism for
//! Windows, macOS, and Linux, with the installer, the feed, the staged apply,
//! and the restart all handled. Everything policy-shaped stays in the parent
//! module; this file only turns those decisions into Velopack calls.
//!
//! ## Feed
//!
//! `HOCKET_UPDATE_FEED` names the release feed. A directory path uses
//! Velopack's file source (useful for a LAN share, a mounted drive, and the
//! acceptance runs); anything else is treated as an HTTP(S) feed. Absent, the
//! transport reports [`Unsupported::NoTransport`] rather than inventing a
//! default location to fetch executables from.

use std::path::Path;

use velopack::{UpdateCheck, UpdateManager, sources};

use super::{CheckOutcome, FEED_ENV, UpdateChannel, UpdateTransport, Unsupported};

/// Velopack-backed transport.
pub struct VelopackTransport {
    feed: Option<String>,
    /// Whether this process is running from a Velopack-managed install.
    installed: bool,
}

impl Default for VelopackTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl VelopackTransport {
    /// Inspect the environment once.
    pub fn new() -> Self {
        Self {
            feed: std::env::var(FEED_ENV).ok().filter(|f| !f.is_empty()),
            installed: is_installed(),
        }
    }

    fn manager(&self) -> Result<UpdateManager, String> {
        let feed = self
            .feed
            .as_deref()
            .ok_or_else(|| format!("{FEED_ENV} is not set"))?;
        // A directory is a file feed; anything else is HTTP(S). Checking the
        // path first means a local acceptance run needs no server.
        if Path::new(feed).is_dir() {
            UpdateManager::new(sources::FileSource::new(feed), None, None)
        } else {
            UpdateManager::new(sources::HttpSource::new(feed), None, None)
        }
        .map_err(|err| err.to_string())
    }
}

/// Whether this process was placed by Velopack's installer.
///
/// Velopack lays out `<root>/current/<exe>` beside its `Update.exe`
/// (`UpdateMac`/`update` off-Windows), so the sibling of our parent directory
/// is the honest tell. A `cargo run` build, a copied binary, or a
/// distro-packaged one all fail this and are reported as
/// [`Unsupported::NotInstalled`] rather than being allowed to overwrite a
/// layout they do not own.
fn is_installed() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(root) = exe.parent().and_then(|dir| dir.parent()) else {
        return false;
    };
    ["Update.exe", "UpdateMac", "update"]
        .iter()
        .any(|name| root.join(name).exists())
}

impl UpdateTransport for VelopackTransport {
    fn availability(&self) -> Result<(), Unsupported> {
        if !self.installed {
            return Err(Unsupported::NotInstalled);
        }
        if self.feed.is_none() {
            return Err(Unsupported::NoTransport);
        }
        Ok(())
    }

    fn current_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn check(&self, _channel: UpdateChannel) -> Result<CheckOutcome, String> {
        // Channel selection rides the feed for now: a beta feed is a
        // different URL. When one feed serves both, this maps to Velopack's
        // channel argument instead.
        let manager = self.manager()?;
        match manager.check_for_updates().map_err(|e| e.to_string())? {
            UpdateCheck::UpdateAvailable(updates) => Ok(CheckOutcome::Update(
                updates.TargetFullRelease.Version.to_string(),
            )),
            UpdateCheck::NoUpdateAvailable => Ok(CheckOutcome::UpToDate),
            UpdateCheck::RemoteIsEmpty => Err("the release feed is empty".to_string()),
        }
    }

    fn download(&self, version: &str) -> Result<(), String> {
        let manager = self.manager()?;
        match manager.check_for_updates().map_err(|e| e.to_string())? {
            UpdateCheck::UpdateAvailable(updates) => {
                let offered = updates.TargetFullRelease.Version.to_string();
                if offered != version {
                    // The feed moved under us between deciding and fetching.
                    // Refuse rather than quietly installing a different build
                    // than the one the user was told about.
                    return Err(format!("feed now offers {offered}, not {version}"));
                }
                manager
                    .download_updates(&updates, None)
                    .map_err(|e| e.to_string())
            }
            UpdateCheck::NoUpdateAvailable => {
                Err(format!("{version} is no longer offered by the feed"))
            }
            UpdateCheck::RemoteIsEmpty => Err("the release feed is empty".to_string()),
        }
    }

    fn apply_and_restart(&self) -> Result<(), String> {
        let manager = self.manager()?;
        match manager.check_for_updates().map_err(|e| e.to_string())? {
            UpdateCheck::UpdateAvailable(updates) => {
                // Replaces this version and relaunches; normally does not
                // return.
                manager
                    .apply_updates_and_restart(&*updates)
                    .map_err(|e| e.to_string())
            }
            _ => Err("nothing staged to apply".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_binary_is_not_an_installed_build() {
        // The test runner is not a Velopack layout, so the transport must say
        // so rather than attempting an update against whatever is around it.
        assert!(!is_installed());
        let transport = VelopackTransport {
            feed: Some("https://example.invalid/feed".into()),
            installed: false,
        };
        assert_eq!(
            transport.availability(),
            Err(Unsupported::NotInstalled),
            "a non-installed build must refuse to self-update"
        );
    }

    #[test]
    fn an_installed_build_without_a_feed_has_no_transport() {
        let transport = VelopackTransport {
            feed: None,
            installed: true,
        };
        assert_eq!(transport.availability(), Err(Unsupported::NoTransport));
    }

    #[test]
    fn installed_with_a_feed_is_available() {
        let transport = VelopackTransport {
            feed: Some("https://example.invalid/feed".into()),
            installed: true,
        };
        assert_eq!(transport.availability(), Ok(()));
    }

    #[test]
    fn current_version_is_the_built_version() {
        let transport = VelopackTransport::new();
        assert_eq!(transport.current_version(), env!("CARGO_PKG_VERSION"));
    }
}
