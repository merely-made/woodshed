# Woodshed full-source authority and success-path audit

**Date:** 2026-10-16. **Status:** full published-source read complete; source audit and recommendations, no product fixes or product qualification performed here.

**Frozen published revision:** `d12201bfe80cc94345c6308c46caaebf4a9fe42e`. The initial primary checkout was `1bc4f6a` plus uncommitted P5a timing work. A fresh fetch found two arrangement commits on main; the audit used a separate exact `git archive` of the published revision instead of modifying the checkout. The P5a owner subsequently took over its implementation and advanced the primary checkout. This report continues to refer to the immutable audit snapshot. It does not claim that later edits have been read or qualified.

## Result

There is real duplicated decision-making, concentrated at boundaries where a projection, compatibility adapter or lifecycle handler independently decides what another owner has accepted. The most consequential cases are preservation before session replacement, malformed saved-model admission, retained musical setup versus current navigation, output/request state versus accepted execution, and native scheduling versus scenario scheduling. Several tests cover one side of these seams or assert synthetic state that the real producer never emits.

The shared stack is substantially adopted: the desktop host, recorder surface/controller, theory catalog, branching history substrate, appearance editor, pure capture kernels and product output runtime all have actual callers. Those boundaries should remain. The useful simplification is to remove the bypasses and competing partial rules around them, together with genuinely unused legacy executors.

## Complete-read receipt

The eleven published-source partitions cover **936 assigned text program/config/scenario/fixture/receipt files and 231,127 lines**. The parent additionally read the omitted W3C `.test` definition: **63 lines**, yielding **937 files and 231,190 lines** in the recorded textual review. The generated [coverage verification](coverage-verification.json) independently checks each assignment's SHA-256 and inclusive read ranges, exact once-only assignment, and zero missing assigned files. The [inventory](inventory.json) and individual lane receipts retain every path and hash.

These counts deliberately include more than implementation code:

| Category | Assigned files | Lines |
|---|---:|---:|
| Program source and tests (`rs`, `py`, `sh`, `ps1`, `c`, etc.) | 303 | 149,331 |
| Native scenarios | 151 | 4,963 |
| Presentation source | 2 | 756 |
| Cargo lockfiles | 6 | 37,386 |
| Manifests, configuration and data fixtures | 159 | 11,140 |
| Receipt metadata and evidence configurations | 315 | 27,551 |
| Additional W3C test definition read by parent | 1 | 63 |
| **Total** | **937** | **231,190** |

Every source/test/program line was assigned for full reading, including code in validation and examples. Repetitive receipt JSON was fully parsed and reviewed structurally; hashes were checked and changed semantic fields compared. A parsed hash inventory is not a claim that each digest received a prose discussion. Readers explicitly reread truncated source output in bounded ranges before marking completion.

The scope includes all root crates and the independent Hocket, Redshank and Ringdown workspaces, plus manifests, scripts, native scenarios and validation programs. Binary assets, images, fonts, compressed archives, build artifacts, third-party dependency implementations and every historical log/screenshot were not an exhaustive source-reading scope. Exact locked supplier locations were inspected when necessary to verify a finding. No native images were newly reviewed here.

The separately frozen P5a overlay now has a **complete collective semantic read of all 33 assigned files / 34,861 lines**, including all 155 tracked patch hunks and 769 added/new lines. The October 17 continuation read the remaining 27,662 lines with unchanged input hashes. See [overlay report](local-overlay-report.md) and its semantic-range ledger. Published coverage and this overlapping local source state remain separate claims; neither establishes runtime qualification.

## First fixes: preserve work and reject invalid authority

### 1. One preservation decision before replacing or closing a product session

**High priority; medium effort.** Woodshed's persona switch runs before ordinary persistence, opens incoming custody and resets the UI/store without the existing close path's save barrier. A pending capture publication or unconfirmed save can lose outgoing work even when reselecting the same persona. Parent corroboration: `woodshed-genet/src/persona.rs:171-199`, `sync.rs:35-57`, and `capture_persistence.rs:545-574,600-660`. References: host H1.

Hocket's native close consults appearance/scenario handling but never recorder project dirtiness or pending project I/O. It can close with unsaved takes; appearance's Save/Discard decision effectively approves closing a different, unsaved document. Parent corroboration: `ports/hocket/.../main.rs:172-184`. Reference: Hocket H2.

