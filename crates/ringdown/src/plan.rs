//! Request planners: one pure function per write the hardware sessions
//! verified, producing a [`Call`] that a driver sends.
//!
//! A planner knows the method, the wire keys and their order, and every
//! reason the firmware is already known to refuse or silently drop a message.
//! It knows nothing about I/O, ids or replies, so each one is pinned by a
//! test that asserts the exact bytes, with no instrument in the loop.
//!
//! # What a reply proves
//!
//! Nothing here can be verified from software: `true` means parsed, not
//! applied (H27), and no method reads a bank back (H25). Each planner's doc
//! therefore names the **receipt** that does verify it — the panel, the
//! owner's ears, `ReadMetronome`, or the remove-until-`false` count (H31) —
//! and a driver's typed method carries that forward as *sent*, never *set*.
//! See `design_docs/2026-09-02_client_surface_plan.md`.

use serde_json::Value;

use alloc::string::String;

use crate::effects::{BankSpec, Effect, ParamError};
use crate::rpc::{Method, Request, RpcError, params};

/// A method and its arguments, ready to be given an id and sent.
///
/// The step between a planner and a [`Request`]: the caller owns the id
/// sequence, so a planner does not choose one.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// The method.
    pub method: Method,
    /// Its arguments, keys in wire order.
    pub params: Value,
}

impl Call {
    /// The request this becomes under `id`.
    pub fn with_id(self, id: i64) -> Request {
        Request::new(id, self.method, self.params)
    }

    /// The complete JSON-RPC message under `id`.
    pub fn encode(&self, id: i64) -> Result<alloc::string::String, RpcError> {
        self.clone().with_id(id).encode()
    }
}

// -- Effect chain --------------------------------------------------------

/// Append `effect` to the chain in `slot`.
///
/// **Receipt: ears.** Audible on a bank the vendor app created, with the
/// full parameter vocabulary (H36, H37). Into an empty tile, or a bank made
/// by `AddBank`, the write is stored and never sounds (H34, H38), and the
/// reply is `true` either way. The chain is not capped at four; that limit
/// is the app's editor (H37).
pub fn add_effect(slot: i64, effect: &Effect) -> Call {
    Call {
        method: Method::AddEffect,
        params: params::add_effect(slot, effect.to_value()),
    }
}

/// Replace the effect at `index` in `slot` with `effect`.
///
/// **Receipt: ears.** Answers `true` on a real bank; that the replacement
/// renders has not been isolated by ear as `AddEffect` has, so treat this as
/// less established than the planners around it.
pub fn update_effect(slot: i64, index: i64, effect: &Effect) -> Call {
    Call {
        method: Method::UpdateEffect,
        params: params::update_effect(slot, index, effect.to_value()),
    }
}

/// Remove the effect at `index` from `slot`.
///
/// **Receipt: the reply itself.** This is the one write whose `true` means
/// *stored*: removing at index 0 until the reply is `false` counts a chain
/// exactly (34 for 34, H31), and any index is honoured (H37). Count before
/// indexing: a factory bank may hold more than it shows, and an assumed
/// length turns every index into a guess.
pub fn remove_effect(slot: i64, index: i64) -> Call {
    Call {
        method: Method::RemoveEffect,
        params: params::remove_effect(slot, index),
    }
}

/// Move the effect at `from` in `slot` to position `to`.
///
/// **Receipt: none from software.** Answers `true` on a real bank (H37);
/// order is audible only where the chain makes it so.
pub fn move_effect(slot: i64, from: i64, to: i64) -> Call {
    Call {
        method: Method::MoveEffect,
        params: params::move_effect(slot, from, to),
    }
}

// -- Banks ---------------------------------------------------------------

/// Select `slot` on the panel.
///
/// **Receipt: panel.** Moves the grid selection, read off the screen for two
/// indices (H25). Do not send one while the owner is comparing by ear: it
/// moves the selection under them and voids the comparison (H33).
pub fn switch_bank(slot: i64) -> Call {
    Call {
        method: Method::SwitchBank,
        params: params::bank(slot),
    }
}

