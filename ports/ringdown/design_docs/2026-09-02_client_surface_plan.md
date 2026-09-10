# Client surface plan: typed writes, a shadow profile, and the bank-creation question

**Date:** 2026-09-02
**Status:** in progress. Decisions 1–4 below were taken by Mark on
2026-09-02. **Phase A landed 2026-09-02** (commit 77207ef). **Phase B landed 2026-09-02**
(commit 3611c55). **Phase C landed 2026-09-02** (uncommitted at time of
writing). B and C are verified against a scripted link only, not against the
instrument. **Phase D closed 2026-09-02**: bank creation works (H47). Phases A–C landed
(77207ef, 3611c55, 86c266a); the session log is at the foot.

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
- **2026-09-02 — Phase A landed.** Two new modules in the core, both
  `no_std`:
  - `ringdown::effects`: `EffectKind` (thirteen variants, `wire_name`,
    `keys`, `from_wire_name`, case-insensitive `canonical_key`), generated
    from one table with `PARAMETER_KEYS` and `EFFECT_TYPES` so they cannot
    drift. `Effect::new(EffectKind)` is the checked path and `with` refuses a
    wrong key with the kind's accepted list in the error; `Effect::unchecked`
    and `with_unchecked` are the probe's path and check nothing. `BankSpec`
    carries the H28 bank model with `BANK_CHAIN_KEY` as the one provisional
    line Phase D will change. `ParamError` is the type for "the firmware
    would answer `true` and do nothing".
  - `ringdown::plan`: a `Call { method, params }` per verified write, each
    doc naming its receipt; the metronome pair return `Result` because
    `params::metronome` now refuses `den` outside `METRONOME_DEN_ACCEPTED`.
  - `rpc` re-exports the moved types, so `rpc::Effect` and
    `rpc::PARAMETER_KEYS` still resolve. `rpc.rs` shrank from 1397 to 1258
    lines; `effects.rs` is 527 and `plan.rs` 344.
  - Tests: every planner pinned to its exact wire bytes, one with the full
    envelope; every planner checked against `param_shape`; the den refusal;
    the wrong-key refusal; the unchecked path. 135 tests across the
    workspace, clippy and rustdoc clean.
  - **Consumer impact, not yet addressed:** `woodshed-instrument` calls
    `rpc::params::metronome` and will not compile against this ringdown
    until it handles the `Result`; its wire-order test also passes `den: 8`,
    which is now refused. Woodshed is its own repo; the change there is one
    `?` and one literal, to be made when it next takes ringdown.
