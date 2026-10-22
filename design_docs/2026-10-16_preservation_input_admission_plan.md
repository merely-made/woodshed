# Preservation and input admission

Date: 2026-10-16; updated 2026-10-22.
Status: Integrated onto P5 at `cec221348154fc8e6abe3f6136f00b14f9860b78`; the shared after-frame/dispatch guard follow-up is at `d955f5dc3b99ec1efd6ec9a56dd3ba007755409e`. Current-source CPU/build and bounded Woodshed native/reopen gates pass. Direct publication to `main` is authorized for the coordinating owner; Git refs establish publication separately. Hocket remains CPU/build only. Redshank D3 has the bounded macOS native/reopen evidence in P6; broader Redshank acceptance remains separate. No physical audio qualification is claimed.

2026-10-20 recheck: the user authorized parallel A1–A5/Q1 work and local commits
only. The current wave overrides this document's historical publication
authorization. Existing preservation/admission fixes remain; a bounded A1 guard
now refuses incoming custody sealed as another persona before retiring outgoing
work. Current device-free results and remaining design/native boundaries are in
the [first-wave recheck receipt](../validation/first-wave-recheck_20261020/README.md).
P5a stays with its existing owner. No push was performed.

2026-10-22 D1: the post-rollout source recheck and proposed persona-store
migration are in [P7](#p7-d1-persona-store-migration-proposed). This proposal
does not authorize its product changes or migrate user data. D3 publication
and qualification retain their separate evidence below.

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

### P6 D3 native recovery and fresh-process reopen

**Status (2026-10-22): qualified at committed `94583610`; published through `428dd7ad`.** The maintainer authorized native chooser
cancellation, failed/successful recovery copies, fresh-process reopening, exposed
fixes and direct main publication. Current primary and reused D3 worktree start
clean at fetched `97bb21e5`. All listener, media and appearance paths are isolated.
P5a remains with its existing owner.

Done when the exact locked desktop binary is exercised through the real native
chooser and recovery controls, canceled/failed copies retain editor and original
generations, a successful copy includes exact edited state and referenced audio,
a separate application process reopens it without the original source media,
and native captures are reviewed. Exposed fixes receive focused checks and a
committed-source native rerun before publication. No physical capture claim is
included in this recovery exercise.

### P7 D1 persona-store migration (proposed)

**Status (2026-10-22): proposed; maintainer rulings required.** Decider: the
maintainer. Rechecked fetched `main` at
`80f2d4eb6dd6b30e3e38b4fae610250f7842b96e`, clean and equal to `origin/main`.
The rollout selects Mere `a07a375ed9f49d31bb757c4dd516a0fa7af33fde`;
Personae remains on Dramatis `7d498c27df8076dacccb74ba4a05f863f9f48c76`,
and Muniment/Eidetic on `34105d63ec17826e0a58a5b461b65150357dd000`.
No production code, dependency pin or user store changes are part of this
proposal. P5a stays with its existing owner.

#### Current-source findings

- `woodshed-genet/src/storage.rs:78-135` opens every identity over the same
  `FsBackend::new()`. Lines 250-303 map it to `genet-state.json`,
  `genet-settings.json` and the adjacent `genet-state.loop-pcm/`, or to exact
  `WOODSHED_STATE`/`WOODSHED_SETTINGS` overrides. Different sealing keys do
  not create different stores. These references are under `crates/`.
- Wrong-key reads are errors, and `SessionStore` quarantines writes after
  failed admission (`woodshed-core/src/sealed_backend.rs:82-97`;
  `woodshed-core/src/storage.rs:297-319`). This protects existing data but
  leaves another persona unable to open its own independent practice at the
  same path. Plaintext adoption is still enabled on ordinary opens.
- The switch barrier preserves outgoing work and checks the selected
  canonical profile (`woodshed-genet/src/persona.rs:167-198`). Installation
  checks a Pending seal, then resets and restores the incoming state
  (`:310-358`). D1 needs explicit incoming store admission before that reset;
  a successful seal alone does not prove the session, settings or PCM opened.
- `PersistedSession` includes working/retained Sets, explorations, musical
  context, SongDoc, exact kit/pattern selections, history, personal relation
  journal, retained readings/presentation and Looper references
  (`woodshed-core/src/storage.rs:375-439`). References and companion PCM
  form one migration closure. Missing/unsupported takes are retained authored
  data today; migration must not quietly drop them.
- `AppSettings` currently combines musical, appearance, accessibility and
  pedal preferences with window geometry (`woodshed-core/src/settings.rs:393-419`).
  The separately authored **theme/font library** already stays global across
  persona reset (`woodshed-genet/src/persona.rs:361-382` and
  `appearance.rs:30-79`). This is distinct from retained musical material.
- Export guards reconstruct `FsBackend::new()` rather than use installed
  paths (`woodshed-genet/src/appearance.rs:75-79`, `music_export.rs:53-68`).
  A namespace change must update these consumers in the same slice.
- File publication is atomic per file; `FsBackend::apply` explicitly is not
  a multi-file transaction (`woodshed-genet/src/storage.rs:332-335,517-528`).
  Migration needs its own completion/admission boundary.
- The pinned Mere `CustodyIdentity` already exposes `connect_as` and
  `IdentityProvider::master_public_key` in
  `crates/graphshell/graphshell-endpoint/src/door/custody_identity.rs`.
  Candidate validation can use explicit custody without changing the family
  choice. The pinned Personae documents `ProfileId` as stable **within a
  vault** (`crates/personae/src/vault.rs:39-47`), so that string alone is not
  a sufficient storage identity across replaced vaults or carried seeds.

The five storage/transition files inspected above have no changes between
the completed D3 publication `8ffa35a9` and this rollout revision. This is a
source recheck, with no new CPU, native or migration acceptance claimed.

#### Recommended storage and authority contract

Use one product-owned store per cryptographic persona. A versioned,
domain-separated digest of the custody-reported persona public key supplies
the directory token; display names and raw profile strings never become path
components. The token is a locator, not a secret or authorization. A sealed
owner manifest binds the store UUID, format version, owner and migration
provenance. Admission checks the selected canonical profile against custody
and the cryptographic owner against the store. Reusing `default` with a new
seed opens a distinct store; carrying the same seed retains the same owner.
This uses existing custody APIs and the existing practice sealing context.
It introduces no vault access or secret-key files in Woodshed.

Illustrative layout, subject to the rulings below:

```text
<application-root>/
  desktop.json                 # device-local window geometry, if D1.2 approved
  themes.json                  # existing shared authored theme/font library
  personas/v1/<owner-token>/
    owner                     # sealed store identity and migration receipt
    session                   # existing session payload, sealed
    settings                  # persona preferences, sealed
    loop-pcm/<namespace>/<digest>.bin
    ready                     # final completion marker; never sufficient alone
```

New roots use strict sealed reads. Plaintext adoption becomes an explicit
legacy-source operation, followed by structural admission, instead of a
fallback on every normal persona-store open. Existing schemas, musical IDs,
history branches, journal source/assertion IDs, capture clocks, codec bytes
and PCM samples retain their meaning. The migration changes storage ownership
and placement; it does not merge or regenerate practice material.

| Data | Recommended owner |
|---|---|
| Complete session, retained musical material, history and personal assertions | Persona |
| All referenced Looper companions and their existing namespace/digests | Same persona store |
| Tuning, instrument, practice/view preferences, selected appearance, accessibility and pedal bindings | Persona |
| Authored theme/font definitions | Existing shared application library |
| Desktop window placement | Device/install, if D1.2 approved |
| Live MIDI routes/inventory, audio devices, event clock, workers and custody handles | Running host; never migrated as durable practice |

Shared crates continue to return validation facts and capabilities. The
Woodshed host owns paths, leases, custody, migration choices, durable admission
and user wording. Hocket and Redshank retain their independent store contracts.
No cross-device synchronization, persona deletion, automatic garbage collection,
cross-persona copy/merge, supplier repin or P5a change is included.

#### Decisions requested from the maintainer

All recommendations in this table remain proposed.

| Ruling | Recommendation | Alternative and consequence |
|---|---|---|
| **D1.1 Legacy assignment and retention** | Ask once which persona receives the existing practice, naming the target and explaining what will be copied. Matching-key decryption is required for sealed data; plaintext still needs explicit assignment. Copy into that one store and retain the original files unchanged, with no automatic expiry. Cancel leaves assignment unresolved and permits unsaved practice. | Automatically assign to the current family persona: fewer steps, but current selection does not establish ownership of plaintext legacy material. Cloning to every persona would duplicate history and assertions and is not recommended. |
| **D1.2 Preference ownership** | Move musical, view, selected appearance, accessibility and pedal preferences with the persona. Keep authored theme/font definitions shared, and split window geometry into device-local installation preferences. | Keep the entire current AppSettings payload per persona, including window geometry: smaller first slice, but window placement follows whichever persona last saved it. |
| **D1.3 Override compatibility** | Add a whole-root `WOODSHED_DATA_DIR` for isolated multi-persona runs. Keep existing exact-file overrides as an explicit single-owner compatibility mode, bound to one owner and refused for another; never silently append persona suffixes or fall back to production paths. | Redefine existing overrides as persona-root selectors: simpler long-term surface, but breaks fixture scripts and their expected output paths. Removing exact-file support requires a separate caller migration. |
| **D1.4 Family choice** | Preserve the current family-wide persona switch. Preflight through explicit `connect_as`, then commit the family choice only after outgoing preservation and incoming admission succeed. | Make persona choice Woodshed-local: reduces cross-app effects, but changes an established product convention and the current-persona no-op contract. |

D1.3 details: whole-root and exact-file modes must not be mixed silently.
The new whole-root mode isolates installation preferences and the theme library
as well as persona stores, unless an explicit library override is supplied.
Existing `WOODSHED_STATE` adjacency rules and explicit `WOODSHED_SETTINGS`
filenames stay exact in compatibility mode. A settings-only override must
require an explicit compatible state destination rather than silently combine
scratch settings with the production session. New empty exact-file stores bind
their owner before the first write; populated legacy files need ownership
admission. Scenario harnesses may provide an explicit expected owner and
migration choice, validated exactly as the UI choice, so fixtures remain
deterministic. Sidecars and every configured destination join export protection.

#### Migration transaction and failure behavior

1. **Preserve and inspect.** Finish the existing outgoing preservation barrier.
   Obtain a single-writer lease for the product root and target. Snapshot the
   legacy session, settings and companion inventory with byte hashes, without
   rewriting or pruning the source. Recheck it before publication. Concurrent
   old binaries do not honor a new lease: migration must refuse known concurrent
   instances and document that running mixed versions during migration is
   unsupported; a lease alone cannot prove legacy files are immutable.
2. **Admit ownership and contents.** Resolve custody for the explicit candidate;
   check selected profile and owner, then authenticate/decode the session and
   settings using existing structural validators and the view's retained-data
   admission. Resolve every referenced companion. Never try every persona key
   behind the user's back. Wrong key, corrupt settings/session, unsupported
   referenced codecs or missing PCM stop automatic migration. Keep authored
   references and original bytes available for recovery; do not publish an
   apparently complete destination with dropped takes.
3. **Build a new candidate.** Use a sibling staging directory and a fresh
   destination; never overwrite or merge an existing persona store. Preserve
   exact admitted session bytes where no wire transformation is needed, and
   exact encoded companion bytes through the same sealing policy. Plaintext
   legacy payloads are sealed into the new store. If D1.2 is approved, split
   only the known window field from settings while retaining other raw fields;
   seed installation geometry only if it has no existing authoritative value.
   Original unsplit settings remain in the retained source. Keep unreferenced
   or foreign-key legacy blobs in that retained source; do not collect them.
4. **Verify and publish.** Open the candidate through a fresh backend handle,
   authenticate its owner, admit the full payload and verify referenced bytes,
   digests, frame counts and unchanged identities. Record source fingerprints,
   target owner/store UUID and format in the receipt. Publish completion last,
   under the lease, using a host-supported no-replace promotion. A partial
   directory or marker alone is never a usable store. Failure leaves the source
   and outgoing window intact, with a retry/cancel/recovery explanation.
5. **Install deliberately.** Only after admission, commit the family choice
   under D1.4, revalidate custody and install the matching store/UI together.
   There is no distributed transaction with djinn: a failure retains outgoing
   work and reports the observed family choice. A crash after the family choice
   leaves a fully admitted target for next launch. Normal save/close logic and
   same-persona selection use the installed owner and resolved paths. Pending
   custody, absent roots and failed migration never select a writable singleton
   or default store. With custody unavailable, start unsaved and expose explicit
   legacy recovery rather than silently loading an unassigned plaintext store.

Once a complete persona store exists, it is authoritative. Do not reimport the
legacy singleton on every startup. A changed legacy fingerprint is a recovery
condition, not permission to overwrite current practice. Retained originals
support deliberate recovery; reopening an older binary writes its old singleton
and does not include subsequent persona-store edits. No automatic downgrade
round-trip or cleanup is promised. Starting a new empty store while legacy
assignment is deferred is explicit and must disclose that later import cannot
overwrite or automatically merge that store.

#### Implementation sequence and owners after the rulings

The coordinator owns shared seams; no two lanes edit a seam concurrently.

| Step | Sole owner / seams | Done-condition |
|---|---|---|
| **D1-A Resolve store identity and paths** | Host storage owner: `storage.rs`, resolved store descriptor, migration/lease primitives and their tests | Distinct keys with the same ProfileId isolate; same owner resolves consistently; hostile names cannot escape roots; new/legacy/partial/pending states are explicit; override/collision rules hold. |
| **D1-B Copy and verify legacy data** | Same host storage owner, with core validator changes assigned to one core owner only if necessary | Interrupted writes, wrong key, missing/unsupported PCM, changed sources, existing destinations, lock loss and retry never overwrite source or claim completion. Valid split/flat legacy and exact settings/session/PCM read back through a fresh handle. |
| **D1-C Admit and install** | Coordinator: `persona.rs`, `session.rs`, `shared.rs`, `main.rs`; one view owner for migration controls | Outgoing work survives canceled/failed incoming admission; family-choice errors are visible; A→B→A restores independent exact practice; current-persona no-op and host runtime bindings remain correct. |
| **D1-D Use installed destinations everywhere** | Coordinator: `appearance.rs`, `music_export.rs`, resolved descriptor consumers; core owner for the approved settings split | Save, close, geometry, theme and MusicXML exports protect every live/legacy/staging store and override sidecar, including nonexisting targets and aliases. No consumer reconstructs the singleton to identify live storage. |
| **D1-E Qualify and publish** | Coordinator schedules checks/native/Git; independent read-only review | Focused store/transition checks, a desktop build, native assignment/cancel/failure and A→B→A, lock/unavailable-custody behavior, plus independent fresh-process reopen of each persona with exact retained identities and PCM. Record source, commands, hashes, observations and publication separately; then prune only clean, merged completed work. |

D1-A/B storage work and disjoint controls/core work may run in parallel once
the descriptor and rulings are fixed. Integration, native execution and shared
seam edits remain serialized. The prior rollout's CPU receipt and D3's native
receipt do not qualify this unimplemented migration.

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

2026-10-21 native D3 probe: real chooser cancellation, protected-destination
refusal and missing-media failure retain the exact editor text. Original
published/pending generation bytes remain identical. A successful native
copy from pending close preserves the draft and local WAV and exits normally.
The probe exposed Rust Debug error internals in the failure panel; the host now
uses readable cause/action text, keeping detailed store errors out of that UI.
Focused regression and committed-source native rerun are next.

2026-10-22 P6 qualification: ten committed-source native captures cover chooser
cancellation, protected-destination and missing-media failures, Keep Working,
successful copying from pending Close, and independent 820x560 rail reopening.
The exact unsaved note, target/representation, library, selection, queue and local
WAV survive. Original published/pending bytes remain identical. A fresh process
with the original WAV unavailable opens the note and plays copied media to 30,000
ms/Completed. Both native processes exit zero. The full desktop gate passes
92 cases with one existing ignore; locked build and independent source/capture
review pass. [Native receipt](../validation/redshank-recovery-native_20261021/README.md).
No P5a or physical microphone/audio acceptance is included.

2026-10-22 native publication: source fix and receipt published through `428dd7ad`;
remote advertised main, all 74 qualified source/config/lock hashes and all 65
evidence files match the fetched tree. [Publication record](../validation/redshank-recovery-native_20261021/publication.json).


2026-10-22 D1 proposal: refreshed clean main at `80f2d4eb`, verified the
post-rollout lock identities and re-read current storage, custody, switch,
settings, capture and export seams. P7 records four requested maintainer
rulings, owner-scoped implementation steps and failure/reopen gates. This is
a documentation-only proposal; no migration code or user-data operation ran.
P5a remains with its existing owner.
