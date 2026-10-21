# Ringdown

An independent cross-platform desktop controller for the HyVibe smart guitar, with performance
control and scripting as its next product target: prepare tones, trigger the
guitar from a keyboard or MIDI controller, and recall song cues. The existing
private client speaks the instrument's JSON-RPC-over-Bluetooth app protocol.
The independent standard path now exports Beats MIDI files and advertises a
software Bluetooth MIDI pedal through the shared MIDI writer.
The performance target covers Windows, Linux and macOS through shared Rust
logic and Cambium views, with native transport adapters. The Mac experiment
below is one adapter receipt within that cross-platform target.

A *ringdown* is how a resonating system decays once it stops being driven — the
tail of a struck string, or of a guitar body rung by the actuator inside it.
Controlling how long the instrument keeps sounding is much of what this thing
is for.

> **Status (2026-10-20):** reusable protocol/client libraries and a research
> probe exist. Earlier hardware sessions established status, metronome and
> effect control, recording retrieval and playable preset creation. The
> [desktop performance assessment and plan](design_docs/2026-09-02_client_surface_plan.md)
> covers MIDI pedal emulation, Beats, a desktop surface and scripting. A
> [Mac peripheral experiment](validation/midi-pedal_20261017/README.md) now has
> one owner-confirmed Effect Next trigger. A [shared Rust BLE transport](design_docs/2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20)
> now has another owner-confirmed trigger and Mac/Windows/Linux compiler evidence.
> Native Windows/Linux execution, timing and live Beats control remain unqualified.
> The [founding plan](design_docs/2026-08-27_ringdown_founding.md)
> preserves the protocol map and historical hardware findings.

## Independent MIDI and Beats build

Run from `ports/ringdown`:

```sh
cargo build --locked --release -p ringdown-probe
```

This is the default workspace member and feature set: shared pack/MIDI-file
export, the existing OS MIDI ports and optional standard BLE peripheral output.
It excludes private HyVibe RPC libraries, their btleplug central driver and the
PacketLogger diagnostic.

```sh
ringdown-probe --audio-pack bundled backbeat new-beat.mid new-rhythm.json
ringdown-probe --midi-ports
ringdown-probe --midi-send 'Exact OS output name' 1 cc 67 127
ringdown-probe --midi-send 'Exact OS output name' 1 cc 67 0
```

Replace the output name with a listed MIDI port and configure the corresponding
binding on the instrument. These sends use the existing MIDI driver and do not
confirm an instrument action. For a persistent software pedal:

```sh
ringdown-probe --ble-midi Ringdown
# Explicit compatibility for receivers that do not negotiate encryption:
ringdown-probe --ble-midi Ringdown --unencrypted-compatibility
```

In that console use `status`, `send 1 cc 67 127`, `send 1 cc 67 0`, `reopen`,
and `quit`. Configure the instrument's binding before sending. Startup, listing
and reopen send no MIDI. Sending requires exactly one current subscriber and
uses the same `MidiOut::send_raw` path as OS ports. Refused/expired/uncertain
messages are reported and never retried automatically. Keep the console open
while pairing; one-shot `--midi-send` does not wait for a BLE subscription.

Mac uses CoreBluetooth with Bluetooth permission; Linux needs BlueZ/D-Bus and
an advertising-capable adapter. Windows checks peripheral-role support and
uses its system advertised name. Its encrypted choice currently refuses to
open because notification-encryption enforcement is not established through
WinRT; compatibility is an explicit choice. Linux encryption additionally
requires BlueZ to accept notification encryption flags. No silent downgrade.
See the [platform assessment and bounded qualification](design_docs/2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20).

