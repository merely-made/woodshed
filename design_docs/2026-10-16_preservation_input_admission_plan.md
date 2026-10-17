# Preservation and input admission

Date: 2026-10-16.
Status: October 16 isolated implementation passed device-free gates; October 17 integration onto P5 main passed current-source CPU gates and all three desktop builds; local handoff ready. Native qualification/publication are owned by the coordinating chat after local handoff.

## Scope and ownership

This is the first remediation pass from the complete-source audit frozen at
`d12201bf`. Implementation began at `2c59bb8` and was fast-forwarded to `21d0c2e`,
including the independently completed catalog/discovery admission-history receipt.
The subsequent P5 shared-clock lane retains primary-checkout source and native
ownership. Audit remediation runs in an
isolated checkout and launches no native apps/devices. The integration owner
serializes focused Cargo checks after agreement with the native lane.

- Integration owner: Woodshed preservation before persona replacement, shared
  host preservation decisions, documentation, integration and source review.
- Core-input lane: history wire discrimination/structural admission and loaded
  Set occurrence identities (`woodshed-core` history/storage, `woodshedding`
  rehearsal).
- Hocket-model-input lane: model topology and portable project/handoff admission.
- Hocket-preservation lane: native project dirty/save/close/replacement admission.
- Redshank-recovery lane: storage load outcome and nonwriting failed-restore host.

Agents edit disjoint paths, do not commit/push, and do not run Cargo/native
independently. The parent owns integration and the eventual scoped local commit.
No supplier repin, broad persona namespace migration, audio scheduling change,
old PCM rewrite, new clock policy, or other audit cleanup is included.

## Phases and done-conditions

### P1. Preserve outgoing work before replacement

Use one host-owned preservation result before close or identity/project change.
Woodshed retains failed/unconfirmed saves, active captures, codec work and
arrangement drafts until safe completion or an explicit discard. Hocket protects
unsaved full project revisions, save work and capture tickets. Save failure or
canceled Save retains live work; capture replacement cannot orphan an execution.

Done when device-free production-hook/state regressions cover rejection, retry,
discard, cancellation and preservation of outgoing data before any incoming reset.

### P2. Reject invalid durable input without adopting writable defaults

Discriminate real legacy history from malformed current history. Check structural
references/topology and Set identity invariants before restoration. Keep valid
legacy migrations and retained branches. Redshank distinguishes no data from
failed recovery and cannot publish/prune default state after failed restoration.

Done when corrupt/index/cycle/duplicate-identity negative fixtures return ordinary
admission errors, old bytes remain intact, and valid current/legacy/fallback paths
retain their exact semantics.

### P3. Integrate and verify without disturbing native qualification

Review the combined changes, run focused tests in a separate Cargo target during
an agreed idle slot, and record exact source, commands and outcomes. Broader checks
follow only from changed seams or failures. Native/physical acceptance remains
separate. Commit locally only; publication is not requested by this task.

Done when relevant focused tests pass, the diff and documentation are reconciled,
and the local commit/worktree state and remaining limits are reported.

## Findings

2026-10-16: current persona settlement still opens incoming custody/resets the UI
before the existing close save barrier. The core/history, Hocket and Redshank
findings are rechecked by their owning lanes before implementation. Prior audit
coverage is source evidence, not test/native acceptance.

## Progress

2026-10-16: fetched primary; HEAD `2c59bb8`, origin/main `d3a1350`. Isolated detached
checkout created from the current local source; primary source untouched. Four
disjoint implementation lanes started, with Cargo/native prohibited until scheduled.


## Implemented contracts

- **Woodshed host H1:** close and persona replacement share a preservation
  result. Failed/unconfirmed stores, active capture and uncommitted drafts retain
  outgoing custody; codec/WAV work finishes before a preserving transition.
  Retry, Cancel, and explicit Discard remain separate decisions. Incoming custody
  must also be admitted before resetting the outgoing session. The process event
  clock survives a successful reset (host H5).
- **Core CORE-1/CORE-2:** recognized current history fields cannot fall through to
  empty legacy history. The locked supplier has no fallible snapshot constructor,
  so a narrowly scoped consumer guard checks references, topology and ownership
  before using its constructor. Valid legacy history still migrates. This guard
  should move to a supplier checked-construction API when available.
- **Loaded Set identity admission:** assigned Card IDs must be unique, legacy
  zero IDs migrate above the retained mint, and exhausted identity space is an
  admission failure. Checks cover active, inactive working and retained Sets;
  ambiguous existing IDs are never silently renumbered.
- **Shared S1:** Hocket model owns graph validation. Complete project archives
  and authenticated handoffs require a valid rooted retained graph before save,
  receive or adoption. Model integration still permits a valid partial node-set
  union. Checkout checks both required ancestry paths with bounded traversal
  before edits; off-path corruption is rejected at whole-graph admission.
