//! The JSON-RPC layer: requests, responses, and the 32 methods.
//!
//! This is JSON-RPC 2.0 in shape, with one deviation that matters — see
//! [`JSONRPC_VERSION`]. Encoding a request produces a complete message ready to
//! hand to [`crate::llt`] for framing; decoding takes whatever arrived on the
//! response characteristic and sorts it into a result or an error.
//!
//! Nothing here does I/O or tracks a connection. Request ids are chosen by the
//! caller, because the caller is what owns the sequence.
//!
//! # Provenance
//!
//! Recovered by static analysis and **confirmed against a real instrument on
//! 2026-08-27**: `GetStatus` round-trips, and its reply is pinned as a test
//! fixture. See `design_docs/2026-08-27_ringdown_founding.md`, Findings F4,
//! F12, and H4.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol version ringdown sends, as a JSON number.
///
/// The vendor's client declares this field as a float and encodes it
/// numerically, putting `"jsonrpc":2.0` on the wire where the JSON-RPC 2.0
/// specification asks for the string `"2.0"`. Ringdown sends what the vendor
/// sends.
///
/// **Hardware correction (2026-08-27).** An earlier reading of this treated the
/// numeric form as *required*, and warned that a spec-compliant string would be
/// rejected. Testing against a real guitar falsified that: the device accepts
/// both forms, and answers both identically. It is lenient on input and it is
/// the *reply* that is fixed — see [`Response::version`], which always arrives
/// as a string. Matching the vendor's request encoding is still the right
/// default, but as the conservative choice rather than a necessity.
pub const JSONRPC_VERSION: f32 = 2.0;

/// Define the method table once, and derive everything from it.
///
/// The wire spellings are irregular (`SetEQGain`, not `SetEqGain`), so they
/// have to be written out. Writing them out *twice* — once for serde and once
/// for a lookup — is how the two drift apart, so this generates the enum, its
/// serde renames, [`Method::ALL`], and [`Method::wire_name`] from a single
/// list.
macro_rules! define_methods {
    ($( $(#[$attr:meta])* $variant:ident => $wire:literal ),* $(,)?) => {
        /// Every method the guitar accepts.
        ///
        /// The wire name is not the Rust name: the wire uses pascal case and
        /// keeps acronyms upper (`"GetStatus"`, `"SetEQBandGain"`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Method {
            $(
                $(#[$attr])*
                #[serde(rename = $wire)]
                $variant,
            )*
        }

        impl Method {
            /// Every method, for exhaustive iteration in tests and tooling.
            pub const ALL: &'static [Method] = &[ $(Method::$variant),* ];

            /// The name this method has on the wire.
            pub fn wire_name(self) -> &'static str {
                match self {
                    $(Method::$variant => $wire),*
                }
            }
        }
    };
}

define_methods! {
    // -- System --
    /// Battery, storage, cpu id, and both firmware versions.
    GetStatus => "GetStatus",
    /// Firmware version.
    GetVersion => "GetVersion",
    /// Set the instrument's clock; recordings are timestamped from it.
    SetDate => "SetDate",
    /// Run the instrument's calibration routine.
    Calibrate => "Calibrate",

    // -- Configuration --
    /// Read the whole configuration, including the live effect catalog.
    ReadConfig => "ReadConfig",
    /// Persist the working configuration to the instrument.
    SaveConfig => "SaveConfig",
    /// Replace the working configuration.
    SetConfig => "SetConfig",

    // -- Banks (presets) --
    /// Read one bank.
    ReadBank => "ReadBank",
    /// Make a bank the active one.
    SwitchBank => "SwitchBank",
    /// Append a bank.
    AddBank => "AddBank",
    /// Delete a bank.
    RemoveBank => "RemoveBank",
    /// Reorder a bank.
    MoveBank => "MoveBank",
    /// Rename a bank.
    SetBankName => "SetBankName",
    /// Set a bank's output gain.
    SetGainBank => "SetGainBank",

    // -- Effects --
    /// Append an effect to a bank's chain.
    AddEffect => "AddEffect",
    /// Replace an effect in a bank's chain.
    UpdateEffect => "UpdateEffect",
    /// Remove an effect from a bank's chain.
    RemoveEffect => "RemoveEffect",
    /// Reorder an effect within a bank's chain.
    MoveEffect => "MoveEffect",
    /// Bind a physical control to an effect parameter.
    SetController => "SetController",

    // -- Equalizer --
    /// Set overall equalizer gain.
    SetEqGain => "SetEQGain",
    /// Set the gain of one equalizer band.
    SetEqBandGain => "SetEQBandGain",

    // -- Aux routing --
    /// Toggle the aux input.
    AuxIn => "AuxIn",
    /// Aux input dry/wet mix.
    AuxInDryWet => "AuxInDryWet",
    /// Toggle the aux output.
    AuxOut => "AuxOut",
    /// Aux output dry/wet mix.
    AuxOutDryWet => "AuxOutDryWet",

    // -- Metronome --
    /// Start the metronome.
    StartMetronome => "StartMetronome",
    /// Stop the metronome.
    StopMetronome => "StopMetronome",
    /// Change tempo or time signature while running.
    UpdateMetronome => "UpdateMetronome",

    // -- Recording --
    /// Begin recording.
    StartRecording => "StartRecording",
    /// End recording.
    StopRecording => "StopRecording",

    // -- Other --
    /// Sustain-killer state for a bank.
    SustainKiller => "SustainKiller",
    /// Metadata for a stored file.
    GetFileInfo => "GetFileInfo",
}

/// A request, ready to serialize.
///
/// `params` is deliberately a [`Value`]: its shape varies per method — a map of
/// strings for most, a typed object for others — and pinning it to one Rust
/// type would misrepresent the protocol. The [`params`] module builds correct
/// ones.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Request {
    /// Protocol version. Always [`JSONRPC_VERSION`], and numeric.
    #[serde(rename = "jsonrpc")]
    pub version: f32,
    /// Correlates the response, and the LLT object id if the message is
    /// chunked.
    #[serde(rename = "id")]
    pub id: i64,
    /// Which method to invoke.
    pub method: Method,
    /// Method-specific arguments.
    pub params: Value,
}

