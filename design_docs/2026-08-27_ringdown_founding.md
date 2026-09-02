# Ringdown — an independent client for the HyVibe smart guitar

**Date:** 2026-08-27
**Status:** in progress. **Phase 1 is complete** as of 2026-08-27: the
protocol is hardware-verified, both transports are implemented, and the one
method that resisted (`ReadConfig`) is established as broken in the firmware
rather than blocked by anything ringdown does — its contents reachable by
composing methods that work (H19). Detail below.

Phase 0 landed 2026-08-27. **Phase 1 record:** the GATT surface (H1), the version banner (H2), and a full `GetStatus`
round-trip (H4) are hardware-verified against instrument H2-CC340, with the
device's reply pinned as a test fixture. Remaining before the phase closes: `ReadConfig`,
which needs **LLT2** (H6/F14). The file mechanism was explored as a way around
that and does not reach the configuration (H16), so Phase 2.5 is confirmed
necessary rather than merely likely. Nothing has been written to the guitar's
configuration; every exchange so far has been a read.

---

## What this is

The HyVibe system turns an acoustic guitar into its own amplifier, multi-effect
processor, looper, and speaker, with the DSP running on hardware inside the
instrument. The only way to configure it is the vendor's iOS/Android app.
**Ringdown is an independent desktop client** that speaks the guitar's own
Bluetooth protocol directly, so the instrument can be configured — eventually
past what the phone app exposes — from a computer, and so its capability is
owned rather than rented from an app that could disappear.

**Ringdown** is how a resonating system decays once it stops being driven — the
tail of a struck string or a rung body. The name carries the mechanism rather
than resembling it, which is the bar this workspace sets for a product-tier
name: this instrument's whole trick is controlling how long its body keeps
sounding, and `SustainKiller` is ringdown control by another name. Verified
free on crates.io (API + sparse index) on 2026-08-27.

**Renamed from `antinode` on 2026-08-27**, before the first push. `antinode`
named the same physics correctly — the aliquot divisions of a vibrating string
— but read as "anti-Node" in a software context and said nothing about what the
crate does. `antinode` 0.0.1 remains claimed and unused. The alternatives are
recorded in the naming ledger; `luthier` was the clearest candidate and was
refused deliberately, reserved along with the guitar-part words for software
about the craft of building instruments rather than talking to one.

**Scope of the name.** "HyVibe" is the vendor's trademark and appears in this
repo only as prose describing what ringdown interoperates with — never in a
crate name, per the expression boundary in `DOC_POLICY.md`. Ringdown is not
affiliated with or endorsed by HyVibe.

---

## The decision: a new repo, on the retinue template

This is a **new repository under `repos/`**, not a subtree of woodshed. The
reasoning:

- **The protocol crate is publish-tier and reusable.** A JSON-RPC-over-BLE
  client for a specific instrument is a library other things can consume; it
  should not be buried inside a practice-toolkit application. This mirrors the
  family's existing posture where `signalman-desktop` roots its own workspace
  outside the protocol repo rather than fusing app and protocol.
- **Woodshed is the first consumer, not the owner.** Woodshed already has the
  metronome, MIDI clock, looper, live-input recording, tuner, and latency
  calibration that this instrument exposes over the wire. It consumes ringdown
  as a git dependency for the UI (the same way woodshed consumes
  `genet-host-api` from genet). The two are siblings, not parent and child.
- **The firmware option wants a home of its own.** If the project ever reaches
  alternative firmware (Phase 4, explicitly gated below), that is `no_std`
  embedded work that belongs beside the protocol core, not inside a desktop
  app.

