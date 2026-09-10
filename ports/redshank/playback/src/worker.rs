use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use redshank_model::RepresentationReceipt;

use crate::backend::PumpState;
use crate::{
    Backend, PlaybackCommand, PlaybackSnapshot, PlaybackState, SnapshotCell, controller_adapter,
};

fn publish(
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    representation: Option<RepresentationReceipt>,
    state: PlaybackState,
    position_ms: u64,
    duration_ms: Option<u64>,
    source: Option<String>,
) {
    let wake = snapshot.lock().ok().and_then(|mut cell| {
        let next = PlaybackSnapshot {
            load_token: token,
            representation,
            state,
            position_ms,
            duration_ms,
            source,
        };
        if cell.value == next {
            return None;
        }
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
    });
    if let Some(wake) = wake {
        wake();
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

pub(super) fn run(receiver: mpsc::Receiver<PlaybackCommand>, snapshot: Arc<Mutex<SnapshotCell>>) {
    let backend = Rc::new(RefCell::new(Backend::default()));
    let mut controller = controller_adapter::controller(backend.clone());
    let mut token = None;
    loop {
        while let Ok(message) = receiver.try_recv() {
            match message {
                PlaybackCommand::Shutdown => return,
                PlaybackCommand::Load {
                    token: next,
                    source,
                    resume_ms,
                } => {
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
        thread::sleep(Duration::from_millis(5));
    }
}
