# Woodshed bounded runtime reconciliation — 2026-10-17

**Target:** local HEAD `a42477297b692feb7df4d8be74fba4cb83bcaf8c` in `/Users/markik/Code/repos/woodshed`. Frozen audit: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`. The d122 audit remains complete for its own snapshot. This is targeted reconciliation of all 35 requested IDs, not a full current-code read or current runtime qualification.

**Disposition:** 30 scoped findings retain implementation/reachability; 5 show source replacement. All current qualification is unassessed. LO-5 duplicates CORE-4 for remediation. Public/test-only cleanup findings keep that reachability limit.

## Key current findings

- Retained setup and selection remain open: saved Path replay uses live setup (CORE-4/LO-5), stale Solo marks fall back to full chord in generic Hear (CORE-6), and saved-song silence substitutes Major (CORE-5).
- Audio gaps remain source-supported: future saved-bar arms can be drained early (AUDIO-01), finite-resource admission is inconsistent (AUDIO-02), onset split batches double-count carried samples (AUDIO-03), and calibration support counts use different pairing rules (AUDIO-04).
- Host/view contracts remain open: persona mutable slots, paint invalidation, missing native fields, MIDI inventory/routing/admission/transport, audio readiness wording, motion/visibility/navigation/provenance, and reading disclosure/placement.
- LO-1/LO-2/LO-3/LO-K1 have source replacements. The frozen compile blocker is historical; full current fixture and qualification closure remain unassessed.
- LO-4 is split: managed Run timing has replacement source; generic Path/Walk Hear retains fixed preview timing. LO-5 duplicates CORE-4 and must not add a canonical remediation count.

## Final archive ledger

**32 raw IDs + LO-4 / LO-5 / LO-K1 = 35 bounded dispositions.** U = unchanged implementation/current reachability confirmed; R = source replacement observed. These source classifications do not certify current behavior or qualification. Every row retains unassessed current runtime, software, native, fresh-process, device/physical, historical-to-HEAD applicability and full-current-read fields in runtime.json.

| Group | ID | Source | Owner / next package | Current qualification |
|---|---|---|---|---|
| raw32 | CORE-3 | U | Woodshed history / occurrence provenance / D7 | unassessed |
| raw32 | CORE-4 | U | Woodshed Card realization / B1 | unassessed |
| raw32 | CORE-5 | U | Woodshed saved-song editor / B2 | unassessed |
| raw32 | CORE-6 | U | Woodshed marked-contact realization / recipes / B1 | unassessed |
| raw32 | CORE-7 | U | Woodshed Pitch motion reading / scene / C3 | unassessed |
| raw32 | CORE-8 | U | Woodshed relation disclosure view / C3 | unassessed |
| raw32 | CORE-C1 | U | Woodshed reading relation policy / D10 | unassessed |
| raw32 | CORE-C2 | U | Woodshed public scene query compatibility / K1/D13 | unassessed |
| raw32 | AUDIO-01 | U | Woodshed output capture scheduler / B3 | unassessed |
| raw32 | AUDIO-02 | U | Woodshed prepared-resource admission / B4 | unassessed |
| raw32 | AUDIO-03 | U | audio-primitives onset clock / B4 | unassessed |
| raw32 | AUDIO-04 | U | audio-primitives calibration / Woodshed display / B4 | unassessed |
| raw32 | AUDIO-05 | U | Woodshed audio API compatibility / K1/D13 | unassessed |
| raw32 | AUDIO-06 | U | Woodshed diagnostic evaluator; parent evidence owner / Q1 | unassessed |
| raw32 | H2 | U | Woodshed filesystem / custody composition / D1 | unassessed |
| raw32 | H3 | U | Woodshed native paint cache / C1 | unassessed |
| raw32 | H4 | U | Woodshed native focused-text adapter / C1 | unassessed |
| raw32 | H6 | U | Woodshed process inventory / persona reset / B6 | unassessed |
| raw32 | H7 | U | Woodshed MIDI routing identity / B6 | unassessed |
| raw32 | H8 | U | Woodshed MIDI worker admission / B6 | unassessed |
| raw32 | H9 | U | Woodshed MIDI transport lifecycle / B6 | unassessed |
| raw32 | H10 | U | Woodshed command compatibility / recorder / K1/D13 | unassessed |
| raw32 | H11 | U | Woodshed capture persistence status / K2 | unassessed |
| raw32 | V01 | U | Woodshed accessibility / motion / C2 | unassessed |
| raw32 | V02 | U | Woodshed render dispatch / animation / C2 | unassessed |
| raw32 | V03 | U | Woodshed observed audio status views / B7 | unassessed |
| raw32 | V04 | U | Woodshed process navigation / C2 | unassessed |
| raw32 | V05 | U | Woodshed CSS/font selector cleanup / K2 | unassessed |
| raw32 | V06 | U | Woodshed provenance formatting / C2 | unassessed |
| raw32 | LO-1 | R | Woodshed bound preview / managed clock admission / P5a/P5 verification | unassessed |
| raw32 | LO-2 | R | Woodshed timing fixtures / clock owners / P5a/P5 fixture verification | unassessed |
| raw32 | LO-3 | R | Woodshed audition / host / history / Catalog audition; D7 for wider semantics | unassessed |
| supplemental | LO-4 | R | Woodshed processor clock / generic audition / P5 managed timing; residual Walk Hear | unassessed |
| supplemental | LO-5 | U | Woodshed Card realization / B1 | unassessed |
| supplemental | LO-K1 | R | Woodshed layout fixture owner / P5a fixture verification | unassessed |

Parent reports the frozen published audit verified at 937 files / 231190 lines and the overlay at 33 files / 34861 lines; safety/ports reviewers are complete. Those are parent-reported closure facts, not verification performed by this lane. Evidence helpers remain open. Parent owns the 93-raw crosswalk, canonical plan, coverage verification and indexing. This runtime ledger requires no broader read, build or implementation to finish its bounded assignment.

## Method and source binding

Frozen reports and existing P5/P6/P7/timing/audio receipts were locators. Targeted source excerpts, local read-only diffs/caller searches and relevant SHA-256 comparisons support this report. Whole-file hashing is byte identity, not semantic coverage. Current ranges below are source locators, not a line-accounted read manifest. Changed-file conclusions concern named logic and callers only.

Initial worktree was clean. During inspection an untracked `validation/full-source-audit_20261016/` appeared; it was left untouched. HEAD remained a424772, and tracked files had no diff from HEAD before writing. No network, tests, builds, native launches, implementation, Git mutations or other-chat messages were performed. Only runtime.md and runtime.json are written. Parent owns coverage verification, evidence helpers and indexing.

The classifications are **unchanged implementation/current reachability confirmed**, **source replacement observed**, **named historical software/native qualification**, and **unassessed**. Unchanged refers to relevant implementation logic, even where enclosing files changed. Source replacement does not certify correction or qualification. Historical qualification is named to a receipt candidate and is not independently rerun or audited here.

## Scoped dispositions

### CORE-3 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed history / occurrence provenance — D7.

Related ranking still consumes practice with stated origin. Staging carries origin, but managed open_practice_history, legacy admission and completion still pass None. New clock lifecycle does not connect this producer to that consumer.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Choose exact occurrence-origin semantics and connect the real Related → Stage → practice flow without double counting.

**Current source locators:** `crates/woodshed-core/src/lib.rs:890–940`; `crates/woodshed-views/src/stage.rs:2274–2296`; `crates/woodshed-views/src/stage/practice_clock.rs:404–424`; `crates/woodshed-views/src/stage/timing.rs:138–153`; `crates/woodshed-views/src/stage/related.rs:635–650`.

### CORE-4 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed Card realization — B1.

Saved Path setup is retained, but generic dots/audio and new managed Path lowering still build from self.tuning and live string/fret limits. Capturing a running Stage does not restore the saved Path setup. LO-5 corroborates this defect.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Resolve retained/inherited setup once; preserve order/repeats and separate viewport clipping from unavailable instructions.

**Current source locators:** `crates/woodshed-core/src/lib.rs:1321–1350`; `crates/woodshed-core/src/lib.rs:1395–1470`; `crates/woodshed-core/src/lib.rs:1972–1993`; `crates/woodshed-core/src/practice_clock.rs:267–286`; `crates/woodshed-views/src/stage/instances.rs:200–250`.

### CORE-5 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed saved-song editor — B2.

Byte-identical SongBar::toggle_silent clears formula_name then substitutes Major and revoices. Current Looper control still calls it.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Retain exact authored chord/pitch payload through silence; preserve SongDoc compatibility pending lossless migration.

**Current source locators:** `crates/woodshed-core/src/song.rs:90–136`; `crates/woodshed-views/src/stage/looper.rs:120–138`.

### CORE-6 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed marked-contact realization / recipes — B1.

Nonempty Solo matching no dot still falls back to card_voicing in generic Hear. Shape editing and retained recipe paths remain. Managed clock now has additional contact/preview agreement refusal; that does not remove the generic fallback.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Keep empty effective Solo empty or unavailable; align chord recipe contact validation with scale-pattern validation.

**Current source locators:** `crates/woodshed-core/src/lib.rs:2030–2054`; `crates/woodshed-core/src/captured_arpeggio.rs:115–150`; `crates/woodshed-views/src/stage/shapes.rs:10–35`; `crates/woodshed-core/src/practice_clock.rs:153–184`; `crates/woodshed-views/src/stage/musical_context.rs:645–710`.

### CORE-7 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed Pitch motion reading / scene — C3.

stage_scene and pitch_motion_reading are byte-identical to d122. Context offsets and staged subject_position still differ; sequential identity/scoring policy remains disconnected.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Make the reading own context and occurrence placement while preserving exact radius.

**Current source locators:** `crates/woodshed-core/src/stage_scene.rs:607–630`; `crates/woodshed-core/src/stage_scene.rs:1394–1440`; `crates/woodshed-core/src/pitch_motion_reading.rs:141–220`; `crates/woodshed-views/src/stage/pitch_motion.rs:34–47`.

### CORE-8 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed relation disclosure view — C3.

Unchanged scene still supplies explicit Context. Current Some(source) arm uses only set_graph_relation_visible; reading-specific context_relation_visible is confined to None.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Apply reading/focus disclosure to explicit Context without deleting core facts.

**Current source locators:** `crates/woodshed-core/src/stage_scene.rs:917–955`; `crates/woodshed-views/src/stage.rs:512–546`; `crates/woodshed-views/src/stage.rs:718–746`.

### CORE-C1 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed reading relation policy — D10.

Byte-identical stage_context and stage_scene retain repeated keyed relation derivation, incidental shared_tones weighting and competing endpoint identity construction. Still a consolidation candidate.

**Reachability:** production derivation; policy candidate, no new runtime severity. **Current qualification:** unassessed.

**Next:** Decide background strength and exact endpoint policy before consolidating final-node derivation.

**Current source locators:** `crates/woodshed-core/src/stage_context.rs:504–574`; `crates/woodshed-core/src/stage_scene.rs:917–955`; `crates/woodshed-core/src/stage_scene.rs:959–1245`.

### CORE-C2 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed public scene query compatibility — K1/D13.

Byte-identical snapshot reasons() ignores captured inventory and recomputes supplied Set facts. Current targeted snapshot caller search finds tests, while production uses detail/source accessors.

**Reachability:** public API; in-repo snapshot callers test-only. **Current qualification:** unassessed.

**Next:** Check external API compatibility before removal/deprecation or moving a renamed live query off the snapshot.

**Current source locators:** `crates/woodshed-core/src/stage_scene.rs:509–526`; `crates/woodshed-core/src/stage_scene.rs:1918–1926`; `crates/woodshed-core/src/stage_scene.rs:2508–2520`.

### AUDIO-01 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed output capture scheduler — B3.

Saved-bar request now passes async input reservation but still calls arm_capture(index,false). Boundary drains Arm once and starts only at matching cursor, then resets pending_len. Selected-Card pinning is a different scope.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Preserve nonseeking future saved-bar arms through intervening boundaries and expose unreachable one-shot termination.

**Current source locators:** `crates/woodshed-genet/src/audio.rs:301–305`; `crates/woodshed-genet/src/audio/input_admission.rs:220–242`; `crates/woodshed-audio/src/output_runtime/processor.rs:944–1000`.

Historical receipt locators: `validation/coreaudio-input_20261017/receipt.json`. They do not certify HEAD.

### AUDIO-02 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed prepared-resource admission — B4.

Changed preparation still accepts finite positive saved-chord inputs and renders/cache-publishes without a completed-sample finiteness check; imported PCM checks rate/length. New bounded phrase preparation does not close this saved-chord/imported-PCM seam.

**Reachability:** malformed retained backing; no reproduced device failure claimed. **Current qualification:** unassessed.

**Next:** Unify finite-resource validation before publication, off callback; retain previous resources on rejection.

**Current source locators:** `crates/woodshed-audio/src/output_runtime/processor.rs:260–310`; `crates/woodshed-audio/src/chord_audio.rs:138–173`; `crates/woodshed-audio/src/offline/song_export.rs:160–205`.

### AUDIO-03 — unchanged implementation/current reachability confirmed

**Owner / next package:** audio-primitives onset clock — B4.

Primitive and Woodshed onset adapter retain frozen bytes. feed counts a partial prefix, then adds the full completed frame including that prefix again. Adapter still calls detector.feed.

**Reachability:** shared API and Woodshed feed; current Hocket enablement and desktop BPM exposure unassessed. **Current qualification:** unassessed.

**Next:** Count each newly consumed sample once and qualify chunk invariance. Do not infer Woodshed wall-time calibration error.

**Current source locators:** `crates/audio-primitives/src/onset.rs:174–205`; `crates/woodshed-audio/src/onset.rs:180–200`.

### AUDIO-04 — unchanged implementation/current reachability confirmed

**Owner / next package:** audio-primitives calibration / Woodshed display — B4.

Unchanged estimator rejects negative offsets while count_matches accepts either sign. Unchanged calibration session combines those results and Settings displays the count.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Return estimate and admitted pair count from one pairing result; retain approximate calibration scope.

**Current source locators:** `crates/audio-primitives/src/calibration.rs:45–95`; `crates/woodshed-audio/src/calibration.rs:285–300`; `crates/woodshed-views/src/stage/settings.rs:117–131`.

### AUDIO-05 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed audio API compatibility — K1/D13.

Legacy Looper/offline executors retain frozen bytes; Song::advance still coexists with Processor. Targeted caller search finds no product construction of Looper or Song::advance calls; render_pattern has its export wrapper and tests.

**Reachability:** public compatibility; current product execution not found by targeted search. **Current qualification:** unassessed.

**Next:** Check external consumers before removing/isolating legacy executors; preserve live Song/Bar projection DTOs.

**Current source locators:** `crates/woodshed-audio/src/looper.rs:60–181`; `crates/woodshed-audio/src/offline.rs:39–76`; `crates/woodshed-audio/src/song.rs:445–635`.

### AUDIO-06 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed diagnostic evaluator; parent evidence owner — Q1.

audio_ddx is byte-identical. Runtime completion/exit criterion still differs from raw zero-event criterion. No historical false pass is inferred where event counts were inspected.

**Reachability:** explicit diagnostic invocation. **Current qualification:** unassessed.

**Next:** Parent should define consistent terminal diagnostic criterion; no helper edits in this lane.

**Current source locators:** `crates/woodshed-audio/examples/audio_ddx.rs:129–144`; `crates/woodshed-audio/examples/audio_ddx.rs:213–217`.

### H2 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed filesystem / custody composition — D1.

open_store_as still uses FsBackend::new per identity. Preservation/reset improvements do not create independent mutable persona slots. Wrong-key quarantine remains protective.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Design namespace and singleton migration with explicit override semantics, retaining quarantine.

**Current source locators:** `crates/woodshed-genet/src/storage.rs:78–132`; `crates/woodshed-genet/src/persona.rs:300–348`.

### H3 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed native paint cache — C1.

leaves.rs adds fretboard clock projection, but Related/Set relation signatures still omit marks/strength while hashing ID/visibility/emphasis/route.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Use complete paint fingerprint and qualify claim-only changes through leaf sync.

**Current source locators:** `crates/woodshed-genet/src/leaves.rs:290–310`; `crates/woodshed-genet/src/leaves.rs:410–445`; `crates/woodshed-views/src/stage.rs:718–746`.

### H4 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed native focused-text adapter — C1.

New arrangement-field support does not register personal-reason note or retained-Set name. Ordinary registry remains Search/CardRename/RelationshipLabel. Ordinary key insertion is not claimed broken.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Register both fields; qualify native IME, pointer caret and selection.

**Current source locators:** `crates/woodshed-genet/src/text.rs:23–115`; `crates/woodshed-views/src/stage/related.rs:478–498`; `crates/woodshed-views/src/stage/overview.rs:990–1015`.

### H6 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed process inventory / persona reset — B6.

reset_for_persona preserves clock/appearance but replaces UiState without retaining MIDI inventory or requesting rescan. midi_devices rescans only on refresh_requested.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Retain inventory or rescan before interpreting reset selections; decide route persistence separately.

**Current source locators:** `crates/woodshed-genet/src/persona.rs:333–348`; `crates/woodshed-genet/src/sync.rs:562–582`; `crates/woodshed-views/src/stage.rs:1330–1410`.

### H7 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed MIDI routing identity — B6.

Rescan replaces lists without rebinding positional dropdown selection to prior desired port identity.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Keep desired port identity canonical and expose disappearance.

**Current source locators:** `crates/woodshed-genet/src/sync.rs:25–32`; `crates/woodshed-genet/src/sync.rs:562–582`; `crates/woodshed-views/src/stage/settings.rs:70–82`.

### H8 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed MIDI worker admission — B6.

Byte-identical MIDI host publishes requested output name before worker open, suppresses connection errors via .ok and has no accepted-status result channel. sync reflects it as connected.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Separate desired and accepted identity and publish worker failures.

**Current source locators:** `crates/woodshed-genet/src/midi.rs:98–114`; `crates/woodshed-genet/src/midi.rs:192–225`; `crates/woodshed-genet/src/sync.rs:570–582`.

### H9 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed MIDI transport lifecycle — B6.

Unchanged worker replaces connection without old Stop/new Start while last_transport survives.

**Reachability:** worker message omission; physical MIDI effects unassessed. **Current qualification:** unassessed.

**Next:** Make transport state connection-specific, including disconnect and shutdown.

**Current source locators:** `crates/woodshed-genet/src/midi.rs:192–225`.

### H10 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed command compatibility / recorder — K1/D13.

Three legacy variants and sync arms remain; targeted crates/ports search finds no view producers. Active Recorder carries frozen authority. Busy guards advanced, so every old weaker-arm behavior is not carried forward.

**Reachability:** dormant public variants/executor; no current UI producer found. **Current qualification:** unassessed.

**Next:** Resolve external compatibility before retiring dormant variants and arms; preserve active LoopClear.

**Current source locators:** `crates/woodshed-core/src/audio.rs:215–226`; `crates/woodshed-genet/src/sync.rs:205–250`.

### H11 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed capture persistence status — K2.

awaiting_commit remains written and only read by combined is_busy; targeted combined-reader references are test-only. Production preservation reads worker/preparation and SessionStore facts.

**Reachability:** test-only combined status, not new preservation failure. **Current qualification:** unassessed.

**Next:** Remove unused combined status or assign a real production purpose; assert actual preservation outcomes.

**Current source locators:** `crates/woodshed-genet/src/capture_persistence.rs:39–65`; `crates/woodshed-genet/src/capture_persistence.rs:350–365`; `crates/woodshed-genet/src/capture_persistence.rs:545–553`; `crates/woodshed-genet/src/capture_persistence.rs:670–690`.

### V01 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed accessibility / motion — C2.

CSS reduction still uses copied incomplete overrides. Overview dynamics/atmosphere use overview_reduced_motion without application accessibility reduction.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Compose one effective application/local policy and cover actual transition selectors.

**Current source locators:** `crates/woodshed-views/src/theme.rs:705–720`; `crates/woodshed-views/src/appearance/view.rs:38–45`; `crates/woodshed-views/src/stage/overview.rs:345–382`.

### V02 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed render dispatch / animation — C2.

Both overview ticks still gate only on remembered Overview panel; host calls both. Legacy sections and relationship/replacement surfaces are absent from that predicate.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Share actual visible-surface predicate including replacement gates.

**Current source locators:** `crates/woodshed-views/src/stage/overview.rs:366–383`; `crates/woodshed-genet/src/main.rs:415–425`; `crates/woodshed-views/src/stage/relationship.rs:346–350`; `crates/woodshed-views/src/stage.rs:3500–3630`.

### V03 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed observed audio status views — B7.

Frozen-byte Settings/Tools infer both streams open from no error and listening from requested tuner.enabled. New consent worker has explicit lazy/pending/refused facts but those views do not project them.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Project observed unopened/pending/accepted/refused input/output states across audio surfaces.

**Current source locators:** `crates/woodshed-views/src/stage/settings.rs:191–195`; `crates/woodshed-views/src/stage/settings.rs:583–604`; `crates/woodshed-views/src/stage/tools.rs:71–105`; `crates/woodshed-genet/src/audio.rs:697–715`; `crates/woodshed-genet/src/audio/input_admission.rs:49–100`.

Historical receipt locators: `validation/coreaudio-input_20261017/receipt.json`. They do not certify HEAD.

### V04 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed process navigation — C2.

Tuner process Open still selects Tools without selecting ToolPage::Tuner.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Route to exact nested Tuner page from any previous tool.

**Current source locators:** `crates/woodshed-views/src/stage/overview.rs:520–541`; `crates/woodshed-views/src/stage/tools.rs:112–117`.

### V05 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed CSS/font selector cleanup — K2.

Retired Card/bar-chip selector blocks and mappings remain beside current tray/recorder owners. Retired-class emission is not established in current scoped paths.

**Reachability:** stylesheet generation; exhaustive current retired-DOM nonuse unassessed. **Current qualification:** unassessed.

**Next:** Complete current selector emitter inventory before removing orphan rules; preserve active editor selectors.

**Current source locators:** `crates/woodshed-views/src/theme.rs:448–487`; `crates/woodshed-views/src/theme.rs:596–620`; `crates/woodshed-views/src/appearance/fonts.rs:1–16`; `crates/woodshed-views/src/stage/set_tray.rs:150–175`.

### V06 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed provenance formatting — C2.

Rehearsal uses raw Song bar while tray displays bar+1; supported persisted/imported enum remains. New UI production not established.

**Reachability:** retained/imported supported Song recipe. **Current qualification:** unassessed.

**Next:** Share display formatter preserving stored zero-based source index.

**Current source locators:** `crates/woodshed-views/src/stage/rehearsal.rs:8–14`; `crates/woodshed-views/src/stage/set_tray.rs:47–55`.

### LO-1 — source replacement observed

**Owner / next package:** Woodshed bound preview / managed clock admission — P5a/P5 verification.

bound_card_preview checks underlying scale/approach/shape and unmarked availability. Empty admitted preview performs fallible output.cancel_preview. Managed lowering also carries unavailable/refusal. Frozen empty-success trigger has replacement source.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Keep historical qualification bound to candidates; assess managed silence policy separately.

**Current source locators:** `crates/woodshed-views/src/stage/timing.rs:8–60`; `crates/woodshed-genet/src/audio.rs:485–501`; `crates/woodshed-core/src/practice_clock.rs:130–180`.

Historical receipt locators: `validation/stage-p5a_20261016/receipt.json`, `validation/stage-p5_20261017/receipt.json`. They do not certify HEAD.

### LO-2 — source replacement observed

**Owner / next package:** Woodshed timing fixtures / clock owners — P5a/P5 fixture verification.

Inspected connected clock fixture now toggles Run before acknowledgement; request assertion precedes drain. rehearsal_dwell is cfg(test); production frame uses sync_practice_clock. Not all fixture bodies are requalified.

**Reachability:** fixtures and changed production clock path. **Current qualification:** unassessed.

**Next:** Do not assert frozen fixture failures at HEAD; use historical gates as locators and leave current suite unassessed.

**Current source locators:** `crates/woodshed-views/src/stage/connected.rs:300–347`; `crates/woodshed-genet/src/drive.rs:270–288`; `crates/woodshed-genet/src/drive.rs:133–140`.

Historical receipt locators: `validation/stage-p5a_20261016/receipt.json`, `validation/stage-p5_20261017/receipt.json`, `validation/stage-p6_20261017/receipt.json`. They do not certify HEAD.

### LO-K1 — source replacement observed

**Owner / next package:** Woodshed layout fixture owner — P5a fixture verification.

Former undefined pitches assertion now uses request.preview.0. Frozen compiler blocker is historical.

**Reachability:** test assertion source only. **Current qualification:** unassessed.

**Next:** Do not carry old compile blocker as active; HEAD compilation remains unassessed.

**Current source locators:** `crates/woodshed-genet/src/layout_tests.rs:1070–1084`.

Historical receipt locators: `validation/stage-p5a_20261016/receipt.json`. They do not certify HEAD.

### LO-3 — source replacement observed

**Owner / next package:** Woodshed audition / host / history — Catalog audition; D7 for wider semantics.

Inspected catalog/discovery sites queue request_observed_preview. One-use session ticket acknowledgement returns on error before Previewed write. Host uses operation result and refuses no-output.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Keep accepted control admission distinct from render/processor-start/physical sound and wider observation policy.

**Current source locators:** `crates/woodshed-views/src/stage/audition.rs:8–112`; `crates/woodshed-views/src/stage/connected.rs:65–85`; `crates/woodshed-genet/src/sync.rs:121–130`; `crates/woodshed-genet/src/sync.rs:299–308`; `crates/woodshed-genet/src/audio.rs:495–502`.

Historical receipt locators: `validation/catalog-audition_20261016/receipt.json`. They do not certify HEAD.

### LO-4 — source replacement observed

**Owner / next package:** Woodshed processor clock / generic audition — P5 managed timing; residual Walk Hear.

Managed lowering emits sequential events with Beats/Seconds even onsets; production uses sync_practice_clock and legacy dwell is test-only. Generic Path/Walk Hear still uses fixed 130ms voicing_shape; arpeggiate_preview transforms only Arpeggiate. Frozen dwell-truncation trigger cannot be carried forward wholesale.

**Reachability:** managed Run replaced; generic Path/Walk audition fixed-shape remains. **Current qualification:** unassessed.

**Next:** Keep managed Run replacement separate from residual generic Walk Hear parity; scope follow-up only if full timing agreement is promised.

**Current source locators:** `crates/woodshed-core/src/practice_clock.rs:167–235`; `crates/woodshed-core/src/lib.rs:702–708`; `crates/woodshed-core/src/lib.rs:1972–1993`; `crates/woodshed-core/src/connected_catalog.rs:106–115`; `crates/woodshed-genet/src/drive.rs:133–140`; `crates/woodshed-genet/src/drive.rs:270–278`.

Historical receipt locators: `validation/stage-p5_20261017/receipt.json`, `validation/stage-p6_20261017/receipt.json`. They do not certify HEAD.

### LO-5 — unchanged implementation/current reachability confirmed

**Owner / next package:** Woodshed Card realization — B1.

Same retained Path setup gap as CORE-4; managed Path also uses live tuning. Count once.

**Reachability:** production. **Current qualification:** unassessed.

**Next:** Track with CORE-4, not a second setup ticket.

**Current source locators:** `crates/woodshed-core/src/lib.rs:1972–1993`; `crates/woodshed-core/src/practice_clock.rs:267–286`.

## Named historical software/native qualification

These are named existing receipt claims, not newly verified acceptance. Source-hash manifests were sampled only for selected relevant files. The counts are byte comparisons, not full receipt/source/supplier closure. Ancestry is recorded in JSON and does not carry qualification forward. Raw logs, captures, independent reviews and evidence-helper correctness were not re-audited.

| Receipt | Named candidate | Selected hashes equal / changed | Recorded status |
|---|---|---:|---|
| stage-p5a_20261016 | `505fb12a223fdb034b21ffeafc150b90e1d2557c` | 23 / 29 | qualified |
| stage-p5_20261017 | `6f65f60f457337e2943346e8d537d5e0f21deac0` | 34 / 21 | qualified |
| stage-p6_20261017 | `d955f5dc3b99ec1efd6ec9a56dd3ba007755409e` | 40 / 15 | qualified |
| stage-p6-merged_20261017 | `39f1dabf6351ce554a2b26bc0487b297f7896a64` | 40 / 15 | qualified |
| stage-p7a_20261017 | `0fafd20a625ef915a5933678e21e43b9de48611e` | 40 / 15 | qualified-bounded-p7a |
| stage-p7b_20261017 | `1a2e9c26acc2b3d3c4264effb3ea322c1920346f` | 0 / 0 | implementation, affected CPU/build gates, native seed/fresh sealed reopen and separate WAV reference passed |
| canonical-clock_20261016 | `9d27b06b4864d3cee04a61b613c6974c4acfb63d` | 0 / 0 | see receipt fields |
| catalog-audition_20261016 | `bd8ff18dca0c2ae107ba1a7d0618a513d909e8cb` | 30 / 23 | qualified |
| looper-followups_20261017 | `e25f66860110215a7b3ebf07a7dda81da532c19b` | 0 / 0 | Software fixes and build passed; physical input admission times out natively; selected-Card native capture/reopen incomplete; isolated supplier scroll proof passed, production adoption pending |
| coreaudio-input_20261017 | `10486bcea3248ccd6adc80e55135435b9880ceca` | 50 / 6 | see receipt fields |

P5a names the repaired admission/timing candidate rather than the frozen LO-K1 failure. P5 names processor-clock Rehearsal; P6 and merged P6 name guided Rehearsal. P7a names mixed positive-Bars backing; P7b names the later Seconds slice. Those bounded candidates do not close full saved-song migration or PCM durability. Catalog-audition names the accepted-only history boundary. Canonical-clock is backing-clock evidence, not onset/calibration qualification.

The looper-followups machine receipt retains an incomplete physical-input attempt; its README points to later CoreAudio candidate `10486bcea3248ccd6adc80e55135435b9880ceca`. That later receipt names a 15.066362-second selected-Card input dwell, exact sealed reopening and native/reference WAV byte equality. It supersedes the earlier incomplete attempt for that historical bounded scenario. It does not establish HEAD device behavior, audibility or round-trip latency. The original machine receipt and later README publication status have different dates/states; neither is silently rewritten.

## Next open packages

| Package | Owner | IDs | Next bounded work |
|---|---|---|---|
| B1 | Card realization / retained recipe | CORE-4, CORE-6, LO-5 | Retained setup and contact/selection truth; count LO-5 once. |
| B2 | Saved-song editor | CORE-5 | Exact silent-toggle payload preservation. |
| B3 | Output processor capture scheduler | AUDIO-01 | Retain future saved-bar arm; distinguish selected pinning. |
| B4 | Prepared resources / audio-primitives | AUDIO-02..04 | Finite PCM, onset sample accounting, estimator support count. |
| B6 | Process MIDI/device host | H6..H9 | Inventory, identity, accepted worker status, connection Stop/Start. |
| B7 | Observed audio status views | V03 | Unopened/pending/refused/accepted projection. |
| C1 | Native paint/text adapters | H3, H4 | Complete invalidation and native fields. |
| C2 | View/render dispatch | V01,V02,V04,V06 | Effective motion, visibility, exact destination and provenance. |
| C3 | Reading/scene/disclosure | CORE-7, CORE-8 | Pitch motion placement and explicit Context filter. |
| D1/D7/D10 | Product contract owners | H2, CORE-3, CORE-C1 | Namespace migration, origin semantics, relation identity/strength. |
| K1/D13; K2 | API compatibility / cleanup owners | CORE-C2,AUDIO-05,H10,H11,V05 | External API decisions and current selector inventory before removal. |
| Q1 | Parent evidence/tooling owner | AUDIO-06 | Diagnostic terminal criterion; no helper/index work in this lane. |
| Residual timing | P5/audition owner | LO-4 | Generic Path/Walk Hear parity, separate from managed Run. |

B1–B4 are distinct material/runtime corrections. B6/B7 and C1–C3 remain host/view work. D1/D7/D10 require product contracts. Public compatibility cleanup must not discard SongDoc, active LoopClear or external API promises. Q1 is the parent evidence lane. Do not implement the historical LO-K1 typo again or repeat broad gates merely because an old audit recorded failure.

## Unassessed and parent handoff

runtime.json records every requested ID, current evidence locators, owners/packages, selected frozen/overlay/current hashes and sampled historical source identities. Full current semantic coverage, HEAD software/native/reopen/device/physical acceptance, uninspected fixture paths, external callers, exhaustive selector inventory and complete historical log/capture/supplier integrity remain unassessed.

Existing P5/P6/P7/audio receipts do not close unrelated finite-PCM, onset, calibration, MIDI, paint, native-text or view-policy findings. Parent can consume these two artifacts for its separate verification/indexing work. The frozen reports and their completed coverage remain untouched.
