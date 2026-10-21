use super::*;
use std::collections::VecDeque;

#[derive(Default)]
struct Fake {
    ready: bool,
    generation: Option<u64>,
    sent: Vec<Vec<u8>>,
    failures: VecDeque<bool>,
    close_error: bool,
}
impl Connection for Fake {
    fn ready(&self) -> bool {
        self.ready
    }
    fn generation(&self) -> Option<u64> {
        self.generation
    }
    fn status(&self) -> Option<String> {
        Some(
            if self.ready {
                "subscribed"
            } else {
                "advertising"
            }
            .into(),
        )
    }
    fn send(&mut self, bytes: &[u8], generation: Option<u64>) -> Result<(), String> {
        if generation != self.generation {
            return Err("subscriber changed before send".into());
        }
        self.sent.push(bytes.to_vec());
        if self.failures.pop_front().unwrap_or(false) {
            Err("injected unknown outcome".into())
        } else {
            Ok(())
        }
    }
    fn close(self) -> Result<(), String> {
        if self.close_error {
            Err("injected close failure".into())
        } else {
            Ok(())
        }
    }
}
fn started() -> (Clock, Fake, OutputState) {
    let mut clock = Clock {
        enabled: true,
        ..Default::default()
    };
    let mut out = Fake {
        ready: true,
        generation: Some(1),
        ..Default::default()
    };
    let mut state = OutputState::default();
    clock.step(&mut out, &mut state);
    clock.playing = true;
    clock.step(&mut out, &mut state);
    assert_eq!(out.sent, [vec![0xfa], vec![0xf8]]);
    (clock, out, state)
}
#[test]
fn replacement_between_polls_pauses_without_replay() {
    let (mut clock, mut out, mut state) = started();
    out.generation = Some(2); // no observed false-ready poll
    for _ in 0..4 {
        clock.step(&mut out, &mut state);
    }
    assert_eq!(out.sent.len(), 2);
    assert!(state.rearm_required);
    clock.playing = false;
    clock.step(&mut out, &mut state);
    clock.playing = true;
    clock.step(&mut out, &mut state);
    assert_eq!(&out.sent[2..], &[vec![0xfa], vec![0xf8]]);
}
#[test]
fn disconnected_start_and_reconnect_require_new_arm() {
    let mut clock = Clock {
        enabled: true,
        playing: true,
        ..Default::default()
    };
    let mut out = Fake::default();
    let mut state = OutputState::default();
    clock.step(&mut out, &mut state);
    out.ready = true;
    out.generation = Some(1);
    clock.step(&mut out, &mut state);
    assert!(out.sent.is_empty());
    assert!(state.rearm_required);
}
#[test]
fn disconnect_during_playback_has_no_stop_or_replay_into_new_peer() {
    let (mut clock, mut out, mut state) = started();
    out.ready = false;
    out.generation = Some(0);
    clock.step(&mut out, &mut state);
    out.ready = true;
    out.generation = Some(2);
    clock.step(&mut out, &mut state);
    assert_eq!(out.sent.len(), 2);
    assert!(state.rearm_required);
}
#[test]
fn failed_start_is_latched_and_no_tick_hides_it() {
    let (mut clock, mut out, mut state) = started();
    clock.playing = false;
    clock.step(&mut out, &mut state);
    out.failures.push_back(true);
    clock.playing = true;
    clock.step(&mut out, &mut state);
    let attempts = out.sent.len();
    for _ in 0..5 {
        clock.step(&mut out, &mut state);
    }
    assert_eq!(out.sent.len(), attempts);
    assert_eq!(state.failed, 1);
    assert!(state.last_failure.as_ref().unwrap().contains("unknown"));
    assert!(state.rearm_required);
}
#[test]
fn failed_tick_stops_clock_until_explicit_rearm_and_retains_history() {
    let (mut clock, mut out, mut state) = started();
    out.failures.push_back(true);
    clock.step(&mut out, &mut state);
    let attempts = out.sent.len();
    clock.step(&mut out, &mut state);
    assert_eq!(out.sent.len(), attempts);
    clock.enabled = false;
    clock.step(&mut out, &mut state);
    clock.enabled = true;
    clock.step(&mut out, &mut state);
    assert_eq!(out.sent.len(), attempts + 2);
    assert!(state.last_failure.is_some());
    assert!(state.status.unwrap().contains("refused/uncertain 1"));
}
#[test]
fn stop_is_attempted_once_and_failed_outcome_is_retained() {
    let (mut clock, mut out, mut state) = started();
    out.failures.push_back(true);
    clock.playing = false;
    for _ in 0..3 {
        clock.step(&mut out, &mut state);
    }
    assert_eq!(out.sent, [vec![0xfa], vec![0xf8], vec![0xfc]]);
    assert_eq!(state.failed, 1);
    assert!(state.last_failure.is_some());
}
#[test]
fn close_failure_does_not_disappear() {
    let (mut clock, mut out, mut state) = started();
    out.close_error = true;
    clock.retire(out, &mut state);
    assert!(state.last_failure.unwrap().contains("close"));
    assert!(!state.ready);
}
#[test]
fn retire_does_not_send_stop_to_replacement() {
    let (mut clock, mut out, mut state) = started();
    out.generation = Some(2);
    let before = state.attempted;
    clock.retire(out, &mut state);
    assert_eq!(state.attempted, before);
}
#[test]
fn cancelled_pending_open_is_closed_and_never_published() {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let state = Arc::new(Mutex::new(OutputState::default()));
    let selection = Arc::new(AtomicU64::new(1));
    tx.send(ClockCmd::Connect {
        target: Some("slow".into()),
        revision: 1,
    })
    .unwrap();
    let change = selection.clone();
    let commands = tx.clone();
    run_with(rx, state.clone(), selection, move |_| {
        change.store(2, Ordering::Release);
        commands
            .send(ClockCmd::Connect {
                target: None,
                revision: 2,
            })
            .unwrap();
        commands.send(ClockCmd::Stop).unwrap();
        Ok(Fake::default())
    });
    let s = state.lock().unwrap();
    assert!(s.connected.is_none());
    assert_eq!(s.attempted, 0);
}
#[test]
fn superseded_open_is_not_started() {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let state = Arc::new(Mutex::new(OutputState::default()));
    tx.send(ClockCmd::Connect {
        target: Some("stale".into()),
        revision: 1,
    })
    .unwrap();
    tx.send(ClockCmd::Stop).unwrap();
    run_with::<Fake>(rx, state, Arc::new(AtomicU64::new(2)), |_| {
        panic!("stale open started")
    });
}

