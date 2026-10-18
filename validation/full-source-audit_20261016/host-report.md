# Woodshed frozen-source audit: host partition

Frozen revision: d12201bfe80cc94345c6308c46caaebf4a9fe42e

Snapshot root: /Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf

Audit artifacts: /Users/markik/Code/testing/woodshed/full-source-audit-20261016/host-coverage.json and host-report.md

## Coverage and method

**COMPLETE collective assigned coverage: 20 files, 18,828 lines, including every assigned test, ignored helper, manifest line, comment and configuration line.** The coordinator read 6 files / 5,372 lines; three explicitly authorized parallel reviewers read disjoint 7-file / 5,180-line, 4-file / 4,910-line and 3-file / 3,366-line partitions. The coordinator verified every assigned SHA-256 and line count against host-assigned.json and independently read the concrete finding locations and selected caller/supplier evidence. Full source reading was collective, not a claim that the coordinator personally reread all 18,828 lines.

host-coverage.json records the exact inclusive read chunks, named reader, manifest hash, actual hash, actual line count and complete status per file. All hashes matched both the reviewed snapshot and manifest. Assigned-file outputs were untruncated. An initial documentation output was truncated and then completely reread in bounded chunks; a truncated neighboring-file locator search is not coverage evidence.

Snapshot CLAUDE.md and design_docs/DOC_README.md were read completely. Relevant complete guidance included the Woodshed Codex handoff, settings persistence split, shared capture accumulator and Mere take acceptance plans. Subreviewers additionally read the persona/sealing and MIDI plans, plus pertinent audio rehome, P5a, WAV and compression plan sections. Neighboring source and the exact locked Mere supplier 6575075057e8909032475ae6c88118a19969eadc were inspected selectively to verify reachability. Such selective reads do not imply complete coverage of other partitions.

No product/source/snapshot files, profiles, Cargo locks, commits or Git state were changed. No builds, tests, native processes or network browsing were run. The only writes were these two external audit artifacts. Plan receipt counts are historical context and were not rerun or treated as current acceptance evidence.

## Findings at a glance

| ID | Priority | Finding | Status versus inspected dirty primary |
|---|---|---|---|
| H1 | P1 | Persona switch bypasses outgoing save/capture preservation | Unfixed in identical persona/capture files |
| H2 | P2 | Persona identity changes key without independent mutable filesystem slots | Acknowledged open gap; unchanged |
| H3 | P2 | Native graph cache omits claim marks and strength | Unfixed; leaves.rs identical |
| H4 | P2 | Focused-text adapter omits two visible editable fields | Unfixed; text.rs identical |
| H5 | P2 | Persona reset drops the injected event clock | Unfixed; persona.rs identical |
| H6 | P2 | Persona reset empties MIDI inventory without automatic refill | Unfixed; relevant reset/sync code unchanged |
| H7 | P2 | MIDI rescan preserves an index while changing its device meaning | Unfixed; relevant sync code unchanged |
| H8 | P2 | MIDI output request is published as accepted connection | Unfixed; midi.rs identical |
| H9 | P2 | MIDI output replacement loses connection-specific Stop/Start | Unfixed; midi.rs identical |
| H10 | P3 | Three unproduced legacy capture commands retain weaker parallel execution | Cleanup; external API use unresolved |
| H11 | P3 | Capture awaiting_commit/is_busy status has no production reader | Cleanup; capture file identical |

P1 denotes a reachable loss-of-unsaved-work path. P2 denotes a concrete correctness/interaction defect with a defined trigger. P3 denotes bounded cruft; it is not a claim of a currently reachable destructive UI route.

## H1 — P1: Persona replacement independently decides to discard outgoing work

Primary snapshot location: crates/woodshed-genet/src/persona.rs:171-199, especially 173-185. Related exact paths: crates/woodshed-genet/src/sync.rs:35-57; crates/woodshed-genet/src/capture_persistence.rs:545-574; crates/woodshed-genet/src/capture_persistence/codec_worker.rs:58-64,115-124; crates/woodshed-genet/src/main.rs:74-150; crates/woodshed-views/src/stage/settings.rs:223-227.

