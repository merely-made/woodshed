# Redshank full-source audit

Frozen Woodshed revision: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`.

Snapshot root: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.

**Coverage complete: 178/178 assigned files, 51,557/51,557 lines, 1,712,889 bytes. All 178 actual SHA-256 values and line counts match the assignment.** The machine-readable [coverage receipt](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/redshank-coverage.json) records each exact successful read range, file hash, status, and reviewer. This was a collective literal read, including every test, vendored schema/fixture, configuration, script, CSS file, and both lockfiles. Grep, hashing and summaries supplemented that read; they did not replace it.

The primary reviewer read 47 files / 11,483 lines; the desktop lane read 13 / 13,414; model/playback read 77 / 13,824; surfaces/storage/web read 41 / 12,836. Three private lanes were used within the human-authorized parallel review. Initially truncated discovery outputs were excluded; the clipped combined `tabs/mod.rs` output was replaced by a dedicated complete read. No assigned ranges remain unread.

This is a read-only architecture/correctness audit. No builds, tests, native processes, network browsing, source edits, lock changes, commits or Git mutations were performed. The only written files are this report and its coverage receipt. “Confirmed” below means source reachability and the described mechanism were established; no new runtime or physical-audio acceptance is claimed.

All source references below resolve to this frozen snapshot. A read-only SHA-256 comparison of all 178 assigned paths in `/Users/markik/Code/repos/woodshed` found no differences at comparison time. **No finding below is already fixed by the known dirty local P5a work.** The local P5a plan concerns Card timing admission in Woodshed, outside this independent Redshank partition.

## Result and priority

The port boundary is substantially deliberate: one durable listening model, host-owned devices/fetch/custody, a reusable surface, and independent desktop/web composition. The material defects cluster where projections or adapters invent facts that should come from those owners: empty-state recovery, effective gain/rate, pending worker activity, gesture lifetime, representation identity, and item-scoped navigation.

| ID | Priority | Finding |
|---|---|---|
| R01 | P1 | Failed restore admits an empty writable model over existing listener data |
| R02 | P2 | Voice preview can initialize output at full gain despite listener mute |
| R03 | P2 | Worker recovery resets actual controls while keeping old published values |
| R04 | P2 | Native idle scheduling omits alignment/export jobs that scenarios recognize |
| R05 | P2 | Dock recording replaces the pointer target before release can save |
| R06 | P2 | Cached-integrity rejection panics through a nested mutable borrow |
| R07 | P2 | Seeking to exact EOF can leave transport permanently buffering |
| R08 | P2 | Sanitized export identities collide and silently overwrite another export |
| R09 | P2 | Voice capture silently drops everything after ten minutes |
| R10 | P2 | Explicit Local audio selection is overridden by feed fallback |
| R11 | P2 | Landscape rail has a visual/ARIA Seek slider with no seek behavior |
| R12 | P2 | Connections card claims cached copies match without identity evidence |
| R13 | P2 | All notes flattens different items into the selected item's timeline |
| R14 | P3 | Subscription receipt requires history retention the store deliberately removed |
| R15 | P3 | Trail labels all retained listening time as this week |
| R16 | P3 | Production HTTP source retains an unbounded test-only range history |
| R17 | P3 | Rate changes bypass the admitted controller rate authority |

P1 identifies the destructive recovery-admission risk; P2 identifies reachable wrong behavior; P3 identifies stale validation, misleading aggregates or bounded architectural cleanup. There is no P0 finding.

## Findings

### R01 — Preserve existing state when restoration cannot establish a model [P1]

- **Trigger/effect:** All published `state-*.json` files fail model deserialization, or store restoration returns an I/O error. Startup creates a default listener and starts writable persistence over that same directory. All-corrupt generations produce no notice at all. Startup alone is not a write: revisions start at 0/0. The next persistent action publishes a default-derived model. In the all-corrupt case, pruning then deletes the old malformed evidence. The error branch displays a notice but likewise authorizes subsequent writes from a replacement model.
- **Reachable evidence:** [storage/src/lib.rs:145](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/storage/src/lib.rs:145), lines 145–156, returns `Ok(None)` after exhausting invalid generations; [desktop/src/main.rs:2270](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/desktop/src/main.rs:2270), lines 2270–2282, defaults and starts `Persistence` on the same store. Main lines 233–247 and 258–285 admit dirty revisions and save them; storage lines 109–129 and 159–182 publish then remove invalid older files.
- **Authority:** Storage owns absent-versus-unrecoverable state. Startup turns both into authority to create a new writable listener. A transient inability to restore must not confer replacement authority.
- **Tests/gap:** Storage lines 420–428 and 430–461 exercise fallback while a valid prior exists; 520–530 exercise cleanup I/O failure with a readable replacement. There is no all-invalid/failed-restore boot-and-edit test proving preservation and write refusal. The retention plan, listening plan lines 1615–1637, does not endorse all-corrupt-as-empty semantics.
- **Smallest coherent direction:** Return distinct Empty/Loaded/NoValidGeneration outcomes and keep failed-recovery sessions non-writing until explicit repair or reset. Carry that admission state into the host rather than relying on a notice beside an ordinary writable default.

### R02 — Put desired output gain in the shared output owner [P2]

- **Trigger/effect:** Set volume to zero while no `AudioRuntime` exists, then audition a retained voice blob whose original episode is missing/unavailable. The preview's independent backend initializes the one shared runtime at its own default volume 100. Playback can be audible at full gain while the dock still reports volume 0.
- **Reachable evidence:** [playback/src/worker.rs:305](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/worker.rs:305), lines 305–308, shares the audio handle but not desired gain; [playback/src/backend.rs:407](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/backend.rs:407), lines 407–409, supplies defaults, 613–638 owns separate backend gain, and 765–769 applies the creating backend's gain to new output. Desktop main lines 1284–1304 dispatches saved voice playback without a successfully loaded episode; session lines 353–354 projects the main snapshot's volume.
- **Authority:** Listener volume and the shared output are the owners. Preview-local default gain is a competing owner of the same audible setting. Separate episode/preview controllers are legitimate; their separate desired output gain is the defect.
- **Tests/gap:** Playback `voice_preview_preserves_the_loaded_episode_snapshot`, lib.rs 743–778, uses `redshank-test://`, never creates output, and does not test mute inheritance. Stream mechanics tests do not establish episode/preview gain ownership.
- **Smallest coherent direction:** Retain desired gain in the shared audio owner even before opening a device. At minimum initialize preview gain from the accepted main gain before creating output. Keep the deliberate unity-rate voice preview policy separate.