Keep preservation product-owned, but use one transition decision inside each host for Close, Switch, Replace and any terminating update path. Finish or explicitly discard outgoing work before changing identity, dropping live media or retiring its completion epoch. The existing Woodshed Retry/Discard/Cancel behavior is a concrete starting point; stale-result rejection must follow preservation, not replace it.

### 2. Durable graph/history input needs one checked admission seam

**High priority; medium effort.** Woodshed's untagged `HistoryWire` can interpret a malformed current lineage as an empty legacy log, because the fallback ignores unknown fields and defaults missing events. Subsequent save can replace retained history with empty data. A well-typed but invalid snapshot instead reaches the locked Chartulary `from_snapshot` vector indexing and can panic before the host can quarantine the file. Parent corroboration: `woodshed-core/src/history.rs:390-438` and exact Mere6575075 `chartulary/src/stemma/snapshot.rs:17-106`. References: core CORE-1/2.

Hocket's history union verifies parent existence but not cycles. An authenticated same-session handoff can pass integration and then loop/allocate indefinitely in the unchecked ancestor walk during Accept. Parent corroboration: `hocket-model/src/history.rs:399-484` and `hocket-project/src/handoff.rs:327-331`. Reference: shared S1. Valid signatures are not structural model validation.

Use explicit legacy-shape discrimination and fallible structural reconstruction in the respective model owner. Validate identity uniqueness, references, acyclicity and root/head invariants before publishing a loaded document. Keep the original data and propagate ordinary admission failure to existing quarantine/preservation behavior. The theory lane also found duplicate nonzero persisted Card IDs admitted by load despite unique-ID action/graph assumptions (theory T4); apply one Set invariant at every loaded active/inactive/retained bank rather than renumbering ambiguous references.

### 3. Redshank recovery must distinguish empty storage from unrecoverable storage

**High priority; small-to-medium effort.** `JsonDirectoryStore::load` returns `Ok(None)` after all existing generations fail decoding. Startup creates an ordinary writable default model. A later persistent action publishes that replacement; pruning removes malformed historical generations. An I/O failure displays a notice but likewise initializes writable replacement state. Parent corroboration: `ports/redshank/storage/src/lib.rs:109-182` and `desktop/src/main.rs:2270-2282`. Reference: Redshank R01.

Return Empty, Loaded and FailedRecovery as distinct outcomes. A failed restore must not authorize replacement or deletion merely because the UI can continue with an in-memory default.

### 4. Preserve the capture retirement handshake through project replacement

**High priority; medium effort.** Hocket accepts a handoff while free capture is live, or admits an Open completion after recording begins. Replacement drops the recorder ticket and queued stop action, but the adapter keeps executing the old free capture. The Record action then has no token with which to stop it; clock and device changes refuse while it remains busy. Parent corroboration: Hocket `state.rs:637-680`, shared Recorder `lib.rs:849-872,1094-1101,1257`, adapter ownership and engine free capture. Reference: Hocket H1.

Block/defer replacement while capture is live, or add explicit fallible retirement while retaining the old token until completion. Clearing the busy flag or rejecting an eventual completion is insufficient.

## Next fixes: make rendered and sounded facts use their owner

### 5. Resolve retained playable material once, then derive dots and sound

**High priority; medium effort.** Saved drawn Paths retain instrument/tuning but their dot/audio implementations use live Stage tuning. Navigating from Standard to Drop D can retune a saved Path without a Card edit; a narrower instrument can drop steps. Dot and audio filters also disagree about the live neck start. Parent corroboration: `woodshed-core/src/lib.rs:1330-1353,1406,1450-1474,1977-1987`. Reference: CORE-4.

Nonempty Solo marks that no longer match a chord's contacts can fall back to the entire chord, while the board excludes every unmarked contact. Reference: CORE-6. The exercise named Pentatonic Box Shift uses approximate fret offsets that produce nonpentatonic notes even on standard guitar; its tests check counts and shifts rather than pitch membership (theory T3).

Reuse the retained setup/contact realization and canonical scale/tuning facts. Distinguish deliberate silence from unavailable material and viewport clipping from instruction content. Test the actual save → navigation/edit → Hear/Run path instead of only unchanged-setup helpers.

