# Recorder tempo replay and runtime diagnostics

Date: 2026-10-19. Status: scoped CPU qualification complete at `7b161a9`; local
for review. Baseline: published `4fab895`. The full shared-view unit target is
blocked by an existing paint API type mismatch, recorded separately below.

This requested B6 runtime slice addresses shared:S3 and shared:S4. It does not
close the historical B6 MIDI H6–H9 findings. Three cheaper Luna workers owned
production tempo admission, independent tempo tests, and wrapper diagnostics.
The coordinator owned selective publication/pull, isolation, source review,
engine and standalone receiving tests, integration and final evidence.

## Runtime and ownership

Tempo admission now precedes any history or voice change. A refused clock keeps
the previous BPM, history and accepted playback. After admission, the Recorder
requeues same-key, bar-aligned replacements only for already accepted voices
that are not pending stops. It uses the existing engine queue and shared B5
store-start helper, preserving model eligibility, PCM, rate and gain. StopAll
or a pending failed stop is not superseded by a tempo edit. No separate desired
voice set, replay queue, scheduler or clock was added.

The old voice remains accepted when a same-key replacement is refused. This
requirement is explicit on RecorderAudio; the shipped adapter's keyed engine
path was traced. The production graph offline test verifies that the current
voice keeps rendering until the new-grid boundary, only one voice slot is used,
and repeated same-key queues select the latest replacement at its stored rate
and gain. No time stretching or historical media/clock migration is performed.

One status reconstruction helper serves healthy recovery and accepted tempo
retry. A new replacement failure overrides that healthy label, and pending-stop
failures remain visible. Poll errors enter the existing availability error fact
so a subsequent healthy service can clear them. No second diagnostic store or
view-specific error model was added.

The reusable wrapper forwards Recorder status into the existing shared renderer
and role=status. Hocket's duplicate accessory label, dead forwarding accessor
and obsolete CSS were removed. Its CPU receiving test finds one shared status
node. Public-wrapper tests drive real Recorder service refusals and recovery
rather than manually populated projections. Custom host projection APIs remain
with their owning hosts.

## Publication and isolation

The primary checkout had unrelated local history and remote main had advanced.
Only the four authorized B5 commits were applied to current remote main in this
isolated worktree. Source/test bytes match the reviewed originals. Incoming
DOC_README entries were preserved and the merged coordination hash refreshed.
The ordinary push reached `4fab895`, followed by a successful fast-forward-only
pull. See the [B5 publication record](../recorder-select-one_20261019/publication.json).
The original B5 receipt retains its original supplier qualification scope.

New work lives on `codex/woodshed-b6-20261019` in
`/Users/markik/Code/worktrees/woodshed-b6-20261019`. This batch made no primary
checkout source change or supplier/manifest/lock edit. Main advanced independently
during the checks; qualification is tied to this recorded baseline and source.

## Qualification

The [receipt](receipt.json) binds all nine changed source/test files, five
unchanged locks, real process exits and log digests. Docs/validation were being
updated during tests; process revision metadata alone is not a clean-tree claim.

| Accepted scoped gate | Passed | Ignored |
|---|---:|---:|
| Recorder unit | 28 | 0 |
| SelectOne integration | 5 | 0 |
| Independent tempo integration | 5 | 0 |
| Public-wrapper diagnostics integration | 2 | 0 |
| Hocket production graph offline engine | 37 | 0 |
| Hocket CPU host | 94 | 2 existing |
| Total distinct cases | 171 | 2 |

All accepted processes returned zero. There are no nested child invocations.
Worker repeats, development runs and negative controls are excluded. Recorder,
wrapper and engine gates use `f32212d`. The sole later source change corrects
the receiving test's unsupported selector; the full CPU host gate passes at
`7b161a9`. Product code, manifests and locks are identical across those gates.

The identical five tempo tests run against isolated published Recorder source
`4fab895` yield three expected failures and two passes. They reproduce playback
loss on accepted tempo change, loss before clock refusal, and replacement of a
specific failed-stop message by a generic one. The [control process](checks/tempo-baseline.json)
records exit 101 and the [log](checks/tempo-baseline.log) retains the assertion
failures. The temporary standalone lock was pruned offline; all 83 remaining
external source identities match the baseline root lock. Separate byte checks
establish the extracted code and identical test file, since process source_commit
identifies the coordinator checkout rather than the extracted code.

## Blocked and excluded gates

The [full shared-view unit attempt](checks/views-unit-blocked.json) returned 101
before tests ran. Unchanged leaves.rs tests mix the locked renderer's PaintCmd
from netrender `d920501` with the direct dev dependency at `e8cda06`, producing
four E0308 errors. No supplier or lock repair was attempted. The public API
diagnostics integration target compiles the library normally and passes; it
does not qualify the blocked paint unit tests or the full workspace.

The [first CPU host attempt](checks/hocket-host.json) failed to compile because
the coordinator regression used unsupported Selector::any. The corrected test
uses Selector::class and is covered by the accepted host-v2 gate. The failed
attempt is retained and excluded from the positive count.

## Limits

No native app, browser lane, resident service, microphone/device, physical
audio or fresh-process native reopen was used or qualified. CPU DOM/layout
harnesses and offline Firewheel execution do not establish device behavior.
Existing command-failure semantics beyond the tested tempo/poll recovery cases
are not broadly requalified. Historical MIDI findings and supplier intake remain
with their separate owners. New B6 implementation stays local for review.
