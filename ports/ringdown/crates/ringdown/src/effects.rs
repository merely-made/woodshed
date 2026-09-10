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
use serde::{Deserialize, Serialize};
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
    /// SYNC, TIME, the SYNC note value, LP, HP, FEEDBACK, DRY/WET, in the
    /// order the app writes them.
    ///
    /// `DelaySync` is the note-value key H31 could not find in twenty-five
    /// guesses; the app's own writes name it (H44). Its value is
    /// milliseconds of a note at the current tempo: 375 at 120 bpm, 562.5
    /// or 750 at 80 bpm. One library bank carried `4.0` before any tempo
    /// was applied, so the library form may be a note code; unresolved.
    Delay => "Delay" ["Sync", "DelayTime", "DelaySync", "Lowpass", "Highpass", "Feedback", "DryWet"],
    /// GAIN (dB), VOL (dB), LP (Hz), HP (Hz). The app abbreviates; the wire
    /// wants the full words (H29).
    Distortion => "Distortion" ["Gain", "Volume", "Lowpass", "Highpass"],
    /// Band sliders and an overall gain. The app writes six bands (H44);
    /// `GainBand7` is parsed by the firmware (H31) and never sent by the
    /// app, and `GainBand8` and up are refused.
    Equalizer => "Equalizer" [
        "GainBand1", "GainBand2", "GainBand3", "GainBand4",
        "GainBand5", "GainBand6", "GainBand7", "Gain",
    ],
    /// Threshold, Hysteresis, Range, Hold, Release, Attack, in the order
    /// the app writes them (H44). `Hysteresis` and `Hold` were never tried
    /// in H31's sweep.
    Gate => "Gate" ["Threshold", "Hysteresis", "Range", "Hold", "Release", "Attack"],
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// The knob's **full word**, not its panel label: `Gain`, `Volume`,
    /// `Lowpass`, `Highpass` — where the app shows `GAIN`, `VOL`, `LP`, `HP`.
    /// Matched case-insensitively. One key the firmware does not know refuses
    /// the whole `AddEffect` (H29).
    pub key: String,
    /// In the knob's own units, unconverted: dB for gains, Hz for corner
    /// frequencies. `Lowpass: 1800` is what the app displays as `1.8 kHz`.
    pub value: f64,
    /// A physical control bound to this knob, with its range. Omitted from
    /// the wire when absent. The app carries it inline on every
    /// `UpdateEffect` of a bound knob and also sends `SetController` when
    /// the binding is made (H44).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<Control>,
}

/// A physical control bound to a parameter, as the wire carries it.
///
/// Only `"Slider"` has been seen as a `source` (H44); the range is in the
/// parameter's own units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Control {
    /// The control, by the app's name for it.
    pub source: String,
    /// Value at the control's minimum.
    pub min: f64,
    /// Value at the control's maximum.
    pub max: f64,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        self.params.push(Parameter {
            key,
            value,
            control: None,
        });
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
            control: None,
        });
        self
    }

    /// Bind a physical control to the knob most recently added.
    ///
    /// The app's form: the binding rides inside the parameter (H44).
    pub fn bound_to(mut self, source: &str, min: f64, max: f64) -> Effect {
        if let Some(last) = self.params.last_mut() {
            last.control = Some(Control {
                source: source.to_string(),
                min,
                max,
            });
        }
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
/// `effects`, as the vendor app itself sends it (H39).
pub const BANK_CHAIN_KEY: &str = "effects";

/// The gain a new [`BankSpec`] carries.
///
/// 20 dB, which is what the vendor app gives a placed `Chorus` or
/// `Crystals` bank (H39) and what the bank-creation tests were heard
/// through (H47). Zero would be silent.
pub const DEFAULT_BANK_GAIN: f32 = 20.0;

/// A bank as the app sends it: a named container of gain, an effect chain,
/// and the per-bank feedback-suppression state (H39).
///
/// This is the unit of tone-sharing, and `AddBank` is its transport. The
/// wire shape is the vendor app's own, read off the wire on 2026-09-02:
/// `name`, `gain`, `effects`, `fbk_onoff`, `fbk_params`, in that order. The
/// two feedback fields are what every ringdown bank object had lacked while
/// the instrument stored the bank and never played it (H38, client surface
/// plan Phase D); whether they are what the DSP needs is the next test on
/// the wire.
///
/// Two serialisations, on purpose. The serde derive uses these field names
/// and is the **persistence** form, what a saved [`crate::profile::Profile`]
/// holds; [`BankSpec::to_value`] is the **wire** form. `sustain_killed` is
/// shadow-only state from `SustainKiller`, which the wire object does not
/// carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BankSpec {
    /// Shown on the panel tile.
    pub name: String,
    /// The bank's gain, and **0 is a sentinel: a bank at 0 never plays**
    /// (H47). Not an attenuation — −5 and 20 are both audible, so 0 is a
    /// silent hole between them and the firmware reads it as "no gain set".
    /// The units and the working range are unestablished; the app picks a
    /// value per effect, from −5 for a reverb to 50 for an octaver, which
    /// looks like level compensation.
    pub gain_db: f32,
    /// Feedback suppression on for this bank. Every factory bank carries it,
    /// most `true` (H39).
    pub fbk_onoff: bool,
    /// Feedback-suppression parameters. Always `[]` in every capture so far;
    /// kept as raw JSON until one is seen.
    pub fbk_params: Vec<Value>,
    /// The chain, in signal order.
    pub chain: Vec<Effect>,
    /// Sustain-killer engaged, as last written by `SustainKiller`. Not part
    /// of the wire object; `None` means never written.
    #[serde(default)]
    pub sustain_killed: Option<bool>,
}