### 6. Preserve material through reversible-looking saved-song controls

**High priority; small effort.** `SongBar::toggle_silent` clears the exact pitches/formula and restores a hardcoded Major chord on the second click. Its test accepts any positive root after the second toggle, so it passes the loss it should detect. Parent corroboration: `woodshed-core/src/song.rs:90-105,128-136,296-311`. Reference: CORE-5.

Represent silence independently or retain/restore the complete original payload. The declared saved-song compatibility migration is not a reason to discard arbitrary retained pitch vectors.

### 7. Let the accepted capture schedule survive until its target

**High priority; small-to-medium effort.** A saved-song arm targeting a later bar is consumed at the first intervening boundary. If that boundary is not its target, the processor drops it without starting capture or reporting refusal. Parent corroboration: `woodshed-audio/src/output_runtime/processor.rs:660-709,815-835`, saved-bar `seek=false` adapter. Reference: AUDIO-01.

Retain the accepted non-seeking arm through intervening bars until target, cancellation, supersession or a terminal/unreachable outcome. Preserve the separate Set seek behavior.

### 8. Share finite-resource admission between live preparation and export

**High priority; medium effort.** Live backing/preview preparation accepts very large finite pitches that can overflow synthesis into NaN PCM; export has a stronger phase/PCM validator and rejects the same material. Imported PCM and prepared click validation have related partial checks. Parent corroboration: processor preparation `:159-199`, `chord_audio.rs:138-173`, export `song_export.rs:168-193`. Reference: AUDIO-02.

Use a preparation-time finite-resource check before command publication, off the callback. Keep valid PCM bytes and previous accepted backing intact on refusal. Shared onset accounting also counts carried partial-frame input twice (AUDIO-03), and calibration's display count includes early pairs the estimator rejects (AUDIO-04). These should be repaired once in their shared primitives, with exact chunk/accounting and admitted-pair tests.

### 9. Use one eligibility predicate for every Hocket layer start

**High priority; small-to-medium effort.** Normal full-track reconciliation honors SelectOne; capture promotion, import replay and inactive-layer unmute can bypass it and start an excluded layer. Model, waveform/export and live audio can disagree. Parent corroboration: recorder `lib.rs:1191-1196,1292-1318,1369-1433`, model `track.rs:107-113`. Reference: shared S2.

A live tempo nudge stops all accepted loops without queuing replay or offering a direct Resume (shared S3). Decide the desired transport behavior explicitly, then reconcile eligible voices through the same predicate on both success and refusal. These are controller seams, not reasons to merge the product engines.

### 10. Separate desired controls, accepted execution and displayed observations

**High priority; medium effort.** Woodshed MIDI marks an output connected before its worker opens it; the worker discards the open error and has no result channel. Port rescan also preserves an index while changing its device meaning, and connection replacement keeps old transport state without sending the old Stop/new Start. Parent corroboration: `woodshed-genet/src/midi.rs:98-114,192-225`, `sync.rs:544-560`. References: host H7-9.

Redshank shares one runtime but gives main and preview backends separate desired gain; a preview can create output at 100 while the listener is muted at zero. Worker recovery can similarly recreate default controls while preserving old displayed values. Parent corroboration: `playback/src/worker.rs:305-308`, `backend.rs:613-638,765-769`. References: Redshank R02/3/17.

Settings saying both input/output streams are open from absence of an error contradicts lazy microphone opening, and tuner views can say Listening after failure (views V03). Route these displays through admitted availability and operation results. Keep a control-side acceptance distinct from synthesized PCM, processor start and audible sound.

### 11. Use actual visibility, complete paint inputs and one work inventory

**Medium priority; medium effort.** The host's graph cache signatures omit marks/strength that the shared painter uses, retaining stale claim strokes after a member/assertion change (host H3). Overview animation consults a remembered workspace tab even while a legacy pill renders Looper/Tools; reduced-motion CSS and scene simulation consult different policies (views V01/2). Native text registration omits two mounted editable fields, so ordinary typing works while IME/pointer caret behavior does not (host H4).

