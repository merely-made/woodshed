use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use fetch::Fetch;
use redshank_model::{MediaSource, RepresentationReceipt};

use crate::backend::PumpState;
use crate::{
    Backend, PlaybackCommand, PlaybackSnapshot, PlaybackState, PreviewSnapshot, PreviewState,
    SnapshotCell, controller_adapter,
};

/// The output levels the worker reports, alongside the transport facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Levels {
    /// Effective, not requested: what the retiming stage actually applied.
    pub(super) rate_percent: u16,
    pub(super) volume_percent: u8,
    pub(super) duck_percent: Option<u8>,
    pub(super) buffered_percent: u8,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            rate_percent: 100,
            volume_percent: 100,
            duck_percent: None,
            buffered_percent: 0,
        }
    }
}

/// Accepted configuration outlives the recoverable decoder/output session.
/// An unloaded rate preference waits for controller admission after Load;
/// snapshots keep their last admitted rate until that succeeds.
#[derive(Clone, Copy)]
struct Controls {
    rate_percent: u16,
    volume_percent: u8,
    duck_percent: Option<u8>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            rate_percent: 100,
            volume_percent: 100,
            duck_percent: None,
        }
    }
}

impl Controls {
    fn restore_gain(&self, backend: &mut Backend) {
        backend.set_volume(self.volume_percent);
        backend.set_duck(self.duck_percent);
    }
}

fn publish(
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    evidence: (
        Option<RepresentationReceipt>,
        Option<Arc<crate::FingerprintContext>>,
    ),
    state: PlaybackState,
    position_ms: u64,
    duration_ms: Option<u64>,
    source: Option<String>,
) {
    let (representation, fingerprint_context) = evidence;
    let wake = {
        let mut cell = snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next = PlaybackSnapshot {
            load_token: token,
            completed_note_seek: cell.value.completed_note_seek,
            representation,
            fingerprint_context,
            state,
            position_ms,
            duration_ms,
            source,
            rate_percent: cell.value.rate_percent,
            volume_percent: cell.value.volume_percent,
            duck_percent: cell.value.duck_percent,
            buffered_percent: cell.value.buffered_percent,
            preview: cell.value.preview.clone(),
        };
        if cell.value == next {
            None
        } else {
            let position_only = cell.value.load_token == next.load_token
                && cell.value.representation == next.representation
                && cell.value.state == next.state
                && cell.value.duration_ms == next.duration_ms
                && cell.value.source == next.source;
            cell.value = next;
            if !position_only || cell.last_wake.elapsed() >= Duration::from_millis(100) {
                cell.last_wake = std::time::Instant::now();
                cell.wake.clone()
            } else {
                None
            }
        }
    };
    if let Some(wake) = wake {
        wake();
    }
}

/// Levels move independently of transport facts, so they publish on their own.
fn publish_levels(snapshot: &Arc<Mutex<SnapshotCell>>, levels: Levels) {
    let wake = {
        let mut cell = snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if (
            cell.value.rate_percent,
            cell.value.volume_percent,
            cell.value.duck_percent,
            cell.value.buffered_percent,
        ) == (
            levels.rate_percent,
            levels.volume_percent,
            levels.duck_percent,
            levels.buffered_percent,
        ) {
            None
        } else {
            cell.value.rate_percent = levels.rate_percent;
            cell.value.volume_percent = levels.volume_percent;
            cell.value.duck_percent = levels.duck_percent;
            cell.value.buffered_percent = levels.buffered_percent;
            cell.last_wake = std::time::Instant::now();
            cell.wake.clone()
        }
    };
    if let Some(wake) = wake {
        wake();
    }
}

fn publish_backend_levels(
    snapshot: &Arc<Mutex<SnapshotCell>>,
    backend: &Backend,
    rate_percent: u16,
) {
    let (volume_percent, duck_percent) = backend.output_levels();
    publish_levels(
        snapshot,
        Levels {
            rate_percent,
            volume_percent,
            duck_percent,
            buffered_percent: backend.buffered_percent(),
        },
    );
}

