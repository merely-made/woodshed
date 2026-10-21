//! One admission gate shared by desktop buttons and opt-in shell commands.
use super::*;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) struct Dispatcher {
    pub tx: Sender<ClockCmd>,
    pub state: Arc<Mutex<OutputState>>,
    pub selection: Arc<AtomicU64>,
    pub busy: Arc<AtomicBool>,
    pub sequence: Arc<AtomicU64>,
    pub last: Arc<Mutex<Option<PedalReport>>>,
    pub bindings: Arc<Mutex<PedalBindings>>,
}
impl Dispatcher {
    /// One status projection for scripting and desktop diagnostic exports.
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "output": self.state.lock().unwrap().clone(),
            "bindings": self.bindings.lock().unwrap().clone(),
            "pedal": self.last.lock().unwrap().clone(),
            "diagnostics": diagnostics::snapshot(),
        })
    }

    pub fn submit(
        &self,
        action: PedalAction,
        binding: PedalBinding,
    ) -> (PedalReport, Option<Receiver<PedalReport>>) {
        self.submit_expected(action, binding, None)
    }
    pub fn submit_expected(
        &self,
        action: PedalAction,
        binding: PedalBinding,
        expected: Option<(u64, Option<u64>)>,
    ) -> (PedalReport, Option<Receiver<PedalReport>>) {
        let id = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        diagnostics::record(
            "admission",
            Some(id),
            expected.map(|t| t.0),
            expected.and_then(|t| t.1),
            &[],
            || format!("action={action:?}; binding={binding:?}"),
        );
        let refuse = |detail: String| {
            let report = PedalReport::refused(id, action, detail);
            diagnostics::record("refused", Some(id), None, None, &[], || report.summary());
            *self.last.lock().unwrap() = Some(report.clone());
            (report, None)
        };
        let messages = match binding.messages() {
            Ok(messages) => messages,
            Err(error) => return refuse(error),
        };
        let revision = self.selection.load(Ordering::Acquire);
        let (ready, generation) = {
            let state = self.state.lock().unwrap();
            (state.ready && state.revision == revision, state.generation)
        };
        if expected.is_some_and(|target| target != (revision, generation)) {
            return refuse(
                "Script target changed; request fresh status before another explicit trigger"
                    .into(),
            );
        }
        if !ready {
            return refuse("Selected MIDI receiver is not ready; nothing queued".into());
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return refuse("A pedal action is still pending; nothing queued".into());
        }
        for (index, bytes) in messages.iter().enumerate() {
            diagnostics::record(
                "planned-message",
                Some(id),
                Some(revision),
                generation,
                bytes,
                || format!("index={index}; action={action:?}"),
            );
        }
        let report = PedalReport {
            id,
            action,
            outcome: PedalOutcome::Queued,
            accepted_messages: 0,
            detail: "Admitted for the current receiver only".into(),
        };
        *self.last.lock().unwrap() = Some(report.clone());
        let (reply, rx) = sync_channel(1);
        let request = Request {
            report: report.clone(),
            messages,
            revision,
            generation,
            due: Instant::now(),
            expires: Instant::now() + Duration::from_millis(100),
            reply: Some(reply),
            busy: self.busy.clone(),
            last: self.last.clone(),
            output: self.state.clone(),
        };
        // Dropping an unreceived Request records a terminal refusal and frees
        // the gate; no action can be left waiting for another output lifetime.
        if let Err(error) = self.tx.send(ClockCmd::Pedal(request)) {
            drop(error);
        }
        (report, Some(rx))
    }
}