### R03 — Restore accepted controls when recreating a worker session [P2]

- **Trigger/effect:** Set rate/volume to nondefault values, encounter an internal worker panic, then select a recording to retry. The new backend and `Levels` return to 100/100/no duck, while the surviving snapshot keeps the previous fields. The next recording can play at 100/100 while the dock still reports 150/40, for example. R06 supplies an ordinary malformed-cache trigger for that recovery path.
- **Reachable evidence:** [worker.rs:507](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/worker.rs:507), lines 507–511, recreates backend/controller/levels; lines 57–70 inherit old snapshot controls, and 861–891 retain that cell across recovery. Backend lines 407–409 define actual defaults. Stored settings are sent once at desktop main 2339–2345; ordinary selection sends only `Load` at session 182–186.
- **Authority:** Persisted settings, worker `Levels`, backend control fields and retained snapshots are separate copies of one effective-control contract. Recovery exposes their disagreement.
- **Tests/gap:** Playback lib.rs 780–806 tests recovery with default controls; 697–741 tests controls without recovery. Neither crosses those paths or checks effective output against the reported state.
- **Smallest coherent direction:** Retain accepted desired configuration outside the recoverable session, apply it before any recreated backend creates output, then publish effective backend values. Explicitly settle capture duck state after recovery.

