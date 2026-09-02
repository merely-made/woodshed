//! The shadow profile: what a client believes the nine grid slots hold.
//!
//! This protocol has no read-back. `ReadBank` answers `""` for every slot
//! (H25), the only count empties the chain to take it (H31), and the vendor
//! app overwrites the whole profile whenever it connects (H32). So a client's
//! own record of what it sent is the only record there is, and this is that
//! record: pure state, no I/O, serialisable so it outlives the process and the
//! app's wipes.
//!
//! It is a **belief**, and built to be honest about its limits:
//!
//! - A slot the client did not build up itself is known only as far as the
//!   client seeded it. A factory bank's own chain is invisible over this
//!   protocol, so its shadow holds the name and whatever the client added,
//!   and nothing says the chain is complete. A driver that drains a slot to
//!   count it will find more than the shadow knows, and what it found is gone
//!   until the app next connects.
//! - The shadow is updated by [`Profile::apply`] only after a reply parsed
//!   (see the client's `edit`), and never on a refusal.
//! - An empty slot is refused as a target for chain and name writes, because
//!   the firmware stores them and never plays them (H33, H34): a shadow that
//!   recorded such a write would be a record of a phantom.
//!
//! [`Edit`] is the vocabulary: the same value the wire is planned from is
//! what the shadow applies, so the two cannot describe different writes.

use alloc::{vec, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::effects::BankSpec;
use crate::plan::Edit;

/// The number of grid slots in a profile (H25).
pub const SLOTS: usize = 9;

/// Why an edit does not fit the profile as the shadow knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// A slot outside `0..9`.
    SlotOutOfRange(i64),
    /// A chain or name write aimed at a slot the shadow holds nothing for.
    /// The firmware would answer `true` and store it where nothing plays
    /// (H33, H34).
    EmptySlot(i64),
    /// An effect index beyond the shadow's chain.
    IndexOutOfRange {
        /// The slot addressed.
        slot: i64,
        /// The index asked for.
        index: i64,
        /// How long the shadow believes the chain is.
        len: usize,
    },
    /// A serialised profile with the wrong number of slots.
    WrongSlotCount(usize),
}

impl core::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProfileError::SlotOutOfRange(s) => write!(f, "slot {s} is outside 0..{SLOTS}"),
            ProfileError::EmptySlot(s) => write!(
                f,
                "slot {s} holds no bank the shadow knows of; the firmware would store the \
                 write and never play it (H33, H34)"
            ),
            ProfileError::IndexOutOfRange { slot, index, len } => write!(
                f,
                "effect {index} in slot {slot} is beyond the {len} the shadow believes are there"
            ),
            ProfileError::WrongSlotCount(n) => {
                write!(f, "a profile has {SLOTS} slots, this one has {n}")
            }
        }
    }
}

impl core::error::Error for ProfileError {}

/// The nine slots, as the client believes them.
///
/// Serialises as a bare nine-element array, `null` for an empty slot, using
/// [`BankSpec`]'s own field names rather than the wire's; the wire shape is
/// [`BankSpec::to_value`] and is provisional, and a saved profile should not
/// change when the research settles it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Option<BankSpec>>", into = "Vec<Option<BankSpec>>")]
pub struct Profile {
    slots: Vec<Option<BankSpec>>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile::empty()
    }
}

impl TryFrom<Vec<Option<BankSpec>>> for Profile {
    type Error = ProfileError;

    fn try_from(slots: Vec<Option<BankSpec>>) -> Result<Profile, ProfileError> {
        if slots.len() != SLOTS {
            return Err(ProfileError::WrongSlotCount(slots.len()));
        }
        Ok(Profile { slots })
    }
}

impl From<Profile> for Vec<Option<BankSpec>> {
    fn from(profile: Profile) -> Self {
        profile.slots
    }
}

impl Profile {
    /// Nine slots, none known.
    pub fn empty() -> Profile {
        Profile {
            slots: vec![None; SLOTS],
        }
    }

    /// Every slot, in grid order.
    pub fn slots(&self) -> &[Option<BankSpec>] {
        &self.slots
    }

    /// What the shadow holds for `slot`; `None` for an empty or out-of-range
    /// slot alike, since neither can be written to.
    pub fn slot(&self, slot: i64) -> Option<&BankSpec> {
        self.index(slot).ok().and_then(|i| self.slots[i].as_ref())
    }

    /// Seed or overwrite what the shadow believes about `slot`.
    ///
    /// This is how a client tells the shadow that a factory bank exists at an
    /// index — `Some(BankSpec::new("Tremolo"))`, chain unknown — so that
    /// writes into it are allowed. It is also the only way to record what a
    /// player's ears or panel established that no reply could.
    pub fn set_slot(&mut self, slot: i64, bank: Option<BankSpec>) -> Result<(), ProfileError> {
        let i = self.index(slot)?;
        self.slots[i] = bank;
        Ok(())
    }