pub(super) struct Request {
    pub report: PedalReport,
    pub messages: Vec<Vec<u8>>,
    pub revision: u64,
    pub generation: Option<u64>,
    pub due: Instant,
    pub expires: Instant,
    reply: Option<SyncSender<PedalReport>>,
    busy: Arc<AtomicBool>,
    last: Arc<Mutex<Option<PedalReport>>>,
    output: Arc<Mutex<OutputState>>,
}
impl Request {
    pub fn finish(&mut self, outcome: PedalOutcome, detail: impl Into<String>) {
        self.report.outcome = outcome;
        self.report.detail = detail.into();
        diagnostics::record(
            "terminal",
            Some(self.report.id),
            Some(self.revision),
            self.generation,
            &[],
            || self.report.summary(),
        );
        if let Some(reply) = self.reply.take() {
            if outcome == PedalOutcome::Uncertain {
                let mut state = self.output.lock().unwrap();
                state.pedal_failure = Some(self.report.clone());
                state.failure(self.report.summary());
            }
            *self.last.lock().unwrap() = Some(self.report.clone());
            self.busy.store(false, Ordering::Release);
            let _ = reply.send(self.report.clone());
        }
    }
    pub fn cancel(&mut self, detail: &str) {
        let outcome = if self.report.accepted_messages == 0 {
            PedalOutcome::Refused
        } else {
            PedalOutcome::Uncertain
        };
        self.finish(outcome, detail);
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        if self.reply.is_some() {
            self.cancel("Output worker closed; action not replayed");
        }
    }
}

#[cfg(test)]
pub(super) fn fixture() -> (Dispatcher, Receiver<ClockCmd>) {
    let (tx, rx) = channel();
    (
        Dispatcher {
            tx,
            state: Arc::new(Mutex::new(OutputState {
                connected: Some("fixture".into()),
                revision: 1,
                ready: true,
                generation: Some(7),
                ..Default::default()
            })),
            selection: Arc::new(AtomicU64::new(1)),
            busy: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
            last: Arc::new(Mutex::new(None)),
            bindings: Arc::new(Mutex::new(PedalBindings::default())),
        },
        rx,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> PedalBinding {
        PedalBinding {
            enabled: true,
            release: Some(0),
            ..Default::default()
        }
    }
    #[test]
    fn pedal_admission_refuses_disabled_unready_stale_selection_and_generation() {
        let (d, rx) = fixture();
        assert_eq!(
            d.submit(PedalAction::EffectNext, PedalBinding::default())
                .0
                .outcome,
            PedalOutcome::Refused
        );
        for expected in [(0, Some(7)), (1, Some(8)), (1, None)] {
            assert_eq!(
                d.submit_expected(PedalAction::EffectNext, binding(), Some(expected))
                    .0
                    .outcome,
                PedalOutcome::Refused
            );
        }
        d.state.lock().unwrap().ready = false;
        assert_eq!(
            d.submit(PedalAction::EffectNext, binding()).0.outcome,
            PedalOutcome::Refused
        );
        d.state.lock().unwrap().ready = true;
        d.selection.store(2, Ordering::Release);
        assert_eq!(
            d.submit(PedalAction::EffectNext, binding()).0.outcome,
            PedalOutcome::Refused
        );
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn pedal_one_slot_and_dropped_request_give_terminal_result_and_allow_fresh_action() {
        let (d, rx) = fixture();
        let (queued, reply) = d.submit(PedalAction::EffectNext, binding());
        assert_eq!(queued.outcome, PedalOutcome::Queued);
        assert_eq!(
            d.submit(PedalAction::EffectPrevious, binding()).0.outcome,
            PedalOutcome::Refused
        );
        drop(rx.recv().unwrap());
        assert_eq!(
            reply.unwrap().recv().unwrap().outcome,
            PedalOutcome::Refused
        );
        assert!(!d.busy.load(Ordering::Acquire));
        assert_eq!(
            d.submit(PedalAction::EffectPrevious, binding()).0.outcome,
            PedalOutcome::Queued
        );
    }
    #[test]
    fn pedal_closed_worker_refuses_instead_of_leaving_queued() {
        let (d, rx) = fixture();
        drop(rx);
        let (_, reply) = d.submit(PedalAction::EffectNext, binding());
        assert_eq!(
            reply.unwrap().recv().unwrap().outcome,
            PedalOutcome::Refused
        );
        assert!(!d.busy.load(Ordering::Acquire));
    }
}
