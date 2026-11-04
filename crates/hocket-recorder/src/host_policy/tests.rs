// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

// Keys qualify both session epoch and role. The same Card number after
// reopening belongs to a different action owner.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Key {
    Context(u64),
    Row(u64, u64),
    Layer(u64, u64, u16),
}
fn fixture() -> HostPolicy<Key> {
    HostPolicy {
        context: Key::Context(1),
        rows: vec![Key::Row(1, 10), Key::Row(1, 20)],
        recordable: vec![Key::Row(1, 10), Key::Row(1, 20)],
        clearable: vec![Key::Row(1, 10), Key::Row(1, 20), Key::Row(1, 30)],
        layers: vec![Key::Layer(1, 10, 0)],
        selected: Some(Key::Row(1, 10)),
        capture_target: None,
        capture_busy: false,
        output_available: true,
        input: HostInput::Lazy,
        backing_refusal: None,
        unavailable_media: Vec::new(),
        tempo: 120.0,
        capabilities: HostCapabilities {
            record: true,
            clear: true,
            stop_capture: true,
            play: true,
            stop: true,
            rewind: true,
            tempo: true,
            click: true,
            replace: true,
            ..Default::default()
        },
    }
}

#[test]
fn rendered_target_survives_selection_change_and_stop_uses_accepted_target() {
    let mut p = fixture();
    let rendered = Key::Row(1, 10);
    p.selected = Some(Key::Row(1, 20));
    assert_eq!(
        p.evaluate(&HostAction::Record(rendered.clone()))
            .unwrap()
            .into_action(),
        HostAction::Record(rendered.clone())
    );
    p.capture_target = Some(rendered.clone());
    assert!(p.evaluate(&HostAction::Select(Key::Row(1, 20))).is_ok());
    assert_eq!(
        p.evaluate(&HostAction::StopCapture(rendered.clone()))
            .unwrap()
            .into_action(),
        HostAction::StopCapture(rendered)
    );
    assert_eq!(
        p.evaluate(&HostAction::StopCapture(Key::Row(1, 20))),
        Err(HostRefusal::StaleKey)
    );
    assert_eq!(p.selected, Some(Key::Row(1, 20)));
    assert_eq!(p.capture_target, Some(Key::Row(1, 10)));
}

#[test]
fn record_requests_lazy_input_but_never_claims_or_changes_capture_admission() {
    let p = fixture();
    assert!(p.evaluate(&HostAction::Record(Key::Row(1, 10))).is_ok());
    assert!(p.capture_target.is_none());
    assert_eq!(p.input, HostInput::Lazy);
    let mut unavailable = fixture();
    unavailable.input = HostInput::Unavailable("Input/output rates are unsupported.".into());
    assert_eq!(
        unavailable.evaluate(&HostAction::Record(Key::Row(1, 10))),
        Err(HostRefusal::InputUnavailable(
            "Input/output rates are unsupported.".into()
        ))
    );
    assert!(unavailable.capture_target.is_none());
}

#[test]
fn stale_context_and_row_keys_cannot_mutate_another_session() {
    let p = fixture();
    for action in [
        HostAction::Select(Key::Row(2, 10)),
        HostAction::Record(Key::Row(2, 10)),
        HostAction::Clear(Key::Row(2, 10)),
        HostAction::Play(Key::Context(2)),
        HostAction::Stop(Key::Context(2)),
        HostAction::NudgeTempo {
            target: Key::Context(2),
            delta: 5.0,
        },
    ] {
        assert_eq!(p.evaluate(&action), Err(HostRefusal::StaleKey));
    }
}

#[test]
fn armed_or_recording_target_guards_retargeting_clear_and_clock_policy() {
    let mut p = fixture();
    p.capture_target = Some(Key::Row(1, 10));
    for action in [
        HostAction::Record(Key::Row(1, 20)),
        HostAction::Clear(Key::Row(1, 30)),
        HostAction::Rewind(Key::Context(1)),
        HostAction::NudgeTempo {
            target: Key::Context(1),
            delta: 5.0,
        },
        HostAction::ToggleClick(Key::Context(1)),
        HostAction::SetReplace {
            target: Key::Context(1),
            replace: true,
        },
    ] {
        assert_eq!(p.evaluate(&action), Err(HostRefusal::BusyCapture));
    }
    assert_eq!(p.permit_source_change(), Err(HostRefusal::BusyCapture));
    assert!(p.evaluate(&HostAction::Stop(Key::Context(1))).is_ok());
    assert!(
        p.evaluate(&HostAction::StopCapture(Key::Row(1, 10)))
            .is_ok()
    );
}