**Trigger and effect.** Finish a new capture and choose a persona while its codec/publication job is pending, or choose a persona while current changes have a failed/unconfirmed save. Choosing the already-active persona is enough. The normal dispatch runs persona handling before push_backend and its preservation tail. settle revokes WAV work, opens incoming custody, replaces the whole UiState and outgoing store, and restores only the durable incoming representation. Its new audio-session identity causes begin_session to clear the live capture bank, cancel the worker epoch and discard late outgoing completion. Unpublished take PCM and unsaved edits can disappear without a retry/discard/cancel decision.

**Owner and competing owner.** SessionStore acknowledgement and capture persistence own whether outgoing practice is confirmed saved. Persona lifecycle owns admission of an incoming identity, but currently independently authorizes replacement. The existing close path already defers for codec preparation and offers retry/discard/cancel for unconfirmed practice. Incoming reset and old-epoch rejection are necessary isolation measures; the missing preservation barrier must precede both reset and custody change.

**Tests and gap.** Persona tests at persona.rs:234-309,588-637 verify isolation and retained appearance. The unused-persona fixture uses a separate MemoryBackend. Capture tests at capture_persistence.rs:1085-1247 cover failed publication, retry and epoch cancellation; 1348-1458 exercise real close hooks. None drives the production persona switch with an outstanding job or failed outgoing save. Successful epoch cancellation is not a preservation receipt.

**Smallest coherent direction.** Reuse one host-owned preserve/retry/discard transition transaction for close and persona replacement. Finish or explicitly discard outgoing work before open_store_as changes custody; only then reset state and reject stale work.

## H2 — P2, acknowledged open gap: Persona keys share singleton mutable files

Primary snapshot location: crates/woodshed-genet/src/storage.rs:78-119,250-286; switching call: crates/woodshed-genet/src/persona.rs:174-199. Supporting paths: crates/woodshed-core/src/sealed_backend.rs:65-97; crates/woodshed-core/src/storage.rs:306-318; crates/woodshed-views/src/stage/settings.rs:218-221.

**Trigger and effect.** Save practice as persona A, then switch to B under the ordinary application profile. Identity selection changes the sealing key, but every branch creates FsBackend::new(), which selects the same genet-state.json/genet-settings.json or the same environment overrides. B receives A's ciphertext under B's key. Current unseal/readback checks correctly reject it and quarantine writes, protecting A; B gets a default unsaved/quarantined session instead of independently writable B practice. The visible promise says switching swaps to that persona's own Set, history and settings.

**Owner and competing owner.** Identity owns cryptographic custody. Host filesystem composition owns mutable slot names. They use different scopes while the UI promises one persona-scoped session authority.

**Tests and gap.** session.rs:195-222 correctly tests wrong-key refusal. persona.rs:594-618 uses a separate empty memory backend and therefore avoids the production slot collision. storage.rs:761-817 verifies independent capture blob namespaces, not independent mutable session/settings slots. This is not a silent-overwrite claim.

**Smallest coherent direction.** Choose one stable persona namespace for mutable practice slots, with explicit migration/recovery of existing singleton files and deliberate override semantics. Keep namespace selection in host composition and retain wrong-key quarantine.

**Known status.** design_docs/2026-07-11_stage_set_tools_plan.md:1707 names full persona namespace migration as open; 1785 excludes it from P5a. Carry it as an acknowledged product gap, not a newly introduced regression.

## H3 — P2: Native graph caching drops part of the shared paint contract

Primary snapshot location: crates/woodshed-genet/src/leaves.rs:276-312,402-453, especially relation hashes at 287-295 and 416-424.

**Trigger and effect.** In Related, add or withdraw a personal reason on an already disclosed pair without changing selection or camera. In Set, hide a non-first generated member of a pair while another eligible member and the first route remain. The current claim summary changes strength/direction, but the host hash checks only relation ID, visibility, emphasis and route. It returns the old native leaf when those stay unchanged. Reader/DOM state can reflect the new claims while the painted stroke retains its old strength/arrows until another hashed property changes.