/// Rename the bank in `slot`.
///
/// **Receipt: panel.** Takes effect only on a slot that already holds an
/// effect; on an empty tile the reply is `true` and nothing changes (H33).
/// Add the first effect, then name the bank.
pub fn set_bank_name(slot: i64, name: &str) -> Call {
    Call {
        method: Method::SetBankName,
        params: params::bank_name(slot, name),
    }
}

/// Set the output gain of the bank in `slot`, in decibels.
///
/// **Receipt: none.** The app displays this as a signed whole number of dB
/// (H28); a write of a quarter-decibel is invisible there, which is what made
/// an earlier reading of this method wrong (H27 as corrected by H28). Whether
/// a whole-dB write shows on the app is untested.
pub fn set_gain_bank(slot: i64, gain_db: f32) -> Call {
    Call {
        method: Method::SetGainBank,
        params: params::bank_gain(slot, gain_db),
    }
}

/// Sustain-killer state for the bank in `slot`. Absent options are left
/// alone.
///
/// **Receipt: none.** Answers `true`; the app has a toggle and a RESET for
/// it (H28) and neither has been read back.
pub fn sustain_killer(slot: i64, killed: Option<bool>, reset: Option<bool>) -> Call {
    Call {
        method: Method::SustainKiller,
        params: params::sustain_killer(slot, killed, reset),
    }
}

/// Move the bank at `from` to `to`.
///
/// **Receipt: panel**, by inference from the model; unexercised.
pub fn move_bank(from: i64, to: i64) -> Call {
    Call {
        method: Method::MoveBank,
        params: params::move_bank(from, to),
    }
}

/// Remove the bank in `slot`.
///
/// **Receipt: panel.** Unexercised. By the model `AddBank` demonstrated (an
/// ordered list, H38) this shifts every later slot down one.
pub fn remove_bank(slot: i64) -> Call {
    Call {
        method: Method::RemoveBank,
        params: params::bank(slot),
    }
}

/// **Insert** `bank` at `slot`, shifting every later bank along one place.
///
/// **Receipt: panel, then ears.** The name appears on the tile at `slot`
/// and every later tile moves; on a full nine-tile profile the last is
/// pushed off (H38). The bank the H38 object made never rendered, and the
/// vendor app creates playable banks with this same method, so the object
/// the firmware wants is what the client surface plan's Phase D is finding.
/// [`BankSpec`] is the current hypothesis, marked as such.
///
/// A client holding slot indices across this call is holding stale ones.
pub fn add_bank(slot: i64, bank: &BankSpec) -> Call {
    Call {
        method: Method::AddBank,
        params: params::add_bank(slot, bank.to_value()),
    }
}

// -- Edits as data -------------------------------------------------------

/// A profile-changing write as a value, so that the wire and the shadow
/// profile consume the same thing.
///
/// [`Edit::call`] is the planner for it; [`crate::profile::Profile::apply`]
/// is what it does to the client's record. Selection (`SwitchBank`) and the
/// metronome are not here because they change no profile state.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// [`add_effect`].
    AddEffect {
        /// Grid slot.
        slot: i64,
        /// What to append.
        effect: Effect,
    },
    /// [`update_effect`].
    UpdateEffect {
        /// Grid slot.
        slot: i64,
        /// Position in the chain.
        index: i64,
        /// The replacement.
        effect: Effect,
    },
    /// [`remove_effect`].
    RemoveEffect {
        /// Grid slot.
        slot: i64,
        /// Position in the chain.
        index: i64,
    },
    /// [`move_effect`].
    MoveEffect {
        /// Grid slot.
        slot: i64,
        /// Current position.
        from: i64,
        /// Destination position.
        to: i64,
    },
    /// [`set_bank_name`].
    SetBankName {
        /// Grid slot.
        slot: i64,
        /// The new name.
        name: String,
    },
    /// [`set_gain_bank`].
    SetGainBank {
        /// Grid slot.
        slot: i64,
        /// Decibels.
        gain_db: f32,
    },
    /// [`sustain_killer`].
    SustainKiller {
        /// Grid slot.
        slot: i64,
        /// Engaged, or leave alone.
        killed: Option<bool>,
        /// Momentary reset, or leave alone.
        reset: Option<bool>,
    },
    /// [`move_bank`].
    MoveBank {
        /// Current slot.
        from: i64,
        /// Destination slot.
        to: i64,
    },
    /// [`remove_bank`].
    RemoveBank {
        /// Grid slot.
        slot: i64,
    },
    /// [`add_bank`].
    AddBank {
        /// Grid slot to insert at.
        slot: i64,
        /// The bank object.
        bank: BankSpec,
    },
}

