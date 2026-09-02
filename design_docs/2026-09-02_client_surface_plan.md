# Client surface plan: typed writes, a shadow profile, and the bank-creation question

**Date:** 2026-09-02
**Status:** plan. Decisions 1–4 below were taken by Mark on 2026-09-02; no
crate has been touched under this plan yet.

Findings that ground this live in `2026-08-27_ringdown_founding.md` (H25–H38)
and the session handoff `2026-09-01_effects_handoff.md`. This plan does not
restate them; it says what the client's surface becomes because of them.

## Objective

Give a consumer (woodshed first) a typed way to do what the hardware sessions
proved by hand through the probe's `--call Method key=value` path: edit a
preset's effect chain, rename and select presets, drive the metronome, and
keep a record of what was sent so it can be checked and re-sent. Today
`ringdown-client::Guitar` types only status and files; every verified write
is a string method name and a `serde_json::Value`, and woodshed carries its
own guard against the one call that wedges the firmware.

## Decisions taken (Mark, 2026-09-02)

1. **Typed operations live in two places.** Pure request planners in the
   sans-io core (`ringdown::rpc`), testable without an instrument, and one
   thin async method per planner on `Guitar`.
2. **The client keeps a shadow profile.** Nine slots, each recording what was
   sent, able to check the instrument still matches and to re-push after the
   vendor app has wiped it.
3. **The driver owns the guards.** `ReadConfig` is refused by the driver, not
   by each consumer; `read_config` leaves `Guitar`.
4. **Bank creation is an open question, not a constraint.** The vendor app
   creates and places banks; that it can and we could not yet is a gap in
   knowledge. Phase 2's target in the founding plan, everything the app can
   do, stands. What changes is that its done-conditions built on `ReadBank`
   round-trips are replaced by receipts of the kind this protocol can give:
   the remove-count and the owner's panel and ears.

Two things are settled and not on the table: the **effect catalog is closed**
at thirteen (H28, read off the app with the scrollbar at both ends), and
adding effect types is firmware work outside this plan's objective.

## What the hardware fixes about the shape

These are the constraints the surface is designed around. Each cites the
finding so the design can be re-examined if the finding falls.

- **No read-back.** `ReadBank` answers `""` for every slot (H25); the only
  count is remove-until-`false`, which empties the chain to count it (H31).
  So the shadow is not a cache, it is the only record a client has.
- **`true` means parsed** (H27). A typed write returns something that says
  *sent*, never *set*, and the doc on each says which receipt proves it.
- **Field order is load-bearing** (H24). Planners emit structs in wire order;
  no `json!` maps for anything the firmware reads positionally.
- **The vocabulary is closed and known** (H28, H31): thirteen kinds, a key
  list per kind. A wrong key is refused by the firmware; it can be refused by
  the compiler first.
- **The vendor app overwrites the profile on connect** (H32). The shadow
  therefore needs to survive a process restart, which means it serialises.
- **`AddBank` inserts and renumbers** (H38). Any shadow that holds slot
  indices is stale after one; the typed layer treats it as a profile-level
  operation, not a bank-level one.
- **`ReadConfig` wedges the firmware** (H18). Refused at the driver.

## Phases

### Phase A — Typed vocabulary and planners (core, sans-io)

Feature target: every write the hardware sessions verified has a planner in
`ringdown::rpc` that produces `(Method, Value)` and cannot produce a message
the firmware would refuse for a reason we already know.

Done-conditions:

- An `EffectKind` enum of the thirteen, with `wire_name()` and the per-kind
  key list from `PARAMETER_KEYS` reachable from it. `Effect::new` takes the
  enum; the `&str` form stays for the probe, marked as the unchecked path.
- `Effect::with(key, value)` refuses a key outside the kind's list and says
  which kind and which keys it accepts. Delay's SYNC note-value key stays
  unknown and is documented as such, not guessed.
- Planner functions for: `add_effect`, `update_effect`, `remove_effect`,
  `move_effect`, `set_bank_name`, `set_gain_bank`, `sustain_killer`,
  `switch_bank`, `move_bank`, `remove_bank`, the metronome trio, and
  `add_bank` (taking a typed `BankSpec`, see Phase D). Each is a pure
  function; each is pinned by a test that asserts the exact wire string.
- `params::metronome` refuses `den` outside `{1,2,4,16}` at build time
  (H24), instead of sending a write the firmware will silently drop.
- `cargo test`, clippy, rustdoc all clean; `no_std` preserved.

Code samples in this plan are illustrative, not compile-ready.

### Phase B — Driver guards and typed methods (`ringdown-client`)

Feature target: a consumer never names a method by string for a verified
operation, and cannot wedge the instrument by accident.

Done-conditions:

- `Guitar::call` and `call_named` refuse `ReadConfig` with a typed error
  naming why (H18) and pointing at the override. An explicit
  `Guitar::allow_wedging_calls()` (name to settle) lifts it for the probe.
  `Guitar::read_config` is removed.
