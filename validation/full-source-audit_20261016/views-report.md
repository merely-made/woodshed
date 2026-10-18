# Woodshed views partition: complete frozen-source audit

Revision: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`.

Snapshot root: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.

**Coverage is complete: 37 of 37 assigned files, 23,962 of 23,962 lines, 953,742 bytes.** Every assigned file was read in full, including embedded tests, the separate test files, CSS, and Cargo configuration. All 37 SHA-256 hashes, line counts, and byte counts match `views-assigned.json`. The per-file hashes, read ranges, chunks, and completion states are in [views-coverage.json](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/views-coverage.json). Small tool-output omissions were explicitly reread before marking coverage complete.

This is a source audit. No builds, tests, native processes, device probes, network requests, or Git mutations were performed. Only this report and the coverage ledger were written. Neighboring frozen files were read to establish execution paths. The primary checkout was inspected read-only solely to distinguish the active P5a changes from the published snapshot.

I read snapshot `CLAUDE.md` and `design_docs/DOC_README.md` in full, the complete settings-persistence split plan, and relevant sections of the Stage plan covering source authority, P5a, Mere dynamics, atmosphere, and retained musical context. Historical acceptance descriptions inform intended boundaries; they are not new verification evidence.

## Findings at the frozen revision

There are three P2 findings and three P3 findings below. None is a claim of observed native failure. The execution paths and mismatched decisions are established by source. The remaining timing/admission concern is tracked separately under the known dirty P5a work.

| ID | Priority | Finding | Principal owner conflict |
|---|---|---|---|
| V01 | P2 | Reduce motion leaves common transitions active and does not dominate Mere simulation | Application accessibility policy versus copied selector list and scene-local policy |
| V02 | P2 | Hidden Mere simulations continue after opening legacy Tools/Looper or the relationship editor | Rendered surface versus remembered workspace tab |
| V03 | P2 | Audio surfaces report readiness from absence of error or requested tuner state | Backend observations versus view-inferred success |
| V04 | P3 | Opening the Mere Tuner process lands on whichever Tools page was previously selected | Process navigation target versus independent tool-page selection |
| V05 | P3 | Retired Card-overlay and song-timeline CSS remains in every generated theme | Current renderer structure versus orphan presentation rules |
| V06 | P3 | Song Card provenance shows two different bar numbers in two shared views | One zero-based source index versus duplicated display formatting |

### V01 — P2: Make one effective reduced-motion policy govern CSS and scene motion

**Trigger and effect.** Turn on Settings → Accessibility → Reduce motion. Hover or focus ordinary `.t-btn`, `.chrome-btn`, `.side-item`, `.select-box`, or `.select-option` controls. Their `0.12s` transitions remain enabled because the override selector list omits those five selectors. Independently, enable Mere scene motion and allow scene motion locally, then turn on the application Reduce motion preference. Arrangement and atmosphere continue advancing: the native simulation paths consult only `overview_reduced_motion`.

**Exact snapshot evidence.**

- [theme.rs:616](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/theme.rs:616), lines 616–620, defines the transition families; [theme.rs:695](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/theme.rs:695), lines 695–705, contains a manually copied, incomplete override list.
- [settings.rs:701](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/settings.rs:701), lines 701–703, writes the application preference; [appearance/view.rs:40](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/appearance/view.rs:40), lines 40–44, passes it only to stylesheet post-processing.
- [overview.rs:348](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/overview.rs:348), lines 348–357, supplies scene-local `overview_reduced_motion` to PhysicsBoard; lines 370–377 use the same local field for atmosphere. Lines 1069–1082 expose a separate local toggle.
- [woodshed-genet/main.rs:353](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/main.rs:353), lines 353–356, invokes both simulation ticks without combining application accessibility policy.

**Authority owner / competing owner.** `AppSettings.accessibility.reduce_motion` owns the application accessibility request. A scene-local motion preference may further restrict animation, but should not defeat that request. The incomplete CSS selector copy and the independent overview flag currently each decide a subset of effective motion.

**Tests and gap.** The complete assigned `theme.rs` has no tests for computed transitions. `overview_dynamics.rs:348–362`, `overview_atmosphere.rs:105–123`, and `overview.rs:1576–1600` test the local reduced-motion flag, not the application flag. Their existing guarantees remain valid; they do not cover this cross-policy combination.

**Smallest coherent consolidation.** Define effective scene reduction as application reduction OR scene reduction, and use it for both shared simulations. Derive the CSS override from the same transition selector source used to emit transitions, or apply one adequately scoped reduction rule that covers the product controls and authored appearance. Verify a mounted ordinary button/selector and a scene with local motion allowed while application reduction is on. This removes duplicated policy; it does not require replacing PhysicsBoard or the atmosphere implementations.

**Dirty P5a status.** Still present in the primary source inspected during this audit; the relevant functions are unchanged there.

### V02 — P2: Gate animation with the surface that is actually rendered

**Trigger and effect.** In Mere, select Cells or Orbits, enable motion, and allow scene motion. Then select the Looper or Tools legacy pill, or open Relationship recipe. The remembered workspace tab is still Overview. Both overview tick methods continue running and requesting frames even though `tab_content` renders a different surface. The hidden graph can move, later session capture can retain those changed positions, and the app keeps doing full UI/leaf work for offscreen animation.

**Exact snapshot evidence.**

- [stage.rs:1442](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage.rs:1442), lines 1442–1455: Looper and Tools update only `section`, retaining `workspace.active_panel()`.
- [stage.rs:3488](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage.rs:3488), lines 3488–3501: render selection uses `section` and, for Overview, `relationship_open`.
- [relationship.rs:346](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/relationship.rs:346), lines 346–350: opening the relationship editor retains the Overview panel and sets `relationship_open`.
- [overview.rs:361](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/overview.rs:361), lines 361–377: animation checks only the remembered panel, ignoring both `section` and `relationship_open`. Persona and appearance gates also render in place of the product at `stage.rs:3552–3560`.
- [woodshed-genet/main.rs:350](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/main.rs:350), lines 350–372: the actual frame hook calls both methods and returns their animation demand; there is no outer visibility guard.
- `overview_presentation.rs:64–82` overlays runtime dynamics positions into the saved presentation. The Stage plan explicitly says hidden Mere views do not advance simulations at `design_docs/2026-07-11_stage_set_tools_plan.md:2614–2620`.

**Authority owner / competing owner.** `tab_content` plus the persona/workshop gates own actual visible content. `workspace.active_panel` is remembered navigation state and cannot alone authorize visible-scene work while legacy sections and subviews coexist.

**Tests and gap.** `overview.rs:1576–1600` checks hiding Mere by activating Practice, which changes the workspace tab. `stage.rs:3942–3958` intentionally demonstrates that selecting Looper leaves the previous workspace tab in place. Neither test composes those behaviors. Relationship editor visibility and modal gates are likewise absent from the animation-gating tests.

**Smallest coherent consolidation.** Add one `overview_scene_visible()` predicate that follows the render dispatch, including Stage section, Overview tab, closed relationship editor, and absent persona/workshop replacement. Reuse it in both ticks and any visibility-dependent leaf work. Preserve the remembered workspace selection if desired; it does not need to double as visibility authority. A small state-transition regression for Overview → Looper, Tools, Relationship, and replacement gates is sufficient before any native qualification.

**Dirty P5a status.** Still present; primary `overview.rs:361–377` matches this behavior.

### V03 — P2: Project accepted audio availability into every audio surface

**Trigger and effect.** On an ordinary successful desktop launch, open Settings → General before using any input feature. It says “Audio: output and input streams open,” although input is deliberately unopened. On a machine whose microphone open fails, turn on the tuner from Tools. The tool continues saying “Listening…” and offers “Stop listening”; Settings → Tuner similarly says “Listening: on.” Those surfaces do not show `audio_error`, even though the backend has explicitly reported failure and will retry only after an explicit disable/reenable. A device-free host with no backend also has `audio_error == None` and gets the same General success sentence.

**Exact snapshot evidence.**

- [settings.rs:191](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/settings.rs:191), lines 191–195, infers both-open from `audio_error == None`.
- [tools.rs:71](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/tools.rs:71), lines 71–105, reads only `tuner.reading` and requested `tuner.enabled`; [settings.rs:583](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/settings.rs:583), lines 583–604, does the same for “Listening: on.”
- [woodshed-genet/audio.rs:485](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/audio.rs:485), lines 485–518, explicitly opens output at startup and sets `input: None`; lines 542–566 report an input-open error, and lines 661–665 retain requested tuner state after refusal.
- [woodshed-genet/sync.rs:114](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/sync.rs:114), line 114, sends the desired state; lines 278–281 reflect error and recorder observations but do not convert the desired flag into an accepted tuner status.
- `woodshed-genet/drive.rs:66–75` likewise reflects errors without changing `tuner.enabled`; lines 134–136 continue polling a requested tuner with no reading.
- The same views already have an honest alternative: `stage.rs:2616–2629` prioritizes `audio_error`, while `stage/looper/recorder.rs:344–356` distinguishes faults, unopened input, and opened configuration. `woodshed-genet/audio.rs:263–270` supplies that observed distinction.

**Authority owner / competing owner.** CpalBackend owns device/session facts. Requested tuner state is user intent. General settings and the two tuner surfaces currently invent effective readiness separately, while Stage transport and the shared recorder use different, more accurate rules.

**Tests and gap.** `woodshed-genet/audio.rs:1079–1104` tests exactly the failed-enable/explicit-retry backend path and checks that the error is visible through the backend API. It does not mount Tools or Settings, so it cannot establish that the user sees the refusal there. Assigned settings tests cover persona text; there are no assigned Tools tests.

**Smallest coherent consolidation.** Feed a small host-observed availability/status value to the views and share the status formatter across General, Stage, and Tools/Settings tuner controls. Keep unopened input distinct from unavailable input and keep desired enabled distinct from accepted listening. The existing recorder observation can inform the shape, but should not become a new device owner. At minimum, surface the existing error in tuner views and stop claiming both streams are open when all that is known is “no reported error.”

**Dirty P5a status.** Still present in primary Settings source; P5a's Card admission work does not address tuner/device readiness.

### V04 — P3: Tuner process navigation must select the Tuner page

**Trigger and effect.** Enable the tuner, visit Mere, select its Tuner process, and choose Open view while Tools last showed Fretboard or Metronome. The app goes to that unrelated Tools page. On a new session the default is Fretboard.

**Exact snapshot evidence.** [overview.rs:524](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/overview.rs:524), lines 524–526, selects only `AppSection::Tools`. [stage.rs:838](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage.rs:838), lines 838–844, defaults `ToolPage` to Fretboard, and lines 1345–1347 initialize it. [tools.rs:112](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/tools.rs:112), lines 112–117, chooses the displayed tool from that independent field.

**Authority owner / competing owner.** The process identity is the intended destination, but the retained nested Tools-page selection wins. This is a routing mismatch, not a reason to move tuner runtime ownership into the graph.

**Tests and gap.** `overview.rs:1602–1624` exercises the exact Tuner navigation action but asserts only `section == Tools`, not `tool_page == Tuner` or mounted tuner controls. Its success assertion therefore misses the wrong destination.

**Smallest coherent consolidation.** Route through one explicit `open_tool(ToolPage::Tuner)` action, or set the nested page alongside the section. Extend the existing navigation test to check the mounted tuner control from both other tool pages while preserving live process state.

**Dirty P5a status.** No relevant fix observed.

### V05 — P3: Remove unreachable presentation branches after renderer replacement

**Trigger and effect.** Every appearance resolution emits CSS for a retired in-canvas Card editor and retired song bar-chip timeline. Current views do not create these classes. There is no immediate musical-state defect, but the obsolete rules keep a second description of layout, click targets, and theme/font mappings alive, increasing maintenance ambiguity and stylesheet size.

**Exact snapshot evidence.** [theme.rs:448](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/theme.rs:448), line 448, and lines 483–487 define `.set-graph-card-root`, `.set-graph-node-card-layer`, and `.set-graph-node-card` descendants. Lines 596–604 define `.bar-lane`, `.bar-chip`, `.bar-current`, `.bar-edit`, and bar text styling. Line 619 and lines 703–704 retain the retired bar selector in motion policies. `appearance/fonts.rs:11` also names the old bar label classes. Repository-wide searches over the frozen source found no class-emitting callers for these retired structures.

**Authority owner / competing owner.** `stage/set_tray.rs:150–166` mounts the current selected editor beside the graph using `.set-graph-selected-card`; `stage.rs:4407–4410` explicitly tests that this editor reserves no in-canvas footprint. `stage/looper.rs:171–180` mounts the shared recorder instead of emitting bar chips. Those renderers own current structure; orphan CSS describes prior structure.

**Tests and gap.** Existing view tests and source searches establish the new structures. No runtime consumer for the old selectors was found. This is a cleanup finding with no active user execution path beyond stylesheet generation, not a claim that old hidden UI is still running.

**Smallest coherent removal.** Delete the orphan rule blocks and their selector-only font/motion mappings after a final selector inventory. Keep `.graph-card-collapse` and `.set-graph-selected-card`, which have current callers. Do not remove `demo.rs`: it has a real browser-host caller, discussed below.

**Dirty P5a status.** No relevant fix observed.

### V06 — P3: Share Song provenance formatting between Card views

**Trigger and effect.** Reopen an existing/imported Card with `from: Recipe::Song { name, bar: 0 }`. Stage's Set tray displays “bar 1,” while the Rehearsal filmstrip displays “bar 0” for the same Card. Later source bars remain offset by one. The snapshot has no current production constructor for Song-derived Cards; the trigger is the persisted supported variant, which the derived serde model still accepts.

**Exact snapshot evidence.** [stage/rehearsal.rs:8](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/rehearsal.rs:8), lines 8–14, formats the raw index. [stage/set_tray.rs:47](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/set_tray.rs:47), lines 47–53, formats `bar + 1`. The domain owner documents `bar` as a source bar index at `crates/woodshedding/src/rehearsal.rs:199–218`. The filmstrip calls its formatter at `stage/rehearsal.rs:141–145`, and the tray calls its formatter at `stage/set_tray.rs:206`.

**Authority owner / competing owner.** Recipe owns the zero-based source index. Two product views separately translate it into user-facing provenance and have drifted. Do not change the stored index.

**Tests and gap.** No assigned test compares provenance text across these views. The supported serialized enum establishes legacy/import reachability; current UI does not generate new instances of this recipe variant.

**Smallest coherent consolidation.** Use one product-level provenance formatter in both Card presentations, with a single regression for source indices zero and one. Retain the domain enum and existing persisted values.

**Dirty P5a status.** No relevant fix observed.

## Known dirty P5a: separate from new findings

The frozen snapshot opens rehearsal observations in `stage.rs:2262–2299` before the host admits audio and records Card preview interest before dispatch in `stage.rs:2176–2193`. Its UI tests can credit rehearsal by toggling state without a backend. This is part of the already identified timing/admission frontier; it is not counted above as a new unresolved finding.

The primary checkout was read without modification. Its dirty `stage.rs` introduces pending rehearsal/admission state, queues frozen `CardPreviewRequest` values, and delegates confirmation to the new `stage/timing.rs`. That file's lines 12–23 validate session/owner/instruction/tempo/epoch; lines 25–50 open observation only on a current successful outcome; lines 53–63 stop a refused run without creating an observed span. Current UI tests explicitly acknowledge a test admission seam. The neighboring primary diff routes the dwell and ordered-preview calculations through a fallible shared timing contract.

Disposition: **source fix in progress / present in dirty source, not independently qualified by this audit**. I did not build, execute tests, inspect a native run, or certify the whole P5a diff. It would be incorrect to report the frozen admission problem as an untouched new issue, and equally incorrect to declare P5a complete from this read-only inspection. The broad “Previewed means request interest versus accepted playback” semantics remain worth confirming across discovery/context paths, which still have their own observation sites; no acoustic-success claim is inferred here.

## Deliberate boundaries and false-positive rebuttals

- **No second Hocket document authority in the Woodshed recorder.** `stage/looper/recorder.rs` builds transient projections from admitted Woodshed `LoopPlan`/`SongDoc` and immutable PCM handles. It uses `hocket_views::projected_surface` and `hocket_recorder::host_policy`; it does not persist an Hocket Session or open an independent engine. Session/source/backing/target and capture-epoch guards have meaningful stale-action tests. `song_recording` is reflected from `backend.loop_capture_busy()` (`woodshed-genet/drive.rs:66–75`, `sync.rs:278`), so its use includes armed capture: a suspected “Stop misses armed capture” mismatch is rebutted by the caller.
- **The active Set plus WorkingSets bank is intentional ownership, not two writable copies of one active Set.** Views use core bank operations to park and activate owners. Tests in `stage/instances.rs` preserve a background runner, same-number Card identity, and saved presentation. The one active Set slot is explicitly supplied to bank accessors.
- **Retained Set snapshots and captured recipes are intentional authored copies.** They have explicit creation/open/add actions and preserve source provenance. Ordinary graph inspection does not author them. Musical-context tests cover source removal, saved setup, new occurrences, and owner separation.
- **RelationshipReading retains historical source facts by design.** It uses the shared Scenomise compiler, keeps invalid optional payloads, and checks owner, full ordered instruction revision, and exact disclosed dataset before source navigation. This is different from silently recomputing a saved reading. The view's JSON envelope is a product-owned persistence boundary rather than an independent generic graph database.
- **Theme seeds and selector-role mapping are legitimate product authority.** Tabard supplies the shared definition/editor/derivation machinery; Woodshed preserves its named themes and maps roles onto product selectors. Workshop preview and application selection have separate persistence-aware tests. The issue is the duplicated reduction policy in V01, not the existence of a host appearance adapter.
- **The static demo is reachable, not safely removable cruft.** `woodshed-web/src/lib.rs:46,78–87` mounts `demo_root` and `DEMO_SHEET`; its module docs at lines 17–29 explicitly disclose the S0-only browser boundary. Old Practice/Song labels in the demo are historical smoke content, not proof that the browser runs the current product. `woodshed-views/src/lib.rs:8–10` is stale about the S2 retirement deadline, but removal must wait for replacement of that real caller.
- **A suspected cross-persona leftover inspector leak is not established.** `UiState::apply_persisted` does not itself clear every transient claim field, but native persona switching first replaces the entire UiState in `woodshed-genet/persona.rs:202–216`, preserving only the process-wide appearance library. That caller rebuts a native persona-leak claim from the restore helper alone.
- **Workspace tree mechanics are shared, with product-owned DTO validation.** The strip currently renders one selected surface; split/tear-out capabilities in its model are not evidence of visible multi-pane UI. Public event mechanics should not be described as native split-window qualification. No user-facing split/tear-out caller was found in the assigned views.

## Additional consolidation candidates and unresolved questions

1. **Unused settings-provider integration.** `settings_provider.rs` is exported, but its only frozen source consumers are its own tests. Settings views write AppSettings directly and appearance uses `commit_selection`. Its named-theme mutation duplicates `UiState::set_theme` semantics and bypasses the appearance transaction if a future caller installs it carelessly. Decide whether a real shared Settings host will consume it; otherwise remove the unused adapter/dependencies. This is not an active duplicate write authority today because no product caller reaches it.
2. **Repeated claims projection during rendering.** `stage.rs:155–178` and `740–752` call `claims_link_reading_for_pair` separately for resting links. `related.rs:42–74` obtains a new depth-one `woodshed_mere` snapshot each time. The core function clones catalog material metadata, aggregates history transitions, and constructs nodes/relations (`woodshed-core/mere.rs:60–140`); catalog relations themselves are already indexed/cached (`woodshed-graph/lib.rs:803–867`). Reuse a render-scoped source reading/index keyed to the actual history/personal-source revision rather than repeatedly rebuilding it per pair, while keeping exact action-time revalidation. This is a source-proven multiplicative work pattern, but no latency regression or benchmark threshold was measured, so it is not promoted to a performance-severity finding.
3. **Geometry-only evidence has a bounded claim.** `canvas_camera.rs:568–603` compares faces and bodies derived from the same swatch. This checks conversion consistency and preservation of Set truth, not actual host event targeting or physical/native paint alignment. The test does not claim otherwise; keep that boundary when consolidating receipts. Likewise recorder mount tests assert queued commands, not successful recording or WAV output.
4. **Observation semantics outside P5a.** Discovery/context preview paths record `Previewed` before host execution, and generic context audition sometimes queues without recording an observation. This may intentionally represent engagement interest. A product ruling is needed before calling either an acoustic-success defect or demanding that every preview produce an accepted-playback observation. Rehearsal measured-span admission is the already active P5a issue.
5. **No native or physical qualification inferred.** The findings need focused fixes/tests and then the owning lane's appropriate acceptance. This report does not reopen or negate earlier exact-revision receipts, and does not assert that a source-only fix is published or device-qualified.

## Coverage receipt

The ledger records each assigned relative path, original and actual SHA-256, expected and actual bytes/lines, complete read ranges, and status. Aggregate verification at completion returned:

```json
{"assigned_files":37,"assigned_lines":23962,"complete_files":37,"read_lines":23962,"all_hashes_match":true}
```

No assigned files or lines remain unread. The questions above concern design disposition and unrun validation, not missing source coverage.
