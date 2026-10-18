# Safety closure reconciliation — 2026-10-17

**Coordinator disposition:** A1, A4 and A5 implementations are present with historical qualification at the inputs below. A2 remains partial: Hocket H1/H2/H4 are implemented and CPU/build qualified; **Hocket H5 remains unresolved**. Confirmation-only **Hocket H3 persona binding remains unresolved**. No current-head native acceptance is established.

Current source: `/Users/markik/Code/repos/woodshed`, user-supplied HEAD label `a424772` (including local label commits). No Git command/ref inspection, tests, builds, native launches, network or messages were performed. This pass writes only `safety.md` and `safety.json`; source, docs and other owners' outputs remain untouched. The 937-file/231,190-line frozen `d12201bf` baseline is retained as parent context, not reread as a new complete audit.

## Evidence boundaries

- **CPU16:** [receipt](/Users/markik/Code/repos/woodshed/validation/preservation-input_20261016/receipt.json), baseline `21d0c2e697165770d9f5787dadc91f526c8e2b02`, imported implementation label `07c170b` per plan. The receipt has no final tested-commit field; qualification is baseline plus source hashes, all matching the retained isolated snapshot. Real final logs agree with the recorded zero exits: root **965**, Hocket **90**, Redshank **95** = **1,150** parent/doc cases; **five** root child invocations counted separately; **nine** ignored. `final-runs.json` lists only root/Hocket; its entries equal the corresponding receipt entries. Redshank command/exit comes from `receipt.json` with independent raw-log corroboration, not that incomplete convenience index. Three prior root failures (undeclared pollster, 36.285541 ms scrub against unchanged <16 ms gate, missing appearance-close persistence fixture) remain failures, superseded by final runs.
- **CPU17:** [integration receipt](/Users/markik/Code/repos/woodshed/validation/preservation-input_20261017/integration.json), baseline `b88cb26836cffb4c4b091d15c53bb1014d575cdc`, imported `07c170b`/`860c5c1`. Plan and native launch bind the final candidate to `cec221348154fc8e6abe3f6136f00b14f9860b78`; all 23 integration source hashes match that native source manifest. Real logs agree with recorded zero exits: root **1,170**, Hocket **90**, Redshank **95** = **1,355** parent/doc cases, **five** separate child invocations, **nine** ignored. All three desktop build logs end in Finished dev with recorded exit zero. These are historical executions, not a424772 tests.
- **Woodshed native only:** [receipt](/Users/markik/Code/repos/woodshed/validation/preservation-input_20261017/native/receipt.json), [completion](/Users/markik/Code/repos/woodshed/validation/preservation-input_20261017/native/completion.json), real `native.log`/`scenario.done`, launch/scenario bindings and native-close action records inspected. Persona / fresh quarantine reopen at `cec2213`: **191 frames / 6 captures**, **98 / 1**. Close / fresh quarantine reopen at `d955f5dc3b99ec1efd6ec9a56dd3ba007755409e`: **139 / 4**, **97 / 1**. All four real logs report RESULT ok. Scenario assertions cover three retained Cards, Retry/Cancel, explicit discard/reset and zero Cards on fresh quarantine reopen. Native-close records establish actual macOS close → Retry → explicit Discard, owned-process exit and preserved original bytes; RESULT ok alone does not establish final close.
- All **70** native artifact-manifest entries and all four phase manifests match their recorded hashes. Original receipt records twelve visually reviewed captures; this pass does not claim a new visual review or rerun. Excluded occlusion, selector/wording and invalid reopen-grammar attempts remain excluded. Quarantine native input is invalid JSON (`expected value at line 1 column 1`), not a negative history-index/Card-ID fixture. Held codec-worker sequencing, incoming-custody failure and capture replacement rejection remain CPU-only. Hocket/Redshank have no native receipt from this work; physical audio is unqualified.

## Per-finding dispositions

### host:H1 — A1 — implemented; historical CPU and bounded Woodshed native/reopen qualified

