# MIDI Design Note

Brief design note covering MIDI in/out for Woodshed. Captures the
decisions that matter — transport ownership, clock sync, what we do
NOT do — before any code lands. Implementation lives in
[`crates/woodshed-instrument/src/midi.rs`](../crates/woodshed-instrument/src/midi.rs).
Since 2026-10-20 `woodshed-audio::midi` re-exports that single implementation;
the existing host backend/clock worker retain output ownership. Ringdown uses
the same module for standard OS-port commands and the optional BLE peripheral
variant. The
[independent build boundary](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#standards-based-midi-and-beats-build-isolation-2026-10-20)
separates those commands from private app RPC. The original design scope below
remains historical; MIDI input does not currently trigger the audio engine.

## BLE output integration (2026-10-20)

`MidiOut` owns either an OS port or the optional `ble-midi` transport; both use
`send_raw` and existing message helpers. The desktop picker exposes encrypted
and explicit unencrypted compatibility choices and reuses its current output
worker/status line. Selecting/listing a choice sends no musical message.
Clock resumes only after a fresh stopped/disabled state when reconnecting
while playing. Incoming sound triggering and guitar clock reception are not
established by this slice; software pedal controls are implemented below.

The [Ringdown transport assessment and receipt](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20)
own platform reuse, native lifecycle, security, compile/hardware evidence and
remaining qualification. One vendored peripheral supplier provides the native
backends; no private app RPC dependency enters this crate. Mac/BlueZ request
notification encryption by default. Windows refuses that requirement because
its enforcement is not established here; explicit compatibility is available.
Woodshed audio and the default Ringdown CLI enable BLE; file-only and OS-port-only
profiles remain available. The historical design below does not override these
implemented ownership boundaries.

[Desktop/reconnect acceptance](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#desktop-path-and-reconnect-qualification-2026-10-20)
adds actual native picker/clock/refresh/close controls and independent MIDI
capture, plus a zero-message guitar BLE lifecycle check. Pending selection
revisions and expected subscriber generations prevent replay into replacement
connections. The existing midir port path retains endpoint identity/liveness;
OS success alone did not detect an erased CoreMIDI destination. Failed/unknown
outcomes remain visible across reconnect. Platform/compiler and radio-delivery
limits are recorded in that receipt.

## Software pedal preparation and dispatch (2026-10-20)

Settings → Audio and MIDI now exposes configurable Effect Next/Previous
bindings and explicit trigger buttons. The device-free
`woodshed-instrument::pedal` model owns validated MIDI encoding; durable
`AppSettings.audio_midi.midi_pedal` stores bindings, initially disabled. Current
Cambium buttons/steppers and the portable `MidiBackend` seam feed one-shot
requests to the existing output worker and `MidiOut`. Optional release scheduling
uses that loop. No parallel writer, private RPC or platform-specific UI is added.

`woodshed-genet --midi-pedal-stdio` enables structured shell requests to that
same owner, using its configured bindings and selected receiver. UI and scripts
share one pending-action gate, selection revision, subscriber generation,
short deadlines, partial-outcome reporting and cancellation with no replay.
Only preparation persists. MIDI acceptance never claims an observed effect.
The [canonical contract and qualification limits](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#software-pedal-controls-and-scripting-2026-10-20)
include the JSONL examples. [Bounded pedal acceptance](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#pedal-acceptance-2026-10-20)
adds 57 focused software cases, actual macOS controls/save/reopen, independent
stdio/partial-send capture and owner-confirmed guitar Next/Previous plus reconnect.
The visible result includes accepted-message count; the existing output state
retains the last uncertain pedal report across later status/refusal/reconnect.
Native Windows/Linux, radio faults and timing remain separate qualification
boundaries; historical receipts are not relabeled.

## Keyboard and incoming-controller mappings (2026-10-20)

The existing pedal panel adds saved per-action Learn/Clear keyboard and Note/CC
mappings, plus transient arming. Shared-host key interception and the current midir parser/callback feed the
same target-bound dispatcher/writer as buttons and stdio. Native qualification
found frame rendering could exceed the previous history cutoff: the sole live
controller edge gate now runs on that callback, while sequence-consumed history
serves learning/display only. Learning establishes its cursor at the click, and
explicit input None retires requested intent even after confirmed disconnection. Ordinary keyboard autorepeat and held controller values are
suppressed; learning sends nothing, input history is consumed once, and lifetime
changes disarm. See the [canonical input contract](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#keyboard-and-incoming-controller-mappings-2026-10-20)
and [focused input evidence](../ports/ringdown/validation/pedal-inputs_20261020/README.md).
A narrow source patch at the existing shared host rejects synthesized focus
presses before dispatch; metadata confirms one resolved host. This implementation
has focused retained-control/gate/build evidence; the [input acceptance packet](../ports/ringdown/validation/pedal-input-acceptance_20261020/README.md)
adds 48 focused cases and a three-process/35-message native controller/fault
cohort. The separate physical cohort confirms real held N/P and fresh CC/Note
actions after reconnect, with 12 OS-accepted messages and five single guitar
changes; one earlier Note-triggered no-change remains unresolved. Reconnect
disarms and does not replay held input. Windows/X11 focus and native runtime on
other platforms remain unqualified; OS acceptance is not device acknowledgement.
The [Previous-source investigation](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#previous-source-diagnostics-2026-10-21)
adds a default-off bounded journal, exposed by existing desktop JSONL Status,
for exact writer bytes/timing and BLE subscription/notification evidence. It
changes neither admission nor release/retry semantics. Its 38 focused cases and
eight-message independent native comparison pass; real P, sparse Note and held
Note each return Rev to Off with the same CC66 bytes in the traced guitar trial.
The earlier miss did not reproduce and its cause remains unproven.
The [desktop diagnostic controls](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#desktop-diagnostic-controls-2026-10-21)
now expose session-only Record/Stop, bounded recent history and full JSON export
in Audio and MIDI. Recording remains off on a normal launch; stopped history is
retained in memory. Export and scripting share one Status projection. This new
UI is compile-qualified only; prior native/device receipts do not qualify it.


---

## What MIDI is good for here

**MIDI Out (Woodshed is the source):**
- Trigger an external drum machine or soft-synth in time with the
  Woodshed metronome / sequencer.
- Send MIDI Clock so external gear locks to our tempo.
- Send Transport (Start/Stop) so external gear plays/pauses with us.

**MIDI In (Woodshed is the listener):**
- Receive MIDI Clock from a DAW or hardware sequencer and slave
  Woodshed's transport to it. Letting the user jam *with* their DAW
  is high-value: you start your DAW project, Woodshed locks in.

We do **not**:
- Use MIDI to trigger Woodshed sounds via Note On/Off. Woodshed is a
  practice tool, not a synth. If a user wants a MIDI-controlled
  drum machine, they should use a real drum machine.
- Accept MIDI Note input to "play along" notes. Onset detection from
  audio is more useful for a guitarist — and a fretted-instrument
  player isn't going to be plugging in a MIDI controller anyway.

## Transport ownership — who's the master?

A user-facing setting picks one of three modes:

| Mode | Master | Clock source | Transport source |
|------|--------|--------------|-----------------|
| **Internal** (default) | Woodshed | Internal | Internal |
| **MIDI Clock master** | Woodshed | Internal, sent on Out port | Internal, sent on Out port |
| **MIDI Clock slave** | External | Received on In port | Received on In port |

The default has to be Internal — a fresh install with no MIDI gear must
just work. The two MIDI modes are opt-in and surface in a "MIDI" tab
or settings panel.

In Clock-slave mode, the BPM slider and Tap button are display-only;
Play/Stop in the UI still works but its semantics shift to "arm" — the
sequencer waits for an incoming MIDI Start before producing audio.

## MIDI Clock specifics

MIDI Clock is 24 PPQN (pulses per quarter note). Wall-clock interval
between consecutive ticks gives:

```
secs_per_tick = (t_n - t_{n-1})
secs_per_beat = secs_per_tick * 24
bpm           = 60 / secs_per_beat
```

In practice we average over ~24 ticks (one beat) and smooth with an
exponential moving average so transient jitter from the host doesn't
make our displayed BPM dance.

Transport messages we care about:
- `0xFA` Start — set play position to beat 1, begin playing.
- `0xFB` Continue — begin playing from wherever we are.
- `0xFC` Stop — pause.

## Out-of-scope (this pass)

- Sysex / device-specific config.
- MTC (MIDI Time Code) — frame-accurate sync. Different use case
  (post-production), not jamming. If someone files an issue we can
  revisit.
- MIDI 2.0. Tooling support is still spotty in 2026.

## Implementation plan

1. **Enumeration** — `list_input_ports()`, `list_output_ports()`
   returning `Vec<String>` of human-readable names. No device opening.
2. **`MidiOut`** — connect once, send raw bytes. Convenience methods:
   `send_note_on`, `send_note_off`, `send_clock_tick`, `send_start`,
   `send_stop`, `send_continue`.
3. **`MidiIn`** — connect once with a callback that forwards parsed
   `MidiEvent` values into a shared queue. UI thread polls the queue.
4. **`MidiClockSync`** — pure helper. `fn record_tick(at: Instant)`
   and `fn estimated_bpm() -> Option<f32>`. Tested without hardware.
5. **No app-side wiring this pass** — the audio crate exposes the
   types; integration with `SequencerEngine` transport and the UI
   surface comes after the user sees the shape.

## Why this order

The pure helpers (`MidiClockSync`, parsing) are testable without
hardware and capture the algorithm. The `MidiOut` / `MidiIn` wrappers
are thin enough that platform-CI absence isn't a blocker — once they
compile and connect, the runtime behavior is in midir's hands.