impl Request {
    /// Build a request with the correct protocol version.
    pub fn new(id: i64, method: Method, params: Value) -> Request {
        Request {
            version: JSONRPC_VERSION,
            id,
            method,
            params,
        }
    }

    /// Build a request taking no arguments.
    pub fn no_params(id: i64, method: Method) -> Request {
        Request::new(id, method, Value::Object(Default::default()))
    }

    /// Serialize to the string that goes on the wire (or into LLT frames).
    pub fn encode(&self) -> Result<String, RpcError> {
        serde_json::to_string(self).map_err(|e| RpcError::Encode(e.to_string()))
    }
}

/// Accept a protocol version given as either a JSON number or a JSON string.
///
/// Needed because the device is inconsistent about which it uses, and being
/// strict here means discarding every valid reply.
fn lenient_version<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) as f32),
        Value::String(s) => Ok(s.trim().parse().unwrap_or(0.0)),
        _ => Ok(0.0),
    }
}

/// An error object returned by the device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceError {
    /// Device error code. Numeric on the wire, and not necessarily an integer.
    pub code: f32,
    /// Human-readable description from the device.
    pub message: String,
}

/// A decoded response.
///
/// `id` is `f64` because the device sends it as a JSON number and the vendor's
/// own client reads it as a float. Use [`Response::id_as_i64`] to compare it
/// against the id sent.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Response {
    /// Protocol version echoed by the device.
    ///
    /// Accepts a number *or* a string, because the device is asymmetric: it
    /// takes `2.0` as a number on the way in and answers with `"2.0"` as a
    /// string on the way out. A client that assumes the reply mirrors the
    /// request fails to parse every response it receives — which is exactly
    /// what happened here before hardware said otherwise.
    #[serde(rename = "jsonrpc", default, deserialize_with = "lenient_version")]
    pub version: f32,
    /// The id of the request this answers.
    #[serde(rename = "id")]
    pub id: f64,
    /// The result, absent when `error` is present.
    #[serde(default)]
    pub result: Option<Value>,
    /// The error, absent on success.
    #[serde(default)]
    pub error: Option<DeviceError>,
}

impl Response {
    /// Parse a response from a complete message.
    pub fn decode(text: &str) -> Result<Response, RpcError> {
        serde_json::from_str(text.trim()).map_err(|e| RpcError::Decode(e.to_string()))
    }

    /// The request id as an integer, if it is one.
    ///
    /// Returns `None` for a fractional id, which would mean the device answered
    /// something this client did not send.
    pub fn id_as_i64(&self) -> Option<i64> {
        let rounded = self.id as i64;
        (rounded as f64 == self.id).then_some(rounded)
    }

    /// Whether this response answers the given request id.
    pub fn answers(&self, request_id: i64) -> bool {
        self.id_as_i64() == Some(request_id)
    }

    /// The result, or the device's error as an `Err`.
    pub fn into_result(self) -> Result<Value, RpcError> {
        match (self.result, self.error) {
            (_, Some(e)) => Err(RpcError::Device(e)),
            (Some(v), None) => Ok(v),
            (None, None) => Ok(Value::Null),
        }
    }

    /// Deserialize the result into a concrete type.
    pub fn result_as<T: for<'de> Deserialize<'de>>(self) -> Result<T, RpcError> {
        let value = self.into_result()?;
        serde_json::from_value(value).map_err(|e| RpcError::Decode(e.to_string()))
    }
}

/// What the device reports for [`Method::GetStatus`].
///
/// The two firmware versions are separate because the instrument runs two
/// processors: one for connectivity, one for the audio DSP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// Device model identifier.
    #[serde(default)]
    pub device: String,
    /// Processor serial, useful for identifying the exact silicon.
    #[serde(rename = "cpu_id", default)]
    pub cpu_id: String,
    /// Battery remaining, **0–100**.
    ///
    /// Note the inconsistency with [`Status::free_space_fraction`], which is a
    /// 0–1 fraction: a real device reported `batt_left: 46` alongside
    /// `free_pct: 0.9949`. The two fields use different scales, and the names
    /// here say which is which rather than papering over it.
    #[serde(rename = "batt_left", default)]
    pub battery_percent: f32,
    /// Free storage in gigabytes.
    #[serde(rename = "free_gb", default)]
    pub free_space_gb: f32,
    /// Free storage as a **0–1 fraction**, not a percentage.
    #[serde(rename = "free_pct", default)]
    pub free_space_fraction: f32,
    /// Connectivity processor firmware version.
    #[serde(rename = "version_esp", default)]
    pub version_esp: String,
    /// Audio DSP processor firmware version.
    #[serde(rename = "version_stm", default)]
    pub version_stm: String,
}

/// Things that can go wrong at the RPC layer.
#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    /// The device returned an error object.
    Device(DeviceError),
    /// A response could not be parsed.
    Decode(String),
    /// A request could not be serialized.
    Encode(String),
}

impl core::fmt::Display for RpcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RpcError::Device(e) => write!(f, "device error {}: {}", e.code, e.message),
            RpcError::Decode(m) => write!(f, "could not decode response: {m}"),
            RpcError::Encode(m) => write!(f, "could not encode request: {m}"),
        }
    }
}

impl core::error::Error for RpcError {}

/// One key in a method's `params` object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamKey {
    /// The wire spelling, which is terse and not uniform — see F13.
    pub name: &'static str,
    /// Whether every call must carry it.
    ///
    /// An optional key is one the vendor's client leaves out entirely rather
    /// than sending as `null`, and [`params`] follows that: omission means
    /// "leave this alone", where `null` would be a value.
    pub required: bool,
}

const fn req(name: &'static str) -> ParamKey {
    ParamKey {
        name,
        required: true,
    }
}

const fn opt(name: &'static str) -> ParamKey {
    ParamKey {
        name,
        required: false,
    }
}

/// The shape of a method's `params`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamShape {
    /// Takes no arguments. Send `{}`.
    None,
    /// An object with these keys.
    Object(&'static [ParamKey]),
    /// The method exists but its parameters were never recovered.
    ///
    /// Distinct from [`ParamShape::None`] on purpose: "takes nothing" and "we
    /// do not know what it takes" look identical at a call site and are
    /// opposite facts. `SetConfig` is the live example — it certainly takes a
    /// configuration, and `ReadConfig` wedging the firmware (H18) means the
    /// shape it would mirror has never been seen.
    Unrecovered,
}