Persona replacement calls the same preserve_request used by close, retains blocked/preparing work, distinguishes Retry/Cancel/Discard, waits for finishing WAV export, and rejects Pending incoming custody before outgoing reset. after_wake resumes the pending switch. Explicit discard is the caller-owned bypass.

**Current callers:** [persona.rs:143–218](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/persona.rs:143); [persona.rs:279–327](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/persona.rs:279); [capture_persistence.rs:648–704](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/capture_persistence.rs:648); [main.rs:175–212](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/main.rs:175); [main.rs:456–511](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/main.rs:456).
**Regression source:** [persona_preservation_tests.rs:116–311](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/persona_preservation_tests.rs:116).
**Receipt scope:** CPU16 root and CPU17 root; persona test file and persona.rs match CPU17 hashes. N-persona/ N-persona-reopen at cec221348154fc8e6abe3f6136f00b14f9860b78; N-close/ N-quarantine-reopen at d955f5dc3b99ec1efd6ec9a56dd3ba007755409e. Only corrupt-input quarantine, three authored Cards, Retry/Cancel/explicit Discard, original bytes and fresh quarantine reopen.
**Limits / blockers:** Current capture_persistence.rs/main.rs and dependencies differ from both native inputs; no a424772 native acceptance. Held codec worker, failed incoming custody and capture replacement are CPU-only. Combined codec-worker wake to persona installation has no scoped native proof.

### host:H5 — A1 — implemented; historical CPU qualified

reset_for_persona carries event_clock and practice_clock_managed across UiState replacement; current persona.rs is byte-identical to CPU17 and both historical native source manifests.

**Current callers:** [persona.rs:330–348](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/persona.rs:330); [main.rs:228–234](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/main.rs:228).
**Regression source:** [persona.rs:366–380](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/persona.rs:366); [persona_preservation_tests.rs:287–311](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/persona_preservation_tests.rs:287).
**Receipt scope:** CPU16 reset/event-clock regression; CPU17 combined reset regression. Native receipts exercise reset but do not assert event timestamps after long idle.
**Limits / blockers:** No current-head test run; no long-idle/event-timestamp native assertion in these receipts.

### core:CORE-1 — A4 — implemented; historical CPU qualified

Any recognized entries/visits/owners key commits to current decoding. Legacy FlatLog requires events. Decode errors flow through session restore to refused writes instead of empty-history adoption.

**Current callers:** [history.rs:390–395](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/history.rs:390); [history.rs:535–573](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/history.rs:535); [storage.rs:524–526](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/storage.rs:524); [session.rs:45–80](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/session.rs:45).
**Regression source:** [history.rs:1019–1040](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/history.rs:1019); [storage.rs:1017–1051](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/storage.rs:1017).
**Receipt scope:** CPU16/CPU17 root. history.rs unchanged from both tested hashes. storage.rs now differs only by seconds: None in a test capture fixture versus the hash-verified CPU16 source. Native quarantine input is syntactically invalid JSON (logs say expected value at line 1 column 1); it does not qualify the malformed-current-history discriminator fixture.
**Limits / blockers:** Exact malformed-current history native/reopen case absent; no current-head execution.

### core:CORE-2 — A4 — implemented via consumer guard; historical CPU qualified

Consumer validates snapshot bounds, duplicate identities/keys, reciprocal links, iterative visit/owner acyclicity, ownership and visit counts before trusted Lineage::from_snapshot. Real session caller quarantines returned errors.

**Current callers:** [history.rs:397–527](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/history.rs:397); [history.rs:548–552](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/history.rs:548); [session.rs:61–80](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/session.rs:61).
**Regression source:** [storage.rs:1017–1051](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/storage.rs:1017).
**Receipt scope:** CPU16/CPU17 root; guard file byte-identical. Historical integration records supplier-invariant peer review.
**Limits / blockers:** Supplier-owned fallible reconstruction API remains future consolidation; current code still calls trusted from_snapshot after local validation. Supplier has since repinned (locks differ); no new supplier/runtime validation was performed here. No malformed-index/cycle native fixture in the bounded quarantine receipt.

