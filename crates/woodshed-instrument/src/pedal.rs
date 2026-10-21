// SPDX-License-Identifier: MPL-2.0
//! Portable pedal preparation. No device, runtime, private protocol or retries.
use serde::{Deserialize, Serialize};
pub mod input;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    EffectNext,
    EffectPrevious,
}
impl Action {
    pub const ALL: [Self; 2] = [Self::EffectPrevious, Self::EffectNext];
    pub fn label(self) -> &'static str {
        match self {
            Self::EffectNext => "Effect Next",
            Self::EffectPrevious => "Effect Previous",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageKind {
    #[default]
    ControlChange,
    NoteOn,
    NoteOff,
    ProgramChange,
}
impl MessageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ControlChange => "Control Change",
            Self::NoteOn => "Note On",
            Self::NoteOff => "Note Off",
            Self::ProgramChange => "Program Change",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::ControlChange => Self::NoteOn,
            Self::NoteOn => Self::NoteOff,
            Self::NoteOff => Self::ProgramChange,
            Self::ProgramChange => Self::ControlChange,
        }
    }
    pub fn supports_release(self) -> bool {
        matches!(self, Self::ControlChange | Self::NoteOn)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Binding {
    pub enabled: bool,
    /// Human-facing MIDI channel, 1–16.
    pub channel: u8,
    pub kind: MessageKind,
    pub number: u8,
    pub value: u8,
    /// Optional release after 50 ms: same CC, or Note Off for Note On.
    /// Standalone Note Off and Program Change have no release.
    pub release: Option<u8>,
}
impl Default for Binding {
    fn default() -> Self {
        Self {
            enabled: false,
            channel: 1,
            kind: MessageKind::ControlChange,
            number: 0,
            value: 127,
            release: None,
        }
    }
}
impl Binding {
    /// Validate even deserialized data immediately before admission. No guessed
    /// manufacturer mapping; an enabled binding is an explicit user choice.
    pub fn messages(&self) -> Result<Vec<Vec<u8>>, String> {
        if !self.enabled {
            return Err("Binding is disabled".into());
        }
        if !(1..=16).contains(&self.channel) {
            return Err("Channel must be 1–16".into());
        }
        if self.number > 127 || self.value > 127 || self.release.is_some_and(|v| v > 127) {
            return Err("MIDI number and values must be 0–127".into());
        }
        if self.release.is_some() && !self.kind.supports_release() {
            return Err("Release is supported only for Control Change and Note On".into());
        }
        let channel = self.channel - 1;
        let status = match self.kind {
            MessageKind::ControlChange => 0xb0,
            MessageKind::NoteOn => 0x90,
            MessageKind::NoteOff => 0x80,
            MessageKind::ProgramChange => 0xc0,
        } | channel;
        let mut press = vec![status, self.number];
        if self.kind != MessageKind::ProgramChange {
            press.push(self.value);
        }
        let mut messages = vec![press];
        if let Some(value) = self.release {
            let status = if self.kind == MessageKind::NoteOn {
                0x80 | channel
            } else {
                status
            };
            messages.push(vec![status, self.number, value]);
        }
        Ok(messages)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    pub inputs: input::Mappings,
    pub effect_next: Binding,
    pub effect_previous: Binding,
}
impl Bindings {
    pub fn get(&self, action: Action) -> &Binding {
        match action {
            Action::EffectNext => &self.effect_next,
            Action::EffectPrevious => &self.effect_previous,
        }
    }
    pub fn get_mut(&mut self, action: Action) -> &mut Binding {
        match action {
            Action::EffectNext => &mut self.effect_next,
            Action::EffectPrevious => &mut self.effect_previous,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Queued,
    Refused,
    Accepted,
    /// A writer error may follow OS acceptance. Never infer non-delivery or retry.
    Uncertain,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub id: u64,
    pub action: Action,
    pub outcome: Outcome,
    pub accepted_messages: usize,
    pub detail: String,
}
impl Report {
    pub fn refused(id: u64, action: Action, detail: impl Into<String>) -> Self {
        Self {
            id,
            action,
            outcome: Outcome::Refused,
            accepted_messages: 0,
            detail: detail.into(),
        }
    }
    pub fn summary(&self) -> String {
        format!(
            "{} #{}: {:?} · {} message(s) accepted by OS · {}",
            self.action.label(),
            self.id,
            self.outcome,
            self.accepted_messages,
            self.detail
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pedal_default_is_disabled_and_cannot_encode() {
        let bindings = Bindings::default();
        for action in Action::ALL {
            assert!(bindings.get(action).messages().is_err());
        }
    }
    #[test]
    fn pedal_all_channels_numbers_and_message_kinds_encode_exactly() {
        for channel in 1..=16 {
            for number in 0..=127 {
                for (kind, status) in [
                    (MessageKind::ControlChange, 0xb0),
                    (MessageKind::NoteOn, 0x90),
                    (MessageKind::NoteOff, 0x80),
                    (MessageKind::ProgramChange, 0xc0),
                ] {
                    let binding = Binding {
                        enabled: true,
                        channel,
                        kind,
                        number,
                        value: 127,
                        release: None,
                    };
                    let mut expected = vec![status | (channel - 1), number];
                    if kind != MessageKind::ProgramChange {
                        expected.push(127);
                    }
                    assert_eq!(binding.messages().unwrap(), vec![expected]);
                }
            }
        }
    }
    #[test]
    fn pedal_releases_keep_channel_and_number_and_use_the_correct_status() {
        for (kind, press, release) in [
            (MessageKind::ControlChange, 0xbf, 0xbf),
            (MessageKind::NoteOn, 0x9f, 0x8f),
        ] {
            let binding = Binding {
                enabled: true,
                channel: 16,
                kind,
                number: 127,
                value: 64,
                release: Some(0),
            };
            assert_eq!(
                binding.messages().unwrap(),
                [vec![press, 127, 64], vec![release, 127, 0]]
            );
        }
    }
    #[test]
    fn pedal_invalid_deserialized_ranges_and_release_kinds_refuse() {
        let valid = Binding {
            enabled: true,
            ..Default::default()
        };
        for binding in [
            Binding {
                channel: 0,
                ..valid.clone()
            },
            Binding {
                channel: 17,
                ..valid.clone()
            },
            Binding {
                number: 128,
                ..valid.clone()
            },
            Binding {
                value: 128,
                ..valid.clone()
            },
            Binding {
                release: Some(128),
                ..valid.clone()
            },
            Binding {
                kind: MessageKind::NoteOff,
                release: Some(0),
                ..valid.clone()
            },
            Binding {
                kind: MessageKind::ProgramChange,
                release: Some(0),
                ..valid.clone()
            },
        ] {
            assert!(binding.messages().is_err(), "{binding:?}");
        }
    }
}