**Reachability.** Related Add/Withdraw queues journal intents at crates/woodshed-views/src/stage/related.rs:405-416,498-528. crates/woodshed-genet/src/sync.rs:96,448-469 commits and installs the journal. Related generated topology at crates/woodshed-views/src/stage.rs:88-104 remains unchanged; personal assertions separately enter the summary through stage/related.rs:111-145 and stage.rs:158-176. main.rs:365-368 calls leaves::sync_all; leaves.rs:306-307 then reuses the old leaf. For Set, stage/set_tray.rs:283 and stage.rs:1643-1646 change member visibility; stage.rs:328-331 deliberately preserves scene epoch. Grouping/filtering at 688-696,740-769 changes the summary on the same stable pair.

The locked supplier proves these omitted values affect pixels: /Users/markik/.cargo/git/checkouts/mere-9d8e871c24d9ba76/6575075/crates/cambium/cambium/src/graph_canvas.rs:547-558 maps marks into start/end/opposed arrows and strength into glyph strength.

**Owner and competing owner.** GraphCanvasSwatch and the shared painter define actual paint inputs. Woodshed's manually maintained partial paint signature independently approximates that contract.

**Tests and gap.** View and supplier tests cover summaries/rendering, but there is no host cache integration test driving a changed claim through sync_all. leaves.rs tests cover ambient composition; the completely read layout tests do not exercise claim-only leaf invalidation. Overview's constant default marks/strength at stage/overview.rs:298-300 do not have this particular failure.

**Smallest coherent direction.** Give the shared paint DTO a complete paint revision/fingerprint, or at minimum hash marks and strength in both host signatures. Consolidate the fingerprint with the painter so future fields cannot silently escape invalidation.

## H4 — P2: Two editable fields never enter the native text seam

Primary snapshot location: crates/woodshed-genet/src/text.rs:15-21,59-73; installed at crates/woodshed-genet/src/main.rs:448. Missing view fields: crates/woodshed-views/src/stage/related.rs:481-495 (personal_relation_note, claims-reason-field) and crates/woodshed-views/src/stage/overview.rs:978-992 (retained_set_name, overview-save-row).

**Trigger and effect.** Focus the personal-reason note or retained-Set name and use native IME, click to position the caret, or drag a selection. focused_text maps only Search, CardRename and RelationshipLabel, plus the shared workshop/font editor. It returns None for these two mounted textboxes. Ordinary character keys still edit the controls through their map_state/on_key handlers, so a blanket 'typing is broken' claim would be false. IME, host caret painting and pointer-based selection are actually missing.

**Exact supplier trace.** The frozen lock's Mere6575075 rootstock/input.rs:568-580 dispatches ordinary keys regardless of the hook; input.rs:632-643 drops IME when the hook returns None; input.rs:278-303 requires it for pointer caret placement/selection; rootstock/host.rs:1906-1910 disables platform IME; rootstock/frame.rs:561-573 needs it for caret/selection overlays. These paths reside beneath the same cached supplier root named in H3.

**Owner and competing owner.** Woodshed owns borrows into its text buffers; the shared host owns native editing geometry and composition. The view's mounted fields and the hand-enumerated host registry have diverged.

**Tests and gap.** scenarios/claims_seed.scn:21-38 genuinely clicks and inputs ordinary text, explaining why that receipt can pass. It does not exercise composition, pointer caret placement or selection. The completely read host layout tests have no native text-seam test for either field.

**Smallest coherent direction.** Map both fields through a stable field identifier and the same shared text slot, then cover IME and pointer selection. A small reusable field-registration seam would remove the need to update wrapper-class matching independently for each new textbox.

## H5 — P2: Persona reset drops process-owned event-time injection

Primary snapshot location: crates/woodshed-genet/src/persona.rs:202-217, especially *ui = UiState::new() at 213. Supporting paths: crates/woodshed-genet/src/main.rs:168-171; crates/woodshed-genet/src/drive.rs:118-121; crates/woodshed-views/src/stage.rs:1369-1370,2160-2163,2206-2207,2232-2233,2262-2263,2303-2304.

