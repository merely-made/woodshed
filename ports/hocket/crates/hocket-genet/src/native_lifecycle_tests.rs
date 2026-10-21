// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Device-free production host/state lifecycle regressions. Execution is owned
//! by the integration lane; no native audio adapter is constructed with devices.
use super::*;
use super::project_correlation_tests::{fixture, handoff, next, reply, save};
use hocket_project::project_store::LoadedProject;
use hocket_recorder::{
    AudioAvailability, AudioObservation, CaptureCompletion, CaptureMode, CapturePhase,
    CaptureToken, LayerKey, RecorderAudio,
};

fn edit(state: &mut AppState) {
    state.recorder.add_track();
    state.tick();
    assert!(state.is_dirty());
}

fn loaded() -> LoadedProject {
    LoadedProject {
        bundle: ProjectBundle::new(Session::new_default(), History::new()),
        media: InMemoryStore::new(),
        missing_media: BTreeSet::new(),
    }
}

#[test]
fn dirty_close_cancel_and_explicit_discard_preserve_the_live_revision_until_exit() {
    let (mut state, _requests) = fixture();
    edit(&mut state);
    let expected = ProjectRevision::current(&state.recorder);
    assert!(!state.request_project_close());
    assert!(expected.matches(&state.recorder));
    state.cancel_preservation();
    assert!(!state.project_close_ready());
    assert!(expected.matches(&state.recorder));
    assert!(!state.request_project_close());
    state.discard_for_preservation();
    assert!(state.request_project_close());
    // The close guard grants permission; it never destroys the live project.
    assert!(expected.matches(&state.recorder));
    edit(&mut state);
    assert!(!state.request_project_close(), "discard cannot approve later edits");
}

#[test]
fn cancelled_and_failed_save_keep_work_and_success_alone_continues_close() {
    let (mut state, requests) = fixture();
    edit(&mut state);
    assert!(!state.request_project_close());
    let expected = ProjectRevision::current(&state.recorder);
    state.save_project_at(None);
    assert!(!state.is_project_io_active());
    assert!(!state.project_close_ready());
    assert!(state.preservation_pending());
    assert!(expected.matches(&state.recorder));
    state.save_project_at(Some("retry.hock".into()));
    let failed = next(&requests);
    state.discard_for_preservation();
    assert!(!state.project_close_ready(), "an admitted save must finish first");
    state.apply_project_update(reply(&failed, ProjectUpdate::Failed {
        action: "save", message: "disk full".into(),
    }));
    assert!(state.is_dirty());
    assert!(expected.matches(&state.recorder));
    assert!(!state.project_close_ready());
    assert!(state.preservation_message().contains("disk full"));
    state.save_project_at(Some("retry.hock".into()));
    let saved = next(&requests);
    state.apply_project_update(reply(&saved, ProjectUpdate::Saved {
        path: "retry.hock".into(), saved_head: state.recorder.history.head,
    }));
    assert!(!state.is_dirty());
    assert!(state.request_project_close());
}

#[test]
fn close_during_in_flight_save_waits_and_acknowledges_only_submitted_revision() {
    let (mut state, requests) = fixture();
    edit(&mut state);
    let head = state.recorder.history.head;
    assert!(state.submit_project(save(&state, "snapshot.hock")));
    let pending = next(&requests);
    assert!(!state.request_project_close());
    state.discard_for_preservation();
    assert!(state.is_project_io_active());
    assert!(!state.project_close_ready());
    edit(&mut state);
    state.apply_project_update(reply(&pending, ProjectUpdate::Saved {
        path: "snapshot.hock".into(), saved_head: head,
    }));
    assert!(state.is_dirty());
    assert!(!state.request_project_close());
}

#[test]
fn cancellation_while_preservation_save_is_pending_does_not_close_on_success() {
    let (mut state, requests) = fixture();
    edit(&mut state);
    assert!(!state.request_project_close());
    state.save_project_at(Some("kept.hock".into()));
    let pending = next(&requests);
    state.cancel_preservation();
    state.apply_project_update(reply(&pending, ProjectUpdate::Saved {
        path: "kept.hock".into(), saved_head: state.recorder.history.head,
    }));
    assert!(!state.is_dirty());
    assert!(!state.project_close_ready());
    assert!(!state.preservation_pending());
}