- **2026-09-02 — Phase B landed** in `ringdown-client`, desk-verified only.
  - `WEDGING_METHODS` (`ReadConfig`) is refused in `call_raw`, the one loop
    under `call`, `call_named` and every typed write, before anything is
    written; the error is `TransportError::Refused` and its message names
    the override. Matching is case-insensitive, since whether the firmware
    is strict about case is untested and a wrong guess costs a power cycle.
    `Guitar::allow_wedging_calls()` lifts it; the name stands as proposed.
    `Guitar::read_config` is gone; the probe's `--config` flag calls the
    override and prints a warning first.
  - `Sent { id, reply }` with `parsed()` is what every typed write returns.
    `Guitar::send(Call)` is the one path under fourteen typed methods, one
    per Phase A planner, each doc repeating only the receipt and linking the
    planner for the full account. `ParamError` maps into
    `TransportError::Param` so a refused `den` surfaces from the driver too.
  - `Guitar::link()` added, public: a platform accessor the tests needed and
    a consumer may too.
  - Tests over a `ScriptedLink` (records writes as text, answers the next
    scripted result under the request's id): the guard fires before the
    link is touched and the override passes; a typed write sends the exact
    planned envelope and carries `false` back as an answer rather than an
    error; a refused `den` never reaches the link; three bank methods send
    their documented params. The link is `#[cfg(test)]` in the client for
    now and is what Phase C's profile tests will build on.
  - 139 tests across the workspace, clippy and rustdoc clean. **Not
    hardware-verified:** nothing here changes bytes on the wire for a call
    that worked before, but the probe's `--call` and `--config` paths have
    only been compiled, not run against the guitar.
- **2026-09-02 — Phase C landed**, desk-verified only. Four departures from
  the phase as written, each a simplification rather than a change of
  intent:
  - **The shadow lives in the core, not the client.** `ringdown::profile`
    is pure state with no I/O, which by this repo's own discipline belongs
    in the sans-io crate; the client keeps the async half. `Profile` is nine
    slots, serialises as a bare array with `null` for empty, refuses a file
    with the wrong slot count, and uses `BankSpec`'s field names rather than
    the wire's so a saved profile survives the wire shape changing.
  - **`BankSpec` is the slot's content;** there is no separate `BankShadow`.
    Same four fields, one type, two serialisations kept deliberately apart:
    serde derive for persistence, `to_value` for the wire.
  - **`plan::Edit` is the vocabulary.** A profile-changing write as data;
    `Edit::call()` plans the same bytes as the function of the same name
    (pinned for every variant) and `Profile::apply` consumes the same value,
    so the wire and the record cannot describe different writes. `apply`
    returns the bank that left the profile, pushed off by `AddBank` on a
    full grid (H38) or taken out by `RemoveBank`; a failed apply changes
    nothing.
  - **Three driver operations instead of two:** `drain_chain` (the H31
    count, destructive by construction and documented as such), `push_bank`
    (chain first, then name, then gain and sustain, the H33 order) and
    `restore_bank` (drain, then push, returning `Restored { drained,
    expected, pushed }` where `drained != expected` is the discrepancy
    signal). `Guitar::edit(&mut profile, &edit)` checks the shadow before
    sending and applies only on a parsed reply; an edit into an empty slot
    never reaches the link. `set_listening(true)` refuses every typed write,
    not only the verify, since any write voids an A/B; reads and the raw
    `call` path are not gated.
  - The shadow cannot know a factory bank's own chain. `restore_bank` on
    such a slot drains effects the shadow never recorded, and they stay
    gone until the vendor app reconnects (H32). Documented on both
    operations; a client should restore only slots it built up itself, or
    accept that.
  - Tests: profile semantics in the core (H38's insert-and-shift by name,
    remove-and-shift, chain edits, every refusal with the state left
    untouched, serde round trip with exact JSON and the wrong-count
    refusal); `Edit` against every planner; in the client over the scripted
    link, `edit` records a parsed reply and not a refused one and refuses an
    empty slot before the link, `restore_bank` drains to `false` and pushes
    in H33 order with the exact method sequence, and listening refuses
    writes while `status` still answers. 148 tests, clippy and rustdoc
    clean.

## Phase D session log (2026-09-02, H2-CC340, owner at the panel)

State on arrival: profile still shifted from H38 (`octave` at 4, Tremolo 5,
Octaver 6, Disto 7, Boost 8); the vendor app has not connected since.

- **D0 — control.** `GetStatus` answered with live values (battery 51%,
  STM V1.2.3, ESP V1.3.0). `PrintBank {bank_num: 5}` on the populated
  Tremolo bank: `true`, nothing followed within the 800 ms drain. That
  method was only ever tried on an empty instrument (H9, H11); it is now a
  dead end on a bank with content too.
- **D1 — baseline.** Owner selects tile 4 (`octave`), plays a G: dry, no
  octave. H38 reproduced before touching anything.
- **D2 — gain hypothesis: killed.** `SetGainBank {bank_num: 4, gain: 0}`
  (decibels, H28): `true`. Owner plays without touching the panel: dry.
  Switches away and back to tile 4, plays: dry. So a zero-dB gain write does
  not revive the H38 bank, and the re-select changes nothing either.
  Caveat carried: `SetGainBank` has no receipt of its own (H27, H28), so this
  kills "the bank lacks a gain and `SetGainBank` supplies it", not "the bank
  has no gain problem".
- **Desk finding, before the session.** The compressor dictionary
  (`compress::KEYWORDS`) holds `effects`, `name`, `gain`, `id`, `preset`,
  `default` and `bank`, and not `chain`, `fx`, `sustain` or `killed`. So the
  `effects` key H38 sent is a firmware word, hypothesis 1 of this plan is
  largely dead on the desk, and `id` joins the candidate bank fields.
- **D3 — `RemoveBank` hardware-verified; the list model holds.**
  `RemoveBank {bank_num: 4}`: `true`. Owner reads the panel: off, reverb,
  chorus, echo*, phaser, tremolo, octave, dist, boost, off. With the leading
  `off` as the panel's off position and the trailing one the empty tile,
  that is the factory layout: Tremolo back at 4, Octaver 5, Disto 6, Boost 7,
  tile 8 empty. So `RemoveBank` takes a bank out and shifts every later one
  down, as `Profile::apply` models it. The `ringdown` tile H38 pushed off is
  not back; it left the profile.
  Two observations carried, not interpreted: tile 4 (Tremolo) "sounds like
  octave -12", most likely a stray Pitch from the H36/H37 session in that
  bank; and the owner marks echo with an asterisk on the panel.
- **D4 — full-object hypothesis: killed.** Into the empty tile 8, recorded
  verbatim: `AddBank {"bank_num":8,"bank":{"id":100,"name":"ringdown",
  "gain":0,"effects":[{"preset":"default","type":"Pitch","bypass":false,
  "params":[{"key":"Shift","value":-12}]}]}}` → `true`, 73 bytes out. Panel:
  tile 8 reads `ringdown`, tiles 4–7 unmoved (no shift into an empty tile, as
  expected). Ears: dry. So `id` and `gain` alongside `name` and `effects` do
  not make the bank render. Same signature as H38, now reproducible.
- **D5 — `SwitchBank` will not select the `AddBank` bank.** `SwitchBank
  {bank_num: 8}`: `true`, and the panel selection did not move; the owner
  selected tile 8 by hand and played: dry. Control: `SwitchBank {bank_num:
  5}`: `true`, panel moved to Octaver on its own. So `SwitchBank` drives the
  panel today (H25 holds) and refuses, with `true`, the bank `AddBank` made.
  The firmware keeps the tile's name and does not treat what is behind it as
  selectable. A name is not a bank, now from the selection side as well as
  the audio side.
- **D6 — `SaveConfig` commit hypothesis: killed.** With Mark's explicit go
  (persistent config): `SaveConfig {}` → `true`. Owner selects tile 8 by
  hand, plays: dry. The bank is not waiting for a commit.
- **D5 RETRACTED, same session.** The "refusal" was stale state: in D4 the
  owner had selected tile 8 by hand to listen, so the D5 `SwitchBank 8`
  found the selection already there and "did not move" was no observation
  at all. Re-run with the selection moved away first (D6c): `SwitchBank 8`
  moved the panel to tile 8. `SwitchBank` selects the `AddBank` bank like
  any other; H25 holds without exception. What stands from D5 is only the
  control. The lesson is the handoff's rule 4 in another form: know the
  state before reading a non-change as a result.
- **D6c, ears — selection-by-RPC hypothesis: killed.** With the selection
  put on tile 8 by `SwitchBank` rather than by hand, the owner played: dry.
- **D7 — `SwitchBank 8` from the panel's off position: no change.** Owner
  put the panel on off (the position before tile 0), `SwitchBank {bank_num:
  8}` → `true`, panel stayed on off, dry. Whether `SwitchBank` can leave the
  off state at all is the control that follows (D7b).
- **D7b — `SwitchBank` cannot leave the off state.** Control: from off,
  `SwitchBank {bank_num: 5}` (Octaver, a real bank) → `true`, panel stayed
  on off. So D7 was not about bank 8: the panel's off position is a state
  no `SwitchBank` overrides, and the reply is `true` regardless (H27). A
  client cannot turn the effects on from the wire with this method; whether
  another method can is untested (`on`, `BypassEffect` are dictionary words).
- **D8 — `AddEffect` into the `AddBank` bank, clean state: dry.** Pitch −12
  into bank 8 → `true`; owner selects tile 8 by hand (panel on 8, not off),
  plays: dry. H38's "nor effects added afterwards" holds with correct
  indices.
- **D9 — the inline chain was stored.** Drain of bank 8 by `RemoveEffect`
  at 0: `true, true, false, false, false, false` — two effects, the inline
  Pitch from the D4 object and the D8 one. So `AddBank` stores its `effects`
  array; the record has a name, a chain, accepts more effects, is selectable,
  survives `SaveConfig`, and does not render. Then one fresh Pitch −12 added
  (`true`) for a listen on a known one-effect chain.
- **Owner's caution, carried into the log:** the panel's off state answers
  `true` to everything and sounds like a dead bank. Today's tile-8 listens
  were made with the panel seen on tile 8; earlier-session silences without
  a panel read now carry that doubt.
- **D9, ears: dry.** One fresh Pitch −12 as the whole chain of bank 8,
  panel on 8: dry. The chain's contents are not the problem.
- **D10 — power cycle: the bank persists and stays silent.** After a
  restart the nine tiles read the same (factory layout, `ringdown` at 8), so
  the saved configuration carries the `AddBank` record; tile 8, panel on 8:
  dry. The DSP does not build a playable bank from it at boot either.

### Where the bank-creation question stands after this session

Killed today, each by one variable with the panel read and the octave
oracle: a missing gain (D2); the object lacking `id`/`gain` (D4); a commit
by `SaveConfig` (D6); loading on selection by RPC (D6c); the inline chain
being ignored (D9: it is stored); a reboot after save (D10). Established on
the way: `RemoveBank` shifts down as modelled (D3); `SwitchBank` selects the
`AddBank` bank like any other (D6c) but cannot leave the panel's off state
for any bank (D7b); `PrintBank` prints nothing over the air on a populated
bank (D0).

What is left is not another guessed key. The `AddBank` record is complete
by every measure this protocol offers and the DSP ignores it, which says
the vendor app does something this session has not seen. Next round:

1. **Capture the app.** The app placing a library bank onto the empty tile
   is the one event that produces a playable bank, and its bytes are
   obtainable: Android's HCI snoop log (developer options) or, for an
   iPhone (which this is), Apple's Bluetooth logging profile on the phone,
   then a sysdiagnose whose packet log opens in PacketLogger or Wireshark on
   a Mac; or, on an Apple-silicon Mac that can run the iPhone app, a live
   PacketLogger capture with no phone involved.
   `compress::decode` turns the captured LLT2 payloads back into JSON. This
   answers whether the app sends `AddBank` at all, with what object, and
   what precedes and follows it. It is the only hypothesis whose outcome is
   not a coin toss, and it replaces guessing at `preset`, `default`, `on`
   or `control` at the bank level, which stay listed only as fallbacks.
2. **The alternative the wording of H32 suggests:** the app may push the
   whole profile through `SetConfig` (shape unrecovered, F-series) rather
   than build banks one call at a time, and `AddBank` may be a path the
   firmware half-implements. The capture settles this too.
3. **Off state.** No method yet found brings the panel out of off; `on` and
   `BypassEffect` are dictionary words to try, read-only-safe, some other
   session.

### Instrument state at close

Factory layout restored by D3 (Tremolo 4, Octaver 5, Disto 6, Boost 7).
Tile 8 holds the `ringdown` `AddBank` record with one Pitch −12, dead. Tile
4 (Tremolo) carries a stray octave from the H36/H37 session. All of it is
in the saved configuration (D6) and survives a restart (D10). Opening the
vendor app restores the factory profile (H32).
- **2026-09-02 — capture decoder landed** (`apps/probe/src/pklg.rs`,
  `ringdown-probe --decode-pklg <file>`), written while the owner set up the
  capture. Reads Apple PacketLogger's `.pklg` directly (record layout per
  Wireshark's `packetlogger.c`, byte order detected from the first record),
  reassembles ACL fragments into L2CAP, keeps ATT, names the guitar's two
  characteristics from service discovery when the capture holds it, and
  decodes every write and notification through the crate's own codec: bare
  compressed messages, LLT2 frames reassembled per object id, LLT2 and LLT1
  acks, plain JSON, the banner. `AddBank`, `SetConfig`, `AddEffect` and
  `UpdateEffect` from the app are pretty-printed where they occur; the
  summary lists every method the app sent. Nine tests, including a
  compressed `AddBank` round trip through the decoder. Untested against a
  real capture until one exists.

### The capture (2026-09-02, vendor app on an Apple-silicon Mac, PacketLogger)

Two captures of the iPhone app running on Mayola's M4, decoded with
`--decode-pklg` on its first meeting with real traffic: 1,540 and 8,994
records, 40 and 267 writes, every one decoded. The first PacketLogger file
was empty (one log record, no HCI) until Apple's macOS Bluetooth logging
profile was installed.

- **The bank object, verbatim from the app** (`AddBank`, capture 1, id 23):
  `{"bank_num":8,"bank":{"name":"Crystals","gain":20.0,"effects":[...],
  "fbk_onoff":true,"fbk_params":[]}}`. Keys in that order. **`fbk_onoff` and
  `fbk_params`** are the two fields no ringdown object ever carried; both
  are dictionary words (F15) read as calibration config until now. No `id`.
  Effect entries are `{preset, type, bypass, params}` as ringdown sends
  them; `preset` is `"default"`, `"None"` or `"Default"` in the same
  profile, so it is lenient. A param may carry
  `"control":{"source":"Slider","min":..,"max":..}` inline, which is the
  `SetController` binding embedded in the effect.
- **The app's sequence on connect:** `GetStatus`, `SetDate`, `GetStatus`,
  `SetDate`, then **`SetConfig` with the whole profile** (three to six LLT2
  frames of 497 bytes), then `SwitchBank 0`, then `SaveConfig`. That is H32
  made exact: the app pushes its profile and the instrument keeps it. After
  an edit: the per-edit method, then `SaveConfig`. Placing a bank:
  `AddBank`, `SaveConfig`, `SwitchBank`.
- **`SetConfig` recovered** (was `ParamShape::Unrecovered`): `{file_type:
  "config", version: 1.0, favorite_banks: [8 banks], calibration_on,
  metronome: {bpm, num, den, nbbars}, equalizer: {params: [GainBand1–6,
  Gain]}, aux_in_drywet, aux_in_on, aux_out_drywet, aux_out_on,
  factory_reset, version_stm, version_esp, cpu_id, free_space}`. Eight
  banks, not nine: the ninth tile is the profile's empty slot.
- **Delay's SYNC note-value key is `DelaySync`** (values 375, 562.5, 750,
  4.0 seen; unit to establish). Another F15 "effect type" that is a
  parameter, like `LFO`. Closes the H31 unknown.
