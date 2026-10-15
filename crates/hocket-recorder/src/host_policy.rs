// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Stateless action policy for an ordered recorder owned by its embedding host.
//!
//! Keys must qualify the current host/session and the rendered occurrence. The
//! host supplies admitted rows and custody keys, rather than translating them
//! into another Hocket Session. Validation permits a request, never certifies
//! device/clock/capture admission. The host realizes effects and reflects the
//! resulting accepted state in its next projection.

#[derive(Clone, Debug, PartialEq)]
pub enum HostAction<K> {
    Select(K),
    Record(K),
    Clear(K),
    StopCapture(K),
    Play(K),
    Stop(K),
    Rewind(K),
    NudgeTempo { target: K, delta: f32 },
    ToggleClick(K),
    SetReplace { target: K, replace: bool },
    Add(K),
    Mute(K),
    Solo(K),
    LayerMute(K),
    ToggleMasterClock(K),
}

/// Explicit host capabilities. An unsupported control is not inferred from
/// similarly named Hocket operations. Selection is allowed by the row key set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostCapabilities {
    pub record: bool,
    pub clear: bool,
    pub stop_capture: bool,
    pub play: bool,
    pub stop: bool,
    pub rewind: bool,
    pub tempo: bool,
    pub click: bool,
    pub replace: bool,
    pub add: bool,
    pub mute: bool,
    pub solo: bool,
    pub layer_mute: bool,
    pub master_clock: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostInput {
    Ready,
    /// Record may request the host's lazy input service. This does not claim a
    /// stream is open or an input/output rate pair has been accepted.
    Lazy,
    Unavailable(String),
}

/// A disposable snapshot supplied by the authoritative application.
pub struct HostPolicy<K> {
    /// Global actions must retain the render-time context key. The key must
    /// include a session/owner epoch; a bare universal "transport" key cannot
    /// protect an embedding host from an old document's delayed event.
    pub context: K,
    pub rows: Vec<K>,
    /// Recordable keys come from the admitted backing, not selected/requested
    /// rows or an array index. They must be members of `rows`.
    pub recordable: Vec<K>,
    /// May include unavailable or inactive retained captures absent from rows.
    pub clearable: Vec<K>,
    pub layers: Vec<K>,
    pub selected: Option<K>,
    /// The original accepted armed/recording target. Changing selection must
    /// not change this value. An unmapped retiring capture uses capture_busy.
    pub capture_target: Option<K>,
    /// Includes boundary-queued or retiring capture even when its original
    /// target no longer belongs to the current projectable row set.
    pub capture_busy: bool,
    pub output_available: bool,
    pub input: HostInput,
    /// A host refusal for the currently requested/admitted backing. Stop and
    /// explicit reference removal remain independent of that refusal.
    pub backing_refusal: Option<String>,
    pub unavailable_media: Vec<K>,
    /// Host-owned editable tempo for the evaluated target (e.g. inherited
    /// tempo or a saved bar's own tempo), not a flattened per-row clock copy.
    pub tempo: f32,
    pub capabilities: HostCapabilities,
}

/// An allowed typed host effect. The original enum is shared with views, so
/// hosts do not translate through a second action vocabulary. Construction is
/// private: effects come from evaluation against a fresh authoritative snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct HostEffect<K>(HostAction<K>);

impl<K> HostEffect<K> {
    pub fn action(&self) -> &HostAction<K> {
        &self.0
    }
    pub fn into_action(self) -> HostAction<K> {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostRefusal {
    StaleKey,
    Unsupported(&'static str),
    BusyCapture,
    OutputUnavailable,
    InputUnavailable(String),
    BackingUnavailable(String),
    MediaUnavailable,
    InvalidTempo,
    InvalidSnapshot,
}

impl std::fmt::Display for HostRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleKey => f.write_str("This recorder target has changed. Use its current controls."),
            Self::Unsupported(action) => write!(f, "{action} is unavailable in this recorder."),
            Self::BusyCapture => f.write_str("Stop recording or armed capture before changing this recorder operation."),
            Self::OutputUnavailable => f.write_str("Audio output is unavailable."),
            Self::InputUnavailable(reason) | Self::BackingUnavailable(reason) => f.write_str(reason),
            Self::MediaUnavailable => f.write_str("Captured audio is unavailable or damaged. Restore it or explicitly clear its reference."),
            Self::InvalidTempo => f.write_str("Choose a finite tempo of at least 1 bpm."),
            Self::InvalidSnapshot => f.write_str("The recorder targets are inconsistent; refresh the host projection."),
        }
    }
}
impl std::error::Error for HostRefusal {}