### shared:S1 — A4 — implemented; historical CPU qualified

Model owns whole-graph validation for key/id, root/head, references, acyclicity and rooted reachability; checkout bounds both executed ancestry walks. Integration validates a temporary union, preserving compatible fragments. Project decode/save, signed handoff create/receive, portable accept and host adoption call validation before mutation.

**Current callers:** [history.rs:404–450](/Users/markik/Code/repos/woodshed/crates/hocket-model/src/history.rs:404); [history.rs:475–566](/Users/markik/Code/repos/woodshed/crates/hocket-model/src/history.rs:475); [persistence.rs:87–109](/Users/markik/Code/repos/woodshed/crates/hocket-model/src/persistence.rs:87); [project_store.rs:128–133](/Users/markik/Code/repos/woodshed/crates/hocket-project/src/project_store.rs:128); [project_store.rs:199–230](/Users/markik/Code/repos/woodshed/crates/hocket-project/src/project_store.rs:199); [handoff.rs:158–164](/Users/markik/Code/repos/woodshed/crates/hocket-project/src/handoff.rs:158); [handoff.rs:263–271](/Users/markik/Code/repos/woodshed/crates/hocket-project/src/handoff.rs:263); [handoff.rs:309–379](/Users/markik/Code/repos/woodshed/crates/hocket-project/src/handoff.rs:309); [state.rs:688–704](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:688); [state.rs:1137–1154](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:1137).
**Regression source:** [native_lifecycle_tests.rs:128–158](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/native_lifecycle_tests.rs:128).
**Receipt scope:** CPU16/CPU17 root (hocket-model/project) plus Hocket host 90 passing/2 ignored. All four model/project receipt source hashes and host state hash match current. The filename native_lifecycle_tests.rs denotes device-free mounted-hook tests, not an OS/native run.
**Limits / blockers:** Ports have CPU/build evidence only; dependency locks differ, so unchanged local files do not establish a current-head integrated pass.

### theory:T4 — A4 — implemented; historical CPU qualified

Set::admit_card_ids preflights duplicate assigned IDs and mint exhaustion atomically, migrates only zero IDs, and is reused in copy. decode_session applies admission to active, inactive and retained Sets; IDs may legitimately overlap across separate Sets.

**Current callers:** [rehearsal.rs:525–569](/Users/markik/Code/repos/woodshed/crates/woodshedding/src/rehearsal.rs:525); [storage.rs:524–544](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/storage.rs:524); [session.rs:61–80](/Users/markik/Code/repos/woodshed/crates/woodshed-genet/src/session.rs:61).
**Regression source:** [storage.rs:1068–1135](/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/storage.rs:1068).
**Receipt scope:** CPU16/CPU17 root. Current admission body is unchanged versus hash-verified CPU16 source. rehearsal.rs differs only by added Repeat::Card variant, and is identical to the d955f5d close-source manifest; storage.rs only adds seconds: None in a test fixture.
**Limits / blockers:** No duplicate-ID native/reopen fixture in bounded receipts; no current-head test run.

### hocket:H1 — A2 — implemented host refusal/defer policy; historical CPU/build only

Accept enters preservation. Open checks live capture at dispatch and again through matching completion/preservation. resolve_preservation refuses active capture and pending I/O even for Discard, preserving the stop token and staged incoming payload.

**Current callers:** [state.rs:680–685](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:680); [state.rs:795–818](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:795); [state.rs:973–1009](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:973); [state.rs:1124–1163](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:1124); [lib.rs:849–868](/Users/markik/Code/repos/woodshed/crates/hocket-recorder/src/lib.rs:849).
**Regression source:** [native_lifecycle_tests.rs:280–365](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/native_lifecycle_tests.rs:280).
**Receipt scope:** CPU16/CPU17 Hocket 90 passing/2 ignored, plus CPU17 desktop build. Relevant state/main/test source hashes match current. Free-capture tests use fake device execution with real recorder services.
**Limits / blockers:** Shared Recorder::replace_project still clears the ticket if directly called during recording; fix is the Woodshed-family Hocket host admission policy, not a generic replacement-during-capture retirement handshake. No Hocket native, physical capture, or current-head integrated qualification.