/// What `params` a method expects.
///
/// Bound method by method from the vendor's `*Params` classes and their
/// `@SerialName` annotations (F13). This is the Phase 2 done-condition "every
/// method's params shape is bound", and it is a `match` without a wildcard
/// arm so that **adding a method to the table cannot compile until its shape
/// is declared here too** — the same anti-drift discipline that generates the
/// wire names from one table.
///
/// A declaration here is what the vendor's client sends, not proof the
/// firmware accepts it. Only the methods marked hardware-verified in the
/// founding doc's Findings have been exercised.
pub fn param_shape(method: Method) -> ParamShape {
    use Method::*;
    match method {
        // -- System --
        GetStatus | GetVersion => ParamShape::None,
        SetDate => ParamShape::Object(DATE),
        Calibrate => ParamShape::None,

        // -- Configuration --
        ReadConfig | SaveConfig => ParamShape::None,
        SetConfig => ParamShape::Unrecovered,

        // -- Banks --
        ReadBank | SwitchBank | RemoveBank => ParamShape::Object(BANK),
        AddBank => ParamShape::Object(ADD_BANK),
        MoveBank => ParamShape::Object(MOVE_BANK),
        SetBankName => ParamShape::Object(BANK_NAME),
        SetGainBank => ParamShape::Object(BANK_GAIN),

        // -- Effects --
        AddEffect => ParamShape::Object(ADD_EFFECT),
        UpdateEffect => ParamShape::Object(UPDATE_EFFECT),
        RemoveEffect => ParamShape::Object(EFFECT),
        MoveEffect => ParamShape::Object(MOVE_EFFECT),
        SetController => ParamShape::Object(CONTROLLER),

        // -- Equalizer --
        SetEqGain => ParamShape::Object(EQ_GAIN),
        SetEqBandGain => ParamShape::Object(EQ_BAND_GAIN),

        // -- Aux routing --
        AuxIn | AuxOut => ParamShape::Object(TOGGLE),
        AuxInDryWet | AuxOutDryWet => ParamShape::Object(VALUE),

        // -- Metronome --
        StartMetronome | UpdateMetronome => ParamShape::Object(METRONOME),
        StopMetronome => ParamShape::None,

        // -- Recording --
        StartRecording => ParamShape::Object(RECORDING),
        StopRecording => ParamShape::None,

        // -- Other --
        SustainKiller => ParamShape::Object(SUSTAIN_KILLER),
        GetFileInfo => ParamShape::Object(FILE),
    }
}

const DATE: &[ParamKey] = &[
    req("year"),
    req("month"),
    req("day"),
    req("hour"),
    req("minute"),
    req("second"),
];

const BANK: &[ParamKey] = &[req("bank_num")];
const ADD_BANK: &[ParamKey] = &[req("bank_num"), req("bank")];
/// Banks reorder with `src`/`dst` where effects use `effect_num`/`effect_dest`.
/// The inconsistency is the vendor's and is preserved rather than smoothed.
const MOVE_BANK: &[ParamKey] = &[req("src"), req("dst")];
const BANK_NAME: &[ParamKey] = &[req("bank_num"), req("name")];
const BANK_GAIN: &[ParamKey] = &[req("bank_num"), req("gain")];

const ADD_EFFECT: &[ParamKey] = &[req("bank_num"), req("effect")];
const UPDATE_EFFECT: &[ParamKey] = &[req("bank_num"), req("effect_num"), req("effect")];
const EFFECT: &[ParamKey] = &[req("bank_num"), req("effect_num")];
const MOVE_EFFECT: &[ParamKey] = &[req("bank_num"), req("effect_num"), req("effect_dest")];
const CONTROLLER: &[ParamKey] = &[
    req("bank_num"),
    req("effect_num"),
    req("parameter"),
    req("source"),
    opt("min"),
    opt("max"),
];

const EQ_GAIN: &[ParamKey] = &[req("gain")];
const EQ_BAND_GAIN: &[ParamKey] = &[req("band"), req("gain")];

const TOGGLE: &[ParamKey] = &[req("toggle")];
const VALUE: &[ParamKey] = &[req("value")];

/// A time signature and a loop length: `num`/`den` are the numerator and
/// **denominator**, `bars` the loop length, and `bpm` counts `den` notes.
///
/// Established from the loop files, where the same three fields appear and
/// reproduce every recording's duration, and confirmed by the instrument's
/// owner for a known take. See [`crate::loopfile`].
///
/// **`den` writes, with two conditions** — the fields must be in declaration
/// order (`bpm, num, den`; the parser drops a `den` that precedes `num`), and
/// the value must be in the firmware's whitelist `{1, 2, 4, 16}`; 8 and 32
/// exist on the instrument's panel but are silently refused over RPC. Sent
/// without `bpm` the call returns `false` outright. See [`params::metronome`].
/// `ReadMetronome` reports the field correctly, so a caller can always tell
/// what the instrument is actually set to.
const METRONOME: &[ParamKey] = &[req("bpm"), opt("num"), opt("den"), opt("bars")];

const RECORDING: &[ParamKey] = &[req("free")];
const SUSTAIN_KILLER: &[ParamKey] = &[req("bank_num"), opt("killed"), opt("reset")];
const FILE: &[ParamKey] = &[req("name")];

