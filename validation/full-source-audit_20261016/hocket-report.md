# Hocket partition: complete frozen-source audit

Reviewed on 2026-10-16 against published Woodshed revision `d12201bfe80cc94345c6308c46caaebf4a9fe42e` at `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.

**Assigned read complete: 73 of 73 files, 20,440 of 20,440 lines, 664,256 bytes.** This includes all 8,683 lockfile lines, every assigned Rust/test/config line, receipt JSON, the receipt launcher, and every assigned scenario. All 73 SHA-256 values and line counts match `hocket-assigned.json`; all were rechecked after reading. `hocket-coverage.json` records each actual hash, status and exact contiguous read ranges. Additional documentation, caller/callee and pinned-supplier reads are separately enumerated there. Searches were used only to trace reachability after the full assigned read.

This is a source review. No build, test execution, native process, device, network request, profile mutation or Git mutation was performed. Historical receipts below are existing evidence inspected as text, not fresh execution or fresh visual review. Only this report and `hocket-coverage.json` were written.

Every source location below is relative to the exact snapshot root above unless an absolute supplier path is supplied.

## Authority map and deliberate separation

| Owner | Actual role observed | Assessment |
| --- | --- | --- |
| Root `hocket-model` | Session, append-only history, phrases, layers and persisted identifiers | Canonical musical/document authority; the port consumes the root crate by path. |
| Root `hocket-project` | Media identity, portable project storage, mix/export, handoff verification and branch acceptance | The port worker calls these operations; it does not implement a competing portable format or mix algorithm. |
| Root `hocket-recorder` | Recorder gestures, capture tickets/promotion, playback projection and fallible audio contract | The desktop delegates typed recorder actions and uses the same controller as the reusable surface. |
| Root `hocket-views` | Recorder layout, scoped leaves and reusable controls | `view.rs`, `leaves.rs` and `theme.rs` compose it; the former standalone recorder rendering is gone. |
| Port `hocket-engine` | Native graph/CPAL lifetime, voice reservation, polling schedule and input integrity | Legitimate product runtime. Its schedule differs deliberately from Woodshed's frame-driven processor. |
| Port `hocket-genet` | Project association, dialogs/worker admission, identity consent, clipboard, appearance, updates and window policy | Legitimate host ownership; the concrete gaps below occur at these seams. |

The current implementation consumes shared `TakeAccumulator`, `BarCountdown`, `CompletedTake`, click timing, onset and meter primitives. Hocket's phase polling, full observing-batch capture, wait-only start, count-in and device ownership are documented product contracts, not evidence that it should adopt Woodshed's scheduler or `OutputRuntime`. Independent port lockfiles/releases and the two selectable updater transports are deliberate boundaries. No direct dependency on a Woodshed application crate or another port was found in the assigned manifests.

## Findings

### H1 — P1: project replacement can orphan an ongoing free capture indefinitely

**Trigger and reachability.** Receive/stage a valid handoff, turn off the master clock, arm a track and start free capture, then click the visible **Accept** action. `view.rs:397-416` routes that action directly to `AppState::accept_incoming`. The latter has no recording guard before either replacement path (`ports/hocket/crates/hocket-genet/src/state.rs:637-680`). A second path is to admit Open while idle, start capture while its worker read is pending, and then receive the matching Open reply (`state.rs:728-741,899-912`). The recording check occurs at Open dispatch, not at replacement admission.

**Effect.** `Recorder::replace_project` clears `ticket` and queued actions while deliberately retaining the busy backend phase (`crates/hocket-recorder/src/lib.rs:846-872`). `DesktopRecorderAudio` still owns the old accepted token (`ports/hocket/crates/hocket-genet/src/recorder_audio.rs:128-153`), and the engine's free buffer keeps appending until explicitly stopped (`ports/hocket/crates/hocket-engine/src/lib.rs:585-607,743-748`). With no recorder ticket, Record cannot call `stop_free_capture`; it returns “capture is already armed or recording” (`crates/hocket-recorder/src/lib.rs:1094-1101`). Device selection also refuses while `is_recording` is true (`state.rs:975-980`). `StopAll` stops voices only (`crates/hocket-recorder/src/lib.rs:1257`), and the host's pre-replacement `stop_all()` is merely queued and then cleared. The new project remains busy and cannot record/save/export; the old free buffer can grow until an independent input fault retires it or the process is restarted.

**Authority owner / competing owner.** The recorder owns the capture ticket; the native adapter owns its execution. Host replacement retires the ticket without retiring its adapter execution. This is a broken retirement handshake, rather than an argument for combining the engine and model.

**Tests/gap.** Shared replacement tests (`crates/hocket-recorder/src/lib.rs:216-234,443-464`) manually finish or retire the fake old capture. They do not exercise an unbounded real free capture after the controller has discarded the only stop token. Host correlation tests prove stale reply rejection, but do not test Accept during free capture or capture beginning after Open dispatch. The native project scenarios run device-free and cannot expose this.

**Smallest coherent direction.** Centralize host replacement admission and reject/defer it while capture is live, including matching Open completion. If replacement during capture is desired, extend the recorder/audio retirement contract so the old token remains usable until stop/cancel is accepted and observed; retain the generation guard against old completion. Do not solve this by clearing the busy flag alone.

### H2 — P1: native close has appearance-save protection but drops unsaved recorder projects

**Trigger and reachability.** Record/paste a layer or add a track without saving, then close the native window. `ports/hocket/crates/hocket-genet/src/main.rs:172-184` returns `CloseDisposition::Exit` whenever no appearance editor blocks it and no unfinished scenario needs readback. It never checks `state.is_dirty()`, `state.is_recording()` or project I/O. The same hook can exit after saving/discarding an appearance draft while the actual recorder project remains unsaved.

**Effect.** Session/history/media exist in `Recorder`/`InMemoryStore` until explicit Save. Startup creates a new empty session (`state.rs:126-145`); no autosave/recovery route appears in the assigned host. Closing therefore discards unsaved captures and edits. Closing while a project worker is saving can also terminate before its result is admitted; no close-path wait is tied to `pending_project`. The appearance editor's close correctness is not recorder-project protection.

**Authority owner / competing owner.** `AppState` owns project dirtiness and the admitted save snapshot. The host close decision delegates only to `AppearanceState`/Workshop, giving the appearance subdocument effective authority to approve closing the whole product.

**Tests/gap.** `appearance_tests.rs:97-113,148-160` and `scenario.rs:270-349` cover appearance dirty-close and Mesquite readback. They do not cover a dirty recorder project, active capture, or a save pending at native close. Existing native Save/Open acceptance does not include unsaved close.

**Smallest coherent direction.** Add one product-level close decision that accounts for the recorder project, in-flight save and appearance draft. Save/Discard/Cancel must resolve the actual project before final close; keep Mesquite's readback deferral beneath that decision. The updater's `ReadyToRestart -> ApplyAndRestart` action (`state.rs:849-870`) should route through the same admission decision on platforms where applying terminates the process.

### H3 — P1: persona confirmation can switch to a different family identity than the one shown

**Trigger and reachability.** Start Hocket with an adopted private Hocket identity separate from family persona A. While Hocket remains open, change djinn's active family persona to B, then use **Use my shared persona / Switch and re-share**. The offer is built from cached `IdentityHome::Apart.family_profile` (`ports/hocket/crates/hocket-genet/src/state.rs:509-519`); confirmation discards the staged value as `_staged` and calls `join_family()` (`state.rs:545-552`). `join_family` reconnects to the currently active persona but retains the old cached `family_profile` in both consent and the new home (`ports/hocket/crates/hocket-genet/src/identity.rs:207-225`).

**Effect.** The dialog can name A while the new speaker/public key actually belongs to B. Consent is saved as `(profile A, key B)`. On a subsequent open with B active, `open_at` requires the consent profile and key both to match (`identity.rs:170-172`), so it can silently return to the old apart Hocket identity. The displayed identity, consent record, actual signer and next-process identity can disagree.

**Authority owner / competing owner.** Djinn owns the current persona/key. Hocket owns the user's decision to rotate its previously shared contact token. Cached presentation state must not authorize whichever identity a later unqualified connection happens to return.

**Supplier check.** The lock selects Mere `91467c0bbf6f92cc26262579dec634155ee33f2d`. Its local cached `crates/graphshell/graphshell-endpoint/src/door/custody_identity.rs:64-76` makes `connect` use `None`, documented as the persona currently in use; `143-150` describes `profile()` as the connection-time profile, and `174-179` documents active-persona switching/reconnection. The exact cache checkout HEAD was verified read-only. Full supplier path and hash are recorded in coverage.

**Tests/gap.** Assigned identity tests cover token parsing, summaries and pending identity, with the route-backed pending test opt-in. None change active family identity between offer and confirmation or verify consent against the actual returned profile.

**Smallest coherent direction.** Stage the target profile and master key alongside the losing token. At confirmation connect explicitly as that target, or re-fetch the current target and require a new confirmation if either profile/key changed. Save consent and the displayed home from the identity actually accepted. Surface refusal in the UI instead of only stderr (`state.rs:561-563`).

### H4 — P2: the saved-head marker misses imported retained branches and media

**Trigger and reachability.** Accept a valid same-session envelope whose active head equals the local saved head, but whose history includes a previously unseen inactive branch/capture. Such envelopes arise naturally when a peer retains a local branch and accepts an older incoming head before handing it back. `accept_incoming` calls portable branch integration and replaces the bundle/store, but ignores `BranchAcceptance` and leaves `saved_head` unchanged (`ports/hocket/crates/hocket-genet/src/state.rs:642-659`).

**Effect.** `is_dirty()` compares only `history.head != saved_head` (`state.rs:933-935`). It reports false even though the in-memory history graph, phrase pool and media have changed and the associated `.hock` lacks them. Existing saved-head acknowledgements are correct for linear edits during Save but insufficient for an append-only graph. The scenario checkpoint's `project-dirty` field also inherits that false negative.

**Authority owner / competing owner.** The durable document is the full `ProjectBundle` plus referenced media. The host's single NodeId is an incomplete competing account of what has been saved. Portable `accept_branch` deliberately integrates off-head nodes and imports media (`crates/hocket-project/src/handoff.rs:327-378`), which must remain possible.

**Tests/gap.** `project_correlation_tests.rs:226-243` covers a changed active head during Save. `122-161` uses an identical self-addressed handoff. Neither imports off-head work with an unchanged active head. Portable retained-branch tests establish preservation in memory, not honest host dirtiness.

**Smallest coherent direction.** Track a saved document revision/fingerprint covering retained graph/pool changes, and acknowledge the submitted revision on Save completion. At minimum, successful branch import must dirty the project even if `accepted_head` is unchanged; this also has to survive status changes and in-flight Save acknowledgements. Keep project association and save acknowledgement host-owned.

### H5 — P2: restored media still appears missing after same-session acceptance

**Trigger and reachability.** Open a project with an unavailable referenced blob, then accept a same-session handoff that supplies that exact blob. Portable acceptance imports validated media into the new store (`crates/hocket-project/src/handoff.rs:353-373`). The host passes `self.recorder.missing_media.clone()` into replacement unchanged (`ports/hocket/crates/hocket-genet/src/state.rs:650-655`).

**Effect.** Playback and export consult the actual store and can use the recovered media, while the rail continues saying “N media blob(s) unavailable” (`ports/hocket/crates/hocket-genet/src/view.rs:269-273`). `Recorder::replace_project` simply installs the supplied set (`crates/hocket-recorder/src/lib.rs:865-868`); no service path reconciles that set. The metadata checkpoint also exports the stale set. The unavailable-media warning can persist until the user saves/reopens.

**Authority owner / competing owner.** Validated content-addressed media in the resulting store is authoritative. The cached `missing_media` projection becomes a second, stale availability reading.

**Tests/gap.** Shared recorder tests check missing media remains visible when absent (`lib.rs:417-440`). Assigned host tests do not cover recovery by handoff or compare the warning/set with the resulting store.

**Smallest coherent direction.** Recompute missing references from the resulting bundle/store at replacement admission, preferably through the portable project's existing media-reference/validation boundary. Preserve genuinely missing local-only branch media; do not indiscriminately clear the set.

### H6 — P2: update channel and check cadence are persisted controls with no execution effect

**Trigger and reachability.** Set `channel=Beta` or change `check_interval_secs` in the provider-backed JSON file, then launch a configured installed build. The real boot and CLI load that file (`ports/hocket/crates/hocket-genet/src/main.rs:79-85,280-283`). Both transports explicitly ignore the `UpdateChannel` parameter (`update/luggage_transport.rs:124-126`, `update/velopack_transport.rs:102-105`). The frame/worker code has no interval clock; boot always sends one check if policy allows, and a long-running process never schedules another.

**Effect.** Beta and Stable use exactly the same feed, and a 300-second versus 604,800-second cadence changes neither restart behavior nor long-running checking. The provider describes, accepts and saves both values as real settings (`update.rs:231-255,280-305`). Full-snapshot Rust reference search found interval usage only in the declaration/default/provider/test, and provider `describe/apply` only in its self-test; there is no mounted settings surface registering that provider in this host. Policy itself is wired, so this is a bounded partial implementation, not a claim that the whole updater is inert.

**Authority owner / competing owner.** `UpdateSettings` is advertised as policy authority, while environment-selected feed and one-off boot sequencing actually determine these behaviors.

**Tests/gap.** `update.rs:667-696` proves schema/storage and rejects an invalid interval. Worker/CLI tests use fake transports that ignore channel and do not exercise cadence. They cannot establish these settings' execution semantics.

**Smallest coherent direction.** Either remove/hide unsupported settings and document that channel is selected by feed, or implement feed/channel selection and one host-owned cadence scheduler using the persisted values. The settings editor/registration can remain a separately named incomplete feature. Avoid carrying a successful self-test as proof of product adoption.

### H7 — P2: the Velopack adapter re-resolves the feed instead of applying the staged offer

**Trigger and reachability.** Select the supported A/B transport with `HOCKET_UPDATE_TRANSPORT=velopack` (`ports/hocket/crates/hocket-genet/src/main.rs:269-273`), download offered version V, and let the feed advance to W before clicking **restart now**. Download checks the requested version (`update/velopack_transport.rs:116-135`), but `apply_and_restart` obtains a new manager, checks the feed again and passes whatever new `UpdateInfo` it returns to apply (`138-148`). It stores neither the staged offer nor its version.

**Effect.** A displayed “V ready” can attempt to apply W, or fail because W's package was never downloaded; withdrawing the feed can block applying an already downloaded V. The stable contract says “apply what is staged” (`update.rs:493-497`), but the transport substitutes current feed state for staged identity. The default Luggage adapter does retain its offered update and verified bytes; keeping the A/B alternative is deliberate, but its semantics differ at this seam.

**Authority owner / competing owner.** The downloaded/staged offer should own apply identity. The current feed becomes a competing apply authority.

**Tests/gap.** Velopack adapter tests (`153-195`) cover availability and built version only. Worker tests fake apply and never model feed drift after staging. No claim is made here that arbitrary unverified bytes would install; the established defect is selecting the wrong offer and losing the promised staged path.

**Smallest coherent direction.** Retain the exact successfully downloaded `UpdateInfo`/artifact identity and consume it on Apply. If the A/B has served its purpose, retire the selectable adapter and unconditional lifecycle dependency through an explicit product decision; do not retain a second transport whose “ready” contract is weaker.

### H8 — P2: handoff preparation performs a full media serialization on the frame-owning thread

**Trigger and reachability.** Click Hand off on a project with substantial accumulated/free-capture media. `ports/hocket/crates/hocket-genet/src/state.rs:483-501` synchronously constructs `HandoffEnvelope` before even showing the save dialog. Portable `create` hashes every referenced buffer, clones all media, serializes the entire unsigned payload and signs it (`crates/hocket-project/src/handoff.rs:162-203`). The project worker then serializes the envelope again (`ports/hocket/crates/hocket-genet/src/project_io.rs:173-177`).

**Effect.** The event loop that must poll capture/input and present the UI performs media-proportional copying, hashing and CBOR construction. The plan/comment characterizes the worker as paying the heavy CBOR pass, but a full pass has already happened synchronously. Hand off refuses active recording, which bounds direct capture-loss risk for that gesture; ordinary playback and the continuously running input reader still share the stalled frame pump. Large-project latency and peak memory have not been measured in this audit. Related host Copy audio/Paste audio paths (`state.rs:329-417`) also render/encode/decode synchronously, but are not independent portable implementations.

**Authority owner / competing owner.** Portable `hocket-project` correctly owns envelope semantics. The host unnecessarily owns bulk execution on its UI thread because signing and preparation are fused. This is a host-thickness/performance seam, not duplicated session truth.

**Tests/gap.** The real worker round trips use tiny fixtures. Native handoff acceptance uses one 48,000-frame buffer. Those prove protocol/workflow correctness, not large-project responsiveness. No timing assertion or new execution is claimed.

**Smallest coherent direction.** Split portable envelope preparation/signing so hashing/serialization/media ownership transfer can run on a bounded worker and the identity capability supplies only the required signing act. Reuse the existing project actor/correlation scope and preserve exact signed bytes. Avoid building a second handoff codec in the host.

## Cruft and false-positive rebuttals

- The former standalone waveform/meter/view implementation has been replaced with thin `RecorderLeaves`/`hocket_views` adapters. The remaining product shell, warm palette and host services are reachable; they should not be deleted merely because Woodshed mounts the same component.
- `Engine::voice_nodes` is a bounded runtime projection, not a second authoritative layer stack. Queue cancellation/reservation has real offline coverage. The allocation guard explicitly excludes pending-layer vector rebuilding and Firewheel update; it does not falsely claim a zero-allocation whole tick.
- Poll-based boundary overshoot, three-tick tempo-restart resync and the lack of physical/mixed-rate input qualification are explicitly documented limitations. Their tests should remain through any shared-kernel change. They are not reopened as new findings here.
- `hocket-headless` is an explicit `--run` manual device demonstration, with nonzero execution failure. Its help accurately limits success to completed capture and queued replay. It is not a hidden green-on-no-device CI gate.
- Existing receipts consistently name source/binary hashes and keep device-free presentation separate from physical audio. Their copied themes/process summaries are historical evidence, not active competing configuration. No receipt or capture was regenerated/reviewed visually in this audit.
- Manifest comments and indexes contain stale explanatory text: `Cargo.toml:75-77` still says exact pins while rows now use `branch = "main"`; `111-112` says wgpu 29 while the row/lock use 30; Hocket `CLAUDE.md:58-59` still advertises weave after root `CLAUDE.md:104-106` records its removal. Hocket `DOC_README.md:71-85` says native acceptance remains open while its lines 14-16 and current native plan record bounded completion. These are low-priority documentation cleanups, with source/current receipt authority taking precedence.
- Workspace-only dependency aliases left after member-edge cleanup are not compiled dependencies by themselves. Their removal is optional manifest cleanup, not runtime de-duplication. Mixed package revisions in a lockfile and optional metadata entries alone are not proof of a second runtime authority.

## Known dirty local P5a work

The frozen `local-uncommitted.patch` was searched by diff headers for assigned Hocket and shared Hocket model/project/recorder paths. It contains Woodshed timing/realization/preview work, including the P5a plan update; no assigned Hocket path or shared Hocket crate is patched. Consequently **none of H1-H8 is counted as already fixed by that known dirty patch**. The live primary checkout was not inspected or changed, and no conclusion is made about later work arriving there after the audit freeze.

## Remaining questions and acceptance limits

1. Should project replacement be blocked during capture, or should explicit cancellation be part of the shared recorder/audio contract? H1 needs one coherent admission choice; generation rejection alone is insufficient for free capture.
2. Should unsaved project close use explicit Save/Discard/Cancel, or does the product intend durable recovery/autosave? No implementation of either exists in this partition. The report recommends routing all destructive lifecycle exits through one product decision.
3. Should the Velopack A/B remain user-selectable? Its continued retention is documented, so retirement is a product choice, while staged-offer correctness remains necessary if it stays.
4. Target identity consent and complete-document save acknowledgement need focused regressions before source-level fixes can be accepted. Native capture, actual close outcomes, large-project responsiveness and fresh-process identity/project reopening remain separate follow-through gates.

**Completion statement:** every assigned file and every assigned line was read; no assigned coverage remains incomplete. Findings are frozen-source conclusions with traced callers/callees and named test gaps. They do not claim new build, native, physical, publication or device qualification.
