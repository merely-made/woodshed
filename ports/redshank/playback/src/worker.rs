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
    pub(super) buffered_percent: u8,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            rate_percent: 100,
            volume_percent: 100,
            buffered_percent: 0,
        }
    }
}

fn publish(
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    representation: Option<RepresentationReceipt>,
    state: PlaybackState,
    position_ms: u64,
    duration_ms: Option<u64>,
    source: Option<String>,
) {
    let wake = {
        let mut cell = snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next = PlaybackSnapshot {
            load_token: token,
            representation,
            state,
            position_ms,
            duration_ms,
            source,
            rate_percent: cell.value.rate_percent,
            volume_percent: cell.value.volume_percent,
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
            cell.value.buffered_percent,
        ) == (
            levels.rate_percent,
            levels.volume_percent,
            levels.buffered_percent,
        ) {
            None
        } else {
            cell.value.rate_percent = levels.rate_percent;
            cell.value.volume_percent = levels.volume_percent;
            cell.value.buffered_percent = levels.buffered_percent;
            cell.last_wake = std::time::Instant::now();
            cell.wake.clone()
        }
    };
    if let Some(wake) = wake {
        wake();
    }
}

fn publish_buffered(snapshot: &Arc<Mutex<SnapshotCell>>, buffered_percent: u8) {
    let levels = {
        let cell = snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Levels {
            rate_percent: cell.value.rate_percent,
            volume_percent: cell.value.volume_percent,
            buffered_percent,
        }
    };
    publish_levels(snapshot, levels);
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
    let preview_backend = Rc::new(RefCell::new(Backend::with_audio(audio, fetch)));
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
    let state = product_state(controller);
    let view = controller
        .snapshot()
        .map_err(|error| format!("{error:?}"))?;
    let (representation, source, _) = backend.borrow().facts();
    publish(
        snapshot,
        token,
        representation,
        state,
        view.position.as_millis() as u64,
        view.duration.map(|duration| duration.as_millis() as u64),
        source,
    );
    publish_buffered(snapshot, backend.borrow().buffered_percent());
    Ok(())
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
        None,
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
) {
    let backend = Rc::new(RefCell::new(Backend::new(fetch)));
    let mut controller = controller_adapter::controller(backend.clone());
    let mut token = None;
    let mut preview = None;
    let mut levels = Levels::default();
    loop {
        while let Ok(message) = receiver.try_recv() {
            match message {
                PlaybackCommand::Shutdown => return,
                #[cfg(test)]
                PlaybackCommand::CrashWorker => panic!("injected playback worker failure"),
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
                        None,
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
                    if let Some(expected) = source.cached_representation()
                        && let Err(error) =
                            backend.borrow_mut().admit_cached_representation(expected)
                    {
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
                PlaybackCommand::Seek(position) => {
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
                    // The WSOLA stage retimes without moving pitch, so the
                    // requested rate applies live; the snapshot reports what
                    // the stage took, which is the request once clamped.
                    levels.rate_percent = backend.borrow_mut().set_rate(percent);
                    publish_levels(&snapshot, levels);
                },
                PlaybackCommand::SetVolume(percent) => {
                    levels.volume_percent = percent;
                    backend.borrow_mut().set_volume(percent);
                    publish_levels(&snapshot, levels);
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
) {
    loop {
        if catch_unwind(AssertUnwindSafe(|| {
            run_session(&receiver, Arc::clone(&snapshot), fetch.clone());
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
            None,
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
