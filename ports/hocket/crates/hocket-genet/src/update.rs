// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Auto-update: policy, honest status, and the transport seam.
//!
//! Per [`design_docs/2026-07-24_auto-update_plan.md`](../../../design_docs/2026-07-24_auto-update_plan.md).
//!
//! Two layers. Everything above [`UpdateTransport`] is platform-neutral and
//! unit-tested, because the *decisions* must be identical on every host; only
//! the mechanism below it is per-platform. That split is what makes "works
//! everywhere" a property of the design rather than something each host has
//! to remember.
//!
//! ## Two rules this module exists to keep
//!
//! - **Configurable, never a checkbox.** [`UpdatePolicy`] is four real
//!   behaviours plus a channel and a cadence, not a boolean.
//! - **Honest status.** [`UpdateStatus`] has a variant for every state the
//!   system can actually be in, including why a failure failed. Nothing here
//!   reports motion that is not happening.

pub mod cli;
pub mod luggage_transport;
pub mod velopack_transport;
pub mod worker;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use mere_surface_api::settings::{
    SettingControl, SettingMovement, SettingMutability, SettingOption, SettingScope,
    SettingSecurity, SettingSpec, SettingValue, SettingsError, SettingsProvider,
};
use serde::{Deserialize, Serialize};
use workbench::SettingsRef;

/// Environment variable naming the release feed, shared by every transport.
pub const FEED_ENV: &str = "HOCKET_UPDATE_FEED";

/// How much the app may do on its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum UpdatePolicy {
    /// Never check. The user updates manually or their package manager does.
    Off,
    /// Check and report, but download nothing without being asked.
    #[default]
    NotifyOnly,
    /// Check and download, but do not apply until the user says so.
    DownloadThenAsk,
    /// Check, download, and apply, restarting when the user next allows it.
    Automatic,
}

impl UpdatePolicy {
    /// Whether this policy checks at all.
    pub fn checks(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Whether this policy may download without asking.
    pub fn may_download(self) -> bool {
        matches!(self, Self::DownloadThenAsk | Self::Automatic)
    }

    /// Whether this policy may apply without asking.
    pub fn may_apply(self) -> bool {
        matches!(self, Self::Automatic)
    }

    /// Stable string form, for settings persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::NotifyOnly => "notify",
            Self::DownloadThenAsk => "download-then-ask",
            Self::Automatic => "automatic",
        }
    }

    /// Parse the persisted form; unknown values fall back to the default
    /// rather than silently disabling updates.
    pub fn from_str_or_default(value: &str) -> Self {
        match value {
            "off" => Self::Off,
            "download-then-ask" => Self::DownloadThenAsk,
            "automatic" => Self::Automatic,
            _ => Self::NotifyOnly,
        }
    }
}

/// Which release stream to follow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum UpdateChannel {
    /// Released builds.
    #[default]
    Stable,
    /// Pre-release builds.
    Beta,
}

impl UpdateChannel {
    /// Stable string form, for settings persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

/// The user's update settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UpdateSettings {
    /// How much the app may do on its own.
    pub policy: UpdatePolicy,
    /// Which release stream to follow.
    pub channel: UpdateChannel,
    /// Minimum seconds between automatic checks. A check the user explicitly
    /// asks for ignores this.
    pub check_interval_secs: u64,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            policy: UpdatePolicy::default(),
            channel: UpdateChannel::default(),
            // Six hours: often enough to matter, rare enough to be invisible.
            check_interval_secs: 6 * 60 * 60,
        }
    }
}

/// Explicit override for the device settings file, useful for isolated runs.
pub const SETTINGS_PATH_ENV: &str = "HOCKET_SETTINGS";

pub const UPDATE_REFERENCE: &str = "pelt/update";

/// The device-local settings file replacing the interim policy environment
/// variable. The owner remains `UpdateSettings`; the provider only supplies
/// the settings projection and persistence boundary.
#[derive(Clone, Debug)]
pub struct UpdateSettingsProvider {
    path: PathBuf,
    settings: UpdateSettings,
}

