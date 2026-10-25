# Client surface plan: desktop performance control, Beats, and scripting

**Date:** 2026-09-02
**Updated:** 2026-10-20.
**Status (2026-10-20):** in progress; standards-based MIDI/Beats split passes bounded native/feature tests and a read-only live RPC check; bounded provenance/terms assessment complete, publication questions open. Shared data-defined audio packs and
sequencer reuse are planned under the root audio consolidation plan. AP0's
bounded embedding assessment and AP1 shared kit/export implementation are complete.
AP2 adds `ringdown-probe --audio-pack` as an offline branch of the existing
CLI, sharing authored-pattern/snapshot open and MIDI export with Woodshed and
Hocket. The current build split below excludes private adapters by default and writes no guitar files.
Woodshed now has a shared Effect Next/Previous pedal panel and same-owner JSONL
scripting, with bounded native and owner-observed guitar acceptance. Independent Ringdown
desktop UI and a shipping guitar export profile remain open. AP3 now
has scoped custom-file discovery/playback, reboot and five-bar intro/loop
evidence; that does not establish broad firmware or meter coverage.
The desktop
performance architecture assessment is complete and the first Mac
software-pedal Effect Next trigger has an owner
confirmation. P0 firmware/capability qualification and production performance
implementation remain open. Phases A–C
landed (77207ef, 3611c55, 86c266a); B/C have scripted-link evidence, not device
qualification of the complete typed/profile path. Phase D closed on hardware
2026-09-02: a client can create a playable bank (H47). Those older receipts
do not establish behavior on the owner's recently updated firmware.

Findings that ground this live in `2026-08-27_ringdown_founding.md` (H25–H38)
and the session handoff `2026-09-01_effects_handoff.md`. This plan does not
restate them; it says what the client's surface becomes because of them.

## Objective

Make Ringdown an independent cross-platform desktop performance controller
for a HyVibe guitar: prepare tones and song cues, trigger supported actions from a screen,
keyboard, MIDI controller or script, and control the instrument's own Beats
rhythms where the firmware exposes them. HyVibe continues to add instrument
features; Ringdown should extend access to those features through desktop and
automation workflows.

The typed writes and serializable shadow profile from A–D supply the
configuration foundation. The next target is a software equivalent of the
Bluetooth stomp box, assessed together with Beats. Woodshed consumes this
capability; running Ringdown must not require launching the practice toolkit.

## Standards-based MIDI and Beats build isolation (2026-10-20)

**Status: implemented locally; bounded host runtime, dependency and read-only RPC qualification.**
The existing crate and CLI boundaries now provide an independent standards-based
build. No second MIDI driver, sequencer, pack loader, RPC session or app was added.
This is an artifact-composition change, not legal clearance or production
BLE-MIDI-peripheral qualification. The unsent outreach draft remains below.

### Owners and source boundary

| Owner | Current responsibility |
|---|---|
| [`woodshed-instrument`](../../../crates/woodshed-instrument/src/lib.rs) | The existing MIDI ports/messages/clock implementation moved from `woodshed-audio` without a second copy. Default feature `midi` selects midir; there are no private RPC dependencies, including optional ones. |
| [`woodshed-audio`](../../../crates/woodshed-audio/src/lib.rs) | Re-exports that MIDI module and the existing public MIDI types. Existing desktop hosts keep using their current MIDI backend and clock worker. |
| [`ringdown-ble::instrument`](../crates/ringdown-ble/src/instrument.rs) | The former `woodshed-instrument` private connection/state/metronome wrapper and its session, moved together beside their transport. The hardware-writing metronome example moved here too; it was compiled, not run. |
| `audio-pattern` / `audio-packs` | Unchanged authority for sequencer data, MIDI-file encoding, original kit admission and retained rhythm exports. `/Drums` discovery and intro/body behavior remain scoped compatibility facts. |
| [`ringdown-probe`](../apps/probe/src/main.rs) | One existing executable, now defaulting to pack/MIDI-file export and standard OS MIDI-port commands. Private RPC and PacketLogger modules require separate feature opt-ins. |

### Build profiles and commands

Run in `ports/ringdown`. `apps/probe` is the workspace's sole default member;
ordinary `cargo build` no longer selects the private workspace libraries.

```sh
# Standard MIDI ports plus shared Beats-file authoring; private code excluded.
cargo build --locked --release -p ringdown-probe

# File authoring only: no MIDI driver or private adapter.
cargo build --locked --release -p ringdown-probe --no-default-features

# Explicit private research builds, with independent opt-ins.
cargo build --locked -p ringdown-probe --no-default-features --features hyvibe-rpc
cargo build --locked -p ringdown-probe --no-default-features --features packetlogger
```

The existing CLI now supports:

```sh
ringdown-probe --audio-pack bundled backbeat new-beat.mid new-rhythm.json
ringdown-probe --midi-ports
ringdown-probe --midi-send 'Exact OS output name' 1 cc 67 127
ringdown-probe --midi-send 'Exact OS output name' 1 cc 67 0
```

The illustrative output name must be replaced with an actual listed port.
Channel numbers are 1–16 at the CLI; message numbers/values are 0–127. `cc`,
`note-on`, `note-off` and `program` messages are explicit owner choices, not
hard-coded guitar actions. A send reports OS acceptance, not observed guitar
state. These commands send one message each; held Note On messages need an
explicit Note Off. All arguments are validated before opening the output.
The current bundled pattern identity is shown in the kit definition; use that
identity rather than assuming an installed pack has the same pattern names.

No arguments prints help. Private RPC scanning requires both its compile-time
feature and the `--rpc` command; `--rpc --help` lists the research options.
`--decode-pklg` requires `packetlogger`, which does not imply BLE/RPC transport.
`--workspace` and `--all-features` are research configurations, not the independent
standards-based shipping configuration.

OS MIDI ports reuse the existing midir implementation. This change does not
advertise a BLE peripheral, pair a guitar, select a universal pedal binding,
start Beats remotely or copy files to a mounted guitar. The earlier Swift
peripheral remains a diagnostic. Production peripheral roles on Windows,
Linux and macOS remain a separate transport qualification target.

### Artifact evidence and remaining scope

[`check-instrument-boundary.py`](../../../scripts/check-instrument-boundary.py)
walks the selected normal/build dependency graph, excluding dev dependencies,
and rejects `ringdown`, `ringdown-client`, `ringdown-ble` and `btleplug`.
The [composition receipt](../validation/standards-boundary_20261020/receipt.json)
records source hashes, checks, selected target graphs and compiler inputs.
Full Cargo metadata and lockfiles can contain unselected private workspace
members; their presence is not evidence of binary inclusion.

The default and file-only graphs exclude private adapters for macOS, Windows
and Linux. Windows/Linux dependency selection is not compilation or execution
on those systems. The initial composition receipt covers host compilation and
release builds, including the opt-in research configurations and relocated
private example. It predates the runtime acceptance below; its frozen claim
that no tests/device commands ran applies to that initial pass only. The root
audio facade is qualified separately in that receipt.

This repository and the CLI source directory still contain research modules.
A whole-repository/source-package archive is not a standards-only source
artifact. Independent binaries must be distributed with their applicable code
licenses/notices and original-asset provenance; imports/retained user samples
carry their own rights. This split resolves binary dependency selection, not
the audit's app-provenance, standards, trademark or source-derivation questions.

### Split acceptance (2026-10-20)

**Result: bounded host acceptance passed; no production behavior fix needed.**
Mark requested runtime verification of MIDI, Beats-file export, default artifact
exclusion and RPC opt-in, with a deliberate guitar checkpoint. The
[acceptance receipt](../validation/standards-split-tests_20261020/receipt.json)
retains commands, test logs, fresh artifact hashes and graph/compiler inputs.
The [integration cases](../apps/probe/tests/standards_cli.rs) launch the actual
CLI; the same cases also run against preserved release executables through
`RINGDOWN_TEST_BINARY`. Its feature profile must match the test build.

| Area | Observed result |
|---|---|
| Standard MIDI | Real OS virtual-input capture received exact CC-67 press/release, Note On/Off and Program Change bytes from both the default/debug and preserved default/release CLI. Channels 1/16 and 0/127 data boundaries are covered. Malformed channel/data/type/arity inputs fail before opening a port; a valid command to a missing port reports failure. The native capture fixture needs a functioning macOS/Linux MIDI subsystem; this run used macOS CoreMIDI. |
| Shared Beats-file export | Bundled and copied user-installed kits export format-0 SMF through the existing shared owner. The installed test kit uses a distinct ID and 120 BPM; deleting its directory and reopening retained rhythm JSON reproduces identical MIDI bytes. Bundled exports reopen identically and existing output files are preserved on refusal. |
| Independent SMF inspection | The existing AP3 read-only inspector confirms one track, 480 PPQ, 4/4, 12 paired kick/snare/hat notes and 1,920 ticks for the bundled 100-BPM bar. Default and file-only release bytes match. This is shared SMF authoring evidence; no new guitar-file delivery or physical Beats playback occurred. |
| Default artifacts | Actual plain workspace `cargo build --release` and its `--no-default-features` variant select no private protocol targets or private RPC/PacketLogger source modules. File-only also excludes midir/instrument I/O. Fresh macOS, Windows and Linux normal/build graphs pass the boundary check. |
| Negative controls | The same checker rejects both private feature selections. `hyvibe-rpc` selects the core/client/BLE adapter; `packetlogger` selects its private codec without selecting the BLE adapter. These controls show the checker does not merely return success for all profiles. |
| RPC opt-in software | CLI help and malformed-argument routing work without scanning. Private core/client/BLE tests pass, including the relocated connection/session cases and ReadConfig/write-refusal guards. Independent feature and combined builds pass. |
| Read-only owned guitar | With Mark's readiness confirmation, the preserved RPC-only binary discovers the guitar, reads its banner and GetStatus, then requests disconnect and exits 0. Status reports H2S, DSP V2.4.0 and connectivity V1.3.0. No settings, recordings or guitar files changed. |

There are **284 passing test executions**, including repeated CLI cases across
feature profiles and the two preserved release executables, with zero failures
or ignored cases in these runs. This is a bounded set, not a full product suite.
The counts and per-run logs are in the receipt. Raw live output stays private;
the retained public log redacts the instrument name/address and processor ID.
The existing probe discards the disconnect result, so its final line establishes
that disconnect was requested and returned, not a separately verified link
acknowledgement. GetStatus acceptance does not qualify every RPC method.

The tested code keeps one shared MIDI implementation, one sequencer/exporter
and one private session. Windows/Linux execution and production BLE-MIDI
peripheral roles remain unqualified. Earlier AP3 physical custom-beat/loop
receipts keep their original scope; no persistent guitar write was requested
or performed here. The outreach draft remains unsent.

## BLE-MIDI peripheral transport (2026-10-20)

**Status: shared implementation landed in the working tree; bounded Mac guitar trigger observed.** Mark requested a three-platform reuse
assessment followed by integration into the existing MIDI path, without a
parallel driver/session. The product needs the computer to advertise a GATT
peripheral; the guitar is the central. Existing private `ringdown-ble` and
btleplug remain separate, and the earlier Swift diagnostic is frozen evidence.

### Reuse assessment

