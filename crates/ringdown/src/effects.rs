//! The effect vocabulary and the typed bank model.
//!
//! The firmware implements exactly thirteen effect types, each with its own
//! parameter keys, and refuses anything else. Both facts are hardware-verified
//! and the catalog is closed: the vendor app's picker was read with the
//! scrollbar at both ends (H28), and every key below answered `true` to a
//! single-parameter `AddEffect` while cross-effect controls answered `false`
//! (H31). That closure is what lets the vocabulary be an enum rather than a
//! convention, and a wrong key be refused here rather than by the instrument.
//!
//! What is *not* closed is what the firmware does with a message it parses.
//! `true` means parsed (H27); an [`Effect`] built here is a well-formed
//! request, and whether it sounds is settled by the player, not by this
//! module. See `design_docs/2026-09-02_client_surface_plan.md`.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde::Serialize;
use serde_json::Value;

/// Define the effect table once and derive the enum, the lookup tables and
/// the wire names from it, so the four cannot drift apart.
macro_rules! define_effects {
    ($( $(#[$attr:meta])* $variant:ident => $wire:literal [ $($key:literal),* $(,)? ] ),* $(,)?) => {
        /// One of the thirteen effect types the firmware implements.
        ///
        /// The list is the vendor app's own, captured top to bottom (H28), and
        /// `AddEffect` is a reliable membership oracle for it: every name here
        /// is accepted and every name tried outside it was refused (H24b,
        /// H28). Each variant's doc names the knobs the app shows for it and
        /// the wire key each knob answers to; [`EffectKind::keys`] is the
        /// same list as data.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum EffectKind {
            $(
                $(#[$attr])*
                $variant,
            )*
        }

        impl EffectKind {
            /// Every kind, in the app's display order.
            pub const ALL: &'static [EffectKind] = &[ $(EffectKind::$variant),* ];

            /// The `type` string the wire carries.
            pub const fn wire_name(self) -> &'static str {
                match self {
                    $(EffectKind::$variant => $wire),*
                }
            }

            /// The parameter keys this kind accepts, in the order the app
            /// shows the knobs.
            ///
            /// Established key by key against the instrument (H31). Keys are
            /// matched case-insensitively by the firmware; these are the
            /// app's own spellings.
            pub const fn keys(self) -> &'static [&'static str] {
                match self {
                    $(EffectKind::$variant => &[$($key),*]),*
                }
            }

            /// The kind a wire `type` string names, if any.
            ///
            /// Exact match. Whether the firmware matches type names
            /// case-insensitively, as it does keys, is untested, so this does
            /// not guess.
            pub fn from_wire_name(name: &str) -> Option<EffectKind> {
                match name {
                    $($wire => Some(EffectKind::$variant),)*
                    _ => None,
                }
            }

            /// The canonical spelling of `key` if this kind accepts it.
            ///
            /// Case-insensitive, because the firmware is (H31); the spelling
            /// returned is the app's, which is what goes on the wire.
            pub fn canonical_key(self, key: &str) -> Option<&'static str> {
                self.keys()
                    .iter()
                    .copied()
                    .find(|k| k.eq_ignore_ascii_case(key))
            }
        }

        impl core::fmt::Display for EffectKind {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.wire_name())
            }
        }

        /// Every parameter key the firmware accepts, per effect type, as data.
        ///
        /// The same table as [`EffectKind::keys`], for tooling that iterates
        /// the vocabulary by string. Generated from one list with the enum, so
        /// the two cannot disagree.
        pub const PARAMETER_KEYS: &[(&str, &[&str])] = &[ $( ($wire, &[$($key),*]) ),* ];

        /// The thirteen effect types the firmware will insert (H28, H29), as
        /// wire names.
        pub const EFFECT_TYPES: [&str; 13] = [ $($wire),* ];
    };
}