### hocket:H2 — A2 — implemented; historical CPU/build only

Native/dispatch/frame exit hooks enter product close admission, appearance cancellation retains the window, revision-bound approval waits for capture/I/O and dirty-project resolution; update restart re-enters this path. Failed/canceled Save retains the pending decision.

**Current callers:** [main.rs:154–183](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/main.rs:154); [main.rs:218–244](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/main.rs:218); [state.rs:821–856](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:821); [state.rs:987–1001](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:987); [state.rs:1068–1178](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:1068).
**Regression source:** [native_lifecycle_tests.rs:28–112](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/native_lifecycle_tests.rs:28); [native_lifecycle_tests.rs:391–449](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/native_lifecycle_tests.rs:391).
**Receipt scope:** CPU16/CPU17 Hocket; CPU17 desktop build. Mounted production-hook assertions are CPU tests.
**Limits / blockers:** No actual Hocket OS-close, Save-dialog, restart or fresh-process native receipt from this work.

### hocket:H4 — A2 — implemented; historical CPU/build only

saved_revision compares complete session/history plus append-only content-addressed media count and missing-media facts. Save acknowledges the submitted snapshot, so concurrent edits or same-head imported branches/media remain dirty.

**Current callers:** [state.rs:68–91](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:68); [state.rs:775–783](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:775); [state.rs:987–1001](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:987); [state.rs:1032–1034](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:1032).
**Regression source:** [native_lifecycle_tests.rs:113–126](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/native_lifecycle_tests.rs:113); [native_lifecycle_tests.rs:189–219](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/native_lifecycle_tests.rs:189); [project_correlation_tests.rs:237–254](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/project_correlation_tests.rs:237).
**Receipt scope:** CPU16/CPU17 Hocket; same-head retained branch/media import regression asserts dirty and close refusal. Current state and regression files match recorded source hashes.
**Limits / blockers:** Specific off-head import test stops at in-memory dirty/close assertions; it is not a native save/reopen/full-bundle comparison. No current-head pass and no Hocket native/reopen gate.

### hocket:H5 — A2 — unresolved

Same-session acceptance imports media into a cloned store but replaces the recorder with self.recorder.missing_media.clone(). Recorder directly installs that set. Missing facts are not recomputed after imported blobs become available.

**Current callers:** [state.rs:700–713](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:700); [handoff.rs:361–379](/Users/markik/Code/repos/woodshed/crates/hocket-project/src/handoff.rs:361); [lib.rs:849–868](/Users/markik/Code/repos/woodshed/crates/hocket-recorder/src/lib.rs:849).
**Receipt scope:** Plan explicitly excludes missing-media presentation (H5); Hocket passing suite/build is not evidence of this defect being fixed. Current host implementation is identical to both CPU receipt hashes.
**Limits / blockers:** Recompute missing-media facts after successful same-session import and qualify the missing-then-restored-media case before closing H5. No fix authorized in this reconciliation.

### redshank:R01 — A5 — implemented; historical CPU/build only

Storage differentiates empty inventory from exhausted invalid generations, propagates restore I/O errors, permits readable older fallback, and refuses publication/pruning unless load succeeds. Failed desktop restore starts an in-memory model with no save worker/sender, latched recovery error, blocked flush and ignored save acknowledgements.