impl UpdateSettingsProvider {
    pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let settings = match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|error| {
                io::Error::new(io::ErrorKind::InvalidData, format!("invalid update settings: {error}"))
            })?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => UpdateSettings::default(),
            Err(error) => return Err(error),
        };
        Ok(Self { path, settings })
    }

    pub fn load_or_default(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        match Self::load(&path) {
            Ok(provider) => provider,
            Err(error) => {
                eprintln!("[hocket] ignoring update settings: {error}");
                Self {
                    path,
                    settings: UpdateSettings::default(),
                }
            }
        }
    }

    pub fn settings(&self) -> UpdateSettings {
        self.settings
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn save(&self, settings: &UpdateSettings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let contents = serde_json::to_vec_pretty(settings).map_err(|error| {
            io::Error::new(io::ErrorKind::InvalidData, format!("serialize update settings: {error}"))
        })?;
        std::fs::write(&tmp, contents)?;
        std::fs::rename(tmp, &self.path)
    }
}

fn option(value: &str, label: &str) -> SettingOption {
    SettingOption {
        value: value.into(),
        label: label.into(),
    }
}

impl SettingsProvider for UpdateSettingsProvider {
    fn describe(&self, reference: &SettingsRef) -> Result<Vec<SettingSpec>, SettingsError> {
        if reference.0 != UPDATE_REFERENCE {
            return Err(SettingsError::UnsupportedReference(reference.clone()));
        }
        Ok(vec![
            SettingSpec {
                id: "update.policy".into(),
                label: "Update policy".into(),
                scope: SettingScope::Device,
                movement: SettingMovement::LocalOnly,
                mutability: SettingMutability::StartupOnly,
                security: SettingSecurity::Ordinary,
                control: SettingControl::Choice {
                    options: vec![
                        option("off", "Off"),
                        option("notify", "Notify only"),
                        option("download-then-ask", "Download then ask"),
                        option("automatic", "Automatic"),
                    ],
                },
                value: SettingValue::Text(self.settings.policy.as_str().into()),
            },
            SettingSpec {
                id: "update.channel".into(),
                label: "Update channel".into(),
                scope: SettingScope::Device,
                movement: SettingMovement::LocalOnly,
                mutability: SettingMutability::StartupOnly,
                security: SettingSecurity::Ordinary,
                control: SettingControl::Choice {
                    options: vec![option("stable", "Stable"), option("beta", "Beta")],
                },
                value: SettingValue::Text(self.settings.channel.as_str().into()),
            },
            SettingSpec {
                id: "update.check_interval_secs".into(),
                label: "Check interval (seconds)".into(),
                scope: SettingScope::Device,
                movement: SettingMovement::LocalOnly,
                mutability: SettingMutability::StartupOnly,
                security: SettingSecurity::Ordinary,
                control: SettingControl::Number {
                    min: Some(300.0),
                    max: Some(604_800.0),
                    step: Some(300.0),
                },
                value: SettingValue::Integer(self.settings.check_interval_secs as i64),
            },
        ])
    }

    fn apply(
        &mut self,
        reference: &SettingsRef,
        setting_id: &str,
        value: SettingValue,
    ) -> Result<(), SettingsError> {
        if reference.0 != UPDATE_REFERENCE {
            return Err(SettingsError::UnsupportedReference(reference.clone()));
        }
        let mut next = self.settings;
        match (setting_id, value) {
            ("update.policy", SettingValue::Text(value)) => {
                next.policy = UpdatePolicy::from_str_or_default(&value);
                if value != next.policy.as_str() {
                    return Err(SettingsError::InvalidValue {
                        setting_id: setting_id.into(),
                        message: format!("unknown policy {value}"),
                    });
                }
            }
            ("update.channel", SettingValue::Text(value)) => {
                next.channel = match value.as_str() {
                    "stable" => UpdateChannel::Stable,
                    "beta" => UpdateChannel::Beta,
                    _ => {
                        return Err(SettingsError::InvalidValue {
                            setting_id: setting_id.into(),
                            message: format!("unknown channel {value}"),
                        });
                    }
                };
            }
            ("update.check_interval_secs", SettingValue::Integer(value))
                if (300..=604_800).contains(&value) =>
            {
                next.check_interval_secs = value as u64;
            }
            ("update.check_interval_secs", SettingValue::Integer(value)) => {
                return Err(SettingsError::InvalidValue {
                    setting_id: setting_id.into(),
                    message: format!("interval {value} is outside 300..=604800"),
                });
            }
            (_, _) => return Err(SettingsError::UnknownSetting(setting_id.into())),
        }
        self.save(&next).map_err(|error| SettingsError::Storage(error.to_string()))?;
        self.settings = next;
        Ok(())
    }
}