impl Edit {
    /// The call this edit is sent as. Same bytes as the planner of the same
    /// name, pinned by test.
    pub fn call(&self) -> Call {
        match self {
            Edit::AddEffect { slot, effect } => add_effect(*slot, effect),
            Edit::UpdateEffect {
                slot,
                index,
                effect,
            } => update_effect(*slot, *index, effect),
            Edit::RemoveEffect { slot, index } => remove_effect(*slot, *index),
            Edit::MoveEffect { slot, from, to } => move_effect(*slot, *from, *to),
            Edit::SetBankName { slot, name } => set_bank_name(*slot, name),
            Edit::SetGainBank { slot, gain_db } => set_gain_bank(*slot, *gain_db),
            Edit::SustainKiller {
                slot,
                killed,
                reset,
            } => sustain_killer(*slot, *killed, *reset),
            Edit::MoveBank { from, to } => move_bank(*from, *to),
            Edit::RemoveBank { slot } => remove_bank(*slot),
            Edit::AddBank { slot, bank } => add_bank(*slot, bank),
        }
    }
}

// -- Metronome -----------------------------------------------------------

/// Start the metronome at `bpm`, optionally setting meter and loop length.
///
/// **Receipt: `ReadMetronome`**, which reports the true state, and the
/// owner's ears. `den` outside `{1, 2, 4, 16}` is refused here because the
/// firmware would answer `true` and drop it (H24).
pub fn start_metronome(
    bpm: i64,
    num: Option<i64>,
    den: Option<i64>,
    bars: Option<i64>,
) -> Result<Call, ParamError> {
    Ok(Call {
        method: Method::StartMetronome,
        params: params::metronome(bpm, num, den, bars)?,
    })
}

/// Change tempo, meter or loop length while the metronome runs.
///
/// **Receipt: `ReadMetronome`.** Fields apply independently: a refused
/// `den` would not stop `bpm` from applying, which is why the refusal is
/// here and not on the wire (H24).
pub fn update_metronome(
    bpm: i64,
    num: Option<i64>,
    den: Option<i64>,
    bars: Option<i64>,
) -> Result<Call, ParamError> {
    Ok(Call {
        method: Method::UpdateMetronome,
        params: params::metronome(bpm, num, den, bars)?,
    })
}

