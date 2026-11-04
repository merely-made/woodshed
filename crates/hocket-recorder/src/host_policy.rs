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
mod tests;
