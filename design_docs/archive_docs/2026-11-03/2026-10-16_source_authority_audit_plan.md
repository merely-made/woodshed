# Woodshed source authority audit and remediation

Date: 2026-10-16; coordination closure: 2026-10-17; updated 2026-10-25.
Status: complete collective read of frozen `d12201bf`; current-source finding
reconciliation complete and published through `8204688`. Remediation remains open;
H3/H5 and Q1 process/link implementation is CPU qualified and published through `6e6498a`.
B1–B3 runtime authority corrections are CPU qualified and published through `b1c9dfd`. B4 PCM/onset/calibration corrections are CPU qualified at `5dd01ae` and published through `8da953b`. B5 shared Recorder admission is CPU qualified at `b804d901` and published through `4fab895`. The requested B6 Recorder runtime slice is CPU qualified at `7b161a9`, published through `fd9fb82`; its original shared-view paint gate limit stays historical. D1 has bounded CPU/macOS/native/reopen qualification at `8bf59b31`. P11 rechecks main `3ce93675` and CPU/build qualifies B8 playback at `046b0bf4` with 167 distinct passes. P12 implements R05/R09 capture and R08 export at `951c7822`, plus V03 observed availability at `770f5443`: 1,153 distinct CPU passes, two desktop builds, Wasm and bounded native availability presentation (two processes/eight reviewed captures). Remaining work is ordered below. The frozen
audit closure itself adds no product fixes or product qualification.

## Scope and result

The full read-through covered 936 assigned files / 231,127 lines plus one
otherwise omitted 63-line W3C fixture: **937 files / 231,190 unique lines**.
Program source and tests account for 303 files / 149,331 lines. Configuration,
scenarios, lockfiles, fixtures and receipt metadata account for the rest.
Receipt JSON received complete structural review; binary media, fonts, every
historical screenshot/log and third-party implementation code were not an
exhaustive reading scope. This was a collective read, not a claim that the
coordinator personally read every partition.

The separate frozen P5a overlay has 33 files / 34,861 semantic lines, including
all its unchanged ranges. It overlaps the published baseline and is not added
to that unique total. Read coverage is source evidence, not passing tests or
native acceptance.

The main architectural finding is competing decisions around accepted facts:
projections infer execution, lifecycle paths bypass preservation, compatibility
adapters discard identity, and verification helpers infer success from incomplete
observations. Existing model, product-runtime and shared-stack owners should
remain; consolidate their bypasses rather than introduce a universal framework.