### R04 — Use one pending-job inventory for native scheduling and scripted waits [P2]

- **Trigger/effect:** Start Realign on a paused downloaded episode, or export annotations while paused. Once input-driven redraws settle, a completed reply can remain in a plain channel until another UI/audio event. The UI remains searching/exporting; another request is refused, and alignment is not admitted/persisted yet.
- **Reachable evidence:** [desktop/main.rs:2169](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/desktop/src/main.rs:2169), lines 2169–2183, omits `alignment_pending` and `export_pending`; `scenario_busy`, lines 404–420, includes them. Workers 1060–1070 and 886–898 only `mpsc::send`; only playback gets the host wake callback at 2375–2377. Realign is exposed by surfaces/tabs/rows.rs 261–268, export by Notes and main's Ctrl/Cmd+E route 2027–2032.
- **Pinned-supplier check:** Mere `91467c0bbf6f92cc26262579dec634155ee33f2d`, verified in the Cargo checkout, returns the application frame hook's bool at `cambium-rootstock/src/frame.rs` 222–270, combines only layout/CSS animations in `presentation/prepare.rs` 81,104,196, requests redraw at frame 831–834, and falls back to Wait at `host.rs` 1989–1998. Winit-host lib.rs 1269–1275,1293–1294 maps it to `ControlFlow::Wait`. Redshank's only animation is the recording pulse, CSS 423–429. Unchanged paused playback does not wake, worker.rs 72–90.
- **Authority/tests:** Two independent host-work enumerations compete. Main's `scenario_wait_covers_asynchronous_product_work`, 3061–3086, tests the more complete scenario predicate; active scenarios/headed receipts keep frames flowing and can conceal the native omission. No native idle-worker completion test exists.
- **Smallest coherent direction:** Wake the host on every worker completion, or derive both scheduling decisions from one job inventory while preserving the scenario rule that ordinary listening does not block quiescence forever.

### R05 — Keep the voice gesture owner alive until release [P2]

- **Trigger/effect:** Press the dock voice button, allow active recording projection, then release. Recording continues because the dock replaced the original capture group and removed its pointer-up handler. Explicit Finish/Cancel remains available, but the advertised release-to-save gesture no longer completes.
- **Reachable evidence:** [surfaces/dock.rs:369](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/surfaces/src/dock.rs:369), lines 369–415, owns the pointer pair; 477–485 removes it when `recording` exists. Desktop main 735–774 begins capture; 535–554 supplies `recording: Some`; dispatch/project 1592–1606 replaces the UI immediately. Hooks 2100–2253 contain no global release route. Rail 179–190 keeps its capture button mounted.
- **Pinned-supplier check:** Exact Mere `91467c0bbf6f92cc26262579dec634155ee33f2d` Cambium `pointer.rs` 256–265 unregisters the old handler during teardown; `runner.rs` 221–226 and 292–300 clear dead captures after rebuild; 895–914 routes pointer-up only to a surviving capture.
- **Authority/tests:** The capture gesture owns press/release; visual recording-state replacement destroys its owner. Dock test 670–703 exercises only `capture_surface`, whose group is never replaced, not the full dock round trip.
- **Smallest coherent direction:** Retain a stable captured node or ancestor until release, sharing this lifecycle between dock and rail. Test press → host recording projection → release on `compact_surface`.

### R06 — Drop cached-admission's mutable borrow before handling refusal [P2]

- **Trigger/effect:** Select cached audio still decodable but changed from the stored digest/length. Its intended integrity refusal instead causes a `RefCell` panic and generic internal-failure recovery, discarding the useful diagnostic and recreating playback state.
- **Reachable evidence:** [worker.rs:561](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/worker.rs:561), lines 561–566, calls `fail` while the condition's `borrow_mut()` temporary remains alive. `fail`, 452–466, signals the controller and borrows the backend again. Controller adapter 55–56 reborrows via `set_playing`; outer recovery 861–891 masks the integrity outcome. Session selection 150–186 sends the cached source unchanged; backend 664–685 performs receipt admission.
- **Authority/tests:** The integrity-admission result should govern the refusal. An error-handling borrow lifetime instead activates the unrelated panic-recovery envelope. Lib.rs 303–344 tests cached refusal directly against the backend and bypasses this worker branch. Preview handling already stores `cached_admission` before its failure call at worker 343–351.
- **Smallest coherent direction:** Materialize the admission result in a local/inner block so its borrow ends before `fail`, matching the preview and pump patterns.