#[test]
fn unchanged_head_media_added_during_save_remains_dirty() {
    let (mut state, requests) = fixture();
    let head = state.recorder.history.head;
    assert!(state.submit_project(save(&state, "before-media.hock")));
    let pending = next(&requests);
    state.recorder.store.put(&[0.25, -0.5], 48_000);
    assert_eq!(state.recorder.history.head, head);
    state.apply_project_update(reply(&pending, ProjectUpdate::Saved {
        path: "before-media.hock".into(), saved_head: head,
    }));
    assert!(state.is_dirty());
    assert!(!state.request_project_close());
}

#[test]
fn host_rejects_invalid_loaded_and_direct_handoff_replacements_without_consuming_payloads() {
    let (mut state, requests) = fixture();
    let expected = ProjectRevision::current(&state.recorder);
    let scope = state.project_scope();
    assert!(state.submit_project(ProjectCommand::Open { path: "invalid.hock".into() }));
    let open = next(&requests);
    let mut invalid = loaded();
    invalid.bundle.format_version += 1;
    state.apply_project_update(reply(&open, ProjectUpdate::Opened {
        path: "invalid.hock".into(), loaded: invalid,
    }));
    assert!(state.has_loaded_project());
    assert!(expected.matches(&state.recorder));
    assert_eq!(state.project_scope(), scope);
    assert!(state.project_path.is_none());
    assert!(state.project_status_label().contains("unsupported project format"));
    state.cancel_preservation();
    // Direct host callers cannot bypass the public bundle admission API.
    let (envelope, key) = handoff(&state);
    let mut received = envelope.receive(key).unwrap();
    received.bundle.format_version += 1;
    state.incoming = Some(received);
    state.accept_incoming();
    assert!(state.incoming.is_some());
    assert!(expected.matches(&state.recorder));
    assert_eq!(state.project_scope(), scope);
    assert!(state.has_loaded_project());
    assert!(state.project_status_label().contains("unsupported project format"));
}

#[test]
fn dirty_accept_failed_save_keeps_both_projects_until_explicit_discard() {
    let (mut state, requests) = fixture();
    let incoming = loaded();
    let expected_incoming = incoming.bundle.session.id;
    state.incoming = Some(ReceivedHandoff {
        sender: personae::InMemoryProvider::from_seed([12; 32]).master_public_key(),
        bundle: incoming.bundle,
        media: incoming.media,
    });
    edit(&mut state);
    let outgoing = ProjectRevision::current(&state.recorder);
    state.accept_incoming();
    state.save_project_at(None);
    assert!(outgoing.matches(&state.recorder));
    assert!(state.incoming.is_some());
    state.save_project_at(Some("outgoing.hock".into()));
    let save = next(&requests);
    state.apply_project_update(reply(&save, ProjectUpdate::Failed {
        action: "save", message: "permission denied".into(),
    }));
    assert!(outgoing.matches(&state.recorder));
    assert!(state.incoming.is_some());
    assert!(state.preservation_pending());
    state.discard_for_preservation();
    assert_eq!(state.recorder.session.id, expected_incoming);
    assert!(state.is_dirty(), "adopted new project has no durable saved revision");
    assert!(state.project_path.is_none());
}

#[test]
fn unchanged_saved_head_imports_retained_branch_and_media_as_dirty() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(save(&state, "root.hock")));
    let saved = next(&requests);
    let head = state.recorder.history.head;
    state.apply_project_update(reply(&saved, ProjectUpdate::Saved {
        path: "root.hock".into(), saved_head: head,
    }));
    let identity = personae::InMemoryProvider::from_seed([19; 32]);
    let key = identity.master_public_key();
    let mut bundle = ProjectBundle::new(state.recorder.session.clone(), state.recorder.history.clone());
    let mut media = state.recorder.store.clone();
    let reference = media.put(&[0.5, -0.25], 48_000);
    let phrase = hocket_model::Phrase::new(reference, 4, bundle.session.bpm, 10);
    let phrase_id = phrase.id;
    let layer = hocket_model::Layer::new(phrase_id);
    bundle.history.commit(hocket_model::Edit::AppendLayer {
        track_id: bundle.session.tracks[0].id, phrase, layer,
    }, &mut bundle.session, 10);
    bundle.history.checkout(head, &mut bundle.session).unwrap();
    let nodes = bundle.history.nodes.len();
    let envelope = HandoffEnvelope::create(&bundle, &media, key, &identity).unwrap();
    state.incoming = Some(envelope.receive(key).unwrap());
    state.accept_incoming();
    assert_eq!(state.recorder.history.head, head);
    assert_eq!(state.recorder.history.nodes.len(), nodes);
    assert!(state.recorder.session.phrases.contains_key(&phrase_id));
    assert!(state.recorder.store.get(&reference).is_some());
    assert!(state.is_dirty());
    assert!(!state.request_project_close());
}

