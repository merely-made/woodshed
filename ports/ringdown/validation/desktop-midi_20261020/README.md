# Actual desktop MIDI and reconnect qualification — 2026-10-20

**Bounded macOS desktop and guitar lifecycle acceptance passes.** The existing
[client surface plan](../../design_docs/2026-09-02_client_surface_plan.md#desktop-path-and-reconnect-qualification-2026-10-20)
owns the implementation and remaining platform/fault scope. This receipt does
not replace the earlier CLI-only BLE or standards-split evidence.

## Native results

All three final cohorts launch the same frozen native Woodshed binary, use
isolated synthetic profiles and a private no-wallet Keeper, and close normally.
Mesquite delivers real retained pointer input through the shipping host's hit
testing; capture readbacks are from the presented application, not desktop grabs.
The observation actions read state and never choose a port or inject MIDI intent.

| Cohort | Actual controls and observations | Independent result |
|---|---|---|
| [Clock/output](native/clock/) | Settings → Audio and MIDI; output picker, Send clock, Tools → Metronome Play/Stop, Refresh ports, None, reopen while idle and while playing, explicit rearm | **293 OS-accepted / 293 received messages**: three Starts, three Stops, 287 ticks. No messages on idle reopen or playback reconnect before rearm. |
| [Endpoint hotplug](native/hotplug/) | Receiving endpoint disappears and is recreated with the same friendly name while playing; visible disconnect, None/reopen, explicit rearm and final Stop | **131 OS-accepted / 131 received messages** across two endpoint lifetimes. One detected disconnect; no replay into the replacement before rearm. Last failure remains visible after successful reconnect. |
| [Guitar BLE lifecycle](native/guitar/) | Actual compatibility picker, subscription, owner MIDI OFF/ON, desktop None/reopen, second subscription and final None | **Zero attempted, accepted or failed musical messages.** Clock stays disabled. The readout changes after guitar disconnect without another desktop click. Desktop reopen required one additional owner MIDI OFF/ON to resubscribe. |

Fourteen presented captures and the checkpoint JSON files retain these states.
The guitar was not sent Clock, Start/Stop or a guessed action; no guitar files or
configuration were written by software. This is a lifecycle check, separate from
the prior owner-confirmed CLI Effect Next trigger. The output has been closed.

## Repairs established by the tests

- Elevate the existing MIDI selector row: its None option previously hit the
  clock button underneath it. The first native attempt failed that assertion.
- Forward desired selection even when no open is confirmed. Selection revisions
  cancel superseded opens, and the worker closes an abandoned result without
  publishing it or sending. Refresh preserves names, not old numeric indices.
- Publish asynchronous status during frames so disconnect/reconnect does not
  require another click.
- Bind output to a subscriber generation, including the final send. Replacement
  entirely between worker polls cannot inherit an old transport intent.
- Validate the original OS-port endpoint identity and liveness using the existing
  midir inventory. CoreMIDI's removed-endpoint trial had reported **123 accepted
  versus 42 received**. The final repaired trial matches 131/131 and pauses visibly.
- Keep failed/unknown send, Stop/close and disconnect reports with counters across
  reconnect. A later successful clock tick cannot clear the report. Queue pressure
  and a closed worker are distinct; expired/cancelled commands are not replayed.

The original output loop was extracted into the same host's `midi/worker.rs`;
the inline predecessor is removed. Fault tests exercise that production step
through a private injected connection. The receiving virtual fixture never sends
MIDI. Neither is another product MIDI driver, scheduler or session.

## Software gates and reproduction

Rust/Cargo/rustdoc 1.99.1; locked/offline with cached dependencies. Native debug
information is disabled and build jobs are bounded to four. Cargo runs from
`/tmp` with the absolute manifest to reuse the qualified artifact directory.

```sh
cargo test --manifest-path /Users/markik/Code/repos/woodshed/Cargo.toml \
  --locked --offline -p woodshed-genet midi -- --nocapture
cargo test --manifest-path /Users/markik/Code/repos/woodshed/Cargo.toml \
  --locked --offline -p woodshed-instrument --features ble-midi
```

From `ports/ringdown`, `cargo test --locked --offline -p ringdown-probe --test
standards_cli` exercises the real CLI. **36 focused executions pass**: 12 host/
selection cases, 17 shared instrument cases and seven CLI cases. Eighteen are new
regressions. This is not a whole-workspace suite. Native desktop build and
Windows GNU/Linux GNU shared-transport target checks pass; the Linux check uses
the prior assessment's real Debian development sysroot. No native Windows/Linux
execution is claimed. Fresh default dependency graphs still exclude private RPC.

`native.py <new-cohort> <scenario.scn>` freezes the app and launches the existing
native lane with synthetic custody. It optionally starts `midi_capture`, a
receiving-only virtual-port example. The endpoint-removal scenario controls only
that fixture. Guitar readiness uses bounded waits and owner checkpoints.

Scenario `wait` waits for idle with a frame budget; it is not a wall-clock dwell.
Final quiet/hotplug checks use `act midi-dwell <milliseconds>` then the existing
wait. [attempts.json](attempts.json) preserves failed input, endpoint-loss and
earlier wait-semantics attempts. They are not final interval passes.

## Evidence and limits

[receipt.json](receipt.json), [source-hashes.json](source-hashes.json),
[source-delta.json](source-delta.json) and [logs/](logs/) retain scope and compiler
outcomes. A null source-delta baseline means that path was not inventoried at
startup, not necessarily that the file is new. Each native folder contains its frozen scenario, binary digest,
process/presentation result, checkpoint observations, verification and captures;
receiving cohorts also retain raw original MIDI bytes. Binaries, synthetic vaults
and baseline source copies remain in scratch under
`/Users/markik/Code/testing/woodshed/desktop-midi_20261020/`.

There were no unexplained missing bytes or control replays in the final receiving
runs. MIDI OS acceptance still has no application-level device acknowledgement;
these results cannot rule out arbitrary radio loss. Physical BLE congestion,
sleep/radio faults, latency/phase, encrypted guitar interoperability, other pedal
actions and native Windows/Linux operation remain separate qualification cues.
The product retains counters and its last failure, not an unlimited delivery
journal. Existing desktop controls are clock/output controls; no guitar CC/pedal
mapper UI was added. Earlier dirty work and historical receipts remain intact;
no commit, push, publication or outreach occurred.
