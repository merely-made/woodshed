# Capture, export identity and observed availability — October 25, 2026

All three authorized slices are implemented and source-published. Redshank
R05/R09 capture and R08 export identity are qualified at
`951c7822511ba21f2d5e89cacfaf81ea48bd73d8`; Woodshed V03 observed audio
availability is qualified at `770f54435dcefe0a9d4a6a46a367a1684184e8b9`.
The refreshed starting main was `91b2dd5b905b190ad38295cdad8576196cf3970e`.
Incoming contextual navigation and playback work did not require another product
slice or a supplier repin. P5a received no changes.

The [machine receipt](receipt.json), [current ledger](../full-source-audit_20261016/current-finding-ledger.json)
and [P12 plan](../../design_docs/2026-10-16_source_authority_audit_plan.md#p12-capture-preservation-export-identity-and-observed-audio-availability-2026-10-25)
record the source, owners, results and qualification bounds. Publication metadata
is recorded separately after the evidence reaches fetched main.

## Resulting behavior

1. A mounted dock voice gesture retains its release target through the host's
   recording projection. Accepted samples supply elapsed/saved duration. At the
   limit, acquisition stops and the ordinary note-save path preserves the
   accepted prefix with persistent limit wording. Failed blob/model/durable
   admission or close preserves PCM, body, anchor and admitted note identity for
   retry. Durable acknowledgement releases PCM; explicit Cancel discards.
   Playback/duck unwinds once at actual input stop, including failure/cap paths.
2. Annotation-export names combine a bounded readable prefix and full ItemId
   BLAKE3 digest. Different IDs whose readable stems collide produce separate
   actual Shareable JSON. A synced sibling-temp write atomically replaces only
   the same item's export, leaving the other item intact.
3. General, Tools, Settings, Stage and overview use observed output/input/tuner
   facts. Unavailable, unopened, requested, permission wait, refused, admitted
   and offline are distinct. Desired tuner enablement alone cannot produce
   Listening or stale readings. Input faults latch, wake the host, retire failed
   pending/calibration work and require a fresh explicit retry; output operation
   refusal remains separate from stream failure. Persona installation carries
   the existing process observations.

One Redshank owner held both slices' shared desktop main/voice/surface seam. One
Woodshed owner held native health through host/model/view projection. The third
lane reviewed source and images read-only; the coordinator owned Cargo/native
scheduling, evidence, canonical docs/ledger and Git.

## Executed checks

| Final gate | Actual result |
|---|---|
| `redshank-final`: surfaces, desktop, model, playback | 287 distinct passes; eight existing ignores; exit 0 |
| `woodshed-formatted-final`: core, audio, desktop host | 866 distinct parent/integration passes; eight existing ignores; five nested child invocations counted separately; exit 0 |
| `redshank-build`, `woodshed-formatted-build` | Two locked desktop builds; exit 0 |
| `redshank-web` | Locked Wasm target check; exit 0 |
| Native absent/offline presentation | Two successful product processes; eight individually reviewed captures; exit 0; no timeout |

Total: **1,153 distinct passes**, **16 existing ignores**, **five additional
nested invocations**, and **24 new executed test functions**. Earlier partial,
control and superseded runs contribute no extra passing count. Actual command,
exit, source-before/after, timing and log digests are retained under `checks/`.
Gate HEADs precede the two source commits; exact source hashes bind the tested
bytes to the qualified Git snapshots. [Source bindings](source-sha256.json)
also retain unchanged repository/vendor inputs; listing a file does not assert
its code executed. Root and Redshank's independent locked supplier cohorts were
checked separately. No Cargo manifest, lockfile or supplier pin changed.

Capture qualification uses deterministic accepted PCM/fake input, production
storage/export workers, exact reopened note/blob data and full mounted pointer
dispatch. It does not establish microphone, audible playback, a native long hold
or ten-minute capture. Physical audio and second-host receiving remain separate.

## Native observations

The source-bound binary SHA-256 is
`a7f92a7c1a64818c90b55aa870ededef23d159c1e93ba4c1f578c79bc9babcff`.
Both processes ran through the production LaunchServices scenario runner with
isolated roots and an explicit nonexistent custody socket. No Keeper was
launched and no persona-owned files were written. Absent-backend mode has no
input/output backend. Offline mode did not enable the tuner or request real
input. Both scenarios completed with `RESULT ok`; the native accessibility
observer then clicked the visible fixture discard-and-close action. Processes
exited successfully, and temporary app cleanup explains the later no-executable
AX lookup. [Saved close observations](native-close-observations.json) retain
that distinction.

The independent reviewer personally viewed all eight PNGs,
checked their controls/wording and lack of clipping, recomputed the binary hash
and checked both process/scenario receipts. Hash validation below cannot replace
that visual review.

| Phase | Reviewed captures |
|---|---|
| Absent | [General](native/absent/01_general_absent.png), [Tools off](native/absent/02_tools_tuner_off.png), [requested without input](native/absent/03_tools_requested_no_input.png), [Settings](native/absent/04_settings_requested_no_input.png), [close guard](native/absent/05_close_guard.png) |
| Offline | [General and lazy input](native/offline/01_general_offline_lazy_input.png), [Tools off](native/offline/02_tools_offline_tuner_off.png), [close guard](native/offline/03_close_guard.png) |

These observations qualify V03 presentation only. Native fault/retry/permission,
real input/output, Redshank native hold/cap/save, resize/reparent behavior and
cross-computer qualification were not exercised by these two scenarios.

## Preserved failures and corrections

Three new baseline controls reproduce dock release loss, export collision and
the first absent-backend General assertion at incoming main (each Cargo exit
101). The baseline availability command stops at its first assertion; its later
tuner assertions are not baseline execution evidence. Patches retain the
test-only controls applied to that source.

The first Redshank batch failed because the new export worker fixture used
private notes. W3C correctly exported an empty item list. The fixture now uses
Shareable annotations and asserts export eligibility before I/O; the distinct
actual JSON checks remain. The final complete command passed.

The first root batch hit an existing rehearsal scroll test whose wheel point
was stale after incoming contextual navigation changed geometry. A clean
`91b2dd5b` control reproduced it. The corrected fixture selects Rehearsal through
the production route, obtains its wheel location and amount from workspace
geometry, and asserts actual page scroll/board movement. It retains visible
viewport, oversized extent and exactly-one-note-click assertions.

The first corrected-fixture probe accidentally reused the control checkout's
old executable in a shared target: its 353-case inventory and old stack line
identified that mismatch despite stable source hashes. `viewport-fixed` is
explicitly excluded as stale artifact evidence. After pruning the control and
cleaning only four local package outputs, `viewport-fixed-r2` compiled the task
source and passed with the expected 363-case inventory. Final full selected
tests and build then passed, and were repeated after focused formatting. This
is why source hashes alone are not accepted as executable freshness evidence.

The initial frozen native copy also lacked executable mode; the runner refused
it before product launch. Restoring the original mode changed no bytes. That
setup attempt is retained separately and contributes no native process count.
Raw failed/superseded logs and original patches retain their original whitespace.

## Publication, cleanup and host limits

Source commits are merged to local main and pushed through the authenticated
HTTPS route, with stored `origin` unchanged. The primary checkout was clean at
intake, then fast-forwarded before work. The temporary upstream control was
pruned; this completed task checkout/branch is pruned after evidence publication.
Intentional archive refs and normal ignored build caches are preserved.

The other computers were not synchronized: o-pc timed out and s-pc,
thinkpad-l14-f and imac-intel refused SSH authentication while the local agent
had no identities. Their revisions, dirtiness and worktrees are unverified.
No remote checkout was accessed or changed. The [sanitized intake](host-sync-intake.json)
records that limit. Root's newer cohort does not supersede Q5's historical
eleven-case receiving receipt or establish current recipe-to-root alignment.

## Portable evidence verification

From any directory, run `python3 <this-folder>/verify.py`. It uses only Python's
standard library and local Git objects. It verifies qualified Git source,
process/log hashes, final counts, the new-test inventory, native receipt/image
membership and the preserved 97-row ledger scope. Add `--working-tree` to require
the current source inputs to match too. It launches no product, Cargo, device or
remote connection. Absolute trial/build paths in raw records are provenance;
the verifier never follows them. No binary, store root, key or target cache is
bundled here.
