# Shared Recorder SelectOne admission

Date: 2026-10-19. Status: implemented and CPU qualified at `b804d901`; local for
review. Baseline: published `8da953b`, following the authorized seven-commit
push and successful fast-forward-only pull.

B5 addresses shared:S2. One cheaper Luna worker owned recorder production
admission, and a second owned independent public-API integration tests. The
coordinator reviewed host/export boundaries, ran the isolated negative control
and final process gates, and reconciled the canonical audit records.

## One occurrence admission rule

The recorder combines track mute/solo with the existing model
`PlaybackMode::is_layer_audible`. Store-backed starts check that rule before
deferred replay, media lookup, PCM cloning and pending-stop supersession. Capture
promotion checks the same rule before its direct start. The two actual
`play_layer` calls are both gated. Reconciliation, layer/track unmute and imported
replay rely on the store start gate, removing repeated caller decisions and the
old track-only helper. Imports retain their queued PhraseId occurrence check.

Inactive captures and imports remain stored but silent; appending does not
automatically select a variation. Sum behavior, captured sample rate and gain,
Free versus Clocked alignment, and existing failed play/stop bookkeeping remain
with their current owners. No public API, persistence field, queue or scheduler
was added. Model-only waveform queries keep their distinct presentation meaning.

Standalone Hocket uses this Recorder; its desktop adapter honors the existing
start-alignment flag. The export collector already follows the model predicate.
A regression compares admitted voice PCM/gain with the public one-cycle exporter.
Woodshed's embedded surface retains Card/backing authority through the shared
host policy; this change does not replace it with a second Hocket Session.

## Qualification

The [process-bound receipt](receipt.json) binds the two changed source/test files,
four unchanged lockfiles, exact commands, actual exits and log digests. Product
source matches committed `b804d901`; docs/validation were being edited during
checks, so process revision metadata alone is not a clean-tree claim.

| Final gate | Passed | Ignored |
|---|---:|---:|
| Recorder library | 27 | 0 |
| Independent Recorder integration tests | 5 | 0 |
| Portable project library | 38 | 0 |
| Shared recorder views library | 13 | 0 |
| Hocket host CPU tests | 93 | 2 existing |
| Total distinct cases | 176 | 2 |

All final processes returned zero. No nested child invocations are included.
Worker repeats and baseline runs are excluded from the positive total. Tests
cover in-process CBOR ProjectBundle roundtrip admission, SelectOne active-none,
an existing selection and a selected appended occurrence, retained capture and
import media, Sum, Free/Clocked alignment, stored rates/gains, mute/solo, missing
media and reconciliation. The existing library controls cover fallible voice
bookkeeping; this batch does not establish new device-failure behavior.

## Behavioral negative control

The identical final integration test file was run against an isolated extraction
of Recorder `8da953b`, leaving the working checkout untouched. Capture promotion,
import replay and inactive-layer unmute failed as expected; two already-correct
cases passed. The [baseline process](checks/baseline-negative-v2.json) records
exit 101 and the [log](checks/baseline-negative-v2.log) contains all three named
assertion failures. These are expected failures, not passing qualification cases.

The first [setup attempt](checks/baseline-negative.json) stopped before testing:
`--locked` refused the standalone fixture's copied workspace lockfile. It is
excluded from behavioral evidence. The rerun used offline-only pruning of the
temporary lock. All 83 remaining external package source identities match the
published root lock; no network resolution or product-lock change was used.
Receipt hashes bind the extracted source, final test, standalone manifest and
pruned lock. The process source_commit identifies the coordinator checkout; the
separate byte checks establish the extracted baseline code under test.

The reproducible fixture remains at
`/Users/markik/Code/testing/woodshed/select-one-baseline-20261019`.

## Limits and remaining work

No native app, browser lane, resident service, microphone/device, physical audio
or fresh-process native reopen was used or qualified. CBOR roundtrip is
in-process CPU admission only; the deferred Deeler UI is not shipped by this
change. No supplier update, old PCM mutation or playback/capture clock migration
was made. The new B5 commits remain local for review. B6 tempo replay and runtime
diagnostics retain their separate audit owners; broader remediation stays open.