#[test]
fn unavailable_audio_refuses_play_record_but_preserves_explicit_reference_clear() {
    let mut p = fixture();
    p.output_available = false;
    p.unavailable_media = vec![Key::Row(1, 30)];
    assert_eq!(
        p.evaluate(&HostAction::Play(Key::Context(1))),
        Err(HostRefusal::OutputUnavailable)
    );
    assert_eq!(
        p.evaluate(&HostAction::Record(Key::Row(1, 10))),
        Err(HostRefusal::OutputUnavailable)
    );
    assert!(p.evaluate(&HostAction::Clear(Key::Row(1, 30))).is_ok());
    assert!(
        p.evaluate(&HostAction::NudgeTempo {
            target: Key::Context(1),
            delta: 5.0
        })
        .is_ok()
    );
    p.output_available = true;
    assert!(
        p.evaluate(&HostAction::Play(Key::Context(1))).is_ok(),
        "inactive missing reference must not block current backing"
    );
    p.unavailable_media.push(Key::Row(1, 10));
    assert_eq!(
        p.evaluate(&HostAction::Play(Key::Context(1))),
        Err(HostRefusal::MediaUnavailable)
    );
    assert_eq!(
        p.evaluate(&HostAction::Record(Key::Row(1, 10))),
        Err(HostRefusal::MediaUnavailable)
    );
    assert!(p.evaluate(&HostAction::Clear(Key::Row(1, 10))).is_ok());
}

#[test]
fn backing_refusal_does_not_lose_stop_or_retained_clear_operations() {
    let mut p = fixture();
    p.backing_refusal = Some("Requested backing was not admitted.".into());
    assert_eq!(
        p.evaluate(&HostAction::Play(Key::Context(1))),
        Err(HostRefusal::BackingUnavailable(
            "Requested backing was not admitted.".into()
        ))
    );
    assert_eq!(
        p.evaluate(&HostAction::Record(Key::Row(1, 10))),
        Err(HostRefusal::BackingUnavailable(
            "Requested backing was not admitted.".into()
        ))
    );
    assert!(p.evaluate(&HostAction::Stop(Key::Context(1))).is_ok());
    assert!(p.evaluate(&HostAction::Clear(Key::Row(1, 30))).is_ok());
}

#[test]
fn unsupported_hocket_controls_and_invalid_clock_are_refused() {
    let p = fixture();
    for action in [
        HostAction::Add(Key::Context(1)),
        HostAction::Mute(Key::Row(1, 10)),
        HostAction::Solo(Key::Row(1, 10)),
        HostAction::LayerMute(Key::Layer(1, 10, 0)),
        HostAction::ToggleMasterClock(Key::Context(1)),
    ] {
        assert!(matches!(
            p.evaluate(&action),
            Err(HostRefusal::Unsupported(_))
        ));
    }
    for delta in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            p.evaluate(&HostAction::NudgeTempo {
                target: Key::Context(1),
                delta
            }),
            Err(HostRefusal::InvalidTempo)
        );
    }
    let mut bad = fixture();
    bad.rows.push(Key::Row(1, 10));
    assert_eq!(
        bad.evaluate(&HostAction::Select(Key::Row(1, 10))),
        Err(HostRefusal::InvalidSnapshot)
    );
}

#[test]
fn heterogeneous_row_tempo_uses_qualified_target_without_authoring_other_rows() {
    let mut p = fixture();
    p.tempo = 90.0;
    let first = HostAction::NudgeTempo {
        target: Key::Row(1, 10),
        delta: 5.0,
    };
    assert_eq!(p.evaluate(&first).unwrap().into_action(), first);
    p.tempo = 150.0;
    let second = HostAction::NudgeTempo {
        target: Key::Row(1, 20),
        delta: -5.0,
    };
    assert_eq!(p.evaluate(&second).unwrap().into_action(), second);
    assert_eq!(p.tempo, 150.0, "policy must not mutate host clock truth");
    assert!(
        p.evaluate(&HostAction::NudgeTempo {
            target: Key::Context(1),
            delta: 5.0
        })
        .is_ok()
    );
    assert_eq!(
        p.evaluate(&HostAction::NudgeTempo {
            target: Key::Row(2, 10),
            delta: 5.0
        }),
        Err(HostRefusal::StaleKey)
    );
}

#[test]
fn busy_capture_without_projectable_target_still_guards_clock_clear_source_and_record() {
    let mut p = fixture();
    p.capture_busy = true;
    assert!(p.capture_target.is_none());
    for action in [
        HostAction::Record(Key::Row(1, 10)),
        HostAction::Clear(Key::Row(1, 30)),
        HostAction::Rewind(Key::Context(1)),
        HostAction::NudgeTempo {
            target: Key::Row(1, 20),
            delta: 5.0,
        },
        HostAction::ToggleClick(Key::Context(1)),
        HostAction::SetReplace {
            target: Key::Context(1),
            replace: true,
        },
    ] {
        assert_eq!(p.evaluate(&action), Err(HostRefusal::BusyCapture));
    }
    assert_eq!(p.permit_source_change(), Err(HostRefusal::BusyCapture));
    assert!(p.evaluate(&HostAction::Stop(Key::Context(1))).is_ok());
    assert_eq!(
        p.evaluate(&HostAction::StopCapture(Key::Row(1, 10))),
        Err(HostRefusal::StaleKey)
    );
    assert!(p.evaluate(&HostAction::Select(Key::Row(1, 20))).is_ok());
}
