//! Live input admission on the existing midir callback; no renderer or writer.
use super::{Dispatcher, MidiEvent, PedalBindings, PedalTarget};
use woodshed_core::midi::{ControllerBinding, ControllerEdges, ControllerEvent, ControllerKind};

pub(super) fn event(event: MidiEvent) -> Option<ControllerEvent> {
    let (kind, channel, number, value) = match event {
        MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        } => (ControllerKind::Note, channel + 1, note, velocity),
        MidiEvent::NoteOff { channel, note, .. } => (ControllerKind::Note, channel + 1, note, 0),
        MidiEvent::ControlChange {
            channel,
            controller,
            value,
        } => (
            ControllerKind::ControlChange,
            channel + 1,
            controller,
            value,
        ),
        _ => return None,
    };
    Some(ControllerEvent {
        binding: ControllerBinding {
            channel,
            kind,
            number,
        },
        value,
    })
}

#[derive(Default)]
pub(super) struct Controller {
    epoch: u64,
    active: bool,
    bindings: PedalBindings,
    armed: bool,
    target: Option<PedalTarget>,
    edges: ControllerEdges,
    invalidated: bool,
    /// A stale UI frame cannot rearm a callback which observed a lifetime loss.
    needs_disarm: bool,
    /// A worker completion must not hide a callback refusal before the UI polls.
    notice: Option<String>,
}
impl Controller {
    pub fn invalidate(&mut self) {
        self.armed = false;
        self.needs_disarm = true;
        self.invalidated = true;
        self.edges.reset();
    }
    pub fn replace_source(&mut self, active: bool) -> u64 {
        self.invalidate();
        self.epoch = self.epoch.saturating_add(1);
        self.active = active;
        self.epoch
    }
    pub fn take_invalidation(&mut self) -> bool {
        std::mem::take(&mut self.invalidated)
    }
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }
    pub fn configure(
        &mut self,
        bindings: &PedalBindings,
        armed: bool,
        target: Option<PedalTarget>,
    ) {
        if self.bindings != *bindings || self.target != target {
            self.edges.reset();
            if self.armed {
                self.invalidate();
            }
            self.bindings = bindings.clone();
            self.target = target;
        }
        // The view must acknowledge disarmed before a fresh explicit Arm.
        if !armed {
            self.needs_disarm = false;
        }
        self.armed = armed && self.active && target.is_some() && !self.needs_disarm;
    }
    pub fn receive(&mut self, epoch: u64, midi: MidiEvent, dispatcher: &Dispatcher) {
        super::diagnostics::record(
            "controller-input",
            None,
            self.target.map(|t| t.revision),
            self.target.and_then(|t| t.generation),
            &[],
            || {
                format!(
                    "{midi:?}; source_epoch={epoch}; active_epoch={}; armed={}",
                    self.epoch, self.armed
                )
            },
        );
        if !self.active || epoch != self.epoch {
            return;
        }
        let Some(event) = event(midi) else { return };
        if self.armed {
            let current = {
                let state = dispatcher.state.lock().unwrap();
                (state.ready
                    && state.revision
                        == dispatcher
                            .selection
                            .load(std::sync::atomic::Ordering::Acquire))
                .then_some(PedalTarget {
                    revision: state.revision,
                    generation: state.generation,
                })
            };
            if current != self.target {
                self.invalidate();
                return;
            }
        }
        match self.edges.observe(&self.bindings.inputs, event) {
            Ok(Some(action)) if self.armed => {
                let target = self.target.unwrap();
                // Same slot, deadlines, expected receiver and partial-outcome
                // owner as buttons, keyboard and stdio. No callback I/O writer.
                let (report, _) = dispatcher.submit_expected(
                    action,
                    self.bindings.get(action).clone(),
                    Some((target.revision, target.generation)),
                );
                super::diagnostics::record(
                    "controller-decision",
                    Some(report.id),
                    Some(target.revision),
                    target.generation,
                    &[],
                    || format!("input={event:?}; {}", report.summary()),
                );
                if report.outcome == super::PedalOutcome::Refused {
                    self.notice = Some(format!("Controller input: {}", report.summary()));
                }
            },
            Err(_) => self.invalidate(),
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::{ClockCmd, OutputState, PedalOutcome};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64},
        mpsc::{Receiver, channel},
    };
    fn fixture() -> (
        Controller,
        Dispatcher,
        Receiver<ClockCmd>,
        PedalBindings,
        u64,
        PedalTarget,
    ) {
        let (tx, rx) = channel();
        let state = OutputState {
            ready: true,
            revision: 1,
            generation: Some(7),
            ..Default::default()
        };
        let dispatcher = Dispatcher {
            tx,
            state: Arc::new(Mutex::new(state)),
            selection: Arc::new(AtomicU64::new(1)),
            busy: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
            last: Default::default(),
            bindings: Default::default(),
        };
        let mut bindings = PedalBindings::default();
        bindings.effect_next.enabled = true;
        bindings.effect_next.number = 67;
        bindings.inputs.effect_next.controller = Some(ControllerBinding {
            channel: 2,
            kind: ControllerKind::ControlChange,
            number: 20,
        });
        let mut controller = Controller::default();
        let epoch = controller.replace_source(true);
        let target = PedalTarget {
            revision: 1,
            generation: Some(7),
        };
        controller.configure(&bindings, false, Some(target));
        controller.configure(&bindings, true, Some(target));
        (controller, dispatcher, rx, bindings, epoch, target)
    }
    fn cc(value: u8) -> MidiEvent {
        MidiEvent::ControlChange {
            channel: 1,
            controller: 20,
            value,
        }
    }
    fn complete(rx: &Receiver<ClockCmd>) {
        let ClockCmd::Pedal(mut request) = rx.try_recv().unwrap() else {
            panic!("not pedal")
        };
        assert_eq!(request.messages[0], [176, 67, 127]);
        request.finish(PedalOutcome::Accepted, "admission fixture; no native send");
    }
    #[test]
    fn callback_admits_without_frame_poll_and_holds_cannot_retrigger() {
        let (mut c, d, rx, _, epoch, _) = fixture();
        c.receive(epoch, cc(127), &d); // unknown initial hold
        assert!(rx.try_recv().is_err());
        c.receive(epoch, cc(0), &d);
        c.receive(epoch, cc(127), &d);
        complete(&rx);
        for _ in 0..64 {
            c.receive(epoch, cc(100), &d);
        }
        assert!(rx.try_recv().is_err());
        c.receive(epoch, cc(0), &d);
        c.receive(epoch, cc(127), &d);
        complete(&rx);
    }
    #[test]
    fn unarmed_history_is_observed_without_replaying_on_arm() {
        let (mut c, d, rx, bindings, epoch, target) = fixture();
        c.configure(&bindings, false, Some(target));
        c.receive(epoch, cc(0), &d);
        c.receive(epoch, cc(127), &d);
        c.configure(&bindings, true, Some(target));
        c.receive(epoch, cc(127), &d);
        assert!(rx.try_recv().is_err());
        c.receive(epoch, cc(0), &d);
        c.receive(epoch, cc(127), &d);
        complete(&rx);
    }
    #[test]
    fn source_and_receiver_changes_need_disarm_ack_and_fresh_release() {
        let (mut c, d, rx, bindings, epoch, target) = fixture();
        c.receive(epoch, cc(0), &d);
        let next = c.replace_source(true);
        c.take_invalidation();
        c.configure(&bindings, true, Some(target)); // stale armed view
        c.receive(epoch, cc(127), &d); // retired callback
        c.receive(next, cc(127), &d);
        assert!(rx.try_recv().is_err());
        c.configure(&bindings, false, Some(target));
        c.configure(&bindings, true, Some(target));
        c.receive(next, cc(0), &d);
        d.state.lock().unwrap().generation = Some(8);
        c.receive(next, cc(127), &d);
        assert!(c.take_invalidation());
        assert!(rx.try_recv().is_err());
        let fresh = PedalTarget {
            generation: Some(8),
            ..target
        };
        c.configure(&bindings, false, Some(fresh));
        c.configure(&bindings, true, Some(fresh));
        c.receive(next, cc(127), &d);
        assert!(rx.try_recv().is_err());
        c.receive(next, cc(0), &d);
        c.receive(next, cc(127), &d);
        complete(&rx);
    }
    #[test]
    fn callback_busy_refuses_instead_of_retaining_a_later_action() {
        let (mut c, d, rx, _, epoch, _) = fixture();
        d.busy.store(true, std::sync::atomic::Ordering::Release);
        c.receive(epoch, cc(0), &d);
        c.receive(epoch, cc(127), &d);
        assert!(rx.try_recv().is_err());
        assert_eq!(
            d.last.lock().unwrap().as_ref().unwrap().outcome,
            PedalOutcome::Refused
        );
        // Independent status/completion updates may advance the shared report.
        *d.last.lock().unwrap() = None;
        assert!(c.take_notice().unwrap().contains("still pending"));
        assert!(c.take_notice().is_none());
        d.busy.store(false, std::sync::atomic::Ordering::Release);
        c.receive(epoch, cc(127), &d);
        assert!(rx.try_recv().is_err());
        c.receive(epoch, cc(0), &d);
        c.receive(epoch, cc(127), &d);
        complete(&rx);
    }

    #[test]
    fn editing_a_mapping_cannot_keep_the_old_armed_authority() {
        let (mut c, d, rx, mut bindings, epoch, target) = fixture();
        c.receive(epoch, cc(0), &d);
        bindings.effect_next.number = 68;
        bindings
            .inputs
            .effect_next
            .controller
            .as_mut()
            .unwrap()
            .number = 21;
        c.configure(&bindings, true, Some(target));
        assert!(c.take_invalidation());
        let event = |value| MidiEvent::ControlChange {
            channel: 1,
            controller: 21,
            value,
        };
        c.receive(epoch, cc(127), &d);
        c.receive(epoch, event(0), &d);
        c.receive(epoch, event(127), &d);
        assert!(rx.try_recv().is_err());
        c.configure(&bindings, false, Some(target));
        c.configure(&bindings, true, Some(target));
        c.receive(epoch, event(127), &d);
        assert!(rx.try_recv().is_err());
        c.receive(epoch, event(0), &d);
        c.receive(epoch, event(127), &d);
        let ClockCmd::Pedal(mut request) = rx.try_recv().unwrap() else {
            panic!("not pedal")
        };
        assert_eq!(request.messages[0], [176, 68, 127]);
        request.finish(PedalOutcome::Accepted, "admission fixture; no native send");
    }
}