impl BankSpec {
    /// An empty bank with this name, at [`DEFAULT_BANK_GAIN`], feedback
    /// suppression on.
    ///
    /// The gain default is deliberately not 0: a bank at 0 is created,
    /// named, selectable and **silent** (H47), which is the failure this
    /// project spent two sessions on.
    pub fn new(name: &str) -> BankSpec {
        BankSpec {
            name: String::from(name),
            gain_db: DEFAULT_BANK_GAIN,
            fbk_onoff: true,
            fbk_params: Vec::new(),
            chain: Vec::new(),
            sustain_killed: None,
        }
    }

    /// Append an effect to the chain.
    pub fn with_effect(mut self, effect: Effect) -> BankSpec {
        self.chain.push(effect);
        self
    }

    /// Set the output gain.
    pub fn gain_db(mut self, gain: f32) -> BankSpec {
        self.gain_db = gain;
        self
    }

    /// Set feedback suppression for this bank.
    pub fn feedback_suppression(mut self, on: bool) -> BankSpec {
        self.fbk_onoff = on;
        self
    }

    /// Set the sustain-killer state (shadow-only; see the struct doc).
    pub fn sustain_killed(mut self, killed: bool) -> BankSpec {
        self.sustain_killed = Some(killed);
        self
    }

    /// The bank object, keys in the app's order: `name, gain, effects,
    /// fbk_onoff, fbk_params`.
    pub fn to_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::from(self.name.as_str()));
        map.insert("gain".to_string(), Value::from(self.gain_db));
        map.insert(
            BANK_CHAIN_KEY.to_string(),
            Value::Array(self.chain.iter().map(Effect::to_value).collect()),
        );
        map.insert("fbk_onoff".to_string(), Value::from(self.fbk_onoff));
        map.insert(
            "fbk_params".to_string(),
            Value::Array(self.fbk_params.clone()),
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
    fn gate_and_delay_take_the_keys_the_app_writes() {
        assert!(
            Effect::new(EffectKind::Gate)
                .with("Hysteresis", 3.0)
                .is_ok()
        );
        assert!(Effect::new(EffectKind::Gate).with("Hold", 10.0).is_ok());
        assert!(
            Effect::new(EffectKind::Delay)
                .with("DelaySync", 375.0)
                .is_ok()
        );
    }

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

    /// The app's exact `AddBank` object from the capture, rebuilt from the
    /// typed model, byte for byte (H39, capture 1, id 23; chain shortened).
    #[test]
    fn the_captured_bank_object_is_reproducible() {
        let bank = BankSpec::new("Crystals").gain_db(20.0).with_effect(
            Effect::unchecked("Chorus")
                .with_unchecked("Frequency", 0.7)
                .bound_to("Slider", 0.6, 3.0)
                .with_unchecked("DryWet", 0.6),
        );
        assert_eq!(
            serde_json::to_string(&bank.to_value()).unwrap(),
            r#"{"name":"Crystals","gain":20.0,"effects":[{"preset":"default","type":"Chorus","bypass":false,"params":[{"key":"Frequency","value":0.7,"control":{"source":"Slider","min":0.6,"max":3.0}},{"key":"DryWet","value":0.6}]}],"fbk_onoff":true,"fbk_params":[]}"#
        );
        // And the wire form reads back into the model.
        let e: Effect = serde_json::from_value(bank.chain[0].to_value()).unwrap();
        assert_eq!(e.params[0].control.as_ref().unwrap().source, "Slider");
        assert_eq!(e.params[1].control, None);
    }

    #[test]
    fn a_bank_spec_serialises_in_model_order_and_omits_absent_fields() {
        let bare = BankSpec::new("octave").to_value();
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"name":"octave","gain":20.0,"effects":[],"fbk_onoff":true,"fbk_params":[]}"#
        );

        let full = BankSpec::new("octave")
            .gain_db(-5.0)
            .feedback_suppression(false)
            .with_effect(Effect::new(EffectKind::Pitch).with("Shift", -12.0).unwrap())
            .to_value();
        assert_eq!(
            serde_json::to_string(&full).unwrap(),
            r#"{"name":"octave","gain":-5.0,"effects":[{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}],"fbk_onoff":false,"fbk_params":[]}"#
        );
    }

    /// A new bank is audible by default. A bank at gain 0 is created,
    /// named, selectable and silent (H47), so the constructor must not
    /// hand anyone one by accident.
    #[test]
    fn a_new_bank_does_not_default_to_a_silent_gain() {
        assert_ne!(BankSpec::new("x").gain_db, 0.0);
        assert_eq!(BankSpec::new("x").gain_db, DEFAULT_BANK_GAIN);
    }

    #[test]
    fn den_error_names_the_whitelist() {
        let text = format!("{}", ParamError::DenNotAccepted(8));
        assert!(text.contains("den 8"), "{text}");
        assert!(text.contains("{1, 2, 4, 16}"), "{text}");
    }
}
