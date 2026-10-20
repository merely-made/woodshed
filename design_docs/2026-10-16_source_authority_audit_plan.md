# Woodshed source authority audit and remediation

Date: 2026-10-16; coordination closure: 2026-10-17.
Status: complete collective read of frozen `d12201bf`; current-source finding
reconciliation complete and published through `8204688`. Remediation remains open;
H3/H5 and Q1 implementation is CPU qualified and published through `6e6498a`.
B1–B3 runtime authority corrections are CPU qualified and published through `b1c9dfd`. B4 PCM/onset/calibration corrections are CPU qualified at `5dd01ae` and published through `8da953b`. B5 shared Recorder admission is CPU qualified at `b804d901` and published through `4fab895`. The requested B6 Recorder runtime slice is CPU qualified at `7b161a9` and local for review, with the separate shared-view unit gate blocked. The frozen
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

Status: scoped CPU qualification complete at `480c0d6`; local for review.
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
stays local for review. The preceding nine B6 commits were integrated and pushed
through `fd9fb82`; original qualification source identities remain unchanged.

## Current findings and next implementation order

1. **Preservation and input admission already have implementations.** A1, A4 and
   A5 are present. Hocket A2's capture/close/full-revision dirty guards are
   implemented, and its missing-media facts (H5) now have the separate CPU-qualified correction below. The preservation
   receipts establish their exact historical software/build results and twelve
   bounded Woodshed persona/close/quarantine/reopen captures. They do not
   establish Hocket/Redshank native acceptance or current-head integration after
   subsequent source/supplier changes. Reuse these owners and the existing
   [preservation plan](2026-10-16_preservation_input_admission_plan.md).
2. **A3 Hocket persona binding and A2 missing-media refresh are implemented.**
   The offered profile, target key and losing token are checked against actual
   admission before consent or identity changes. CPU controls cover drift,
   consent-write refusal and persisted exact target facts. Same-session import
   refreshes availability while retaining unresolved local media and branches.
   Real custody IPC, Hocket native interaction and fresh-process persona reopen
   remain unqualified; no native lane was taken over.
3. **Q1 evidence integrity has scoped executable fixes.** CI/font gates now
   require observed process evidence and reject failed or unfinished suites.
   The link checker resolves historical local targets from the same tree as
   historical referrers, while binary links remain existence checks. Fourteen
   process controls, seven link controls and the real documentation scan pass.
   Historical artifacts remain unchanged; their missing execution metadata is
   not synthesized and their product runs are not reclassified as failures.
4. **B1–B3 now have scoped CPU-qualified corrections.** Retained Path/contact
   resolution is shared across display, generic sound, managed lowering and
   durable recipe validation, with retained neck geometry in the view. SongBar
   silence preserves authored payload and one audibility rule. Future-bar Arms
   use the existing bounded queue and cleanup, with ordered stop/pause/seek and
   unreachable-target observations. B4 now has scoped CPU-qualified finite PCM rejection, chunk-invariant onset
   accounting and a single calibration estimate/support result. B5 now admits capture/import/unmute/reconciliation through one occurrence rule
   using the existing model predicate; inactive media remains retained. The requested shared Recorder tempo replay and runtime diagnostic slice now
   has scoped CPU qualification: accepted-clock-first keyed replacement,
   preserved failure facts and one shared status renderer. Its full shared-view
   unit gate is blocked by the pre-existing paint API mismatch.
   Redshank preview/runtime control and idle-worker completion remain separate
   product/controller seams. Protocol/instrument fixes B12–B14 preserve write,
   readback, restoration, incomplete data and output-file outcomes. Ringdown X1
   aliases theory T2; R8 is a P3 metadata duplication, not an established default
   hardware-guard bypass.
5. **Projection, packaging and compatibility cleanup follow actual reachability.**
   All nine named obsolete Looper scenarios remain unchanged (Q2). Release
   packagers still need build-to-binary binding and architecture checks (Q3),
   and the scorer still admits missing notes/mismatched source identity (Q4).
   macOS microphone wording now includes recording; Windows/Linux wording does
   not. The recipe supplier cohort has since aligned, but new lock metadata is
   not a new instrument execution receipt. Public legacy executors/query APIs
   remain compatibility-gated removals (K1); do not delete saved formats, PCM,
   inactive media or live orphan-clear paths merely to reduce duplication.

The [current per-finding ledger](../validation/full-source-audit_20261016/current-finding-ledger.json)
maps every one of the 93 original raw index rows, plus O1 and the separately
named LO-4/LO-5/LO-K1 items. No entry is silently omitted or counted as a unique
confirmed bug. The runtime reviewer records five source replacements in the
old overlay findings: LO-1, LO-2, LO-K1, LO-3 and LO-4. The undefined-pitches
compile failure is historical, and generic Walk/Hear timing remains distinct
from the managed processor-clock path. Source replacement is not a new
current-head acceptance claim. Full details and evidence limits are in the
[safety](../validation/full-source-audit_20261016/current/safety.md),
[runtime](../validation/full-source-audit_20261016/current/runtime.md),
[port/protocol](../validation/full-source-audit_20261016/current/ports.md), and
[evidence/tooling](../validation/full-source-audit_20261016/current/evidence-tooling-current.md)
reconciliations.

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