/// Resolve the device-local settings path. `HOCKET_SETTINGS` is the only
/// override; otherwise it follows the platform data root convention used by
/// Hocket's local identity.
pub fn settings_path() -> PathBuf {
    if let Some(path) = std::env::var_os(SETTINGS_PATH_ENV) {
        return PathBuf::from(path);
    }
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(root).join("Hocket/update-settings.json");
    }
    if let Some(root) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(root).join("hocket/update-settings.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/hocket/update-settings.json");
    }
    PathBuf::from("hocket-update-settings.json")
}

/// Why the app cannot update itself in this installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// Running from a build that our installer did not place (a `cargo run`
    /// dev build, a copied binary). Self-updating would corrupt whatever
    /// layout it is actually running in.
    NotInstalled,
    /// The platform build has no update transport compiled in.
    NoTransport,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(
                f,
                "not an installed build, so updates are managed outside the app"
            ),
            Self::NoTransport => write!(f, "no update transport in this build"),
        }
    }
}

/// What the app is actually doing about updates, right now.
///
/// Every variant is a state the system is genuinely in. There is deliberately
/// no "working..." catch-all: a spinner with nothing behind it is exactly what
/// this type exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Updates are off by policy.
    Disabled,
    /// This installation cannot self-update, and why.
    Unsupported(Unsupported),
    /// Nothing in flight, and we have never checked this run.
    Idle,
    /// A check is in flight.
    Checking,
    /// Checked, and this is the newest build.
    UpToDate {
        /// The version we are running.
        current: String,
    },
    /// A newer version exists and has not been downloaded.
    Available {
        /// The version on offer.
        version: String,
    },
    /// A download is in flight.
    Downloading {
        /// The version being fetched.
        version: String,
    },
    /// Downloaded and staged; it applies on restart.
    ReadyToRestart {
        /// The staged version.
        version: String,
    },
    /// Something failed, with the reason kept rather than swallowed.
    Failed {
        /// What we were attempting.
        during: &'static str,
        /// Why it failed, in the words the layer below used.
        reason: String,
    },
}

impl UpdateStatus {
    /// One line for the UI. Short, and never claims motion that is not
    /// happening.
    pub fn summary(&self) -> String {
        match self {
            Self::Disabled => "Updates off".to_string(),
            Self::Unsupported(why) => format!("Updates unavailable: {why}"),
            Self::Idle => "Not checked yet".to_string(),
            Self::Checking => "Checking for updates".to_string(),
            Self::UpToDate { current } => format!("Up to date ({current})"),
            Self::Available { version } => format!("Version {version} available"),
            Self::Downloading { version } => format!("Downloading {version}"),
            Self::ReadyToRestart { version } => format!("{version} ready, restart to finish"),
            Self::Failed { during, reason } => format!("Update {during} failed: {reason}"),
        }
    }

    /// What clicking this status would do, if anything.
    ///
    /// The status knows what it invites, so the label and the action stay in
    /// one place instead of the view guessing. `None` means there is nothing
    /// to carry on with, and the chip stays a plain label.
    pub fn action_label(&self) -> Option<&'static str> {
        match self {
            Self::Available { .. } => Some("download"),
            Self::ReadyToRestart { .. } => Some("restart now"),
            Self::Failed { .. } => Some("try again"),
            _ => None,
        }
    }

}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Already newest.
    UpToDate,
    /// A newer version is on offer.
    Update(String),
}

/// What the app should do next, given its policy and what a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextStep {
    /// Nothing to do.
    Rest,
    /// Download the named version now.
    Download(String),
    /// Tell the user and wait.
    AskUser(String),
}

