# Previous input comparison — 2026-10-21

**Bounded software/native/guitar comparison complete: the earlier miss did not
reproduce, and its historical cause is not established.**
Investigate the earlier OS-accepted Note-triggered Previous
whose guitar effect change was not observed. The original acceptance packet
remains frozen; new evidence cannot retroactively explain that event.

Both input routes resolve to Effect Previous, the same configured outgoing
Channel 1 / CC66 values 127 then 0, through the existing dispatcher and worker.
The investigation compares sparse Note input, held Note input and the keyboard
hook. It does not introduce a second writer or a retry path.

## Diagnostics

Set `INSTRUMENT_MIDI_DIAGNOSTICS=1` before app startup. The shared instrument
module owns a process-local bounded journal (512 events), with monotonically
sequenced elapsed microseconds, request ids, receiver revision/generation,
short-message bytes and bounded text. Overwriting/truncation is explicit. The
existing opt-in `--midi-pedal-stdio` Status command returns `diagnostics`; the
snapshot does not drain or change device state. Recording is off by default
and performs no file I/O on input/output callbacks. No dependency was added. Diagnostics have overhead; their timing observations
are descriptive and are not a latency qualification.

Writer event ids correlate input decisions, admission/planned messages and
send start/result/terminal outcome. BLE events expose the encoded notification
packet, generation, OS result and subscription changes. Their sequential place
inside a single-slot writer gesture correlates them with the request; they do
not invent a BLE request id or claim an over-the-air capture/device ACK.

## Qualification boundaries

Software comparison, native virtual-port receiving bytes and owner-observed
guitar outcomes are separate evidence. The guitar trial ran only after
owner readiness, with USB off, the already approved explicit compatibility
output, and visible effect names before/after individual gestures. No guitar
files or configuration are written; uncertain/unobserved actions are never
retried automatically. Native Windows/Linux, encrypted interoperability and
performance timing are outside this bounded comparison.

## Current results

38 focused cases pass: 31 desktop MIDI dispatcher/worker/controller cases,
five retained input cases with diagnostics enabled, and two journal-bound
cases. Three cases are new: journal sequence/bounds, explicit UTF-8-safe
truncation, and Note/keyboard-binding parity through the actual worker step.
Mac app/fixtures and shared Windows/Linux BLE instrument checks pass. A first
version of the new comparison test omitted its optional release fixture;
setting release=0 corrected the test, with no change to production scheduling.

The new native cohort (`native-r1`) alternates the production keyboard hook P
and selected-input Note36 twice, using the existing desktop controls, synthetic
keeper/profile and receiving fixture. It records exactly eight independently
received messages: four `[176,66,127]`, `[176,66,0]` pairs. Writer trace bytes and
request ids match every receiving packet; each route admits two gestures. The
four observed writer press/release gaps are 50,510 / 50,321 / 51,100 / 50,679
microseconds. They describe this traced run and do not qualify performance.
There are 107 journal events, zero overwrites, and three presented captures.
The status snapshot is taken before closing; the final native checkpoint proves
both connections closed and inputs disarmed. Scenario P injection enters the
production hook, not OS keyboard delivery; the separate physical trial uses real P.

No Note-specific MIDI-byte or release-path difference reproduced in software or
this native cohort. The original 134-file evidence manifest is still unchanged;
the new packet does not relabel its negative observation.

## Guitar comparison

The final physical cohort (`guitar-r2`) uses the same saved bindings and desktop
writer, with live read-only Status queries before each deliberate controller
press. The owner starts at Off and observes each Next anchor change to Rev, then
each Previous change back to Off. A real keyboard P provides the control;
the two Note inputs come from the existing virtual input fixture, not a physical
external controller. Incoming channel-3 Note36 is interpreted by the mapping;
the guitar receives channel-1 CC66 in all three Previous cases.

| Previous source | Request | Writer bytes (decimal) | Press/release gap | Owner observation |
| --- | ---: | --- | ---: | --- |
| Real keyboard P | 2 | 176,66,127 then 176,66,0 | 50.428 ms | Rev → Off |
| Sparse Note release/press | 4 | 176,66,127 then 176,66,0 | 50.491 ms | Rev → Off |
| Note release/press + 32 held repeats | 6 | 176,66,127 then 176,66,0 | 50.526 ms | Rev → Off |

The BLE notification buffers match each writer MIDI message, with the expected
packet timestamps differing by 50 ms in each pair. All submission results are
successful on generation 1; there are zero disconnects. These are the buffers
submitted through the existing supplier, not an independent radio capture.
Owner observations establish the visible guitar changes separately.

All six deliberate gestures (three Next anchors plus three Previous) result in
12 attempted/OS-accepted messages and six observed effect changes. The final
snapshot has 124 events, no overwrites and no truncation. All 42 raw fixture
input packets have matching parsed controller events; five controller decisions
and one real keyboard decision account for all six writer gestures. Five
presented captures and successful process completion are retained. The final
checkpoint shows both ports closed, inputs disarmed and count twelve.

The first setup attempt (`guitar-r1`) timed out during a fresh Bluetooth consent
prompt for the rebuilt temporary test bundle. Owner consent was confirmed, but
the live Status still showed its output closed with zero attempts; restarting
that same binary opened the output normally. No effect gesture was retried.
Permission/setup evidence lives under `diagnostics/`, separate from the passing
cohort. The owner corrected an ambiguous "none" observation to displayed Off in
the sparse Note case; the verbatim replies and clarification are retained.

## Conclusion and re-entry cue

The comparison found no Note-specific mapping, outgoing-byte, release-scheduler
or held-repeat defect. The earlier event does not have this per-message trace
or an independent radio capture, so these results cannot identify its exact
cause. The original no-change observation remains unresolved; no speculative
protocol fix or retry was added.

For another observed miss, the useful evidence is the immediate existing Status
snapshot with diagnostics enabled: input event/arming and decision id, planned
bytes, writer send start/result, subscription generation, BLE notification
buffers/results, plus the effect names before and after. `observe.py` only
queries that existing Status command and saves its response; it sends no MIDI
trigger and creates no second connection. A `truncated` event or nonzero
`overwritten` count explicitly limits conclusions from the history.