fn publish_preview(snapshot: &Arc<Mutex<SnapshotCell>>, preview: Option<PreviewSnapshot>) {
    let wake = {
        let mut cell = snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut next = cell.value.clone();
        next.preview = preview;
        if cell.value == next {
            None
        } else {
            let position_only = cell.value.load_token == next.load_token
                && cell.value.representation == next.representation
                && cell.value.state == next.state
                && cell.value.position_ms == next.position_ms
                && cell.value.duration_ms == next.duration_ms
                && cell.value.source == next.source
                && match (&cell.value.preview, &next.preview) {
                    (Some(previous), Some(current)) => {
                        previous.id == current.id
                            && previous.state == current.state
                            && previous.duration_ms == current.duration_ms
                    },
                    _ => false,
                };
            cell.value = next;
            if !position_only || cell.last_wake.elapsed() >= Duration::from_millis(100) {
                cell.last_wake = std::time::Instant::now();
                cell.wake.clone()
            } else {
                None
            }
        }
    };
    if let Some(wake) = wake {
        wake();
    }
}

struct PreviewSession {
    id: String,
    controller: controller_adapter::Controller,
    backend: controller_adapter::BackendRef,
    resume_main: bool,
}

fn preview_snapshot(
    preview: &mut PreviewSession,
    state: PreviewState,
) -> Result<PreviewSnapshot, String> {
    let view = preview
        .controller
        .snapshot()
        .map_err(|error| format!("{error:?}"))?;
    Ok(PreviewSnapshot {
        id: preview.id.clone(),
        state,
        position_ms: view.position.as_millis() as u64,
        duration_ms: view.duration.map(|duration| duration.as_millis() as u64),
    })
}

fn stop_preview(
    preview: &mut Option<PreviewSession>,
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    resume_main: bool,
) {
    let Some(session) = preview.take() else {
        publish_preview(snapshot, None);
        return;
    };
    let id = session.id;
    if let Err(error) = session.backend.borrow_mut().clear() {
        publish_preview(
            snapshot,
            Some(PreviewSnapshot {
                id,
                state: PreviewState::Unavailable(format!(
                    "Could not stop voice-note playback: {error}"
                )),
                position_ms: 0,
                duration_ms: None,
            }),
        );
    } else {
        publish_preview(snapshot, None);
    }
    if resume_main && session.resume_main {
        let _ = command(
            controller,
            backend,
            snapshot,
            token,
            servo_media_player::controller::PlaybackCommand::Play,
            "resume after preview",
        );
    }
}

fn fail_preview(
    preview: &mut Option<PreviewSession>,
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    error: impl std::fmt::Display,
) {
    let Some(session) = preview.take() else {
        return;
    };
    let mut message = error.to_string();
    if let Err(cleanup) = session.backend.borrow_mut().clear() {
        message.push_str(&format!("; preview cleanup failed: {cleanup}"));
    }
    publish_preview(
        snapshot,
        Some(PreviewSnapshot {
            id: session.id,
            state: PreviewState::Unavailable(message),
            position_ms: 0,
            duration_ms: None,
        }),
    );
    if session.resume_main {
        let _ = command(
            controller,
            backend,
            snapshot,
            token,
            servo_media_player::controller::PlaybackCommand::Play,
            "resume after preview failure",
        );
    }
}

