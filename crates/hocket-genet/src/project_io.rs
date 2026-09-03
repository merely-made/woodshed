// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Background project save/open work for the Genet host.
//!
//! The host kernel keeps the live audio engine and session state. This actor
//! receives cloned save snapshots or a project path, performs zip-archive
//! project I/O away from the kernel thread, then reports a typed result for the
//! next frame to apply.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use armillary::{ActorHandle, Emitter, Wake, spawn};
use muniment::ZipBackend;
use std::collections::BTreeSet;
use hocket_engine::export::{ExportLength, render_mix, write_stereo_wav};
use hocket_engine::handoff::{HandoffEnvelope, ReceivedHandoff};
use hocket_engine::media::InMemoryStore;
use hocket_engine::project_store::{LoadedProject, ProjectStore};
use hocket_model::{NodeId, ProjectBundle, Session, TrackId};
use personae::Ed25519PublicKey;

pub enum ProjectCommand {
    Save {
        path: PathBuf,
        bundle: ProjectBundle,
        media: InMemoryStore,
        saved_head: NodeId,
    },
    Open {
        path: PathBuf,
    },
    ExportMix {
        path: PathBuf,
        session: Session,
        media: InMemoryStore,
        solo: BTreeSet<TrackId>,
        length: ExportLength,
    },
    /// Serialize an already-signed hand-off envelope and write it to `path`.
    /// The main thread builds the envelope (signing needs the private identity);
    /// the worker only pays the CBOR-over-media serialization and the file I/O.
    WriteHandoff {
        path: PathBuf,
        envelope: HandoffEnvelope,
    },
    /// Read a hand-off file, authenticate it against `recipient`, and
    /// materialize its snapshot for review. Verification needs only the
    /// recipient's public key, so the whole receive runs off the kernel thread.
    ReadHandoff {
        path: PathBuf,
        recipient: Ed25519PublicKey,
    },
}

pub enum ProjectUpdate {
    Saved {
        path: PathBuf,
        saved_head: NodeId,
    },
    Opened {
        path: PathBuf,
        loaded: LoadedProject,
    },
    Exported {
        path: PathBuf,
    },
    /// A hand-off envelope was serialized and written to `path`.
    HandoffWritten {
        path: PathBuf,
    },
    /// An incoming hand-off authenticated and materialized, ready to stage for
    /// review. It is not applied to the live session until the host accepts it.
    HandoffReceived {
        received: ReceivedHandoff,
    },
    Failed {
        action: &'static str,
        message: String,
    },
}

pub fn spawn_project_worker(wake: Wake) -> (ActorHandle<ProjectCommand>, Receiver<ProjectUpdate>) {
    spawn(wake, |commands, updates| {
        while let Ok(command) = commands.recv() {
            run_command(command, &updates);
        }
    })
}