The [frozen synthesis](../validation/full-source-audit_20261016/AUDIT.md), all
eleven partition reports, exact coverage ledgers and original receipts are
archived in the [audit record](../validation/full-source-audit_20261016/README.md).
The [original package catalog and complete finding crosswalk](../validation/full-source-audit_20261016/REMEDIATION_PLAN.md#complete-finding-crosswalk)
retain scopes and aliases as dated planning evidence. This document and the
current reconciliation ledger own the present frontier; the archived proposals
do not supersede later rulings or qualify implementation.

## Phases and done-conditions

### P1 Complete and verify the frozen collective read

Done: all eleven assignment sets and ranges cover the exact frozen inputs;
937 unique file hashes and line counts match Git blobs at `d12201bf`. The
33-file overlay's exact external source and semantic range ledger also match.
The verifier distinguishes partial parent corroboration from additional unique
coverage, refuses optimized Python, and has negative range controls. Its
execution receipt records the actual process exit.

### P2 Reconcile existing fixes before proposing more implementation

Current read-only comparison uses local source `a424772`. Of the original 936
assigned paths, 841 remain byte-identical and 95 changed; none is missing. These
hashes are metadata, not a full semantic audit of all changed/new current files.
The current ledger uses targeted caller/source and existing-receipt review.
Every historical receipt retains its original source, suppliers and limits.
There is no new current-head native, port-native, physical-device or audio claim.

Done when the raw finding index has an explicit disposition or unassessed state
for every entry, aliases are preserved, existing safety work is credited with
its actual receipt scope, and open work has an owner and acceptance boundary.

### P3 Close coordination and keep one actionable frontier

Done when the synthesis, coverage and current reconciliation are indexed in
DOC_README, exact source/receipt identities are retained, no product code has
been changed, and publication status is stated separately. This closes the
source-audit delivery; it does not mark the remaining fixes complete.

### P4 Repair Hocket admission and operational evidence gates

Status (2026-10-18): implemented and CPU qualified at `0effba1`; published through `6e6498a`. Three
lower-cost parallel workers have disjoint ownership: Hocket host identity/media
admission (H3/H5), CI/font process evidence (E1), and revision-scoped target
resolution in the link checker (E2). The coordinator owns integration and custody
review. No worker owns native validation or supplier/lock updates.

Done when staged identity consent binds the actual accepted profile/key and
preserves the current identity on rejection; successful same-session import
refreshes missing-media facts; process gates reject failed, incomplete or
unbound executions; historical link targets resolve from the historical tree;
and meaningful regression controls pass with observed process exits. Native
custody/reopen and physical qualification remain separate. The [batch receipt](../validation/admission-evidence_20261017/README.md) records
93 Hocket, 14 process and seven link controls (114 distinct passes), two existing
ignores, real exits and retained excluded attempts. New implementation
commits were published on 2026-10-18 under explicit user authorization.

### P5 Reduce competing runtime/material decisions (B1–B3)

Status (2026-10-18): implemented and CPU qualified at `662cda5`; published through `b1c9dfd`.
Baseline: published `6e6498a`. Three Luna workers
have disjoint code ownership: core retained-setup/selection realization (B1),
SongBar payload-preserving silence (B2), and processor future-bar capture arms
(B3). The coordinator owns cross-consumer integration, authority review and the
receipt. No worker owns native validation, device operations, supplier updates,
or capture clock migration.

Prefer one resolution of retained instrument/setup and selected contacts, one
lossless authored song payload with explicit audibility, and the existing bounded
processor command authority. Do not add parallel schedulers, material caches,
backup chord copies or a universal framework. Compatibility must preserve old
SongDoc content, existing takes and PCM. Done when meaningful CPU regressions
prove retained/inherited setup and repeated Path order, no full-chord fallback
for stale Solo, exact silence/serialization preservation, future-bar arm retention
and cancellation/termination, with relevant no-allocation/parity checks. Native
qualification remains separate. The [runtime receipt](../validation/runtime-authority_20261018/README.md)
records 859 distinct parent/library passes, five separate child invocations and
seven existing ignores, with real process exits and excluded draft runs. Path
neck geometry shares retained setup and the existing empty-geometry contract;
no separate scheduler, backup chord store or cleanup path was retained. The nine reviewed commits were published under explicit user authorization.

### P6 Unify bounded PCM and timing evidence admission (B4)

Status (2026-10-18): implemented and CPU qualified at `5dd01ae`; published through `8da953b`. Baseline: published `b1c9dfd`. Three Luna workers own disjoint PCM preparation/export, onset accounting, and calibration pairing/session files. The coordinator owns integration, independent source review and evidence.

Done when imported and rendered nonfinite PCM is refused before resource publication while accepted backing survives rejection; onset clocks count each incoming sample once independent of chunk boundaries; calibration estimate and displayed support derive from one admitted pairing result. Preserve existing take PCM and clocks. No supplier/lock updates or native/device lane operations. The seven reviewed commits were published on 2026-10-19. The [process-bound receipt](../validation/pcm-timing_20261018/README.md) records 308 distinct library passes (61 primitives, 211 Woodshed audio, 36 Hocket engine), no ignores or nested child invocations, 11 exact source hashes and four unchanged locks.

### P7 Admit shared Recorder voices from one occurrence rule (B5)

Status (2026-10-19): implemented and CPU qualified at `b804d901`; published through `4fab895`. Baseline: published `8da953b`. A Luna worker owns
shared Recorder production admission; an independent Luna worker owns black-box
regressions in a separate integration test file. The coordinator owns host/export
boundary review and evidence. No worker owns native validation or supplier pins.

Done when capture promotion, import replay, layer unmute and reconciliation all
admit voice starts through one occurrence rule using the model's existing
`PlaybackMode::is_layer_audible`, plus track mute/solo. Retain inactive captures
and imports without silently selecting them; preserve Sum behavior, free/clocked
start alignment, media, stored sample rates and failure bookkeeping. Supported
serialized SelectOne sessions suffice; this does not ship the deferred Deeler UI.
The [receipt](../validation/recorder-select-one_20261019/README.md) records 176
distinct final cases and two existing ignores. The same five integration tests
against isolated published Recorder source give three expected bypass failures
and two passes; the setup lock refusal is excluded. Both source/test files and
four unchanged locks are hash bound. New implementation remains local for review;
native/device checks stay separate.

### P8 Preserve tempo playback and surface Recorder diagnostics (requested B6 slice)

Status (2026-10-19): scoped CPU qualification complete at `7b161a9`; published through `fd9fb82`. Baseline: published `4fab895`, integrated on remote `be4d6d0`. Work is isolated in `codex/woodshed-b6-20261019`; the primary checkout
and supplier/native owners remain untouched. Three Luna workers own production
Recorder tempo admission, independent tempo tests, and shared wrapper diagnostic
projection/DOM tests. The coordinator owns the engine offline contract test,
integration and evidence.

Done when clock refusal leaves previous tempo/history/voices untouched, accepted
tempo changes requeue only previously accepted voices on the new grid without
stopping them first, and a failed replacement preserves the prior accepted key
with a visible error. Explicit pending stop intent must not be superseded by a
tempo edit. Reuse B5 admission and existing keyed engine replacement; no second
desired-voice store. Wrapper status must come from Recorder facts and render via
the existing shared surface. No supplier/lock edits or native/device operations.
This requested slice targets shared:S3/S4; historically named B6 MIDI H6–H9
remain separate. The [receipt](../validation/recorder-runtime_20261019/README.md)
records 171 distinct passes, two existing ignores, three expected baseline
failures and actual process bindings. Nine source/test files and five unchanged
locks are hash bound. The full shared-view unit target remains blocked by the
existing paint API type mismatch; its new public-wrapper integration target
passes independently. New work stays published through `fd9fb82`.

### P9 — MIDI admission and connection ownership (2026-10-19)

Status: scoped CPU qualification complete at `480c0d6`; published through `20bcc1c`.
[Receipt](../validation/midi-admission_20261019/README.md): 42 distinct passes,
zero ignores, with failed/canceled attempts retained separately. Ordinary host
frames service pending completion; atomic clock configuration avoids transient
transport changes. Historical source observations remain preserved in the ledger.

This bounded follow-up addresses historical host H6–H9. One Luna worker owns
MidiUiState discovery reconciliation and pure selection tests; a second owns
the output worker and fake-connection tests. The coordinator owns persona reset,
backend completion polling, host projection and diagnostics, review and receipts.
Port names remain the backend identity; rescanning must retain the selected name
or clear it when absent. Persona changes retain process routing and inventory.
Output status must describe the worker's accepted connection, with open failures
visible and stale completions unable to overwrite newer requests. Replacement
must stop the old running connection and start the new one.

Reuse the existing backend seam, one refresh helper and one output worker state
machine; do not add another scheduler or desired-state framework. Qualify with
CPU fakes through production transitions. Supplier intake, device enumeration,
native validation and physical MIDI timing remain outside this batch. New work
stays published through `20bcc1c`. The preceding nine B6 commits were integrated and pushed
through `fd9fb82`; original qualification source identities remain unchanged.

### P10 Recheck A1–A5 and Q1 against current source (2026-10-20)

The user authorized parallel rechecks and concrete corrections, local commits
only, with P5a retained by its existing owner. The isolated branch starts at
fetched `origin/main` `9f80bb5eabc4b513e057b22af6f80d83ecb8e9ac`; the primary
checkout and unrelated work remain in place.

Shared seams have one owner: A1 owns Woodshed lifecycle/preservation; one Hocket
owner combines A2/A3 and Hocket's A4 model/project/custody admission; A5 owns
Redshank storage/desktop recovery; the coordinator owns Woodshed A4 history/Set
admission, Q1 diagnostics/evidence, integration, documentation and Git. Agents
do not run Cargo/native or commit independently. P5a timing, audio scheduling
and native acceptance remain with the existing owner.

Source rechecks find the original first-wave preservation/admission fixes already
implemented. A1 still accepted a seal for a different persona if the family
selection changed during incoming open. The live-switch guard now refuses that
mismatch before replacing outgoing state, retaining the target for explicit
Retry; the regression covers saved instructions, the audio epoch, export state
and both clock bindings. Same-persona behavior is unchanged.
The locked Stemma supplier is now Eidetic `738ef159`, whose
`from_snapshot` remains trusted/infallible; Woodshed's consumer guard remains
necessary. Q1's process/link fixes are present. Runtime `audio_ddx` modes
previously reported device events but returned success. All diagnostic
modes now require output progress and zero device events, using the same strict
terminal result; device-free negative tests cover refusal. Product recovery
policy is unchanged.

Done when owning lanes finish production-caller rechecks, focused current-source
CPU gates record real process exits and source/lock identity, the combined diff
is reviewed, and source/receipts are committed locally. Native runs, physical
audio, fresh-process custody/reopen and unresolved design rulings retain their
own acceptance boundaries. Selecting the same persona and future capture
replacement/recovery interactions remain design choices, not new fixes here.

Current gate: **1,020 distinct passing parent/library/control cases**, nine
existing ignores, and five child invocations recorded separately. All nine
counted processes exited zero. A1 and Q1 corrections are local at `22c9900` and
`6a65a4d`; no push. The [current-source receipt](../validation/first-wave-recheck_20261020/README.md)
retains source/lock hashes, original process evidence, ownership and qualification
limits. Existing native evidence is not transferred to this revision.

2026-10-21 delivery: the user authorized publication and scoped cleanup. Remote
`main` remained at `9f80bb5e`; the qualified three-commit branch fast-forwarded
it to `cb4ed31`. Fifty repeated focused A1/Q1 checks pass and all original
source/evidence hashes match. A fresh remote clone verifies the published source
and retained receipt bytes. The [publication record](../validation/first-wave-recheck_20261020/publication.json)
keeps actual process exits and the narrow cleanup scope. Native/custody/device
acceptance, P5a and unresolved design decisions retain their existing boundaries.

### P11 Recheck the remaining frontier and repair playback admission/controls (2026-10-23)

**Status: implemented and scoped CPU/build qualified at `046b0bf4`.** The
[B8 receipt](../validation/audit-playback_20261023/README.md) records 167 distinct
passing cases, eight existing ignores and the locked desktop build. Primary was clean and
four commits behind; fetch/pull brought it to `3ce93675179098b7d1c81da20cb4f63caa75e6b5`,
equal to origin/main. The isolated `codex/woodshed-playback-controls-20261023`
branch starts from that revision. The maintainer authorizes commit, push, merge
and scoped pruning, and allows this coordinator to take over residual P5a work
if its old reservation is stale.

The historical P5a owner was the link-model coordinator after the October 16
handoff. Its bounded timing/admission work is already implemented and qualified
in the [Stage plan](2026-07-11_stage_set_tools_plan.md#p5a-admit-one-resolved-card-timing-contract-2026-10-16).
That completed assignment is retired as an active reservation. LO-4 generic
Path/Walk Hear parity remains a separate residual; this batch does not reopen
completed managed timing or claim new P5a execution qualification.

Targeted source triage, rather than a new full-source audit, rechecked the open
safety, runtime, port and evidence frontiers. No scoped A1–A5/D1 preservation or
identity P1 bypass was established. D1's host:H2 singleton observation is stale
and is superseded by its [qualified receipt](../validation/persona-store_20261022/README.md).
B7's remaining V03 falsely infers stream/listening availability; shared:S4 is
already implemented and CPU qualified. Redshank R04's native idle predicate now
includes alignment and export jobs; idle native completion is still separately
unqualified. Frozen audit reports remain historical inputs, not the live backlog.

**Selected slice: B8 Redshank R02/R03/R06/R07/R17.** Ordinary cached-integrity
refusal can panic through an outstanding backend borrow; recovery then exposes
competing effective-control copies. This reachable panic/control path takes
priority over the remaining availability caption. It also admits exact-end EOF
without requiring a nonexistent first frame. R05/R09 capture gesture/capacity,
R08 export identity, B7, and the remaining C/Q/K work stay separate.

One owner owns the entire `ports/redshank/playback` seam, including its regression
fixtures. The coordinator alone owns Cargo/build/native scheduling, integration,
ledger/docs and Git. Two other lanes independently reconcile and review source;
they do not edit the shared playback seam or update suppliers.

Done-conditions:

1. Cached-integrity refusal keeps its diagnostic without panic and accepts a
   subsequent valid load; no retained file is rewritten by refusal.
2. Listener volume/duck has one shared output owner before device creation,
   including a voice preview opened without an episode. Preview unity-rate policy
   stays distinct from listener rate.
3. Accepted rate/volume/duck configuration survives worker recreation and is
   applied before resumed output; snapshots describe effective backend/controller
   values. Rate commands pass through the existing controller admission.
4. Exact-duration seek/reopen reaches a coherent terminal state without creating
   output solely to finish; normal decoding and paused behavior remain supported.
5. Regressions execute real worker/controller/backend paths with actual process
   exits and source bindings. Run focused playback/model/session receiving gates,
   the affected desktop build, independent review, then commit/integrate/push,
   verify the fetched source/evidence and prune this completed checkout/branch.
   CPU/decoded-source evidence does not establish physical sound or a new native
   acceptance claim.

### P12 Capture preservation, export identity and observed audio availability (2026-10-25)

**Status: implemented, source merged/pushed and scoped qualification complete.**
Redshank capture/export source is `951c7822`; Woodshed availability source is
`770f5443`. The [receipt](../validation/capture-export-availability_20261025/README.md)
records 1,153 distinct passes, 16 existing ignores, five nested invocations counted
separately, 24 new executed tests, two locked desktop builds and the Redshank
Wasm check. Two native Woodshed processes and eight individually reviewed
captures qualify absent/offline availability presentation only. No real input,
physical audio, Redshank native capture or second-host acceptance is inferred.

The maintainer
approved all three next slices and commit/push/merge/prune, with refreshed
upstream and cross-computer alignment where access permits. Local M4 main was
clean at `da66d6c0`; HTTPS fetch and fast-forward brought it to `91b2dd5b`.
SSH GitHub authentication failed, so publication uses the existing authenticated
HTTPS route without changing stored remote configuration. Local Woodshed had
no uncommitted work or stray registered worktrees at intake. Retained archive
refs are intentional recovery material, not disposable stray checkouts.

Incoming work adds contextual navigation, playback-clock behavior and a newer
root supplier graph. Targeted source review finds no adjacent P1 regression or
reason to widen this batch: R05/R09, R08 and V03 were present at `91b2dd5b`
intake and are now implemented as recorded above. Root and
Redshank have deliberately independent locked supplier cohorts; run each actual
consumer lane and retain historical qualification on its original source.
Do not turn source-cohort differences alone into an automatic repin project.

| Slice | Sole code owner / shared seams | Done-condition |
|---|---|---|
| **1 R05/R09 capture** | Redshank owner: desktop main/voice and dock/rail/capture surfaces/tests | Full dock press → host recording projection → release retains its gesture owner. Accepted sample count supplies duration. Reaching capacity stops capture and finishes through the ordinary note-save path with clear limit wording. Failed blob/admission/durable-save and close attempts retain the frozen target and captured prefix for retry; explicit Cancel alone discards. Fake-feed and mounted-host regressions exercise capacity, failures and exact retained data without using a microphone. |
| **2 R08 export identity** | Same Redshank owner owns desktop main and export helper/tests | Different complete ItemIds with the same readable sanitized stem export independently using bounded names plus a complete-identity digest. Replacement of one item's own export is atomic; actual bytes and export worker outcomes are checked. |
| **3 V03 availability** | Woodshed owner: core neutral facts, host audio/input/sync/drive/main/persona, shared view wording and mounted tests | General/Tools/Settings/Stage consume observed output/input/tuner facts. Absent backend, unopened input, request/permission wait/refusal and admitted listening remain distinct. Microphone refusal does not hide usable output. Retained process observations survive persona installation; desired enabled alone never means listening. Existing explicit retry edges remain, without frame-by-frame reopening. |

The Redshank owner owns both slices' shared `desktop/main.rs`; no second writer
integrates that seam. The Woodshed owner owns its separate host/model/view seam.
A third lane reviews upstream and both implementations read-only. The coordinator
alone owns Cargo/native scheduling, receipts, canonical docs/ledger and Git.

Focused tests are added before production changes where the seam permits.
Initial device-free baseline runs reproduce the full-dock release loss and
export collision (each actual Cargo exit101, stable source). The root V03
baseline also reproduced its absent-backend General assertion failure on the new
supplier graph (actual Cargo101, stable source). At baseline the capacity/retention
seam required a concrete input Stream; the implemented bounded host seam now
exercises deterministic accepted PCM, failure/retry and exact retained data.
Those controls remain distinct from a microphone claim.

Remote host intake was read-only: the SSH agent had no identities, three hosts
refused authentication, and o-pc timed out. The maintainer was asked to unlock
the normal SSH identity while local work continues. Do not reset, stash or
bulk-merge remote checkouts from stale inventory. Refresh and fast-forward only
clean current branches after access is available; preserve/report divergent or
dirty work. Scope cleanup to proven completed Woodshed checkouts and this batch.

## Current findings and next implementation order

**Refreshed 2026-10-25 at fetched/pulled `91b2dd5b`; selected corrections
qualified through `770f5443`.** This is targeted source reconciliation of the
remaining frontier, not a new exhaustive audit or a transfer of historical
qualification to today's suppliers. The [P11 recheck](../validation/audit-playback_20261023/recheck.json)
and [P12 receipt](../validation/capture-export-availability_20261025/receipt.json)
retain their respective source boundaries. The mutable [per-finding ledger](../validation/full-source-audit_20261016/current-finding-ledger.json)
retains all 97 named rows (93 original raw rows, O1 and LO-4/LO-5/LO-K1).

1. **B11 Hocket staged Apply/feed and handoff is the next product slice to
   recheck.** The last targeted source review found staged Apply re-resolving
   its feed and media-proportional handoff preparation on the UI thread. Recheck
   those concrete receiving paths against then-current main before changing
   them; preserve admitted model/media/custody and keep one owner per shared
   seam. This priority is a remaining source-supported defect queue, not a new
   native or physical qualification claim.
2. **B8 and the approved capture/export/availability slices are implemented.**
   P11 qualifies cache refusal, worker controls/recovery and exact-end completion
   at `046b0bf4`. P12 qualifies R05/R09 capture preservation and R08 complete
   export identity at `951c7822`, plus V03 observed availability at `770f5443`.
   Capture uses deterministic PCM and mounted dispatch with actual storage/export
   workers; its native microphone/hold/cap lane remains open. V03's native lane
   covers absent/offline presentation only. R04's scheduling predicate includes
   alignment/export work; actual idle native completion is not newly qualified.
   Shared Recorder diagnostic S4 already has its own implemented CPU receipt.
3. **Preservation/identity and bounded P5a retain their existing results.** No
   scoped A1–A5/D1 P1 mechanism was re-established. D1's `8bf59b31` receipt
   retains bounded CPU/macOS/native/reopen qualification. H3/H5 and malformed
   model admission retain scoped CPU qualification; real-custody/native Hocket
   receiving stays separate. P5a's completed reservation is retired; this batch
   changes none of its code. Generic Path/Walk Hear parity (LO-4) remains distinct
   and LO-5 is the resolved B1 alias. Q1 and B1–B6 keep their dated receipts.
4. **B12–B14 and C1–C5 remain component-scoped follow-ups.** Paint/native-text,
   navigation/motion/visibility, musical reading and catalog findings need their
   own current-source slices. The old private instrument Connection/Session and
   example now live in optional Ringdown research; live public/optional protocol
   findings stay with that owner. This is not a current Woodshed GUI RPC path or
   authorization for additional hardware operations.
5. **Q2–Q5 and K1–K3 follow reachable product defects.** Scenario targets,
   release binary/source/architecture binding, scorer admission and bounded
   diagnostics retain their evidence/maintenance work. Q5 preserves the October
   22 eleven-case/one-ignore receiving result at `cbda7166` on its old
   Mere/Genet cohort. Incoming root main advanced independently; historical
   recipe-to-root alignment wording is superseded, with no automatic repin or
   new cross-cohort qualification. Unreleased status removes unnecessary legacy
   migration obligations while every receipt still needs actual execution.

The three approved slices remain the complete scope of this batch. Additional
input-health/retry receiving corrections belong within V03; the incoming
navigation geometry required a test-fixture correction, not a fourth product
feature. Local main and GitHub are aligned after publication. Other computers
remain unsynchronized/unverified until SSH access permits fresh source and
working-tree checks; no remote tree was changed from stale inventory.

## Existing rulings and authority boundaries

After save failure, keep the window open with Retry and an explicit
Discard-and-close path; Cancel retains work. Accepted preservation behavior is
not reopened as an unanswered product-policy question.

Existing takes retain their original playback clocks and PCM. New capture clock
adoption does not authorize rewriting old media. Saved-song compatibility stays
until its representation is lossless. These earlier user instructions constrain
all remaining cleanup.

Keep one Woodshed output runtime; Hocket's polling schedule and Woodshed's
sample-driven schedule remain deliberately distinct. Shared structural APIs
return facts; product owners decide musical interpretation, durable admission,
custody and user-facing wording. Neither synthetic PCM nor a correct control
request proves microphone, audibility, device timing or round-trip latency.

## Progress

2026-10-16: eleven frozen partitions and parent corroboration completed;
recommendations and isolated evidence-helper counterexamples recorded externally.

2026-10-17: P5a's full 33-file semantic continuation and Ringdown/instrument
reconciliation completed. Preservation/input implementation and later timed
practice/audio qualification retain their separate owning receipts.

2026-10-17 coordination closure: frozen Git/range/hash verification passes;
safety/runtime/port/evidence reconciliation is archived with its current input
revision. Audit documentation and verifier only; no product fixes, new native
runs, physical checks or publication added by this closure. The label-layout
implementation remains a separate preceding local change.

2026-10-17 implementation restart: user approved the next H3/H5 and Q1 fixes with cheaper parallel workers. Historical reports and source-bound qualification inputs remain unchanged; this batch will record its own source and executable checks.

2026-10-17 H3/H5 and Q1 implementation: three Luna workers supplied disjoint patches; coordinator source review and final H5 fixture completed integration. Source `0effba1` passes 93 Hocket cases; process/link controls contribute 21 distinct cases. The original H4 dirty-state regression remains unchanged. New commits remain local, with native custody/reopen and physical evidence explicitly outside this batch.

2026-10-18: published six H3/H5 and Q1 commits through `6e6498a` after remote refresh; started B1–B3 with cheaper parallel workers and explicit ownership. The native lane remains untouched.

2026-10-18 B1–B3 completion: source `662cda5` is CPU qualified, with 859 distinct parent/library passes, five separate child invocations and seven existing ignores. The final review added the retained-instrument Path neck receiving case and reused the existing empty geometry and capture cancellation. Four canonical findings are addressed; LO-5 remains an alias of CORE-4. All new commits remain local. No native/device or fresh-process native acceptance is claimed.

2026-10-18: pushed the nine reviewed B1–B3 commits through `b1c9dfd` after refreshing origin/main. Started B4 with three cheaper Luna workers and explicit disjoint ownership; native validation remains untouched.

2026-10-18 B4 completion: source `5dd01ae` passes 308 distinct cases across shared primitives, Woodshed audio and the Hocket engine. Source review consolidated the third frequency-bound copy in phrase preparation and removed a test-only session outcome helper. Imported and completed rendered PCM reject nonfinite values before publication; existing resources survive failed preparation. Onset accounting counts new samples once; calibration displays the estimator’s admitted support. The next frontier starts at B5. Implementation remains local; no native lane, device, supplier or original-media operations were performed.

2026-10-19: refreshed remote, pushed seven reviewed B4 commits through `8da953b`, then pulled with `--ff-only` (already current). Started B5 shared occurrence admission and independent public-API tests with two Luna agents. No native/device or supplier operations authorized for this batch.

2026-10-19 B5 completion: source `b804d901` passes 176 distinct cases with two existing Hocket host ignores. A public-API test fixture roundtrips CBOR and compares actual accepted voices with model eligibility and offline one-cycle PCM. Isolated published source reproduces three bypass failures with the identical tests; a preceding standalone lock setup refusal is explicitly excluded. Source review removed repeated caller decisions and the unused track-only wrapper; both actual start sites use one occurrence rule. New work remains local; B6 tempo/replay and diagnostics stay separate, and the native lane remains untouched.

2026-10-19 publication/B6 start: primary checkout contained additional unrelated local history and remote had advanced. Applied only the four reviewed B5 commits to isolated current main; source/test bytes match original review, automatic DOC_README merge retains incoming entries, and coordination hash was refreshed. Pushed through `4fab895`, then pulled successfully. Started the requested tempo/replay and diagnostic slice with three Luna workers. Engine already supports keyed next-bar replacement while the old voice continues; use that capability rather than stop-before-clock or a new replay state store. Supplier intake and native lane remain outside ownership.

2026-10-19 requested B6 completion: source `7b161a9` is scoped CPU qualified with 171 distinct passes and two existing Hocket host ignores. The isolated published Recorder control produces three expected failures with identical tests. Engine execution proves old voice continuity and latest keyed replacement; public wrapper and standalone CPU DOM tests prove status delivery and one visible owner. The failed selector compile attempt and blocked shared-view paint unit gate remain separately recorded, never counted as passing. Product source did not change after accepted recorder/wrapper/engine gates; the final host gate covers the sole receiving-test selector correction. Supplier inputs, native lane and primary source were preserved. At that boundary historical MIDI H6–H9 remained open; P9 records their later scoped CPU follow-up.


2026-10-21 design-decision follow-up: the user accepted D3 recovery exit/copy and
authorized direct publication. P4 current-persona no-op and its 28-case receipt
are published at `01c4b3af`. D3 implementation and qualification live in the
[existing preservation plan](2026-10-16_preservation_input_admission_plan.md#p5-d3-explicit-failed-recovery-exit-and-independent-copy).
This is a separate product-policy slice after the P10 first-wave recheck; P5a
remains with its existing owner. Persona-store migration, capture-capacity
policy and supplier checked snapshot construction remain separate work.

2026-10-22 D3 native follow-up: committed `94583610` repairs exposed copy-failure
wording and passes real macOS chooser cancellation, refusal/I/O failure, successful
copy and independent copied-media reopening. Ten accepted captures and a 92-case
desktop gate (one existing ignore) are in the [native recovery receipt](../validation/redshank-recovery-native_20261021/README.md).
P5a stays with its existing owner; physical audio and broader platform acceptance
remain separate.


2026-10-23 P11 qualification: B8 source `046b0bf4fe06a8e18dfe7b8dc8874813edfcbfbd`
passes 39 playback unit and three IPC integration cases, plus 92 desktop,18 model
unit and15 model integration cases:167 distinct passes, eight existing ignores.
Seven new regressions include three observed baseline failures (Cargo101, stable
source) and actual reconstructed backend/controller/gain checks. The successful
locked desktop build, changed-file format gate and both independent reviews
pass. The earlier wrong `redshank-genet` package selector ran no product test;
its failure is retained and excluded. Tests/build ran on stable dirty source
whose compiled product inputs exactly match the committed source. No supplier,
manifest, lock, shipping public API or P5a code changed. No new native/physical
or second-host acceptance follows. The current ledger corrects D1,S4,R04,Q5 and
optional Ringdown reach, retains all97 rows, and marks only the selected B8
findings CPU/build qualified. Publication verification and scoped pruning follow.


2026-10-23 P11 publication: source, receipt and corrected frontier are merged
and pushed through `f2b16aea`. Advertised and fetched main agree; all265 qualified
source bindings,23 evidence files and the three canonical/index/ledger files
match the fetched remote tree. The [publication record](../validation/audit-playback_20261023/publication.json)
records that observation; its metadata commit extends no qualification claim.
The completed clean checkout/branch are pruned after that publication.


2026-10-25 P12 source feedback: V03 also requires input-device errors to reach
observed health rather than only stderr, wake an idle host, and retire a failed
cached input session before an explicit retry. Output operation/capture refusal
must remain separate from actual stream failure. The same availability owner
covers those producer-to-view seams with injected fault/retry controls. Capture
review requires accepting/saving the prefix at capacity with persistent limit
wording, idempotent late release, retained PCM/body/anchor/note identity through
durable save, and playback/duck restoration at actual input stop rather than
final data acknowledgement. Export acceptance uses real worker/file results for
two colliding readable stems. These are required receiving paths within the
approved three slices; no new supplier/hardware workstream is inferred.


2026-10-25 P12 qualification: Redshank source `951c7822` and Woodshed source
`770f5443` are merged and pushed. Final selected gates pass 1,153 distinct cases,
with 16 existing ignores, five child invocations counted separately and 24 new
executed test functions. Both locked desktop builds and Redshank's Wasm check
pass. The [receipt](../validation/capture-export-availability_20261025/README.md)
preserves three reproduced baseline failures and all failed/control/superseded
runs. The export fixture's private/Shareable correction retains actual distinct
payload assertions. The incoming rehearsal geometry failure reproduces on clean
`91b2dd5b`; its corrected fixture retains visibility/extent/hit assertions and
adds actual page movement. One intermediate probe reused a stale control binary
and is excluded; an explicit four-local-package clean restored a source-bound
363-case host executable before final successful gates. Stable source alone did
not establish executable freshness.

Two successful native processes at the source-bound `770f5443` binary produce
eight reviewed nonblank captures of absent/offline availability and visible close
guards. Each scenario completes before explicit fixture discard-and-close;
process exits are zero, with no timeout or persona-owned files written. Real
microphone/output, native fault/retry/permission and Redshank native capture are
not covered. Publication binding and completed-checkout pruning follow. The
other computers remain inaccessible through current SSH credentials; their
checkout revisions and dirtiness are unverified and no remote mutation occurred.


2026-10-25 P12 publication: source, evidence and updated frontier are merged and
pushed through `dc21232d`. Advertised/fetched/local main agree. All 644
source bindings, 64 evidence files including their manifest, and the three
canonical plan/index/ledger files match the fetched remote tree. The
[publication record](../validation/capture-export-availability_20261025/publication.json)
records that observation. Both completed task/control checkouts are removed,
the merged task branch is deleted, and only the primary checkout remains
registered. Intentional archive refs and normal build caches are preserved.
The metadata commit extends no source/native/device claim. Other-host checkout
synchronization remains blocked by access; local/GitHub publication does not
establish their state.