- **The equalizer has six bands** in the app's own writes (`GainBand1–6` +
  `Gain`); H31's accepted `GainBand7` is a key the firmware parses and the
  app never sends.
- **ATT MTU is 500**, negotiated by the app (request 527, response 500), so
  the app's write length is 497, which is what every LLT2 frame in the
  capture measures. `ASSUMED_WRITE_LEN` of 514 is wrong for this
  instrument; ringdown has never sent a write over 497 bytes, which is why
  it never failed. To fix in the driver.
- **The only `false` replies in 267 writes:** five `UpdateMetronome
  {"den":8}` from the app, `den` alone without `bpm`, exactly the refusal
  H24 recorded. The vendor app's own denominator control does not work
  over this path, live.
- Vocabulary confirmed from the app's own writes: `UpdateEffect` carries a
  whole effect with `control` inline; `SetController {bank_num, effect_num,
  parameter, source, min, max}`; `MoveBank {src, dst}`; `RemoveBank
  {bank_num}`; `SetBankName`; `StartMetronome {}`; `UpdateMetronome {bpm}`;
  `StartRecording {free: true}`; `StopRecording {}`. `SwitchBank` is sent
  after nearly every edit.
- **2026-09-02 — corrections from the capture, desk-verified** (H39–H45 in
  the founding doc). `BankSpec` is now the app's object: `name`, `gain`
  (always sent), `effects`, `fbk_onoff`, `fbk_params`, in the app's order,
  with `sustain_killed` kept as shadow-only state; the captured `Crystals`
  object is rebuilt from the typed model byte for byte in a test.
  `Parameter` carries an optional `control` binding and `Effect::bound_to`
  sets it. `Gate` gains `Hysteresis` and `Hold`, `Delay` gains `DelaySync`,
  both in the app's key order. `params::metronome` and the two metronome
  planners take `bpm` as an option, since the app writes one field at a
  time and the reply is then an oracle (H43); the `METRONOME` shape is all
  optional. `SetConfig` has a declared shape and `ParamShape::Unrecovered`
  is gone: every method is declared. `ASSUMED_WRITE_LEN` is 497 (H45).
  Persisted profiles change shape (`fbk_onoff`, `fbk_params` added,
  `gain_db` required); none exist yet outside tests. Consumer note: the
  metronome signature changes again for woodshed. 156 tests, clippy and
  rustdoc clean.
