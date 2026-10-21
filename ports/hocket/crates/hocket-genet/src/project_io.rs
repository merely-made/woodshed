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

use armillary::{ActorHandle, Correlated, Emitter, Wake, spawn};
use hocket_model::{NodeId, ProjectBundle, Session, SessionId, TrackId};
use hocket_project::export::{ExportLength, render_mix, write_stereo_wav};
use hocket_project::handoff::{HandoffEnvelope, ReceivedHandoff};
use hocket_project::media::InMemoryStore;
use hocket_project::project_store::{LoadedProject, ProjectStore};
use muniment::ZipBackend;
use personae::Ed25519PublicKey;
use std::collections::BTreeSet;

/// Host-owned replacement/identity generation plus persisted session identity.
/// Neither value is serialized into a project or handoff envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectScope {
    pub generation: u64,
    pub session: SessionId,
}

pub struct ProjectRequest {
    pub scope: ProjectScope,
    pub command: Correlated<ProjectCommand>,
}

pub struct ProjectReply {
    pub scope: ProjectScope,
    pub update: Correlated<ProjectUpdate>,
}

pub enum ProjectCommand {
    LoadPack { path: PathBuf, before: Option<String> },
    ExportMidi { path: PathBuf, selection: audio_packs::PackSelection },
    RenderPack { selection: audio_packs::PackSelection, revision: String, track: TrackId, bpm: f32, meter: hocket_model::TimeSignature, bars: u8 },
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
    PackLoaded { before: Option<String>, selection: audio_packs::PackSelection },
    PackRendered { revision: String, track: TrackId, bpm: f32, meter: hocket_model::TimeSignature, bars: u8, samples: Vec<f32> },
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

pub fn spawn_project_worker(wake: Wake) -> (ActorHandle<ProjectRequest>, Receiver<ProjectReply>) {
    spawn(wake, |commands, updates| {
        while let Ok(command) = commands.recv() {
            run_command(command, &updates);
        }
    })
}

fn run_command(request: ProjectRequest, updates: &Emitter<ProjectReply>) {
    let scope = request.scope;
    let id = request.command.request;
    let emit = |value| {
        updates.emit(ProjectReply {
            scope,
            update: Correlated::new(id, value),
        })
    };
    match request.command.value {
        ProjectCommand::LoadPack { path, before } => {
            match audio_packs::files::load(&path, None) {
                Ok(selection) => emit(ProjectUpdate::PackLoaded { before, selection }),
                Err(e) => emit(ProjectUpdate::Failed { action: "load kit", message: e.to_string() }),
            }
        },
        ProjectCommand::ExportMidi { path, selection } => {
            match audio_packs::files::export_midi(&selection, &path) {
                Ok(()) => emit(ProjectUpdate::Exported { path }),
                Err(e) => emit(ProjectUpdate::Failed { action: "export MIDI", message: e.to_string() }),
            }
        },
        ProjectCommand::RenderPack { selection, revision, track, bpm, meter, bars } => {
            let result = selection.at_tempo(bpm).and_then(|p| p.render_bars(48_000, bars));
            match result {
                Ok(samples) => emit(ProjectUpdate::PackRendered { revision, track, bpm, meter, bars, samples }),
                Err(e) => emit(ProjectUpdate::Failed { action: "render drum layer", message: e.to_string() }),
            }
        },

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
                Ok(()) => emit(ProjectUpdate::Saved { path, saved_head }),
                Err(message) => emit(ProjectUpdate::Failed {
                    action: "save",
                    message,
                }),
            }
        },
        ProjectCommand::Open { path } => {
            let result = ZipBackend::open(&path)
                .map_err(|error| error.to_string())
                .and_then(|backend| {
                    pollster::block_on(ProjectStore::new(backend).load())
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(loaded) => emit(ProjectUpdate::Opened { path, loaded }),
                Err(message) => emit(ProjectUpdate::Failed {
                    action: "open",
                    message,
                }),
            }
        },
        ProjectCommand::ExportMix {
            path,
            session,
            media,
            solo,
            length,
        } => match render_mix(&session, &media, &solo, length)
            .and_then(|mix| write_stereo_wav(&path, &mix))
        {
            Ok(()) => emit(ProjectUpdate::Exported { path }),
            Err(error) => emit(ProjectUpdate::Failed {
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
                Ok(()) => emit(ProjectUpdate::HandoffWritten { path }),
                Err(message) => emit(ProjectUpdate::Failed {
                    action: "hand off",
                    message,
                }),
            }
        },
        ProjectCommand::ReadHandoff { path, recipient } => {
            let result = std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    HandoffEnvelope::from_bytes(&bytes).map_err(|error| error.to_string())
                })
                .and_then(|envelope| {
                    envelope
                        .receive(recipient)
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(received) => emit(ProjectUpdate::HandoffReceived { received }),
                Err(message) => emit(ProjectUpdate::Failed {
                    action: "open hand-off",
                    message,
                }),
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use hocket_model::{Edit, History, Layer, Phrase, Session};
    use hocket_project::media::MediaStore;
    use personae::{IdentityProvider, InMemoryProvider};

    fn test_scope() -> ProjectScope {
        ProjectScope {
            generation: 7,
            session: serde_json::from_str("\"00000000-0000-4000-8000-000000000023\"").unwrap(),
        }
    }

    fn receive(updates: &Receiver<ProjectReply>) -> ProjectUpdate {
        let reply = updates.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(reply.scope, test_scope());
        assert_eq!(reply.update.request, armillary::RequestId(23));
        reply.update.value
    }

    fn send(worker: &ActorHandle<ProjectRequest>, value: ProjectCommand) -> bool {
        worker.command(ProjectRequest {
            scope: test_scope(),
            command: Correlated::new(armillary::RequestId(23), value),
        })
    }

    #[test]
    fn worker_saves_then_opens_a_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("practice.hock");
        let wake: Wake = Arc::new(|| {});
        let (worker, updates) = spawn_project_worker(wake);
        let bundle = ProjectBundle::new(Session::new_default(), History::new());
        let saved_head = bundle.history.head;

        assert!(send(
            &worker,
            ProjectCommand::Save {
                path: path.clone(),
                bundle: bundle.clone(),
                media: InMemoryStore::new(),
                saved_head,
            }
        ));
        match receive(&updates) {
            ProjectUpdate::Saved {
                path: saved,
                saved_head: head,
            } => {
                assert_eq!(saved, path);
                assert_eq!(head, saved_head);
            },
            _ => panic!("expected save result"),
        }

        assert!(send(&worker, ProjectCommand::Open { path }));
        match receive(&updates) {
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

        assert!(send(
            &worker,
            ProjectCommand::ExportMix {
                path: path.clone(),
                session,
                media,
                solo: BTreeSet::new(),
                length: ExportLength::OneCycle,
            }
        ));
        match receive(&updates) {
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

        assert!(send(
            &worker,
            ProjectCommand::WriteHandoff {
                path: path.clone(),
                envelope,
            }
        ));
        match receive(&updates) {
            ProjectUpdate::HandoffWritten { path: written } => assert_eq!(written, path),
            _ => panic!("expected hand-off written"),
        }
        assert!(path.is_file());

        assert!(send(
            &worker,
            ProjectCommand::ReadHandoff {
                path,
                recipient: me.master_public_key(),
            }
        ));
        match receive(&updates) {
            ProjectUpdate::HandoffReceived { received } => {
                assert_eq!(received.bundle, bundle);
                assert_eq!(received.media.len(), 1);
                assert_eq!(received.sender, me.master_public_key());
            },
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

        assert!(send(
            &worker,
            ProjectCommand::WriteHandoff {
                path: path.clone(),
                envelope,
            }
        ));
        match receive(&updates) {
            ProjectUpdate::HandoffWritten { .. } => {},
            _ => panic!("expected hand-off written"),
        }

        // A hand-off addressed to someone else must surface an error, and the
        // worker must survive it (a later read still gets served).
        assert!(send(
            &worker,
            ProjectCommand::ReadHandoff {
                path,
                recipient: other.master_public_key(),
            }
        ));
        match receive(&updates) {
            ProjectUpdate::Failed { action, .. } => assert_eq!(action, "open hand-off"),
            _ => panic!("expected a surfaced failure"),
        }
        drop(worker);
    }
    #[test]
    fn every_worker_failure_preserves_the_exact_request_and_scope() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing-parent").join("out");
        let identity = InMemoryProvider::from_seed([33; 32]);
        let bundle = ProjectBundle::new(Session::new_default(), History::new());
        let envelope = HandoffEnvelope::create(
            &bundle,
            &InMemoryStore::new(),
            identity.master_public_key(),
            &identity,
        )
        .unwrap();
        let commands = [
            ProjectCommand::Save {
                path: dir.path().to_owned(),
                saved_head: bundle.history.head,
                bundle: bundle.clone(),
                media: InMemoryStore::new(),
            },
            ProjectCommand::Open {
                path: missing.clone(),
            },
            ProjectCommand::ExportMix {
                path: missing.clone(),
                session: bundle.session.clone(),
                media: InMemoryStore::new(),
                solo: BTreeSet::new(),
                length: ExportLength::OneCycle,
            },
            ProjectCommand::WriteHandoff {
                path: missing.clone(),
                envelope,
            },
            ProjectCommand::ReadHandoff {
                path: missing,
                recipient: identity.master_public_key(),
            },
        ];
        let (worker, updates) = spawn_project_worker(Arc::new(|| {}));
        for (index, command) in commands.into_iter().enumerate() {
            let id = armillary::RequestId(100 + index as u64);
            let scope = ProjectScope {
                generation: index as u64,
                session: SessionId::new(),
            };
            assert!(worker.command(ProjectRequest {
                scope,
                command: Correlated::new(id, command)
            }));
            let reply = updates.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(reply.scope, scope);
            assert_eq!(reply.update.request, id);
            assert!(matches!(reply.update.value, ProjectUpdate::Failed { .. }));
        }
    }
    /// Explicit filesystem fixture; never opens a device or user identity.
    #[test]
    #[ignore = "requires an owned HOCKET_PROJECT_NATIVE_FIXTURE_DIR"]
    fn prepare_native_project_fixture() {
        let directory = std::path::PathBuf::from(std::env::var_os("HOCKET_PROJECT_NATIVE_FIXTURE_DIR").expect("owned fixture directory"));
        assert!(directory.is_absolute() && directory.is_dir());
        let path = directory.join("fixture.hock");
        let wav = directory.join("reference.wav");
        assert!(!path.exists() && !wav.exists(), "never overwrite fixture artifacts");
        let mut session = Session::new_default();
        let mut history = History::new();
        let mut media = InMemoryStore::new();
        let samples: Vec<f32> = (0..48_000).map(|i| [0.25, -0.5, 0.75][i % 3]).collect();
        let reference = media.put(&samples, 48_000);
        let phrase = Phrase::new(reference, session.bars_per_phrase, session.bpm, 1);
        let layer = Layer::new(phrase.id);
        history.commit(Edit::AppendLayer { track_id: session.tracks[0].id, phrase, layer }, &mut session, 1);
        let mix = render_mix(&session, &media, &BTreeSet::new(), ExportLength::OneCycle).unwrap();
        write_stereo_wav(&wav, &mix).unwrap();
        let bundle = ProjectBundle::new(session, history);
        pollster::block_on(ProjectStore::new(ZipBackend::open(&path).unwrap()).save(&bundle, &media)).unwrap();
        let stored = pollster::block_on(ProjectStore::new(ZipBackend::open(&path).unwrap()).load()).unwrap();
        assert_eq!(stored.bundle, bundle);
        assert_eq!(stored.media.get(&reference).unwrap().samples.iter().map(|s| s.to_bits()).collect::<Vec<_>>(), samples.iter().map(|s| s.to_bits()).collect::<Vec<_>>());
        std::fs::write(directory.join("fixture.json"), serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();
    }

}
