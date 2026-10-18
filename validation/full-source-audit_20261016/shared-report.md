# Shared partition source audit

Frozen published revision: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`.
Snapshot root: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.
Audit is READ-ONLY against the frozen source. Only `shared-coverage.json` and this report were written.

## Coverage and evidence limits

**COMPLETE assigned read: 33 of 33 files, 9,587 of 9,587 lines.** Every assigned source, test, example, scenario and Cargo manifest line was displayed and read in numbered, nontruncated batches. The manifest SHA-256 and line count matched for every file before reading; SHA-256 was verified again after the complete read. `shared-coverage.json` records per-file supplied/observed hashes, exact read ranges, counts and complete status. No assigned omissions remain.

Snapshot `CLAUDE.md` (1–117) and `design_docs/DOC_README.md` (1–413, with the initially truncated middle reread separately) were read. Relevant evidence includes the full October 15 Codex handoff and October 16 shared-capture plan, the Hocket project description, the host-refactor extraction contract/progress, the audio-ports rehome ownership sections and P5a timing scope. Neighboring Hocket native host/worker/engine, Woodshed Looper adapter and Redshank transcript callers were inspected selectively to establish reachability. Those neighboring files are **not** claimed as exhaustive coverage.

No build, test, native process, network request, profile action, lockfile change, Git mutation or product-source edit was performed. Findings below are source-proven execution paths, not runtime reproductions. Existing tests are described from their source, not claimed as rerun.

## Findings

### S1 — P1: An authenticated cyclic history can hang same-session acceptance

**Trigger and effect.** A sender with a legitimate identity creates a recipient-addressed handoff for the current SessionId/root, adds a new node whose parent is itself (or two new nodes pointing to each other), and makes that node the incoming head. No missing media is needed. `HandoffEnvelope::create` signs it; `receive` checks signature/address/media but does not validate graph topology. `History::integrate` accepts it because every parent exists in the combined key set. `accept_branch` calls `checkout`, whose `ancestor_chain` follows that cycle without a visited set or step bound. Pressing Accept therefore loops/allocates on the UI thread before the later snapshot comparison can reject the transfer. Signature validity does not establish model validity.

**Exact frozen evidence.**

- [History graph union only checks existence of parents](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/hocket-model/src/history.rs:452): 452–484; [unchecked ancestor walk](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/hocket-model/src/history.rs:435): 435–443; checkout invokes it at 409–410.
- `crates/hocket-project/src/handoff.rs`: 155–211 signs arbitrary bundles; 230–291 authenticates/materializes without topology validation; 327–331 integrates then checks out before 350–351 checks snapshot equality.
- Native reachability: `ports/hocket/crates/hocket-genet/src/project_io.rs` 186–198 reads/authenticates into `HandoffReceived`; `state.rs` 920–925 stages it; 637–646 accepts it on the UI action path.
- Local archives also lack structural validation: `crates/hocket-model/src/persistence.rs` 92–99 checks only schema version, and `crates/hocket-project/src/project_store.rs` 200–229 admits that bundle. `state.rs` 899–912 installs it and reports Saved. The immediate hang described here specifically uses same-session handoff acceptance, where checkout is reachable.

**Authority and competing admission.** Hocket model owns graph invariants. Project decoding, authenticated receipt, graph integration and same-session snapshot comparison currently apply different subsets of admission. The later handoff-only snapshot comparison is insufficient to protect the earlier model walk. This is an absent shared invariant check, not a request to merge Hocket and Woodshed persistence.

**Current tests/gap.** History tests at 733–770 cover different roots, conflicting existing nodes and missing parents; handoff tests at 658–697 cover changed tempo, missing historical phrases and unrecorded phrases. None covers cycles, map-key/node-ID mismatch, an absent head/root, an extra parentless root or a detached subgraph. All ordinary construction APIs create an acyclic graph, which explains why their own round trips pass.

**Smallest coherent direction.** Put structural history validation in `hocket-model` and use it at project/handoff admission and integration; check key/ID agreement, one valid genesis root, head existence, acyclicity and reachability. Make ancestor traversal fail boundedly even if public mutable model fields are misused later. Preserve transactional accept behavior and retained branches. Add malformed-input cases at the real load/receive/accept seams when implementing the fix.

### S2 — P2: Live playback bypasses the model's SelectOne audibility rule

**Trigger and effect.** Open or accept a valid Deeler/SelectOne project (normal Open accepts arbitrary supported `Session` profiles), arm a track with `active: None` or with another layer active, then record or paste a new layer. `promote` and imported replay check only track mute/solo, so the new inactive layer starts anyway. Separately, mute then unmute an inactive SelectOne layer: the unmute branch calls `play_from_store` without checking that layer's playback-mode eligibility. The live engine can sound a layer that the model, waveform overview and exported WAV exclude; with another layer active it can play more than one variation.

**Exact frozen evidence.**

- [Canonical model audibility rule](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/hocket-model/src/track.rs:107): 107–113.
- [Capture promotion bypass](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/hocket-recorder/src/lib.rs:1292): 1292–1318; `track_is_audible` 1369–1372 checks only track mute/solo.
- Imported replay: `crates/hocket-recorder/src/lib.rs` 973–982; inactive-layer unmute: 1191–1196; `play_from_store` 1393–1433 contains no playback-mode gate. Full-track reconciliation does correctly call `is_layer_audible` at 1383–1389, so the controller has divergent rules internally.
- Export/waveform use the correct shared predicate: `crates/hocket-project/src/export.rs` 266–271; `waveform.rs` 30–37 reuses those sources.
- Native reachability: `state.rs` 899–912 opens supported project sessions; 407–412 pastes through `append_imported_audio`; `recorder_audio.rs` 155–175 executes each submitted layer and does not reinterpret model eligibility. `view.rs` 99–104 recognizes the Deeler profile. New captures flow through the same mounted recorder controls as ordinary sessions.

**Authority and competing owner.** `PlaybackMode::is_layer_audible` is the model-owned eligibility rule. The recorder's capture/import/unmute shortcuts compete with that rule; the engine should continue to execute the admitted projection, not acquire another interpretation of SelectOne.

**Current tests/gap.** Model tests prove SelectOne behavior (`track.rs` 201–214), project waveform tests prove it (`waveform.rs` 119–138), but every shared-recorder fixture defaults to Sum. Recorder import and promotion tests check target/rate preservation, not SelectOne exclusion. Deeler combination/settings UI is deferred in the host plan; this finding does not claim that UI is shipped. Opening an already-supported serialized SelectOne session is sufficient to reach it.

**Smallest coherent direction.** Route every layer-start path through one occurrence-level predicate combining track mute/solo and `PlaybackMode::is_layer_audible`. If recording a new variation should explicitly select it, author the model selection edit and reconcile through that predicate; do not silently start an unselected layer. Test active None, another active layer, inactive-layer unmute and import, plus export/live eligible-source parity.

### S3 — P2: Every live BPM nudge stops all loops without resuming them

**Trigger and effect.** With at least one accepted loop voice and no active capture, press Increase/Decrease tempo. `NudgeBpm` stops every voice, admits the new clock and commits the BPM edit, but never reconciles playback or marks replay pending. Subsequent services leave all loops stopped even though layers remain unmuted and export still includes them. The shared Hocket projection exposes Stop, but no Play command, so the same transport offers no direct resume after this normal action; the user must toggle mute/solo or otherwise cause reconciliation. If the new clock is refused after the stops succeed, old playback is still lost while the model BPM remains unchanged.

**Exact frozen evidence.**

- [NudgeBpm stops voices, changes clock, commits, and exits](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/hocket-recorder/src/lib.rs:1199): 1199–1228.
- Replay runs only if `needs_replay` is true at `lib.rs` 964–966; this live-tempo branch does not set it. `stop_voice` removes accepted keys at 1347–1351. `reconcile_all_playback` explicitly sets the missing flag at 1439–1440.
- `crates/hocket-views/src/recorder_adapter.rs` 178–189 exposes Stop and tempo, with `transport.play` left at the `TransportProjection::new` default None (`projection.rs` 99–119).
- Native reachability: `ports/hocket/crates/hocket-genet/src/view.rs` 39–44 mounts shared transport; 481–482 queues the action; `main.rs` 154–157 services it immediately. `recorder_audio.rs` 177–179 really stops the engine layer. Engine `lib.rs` 959–964 documents existing loops as fixed-length buffers at their captured tempo; its `set_tempo` 965–992 updates the click/grid and does not itself require a layer stop.

**Authority and competing owner.** Recorder owns desired layer playback and the accepted voice set; the engine owns the clock operation. Stopping the projection as a preliminary step without restoring the desired state leaves those responsibilities disconnected. This does not require a common Woodshed/Hocket transport model.

**Current tests/gap.** Recorder tempo tests at 327–350 and 353–404 use no playing layers. The stop-refusal test at 496–511 covers refusing the edit when a stop fails, but not successful stops followed by success/refusal of the clock. `two_mounts.rs` 429–435 and `two_mounts.scn` 11–13 assert changed BPM/other-mount isolation and do not assert the seeded loop remains playing.

**Smallest coherent direction.** Preserve accepted voices if the clock contract supports that operation, or explicitly queue/reconcile the desired eligible voices after successful clock admission and restore old desired playback on clock refusal. If product policy intentionally pauses all loops on tempo changes, make that an explicit transport state with a reachable Resume operation and qualify it as such. Add the missing nonempty-session cases.

### S4 — P2: The reusable Recorder adapter suppresses runtime failure diagnostics

**Trigger and effect.** Mount the public `hocket_views::surface` with a Recorder and a backend that refuses capture/playback/clock, or reports unavailable audio. `Recorder::service` writes the refusal into `audio_status_label`, but `recorder_projection` never fills `transport.status` or `transport.input_status`. Those fields default to empty, so the shared surface renders no failure message. A refused Record leaves the interface with no explanation; the second-host example is a reachable composition of exactly this wrapper. The standalone native Hocket host separately adds an audio-status accessory and partly masks this gap.

**Exact frozen evidence.**

- [Compatibility projection omits status fields](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/hocket-views/src/recorder_adapter.rs:141): transport construction 141–213 and return 214–224. `projection.rs` 99–119 initializes both status fields to empty.
- `crates/hocket-views/src/surface.rs` 488–495 renders statuses only when nonempty; `hocket-views/src/lib.rs` 44–59 uses this adapter for public `surface`.
- `crates/hocket-recorder/src/lib.rs` 883–925, 968–970, 1347–1356 and 1451–1453 retain/report the actual errors.
- `crates/hocket-views/examples/two_mounts.rs` 105–117 mounts both wrappers without an accessory; 231–234 can refuse capture; 473–486 tests refusal but only inspects controller state/status. Native workaround: `ports/hocket/crates/hocket-genet/src/view.rs` 39–44 supplies an accessory and 473–474 renders `state.audio_status_label`.
- The extraction contract explicitly says runtime refusal must remain visible: `ports/hocket/design_docs/2026-07-08_genet_host_refactor_plan.md` 408–415.

**Authority and competing owner.** Recorder already owns the diagnostic facts; the compatibility adapter silently drops them, and standalone composition has its own repair. Host-selected wording remains legitimate, but the provided complete Recorder wrapper should either project that diagnostic or require an explicit diagnostic adapter instead of quietly omitting it.

**Current tests/gap.** Controller tests assert error strings, and the synthetic second-host test also asserts only the string on state. Generic projection tests prove a manually supplied input status is visible (`projection_tests.rs` 131–142); none proves the real Recorder compatibility projection forwards its own refusal.

**Smallest coherent direction.** Populate the existing status/input-status fields from the Recorder's factual state, with host wording overrides if needed. Reuse the resulting projection in native composition so failures do not depend on a private accessory. Assert DOM-visible error status after a real wrapper-level rejected action.

## Additional contract concern for parent triage

### C1 — P2 API contract gap; normal native dispatch mitigates it: compatibility conversion drops frozen action targets

The renderer correctly captures `ProjectedAction::Record(target)` (`surface.rs` 385–414), but `recorder_adapter.rs` 227–235 maps Record, Stop, tempo/click/master-clock/Add to unqualified Recorder actions and discards the key. `HocketKey::Transport` contains only persisted SessionId (19–25), not the control epoch. `Recorder::apply(ToggleRecord)` resolves the currently armed track at `lib.rs` 1109–1135.

**Concrete API trigger/effect:** an embedding host retains or queues a rendered Record callback/action, changes the armed track or replaces the project, then delivers that old callback. The Record originally rendered for A becomes ToggleRecord and can capture to B/current session; a stale Stop or tempo action similarly operates on current state. The conversion no longer carries enough information for Recorder to reject it. A simpler custom queue can submit Arm(B) then the old rendered Record(A) before service, with the same result.

**Reachability qualification:** `hocket_views::{surface,table,transport}` are public generic Dispatch wrappers (`lib.rs` 44–97), so queued/custom embedding hosts can reach the gap. Current native Hocket services every dispatch before another input (`main.rs` 154–157), and the two-mount example does likewise (`two_mounts.rs` 304–306); those hosts reduce the ordinary input-order exposure. I did **not** establish a headed stale-callback reproduction in native Hocket and do not count this as a confirmed native race. Woodshed's direct host projection does retain its qualified owner/epoch keys and evaluates fresh policy (`woodshed-views/src/stage/looper/recorder.rs` 514 onward), so this is not evidence of a Woodshed Looper misroute.

**Authority/direction/tests:** target selection belongs to the render-time host/Recorder owner. Preserve context/epoch/target through the compatibility action seam and validate it before resolving runtime intent, or explicitly narrow/document the public wrapper as synchronous and nonretaining. Existing generic projection tests only prove the key survives up to dispatch; controller tests prove capture-completion tickets, not render-action freshness. Parent should decide whether the public reusable contract warrants a fix now.

## Deliberate boundaries and false-positive rebuttals

- Hocket Session/History and Woodshed Set/Card/LoopPlan are different product authorities. The stateless `HostPolicy` permits typed host effects over supplied keys; it does not invent another session or claim device admission (`host_policy.rs` 7–13, 63–99, 140–175). The actual Woodshed caller constructs policy from accepted backing observations and retains its owner at `woodshed-views/src/stage/looper/recorder.rs` 442–497. Do not collapse the two product models as cleanup.
- Hocket's `.hock` archive/media identity and Woodshed's sealed capture companions are intentionally separate formats. Shared project operations are device-free and do not depend on the port's native services. No actual root-to-port product dependency was found in the assigned manifests; sharing the model/project/controller/views is real source reuse.
- Retained history branches and append-only phrase/media pools are explicit behavior (`history.rs` 17–29; `handoff.rs` 303–304), not dead duplicates. Project save considers the phrase pool so inactive retained captures survive. Pruning it just because a phrase is not on the active track stack would destroy history.
- Export's default rejection of unequal loop lengths is explicit duration policy; Bars/frames opt into repeating shorter buffers (`export.rs` 7–12, 104–159). Track waveform intentionally ignores track mute and host solo while honoring layer rules (`waveform.rs` 15–20). Offline export is a desired mix, not a claim about the currently running device or whether Stop All was pressed.
- Repeated `referenced_media` helpers in project_store (233–240) and handoff (397–404), plus parallel file/byte WAV writer loops (export 164–201), are small literal maintenance duplication with no present behavioral disagreement found. They can move to one helper when touched; they do not justify a new generalized media framework.
- Recorder mount leaf keys are scoped by mount and stable occurrence; independent same-phrase gain envelopes and unmount isolation have direct tests. No evidence that this is a second audio/media custody owner. Synthetic two-mount samples/meters are explicitly labeled and do not imply physical qualification.
- `timed-text` is a bounded transcript subset by contract, with caller-owned input limits (`lib.rs` 27–29), checked timestamp arithmetic, skipped-block counts and preserved payload/settings. Redshank's fetcher supplies the byte limit, rejects no-cue results (`desktop/src/main.rs` 101–115), and its view discloses skipped blocks (`surfaces/src/tabs/listen.rs` 271–277). No duplicate live transcript parser or hidden success fallback was found in this partition.
- `woodshed-web` deliberately mounts the static `woodshed_views::demo` with unit state and inert hooks (`lib.rs` 17–29, 46, 78–99), not the application. Its rendering-only limit is documented; `demo.rs` 98–111 visibly identifies S0. Native-target compilation of this crate is vacuous because the crate is cfg-wasm-only (41); do not treat a native root check as browser-product qualification. This is an unfinished rendering preview, not evidence of a second application authority.

## Known dirty local P5a work — tracked separately

Read-only `git diff --stat` and a path-limited diff inspection of `/Users/markik/Code/repos/woodshed` found active Woodshed audio/core/host/view timing changes. The P5a plan diff says authored positive BPM must be honored, mathematical duration validation remains separate from device-rate PCM admission, and legacy effective minima are disclosed. These are ongoing changes, not accepted frozen-source fixes.

`git diff --name-only -- crates/hocket-model crates/hocket-project crates/hocket-recorder crates/hocket-views crates/timed-text crates/woodshed-web` returned no paths at inspection time. None of S1–S4 or C1 is claimed fixed by known dirty P5a. Conversely, P5a's duplicated Card dwell/preview arithmetic and refusal handling are outside this assigned shared partition and should be consolidated by the parent with its owning reviewer rather than counted again here. No primary source was edited or exhaustively reread, and concurrent changes may continue after this observation.

## Unresolved questions

1. Should master-clock-off forcibly mute the click or may click preference remain independent? `hocket-model/src/session.rs` 51–53 and 99–102 describe off as no audible click, but Recorder's ToggleMasterClock only commits model state (`lib.rs` 1230–1239); sync/nudge pass `self.click` independently (1221–1226, 1335–1340). The UI exposes separate Click and Master clock controls. This is a demonstrated contract disagreement, but the product intent needs confirmation before calling the implementation wrong or merging those flags.
2. Is ordinary tempo change intended to keep/restart existing loops or explicitly pause them? S3 proves current behavior and missing Resume; the chosen transport behavior should determine the smallest fix.
3. Is delayed callback retention part of the public compatibility wrapper contract? C1 is a proven information-loss seam, with normal native dispatch mitigation described above.
4. Are there external legitimate producers of SelectOne projects beyond the model constructors and existing archives? S2 is reachable through supported file/session admission regardless, but broader Deeler UI completion remains deferred and should not be overstated.
5. No product deletion is proposed. Historical docs that still mention Moothold, postcard or retired frameworks need owning-plan reconciliation if documentation cleanup is authorized; comments alone do not establish live duplicate implementations.

The assigned source read is complete. The parent still owns independent finding verification, cross-partition overlap resolution and the collective COMPLETE verdict.

## Assigned coverage by crate

| Crate | Files | Lines | Status |
|---|---:|---:|---|
| `hocket-model` | 9 | 2,129 | Complete; hashes match |
| `hocket-project` | 7 | 2,146 | Complete; hashes match |
| `hocket-recorder` | 3 | 2,222 | Complete; hashes match |
| `hocket-views` | 10 | 2,772 | Complete; hashes match |
| `timed-text` | 2 | 181 | Complete; hashes match |
| `woodshed-web` | 2 | 137 | Complete; hashes match |
| **Total** | **33** | **9,587** | **Complete** |
