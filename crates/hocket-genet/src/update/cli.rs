// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! `--update-now`: run the update flow in the terminal and report honestly.
//!
//! Hocket is a GUI app, so without this there is no way to *prove* an update
//! cycle except by clicking. This path runs check → download → apply
//! synchronously, printing each real state, and is what the acceptance runs
//! in the plan's H4 drive. It is also genuinely useful on its own: a
//! scriptable update for a machine that is not in front of anyone.
//!
//! It respects the same policy the GUI does — both load it from the same
//! `update-settings.json` through `UpdateSettingsProvider` — so a `notify`
//! policy reports an available version without fetching it, exactly as the app
//! would. Point `HOCKET_SETTINGS` at another file to run against an isolated
//! policy.

use super::{
    CheckOutcome, NextStep, UpdateSettings, UpdateStatus, UpdateTransport, after_download, decide,
};

/// Exit code when the flow could not run at all (unsupported, or failed).
pub const EXIT_FAILED: i32 = 1;

/// Run one update pass and return the process exit code.
pub fn run_update_now<T: UpdateTransport>(transport: T, settings: UpdateSettings) -> i32 {
    println!("hocket {}", transport.current_version());
    println!("policy: {}", settings.policy.as_str());

    if let Err(why) = transport.availability() {
        println!("{}", UpdateStatus::Unsupported(why).summary());
        return EXIT_FAILED;
    }
    if !settings.policy.checks() {
        println!("{}", UpdateStatus::Disabled.summary());
        return 0;
    }

    println!("{}", UpdateStatus::Checking.summary());
    let outcome = match transport.check(settings.channel) {
        Ok(outcome) => outcome,
        Err(reason) => {
            println!(
                "{}",
                UpdateStatus::Failed {
                    during: "check",
                    reason,
                }
                .summary()
            );
            return EXIT_FAILED;
        }
    };

    let version = match &outcome {
        CheckOutcome::UpToDate => {
            println!(
                "{}",
                UpdateStatus::UpToDate {
                    current: transport.current_version(),
                }
                .summary()
            );
            return 0;
        }
        CheckOutcome::Update(version) => version.clone(),
    };

    match decide(settings, &outcome) {
        NextStep::Rest => {
            println!("{}", UpdateStatus::Available { version }.summary());
            0
        }
        NextStep::AskUser(version) => {
            // Honest: this policy found something and is not allowed to fetch
            // it. Say so, and say what would.
            println!("{}", UpdateStatus::Available { version }.summary());
            println!(
                "policy {} does not download; change the device update setting to Automatic to apply",
                settings.policy.as_str()
            );
            0
        }
        NextStep::Download(version) => {
            println!(
                "{}",
                UpdateStatus::Downloading {
                    version: version.clone(),
                }
                .summary()
            );
            if let Err(reason) = transport.download(&version) {
                println!(
                    "{}",
                    UpdateStatus::Failed {
                        during: "download",
                        reason,
                    }
                    .summary()
                );
                return EXIT_FAILED;
            }
            println!("{}", after_download(settings, version).summary());

            // The GUI stages and offers; a terminal run was asked to do it,
            // so it applies. On Windows this hands off to the installer and
            // the process exits; elsewhere it swaps in place.
            println!("applying");
            match transport.apply_and_restart() {
                Ok(()) => {
                    println!("applied");
                    0
                }
                Err(reason) => {
                    println!(
                        "{}",
                        UpdateStatus::Failed {
                            during: "restart",
                            reason,
                        }
                        .summary()
                    );
                    EXIT_FAILED
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{UpdateChannel, UpdatePolicy, Unsupported};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct FakeTransport {
        availability: Result<(), Unsupported>,
        check: Result<CheckOutcome, String>,
        downloaded: Arc<Mutex<Vec<String>>>,
        applied: Arc<Mutex<bool>>,
    }

    impl FakeTransport {
        fn offering(version: &str) -> Self {
            Self {
                availability: Ok(()),
                check: Ok(CheckOutcome::Update(version.to_string())),
                downloaded: Arc::new(Mutex::new(Vec::new())),
                applied: Arc::new(Mutex::new(false)),
            }
        }
    }

    impl UpdateTransport for FakeTransport {
        fn availability(&self) -> Result<(), Unsupported> {
            self.availability.clone()
        }
        fn current_version(&self) -> String {
            "0.1.0".into()
        }
        fn check(&self, _channel: UpdateChannel) -> Result<CheckOutcome, String> {
            self.check.clone()
        }
        fn download(&self, version: &str) -> Result<(), String> {
            self.downloaded.lock().unwrap().push(version.to_string());
            Ok(())
        }
        fn apply_and_restart(&self) -> Result<(), String> {
            *self.applied.lock().unwrap() = true;
            Ok(())
        }
    }

    fn settings(policy: UpdatePolicy) -> UpdateSettings {
        UpdateSettings {
            policy,
            ..Default::default()
        }
    }

    #[test]
    fn automatic_downloads_and_applies() {
        let transport = FakeTransport::offering("0.2.0");
        let code = run_update_now(transport.clone(), settings(UpdatePolicy::Automatic));
        assert_eq!(code, 0);
        assert_eq!(&*transport.downloaded.lock().unwrap(), &["0.2.0"]);
        assert!(*transport.applied.lock().unwrap(), "should have applied");
    }

    #[test]
    fn notify_only_reports_without_fetching() {
        let transport = FakeTransport::offering("0.2.0");
        let code = run_update_now(transport.clone(), settings(UpdatePolicy::NotifyOnly));
        assert_eq!(code, 0, "finding an update is not a failure");
        assert!(
            transport.downloaded.lock().unwrap().is_empty(),
            "notify-only must not fetch, even when driven from a terminal"
        );
        assert!(!*transport.applied.lock().unwrap());
    }

    #[test]
    fn an_uninstalled_build_exits_nonzero() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.availability = Err(Unsupported::NotInstalled);
        assert_eq!(
            run_update_now(transport, settings(UpdatePolicy::Automatic)),
            EXIT_FAILED
        );
    }

    #[test]
    fn a_failed_check_exits_nonzero() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.check = Err("feed unreachable".into());
        assert_eq!(
            run_update_now(transport, settings(UpdatePolicy::Automatic)),
            EXIT_FAILED
        );
    }

    #[test]
    fn up_to_date_exits_zero_without_applying() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.check = Ok(CheckOutcome::UpToDate);
        let code = run_update_now(transport.clone(), settings(UpdatePolicy::Automatic));
        assert_eq!(code, 0);
        assert!(!*transport.applied.lock().unwrap());
    }

    #[test]
    fn policy_off_does_not_even_check() {
        let transport = FakeTransport::offering("0.2.0");
        let code = run_update_now(transport.clone(), settings(UpdatePolicy::Off));
        assert_eq!(code, 0);
        assert!(transport.downloaded.lock().unwrap().is_empty());
    }
}