**Trigger and effect.** Switch persona, let the view sit idle, then start or record a practice interaction. Boot injects event_clock once; the reset preserves appearance but replaces that callback with None. refresh_event_time no longer samples interaction time and uses the preceding frame's now_ms, which can precede a long idle interval. Practice timestamps/spans can incorporate idle time. drive.rs explicitly documents why frame-only observations are insufficient.

**Owner and competing owner.** Clock injection belongs to process-owned host wiring. The persona reset recreates initialization without reproducing that host capability.

**Tests and gap.** Reset tests preserve appearance identity but never assert event-clock retention. History tests manually supply now_ms and cannot expose this integration failure.

**Smallest coherent direction.** Preserve/reinject process capabilities through one host initialization helper shared by boot and session replacement. Cover reset with a deterministic event clock and a deliberately stale frame timestamp.

## H6 — P2: Persona reset loses device inventory without the promised refill

Primary snapshot location: crates/woodshed-genet/src/persona.rs:175-183 and crates/woodshed-genet/src/sync.rs:544-560. Supporting paths: crates/woodshed-views/src/stage.rs:946-959 and crates/woodshed-genet/src/main.rs:209-212.

**Trigger and effect.** With MIDI ports present or connected, switch persona and inspect MIDI Settings. UiState::new clears inventories and sets both selections to None, with refresh_requested false. The next dispatch interprets that as disconnect requests. Contrary to persona.rs:181-182's comment, inventories do not refill: sync rescans only on the refresh flag; boot's unconditional discovery never reruns. Only pressing Refresh ports repairs the empty lists.

**Owner and competing owner.** Device discovery is process-owned observation. Session reset treats it as disposable preference state; synchronization assumes boot already filled it.

**Tests and gap.** No assigned persona/sync test asserts inventory restoration through a real reset.

**Smallest coherent direction.** Retain inventory or request a rescan before interpreting reset selections. Decide separately whether selected routes should survive a persona switch; even an intentional disconnect should not require manual rediscovery.

## H7 — P2: MIDI discovery changes reinterpret the user's selected index

Primary snapshot location: crates/woodshed-genet/src/sync.rs:25-32,547-557. UI entry: crates/woodshed-views/src/stage/settings.rs:74-76.

**Trigger and effect.** Select B at dropdown index 2 from [A,B]. Remove A and refresh to discover [B,C]. The code replaces the list while preserving selected index 2, resolves C and connects it. A shorter [B] list instead silently disconnects. Both input and output share the problem. Clock can route to a different device without a new routing choice.

**Owner and competing owner.** User routing intent should be a port identity. A positional dropdown index and the backend's connected name independently represent that intent; discovery replaces the index's meaning.

**Tests and gap.** The fully read synchronization tests do not exercise rescan reorder/disappearance.

**Smallest coherent direction.** Preserve desired port identity across discovery, derive the new index from it, and expose disappearance as unavailable/disconnected. Prefer one canonical desired identity over an index with mutable interpretation.

## H8 — P2: Requested MIDI output is reported as accepted despite worker failure

Primary snapshot location: crates/woodshed-genet/src/midi.rs:98-114,202-203,217-223. Call/display trace: crates/woodshed-genet/src/sync.rs:555-560 and crates/woodshed-views/src/stage/settings.rs:38-42,81.

**Trigger and effect.** Select a stale/unavailable output name or encounter an open failure. connect_output immediately stores the requested name as connected_out and sends a command. The worker owns the actual Option<MidiOut>, turns connect errors into None with .ok(), and has no result/status channel. Settings displays out: <name> even though no connection exists. Future dispatches see the same 'connected' name and do not retry. Send errors are suppressed too.

woodshed-audio/src/midi.rs:304-310,345-353 returns concrete creation/find/connect errors; it is the host adapter that discards them.

**Owner and competing owner.** The worker's accepted connection is authoritative. MidiHost.connected_out is a request represented as accepted fact.