- One async method per Phase A planner. Each returns `Result<Sent, _>`,
  where `Sent` carries the request id and the raw reply, and the method's
  doc states which receipt verifies it: panel (`SetBankName`, `SwitchBank`),
  ears (`AddEffect`, `bypass`), `ReadMetronome` (metronome), or none.
- The probe's `--call` path keeps working unchanged; it is the research
  instrument and must not lose reach.
- Woodshed's `FORBIDDEN_METHODS` becomes redundant. Noted here; woodshed is
  its own repo and is not edited under this plan.

### Phase C — Shadow profile (`ringdown-client`)

Feature target: a client can say what it believes each slot holds, check
that belief against the instrument without ears, and restore it.

Done-conditions:

- `Profile` of nine `Slot`s; a `Slot` is `Option<BankShadow>` where
  `BankShadow { name, gain_db, sustain_killed, chain: Vec<Effect> }`, the
  bank model H28 read off the app. Serialises with serde so a desktop client
  can keep it on disk across the vendor app's wipes (H32).
- Every Phase B write goes through the profile, which applies the same
  change to its shadow only when the reply parsed as `true`. `AddBank` and
  `RemoveBank` shift the slot vector exactly as H38 describes.
- `Profile::verify_chain_len(slot)`: drain-and-count, then re-push the
  shadow's chain. Documented as destructive to the live chain during the
  check, and refused while the owner is listening (a flag the caller sets,
  after H37's caution about sending during an A/B).
- `Profile::re_push(slot)` re-sends a slot's shadow in the order H33
  requires: first effect, then name, then gain and sustain.
- Tests over a fake `Link` that records what was written and answers `true`
  or `false` by script: the shadow tracks accepts, ignores refusals, and
  re-push emits the same bytes the original edits did.

### Phase D — Research: how the app creates a playable bank

Feature target: ringdown creates a bank in a slot that renders audio, heard
by the owner with the octave-down Pitch oracle.

This is research with hardware, so it is written as hypotheses to kill, not
as steps. The H38 call is unreproducible: the bank object it sent was never
recorded. That is the first thing to fix.

Hypotheses, one hardware test each, cheapest first:

1. **The chain key is not `effects`.** H38's object carried an `effects`
   array; the app's own word for it is unknown. Candidates from the app's
   vocabulary: `chain`, `fx`, `effect`. Test: `AddBank` with a one-effect
   chain under each key, octave oracle, drain-count afterwards.
2. **The bank object needs its full shape.** `{ name, gain, sustain killer,
   chain }` per H28, with keys in the app's order. A partial object may be
   parsed (`true`) and stored dead. Test: full object, then the same object
   with one field removed at a time.
3. **`preset` capitalisation inside the chain** (H28 point 2) or another
   field-order slip makes the whole bank unrenderable while still `true`.
4. **A commit step follows.** `SaveConfig` after `AddBank`, or `SwitchBank`
   onto the new slot, is what the app sends next (H34's hypothesis, still
   untested on a real `AddBank`).
5. **The app never calls `AddBank` on a full profile.** It may `RemoveBank`
   first, or place into an empty tile only. Test: `AddBank` into slot 8 of a
   factory profile after the app's reset.

Done-conditions:

- The bank object sent is recorded verbatim in Findings for every attempt.
- Either a playable bank is created and its recipe pinned as a `BankSpec`
  test in Phase A, or all five hypotheses are recorded as killed with the
  evidence, and the next round of hypotheses is written before the session
  ends.
- The instrument is left with the app's factory profile (open the app),
  and the shadow from Phase C is what proves the client's edits survive a
  re-push afterwards.

Owner protocol for the session is the handoff's list: one variable, one
listen; count before indexing; nothing sent while he is comparing by ear.

## Findings

- **2026-09-02 — The H38 bank object is unrecorded.** Not in the founding
  doc, the commit, or the probe source; the call went through `--call` with
  a literal typed at the shell. The finding "the bank it makes never
  renders" is therefore a finding about one unknown object, and Phase D
  starts by making every attempt reproducible.
- **2026-09-02 — Woodshed drives the guitar by string.** `woodshed-instrument`
  calls `ReadMetronome` and `GetAnalysis` via `call_named`, builds the
  metronome write with `rpc::params::metronome`, and keeps its own
  `FORBIDDEN_METHODS` list for `ReadConfig`. It is the consumer that shows
  the driver's missing surface.
- **2026-09-02 — `Guitar::read_config` exists and is reachable** from the
  probe's `--config` flag, in a driver whose README says the method is
  refused rather than exposed. The guard is a comment; Phase B makes it code.

## Progress

- **2026-09-02** — Plan written after the effects handoff review. Doc drift
  fixed the same session: `den` wording in the handoff, `params::bank` doc
  moved from H36 to H38, stale `bypass` and crate-status docs, README
  capability line, one rustdoc warning. Workspace green: 123 tests, clippy
  and rustdoc clean.