fn run_command(command: ProjectCommand, updates: &Emitter<ProjectUpdate>) {
    match command {
        ProjectCommand::Save {
            path,
            bundle,
            media,
            saved_head,
        } => {
            let result = ZipBackend::open(&path)
                .map_err(|error| error.to_string())
                .and_then(|backend| {
                    pollster::block_on(ProjectStore::new(backend).save(&bundle, &media))
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => updates.emit(ProjectUpdate::Saved { path, saved_head }),
                Err(message) => updates.emit(ProjectUpdate::Failed {
                    action: "save",
                    message,
                }),
            }
        }
        ProjectCommand::Open { path } => {
            let result = ZipBackend::open(&path)
                .map_err(|error| error.to_string())
                .and_then(|backend| {
                    pollster::block_on(ProjectStore::new(backend).load())
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(loaded) => updates.emit(ProjectUpdate::Opened { path, loaded }),
                Err(message) => updates.emit(ProjectUpdate::Failed {
                    action: "open",
                    message,
                }),
            }
        }
        ProjectCommand::ExportMix {
            path,
            session,
            media,
            solo,
            length,
        } => match render_mix(&session, &media, &solo, length)
            .and_then(|mix| write_stereo_wav(&path, &mix))
        {
            Ok(()) => updates.emit(ProjectUpdate::Exported { path }),
            Err(error) => updates.emit(ProjectUpdate::Failed {
                action: "export",
                message: error.to_string(),
            }),
        },
        ProjectCommand::WriteHandoff { path, envelope } => {
            let result = envelope
                .to_bytes()
                .map_err(|error| error.to_string())
                .and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()));
            match result {
                Ok(()) => updates.emit(ProjectUpdate::HandoffWritten { path }),
                Err(message) => updates.emit(ProjectUpdate::Failed {
                    action: "hand off",
                    message,
                }),
            }
        }
        ProjectCommand::ReadHandoff { path, recipient } => {
            let result = std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    HandoffEnvelope::from_bytes(&bytes).map_err(|error| error.to_string())
                })
                .and_then(|envelope| envelope.receive(recipient).map_err(|error| error.to_string()));
            match result {
                Ok(received) => updates.emit(ProjectUpdate::HandoffReceived { received }),
                Err(message) => updates.emit(ProjectUpdate::Failed {
                    action: "open hand-off",
                    message,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use hocket_engine::media::MediaStore;
    use hocket_model::{Edit, History, Layer, Phrase, Session};
    use personae::{IdentityProvider, InMemoryProvider};

    #[test]
    fn worker_saves_then_opens_a_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("practice.hock");
        let wake: Wake = Arc::new(|| {});
        let (worker, updates) = spawn_project_worker(wake);
        let bundle = ProjectBundle::new(Session::new_default(), History::new());
        let saved_head = bundle.history.head;

        assert!(worker.command(ProjectCommand::Save {
            path: path.clone(),
            bundle: bundle.clone(),
            media: InMemoryStore::new(),
            saved_head,
        }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::Saved {
                path: saved,
                saved_head: head,
            } => {
                assert_eq!(saved, path);
                assert_eq!(head, saved_head);
            }
            _ => panic!("expected save result"),
        }

        assert!(worker.command(ProjectCommand::Open { path }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::Opened { loaded, .. } => assert_eq!(loaded.bundle, bundle),
            _ => panic!("expected open result"),
        }
        drop(worker);
    }

    #[test]
    fn worker_exports_audible_mix_to_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("practice-mix.wav");
        let wake: Wake = Arc::new(|| {});
        let (worker, updates) = spawn_project_worker(wake);
        let mut session = Session::new_default();
        let mut media = InMemoryStore::new();
        let reference = media.put(&[0.25, -0.25], 48_000);
        let phrase = Phrase::new(reference, session.bars_per_phrase, session.bpm, 1);
        session.phrases.insert(phrase.id, phrase.clone());
        session.tracks[0].layers.push(Layer::new(phrase.id));

        assert!(worker.command(ProjectCommand::ExportMix {
            path: path.clone(),
            session,
            media,
            solo: BTreeSet::new(),
            length: ExportLength::OneCycle,
        }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::Exported { path: exported } => assert_eq!(exported, path),
            _ => panic!("expected export result"),
        }
        assert!(path.is_file());
        drop(worker);
    }

    fn bundle_with_media(store: &mut InMemoryStore) -> ProjectBundle {
        let mut session = Session::new_default();
        let mut history = History::new();
        let reference = store.put(&[0.25, -0.5, 0.75], 48_000);
        let phrase = Phrase::new(reference, session.bars_per_phrase, session.bpm, 1);
        let layer = Layer::new(phrase.id);
        history.commit(
            Edit::AppendLayer {
                track_id: session.tracks[0].id,
                phrase,
                layer,
            },
            &mut session,
            1,
        );
        ProjectBundle::new(session, history)
    }

    #[test]
    fn worker_writes_then_reads_a_self_addressed_handoff() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pass.hocket");
        let wake: Wake = Arc::new(|| {});
        let (worker, updates) = spawn_project_worker(wake);

        let me = InMemoryProvider::from_seed([1; 32]);
        let mut store = InMemoryStore::new();
        let bundle = bundle_with_media(&mut store);
        let envelope =
            HandoffEnvelope::create(&bundle, &store, me.master_public_key(), &me).unwrap();

        assert!(worker.command(ProjectCommand::WriteHandoff {
            path: path.clone(),
            envelope,
        }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::HandoffWritten { path: written } => assert_eq!(written, path),
            _ => panic!("expected hand-off written"),
        }
        assert!(path.is_file());

        assert!(worker.command(ProjectCommand::ReadHandoff {
            path,
            recipient: me.master_public_key(),
        }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::HandoffReceived { received } => {
                assert_eq!(received.bundle, bundle);
                assert_eq!(received.media.len(), 1);
                assert_eq!(received.sender, me.master_public_key());
            }
            _ => panic!("expected hand-off received"),
        }
        drop(worker);
    }

    #[test]
    fn reading_a_handoff_for_another_recipient_fails_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pass.hocket");
        let wake: Wake = Arc::new(|| {});
        let (worker, updates) = spawn_project_worker(wake);

        let sender = InMemoryProvider::from_seed([1; 32]);
        let recipient = InMemoryProvider::from_seed([2; 32]);
        let other = InMemoryProvider::from_seed([3; 32]);
        let mut store = InMemoryStore::new();
        let bundle = bundle_with_media(&mut store);
        let envelope =
            HandoffEnvelope::create(&bundle, &store, recipient.master_public_key(), &sender)
                .unwrap();

        assert!(worker.command(ProjectCommand::WriteHandoff {
            path: path.clone(),
            envelope,
        }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::HandoffWritten { .. } => {}
            _ => panic!("expected hand-off written"),
        }

        // A hand-off addressed to someone else must surface an error, and the
        // worker must survive it (a later read still gets served).
        assert!(worker.command(ProjectCommand::ReadHandoff {
            path,
            recipient: other.master_public_key(),
        }));
        match updates.recv_timeout(Duration::from_secs(5)).unwrap() {
            ProjectUpdate::Failed { action, .. } => assert_eq!(action, "open hand-off"),
            _ => panic!("expected a surfaced failure"),
        }
        drop(worker);
    }
}