fn start_preview(
    preview: &mut Option<PreviewSession>,
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    id: String,
    source: MediaSource,
) {
    let resume_main = preview.as_ref().map_or_else(
        || controller.state() == servo_media_player::PlaybackState::Playing,
        |active| active.resume_main,
    );
    stop_preview(preview, controller, backend, snapshot, token, false);
    if controller.state() == servo_media_player::PlaybackState::Playing
        && !command(
            controller,
            backend,
            snapshot,
            token,
            servo_media_player::controller::PlaybackCommand::Pause,
            "pause for preview",
        )
    {
        return;
    }

    let audio = backend.borrow().audio_handle();
    let fetch = backend.borrow().fetch_handle();
    let blobs = backend.borrow().blob_reader();
    let preview_backend = Rc::new(RefCell::new(
        Backend::with_audio(audio, fetch).with_blobs(blobs),
    ));
    let preview_controller = controller_adapter::controller(preview_backend.clone());
    *preview = Some(PreviewSession {
        id: id.clone(),
        controller: preview_controller,
        backend: preview_backend,
        resume_main,
    });
    publish_preview(
        snapshot,
        Some(PreviewSnapshot {
            id,
            state: PreviewState::Loading,
            position_ms: 0,
            duration_ms: None,
        }),
    );

    let load = preview
        .as_mut()
        .expect("preview session was initialized")
        .controller
        .command(servo_media_player::controller::PlaybackCommand::Load(
            controller_adapter::map_source(&source),
        ));
    if let Err(error) = load {
        fail_preview(
            preview,
            controller,
            backend,
            snapshot,
            token,
            format!("Could not load voice note: {error:?}"),
        );
        return;
    }
    let cached_admission = source.cached_representation().map(|expected| {
        let session = preview.as_mut().expect("preview session was initialized");
        session
            .backend
            .borrow_mut()
            .admit_cached_representation(expected)
    });
    if let Some(Err(error)) = cached_admission {
        fail_preview(preview, controller, backend, snapshot, token, error);
        return;
    }
    let loading = preview_snapshot(
        preview.as_mut().expect("preview session was initialized"),
        PreviewState::Loading,
    );
    match loading {
        Ok(loading) => publish_preview(snapshot, Some(loading)),
        Err(error) => fail_preview(preview, controller, backend, snapshot, token, error),
    }
}

fn complete_preview(
    preview: &mut Option<PreviewSession>,
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
) {
    let Some(mut session) = preview.take() else {
        return;
    };
    let result = session
        .controller
        .signal(servo_media_player::controller::PlaybackSignal::EndOfStream)
        .map_err(|error| format!("Could not finish voice-note playback: {error:?}"))
        .and_then(|()| preview_snapshot(&mut session, PreviewState::Ended));
    let cleanup = session.backend.borrow_mut().clear();
    match (result, cleanup) {
        (Ok(ended), Ok(())) => publish_preview(snapshot, Some(ended)),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => publish_preview(
            snapshot,
            Some(PreviewSnapshot {
                id: session.id.clone(),
                state: PreviewState::Unavailable(error),
                position_ms: 0,
                duration_ms: None,
            }),
        ),
        (Err(error), Err(cleanup)) => publish_preview(
            snapshot,
            Some(PreviewSnapshot {
                id: session.id.clone(),
                state: PreviewState::Unavailable(format!("{error}; additionally {cleanup}")),
                position_ms: 0,
                duration_ms: None,
            }),
        ),
    }
    if session.resume_main {
        let _ = command(
            controller,
            backend,
            snapshot,
            token,
            servo_media_player::controller::PlaybackCommand::Play,
            "resume after preview",
        );
    }
}

fn product_state(controller: &controller_adapter::Controller) -> PlaybackState {
    match controller.terminal() {
        Some(servo_media_player::controller::PlaybackTerminal::EndOfStream) => PlaybackState::Ended,
        Some(servo_media_player::controller::PlaybackTerminal::Error(error)) => {
            PlaybackState::Unavailable(error.clone())
        },
        None => match controller.state() {
            servo_media_player::PlaybackState::Stopped
            | servo_media_player::PlaybackState::Paused => PlaybackState::Paused,
            servo_media_player::PlaybackState::Buffering => PlaybackState::Loading,
            servo_media_player::PlaybackState::Playing => PlaybackState::Playing,
        },
    }
}

fn project(
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
) -> Result<(), String> {
    let (representation, source, _) = backend.borrow().facts();
    if source.is_none() {
        // No source can admit a rate yet. Gain remains a real output setting,
        // including for a voice preview; retain the last admitted main rate.
        let rate = snapshot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .value
            .rate_percent;
        publish_backend_levels(snapshot, &backend.borrow(), rate);
        return Ok(());
    }
    let state = product_state(controller);
    let view = controller
        .snapshot()
        .map_err(|error| format!("{error:?}"))?;
    publish(
        snapshot,
        token,
        (representation, backend.borrow().fingerprint_context()),
        state,
        view.position.as_millis() as u64,
        view.duration.map(|duration| duration.as_millis() as u64),
        source,
    );
    publish_backend_levels(
        snapshot,
        &backend.borrow(),
        (view.rate * 100.0).round() as u16,
    );
    Ok(())
}