    /// Whether `edit` fits the profile as the shadow knows it, without
    /// applying it. The check a driver runs before sending.
    pub fn check(&self, edit: &Edit) -> Result<(), ProfileError> {
        self.clone().apply(edit).map(|_| ())
    }

    /// Apply `edit` to the shadow.
    ///
    /// Returns the bank that left the profile, if one did: pushed off the end
    /// by an `AddBank` on a full profile (H38), or taken out by `RemoveBank`.
    /// Nothing is changed on an error.
    ///
    /// The bank-level moves follow the model `AddBank` demonstrated: the
    /// profile is an ordered list and insertion shifts (H38). `RemoveBank` and
    /// `MoveBank` are modelled the same way and are unexercised on hardware.
    /// `MoveEffect` is modelled as remove-then-insert at the destination
    /// index; the firmware honours the call (H37) but the exact placement
    /// rule is not established by ear.
    pub fn apply(&mut self, edit: &Edit) -> Result<Option<BankSpec>, ProfileError> {
        match edit {
            Edit::AddEffect { slot, effect } => {
                self.bank_mut(*slot)?.chain.push(effect.clone());
                Ok(None)
            }
            Edit::UpdateEffect {
                slot,
                index,
                effect,
            } => {
                let bank = self.bank_mut(*slot)?;
                let i = chain_index(*slot, *index, bank.chain.len())?;
                bank.chain[i] = effect.clone();
                Ok(None)
            }
            Edit::RemoveEffect { slot, index } => {
                let bank = self.bank_mut(*slot)?;
                let i = chain_index(*slot, *index, bank.chain.len())?;
                bank.chain.remove(i);
                Ok(None)
            }
            Edit::MoveEffect { slot, from, to } => {
                let bank = self.bank_mut(*slot)?;
                let len = bank.chain.len();
                let f = chain_index(*slot, *from, len)?;
                let t = chain_index(*slot, *to, len)?;
                let effect = bank.chain.remove(f);
                bank.chain.insert(t, effect);
                Ok(None)
            }
            Edit::SetBankName { slot, name } => {
                self.bank_mut(*slot)?.name = name.clone();
                Ok(None)
            }
            Edit::SetGainBank { slot, gain_db } => {
                self.bank_mut(*slot)?.gain_db = Some(*gain_db);
                Ok(None)
            }
            Edit::SustainKiller { slot, killed, .. } => {
                // `reset` is momentary and leaves no state to shadow.
                let bank = self.bank_mut(*slot)?;
                if let Some(killed) = killed {
                    bank.sustain_killed = Some(*killed);
                }
                Ok(None)
            }
            Edit::MoveBank { from, to } => {
                let f = self.index(*from)?;
                let t = self.index(*to)?;
                let bank = self.slots.remove(f);
                self.slots.insert(t, bank);
                Ok(None)
            }
            Edit::RemoveBank { slot } => {
                let i = self.index(*slot)?;
                let removed = self.slots.remove(i);
                self.slots.push(None);
                Ok(removed)
            }
            Edit::AddBank { slot, bank } => {
                let i = self.index(*slot)?;
                self.slots.insert(i, Some(bank.clone()));
                Ok(self.slots.pop().flatten())
            }
        }
    }

    fn index(&self, slot: i64) -> Result<usize, ProfileError> {
        usize::try_from(slot)
            .ok()
            .filter(|&i| i < SLOTS)
            .ok_or(ProfileError::SlotOutOfRange(slot))
    }

    fn bank_mut(&mut self, slot: i64) -> Result<&mut BankSpec, ProfileError> {
        let i = self.index(slot)?;
        self.slots[i].as_mut().ok_or(ProfileError::EmptySlot(slot))
    }
}

