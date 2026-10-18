# Q-PC MIDI pedal experiment, 2026-10-17

Status: first software-pedal trigger succeeded. The direct peripheral obtained
a MIDI subscription and the owner confirmed an effect change after one CC-67
press/release pair. P0 is still in progress: firmware versions, app RPC positive
control, Beats control, and broader performance behavior are unqualified.

## First successful trigger

| Item | Recorded result |
|---|---|
| Path | Mac CoreBluetooth peripheral, advertised as `Ringdown`, explicit `--unencrypted-diagnostic` access mode |
| Guitar binding | Effect Next, channel 1, Control Change, number 67; configured by owner |
| Subscription | Observed at 23:56:57 EDT on October 17 (03:56:57 UTC October 18); maximum notification payload 497 bytes |
| MIDI stimulus | `B0 43 7F`, then `B0 43 00` after a scheduled 50 ms; both accepted by macOS |
| Owner observation | `The MIDI sign appeared.` Then, asked whether the effect advanced: `it did` |
| Scope | Effect change confirmed; effect names, number of advances, audible change and latency were not supplied |

See `effect-next-observation.json` and `direct-peripheral-unencrypted.log`.
This is an owner-confirmed musical-control observation with an independently
logged subscription/send, not a firmware-wide protocol Finding: the local
`GetStatus` positive-control requirement remains unsatisfied. The CC number
comes from the configured binding, not a universal guitar command table.

## Pedal action menu and Beats boundary

The owner read the complete MIDI action menu in order (after local midnight,
October 18):

1. loop record
2. loop record met
3. loop play stop
4. looper set volume
5. effect next
6. effect previous
7. effect switch
8. effect control
9. effect set volume
10. effect on/off
11. loop record met bars

See `midi-action-menu.json`. Case is not asserted; spoken “slash” is rendered
as `/`. There is no standalone Beats/metronome action in this reported menu.
The two recording-with-metronome labels have untested behavior and were not
triggered. This menu does not establish that Beats is inaccessible over app
RPC, nor that the recording entries offer independent rhythm control.
Volume/parameter labels suggest useful future continuous controls, but their
message/value semantics remain untested. Only Effect Next was exercised.

## Observations

- Host: Q-PC, Intel macOS 15.8.1 (24H32), Swift 6.1.2.
- Audio MIDI Setup opened. Automation through System Events was refused with
  `osascript is not allowed assistive access (-1728)`. The owner performed
  the advertising/pairing steps manually.
- The owner reported that the guitar paired successfully with Q-PC. This is
  an owner-reported panel result, not a confirmed CoreMIDI connection.
- `endpoints-after-reported-pairing.json`: an initialized CoreMIDI client sees
  the Apple Bluetooth MIDI driver with no entities and no active sources or
  destinations. Network MIDI has no entities; the IAC bus is offline. There
  is no eligible destination to send to. The earlier pre-pairing inventory
  did not create a MIDI client and is only a preliminary observation.
- The owner saw `Loop record / DISABLED` in the guitar's bindings menu. It
  was left disabled. The owner subsequently configured Effect Next as described below.
- The existing Ringdown probe built with the locked nested workspace in
  19.20 seconds. A single five-second app-service scan found no matching
  guitar (`rpc-status.log`, exit 1); no connection, GetStatus or write
  occurred. This does not establish why the app service was undiscoverable,
  whether MIDI and RPC coexist, or the installed firmware versions. The
  probe log's generic troubleshooting suggestions are not findings.

The owner subsequently confirmed that the fresh Q-PC pairing succeeded. The
absence of a CoreMIDI endpoint does **not** retract that panel observation or
prove a failed Bluetooth link. It means this helper has no OS destination to
send through. MIDI OFF/ON and one fresh pairing produced the same inventory;
see `endpoints-after-midi-toggle.json` and `endpoints-after-fresh-pairing.json`.

