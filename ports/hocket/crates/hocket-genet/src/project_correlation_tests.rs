// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::sync::{Arc, mpsc::Receiver};
use std::time::Duration;

fn make_state(project: ActorHandle<ProjectRequest>) -> AppState {
    let (updater, _) = armillary::spawn::<crate::update::worker::UpdateCommand, (), _>(
        Arc::new(|| {}),
        |commands, _| while commands.recv().is_ok() {},
    );
    AppState::new(
        project,
        updater,
        Default::default(),
        Err("CPU correlation fixture".into()),
        true,
    )
}
fn fixture() -> (AppState, Receiver<ProjectRequest>) {
    let (worker, requests) = armillary::spawn::<ProjectRequest, ProjectRequest, _>(
        Arc::new(|| {}),
        |commands, updates| {
            while let Ok(request) = commands.recv() {
                updates.emit(request);
            }
        },
    );
    (make_state(worker), requests)
}
fn next(requests: &Receiver<ProjectRequest>) -> ProjectRequest {
    requests.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn save(state: &AppState, path: &str) -> ProjectCommand {
    ProjectCommand::Save {
        path: path.into(),
        saved_head: state.recorder.history.head,
        bundle: ProjectBundle::new(
            state.recorder.session.clone(),
            state.recorder.history.clone(),
        ),
        media: state.recorder.store.clone(),
    }
}
fn reply(request: &ProjectRequest, value: ProjectUpdate) -> ProjectReply {
    ProjectReply {
        scope: request.scope,
        update: Correlated::new(request.command.request, value),
    }
}
fn handoff(state: &AppState) -> (HandoffEnvelope, Ed25519PublicKey) {
    let identity = personae::InMemoryProvider::from_seed([41; 32]);
    let key = identity.master_public_key();
    (
        HandoffEnvelope::create(
            &ProjectBundle::new(
                state.recorder.session.clone(),
                state.recorder.history.clone(),
            ),
            &state.recorder.store,
            key,
            &identity,
        )
        .unwrap(),
        key,
    )
}
#[test]
fn status_changes_do_not_release_pending_work_and_duplicates_do_nothing() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(save(&state, "one.hock")));
    let request = next(&requests);
    state.project_status = ProjectStatus::TokenCopied;
    assert!(state.is_project_io_active());
    assert!(!state.submit_project(ProjectCommand::Open {
        path: "two.hock".into()
    }));
    state.apply_project_update(reply(
        &request,
        ProjectUpdate::Saved {
            path: "one.hock".into(),
            saved_head: state.recorder.history.head,
        },
    ));
    assert!(!state.is_project_io_active());
    assert_eq!(state.project_label(), "one");
    state.apply_project_update(reply(
        &request,
        ProjectUpdate::Saved {
            path: "wrong.hock".into(),
            saved_head: state.recorder.history.head,
        },
    ));
    state.apply_project_update(reply(
        &request,
        ProjectUpdate::Failed {
            action: "save",
            message: "late".into(),
        },
    ));
    assert_eq!(state.project_label(), "one");
    assert_eq!(state.project_status_label(), "saved");
}
#[test]
fn unsolicited_results_never_associate_a_file_or_change_status() {
    let (mut state, _) = fixture();
    let before = state.project_status_label();
    state.apply_project_update(ProjectReply {
        scope: state.project_scope(),
        update: Correlated::new(
            RequestId(1),
            ProjectUpdate::Saved {
                path: "unsolicited.hock".into(),
                saved_head: state.recorder.history.head,
            },
        ),
    });
    assert!(state.project_path.is_none());
    assert_eq!(state.project_status_label(), before);
}
#[test]
fn same_session_acceptance_retires_old_save_and_old_failure_cannot_release_new_work() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(save(&state, "old.hock")));
    let old = next(&requests);
    let (envelope, key) = handoff(&state);
    state.incoming = Some(envelope.receive(key).unwrap());
    let session = state.recorder.session.id;
    state.accept_incoming();
    assert_eq!(state.recorder.session.id, session);
    assert_ne!(state.project_scope(), old.scope);
    assert!(state.submit_project(save(&state, "new.hock")));
    let current = next(&requests);
    assert_ne!(current.command.request, old.command.request);
    state.apply_project_update(reply(
        &old,
        ProjectUpdate::Failed {
            action: "save",
            message: "old failure".into(),
        },
    ));
    assert!(state.is_project_io_active());
    assert_eq!(state.project_status_label(), "saving");
    state.apply_project_update(reply(
        &old,
        ProjectUpdate::Saved {
            path: "old.hock".into(),
            saved_head: state.recorder.history.head,
        },
    ));
    assert!(state.project_path.is_none());
    state.apply_project_update(reply(
        &current,
        ProjectUpdate::Saved {
            path: "new.hock".into(),
            saved_head: state.recorder.history.head,
        },
    ));
    assert!(!state.is_project_io_active());
    assert_eq!(state.project_label(), "new");
}
#[test]
fn wrong_scope_cannot_clear_the_admitted_request() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(save(&state, "kept.hock")));
    let request = next(&requests);
    let mut wrong = reply(
        &request,
        ProjectUpdate::Failed {
            action: "save",
            message: "foreign".into(),
        },
    );
    wrong.scope.generation += 1;
    state.apply_project_update(wrong);
    assert!(state.is_project_io_active());
    state.apply_project_update(reply(
        &request,
        ProjectUpdate::Failed {
            action: "save",
            message: "real failure".into(),
        },
    ));
    assert!(!state.is_project_io_active());
    assert_eq!(
        state.project_status_label(),
        "project error: save: real failure"
    );
}
#[test]
fn handoff_read_and_write_reserve_busy_and_stale_incoming_is_not_staged() {
    let (mut state, requests) = fixture();
    let (envelope, key) = handoff(&state);
    assert!(state.submit_project(ProjectCommand::WriteHandoff {
        path: "pass.hocket".into(),
        envelope
    }));
    let written = next(&requests);
    assert!(state.is_project_io_active());
    assert_eq!(state.project_status_label(), "writing hand-off");
    state.apply_project_update(reply(
        &written,
        ProjectUpdate::HandoffWritten {
            path: "pass.hocket".into(),
        },
    ));
    assert!(!state.is_project_io_active());
    assert!(state.submit_project(ProjectCommand::ReadHandoff {
        path: "pass.hocket".into(),
        recipient: key
    }));
    let read = next(&requests);
    assert!(state.is_project_io_active());
    assert_eq!(state.project_status_label(), "opening hand-off");
    let (incoming, _) = handoff(&state);
    state.retire_project_work();
    state.apply_project_update(reply(
        &read,
        ProjectUpdate::HandoffReceived {
            received: incoming.receive(key).unwrap(),
        },
    ));
    assert!(state.incoming.is_none());
}
#[test]
fn edits_during_save_remain_dirty_after_matching_completion() {
    let (mut state, requests) = fixture();
    let saved_head = state.recorder.history.head;
    assert!(state.submit_project(save(&state, "snapshot.hock")));
    let request = next(&requests);
    state.recorder.add_track();
    state.tick();
    assert_ne!(state.recorder.history.head, saved_head);
    state.apply_project_update(reply(
        &request,
        ProjectUpdate::Saved {
            path: "snapshot.hock".into(),
            saved_head,
        },
    ));
    assert!(state.is_dirty());
    assert_eq!(state.project_status_label(), "unsaved changes");
}
#[test]
fn disconnection_releases_pending_without_claiming_success() {
    let (done, stopped) = std::sync::mpsc::channel();
    let (worker, _) =
        armillary::spawn::<ProjectRequest, ProjectReply, _>(Arc::new(|| {}), move |commands, _| {
            drop(commands);
            done.send(()).unwrap();
        });
    stopped.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut state = make_state(worker);
    assert!(!state.submit_project(save(&state, "never.hock")));
    assert!(!state.is_project_io_active());
    assert_eq!(
        state.project_status_label(),
        "project error: project worker stopped"
    );
    assert!(state.project_path.is_none());
}

