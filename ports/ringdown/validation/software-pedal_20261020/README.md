# Software pedal implementation — 2026-10-20

Compiler evidence for the new configurable Effect Next/Previous controls and
same-owner desktop JSONL scripting. [Canonical contract and open qualification](../../design_docs/2026-09-02_client_surface_plan.md#software-pedal-controls-and-scripting-2026-10-20).

## Implemented

- One portable `woodshed-instrument::pedal` model for bindings, validated message
  encoding and explicit outcomes; the existing raw probe parser reuses it.
- Disabled-by-default retained preparation in `AppSettings`, current Cambium
  MIDI panel controls and transient one-shot requests through `MidiBackend`.
- UI and opt-in `--midi-pedal-stdio` share one bounded admission gate and the
  current output worker/writer. No second connection, clock worker or RPC path.
- Selection/subscriber guards and deadlines on each message, scheduled optional
  release, visible partial/uncertain results and cancellation without replay.

## Compiler results

All six commands in [receipt.json](receipt.json) exit 0 with Rust 1.99.1:

| Scope | Evidence |
| --- | --- |
| macOS desktop | [build](logs/build-final-source.log) |
| Driver-free portable model | [check](logs/portable-check.log) |
| Windows instrument + BLE target | [check](logs/windows-check.log) |
| Linux instrument + BLE target | [check](logs/linux-check.log) |
| Ringdown default CLI | [check](logs/probe-final-source.log) |
| Ringdown file-only CLI | [check](logs/probe-file-only.log) |

Windows/Linux checks compile the shared instrument/model and maintained native
transport dependencies. They do not compile/link/run the complete desktop app
on those systems. The desktop dispatcher, stdio and view additions use shared
Rust/Cambium/std APIs with no new platform branches; only macOS has a desktop
build in this receipt. Existing unrelated deprecation/dead-code warnings remain.

## Limits and preservation

No tests were added or run, no UI process was launched, and no guitar action,
RPC call or device file write occurred in this implementation pass. New control,
scripting, persistence and cancellation behavior remains runtime-unqualified.
Older source-bound hardware/native receipts are unchanged. Neither new Effect
Previous support nor an accepted MIDI message proves a musical outcome.

[source-hashes.json](source-hashes.json) freezes the implementation sources and
locks. Pre-turn source/status/patch copies and build attempts remain in
`/Users/markik/Code/testing/woodshed/software-pedal_20261020/`. Only the two new
dependency edges changed lockfile data; no dependency versions were refreshed.
No commit, push, publication or outreach was performed.