Redshank native scheduling omits alignment/export work that its scenario busy predicate includes. Plain-channel worker completion need not wake an idle paused app; active scenarios conceal the problem (R04). The dock also replaces its voice-button pointer owner on recording projection before release (R05); the parent inspected both the handler and the replacement branch, and the reviewer checked the exact supplier capture teardown.

Make the shared rendering/host DTO own its full revision, derive visibility from the renderer dispatch, compose effective reduced motion once, register fields through a shared text seam, and derive native/scenario work scheduling from one inventory or wake every worker completion.

### 12. Remove request-shaped identity and success claims from adapters

**High priority where it changes identity; otherwise medium.** Hocket stages a persona name but discards it at confirmation; an unqualified new custody connection can return a different active persona, and cached consent combines the old name with the new key. Parent corroboration: `state.rs:509-563`, `identity.rs:207-225`; reviewer verified the exact locked supplier's connect semantics. Reference: Hocket H3. Stage and confirm the actual target profile/key, or require fresh consent after a change.

Hocket's dirty marker compares only saved head despite off-head retained branch/media imports (H4), and missing-media metadata survives recovery even after validated blobs arrive (H5). Use the complete admitted document revision and derive availability from the resulting store.

Ringdown's restore wrapper drains effects, discards individual re-push replies and returns success/counts even after false replies. The Woodshed instrument adapter likewise caches writes as AlreadyMatched without readback; its hardware example prints mismatch yet can return success and skips restoration on intervening error. Parent corroboration: `ringdown-client/src/lib.rs:753-801`; source reports ringdown R1/X1 and theory T1/2. Keep attempted, parsed, accepted and verified outcomes distinct; use the existing lower-level receipts. The instrument path is a public library/example with no current Woodshed GUI consumer.

## Evidence and cruft frontier

### 13. Fail evidence gates on actual command failure

**High priority for verification integrity; small effort.** CI/font helpers parse only `test result: ok` summaries, discarding later FAILED summaries; one writer invents zero process exits. The doc-link helper loads baseline referring text but checks current target headings, subtracting away new target regressions. Parent [isolated counterexamples](parent-verification.json) establish both logical false passes. References: evidence E1/E2. This does not establish that any retained accepted run actually failed; the reviewed historical logs had passing outcomes.

Package scripts hash whichever existing executable they find while attributing it to current checkout/lock state, without a build receipt binding those facts (tooling T2/3). Benchmark missing note arrays can become perfect empty scores and mismatched source identities are not checked (tooling T4/5). These are operational helper contracts, not claims about current published artifacts or analyzer quality.

### 14. Remove unreferenced execution machinery rather than preserving self-tests indefinitely

**Medium cleanup priority; small-to-medium effort after compatibility review.** The old exported single-bar Looper, Song::advance execution state machine and PatternRenderState retain independent capture/scheduling/mixing rules. Current in-repo product callers use the production processor; these helpers are exercised by their own tests. Parent read the full legacy Looper and the audio lane traced callers (AUDIO-05).

Three legacy capture command variants have no current producers but retain weaker parallel execution beside frozen RecorderRequest (host H10). A capture awaiting_commit/is_busy state is production-written but test-only-read (H11). Old in-canvas editor/song-chip CSS remains unproduced (views V05). Keyed graph relation derivation is repeated and StageGraphSnapshot::reasons ignores its own snapshot (core CORE-C1/C2). These are concrete removal/consolidation candidates. External public API consumers still require a compatibility decision; no deletion is authorized by this audit.

Nine retained native scenario files target removed Looper controls; parse success cannot establish that their clicks still run (tooling T1). Update operational scenarios and retire obsolete ones through the owning plan, while retaining historical receipts at their original revision.

## Recommended implementation order

1. **Preservation and malformed-input admission.** Shared lifecycle barriers inside each product; current/legacy history discrimination; fallible checked topology and Set IDs; failed-restore nonwriting state. Add negative controls that prove retained bytes/live media survive the refusal.
2. **Musical/runtime owner consistency.** Retained setup/contact realization, silence preservation, target-bound arms, finite PCM preparation, SelectOne and actual desired/accepted control state. Reuse existing primitives and product owners.
3. **Host projections and worker scheduling.** Complete paint invalidation, visibility/motion, native text, work completion wakes and gesture lifetime. Exercise real host hooks, including idle and replacement transitions.
4. **Evidence and removal pass.** Fail-closed command receipts and baseline targets; package provenance; benchmark identity/schema; remove unused execution paths and strengthen surviving tests around real integration seams.