### R07 — Admit EOF before first-frame readiness as a terminal fact [P2]

- **Trigger/effect:** Drag a normal WAV's seek track to its exact right endpoint, or reopen at saved exact duration. Seek clears first-frame state; the next decode yields EOF without a frame. Every pump returns Idle, so transport remains Buffering indefinitely. A zero-audio admitted source shares the hole.
- **Reachable evidence:** [backend.rs:554](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/backend.rs:554), lines 554–560, resets decode state; 754–758 returns Idle before considering EOF; 884 sets EOF. Worker 761–768 ignores Idle for the buffering controller; session 374–380 projects Loading as Buffering. Dock 242–248 emits `Seek(fraction*total)`, including equality, through main 1504–1508.
- **Supplier/authority:** Locked Genet `e37762d` controller.rs 232–244 permits equality with duration. Locked Symphonia RIFF 0.5.5 wave/mod.rs 206–246 permits equality and common.rs 337–342 yields EOF. Backend EOF never reaches the controller's terminal authority because readiness incorrectly requires another decoded frame.
- **Tests/gap:** Accurate-seek tests, backend 1023–1058, stop at 5,123 ms of a six-second WAV; output 398–469 begins with 37 frames, not zero remaining frames.
- **Smallest coherent direction:** Handle first-decode EOF explicitly without creating an audio sink merely to end. Preserve coherent terminal position/state for end seeks and explicit terminal/unavailable treatment for unusable sources.

### R08 — Preserve full item identity in annotation-export destinations [P2]

- **Trigger/effect:** Export two different items whose IDs differ only by characters replaced with `-`, such as `/tmp/a/b.mp3` and `/tmp/a-b.mp3`. The second export silently replaces the first export's JSON and reports success.
- **Reachable evidence:** [desktop/main.rs:869](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/desktop/src/main.rs:869), lines 869–902, derives a path and calls overwriting `fs::write` at 892. `export_stem`, 1901–1918, is a many-to-one sanitizer. Local IDs use full paths at 613–638. Export is exposed in Notes and via Ctrl/Cmd+E.
- **Authority/tests:** The model's complete ItemId owns identity. A readable sanitized stem becomes competing filesystem identity. No desktop collision/two-item export test exists; W3C serialization tests do not cover destination admission.
- **Smallest coherent direction:** Use a bounded readable stem plus a digest of the complete ItemId, or a caller-chosen destination with explicit replacement semantics. Destination authority remains in the host.

### R09 — Report capture capacity instead of silently discarding the tail [P2]

- **Trigger/effect:** Record longer than ten minutes. The callback stops accepting samples at the cap but leaves capture active. The UI wall-clock duration keeps increasing; Finish saves only ten minutes and reports success without disclosing the lost tail.
- **Reachable evidence:** [desktop/voice.rs:102](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/desktop/src/voice.rs:102), lines 16,102–107 define/allocate the cap; 177–188 silently break; 134–152 successfully writes the accumulated body. Main 535–543 projects elapsed wall time, 777–803 accepts the shortened body, 1781–1786 announces saved.
- **Authority/tests:** Accepted sample count owns captured duration. Wall-clock `VoiceSession` state independently claims continued capture while the adapter withholds its capacity result. Voice test 270–287 covers only short content addressing/path confinement.
- **Smallest coherent direction:** Return limit-reached/accepted-frame facts to the host, automatically stop or clearly expose refusal, and project accepted duration. A shared accumulator can own bounded acceptance mechanics while Redshank retains capture policy and custody.