- [btleplug](https://github.com/deviceplug/btleplug) and
  [Bluest](https://github.com/alexmoon/bluest) implement central/client roles;
  neither supplies this peripheral role. Keep them out of the standard path.
- [ble-peripheral-rust](https://github.com/rohitsangwan01/ble-peripheral-rust)
  0.2.0 at `3135b029bd9f3e57c871039b087080bf511f6fe3` supplies all three native
  backends under MIT: CoreBluetooth, BlueR/BlueZ and Windows WinRT. It is the
  selected reusable base, with retained source/license in
  `vendor/ble-peripheral-rust`; no parallel platform implementation is added.
- Source inspection found notification backpressure/errors discarded on Mac
  and Linux, a Mac process singleton that prevents clean reopen, service/
  advertising callback ordering races, and platform-specific subscription and
  shutdown gaps. These are repaired in the single supplier. Notifications retain
  their native outcome rather than reporting every attempt as sent.
- [Bluster](https://github.com/dfrankland/bluster) has older platform bindings
  and no Windows backend in the inspected manifest; BlueR directly is a good
  Linux substrate but not a replacement for the three-platform supplier.

### Platform and integration contract

| Platform | Reused interface | Runtime boundary |
|---|---|---|
| macOS | CoreBluetooth CBPeripheralManager and generated objc2 bindings | Serial delegate callbacks, explicit service/advertising completion, targeted subscription notifications and queue-full refusal; application Bluetooth usage metadata required. |
| Linux | BlueR local GATT/advertising over BlueZ | Running bluetoothd, D-Bus policy and a peripheral-capable adapter; retain GATT registration while stopping advertising, cancel tasks and propagate notification writes. |
| Windows | WinRT GattServiceProvider/GattLocalCharacteristic | Check peripheral-role support, observe advertising completion, target a subscribed client, inspect notification result and remove registered handlers. Windows controls the advertised local name. Notification-encryption enforcement is not established through this API, so the encrypted choice refuses registration; explicit unencrypted compatibility is implemented. Packaging capability and hardware/driver support need native qualification. |

`woodshed-instrument::midi::MidiOut` remains the writer consumed by the existing
host/clock path and Ringdown CLI. BLE is another connection variant, sharing
`send_raw` and the named message helpers. One shared encoder handles complete
short MIDI messages and the BLE 13-bit timestamp; platform code has no MIDI
message or guitar-action knowledge. File-only builds keep the radio excluded.
The private RPC adapter remains an independent opt-in.

Default access requires encryption. The earlier guitar trial only subscribed
with explicit unencrypted compatibility access; retain that visible owner
choice and never silently downgrade security or claim full standards conformance.
Messages require a current subscription, are bound to its generation and are
not replayed after reconnect. Congestion, disconnected, malformed, expired and
uncertain outcomes must be reported; OS acceptance is not a musical observation.
SysEx/stream parsing and live latency/phase guarantees are outside this slice.

Platform documentation: [Apple peripheral role](https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/PerformingCommonPeripheralRoleTasks/PerformingCommonPeripheralRoleTasks.html),
[Windows GATT server](https://learn.microsoft.com/en-us/windows/apps/develop/devices-sensors/gatt-server),
[BlueZ characteristic API](https://raw.githubusercontent.com/bluez/bluez/master/doc/org.bluez.GattCharacteristic.rst),
[BLE-MIDI specification entry](https://midi.org/midi-over-bluetooth-low-energy-ble-midi).
No gated specification file was downloaded. Sources establish interfaces, not
platform execution or standards-body qualification; preserve those distinctions
in the completion receipt.

### Implemented ownership and operation

- Shared transport: `woodshed-instrument::midi::ble`, an optional `ble-midi`
  feature on the existing writer. `woodshed-audio` enables it by default and
  re-exports the one MIDI implementation. Ringdown's default CLI includes
  OS ports, BLE pedal output and pack/SMF export; `--no-default-features`
  remains file-only, and `--no-default-features --features midi` is OS MIDI only.
- Desktop output choices use the existing picker, backend, clock worker and
  status line. Listing does not open a radio; an absent OS-port service does
  not hide the independent BLE choices. Open/send failures reach the existing
  readout. The worker closes an old output before opening a replacement and
  joins at host shutdown. Reconnection during playback requires a fresh
  stopped/disabled state before clock resumes; no Start/clock backlog is replayed.
  A failed/uncertain Start or clock send also stops clock output until explicit
  rearming; a later tick cannot hide the original failure.
- The persistent CLI uses `--ble-midi Ringdown`, with `status`, `reopen`,
  `send 1 cc 67 127`, `send 1 cc 67 0`, and `quit`. Add
  `--unencrypted-compatibility` only as an explicit security choice. Reopen
  closes the existing writer before creating a new one and sends no MIDI.
  Arbitrary messages do not assign guitar bindings; channel 1 / CC 67 means
  Effect Next only because the owner selected that binding on the instrument.
- One bounded command slot, a current subscriber generation and a 100 ms
  pre-send deadline prevent delayed control replay. Native failure or an
  uncertain completion is surfaced without automatic retry. Read/write
  handling is bounded; Mac rejects multi-request write batches atomically,
  and WinRT handlers use deferrals instead of blocking callback threads.
- Linux uses one retained BlueR 0.17.4 source distribution under the supplier,
  with only notification-encryption flag exposure/passthrough changed. Its
  original BSD license is retained. BlueZ must accept those flags; the minimum
  compatible BlueZ version is not established. Registration failure does not
  fall back to plaintext. Radio power and system adapter names are not changed.
- Existing package scripts retain supplier MIT notices, plus BlueR's BSD
  notice on Linux. The Mac app and CLI carry Bluetooth usage metadata.
  Package scripts were edited; no release archive, installer, signing,
  notarization or publication was performed.

### Bounded qualification and remaining cues

[Receipt and reproduction commands](../validation/ble-midi_20261020/README.md)
retain source/binary hashes, supplier provenance, compile logs, dependency
closures and the owner-observed action. The locked/offline native Mac CLI
build and Woodshed desktop host check pass. Windows GNU and Linux GNU Rust
target checks pass; Linux uses actual Debian D-Bus/ALSA/systemd development
packages in a scratch sysroot. Those checks do not link or execute Windows or
Linux applications. Native Linux SSH access was unavailable in this session.
Default selected dependency closures exclude private RPC and btleplug on all
three targets; previous split receipts remain historical.

On Q-PC, with USB OFF and explicit unencrypted compatibility, the guitar
subscribed and macOS accepted a channel-1 CC-67 press/release. After the owner
configured Effect Next, the owner observed exactly **one** effect advance.
An earlier accepted pair preceded that binding and has no musical conclusion;
the owner explicitly requested the repeated trial. The trigger binary and
later CLI-only reopen addition have separate hashes; shared MIDI and Mac
supplier source hashes are unchanged across those two binaries. Shutdown and
reopen results are recorded separately from the musical observation.

No new automated test suite was added or run in this implementation pass.
The earlier 284 split test executions are not BLE-MIDI tests. Native Windows/
Linux pairing, encrypted interoperability, congestion/disconnect fault cases,
MIDI Clock jitter/phase, broader pedal actions, GUI hardware interaction,
signed distribution and standards conformance remain qualification cues.
There is still no guitar-menu evidence for a standalone Beats start/stop
binding or MIDI Clock reception. Beats export/local authoring and the Mere
composition direction keep their existing owners and plans.

### Desktop path and reconnect qualification (2026-10-20)

**Status: bounded macOS desktop and guitar reconnect acceptance complete.**
[Receipt, native scenarios and captures](../validation/desktop-midi_20261020/README.md)
retain the exact frozen native binaries, source hashes, independent receiver
records and failed attempts. Native Windows/Linux operation remains open.

| Path | Observed result |
|---|---|
| Actual desktop picker, clock toggle, Play/Stop, refresh and None/reopen | 293 OS-accepted messages and 293 independently received messages: three Starts, three Stops and 287 clock ticks. No messages on idle reopen or playback reconnect before explicit rearming. |
| Endpoint removed/recreated while playing | 131 OS-accepted messages and 131 received; one detected disconnect, paused output and retained failure after reconnect. No replay before rearming; two Starts and one final Stop across the two receiver lifetimes. |
| Guitar through actual desktop BLE picker | Subscription, guitar MIDI OFF detection without another desktop click, MIDI ON reconnection, desktop None/reopen and a second resubscription; zero attempted/sent/failed musical messages, clock disabled throughout. Reopening the desktop service required one owner MIDI OFF/ON toggle to resubscribe. |
| Focused software gates | 36 passing executions: 12 host/selection cases, 17 shared instrument cases, seven real-CLI regressions. Eighteen new regressions cover worker/subscriber/queue/lifetime faults. Final native desktop build and Windows/Linux shared-transport target checks pass. |

The same output worker now lives in `woodshed-genet/src/midi/worker.rs`; the
old inline loop is removed. A private test seam feeds that production loop
faults, rather than implementing a second scheduler. Transport intent crosses
its existing channel as one snapshot; selection revisions cancel superseded
opens. The shared writer accepts an expected subscription generation, checked
before queueing and again at native execution. A subscriber replacement wholly
between worker polls therefore cannot receive an old Start/Stop/clock intent.

Repairs found through qualification:

- The output menu's None option overlapped the clock button in native hit
  testing. Elevating the existing selector row repairs both painting and input.
- Comparing only confirmed connections missed None during an in-flight open.
  Dispatch now forwards desired intent to the host, which deduplicates it;
  superseded opens are closed without publishing or sending.
- Frame polling publishes asynchronous readiness/errors without another click.
  Refresh preserves selected names through reordering; vanished ports become
  None instead of silently selecting a different indexed destination.
- CoreMIDI reported success after a receiving endpoint disappeared. The failing
  run accepted 123 messages while the fixture received 42. The existing midir
  path now retains the original endpoint identity and a read-only inventory,
  validates liveness before sending, and pauses/requires explicit reconnect.
  It never substitutes a new endpoint by friendly name. This is not another
  MIDI driver or sending connection.
- Failed Start/tick/Stop/close and disconnect reports remain visible with
  attempted/OS-accepted/refused-or-uncertain counters. A successful later tick,
  reconnect or selection cannot silently clear the last failure. Closed worker
  and queue pressure are distinct failures. Timed-out commands are cancelled
  and rejected if execution resumes after their deadline.
- Scenario `wait` waits for idle with a frame budget; it is not a wall-clock
  dwell. Final quiet-interval and hotplug scenarios use an explicit bounded
  elapsed-time observation followed by that existing wait. Earlier attempts
  are retained and are not interval-qualification passes.

The three final native cohorts pass with fourteen presented captures under
isolated synthetic profiles; the normal practice profile and historical
receipts are untouched. The guitar session closed normally and wrote no guitar
files/configuration. No private RPC or guessed guitar Clock/Beats operation was
used. These tests establish observed receiver parity and explicit refusal/
unknown/rearm policy, not guaranteed radio delivery: ordinary MIDI has no
application-level acknowledgement. BLE congestion at a physical controller,
sleep/radio-loss faults, latency/phase, other guitar actions, encrypted guitar
compatibility and the native Windows/Linux matrix retain separate gates.
Existing desktop controls are clock/output controls; a guitar-specific CC/pedal
mapper UI was not added or claimed.

## Software pedal controls and scripting (2026-10-20)

**Status: implemented locally; bounded software/native macOS and owner-observed guitar qualification (2026-10-20).** Effect
Next and Effect Previous are available in Woodshed's existing **Settings → Audio
and MIDI** panel. This advances the shared P1/P2 foundation; independently
launchable Ringdown views, external input mappings, cues, native three-platform
qualification and performance measurements remain open. The acceptance below
qualifies this bounded pedal slice; earlier transport/guitar receipts remain frozen.

### Owners and preparation

- `woodshed-instrument::pedal` owns portable Serde action/binding/report data and
  message validation/encoding. It compiles without the `midi` feature or a native
  driver. The existing probe's raw MIDI parser also uses this encoder.
- `AppSettings.audio_midi.midi_pedal` retains the two bindings through the existing
  settings store. Both start disabled; channel is human-facing 1–16 and MIDI
  number/value is 0–127. No manufacturer mapping is inferred. Configuration and
  enable/disable edits send nothing; a separate Trigger button sends an action.
- Each binding selects Control Change, Note On, Note Off or Program Change.
  CC and Note On can optionally release 50 ms after OS acceptance of the first
  message. Release repeats the CC with its configured release value, or emits
  Note Off with its release velocity. Program Change emits two bytes and has no
  release; standalone Note Off has no release. This is a prepared momentary
  gesture, not a held-input abstraction or verified guitar mapping.
- The Cambium panel reuses current buttons, bounded steppers, status projection
  and `MidiBackend`. Its one-shot request is consumed after dispatch. The native
  key intercept suppresses Enter/Space autorepeat on pedal trigger buttons.
  Connections, queued requests and reports remain transient; reopening settings
  or a project cannot restore a pending trigger.

### One output owner

Desktop buttons and opt-in shell input share `midi::pedal::Dispatcher` and the
existing output worker. At most one pedal action is admitted. Admission requires
an enabled valid binding and a ready selected receiver. The request captures
immutable MIDI bytes, output-selection revision and subscriber generation. Its
first message must start within 100 ms. The worker checks connection identity,
readiness, generation and expiry before **each** message. An optional release is
scheduled by that worker after 50 ms with a further 100 ms admission window;
the gap does not block the clock loop or create a second scheduler. Native
writer calls may still block within existing transport bounds; timing accuracy
is unmeasured.

Disconnect, selection change, expiry or close cancels remaining messages. No
replacement receiver receives a retained press/release. Results distinguish
`queued`, `refused`, `accepted` and `uncertain` and include the number of messages
accepted by the OS. A failure after a press can leave the gesture incomplete:
there is no compensating send to another receiver and no automatic retry.
Accepted MIDI remains **unobserved musical state**, never a guitar ACK. The
existing clock rearm policy and private RPC isolation remain at their owners.

### Shell contract

Launch the **same desktop process** with `--midi-pedal-stdio`. This opts into
JSON-lines stdin/stdout alongside its UI, without opening another output. Select
its output and configure/enable bindings in the UI. With no flag, stdin is not
read. EOF ends the scripting input lane without closing the application.
The existing standalone probe remains a separate alternative owner; do not run
its advertiser alongside the desktop to control the same guitar.

Illustrative JSON requests (replace the target numbers from the actual status):

```json
{"id":"status-1","command":"status"}
{"id":"cue-1","command":"trigger","action":"effect-next","output_revision":1,"generation":3}
{"id":"cue-2","command":"trigger","action":"effect-previous","output_revision":1,"generation":3}
```

Status returns `output`, current `bindings`, and the last `pedal` report. Use
`output.revision` and `output.generation` for each explicit trigger; generation
is JSON `null` for an OS port. A changed target is refused. Trigger replies carry
the request's string `id` plus the terminal `pedal` report with its numeric
command id, action, outcome, accepted-message count and detail. Syntactic errors
return `error`, not an action result. Wait for each reply; do not bulk-buffer live
cues or retry an uncertain action automatically. IDs must be unique in this
stdio session (1–128 UTF-8 bytes); duplicates are refused, not replayed. Input
lines are limited to 4096 bytes and the lane admits at most 4096 command IDs.
This is local process I/O, not a network API, persistent daemon, or new language.

### Initial implementation evidence (before acceptance)

Source inspection established shared ownership, checked channel/value encoding,
transient command state, and generation/revision checks. The implementation
receipt records native build, portable-driver-free check, maintained probe check,
and Windows/Linux shared instrument target checks. No new tests or UI/device
experiments were run in this implementation pass. The earlier owner-observed
channel-1 CC67 press/release advanced Effect Next once; it is an assigned binding,
not a universal default. Effect Previous, new control interaction, stdio routing,
save/reopen, cancellation/partial outcomes and timing needed focused qualification
at that checkpoint; the subsequent acceptance below advances those gates. No
guitar files or settings were changed during the initial implementation slice.

**Progress (2026-10-20):** shared binding/encoding model, retained preparation,
existing-view controls, bounded worker actions and same-owner JSONL input are
implemented. [Implementation receipt](../validation/software-pedal_20261020/README.md)
records source/build evidence and keeps hardware acceptance separate. Mere
composition, shared packs/sequencer and Beats export retain their current plans.

### Keyboard and incoming-controller mappings (2026-10-20)

**Implemented with focused checks, including the shared focus-snapshot gate.**
The [initial input receipt](../validation/pedal-inputs_20261020/README.md) records
the predecessor implementation. The subsequent [input acceptance packet](../validation/pedal-input-acceptance_20261020/README.md)
records the native probes and callback/reconnect repairs below; it does not
relabel the earlier source or its evidence.

Settings → Audio and MIDI reuses the existing pedal panel for each action's
**Learn key / Clear key** and **Learn MIDI / Clear MIDI** controls. Keyboard
bindings accept ASCII letters/digits and arrows, Page Up/Down, Home/End, with
optional Shift/Alt. They are logical keys; Ctrl/Cmd command chords, Enter/Space
button activation, Tab navigation and text/menu editing retain their owners.
Learning records preparation and sends nothing. Duplicate mappings across the
two actions are refused. Inputs start disarmed; **Arm pedal inputs** is live
state, requires a ready output and a valid enabled action, and never persists.
Mapping/output-binding edits and input/output lifetime changes disarm inputs.

`AppSettings.audio_midi.midi_pedal.inputs` stores only mappings. Old settings
without `inputs` reopen with unassigned mappings. Keyboard events enter the
existing shared-host key interceptor; ordinary OS autorepeat is consumed
without another request. Incoming controllers reuse the selected midir input and parser. One callback-side
`ControllerEdges` owner feeds the existing dispatcher independently of rendering;
the frame-polled history serves learning/display only. A monotonic sequence
cursor consumes that history once, including events sharing a timestamp. No
new input driver, output worker, dispatcher, host loop or MIDI-thru path is
introduced. Arming, source epoch, prepared bindings and expected receiver are
published to that owner before slow persistence/rendering. Callback refusals
retain a bounded notice until the UI polls, even if a later worker completion
advances the ordinary last report.

Controller mappings match channel 1–16, Note or CC and number 0–127. Notes
release on Note Off or Note On velocity zero; CC releases at 0–63 and presses
at 64–127. A release must be observed before the first press after learning,
a mapping change or an input-history gap; repeated positive values cannot
retrigger. The callback observes releases even while disarmed. Removed input
endpoints and history overflow/reset disarm. Retired source callbacks and
changed receiver lifetimes cannot inherit armed authority; the view must
acknowledge disarmed before a fresh Arm. Reopening an input is explicit. The
UI forwards None even if the confirmed input is already absent, retiring the
previous requested name before reselection. The predecessor 100 ms cutoff
on frame-polled input was removed: normal native rendering exceeded it and
discarded learning events. Learning starts its history cursor at the click;
history can never replay an action. Live callback admission retains the
existing dispatcher/worker deadlines (100 ms to start, bounded release),
without a second scheduler. Relative encoders,
Program Change input, SysEx and continuous expression are outside this slice.

Accepted keyboard/controller presses capture the output revision/subscriber
generation currently shown to the UI, then use the **same bounded dispatcher**
as buttons and stdio. Busy/unready/expired/stale targets are refused; reconnect
cannot retarget the old action. The existing writer owns optional release,
partial outcomes and retained uncertainty. Input mapping does not establish
musical timing, instrument reception or an audio engine trigger.

**Shared-host correction:** pinned Mere `d1a200b1` discarded winit's
`KeyboardInput.is_synthetic`. Winit 0.30.13 can synthesize non-repeat presses
for held keys on focus regain on Windows/X11, with ordinary character/text
fields. Filtering app key data cannot reliably distinguish them. A narrow
MPL-2.0 supplier patch now rejects synthesized presses at that same shared
event owner, before editing, button activation or shortcuts; releases keep
their existing Tab-navigation path. Normal OS repeat is still filtered by the
app's interceptor. The root Cargo source patch consumes one provenance-pinned
snapshot of `cambium-genet-winit-host`; metadata confirms exactly one host.
All its dependency identities remain the original Mere/Genet sources. No app
host loop or replacement input driver is introduced. The exact source delta,
license, hashes and return-to-owner condition live in
`vendor/cambium-genet-winit-host/PATCHES.md` and the input receipt. The shared
owner can absorb that same change, then its consumed pin and this supplier
patch are replaced together. Focus-snapshot admission has a focused test and
Windows/Linux gate compilation; native Windows/X11 focus interaction remains
runtime qualification, not established by Mac software checks.

### Input acceptance follow-through (2026-10-21)

**Software/native acceptance passes; physical keyboard/controller/reconnect
checks are complete with one unresolved guitar observation in the
[acceptance packet](../validation/pedal-input-acceptance_20261020/README.md).**
This run spans October 20–21 and keeps its start-date artifact directory.

The initial native run found a render-coupling defect: correctly received
controller events appeared in display history but expired before learning or
live frame dispatch. Live admission now runs at the existing parsed-input
callback into the same dispatcher; the old UI-owned edge state was removed.
A subsequent native run confirmed learning/holds and exposed skipped explicit
None intent after input loss. Forwarding selection through the deduplicating
input owner fixes reopening. The repaired configuration cohort records exactly
25 OS-accepted/independently received messages, including an actual one-message
partial press/release, source and receiver replacement, quiet rearm of held
controls and fresh actions. Two fresh processes prove saved mappings and
Clear/reassign edits survive reopening while live arming, ports, learning and
pending intent do not. They add ten exact received messages (35 total) and
visibly retain busy refusal without deferred replay. All 48 distinct focused
cases and the final Mac app/shared Windows/Linux instrument checks pass.
No guitar action is implied by those fixtures.

Scenario key injection calls the production application interceptor and its
consumed-event tail. It is not OS keyboard/focus input. Physical keyboard and
guitar checks form a distinct, owner-gated cohort. The native virtual source
uses the existing midir API only as a test fixture; it is not a product driver.
The physical cohort records 13 real N key events yielding one Next gesture,
one P yielding Previous, and selected-input CC/Note mapping. Its six gestures
produce 12 OS-accepted messages; the owner observes five single changes and
reports no change for the first Note-triggered Previous. That observation
remains unresolved: fresh Next/Previous after reconnect both work, without
proving the earlier guitar result. OFF/ON changes subscriber generation,
disarms and leaves count eight unchanged through explicit rearm/held values;
fresh release/press then works. Connections close after the final observation.
The shared synthesized-focus gate remains source/focused-test-qualified, with
Windows/X11 native focus interaction and other-platform runtime still open.

### Previous source diagnostics (2026-10-21)

**Status: bounded software/native/guitar comparison complete; miss not reproduced.** The earlier Note-triggered Previous with no observed
change remains unresolved. Comparison uses the existing keyboard/Note mapping,
dispatcher, connection and writer; outgoing Previous remains Channel 1 / CC66
127 then 0, independently of the incoming Note binding.

The shared instrument seam adds an opt-in process-local diagnostic journal
(`INSTRUMENT_MIDI_DIAGNOSTICS=1` at startup). It retains at most 512 events with
sequence, monotonic elapsed microseconds, request id where available, output
revision/generation, exact short-message bytes and bounded detail. Overwritten
and truncated evidence is explicit. There is no hot-path file I/O, extra writer,
retry, background logger or default recording. The existing desktop JSONL Status
command exposes a read-only snapshot. Events cover input interpretation,
admission, planned bytes, each writer send, BLE notification packet/result and
subscriber changes. BLE packet events are OS submission evidence, not an air
capture or guitar acknowledgement; sequential single-slot writer events provide
their request correlation.

Done conditions are a focused Note/keyboard writer comparison, native retained
keyboard-hook and selected-input comparison against independent receiving bytes,
and a separate owner-gated guitar comparison with visible effect names. Sparse
and held Note input are distinguished, one gesture is observed at a time, and
an unconfirmed action is not retried automatically. The earlier negative result
is retained even if a new bounded trial cannot reproduce it. Evidence lives in
[Previous diagnostics](../validation/pedal-previous-diagnostics_20261021/README.md).

Current evidence: 38 focused cases, Mac app/fixture build and shared Windows/Linux
instrument checks pass. A fresh native cohort alternates P and Note36 twice:
eight independent receiving messages exactly match the writer trace, with four
CC66 127/0 pairs and observed release gaps of 50.3–51.1 ms. The journal retains
107 events without overwrite. The separate guitar cohort uses real P, sparse
Note36 and Note36 with 32 held repeats. All emit identical outgoing CC66 127/0
messages, with observed writer gaps 50.428 / 50.491 / 50.526 ms and 50 ms BLE
packet timestamp gaps. The owner confirms Rev → Off in all three cases. Three
observed Next anchors plus those Previous actions total 12 attempted/accepted
messages, with one subscription generation, zero disconnects/failures, 124
complete journal events, and both ports closed at completion. No Note-specific
output difference reproduced; this does not establish the historical miss's
cause. The original negative receipt remains frozen. No protocol, timing or
retry change was justified by this comparison.


### Desktop diagnostic controls (2026-10-21)

**Status: implemented; bounded macOS controls, export and reopen acceptance pass.**
The existing Audio and MIDI pedal panel gains Record/Stop, Show/Hide latest
events and Export MIDI diagnostics controls. The one existing journal now allows
runtime opt-in. A normal process starts recording off; the explicit startup
environment opt-in remains supported. No recording preference or history enters
saved settings or projects. Stop retains bounded history; restarting appends
with start/stop boundary events so a deliberate pause is visible. Off uses an
atomic fast path and does not build details or acquire the history lock.

The panel shows recording state, retained/capacity, overwritten and truncated
counts, plus up to 12 recent events. Export includes the full retained snapshot
(up to 512 entries), current output status, bindings and pedal report. Both
scripting Status and desktop export use the same `Dispatcher::status` projection;
only the export wraps it in a versioned JSON envelope. A revision-keyed cache
avoids copying unchanged history for UI polling.

Export freezes immutable bytes on the explicit click, uses the existing rfd
save-dialog/background-job/wake/close-preservation pattern, and creates a new
JSON file. Existing files are refused. Cancel/failure retains history, and a
pending export keeps the window open until completed or cancelled. Recording
changes after the click do not change the requested export. The file worker
never opens MIDI or Bluetooth and does not own musical actions. The portable
MIDI seam provides unsupported-host defaults; no native dependency enters the
core or views, and no new dependency or platform driver was added.

The separate [desktop acceptance packet](../validation/midi-diagnostics-acceptance_20261021/README.md)
records 39 passing focused cases and a native macOS controls/save-dialog cohort.
Starting/stopping through the retained controls captures ten events; an actual
send while stopped reaches the independent receiver without changing history.
Restart appends to 20 events, with zero overwritten/truncated entries. The
latest-event disclosure and counts agree with Status. Native Cancel retains
history and releases busy state. Native Save creates a JSON whose complete
`status` equals scripting Status without its request id. Six messages reach the
independent receiver through the existing desktop writer. Exiting while
recording is on and reopening the same saved profile starts off with no history;
the saved bindings survive and no diagnostic preference enters saved settings.

Both pending-close branches pass in the headed app: an acceptance command uses
the existing host `WindowCommands::close`, observes the production guard's
keep-visible notice, then real native Cancel or Save wakes that guard and allows
normal process exit. Save completes an exact Status snapshot before exit;
neither close cohort attempts MIDI. This qualifies the real host command and
worker/wake paths, not an OS mouse click. Separately, macOS Quit while its save
panel is modal returns User canceled (-128); after cancelling that panel, the
app remains open and a fresh native Quit exits normally. The windowless host
regression also covers worker failure. These are distinct close observations.

The initial interrupted save attempt and superseded driver attempts are retained
as such. A later complete cohort passes with native folder navigation and Save.
Mac desktop compilation passes. Portable-core and Windows/Linux shared-instrument
compilation remain recorded in the earlier implementation packet; Windows/Linux
native controls/dialogs are not qualified here. No guitar was used or needed.
Earlier device evidence stays tied to its original source. See the separate
[implementation/build packet](../validation/midi-diagnostics-ui_20261021/README.md).

The later [current-main integration](../../../validation/ringdown-sync_20261021/README.md)
retains these controls on the current notation/audio cohort, folds upstream
completion/failure projection into the same pedal writer, and updates the
existing host admission patch to that cohort. Merged desktop/Hocket compilation
and 43 focused desktop MIDI/diagnostic cases pass. This does not extend the
historical native/device acceptance. The subsequent
[Windows/Linux diagnostic packet](../validation/midi-diagnostics-cross-platform_20261021/README.md)
now has complete Windows and Linux Xwayland cohorts: default-off same-profile
reopen, retained history across Stop/restart, whole Status/JSON parity, native
Cancel/Save and both pending native close-event branches. Four fresh processes
exit normally on one frozen binary per host; the same 43 desktop cases and one
isolated journal case pass on each. Linux retains 20 events and captures six
independent ALSA messages. Windows retains eight events from explicit no-output
dispatcher refusals and attempts zero MIDI messages. Scrolled readout captures
and independent local JSON rechecks pass. No guitar was used. Native Wayland
and Windows MIDI/BLE delivery remain unqualified; final Windows preservation
metadata collection is pending in that packet.


### Pedal acceptance (2026-10-20)

**Bounded acceptance passes; the final native readout cohort is recorded in the
[acceptance receipt](../validation/pedal-acceptance_20261020/README.md).** This is
separate from the preceding compiler-only implementation receipt.

- **57 focused software cases pass:** 28 host/admission/worker/stdio/selection/key
  cases, 21 shared instrument cases, one settings round trip and seven existing
  CLI cases. Twenty-one cases are new. Channel/number/kind encoding covers every
  channel and number. Fault checks exercise disabled/invalid/stale/unready/busy
  admission, closed worker, first-send uncertainty, failed release, disconnect,
  subscriber replacement, output revision change, deadline expiry and close
  both before and after a press. Repeated Enter/Space is suppressed through the
  actual retained desktop controls and production key policy.
- **Native macOS control and persistence:** real hit-tested input configures both
  actions, cycles all message types, exercises channel/value/release controls,
  enables the bindings and presses both buttons. Configuration sends no MIDI.
  Saved settings exactly match the live preparation and survive a fresh app
  process; output selection, pending actions and reports reopen empty.
- **Independent receiving fixture:** UI and same-owner stdio produce exact
  byte parity. The receiver is removed after it records a press, before the
  scheduled release. The terminal result is uncertain with one OS-accepted
  message, no retained release and no replay after explicit reopen. Offline,
  duplicate-ID and stale-target triggers are refused. Fresh explicit triggers
  succeed on the replacement. The fixture only receives; it is not a product
  driver. Counts, receiver epochs, original bytes and quiet intervals are kept
  in the receipt, rather than inferred from host success.
- **Live guitar, owner-observed:** after the owner cleared/paired Ringdown and
  confirmed channel-1 CC67/CC66 bindings, the actual desktop BLE picker and
  `--midi-pedal-stdio` use the existing writer in explicit unencrypted
  compatibility mode. One Next gesture advances exactly once and one Previous
  gesture moves back exactly once, confirmed separately by the owner. Four
  messages total are accepted by macOS, with zero send failures. MIDI OFF is
  detected; ON resubscribes with a new generation, without changing that count.
  The desktop then closes its output. Clock stays off. No private RPC or device
  file write is part of this session. Exact effect names and firmware are not
  reread; this does not expand earlier firmware/version claims.

**Repairs from qualification:** the visible pedal result now includes its
OS-accepted-message count. `OutputState` retains the last uncertain pedal report
separately from its ordinary last transport failure; disconnect/readiness
updates, later refusals, successful actions and closing the output cannot erase
an incomplete gesture. The history is bounded to that one report, not a durable
journal. The existing worker/writer and generation checks remain the dispatch
path. A terminal stdio input error emits one error and stops the input lane;
oversized lines are drained and permit the next valid command. Tests use the
production stream loop and worker step, with injected streams/connections only
at those existing boundaries.

**Limits:** OS-port removal supplies an actual partial-send observation; injected
writer faults cover refused/unknown sends and release errors. No physical radio
congestion or partial BLE delivery experiment is claimed. This guitar session
establishes the assigned Next/Previous controls only, not arbitrary MIDI
bindings, other firmware, encrypted pairing, timing guarantees, standalone Beats
control or native Windows/Linux execution. Windows/Linux shared-instrument
checks still pass; full native product qualification remains open. Failed fixture
attempts and pre-repair runs are retained separately; all older receipts remain
unchanged. No publication or outreach occurred.

**Progress (2026-10-20):** new controls, retained preparation, same-owner scripting,
partial-outcome reporting and reconnect are qualified to the scope above. P1/P2
remain open for independently runnable Ringdown UI, external mappings, cues,
native platform/runtime coverage and performance budgets.

## Provenance, terms and standards audit (2026-10-20)

**Status: bounded assessment complete; publication questions remain open.**
Mark requested a provenance/terms audit, standards-based compatibility options,
plan updates and an outreach draft. The draft below is saved for review and
has **not been sent**. No vendor contact, guitar access, new decompilation,
firmware download, code migration, test suite, commit or publication occurred.

[Audit receipt](../validation/provenance-terms_20261020/receipt.json),
[file ledger](../validation/provenance-terms_20261020/file-ledger.json),
[primary-source register](../validation/provenance-terms_20261020/sources.json)
and [dependency declarations](../validation/provenance-terms_20261020/dependency-licenses.json)
retain the exact scope. Sixteen Rust files (9,919 lines) were inventoried;
provenance comments, selected implementations, wire constants, fixtures and
manifests were inspected. This was not a line-by-line comparison of all code
against the Android source. The original APK/decompiled tree was not located
in the bounded locations recorded in the receipt. Frozen hashes identify the
inspected dirty checkout rather than claiming a released revision.

### What standards can establish

A standards-based implementation can establish useful compatibility from
public interface specifications and owner-configured bindings, reducing the
need to derive behavior from the Android app. It supports a narrower product
with fewer HyVibe-specific inputs. Conformance alone does not establish that
all contracts, patent claims, marks, assets or specification licenses permit
our distribution. Prior research history also remains part of provenance.

| Layer or workflow | Public standard / vendor-specific part | Current evidence and consequence |
|---|---|---|
| External audio | Ordinary host audio and the guitar's advertised external PC input/speaker feature. This pass does not certify an A2DP profile or codec. | Host-generated adaptive accompaniment can use ordinary audio without invoking the private RPC API. Audio input, routing, feedback and latency still require qualification. [Vendor feature overview](https://www.hyvibeguitar.com/) |
| MIDI pedal messages | MIDI 1.0 channels, Note/Control Change messages and BLE-MIDI transport; the action mapping belongs to the owner/device. | Vendor instructions explicitly describe external controllers, bindings, CC and Note On. Our October 17 standard-service diagnostic triggered Effect Next with an owner-configured channel-1 CC-67 mapping. CC-67 is not a universal Next Effect command. Other action/value/transport cases remain unqualified. [MIDI instructions](https://www.hyvibeguitar.com/midi-instructions-fr/), [BLE-MIDI specification entry](https://midi.org/midi-over-bluetooth-low-energy-ble-midi) |
| Beat file payload | Standard MIDI File format 0, PPQ, meter and note events. | Our original files play and survive reboot. The tested five-bar 4/4 file has an authored first-bar lead-in and bars 2–5 repeating after each start. `/Drums`, naming/discovery, accepted notes and that intro/body rule are HyVibe-specific compatibility facts, not SMF rules or complete General MIDI conformance. [SMF specification entry](https://midi.org/standard-midi-files-specification), [AP3 device evidence](../../../validation/audio-packs-ap3_20261020/README.md#distinguishable-bar-loopcount-in-trial) |
| USB delivery | Observed USB mass storage/FAT32; the OS handles the mounted filesystem. | It is an offline file path on this instrument, not evidence of USB MIDI, USB audio or a live control API. Preserve original files and retain separate approval for persistent writes. |
| Bluetooth app link | BLE/GATT is standard; the `eb65...4160/4161/4162` service/characteristics are a vendor application profile. | `ringdown-ble` connects OS/btleplug I/O to these identifiers. Standard GATT operations do not define the payload vocabulary. |
| App RPC envelope | Public JSON-RPC 2.0 specifies request/response shapes. | HyVibe methods/parameters and LLT/LLT2/compression are vendor-specific. Current `rpc.rs` sends numeric `jsonrpc: 2.0`; the standard requires string `"2.0"`. Historical hardware notes report both accepted, but our current output is not strict JSON-RPC conformance. A conforming envelope would still leave private command semantics. [JSON-RPC specification](https://www.jsonrpc.org/specification) |
| File transfer / recordings | CRC-32/MPEG-2 and RIFF/WAVE are public algorithm/container conventions. | `crc32.rs` generates its table from the polynomial. RPC download commands, framing and the `HyVibe loop file` JUNK metadata remain vendor-specific. A 92-byte captured WAV-header fixture contains metadata, not recorded audio. |

**Recommended release direction:** retain generic audio and SMF authoring;
prioritize a configurable standards-based MIDI controller path and explicit
file compatibility profile. Keep private RPC capabilities separately scoped.
Detailed effect/preset editing cannot be claimed from the standards path.
The owner's pedal menu did not expose standalone Beats start/stop; do not
invent such a mapping or infer MIDI Clock reception.

At the audit baseline, these were proposed boundaries: the probe and
`woodshed-instrument` still unconditionally included the private adapter.
The [subsequent implementation above](#standards-based-midi-and-beats-build-isolation-2026-10-20)
replaces that baseline with independent binary dependency/source selection.
The earlier frozen audit receipt remains historical. Whole-repository/source
archives and broader publication questions retain their separate scope.

### Provenance and terms findings

- **PT1 — Earlier legal conclusions need qualification.** The founding plan's
  L1–L4 report a label-only terms resource, missing bundled URL and zero
  anti-RE phrases in an inspected v1.1.2 Android snapshot. They do not establish
  that no store, runtime, account or version-specific agreement applied.
  Those searches were not reproduced here. Hardware acceptance establishes
  interface behavior and does not authenticate the APK or settle legal access.
- **PT2 — Original artifact provenance is incomplete.** The founding report
  identifies `audio.hyvibe.app` 1.1.2 / versionCode 4022, JADX 1.5.6, an abbreviated
  digest and an APKPure source, with a signing-identity caveat. Recover an
  authoritative source, full digest and trusted signing comparison, plus the
  applicable presented terms, before treating this research provenance as
  independently verified. No replacement APK was downloaded in this audit.
- **PT3 — Public vendor pages support some workflows, without a developer
  grant.** Current [MIDI instructions](https://www.hyvibeguitar.com/midi-instructions-fr/)
  describe third-party controller operation. The [Terms of Sale](https://www.hyvibeguitar.com/terms-of-sale/)
  govern website purchases and reserve rights in website elements; they are
  not affirmative permission for private RPC software or factory content.
  The [privacy policy](https://www.hyvibeguitar.com/privacy-policy/) covers the
  website/app service and refers to Terms and Conditions. The candidate
  `/terms-and-conditions/` URL was not accessible in this pass; that does not
  prove no agreement exists elsewhere. No public third-party-client grant was
  located in these inspected pages. App Store standard terms cannot simply be
  substituted for the Android build's applicable terms.
- **PT4 — Implementation and functional facts need separate accounting.**
  `compress.rs` attributes the codec and 163-entry wire dictionary to Android
  `JsonCompressor`/`NibbleList`; `llt2.rs` cites fallback instruction inspection.
  These deserve explicit necessity/expression review, including the exact
  dictionary order. Original Rust structure and interoperability purpose do
  not by themselves certify non-infringement. The work is not clean-room.
  `crc32.rs`'s generated table is a useful example of grounding an implementation
  in algorithmic requirements rather than transcribing a vendor table.
- **PT5 — A separate external-source item exists.** `apps/probe/src/pklg.rs`
  cites Wireshark's PacketLogger reader for format facts. The inspected
  [upstream file](https://gitlab.com/wireshark/wireshark/-/raw/master/wiretap/packetlogger.c)
  declares GPL-2.0-or-later. Selected Rust/C reading logic differs in structure,
  but this is not a complete expression comparison or a pinned historical
  source reconstruction. Merely learning format facts is not a finding that
  GPL code was copied. Resolve the derivation and any notice/license obligations
  before shipping that diagnostic; the Cargo dependency list cannot answer it.
- **PT6 — Dependency declarations are encouraging and bounded.** Locked,
  offline Cargo metadata resolved 124 packages, all with license declarations;
  none declare GPL. This is not a license-text/notice audit or a verdict on
  embedded source, platform SDKs, native/system libraries, release features,
  assets or unrelated patents. The new original kit and MIDI trials have
  explicit local provenance; do not substitute factory samples/beats/catalogs.
  Captured status fixtures retain an instrument identifier and should receive
  a privacy/minimum-necessary-data review before republication.
- **PT7 — A public specification has its own conditions.** JSON-RPC's document
  permits implementation and specifies notice requirements for copying or
  derivative explanation. MIDI's [IP policy](https://midi.org/mma-ip-policy)
  has conditional royalty-free essential-claim commitments; this is not a
  universal patent clearance. The [MIDI 2.0 logo program](https://midi.org/midi-2-0-logo-licensing-program)
  distinguishes product implementation from logo licensing; it does not
  establish every MIDI 1.0/BLE-MIDI access/mark rule. Do not redistribute gated
  specification PDFs. Bluetooth's [qualification overview](https://www.bluetooth.com/develop-with-bluetooth/qualify/)
  and [QPRD v5](https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/QPRD/out/en/index-en.html)
  require a product-specific determination for our OS-API-only software and
  any implemented profile layer. Neither an exemption nor a hardware-product
  fee obligation was established here. Make no certification/logo claims.
- **PT8 — Legal references are conditional, not clearance.** U.S.
  [17 USC 102(b)](https://www.govinfo.gov/content/pkg/USCODE-2024-title17/html/USCODE-2024-title17-chap1-sec102.htm)
  separates ideas/procedures/systems from copyright protection. That does not
  dispose of copied expression or separate rights. [Section 1201(f)](https://www.govinfo.gov/content/pkg/USCODE-2024-title17/html/USCODE-2024-title17-chap12-sec1201.htm)
  imposes conditions on its interoperability exception; the earlier statement
  that it exempts this work regardless is withdrawn. No finding establishes
  circumvention, an applicable exception, or enforceability of any contract.
  Publisher/distribution jurisdictions, actual terms and any patent analysis
  remain matters for a targeted legal review, whether distribution is paid or
  free. These references use official 2024 statutory editions and are not an
  all-jurisdiction/current-case-law opinion.

### Bounded follow-up and release evidence

The audit is complete as an assessment. Its open questions are concrete:

| Item | Next evidence / owner | Scope |
|---|---|---|
| A1 | Maintainer + authorized app distribution/vendor: original APK provenance and version-specific/store/runtime terms | Reconstruct existing research access and terms; do not assume missing strings mean no contract. |
| A2 | Maintainer + software/IP reviewer: codec/dictionary derivation record and any necessary rights | Review actual implementation expression and minimum interface facts. |
| A3 | Maintainer: PacketLogger reference/derivation, historical source and notice decision | Diagnostic distribution; avoid assuming a metadata-only license scan clears this source relationship. |
| A4 | Maintainer + relevant standards body/platform guidance: specification/IP/mark terms and application-only Bluetooth scope | Determine obligations for the actual shipped profiles/OS APIs; no blanket free/exempt or fee claim. |
| A5 | Independent build split implemented above; retained selected graphs and host compiler/build evidence | Binary isolation qualified on the host; Windows/Linux compilation and source-distribution packaging remain separate. |
| A6 | Maintainer: fixture privacy and asset redistribution records | Keep original/synthetic content and necessary interface vectors; review identifiers and factory-derived settings. |
| A7 | Maintainer + HyVibe/legal contact: documented interfaces and any appropriate written grant | Cover public protocol documentation, independent implementation, downstream forks and commercial redistribution, plus relevant HyVibe-held rights. |

Continue generic pack/audio work under its own license obligations. Treat a
supported public RPC adapter release as needing resolution of the applicable
items above or a documented legal decision. This is an engineering release
recommendation, not a conclusion that the existing adapter infringes. Separate
packaging does not erase earlier provenance, and HyVibe can clarify/grant only
rights it controls. Keep firmware replacement under its existing separate plan.

### Outreach draft (not sent)

**Status: local draft for Mark's review.** No email, contact-form submission or
external draft has been created. Confirm the intended recipient and review the
provenance wording before sending; retain the requirement that any eventual
agreement fit public open-source documentation and downstream distribution.

**Subject:** Open-source desktop companion: MIDI control and custom Beats workflows

Hello HyVibe team,

I'm Mark, a HyVibe guitar owner and the maintainer of Woodshed, an open-source
practice application, and its Ringdown instrument-control component. We're
working toward Windows, Linux and macOS workflows for practice and live use.

I'd like to coordinate with you on interoperability and understand which
interfaces and terms you recommend for independent developers. We've tested
a software BLE-MIDI controller with an owner-configured binding, and original
MIDI rhythms loaded through USB play in Beats and survive a restart. A small
marked-bar trial also helped us identify the lead-in and repeat behavior.

The use cases are desktop accessibility, performance scripting, set-list
control and creating original practice rhythms. We intend to distribute our
own implementation and original content, with clear independent-project
branding and support boundaries. The planned integration uses the instrument's
existing interfaces; firmware replacement is outside this request.

Some earlier Ringdown protocol research included static inspection of an
Android build and observations of an owned guitar. We'd welcome official,
publicly usable interface documentation and clarity on the applicable terms
as we prepare a supported integration.

Could you point us to the right technical or licensing contact and help clarify:

1. Which MIDI, Beats-file and app-control interfaces are supported for
   independent software, and whether public documentation is available?
2. Which app/device/developer terms apply, especially to the HyVibe-specific
   app-control protocol and publication of an independent implementation and
   factual protocol documentation?
3. Whether you can provide written clarification or any needed permission
   compatible with MPL-2.0 distribution, including downstream forks and
   commercial redistribution, and guidance on compatibility wording?

We'd be happy to share a short demonstration and the interface questions,
and to discuss how to make the integration useful and maintainable for guitar
owners. Thank you for considering it.

Best,
Mark

## Desktop performance assessment (2026-10-17)

### Direction and assessment scope

Mark requested this assessment and plan update on 2026-10-17 after identifying
live control, performance scripting, and the guitar's new Beats feature as
product opportunities. This authorizes the planning direction; the adapters,
command names and cue format below are implementation candidates.

This pass inspected Woodshed `c6036f21afad72dd6d4850b8dd23a9c090ff49e1`,
Ringdown source and earlier receipts, and the primary sources linked below.
Fetched `origin/main` at `125be57` before delivery: Ringdown and
`woodshed-instrument` are unchanged from that source baseline, and there is
still no instrument integration in the desktop host or views.
It performed no Bluetooth discovery, device writes, firmware installation or
new hardware qualification. The owner's current firmware versions, updated
app version and MIDI bindings have not been recorded.

### Cross-platform requirement and stack review (2026-10-18)

**Owner ruling:** Ringdown is cross-platform software. Windows, Linux and
macOS are product targets from the first performance implementation. A Mac
hardware experiment is evidence for one adapter; it does not select a Mac
application, Mac-specific service, or a later-porting strategy. Direct browser
control remains capability-dependent, with shared portable logic and the
existing native-companion candidate where the browser cannot supply the role.

The review read the founding layout/phase history and effects handoff, the
Woodshed smart-instrument and audio-ports rehome plans, the shared desktop-host
migration, current crate seams, and the current source-audit reconciliation.
The relevant existing decisions are:

- **Reuse the protocol and driver split.** `ringdown` is `no_std`/sans-io;
  `ringdown-client::Guitar<Link>` owns the RPC exchange. `Link` deliberately
  avoids mandatory `Send` futures so browser adapters remain possible. Its
  discovery/connection lifecycle is platform-owned. The driver still uses
  Tokio deadlines; browser/runtime portability needs a timing seam before
  claiming that path works. This is not a reason to duplicate the driver.
- **Add the pedal role beside app RPC.** The existing `Link` carries an
  already-open app GATT connection. Advertising a local MIDI service and
  managing its subscribers needs its own adapter contract. Put MIDI messages,
  framing, bindings, action/cue state and outcomes in shared Rust logic, and
  keep OS objects/callbacks in thin transport shells. The Swift diagnostic is
  a retained experiment, not the product implementation.
- **Compose the existing UI stack.** Woodshed's host migration extracted the
  window/layout/input/accessibility assembly into `cambium-genet-winit-host`,
  now supplied by Mere. Ringdown views use Cambium and the existing native
  host; a browser host can consume portable views subject to its capabilities.
  Do not introduce another toolkit or copy the native event loop. Sources:
  Woodshed [host migration](../../../design_docs/2026-08-09_cambium_desktop_host_migration.md);
  `mere/design_docs/cambium_docs/technical_architecture/2026-09-03_cambium_architecture.md`.
- **Preserve product authority.** Woodshed already has a long-lived instrument
  `Session`, explicit release and Detached/Follow/Drive metronome policy.
  Ringdown owns reusable performance actions and connection mechanics;
  Woodshed retains practice/Set and tempo-authority policy. The rehome keeps
  independent nested workspaces and releases, not a port-to-port runtime
  dependency or a second practice/recorder state owner.
- **Separate connection lifetime from background residency.** One maintained
  session is needed for UI/CLI dispatch. That does not establish a new daemon.
  Use existing host wake/lifetime contracts. If operation with all application
  hosts closed becomes a requirement, assess composition with the family's
  designated Djinn resident before implementation. Djinn owns background
  service lifetime while domain crates retain semantics; no Ringdown service
  or completed generic scheduler is inferred from that design. Sources:
  `mere/design_docs/mere_docs/implementation_strategy/2026-08-22_djinn_family_resident_services_plan.md`, §1;
  `mere/design_docs/2026-08-12_family_composition_thesis_brief.md`, §7.

The [Ringdown audit](../../../validation/full-source-audit_20261016/ringdown-report.md)
and [current finding ledger](../../../validation/full-source-audit_20261016/current-finding-ledger.json)
remain the remediation authority. Current source inspection reconfirmed the
mechanisms for absent status fields becoming defaults (R3), restoration losing
negative per-write receipts (R1), weak ACK-envelope admission (R5), and malformed
Unicode hex slicing (R2). No tests or fresh hardware calls were run for this
review. Scope fixes to operations entering the performance surface: preserve
status presence and qualify RPC admission before presenting live RPC status;
retain individual refusal receipts before exposing restoration as preparation.
Pedal work need not absorb unrelated file-transfer or capture-tool cleanup.

### Evidence and what it establishes

| Evidence | Confidence and implication |
|---|---|
| [Typed planners](../crates/ringdown/src/plan.rs), [shadow profile](../crates/ringdown/src/profile.rs), [driver](../crates/ringdown-client/src/lib.rs) | **Static-read of current source; earlier fixture and hardware receipts remain scoped to their runs.** Reusable configuration machinery exists. `Sent` distinguishes a parsed reply from an observed musical result. |
| [BLE shell](../crates/ringdown-ble/src/lib.rs), [workspace](../Cargo.toml), [probe](../apps/probe/src/main.rs) | **Static-read.** The desktop connects as a central to the guitar's app GATT service. The only Ringdown app is the research probe; there is no performance UI, resident control service or BLE MIDI pedal adapter. |
| Founding Findings H24/H25/H27/H32/H43/H47 | **Historical hardware-verified.** Preset/effect editing, metronome control and playable bank creation have evidence. Read-back is limited, the phone app overwrites profiles on connect, and a parsed reply alone cannot establish effect state. Recheck applicability on current firmware. |
| [HyVibe MIDI instructions](https://www.hyvibeguitar.com/airturn-midi-instructions/) | **Static-read of vendor documentation.** The guitar scans for a BLE MIDI pedal; bindings include loop record, play/stop and effect actions. Channel 1 and configurable CC/Note On bindings are documented. The four-switch AirTurn numbers are 64, 66, 65, 67; these identify switches, not universal guitar commands. Message values, release behavior and the documented hold-to-erase gesture still need device evidence. |
| Owner report and clarification, 2026-10-17; [HyVibe feature overview](https://www.hyvibeguitar.com/) | **Owner-reported for Beats; static-read for vendor drum-machine claim.** The owner describes built-in rhythm patterns as alternatives to the metronome click, with no custom beat authoring. The product label does not establish a programmable drum machine or a remote-control protocol. |
| [Existing RPC vocabulary](../crates/ringdown/src/rpc.rs), `METRONOME` and `Method`; [compression dictionary](../crates/ringdown/src/compress.rs) | **Static-read.** Known metronome parameters cover `bpm`, `num`, `den`, `bars`; no explicit Beats-pattern or MIDI-pedal implementation was found. `StartRecording`/`StopRecording` are not evidence of looper pedal equivalence. A new firmware capability may use extra fields, new methods, MIDI, or panel-only control. |

The [vendor downloads page](https://www.hyvibeguitar.com/downloads/) lists
separate H1, H2 and Essential firmware families. Its version labels do not
establish the owner's installed versions, a Beats introduction version, or
support across every model. Capture the actual instrument and both reported
processor versions before selecting a capability profile.

### Two output paths, one set of performance actions

1. **App RPC:** use the existing `Guitar<Link>` for tone preparation, absolute
   preset selection, parameter writes and metronome operations. Assess Beats
   extensions here. Preserve the JSON field order and compression discipline
   already established by device work.
2. **Pedal MIDI:** send the messages bound in the instrument's MIDI menu.
   The guitar scanning for the pedal implies the desktop must expose the
   peripheral side of BLE MIDI, or use an OS MIDI endpoint that supplies it.
   The Q-PC experiment below confirms this role and one owner-observed
   Effect Next change through a direct peripheral. It is a separate adapter
   from the app RPC `Link`; broader delivery semantics remain unqualified.

The current [btleplug upstream](https://github.com/deviceplug/btleplug)
explicitly supports central operation only. Reusing the current BLE shell
does not implement pedal emulation. Prefer an existing OS MIDI route before
owning BLE MIDI framing. A [virtual MIDI port](https://developer.apple.com/library/archive/qa/qa1831/_index.html)
alone is not proof that the guitar can discover the computer.

| Platform | First candidate and qualification boundary |
|---|---|
| macOS, first device experiment | [Audio MIDI Setup can advertise the Mac as a Bluetooth MIDI peripheral](https://support.apple.com/guide/audio-midi-setup/set-up-bluetooth-midi-devices-ams33f013765/mac). On Q-PC, pairing succeeded but CoreMIDI supplied no destination. A direct [CoreBluetooth peripheral](https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager) obtained a subscription with explicit unencrypted diagnostic access, and the owner confirmed Effect Next changed after one CC event pair. The strict encrypted-access attempt did not subscribe. See the P0 receipt; firmware baseline, value/release semantics, latency and automatic reconnect remain open. |
| Windows | Inspect the installed OS MIDI transport first. [GATT service publication](https://learn.microsoft.com/en-us/windows/apps/develop/devices-sensors/gatt-server) and [adapter peripheral-role support](https://learn.microsoft.com/en-us/uwp/api/windows.devices.bluetooth.bluetoothadapter.isperipheralrolesupported) provide a possible custom route. Neither documents Ringdown compatibility; qualify the actual OS, adapter, MIDI routing and guitar. |
| Linux | [BlueZ GATT registration](https://github.com/bluez/bluez/blob/master/doc/org.bluez.GattManager.rst) and [LE advertising](https://github.com/bluez/bluez/blob/master/doc/org.bluez.LEAdvertisingManager.rst) provide a possible route. Assess the existing MIDI stack, controller capabilities and permissions before choosing an adapter. No Linux guitar receipt exists for this target. |
| Web | [Web Bluetooth provides the central role](https://developer.chrome.com/docs/capabilities/bluetooth). A supported browser may implement the existing app `Link`; that does not let it advertise as a pedal. Editing/mappings can work offline; a paired native companion can own control where direct access is unavailable. Direct RPC and companion control require separate qualification. |

Use the [BLE MIDI specification](https://midi.org/midi-over-bluetooth-low-energy-ble-midi)
if a custom transport is necessary. Do not substitute unframed MIDI bytes for
BLE MIDI packets or infer guitar clock-following from MIDI connectivity.

### Product and ownership shape

Keep pure action, cue, binding and capability state beneath the native
adapters; retain the existing sans-io core. The driver continues to own RPC.
A native session coordinates MIDI and RPC with one ordered dispatch path;
screen buttons, keyboard, controller input, CLI and Woodshed use that same
path. Ringdown's application host owns connections and OS MIDI resources;
the shared Cambium host supplies UI lifecycle/input/wake hooks, not an async
runtime or instrument service. Exact crate boundaries and transport adapters
are settled against the cross-platform requirements before implementation.

Build one Ringdown desktop surface for Windows, Linux and macOS with
connection status, the loaded song/cue, large action buttons and explicit input mappings. Save preparations
and cue files locally. One maintained session owns each guitar: a CLI invocation
must reach that session, rather than reconnecting for every cue or competing
with the UI for the radio. Start scripting with typed commands and structured
results usable by an ordinary shell or DAW; a new embedded scripting language
is unnecessary for the first implementation.

Keep Ringdown independently runnable and released under `ports/ringdown`.
Woodshed owns its Set, practice transport and recorder data; its instrument
adapter consumes Ringdown actions. A guitar loop, a Woodshed take and Hocket
transport are distinct resources. Mounting a UI or sharing a tempo does not
merge their state or demonstrate synchronized clocks.

### Performance semantics

- Bind actions, not guessed button numbers. Distinguish an absolute preset
  selection from next/previous, an explicit start from a toggle, and button
  press/release from hold. Record the actual guitar binding and MIDI values.
  Suppress keyboard autorepeat and duplicate input dispatch.
- A capability record names the instrument, processor firmware, app/capture
  version, connection path and evidence for each action. Keep untested,
  observed-working and observed-unsupported distinct. Separate tempo and
  start/stop from Beats-pattern selection; connectivity is not capability.
- Preserve the difference between queued, sent, parsed, refused, outcome
  unknown and observed state. MIDI delivery has no automatic musical
  acknowledgement. After a timeout, do not retry a toggle, record or erase
  action blindly, or resend it through the other path.
- Preload tone banks and other slow preparation before performance. A cue is
  ordered but not atomic across MIDI and RPC. Stop on a failed/unknown step,
  report completed steps and require explicit recovery. Coalesce continuous
  parameter updates; keep bulk retrieval/profile restoration out of the live
  command queue.
- Prefer the guitar's local Beats clock. Tempo matching does not establish
  phase alignment. Measure input-to-observed-action latency and jitter,
  distinguish host send times from audio onset, and record trial counts,
  median, p95, worst case and misses. Set an acceptable budget for each
  action before qualifying it for live use; do not promise sample accuracy,
  MIDI clock following, fills or bar-boundary changes without evidence.
- Disconnect cancels pending live triggers and disarms automation. Reconnect
  requires explicit resume; it never replays stale presses or an erase hold.
  “Cancel cues” stops local dispatch, not necessarily a running guitar loop
  or beat. Expose a guitar stop only where that action is proven. Preparation
  and live operation are explicit so a performance trigger cannot silently
  rebuild a profile or destroy an existing recording.

### Phases for the next implementation

These extend A–D below. P0 supplies the hardware facts; P1 starts a usable CLI
and P2 the desktop surface. P3 depends only on the Beats capabilities P0
establishes. If Beats control is unavailable, P1/P2/P4 can progress with the
supported actions and a visible limitation.

#### P0 — Establish pedal and Beats capabilities on the reference guitar

**Feature target:** an evidence-backed action/path table for current firmware.

**First software-pedal trigger succeeded (2026-10-17):**
[Q-PC experiment record](../validation/midi-pedal_20261017/README.md) and
[structured observation](../validation/midi-pedal_20261017/effect-next-observation.json).
The direct `Ringdown` CoreBluetooth peripheral obtained a guitar subscription
and sent channel-1 CC 67, value 127 then 0 after a scheduled 50 ms. The owner
had assigned that binding to Effect Next, saw the MIDI indicator, and confirmed
that the effect changed. Exact effect names and advance count were not supplied.

CoreMIDI's advertised route exposed no destination. The strict direct service
requiring encryption connected without a MIDI subscription; an ATT request
failed before disconnection. The explicit `--unencrypted-diagnostic` variation
then subscribed and carried the successful event pair. This supports a session
compatibility constraint, not a decoded error or a shipping security decision.

This is owner-confirmed control plus transport logs, not a current-firmware
protocol Finding with the required `GetStatus` positive control. The app-service
scan found no guitar; versions, app RPC, Beats, Note/Program Change, isolated
press/release semantics, latency and automatic reconnect remain unqualified.
P0 is partially complete.

**MIDI menu inspection (2026-10-18, owner-reported):** the complete list is
[recorded here](../validation/midi-pedal_20261017/midi-action-menu.json): loop
record, loop record met, loop play stop, looper set volume, effect next, effect
previous, effect switch, effect control, effect set volume, effect on/off,
loop record met bars. There is no standalone Beats/metronome action in that
menu. The `loop record met` labels do not establish independent Beats control
and were not triggered. Scope initial MIDI performance work to effect/loop
controls with per-action qualification; investigate Beats through observed
panel/app behavior and recovered RPC. Volume/parameter actions are candidates
for continuous input, not yet proven message semantics.

**USB inspection (2026-10-18, file-observed; no RPC positive control):**
[USB receipt](../validation/usb_20261018/README.md). USB-C–to–USB-C exposed a
FAT32 `HYVIBE` mass-storage volume; no new USB MIDI/serial endpoint appeared.
The files include two readable configuration JSONs, nine user banks, 44 loop
WAVs and ten named drum MIDI files. Their headers declare format 0, one track,
division 480. Stored metronome fields are `drums_loop`, `dl_filename`, `bpm`,
`num`, `den`, `nbbars`; the snapshot selected `6-Blues.mid`, 100 BPM, 4/4,
with `nbbars: 12`. Schema versions 3/2 are not firmware versions.

Private configuration snapshots permit offline work and preservation without
calling `ReadConfig`. They are a captured file source, not live readback or
an automatically valid shadow profile. USB loading of added/edited patterns,
pattern event semantics and the RPC path for the stored Beats fields remain
untested. The disk was ejected after inspection; no guitar files were edited.

Done-conditions:

- Record Windows/Linux/macOS adapter requirements and capability gaps for both
  roles: app RPC central and pedal peripheral. Assess advertising, subscription,
  access requirements, release and reconnect before fixing a common contract.
  Q-PC supplies one adapter result; the other desktop targets remain open.
- Record guitar model/id, both processor versions, OS/adapter, app version,
  existing MIDI bindings and an initial `GetStatus` positive control. Establish
  which pre-update Ringdown reads/writes still work. Preserve the current
  tones and recordings; factory reset or opening the app is not a neutral
  restoration step, because the app may overwrite the profile.
- Advertise the Mac through Audio MIDI Setup and attempt the guitar's MIDI
  pairing. Identify the usable MIDI endpoint or direct peripheral subscription.
  The Q-PC direct route has the first Effect Next observation; qualify the
  remaining paths and behaviors. Prove a nondestructive action
  first, then record the exact press/release/hold sequences and observed
  behavior for the desired pedal actions. Test erasure only on an explicitly
  disposable loop with the owner. If a physical pedal is available, capture
  it; ownership of one is not assumed.
- Check MIDI alone, RPC alone, and coexistence. Determine whether the guitar
  accepts both, whether a physical pedal can remain connected, whether either
  steals the other's connection, and what disconnect/reconnect does. A shared
  controller is exposed only for the combinations actually demonstrated.
- For Beats, first observe the current panel and app. Preserve raw replies
  from known reads, then capture one official-app operation at a time where
  the app exposes the feature. Identify pattern names/ids, tempo, level,
  start/stop, persistence and any MIDI bindings. Do not guess new RPC names or
  reuse undocumented `SetConfig` fields to discover them. If the app has no
  control, record that boundary and inspect the panel's MIDI assignments.
- Replay only recovered operations with owner observation and before/after
  positive controls. Classify every desired action as working, unsupported
  or still untested for the exact firmware/path. Assess pattern switching
  while running, loop interaction and phase behavior without assuming them.
  Establish timing budgets from observed trials and the intended musical use.

The smallest joint proof is two prepared tones selected from a desktop input,
a working pedal-style loop action, and Beats start/stop or pattern selection
where reachable. Record partial success honestly. Stop a device experiment
on lost positive control or ambiguous state; recover with the owner before
another write. This document update does not itself execute that session.

#### P1 — Shared performance actions, maintained session and CLI

**Feature target:** the proven P0 actions are callable without the research probe.

Done-conditions:

- Shared Rust action, binding, cue, capability and session logic dispatches
  through distinct RPC and pedal-output seams. Keep channel/value/framing
  rules out of UI, CLI and OS callbacks. Preserve the existing `Guitar<Link>`
  driver and its `ReadConfig` guard; raw research calls remain in the probe.
- Define the pedal adapter against Windows, Linux and macOS requirements before
  fixing its interface. Implement the desktop shells as part of this phase;
  the Q-PC result informs the macOS adapter only. Report hardware/OS capability
  limits explicitly. A successful Mac build or stubbed other-platform adapter
  does not close cross-platform control. The Swift helper stays experimental.
- CLI commands dispatch through the one session holding the connection instead
  of advertising/reconnecting per trigger. Define app/headless host lifetime,
  wake, release and cancellation using existing stack contracts; persistent
  background service ownership requires the Djinn assessment above. Qualify
  access-requirement compatibility before choosing a shipping policy.
- Before exposing an RPC operation, address the relevant audit finding at its
  existing owner and retain raw/per-action outcomes. Scope status, ACK admission
  and preparation receipts to the operations actually included.
- A CLI can identify/select a guitar, report capabilities, recall a prepared
  tone and trigger a proven action through the maintained session. Define
  machine-readable outcomes, exit status, cancellation and connection ownership.
- Explicit USB snapshot intake validates file schema and bank/effect mapping
  into the existing model before using it as preparation. Keep stored snapshots,
  intended edits and live observations distinct. Use portable filesystem I/O;
  do not infer USB MIDI/RPC support from the storage volume.
- A dry run validates actions/cues and prints their planned order without
  sending device commands. Record dispatch logs with cue/action identity and
  timings. Fixture checks cover duplicate input, press/release handling,
  refusal, timeout uncertainty and disconnection with pending triggers.
- Build and exercise the same CLI/session contract on Windows, Linux and
  macOS, and record physical guitar-control receipts for each advertised
  transport. Shared-core fixtures establish semantics; they do not replace
  adapter/device qualification. Record unavailable hardware as an open gate.

#### P2 — Desktop stomp box and external controller mappings

**Feature target:** play a small performance without using the phone app.

Done-conditions:

- Independently launchable Ringdown UI for Windows, Linux and macOS on the
  existing Cambium shared native host, with common product views and large
  keyboard-accessible controls, connection/capability status, loaded cue and
  editable bindings for keyboard and an OS MIDI input. Use the P1 dispatcher.
- Persist mappings and prepared tones. Make observed state, intended state
  and unknown state understandable; expose explicit release of the guitar.
- Qualify focus loss, key repeat, duplicate bindings, held input, link loss,
  cancel and explicit resume. The owner demonstrates tone changes and a
  supported loop action with repeated triggers inside the agreed budgets.

#### P3 — Beats and song cues

**Feature target:** a song preparation can recall supported built-in guitar
timekeeping patterns and settings. The owner's clarification describes Beats
as preset alternatives to a metronome click; user-authored patterns are a
separate possible extension.

Done-conditions:

- Model discovered built-in pattern identity and supported controls. Use the
  USB-observed pattern names and stored Beats keys as research inputs, not
  assumed RPC arguments. Assess selection, tempo and start/stop without
  assuming a sequencer, fills or a pattern editor in the guitar. Preserve manual-only/unsupported Beats
  capabilities visibly on older or limited devices.
- A versioned local cue references prepared tone identity, tempo and optional
  Beats settings. Resolve the tone to a slot for the prepared profile; a
  renumbered bank must not silently become a different song's tone.
- Validate the whole cue before dispatch and report partial application.
  Manual Next works independently of timed execution. Device evidence covers
  the chosen pattern/tempo/start behavior and interaction with a guitar loop;
  no bar-quantized claim without a usable phase source and device proof.

**Shared custom beat foundation (implemented at file/API level 2026-10-19).** Consume the
[shared data-defined audio-pack work](../../../design_docs/2026-09-09_audio_ports_rehome_plan.md#shared-data-defined-audio-packs-2026-10-19):
bundled and installed TOML/JSON definitions plus assets, reuse of the existing
Woodshed sequencer, and an AP0 Rust embedding assessment before new authoring
or DSP implementation. Ringdown owns instrument-specific MIDI validation and
delivery; portable pattern/export code belongs to the shared root layer.
The protocol core remains sans-io. Optional local audition uses a prepared
instrument through the existing Firewheel foundation and shared controls;
MIDI export itself needs neither a graph nor an audio device.

The USB inspection gives a concrete device candidate: compatible MIDI files
for the guitar's existing drum player. Establish file discovery/naming, note
mapping, timing, loop boundaries and actual acceptance. A pack's sample assets
or synthesis definition do not transfer into the guitar through a MIDI file.
Preserve originals and make the authored artifact reviewable before requesting
an explicitly approved persistent-device trial. On 2026-10-20, AP3 performed
that checkpoint: the owner approved adding `11-Ringdown_AP3.mid`; exact
readback and unchanged hashes for all ten factory files were verified before
ejection. The owner confirmed it appears and plays repeatedly, then confirmed
it still appears and plays after a power cycle with USB OFF. This is bounded
custom-file discovery, playback and reboot-persistence evidence, recorded in the
[AP3 receipt](../../../validation/audio-packs-ap3_20261020/README.md).
The five identical bars do not establish exact loop/count-in behavior, and
this trial does not establish Beats RPC or MIDI-pedal start/stop.
The subsequent five-bar diagnostic with distinct 1/2/3/4/5-hit signatures is
now owner-confirmed: first bar A once, B–E repeating; stop/wait/start brings A
back once. Playback begins directly with A, and no separate count-in control
was found. This establishes an authored one-bar lead-in followed by a four-bar
repeat body for this 4/4 file. Future guitar-specific export should explicitly
supply that intro/body structure, without changing generic shared MIDI export.
Other lengths/meters, firmware and phase accuracy remain unqualified. The
approved copy/readback, unchanged prior hashes, safe ejection and unidentified
extra directory entry remain recorded in the same AP3 receipt.

Shared authoring/export can progress without the guitar.
The shared `audio-pattern`/`audio-packs` libraries and `audio-pack` file command
now export a data-defined drum kit and retain its source/assets without a
device. Ringdown's own application integration remains P3 work;
guitar playback and RPC control each require their own evidence. The vendor's
UI remains owner-described as built-in timekeeping patterns without a custom
editor. P1/P2/P4 performance control remains independently useful.

#### P4 — Performance sequences and host integration

**Feature target:** the same actions can be driven by a set list, script or host.

Done-conditions:

- CLI and desktop load the same cue data; shell/DAW input and Woodshed call
  the same session actions. A running desktop and a script share connection
  ownership and ordered dispatch. No second protocol or recorder authority.
- Support cancellable manual sequences first. Add relative/absolute timing
  only with an explicit clock source, deadline policy and late-event result;
  local timing must not masquerade as the guitar's bar position.
- Qualify a short multi-cue performance with the owner, including a deliberate
  disconnect: no stale record, toggle or erase is replayed on resume. Capture
  the exact source, platform, firmware, bindings and timing results. Required
  Windows/Linux/macOS qualification starts in P1/P2; it is not deferred to P4.
  Browser/companion control needs its own receipt.

## Original configuration foundation (A–D)

The sections below retain the 2026-09-02 design and session record. Their
historical “today,” catalog and firmware observations refer to that work;
the assessment above controls the next product target.

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

The **then-observed effect catalog contained thirteen kinds** (H28, read off
that app with the scrollbar at both ends). That is the implemented vocabulary,
not a claim that later vendor firmware cannot add effects. Implementing new
DSP effects in firmware remains outside this client plan.

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

- **2026-10-17 — Beats scope clarified by the owner (owner-reported).** Beats
  provides built-in timekeeping patterns replacing the usual metronome click;
  it is not described as a custom beat editor. Custom authoring is a possible
  software extension, with its rendering/output path still to be assessed.
- **2026-10-17 — Desktop performance assessment.** The source/documentation
  evidence table above is the current assessment record. Pedal MIDI is a
  distinct path from app RPC; macOS supplies a candidate advertised MIDI
  peripheral route. Beats is owner-reported and publicly supported at the
  drum-machine product level, with remote operations still unknown. The later
  P0 experiment below adds a transport trace and owner-confirmed Effect Next
  observation; current-firmware RPC qualification remains open.
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

- **2026-10-21 — Desktop diagnostic acceptance complete.** Thirty-nine focused
  cases and native controls/save-dialog/reopen checks pass. Stop retains history,
  restart appends, off-time writer delivery does not record, and exported JSON
  equals Status. Same-profile reopen starts off. Real dialog completion releases
  pending host-command closes in both Cancel and Save branches; native macOS
  Quit's modal refusal is documented separately. See the acceptance packet above.
  The existing journal/status/writer/dialog owners remain the integration points.


- **2026-10-21 — Desktop diagnostic toggle/history/export implemented.** Added
  session-only runtime opt-in and capture boundary markers to the existing
  journal, shared one Status projection between scripting/export, and composed
  existing settings buttons/readouts with a bounded recent-event view. New-file
  JSON export uses the established background dialog/wake/close flow. Mac build,
  portable core and Windows/Linux shared-instrument compile. That implementation
  pass ran no tests; the later desktop acceptance entry above owns runtime results.

- **2026-10-21 — Previous input comparison complete; historical miss not reproduced.**
  Added default-off bounded diagnostics through the shared instrument seam and
  existing desktop Status. Thirty-eight focused cases and shared Windows/Linux
  instrument checks pass. Native keyboard/Note comparison independently receives
  eight exact matching messages. Real P, sparse Note and held Note each return
  Rev to Off with identical CC66 press/release and 50.4–50.5 ms observed gaps.
  Twelve physical OS-accepted messages, zero disconnects, no duplicate gesture
  and complete 124-event trace; ports close. The earlier no-change observation
  remains unexplained, with no speculative protocol or retry fix.

- **2026-10-21 — Input desktop and bounded guitar qualification.** Native learning exposed
  render-delayed events; moved the sole live controller edge gate to the existing
  MIDI callback and dispatcher, leaving frame history for preparation only.
  Corrected explicit input None forwarding after endpoint loss, and retained
  callback refusals for the UI. The final three-process native cohort records
  35 exact messages, saved/Clear/reassign reopening and quiet held-input/reconnect
  behavior; 48 focused cases pass. Physical N/P and selected-input CC/Note checks
  finish at 12 OS-accepted messages with five owner-confirmed single changes;
  one earlier Note-triggered no-change remains unresolved. Reconnect disarms,
  held inputs stay quiet, fresh actions work, and both ports close. The packet
  preserves the negative observation separately from later successful actions.

- **2026-10-20 — Keyboard/controller input implementation.** Added saved
  per-action Learn/Clear mappings and live arming in the existing controls;
  shared-host keyboard admission and sequence-consumed existing MIDI input feed
  the same target-bound dispatcher. Sixteen focused cases pass (ten instrument,
  four retained desktop input/control, one settings round trip, one shared-host
  gate), including eleven new cases; Mac desktop build and Windows/Linux shared-instrument checks pass.
  Native controller/keyboard/guitar qualification is not repeated here. The
  shared host now rejects synthesized focus presses through one source-patched
  supplier, without another host loop; native focus interaction remains
  qualification, as recorded in the input receipt above.

- **2026-10-20 — Actual desktop MIDI and reconnect acceptance.** Thirty-six
  focused executions and three native macOS cohorts pass. Independent captures
  match 293 ordinary-path and 131 hotplug-path OS-accepted messages; fourteen
  presented captures retain picker/status/clock/close/rearm and guitar connection
  observations. Menu overlap, pending-open cancellation, stale subscriber
  generation, status polling and defunct CoreMIDI endpoint reporting were fixed
  in the existing owners. Guitar lifecycle checks send zero musical messages;
  native Windows/Linux and physical BLE fault/timing campaigns remain open.

- **2026-10-20 — Shared BLE-MIDI peripheral integrated.** Reused and repaired
  one MIT supplier for Mac/BlueZ/WinRT, including a narrow BlueR security-flag
  patch. Existing MidiOut, desktop picker/clock/status and CLI own the path.
  Mac CLI/desktop and Windows/Linux Rust target checks pass; the owner observes
  one Effect Next advance through the Rust Mac transport. Other native
  platforms, encrypted interoperability and performance timing remain open;
  see the scoped BLE-MIDI receipt above.

- **2026-10-19 — Shared audio-pack direction adopted.** Linked shared AP0–AP3
  in the root audio consolidation plan. Ringdown consumes sequencer-based
  authoring and MIDI export, with instrument-specific validation; suppliers,
  pack loading and local audition remain unimplemented and unqualified.
- **2026-10-19 — AP0 assessed.** Shared original-source probes establish MIDI
  export with midly and local audition routes through Firewheel, including
  FunDSP/RustySynth candidates. Ringdown keeps its sans-io boundary and consumes
  portable export after extraction. No guitar write or playback trial occurred.
- **2026-10-19 — AP1 shared foundation implemented.** Portable pattern/sound
  ownership, midly MIDI export and bounded kit/snapshot loading now exist in
  root libraries. Bundled/directory/TOML and source-absent reopen workflows
  run through the shared file command. Ringdown's app and device acceptance
  remain open; no transport, firmware inference or guitar write was added.

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

- **2026-10-17 — Assessment and plan update complete.** Adopted desktop
  performance control and scripting as the next product direction at Mark's
  request; assessed software pedal and Beats together, linked platform and
  protocol evidence, and defined P0–P4. Reconciled README/founding/index
  pointers and H47 bank-creation status. Implementation and the current
  guitar session remain pending.

- **2026-10-17 — Beats scope clarified.** Narrowed P3 to the guitar's built-in
  timekeeping patterns and recorded custom rhythm authoring as a separate
  possible extension. No new device capability is claimed.

- **2026-10-17 — P0 Q-PC session started.** Built a CoreMIDI inventory/pulse
  helper, a CoreBluetooth peripheral diagnostic, and the existing RPC probe.
  Pairing succeeded; daemon logs confirm H2-CC340 connected with the Mac in
  the peripheral role. No CoreMIDI destination or direct notification
  subscription appeared. An ATT request failed before an unencrypted link
  disconnected; the internal status has not been decoded. A five-second
  app-service scan found no guitar. Saved observations under
  `validation/midi-pedal_20261017`; the one guarded pulse was refused locally.
  The explicit unencrypted-access variation then obtained a MIDI subscription.
  One channel-1 CC-67 press/release pair was accepted by macOS and the owner
  confirmed the effect changed. Exact effect names/advance count were not
  supplied. This is a scoped owner observation plus transport evidence; the
  `GetStatus` firmware baseline and broader performance qualification remain
  open. This supplies one macOS adapter experiment for the cross-platform
  performance implementation.


- **2026-10-18 — Cross-platform ruling and stack review.** Corrected the
  Mac-specific P1 direction after the owner's ruling. Windows, Linux and macOS
  are first-implementation targets with shared Rust performance logic and
  thin transport shells. Reviewed existing protocol/driver seams, Woodshed's
  session policy, Cambium host extraction, independent port release boundaries,
  family resident ownership and the source audit. A maintained connection does
  not authorize a new background resident. P1/P2 now carry cross-platform
  qualification; applicable audit repairs stay with their current owners.
  This pass changed documentation only; no builds, tests or hardware calls.

- **2026-10-18 — USB storage and Beats files observed.** Read two configuration
  files and inspected ten drum MIDI headers after the owner connected USB-C.
  Preserved private config snapshots outside Git; public receipt contains facts
  and hashes. This adds an offline profile source and a concrete guitar-native
  custom-pattern candidate, not a live RPC or custom-loading proof. Recovered
  from media loss with owner USB OFF/ON after the file-access prompt; safely
  ejected at closeout. No guitar files were edited or production code changed.

## P0 transport trial close (2026-10-18)

The first Mac software-pedal trial ended after one owner-confirmed Effect Next
change and the full MIDI-menu readout. The helper exited normally and removed
its service. The bindings/pairing entered by the owner remain on the guitar.
No recording, erase, bulk profile operation or guessed Beats command was sent.
The [receipt](../validation/midi-pedal_20261017/README.md) preserves source,
logs and observation limits. This closes the bounded connectivity/first-trigger
experiment, not all P0 capability qualification or any production phase.

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

- **2026-10-20 — AP2 implemented; compilation/file evidence.** Existing probe CLI gains an
  offline shared-pack export path, including retained edited-selection reopening.
  The root plan owns shared controls, project custody and the Mere music binding;
  Ringdown's protocol core remains unchanged and device acceptance stays in P3.

- **2026-10-20 — AP3 approved custom MIDI trial.** An app-authored kick/snare/hat
  bar was prepared as a reviewable five-bar diagnostic file. After explicit
  additive-file approval, exact readback, unchanged factory hashes and safe
  ejection, the owner confirmed discovery and repeated physical playback,
  including discovery/playback after a power cycle with USB OFF.
  Precise loop semantics and remote Beats commands remain open; portable
  authoring/export and the protocol core keep their existing ownership.

- **2026-10-20 — Bounded provenance/terms audit complete; outreach drafted only.**
  Inventoried 16 Rust files, sampled provenance/implementation/fixture sites,
  resolved 124 declared package licenses and reviewed public standards/vendor
  sources. Separated standards-based MIDI/SMF/audio from private GATT/RPC/
  LLT2/codec; identified APK/terms, codec, PacketLogger reference, fixture,
  packaging and standards-license questions. Qualified the founding plan's
  historical legal conclusions and saved the outreach draft above. No message,
  guitar access, implementation change, test suite or publication occurred.

- **2026-10-20 — Independent MIDI/Beats build boundary implemented.** Moved the
  existing MIDI module into the shared instrument seam and its private session
  into the existing BLE adapter. The audio facade re-exports MIDI; the existing
  CLI defaults to standards-based pack/export/OS-port commands. Private RPC and
  PacketLogger require independent features; ordinary workspace builds select
  only the CLI. Composition and bounded compilation evidence live beside the
  audit, without replacing the earlier device or provenance receipts.

- **2026-10-20 — Split runtime acceptance passed.** Added real-CLI integration
  cases, exercised native virtual-port MIDI and retained kit removal/reopen,
  inspected emitted SMF through the existing AP3 inspector, and checked plain
  default/file-only builds with private-feature negative controls. Across the
  bounded matrix, 284 test executions pass. After Mark readied the guitar, the
  RPC-only binary read banner/status on H2S DSP V2.4.0 / ESP V1.3.0 and requested
  disconnect. No guitar settings/files changed; no message or publication.