fn admit_rate(
    controller: &mut controller_adapter::Controller,
    percent: u16,
) -> Result<u16, String> {
    let bounded = percent.clamp(
        crate::backend::MIN_RATE_PERCENT,
        crate::backend::MAX_RATE_PERCENT,
    );
    controller
        .command(servo_media_player::controller::PlaybackCommand::SetRate(
            f64::from(bounded) / 100.0,
        ))
        .map_err(|error| format!("controller rate failed: {error:?}"))?;
    controller
        .snapshot()
        .map(|view| (view.rate * 100.0).round() as u16)
        .map_err(|error| format!("controller rate snapshot failed: {error:?}"))
}

fn fail(
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    error: impl std::fmt::Display,
) {
    let mut message = error.to_string();
    if let Err(signal) = controller.signal(servo_media_player::controller::PlaybackSignal::Error(
        message.clone(),
    )) {
        message.push_str(&format!("; controller error signal failed: {signal:?}"));
    }
    if let Err(cleanup) = backend.borrow_mut().clear() {
        message.push_str(&format!("; backend cleanup failed: {cleanup}"));
    }
    publish(
        snapshot,
        token,
        (None, None),
        PlaybackState::Unavailable(message),
        0,
        None,
        None,
    );
}

fn command(
    controller: &mut controller_adapter::Controller,
    backend: &controller_adapter::BackendRef,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    command: servo_media_player::controller::PlaybackCommand,
    label: &str,
) -> bool {
    match controller.command(command) {
        Ok(()) => true,
        Err(error) => {
            fail(
                controller,
                backend,
                snapshot,
                token,
                format!("controller {label} failed: {error:?}"),
            );
            false
        },
    }
}