/// Every parameter key the firmware accepts, per effect type.
///
/// Established key by key against the instrument on 2026-09-01: each entry
/// answered `true` to a single-parameter `AddEffect`, and four cross-effect
/// controls (`Chorus.Gain`, `Reverb.Attack`, ...) answered `false`, so this is
/// per-effect vocabulary rather than one shared list. Keys match
/// case-insensitively but are recorded as the app's own words.
///
/// Tremolo's single FREQ knob is `LFO` -- twenty-two other names were refused
/// first, including `Frequency`, which every other FREQ knob uses. Delay's
/// SYNC note-value knob is absent because its key is still unknown after
/// twenty-five refusals; the working hypothesis is that with `Sync: 1` the
/// note fraction travels in `DelayTime`. See the founding doc, H31.
pub const PARAMETER_KEYS: &[(&str, &[&str])] = &[
    ("Chorus", &["Frequency", "DryWet"]),
    (
        "Compressor",
        &[
            "Attack",
            "Release",
            "Threshold",
            "Ratio",
            "DryGain",
            "WetGain",
        ],
    ),
    (
        "Delay",
        &[
            "DelayTime",
            "Sync",
            "Lowpass",
            "Highpass",
            "Feedback",
            "DryWet",
        ],
    ),
    ("Distortion", &["Gain", "Volume", "Lowpass", "Highpass"]),
    (
        "Equalizer",
        &[
            "GainBand1",
            "GainBand2",
            "GainBand3",
            "GainBand4",
            "GainBand5",
            "GainBand6",
            "GainBand7",
            "Gain",
        ],
    ),
    ("Gate", &["Threshold", "Range", "Release", "Attack"]),
    ("Highpass", &["Frequency", "Q"]),
    ("Lowpass", &["Frequency", "Q"]),
    ("Notch", &["Frequency", "Q"]),
    ("Phaser", &["Frequency", "Feedback", "DryWet"]),
    ("Pitch", &["Shift"]),
    ("Reverb", &["Decay", "DryWet"]),
    ("Tremolo", &["LFO"]),
];

/// The thirteen effect types the firmware will insert (H28, H29).
pub const EFFECT_TYPES: [&str; 13] = [
    "Chorus",
    "Compressor",
    "Delay",
    "Distortion",
    "Equalizer",
    "Gate",
    "Highpass",
    "Lowpass",
    "Notch",
    "Phaser",
    "Pitch",
    "Reverb",
    "Tremolo",
];

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
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Effect {
    /// A named voicing of this type. `"default"` — lowercase, as the app
    /// displays it — is always valid.
    pub preset: String,
    /// One of the thirteen the firmware implements (H28): `Chorus`,
    /// `Compressor`, `Delay`, `Distortion`, `Equalizer`, `Gate`, `Highpass`,
    /// `Lowpass`, `Notch`, `Phaser`, `Pitch`, `Reverb`, `Tremolo`.
    /// `AddEffect` refuses any other name, reliably.
    #[serde(rename = "type")]
    pub kind: String,
    /// Loaded but switched off. Accepted by the firmware; whether it actually
    /// silences the effect is not yet established by ear.
    pub bypass: bool,
    /// Knob overrides. Empty means "the preset's values" and is always
    /// accepted; a partial list is accepted too. Must be present — `null` or
    /// absent is refused (H26).
    pub params: Vec<Parameter>,
}

impl Effect {
    /// An effect of `kind` at its `default` preset with no overrides.
    pub fn new(kind: &str) -> Effect {
        Effect {
            preset: String::from("default"),
            kind: String::from(kind),
            bypass: false,
            params: Vec::new(),
        }
    }