- **Hocket H1/H2/H4:** full project revision replaces the saved-head shortcut.
  Native/caption/scenario/update exit and Open/Accept/Restart replacement share
  host preservation admission. Active capture or admitted I/O blocks replacement;
  successful save acknowledges the submitted revision, preserving later edits as
  dirty. Failed/canceled Save retains the pending decision and outgoing project.
- **Redshank R01:** an empty directory differs from an unrecoverable listed
  generation set. Readable older valid generations and disappeared-newest
  fallback remain available. Failed restore installs a nonwriting recovery host,
  with a latched notice and no save worker; edits, acknowledgements and close
  cannot publish or prune default state.

## Review and verification record

Four disjoint implementation agents completed source changes and then reviewed
other lanes read-only. Peer review identified incoming custody failure retention,
partial graph-union compatibility and vanished-generation fallback; those issues
are corrected before the final gate. The existing 16 ms history-scrub assertion
is retained: full graph admission belongs at durable boundaries, while checkout
checks only the paths it will execute.

The first root run failed to compile a test helper that used an undeclared
executor; the helper now polls the immediate in-memory backend directly. The
second root run caught redundant whole-graph validation in every scrub. A further
root run caught an appearance-close fixture with no installed session/store; the
fixture now supplies admitted in-memory persistence before exercising deferred
close. These failures are recorded as failures, not passing receipt evidence.

Final command exits, case accounting, changed-source hashes and remaining gates
are recorded in `validation/preservation-input_20261016/receipt.json`.

## Qualification boundary and remaining work

This slice qualifies source and device-free tests only. No native app, device,
profile migration, physical microphone or playback qualification is performed.
Existing native receipts refer to their own historical source. New Retry/Discard
panels and OS close paths still need an independently scheduled native receipt.
The combined codec-worker-wake-to-persona-install path has unit coverage of its
barrier and transition separately; it has no new headed/fresh-process receipt.

Persona namespace migration, Hocket identity-confirmation behavior (H3), missing
media presentation (H5), audibility/tempo replay, MIDI/runtime authority, broader
receipt-checker fixes and other audit findings remain separate work. No supplier
pin, clock rule or PCM data is changed by this remediation.


2026-10-16 final gate: root 965, Hocket 90, Redshank 95 distinct passing
cases (1,150 total), plus five existing capture child-process invocations counted
separately. Nine ignored tests remain ignored. All three final Cargo commands
exited zero; the root gate includes the unchanged history scrub timing assertion.
Source/design-document `git diff --check 21d0c2e -- crates ports design_docs`
passes. Raw Cargo logs retain terminal blank lines; the initial staged whitespace
check flagged those three logs (exit 2), recorded separately in the receipt.
Primary source and native validation were untouched by
this lane; the scheduling slot was explicitly released to the P5 owner afterward.


## Integration onto P5 main — 2026-10-17

Primary was refreshed clean at `b88cb26836cffb4c4b091d15c53bb1014d575cdc`,
matching `origin/main`. The two isolated remediation commits `07c170b` and
`860c5c1` are applied locally. Additive conflicts preserve both the P5 and audit
index entries, and both `practice_clock_managed` and injected `event_clock` across
persona reset. The reset regression now checks both bindings while ensuring run
position and processor program remain cleared.

This lane owns primary source and CPU checks until a clean local commit handoff.
It launches no native applications and does not publish. The coordinating P6 chat
owns the subsequent bounded preservation UI/OS/reopen receipt and publication;
P6 itself remains outside this integration. Historical October 16 receipts retain
their original hashes, commands and qualification limits. Current-source results
are recorded separately in `validation/preservation-input_20261017/integration.json`.


2026-10-17 integrated test gate: root seven-package gate 1,170 passing cases,
Hocket host 90, Redshank desktop/storage 95 (1,355 total). All three processes
exited zero. The root gate includes P5 audio/runtime cases and the unchanged
history scrub assertion. Five capture child invocations remain separately counted;
ignored cases are recorded directly in the current receipt. Three reviewers read
the complete automatically merged stage/persona/main/audio/index files and
compared input sources against the original commit; no merge blocker remained.
A stale index sentence now distinguishes completed capture primitive extraction
from remaining DSP consolidation. Serial desktop builds precede the local handoff.


2026-10-17 final local integration gate: all three desktop builds exit zero;
1,355 passing cases plus five separately counted capture child invocations, with
nine existing ignores. Current changed-source and executable hashes match the
receipt. Supplier lockfiles are byte-identical to P5 baseline. No native app was
launched here. The coordinating chat receives the clean local revision, exact
binaries, selector/fixture guidance and source/build/native scheduling ownership.