#[test]
fn same_session_import_restores_one_missing_phrase_and_keeps_local_missing_facts() {
    let (mut state, _requests) = fixture();
    let mut sender_media = InMemoryStore::new();
    let restored_pcm = vec![0.5, -0.25, 0.75, -1.0];
    let restored = sender_media.put(&restored_pcm, 48_000);
    let phrase = hocket_model::Phrase::new(restored, 4, state.recorder.session.bpm, 10);
    let layer = hocket_model::Layer::new(phrase.id);
    state.recorder.history.commit(hocket_model::Edit::AppendLayer {
        track_id: state.recorder.session.tracks[0].id, phrase, layer,
    }, &mut state.recorder.session, 10);
    let sender_bundle = ProjectBundle::new(
        state.recorder.session.clone(), state.recorder.history.clone(),
    );
    let accepted_head = sender_bundle.history.head;

    // The local project also retains a branch the sender has never seen.
    // Both referenced blobs are missing locally, as after a partial-media open.
    let mut absent_media = InMemoryStore::new();
    let still_missing = absent_media.put(&[0.125, -0.375], 48_000);
    let local_phrase = hocket_model::Phrase::new(
        still_missing, 4, state.recorder.session.bpm, 20,
    );
    let local_layer = hocket_model::Layer::new(local_phrase.id);
    state.recorder.history.commit(hocket_model::Edit::AppendLayer {
        track_id: state.recorder.session.tracks[0].id,
        phrase: local_phrase, layer: local_layer,
    }, &mut state.recorder.session, 20);
    let local_head = state.recorder.history.head;
    let retained_pcm = vec![0.0625, -0.0625];
    let retained = state.recorder.store.put(&retained_pcm, 48_000);
    state.recorder.missing_media = BTreeSet::from([restored, still_missing]);
    // Simulate the already-opened, unchanged project without a save or device.
    state.saved_revision = ProjectRevision::current(&state.recorder);
    assert!(!state.is_dirty());
    assert!(state.recorder.store.get(&restored).is_none());

    let identity = personae::InMemoryProvider::from_seed([29; 32]);
    let key = identity.master_public_key();
    let envelope = HandoffEnvelope::create(
        &sender_bundle, &sender_media, key, &identity,
    ).unwrap();
    state.incoming = Some(envelope.receive(key).unwrap());
    state.accept_incoming();

    assert_eq!(state.recorder.history.head, accepted_head);
    assert!(state.recorder.history.nodes.contains_key(&local_head));
    assert!(state.recorder.session.phrases.values().any(|p| p.media == still_missing));
    assert_eq!(state.recorder.missing_media, BTreeSet::from([still_missing]));
    assert_eq!(state.recorder.store.get(&restored).unwrap().samples, restored_pcm);
    assert_eq!(state.recorder.store.get(&retained).unwrap().samples, retained_pcm);
    assert!(state.recorder.store.get(&still_missing).is_none());
    assert!(state.is_dirty());
}