**Tests and gap.** midi.rs has no tests. Adjacent MIDI tests cover parsing/tempo, not host request/worker admission/status.

**Smallest coherent direction.** Add a worker result/status channel, retain requested and accepted identities separately, and publish failures. Derive displayed connection state from accepted worker state. A device-free connector stub can test refusal without hardware.

## H9 — P2: MIDI transport state survives replacement of the connection it belongs to

Primary snapshot location: crates/woodshed-genet/src/midi.rs:192-225. Normal route: Settings picker to sync::midi_devices to ClockCmd::Connect; clock state is fed by main.rs:362-363 and drive.rs:181-183.

**Trigger and effect.** With output A, Send clock enabled and transport running, select B. Connect replaces conn while last_transport remains true. Since want is still true, the transport-change branch is skipped: A receives no Stop before being dropped and B receives clock ticks without Start. Devices requiring Start do not begin the intended sequence. Selecting None also drops A before any Stop can reach it. Shutdown's Stop command returns immediately without sending a MIDI Stop as well.

**Owner and competing owner.** Transport state is connection-specific. The worker's global last_transport boolean survives a connection identity change.

**Tests and gap.** There is no host clock-loop test for connect/switch/disconnect/shutdown message order. This is a source-proven emitted-message omission, not a claim of observed physical device behavior.

**Smallest coherent direction.** Stop the previous accepted connection before replacement/shutdown, reset per-connection transport state and Start a newly accepted connection when master playback is already running. This belongs in the same acknowledged connection state machine as H8.

## H10 — P3: Dormant capture commands preserve a weaker parallel executor

Snapshot locations: crates/woodshed-core/src/audio.rs:171-175 and crates/woodshed-genet/src/sync.rs:193-243; backend entry points at crates/woodshed-genet/src/audio.rs:934-945. Active path: crates/woodshed-genet/src/sync.rs:477-539 and crates/woodshed-views/src/stage/looper/recorder.rs:573-607,635-651.

Whole frozen-tree caller searches found no producers for AudioRequest::{SongRecordToggle,SongClearLoop,LoopRecordToggle}. These old arms select mutable song_edit_cursor or stop whichever capture is busy, whereas the current RecorderRequest route freezes session/backing/target and qualifies Stop by epoch. song_clear_loop also bypasses the busy check in recorder_clear_saved at audio.rs:296-300.

There is no current production execution trigger in this tree; do not report this as a reachable destructive UI bug. The effect is retained, weaker command machinery that can confuse ownership and future callers. Establish external public-core compatibility, then retire/deprecate those variants and their duplicate branches together. Preserve LoopClear: actual global clear controls at stage/looper.rs:467-495 use it to address retained unavailable/orphaned takes without admitted backing.

Tests/gap: the current production recorder route carries qualified requests; no view action in the frozen tree produces these three legacy variants, so a current UI-flow test cannot establish their required behavior. Source searches cannot resolve out-of-tree public API compatibility. Keep removal separate from claims that the active recorder flow is broken.

## H11 — P3: Capture busy status has a test-only second definition

Snapshot locations: crates/woodshed-genet/src/capture_persistence.rs:39,57-64,346,535.

awaiting_commit is set and cleared in production, but its sole reader is is_busy, whose callers all reside in tests. Real lifecycle callers use preparation_busy; failed publication is represented by verified SessionStore save problems. This is parallel unused status, not evidence of a live preservation failure by itself. It can make assertions on an obsolete reservation look like coverage of an actual close/transition decision.

Tests/gap: capture_persistence.rs:1040-1061,1112-1128,1212-1229 and other tests assert is_busy directly, while the actual close-hook tests are at 1348-1458. Every preparation updates the flag, but no production decision reads that combined status. Direct assertions should not be substituted for the real transition outcome.

Smallest direction: remove the unused field/method and assert production preservation outcomes through the real hooks, or assign one explicit production purpose and eliminate the overlapping approximation.

## Known dirty P5a work: separate from unresolved findings