impl<K: Clone + Eq> HostPolicy<K> {
    pub fn evaluate(&self, action: &HostAction<K>) -> Result<HostEffect<K>, HostRefusal> {
        self.validate_keys()?;
        match action {
            HostAction::Select(key) => self.row(key)?,
            HostAction::Record(key) => {
                self.row(key)?;
                if !self.recordable.contains(key) {
                    return Err(HostRefusal::StaleKey);
                }
                self.supported(self.capabilities.record, "Record")?;
                self.idle()?;
                self.output()?;
                self.backing()?;
                if self.unavailable_media.contains(key) {
                    return Err(HostRefusal::MediaUnavailable);
                }
                if let HostInput::Unavailable(reason) = &self.input {
                    return Err(HostRefusal::InputUnavailable(reason.clone()));
                }
            },
            HostAction::Clear(key) => {
                if !self.clearable.contains(key) {
                    return Err(HostRefusal::StaleKey);
                }
                self.supported(self.capabilities.clear, "Clear")?;
                self.idle()?;
                // Removing a retained reference is control-side authoring; the
                // host may expose it even with output or media unavailable.
            },
            HostAction::StopCapture(key) => {
                let accepted = self.capture_target.as_ref().ok_or(HostRefusal::StaleKey)?;
                if key != accepted {
                    return Err(HostRefusal::StaleKey);
                }
                self.supported(self.capabilities.stop_capture, "Stop capture")?;
                return Ok(HostEffect(HostAction::StopCapture(accepted.clone())));
            },
            HostAction::Play(key) => {
                self.global(key)?;
                self.supported(self.capabilities.play, "Play")?;
                self.output()?;
                self.backing()?;
                if self
                    .rows
                    .iter()
                    .any(|row| self.unavailable_media.contains(row))
                {
                    return Err(HostRefusal::MediaUnavailable);
                }
            },
            HostAction::Stop(key) => {
                self.global(key)?;
                self.supported(self.capabilities.stop, "Stop")?;
                // Safe control-side stop remains available after output loss.
            },
            HostAction::Rewind(key) => {
                self.global(key)?;
                self.supported(self.capabilities.rewind, "Rewind")?;
                self.idle()?;
                self.output()?;
                self.backing()?;
            },
            HostAction::NudgeTempo { target, delta } => {
                if target != &self.context && !self.rows.contains(target) {
                    return Err(HostRefusal::StaleKey);
                }
                self.supported(self.capabilities.tempo, "Tempo")?;
                self.idle()?;
                if !delta.is_finite()
                    || !self.tempo.is_finite()
                    || self.tempo < 1.0
                    || !(self.tempo + delta).is_finite()
                {
                    return Err(HostRefusal::InvalidTempo);
                }
                // The owner applies its clock policy; this effect does not
                // retime captured PCM or assert that audio accepted the edit.
            },
            HostAction::ToggleClick(key) => {
                self.global(key)?;
                self.supported(self.capabilities.click, "Click")?;
                self.idle()?;
            },
            HostAction::SetReplace { target, .. } => {
                self.global(target)?;
                self.supported(self.capabilities.replace, "Replace")?;
                self.idle()?;
            },
            HostAction::Add(key) => {
                self.global(key)?;
                self.supported(self.capabilities.add, "Add")?;
            },
            HostAction::Mute(key) => {
                self.row(key)?;
                self.supported(self.capabilities.mute, "Mute")?;
            },
            HostAction::Solo(key) => {
                self.row(key)?;
                self.supported(self.capabilities.solo, "Solo")?;
            },
            HostAction::LayerMute(key) => {
                if !self.layers.contains(key) {
                    return Err(HostRefusal::StaleKey);
                }
                self.supported(self.capabilities.layer_mute, "Layer mute")?;
            },
            HostAction::ToggleMasterClock(key) => {
                self.global(key)?;
                self.supported(self.capabilities.master_clock, "Master clock")?;
                self.idle()?;
            },
        }
        Ok(HostEffect(action.clone()))
    }

    /// Existing host source controls can use the same busy guard without
    /// exporting another source model or a generic source-change framework.
    pub fn permit_source_change(&self) -> Result<(), HostRefusal> {
        self.idle()
    }

    fn validate_keys(&self) -> Result<(), HostRefusal> {
        let repeated = |keys: &[K]| {
            keys.iter()
                .enumerate()
                .any(|(index, key)| keys[..index].contains(key))
        };
        if repeated(&self.rows)
            || repeated(&self.clearable)
            || repeated(&self.layers)
            || self.rows.contains(&self.context)
            || self.layers.contains(&self.context)
            || self.recordable.iter().any(|key| !self.rows.contains(key))
        {
            return Err(HostRefusal::InvalidSnapshot);
        }
        Ok(())
    }
    fn supported(&self, value: bool, action: &'static str) -> Result<(), HostRefusal> {
        if value {
            Ok(())
        } else {
            Err(HostRefusal::Unsupported(action))
        }
    }
    fn global(&self, key: &K) -> Result<(), HostRefusal> {
        if key == &self.context {
            Ok(())
        } else {
            Err(HostRefusal::StaleKey)
        }
    }
    fn row(&self, key: &K) -> Result<(), HostRefusal> {
        if self.rows.contains(key) {
            Ok(())
        } else {
            Err(HostRefusal::StaleKey)
        }
    }
    fn idle(&self) -> Result<(), HostRefusal> {
        if !self.capture_busy && self.capture_target.is_none() {
            Ok(())
        } else {
            Err(HostRefusal::BusyCapture)
        }
    }
    fn output(&self) -> Result<(), HostRefusal> {
        if self.output_available {
            Ok(())
        } else {
            Err(HostRefusal::OutputUnavailable)
        }
    }
    fn backing(&self) -> Result<(), HostRefusal> {
        self.backing_refusal.clone().map_or(Ok(()), |reason| {
            Err(HostRefusal::BackingUnavailable(reason))
        })
    }
}

#[cfg(test)]
mod tests {
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
}