define_effects! {
    /// FREQ, DRY/WET.
    Chorus => "Chorus" ["Frequency", "DryWet"],
    /// Attack, Release, Threshold, Ratio, DryGain, WetGain, as labelled.
    Compressor => "Compressor" ["Attack", "Release", "Threshold", "Ratio", "DryGain", "WetGain"],
    /// TIME, SYNC, LP, HP, FEEDBACK, DRY/WET.
    ///
    /// The SYNC note-value knob's key is **unknown** after twenty-five
    /// refusals (H31); it is not listed rather than guessed. The working
    /// hypothesis is that with `Sync: 1` the note fraction travels in
    /// `DelayTime`, testable only by ear against a running metronome.
    Delay => "Delay" ["DelayTime", "Sync", "Lowpass", "Highpass", "Feedback", "DryWet"],
    /// GAIN (dB), VOL (dB), LP (Hz), HP (Hz). The app abbreviates; the wire
    /// wants the full words (H29).
    Distortion => "Distortion" ["Gain", "Volume", "Lowpass", "Highpass"],
    /// Seven band sliders and an overall gain. `GainBand8` and up are refused.
    Equalizer => "Equalizer" [
        "GainBand1", "GainBand2", "GainBand3", "GainBand4",
        "GainBand5", "GainBand6", "GainBand7", "Gain",
    ],
    /// Threshold, Range, Release, Attack, as labelled.
    Gate => "Gate" ["Threshold", "Range", "Release", "Attack"],
    /// FREQ, Q.
    Highpass => "Highpass" ["Frequency", "Q"],
    /// FREQ, Q. `Q` is by pattern with the other filters and untested (H31).
    Lowpass => "Lowpass" ["Frequency", "Q"],
    /// FREQ, Q.
    Notch => "Notch" ["Frequency", "Q"],
    /// FREQ, FEEDBACK, DRY/WET.
    Phaser => "Phaser" ["Frequency", "Feedback", "DryWet"],
    /// SHIFT, in semitones. `Shift: -12` is the octave-down oracle the
    /// hardware sessions settled on: unmaskable, and touched by no factory
    /// bank (H36).
    Pitch => "Pitch" ["Shift"],
    /// DECAY, DRY/WET.
    Reverb => "Reverb" ["Decay", "DryWet"],
    /// One knob, FREQ, whose key is `LFO`. Twenty-two other names were
    /// refused first, `Frequency` among them (H31).
    Tremolo => "Tremolo" ["LFO"],
}

/// A request this module refuses to build, because the firmware would parse
/// it and change nothing.
///
/// Every variant is a hardware fact with a finding behind it. Refusing here
/// turns a silent `true` into an error the caller can see, which is the only
/// place in this protocol that can happen: the instrument itself never says
/// no to these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamError {
    /// A parameter key this effect kind does not have. The firmware refuses
    /// the whole `AddEffect` for one unknown key (H29).
    UnknownKey {
        /// The effect the key was offered to.
        kind: EffectKind,
        /// The key as the caller spelled it.
        key: String,
    },
    /// A metronome denominator outside the firmware's whitelist `{1, 2, 4,
    /// 16}`. Every other value is dropped with a `true` reply, including the
    /// 8 and 32 the instrument's own panel offers (H24).
    DenNotAccepted(i64),
}

impl core::fmt::Display for ParamError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParamError::UnknownKey { kind, key } => {
                write!(f, "{kind} has no parameter `{key}`; it accepts")?;
                for (i, k) in kind.keys().iter().enumerate() {
                    write!(f, "{} `{k}`", if i == 0 { "" } else { "," })?;
                }
                Ok(())
            }
            ParamError::DenNotAccepted(den) => write!(
                f,
                "den {den} is outside the firmware's whitelist {{1, 2, 4, 16}}; \
                 the instrument would answer true and change nothing (H24)"
            ),
        }
    }
}

impl core::error::Error for ParamError {}