The frozen preview path at sync.rs:138-151 submits through void preview_pitches. drive.rs:232-239 advances/records an occurrence before preview admission. The runtime can reject resource bounds/backpressure at woodshed-audio/src/output_runtime.rs:621-637 and can return success without sounding preview while backing plays at 618-619. This belongs to the authorized timing/admission work already in progress.

Read-only primary comparison shows typed CardPreview requests, admit_card_preview, CardPreviewAdmission, rehearsal_pending and typed dwell-error handling in audio.rs/drive.rs/sync.rs and supporting sources. The coordinator directly read the sync diff; audio reviewer inspected audio/drive diffs. Treat the frozen deficiency as under active local repair, not an additional independent fix request or a qualified published fix. No compile/test/native claim is made for that dirty work.

At inspection, primary persona.rs, storage.rs, capture_persistence.rs, codec_worker.rs, midi.rs, leaves.rs, text.rs, appearance.rs, fonts.rs and wav_export.rs were byte-identical to the snapshot. Relevant MIDI synchronization remains outside the inspected P5a diff. Other primary files differ and were not treated as audit baseline or edited.

## Deliberate authority and false-positive rebuttals

- CpalBackend, LoopCaptures, Woodshed capture custody and backing-plus-take WAV export preserve heterogeneous Set timing, stable WorkingSetId/CardId ownership and a single product-owned OutputRuntime. They do not create a shadow Hocket Session/History merely because Woodshed composes a shared recorder surface. The current audio rehome/shared capture plans explicitly retain scheduling, integrity and custody locally.
- Saved SongDoc and Set-derived LoopPlan coexist intentionally pending a documented migration choice. Saved songs retain authored frequencies/formula/root/per-bar data that cannot be discarded merely to remove a second type. last_song/last_loop_plan distinguish admitted backend state from desired UI edits; merging them would erase a tested refusal boundary.
- Capture codec work owns encoding facts; durable SessionStore acknowledgement decides manifest publication. Corrupt/missing/unknown-codec companions retain their references, and ordinary saves cannot silently erase them. Recovery tests exercise real backend damage and explicit Clear.
- FsBackend::put's no-path Ok does not establish a current false durable-success path: SessionStore verifies exact readback and records failure.
- WAV replacement's fingerprint/rename interval is documented as lacking filesystem compare-and-swap against arbitrary external writers. It is not represented here as a newly hidden atomicity guarantee. Actual UI busy/pending/replacement gates also rebut an unproven export worker-overwrite claim.
- Shared Tabard authoring, font-role rendering, native host lifecycle and Mesquite scenario pump are composed through their owning services. Appearance-specific storage choice, colors, text-buffer borrows and close policy are appropriate Woodshed bindings.
- Scenario act handlers that emit boolean observations and return 'recognized' are followed by explicit assert-event checks in claims_seed.scn and claims_reopen.scn. They are not false successes just because act itself returns true. Claims input-text covers normal key insertion, not H4's IME/caret contract.
- await-selected-take-playback observes an actual device output cursor reaching the selected segment. verify-mere-take-retention separately compares sealed companion and hydrated PCM bits. The plan explicitly leaves physical audibility/latency open; this observation does not itself promise audible take output.
- External MIDI Start/Continue/Stop following is described in an older design note, while the current public seam promises BPM following/recent events. Its absent input-transport control is left as a scope question instead of asserted as a regression.

## Remaining questions and limits

1. What preservation/discard interaction should an explicit persona switch use, and should selecting the already-active persona be a no-op?
2. What migration and environment-override policy should admit persona-scoped mutable practice roots? Current source protects foreign ciphertext but does not implement that product promise.
3. Should MIDI routing selection survive a persona change? Discovery inventory and worker-accepted status must still remain honest regardless of that choice.
4. Do external consumers construct the three unproduced public capture variants?
5. Is full incoming MIDI transport control intentionally deferred?
6. The coordinator and reviewers established source reachability only. Device/physical effects, native interaction, focused regression execution and fresh-process acceptance remain future validation work.

The complete assigned coverage is 20 files / 18,828 lines; no assigned line remains unread by the collective host review.
