# Preservation and input admission

Date: 2026-10-16; updated 2026-10-21.
Status: Integrated onto P5 at `cec221348154fc8e6abe3f6136f00b14f9860b78`; the shared after-frame/dispatch guard follow-up is at `d955f5dc3b99ec1efd6ec9a56dd3ba007755409e`. Current-source CPU/build and bounded Woodshed native/reopen gates pass. Direct publication to `main` is authorized for the coordinating owner; Git refs establish publication separately. Hocket and Redshank have CPU/build evidence only. No physical audio qualification is claimed.

2026-10-20 recheck: the user authorized parallel A1–A5/Q1 work and local commits
only. The current wave overrides this document's historical publication
authorization. Existing preservation/admission fixes remain; a bounded A1 guard
now refuses incoming custody sealed as another persona before retiring outgoing
work. Current device-free results and remaining design/native boundaries are in
the [first-wave recheck receipt](../validation/first-wave-recheck_20261020/README.md).
P5a stays with its existing owner. No push was performed.

## Scope and ownership

This is the first remediation pass from the complete-source audit frozen at
`d12201bf`. Implementation began at `2c59bb8` and was fast-forwarded to `21d0c2e`,
including the independently completed catalog/discovery admission-history receipt.
The work was integrated onto P5 and then extended with bounded Woodshed
preservation UI/OS/reopen qualification. The coordinating owner serializes
integration, validation and publication.

- Integration owner: Woodshed preservation before persona replacement, shared
  host preservation decisions, documentation, integration and source review.
- Core-input lane: history wire discrimination/structural admission and loaded
  Set occurrence identities (`woodshed-core` history/storage, `woodshedding`
  rehearsal).
- Hocket-model-input lane: model topology and portable project/handoff admission.
- Hocket-preservation lane: native project dirty/save/close/replacement admission.
- Redshank-recovery lane: storage load outcome and nonwriting failed-restore host.

Agents edit disjoint paths and do not run Cargo/native independently. Direct
publication to `main` is explicitly authorized for the coordinating owner; this
authorization does not mean a push has occurred. No supplier repin, broad persona
namespace migration, audio scheduling change, old PCM rewrite, new clock policy,
or other audit cleanup is included.

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
separate. Direct publication to `main` is authorized for this integrated work.

Done when relevant focused tests pass, the diff and documentation are reconciled,
and the current-source revision, publication outcome and remaining limits are
reported.

### P4 Current-persona selection

2026-10-21 D2 implementation: selecting the canonical profile that already owns
the installed sealed store should only dismiss the switch picker when its
roster also records that profile as the family's chosen persona. Otherwise the
existing preservation/admission transaction must run. This keeps a selection
from silently swallowing a genuine family-choice change. The candidate starts
from fetched `71ac17e1` in an isolated checkout; the primary is preserved.

Done when the production selection hook retains dirty practice, installed
storage, audio-session identity, clocks and pending export/close state on the
no-op path; different identities, pending/uninstalled stores and changed family
choice still reach preservation; and a pre-change negative control plus the
focused current-source gate are recorded. This slice adds no Reload action,
capture cancellation, persona namespace migration or Redshank recovery policy.

Local source `6ca442f6` implements this candidate. The new production-hook
regression first fails on the unchanged current persona implementation at the
expected pending-transition assertion (exit 101). With the guard, all 28
`persona::` cases pass, including five negative identity/store/family scenarios
in the second new case. Exact source/lock bytes match the committed candidate;
the [receipt](../validation/persona-selection-noop_20261021/README.md) preserves
both process outcomes. This qualified source and evidence were published at `01c4b3af` on October 21.
Native/custody/capture acceptance remains separate. D3 is accepted below.

### P5 D3 explicit failed-recovery exit and independent copy

**Status (2026-10-21): CPU qualified at `421ea8e5`, published through `f4904a27`.** The maintainer
approved D3 and direct publication to main. This supersedes the current wave's
local-only publication instruction above. The already qualified P4 persona
candidate and receipt were published at `01c4b3af` before this D3 work began.

A failed original restore keeps its generation store nonwriting for the whole
process. An unchanged recovery session closes without a save attempt. A changed
session offers **Save Recovery Copy**, **Keep Working**, and **Exit Without
Saving**. Keep Working retains drafts and active capture. Copy requires capture
and admitted listener jobs to settle; explicit discard cancels capture only
after the choice. Application appearance retains its existing close decision.

Recovery copy is an independent export to a new, nonexisting directory outside
the original root. It preserves the complete current model, copies referenced
local/cache/voice media, and rewrites only copied library paths. Origin URLs,
representation receipts, transcript bytes and timed-target evidence retain
their meanings. Unsupported/missing media refuses publication. Model publication
comes last; failure may leave an incomplete new directory that cannot be reused.
A copy neither acknowledges original durability nor admits media deletion. Copy
success may finish a pending close only when the captured revision and draft
still match. Existing `REDSHANK_DATA_DIR` selects the copy on a fresh launch;
its absolute media paths keep the chosen directory location significant.

Owners are disjoint: storage snapshot and tests; shared recovery presentation
and tests; desktop lifecycle/worker integration, docs, qualification and Git by
the coordinator; independent read-only integration review. P5a stays with its
existing owner. Recording-capacity policy, persona-store migration and checked
supplier API intake are separate work.