### R10 — Separate explicit local scope from automatic feed fallback [P2]

- **Trigger/effect:** With at least one subscription, click Library → Local audio. `None` is stored but interpreted as the playing feed or first feed, so the local list cannot be selected through that row. Local files remain accessible through other paths; this finding is about the broken Library scope action.
- **Reachable evidence:** [tabs/library.rs:86](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/surfaces/src/tabs/library.rs:86), lines 86–103, emits `SelectFeed(None)`; surfaces lib.rs 690–701 applies fallback; Library 341–387 only shows local items when `current_feed()` is None. Desktop main 1577 and web lib.rs 293 apply the command literally.
- **Authority/tests:** Explicit navigation competes with automatic defaulting; scene/mod.rs 18–24 also interprets None as overview. Library test 591–603 checks command emission, not apply-and-rerender behavior.
- **Smallest coherent direction:** Model Auto/Overview, Local and Feed explicitly or remove fallback from explicit Library selection.

### R11 — Share seek interaction across dock and rail [P2]

- **Trigger/effect:** Enter the landscape rail band, e.g. 820×560, and click/drag its vertical Seek track. It emits no seek command and lacks focusability, despite declaring `role=slider`.
- **Reachable evidence:** [rail.rs:15](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/surfaces/src/rail.rs:15), lines 15–89, has no pointer/key handlers or tabindex. Dock 180–252 implements pointer seeking. Desktop main 2091–2097 and web lib.rs 584–592 automatically select Rail.
- **Authority/tests:** A copied rail visual/ARIA implementation omits the shared transport behavior. Rail tests 254–271 check classes and Pause/AddText commands, not seek.
- **Smallest coherent direction:** Use an orientation-parameterized seek interaction/control with separate horizontal/vertical geometry.

### R12 — Remove the cached-location identity success claim [P2]

- **Trigger/effect:** Select an offline episode whose notes target different or unproven bytes, then view Mere Chain's connections card. It says `cached · matches` despite contrary or absent comparison evidence. Exact-note guards still refuse unsafe seeks; this is a misleading identity claim in another projection.
- **Reachable evidence:** [scene/geometry.rs:229](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/surfaces/src/scene/geometry.rs:229), lines 229–237, returns the literal from SourceKind alone. Card 91–94 displays it and Chain 298–307 mounts the card. Session 552–587 already supplies true/false/unproven `RepresentationSummary`; Notes 270–290 renders it honestly.
- **Authority/tests:** Receipt comparison owns identity. Scene geometry invents success from storage location. Chain tests 373–382 use Cloud, not Offline+Different/Unproven; Notes tests 602–623 establish the unused honest formatter.
- **Smallest coherent direction:** Share evidence-aware representation formatting or say only cached when no comparison is established.

### R13 — Keep item identity visible when projecting All notes [P2]

- **Trigger/effect:** Have notes on two episodes at similar offsets, then select All notes. They merge into one offset cluster under the selected episode's title/duration; row labels omit item identity. Foreign offsets are plotted/clamped against the selected duration. Actual OpenNote retains annotation identity, so this is a misleading projection, not a demonstrated wrong-source seek.
- **Reachable evidence:** [tabs/notes.rs:14](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/surfaces/src/tabs/notes.rs:14), lines 14–22,95–105,318–327, feeds all notes into one timeline; header 25–49 keeps selected-item context. Session 642–654 retains item IDs, surfaces lib.rs 788–805 clusters only by offset, and rows.rs 343–354 omits source item labels. The selected-item representation card at Notes 293–315 also sits beside this cross-item set.
- **Authority/tests:** Annotation target ItemId owns the scope of each offset; the surface drops that scope. Fixtures at Notes 371–379 use one item only.
- **Smallest coherent direction:** Group AllNotes by ItemId with labels and appropriate timelines/receipts, or use an explicitly cross-item list without one selected-item timeline.