#[test]
fn stale_open_cannot_replace_current_session_or_associate_its_path() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(ProjectCommand::Open {
        path: "old.hock".into()
    }));
    let old = next(&requests);
    state.retire_project_work();
    let current = state.recorder.session.id;
    let loaded = hocket_project::project_store::LoadedProject {
        bundle: ProjectBundle::new(Session::new_default(), History::new()),
        media: InMemoryStore::new(),
        missing_media: BTreeSet::new(),
    };
    state.apply_project_update(reply(
        &old,
        ProjectUpdate::Opened {
            path: "old.hock".into(),
            loaded,
        },
    ));
    assert_eq!(state.recorder.session.id, current);
    assert!(state.project_path.is_none());
}

#[test]
fn matching_incoming_is_staged_and_matching_open_retires_it() {
    let (mut state, requests) = fixture();
    let (envelope, key) = handoff(&state);
    assert!(state.submit_project(ProjectCommand::ReadHandoff {
        path: "pass.hocket".into(),
        recipient: key
    }));
    let read = next(&requests);
    state.apply_project_update(reply(
        &read,
        ProjectUpdate::HandoffReceived {
            received: envelope.receive(key).unwrap(),
        },
    ));
    assert!(state.incoming.is_some());
    assert!(!state.is_project_io_active());
    assert!(state.submit_project(ProjectCommand::Open {
        path: "new.hock".into()
    }));
    let open = next(&requests);
    let loaded = hocket_project::project_store::LoadedProject {
        bundle: ProjectBundle::new(Session::new_default(), History::new()),
        media: InMemoryStore::new(),
        missing_media: BTreeSet::new(),
    };
    let expected = loaded.bundle.session.id;
    state.apply_project_update(reply(
        &open,
        ProjectUpdate::Opened {
            path: "new.hock".into(),
            loaded,
        },
    ));
    assert_eq!(state.recorder.session.id, expected);
    assert!(state.incoming.is_none());
    assert!(!state.is_project_io_active());
    assert_eq!(state.project_label(), "new");
}