/// The decision layer.
///
/// Pure: this is the behaviour that must be identical on every host, so it
/// takes no I/O and is tested directly.
pub fn decide(settings: UpdateSettings, outcome: &CheckOutcome) -> NextStep {
    match outcome {
        CheckOutcome::UpToDate => NextStep::Rest,
        CheckOutcome::Update(version) => {
            if !settings.policy.checks() {
                // A policy that does not check cannot act on a result.
                NextStep::Rest
            } else if settings.policy.may_download() {
                NextStep::Download(version.clone())
            } else {
                NextStep::AskUser(version.clone())
            }
        }
    }
}

/// After a successful download, whether to apply now or wait for the user.
pub fn after_download(settings: UpdateSettings, version: String) -> UpdateStatus {
    // Even `Automatic` stages rather than yanking the app out from under a
    // recording: the restart is offered, never forced.
    let _ = settings.policy.may_apply();
    UpdateStatus::ReadyToRestart { version }
}

/// The per-platform mechanism.
///
/// Desktop is Velopack; a surface Velopack does not serve (a service worker,
/// firmware offered over the mesh) implements this instead of reimplementing
/// the policy above it.
pub trait UpdateTransport: Send {
    /// Whether this build can actually self-update.
    fn availability(&self) -> Result<(), Unsupported>;

    /// The running version.
    fn current_version(&self) -> String;

    /// Ask the feed what is newest.
    fn check(&self, channel: UpdateChannel) -> Result<CheckOutcome, String>;

    /// Fetch a version and stage it.
    fn download(&self, version: &str) -> Result<(), String>;

    /// Apply what is staged and restart.
    fn apply_and_restart(&self) -> Result<(), String>;
}

/// Boxed transports delegate, so the host can pick one at runtime.
impl UpdateTransport for Box<dyn UpdateTransport> {
    fn availability(&self) -> Result<(), Unsupported> {
        (**self).availability()
    }

    fn current_version(&self) -> String {
        (**self).current_version()
    }

    fn check(&self, channel: UpdateChannel) -> Result<CheckOutcome, String> {
        (**self).check(channel)
    }

    fn download(&self, version: &str) -> Result<(), String> {
        (**self).download(version)
    }

