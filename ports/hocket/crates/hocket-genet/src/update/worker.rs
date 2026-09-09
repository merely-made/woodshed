// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The update worker: an armillary actor, shaped like [`crate::project_io`].
//!
//! Update work is network and disk I/O, so it never runs on the UI thread and
//! certainly never on the audio thread. Commands go in, [`UpdateStatus`] comes
//! back out, and the event loop wakes to apply it — the same seam the project
//! worker uses.

use std::sync::mpsc::Receiver;

use armillary::{ActorHandle, Emitter, Wake, spawn};

use super::{
    CheckOutcome, NextStep, UpdateSettings, UpdateStatus, UpdateTransport, after_download, decide,
};

/// What the host asks the updater to do.
pub enum UpdateCommand {
    /// Check the feed. `user_asked` distinguishes an explicit request (which
    /// reports its result either way) from the automatic one.
    Check {
        /// The policy and channel in force.
        settings: UpdateSettings,
        /// Whether the user asked for this check.
        user_asked: bool,
    },
    /// Fetch and stage a version the user accepted.
    Download {
        /// The version to fetch.
        version: String,
        /// The policy in force.
        settings: UpdateSettings,
    },
    /// Apply what is staged and restart.
    ApplyAndRestart,
}

/// Spawn the update actor over a transport.
pub fn spawn_update_worker<T>(
    wake: Wake,
    transport: T,
) -> (ActorHandle<UpdateCommand>, Receiver<UpdateStatus>)
where
    T: UpdateTransport + 'static,
{
    spawn(wake, move |commands, updates| {
        while let Ok(command) = commands.recv() {
            run_command(&transport, command, &updates);
        }
    })
}

fn run_command<T: UpdateTransport>(
    transport: &T,
    command: UpdateCommand,
    updates: &Emitter<UpdateStatus>,
) {
    // An installation that cannot update says so once, for any command,
    // rather than failing halfway through one.
    if let Err(why) = transport.availability() {
        updates.emit(UpdateStatus::Unsupported(why));
        return;
    }

    match command {
        UpdateCommand::Check {
            settings,
            user_asked,
        } => {
            if !settings.policy.checks() {
                updates.emit(UpdateStatus::Disabled);
                return;
            }
            updates.emit(UpdateStatus::Checking);
            let outcome = match transport.check(settings.channel) {
                Ok(outcome) => outcome,
                Err(reason) => {
                    updates.emit(UpdateStatus::Failed {
                        during: "check",
                        reason,
                    });
                    return;
                }
            };
            match (&outcome, decide(settings, &outcome)) {
                (CheckOutcome::UpToDate, _) => {
                    // Report "up to date" whether or not the user asked: a
                    // silent no-op after an explicit check reads as a hang.
                    let _ = user_asked;
                    updates.emit(UpdateStatus::UpToDate {
                        current: transport.current_version(),
                    });
                }
                (_, NextStep::AskUser(version)) => {
                    updates.emit(UpdateStatus::Available { version });
                }
                (_, NextStep::Download(version)) => {
                    download(transport, &version, settings, updates);
                }
                (_, NextStep::Rest) => {
                    updates.emit(UpdateStatus::UpToDate {
                        current: transport.current_version(),
                    });
                }
            }
        }
        UpdateCommand::Download { version, settings } => {
            download(transport, &version, settings, updates);
        }
        UpdateCommand::ApplyAndRestart => {
            // Normally does not return: the process is replaced.
            if let Err(reason) = transport.apply_and_restart() {
                updates.emit(UpdateStatus::Failed {
                    during: "restart",
                    reason,
                });
            }
        }
    }
}