Done when focused production-hook tests cover clean/dirty close, exact Keep
Working retention, cancellation/failure, revision/draft changes during copying,
original generation/media preservation and coherent independent reopen; both
surface layouts emit the intended controls; integration is reviewed, checked,
committed and published. Native chooser/window/appearance interaction remains
a separate observation unless exercised explicitly.

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

This lane owned primary source and CPU checks through the local handoff. It
launched no native applications and did not publish. The coordinating owner
subsequently integrated the bounded preservation UI/OS/reopen work and owns the
authorized direct publication. P6 remains separately scoped, though its bounded
persona-preservation path was exercised in the native receipt. Historical October
16 receipts retain their original hashes, commands and qualification limits.
Current-source CPU/build results remain in
`validation/preservation-input_20261017/integration.json`.


2026-10-17 integrated test gate: root seven-package gate 1,170 passing cases,
Hocket host 90, Redshank desktop/storage 95 (1,355 total). All three processes
exited zero. The root gate includes P5 audio/runtime cases and the unchanged
history scrub assertion. Five capture child invocations remain separately counted;
ignored cases are recorded directly in the current receipt. Three reviewers read
the complete automatically merged stage/persona/main/audio/index files and
compared input sources against the original commit; no merge blocker remained.
A stale index sentence now distinguishes completed capture primitive extraction
from remaining DSP consolidation. Serial desktop builds precede the local handoff.


2026-10-17 final integration gate at `cec221348154fc8e6abe3f6136f00b14f9860b78`:
1,355 distinct parent/doc cases passed, five capture child invocations are counted
separately, and nine tests were ignored. Woodshed, Hocket and Redshank desktop
builds all exited zero. Supplier lockfiles are byte-identical to the P5 baseline.
The CPU/build receipt is
[`integration.json`](../validation/preservation-input_20261017/integration.json).

2026-10-17 bounded Woodshed native qualification on `cec2213`: persona Retry and
Cancel retained the three authored Cards; explicit discard/reset left the
quarantined input untouched, and a fresh process reopened with zero discarded live
Cards. The seed phase reports 191 presented frames across six distinct captures;
the fresh-process reopen phase reports 98 presented frames and one capture. These
results qualify only the scoped Woodshed behavior.

2026-10-17 follow-up at `d955f5dc3b99ec1efd6ec9a56dd3ba007755409e` adds the shared
`after_frame`/dispatch guard and one host regression. Its native close phase
reports 139 presented frames across four captures. The actual macOS close → Retry
→ explicit Discard sequence exited the owned process while preserving the
original quarantined bytes. A corrected fresh-process quarantine-reopen run passed
(`RESULT ok`) with one capture, 97 presented frames and 98 redraws; the screenshot
shows the quarantine notice and zero Cards. Two earlier reopen attempts with
invalid assertion syntax (`close-reopen` and `close-reopen-final`) are excluded;
the corrected `quarantine-reopen` run supersedes them. Native manifests and
reviewed captures are linked from
[`native/README.md`](../validation/preservation-input_20261017/native/README.md).

Hocket and Redshank remain qualified only by their CPU test and desktop-build
commands in `integration.json`; neither has a native receipt from this work.
Codec-worker failure and incoming-custody opener failure are covered by
device-free tests, not the native runs. The broader audit findings remain out of
scope. No physical microphone, playback, or other physical-audio result is
claimed. Direct publication to `main` is authorized for the coordinating owner; its
outcome is established separately by Git refs and the final coordinating report.


2026-10-17 publication integration: remote `28a0741` adds arrangement edit
intents and Mere `4c4ea11`. The merged source `39f1dab` repeats 911 affected
Woodshed cases (including the 205 desktop cases), five separate child invocations,
seven existing ignores, the desktop build and the P6 native/reopen flow. Its eight
new captures and exact 22-row sealed reopen are recorded in
[`stage-p6-merged_20261017`](../validation/stage-p6-merged_20261017/README.md).
The historical preservation captures above remain bound to their recorded source;
no new Hocket/Redshank native claim follows from this merge. The supplier's checked
keyboard and close-hook blobs have no delta in the repin.

2026-10-21: accepted D3; current source refreshed at `71ac17e1`, P4 published at
`01c4b3af`, isolated D3 worktree created from that published source.

2026-10-21 D3 implementation: recovery copy uses snapshot-only draft
materialization, preserving point/span targets and the live editor. Review also
corrected recovery's never-admitted removal queues in idle/scenario predicates
and replaced an unbounded generation-path banner with a count. Focused locked
gates pass 22 storage, 107 surface and 92 desktop tests (221 distinct passes,
one existing desktop ignore). Desktop tests include both native/application
close hooks, actual Keep Working control and the final dirty-appearance barrier
in the GPU-free host harness. Ordinary desktop and actual Wasm browser checks
pass. Source and receipt are published through `f4904a27`. The
[publication record](../validation/redshank-recovery-copy_20261021/publication.json)
verifies 74 source/config/lock files and 24 evidence files against the fetched
remote tree. The [D3 receipt](../validation/redshank-recovery-copy_20261021/README.md)
records exact commands, logs, source hashes and archived audit chats. Native window,
picker, fresh-process and physical-device observations are not claimed.

2026-10-21 D3 publication: `main` advanced from `01c4b3af` to `f4904a27`;
remote advertised main and fetched source/evidence bytes match. The following
metadata commit records that observation without extending qualification scope.