/// One knob of an effect, as the wire carries it.
///
/// Field order is the wire order, and the wire is order-sensitive (H24), so
/// this is a struct rather than a map: serde emits struct fields in
/// declaration order whatever the map type does.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Parameter {
    /// The knob's **full word**, not its panel label: `Gain`, `Volume`,
    /// `Lowpass`, `Highpass` — where the app shows `GAIN`, `VOL`, `LP`, `HP`.
    /// Matched case-insensitively. One key the firmware does not know refuses
    /// the whole `AddEffect` (H29).
    pub key: String,
    /// In the knob's own units, unconverted: dB for gains, Hz for corner
    /// frequencies. `Lowpass: 1800` is what the app displays as `1.8 kHz`.
    pub value: f64,
}

/// An effect in a bank's chain, as the wire carries it.
///
/// Declaration order is wire order — `preset, type, bypass, params` — and
/// must stay so (H24, H29).
///
/// Built from an [`EffectKind`] by [`Effect::new`], every key added with
/// [`Effect::with`] is checked against that kind's vocabulary. Built from a
/// string by [`Effect::unchecked`], nothing is checked: that path exists for
/// the probe, whose job is to send the instrument things this crate does not
/// yet know about.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Effect {
    /// A named voicing of this type. `"default"` — lowercase, as the app
    /// displays it — is always valid.
    pub preset: String,
    /// One of the thirteen the firmware implements (H28): see
    /// [`EffectKind`]. `AddEffect` refuses any other name, reliably.
    #[serde(rename = "type")]
    pub kind: String,
    /// Loaded but switched off. A real toggle, established by ear on a factory
    /// bank: a single bypassed Pitch is dry, and four bypassed effects around a
    /// live one leave only the live one audible (H37). A client can A/B an
    /// effect without removing it.
    pub bypass: bool,
    /// Knob overrides. Empty means "the preset's values" and is always
    /// accepted; a partial list is accepted too. Must be present — `null` or
    /// absent is refused (H26).
    pub params: Vec<Parameter>,
}

impl Effect {
    /// An effect of `kind` at its `default` preset with no overrides.
    pub fn new(kind: EffectKind) -> Effect {
        Effect::unchecked(kind.wire_name())
    }

    /// An effect whose `type` is taken on trust.
    ///
    /// The unchecked path: no vocabulary is consulted here or in any
    /// [`Effect::with`] that follows, so a name the firmware refuses goes out
    /// as typed and is refused there. For probing; a client should use
    /// [`Effect::new`].
    pub fn unchecked(kind: &str) -> Effect {
        Effect {
            preset: String::from("default"),
            kind: String::from(kind),
            bypass: false,
            params: Vec::new(),
        }
    }

    /// The kind this effect's `type` names, or `None` if it was built
    /// unchecked with a name the vocabulary does not have.
    pub fn checked_kind(&self) -> Option<EffectKind> {
        EffectKind::from_wire_name(&self.kind)
    }

    /// Add a knob override, refusing a key this kind does not have.
    ///
    /// The key is matched case-insensitively and sent in the app's own
    /// spelling. On an effect whose kind is unknown to the vocabulary (see
    /// [`Effect::unchecked`]) any key is accepted as typed.
    pub fn with(mut self, key: &str, value: f64) -> Result<Effect, ParamError> {
        let key = match self.checked_kind() {
            Some(kind) => kind
                .canonical_key(key)
                .ok_or_else(|| ParamError::UnknownKey {
                    kind,
                    key: key.to_string(),
                })?
                .to_string(),
            None => key.to_string(),
        };
        self.params.push(Parameter { key, value });
        Ok(self)
    }

    /// Add a knob override with no vocabulary check, as typed.
    ///
    /// For probing keys the vocabulary does not have yet — Delay's SYNC
    /// note value is the standing example.
    pub fn with_unchecked(mut self, key: &str, value: f64) -> Effect {
        self.params.push(Parameter {
            key: key.to_string(),
            value,
        });
        self
    }

    /// Load it switched off.
    pub fn bypassed(mut self) -> Effect {
        self.bypass = true;
        self
    }

    /// The JSON the wire wants, fields in wire order.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("an Effect is always serialisable")
    }
}