fn run_session(
    receiver: &mpsc::Receiver<PlaybackCommand>,
    snapshot: Arc<Mutex<SnapshotCell>>,
    fetch: Arc<dyn Fetch>,
    blobs: Option<crate::HostBlobReader>,
    controls: &mut Controls,
) {
    let backend = Rc::new(RefCell::new(Backend::new(fetch).with_blobs(blobs)));
    controls.restore_gain(&mut backend.borrow_mut());
    let mut controller = controller_adapter::controller(backend.clone());
    let mut token = None;
    let mut preview = None;
    let admitted_rate = snapshot
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .value
        .rate_percent;
    publish_backend_levels(&snapshot, &backend.borrow(), admitted_rate);
    loop {
        while let Ok(message) = receiver.try_recv() {
            let note_seek_request = match &message {
                PlaybackCommand::SeekNote {
                    token: expected,
                    request_id,
                    ..
                } if token == Some(*expected) => Some(*request_id),
                // A stale note must not affect the newly selected transport.
                PlaybackCommand::SeekNote { .. } => continue,
                _ => None,
            };
            match message {
                PlaybackCommand::Shutdown => return,
                #[cfg(test)]
                PlaybackCommand::CrashWorker => panic!("injected playback worker failure"),
                #[cfg(test)]
                PlaybackCommand::InspectControls(sender) => {
                    let rate = backend.borrow().rate_percent();
                    let gain = backend.borrow().effective_volume();
                    let admitted_rate = controller
                        .snapshot()
                        .ok()
                        .map(|view| (view.rate * 100.0).round() as u16);
                    let _ = sender.send((rate, admitted_rate, gain));
                    continue;
                },
                PlaybackCommand::Load {
                    token: next,
                    source,
                    resume_ms,
                } => {
                    stop_preview(
                        &mut preview,
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        false,
                    );
                    token = Some(next);
                    publish(
                        &snapshot,
                        token,
                        (None, None),
                        PlaybackState::Loading,
                        0,
                        None,
                        None,
                    );
                    if let Err(error) = controller_adapter::load(&mut controller, &source) {
                        fail(
                            &mut controller,
                            &backend,
                            &snapshot,
                            token,
                            format!("controller load failed: {error:?}"),
                        );
                        continue;
                    }
                    let cached_admission = source
                        .cached_representation()
                        .map(|expected| backend.borrow_mut().admit_cached_representation(expected));
                    if let Some(Err(error)) = cached_admission {
                        fail(&mut controller, &backend, &snapshot, token, error);
                        continue;
                    }
                    if let Err(error) = admit_rate(&mut controller, controls.rate_percent) {
                        fail(&mut controller, &backend, &snapshot, token, error);
                        continue;
                    }
                    if resume_ms > 0
                        && !command(
                            &mut controller,
                            &backend,
                            &snapshot,
                            token,
                            servo_media_player::controller::PlaybackCommand::Seek(
                                Duration::from_millis(resume_ms),
                            ),
                            "resume seek",
                        )
                    {
                        continue;
                    }
                    // Pump owns the first decode and creates the sink before Ready.
                },
                PlaybackCommand::Play => {
                    stop_preview(
                        &mut preview,
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        false,
                    );
                    if !command(
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        servo_media_player::controller::PlaybackCommand::Play,
                        "play",
                    ) {
                        continue;
                    }
                },
                PlaybackCommand::Pause => {
                    stop_preview(
                        &mut preview,
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        false,
                    );
                    if !command(
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        servo_media_player::controller::PlaybackCommand::Pause,
                        "pause",
                    ) {
                        continue;
                    }
                },
                PlaybackCommand::Stop => {
                    stop_preview(
                        &mut preview,
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        false,
                    );
                    if !command(
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        servo_media_player::controller::PlaybackCommand::Stop,
                        "stop",
                    ) {
                        continue;
                    }
                },
                PlaybackCommand::Seek(position)
                | PlaybackCommand::SeekNote {
                    position_ms: position,
                    ..
                } => {
                    stop_preview(
                        &mut preview,
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        false,
                    );
                    if !command(
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        servo_media_player::controller::PlaybackCommand::Seek(
                            Duration::from_millis(position),
                        ),
                        "seek",
                    ) {
                        continue;
                    }
                },
                PlaybackCommand::SetRate(percent) => {
                    let bounded = percent.clamp(
                        crate::backend::MIN_RATE_PERCENT,
                        crate::backend::MAX_RATE_PERCENT,
                    );
                    if backend.borrow().facts().1.is_some() {
                        match admit_rate(&mut controller, bounded) {
                            Ok(accepted) => controls.rate_percent = accepted,
                            Err(error) => {
                                fail(&mut controller, &backend, &snapshot, token, error);
                                continue;
                            },
                        }
                    } else {
                        controls.rate_percent = bounded;
                    }
                },
                PlaybackCommand::SetVolume(percent) => {
                    backend.borrow_mut().set_volume(percent);
                    controls.volume_percent = percent;
                },
                PlaybackCommand::Duck(percent) => {
                    backend.borrow_mut().set_duck(Some(percent));
                    controls.duck_percent = Some(percent);
                },
                PlaybackCommand::Unduck => {
                    backend.borrow_mut().set_duck(None);
                    controls.duck_percent = None;
                },
                PlaybackCommand::StartPreview { id, source } => {
                    start_preview(
                        &mut preview,
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        id,
                        source,
                    );
                },
                PlaybackCommand::StopPreview => stop_preview(
                    &mut preview,
                    &mut controller,
                    &backend,
                    &snapshot,
                    token,
                    true,
                ),
            }
            if let Err(error) = project(&mut controller, &backend, &snapshot, token) {
                fail(&mut controller, &backend, &snapshot, token, error);
            } else if let Some(request_id) = note_seek_request {
                // Position is already published; acknowledge only successful
                // seeks, so hosts never mistake a pre-seek clock for its result.
                let wake = {
                    let mut cell = snapshot.lock().unwrap_or_else(|p| p.into_inner());
                    cell.value.completed_note_seek = Some(request_id);
                    cell.wake.clone()
                };
                if let Some(wake) = wake {
                    wake();
                }
            }
        }
        // Drop the mutable adapter borrow before controller signals call back
        // through the source/sink traits.
        let pump = backend.borrow_mut().pump();
        match pump {
            Ok(PumpState::Ready) => {
                if let Err(error) =
                    controller.signal(servo_media_player::controller::PlaybackSignal::Ready)
                {
                    fail(
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        format!("controller ready failed: {error:?}"),
                    );
                } else if let Err(error) = project(&mut controller, &backend, &snapshot, token) {
                    fail(&mut controller, &backend, &snapshot, token, error);
                }
            },
            Ok(PumpState::EndOfStream) => {
                if let Err(error) =
                    controller.signal(servo_media_player::controller::PlaybackSignal::EndOfStream)
                {
                    fail(
                        &mut controller,
                        &backend,
                        &snapshot,
                        token,
                        format!("controller end-of-stream signal failed: {error:?}"),
                    );
                } else if let Err(error) = project(&mut controller, &backend, &snapshot, token) {
                    fail(&mut controller, &backend, &snapshot, token, error);
                }
            },
            Ok(PumpState::Idle)
                if controller.state() == servo_media_player::PlaybackState::Playing =>
            {
                if let Err(error) = project(&mut controller, &backend, &snapshot, token) {
                    fail(&mut controller, &backend, &snapshot, token, error);
                }
            },
            Ok(PumpState::Idle) => {},
            Err(error) => fail(
                &mut controller,
                &backend,
                &snapshot,
                token,
                format!("backend playback failed: {error}"),
            ),
        }
        let preview_pump = preview
            .as_ref()
            .map(|session| session.backend.borrow_mut().pump());
        if let Some(preview_pump) = preview_pump {
            match preview_pump {
                Ok(PumpState::Ready) => {
                    let ready = preview
                        .as_mut()
                        .expect("preview pump requires a session")
                        .controller
                        .signal(servo_media_player::controller::PlaybackSignal::Ready)
                        .and_then(|()| {
                            preview
                                .as_mut()
                                .expect("preview pump requires a session")
                                .controller
                                .command(servo_media_player::controller::PlaybackCommand::Play)
                        });
                    if let Err(error) = ready {
                        fail_preview(
                            &mut preview,
                            &mut controller,
                            &backend,
                            &snapshot,
                            token,
                            format!("Could not start voice-note playback: {error:?}"),
                        );
                    } else {
                        let playing = preview_snapshot(
                            preview.as_mut().expect("preview pump requires a session"),
                            PreviewState::Playing,
                        );
                        match playing {
                            Ok(playing) => publish_preview(&snapshot, Some(playing)),
                            Err(error) => fail_preview(
                                &mut preview,
                                &mut controller,
                                &backend,
                                &snapshot,
                                token,
                                error,
                            ),
                        }
                    }
                },
                Ok(PumpState::EndOfStream) => {
                    complete_preview(&mut preview, &mut controller, &backend, &snapshot, token)
                },
                Ok(PumpState::Idle)
                    if preview.as_ref().is_some_and(|session| {
                        session.controller.state() == servo_media_player::PlaybackState::Playing
                    }) =>
                {
                    let playing = preview_snapshot(
                        preview.as_mut().expect("preview pump requires a session"),
                        PreviewState::Playing,
                    );
                    match playing {
                        Ok(playing) => publish_preview(&snapshot, Some(playing)),
                        Err(error) => fail_preview(
                            &mut preview,
                            &mut controller,
                            &backend,
                            &snapshot,
                            token,
                            error,
                        ),
                    }
                },
                Ok(PumpState::Idle) => {},
                Err(error) => fail_preview(
                    &mut preview,
                    &mut controller,
                    &backend,
                    &snapshot,
                    token,
                    format!("Voice-note playback failed: {error}"),
                ),
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn run(
    receiver: mpsc::Receiver<PlaybackCommand>,
    snapshot: Arc<Mutex<SnapshotCell>>,
    fetch: Arc<dyn Fetch>,
    blobs: Option<crate::HostBlobReader>,
) {
    let mut controls = Controls::default();
    loop {
        if catch_unwind(AssertUnwindSafe(|| {
            run_session(
                &receiver,
                Arc::clone(&snapshot),
                fetch.clone(),
                blobs.clone(),
                &mut controls,
            );
        }))
        .is_ok()
        {
            return;
        }
        let token = snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .value
            .load_token;
        publish_preview(&snapshot, None);
        publish(
            &snapshot,
            token,
            (None, None),
            PlaybackState::Unavailable(
                "Playback recovered from an internal audio failure; select a recording to retry"
                    .into(),
            ),
            0,
            None,
            None,
        );
    }
}
