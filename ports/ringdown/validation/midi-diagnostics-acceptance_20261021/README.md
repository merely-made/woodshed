# MIDI diagnostics desktop acceptance — 2026-10-21

**Bounded acceptance passes.** The existing journal, Status projection, desktop
writer, settings controls and native export worker are exercised together. The
current desktop binary is byte-identical to the accepted final native binary.
No guitar was required. Earlier implementation/device packets remain frozen.

## Results

| Coverage | Evidence and result |
|---|---|
| Default off | `native/controls/midi-default.json`: recording off, no history, zero MIDI attempts, no startup environment opt-in. |
| Start / Stop | Real retained controls capture one writer gesture and retain ten events including start/stop boundaries. |
| Recording stopped | A second real writer gesture reaches the independent receiver, advancing accepted count 2 → 4; the diagnostic snapshot is exactly unchanged. |
| Restart / history | Restart appends to 20 events, with four recording boundaries, zero overwritten and zero truncated entries. Show latest events presents the last 12; counts agree with Status. |
| Native Cancel | Actual native Escape/Cancel releases the export job and displays “Diagnostic export cancelled; history retained.” Full history remains equal. |
| Native Save | Actual native Go to Folder / Save creates `native/controls/midi-diagnostics.json`. Every field of its `status` equals the independent scripting Status response, excluding only the request id. |
| Independent delivery | Six exact messages: three channel-1 CC67 press/release pairs. The middle pair is deliberately unrecorded. |
| Reopen | Exit with recording on, reopen the same encrypted profile in a fresh process: recording off, no history, saved pedal bindings retained. Diagnostics do not enter saved settings. |
| Pending close / Cancel | In the headed app, the existing host close command is deferred, with the guard notice visible in state. Native Cancel wakes completion and allows normal process exit. Zero MIDI attempts. |
| Pending close / Save | Same close path; native Save writes a complete Status snapshot before completion wakes the guard and exits normally. Zero MIDI attempts. |

`receipt.json` and `native/controls/verification.json` record the assertions and
binary/export hashes. Native control and reopen runs finish with `RESULT ok`.
The two deliberate close runs terminate before scenario completion: their generic
launcher reports `success: false` solely because there is no `scenario.done`.
The dedicated close assertions require exit code 0, no timeout, no surviving
post-close checkpoint, the observed keep-visible guard, and the recorded native
Cancel/Save completion. Those close checks pass.

## Focused software checks

**39 distinct cases pass**: 36 desktop MIDI/control/selection/export lifecycle
cases; two bounded-journal/truncation cases; one runtime opt-in lifecycle case.
The lifecycle case has its own test process so its process-local toggle cannot
interfere with unrelated transport tests. It covers default off, lazy disabled
recording, idempotent controls, retained stop, append on restart, and a producer
racing Stop. No event can append after the stop boundary in that race.

The existing windowless host harness checks pending native-close policy through
cancel, successful save, and worker failure. Injected worker results are labeled
as software tests; they are not native file-dialog evidence.

## Native observation boundaries

Native app controls are retained, hit-tested actions through the production
view/dispatch path. The save chooser is the real macOS panel, answered through
native keyboard events. The close command comes through the existing
`WindowCommands::close` seam and production host policy; the pause/checkpoint
commands observe it without replacing the close guard or export worker.

A separate OS observation matters: macOS Quit with the save panel modal returns
User canceled (-128). Canceling that panel leaves the app open; a fresh native
Quit after dismissal exits normally. This differs from a close already queued
through the host command. Both observations are retained and not conflated.

The final native cohort is `controls-r3`, `reopen-r2`, `close-cancel-r2`, and
`close-save-r1` in the scratch directory recorded in the receipt. The final
controls driver completes without manual intervention. Earlier attempts are
summarized in `attempts.json`: one interrupted save sequence/gate timeout,
superseded native-window discovery, a premature Save confirmation, and the
initially unexpected native Quit refusal. Those observations prompted driver
repairs and a clean repeat; no musical retry, writer, export or timing change
was made. Full desktop screenshots containing other apps remain outside this
packet; published native dialog captures are limited to the observed panel.

## Reuse and limits

`run.py` reuses the native launcher, synthetic keeper, encrypted fixture profile
and independent virtual MIDI receiver. `observe.py` only queries Status.
`native_dialogs.py` answers actual dialogs. `verify.py` checks independent data
and expected close outcomes. Each run freezes its binary and scenario externally;
this packet retains hashes, recipes, observations and captures without binaries
or profile keys. The native window inventory helper is reused from the existing
Turnstone acceptance infrastructure and recorded by hash in the receipt.

Native runtime scope is macOS on Q-PC. Windows/Linux native UI and dialog
behavior remain unqualified. Their earlier compiler results remain in the
separate implementation packet. Virtual delivery does not establish physical
instrument action, BLE reception, or hardware acknowledgement. There was no
persistent guitar write, publication or new production MIDI implementation.