/// The key under which a bank object carries its chain.
///
/// **Provisional.** `effects` is what the H38 `AddBank` sent, and that bank
/// never rendered; the app's own word for the field is unrecovered. Kept as
/// one constant so the bank-creation research (client surface plan, Phase D)
/// changes one line when it learns the answer.
pub const BANK_CHAIN_KEY: &str = "effects";

/// A bank as the app models it: a named container of gain, sustain-killer
/// state and an effect chain (H28).
///
/// This is the unit of tone-sharing, and `AddBank` is its transport. What the
/// firmware wants the object to look like is **not yet established**: of the
/// keys [`BankSpec::to_value`] emits, only `name` has been seen to take effect
/// (H38 put it on the tile). The others follow the app's model and the
/// vocabulary of the per-field methods (`gain` from `SetGainBank`, `killed`
/// from `SustainKiller`), which is a hypothesis, not a finding.
#[derive(Debug, Clone, PartialEq)]
pub struct BankSpec {
    /// Shown on the panel tile.
    pub name: String,
    /// Output gain in decibels, signed and small (`Gain -5`, `Gain 2` on the
    /// app's display, H28). `None` leaves it out.
    pub gain_db: Option<f32>,
    /// Sustain-killer engaged. `None` leaves it out.
    pub sustain_killed: Option<bool>,
    /// The chain, in signal order.
    pub chain: Vec<Effect>,
}

impl BankSpec {
    /// An empty bank with this name.
    pub fn new(name: &str) -> BankSpec {
        BankSpec {
            name: String::from(name),
            gain_db: None,
            sustain_killed: None,
            chain: Vec::new(),
        }
    }

    /// Append an effect to the chain.
    pub fn with_effect(mut self, effect: Effect) -> BankSpec {
        self.chain.push(effect);
        self
    }

    /// Set the output gain.
    pub fn gain_db(mut self, gain: f32) -> BankSpec {
        self.gain_db = Some(gain);
        self
    }

    /// Set the sustain-killer state.
    pub fn sustain_killed(mut self, killed: bool) -> BankSpec {
        self.sustain_killed = Some(killed);
        self
    }