### R14 — Remove stale all-generation preservation from the subscription receipt [P3]

- **Trigger/effect:** Run the public subscription script on an established listener directory and allow its five serial subscription/refresh saves. It can reject a correct run with “An original model generation changed or disappeared” because storage deliberately pruned those files.
- **Reachable evidence:** [scripts/subscribe-podcasts.ps1:80](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/scripts/subscribe-podcasts.ps1:80), lines 80–83 captures every old generation; 92–96 requests five changes with waits; 134–135 requires every original filename/hash to survive. Storage 105–133 prunes after publication at 180–182, retaining exactly three valid generations. The old script's empty-store first run can pass; the conflict affects existing data and enough new publications.
- **Authority/tests:** The store owns retention. The receipt independently enforces the retired unbounded-history policy. Storage test 430–461 explicitly expects only generations [18,19,20]; no script regression reconciles that contract.
- **Smallest coherent direction:** Check preservation of listener entries and the current/two-recovery contract, not immortal historical filenames. If archival before-images are needed, copy them into explicit evidence custody before running.

### R15 — Stop inventing a weekly bound in the Trail projection [P3]

- **Trigger/effect:** View Trail with sessions older than the current week. Their durations are included in the value labeled this week.
- **Reachable evidence:** [scene/mod.rs:39](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/surfaces/src/scene/mod.rs:39), lines 39–47, sums every session. Session 683–727 projects all retained history plus the open session without filtering, and rows omit numeric timestamps.
- **Authority/tests:** Calendar/time scope belongs to the host. The surface asserts a bound its input cannot establish. Trail test 160–170 only sums a short fixture.
- **Smallest coherent direction:** Label this as the displayed/retained total, or have the host supply an explicitly bounded weekly aggregate.

### R16 — Keep full range traces out of the production source [P3]

- **Trigger/effect:** Progressive playback or repeated out-of-cache seeks append one range pair per fetch forever during a live source. The byte cache is bounded; the production statistics vector is not. Each pair alone adds 16 bytes plus vector capacity. This is unnecessary lifetime growth, not a claim that a normal episode necessarily exhausts memory.
- **Reachable evidence:** [http_range.rs:15](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/http_range.rs:15), lines 15–20,36,98–103 stores the trace; 179–182 appends; only its accessor 109–112 is cfg(test). Backend open_http 283–289 creates the source; Read 187–213 fetches chunks; eviction 164–175 does not prune statistics.
- **Authority/tests:** Test instrumentation escapes the progressive source's memory policy. HTTP tests 435–485 assert byte-cache bounds and trace length, not lifetime memory after repeated evictions.
- **Smallest coherent direction:** Compile full range history only for tests. Keep runtime counters and, if needed, a bounded telemetry ring.

### R17 — Route effective rate through the existing controller contract [P3]

- **Trigger/effect:** Any product SetRate directly changes the backend and worker Levels, bypassing Genet's existing SetRate command. Audible retiming works, but controller snapshots remain at 1.0. This is established competing authority, not evidence of wrong pitch or frame ratios.
- **Reachable evidence:** [worker.rs:670](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/redshank/playback/src/worker.rs:670), lines 670–675, bypasses the controller. Adapter 43–47 already implements its source rate seam. Project 428–448 reads but ignores controller rate. Locked Genet e37762d controller 47–54 defines SetRate, 248–258 updates self.rate, 174–184 publishes it, and 187–198 resets it on load.
- **Authority/tests:** The admitted generic player contract competes with worker/backend rate copies. Playback levels test 697–741 verifies only the product snapshot; retiming tests verify actual ratios but never controller agreement.
- **Smallest coherent direction:** Keep desired pre-load configuration outside the controller, normalize to admitted effective limits, apply through its rate command after load and on changes, and publish the admitted result. Do not merely forward an unclamped request and create a different controller/backend mismatch.

## Deliberate boundaries and rejected findings