    fn apply_and_restart(&self) -> Result<(), String> {
        (**self).apply_and_restart()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(policy: UpdatePolicy) -> UpdateSettings {
        UpdateSettings {
            policy,
            ..Default::default()
        }
    }

    #[test]
    fn policy_is_four_behaviours_not_a_boolean() {
        assert!(!UpdatePolicy::Off.checks());
        for policy in [
            UpdatePolicy::NotifyOnly,
            UpdatePolicy::DownloadThenAsk,
            UpdatePolicy::Automatic,
        ] {
            assert!(policy.checks());
        }
        assert!(!UpdatePolicy::NotifyOnly.may_download());
        assert!(UpdatePolicy::DownloadThenAsk.may_download());
        assert!(!UpdatePolicy::DownloadThenAsk.may_apply());
        assert!(UpdatePolicy::Automatic.may_apply());
    }

    #[test]
    fn notify_only_asks_and_never_downloads() {
        let step = decide(
            settings(UpdatePolicy::NotifyOnly),
            &CheckOutcome::Update("0.2.0".into()),
        );
        assert_eq!(step, NextStep::AskUser("0.2.0".into()));
    }

    #[test]
    fn downloading_policies_download() {
        for policy in [UpdatePolicy::DownloadThenAsk, UpdatePolicy::Automatic] {
            let step = decide(settings(policy), &CheckOutcome::Update("0.2.0".into()));
            assert_eq!(step, NextStep::Download("0.2.0".into()));
        }
    }

    #[test]
    fn off_never_acts_even_on_a_found_update() {
        let step = decide(
            settings(UpdatePolicy::Off),
            &CheckOutcome::Update("9.9.9".into()),
        );
        assert_eq!(step, NextStep::Rest, "policy Off must not act");
    }

    #[test]
    fn up_to_date_rests_under_every_policy() {
        for policy in [
            UpdatePolicy::Off,
            UpdatePolicy::NotifyOnly,
            UpdatePolicy::DownloadThenAsk,
            UpdatePolicy::Automatic,
        ] {
            assert_eq!(decide(settings(policy), &CheckOutcome::UpToDate), NextStep::Rest);
        }
    }

    #[test]
    fn even_automatic_stages_rather_than_restarting_under_the_user() {
        // Hocket can be recording. An update never yanks the app away; it
        // stages and offers.
        let status = after_download(settings(UpdatePolicy::Automatic), "0.2.0".into());
        assert_eq!(status, UpdateStatus::ReadyToRestart { version: "0.2.0".into() });
        // Offered, not forced: the restart is the user's click.
        assert_eq!(status.action_label(), Some("restart now"));
    }

    #[test]
    fn status_summaries_never_claim_motion_that_is_not_happening() {
        assert_eq!(UpdateStatus::Idle.summary(), "Not checked yet");
        assert_eq!(
            UpdateStatus::UpToDate { current: "0.1.0".into() }.summary(),
            "Up to date (0.1.0)"
        );
        // A failure keeps its reason instead of degrading to "something went wrong".
        let failed = UpdateStatus::Failed {
            during: "check",
            reason: "feed unreachable".into(),
        };
        assert!(failed.summary().contains("feed unreachable"));
    }

    #[test]
    fn a_dev_build_reports_why_it_cannot_update() {
        let status = UpdateStatus::Unsupported(Unsupported::NotInstalled);
        let summary = status.summary();
        assert!(summary.contains("not an installed build"), "got: {summary}");
        // And offers no action: a click that is guaranteed to fail is the
        // dishonest-UI failure mode this type exists to avoid.
        assert_eq!(status.action_label(), None);
    }

    #[test]
    fn only_states_worth_acting_on_offer_an_action() {
        assert_eq!(
            UpdateStatus::Available { version: "0.2.0".into() }.action_label(),
            Some("download")
        );
        assert_eq!(
            UpdateStatus::ReadyToRestart { version: "0.2.0".into() }.action_label(),
            Some("restart now")
        );
        assert_eq!(
            UpdateStatus::Failed { during: "check", reason: "no".into() }.action_label(),
            Some("try again")
        );
        // In flight, or nothing to do: a click must not invent work.
        for status in [
            UpdateStatus::Checking,
            UpdateStatus::Downloading { version: "0.2.0".into() },
            UpdateStatus::Disabled,
            UpdateStatus::Unsupported(Unsupported::NotInstalled),
            UpdateStatus::UpToDate { current: "0.2.0".into() },
            UpdateStatus::Idle,
        ] {
            assert_eq!(status.action_label(), None, "{status:?} must not be clickable");
        }
    }

    #[test]
    fn policy_round_trips_through_its_persisted_form() {
        for policy in [
            UpdatePolicy::Off,
            UpdatePolicy::NotifyOnly,
            UpdatePolicy::DownloadThenAsk,
            UpdatePolicy::Automatic,
        ] {
            assert_eq!(UpdatePolicy::from_str_or_default(policy.as_str()), policy);
        }
        // An unreadable setting must not silently disable updates.
        assert_eq!(
            UpdatePolicy::from_str_or_default("nonsense"),
            UpdatePolicy::NotifyOnly
        );
    }

    #[test]
    fn device_provider_describes_and_persists_update_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-settings.json");
        let mut provider = UpdateSettingsProvider::load(&path).unwrap();
        let reference = SettingsRef(UPDATE_REFERENCE.into());
        let specs = provider.describe(&reference).unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].scope, SettingScope::Device);
        assert_eq!(specs[0].mutability, SettingMutability::StartupOnly);
        provider
            .apply(
                &reference,
                "update.policy",
                SettingValue::Text("automatic".into()),
            )
            .unwrap();
        assert_eq!(
            UpdateSettingsProvider::load(&path).unwrap().settings().policy,
            UpdatePolicy::Automatic
        );
        assert!(matches!(
            provider.apply(
                &reference,
                "update.check_interval_secs",
                SettingValue::Integer(1)
            ),
            Err(SettingsError::InvalidValue { .. })
        ));
    }
}