fn pedal_request() -> (
    super::super::pedal::Dispatcher,
    Request,
    std::sync::mpsc::Receiver<super::super::PedalReport>,
) {
    let (d, rx) = super::super::pedal::fixture();
    let (_, reply) = d.submit(
        super::super::PedalAction::EffectNext,
        super::super::PedalBinding {
            enabled: true,
            number: 67,
            release: Some(0),
            ..Default::default()
        },
    );
    let ClockCmd::Pedal(request) = rx.recv().unwrap() else {
        panic!("expected pedal")
    };
    (d, request, reply.unwrap())
}
fn pedal_receiver() -> Fake {
    Fake {
        ready: true,
        generation: Some(7),
        ..Default::default()
    }
}
fn due(request: &mut Request) {
    request.due = Instant::now();
    request.expires = request.due + Duration::from_secs(1);
}
#[test]
fn note_previous_and_keyboard_binding_reach_identical_writer_messages() {
    use super::super::{
        MidiEvent, PedalAction, PedalBindings, PedalTarget, controller::Controller,
    };
    use woodshed_core::midi::{ControllerBinding, ControllerKind, PedalShortcut};
    let (dispatcher, rx) = super::super::pedal::fixture();
    let mut bindings = PedalBindings::default();
    bindings.effect_previous.enabled = true;
    bindings.effect_previous.number = 66;
    bindings.effect_previous.release = Some(0);
    let shortcut = PedalShortcut {
        key: "p".into(),
        shift: false,
        alt: false,
    };
    bindings.inputs.effect_previous.keyboard = Some(shortcut.clone());
    bindings.inputs.effect_previous.controller = Some(ControllerBinding {
        channel: 3,
        kind: ControllerKind::Note,
        number: 36,
    });
    let target = PedalTarget {
        revision: 1,
        generation: Some(7),
    };
    let mut controller = Controller::default();
    let epoch = controller.replace_source(true);
    controller.configure(&bindings, false, Some(target));
    controller.configure(&bindings, true, Some(target));
    controller.receive(
        epoch,
        MidiEvent::NoteOff {
            channel: 2,
            note: 36,
            velocity: 64,
        },
        &dispatcher,
    );
    controller.receive(
        epoch,
        MidiEvent::NoteOn {
            channel: 2,
            note: 36,
            velocity: 110,
        },
        &dispatcher,
    );
    let ClockCmd::Pedal(mut request) = rx.try_recv().unwrap() else {
        panic!("expected controller request")
    };
    assert_eq!(request.report.action, PedalAction::EffectPrevious);
    for _ in 0..32 {
        controller.receive(
            epoch,
            MidiEvent::NoteOn {
                channel: 2,
                note: 36,
                velocity: 100,
            },
            &dispatcher,
        );
    }
    assert!(rx.try_recv().is_err());
    let mut note_out = pedal_receiver();
    assert!(!pedal_step(
        &mut request,
        &mut note_out,
        &dispatcher.state,
        &dispatcher.selection,
        1
    ));
    due(&mut request);
    assert!(pedal_step(
        &mut request,
        &mut note_out,
        &dispatcher.state,
        &dispatcher.selection,
        1
    ));
    let action = bindings.inputs.keyboard_action(&shortcut).unwrap().unwrap();
    let (_, reply) =
        dispatcher.submit_expected(action, bindings.get(action).clone(), Some((1, Some(7))));
    let ClockCmd::Pedal(mut request) = rx.try_recv().unwrap() else {
        panic!("expected keyboard binding request")
    };
    let mut key_out = pedal_receiver();
    assert!(!pedal_step(
        &mut request,
        &mut key_out,
        &dispatcher.state,
        &dispatcher.selection,
        1
    ));
    due(&mut request);
    assert!(pedal_step(
        &mut request,
        &mut key_out,
        &dispatcher.state,
        &dispatcher.selection,
        1
    ));
    assert_eq!(note_out.sent, [vec![0xb0, 66, 127], vec![0xb0, 66, 0]]);
    assert_eq!(key_out.sent, note_out.sent);
    assert_eq!(reply.unwrap().recv().unwrap().accepted_messages, 2);
}
#[test]
fn pedal_press_release_exact_once_and_clock_progresses_in_gap() {
    let (d, mut r, reply) = pedal_request();
    let mut out = pedal_receiver();
    assert!(!pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    assert_eq!(out.sent, [vec![0xb0, 67, 127]]);
    assert!(!pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    let mut clock = Clock {
        enabled: true,
        playing: false,
        ..Default::default()
    };
    with_state(&d.state, |s| clock.step(&mut out, s));
    clock.playing = true;
    with_state(&d.state, |s| clock.step(&mut out, s));
    assert_eq!(&out.sent[1..], &[vec![0xfa], vec![0xf8]]);
    due(&mut r);
    assert!(pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    let report = reply.recv().unwrap();
    assert_eq!(report.outcome, PedalOutcome::Accepted);
    assert_eq!(report.accepted_messages, 2);
    assert_eq!(out.sent.last().unwrap(), &[0xb0, 67, 0]);
    assert!(!d.busy.load(Ordering::Acquire));
}
#[test]
fn pedal_first_writer_error_is_uncertain_and_never_sends_release() {
    let (d, mut r, reply) = pedal_request();
    let mut out = pedal_receiver();
    out.failures.push_back(true);
    assert!(pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    let report = reply.recv().unwrap();
    assert_eq!(report.outcome, PedalOutcome::Uncertain);
    assert_eq!(report.accepted_messages, 0);
    assert_eq!(out.sent.len(), 1);
    assert_eq!(d.state.lock().unwrap().failed, 1);
}
#[test]
fn pedal_release_writer_error_reports_one_accepted_and_no_retry() {
    let (d, mut r, reply) = pedal_request();
    let mut out = pedal_receiver();
    assert!(!pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    due(&mut r);
    out.failures.push_back(true);
    assert!(pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    let report = reply.recv().unwrap();
    assert_eq!(report.outcome, PedalOutcome::Uncertain);
    assert_eq!(report.accepted_messages, 1);
    assert!(
        d.state
            .lock()
            .unwrap()
            .last_failure
            .as_ref()
            .unwrap()
            .contains("1 message(s) accepted")
    );
    assert_eq!(out.sent, [vec![0xb0, 67, 127], vec![0xb0, 67, 0]]);
    drop(r);
    assert_eq!(out.sent.len(), 2);
    assert!(!d.busy.load(Ordering::Acquire));
}
#[test]
fn pedal_disconnect_replacement_selection_and_expiry_before_press_refuse_without_send() {
    for fault in 0..5 {
        let (d, mut r, reply) = pedal_request();
        let mut out = pedal_receiver();
        let connection_revision = if fault == 4 { 2 } else { 1 };
        match fault {
            0 => out.ready = false,
            1 => out.generation = Some(8),
            2 => d.selection.store(2, Ordering::Release),
            3 => r.expires = Instant::now() - Duration::from_secs(1),
            _ => {},
        }
        assert!(pedal_step(
            &mut r,
            &mut out,
            &d.state,
            &d.selection,
            connection_revision
        ));
        let report = reply.recv().unwrap();
        assert_eq!(report.outcome, PedalOutcome::Refused);
        assert_eq!(report.accepted_messages, 0);
        assert!(out.sent.is_empty());
    }
}
#[test]
fn pedal_disconnect_replacement_selection_and_expiry_after_press_cancel_release() {
    for fault in 0..5 {
        let (d, mut r, reply) = pedal_request();
        let mut out = pedal_receiver();
        assert!(!pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
        due(&mut r);
        let connection_revision = if fault == 4 { 2 } else { 1 };
        match fault {
            0 => out.ready = false,
            1 => out.generation = Some(8),
            2 => d.selection.store(2, Ordering::Release),
            3 => r.expires = Instant::now() - Duration::from_secs(1),
            _ => {},
        }
        assert!(pedal_step(
            &mut r,
            &mut out,
            &d.state,
            &d.selection,
            connection_revision
        ));
        let report = reply.recv().unwrap();
        assert_eq!(report.outcome, PedalOutcome::Uncertain);
        assert_eq!(report.accepted_messages, 1);
        assert_eq!(out.sent, [vec![0xb0, 67, 127]]);
        assert!(
            d.state
                .lock()
                .unwrap()
                .last_failure
                .as_ref()
                .unwrap()
                .contains("1 message(s) accepted")
        );
        drop(r);
        // Ordinary disconnect and reconnect status must not erase the incomplete gesture.
        let mut clock = Clock::default();
        with_state(&d.state, |state| clock.step(&mut out, state));
        assert_eq!(
            d.state
                .lock()
                .unwrap()
                .pedal_failure
                .as_ref()
                .unwrap()
                .accepted_messages,
            1
        );
        assert!(
            d.state
                .lock()
                .unwrap()
                .status
                .as_ref()
                .unwrap()
                .contains("1 message(s) accepted")
        );
        // The replacement receives no retained release. Only a fresh explicit action can proceed.
        out.ready = true;
        out.generation = Some(9);
        assert!(!d.busy.load(Ordering::Acquire));
        assert_eq!(out.sent.len(), 1);
    }
}
#[test]
fn pedal_close_between_press_and_release_reports_incomplete_without_compensation() {
    let (d, mut r, reply) = pedal_request();
    let mut out = pedal_receiver();
    assert!(!pedal_step(&mut r, &mut out, &d.state, &d.selection, 1));
    drop(r);
    let report = reply.recv().unwrap();
    assert_eq!(report.outcome, PedalOutcome::Uncertain);
    assert_eq!(report.accepted_messages, 1);
    assert_eq!(out.sent, [vec![0xb0, 67, 127]]);
}

#[test]
fn pedal_explicit_none_retains_incomplete_in_closed_readout() {
    let (d, mut request, reply) = pedal_request();
    let mut out = pedal_receiver();
    assert!(!pedal_step(
        &mut request,
        &mut out,
        &d.state,
        &d.selection,
        1
    ));
    drop(request);
    assert_eq!(reply.recv().unwrap().accepted_messages, 1);
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(ClockCmd::Connect {
        target: None,
        revision: 1,
    })
    .unwrap();
    tx.send(ClockCmd::Stop).unwrap();
    run_with::<Fake>(rx, d.state.clone(), d.selection.clone(), |_| {
        panic!("None must not open an output")
    });
    let state = d.state.lock().unwrap();
    assert!(!state.ready);
    assert!(state.connected.is_none());
    assert!(
        state
            .status
            .as_ref()
            .unwrap()
            .contains("1 message(s) accepted")
    );
    assert_eq!(state.pedal_failure.as_ref().unwrap().accepted_messages, 1);
}