- **Next on the wire, when the guitar is on:** `AddBank` into an empty tile
  with `BankSpec::new("ringdown").gain_db(20.0)` and the octave, the app's
  field set exactly; then the owner's ear. If it renders, Phase D closes
  and `AddBank`, `SaveConfig`, `SwitchBank` is the app's own recipe (H42).

### Phase D closed (2026-09-02): the client can create a playable bank

After the capture supplied a known-good object, eight `AddBank` calls into
tile 8 of an app-restored factory profile, one variable each, judged by ear.
The full series and both retractions are H47 in the founding doc. In short:

- **`AddBank` creates a bank the DSP plays**, the moment it is added. No
  `SaveConfig`, no `SwitchBank` needed after it; those are the app's habit
  (H42). H38's "the bank it creates never renders" is retracted, and D1–D10
  above rest on the same silence and go with it.
- **The requirement is `gain != 0`.** Not the feedback fields, not `id`, not
  the JSON number type. And 0 is a **sentinel rather than a level**: −5 and
  20 are both audible, so 0 is a silent hole between them.
- Two errors of mine, caught in the hour: blaming the feedback fields on a
  comparison that changed four things at once, and claiming gain explained
  the earlier failures when those ran on a profile the app had not yet
  rebuilt. The owner caught a third — every test bank had been `Pitch −12`,
  so a stale chain and a fresh one were indistinguishable until `Shift +12`
  became the oracle.

**Done-conditions, against the phase as written:** a playable bank is
created and its recipe is pinned in `BankSpec` (`DEFAULT_BANK_GAIN` is 20,
and a test refuses a silent default); every object sent is recorded verbatim
in H47; the instrument is left with a test bank in tile 8 that the vendor
app will clear on its next connect.

**What Phase D leaves open**, and it is no longer blocking anything:
the gain scale's units and working range (the app compensates per effect,
−5 for a reverb to 50 for an octaver); whether `SetGainBank(slot, 0)`
silences a bank that is already playing; and whether a bank object needs
`name` and `effects` at all, since every object that played carried both.