- **Independent port authority is intentional.** CLAUDE.md, the listening plan and workspace exclusions establish separate listening/annotation ownership. Redshank does not become Woodshed product state merely because it shares repository custody. No finding recommends merging its library/queue/notes into Woodshed's Set model.
- **Appearance is separate application authority.** Shared Tabard choice/workshop and listener seed/mode fallback are explicitly separated and tested; their existence does not duplicate audio/model ownership.
- **Projection is not durable duplication.** Session's library, queue and note vectors are views of the one durable model and token-qualified playback snapshot. The defects above concern incorrect projections, not those vectors existing.
- **Controller/facade distinction is legitimate.** Redshank's facade owns load tokens, representation receipts and isolation; Genet owns admitted generic transitions. R17 identifies a concrete bypass rather than condemning the facade.
- **Original and derived note targets are different facts.** Preserved original targets, fingerprints and derived estimates protect captured evidence. Model shape/version checks and host destination admission serve separate boundaries. Their mirrors do not by themselves warrant consolidation.
- **Private playback snapshots are deliberate identity protection.** Hashing and decoding the same copied bytes establishes an invariant the cache alone cannot establish. It is not redundant caching.
- **Shared seams are real.** Live cache consumes host Fetch, feed import uses Errand facts, DSP alignment is in audio-primitives, and output construction uses the shared Firewheel contract. This audit did not prove a second live device runtime or independent fetch client inside the product.
- **Experimental playback is excluded.** The spike workspace, old Firewheel graph and separate HTTP experiment are historical comparison instruments, excluded by the live workspace. Their old lockfile and sibling paths do not prove live authority duplication. Vendored W3C schemas are provenance-preserving test instruments, not runtime cruft.
- **Browser preview is explicit.** Web lib.rs 638–642 identifies in-memory fixture state without audio/files/network/durable storage, and unsupported commands report missing adapters. Its simulation is not a production playback receipt.
- **Embedding wrapper has a stated seam.** surface_api.rs 3–15 explains the per-frame projection/command pump the generic RunnerSurfaceSession lacks; the wrapper delegates retained-session behavior without taking host custody.
- **Diagnostics distinguishes execution from durable acknowledgement.** ExecutionFinished does not advance durable state; diagnostics worker tests cover that separation.
- **New-span draft-loss concern was not promoted.** Main 834–866 clears a span editor earlier than save_note 689–732, but current native begin_note creates only end_offset=None; editing an existing span takes EditNote. No shipping native creator of a new span was found. Consolidate before enabling it, but do not report presently reachable native draft loss without that caller.
- **Generic truncated-network-success was rebutted.** The locked Fetch implementation turns short bodies into FetchError and this source maps it to an error. The audit does not claim every network truncation becomes successful EOF.
- **Qualification artifacts remain bounded.** Synthetic identity/alignment receipts explicitly exclude human acoustic/visual acceptance and real-recording confidence calibration. Layout fixtures and appearance receipts do not prove voice capture/output. The fixture voice blob deliberately tests a row, not a real content-addressed recording.

## Consolidation order and unresolved questions

First preserve failed-restore custody (R01), then unify effective output controls and recovery (R02/R03/R17), job completion/wake facts (R04), and the capture gesture/capacity contract (R05/R09). R06 and R07 are focused adapter corrections. Resolve export identity before relying on multi-item export custody (R08). The remaining surface and receipt changes can then consume truthful owner facts with smaller risk.

Runtime qualification remains open by design: idle paused alignment/export completion, full-dock pointer release, actual muted preview/output recovery, exact-end seek, multi-item export collision, long capture capacity reporting and all-invalid-store write refusal have not been exercised in this audit. The exact source paths and existing test gaps are recorded so implementation owners can select focused regressions. Physical devices, output disconnect/retry, human AT, real-speech alignment calibration and long/publisher transcript behavior keep their existing independent gates.

There are **zero unresolved assigned-read or hash questions**. Remaining uncertainties are behavioral qualification and the maintainer's preferred explicit recovery/limit UX. No action was taken to choose or implement those policies.