/// Real recorder services, fake device execution. A stopped free capture emits
/// its accepted token back through the normal promotion path.
#[derive(Default)]
struct Audio {
    phase: CapturePhase,
    accepted: Option<CaptureToken>,
    completion: Option<CaptureCompletion>,
    stopped: Option<CaptureToken>,
}
impl RecorderAudio for Audio {
    fn availability(&self) -> AudioAvailability {
        AudioAvailability { sample_rate: 48_000, input: true, error: None }
    }
    fn poll(&mut self) -> AudioObservation {
        AudioObservation { phase: self.phase.clone(), completed: self.completion.take(), ..Default::default() }
    }
    fn set_clock(&mut self, _: f32, _: u8, _: u8, _: bool) -> Result<(), String> { Ok(()) }
    fn set_click(&mut self, _: bool) -> Result<(), String> { Ok(()) }
    fn begin_capture(&mut self, token: CaptureToken, mode: CaptureMode) -> Result<(), String> {
        assert_eq!(mode, CaptureMode::Free);
        self.accepted = Some(token);
        self.phase = CapturePhase::FreeRecording { samples_done: 8 };
        Ok(())
    }
    fn stop_free_capture(&mut self, token: CaptureToken) -> Result<(), String> {
        assert_eq!(self.accepted, Some(token), "the original stop token must survive refusal");
        self.stopped = Some(token);
        self.accepted = None;
        self.phase = CapturePhase::Complete;
        self.completion = Some(CaptureCompletion { token, sample_rate: 48_000, samples: vec![0.25; 8] });
        Ok(())
    }
    fn play_layer(&mut self, _: LayerKey, _: Vec<f32>, _: u32, _: f32, _: bool) -> Result<(), String> { Ok(()) }
    fn stop_layer(&mut self, _: LayerKey) -> Result<(), String> { Ok(()) }
}
fn service(state: &mut AppState, audio: &mut Audio) {
    state.recorder.service(audio, 100, 0.016);
}
fn start_free_capture(state: &mut AppState) -> Audio {
    let mut audio = Audio::default();
    state.recorder.toggle_master_clock();
    state.recorder.arm(0);
    service(state, &mut audio);
    state.recorder.toggle_record();
    service(state, &mut audio);
    assert!(state.is_recording());
    assert!(state.recorder.active_capture_token().is_some());
    audio
}
fn stop_free_capture(state: &mut AppState, audio: &mut Audio, token: CaptureToken) {
    state.recorder.toggle_record();
    service(state, audio);
    assert_eq!(audio.stopped, Some(token));
    service(state, audio);
    assert!(!state.is_recording());
    assert_eq!(state.recorder.session.tracks[0].layers.len(), 1);
}

#[test]
fn accept_during_free_capture_keeps_ticket_payload_and_playable_outgoing_take() {
    let (mut state, _requests) = fixture();
    let (envelope, key) = handoff(&state);
    state.incoming = Some(envelope.receive(key).unwrap());
    let mut audio = start_free_capture(&mut state);
    let token = state.recorder.active_capture_token().unwrap();
    let scope = state.project_scope();
    state.accept_incoming();
    state.discard_for_preservation();
    assert_eq!(state.project_scope(), scope);
    assert_eq!(state.recorder.active_capture_token(), Some(token));
    assert!(state.incoming.is_some());
    assert!(state.is_recording());
    assert!(!state.request_project_close());
    stop_free_capture(&mut state, &mut audio, token);
    state.retry_preservation();
    assert_eq!(state.project_scope(), scope, "newly promoted take needs a decision");
    state.cancel_preservation();
    assert!(state.incoming.is_some());
    assert!(state.is_dirty());
}

#[test]
fn close_during_free_capture_cannot_discard_its_stop_token() {
    let (mut state, _requests) = fixture();
    let mut audio = start_free_capture(&mut state);
    let token = state.recorder.active_capture_token().unwrap();
    assert!(!state.request_project_close());
    state.discard_for_preservation();
    assert!(!state.project_close_ready());
    assert_eq!(state.recorder.active_capture_token(), Some(token));
    stop_free_capture(&mut state, &mut audio, token);
    state.retry_preservation();
    assert!(!state.project_close_ready());
    assert!(state.is_dirty());
    state.discard_for_preservation();
    assert!(state.request_project_close());
}

#[test]
fn matching_open_rechecks_edits_made_after_dispatch_even_without_capture() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(ProjectCommand::Open { path: "incoming.hock".into() }));
    let open = next(&requests);
    edit(&mut state);
    let outgoing = ProjectRevision::current(&state.recorder);
    state.apply_project_update(reply(&open, ProjectUpdate::Opened {
        path: "incoming.hock".into(), loaded: loaded(),
    }));
    assert!(outgoing.matches(&state.recorder));
    assert!(state.has_loaded_project());
    assert!(state.preservation_pending());
    assert!(state.project_path.is_none());
}