The structural template is **retinue**, which solves a structurally identical
problem for LoRa radios: a sans-io protocol core ("pure functions over bytes,
replayable against fixtures"), a thin I/O shell over it, `no_std` firmware
crates on embassy + esp-hal, a serial-DFU flashing tool, and receipt-driven
validation against a reference implementation. Ringdown borrows that shape
directly. In particular, HyVibe's LLT chunking layer is the same class of thing
as retinue's HDLC framing codec in `iface::hdlc` — a sans-io framer with a
replayable fixture suite.

### Intended crate layout (built across Phases 0–2, not pre-built)

```
crates/
  ringdown/          Sans-io protocol core. JSON-RPC 2.0 envelope, the LLT
                     chunk/reassembly state machine, and the domain model
                     (Bank, Effect, Parameter, Equalizer, Status, …). No BLE,
                     no async runtime, no_std-capable. Publish-tier; the named
                     crate. Replayable against captured fixtures.
  ringdown-ble/      Transport shell: btleplug on desktop (Windows/macOS/Linux).
                     The tokio side of "sans-io core, tokio shell". Plain
                     hyphenated support-crate name per the tier rule; likely
                     publish = false until it has an audience.
apps/
  <spike-cli>/       The consumer that reveals the library's shape (the park.rs
                     / linkboy precedent): connect, GetStatus, ReadConfig, dump.
                     Plain descriptive name, settled in Phase 0. publish = false.
```

Woodshed stays where it is and gains a dependency on `ringdown` + `ringdown-ble`
plus a view surface — no ringdown code lives in the woodshed tree.

---

## Non-goals (v1)

- **No audio over Bluetooth.** The DSP is on the instrument; there is no audio
  stream to carry. Ringdown configures the guitar, it does not process its
  sound on the desktop. (Aux-jack audio routing on the guitar is configured via
  RPC, but the samples never touch the computer.)
- **No file-transfer or firmware-upload paths in the client.** The protocol has
  `sendFile` / `.part` chunking machinery; it is out of scope for the client
  and deliberately left alone (§ Findings, "Gaps").
- **No mobile client.** The protocol is identical from iOS, but ringdown is a
  desktop project.
- **Firmware is not a v1 goal.** It is Phase 4, gated behind its own
  assessment; see below.

---

## Phases

### Phase 0 — Found the repo and claim the name

**Feature target:** the repo is a real project and the name is secured.

Done-conditions:

- Repo exists at `repos/ringdown` with `git init` and the doc scaffold
  (`DOC_POLICY.md`, `DOC_README.md`, this plan). — **met 2026-08-27**
- License chosen (Decision D1) and `LICENSE` added. — **met 2026-08-27**
  (MPL-2.0, byte-identical to retinue's copy)
- Root workspace `Cargo.toml` with the `ringdown` core crate stubbed. —
  **met 2026-08-27** (`cargo build` and `cargo package` both clean; the crate
  is `#![no_std]` and `#![forbid(unsafe_code)]` from the first commit, so the
  sans-io posture is enforced by the compiler rather than by intention)
- **crates.io `ringdown` 0.0.1 reservation published** (Decision D2). —
  **met 2026-08-27.** The ledger's heddle lesson is explicit: a banked winner
  unclaimed is a winner lost, and `coppice` (banked "clean" on 2026-07-30,
  actually taken since 2025-01) is the fresh reminder that the check and the
  claim are one step.

**Phase 0 is complete.**

### Phase 1 — Protocol core and the live proof (the instrument that measures everything after it)

**Feature target:** ringdown connects to the actual guitar, reads its status
and its live effect catalog, and every static claim in Findings is confirmed or
corrected against hardware.

Done-conditions:

- **LLT codec, sans-io, round-trips against captured fixtures.** Chunk a
  message larger than the write length and reassemble it; the fixture set
  includes a payload containing quotes and backslashes, to prove the iterative
  chunk-sizing loop (Findings F5) rather than an arithmetic split. —
  **met 2026-08-27.** `llt` and `handshake` implemented; 21 tests plus a
  doctest, `cargo clippy` and `cargo fmt` clean, no warnings. Covered:
  escaping-heavy payloads, control characters (the six-character worst case),
  multi-byte characters never split across a frame boundary, every frame within
  the write length, contiguous 1-based sequencing, reassembly equalling the
  original, refusal at the un-negotiated 20-byte write length, status-code
  round-trip, and ack-versus-reply demultiplexing. These are property tests
  written against the recovered spec — **not** fixtures captured from a real
  device, which remains a Phase 1 item.
- **JSON-RPC envelope and the 32 typed request/response methods** compile and
  serialize to the wire field names recorded in Findings. — **met 2026-08-27.**
  `rpc` module: all 32 methods generated from a single table (so the serde
  renames and the wire-name lookup cannot drift), request/response envelopes,
  the typed `Status` result, device errors, an id allocator, and builders for
  every params shape in F13. The numeric-`jsonrpc` deviation (F12) is asserted
  by a test in both directions. 37 tests green.
- **`ringdown-ble` connects to the guitar over GATT**, negotiates MTU, writes
  to RX (`…4161`), subscribes to notifications on TX (`…4162`).
- **The spike CLI issues `GetStatus` and prints** `device`, `cpuID`,
  `battLeft`, `versionESP`, and `versionSTM` from the real instrument. This is
  the positive control: it promotes the whole static map from static-read to
  hardware-verified in one shot, per the repo's provenance rule.
- **`GetStatus` answers from the real instrument.** — **met 2026-08-27** (H4);
  the reply is pinned as a fixture in `rpc.rs`.
- **`ReadConfig` returns the live effect catalog**, and it is persisted as the
  authoritative fixture. This closes the "effect catalog lives in the vendor's
  Firebase, not the APK" gap (Findings, Gaps) by reading it from the guitar
  instead.
- **The two hardware unknowns are answered** and recorded in Findings: does the
  device require BLE bonding, and how does it advertise (name, service UUID in
  the advertisement)?
- **The connect sequence is confirmed against hardware** (F9), including that
  the version banner read (F10) returns what the static read predicts, and
  which of the two banner forms this guitar uses.
- **How an over-MTU response arrives is established** (F11's open question),
  and inbound handling is written to match what was captured rather than to a
  guess.

### Phase 2 — Full configuration parity

**Feature target:** everything the phone app can do, ringdown can do, from the
desktop.

Done-conditions:

- **Every method's `params` shape is bound** — the ~30 request classes mapped
  from the recovered parameter vocabulary (Findings F4). Mechanical, not hard.
- **The write commands are implemented and exercised:** bank CRUD
  (Add/Remove/Move/SwitchBank, SetBankName, SetGainBank), effect CRUD
  (Add/Update/Remove/MoveEffect), EQ (SetEQGain, SetEQBandGain), aux routing
  (AuxIn/AuxOut and their dry/wet), metronome (Start/Stop/Update), recording
  (Start/Stop), SustainKiller, and SetController.
- **A round-trip receipt per command family:** `ReadBank` → mutate → write →
  `ReadBank` reflects the change, captured as a replayable receipt in the
  retinue discipline.
- **Woodshed consumes ringdown:** git dependency wired, one view surface that
  connects and switches banks, proving the sibling-consumer topology end to
  end.

### Phase 2.5 — LLT2 (inserted 2026-08-27, discovered by hardware)

**Feature target:** ringdown speaks the transport this firmware actually uses,
so that any message larger than a single write works in both directions.

Not optional and not deferrable: every large read (`ReadConfig`, `ReadBank`)
and every large write (`SetConfig`, `AddEffect` with a full effect) needs it.
Self-contained, though — a codec with a known dictionary and no I/O, which
makes it sans-io work testable against fixtures before it meets the guitar.

Done-conditions:

- `JsonCompressor` encode and decode round-trip in Rust, dictionary-exact.
- A payload captured from the device decodes to valid JSON.
- LLT2 binary framing (transfer type, object id, little-endian index) chunks and
  reassembles; CRC32 matches on file transfers.
- `ReadConfig` returns the live configuration from the instrument.

### Phase 3 — Beyond the app

**Feature target:** the things the phone app cannot do — the reason to own the
protocol rather than rent it.

**The dictionary named the targets** (F15). Nineteen methods exist in firmware
that the vendor's app never calls, and the strongest of them is
`SetSpeakerBiquads` — raw biquad coefficients on the speaker path, i.e.
arbitrary filter design on the instrument, reachable without any firmware work.
`GetLevels`, `StartTuner`/`StopTuner`, and `StartAnalysis`/`GetAnalysis` are the
other immediate ones, and the tuner in particular already has a UI waiting for
it in woodshed.

Candidate scope (Decision D3 sets the actual target):

- Preset (bank) management as real files on the desktop: export, import,
  version, diff, share.
- Controller remapping via `SetController` beyond the app's partial exposure
  (`Control.source` binds a physical knob/pedal to a parameter with its own
  min/max scaling).
- Parameter automation, and driving the guitar's metronome/looper transport
  from woodshed's own.

Done-conditions: **deferred**, set once Phase 1 reveals what the DSP actually
implements.

### Phase 4 — Alternative firmware (SEPARATE OBJECTIVE — its own assessment required)

This phase is recorded, not planned. Per the workspace method, open-source
firmware is a **new objective** and gets its own Assess pass before any Assemble
or Action; it is not folded into this client project. What Phase 1 buys is the
instrumentation that makes that assessment possible — `GetStatus.cpuID` names
the exact silicon, `ReadConfig` enumerates what the current DSP does.

Known shape, and the honest blockers, so the future assessment starts informed:

- **Two processors.** An ESP32 handles connectivity (it owns the RPC layer);
  an **STM32 runs the audio DSP** (Findings F6). Mark's existing embedded work
  is esp-hal on ESP32-S3 (retinue) — that transfers to the *connectivity* half
  but not the audio half. embassy supports STM32 via `embassy-stm32`, but no
  existing driver applies.
- **Signing is unknown, not absent.** No crypto appears anywhere in the
  vendor's *app* protocol or transport code (Findings F7). That is the app's
  silence; firmware-signature verification would live on the device, where the
  app cannot see it. By the workspace rule, an absence is evidence only with a
  positive control in the same run, and there is none. A failed flash likely
  means opening the guitar to reach STM32 SWD pads.
- **Firmware ships as `.hef` over USB** with a user-accessible bootloader (hold
  center + tap power), per vendor docs — a documented, non-exotic entry point,
  format not yet mapped.
- **The FX bridge exists.** FAUST compiles to embedded C++ for STM32
  (Electrosmith's Daisy is an STM32H7 doing exactly this), and
  [Guitarix](https://guitarix.org/) (25+ amp/effect modules) was prototyped in
  FAUST. **Licensing caveat:** Guitarix is GPLv2, which would infect derived
  firmware and sits crosswise to this workspace's usual permissive posture; the
  FAUST standard libraries are more permissively licensed and are the better
  starting point.

Entry condition: a completed Phase 4 assessment doc with its own done-conditions
and Mark's sign-off. Nothing in Phases 0–3 depends on this phase.

---

## Findings — the recovered protocol (2026-08-27)

Source: static analysis (jadx 1.5.6) of the vendor Android app, package
`audio.hyvibe.app` v1.1.2 (versionCode 4022), APK SHA-256
`77fec6aa…2dc8c303`. **Confidence: static-read** for everything below until
Phase 1's positive control promotes it to hardware-verified. Class names are
cited for provenance only; no vendor source is reproduced here (expression
boundary). An interactive rendering of this map was produced during the
assessment; this section is the durable authoritative record.

The app's own code splits three ways: `com.xsquad.*` is a generic BLE
transport, `com.hyvibe.*` is the device protocol and domain model (the portable
part, no Android deps in the protocol path), and `net.weeteam.hyvibe.*` is
Android UI. Ringdown reimplements the middle layer.

**F1 — BLE surface** (`com.hyvibe.adapters.BLEConstantsKt`):

| Role | UUID | Direction |
|---|---|---|
| Guitar service | `eb65b6c6-fec3-4ed1-a6fc-9eff755a4160` | — |
| RX — request | `eb65b6c6-fec3-4ed1-a6fc-9eff755a4161` | client writes → |
| TX — response | `eb65b6c6-fec3-4ed1-a6fc-9eff755a4162` | ← notifications |

RX is the write characteristic, TX is the notify characteristic
(`BLEConnectableDevice`). There is a second, app-side service
(`d6db7b7f-…`) with a Nordic-UART-style characteristic (`6E400003-…`); it is
not the guitar-control path and is out of scope until Phase 1 says otherwise.

**F2 — MTU / write length.** On connect the client requests MTU 517; usable
write length is `mtu − 3`, i.e. **514 bytes** on success, falling back to **20**
if the peer refuses (`BLEConnectableDevice`: `requestMtu(517)`,
`maxWriteLength = mtu − 3`, default 20). This value is the input to the
chunking decision in F5, so it is tracked at runtime, never assumed.

**F3 — Transport stack, four layers.**
1. BLE GATT (F1) — one service, write + notify.
2. **LLT framing** — chunks any RPC message larger than the write length into
   acknowledged, sequenced pieces; bypassed when a message already fits.
3. **JSON-RPC 2.0** — standard envelope, wire fields `jsonrpc`, `id`, `method`,
   `params`, `result` (`com.hyvibe.models.RPC.*`). 10-second default request
   timeout (`LLTManager`).
4. Domain model (F4).

LLT frame fields (`LLTRequest`, `@SerialName`): `oid` (object id — mirrors the
JSON-RPC `id`), `mid` (chunk sequence, 1-based), `d` (escaped payload slice),
`s` (optional total unescaped length), `n` (optional filename, file transfer
only). Frames are newline-terminated; payload is backslash/quote-escaped. LLT
status codes (`LLTCode`): `0 LLT_ABORT, 1 LLT_OK, 2 LLT_DONE, 3 LLT_TIMEOUT,
4 LLT_OVERLOAD, 5 LLT_MALFORMED, 6 LLT_MISSING_CHUNK, 7 LLT_WRONG_MID,
8 LLT_WRONG_OID`. The device ACKs each chunk with a code before the next is
sent; `LLT_OK` means continue.

**F4 — The 32 RPC methods** (`RPCRequestType`, wire names exact):

- System: `GetStatus`, `GetVersion`, `SetDate`, `Calibrate`
- Config: `ReadConfig`, `SaveConfig`, `SetConfig`
- Banks/presets: `ReadBank`, `SwitchBank`, `AddBank`, `RemoveBank`, `MoveBank`,
  `SetBankName`, `SetGainBank`
- Effects: `AddEffect`, `UpdateEffect`, `RemoveEffect`, `MoveEffect`,
  `SetController`
- EQ: `SetEQGain`, `SetEQBandGain`
- Aux routing: `AuxIn`, `AuxInDryWet`, `AuxOut`, `AuxOutDryWet`
- Metronome: `StartMetronome`, `StopMetronome`, `UpdateMetronome`
- Recording: `StartRecording`, `StopRecording`
- Other: `SustainKiller`, `GetFileInfo`

Recovered param vocabulary (from `ConnectableDeviceCommunicator`) includes
`bankNumber`, `effectNumber`, `bandNumber`, `gainValue`, `knobValue`, `bpm`,
`num`/`den`/`nbBars`, `parameter`, `options`, `source`, and date fields — the
per-method binding is the mechanical Phase 2 pass.

**F5 — LLT chunk sizing is iterative, not arithmetic.** The app shrinks each
chunk in a loop until the fully serialized frame fits the write length, because
escaping changes length unpredictably (`LLTManager.sendMessage`). A client that
computes slice size once from an average overflows the MTU on any payload
containing quotes or backslashes. The Phase 1 fixture set must include such a
payload.

**F6 — Two processors.** `Status` (`com.hyvibe.models.Status`) reports
`versionESP` and `versionSTM` separately, plus `device`, `cpuID`, `battLeft`,
`freeSpaceGb`, `freeSpacePct`. Interpretation: ESP32 for connectivity, STM32 for
the audio DSP; the separate firmware versions and the file-transfer machinery
are consistent with two independently-flashed images. Hardware-verify in
Phase 1.

**F7 — No authentication in the protocol.** No pairing token, challenge, or
crypto anywhere in `com.hyvibe.*` or `com.xsquad.*`. The Firebase account the
app requires is for preset sync and effect-definition download, not for talking
to the guitar. A client that can reach the GATT service can issue any command.
(This is the app-side view; whether the device gates anything is a Phase 1
question.)

**F8 — DSP domain model.**
- `Effect` = `{ type, preset, bypass, params[] }`
- `Parameter` = `{ key, value: float, control }`
- `ParamDefinition` = `{ key, min, max, defaultValue, precision, unit,
  scale_func, scale_fact, control }` — the full typed definition of a knob,
  including the named function mapping control position to DSP value.
- `Control` = `{ min, max, source }` — binds a physical control to a parameter.
- `EffectDefinition` = `{ name, version, file, guitar_min_version, params[] }`.
- `Bank` = an ordered chain of effects (a preset).

**F12 — `jsonrpc` is a NUMBER, not the specified string.** This is the single
most consequential deviation found, and a client that gets it "right" gets it
wrong. `RPCRequestKt` declares `JSONRPC_VERSION = 2.0f` — a float — and
`RPCRequest$$serializer` emits it with `encodeFloatElement`. The wire therefore
carries:

```json
{"jsonrpc":2.0,"id":1,"method":"GetStatus","params":{}}
```

JSON-RPC 2.0 §4 requires `"jsonrpc": "2.0"` as a **string**. This device does
not implement that. Any client built on a conforming JSON-RPC library will emit
the string form and must be made to emit a number instead. Ringdown matches the
device, and says so at [`rpc::JSONRPC_VERSION`] so nobody later "fixes" it.

Related: request ids are `int` outbound but the **response** envelope types
`id` as a *float* (`RPCResponse`, and likewise `RPCError.code`). Ringdown
parses ids as `f64` and narrows, so `3` and `3.0` are the same id and a
fractional id matches nothing.

**F13 — Params wire keys are terse and irregular**, recovered from the
`*Params` classes' `@SerialName` annotations. They do not follow one
convention, so they are worth having in one place:

| Method family | Keys |
|---|---|
| Bank selection | `bank_num` |
| Bank rename | `bank_num`, `name` |
| Bank gain | `bank_num`, `gain` |
| Bank reorder | `src`, `dst` |
| Effect reorder | `bank_num`, `effect_num`, `effect_dest` |
| Effect add/update | `bank_num`, `effect`, `effect_num` |
| Equalizer | `gain`, `band` |
| Controller binding | `bank_num`, `effect_num`, `parameter`, `source`, `min`, `max` |
| Sustain killer | `bank_num`, `killed`, `reset` |
| Recording | `free` |
| Metronome | `bpm`, `num`, `den`, `bars` |
| Aux toggles | `toggle` / `value` |
| File info | `name` |

Note the inconsistency worth not smoothing over: bank reorder uses `src`/`dst`
while effect reorder uses `effect_num`/`effect_dest`. `Status` likewise uses
snake case (`batt_left`, `cpu_id`, `free_gb`, `free_pct`, `version_esp`,
`version_stm`) where the RPC envelope does not.

**F9 — The connection sequence, in order.** From `BLEConnectableDevice`'s GATT
callbacks, `onServicesDiscovered` onward:

1. Discover the guitar service (F1).
2. Set the **request** characteristic's write type to Android's `2`
   (`WRITE_TYPE_DEFAULT`) — i.e. **write with response**, not
   write-without-response. Consistent with a device that acknowledges every
   frame; a client must not "optimise" this to no-response.
3. Enable notifications on the **response** characteristic.
4. **Read** the response characteristic once — this returns the version banner
   (F10), not a JSON message.
5. `requestMtu(517)`.
6. Signal ready; RPC may begin.

**F10 — A version banner precedes RPC, and it is not JSON.** The connect-time
read of the response characteristic returns one of two plain-text forms:

- `S<stm>_E<esp>\n` — e.g. `S1.2.3_E2.7.0`; both processors.
- `@version <esp>\n` — older firmware, ESP only; the vendor's client then
  **assumes** an STM version of `1.2.2` rather than reading one.

This is distinct from, and earlier than, the `GetVersion` RPC method. Ringdown
parses both forms and flags whether the STM version was reported or assumed
(`handshake::Banner::stm_was_implied`), because an inherited assumption should
not be indistinguishable from a device-reported fact.

**F11 — Inbound has no reassembly, and the response characteristic is
multiplexed.** `receiveMessage` UTF-8-decodes each notification and fans it
straight out to listeners with **no buffering and no reassembly**. Consequences:

- Every notification is expected to be a complete message on its own.
- The same characteristic carries *both* LLT acknowledgements and complete
  JSON-RPC replies, so a client demultiplexes by shape — try to parse an ack,
  and treat a non-match as a JSON-RPC reply rather than as an error. Ringdown
  does this in `llt::Ack::parse`, which returns `Option` for exactly this
  reason.
- **Open question for Phase 1:** how a response larger than the MTU arrives,
  given the vendor's client cannot reassemble one. Either large replies are
  chunked by a mechanism not visible in the app, or `ReadConfig` returns
  something smaller than expected. Do not build inbound reassembly on
  speculation — capture a real `ReadConfig` first.

**Gaps (need the physical guitar, not more decompiling):**
- The effect *catalog* (which effects exist, with their `ParamDefinition`s)
  lives in the vendor's Firebase (`hyvibe_factory`), not the APK. Recovered
  instead from the instrument via `ReadConfig` in Phase 1.
- BLE bonding requirement and advertisement format — Phase 1.
- File-transfer / firmware-upload wire path (`sendFile`, `.part`) — unmapped,
  and deliberately out of scope for the client.

**Provenance caveat.** The APK's signing certificate carries the generic
placeholder subject `CN=Android, O=Google Inc.` rather than a HyVibe developer
key, so the signature does not independently attest authorship. Nothing in the
decompiled code looks tampered with and the 2019 validity start fits the app's
era, but this build was only ever read, never run. The protocol is a hypothesis
until Phase 1's control run.

---

## Findings — the first hardware run (2026-08-27)

First contact with a real instrument. **Confidence: hardware-verified** for
everything in this section, which supersedes the static-read confidence of the
corresponding items above.

Reference instrument, from its own System Menu (an independent second source,
read off the device before the run so it could not be rationalised after):

| Field | Value |
|---|---|
| Guitar (STM, audio DSP) | V1.2.3 |
| Wless (ESP, connectivity) | V1.3.0 |
| BT ID | H2-CC340 |
| Model | R1C0M8 |

**H1 — F1 CONFIRMED. The GATT surface is exactly as recovered.** The device
advertises `eb65b6c6-…4160` *in the advertisement itself*, and on connection
both the request and response characteristics are present at the recovered
UUIDs. Address `D4:8A:FC:93:4C:CA`, advertised name `H2-CC340`, matching the
System Menu's BT ID.

**H2 — F10 CONFIRMED, and it is the two-processor form.** The connect-time GATT
read of the response characteristic returned a banner parsing to **STM 1.2.3 /
ESP 1.3.0** — identical to the System Menu, from a completely different path.
`stm_was_implied` was false, so this device uses `S<stm>_E<esp>`, not the legacy
`@version` form. F6's two-processor interpretation is confirmed three ways now:
the `Status` model, the System Menu's separate "Guitar"/"Wless" rows, and this
banner.

**H3 — RPC IS UNANSWERED. `GetStatus` timed out after 10s.** Everything up to
the RPC round-trip works; the request is written without error and nothing comes
back. Narrowly scoped, and the most important open item.

What the teardown rules *out* as the cause, re-checked after the failure:

- The request shape is right. `ConnectableDeviceCommunicator` builds exactly
  `new RPCRequest(2.0f, getCounter(), GET_STATUS, MapsKt.emptyMap())`, and
  `RPCRequest.write$Self` encodes in the order jsonrpc, id, method, params, with
  `RPCRequestTypeSerializer.serialize` emitting `encodeString(getMethod())` — a
  plain method name. Ringdown's bytes match this.
- No newline is appended to an unsplit message. `sendMessage` writes
  `toUtf8Bytes(m)` verbatim; only LLT frames get `\n`.
- Replies do arrive as notifications: `onCharacteristicChanged` →
  `readCharacteristic` → `receiveMessage`.

**The most likely remaining cause is the MTU**, and the reason is structural.
The vendor's client calls `requestMtu(517)` *before* any RPC and treats the
result as load-bearing — `maxWriteLength` starts at 20 and is only raised in
`onMtuChanged`. btleplug exposes no MTU API at all (see `ASSUMED_WRITE_LEN`), so
ringdown cannot make that request. A 54-byte `GetStatus` sent over an
unnegotiated 23-byte MTU becomes a queued/long write, which the device's GATT
server may not accept as a single attribute value. Note the corollary: at a
20-byte write length the protocol is *unusable by construction*, since an LLT
frame's own wrapper exceeds it — which is precisely why the vendor negotiates
first. Untested, and the next thing to test.

**Instrument defect found and fixed in the same session.** The client discarded
every notification it did not recognise, so "the device said nothing" and "the
device said something unexpected" were indistinguishable — the two failures have
opposite fixes. `TransportError::Timeout` now carries everything overheard, and
`ringdown-probe --diagnose` tries each candidate encoding in turn (numeric vs
string `jsonrpc`, with/without response, newline-terminated, no `params`) and
first checks whether the device ever speaks unprompted at all.

**H4 — RPC CONFIRMED. `GetStatus` round-trips, and H3 was our bug.** The
diagnostic run answered on **all five** candidate encodings, including the
original. The request had been correct from the start; the client was failing to
parse the reply and silently discarding it. Byte-exact captured reply, now
pinned as a test fixture in `rpc.rs`:

```json
{"jsonrpc":"2.0","id":90,"result":{"free_gb":7.634,"free_pct":0.9949,
 "batt_left":46,"version_stm":"V1.2.3","version_esp":"V1.3.0",
 "cpu_id":"PIdXXddxLAU=","device":"H2S"}}\n
```

Four corrections follow, and one of them retracts an earlier finding:

- **F12 is half wrong, and the wrong half was the emphatic half.** The device
  accepts `jsonrpc` as a number *or* as the spec's string, and answers both
  identically — so the earlier claim that a spec-compliant client "is writing
  JSON-RPC at something that does not speak it" is **false**. What is actually
  fixed is the *reply*: it always arrives with `"jsonrpc":"2.0"` as a **string**.
  The device is lenient inbound and conventional outbound. Ringdown still sends
  the numeric form to match the vendor, but as the conservative choice rather
  than a necessity.
- **That asymmetry was the whole bug.** `Response.version` was typed `f32`, a
  string would not deserialize into it, `Response::decode` errored, and the
  receive loop discarded the answer as unrecognised. Assuming a reply mirrors
  its request is the mistake; the field now accepts either.
- **`batt_left` and `free_pct` use different scales.** `46` alongside `0.9949`:
  battery is 0–100, free space is a 0–1 fraction. Renamed to
  `battery_percent` and `free_space_fraction` so the difference is visible at
  every call site instead of living in a comment.
- **Version strings carry a `V`** (`"V1.2.3"`), vindicating the permissive
  `Version::parse`. A stricter parser would have rejected the device's own
  spelling.

Also observed: `cpu_id` is base64 (`PIdXXddxLAU=`, 8 bytes decoded — consistent
with an ESP32 MAC-derived id), `device` is `"H2S"` where the System Menu shows
model `R1C0M8`, so those are different identifiers. Replies are
newline-terminated even when unsplit, though the vendor's client does not
require that on the way out.

**H5 — The guitar recognises ringdown as an app client.** Mark observed the
front-panel mobile-app indicator light up while the probe was connected — the
same icon the manual documents for "mobile app connection". The instrument
treats this as a legitimate app session, not merely an anonymous GATT
connection.

**Still open after this run:** how an over-MTU response arrives (F11), whether
bonding is ever required, and the effect catalog itself — all reachable now via
`--config`, since `ReadConfig` is the first request likely to exceed the MTU in
both directions.

**H21 — The params shapes are bound for all 32 methods, and binding them found
an absent optional being sent as `null`.** (2026-08-28, desk work.)

`rpc::param_shape` declares every method's `params` keys and which are
required, read from the vendor's `*Params` classes (F13). It is a `match`
without a wildcard arm, so a method added to the table cannot compile until its
shape is declared — the same anti-drift discipline that generates the wire
names. A test then checks each `params::*` constructor against its declaration:
every key emitted must be declared, and every required key must be emitted.
That is what makes the table load-bearing rather than decorative.

**One method is `Unrecovered` rather than described: `SetConfig`.** It plainly
takes a configuration, and the call whose reply would show its shape is
`ReadConfig`, which wedges the firmware (H18). Recording that as a distinct
state matters, because "takes nothing" and "we do not know what it takes" look
identical at a call site and are opposite facts.

**The gap the pass found.** `params::metronome(bpm, num, None, None)` was
building its object with `json!`, which renders an absent `Option` as `null`.
So every metronome write sent **`"den": null`** — including both writes in the
W2 hardware run — while `woodshed-instrument` documented itself as never
writing `den`. The stated invariant and the wire disagreed. No harm resulted;
the reference instrument was in 5/4 before and after, as its owner confirmed.
But omission and `null` are different messages — one says "leave this alone",
the other says "set it to nothing" — and the intended one was never being sent.
`params` now builds objects by insertion and leaves absent optionals out, which
also fixes `set_controller`'s `min`/`max` and `sustain_killer`'s
`killed`/`reset`.

Worth noting how it was found: not by testing, but by writing down what each
method's parameters *are* and comparing that to what the code emits. The
declaration was the instrument.

**H38 — `AddBank` inserts and shifts, and the bank it makes is audio-dead.**
(2026-09-01, read off the panel by the owner.)

`AddBank {bank_num: 4, bank: {...}}` sent to a nine-tile profile **inserted** a
bank at index 4 and pushed every later bank along one place: Tremolo 4→5,
Octaver 5→6, Disto 6→7, Boost 7→8, and the ninth — the tile named "ringdown" —
**off the end of the list entirely.** The method is literal: it *adds* a bank
to an ordered list, it does not replace the one at that index.

The bank it creates is real to the panel and dead to the DSP:

- It takes the `name` from the bank object and shows it on the tile.
- It occupies an index and displaces its neighbours.
- **Neither its own `effects` chain nor effects added to it afterwards
  render.** Slot 8 had the same signature for the same reason — it received an
  `AddBank` too.

So a client can *edit* banks the app created (H36: `AddEffect` into a real bank
is audible) but cannot *create* a playable one. Creating a bank appears to
need something `AddBank` alone does not supply.

**Two retractions against myself, both from this sequence.** I reported that
`AddBank` "poisons" a bank, and that bank 4 no longer accepted working effects.
Neither was true: bank 4's Tremolo was **displaced to index 5**, and my
follow-up `AddEffect` went into the freshly-inserted octave bank rather than
the Tremolo one. Every `bank_num` I used after the insert was off by one, and I
read the resulting silence as damage.

**The operational rule this yields is sharp: `AddBank` renumbers everything
after it.** A client holding bank indices across an `AddBank` is holding stale
indices, and on a full profile it will silently push the last bank out of
existence. Treat it as destructive to the *profile*, not to a bank — and note
that `ReadBank` gives no way to re-read the layout afterwards, so only the
player can see what happened.

**H37 — Bounds of the boon: `bypass` works, no render cap at four, and a bank
must be re-selected for edits to settle.** (2026-09-01, on bank 4 — a real
factory bank — using the octave-down Pitch as an unmaskable oracle.)

- **`bypass: true` silences an effect.** Bank 4 drained to exactly one bypassed
  `Pitch {Shift: -12}`: dry, no octave. Then four bypassed effects (Chorus,
  Phaser, Reverb, Gate) alongside one live Pitch: only the octave. So it is a
  real toggle and a client can A/B an effect without removing it. This
  **supersedes** the retraction in H34/H35 — that retraction was correct at the
  time, since the evidence then came from the phantom slot; the behaviour is
  now established on a real bank with a single variable.
- **The DSP renders past four.** With four bypassed effects ahead of it, an
  audible Pitch in **fifth position** sounds. Six effects added to a real bank
  and six drained back out, so storage is at least six too. **The four-effect
  limit is the vendor app's UI convention, not a firmware constraint** — a
  client may exceed it, which is a capability the vendor's own app does not
  expose. (H31's "no cap" claim was retracted in H35 for measuring the phantom
  slot; the conclusion happens to be right, but only this test earns it.)
- ~~**Edits do not settle until the bank is re-selected.**~~ **Downgraded, not
  established.** After the five-effect write the owner heard the octave "and
  definitely more", and switching off the tile and back left exactly the
  octave — but on reflection the owner reports hearing edits land *live* all
  session without switching, which is the larger sample. One transient with at
  least two competing explanations (DSP settling lag, monitoring volume) does
  not support a rule, and it was written up as one on a single observation.
  What stands: an unexplained transient after a multi-effect write. Whether a
  re-select is ever needed is **untested**.

**A correction against myself in the same batch.** From a drain that counted
three where I expected two, I concluded — and told the owner — that
`RemoveEffect` ignores a non-zero index. A count-only test disproved it
immediately: three added, one removed at index 1, two left. `RemoveEffect`
honours any index. The real cause was arithmetic: **the factory Tremolo bank
held two effects, not one**, so index 1 was a factory effect rather than the
Pitch I meant to remove — which is also why the octave survived the first
bypass attempt. The lesson is narrow and repeatable: *count the bank before
indexing into it*, because `ReadBank` will not tell you and an assumed length
turns every index into a guess.

**H36 — `AddEffect` audibly modifies a real bank. Proven, and it explains the
phantom.** (2026-09-01, by ear, orthogonal effect, factory bank.)

`Pitch {Shift: -12}` added to **bank 4** — the factory *Tremolo* tile — and the
owner selected it and played: **tremolo plus an octave below.** An octave down
cannot be masked by amplitude modulation and nothing else in that bank touches
pitch, so this is the clean audible confirmation the session had been missing.

**What it restores.** H35 downgraded "effects reach the audio path" because the
only supporting event was ambiguous. It is now established — but on *new*
evidence, not by rehabilitating the old: a chosen-orthogonal effect, a bank
known to be real, and selection driven by the owner rather than by a
`SwitchBank` that could move things underneath. The content-sharing path is
real: a client can put a specific effect, with parameters, into a player's bank
and they will hear it.

**And it explains slot 8.** The owner's framing settled it — the app separates
*Change* (swap which bank occupies a tile) from *Modify* (edit a bank's chain,
up to four). Slot 8 was Factory Standard's empty `+`. `SetBankName` gave it a
name and `AddEffect` stored a chain, both answering `true`, but **no bank
record was ever created there**, so the DSP had nothing to load. A name is not
a bank, and a stored chain is not a bank either. To fill an empty tile a client
must `AddBank` first; `AddEffect` alone writes into a gap.

That is the sixth and last hypothesis for the silence, after: wrong parameter
values, `UpdateEffect` not applying, `SaveConfig` not committing, delay units,
and chain overflow. Each was tested and each was wrong. The one that held came
from the owner's knowledge of the app's own verbs, not from the protocol.

**Now testable on real banks, and all still open:** whether `bypass` silences
an effect; a real bank's chain capacity (the app enforces four; the 34 counted
in slot 8 measured a phantom and proves nothing); and `AddBank`'s wire shape,
which is what a client needs to create a bank rather than only edit one.

**H35 — What the phantom slot invalidates, and what survives it.**
(2026-09-01, prompted by the owner's domain pushback.)

H34 established slot 8 is not a real bank. Nearly every effect *audibility* and
*capacity* claim this session was measured there, so they fall together:

- **"The firmware does not cap a chain at four" (H31) is RETRACTED.** The
  thirty-four-deep count was `AddEffect` into slot 8, a phantom that is not a
  playable bank. It proves nothing about a real bank's capacity. The app
  enforces four, and four is now the best estimate for the firmware too;
  real-bank capacity is **untested**.
- **`bypass` behaviour is unknown** (as H34 said).
- **`AddEffect` inserting an *audible* effect into a real bank is unconfirmed.**
  Re-examined, the one "audible" event (H26/H29) was `SwitchBank 0` followed by
  `AddEffect Distortion`, and the owner heard "tons of reverb and feedback" —
  the *reverb* is bank 0's own pre-existing effect, made live by the switch.
  The added distortion's contribution was never isolated. So "effects reach the
  audio path" is downgraded to: **switching to a bank makes its stored chain
  audible; whether our `AddEffect` adds to that chain audibly is not shown.**

What genuinely survives, because none of it depends on audibility or on slot 8:

- The transport, codec, framing, file transfer (all hardware-verified earlier).
- Metronome read; `UpdateMetronome` bpm/num (panel-confirmed); `den` not
  writable; `StartMetronome` (started with no hand on the guitar).
- `SwitchBank` moves the panel selection (panel-confirmed).
- **The `AddEffect` parse oracle and the parameter vocabulary (H28–H31).**
  These rest on `true`/`false` *validation*, which is independent of routing:
  the firmware refuses `Chorus.Gain` and accepts `Distortion.Gain` whether or
  not the slot ever plays. The thirteen types and their keys stand.

The through-line of the day's errors: `true` means parsed (H27), and this
session repeatedly read it as *done* — stored, audible, capped, bypassed. It is
only ever parsed. Audibility needs a bank known to be routed and an effect
chosen not to mask what is already there; neither was controlled until the
owner said so.

**H34 — Slot 8 is not in the audio path. Everything added there all session
was stored but never played.** (2026-09-01, decisive by ear.)

A default `Distortion` — the effect that fed back on a G in bank 0 earlier
today — was added to slot 8 with `bypass: false`, the owner selected the tile
(panel-named "ringdown") and played a G at volume: **no feedback, no sound.**
In bank 0 that same effect is unmistakable. So slot 8, the tile that had been
Factory Standard's empty `+`, is not routed to the DSP.

The protocol never lied about *storing*: `AddEffect` answered `true`,
`RemoveEffect` counted the chain 34 deep (H31). But **stored is not audible**,
and slot 8 was where nearly every effect experiment this session was run.
Confirmed-audible effects only ever happened in **bank 0** (H26/H29 — the
Distortion that fed back). Those stand. What falls:

- **The `bypass` conclusion (H31's close) is retracted.** "Sounds like one,
  not a wall" for a ten-deep bypassed chain was taken as proof `bypass`
  silences an effect. That chain was in slot 8, which is silent regardless, so
  the observation was about some other live bank and proves nothing about
  `bypass`. Whether `bypass` works is now **unknown**.
- Every "is this audible" question answered against slot 8 is void. The
  vocabulary table (H31) is **not** affected: it rests on the `true`/`false`
  parse oracle, which is independent of routing — `Chorus.Gain` is refused and
  `Distortion.Gain` accepted whether or not the slot ever plays.

**Why slot 8 is silent is the open question.** The leading hypothesis: the
empty `+` slot is not a committed bank, and `AddEffect` to `bank_num: 8` fills
a working copy the DSP does not play until it is committed — `SaveConfig`, or
`AddBank` to actually create a bank in the slot. Untested. It would explain
every silent slot-8 result at a stroke, and would mean **`SaveConfig` is a
required step this project has never exercised.**

The methodological lesson, again and sharper: the audible oracle (feedback on
a G, H33) is only meaningful in a bank known to be routed. Slot choice is a
variable, and it was never controlled — bank 0 was audible, slot 8 was not,
and the difference was invisible because `AddEffect` says `true` either way.

**H33 — `SetBankName` takes only on a populated slot; an unnamed populated
slot shows its neighbour's name.** (2026-09-01, three panel reads by the owner.)

| slot 8 when renamed | reply | panel afterwards |
|---|---|---|
| holding effects (first session) | `true` | **ringdown** |
| empty, after the app's reset — twice | `true` | nothing; stays the `+` tile |
| holding one effect, unnamed | — | **boost** (tile 8's name) |
| holding one effect, then renamed | `true` | **ringdown** |

So the rename is honoured only when the slot has a chain, and refused
silently — `true` — when it does not (H27 again). Between the two, a slot that
has content but no name renders the name of the slot before it, which is a
panel fallthrough rather than a Boost chain: the slot held one bypassed
Tremolo and nothing else.

For a client the order matters: **add the first effect, then name the bank**,
and treat a rename sent to an empty slot as dropped. The panel is the only
read-back for the name, since `ReadBank` never reports anything.

Closed by ear afterwards, with no commands in flight: the renamed slot, holding
only its one bypassed Tremolo, sounds like nothing at all — so "boost" was the
label alone and `bypass` holds. The owner's method for that is worth keeping
as a **repeatable audible oracle**: on this instrument a G played at full
volume feeds back strongly through Boost or a live Tremolo and not at all
through an empty or fully-bypassed chain. Feedback onset on one note is a
binary, sensitive test of whether a chain is live, and a far better one than
"does it sound different" — which is what earlier bypass tests were reduced to.

One caution the same session produced: a `SwitchBank` sent while the owner is
comparing tiles by ear voids the comparison, because it moves the selection
under them. Panel-redraw switches should be sent only when the owner is not
listening for a difference.

**H32 — The vendor app overwrites the instrument's configuration when it
connects. Ringdown's writes are volatile while the app is in use.** (2026-09-01,
observed twice by the owner.)

Twice today a state this client had written — a chain of effects in slot 8,
and later the slot's name, which the owner had read as `ringdown` on the
panel — was gone after the owner opened the vendor app. The app's own words
for it: it "cleared the caches and reset the bank to the factory default
config". It does not read the instrument's state on connect; it pushes the
profile it holds.

That is consistent with everything else recorded about the app: it never calls
`ReadBank` or `ReadConfig` (H7, H19), keeps presets in Firebase and local
storage, and treats the instrument as a sink. So **the app is authoritative
by construction**, and any write from another client survives exactly until
the app next connects.

Consequences for any client built on this crate:

- **Writes are not durable against the app.** A user who edits from the
  desktop and then opens the phone app loses the desktop's changes without
  warning, and the app will not know they existed. A client should say so —
  "changes here last until the HyVibe app connects" — rather than imply
  persistence.
- **A separate profile does not help**, since the reset re-pushes the whole
  grid, including the empty slot that had been renamed.
- **The safe workflow is one client at a time**, and the instrument already
  enforces one *connection* at a time (H-series, `Session::release`). What it
  does not enforce is one *authority* at a time, and that is the gap.
- **Re-sending is cheap.** A client that keeps its own record of what it
  wrote can re-push after the app has been and gone; the `RemoveEffect`
  count (H31) tells it whether the chain it expects is still there.

Confirmed by re-sending `SetBankName(8, "ringdown")` after the wipe: `true`,
and pending the owner's second read of the panel.

**H31 — The wire vocabulary, verified per effect; no firmware chain cap; and
a counting read-back.** (2026-09-01, ~120 calls against H2-CC340 on slot 8,
`GetStatus` control after every batch.)

**Validation is per effect, not against one global list.** The control that
proves it: `Chorus.Gain`, `Reverb.Attack`, `Highpass.DryWet` and
`Notch.Feedback` — each a key some *other* effect accepts — are all refused.
So an accepted key below belongs to that effect specifically.

| Effect | Wire keys (accepted) | Knob it names |
|---|---|---|
| Chorus | `Frequency`, `DryWet` | FREQ, DRY/WET |
| Compressor | `Attack`, `Release`, `Threshold`, `Ratio`, `DryGain`, `WetGain` | as labelled |
| Delay | `DelayTime`, `Sync`, `Lowpass`, `Highpass`, `Feedback`, `DryWet` | TIME, SYNC, LP, HP, FEEDBACK, DRY/WET |
| Distortion | `Gain`, `Volume`, `Lowpass`, `Highpass` | GAIN, VOL, LP, HP |
| Equalizer | `GainBand1` … `GainBand7`, `Gain` | the seven sliders (`GainBand8`+ refused) |
| Gate | `Threshold`, `Range`, `Release`, `Attack` | as labelled |
| Highpass | `Frequency`, `Q` | FREQ, Q |
| Lowpass | `Frequency` (+ `Q` by pattern, untested) | FREQ, Q |
| Notch | `Frequency`, `Q` | FREQ, Q |
| Phaser | `Frequency`, `Feedback`, `DryWet` | FREQ, FEEDBACK, DRY/WET |
| Pitch | `Shift` | SHIFT |
| Reverb | `Decay`, `DryWet` | DECAY, DRY/WET |
| Tremolo | `LFO` | FREQ — found on the 23rd candidate; `Frequency` is refused |

Refused guesses worth keeping so nobody re-tries them: `Time`, `Bpm`, `Beat`,
`Division`, `Note`, `NoteValue`, `Subdivision`, `Tempo`, `Slider`, `Beats`,
`Ratio` for Delay's note-value knob (**still unnamed**; it is the SYNC
subdivision and greyed unless SYNC is on); `Pitch` for Pitch; and for Tremolo
`Frequency`, `Rate`, `Speed`, `Slider`, `amp`, `Freq`, `Tremolo`, `DryWet`,
`Depth`, `Gain`, `Q`, `Volume`, `Shift`, `Decay`, `DelayTime`, `Feedback`,
`Sync`, `Pitch`, `GainBand1`, `Period`, `Modulation`, `Wet` — twenty-two
refusals before **`LFO`** was accepted. It is one of the dictionary's
"effect type" strings (F15) that never was a type; it is Tremolo's rate.

For Delay's note-value knob, eleven more were refused after the list above:
`Decay`, `Frequency`, `Gain`, `Q`, `Shift`, `Pitch`, `amp`, `GainBand1`,
`SyncTime`, `SyncDivision`, `Fraction`, `Duration`, `Length`, `DelayBeat`.
The owner describes the knob as switching the echo from a fixed time to a
fraction of the metronome's beat, adjusting with its BPM — so the surviving
hypothesis is that with `Sync: 1` the note fraction is carried by `DelayTime`
in beats. The oracle cannot test that, since values are lenient; only ears
against a running metronome can.

Two batches of ~25 calls each ended in a Windows BLE drop — `HRESULT
0x80000013, "The object has been closed"`, the same error seen once during the
calibration probes. `GetStatus` answered on the next connection both times, so
this is the host stack releasing the handle, not the firmware wedging; a long
session should expect it and resume.

**The firmware does not cap a chain at four.** Six adds then removal at
index 0: six `true`s and a `false`. Later, after the sweeps, forty-five
removals: **thirty-four `true`s then `false`** — exactly the number of
`AddEffect` calls that had answered `true` since the app last reset the slot.
Two consequences:

- The four-effect limit is the app's editor, not the instrument. A client
  that ignores it can chain more; whether the DSP has a practical ceiling is
  a different question and untested.
- **`RemoveEffect` answers `false` on an empty chain**, which makes
  remove-until-`false` a *count* of what the chain holds — the first
  read-back this protocol offers for bank contents, since `ReadBank` never
  says anything. And 34 for 34 is the first method here whose `true` is
  demonstrated to mean **stored** rather than merely parsed (contrast H27).

Also settled by the count: every `bypass: true` effect was really in the
chain, so the owner's "sounds like one, not a wall" verdict on a chain of
ten is a clean confirmation that `bypass` silences an effect.

**H30 — Every effect's knobs, read off the app's editors.** (2026-09-01, the
owner's screenshots of all thirteen editor screens at `Preset: default`.)

This is the parameter vocabulary the app fetches from Firebase and the APK
never contained. Labels are as displayed; defaults are the `default` preset.

| Effect | Knobs (label: default) |
|---|---|
| Chorus | FREQ 3.0 Hz · DRY/WET 100 % |
| Compressor | ATTACK 5.1 ms · RELEASE 55 ms · THRESHOLD −16.0 dB · RATIO 3.5:1 · DRY GAIN −100 dB · WET GAIN −8 dB |
| Delay | SYNC (off) · TIME 450 ms · BPM 3/4 beat · LP 2.5 kHz · HP 52 Hz · FEEDBACK 34 % · DRY/WET 100 % |
| Distortion | GAIN 50.0 dB · VOL −25.0 dB · LP 1.8 kHz · HP 94 Hz |
| Equalizer | 100 · 200 · 400 · 600 · 1k · 2k · GAIN (seven sliders, 0–1) |
| Gate | THRESHOLD −60 dB · RANGE −89 dB · RELEASE 200 ms · ATTACK 1.1 ms |
| Highpass | FREQ 140 Hz · Q 0.7 |
| Lowpass | FREQ 3.0 kHz · Q 0.7 |
| Notch | FREQ 20 Hz · Q 1 |
| Phaser | FREQ 0.4 Hz · FEEDBACK 68 % · DRY/WET 100 % |
| Pitch | SHIFT −12 st |
| Reverb | DECAY 25 % · DRY/WET 100 % |
| Tremolo | FREQ 3.8 Hz |

Three things the table settles before any wire test:

- **The v1.1.2 keyword dictionary is the older half of this vocabulary.** Its
  parameter names — `Frequency`, `DryWet`, `DelayTime`, `Feedback`, `Sync`,
  `Q`, `Decay`, `Shift`, `Gain`, `GainBand1`–`6` — are these knobs for the
  effects that existed then. `Compressor` and `Gate` have no dictionary
  entries because they postdate that app, which is why their names had to
  come from the screen.
- **Labels abbreviate wire keys** (H29: `VOL` → `Volume`, `LP` → `Lowpass`),
  so the table is the *display* layer; the wire layer is established per key
  by the `AddEffect` oracle, which refuses an unknown key reliably.
- **A chain holds four effects.** The owner's own two test banks show four
  rows each and no "Add an effect" after the fourth. Whether the firmware
  enforces the same cap or silently drops a fifth is a separate question
  (H27), testable without ears: `RemoveEffect` at index 0 until it answers
  `false` counts what the chain actually holds.

**H29 — Effect parameters transmit. H26's "structural" verdict was wrong; it
was vocabulary all along.** (2026-09-01, against H2-CC340 on the empty slot 8,
every call read back as `true`/`false`; `GetStatus` control after each batch.)

A Distortion with all four of its knobs set was accepted:

```json
{"preset":"default","type":"Distortion","bypass":true,
 "params":[{"key":"Gain","value":50.0},{"key":"Volume","value":-25.0},
           {"key":"Lowpass","value":1800.0},{"key":"Highpass","value":94.0}]}
```

**The rules, each established by a pair of writes that differ in one thing:**

- **Keys are the knob's full word, not its label.** The app shows `GAIN`,
  `VOL`, `LP`, `HP`; the firmware takes `Gain`, `Volume`, `Lowpass`,
  `Highpass`. `VOL`, `LP`, `HP`, `LPF` and `Cutoff` are all refused. `GAIN`
  passed only because that label *is* the word.
- **Keys are case-insensitive.** `GAIN`, `Gain` and `gain` all accepted.
- **Values are lenient.** `Gain: 0.5` accepted alongside `Gain: 50.0`;
  `Lowpass: 1800` accepted where the app displays `1.8 kHz`. No unit
  conversion, no range refusal seen.
- **One unknown key refuses the whole message.** `Gain` + `VOL` together is
  `false` although `Gain` alone is `true`. Validation is all-or-nothing.
- **Partial parameter lists are fine.** A single `Gain` is accepted; the
  firmware presumably fills the rest from the preset.

So H26's sweep of twelve dictionary names failed because none of them are
Distortion's parameters — `DryWet`, `Frequency`, `Decay` belong to other
effects, if to any. "Structural rather than vocabulary" was a wrong inference
from a sweep whose vocabulary was wrong. H28 supplied the labels; the
full-word rule was found here by trying `Volume` for `VOL`.

**The catalog prediction held:** `Compressor` and `Gate` both accepted, so
`AddEffect`'s type oracle is thirteen for thirteen.

**One retraction against myself:** a four-parameter message earlier drew
silence, which I read as the `ReadConfig` wedge signature. Retested
identically it returned a clean `false`, and `GetStatus` answered throughout.
It was a BLE hiccup; the wedge pattern is one message *and* every message
after it, and this was neither.

**Consequence for the content plan.** H26's line — "a practice item can carry
an effect but not a tone" — no longer holds. Type, preset and every parameter
travel in one `AddEffect`. What remains unknown is the parameter vocabulary of
the other twelve effect types, which the app's editor screens will give by the
same label-to-word rule, and whether `bypass: true` actually silences an
effect — slot 8 now carries one live Distortion and several bypassed ones, and
only the owner's ears can say if that sounds like one or like nine.

**H28 — The vendor app's own screens, read directly. The data model, the real
effect catalog, and three of our conclusions overturned.** (2026-09-01, the
owner walking the app's UI and screenshotting it.)

Everything below is observation of the shipped app, not inference from its
decompiled code — which matters, because the app fetches its effect and preset
definitions from Firebase at runtime and that data was never obtainable from
the APK. The screens are the only source for it.

**The data model, three levels deep:**

```
Profile        a named set of nine bank slots  ("Factory Standard", "edit1")
  └ Bank       a named container: { name, gain (dB), sustain killer, chain[] }
      └ Effect { type, preset, bypass, params[] }   ordered, reorderable
```

Banks are a **library**, not just the nine on the grid: `12 String`, `Blakes B`,
`Comp dist`, `Crystals`, `Fantasy`, `Fusion`, `Megaphone`, `Overdrive`,
`Ping Pong`, `Rev`, `Vintage` and more sit alongside the placed ones, which the
picker greys out. A profile is an *assignment* of nine library banks to slots.

Every control maps onto a method already in the table, which is the strongest
confirmation the recovered surface is complete:

| App control | Method |
|---|---|
| bank name | `SetBankName(bank_num, name)` |
| bank **Gain** | `SetGainBank(bank_num, gain)` |
| **Sustain Killer** toggle + RESET | `SustainKiller(bank_num, killed, reset)` |
| "Add an effect" | `AddEffect(bank_num, effect)` |
| swipe-to-delete a chain row | `RemoveEffect(bank_num, effect_num)` |
| drag handles on chain rows | `MoveEffect(bank_num, effect_num, effect_dest)` |
| placing a library bank in a slot | `AddBank(bank_num, bank)` |
| selecting a grid slot | `SwitchBank(bank_num)` |

**Three findings this overturns:**

1. **The effect catalog is exactly thirteen, and closed.** Captured top and
   bottom with the scrollbar at each end: `Chorus`, `Compressor`, `Delay`,
   `Distortion`, `Equalizer`, `Gate`, `Highpass`, `Lowpass`, `Notch`,
   `Phaser`, `Pitch`, `Reverb`, `Tremolo`.

   H24b's eleven accepted types are **exactly these thirteen minus `Compressor`
   and `Gate`** — the two the keyword dictionary never carried, so the sweep
   could not have tried them. And the eight it rejected (`Boost`, `Volume`,
   `Echo`, `Octaver`, `Disto`, `DelaySync`, `LFO`, `None`) are precisely the
   ones absent from the app's list.

   **Zero false positives and zero false negatives on everything tested**,
   which promotes `AddEffect`'s type validation to a *reliable membership
   oracle* — worth distinguishing from the `true` it returns for `den`, which
   means nothing. The dictionary was the flawed input: eight junk names,
   two real ones missing. Prediction for the next hardware session, falsifiable:
   `Compressor` and `Gate` are both accepted.

2. **`preset` is lowercase `"default"`.** Every `AddEffect` this client has sent
   carried `"Default"`, capitalised, from the vendor's
   `PresetsActivity.DEFAULT_PRESET_NAME`. The app displays `default`. On
   firmware this literal about field order (H24), a capital letter is a
   candidate cause rather than a detail.

3. **The parameter vocabulary is per-effect and unit-bearing.** Distortion's
   knobs are **GAIN** (50.0 dB), **VOL** (−25.0 dB), **LP** (1.8 kHz) and
   **HP** (94 Hz). None of those are the dictionary keys swept in H26
   (`DryWet`, `Frequency`, `Decay`, …). That sweep refused all twelve because
   all twelve were from the wrong vocabulary — and it sent `value: 0.5` to
   fields whose real magnitudes are 50, −25, 1800 and 94.

**And it voids H27's second example.** Bank gain displays as **`Gain -5`** and
**`Gain 2`** — signed, small integers. That is **decibels**, not a 0–1
fraction. So `SetGainBank(gain: 0.25)` set a quarter of a decibel: inaudible,
and invisible on a display showing whole numbers. The write was almost certainly
applied and the experiment could not have detected it.

H27's rule still holds — `den` outside its whitelist is properly controlled
evidence — but `SetGainBank` must not be cited for it. **Three separate wrong
readings were stacked on that one call**: wrong dial, then assumed-discarded,
then wrong units. Each time the reported conclusion was more confident than the
evidence supported.

**For the content plan (H26), the unit of tone-sharing is the bank.** Named,
self-contained, carrying its chain plus gain and sustain settings — exactly what
"share a tone" means, and `AddBank` is its transport.

**H27 — `true` from this firmware means "parsed", not "applied".** (2026-09-01,
against H2-CC340, dial read by the owner.)

`SetGainBank` returns `true` and does not move the instrument's **Effects
volume** dial. Established the only way it can be: the owner set the dial to a
known 100%, a write of `gain: 0.25` was sent, and the dial stayed at 100%.

An earlier write of `gain: 0.75` had been recorded here as promising. It was
not — the dial happened to already sit at 75%, so a discarded write and an
applied one produced identical evidence. **That coincidence was the finding's
whole basis and it was worthless.** Only a start value deliberately far from
the target could separate them, which is what the owner's second run supplied.

That makes three separate fields where a `true` reply accompanies no change:
`den` outside its `{1,2,4,16}` whitelist (H24), `den` dropped from a bundled
metronome write, and `SetGainBank` here. The generalisation is worth stating as
a rule for anyone building on this protocol:

> **A `true` result means the request was accepted and parsed. It does not mean
> the instrument changed.** Confirm every write by reading the value back, and
> where the change is one a player can perceive, by having them look at the
> instrument. Where no read-back exists — bank contents, effect parameters,
> gains — a write is simply unverifiable from software, and should be presented
> to a user as *sent* rather than as *set*.

What `SetGainBank` does instead is unknown. It may set some other gain with no
panel readout, or nothing at all; nothing in the protocol distinguishes those,
which is itself an instance of the rule above.

**H26 — Effects reach the audio path. Parameters do not.** (2026-09-01,
against H2-CC340, confirmed by ear by the instrument's owner.)

`AddEffect` with an **empty** parameter list loads an effect that is audible —
the owner was playing at the time and heard it arrive, loudly. That is the
first effect this client has put into the sound, and it retires the earlier
reading that accepted writes were inert:

> "Accepted is not audible, and that is still open." — H24b

They were audible. The earlier attempts were aimed at a bank that was not the
one playing, which is a mistake about *where*, not about *whether*.

**A non-empty `params` array is refused**, and the refusal is the firmware's:

| message | transport | result |
|---|---|---|
| `params: []` | LLT2 | **true**, and audible |
| `params: [{"key":"DryWet","value":0.5}]` | LLT2 | false |
| `params: [{"key":"DryWet","value":0.5}]` | **LLT, plain JSON** | false |
| `params: []` | LLT, plain JSON | **true** |

The plain-JSON row is the one that matters. LLT runs no compression at all, so
an identical rejection there clears the codec — which the round-trip test in
`compress` independently confirms encodes the nested shape exactly.

**And it is structural rather than vocabulary.** Every parameter name the
keyword dictionary carries was swept singly against `Tremolo` at a mid-range
value — `DryWet`, `Frequency`, `Decay`, `DelayTime`, `Feedback`, `Gain`, `Q`,
`Shift`, `Slider`, `Sync`, `Pitch`, `amp` — and all twelve were refused. A
wrong key would show up as *some* keys working.

**The untried lead is `preset`.** Every `AddEffect` this client has sent
carried `preset: "Default"`, a name lifted from the vendor app's
`PresetsActivity.DEFAULT_PRESET_NAME`. But the instrument's own grid is a list
of *named* presets, and `Effect` carries `preset` and `params` as separate
fields. The likely division is that `preset` selects a stored
parameterization and `params` is a live-override channel this firmware does not
accept. If so, sending a real preset name is how a *particular* sound gets
loaded, and `params` is not needed for it.

~~**What this settles for the content plan.** Shareable effect content splits in
two: "put a Tremolo in this bank" is transmissible today, while "this specific
tremolo, at these settings" is not.~~ **Overturned by H29**: parameters
transmit once the keys are the knobs' full words. "Structural rather than
vocabulary" above was wrong — the vocabulary swept was simply not Distortion's.

**A physical hazard worth recording.** This instrument is its own speaker: the
actuator drives the top, the pickup hears the top. A gain-adding effect
therefore closes a real acoustic feedback loop, which is what the owner heard.
Gain-shaped effects (`Boost`, `Volume`, `Distortion`) are a different risk
class from modulation ones and should not be sent to an instrument someone is
playing without warning them first.

**H25 — `SwitchBank` drives the instrument's preset grid, and the numbering is
direct.** (2026-08-29, against H2-CC340, with the owner reading the panel.)

The guitar's front panel shows a **3×3 grid of nine named presets**, scrolled
through by name with no numbers displayed. `SwitchBank n` selects the nth of
them, zero-indexed in grid order, confirmed twice:

| `bank_num` | panel shows |
|---|---|
| 3 | `tremolo` |
| 4 | `megaphone` |

On the reference instrument the grid reads `rev, chorus, echo, tremolo,
megaphone, boost, overdrive, disto, vintage` (seven of them edited by the
owner). An "off" state exists but sits outside the bank numbering.

**So "bank" is the panel preset, not an app-side store.** That corrects a
working assumption held since H7 — that banks were Firebase-resident and the
instrument held none, on the evidence that `ReadBank` returns `""`. It returns
`""` for every bank including ones the panel plainly populates, so **`ReadBank`
does not report bank contents at all**; its empty string says nothing about
whether a bank is empty. Every inference drawn from "the instrument has no
banks" is void.

**A caution this cost.** `bank_num: 0` is the *first preset*, not a scratch
slot — on this instrument, `rev`. Effect writes aimed at "an empty bank 0"
were aimed at the owner's reverb preset.

**H24b — `AddEffect` validates the effect type.** Swept all nineteen names
from the keyword dictionary, each add paired with an immediate remove:

- **Accepted (11):** `Chorus`, `Delay`, `Distortion`, `Equalizer`, `Highpass`,
  `Lowpass`, `Notch`, `Phaser`, `Pitch`, `Reverb`, `Tremolo`
- **Rejected (8):** `Boost`, `DelaySync`, `Disto`, `Echo`, `Octaver`, `Volume`,
  `LFO`, `None`

The rejections split cleanly: aliases whose canonical form is accepted (`Disto`
vs `Distortion`, `DelaySync` vs `Delay`), things that are not chain effects
(`None`, `LFO`), and gain stages (`Boost`, `Volume`) — `Boost` being rejected
while sitting on the panel as a preset is the tell that preset names and effect
types are different vocabularies. So the dictionary's nineteen is the app's
superset; eleven is what this firmware will insert.

**Accepted is not audible — now tested cleanly, and negative.** After the bank
model was corrected, the test was run properly twice: `AddEffect` → `true` and
`SwitchBank` to that same bank → `true`, in both orders (switch-then-add and
add-then-switch), on two different banks, with the owner listening. No audible
change, and the instrument's FX indicator never appeared. `SaveConfig` (`true`)
is not the missing commit step either. **Whatever store `AddEffect` writes,
the audible chain does not load from it** — the accepted/rejected split of
H24b is real validation, but validation of a write into a dead end. How the
phone app gets an effect into the live path remains unrecovered.

Two cautions from how this was learned, recorded because they cost real state:

-

**H23 — PARTIALLY SUPERSEDED by H24: the `ReadMetronome` half stands, the
"ignores `den`" half was ringdown's own serialization.** (2026-08-28, against
H2-CC340, with the owner reading the instrument's own display at each step.)

Run properly this time: the owner set a deliberately unusual signature, the
wire was read, a write was made, and the wire *and* the display were both read
again — instead of comparing one reading to a display observed at a different
moment.

| step | wire | display |
|---|---|---|
| owner sets 11/16 at 86 | `{"bpm":86,"den":16,"num":11}` | 11/16, 86 |
| write `bpm 93, num 10, den 32` → `true` | `{"bpm":93,"den":16,"num":10}` | 93, 10/16 |
| write `bpm 93, num 10, den 8` → `true` | `{"bpm":93,"den":16,"num":10}` | — |

**`bpm` and `num` apply. `den` never moves, and the call returns `true`
anyway.** Neither direction nor range explains it: 8 is the denominator 28 of
this instrument's loops use, and the owner confirms 32 is a selectable setting
on the guitar's own menu. `UpdateMetronome` simply does not write this field.

`ReadMetronome`, by contrast, returned 11/16 exactly — an unusual signature that
cannot arrive by accident, so the read path is confirmed sound.

**This retracts the anomaly H22 and two design docs were built around.** There
was no discrepancy between the wire and the instrument. The original comparison
took a `ReadMetronome` result at one moment, then compared it against a display
state after the setting had been changed for a test, without re-reading. The
guitar was correct every time; the reading was stale. The owner said as much at
the time — *"it was 5/4 before and it was 5/4 after"* — and that was recorded in
these docs as `den: 4` having been applied, which never happened.

The lesson is the one the workspace already has a rule for: an observation is
evidence about the moment it was taken. Two readings separated by a change are
two different experiments, and treating them as one manufactured a firmware
mystery that cost several sessions.

**SUPERSEDED by H24 below: `den` *is* writable, and every conclusion in this
finding past the read-path confirmation was an artifact of ringdown's own
serialization.** The table above is real data, but each of those writes was
sent with alphabetized keys, which is the very thing that made `den` inert.
"Read-only for everyone, vendor included" was declared here and was wrong; the
owner disproved it with the app in a minute. Kept as recorded because the
retraction is the instructive part.

**H24 — `den` writes over BLE. Two bugs stood in the way: one ours, one the
vendor's.** (2026-08-28, against H2-CC340; exhaustive value sweep, both
transports, race-controlled with in-connection pauses, panel cross-checked by
the owner throughout.)

**Bug one — ringdown's.** serde_json's default map alphabetizes keys, so every
params object this client had ever sent was reordered: `{bpm, den, num}`
instead of the declaration order `{bpm, num, den}` the vendor's serializer
emits. The firmware's parser is order-sensitive: **a `den` arriving before
`num` is silently dropped** while the fields around it apply and the call
returns `true`. Enabling `preserve_order` made `den` writable at once. The
crate now pins declaration order with a test, and the Cargo.toml comment marks
the feature load-bearing.

**Bug two — the vendor's.** With order fixed, the acceptance set was mapped
exhaustively: every value 1–32 written from a known-good baseline with a read
between, plus 256 as a boundary probe.

| sent | result |
|---|---|
| 1, 2, 4, 16 | **applies** |
| every other value 1–32 | silently refused, `true` returned |
| 256 | silently refused, `true` returned |

**The firmware whitelists `{1, 2, 4, 16}` and silently drops the rest —
including 8 and 32, which the instrument's own front panel offers.** The panel
path sets all six of `{1,2,4,8,16,32}`; the RPC path accepts four. A table
with a hole in it, shipped invisibly because the app layer discards every RPC
result (`MetronomeModel` returns `Unit` without checking), so no one at the
vendor was positioned to see a refusal.

Ruled out along the way, each by one targeted write: value transforms (the one
apparent "8 → 4" was a stale baseline — the owner's app test had moved the
instrument between sessions), a settle race (a 5 s in-connection pause after a
refused write changed nothing while `bpm` in the same message applied
instantly), compound-meter gating (6/8 refused like 5/8), a `den > num` rule,
LLT-vs-LLT2 encoding (plain-text refusals identical), and an
exponent-is-power-of-two check (predicted 256 accepted; it was refused).

Fields apply independently: a message with a refused `den` still applies its
`bpm` and `num`. Sent without `bpm` entirely, the call returns `false` and
changes nothing — the one honest error this handler produces.

**Consequences.** Ringdown documents the whitelist at `params::metronome`.
Woodshed may now offer denominator control for `{1,2,4,16}` if wanted — its
current bpm+num-only posture is safe but no longer forced. And the field-order
sensitivity is a standing hazard for *every* method: any params object this
client sends must keep declaration order, which `preserve_order` plus the
pinned test now guarantee.

**H22 — The loop header is a time signature, a bar count and a completion flag,
read across the whole library.** (2026-08-28, `probe --index` against
H2-CC340 — 31 headers, one round trip each, about a minute of instrument time.)

The six values are:

```text
version, tempo_bpm, beats_per_bar, den, bars, partial
```

`beats_per_bar`/`den` are a **time signature**, numerator and denominator, and
`tempo_bpm` counts `den` notes rather than quarters. The instrument's owner
confirms `loop0031.wav` is **200 BPM, 7/8, 4 bars**, which is exactly
`7, 8, 4` in fields three to five.

The relation, holding across all 31 files:

```text
beats    = beats_per_bar × bars
nominal  = beats × 60 / tempo_bpm
recorded = ceil(nominal_samples / 256) × 256      (when partial == 0)
```

**24 of 31 have `partial == 0` and land on that block-rounded length exactly,
with no exceptions. The other 7 are all short of the grid**, between 29% and
96% of it — so the last field distinguishes a take that ran its full bar count
from one that did not. Very likely `StartRecording`'s `free` flag, though that
is an inference from two matching descriptions rather than an observation.

**The library, as read:** five tempos (60, 90, 120, 160, 200), bar counts of 4,
8, 12 and 14, and meters of 4/4, 4/8 and 7/8.

**This corrects H20, which was fitted to one file.** Three claims there were
wrong, and each needed the corpus rather than more thought:

| H20 claimed | Actually |
|---|---|
| block is 2048 samples | **256.** `loop0031` alone also divides by 2048; nothing else does |
| the two length fields are inseparable | **separable.** The meter takes 2 values across the corpus, the bar count 4 |
| the last value is spare | **a completion flag**, and it explains the 7 outliers |

The first is the instructive one. A single sample is consistent with every
block size that happens to divide it, and the largest such divisor looks like
the answer. Reading 31 headers cost a minute and settled it.

**`den` is a denominator, and there was never a contradiction.** The earlier
reading rejected that on a comparison between a `ReadMetronome` result and a
display state — but the two were observed at different times, with the setting
changed in between, and no second read was taken. The instrument reported
correctly throughout. The "field nobody understands" that two design docs were
built around was an artefact of comparing a stale reading to a later display.

See H23, which tested this properly.

**H20 — SUPERSEDED by H22. The loop header's `200` is a tempo, and a loop's
metadata costs one round trip rather than ten minutes.** (2026-08-28, desk work
against the already-retrieved `loop0031.wav`; no instrument involved.) The
transport conclusion below stands and is what made H22 affordable; the
field-level claims do not.

The `JUNK` chunk's six values are `1, 200, 7, 8, 4, 0`, and the audio they sit
in front of is 741,376 bytes of 16-bit mono at 44.1 kHz — **370,688 samples,
8.4056 s**. That is enough to identify two of the six by arithmetic:

- **`200` is BPM.** 7 × 4 = 28 beats at 200 BPM is 8.400 s. **No other product
  of these values is close:** 7 × 8 = 56 beats would run 16.8 s and 8 × 4 = 32
  would run 9.6 s. So 28 is the loop's length in beats and `8` is not a length
  field at all.
- **The residual 5.6 ms is block rounding, and exactly so.** 370,688 is
  precisely 181 × 2048, and 2048 is the *largest* power of two that divides it
  — 4096 does not. The tempo-exact 370,440 samples is 180.88 blocks; rounding
  up to 181 reproduces the file size to the byte. The recorder captures whole
  2048-sample DSP blocks, which also names the audio block size of the STM32
  side for free.

**What is deliberately not concluded.** Which of `7` and `4` is bars and which
is beats-per-bar does not follow, because only their product reaches the audio:
7 bars of 4 and 4 bars of 7 are the same 28 beats. `ringdown::loopfile` names
them `length_first` and `length_second` for that reason. Naming a field for an
inferred meaning is the exact error that `den` was, and that one cost a write
to the owner's instrument; a positional name that looks unfinished is the
cheaper mistake.

~~`8` remains unexplained, with one suggestive coincidence: `ReadMetronome` on
this instrument returns `den: 8` while its metronome is demonstrably in 5/4.~~
**Wrong on both halves — see H22 and H23.** `8` is the time signature's
denominator, and the `ReadMetronome` "coincidence" was a stale reading compared
against a later display.

**The consequence is larger than the puzzle.** `DumpFile` takes an offset and a
size (H14), so a loop's header is **one round trip of 92 bytes**. Retrieving a
loop's *audio* took 620.7 s over roughly 3,700 round trips when measured
(recorded in woodshed's `2026-08-27_smart_instrument_plan.md`); retrieving its
*tempo, length and format* is a single call. Indexing all 31 loops is seconds,
not the five hours a bulk archive would take, and browsing a library by tempo
is practical even though downloading it is not. This substantially softens the
throughput constraint recorded in W3: the slow path is only ever needed for the
take actually wanted.

Implemented in `ringdown::loopfile` with the reference file's first 92 bytes
pinned as a fixture — verified byte-identical to the file on disk, not
transcribed by eye — and `probe --index` reads the whole library this way.
The index also prints each header's values across loops, which is what will
separate the two length fields: a bar count varies with how long you played and
a meter does not, so one file cannot tell them apart and a dozen can.

**H19 — `ReadConfig` is dead code in the vendor's app too, and ringdown does
not need it.** Two facts close H18.

**The app never calls it.** `readConfig` appears nowhere in the application
layer — the same as `readBank` and the nineteen dictionary methods. It exists
in the vendor's library, is exposed by `DeviceCommunicator`, and is invoked by
nothing. So the firmware handler behind it has almost certainly never been
exercised by the vendor either, which is the readiest explanation for why it
hangs: untested code, and a bug in it that nobody was positioned to notice.

**And its reply would be enormous.** `UserConfig` carries
`favorite_banks: List<Bank>` — every bank, each with its full effect chain and
every parameter — alongside the equalizer, metronome, aux settings, and device
identity. Even compressed that dwarfs a single notification, on a device with
no inbound reassembly on either side (F11). A handler attempting to serialise
it is a plausible place to run out of memory or overrun a buffer, which fits
the observed symptom exactly: no reply at all, and an RPC task that never
recovers.

**The reframe: the configuration is reachable without it.** Almost everything
`UserConfig` holds is available from methods already confirmed working:

| `UserConfig` field | Reachable via | Status |
|---|---|---|
| `cpu_id`, `version_esp`, `version_stm`, `free_space` | `GetStatus` | confirmed (H4) |
| `metronome` | `ReadMetronome` | confirmed (H9) |
| `favorite_banks` | `ReadBank` per index | confirmed (H7) |
| calibration / feedback filters | `GetAnalysis` | confirmed (H9) |
| `equalizer` | — | **write-only** |
| `aux_in_on`, `aux_in_drywet`, `aux_out_*` | — | **write-only** |

So `ReadConfig` was never the gateway it appeared to be. It is one convenience
call over a set of working ones, and ringdown can assemble the same picture by
composing them. The genuine gap is narrow and worth naming precisely: **the
equalizer and aux settings are write-only over this protocol.** `SetEQGain`,
`SetEQBandGain`, `AuxIn`, `AuxInDryWet`, `AuxOut` and `AuxOutDryWet` all set;
nothing reads them back. A client must therefore treat its own last-written
values as the record, or read them off the instrument's screen.

`GetLevels` would have been the natural reader for at least the aux path, and
it is precisely the one swept method the firmware does not implement (H8).

**H17 — LLT2 CONFIRMED AGAINST HARDWARE. The codec is correct.** With the
transport wired in and selected from the banner, a compressed exchange
succeeded end to end:

```
-> 18 bytes                       (GetStatus, compressed from 55)
<- 141 bytes  [compressed] {"jsonrpc":"2.0","id":1,"result":{...}}
```

The firmware **understood our compressed request** and **answered compressed**,
and our decoder read its reply back into correct JSON. That validates, in one
exchange: the 163-entry dictionary including all ten reconstructed entries, the
nibble packing and alignment, the BCD number encoding, the frame-free
short-message path, and the decision to write `decode` as the inverse of
`encode` rather than as a copy of the vendor's off-by-one decoder. F17 moves
from careful reading to confirmed fact.

It also settles a question F11 left open: **the device mirrors the encoding.**
Asked in plain JSON it answered in plain JSON (170 bytes); asked compressed it
answered compressed (141 bytes for the same content). The client's choice of
transport determines the reply's.

**H18 — `ReadConfig` wedges the firmware's RPC handler.** This is the sharpest
negative result of the project and it cost several runs to see clearly.

`ReadConfig` returns nothing — not at the 10-second default, not at 60 seconds,
over LLT2 as over LLT, with zero notifications of any kind. That alone would
merely be unsupported. What makes it worse is what happens next: **every
subsequent request is met with silence too, including `GetStatus`, and the
condition survives disconnecting and reconnecting.** Only a power cycle clears
it.

Demonstrated deliberately: `GetStatus` succeeded, `ReadConfig` was attempted,
and a plain `GetStatus` on a fresh connection minutes later timed out with
nothing heard. The instrument had been answering happily immediately before.

Consequences worth stating plainly:

- **The transport is not the problem, and Phase 2.5 was not the blocker it
  looked like.** LLT2 works (H17). `ReadConfig` fails for a reason inside the
  firmware, not in the framing or the codec.
- **Each attempt costs a power cycle**, so this is expensive to probe and
  should not be retried casually.
- **It poisons unrelated diagnosis.** A wedged handler presents as some *other*
  request failing, which is precisely the misleading symptom the probe's own
  advice was written to warn about — and it was, several messages before anyone
  realised it was describing the situation at hand.
- The vendor's app presumably avoids this, either by never calling `ReadConfig`
  on this firmware or by calling it in a context not yet reproduced. Worth
  checking what the app does immediately before its own `readConfig`.

Open, and not to be guessed at: whether some parameter, ordering, or prior call
makes `ReadConfig` safe; and whether `SetConfig`/`SaveConfig` share the fault.
Testing those means more power cycles and, for the writing ones, accepting a
change to the instrument — a decision for the owner rather than a curiosity to
satisfy.

**F17 — LLT2 fully specified and implemented.** Both halves are now in the
core: `compress` (the JSON codec) and `llt2` (the binary framing).

**The codec.** A nibble-oriented, domain-specific JSON encoder — not a general
compressor. Structural tokens cost one nibble; a string in the 163-entry
keyword dictionary costs a nibble plus a byte, so `"jsonrpc"` travels in 12
bits rather than 72; other strings carry a length then UTF-8; numbers are BCD,
one nibble per character. Measured on real payloads:

| Message | Raw | Compressed |
|---|---|---|
| `GetStatus` request | 55 | **18** (33%) |
| `GetStatus` reply (captured) | 170 | **77** (45%) |
| `GetAnalysis` reply (captured) | 67 | 51 (76%) |
| effect chain | 162 | **48** (30%) |

**The dictionary needed reconstruction.** Ten of the 163 entries are stored in
the vendor source as symbolic Java constants rather than literals
(`FirebaseAnalytics.Param.METHOD`, `PresetsActivity.DEFAULT_PRESET_NAME`, and
so on). Each was resolved from its defining class and then **checked by
position**: the dictionary is in codepoint order, so `method` must fall between
`message` and `metronome`, `Default` between `Decay` and `Delay`. All ten
landed where they had to. Order is the encoding, so a wrong entry is a wrong
protocol.

**One deliberate divergence from the vendor's decoder.** As decompiled, it
advances one nibble too many for every multi-nibble token — `BT_STRING_DICT`
consumes four where the encoder writes three, with the same off-by-one in all
four variable-length branches and single-nibble tokens correct. Correct-for-
simple, uniformly-wrong-by-one-for-compound is the signature of a decompiler
mis-hoisting a loop increment, and such a decoder could not read its own
encoder's output. `decode` is therefore written as the exact inverse of
`encode` and the pair is round-trip tested. The peer that matters is the
firmware.

**The framing needed a second decompiler pass.** `LLT2Manager.sendMessage`
defeated JADX outright ("Method not decompiled: instruction units count: 514"),
so the layout was read from a fallback-mode pass over the raw instruction
stream, transcribing the actual shift-and-mask sequence:

```
byte 0      transfer type — 'J' (74) for a JSON message
byte 1      reserved, always zero
byte 2      object id, low byte
byte 3-4    frame number, 16-bit little-endian, 1-based
byte 5-8    total compressed length, 32-bit LE — FIRST FRAME ONLY
next 2      this frame's payload length, 16-bit LE
remainder   payload
```

Header is **11 bytes on the first frame, 7 after**; each carries
`min(remaining, write_len - header)`. Corroborated independently by
`onDeviceMessageReceived`, which decompiled cleanly and validates exactly those
five header bytes before reading a status code from byte 5.

**A compressed message that fits one write is sent bare, with no header at
all** — the same shape as LLT's short-message path. Limits from the vendor:
16 KB compressed, 128 KB uncompressed.

**Demultiplexing is unambiguous by construction.** A compressed document always
begins with the start nibble in the high half of byte zero, so its first byte
is below `0x10`; an acknowledgement's first byte is the transfer type `'J'`.
The two inbound shapes cannot be confused, and there is a test asserting it.

**Unverified against hardware.** No compressed exchange has happened yet. 79
tests pass, including round-trips of both captured device payloads, but that
demonstrates self-consistency, not agreement with the firmware. The next run
settles it.

**H16 — The configuration is not a file. LLT2 is back on the critical path.**
Using the H15 oracle, **55 directory names** were probed — product-shaped names
in both capitalisations, plus ESP32 filesystem mount points (`/spiffs`,
`/littlefs`, `/nvs`, `/flash`, `/sd`) and unix conventions. Exactly two exist:

- **`/Loops`** — the recordings, confirmed readable (H13, H14).
- **`/Calibration`** — exists; **189 guessed filenames inside it found nothing**.

Nothing resembling `/Config`, `/Settings`, `/Banks`, `/Presets` or `/Data` is
present. The app is no help here either: its only path-shaped constants are
Firebase references (`/CONFIGS/hyvibe_user/favorite_banks/`, `metronome`,
`audio`), because it never reads device files.

**So the H14 hope is dead, and it should be said plainly rather than left to
fade.** `DumpFile` opened a real bulk read path, and the reasoning that it might
make `ReadConfig` unnecessary was sound — it just turned out to be false. The
configuration is not stored anywhere the file mechanism can reach; it lives in
flash or NVS behind the RPC layer. **`ReadConfig` is the only route to it, and
`ReadConfig` needs LLT2.** Phase 2.5 is required after all.

What the file path *is* good for is real, just not this: the loops are readable
and backup-able from the desktop, which the vendor only offers over USB
mass-storage.

`/Calibration` remains opaque. It exists, holds none of 189 plausible names, and
may simply be empty — the calibration results are already retrievable through
`GetAnalysis` (H9), so nothing is obviously missing by not reading it.

**H15 — CONFIRMED. The error code distinguishes a missing file from a missing
directory, and the device has a filesystem oracle.**

The control pair ran with both predictions stated in advance, and both held:

| Probe | Predicted | Actual |
|---|---|---|
| `/Loops/zzz_not_here.wav` | 4 | **4** |
| `/Zzzz/zzz_not_here.wav` | 5 | **5** |

So `GetFileInfo` answers **4** when the directory exists but the file does not,
and **5** when the directory itself is absent. A missing-file query is therefore
a **directory-existence test**, which is the only enumeration this protocol
offers — there is no listing method anywhere in the dictionary.

This is the first claim this session to be established the right way round:
hypothesis first, predictions written down before the run, a positive control
and a negative control in the same run. Everything it says is worth more than
the five corrected claims that preceded it, and it took one extra command to
get. `ringdown-probe --dirs` implements the sweep, with `/Loops` built in as
the positive control and a loud refusal to report anything if that control
fails.

*Original hypothesis, retained for the record:*

**H15 (as filed): the error code distinguishes a missing file
from a missing directory.** Three config-path guesses all failed, but not
identically:

| Path | Code |
|---|---|
| `/config.json` | 4 |
| `/Config/config.json` | **5** |
| `/config` | 4 |

The one path naming a directory that plausibly does not exist returned a
different code. Set against the earlier `/Loops/loop0032.wav` failure, which
returned **4** for a missing file in a directory that certainly *does* exist,
the shape suggests:

- **code 4** — the directory exists, the file does not
- **code 5** — the directory itself does not exist

If that holds it is a **filesystem oracle**: directories can be enumerated by
probing paths and reading the code, with no listing method required. That would
be the cleanest route to finding where a configuration lives.

**This is a hypothesis from three observations and it is filed as one.** Five
times this session a general rule was drawn from a single reading and had to be
corrected — twice about error codes specifically (H8, H10). The distinguishing
feature of a real test is that it can fail, so before this is used it gets a
control pair with predictions stated in advance:

| Probe | Predicted | Why |
|---|---|---|
| `/Loops/zzz_not_here.wav` | **4** | positive control: directory known to exist |
| `/Zzzz/zzz_not_here.wav` | **5** | negative control: directory certainly absent |

Both predictions must hold. If the codes come back equal, the hypothesis is
dead and the earlier code-5 was about something else in that path. If they come
back as predicted, directory probing is sound and worth automating.

**H14 — `DumpFile` works. The bulk read path is open, and it does not need
LLT2.** Parameters are `name`, `offset`, `size`; the result is an **uppercase
hex string** of the file's bytes.

```
DumpFile name=/Loops/loop0031.wav offset=0 size=64
-> "5249464654500B00574156454A554E4B28000000487956696265206C6F6F70..."
```

Decoded, those 64 bytes are a real WAV header:

```
RIFF | size 741460 | WAVE | JUNK (40) | "HyVibe loop file"
     | 1, 200, 7, 8, 4, 0 | fmt ...
```

**The cross-check is exact.** The RIFF payload size of 741,460 plus the 8-byte
header is **741,468** — precisely the size `GetFileInfo` reported for the same
path. Two independent methods, agreeing byte-for-byte on a real file. That is
the strongest single confirmation the protocol map has received.

The `JUNK` chunk is a vendor extension labelled **"HyVibe loop file"** carrying
six 32-bit values: `1, 200, 7, 8, 4, 0`. Anyone reading these files later will
want this decoded; it is not needed to read the audio, since the chunk is
`JUNK` and standard readers skip it.

**Decoded 2026-08-28 — see H20.** The guess filed here, that `8` and `4` are a
time signature, is wrong: `200` is the tempo and `7 × 4 = 28` is the length in
beats, which leaves `8` as the value that is *not* a length.

**Parameter naming is not uniform across the file methods.** `name` works;
`file` returns **code 2, "OPEN FILE ERROR"** — note both a different code *and*
different capitalisation from the code 4 "open file error" seen earlier. Two
distinct error paths exist in the firmware, plausibly one per processor, and
neither code nor message casing can be assumed shared between them. This
reinforces H10: classify on message content, and do not treat any code as
canonical.

**What this changes for the plan.** A bulk read path that returns file bytes
through ordinary RPC replies means large data can leave the instrument
**without LLT2**. If the configuration exists as a file, `ReadConfig` may be
avoidable entirely. Phase 2.5 is therefore no longer certainly on the critical
path — it depends on whether a config file exists and can be named. Throughput
is the obvious cost: the reply is hex, so two characters per byte, and a
514-byte write length caps a single call at roughly 200 bytes of file data.
Fine for a configuration, slow for a 741 KB loop.

**H13 — The file mechanism works, and the naming off-by-one is confirmed.**

```
GetFileInfo /Loops/loop0031.wav -> {"crc32":1693117162,"size":741468}
GetFileInfo /Loops/loop0001.wav -> {"crc32":27019092,"size":3241564}
```

Three results in one run:

- **`GetLastRecordingName` returns the *next* filename, not the last written
  one.** `loop0031.wav` exists and `loop0032.wav` does not, while the method
  reports `0032`. Every earlier `GetFileInfo` failure was this off-by-one, not
  a path convention — the paths were right all along, the *number* was one past
  the end. Any caller wanting the most recent recording must subtract one, and
  should not assume the reported name is openable.
- **`GetFileInfo` is confirmed and returns `{crc32, size}`** for a path of the
  form `/Loops/loopNNNN.wav`. Sizes are real audio: 3.24 MB for loop0001, 741 KB
  for loop0031.
- **The H12 retraction's arithmetic checks out.** Two samples of 3.24 MB and
  0.74 MB bracket the 1.26 MB average that 39 MB across 31 files implies. The
  storage figure was consistent with the loops from the start, which is what the
  retraction claimed and this independently confirms.

**Why `crc32` matters beyond a checksum.** `LLT2Manager.sendFile` tracks a
CRC32 across a transfer, and the dictionary carries `DumpFile`, `file`,
`file_type`, `offset`, `size` and `data`. So the shape of the file protocol is
legible: ask `GetFileInfo` for size and checksum, then move the bytes in ranges,
verifying against the CRC. That is a **read path for bulk data that does not go
through an RPC result**, and it is the strongest remaining candidate for
retrieving a configuration without implementing LLT2 first.

`DumpFile` is the untested half. Its parameter names are a guess — `name`
matches `GetFileInfo`, while the dictionary also offers `file`; `offset` and
`size` are likely. Despite sitting in the mutating group of F15 by name,
"dump" here reads as *emit to the caller*, so trying it against an existing loop
file is a read rather than a change to the instrument.

**H12 — RETRACTED 2026-08-27, same day, by the instrument's owner.** The
claim below was that the device holds no recordings. It holds **31 loops**.

The retraction matters more than the claim did, because the reasoning was
checkable and wrong. 7.634 GB free at 99.49% gives a total of 7.673 GB and
**39 MB used**. I read 39 MB as "firmware and system only" without ever asking
what 31 loops would weigh: 39 MB over 31 files is **1.26 MB each**, which at
44.1 kHz/16-bit is roughly 7–15 seconds of audio per loop — precisely what a
looper produces. The number was consistent with the loops all along. A large
card makes real content look like rounding error, and "99.5% free" is not
"empty" unless you check the magnitude of what you expect to find.

**What this leaves standing, and what it changes:**

- The banks may still be genuinely empty; that is a separate question from
  recordings and untouched by this retraction.
- The `GetFileInfo` failures now have a much better explanation.
  `GetLastRecordingName` returns `/Loops/loop0032.wav` while 31 loops exist, so
  the returned name is most likely the **next** name rather than the last
  written one — an off-by-one in naming, not in the protocol. Every path form
  failed because every path form named a file that does not exist yet.
- The immediate test is to ask for a file that does exist: `loop0031.wav`,
  or any lower number.

**The methodological note below survives its own counterexample**, and is
sharpened by it. It warned against explaining emptiness with elaborate theories
instead of checking the simple thing — and was itself an elaborate theory built
on an unchecked number, written in the same breath as the warning. Five times
in one session a general claim was drawn from a single unverified reading. The
pattern is not carelessness about any particular fact; it is that in reverse
engineering every fact arrives as a sample of one, and the discipline has to be
applied to one's own conclusions at exactly the moment they feel most
explanatory.

---

*Original claim, retained for the record:*

**H12 — We have been testing read APIs against an empty instrument.** All
three path forms — `/Loops/loop0032.wav`, `loop0032.wav`,
`Loops/loop0032.wav` — fail identically with "open file error", which rules
out the path convention as the cause and leaves the obvious one: **the file
does not exist.**

The arithmetic in `GetStatus` says the same thing plainly, and it was there all
along. 7.63 GB free at 99.5% means a total around 7.67 GB and roughly **40 MB
used** — firmware and system only. There are no recordings on this instrument.

That single fact explains nearly every unexplained result of the session, and
none of them were protocol problems:

- Eight empty banks: no custom banks were ever created through the vendor's app.
- `GetFileInfo` failing on every path form: there is no file to open.
- `PrintBank` returning `true` and producing nothing retrievable: nothing to
  print.

`GetLastRecordingName` returning `/Loops/loop0032.wav` against an empty
filesystem means it reports a *remembered or next* name rather than an existing
file — worth knowing, and not deducible from a device that has recordings.

**The methodological point, which is the durable one.** A read API exercised
against an empty device returns empty, and empty is indistinguishable from
broken until you put something there. Several hours went into explaining
emptiness with transport theories — LLT2, produce-then-fetch, path conventions
— when the instrument was simply reporting, accurately, that it holds nothing.
The check that would have caught it was free and sitting in the first
successful response.

**The test that settles it**, and the right next step before more theorising:
record a loop on the guitar with its own looper, or create a bank in the
vendor's app, then re-run `GetLastRecordingName`, `GetFileInfo` and `ReadBank`.
If they return content, the file and bank mechanisms are confirmed end to end
and no further protocol work was ever required for them.

**H10 — Error code 4 is generic; classify on the message. Corrects H8.**
`GetFileInfo` with `name=/Loops/loop0032.wav` returned **code 4, "open file
error"** — the same code that carried "Method not found error" for `GetLevels`.
So code 4 is a general failure code and the *message* is what distinguishes
causes. H8's claim that "error 4 is a membership test" is wrong as stated: the
membership test is the message text.

This was a live defect in the sweep, which classified on `code == 4.0` and would
therefore have filed any existing-but-failing method as not implemented. That is
the more damaging direction of the two mistakes — it retires a capability that
exists — and it is the third time this project has drawn a general rule from a
single observation. Fixed to match on the message.

**H11 — The produce-then-fetch theory is falsified, and the banks are probably
just empty.** Two results together:

- `GetLastRecordingName` still returns `/Loops/loop0032.wav` *after* a
  `PrintBank` call, so `PrintBank` does not produce a retrievable file — or at
  least not one that becomes "the last recording". The H9 speculation that
  `ReadBank`/`PrintBank` are commands producing output elsewhere is not
  supported.
- `ReadBank 1` returns `""` like every other slot, and nothing follows it.

Taken with the `GetStatus` report of **99.5% free space**, the simplest
explanation is now the likeliest: the user has never created custom banks
through the vendor's app, the device's bank slots are genuinely empty, and
`ReadBank` is reporting that accurately. The instrument's current sound would
then live in the configuration rather than in a bank — which is what
`ReadConfig` returns, and what LLT2 is still blocking.

**`GetFileInfo` rejects the path it was given.** `/Loops/loop0032.wav` comes
verbatim from `GetLastRecordingName`, so either the two use different
conventions (a leading slash, a different root, a name without a directory), or
the file no longer exists. Worth trying the variants before concluding anything
about the file mechanism; the method itself clearly exists, since it got far
enough to try opening something.

**H9 — Five of six swept methods exist, and one of them returns this guitar's
own resonances.** Sweep results from H2-CC340:

| Method | Result |
|---|---|
| `BTcheck` | `true` |
| `GetAnalysis` | `[[4,106,-3.3,6],[4,228,-6.8,3.75],[4,545,-7.8,8.1],[4,3760,-3.8,6]]` |
| `GetLastRecordingName` | `"/Loops/loop0032.wav"` |
| `GetLevels` | not implemented (error 4) |
| `PrintBank` | `true` |
| `ReadMetronome` | `{"bpm":60,"den":8,"num":5}` |

`PrintBank` with `bank_num=0` also returns `true`.

**`GetAnalysis` is the find.** Four four-element rows, and read against the
dictionary's `nbreso`, `f0`, `Q`, `Gain`, `Notch`, `fbk_params` and `fbk_onoff`
keys, the shape is almost certainly `[filter_type, frequency_hz, gain_db, Q]`:

| f (Hz) | gain (dB) | Q |
|---|---|---|
| 106 | −3.3 | 6 |
| 228 | −6.8 | 3.75 |
| 545 | −7.8 | 8.1 |
| 3760 | −3.8 | 6 |

Every gain is negative, so these are **cuts** — a measured feedback-suppression
filter bank derived from the instrument's own calibration run. And the
frequencies are where an acoustic guitar body actually resonates: ~106 Hz sits
at the Helmholtz air resonance, ~228 Hz at the principal top-plate mode, 545 Hz
at a higher plate mode.

That is this specific instrument's measured acoustic signature, readable over
Bluetooth. It is also, precisely, a list of the plate resonances a luthier maps
when voicing a top — the thing the project is named after. **Confidence:** the
numbers are hardware-verified; the column interpretation is inference from the
dictionary and from the values' physical plausibility, and wants confirmation
against a second instrument or a re-calibration before being relied on.

`GetLastRecordingName` returning a **path** — `/Loops/loop0032.wav` — matters
structurally: the device has a filesystem with recorded loops, reachable by
name, and `GetFileInfo` takes exactly such a name. Together with `DumpFile`,
`offset`, `size` and `file_type` in the dictionary, this is very likely how
large data leaves the instrument, which would explain `ReadBank` returning `""`
and `PrintBank` returning `true`: they are commands that *produce* something
elsewhere, not queries that return it. Testable with `GetFileInfo`.

`ReadMetronome` returning `{"bpm":60,"den":8,"num":5}` is live instrument state
and confirms the metronome parameter *names* from F13 against hardware.

~~**Corrected 2026-08-27: this is not "5/8".**~~ **Retracted 2026-08-28 — that
"correction" was itself the error. See H23.** `den` *is* the denominator, so
this reading is 5/8 and the instrument was in 5/8 when it was taken. The
display that read 5/4 was observed later, after the setting had been changed
for a test and without a second read, so the two were never comparable. `den`
was never changed from 8 to 4 by anything sent here; the instrument was already
at 4 by the time that was checked.

`num` and `den` are both the time signature, and `ReadMetronome` reports both
correctly. Writing `den` works too, with the field order and value whitelist
established in H24.

**H8 — The device reports unknown methods, which makes the dictionary
testable.** `GetLevels` — one of the nineteen names the keyword dictionary
leaked — returned a clean JSON-RPC error: **code 4, "Method not found error"**.

Two conclusions, one narrow and one broad. Narrowly, `GetLevels` is not
implemented in this firmware (STM 1.2.3 / ESP 1.3.0) despite being in the
dictionary, which confirms the caveat recorded with F15: a dictionary entry
proves the firmware knows the *string*, not that a method stands behind it.
Broadly, error 4 is a **membership test** — the device will say which names are
real, so the whole undocumented surface can be enumerated rather than guessed
at. `ringdown-probe --sweep` does this.

The sweep is restricted to query-shaped methods (`BTcheck`, `GetAnalysis`,
`GetLastRecordingName`, `GetLevels`, `PrintBank`, `ReadMetronome`). The
thirteen mutating ones are deliberately excluded: enumerating them would mean
firing state-changing commands with guessed parameters at a working instrument
to satisfy curiosity, and `LaunchCalibration` alone would re-run the
instrument's self-calibration. They remain one `--call` away when there is a
reason to want one.

Also note the error path itself is now hardware-verified: a device error
decodes, carries its code and message, and surfaces as `RpcError::Device`.

**H7 — `ReadBank` answers; the transport split is by size, not by method.**
After the connection-leak fix and a power cycle, `ReadBank 0` **replied**
rather than timing out — so a second method beyond `GetStatus` is reachable over
the plain transport. Its `result` was an empty string.

Two things that clarifies. First, `readBank` is declared
`readBank(int) -> String` in `DeviceCommunicator`, so a string result is its
normal shape, not a degenerate one. Second, **the vendor's own app never calls
it**: every `readBanks` in the UI reads Firebase or local storage, never the
device. So `ReadBank` joins the nineteen dictionary methods as firmware surface
the app leaves unused, and its semantics are ours to establish rather than to
copy. An empty result may mean the bank is empty, that banks are 1-indexed, or
that bank content is fetched some other way (the dictionary's `PrintBank`,
`DumpFile`, `file`, `offset`, `size` keys suggest a file path).

The useful conclusion is about the transport, and it corrects a guess in H6:
the LLT2 boundary is **by message size, not by method**. `ReadBank` is a small
enough exchange to work uncompressed; `ReadConfig` is not. That means Phase 2.5
is required for the large reads specifically, not for "the methods the app
doesn't use".

**H6 — `ReadConfig` is silent, and it identified a second transport we had not
implemented at all.** `GetStatus` round-trips, but `ReadConfig` — same request
size, much larger reply — returns nothing whatever in 10s. The MTU explanation
is dead: the `GetStatus` reply was **171 bytes** and arrived intact, so the
negotiated MTU is at least 174.

The cause is in `ConnectableDeviceCommunicator` line ~5898, which chooses a
transport by firmware version:

```java
(stmVersion.isAtLeast(1,2,2) && espVersion.isAtLeast(1,2,2))
    ? new LLT2Manager(...)   // binary framing + compression
    : new LLTManager(...)    // the one ringdown implements
```

The reference instrument is STM 1.2.3 / ESP 1.3.0, so **it is an LLT2 device**.
Ringdown implements LLT1 only. Small uncompressed requests still work because
short messages bypass framing on both paths and the device answers in kind — so
`GetStatus` succeeding was never evidence that the transport was complete.

**F14 — LLT2, the transport for anything large.** Differences from LLT1, which
are not incremental:

- Operates on **bytes**, not strings (`request(byte[] …)`).
- Payloads are **compressed** by `JsonCompressor`, a bespoke codec — not gzip.
- Frames carry a **binary header**: `data[0]` is a transfer type, `data[2]` the
  object id, `data[3..4]` a little-endian index.
- `sendFile` tracks a **CRC32** across the transfer.
- Receive path is `JsonCompressor.decode(data)`, which returns null unless the
  first nibble is `BT_START` — so the format is self-describing, and an
  uncompressed message is simply ignored by an LLT2 reader.

`JsonCompressor` is a nibble-level JSON codec: 4-bit block-type tags for
structural tokens (`{`, `}`, `,`, `:`, `[`, `]`, `true`, `false`, `null`),
8-bit indices into a 153-entry keyword dictionary for known strings, short-string
and BCD encodings for the rest. Implementing it is real work, but it is
self-contained and fully described by `JsonCompressor.decode`/`encode` and
`NibbleList`.

**F15 — The keyword dictionary leaks the device's whole vocabulary, including
capabilities the phone app never exposes.** The dictionary must contain every
string the firmware exchanges, so it is effectively a manifest.

**Nineteen methods absent from `RPCRequestType`** — i.e. present in the firmware
but never called by the vendor's own app:

`ActivateSpkFilter`, `BTcheck`, `BypassEffect`, `DumpFile`, `GetAnalysis`,
`GetLastRecordingName`, `GetLevels`, `LaunchCalibration`, `PrintBank`,
`PullFbk`, `ReadMetronome`, `SetGainPreamp`, `SetPhaseInv`,
`SetSpeakerBiquads`, `StartAnalysis`, `StartRendering`, `StartTuner`,
`StopRendering`, `StopTuner`

Several are exactly the "past what the phone app exposes" the project exists
for. `SetSpeakerBiquads` is the standout: raw biquad coefficients on the speaker
path is arbitrary filter design on the instrument's output, without touching
firmware at all. `GetLevels` pairs with the dictionary's `input_level` /
`output_level` keys for live metering; `StartTuner` / `StopTuner` expose the
tuner woodshed already has a UI for; `StartAnalysis` / `GetAnalysis` reach the
calibration engine; `SetGainPreamp` and `SetPhaseInv` are preamp controls the
app's Input Levels screen only partly surfaces.

**The effect catalog**, previously assumed to be reachable only via Firebase or
`ReadConfig`: `Boost`, `Chorus`, `Delay`, `DelaySync`, `Disto`, `Distortion`,
`Echo`, `Equalizer`, `Highpass`, `Lowpass`, `Notch`, `Octaver`, `Phaser`,
`Pitch`, `Reverb`, `Tremolo`, `Volume`, `LFO`, `None`.

**Parameter names**: `Decay`, `DelayTime`, `DryWet`, `Feedback`, `Frequency`,
`Gain`, `GainBand1`–`GainBand6`, `Q`, `Shift`, `Slider`, `Sync`, `Pitch`, `amp`.

**Config keys** not previously seen: `favorite_banks`, `fbk_onoff`, `fbk_params`,
`gain_master`, `gain_preamp`, `calibration_on`, `input_level`, `output_level`,
`nbreso`, `f0`, `f1`, `afmin`/`afmax`/`afstep`, `agmin`, `wireless`, `guitar`.

Caveat worth keeping: a dictionary entry proves the string is known to the
firmware, **not** that the method is callable or that its parameters are what
one would guess. Each needs its own hardware test, and `--diagnose` is the shape
that test should take.

## Findings — the vendor's licensing posture (2026-08-27)

Checked because the project's footing depends on it. **Not legal advice**; this
records what was actually found, so a future decision starts from facts rather
than assumption. Anything commercial warrants a real lawyer.

**L1 — No EULA reaches the user.** The app's `terms_of_use` string resource is a
bare twelve-character *label* ("terms of use"), not a document; registration
shows an "I agree with the terms of use" checkbox with **no terms body and no
URL bundled anywhere in the APK**. The only http(s) URLs in app resources are
Google/Firebase policy links, the Firebase instance, and a Play Store link.

**L2 — No anti-reverse-engineering clause exists to breach.** Zero occurrences
of "reverse engineer", "decompile", or "disassemble" across the app's entire
string resources.

**L3 — The bundled privacy policy is generator boilerplate with an unfilled
placeholder.** Effective 2020-07-27; it refers to "our Terms and Conditions,
which is accessible at HyVibe" — the site name was never substituted for a
link. `hyvibeguitar.com/terms-and-conditions/` returns **404**.

**L4 — The only site-wide legal document is a Terms of *Sale*.** It governs
e-commerce (sales, shipping, returns), not software licensing. Its IP clause
(Article 13) reserves rights in **website** elements only — not the purchased
product, its software, or its firmware.

**Consequence for Phases 0–3 (the client).** The sharpest constraint on
interoperability work — a contractual anti-RE clause in an accepted EULA — is
simply **absent** here. What remains is copyright, where intermediate copying to
extract interface facts for interoperability is long-settled fair use in the US
(*Sega v. Accolade*, 9th Cir. 1992; *Sony v. Connectix*, 9th Cir. 2000), and
where interfaces and functional facts are not protectable expression in the
first place (17 U.S.C. §102(b); *Google v. Oracle*, 2021). DMCA §1201
anti-circumvention does not engage: a standard APK is an ordinary ZIP with no
technological protection measure to circumvent, and §1201(f) exempts
interoperability reverse engineering regardless. Trade-secret law treats reverse
engineering as a proper means of acquisition absent a confidential relationship;
there is none. The operative discipline is therefore the **expression boundary**
in `DOC_POLICY.md`, which this repo already keeps.

**Consequence for Phase 4 (firmware) — materially different, and this is the
real reason the phase stays gated.** Replacing firmware is not the same legal
act as talking to a documented-by-inspection radio protocol:

- If the device *does* verify firmware signatures, that verification is
  plausibly a technological protection measure, and defeating it engages DMCA
  §1201 in a way the client work never does. §1201(f) may or may not cover it;
  the exemption is narrower than it is usually assumed to be. Recall F7: signing
  is **unknown, not absent**.
- Vendor firmware images are the vendor's copyrighted work: they may be analysed
  but never redistributed, patched-and-redistributed, or vendored into this repo.
- Warranty consequences are real and separate from copyright.

None of this blocks Phase 4; it defines what its assessment must actually
answer, alongside the technical blockers already recorded.

**Provenance-of-the-APK caveat (repeat of the teardown note, relevant here).**
The analysed build came from a third-party mirror (APKPure), not Google Play,
and its signing certificate carries a generic placeholder subject rather than a
HyVibe developer key — so authorship is not independently attested. Phase 1's
hardware control run is what makes this moot: whatever the real guitar answers
is the truth, whatever the APK said.

## Decisions (Mark's — pending)

- **D1 — License. DECIDED 2026-08-27: `MPL-2.0`**, matching the retinue family
  rather than woodshed's `MIT OR Apache-2.0`. `LICENSE` is byte-identical to
  retinue's copy. Two consequences worth recording, because they were not the
  reason for the choice but they fall out of it well:
  - **File-level copyleft is the right shape for a protocol crate.**
    Improvements to ringdown's own files stay open, while the crate can still
    be linked into differently-licensed applications — including a proprietary
    one, which a hard copyleft would have foreclosed.
  - **It keeps the Phase 4 FX path open.** MPL-2.0 §3.3 permits distributing a
    Larger Work under a **Secondary License** (GPL 2.0+, LGPL 2.1+, AGPL 3.0+)
    so long as the covered files are not marked "Incompatible With Secondary
    Licenses" — and ours are not. So the GPLv2 tension flagged against Guitarix
    (Phase 4) does **not** bite ringdown: MPL-2.0 code may be combined into a
    GPL work. Had this gone MIT/Apache the answer would also have been fine;
    the point is that MPL-2.0 costs nothing here and is not the trap a
    copyleft-adjacent license might look like at first glance.
- **D2 — Claim the crates.io name. DONE 2026-08-27.** `ringdown` 0.0.1
  published to crates.io (MPL-2.0), confirmed live via the registry API
  independently of cargo's own success message. The name is claimed with a real
  publish rather than an intention, per the heddle lesson. Note for the ledger:
  the crate is a **stub** — a documented `no_std` lib with no protocol code —
  which is the point. The reservation exists to hold the name while Phase 1 is
  built, exactly as `mora` and the signalman/postilion/linkboy trio were
  founded.
- **D3 — Phase 3 scope.** Which "beyond the app" capability leads. Set after
  Phase 1.
- **D4 — Is Phase 4 (firmware) ever entered?** Open. Requires its own
  assessment doc first regardless.

## Open questions (answered by Phase 1, not by debate)

- Does the guitar require BLE bonding / pairing?
- Exact STM32 and ESP part numbers (`cpuID`, version strings).
- The real effect catalog and every parameter's range/unit/scaling.

---

## Progress

- **2026-08-27** — Repo founded: `git init`, `.gitignore`, `DOC_POLICY.md`
  (canonical core + ringdown addendum), `DOC_README.md`, this plan. Protocol
  map recorded in Findings from the v1.1.2 APK teardown.
- **2026-08-27** — Vendor licensing posture checked and recorded (Findings
  L1–L4): no EULA reaches the user, no anti-reverse-engineering clause exists,
  and the only site-wide legal document is a Terms of Sale governing
  e-commerce. Consequences split by phase; Phase 4's are the ones that bite.
- **2026-08-27** — Terminology corrected across all four docs: the work is
  **reverse engineering for interoperability**, not clean-room. The earlier
  "clean-room" wording was an overclaim — that term denotes a personnel split
  (one party reads, a separate party implements) which this project does not
  have. `DOC_POLICY.md`'s addendum now names the rule "expression boundary" and
  explains the distinction, and records how a genuine split could be arranged
  later from the Findings spec if one is ever wanted.
- **2026-08-27** — D1 decided (MPL-2.0). `LICENSE` installed, workspace and
  `crates/ringdown` stub created; `cargo build` and `cargo package` clean.
- **2026-08-27 — PHASE 0 LANDED.** Initial commit on `main` (branch renamed
  from git's `master` default to match the family's four other repos).
  `ringdown` 0.0.1 published to crates.io under MPL-2.0 and confirmed live via
  the registry API.
- **2026-08-27 — Phase 1 transport layer landed** (the half that needs no
  hardware). `llt` (framing, chunking, ack parsing) and `handshake` (the
  connect-time version banner) implemented sans-io on `alloc`; 21 tests and a
  doctest green, clippy and fmt clean, zero warnings.
  - Three findings were recovered while grounding the code and are now recorded:
    **F9** the ordered connect sequence including write-with-response on the
    request characteristic, **F10** the pre-RPC plain-text version banner in two
    forms, and **F11** that inbound has no reassembly and the response
    characteristic multiplexes acks with replies.
  - F11 leaves a real open question — how an over-MTU response arrives at all —
    which is deliberately **not** solved in code. Writing speculative inbound
    reassembly would violate the repo's provenance rule; it waits for a captured
    `ReadConfig`.
  - Next: `rpc` (the JSON-RPC envelope and the 32 typed methods) is also
    desk-doable. The phase does not close until `GetStatus` answers from the
    physical guitar.
- **2026-08-27 — RPC layer landed**, all 32 methods from one macro table, plus
  Findings F12 (numeric `jsonrpc`) and F13 (the params key table).
- **2026-08-27 — transport + probe landed**, and the probe made **first contact
  with a real guitar**. Results in Findings H1–H3: the GATT surface and the
  version banner are now hardware-verified; `GetStatus` is unanswered. Two
  corrections fell out of the run and its preparation:
  - The scan filtered on the advertised service UUID, which would have produced
    a false negative on any device that does not advertise it. Now matches on
    service *or* name, recording which.
  - `Version::parse` was stricter than the vendor's, which strips all non-digit
    characters per component. The System Menu displays versions as `V1.2.3`, so
    a banner spelled that way would have been rejected by us and accepted by the
    vendor's client. Now matches their permissiveness.
  - The receive path discarded unrecognised notifications, making silence and
    an unexpected reply look identical. Timeouts now carry what was overheard.
- **2026-08-28 — desk work, no instrument involved.** Two pieces, both closing
  things that had been left open rather than opening anything new.
  - **The loop header is decoded (H20).** `ringdown::loopfile` parses a
    recording's WAV header and the vendor's `JUNK` chunk. `200` is the tempo and
    `7 × 4 = 28` the length in beats, established against the audio's own
    duration and confirmed to the byte once 2048-sample block rounding is
    accounted for. Which of the two length fields counts bars is deliberately
    *not* concluded — one file cannot separate them — and `probe --index` reads
    every loop's header in one round trip each to collect what will.
  - **Every method's params shape is bound (H21).** `rpc::param_shape` covers
    all 32, checked against the `params::*` constructors by test. Doing it found
    `"den": null` going out on every metronome write, contradicting the
    invariant `woodshed-instrument` claimed for itself. Fixed.
  - 15 + 6 new tests; 102 in `ringdown`, 3 in the probe; clippy and fmt clean.
  - **Not yet reaching woodshed.** It takes ringdown as a git dependency, so
    both changes need a push before its build sees them. The `woodshed-instrument`
    test run on 2026-08-28 passed against the *pushed* ringdown and therefore
    says nothing about either change.
- **2026-08-28 — `probe --index` run against the instrument; H20 corrected to
  H22.** 31 loop headers in about a minute, one round trip each. The header is
  a time signature, a bar count and a completion flag; `beats_per_bar × bars`
  beats at `tempo_bpm` (counting `den` notes) block-rounded to 256 samples
  reproduces all 24 complete takes exactly, and the 7 marked otherwise are all
  short of the grid. `loop0031` is 200 BPM, 7/8, 4 bars, confirmed by the owner.
  - Three claims fitted to one file were wrong: the block is 256 not 2048, the
    two length fields are separable, and the last value is not spare. The
    corpus test in `loopfile` now pins all 31 observations, so a model that fits
    one file and not the library cannot pass again.
  - `den` **is** the denominator. The `ReadMetronome` reading that argued
    against it is itself the anomaly, alongside `UpdateMetronome` refusing to
    restore `den: 8` — two methods mishandling a settled field. Woodshed keeps
    refusing to write `den`, now for that reason rather than for not knowing
    what it means.
- **2026-08-28 — `den` writes; two bugs found, one ours (H24).** The owner
  disproved "read-only for everyone" with the vendor app in a minute, which
  forced the real investigation. serde_json's alphabetized keys were dropping
  `den` on the firmware's order-sensitive parser (`preserve_order` now enabled
  and pinned by test), and with order fixed an exhaustive 1–32 + 256 sweep
  showed the firmware whitelists `{1,2,4,16}` — silently refusing 8 and 32,
  which its own panel offers. Race, transform, compound-meter, den>num,
  transport-encoding and exponent-pow2 theories each killed by one targeted
  write. `--call pause` added to the probe for in-connection settle tests.
- **2026-08-29 — preset switching works; the bank model was wrong (H25).** The
  guitar's nine panel presets *are* the banks, numbered 0–8 in grid order, and
  `SwitchBank` moves the panel selection — confirmed by the owner reading the
  screen for two different indices. This voids the long-held assumption that
  banks were app-side and the instrument held none; that came from `ReadBank`
  returning `""`, which it does for every bank regardless of content. `ReadBank`
  simply does not report bank contents.
  - `AddEffect` validates the type: 11 of the dictionary's 19 names are
    insertable, the rest being aliases, non-effects, or gain stages (H24b).
  - Accepted adds still produced no sound, but they were aimed at bank 0 while
    a different bank was audible, so audibility is untested rather than
    disproved.
  - Correction worth carrying: `bank_num: 0` is the owner's first preset, not
    scratch space. Effect writes today went into their `rev`.
- **2026-09-01 — the transport is a seam now, not a weld.** `ringdown::link::Link`
  is four methods (write, read the response characteristic, next notification
  within a duration, disconnect); everything above it is transport-agnostic.
  The driver moved out of `ringdown-ble` into `ringdown-client`, because while
  it lived beside the btleplug code a CoreBluetooth or Web Bluetooth client had
  to depend on btleplug to reach it — the dependency it was trying to escape.
  `cargo tree -p ringdown-client` now shows zero btleplug, which is the
  assertion that matters and is worth re-running if anyone touches the layout.
  - Layout: `ringdown` (protocol + Link) → `ringdown-client` (`Guitar<L>`) →
    `ringdown-ble` (discovery, connect, `BtleplugLink`, and a `Guitar` alias so
    callers still write `Guitar`).
  - `connect` became a free function in `ringdown-ble`: Rust forbids inherent
    impls on a foreign type, and `Guitar` is foreign there now. Arguably where
    it belonged — finding an instrument is the platform's job, driving one is
    not.
  - Deliberately absent from the trait: discovery, pairing, reconnection. They
    differ enough between platforms that abstracting them now would produce a
    trait shaped like whichever platform was written first.
  - Two regressions the compiler could not catch, both found by review: moving
    notification tracing into the link silently broke `--trace`, and the split
    left discovery's tests in the crate that no longer had the functions.
    Hardware-verified after each phase rather than only at the end.
- **2026-09-01 — the chain methods all answer `true` on slot 8.** `SetBankName`
  (`"ringdown"`), `SustainKiller` (`killed: false, reset: false`), `MoveEffect`
  (0 → 1), `UpdateEffect` (index 0 → a Tremolo) and `RemoveEffect` (index 1),
  `GetStatus` fine after. By H27's rule none of these `true`s is evidence of a
  change; the bank-name one is the only write here with a **panel** read-back,
  and that observation is pending. Typed `Effect`/`Parameter` builders added
  to `rpc` so wire order and the established vocabulary are enforced by the
  compiler rather than remembered.
- **2026-09-01 — first panel-confirmed write, and a chain cap.** The owner
  reads slot 8's name on the grid as **`ringdown`** after `SetBankName`: the
  first `true` in this project verified by the instrument's display rather
  than by ear. Slot 8 with one live Distortion and nine `bypass: true` ones
  sounds "much more like one than a wall" — `bypass` is honored, **subject
  to** the owner's report that a chain holds **at most four effects**, which
  the app's own bank editor confirms (four rows, then no "Add an effect"). If
  the firmware also caps at four and silently drops the rest, that is H27
  again and confounds the bypass reading; test pending on a reset slot. Also:
  the panel cannot edit an effect's parameters at all — only the app can —
  which is exactly the surface a desktop client earns (woodshed W3).