    /// The bank object, keys in the order `name, gain, killed, <chain>`,
    /// absent optionals omitted rather than sent as `null`.
    pub fn to_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::from(self.name.as_str()));
        if let Some(gain) = self.gain_db {
            map.insert("gain".to_string(), Value::from(gain));
        }
        if let Some(killed) = self.sustain_killed {
            map.insert("killed".to_string(), Value::from(killed));
        }
        map.insert(
            BANK_CHAIN_KEY.to_string(),
            Value::Array(self.chain.iter().map(Effect::to_value).collect()),
        );
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    /// The vocabulary table covers every effect type exactly once, with no
    /// key listed twice, and agrees with the enum it was generated beside.
    #[test]
    fn the_parameter_table_covers_every_effect_exactly_once() {
        assert_eq!(EffectKind::ALL.len(), 13);
        assert_eq!(PARAMETER_KEYS.len(), 13);
        for (kind, keys) in PARAMETER_KEYS {
            assert!(EFFECT_TYPES.contains(kind), "{kind} is not an effect type");
            assert!(!keys.is_empty());
            for (i, a) in keys.iter().enumerate() {
                assert!(!keys[i + 1..].contains(a), "{kind} lists {a} twice");
            }
            let from_enum = EffectKind::from_wire_name(kind).unwrap();
            assert_eq!(from_enum.keys(), *keys);
            assert_eq!(from_enum.wire_name(), *kind);
        }
        for kind in EffectKind::ALL {
            assert_eq!(EffectKind::from_wire_name(kind.wire_name()), Some(*kind));
        }
    }

    /// The effect object must serialise in wire order, because the firmware
    /// drops fields that arrive out of sequence. This is the exact four-knob
    /// Distortion the instrument accepted on 2026-09-01 (H29).
    #[test]
    fn a_typed_effect_serialises_in_wire_order() {
        let e = Effect::new(EffectKind::Distortion)
            .with("Gain", 50.0)
            .unwrap()
            .with("Volume", -25.0)
            .unwrap()
            .with("Lowpass", 1800.0)
            .unwrap()
            .with("Highpass", 94.0)
            .unwrap()
            .bypassed();
        let text = serde_json::to_string(&e.to_value()).unwrap();
        assert_eq!(
            text,
            r#"{"preset":"default","type":"Distortion","bypass":true,"params":[{"key":"Gain","value":50.0},{"key":"Volume","value":-25.0},{"key":"Lowpass","value":1800.0},{"key":"Highpass","value":94.0}]}"#
        );
    }

    /// An effect with no overrides still carries `params: []` — the one shape
    /// the firmware accepts for "use the preset" (H26).
    #[test]
    fn a_bare_effect_still_sends_an_empty_params_array() {
        let v = Effect::new(EffectKind::Tremolo).to_value();
        assert_eq!(v["params"], serde_json::json!([]));
        assert_eq!(v["preset"], "default");
        assert_eq!(v["type"], "Tremolo");
    }

    /// A key another effect owns is refused for this one, which is exactly
    /// the cross-effect control the hardware run used (H31).
    #[test]
    fn a_key_from_the_wrong_effect_is_refused_with_the_right_list() {
        let err = Effect::new(EffectKind::Chorus)
            .with("Gain", 1.0)
            .unwrap_err();
        assert_eq!(
            err,
            ParamError::UnknownKey {
                kind: EffectKind::Chorus,
                key: "Gain".into()
            }
        );
        let text = format!("{err}");
        assert!(text.contains("Chorus has no parameter `Gain`"), "{text}");
        assert!(text.contains("`Frequency`, `DryWet`"), "{text}");
    }

    /// Tremolo's one knob is `LFO`; `Frequency` is the refused guess every
    /// other FREQ knob would suggest (H31).
    #[test]
    fn tremolo_takes_lfo_and_not_frequency() {
        assert!(Effect::new(EffectKind::Tremolo).with("LFO", 4.0).is_ok());
        assert!(
            Effect::new(EffectKind::Tremolo)
                .with("Frequency", 4.0)
                .is_err()
        );
    }

    /// Keys match case-insensitively and go out in the app's spelling.
    #[test]
    fn keys_are_matched_case_insensitively_and_sent_canonically() {
        let e = Effect::new(EffectKind::Pitch).with("shift", -12.0).unwrap();
        assert_eq!(e.params[0].key, "Shift");
    }

    /// The unchecked path checks nothing, in either the type or the keys.
    #[test]
    fn the_unchecked_path_sends_what_it_is_given() {
        let e = Effect::unchecked("Octaver")
            .with("Anything", 1.0)
            .unwrap()
            .with_unchecked("More", 2.0);
        assert_eq!(e.checked_kind(), None);
        assert_eq!(e.params.len(), 2);
        assert_eq!(e.params[0].key, "Anything");

        // A checked kind can still take an unchecked key, for probing.
        let d = Effect::new(EffectKind::Delay).with_unchecked("NoteValue", 0.25);
        assert_eq!(d.params[0].key, "NoteValue");
    }

    #[test]
    fn a_bank_spec_serialises_in_model_order_and_omits_absent_fields() {
        let bare = BankSpec::new("octave").to_value();
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"name":"octave","effects":[]}"#
        );

        let full = BankSpec::new("octave")
            .gain_db(-5.0)
            .sustain_killed(false)
            .with_effect(Effect::new(EffectKind::Pitch).with("Shift", -12.0).unwrap())
            .to_value();
        assert_eq!(
            serde_json::to_string(&full).unwrap(),
            r#"{"name":"octave","gain":-5.0,"killed":false,"effects":[{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}]}"#
        );
    }

    #[test]
    fn den_error_names_the_whitelist() {
        let text = format!("{}", ParamError::DenNotAccepted(8));
        assert!(text.contains("den 8"), "{text}");
        assert!(text.contains("{1, 2, 4, 16}"), "{text}");
    }
}