/// Stop the metronome.
///
/// **Receipt: ears.**
pub fn stop_metronome() -> Call {
    Call {
        method: Method::StopMetronome,
        params: params::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectKind;
    use alloc::string::String;
    use alloc::vec::Vec;

    fn wire(call: &Call) -> String {
        serde_json::to_string(&call.params).unwrap()
    }

    fn octave() -> Effect {
        Effect::new(EffectKind::Pitch).with("Shift", -12.0).unwrap()
    }

    /// The exact message the octave-down oracle went out as (H36), envelope
    /// and all.
    #[test]
    fn add_effect_encodes_the_octave_oracle_exactly() {
        let call = add_effect(4, &octave());
        assert_eq!(call.method, Method::AddEffect);
        assert_eq!(
            call.encode(9).unwrap(),
            r#"{"jsonrpc":2.0,"id":9,"method":"AddEffect","params":{"bank_num":4,"effect":{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}}}"#
        );
    }

    #[test]
    fn effect_chain_planners_pin_their_wire_bytes() {
        assert_eq!(
            wire(&update_effect(4, 1, &octave())),
            r#"{"bank_num":4,"effect_num":1,"effect":{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}}"#
        );
        assert_eq!(
            wire(&remove_effect(4, 0)),
            r#"{"bank_num":4,"effect_num":0}"#
        );
        assert_eq!(
            wire(&move_effect(4, 0, 1)),
            r#"{"bank_num":4,"effect_num":0,"effect_dest":1}"#
        );
    }

    #[test]
    fn bank_planners_pin_their_wire_bytes() {
        assert_eq!(wire(&switch_bank(3)), r#"{"bank_num":3}"#);
        assert_eq!(
            wire(&set_bank_name(8, "ringdown")),
            r#"{"bank_num":8,"name":"ringdown"}"#
        );
        assert_eq!(
            wire(&set_gain_bank(0, -5.0)),
            r#"{"bank_num":0,"gain":-5.0}"#
        );
        assert_eq!(
            wire(&sustain_killer(8, Some(false), Some(false))),
            r#"{"bank_num":8,"killed":false,"reset":false}"#
        );
        assert_eq!(wire(&sustain_killer(8, None, None)), r#"{"bank_num":8}"#);
        assert_eq!(wire(&move_bank(0, 1)), r#"{"src":0,"dst":1}"#);
        assert_eq!(wire(&remove_bank(4)), r#"{"bank_num":4}"#);
        assert_eq!(
            wire(&add_bank(4, &BankSpec::new("octave").with_effect(octave()))),
            r#"{"bank_num":4,"bank":{"name":"octave","effects":[{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}]}}"#
        );
    }

    #[test]
    fn metronome_planners_pin_their_wire_bytes_and_refuse_bad_den() {
        assert_eq!(
            wire(&start_metronome(200, Some(7), Some(4), Some(4)).unwrap()),
            r#"{"bpm":200,"num":7,"den":4,"bars":4}"#
        );
        assert_eq!(
            wire(&update_metronome(96, None, None, None).unwrap()),
            r#"{"bpm":96}"#
        );
        assert_eq!(wire(&stop_metronome()), r#"{}"#);
        assert_eq!(
            update_metronome(96, Some(6), Some(8), None).unwrap_err(),
            ParamError::DenNotAccepted(8)
        );
    }

    /// An edit as data plans the same call as the function of the same name,
    /// for every variant, so the shadow and the wire cannot diverge.
    #[test]
    fn every_edit_plans_the_same_call_as_its_planner() {
        let bank = BankSpec::new("b");
        let pairs: Vec<(Edit, Call)> = alloc::vec![
            (
                Edit::AddEffect {
                    slot: 4,
                    effect: octave()
                },
                add_effect(4, &octave())
            ),
            (
                Edit::UpdateEffect {
                    slot: 4,
                    index: 1,
                    effect: octave()
                },
                update_effect(4, 1, &octave())
            ),
            (
                Edit::RemoveEffect { slot: 4, index: 0 },
                remove_effect(4, 0)
            ),
            (
                Edit::MoveEffect {
                    slot: 4,
                    from: 0,
                    to: 1
                },
                move_effect(4, 0, 1)
            ),
            (
                Edit::SetBankName {
                    slot: 8,
                    name: "ringdown".into()
                },
                set_bank_name(8, "ringdown")
            ),
            (
                Edit::SetGainBank {
                    slot: 0,
                    gain_db: -5.0
                },
                set_gain_bank(0, -5.0)
            ),
            (
                Edit::SustainKiller {
                    slot: 8,
                    killed: Some(false),
                    reset: None
                },
                sustain_killer(8, Some(false), None)
            ),
            (Edit::MoveBank { from: 0, to: 1 }, move_bank(0, 1)),
            (Edit::RemoveBank { slot: 4 }, remove_bank(4)),
            (
                Edit::AddBank {
                    slot: 4,
                    bank: bank.clone()
                },
                add_bank(4, &bank)
            ),
        ];
        for (edit, call) in &pairs {
            assert_eq!(&edit.call(), call, "{edit:?}");
        }
    }

    /// Every planner's output satisfies the declared params shape for its
    /// method: no undeclared key, every required key present.
    #[test]
    fn every_planner_matches_its_declared_shape() {
        let bank = BankSpec::new("b");
        let calls: Vec<Call> = alloc::vec![
            add_effect(0, &octave()),
            update_effect(0, 0, &octave()),
            remove_effect(0, 0),
            move_effect(0, 0, 1),
            switch_bank(0),
            set_bank_name(0, "n"),
            set_gain_bank(0, 0.0),
            sustain_killer(0, Some(true), None),
            move_bank(0, 1),
            remove_bank(0),
            add_bank(0, &bank),
            start_metronome(120, Some(4), Some(4), None).unwrap(),
            update_metronome(120, None, None, None).unwrap(),
            stop_metronome(),
        ];
        for call in &calls {
            crate::rpc::assert_matches_shape(call.method, &call.params);
        }
    }
}