#[test]
fn real_worker_round_trip_connects_save_open_and_handoff_to_the_live_host() {
    let dir = tempfile::tempdir().unwrap();
    let (worker, updates) = crate::project_io::spawn_project_worker(Arc::new(|| {}));
    let mut state = make_state(worker);
    state.recorder.add_track();
    state.tick();
    let tracks = state.recorder.session.tracks.len();
    assert!(state.submit_project(save(
        &state,
        dir.path().join("saved.hock").to_str().unwrap()
    )));
    state.apply_project_update(updates.recv_timeout(Duration::from_secs(5)).unwrap());
    assert_eq!(state.project_label(), "saved");
    assert!(!state.is_dirty());
    state.recorder.add_track();
    state.tick();
    assert!(state.is_dirty());
    let before = state.project_scope();
    assert!(state.submit_project(ProjectCommand::Open {
        path: dir.path().join("saved.hock")
    }));
    state.apply_project_update(updates.recv_timeout(Duration::from_secs(5)).unwrap());
    assert_ne!(state.project_scope(), before);
    assert_eq!(state.recorder.session.tracks.len(), tracks);
    assert!(!state.is_dirty());
    let (envelope, key) = handoff(&state);
    let path = dir.path().join("pass.hocket");
    assert!(state.submit_project(ProjectCommand::WriteHandoff {
        path: path.clone(),
        envelope
    }));
    assert!(state.is_project_io_active());
    state.apply_project_update(updates.recv_timeout(Duration::from_secs(5)).unwrap());
    assert!(path.is_file());
    assert!(!state.is_project_io_active());
    assert!(state.submit_project(ProjectCommand::ReadHandoff {
        path,
        recipient: key
    }));
    state.apply_project_update(updates.recv_timeout(Duration::from_secs(5)).unwrap());
    assert!(state.incoming.is_some());
    assert!(!state.is_project_io_active());
    assert_eq!(state.project_label(), "saved");
}
