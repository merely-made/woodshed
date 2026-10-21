# Pedal input acceptance — 2026-10-20 to 2026-10-21

**Software/native acceptance passes; physical keyboard, controller and reconnect
checks are complete with one unresolved guitar observation.**
The final native cohort uses three fresh app processes and independently
records 35 exact messages. Forty-eight distinct focused cases pass.
The artifact directory retains the run's start date. See the
[canonical input contract](../../design_docs/2026-09-02_client_surface_plan.md#keyboard-and-incoming-controller-mappings-2026-10-20).
The [initial implementation receipt](../pedal-inputs_20261020/README.md) remains
frozen evidence for its predecessor source.

## Native method and current evidence

The existing Mesquite lane drives the real retained app controls, rasterizes
and presents the normal desktop UI, and captures that presented source. An
isolated synthetic keeper/profile supplies actual encrypted settings custody.
No user profile, guitar files or private RPC is involved. The existing
receiving-only `midi_capture` fixture records independent output bytes. The
new `midi_controller` example is a qualification-only virtual source, using
the existing midir API to feed the app's selected input. It does not replace
any product driver or perform mapping/output scheduling.

`native.py` configures the output bindings with ordinary controls, learns N/P
and channel-2 CC20/channel-3 Note36, refuses duplicate assignments, and verifies
preparation sends zero messages. Its next process checks saved preparation and
zero restored arming/learning/selection/pending intent. Clear/reassign changes
are saved and checked by a third process. Native source/receiver replacement
and held-value checks share the product's parser, dispatcher and output writer.

**Key injection boundary:** the scenario calls the production application
interceptor with explicit `KeyPress` data and runs its consumed-key dispatch
tail. It does not inject OS key events or qualify native focus delivery. Real
keyboard operation is reserved for the separate owner-observed guitar trial.
The shared focus-snapshot gate retains its direct production-module test;
Windows/X11 native focus switching remains unqualified on this Mac.

The configuration process independently records **25 exact messages**:
64 injected matching key repeats add none; held CC and note values add one
gesture per release/press; both Note Off and zero-velocity Note On release;
wrong channel/number and disarmed presses add none. Source removal disarms,
explicit None/reselection reopens the input, and an initial held value cannot
fire. Removing the receiving endpoint after the next keyboard press yields an
uncertain result with one accepted message and a cancelled release. Reopening
retains that result and sends nothing until explicit rearm/fresh gestures.

Two further fresh processes prove saved preparation reopens with no selected
ports, live arming, learner, pending request or output. Fresh N/P and controller
gestures work after explicit connection/arm. A rapid pair of CC presses admits
one gesture and visibly refuses the busy request with no deferred send. Clear
Previous MIDI, reassign its key to Q, and reopen again retains exactly those
edits. This contributes ten further received messages: **35 total**, including
one accepted press whose release was cancelled. The final process sends zero.
The packet contains 13 presented captures, exact scenarios, checkpoints and
independent receive/send logs under `native/`.

## Focused checks

| Gate | Distinct passing cases |
| --- | ---: |
| Desktop MIDI dispatcher/worker/callback | 30 |
| Retained keyboard/controller controls | 5 |
| Retained Enter/Space button repeat | 1 |
| Portable MIDI input/history/encoding | 10 |
| Settings round trip | 1 |
| Exact shared-host synthesized-focus module | 1 |
| **Total** | **48** |

Six regressions are new in this run: five callback admission/lifetime/busy cases
and a real retained search-textbox focus case. Repeated runs are not additional
cases. The final Mac app/fixture build, Windows GNU and Linux GNU shared
instrument BLE checks, and portable no-default-features check pass; logs are in
`checks/`. No full workspace test suite was run.

## Repairs found by native execution

1. **Frame-polled input expired during normal rendering.** Correct packets were
   visible in input history, but the old 100 ms cutoff discarded learning and
   mapped actions. One live `ControllerEdges` owner now runs in the existing
   parsed midir callback and submits to the same bounded dispatcher. The UI
   edge owner was removed. Frame history is exclusively preparation/display
   and can never replay an action. The existing worker still owns send deadlines,
   release scheduling and partial outcomes; there is no second writer or clock.
2. **Explicit None was skipped after input loss.** Comparing only the confirmed
   input left its old requested name in place, so selecting it again did not
   reopen. The UI now forwards all input intent through the deduplicating owner,
   including None while disconnected. Source callbacks carry a retiring epoch.
3. **Learning cursor and live authority.** Learning flushes prior history at the
   click rather than dropping the next frame's new press. Arming/configuration
   reaches the callback owner before persistence/rendering. A lost source or
   changed receiver requires disarmed acknowledgement before a new Arm; held
   values remain release-gated. Pending callback refusals survive until the UI
   can read their notice, even if the ordinary worker report advances.

The focused callback checks cover admission without any frame poll, held-value
suppression, disarmed holds, retired source callbacks, receiver replacement,
configuration edits while armed, and busy refusal without deferred replay.
They inject only at the existing dispatcher/output seams and send no native
MIDI themselves. Existing worker fault checks retain the partial-send contract.

## Guitar/physical-key method

`guitar.py` opens only an explicitly selected compatibility BLE output and
loads the UI-authored synthetic preparation from a passed native cohort. The
launcher sends no pedal actions. Its read-only count barriers wait for the
owner's real N/P keyboard gestures and deliberately supplied controller input.
Next/Previous observation, radio OFF/ON, held-input quiet and explicit fresh
rearm are separate checkpoints; no musical action is automatically retried.
The final physical cohort (`guitar-r4`) passes every desktop count/state assertion
and exits successfully with 11 presented captures, **12 attempted/accepted
messages**, both ports closed and inputs disarmed. The key trace records 13 real
N events during the held key and one real P event. No scenario key injection is
used in this cohort. The selected input is a virtual MIDI source, not a physical
controller device. Its four deliberate gestures each contain a release, press
and 32 held-positive repeats, admitted by the product's existing parser/mapping.

| Physical action | Cumulative OS-accepted messages | Owner observation |
| --- | ---: | --- |
| Real held N → Next | 2 | Advanced exactly once |
| Real P → Previous | 4 | Moved back exactly once |
| CC20 input → Next | 6 | Advanced exactly once |
| Note36 input → Previous | 8 | No change; MIDI Pedal reported on |
| Fresh CC20 after reconnect → Next | 10 | Advanced exactly once |
| Fresh Note36 after reconnect → Previous | 12 | Moved back exactly once |

**The initial Note-triggered Previous remains unresolved at the guitar.** Its
press/release was OS-accepted on the subscribed first generation, with no
reported send failure; that is not a guitar acknowledgement. A later disconnect
was recorded, but the evidence does not establish that it caused the earlier
no-change observation. After the owner explicitly returned to Effects and
confirmed readiness, fresh actions on generation 2 both worked. These do not
erase or automatically retry the earlier observation. No device-side, raw BLE
or radio-fault capture is claimed.

MIDI OFF disarms at count eight. MIDI ON produces generation 2, still disarmed
and still count eight; explicit rearm and 32 positive-only values both offline
and after rearm produce no extra gesture. The two fresh release/press actions
then reach exactly ten/twelve messages with ids 5/6. No delayed release or
replayed musical action appears in these desktop counters.

Three earlier setup cohorts remain separate in `attempts.json` and
`diagnostics/`: new-root fixture misuse before product launch; Bluetooth
permission/startup timeout; then a first real N report of no change with an
intervening disconnect/resubscribe and zero attempted messages while disarmed.
The existing macOS scenario launcher now accepts an explicit test bundle
identifier so an owner-granted permission can be retained across reruns. No
Bluetooth permission is changed by the launcher; the owner granted it. The
final cohort enables the existing `CAMBIUM_HOST_KEY_TRACE` environment flag.

See `guitar/owner-observations.json`, `guitar/verification.json`, the exact
executed scenario, checkpoints and native trace for the separate observations.

## Scope and preservation

This is a bounded macOS desktop qualification. Shared instrument target checks,
portable mapping tests and the shared focus gate do not establish full native
Windows/Linux desktop behavior, Windows/X11 focus interaction, encrypted BLE
interoperability, physical radio faults or performance timing guarantees.

The primary checkout already contains unrelated work. Before-state/source
copies, failed cohorts and native profiles remain under
`Code/testing/woodshed/pedal-input-acceptance_20261020/`. Only successful final
cohorts become acceptance evidence; earlier failures retain their own binary,
scenario and diagnostic identity. Native launches are coordinated with the
user-authorized “Find next Eidetic moves” desktop window. No publication,
staging, commit, push, outreach or guitar file write is part of this run.
