//! Native stream ownership never crosses the worker boundary. UI service only
//! polls cloned analyzer handles; cancellation never joins a blocked OS call.
use super::*;
use std::time::Instant;

const OPEN_TIMEOUT: Duration = Duration::from_secs(10);
type Opener = Box<dyn FnMut() -> Result<InputSession, String> + Send>;
type Authorizer = Box<dyn FnMut(&mut dyn FnMut()) -> Result<(), String> + Send>;

#[derive(Clone)]
struct Handles {
    tuner: TunerHandle,
    onset: OnsetHandle,
    capture: LooperCaptureHandle,
}
impl Handles {
    fn from_session(session: &InputSession) -> Self {
        Self { tuner: session.tuner.clone(), onset: session.onset.clone(), capture: session.capture.clone() }
    }
    fn into_session(self) -> InputSession {
        InputSession { _stream: InputStream::Worker, tuner: self.tuner, onset: self.onset, capture: self.capture }
    }
}

pub(super) struct Worker {
    requests: Option<mpsc::Sender<()>>,
    replies: Option<mpsc::Receiver<Event>>,
    opener: Option<Opener>,
    authorizer: Option<Authorizer>,
    in_flight: bool,
    deadline: Option<Instant>,
    expired: bool,
    awaiting_permission: bool,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
enum Reply { Ready(Handles), Failed(String), Retired }
enum Event {
    WaitingPermission(Instant),
    Opening(Instant),
    Complete(Instant, Result<Handles, String>),
}
impl Event {
    fn at(&self) -> Instant {
        match self { Self::WaitingPermission(at) | Self::Opening(at) | Self::Complete(at, _) => *at }
    }
}
impl Worker {
    pub(super) fn new() -> Self {
        Self { requests: None, replies: None, opener: Some(Box::new(InputSession::open)),
            authorizer: Some(Box::new(super::microphone_access::authorize)),
            in_flight: false, deadline: None, expired: false, awaiting_permission: false, wake: None }
    }
    pub(super) fn request(&mut self, now: Instant) -> Result<(), String> {
        if self.in_flight {
            return if self.expired { Err("Audio input is still opening after timeout; wait for the OS before retrying.".into()) } else { Ok(()) };
        }
        if self.requests.is_none() {
            let mut open = self.opener.take().ok_or("Audio input worker is unavailable.")?;
            let mut authorize = self.authorizer.take().ok_or("Microphone authorization worker is unavailable.")?;
            let (requests, commands) = mpsc::channel();
            let (results, replies) = mpsc::channel();
            let wake = self.wake.clone();
            // Neither InputSession nor InputEngine needs Send: both are created,
            // retained, and finally dropped inside this closure.
            std::thread::Builder::new().name("woodshed-input-admission".into()).spawn(move || {
                let mut session: Option<InputSession> = None;
                let emit = |event| {
                    let delivered = results.send(event).is_ok();
                    if delivered { if let Some(wake) = &wake { wake(); } }
                    delivered
                };
                while commands.recv().is_ok() {
                    let result = if let Some(session) = &session {
                        Ok(Handles::from_session(session))
                    } else {
                        let mut notified = false;
                        let authorized = authorize(&mut || {
                            if !notified {
                                notified = true;
                                emit(Event::WaitingPermission(Instant::now()));
                            }
                        });
                        match authorized {
                            Err(error) => Err(error),
                            Ok(()) => {
                                // Hardware time starts here, after explicit consent,
                                // rather than at UI dispatch or delayed UI polling.
                                if !emit(Event::Opening(Instant::now())) { break; }
                                open().map(|opened| {
                                    opened.tuner.set_enabled(false);
                                    opened.onset.set_enabled(false);
                                    opened.capture.set_enabled(false);
                                    let handles = Handles::from_session(&opened);
                                    session = Some(opened);
                                    handles
                                })
                            },
                        }
                    };
                    if !emit(Event::Complete(Instant::now(), result)) { break; }
                }
                // Native Drop, including a late successful open after backend
                // destruction, happens here. Never join this worker on UI Drop.
            }).map_err(|error| format!("Audio input worker: {error}"))?;
            self.requests = Some(requests);
            self.replies = Some(replies);
        }
        self.requests.as_ref().unwrap().send(()).map_err(|_| "Audio input worker disconnected.".to_string())?;
        self.in_flight = true;
        self.expired = false;
        self.awaiting_permission = false;
        self.deadline = Some(now + OPEN_TIMEOUT);
        Ok(())
    }
    fn poll(&mut self, now: Instant) -> Option<Reply> {
        if !self.in_flight { return None; }
        // Apply queued phase events before comparing UI time. Their actual
        // worker timestamps distinguish consent time from hardware-open time.
        loop {
            match self.replies.as_ref()?.try_recv() {
                Ok(event) => {
                    let timed_out = !self.expired && self.deadline.is_some_and(|deadline| event.at() >= deadline);
                    if timed_out { self.expire(); }
                    match event {
                        Event::WaitingPermission(_) if !self.expired => {
                            self.awaiting_permission = true;
                            self.deadline = None;
                        },
                        Event::Opening(start) if !self.expired => {
                            self.awaiting_permission = false;
                            self.deadline = Some(start + OPEN_TIMEOUT);
                        },
                        Event::Complete(_, result) => {
                            self.in_flight = false;
                            self.deadline = None;
                            self.awaiting_permission = false;
                            return Some(if timed_out { Self::timeout_reply() } else if self.expired { Reply::Retired } else {
                                match result { Ok(handles) => Reply::Ready(handles), Err(error) => Reply::Failed(error) }
                            });
                        },
                        _ => {},
                    }
                    if timed_out { return Some(Self::timeout_reply()); }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.in_flight = false;
                    self.deadline = None;
                    self.awaiting_permission = false;
                    return Some(Reply::Failed("Audio input worker disconnected.".into()));
                },
            }
        }
        if !self.expired && self.deadline.is_some_and(|deadline| now >= deadline) {
            self.expire();
            return Some(Self::timeout_reply());
        }
        None
    }
    fn expire(&mut self) {
        self.expired = true;
        self.awaiting_permission = false;
        self.deadline = None;
    }
    fn timeout_reply() -> Reply {
        Reply::Failed("Audio input opening timed out after 10 seconds outside explicit consent waiting. The OS worker may still be opening. Retry after it returns.".into())
    }
    pub(super) fn needs_service(&self) -> bool { self.in_flight && !self.expired }
    pub(super) fn awaiting_permission(&self) -> bool { self.awaiting_permission && self.needs_service() }
    pub(super) fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) { self.wake = Some(wake); }
}