fn chain_index(slot: i64, index: i64, len: usize) -> Result<usize, ProfileError> {
    usize::try_from(index)
        .ok()
        .filter(|&i| i < len)
        .ok_or(ProfileError::IndexOutOfRange { slot, index, len })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{Effect, EffectKind};
    use alloc::string::String;

    fn octave() -> Effect {
        Effect::new(EffectKind::Pitch).with("Shift", -12.0).unwrap()
    }

    /// The nine-tile profile H38 was run against, by name where the finding
    /// recorded one.
    fn nine_tiles() -> Profile {
        let mut p = Profile::empty();
        for (i, name) in [
            "b0", "b1", "b2", "b3", "Tremolo", "Octaver", "Disto", "Boost", "ringdown",
        ]
        .iter()
        .enumerate()
        {
            p.set_slot(i as i64, Some(BankSpec::new(name))).unwrap();
        }
        p
    }

    fn names(p: &Profile) -> Vec<Option<String>> {
        p.slots()
            .iter()
            .map(|s| s.as_ref().map(|b| b.name.clone()))
            .collect()
    }

    /// Exactly what H38 observed: insert at 4, everything after shifts, the
    /// ninth falls off — and the shadow hands it back.
    #[test]
    fn add_bank_inserts_shifts_and_returns_the_bank_pushed_off() {
        let mut p = nine_tiles();
        let displaced = p
            .apply(&Edit::AddBank {
                slot: 4,
                bank: BankSpec::new("octave"),
            })
            .unwrap();
        assert_eq!(displaced.unwrap().name, "ringdown");
        assert_eq!(
            names(&p),
            [
                "b0", "b1", "b2", "b3", "octave", "Tremolo", "Octaver", "Disto", "Boost"
            ]
            .map(|s| Some(String::from(s)))
        );
        assert_eq!(p.slots().len(), SLOTS);
    }

    #[test]
    fn remove_bank_shifts_down_and_leaves_the_last_slot_empty() {
        let mut p = nine_tiles();
        let removed = p.apply(&Edit::RemoveBank { slot: 4 }).unwrap();
        assert_eq!(removed.unwrap().name, "Tremolo");
        assert_eq!(names(&p)[4].as_deref(), Some("Octaver"));
        assert_eq!(names(&p)[8], None);
        assert_eq!(p.slots().len(), SLOTS);
    }

    #[test]
    fn chain_edits_track_what_was_sent() {
        let mut p = nine_tiles();
        p.apply(&Edit::AddEffect {
            slot: 4,
            effect: octave(),
        })
        .unwrap();
        p.apply(&Edit::AddEffect {
            slot: 4,
            effect: Effect::new(EffectKind::Reverb),
        })
        .unwrap();
        assert_eq!(p.slot(4).unwrap().chain.len(), 2);
        assert_eq!(p.slot(4).unwrap().chain[0].kind, "Pitch");

        p.apply(&Edit::MoveEffect {
            slot: 4,
            from: 0,
            to: 1,
        })
        .unwrap();
        assert_eq!(p.slot(4).unwrap().chain[1].kind, "Pitch");

        p.apply(&Edit::UpdateEffect {
            slot: 4,
            index: 0,
            effect: Effect::new(EffectKind::Chorus),
        })
        .unwrap();
        assert_eq!(p.slot(4).unwrap().chain[0].kind, "Chorus");

        p.apply(&Edit::RemoveEffect { slot: 4, index: 0 }).unwrap();
        assert_eq!(p.slot(4).unwrap().chain.len(), 1);

        p.apply(&Edit::SetBankName {
            slot: 4,
            name: "trem".into(),
        })
        .unwrap();
        p.apply(&Edit::SetGainBank {
            slot: 4,
            gain_db: -5.0,
        })
        .unwrap();
        p.apply(&Edit::SustainKiller {
            slot: 4,
            killed: None,
            reset: Some(true),
        })
        .unwrap();
        let b = p.slot(4).unwrap();
        assert_eq!(b.name, "trem");
        assert_eq!(b.gain_db, Some(-5.0));
        assert_eq!(b.sustain_killed, None, "reset alone leaves no state");
    }

    /// The refusals are the point: each is a write the firmware would take
    /// with `true` and the shadow would then misdescribe.
    #[test]
    fn edits_that_the_shadow_cannot_honestly_record_are_refused() {
        let mut p = nine_tiles();
        p.set_slot(8, None).unwrap();
        let add = |slot| Edit::AddEffect {
            slot,
            effect: octave(),
        };
        assert_eq!(p.check(&add(8)), Err(ProfileError::EmptySlot(8)));
        assert_eq!(p.check(&add(9)), Err(ProfileError::SlotOutOfRange(9)));
        assert_eq!(p.check(&add(-1)), Err(ProfileError::SlotOutOfRange(-1)));
        assert_eq!(
            p.check(&Edit::SetBankName {
                slot: 8,
                name: "x".into()
            }),
            Err(ProfileError::EmptySlot(8))
        );
        assert_eq!(
            p.check(&Edit::RemoveEffect { slot: 4, index: 0 }),
            Err(ProfileError::IndexOutOfRange {
                slot: 4,
                index: 0,
                len: 0
            })
        );
        // AddBank into an empty slot is fine: that is what it is for.
        assert!(
            p.check(&Edit::AddBank {
                slot: 8,
                bank: BankSpec::new("new")
            })
            .is_ok()
        );
        // And a refused apply changes nothing.
        let before = p.clone();
        assert!(p.apply(&add(8)).is_err());
        assert_eq!(p, before);
    }

    /// The on-disk form is the model's own field names, not the wire's, and
    /// a file with the wrong slot count is refused rather than adopted.
    #[test]
    fn a_profile_round_trips_through_serde_in_its_own_vocabulary() {
        let mut p = Profile::empty();
        p.set_slot(
            4,
            Some(BankSpec::new("Tremolo").gain_db(-5.0).with_effect(octave())),
        )
        .unwrap();
        let text = serde_json::to_string(&p).unwrap();
        assert_eq!(
            text,
            r#"[null,null,null,null,{"name":"Tremolo","gain_db":-5.0,"sustain_killed":null,"chain":[{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}]},null,null,null,null]"#
        );
        let back: Profile = serde_json::from_str(&text).unwrap();
        assert_eq!(back, p);

        let short: Result<Profile, _> = serde_json::from_str("[null,null]");
        assert!(short.is_err());
    }
}