**Current callers:** [lib.rs:108–132](/Users/markik/Code/repos/woodshed/ports/redshank/storage/src/lib.rs:108); [lib.rs:178–181](/Users/markik/Code/repos/woodshed/ports/redshank/storage/src/lib.rs:178); [main.rs:182–208](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:182); [main.rs:281–298](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:281); [main.rs:335–348](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:335); [main.rs:592–601](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:592).
**Regression source:** [lib.rs:454–503](/Users/markik/Code/repos/woodshed/ports/redshank/storage/src/lib.rs:454); [lib.rs:531–577](/Users/markik/Code/repos/woodshed/ports/redshank/storage/src/lib.rs:531); [main.rs:3248–3341](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:3248).
**Receipt scope:** CPU16/CPU17 Redshank: 80 desktop + 15 storage, no ignores; CPU17 desktop build. Both product files match both receipts. Invalid-generation and I/O fixtures check original bytes and inventory; valid older fallback tested separately.
**Limits / blockers:** No Redshank native/reopen acceptance. Recovery/reset interaction remains a separate product decision; current safe policy is nonwriting until repair and reopen. Dependency lock differs from receipts.

### hocket:H3 — A3 confirmation only — unresolved; persona binding still open

Offer stages cached family profile and losing token. Confirmation takes _staged but ignores it. join_family reconnects via CustodyIdentity::connect, writes the cached profile with the returned master key, and displays the cached profile. No target-profile/key equality or renewed consent check is present.

**Current callers:** [state.rs:552–562](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:552); [state.rs:588–607](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:588); [identity.rs:207–227](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/identity.rs:207).
**Receipt scope:** Preservation plan explicitly leaves H3 separate; current state.rs is identical to CPU16/CPU17, so passing those suites cannot establish a binding fix.
**Limits / blockers:** Bind shown profile and key to actual accepted identity or re-offer on change; add change-between-offer-and-confirmation and persisted-consent/reopen evidence before closure. No fix authorized here.

## Source drift and closure limits

Exact recorded/current SHA-256 pairs for all 23 CPU source entries and every changed native-manifest entry are in `safety.json`. Hash equality describes local source identity only; dependency or host drift prevents promotion to current integrated acceptance.

| Historical input | Bound paths | Match current | Differ |
| --- | ---: | ---: | ---: |
| N-persona | 328 | 275 | 53 |
| N-close | 331 | 285 | 46 |

Named relevant differences: CPU17 differs on `woodshed-core/storage.rs`, `woodshed-genet/{audio,capture_persistence,main}.rs`, `woodshed-views/stage.rs`, and `woodshedding/rehearsal.rs`. CPU16 also differs on `woodshed-genet/persona.rs`. Current persona.rs and preservation regression file match CPU17 and the historical native source bindings. Hocket model/project files, Hocket state/main/tests and Redshank storage/desktop implementation match both CPU source receipts. All three Cargo.lock files differ from the recorded CPU17 locks.

Content comparison against the hash-verified isolated CPU16 snapshot identifies the small deltas: storage.rs adds only `seconds: None` to a test capture fixture; rehearsal.rs adds only `Repeat::Card`; persona.rs additionally retains `practice_clock_managed` and adds the combined clock-reset regression (already covered in CPU17). `decode_session`, `Set::admit_card_ids` and `preserve_request` bodies remain identical to that snapshot. `main.rs` adds the shared dispatch/frame close guard and enables practice-clock management; the guard is historically native-qualified at d955f5d, but current main.rs still differs from its hash. `capture_persistence.rs` now carries seconds metadata through equality/restore/references, checked frame conversion, seconds clock selection and retained-take matching, plus new tests. Those pipeline changes are material to source identity even though the preservation barrier body is unchanged. No exact historical native source text for changed main/capture files is asserted beyond the recorded hash comparison.

Additional native-manifest drift includes audio/runtime, arrangement, sync, host storage, Stage projection, Cargo manifests/locks and port appearance/scenario files. This report inventories those changed hashes without auditing unrelated behavior. Neither unchanged persona/reset code nor a prior native success licenses current-head acceptance.

For parent indexing: close the targeted implementation reconciliation for A1/A4/A5 with these evidence limits; retain A2 as partial due to H5 and leave A3/H3 open. A4 uses an interim consumer guard rather than a new supplier checked API. Retain separate future gates for current integrated execution, Hocket/Redshank native/reopen, exact malformed-model native cases and codec-wake sequencing; no new fixes or qualification runs were started. Publication is unexamined.
