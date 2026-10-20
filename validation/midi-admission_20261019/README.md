# MIDI routing and output admission

Date: 2026-10-19. Status: scoped CPU qualification complete at `480c0d6`;
local for review. Baseline: published `fd9fb82`.

This bounded follow-up addresses historical host:H6–H9. Luna workers own port
selection reconciliation and output connection/transport state respectively.
The coordinator owns the backend completion hook, persona reset, host projection,
settings diagnostic delivery and integration evidence. Supplier manifests/locks,
native validation and physical MIDI acceptance remain outside this batch.

The running process owns MIDI inventory, routing and clock settings. A persona
reset retains that state. One view-state helper resolves selected names and
reconciles refreshed inventories; the host no longer duplicates index-to-port
interpretation. Missing names clear selection rather than selecting a different
port at the same position. Names are the existing backend identity; duplicate
physical ports with identical names remain ambiguous.

The host polls worker completion facts before publishing accepted output state.
It forwards desired output every tick so returning to the currently accepted
port can supersede a pending replacement. The backend deduplicates requests.
The settings surface exposes backend diagnostics through role=status.

CPU tests use fake connections and the production state transitions. They do
not enumerate hardware, open native windows, measure clock scheduling or prove
physical MIDI delivery. The [receipt](receipt.json) binds nine source/test files, five unchanged locks,
actual exits and log digests. The frozen audit remains a historical record.

Output ownership is split at the necessary thread boundary: product intent and
completion filtering live in OutputControl; one worker OutputMachine owns the
opened connection and transport. Production channel tests cover superseding a
pending target with an accepted route or disconnect, rejecting stale results,
and reopening a refused target only after the user changes route. A candidate
opens before the old transport is touched; failed replacement leaves it intact.
A successful replacement sends Stop to the old connection and Start to the new
one as appropriate. Start/Stop/tick failures remain diagnostic facts, bounded per
connection; an explicit route change can establish a new connection. Shutdown
attempts Stop before dropping the worker's connection.

The coordinator's review corrected command dispatch to return its acknowledgement,
then replaced three clock-setting messages with one atomic configuration message.
This avoids transient Start/Stop caused by separate enabled/playing updates.
A closed worker acknowledgement channel clears accepted output and reports failure.
The independent Luna reviewer found that dispatch-only polling could leave results
stale until another click. Ordinary host frames now share the same completion
projection and remain live while output needs service, including without an audio
backend. The reviewer verified that bounded correction by source review.

Remaining limits: discovery still uses the existing empty-list fallback on scan
failure, and input-open failure policy is unchanged. Those are separate from
H6–H9's inventory retention, selection identity, output admission and connection
transport corrections. Port names remain the existing identity and cannot
distinguish identically named physical devices. A transport write fault remains
visible and suppresses repeated tick writes on that connection; changing route
can establish a fresh connection. Hardware scheduling and recovery are not CPU
qualification claims.

## Qualification

| Accepted final gate | Passed | Ignored |
|---|---:|---:|
| Host MIDI command, channel, transport and receiving tests | 18 | 0 |
| Host persona regressions | 20 | 0 |
| Public view-state selection integration | 5 | 0 |

There are 42 distinct passing cases: one persona reset case appears in both host
filters and is counted once. No nested child tests ran. All final processes
returned zero at `480c0d6`; source hashes were frozen before and checked after.
The final gate captures and logs are in `checks/`.

The initial receiving fixture used a role selector with a DOM-only matching API,
which requires accessibility projection for role selectors. That real exit-101
run (17 passes, one failure) is retained and excluded from positive accounting.
The fixture now checks the actual role attribute through ordinary DOM matching.
An obsolete queued v2 gate was canceled (real exit -15), and passing development
repeats are excluded. Original recordings and playback clocks are outside the
changed paths. This batch is local; no new native or physical qualification is
claimed.