#[test]
fn open_dispatched_then_capture_starts_completion_is_retained_for_explicit_retry() {
    let (mut state, requests) = fixture();
    assert!(state.submit_project(ProjectCommand::Open { path: "incoming.hock".into() }));
    let open = next(&requests);
    let mut audio = start_free_capture(&mut state);
    let token = state.recorder.active_capture_token().unwrap();
    let scope = state.project_scope();
    let incoming = loaded();
    let expected = incoming.bundle.session.id;
    state.apply_project_update(reply(&open, ProjectUpdate::Opened {
        path: "incoming.hock".into(), loaded: incoming,
    }));
    state.discard_for_preservation();
    assert_eq!(state.project_scope(), scope);
    assert_eq!(state.recorder.active_capture_token(), Some(token));
    assert!(state.has_loaded_project());
    assert!(state.project_path.is_none());
    stop_free_capture(&mut state, &mut audio, token);
    state.retry_preservation();
    assert_eq!(state.project_scope(), scope);
    state.cancel_preservation();
    assert!(state.has_loaded_project());
    state.retry_loaded_project();
    assert_eq!(state.project_scope(), scope, "dirty outgoing capture still protected");
    state.discard_for_preservation();
    assert_eq!(state.recorder.session.id, expected);
    assert_eq!(state.project_label(), "incoming");
    assert!(!state.has_loaded_project());
    assert!(!state.is_recording());
    assert!(!state.is_dirty());
}

type Host = cambium_genet_winit_host::Harness<AppState, crate::Logic, crate::Child>;
fn mounted() -> (Host, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (mut state, _requests) = fixture();
    state.appearance = crate::appearance::AppearanceState::load(
        dir.path().join("choice.json"), dir.path().join("themes.json"), vec![],
    );
    let sheet = state.appearance.take_stylesheet_change().unwrap();
    edit(&mut state);
    let runtime = std::rc::Rc::new(std::cell::RefCell::new(crate::Runtime::new(None)));
    let mut host = cambium_genet_winit_host::Harness::with_hooks(
        cambium_genet_winit_host::Init {
            state, logic: crate::view::root as crate::Logic, sheet, fonts: vec![], images: vec![],
        }, crate::hooks(&runtime),
    );
    host.layout_at(1180.0, 700.0);
    (host, dir)
}
fn click(host: &mut Host, action: &str) {
    assert!(host.click_on(&taproot::Selector::role("button").with_attr("data-action", action)), "{action}");
}

#[test]
fn production_native_close_hooks_require_project_cancel_or_discard() {
    let (mut host, _dir) = mounted();
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    host.relayout();
    assert!(!host.close_requested());
    assert!(host.state().preservation_pending());
    click(&mut host, "preservation-cancel");
    assert!(!host.close_requested());
    assert!(host.state().is_dirty());
    host.request_close(cambium_genet_winit_host::CloseRequest::Native);
    host.relayout();
    click(&mut host, "preservation-discard");
    assert!(host.close_requested());
}

#[test]
fn appearance_discard_and_save_cannot_approve_discarding_the_recorder_project() {
    for appearance_action in ["discard-close", "save-close"] {
        let (mut host, _dir) = mounted();
        click(&mut host, "toggle-appearance");
        click(&mut host, "edit-appearance");
        host.request_close(cambium_genet_winit_host::CloseRequest::Native);
        host.relayout();
        assert!(!host.close_requested());
        click(&mut host, appearance_action);
        assert!(!host.close_requested());
        assert!(!host.state().appearance.editor_open);
        assert!(host.state().is_dirty());
        assert!(host.state().preservation_pending());
        click(&mut host, "preservation-cancel");
        assert!(!host.close_requested());
    }
}

#[test]
fn update_restart_waits_for_project_preservation_and_appearance_cancellation() {
    let (mut host, _dir) = mounted();
    host.update(|state| {
        state.apply_update_status(crate::update::UpdateStatus::ReadyToRestart {
            version: "0.3.0".into(),
        });
        state.advance_update();
    });
    host.relayout();
    assert!(!host.close_requested());
    assert!(host.state().preservation_pending());
    click(&mut host, "toggle-appearance");
    click(&mut host, "edit-appearance");
    click(&mut host, "preservation-discard");
    assert!(host.state().appearance.close_app);
    assert!(!host.close_requested());
    click(&mut host, "cancel-close");
    assert!(!host.state().appearance.close_app);
    assert!(!host.state().project_close_ready());
    assert!(!host.close_requested());
    click(&mut host, "preservation-cancel");
    assert!(!host.state().preservation_pending());
    assert!(host.state().is_dirty());
}