pub(super) struct CaptureReservation {
    index: usize,
    selected: bool,
    epoch: u64,
    session: u64,
    backing: u64,
    output: u64,
    plan: Option<LoopPlan>,
}

impl CpalBackend {
    pub(super) fn input_error(&mut self, error: String) -> String {
        let message = match self.initial_error.as_ref() {
            Some(initial) => format!("{initial}; {error}"), None => error,
        };
        self.error = Some(message.clone());
        message
    }

    pub(crate) fn input_admission_pending(&self) -> bool {
        self.input_admission.needs_service()
    }

    pub(super) fn request_capture(&mut self, index: usize, selected: bool) -> Result<(), String> {
        self.finish_session_reset()?;
        if self.loop_capture_busy() { return Err("Capture is already armed or active.".into()); }
        self.check_set_capture_quota()?;
        let output = self.output.as_ref().ok_or("Audio output is unavailable.")?;
        if index >= output.song().bars.len() { return Err("Recording target is not in the accepted backing.".into()); }
        self.recorder_capture_epoch = self.recorder_capture_epoch.wrapping_add(1);
        self.recorder_capture_target = Some(index);
        self.pending_capture = Some(CaptureReservation {
            index, selected, epoch: self.recorder_capture_epoch, session: self.input_session_epoch,
            backing: self.backing_revision, output: output.observation_id(), plan: self.captures.plan.clone(),
        });
        if !self.ensure_input() {
            if self.input_admission.needs_service() { return Ok(()); }
            self.pending_capture = None;
            self.recorder_capture_target = None;
            return Err(self.error.clone().unwrap_or_else(|| "Audio input is unavailable.".into()));
        }
        self.arm_reserved_capture()
    }

    fn arm_reserved_capture(&mut self) -> Result<(), String> {
        let Some(reservation) = self.pending_capture.take() else { return Ok(()); };
        if reservation.session != self.input_session_epoch || reservation.backing != self.backing_revision
            || reservation.epoch != self.recorder_capture_epoch
            || self.recorder_capture_target != Some(reservation.index)
            || reservation.plan != self.captures.plan
            || self.output.as_ref().map(OutputRuntime::observation_id) != Some(reservation.output)
        {
            return Err("Pending Record backing, session or target changed; Record again on the current Card.".into());
        }
        if self.native_output && self.output_stream_count() == 0 {
            return Err("Audio output became unavailable while opening input. Pending Record was cancelled; the loaded backing remains unchanged.".into());
        }
        self.check_set_capture_quota()?;
        self.ensure_capture_input()?;
        let input = self.input.as_ref().ok_or("Audio input is unavailable.")?;
        input.capture.set_enabled(true);
        let result = self.with_output(|output| if reservation.selected {
            output.arm_selected_capture(reservation.index)
        } else {
            output.arm_capture(reservation.index, false)
        });
        if result.is_err() {
            if let Some(input) = &self.input { input.capture.set_enabled(false); }
        }
        result
    }

    pub(super) fn service_input_admission(&mut self) {
        match self.input_admission.poll(Instant::now()) {
            Some(Reply::Ready(handles)) => {
                let input = handles.into_session();
                input.tuner.set_enabled(self.tuner_requested);
                self.input = Some(input);
                self.error = self.initial_error.clone();
                if let Err(error) = self.arm_reserved_capture() { self.input_error(error); }
                if std::mem::take(&mut self.pending_calibration) { self.calibration_start(); }
            },
            Some(Reply::Failed(error)) => {
                self.pending_capture = None;
                self.pending_calibration = false;
                self.input_error(error);
            },
            Some(Reply::Retired) | None => {},
        }
    }
}

#[cfg(test)]
mod tests;