fn download<T: UpdateTransport>(
    transport: &T,
    version: &str,
    settings: UpdateSettings,
    updates: &Emitter<UpdateStatus>,
) {
    updates.emit(UpdateStatus::Downloading {
        version: version.to_string(),
    });
    match transport.download(version) {
        Ok(()) => updates.emit(after_download(settings, version.to_string())),
        Err(reason) => updates.emit(UpdateStatus::Failed {
            during: "download",
            reason,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{UpdateChannel, UpdatePolicy, Unsupported};
    use std::sync::{Arc, Mutex};

    /// A transport that answers from a script, and records what it was asked.
    #[derive(Clone)]
    struct FakeTransport {
        availability: Result<(), Unsupported>,
        check: Result<CheckOutcome, String>,
        download: Result<(), String>,
        downloaded: Arc<Mutex<Vec<String>>>,
    }

    impl FakeTransport {
        fn offering(version: &str) -> Self {
            Self {
                availability: Ok(()),
                check: Ok(CheckOutcome::Update(version.to_string())),
                download: Ok(()),
                downloaded: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl UpdateTransport for FakeTransport {
        fn availability(&self) -> Result<(), Unsupported> {
            self.availability.clone()
        }
        fn current_version(&self) -> String {
            "0.1.0".to_string()
        }
        fn check(&self, _channel: UpdateChannel) -> Result<CheckOutcome, String> {
            self.check.clone()
        }
        fn download(&self, version: &str) -> Result<(), String> {
            self.downloaded.lock().unwrap().push(version.to_string());
            self.download.clone()
        }
        fn apply_and_restart(&self) -> Result<(), String> {
            Ok(())
        }
    }

    fn drain(transport: &FakeTransport, command: UpdateCommand) -> Vec<UpdateStatus> {
        let wake: Wake = Arc::new(|| {});
        let (worker, updates) = spawn_update_worker(wake, transport.clone());
        worker.command(command);
        // Dropping the handle closes the command channel, ending the actor
        // loop; the emitter closes with it, so `iter` terminates.
        drop(worker);
        updates.iter().collect()
    }

    fn settings(policy: UpdatePolicy) -> UpdateSettings {
        UpdateSettings {
            policy,
            ..Default::default()
        }
    }

    #[test]
    fn notify_only_reports_availability_without_downloading() {
        let transport = FakeTransport::offering("0.2.0");
        let seen = drain(
            &transport,
            UpdateCommand::Check {
                settings: settings(UpdatePolicy::NotifyOnly),
                user_asked: true,
            },
        );
        assert_eq!(
            seen,
            vec![
                UpdateStatus::Checking,
                UpdateStatus::Available {
                    version: "0.2.0".into()
                }
            ]
        );
        assert!(
            transport.downloaded.lock().unwrap().is_empty(),
            "notify-only must not fetch anything"
        );
    }

    #[test]
    fn automatic_downloads_and_stages_but_does_not_restart() {
        let transport = FakeTransport::offering("0.2.0");
        let seen = drain(
            &transport,
            UpdateCommand::Check {
                settings: settings(UpdatePolicy::Automatic),
                user_asked: false,
            },
        );
        assert_eq!(
            seen,
            vec![
                UpdateStatus::Checking,
                UpdateStatus::Downloading {
                    version: "0.2.0".into()
                },
                UpdateStatus::ReadyToRestart {
                    version: "0.2.0".into()
                },
            ],
            "even automatic stages and waits for a restart"
        );
        assert_eq!(&*transport.downloaded.lock().unwrap(), &["0.2.0"]);
    }

    #[test]
    fn a_failed_check_keeps_its_reason() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.check = Err("feed unreachable".into());
        let seen = drain(
            &transport,
            UpdateCommand::Check {
                settings: settings(UpdatePolicy::NotifyOnly),
                user_asked: true,
            },
        );
        assert_eq!(
            seen,
            vec![
                UpdateStatus::Checking,
                UpdateStatus::Failed {
                    during: "check",
                    reason: "feed unreachable".into()
                }
            ]
        );
    }

    #[test]
    fn a_failed_download_keeps_its_reason() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.download = Err("connection reset".into());
        let seen = drain(
            &transport,
            UpdateCommand::Download {
                version: "0.2.0".into(),
                settings: settings(UpdatePolicy::DownloadThenAsk),
            },
        );
        assert_eq!(
            seen,
            vec![
                UpdateStatus::Downloading {
                    version: "0.2.0".into()
                },
                UpdateStatus::Failed {
                    during: "download",
                    reason: "connection reset".into()
                }
            ]
        );
    }

    #[test]
    fn an_uninstalled_build_reports_unsupported_for_any_command() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.availability = Err(Unsupported::NotInstalled);
        let seen = drain(
            &transport,
            UpdateCommand::Check {
                settings: settings(UpdatePolicy::Automatic),
                user_asked: true,
            },
        );
        assert_eq!(seen, vec![UpdateStatus::Unsupported(Unsupported::NotInstalled)]);
        assert!(transport.downloaded.lock().unwrap().is_empty());
    }

    #[test]
    fn policy_off_reports_disabled_and_never_checks() {
        let transport = FakeTransport::offering("0.2.0");
        let seen = drain(
            &transport,
            UpdateCommand::Check {
                settings: settings(UpdatePolicy::Off),
                user_asked: false,
            },
        );
        assert_eq!(seen, vec![UpdateStatus::Disabled]);
    }

    #[test]
    fn an_up_to_date_check_says_so_rather_than_going_quiet() {
        let mut transport = FakeTransport::offering("0.2.0");
        transport.check = Ok(CheckOutcome::UpToDate);
        let seen = drain(
            &transport,
            UpdateCommand::Check {
                settings: settings(UpdatePolicy::NotifyOnly),
                user_asked: true,
            },
        );
        assert_eq!(
            seen,
            vec![
                UpdateStatus::Checking,
                UpdateStatus::UpToDate {
                    current: "0.1.0".into()
                }
            ]
        );
    }
}
