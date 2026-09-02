# Handoff: effects work on the HyVibe, 2026-09-01

Written at the end of a long hardware session, for whoever picks this up.
Findings live in `2026-08-27_ringdown_founding.md` as H1–H38; this is the
**state, the open questions, and the failure modes that cost the most time.**

## Read this first: how to not repeat the day's mistakes

The session produced ~15 findings and **retracted 6 of them**, all for the same
few reasons. In rough order of how much time each cost:

1. **`true` means parsed, never applied (H27).** Nearly every method answers
   `true` to things it silently drops — `den` outside its whitelist, `AddEffect`
   into a bank that cannot render, `SetBankName` on an empty tile. *A `true` is
   not evidence of anything.* Only the owner's eyes (the panel) and ears settle
   a write.
2. **Control the bank.** Effects are only audible in a bank the **app**
   created. Writes to an empty tile, or to a bank made by `AddBank`, store fine
   and never sound. Half the session's "effects don't work" conclusions were
   experiments run in a slot that could not have played anything.
3. **Choose an orthogonal probe.** Adding Distortion after Reverb is mud and
   proves nothing. The oracle that works is **`Pitch {Shift: -12}`** — an octave
   down cannot be masked, and no default bank touches pitch. The owner's own
   test is as good: a G at full volume feeds back through a live gain effect and
   not through a silent chain.
4. **Count before indexing.** `ReadBank` always returns `""`, so a bank's length
   is unknown. `RemoveEffect` at index 0 until it answers `false` **counts** the
   chain — the only read-back this protocol has. Two wrong findings came from
   assuming a bank held one effect when it held two.
5. **Do not send anything while the owner is listening for a difference.** A
   `SwitchBank` moves the selection under them and voids the comparison.
6. **One variable, one listen.** Batching writes and then asking "what do you
   hear" produced most of the ambiguous results.

## What is established and usable

`AddEffect` into an **app-created** bank is audible and complete: 13 effect
types, the full per-effect parameter vocabulary (`rpc::PARAMETER_KEYS`),
`bypass` works, chains render past the app's four-effect limit, `MoveEffect`
and `RemoveEffect` work at any index. Metronome bpm/num write; `den` does not.
`SwitchBank` drives the panel. `SetBankName` works on a populated bank.

**So: a client can send someone a tone for a bank they already have.**

## What is not possible, or unknown

- **Creating a playable bank.** `AddBank` *inserts* at an index, shifts every
  later bank along, pushes the ninth off the end of the profile — and the bank
  it creates never renders (H38). No known way to make a new playable bank.
- **`den`**, Delay's SYNC note-value key (25+ names refused), and whether the
  unexplained transient after a multi-effect write is real (H37, downgraded).
- **Durability.** The vendor app overwrites the whole profile when it connects
  (H32). Every write is volatile until then.

## Instrument state as of this handoff

Profile is shifted: an `AddBank` inserted "octave" at index 4, pushing Tremolo
to 5, Octaver 6, Disto 7, Boost 8, and the tile named "ringdown" off the list.
Several banks hold stray test effects. **Opening the vendor app restores the
factory profile** and clears all of it; nothing here needs repair by hand.

## Working with the owner

He plays the instrument and reads its panel, so he is the measuring device, and
every correction of consequence this session came from him rather than from the
protocol. Tell him precisely what to listen or look for and what each outcome
would mean, send one thing at a time, and take a pushback on test design as
data — he caught the mud test, the phantom bank, and the stale-index arithmetic.