The owner described the bindings editor: channels 1–16, Note On, Note Off,
Control Change and Program Change types, and numbers 0–127. At the operator's
request, the owner set Effect Next to channel 1, Control Change, number 67.
After clearing/re-pairing, the owner answered `ok` to the request to confirm
that binding and return to the Effects screen; no current effect name was supplied.
Control Change was exercised for Effect Next below. Note On, Note Off and
Program Change have not been exercised.

## Direct peripheral diagnostic

The OS route exposed no destination, so a separate CoreBluetooth diagnostic
was prepared. This is a route experiment, not a conclusion that the guitar's
pairing failed. The helper publishes the standard BLE MIDI service/IO
characteristic and answers an initial read with empty data if requested.
The first version required encrypted access/notifications. Interface sources:

- [Silicon Labs BLE MIDI overview](https://docs.silabs.com/bluetooth/2.13/bluetooth-code-examples-applications/midi-over-ble)
- [Infineon BLE MIDI server overview](https://community.infineon.com/t5/Knowledge-Base-Articles/BLE-MIDI-Server-Example/ta-p/246340)
- [Apple peripheral-role tasks](https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/PerformingCommonPeripheralRoleTasks/PerformingCommonPeripheralRoleTasks.html)

The direct helper compiled without warnings. Runtime callbacks confirmed
Bluetooth authorization allowed, manager powered on, service registration
without error, and advertising accepted without error (`direct-peripheral.log`).
Audio MIDI Setup was closed before this helper began advertising. The first
name was `Ringdown Q-PC`; the owner reported no scan result and said they
also used Clear. The exact meaning of Clear/remaining bindings was not
observed by the operator.

The name was shortened to `Ringdown` to fit the name plus 128-bit service in
the initial advertising budget rather than relying on service UUID overflow.
The installed Apple SDK documents this budget in `CBPeripheralManager.h`.
This is a mitigation hypothesis; no over-the-air capture proves what the
initial advertisement contained. `direct-peripheral-short-name.log` records
the second service/advertising callbacks. During that encryption-required
attempt there were zero subscribers and no read/write callbacks. Advertising
acceptance alone did not prove discovery, and no callback did not prove no
physical link.

The owner subsequently reported successful pairing with `Ringdown` and
confirmed that the guitar was beside Q-PC. The Bluetooth daemon independently
records a successful incoming connection to H2-CC340, with the Mac in the
peripheral role. The session negotiated ATT MTU 500. An incoming ATT request
(opcode `0x12`) failed with Apple's internal `STATUS 4305`; its exact meaning
has not been established. The connection ended about 31 seconds after it
began, locally initiated, with `encrypted:0`. These are transport observations,
not current-firmware protocol Findings qualified by a `GetStatus` control.
See `bluetooth-gatt-handshake.json` for scoped daemon events with the address
and peer identifier redacted. The daemon's discovery of the guitar's app
service is not an app RPC session or a successful status read.

At 03:49:49 UTC the console's one `pulse67` attempt was refused locally because
there were no subscribers. No MIDI bytes were sent. This does not show that
the guitar rejected CC 67. Successful panel pairing and a live Bluetooth link
were both supported at this stage; MIDI notification subscription and musical
control were still missing.

Because the direct probe required encryption and the link remained unencrypted,
a single compatibility variation was prepared: `--unencrypted-diagnostic`
permits reads/writes/notifications without encryption. The default remains
encryption-required. This diagnostic is not a standards-compliance or shipping
security decision. The source compiled without warnings, the modified service
registered, and advertising as `Ringdown` started at 03:54:17 UTC. The owner
was asked to reconnect once. The guitar subscribed at 03:56:57 UTC and the
owner reported the MIDI sign appeared. At 03:58:00 UTC the operator issued one
`pulse67`; macOS accepted the press and release. The owner then confirmed the
effect changed. No automatic resend occurred. The application received no
read callback, so an initial read is not established as necessary for this
guitar's successful subscription.

`bluetooth-unencrypted-handshake.json` records the successful incoming link
and MTU 500. `local-gatt-service-table.json` shows one local MIDI service at
registration, reducing the duplicate-service hypothesis. This comparison
supports an access-requirement compatibility issue for the strict probe on
this session, but it does not decode status 4305, isolate every macOS cache
variable, or establish what encrypted operation the firmware supports.

An attempt to use the other iMac as a passive observer stopped at the
configured SSH authentication failure. No credential changes were attempted.

The action-list inspection above completes this bounded trial. Next work is
to build a resident peripheral and action dispatcher around the demonstrated
route, while qualifying firmware versions/app `GetStatus`, isolated value and
release semantics, Note/Program Change, latency, reconnect behavior, and
MIDI/RPC coexistence. Investigate Beats through observed panel/app controls
and recovered app RPC; no new rhythm method is guessed from these labels.
Recording, erasure, profile changes and message sweeps have not been attempted.
The diagnostic was closed at 00:00:25 EDT October 18 (04:00:25 UTC): it stopped
advertising, removed its service and exited normally. The owner-configured
MIDI binding/pairing were not reverted. No persistent config write was sent
by either helper. No full Rust suite or production UI validation was run.

## Probe helper

`midi_probe.swift` uses CoreMIDI's existing OS transport. It does not advertise
its own service. Build it from this directory with:

```sh
xcrun swiftc midi_probe.swift -o /tmp/ringdown-midi-probe-20261017
/tmp/ringdown-midi-probe-20261017 list
```

The helper compiles without warnings and its list operation ran successfully.
This is build and host-inventory evidence, not guitar-control acceptance.

Its `pulse` mode requires a destination unique ID, MIDI message type, channel,
number, press/release values, 30–100 ms duration, and a nondestructive action
label. Values must come from the session's explicit binding/experiment; no
example command here substitutes guessed bindings. It refuses offline or
ambiguous destinations and sends no automatic retries. OS send acceptance
is logged separately from an unobserved musical result.

`watch SECONDS` keeps a CoreMIDI client alive and prints inventory changes;
it sends no MIDI. `endpoint-watch.log` and `endpoint-reconnect-watch.log` are
the observations from two bounded watches.

`ble_midi_peripheral.swift` runs from a temporary app bundle with the supplied
Info.plist. It starts with no advertising or transmission. Encryption is required by
default; the explicit `--unencrypted-diagnostic` flag selects the temporary
compatibility experiment and is logged at startup. Subscription generation
prevents a delayed release from being replayed across a reconnect. Its explicit
console commands are `status`, `advertise`, `pulse67`, `stop` and `quit`.
`pulse67` is a narrow experimental channel-1 CC-67 press/release pair
(127, then 0 after 50 ms), refused unless exactly one central is subscribed.
It is not an established HyVibe command: its meaning depends on the owner's
binding. The first console pulse command was refused before transmission;
the second, in diagnostic access mode with a subscriber, sent the one event
pair described above. Runtime app files and executables live under `/tmp`;
the source and logs are retained here.

To rebuild the direct helper from this directory:

```sh
mkdir -p '/tmp/Ringdown MIDI Probe.app/Contents/MacOS'
cp ble-midi-probe-info.plist '/tmp/Ringdown MIDI Probe.app/Contents/Info.plist'
xcrun swiftc ble_midi_peripheral.swift -o '/tmp/Ringdown MIDI Probe.app/Contents/MacOS/ringdown-ble-midi'
'/tmp/Ringdown MIDI Probe.app/Contents/MacOS/ringdown-ble-midi' /tmp/ringdown-pedal-new-session.log --unencrypted-diagnostic
```

Startup does not advertise or transmit. Use `advertise` explicitly, obtain the
guitar's subscription, check the intended binding and visible effect, then use
`pulse67` once. `quit` stops advertising and removes this helper's services.
The fixed experimental trigger is not a general performance interface.