    /// Add a knob override.
    pub fn with(mut self, key: &str, value: f64) -> Effect {
        self.params.push(Parameter {
            key: String::from(key),
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

/// Builders for each method's arguments.
///
/// The wire keys are terse and inconsistent (`bank_num`, `effect_num`, `src`,
/// `dst`), so these exist to keep that vocabulary in one place rather than
/// spread across call sites as string literals. Each is checked against
/// [`param_shape`] by test, so a constructor cannot drift from the declared
/// shape without something failing.
pub mod params {
    use super::*;
    use serde_json::json;

    /// Build an object, leaving out the keys whose value is absent.
    ///
    /// Omission and `null` are different messages: one says "leave this as it
    /// is", the other says "set it to nothing". `serde_json::json!` renders an
    /// absent `Option` as `null`, so building these by hand would quietly send
    /// the second while meaning the first.
    fn object(fields: &[(&str, Option<Value>)]) -> Value {
        let mut map = serde_json::Map::new();
        for (key, value) in fields {
            if let Some(value) = value {
                map.insert((*key).to_string(), value.clone());
            }
        }
        Value::Object(map)
    }

    /// No arguments.
    pub fn none() -> Value {
        json!({})
    }

    /// A bank, by index.
    ///
    /// A bank is a named container — `{ name, gain, sustain killer, effect
    /// chain }` — and the instrument holds a whole **library** of them. A
    /// *profile* assigns nine of those banks to the front-panel grid, and this
    /// index addresses a **grid slot**, 0–8 in scroll order, not a library
    /// entry. `SwitchBank` moves the panel selection, hardware-confirmed.
    ///
    /// There is no scratch slot: `bank_num: 0` is the player's first preset,
    /// and a write aimed there edits something they use.
    ///
    /// **An empty tile is not a bank.** The factory profile leaves slot 8
    /// empty, and it is tempting as scratch space, but `AddEffect` there
    /// stores a chain the DSP never plays and `SetBankName` labels a tile with
    /// no bank behind it — both answering `true`. Creating a bank in an empty
    /// tile needs `AddBank`; `AddEffect` only *modifies* a bank that exists.
    /// See the founding doc, H36.
    ///
    /// Note that `ReadBank` answers `""` for every index, populated or not, so
    /// it cannot be used to check whether a bank is empty — or to verify that a
    /// write to one landed.
    pub fn bank(bank_num: i64) -> Value {
        json!({ "bank_num": bank_num })
    }

    /// A single unnamed value, for the dry/wet and gain setters.
    pub fn value(v: f32) -> Value {
        json!({ "value": v })
    }

    /// An on/off toggle, for the aux switches.
    pub fn toggle(on: bool) -> Value {
        json!({ "toggle": on })
    }

    /// Overall equalizer gain.
    pub fn eq_gain(gain: f32) -> Value {
        json!({ "gain": gain })
    }

    /// Gain for one equalizer band.
    pub fn eq_band_gain(band: i64, gain: f32) -> Value {
        json!({ "band": band, "gain": gain })
    }

    /// A bank's output gain.
    pub fn bank_gain(bank_num: i64, gain: f32) -> Value {
        json!({ "bank_num": bank_num, "gain": gain })
    }

    /// Rename a bank.
    ///
    /// Takes effect only on a slot that already holds an effect; sent to an
    /// empty slot the instrument answers `true` and keeps nothing (H33). Add
    /// the first effect, then name the bank. The panel is the only read-back.
    pub fn bank_name(bank_num: i64, name: &str) -> Value {
        json!({ "bank_num": bank_num, "name": name })
    }

    /// Reorder a bank.
    pub fn move_bank(from: i64, to: i64) -> Value {
        json!({ "src": from, "dst": to })
    }

    /// Reorder an effect within a bank.
    pub fn move_effect(bank_num: i64, from: i64, to: i64) -> Value {
        json!({ "bank_num": bank_num, "effect_num": from, "effect_dest": to })
    }

    /// Remove an effect from a bank.
    pub fn remove_effect(bank_num: i64, effect_num: i64) -> Value {
        json!({ "bank_num": bank_num, "effect_num": effect_num })
    }

    /// Add an effect to a bank. `effect` is a serialized effect object.
    pub fn add_effect(bank_num: i64, effect: Value) -> Value {
        json!({ "bank_num": bank_num, "effect": effect })
    }

    /// Replace an effect in a bank.
    pub fn update_effect(bank_num: i64, effect_num: i64, effect: Value) -> Value {
        json!({ "bank_num": bank_num, "effect_num": effect_num, "effect": effect })
    }

    /// Add a bank. `bank` is a serialized bank object.
    pub fn add_bank(bank_num: i64, bank: Value) -> Value {
        json!({ "bank_num": bank_num, "bank": bank })
    }

    /// Bind a physical control to an effect parameter, with its own range.
    ///
    /// An absent `min` or `max` is left out rather than sent as `null`, so the
    /// instrument keeps whatever range it already had.
    pub fn set_controller(
        bank_num: i64,
        effect_num: i64,
        parameter: &str,
        source: &str,
        min: Option<f32>,
        max: Option<f32>,
    ) -> Value {
        object(&[
            ("bank_num", Some(json!(bank_num))),
            ("effect_num", Some(json!(effect_num))),
            ("parameter", Some(json!(parameter))),
            ("source", Some(json!(source))),
            ("min", min.map(|v| json!(v))),
            ("max", max.map(|v| json!(v))),
        ])
    }

    /// Sustain-killer state for a bank.
    pub fn sustain_killer(bank_num: i64, killed: Option<bool>, reset: Option<bool>) -> Value {
        object(&[
            ("bank_num", Some(json!(bank_num))),
            ("killed", killed.map(|v| json!(v))),
            ("reset", reset.map(|v| json!(v))),
        ])
    }

    /// Start recording. `free` selects free-running rather than bar-locked.
    pub fn start_recording(free: bool) -> Value {
        json!({ "free": free })
    }

    /// Metronome tempo, and optionally its meter and loop length.
    ///
    /// `num` and `den` are the time signature's numerator and denominator, and
    /// `bpm` counts `den` notes — so 7/8 at 200 is 200 eighth-notes a minute.
    /// `bars` is the loop length. See [`crate::loopfile`], where the same
    /// fields appear in every recording's header and account for its duration.
    ///
    /// Absent values are omitted rather than sent as `null`, so the instrument
    /// keeps whatever it had for them.
    ///
    /// **Two hardware facts govern `den`** (mapped exhaustively 2026-08-28):
    ///
    /// 1. **Field order matters.** The firmware parser drops `den` when it
    ///    arrives before `num`, which is exactly what serde_json's default
    ///    BTreeMap produced by alphabetizing keys. This crate enables
    ///    `preserve_order` so params serialize in declaration order
    ///    (`bpm, num, den, bars`), and a test pins that.
    /// 2. **The firmware whitelists `{1, 2, 4, 16}`.** Every other value
    ///    1–32, plus 256, is silently dropped with a `true` reply — including
    ///    8 and 32, which the instrument's own panel offers. A table with a
    ///    hole in it, invisible to the vendor because their app discards every
    ///    result.
    ///
    /// `bpm` and `num` write normally, each field applies independently, and
    /// `ReadMetronome` always reports the true state.
    pub fn metronome(bpm: i64, num: Option<i64>, den: Option<i64>, bars: Option<i64>) -> Value {
        object(&[
            ("bpm", Some(json!(bpm))),
            ("num", num.map(|v| json!(v))),
            ("den", den.map(|v| json!(v))),
            ("bars", bars.map(|v| json!(v))),
        ])
    }

    /// Set the instrument's clock.
    pub fn date(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> Value {
        json!({
            "year": year, "month": month, "day": day,
            "hour": hour, "minute": minute, "second": second,
        })
    }

    /// Metadata for a stored file, by name.
    pub fn file_info(name: &str) -> Value {
        json!({ "name": name })
    }
}

/// Allocate request ids.
///
/// The device correlates responses by id, and LLT reuses the same value as its
/// object id, so ids must not repeat within a session. Starts at 1: the
/// vendor's client treats 0 as unset.
#[derive(Debug, Clone)]
pub struct RequestIds {
    next: i64,
}

impl Default for RequestIds {
    fn default() -> Self {
        RequestIds { next: 1 }
    }
}

impl RequestIds {
    /// A fresh sequence starting at 1.
    pub fn new() -> Self {
        Self::default()
    }

    /// The next id, wrapping before it could ever go negative.
    pub fn next_id(&mut self) -> i64 {
        let id = self.next;
        self.next = if self.next == i64::MAX {
            1
        } else {
            self.next + 1
        };
        id
    }
}

/// Collect every wire name, for tooling that needs the vocabulary.
pub fn all_method_names() -> Vec<&'static str> {
    Method::ALL.iter().map(|m| m.wire_name()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn the_protocol_version_is_a_number_not_a_string() {
        let req = Request::no_params(1, Method::GetStatus);
        let encoded = req.encode().unwrap();
        assert!(
            encoded.contains("\"jsonrpc\":2.0"),
            "must be numeric 2.0, not the spec's string: {encoded}"
        );
        assert!(
            !encoded.contains("\"jsonrpc\":\"2.0\""),
            "a string here would be spec-correct and device-wrong: {encoded}"
        );
    }

    #[test]
    fn a_minimal_request_has_the_expected_shape() {
        let encoded = Request::no_params(7, Method::GetStatus).encode().unwrap();
        let v: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(v["id"], 7);
        assert_eq!(v["method"], "GetStatus");
        assert!(v["params"].is_object());
    }

    #[test]
    fn every_method_has_a_distinct_wire_name() {
        let names = all_method_names();
        assert_eq!(names.len(), 32);
        assert!(!names.iter().any(|n| n.is_empty()), "a name failed to map");
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 32, "wire names must be unique");
    }

    /// The wire spellings are irregular — `SetEQGain` is not `SetEqGain` — so
    /// the ones that differ from a mechanical transform are pinned.
    #[test]
    fn the_irregular_wire_names_are_exact() {
        assert_eq!(Method::SetEqGain.wire_name(), "SetEQGain");
        assert_eq!(Method::SetEqBandGain.wire_name(), "SetEQBandGain");
        assert_eq!(Method::GetStatus.wire_name(), "GetStatus");
        assert_eq!(Method::AuxInDryWet.wire_name(), "AuxInDryWet");
    }

    #[test]
    fn methods_round_trip_through_their_wire_names() {
        for &method in Method::ALL {
            let encoded = serde_json::to_string(&method).unwrap();
            let decoded: Method = serde_json::from_str(&encoded).unwrap();
            assert_eq!(method, decoded);
            assert_eq!(encoded, format!("\"{}\"", method.wire_name()));
        }
    }

    #[test]
    fn a_success_response_decodes() {
        let r = Response::decode(r#"{"jsonrpc":2.0,"id":3,"result":{"batt_left":0.87}}"#).unwrap();
        assert!(r.answers(3));
        assert_eq!(r.into_result().unwrap()["batt_left"], 0.87);
    }

    #[test]
    fn an_error_response_becomes_an_err() {
        let r =
            Response::decode(r#"{"jsonrpc":2.0,"id":4,"error":{"code":-32601,"message":"no"}}"#)
                .unwrap();
        match r.into_result() {
            Err(RpcError::Device(e)) => {
                assert_eq!(e.message, "no");
                assert_eq!(e.code, -32601.0);
            }
            other => panic!("expected a device error, got {other:?}"),
        }
    }

    /// The device sends ids as JSON numbers, and the vendor's client reads them
    /// as floats, so `3` and `3.0` are the same id.
    #[test]
    fn ids_sent_as_floats_still_match() {
        let r = Response::decode(r#"{"jsonrpc":2.0,"id":3.0,"result":null}"#).unwrap();
        assert!(r.answers(3));
        assert_eq!(r.id_as_i64(), Some(3));
    }

    #[test]
    fn a_fractional_id_answers_nothing() {
        let r = Response::decode(r#"{"jsonrpc":2.0,"id":3.5,"result":null}"#).unwrap();
        assert_eq!(r.id_as_i64(), None);
        assert!(!r.answers(3));
        assert!(!r.answers(4));
    }

    /// A byte-exact reply captured from a real guitar (H2-CC340, STM 1.2.3 /
    /// ESP 1.3.0) on 2026-08-27. Every other test in this file asserts against
    /// the recovered spec; this one asserts against the device.
    const CAPTURED_GETSTATUS_REPLY: &str = concat!(
        r#"{"jsonrpc":"2.0","id":90,"result":{"free_gb":7.634,"free_pct":0.9949,"#,
        r#""batt_left":46,"version_stm":"V1.2.3","version_esp":"V1.3.0","#,
        r#""cpu_id":"PIdXXddxLAU=","device":"H2S"}}"#,
        "\n"
    );

    /// The regression that cost a debugging session: the device takes `jsonrpc`
    /// as a number but answers with it as a string. Parsing the reply strictly
    /// discards every valid response, and the symptom is indistinguishable from
    /// the device never answering.
    #[test]
    fn the_captured_reply_parses() {
        let r = Response::decode(CAPTURED_GETSTATUS_REPLY)
            .expect("a real reply from a real device must parse");
        assert!(r.answers(90));
        assert_eq!(r.version, 2.0, "the string \"2.0\" must read as 2.0");

        let s: Status = r.result_as().unwrap();
        assert_eq!(s.device, "H2S");
        assert_eq!(s.cpu_id, "PIdXXddxLAU=");
        assert_eq!(s.version_stm, "V1.2.3");
        assert_eq!(s.version_esp, "V1.3.0");
        assert_eq!(s.battery_percent, 46.0);
        assert!((s.free_space_gb - 7.634).abs() < 1e-4);
        assert!((s.free_space_fraction - 0.9949).abs() < 1e-6);
    }

    /// The version strings the device sends carry a `V`, and the handshake
    /// parser has to cope with the same spelling.
    #[test]
    fn the_captured_versions_carry_a_v_prefix() {
        let r = Response::decode(CAPTURED_GETSTATUS_REPLY).unwrap();
        let s: Status = r.result_as().unwrap();
        assert!(s.version_stm.starts_with('V'));
        assert_eq!(
            crate::handshake::Version::parse(&s.version_stm),
            crate::handshake::Version::parse("1.2.3"),
            "a V-prefixed version must read the same as a bare one"
        );
    }

    #[test]
    fn a_numeric_jsonrpc_in_a_reply_also_parses() {
        // Accepting both directions costs nothing and guards against firmware
        // that answers the way it is asked.
        let r = Response::decode(r#"{"jsonrpc":2.0,"id":5,"result":null}"#).unwrap();
        assert_eq!(r.version, 2.0);
        assert!(r.answers(5));
    }

    #[test]
    fn a_status_result_decodes_into_its_type() {
        let r = Response::decode(
            r#"{"jsonrpc":2.0,"id":1,"result":{"device":"HyVibe","cpu_id":"DEADBEEF",
                "batt_left":0.42,"free_gb":3.5,"free_pct":58.0,
                "version_esp":"2.7.0","version_stm":"1.2.3"}}"#,
        )
        .unwrap();
        let s: Status = r.result_as().unwrap();
        assert_eq!(s.device, "HyVibe");
        assert_eq!(s.cpu_id, "DEADBEEF");
        assert_eq!(s.version_stm, "1.2.3");
        assert_eq!(s.version_esp, "2.7.0");
        assert!((s.battery_percent - 0.42).abs() < 1e-6);
    }

    /// A device that omits fields must not fail the whole decode.
    #[test]
    fn a_sparse_status_still_decodes() {
        let r = Response::decode(r#"{"jsonrpc":2.0,"id":1,"result":{"device":"HyVibe"}}"#).unwrap();
        let s: Status = r.result_as().unwrap();
        assert_eq!(s.device, "HyVibe");
        assert_eq!(s.cpu_id, "");
        assert_eq!(s.battery_percent, 0.0);
    }

    #[test]
    fn a_result_absent_response_is_null_not_an_error() {
        let r = Response::decode(r#"{"jsonrpc":2.0,"id":9}"#).unwrap();
        assert!(r.answers(9));
        assert_eq!(r.into_result().unwrap(), Value::Null);
    }

    #[test]
    fn params_use_the_wire_vocabulary() {
        assert_eq!(params::bank(3)["bank_num"], 3);
        assert_eq!(params::move_bank(1, 4)["src"], 1);
        assert_eq!(params::move_bank(1, 4)["dst"], 4);
        let me = params::move_effect(2, 0, 3);
        assert_eq!(me["bank_num"], 2);
        assert_eq!(me["effect_num"], 0);
        assert_eq!(me["effect_dest"], 3);
        assert_eq!(params::bank_name(1, "Clean")["name"], "Clean");
        assert_eq!(params::eq_band_gain(2, -3.5)["band"], 2);
        assert_eq!(params::toggle(true)["toggle"], true);
    }

    #[test]
    fn request_ids_are_unique_and_start_at_one() {
        let mut ids = RequestIds::new();
        assert_eq!(ids.next_id(), 1);
        assert_eq!(ids.next_id(), 2);
        assert_eq!(ids.next_id(), 3);
    }

    #[test]
    fn request_ids_wrap_rather_than_overflow() {
        let mut ids = RequestIds { next: i64::MAX };
        assert_eq!(ids.next_id(), i64::MAX);
        assert_eq!(
            ids.next_id(),
            1,
            "must wrap to a valid id, never to 0 or negative"
        );
    }

    /// A request large enough to need chunking must survive the round trip
    /// through the transport layer beneath it.
    #[test]
    fn a_large_request_survives_llt_framing() {
        use crate::llt;

        let big = Value::Array((0..500).map(|i| serde_json::json!({ "k": i })).collect());
        let req = Request::new(11, Method::SetConfig, big);
        let encoded = req.encode().unwrap();
        assert!(encoded.len() > 514);

        let out = llt::frame_message(&encoded, req.id, 514).unwrap();
        assert!(out.is_chunked());

        let mut rebuilt = String::new();
        for frame in out.frames() {
            let v: Value = serde_json::from_str(frame.trim_end()).unwrap();
            assert_eq!(
                v["oid"].as_i64().unwrap(),
                11,
                "object id must mirror the request id"
            );
            rebuilt.push_str(v["d"].as_str().unwrap());
        }
        assert_eq!(rebuilt, encoded);
    }

    /// Every method's params shape is declared, and the declarations are not
    /// all the same. A table that had quietly collapsed to "everything takes
    /// nothing" would still satisfy an exhaustive match, so check that it
    /// carries real distinctions.
    #[test]
    fn the_shape_table_covers_every_method_with_variety() {
        let mut nones = 0;
        let mut objects = 0;
        let mut unrecovered = 0;
        for &m in Method::ALL {
            match param_shape(m) {
                ParamShape::None => nones += 1,
                ParamShape::Object(keys) => {
                    assert!(!keys.is_empty(), "{m:?} declares an object with no keys");
                    objects += 1;
                }
                ParamShape::Unrecovered => unrecovered += 1,
            }
        }
        assert_eq!(nones + objects + unrecovered, Method::ALL.len());
        assert!(nones > 0 && objects > 0, "the table lost its distinctions");
        // SetConfig is the one method whose shape was never recovered, because
        // ReadConfig — the call whose reply would show it — wedges the
        // firmware. If this becomes 0 the shape was found; if it grows,
        // something was demoted and the Findings should say why.
        assert_eq!(unrecovered, 1, "exactly SetConfig should be unrecovered");
        assert_eq!(param_shape(Method::SetConfig), ParamShape::Unrecovered);
    }

    /// No key is declared twice for one method, which a hand-written table
    /// makes easy to do and hard to see.
    #[test]
    fn no_method_declares_a_duplicate_key() {
        for &m in Method::ALL {
            if let ParamShape::Object(keys) = param_shape(m) {
                for (i, a) in keys.iter().enumerate() {
                    for b in &keys[i + 1..] {
                        assert_ne!(a.name, b.name, "{m:?} declares {} twice", a.name);
                    }
                }
            }
        }
    }

    /// The constructors in [`params`] must produce what the table says.
    ///
    /// This is what makes the table load-bearing rather than decorative: a
    /// constructor and its declaration live in two places and drift silently
    /// otherwise. Every key a constructor emits must be declared, and every
    /// required key must be emitted.
    #[test]
    fn every_constructor_matches_its_declared_shape() {
        let cases: alloc::vec::Vec<(Method, Value)> = alloc::vec![
            (Method::GetStatus, params::none()),
            (Method::GetVersion, params::none()),
            (Method::Calibrate, params::none()),
            (Method::ReadConfig, params::none()),
            (Method::SaveConfig, params::none()),
            (Method::StopMetronome, params::none()),
            (Method::StopRecording, params::none()),
            (Method::ReadBank, params::bank(0)),
            (Method::SwitchBank, params::bank(3)),
            (Method::RemoveBank, params::bank(2)),
            (Method::AddBank, params::add_bank(1, serde_json::json!({}))),
            (Method::MoveBank, params::move_bank(0, 1)),
            (Method::SetBankName, params::bank_name(0, "Clean")),
            (Method::SetGainBank, params::bank_gain(0, 0.5)),
            (
                Method::AddEffect,
                params::add_effect(0, serde_json::json!({}))
            ),
            (
                Method::UpdateEffect,
                params::update_effect(0, 1, serde_json::json!({})),
            ),
            (Method::RemoveEffect, params::remove_effect(0, 1)),
            (Method::MoveEffect, params::move_effect(0, 1, 2)),
            (
                Method::SetController,
                params::set_controller(0, 1, "DryWet", "knob", Some(0.0), Some(1.0)),
            ),
            (
                Method::SetController,
                params::set_controller(0, 1, "DryWet", "knob", None, None),
            ),
            (Method::SetEqGain, params::eq_gain(0.0)),
            (Method::SetEqBandGain, params::eq_band_gain(2, -3.0)),
            (Method::AuxIn, params::toggle(true)),
            (Method::AuxOut, params::toggle(false)),
            (Method::AuxInDryWet, params::value(0.5)),
            (Method::AuxOutDryWet, params::value(0.5)),
            (
                Method::StartMetronome,
                params::metronome(120, Some(4), None, None),
            ),
            (
                Method::UpdateMetronome,
                params::metronome(96, None, None, None),
            ),
            (Method::StartRecording, params::start_recording(true)),
            (
                Method::SustainKiller,
                params::sustain_killer(0, Some(true), None),
            ),
            (Method::SustainKiller, params::sustain_killer(0, None, None)),
            (
                Method::GetFileInfo,
                params::file_info("/Loops/loop0001.wav")
            ),
            (Method::SetDate, params::date(2026, 8, 28, 12, 0, 0)),
        ];

        for (method, value) in &cases {
            let object = value
                .as_object()
                .unwrap_or_else(|| panic!("{method:?} params are not an object: {value}"));
            match param_shape(*method) {
                ParamShape::None => assert!(
                    object.is_empty(),
                    "{method:?} takes nothing but was given {value}"
                ),
                ParamShape::Object(keys) => {
                    for emitted in object.keys() {
                        assert!(
                            keys.iter().any(|k| k.name == emitted),
                            "{method:?} emitted undeclared key {emitted}"
                        );
                    }
                    for key in keys.iter().filter(|k| k.required) {
                        assert!(
                            object.contains_key(key.name),
                            "{method:?} omitted required key {}",
                            key.name
                        );
                    }
                }
                ParamShape::Unrecovered => {}
            }
        }
    }

    /// An absent optional is left out, not sent as `null`. Omission means
    /// "leave it alone"; `null` is a value, and the two reach the firmware as
    /// different requests.
    #[test]
    fn absent_optionals_are_omitted_rather_than_nulled() {
        let m = params::metronome(120, None, None, None);
        assert_eq!(m, serde_json::json!({ "bpm": 120 }));
        assert!(!m.as_object().unwrap().contains_key("den"));

        let sk = params::sustain_killer(4, None, None);
        assert_eq!(sk, serde_json::json!({ "bank_num": 4 }));

        let c = params::set_controller(0, 1, "DryWet", "knob", None, None);
        assert!(!c.as_object().unwrap().contains_key("min"));

        // Present ones still travel.
        let full = params::metronome(96, Some(5), Some(8), Some(2));
        assert_eq!(full.as_object().unwrap().len(), 4);
    }

    /// Params must serialize in declaration order, because the firmware's
    /// parser is order-sensitive: a `den` that arrives before `num` is
    /// silently dropped (hardware, 2026-08-28). serde_json alphabetizes by
    /// default — that exact behaviour hid the metronome denominator for a day
    /// — so `preserve_order` is enabled in Cargo.toml and this test fails if
    /// anyone "tidies" the feature away.
    #[test]
    fn metronome_params_keep_wire_order_not_alphabetical() {
        let p = params::metronome(93, Some(6), Some(8), None);
        let text = serde_json::to_string(&p).unwrap();
        let pos = |k: &str| {
            text.find(k)
                .unwrap_or_else(|| panic!("{k} missing in {text}"))
        };
        assert!(
            pos("bpm") < pos("num") && pos("num") < pos("den"),
            "alphabetized params regressed: {text}"
        );
    }

    /// The vocabulary table covers every real effect type exactly once, with
    /// no key listed twice.
    #[test]
    fn the_parameter_table_covers_every_effect_that_takes_parameters() {
        for (kind, keys) in PARAMETER_KEYS {
            assert!(EFFECT_TYPES.contains(kind), "{kind} is not an effect type");
            assert!(!keys.is_empty());
            for (i, a) in keys.iter().enumerate() {
                assert!(!keys[i + 1..].contains(a), "{kind} lists {a} twice");
            }
        }
        let with_keys: alloc::vec::Vec<&str> = PARAMETER_KEYS.iter().map(|(k, _)| *k).collect();
        for kind in EFFECT_TYPES {
            assert!(with_keys.contains(&kind), "{kind} has no parameter entry");
        }
        assert_eq!(with_keys.len(), EFFECT_TYPES.len());
    }

    /// The effect object must serialise in wire order, because the firmware
    /// drops fields that arrive out of sequence. This is the exact four-knob
    /// Distortion the instrument accepted on 2026-09-01 (H29).
    #[test]
    fn a_typed_effect_serialises_in_wire_order() {
        let e = Effect::new("Distortion")
            .with("Gain", 50.0)
            .with("Volume", -25.0)
            .with("Lowpass", 1800.0)
            .with("Highpass", 94.0)
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
        let v = Effect::new("Tremolo").to_value();
        assert_eq!(v["params"], serde_json::json!([]));
        assert_eq!(v["preset"], "default");
    }

    /// The two reorder methods use different key names for the same idea. It
    /// reads like a bug and is not; pinning it stops a future tidy-up.
    #[test]
    fn bank_and_effect_reordering_keep_their_different_keys() {
        assert_eq!(
            param_shape(Method::MoveBank),
            ParamShape::Object(&[
                ParamKey {
                    name: "src",
                    required: true
                },
                ParamKey {
                    name: "dst",
                    required: true
                },
            ])
        );
        let effect = params::move_effect(0, 1, 2);
        assert!(effect.get("effect_dest").is_some());
        assert!(effect.get("dst").is_none());
    }
}