File export creates new local files; guitar delivery remains explicit.
`--no-default-features` builds file-only; add `--features midi` for OS ports
without the peripheral supplier. The shared Woodshed desktop MIDI output
picker exposes both BLE choices and its existing status line reports state.
Woodshed's existing Audio and MIDI panel now adds shared configurable Effect
Next/Previous bindings and triggers; `woodshed-genet --midi-pedal-stdio` accepts
JSONL shell commands through that same desktop owner and writer. Both bindings
start disabled. See the [binding, dispatch and scripting contract](design_docs/2026-09-02_client_surface_plan.md#software-pedal-controls-and-scripting-2026-10-20).
[Bounded acceptance](validation/pedal-acceptance_20261020/README.md) adds 57
focused software cases, actual macOS controls and fresh-process saved bindings,
independent scripted byte capture and an incomplete press/release outcome.
The owner confirms Next once and Previous once through the desktop writer;
MIDI OFF/ON reconnects without replay. The UI retains uncertain pedal outcomes
and their accepted-message counts. Native Windows/Linux, physical radio faults
and performance timing remain unqualified.

The same panel also provides saved keyboard and incoming MIDI Note/CC mappings
with Learn/Clear and explicit live arming. They feed the existing dispatcher;
learning sends nothing. Ordinary key autorepeat and repeated controller press
values are suppressed. The [input contract and focused evidence](design_docs/2026-09-02_client_surface_plan.md#keyboard-and-incoming-controller-mappings-2026-10-20)
record the shared-host synthesized-focus-press correction and the native/controller
qualification boundary.

Private research commands require explicit build features:

```sh
cargo run --locked -p ringdown-probe --no-default-features --features hyvibe-rpc -- --rpc --help
cargo run --locked -p ringdown-probe --no-default-features --features packetlogger -- --decode-pklg capture.pklg
```

`--workspace` or `--all-features` also selects research code and is not a
standards-only release configuration. The source repository still contains
private research modules; this is an independent binary boundary. See the
[build profiles, ownership and evidence](design_docs/2026-09-02_client_surface_plan.md#standards-based-midi-and-beats-build-isolation-2026-10-20).

## Split acceptance

The [2026-10-20 bounded acceptance](design_docs/2026-09-02_client_surface_plan.md#split-acceptance-2026-10-20)
passes 284 test executions across the selected feature/library/release cases.
Native virtual-port capture verifies the CLI's MIDI bytes; installed-pack
removal/reopen preserves MIDI; default/file-only builds exclude private code.
A read-only live RPC status check succeeds on H2S DSP V2.4.0 / ESP V1.3.0.
That earlier receipt covers dependency selection for Windows/Linux, not native
execution. The subsequent BLE slice adds target compilation and a Mac guitar
trigger; native Windows/Linux execution and broader guitar Beats profiles
remain open.

## Desktop reconnect acceptance

The [bounded desktop/reconnect qualification](design_docs/2026-09-02_client_surface_plan.md#desktop-path-and-reconnect-qualification-2026-10-20)
passes 36 focused cases and three native macOS cohorts. Actual desktop MIDI
clock controls produce exact independent receiver parity (293 messages); an
endpoint-removal/recreation trial pauses output, preserves its failure report,
and waits for explicit rearming (131 messages, exact receiver parity). The
actual BLE picker detects and reconnects to the owner's guitar without another
desktop click; that lifecycle check keeps clock off and sends zero messages.
A desktop BLE service reopen needed one guitar MIDI OFF/ON toggle to resubscribe.
Native Windows/Linux, physical BLE congestion, timing and runtime acceptance of
the subsequently implemented pedal controls remain separately scoped. Older receipts are retained unchanged.

## Existing capabilities

The following have historical device evidence; current firmware needs a fresh
qualification run. The typed driver/profile path also has scripted-link
coverage, which is distinct from those hardware sessions.

- Read identity, battery, storage and both firmware versions.
- Read and drive the instrument's metronome.
- Read the guitar's own measured body resonances — the plate modes it derives
  by calibration and notches to prevent feedback.
- Edit a preset's effect chain and hear it: add, remove, reorder and bypass
  any of the thirteen effects, with every knob the vendor app exposes. Rename
  presets and switch between them. Read the recordings stored on the device.
  Create a playable preset with the bank shape established by H47; save a
  local shadow profile for restoration. The vendor app was observed to
  overwrite the guitar's profile when it connects.
- Reach methods the vendor's app did not call in the original investigation,
  five of which were confirmed to exist on that firmware.

The driver refuses `ReadConfig`: it hung the observed firmware's RPC handler
until power-cycled. Working calls reach portions of that information. The
performance path retains this guard unless a firmware fix is established.

The [2026-10-18 USB inspection](validation/usb_20261018/README.md) read
stored user/device configuration with nine tone banks, listed 44 recording
files, and inspected ten drum MIDI-file headers. This supplies a file-based
snapshot source and stored Beats selection keys. Later [AP3 trials](../../validation/audio-packs-ap3_20261020/README.md)
confirmed original custom MIDI playback, reboot persistence and a scoped
five-bar 4/4 intro/loop rule. Broader firmware and live Beats RPC control remain
unqualified.

## Design

The private protocol was researched for interoperability using Android static
inspection and owned-device observations. Its provenance and terms questions
are recorded in the audit below. Architecture follows the retinue template — a sans-io protocol
core, a thin Bluetooth I/O shell, and receipt-driven validation.
[Woodshed](https://github.com/merely-made/woodshed) is the first consumer and
repository host. Ringdown remains a separately versioned and released nested
workspace under `ports/ringdown`.

Ringdown is not affiliated with or endorsed by HyVibe; "HyVibe" is their
trademark, used here only to say what this interoperates with.

Start with [`design_docs/DOC_README.md`](design_docs/DOC_README.md).

## Provenance and publication scope

The [2026-10-20 bounded audit](design_docs/2026-09-02_client_surface_plan.md#provenance-terms-and-standards-audit-2026-10-20)
separates public MIDI/SMF/JSON-RPC conventions from HyVibe-specific GATT,
framing, codec and command semantics. It records unresolved research-artifact,
terms, implementation-derivation, diagnostic-reference and release-packaging
questions; it is not legal clearance. A vendor outreach draft is saved in the
same plan and has not been sent.
