# Software pedal acceptance — 2026-10-20

Bounded software, native macOS and owner-observed guitar qualification of the
[shared pedal contract](../../design_docs/2026-09-02_client_surface_plan.md#pedal-acceptance-2026-10-20).
The preceding [implementation receipt](../software-pedal_20261020/README.md)
remains compiler-only evidence for its original source.

## Results

| Layer | Acceptance |
| --- | --- |
| Focused software | **57 cases pass**, including 21 new regressions: host/admission/worker/stdio/selection/key policy 28; shared instrument 21; settings round trip 1; existing CLI 7. |
| Actual desktop controls | Native retained hit testing configures channel, type, number, values, release and enablement, then presses both action buttons. Preparation sends zero MIDI; two gestures produce four exact received messages. |
| Fresh-process persistence | Live and saved bindings match exactly. A second process restores both bindings with no output selection, pending action, report or attempted send restored. |
| Desktop scripting and actual partial send | Same-owner JSONL uses the selected output and saved bindings. Nine OS-accepted messages match nine independently received messages, including a single press whose release is cancelled when its endpoint disappears. |
| Physical guitar | Owner confirms Next advances once and Previous moves back once through the desktop's stdio/writer. Four MIDI messages are OS-accepted, with no send failures. MIDI OFF/ON changes subscriber generation without increasing that count. Output closes afterward. |
| Windows/Linux | Shared instrument/model/BLE target checks pass. Native full desktop linking, interaction and physical adapters are not qualified here. |

Final source, logs, counts and cohort identities are in [receipt.json](receipt.json).
Native cohorts and their immutable presented captures are under [native/](native/).
Only final cohorts are acceptance evidence; [attempts.json](attempts.json) retains
fixture failures and preceding runs separately.

## What the fault tests establish

The tests call the production dispatcher, worker step, stream loop and retained
control/key policy. Fake connections and bounded in-memory streams are injected
at existing I/O boundaries; there is no alternate MIDI driver or scheduler.

- Every channel 1–16, number 0–127 and supported message kind is encoded against
  original expected bytes (8,192 combinations). Release status/value/channel
  rules, disabled defaults and invalid deserialized ranges are covered.
- Disabled, unready, stale selection/generation, occupied slot and closed worker
  refuse admission. Queued requests are terminally cancelled on drop/close.
- First-send writer errors remain uncertain even with zero confirmed messages;
  release writer errors retain one accepted message. There is no retry.
- Disconnect, replacement between polls, output revision mismatch and expiry are
  checked both before a press and after it. A replacement cannot inherit release.
  The existing clock can progress during the scheduled press/release gap.
- The actual desktop controls suppress 32 repeated Enter/Space activations while
  accepting fresh activations. Disabled/unready controls do not admit actions.
- JSONL tests cover status, duplicate IDs, disabled actions, missing target,
  unknown actions/fields, malformed JSON, invalid IDs/generation, oversized-line
  draining and a terminal input error. The latter emits one error then stops.
- Existing settings serialization round-trips both prepared bindings without live
  output/command fields. Native save/reopen supplies filesystem/runtime evidence.

## Native independent delivery and partial outcome

The final configure/reopen cohort uses one frozen app binary across two processes,
a private synthetic no-wallet Keeper, isolated profiles and the receiving-only
`midi_capture` fixture. Mesquite supplies genuine retained pointer input and
presented-frame capture. Observation hooks read state/saved settings only.

The first process configures channel-1 CC67 Next and CC66 Previous, values 127/0
with release enabled. It cycles all four message kinds and exercises channel,
value and release-value steppers, including bounds. Both saved mappings match
exactly before any output selection. The two UI buttons emit four messages.

The second process restores preparation, with no live connection or request.
Two scripted gestures emit four messages. The receiving endpoint is then removed
immediately after recording the next press. The desktop reports **uncertain,
one message accepted by the OS**, cancels release, and sends no retained message
into the replacement. Offline, duplicate-ID and old-target commands are refused.
After explicit output reopen, two fresh actions emit four more messages.

The whole cohort has **13 accepted / 13 received messages**, in exact order,
across two receiving endpoint lifetimes: four in the first process and nine in
the second. This is deliberate cancellation of the missing release, visibly
reported; it is not a claim that every action completed. No clock messages occur.

The final capture gate waits until the partial-outcome frame is actually saved
before submitting the next request. This prevents an offline refusal from racing
and replacing the very readout being inspected. Incomplete history remains
visible after reconnect, later accepted actions and explicit output close.

## Physical guitar checkpoint

The owner cleared and paired Ringdown again and confirmed the guitar's
Next/Previous MIDI mappings. Woodshed's real output picker selects the explicit
unencrypted compatibility output; zero messages are sent until a subscription
is observed and the owner confirms readiness. Separate JSONL requests produce:

1. Channel 1 / CC67 / 127 then 0: owner reports “once.”
2. Channel 1 / CC66 / 127 then 0: owner confirms it moved back exactly once.

MIDI OFF is independently observed as not ready, with one disconnect and four
accepted messages. ON produces subscriber generation 2 after generation 1, still
with four attempted/accepted messages and zero send failures. After a quiet
interval, the native scenario selects None and closes the output. Clock remains
off; no private RPC, guitar file write or extra effect gesture is performed.
Only synthetic application settings are copied into the hardware cohort; the
normal user profile is never opened by the acceptance runner.

The hardware cohort's frozen binary predates the final **closed-output readout**
repair. That last repair changes status formatting/retention and its regression;
it does not change pedal encoding, admission, scheduling, writer or BLE delivery.
Its final-source acceptance is supplied by the software and virtual native
cohort. The hardware binary hash is retained separately rather than relabeled.

## Repairs and evidence limits

Qualification added accepted-message counts to visible pedal results and retained
the last uncertain pedal report in the existing output state. Ordinary transport
failures, refusals, reconnects and successful actions cannot erase it. A final
native assertion found that Connect(None) overwrote the retired writer's readout;
one shared closed-status formatter fixes both close paths. This remains bounded
last-failure state, not a durable event journal. A terminal stdio I/O error also
now stops its input lane rather than repeatedly returning errors.

Actual OS endpoint removal establishes one partial gesture; injected writer
errors cover other uncertain outcomes. Physical BLE congestion/partial delivery,
latency, clock phase, encrypted guitar interoperability, other instruments or
firmware, incoming controller mappings and native Windows/Linux operation remain
open. OS acceptance alone is not device acknowledgement; the two guitar actions
add the owner's observations, without exact effect names or a new firmware read.

All historical receipts and concurrent work are preserved. No commit, push,
publication or outreach occurred. Binaries, private custody, failed attempts and
baseline source copies remain in the scratch path recorded in the receipt.
