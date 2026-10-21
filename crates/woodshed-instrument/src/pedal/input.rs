// SPDX-License-Identifier: MPL-2.0
//! Prepared input mappings and momentary-controller edges. No device or writer.
use super::Action;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    /// Lowercase ASCII letter/digit, or a supported navigation key name.
    pub key: String,
    pub shift: bool,
    pub alt: bool,
}
impl Shortcut {
    pub fn valid(&self) -> bool {
        (self.key.len() == 1
            && self
                .key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
            || matches!(
                self.key.as_str(),
                "ArrowLeft"
                    | "ArrowRight"
                    | "ArrowUp"
                    | "ArrowDown"
                    | "PageUp"
                    | "PageDown"
                    | "Home"
                    | "End"
            )
    }
    pub fn label(&self) -> String {
        format!(
            "{}{}{}",
            if self.alt { "Alt+" } else { "" },
            if self.shift { "Shift+" } else { "" },
            self.key
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerKind {
    #[default]
    Note,
    ControlChange,
}
impl ControllerKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::ControlChange => "CC",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerBinding {
    pub channel: u8,
    pub kind: ControllerKind,
    pub number: u8,
}
impl ControllerBinding {
    pub fn valid(self) -> bool {
        (1..=16).contains(&self.channel) && self.number <= 127
    }
    pub fn label(self) -> String {
        format!(
            "Ch {} · {} {}",
            self.channel,
            self.kind.label(),
            self.number
        )
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ControllerEvent {
    pub binding: ControllerBinding,
    pub value: u8,
}
impl ControllerEvent {
    pub fn pressed(self) -> bool {
        match self.binding.kind {
            ControllerKind::Note => self.value > 0,
            ControllerKind::ControlChange => self.value >= 64,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Mapping {
    pub keyboard: Option<Shortcut>,
    pub controller: Option<ControllerBinding>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Mappings {
    pub effect_next: Mapping,
    pub effect_previous: Mapping,
}
impl Mappings {
    pub fn get(&self, action: Action) -> &Mapping {
        match action {
            Action::EffectNext => &self.effect_next,
            Action::EffectPrevious => &self.effect_previous,
        }
    }
    pub fn get_mut(&mut self, action: Action) -> &mut Mapping {
        match action {
            Action::EffectNext => &mut self.effect_next,
            Action::EffectPrevious => &mut self.effect_previous,
        }
    }
    pub fn keyboard_action(&self, key: &Shortcut) -> Result<Option<Action>, &'static str> {
        if !key.valid() {
            return Ok(None);
        }
        let mut found = None;
        for action in Action::ALL {
            if self.get(action).keyboard.as_ref() == Some(key) {
                if found.is_some() {
                    return Err("Duplicate keyboard mapping; inputs disarmed");
                }
                found = Some(action);
            }
        }
        Ok(found)
    }
    pub fn conflict(&self) -> bool {
        let a = &self.effect_next;
        let b = &self.effect_previous;
        a.keyboard
            .as_ref()
            .is_some_and(|k| Some(k) == b.keyboard.as_ref())
            || a.controller.is_some_and(|c| Some(c) == b.controller)
    }
}

#[derive(Default)]
pub struct ControllerEdges {
    mappings: Mappings,
    /// Unknown/held until an actual release is observed. Changes and gaps cannot
    /// turn a held control into a new press. No held state is persisted.
    released: [bool; 2],
}
impl ControllerEdges {
    pub fn reset(&mut self) {
        self.released = [false; 2];
    }
    pub fn observe(
        &mut self,
        mappings: &Mappings,
        event: ControllerEvent,
    ) -> Result<Option<Action>, &'static str> {
        if self.mappings != *mappings {
            self.mappings = mappings.clone();
            self.reset();
        }
        if !event.binding.valid() || event.value > 127 {
            return Ok(None);
        }
        if mappings.conflict() {
            return Err("Duplicate input mappings; inputs disarmed");
        }
        for (index, action) in Action::ALL.into_iter().enumerate() {
            if mappings.get(action).controller == Some(event.binding) {
                if !event.pressed() {
                    self.released[index] = true;
                    return Ok(None);
                }
                let fresh = std::mem::replace(&mut self.released[index], false);
                return Ok(fresh.then_some(action));
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub revision: u64,
    pub generation: Option<u64>,
}
#[derive(Default)]
pub struct ControllerBatch {
    pub events: Vec<ControllerEvent>,
    /// Input disappearance, history overflow/reset or a fresh source lifetime.
    pub invalidated: bool,
    pub notice: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(kind: ControllerKind, value: u8) -> ControllerEvent {
        ControllerEvent {
            binding: ControllerBinding {
                channel: 1,
                kind,
                number: 67,
            },
            value,
        }
    }
    #[test]
    fn controller_hold_repeats_once_per_release_and_fresh_press() {
        for kind in [ControllerKind::Note, ControllerKind::ControlChange] {
            let mut mappings = Mappings::default();
            mappings.effect_next.controller = Some(event(kind, 127).binding);
            let mut edges = ControllerEdges::default();
            // A held control at startup/configuration cannot turn into a press.
            for _ in 0..64 {
                assert_eq!(edges.observe(&mappings, event(kind, 127)).unwrap(), None);
            }
            assert_eq!(edges.observe(&mappings, event(kind, 0)).unwrap(), None);
            assert_eq!(
                edges.observe(&mappings, event(kind, 127)).unwrap(),
                Some(Action::EffectNext)
            );
            for _ in 0..64 {
                assert_eq!(edges.observe(&mappings, event(kind, 100)).unwrap(), None);
            }
            assert_eq!(edges.observe(&mappings, event(kind, 0)).unwrap(), None);
            assert_eq!(
                edges.observe(&mappings, event(kind, 127)).unwrap(),
                Some(Action::EffectNext)
            );
        }
    }
    #[test]
    fn mapping_change_reset_and_duplicates_do_not_rearm_a_held_controller() {
        let e = event(ControllerKind::ControlChange, 127);
        let mut mappings = Mappings::default();
        mappings.effect_next.controller = Some(e.binding);
        let mut edges = ControllerEdges::default();
        assert_eq!(
            edges
                .observe(&mappings, ControllerEvent { value: 0, ..e })
                .unwrap(),
            None
        );
        edges.reset();
        assert_eq!(edges.observe(&mappings, e).unwrap(), None);
        mappings.effect_previous.controller = Some(e.binding);
        assert!(edges.observe(&mappings, e).is_err());
        mappings.effect_previous.controller = None;
        assert_eq!(edges.observe(&mappings, e).unwrap(), None);
    }
    #[test]
    fn cc_threshold_and_channel_number_isolation() {
        let e = event(ControllerKind::ControlChange, 127);
        let mut mappings = Mappings::default();
        mappings.effect_previous.controller = Some(e.binding);
        let mut edges = ControllerEdges::default();
        assert_eq!(
            edges
                .observe(&mappings, ControllerEvent { value: 63, ..e })
                .unwrap(),
            None
        );
        let other = ControllerEvent {
            binding: ControllerBinding {
                channel: 2,
                ..e.binding
            },
            ..e
        };
        assert_eq!(edges.observe(&mappings, other).unwrap(), None);
        assert_eq!(
            edges
                .observe(&mappings, ControllerEvent { value: 64, ..e })
                .unwrap(),
            Some(Action::EffectPrevious)
        );
        assert_eq!(edges.observe(&mappings, e).unwrap(), None);
    }
    #[test]
    fn keyboard_exact_modifiers_and_duplicate_mapping_refusal() {
        let key = Shortcut {
            key: "n".into(),
            shift: false,
            alt: true,
        };
        let mut mappings = Mappings::default();
        mappings.effect_next.keyboard = Some(key.clone());
        assert_eq!(
            mappings.keyboard_action(&key).unwrap(),
            Some(Action::EffectNext)
        );
        assert_eq!(
            mappings
                .keyboard_action(&Shortcut {
                    alt: false,
                    ..key.clone()
                })
                .unwrap(),
            None
        );
        mappings.effect_previous.keyboard = Some(key.clone());
        assert!(mappings.keyboard_action(&key).is_err());
        assert!(
            !Shortcut {
                key: "Escape".into(),
                ..key
            }
            .valid()
        );
    }
}