Effort sizes are relative implementation scope, not calendar estimates. Device-free negative-path regressions belong with each actual change; native/fresh-reopen and physical/device qualification remain separately scoped. This audit ran no product test suites or builds.

## Reports and scope limits

| Partition | Complete files | Complete lines | Report |
|---|---:|---:|---|
| Core | 49 | 23,561 | [Core](core-report.md) |
| Views | 37 | 23,962 | [Views](views-report.md) |
| Native host | 20 | 18,828 | [Host](host-report.md) |
| Audio/primitives | 34 | 12,014 | [Audio](audio-report.md) |
| Shared recorder/project/model/views/web/text | 33 | 9,587 | [Shared](shared-report.md) |
| Theory/instrument/graph | 21 | 11,180 | [Theory](theory-report.md) |
| Hocket port | 73 | 20,440 | [Hocket](hocket-report.md) |
| Redshank port | 178 | 51,557 | [Redshank](redshank-report.md) |
| Ringdown port | 23 | 11,066 | [Ringdown](ringdown-report.md) |
| Tooling/root scenarios | 97 | 13,362 | [Tooling](tooling-report.md) |
| Validation programs/metadata | 371 | 35,570 | [Evidence](evidence-report.md) |

The [reviewer finding index](reviewer-finding-index.json) preserves raw findings, candidate cleanups, acknowledged gaps and overlaps; its length is not a count of unique confirmed production bugs. The synthesis above groups overlapping reports and names parent-corroborated source families. Individual reports retain narrower remaining findings, exact callers, source locations, test gaps and policy questions. Examples include Redshank export identity collisions, exact-end seeking, capture caps and identity copy; Ringdown malformed protocol inputs/offline capture scoping; Hocket updater settings and staged-offer drift. These are source-derived findings, not newly observed incidents.

Preserve the deliberate boundaries: one Woodshed output runtime; Hocket polling versus Woodshed sample-driven scheduling; saved-song compatibility until representation is lossless; explicit legacy PCM clocks; Set occurrence versus retained copy; personal assertions versus generated catalog facts; product musical judgment over shared structural capabilities. The report does not recommend a broad new framework or copying product authority into Graphshell.

The primary P5a owner has been notified of the overlay's remaining empty-realization-as-silence admission issue (LO-1). Old fixture errors in that frozen overlay are being repaired in the active implementation lane; this report does not present those older compile/test failures as the current checkout's final status. The owner exclusively holds source/Cargo/native/Git publication work. No product files were edited or committed by this audit, and nothing was pushed.


## Collective cleanup reconciliation — 2026-10-17

The [prioritized remediation plan](REMEDIATION_PLAN.md) is the single cleanup frontier. Ringdown was already included in the published coverage. R8 is the instrument review's redundant-method-metadata cleanup, P3, with no ordinary default-path bypass. X1 aliases theory T2 and is counted once, including T2's post-write error/restoration gap. Theory T1 remains the distinct discarded-write-receipt/cached-match defect. B12/B13/B14 and K3 cover the Ringdown findings with the correct owners and acceptance checks.

Selective semantic inspection at primary HEAD 125be57d184f46b9f878463243d5a6ed8d8df4c3 found new preservation/history/Set/recovery implementations at several first-priority sites. A0 matches those to exact-source receipts before rebuilding them. Hocket's persona confirmation still discarded the staged target before an unqualified connect; A3 is therefore the first new product correction proposed after that closure pass, subject to current-source retargeting. A later hash sample at b8095653f66a718ba5b39fa5b10232b549bce906 found all 23 Ringdown files and three instrument files still unchanged. No current tests, native or hardware outcomes were established here; concurrent product work was left untouched.

The independently completed overlay continuation is retained: 33 files / 34,861 semantic lines, with LO-5 corroborating CORE-4 and LO-4 a distinct inherited Walk-timing follow-up. It does not change the published 937-file / 231,190-line total. The [reconciliation ledger](reconciliation-20261017.json) records input hashes, aliases and source comparisons. No code, locks, snapshots, profiles, commits or Git state were changed by this follow-up.
