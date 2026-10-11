# Audio ports rehome plan

**Status (2026-09-09): LANDED.** Hocket and Ringdown live in Woodshed's
`ports/` alongside Redshank while remaining separately named, versioned,
tested, and released embeddable products. Woodshed and Redshank use MPL-2.0.
The former repositories are archived with relocation notices. The inherited
formatting and Hocket Clippy cleanup recorded below remains separate work.

**Follow-up status (2026-10-11): IN PROGRESS.** The maintainer approved
orchestrating the current-source Firewheel migration and family consolidation
assessment. The implementation sequence and lane gates are recorded below;
the original history-preserving rehome remains landed.

## Ruling

`ports/` is the home for independently named audio and instrument products
whose development composes closely with Woodshed. It is not a second shared
crate directory. Generic code used by more than one product continues to live
under root `crates/`; a port must not depend on another port merely because
they share a repository.

Hocket, Redshank, and Ringdown remain nested Cargo workspaces. They keep their
own lockfiles, package versions, executable hosts, release tags, design indexes,
and validation commands. Woodshed's root workspace does not list their packages
as members. This prevents every application backend and host from entering one
dependency graph while still allowing deliberate relative-path dependencies at
stable shared or consumer boundaries.

Repository tags are product-qualified: `woodshed-v*`, `hocket-v*`,
`redshank-v*`, and `ringdown-v*`. A GitHub repository has only one global
"latest release", so Hocket's updater must use a stable Hocket-only signed
manifest URL rather than the `github:owner/repo` latest-release shorthand.

The move reverses Hocket's May 2026 and Ringdown's August 2026 separate-repo
rulings. Those decisions described the right product boundaries at the time;
they do not require separate Git repositories now that an actual audio family,
shared DSP dependency, and Woodshed instrument consumer exist.

## Phase 1: preserve and import history

Done when:

- every local Hocket and Ringdown commit is present on its existing remote;
- each repository's full reachable `main` history is merged into Woodshed;
- the imported trees live at `ports/hocket` and `ports/ringdown` without
  squashing;
- port-local license, attribution, provenance, release, scenario, and design
  material remains intact.

## Phase 2: make the monorepo authoritative

Done when:

- Hocket consumes root `audio-primitives` by a relative path plus version;
- `woodshed-instrument` consumes the imported Ringdown core and BLE shell by
  relative path;
- package repository metadata points to each port's Woodshed subtree;
- stale prose saying either product must live in a separate repository is
  corrected while preserving the historical reason for the earlier ruling;
- Woodshed root package metadata, Redshank's nested manifests, and license
  files state MPL-2.0;
- root documentation explains that ports have independent release trains and
  nested workspaces.

## Phase 3: validate and publish the move

Done when:

- Woodshed's root workspace, Hocket's nested workspace, Ringdown's nested
  workspace, and Redshank's nested workspace pass their relevant locked tests;
- strict Clippy and formatting pass in the moved Hocket and Ringdown workspaces;
- the resulting Woodshed history is pushed to `main` without overwriting
  intervening work;
- the former Hocket and Ringdown repositories receive relocation notices only
  after the new paths are live.

## Findings

- **2026-09-09:** Hocket had one clean local commit ahead of `origin/main` and
  Ringdown had six. Both sets were pushed before import, creating recoverable
  source checkpoints.
- **2026-09-09:** Hocket already consumes Woodshed's unpublished
  `audio-primitives` from the Woodshed Git repository. Ringdown's unpublished
  BLE shell is already consumed by `woodshed-instrument` from Ringdown Git.
  These are real cross-repository development edges, not speculative affinity.
- **2026-09-09:** Hocket and Ringdown are MPL-2.0. Keeping a `LICENSE` inside
  each imported port preserves their standalone package boundary while the
  Woodshed root and the root-licensed Redshank port adopt the same license.

## Progress

- **2026-09-09:** Plan opened after the maintainer approved the rehome and the
  Woodshed MPL-2.0 conversion. Source-remote checkpoint pushes completed.
- **2026-09-09:** Full Hocket and Ringdown `main` histories were merged without
  squashing at `ports/hocket` and `ports/ringdown`. Nested workspaces are
  explicitly excluded from the root workspace; Hocket's shared-DSP edge and
  Woodshed's Ringdown consumer edge now use relative paths. Validation,
  publication, and old-repository relocation notices remain.
- **2026-09-09:** The first root build against Ringdown's imported tip exposed
  API drift hidden by Woodshed's older Git lock. `woodshed-instrument` now uses
  Ringdown's transport constructor and fallible optional-field metronome
  builder. Its 16 tests and strict Clippy pass.
- **2026-09-09:** Locked workspace tests pass for Woodshed, Hocket, Ringdown,
  and Redshank. Ringdown's strict workspace Clippy passes. Hocket's strict
  workspace Clippy reports nine inherited `collapsible_if`/`derivable_impls`
  findings, and the family rustfmt policy reports inherited source drift in
  Woodshed, Hocket, and Ringdown. The rehome changed two Rust files; their
  behavior is covered by the green suites. Repository policy reserves the
  broad formatting sweep for a separate commit and blame-ignore entry.
- **2026-09-09:** Woodshed `main` published the history imports, dependency
  rewiring, MPL-2.0 conversion, and compatibility repair through `36b066d`.
  The former Hocket repository points to its new home at `b163858`; the former
  Ringdown repository does the same at `30af57d`. Both source repositories are
  archived read-only after those notices became reachable.

## Standalone audio foundation and consolidation (2026-10-11)

### Decision and ownership

Accepted by the maintainer after the October 11 assessment: maintain the
Firewheel fork as a standalone graph foundation with bounded continuous capture
and producer playback. Each product retains its own schema, identity, history,
and authority. A graph is a runtime projection. Pure DSP remains in
`audio-primitives`; network, codecs, durable bytes, and product rules stay
outside the graph kernel. The unimplemented streamed sampler is not a shipping
replacement for the removed stream nodes.

Use exact-revision integration and directly published commits. Local validation,
Hocket host infrastructure, and Redshank retention can proceed independently of
the fork bridge. Consumer API migration follows bridge checks. The root
coordinator owns integration, canonical status updates, source receipts, and
cross-product acceptance. Agents own disjoint source paths in separate
worktrees; they report commits, commands, failures, and unrun gates explicitly.

The September 26 Redshank own-repository ruling in the
[listening plan](2026-09-01_listening_annotation_port_plan.md) supersedes the
earlier general incubation direction for that product. Its extraction follows
its existing recording/Turnstone/native gates and independently sourced shared
crates. Hocket and Ringdown remain independent nested workspaces.

### F1. Enforce existing product boundaries

Done when all four workspace roots have runnable local locked-resolution and
scoped CPU checks; platform packaging remains explicit and release tags remain
product-qualified; Hocket uses Mesquite capture lifecycle with truthful
failure outcomes; and Redshank bounds generation retention while preserving
last-valid recovery and failed-publication behavior. Cosmetic cleanup is
limited to verified obsolete claims and unused declarations. Preserve live
updater choices and historical/data compatibility paths.

### F2. Restore the standalone streaming contract

Done when the owned Firewheel 0.14 foundation exposes bounded capture and
producer adapters; offline graph tests cover exact samples, partial transfers,
backpressure, overflow/underrun, EOF/reset, channel and rate conversion, and
frame accounting; normal processing adds no mutex or per-buffer allocation;
and stream/device ownership is explicit. A supported feature excludes Bevy ECS.
Keep attribution and upstream history. A 0.10 maintenance backport remains a
fallback if adapting the focused bridge is infeasible.

### F3. Migrate consumers independently

Done when Hocket and Redshank use an exact fork revision, handle fallible graph
construction and separate CPAL stream lifetime, and pass their locked gates.
Hocket retires reproduced sampler/silent-monitor workarounds and qualifies
count-in/capture/replay timing and restart. Redshank preserves progressive
decoding, rates/channels, queued/presented source clocks, seeks, EOF, gain/duck,
resume, and frozen annotation anchors. Headless results do not close device,
native capture, or fresh-process reopen acceptance.

### F4. Consolidate Woodshed product execution

**Single-output runtime status (2026-10-11): IN PROGRESS.** The maintainer
authorized implementation and direct-main publication from refreshed `48d4e5a`.
Runtime, desktop integration and input/calibration are disjoint source lanes;
the Woodshed agent prepares native checks and waits for a source/binary freeze.

Follow [P7](2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set)
for Set-derived LoopPlan, stable capture segment identity, saved-song migration,
and companion persistence. Done when one output device/clock runs click,
preview, Set playback and loop voices without callback control mutexes or
recording allocations; the connected Hear/Add/rehearse/loop/edit/reopen flow
passes current-source automated and native acceptance. Preserve exploration
and deliberate practice as complementary flows.

The next runtime slice belongs to root `woodshed-audio`: one `OutputRuntime`,
one Firewheel context and CPAL output stream, and processor-owned click,
preview and backing lanes. Keep Set/Card ownership, admission and prepared PCM
on the control side; publish bounded commands and generation-qualified status.
Use the device sample clock while preserving independent logical transport
offsets, phase-preserving tempo edits, variable backing boundaries and ordered
seek/record-arm. Fill every output frame, including a one-shot's final tail.
Hocket's public engine carries Hocket model identities and different tempo and
next-bar semantics; root must not acquire a dependency on that port.

Prepare recording storage before arming and transfer exclusive mutable buffer
ownership to the processor. A bounded input feed must report overflow and
invalidate a truncated take; input-rate conversion or refusal is explicit.
Preserve the host's lazy shared tuner/onset/capture input session. One output
clock does not qualify input/output hardware synchronization or remove the
existing input-analyzer locks. Before replacing the engines, wire idle host
service for preview completions, Firewheel updates and resource retirement,
and adapt calibration to a small click-control/clock seam. Keeping the old
metronome solely for calibration would preserve the second output stream.

Separately, replace infallible saved-song admission and fallible Set admission
with one guarded, fallible backing operation. Every rejected replacement,
including an edit to an already loaded saved song during capture, must retain
the previous accepted backing. Output unavailability must not look accepted.
The independent SongDoc/bar editor is still live. Lossless migration requires
exact pitch vectors, root frequency and display formula, per-bar tempo/meter,
and hold duration; current Cards and the global LoopPlan meter cannot express
all of those values. Freezing legacy authoring removes a product capability
and needs its own decision. Do not silently revoice or flatten saved material.
Transient Set PCM still needs companion storage and transaction rules before
recordings can be claimed to survive session replacement or restart.

### F5. Complete portable product seams

Done when Hocket project/media/store/export/handoff operations can be used
without its device runtime; Redshank shares portable transitions required by
its standalone and Turnstone hosts, explicitly admits host audio/microphone
capabilities, and completes its ruled extraction after existing gates. Shared
path crates need independent Git/package sourcing before extraction. Package
renaming, retiring updater transports, and promoting browser previews to full
products require explicit product decisions, not generic cleanup.

The current Hocket boundary is model-owned session/history/schema and
`hocket-project`-owned media/store/export/handoff/waveforms, with the engine
executing a projection. Its project dependency closure excludes Firewheel,
CPAL and windowing. The next host correction is explicit operation/session
generation correlation for project-worker replies: display status currently
stands in for busy state, handoff commands do not reserve it, and a late save
reply can apply after a session replacement. Portable acceptance and native
save/open/export/handoff are separate claims.

Redshank's portable transition candidate remains its desktop `Session`:
selection, load generations, progress, capture anchors and projection. Extract
the transitions needed by actual hosts and admit microphone/playback
capabilities explicitly. The browser's in-memory fixture path does not qualify
real clocks, device capture or durability. Its shared root `audio-primitives`
and `timed-text` paths need independent Git/package sourcing before extraction.
The October 11 follow-up did not refresh Turnstone source or close that
product's existing recording/Turnstone gates.

### Findings

- **2026-10-11:** Audit source was Woodshed `6dfb2fe`, Firewheel fork `34b35ca`
  (0.14), and Turnstone `53fa567`. Current implementation starts at Woodshed
  `cd69da0`, preserving the subsequent Hocket Tabard integration and Rust 1.97
  toolchain update. All three UI roots use Mere `e95326d` / Genet `7422e90`.
- **2026-10-11:** Current Firewheel consumers use 0.10. The fork removed
  reader/writer stream nodes; its streamed sampler contains panic paths.
  Isolated probes reproduce the old play-event mismatch and verify new helper,
  post-construction PCM, within-block scheduling, and input-only graph behavior.
- **2026-10-11:** The audit passed 747 current-source CPU checks plus five
  migration probes; seven Redshank fixture/device tests were ignored. This is
  a baseline at its source revision, not consumer migration or new native proof.
- **2026-10-11 (audit `6dfb2fe`):** CI checks only the root workspace. Windows release still
  listens for `v*`. Hocket owns custom blocking readback and ignores capture
  errors. Redshank publishes full immutable model generations without pruning.
  Woodshed starts two output engines with callback mutexes/recording allocation;
  its independent SongDoc/bar editor remains the open P7 authority duplication.

### Progress

- **2026-10-11:** Refreshed remotes, created isolated integration worktree at
  `cd69da0`, and opened parallel foundation lanes. No stack repin is inferred
  from newer upstream revisions; native receipts remain revision-specific.
- **2026-10-11:** Refresh corrected the Hocket scenario finding: `cd69da0`
  already adopted Mesquite in its Tabard integration. The host lane preserves
  that work and addresses residual capture-only dependencies, early-close
  lifecycle and project-worker quiescence, plus manual-demo failure semantics.
- **2026-10-11:** Redshank generation retention implemented: current published
  state plus two valid recovery states, pruning only after flushed publication.
  Pending and unrelated files remain untouched. Ten storage tests and strict
  package Clippy pass, including repeated saves, two corrupt latest states,
  invalid newer files, failed publication, and explicit post-publication cleanup
  error behavior. Desktop/reopen qualification is pending.
- **2026-10-11:** Portable Hocket project seam extracted into `hocket-project`:
  media/store/export/handoff/waveform and 30 moved tests. Model, engine, capture
  and history add 54 passing checks. The normal project tree excludes Firewheel
  and CPAL; model schema, MediaRef, project and envelope formats are preserved.
  Combined host validation and native project acceptance follow integration.
- **2026-10-11:** F1 CI/release changes integrated: independent locked-resolution
  and scoped CPU gates for Woodshed/Hocket/Redshank/Ringdown; release tags use
  `woodshed-v*`, with version-derived prerelease classification. The CI lane
  passes 1,328 CPU checks (eight ignored), four resolution checks, actionlint and
  eight real PowerShell fixture packaging cases. Windows binary qualification
  and actual GitHub Actions execution are separate from those local fixtures.
- **2026-10-11:** Hocket host/manual F1 source integrated. Residual close/readback
  and project-worker quiescence fixes pass 70 CPU checks plus five finite native
  processes, 19 reviewed captures and two appearance reopens at `9dc042f`.
  See the [port receipt](../ports/hocket/design_docs/receipts/host_acceptance_20261011/README.md).
  The integrated host/project scope passes 152 CPU tests (one custody receipt
  ignored). Native project save/open/export and handoff activity remain open.
- **2026-10-11:** Redshank F1 retention passes three native processes: eleven
  shipping-host writes prune twenty generations to three; fresh reopen restores
  saved settings; corrupting both latest states recovers the oldest retained
  valid state. Pending and unrelated media survive. The
  [source-bound receipt](receipts/audio_foundation_20261011/retention/README.md)
  qualifies `9632f37`, before consumer migration.
- **2026-10-11:** F2 fork branch `owned/stream-foundation` is pushed at exact
  revision `2a0a342da32a1d2eba7c73ede7424d93d76e420c`; its
  [draft PR](https://github.com/mark-ik/firewheel/pull/1) restores bounded nodes,
  integer source clocks, EOF flush/drain and safe preallocation limits. Fourteen
  offline graph tests, callback allocation guards, a concurrent EOF publication
  check and feature/lint gates pass. Physical audio and consumer acceptance
  remain separate. This provisional revision is superseded by the final
  material-EOF correction below.
- **2026-10-11:** Consumer review reproduced all-zero short EOF PCM at the
  provisional fork pin. Final pushed provider `2b03846d2e0dcc6ded2a85367bb8c5071ae99cdc`
  corrects the resampler packet copy after delay trimming. The short-EOF matrix
  renders 288 reader/writer cases across rates, lengths, qualities and channels;
  callback allocation guards and concurrent EOF checks pass. GitHub Actions
  passes standalone Linux/macOS/Windows, full tests, formatting and docs at
  that exact head. Both consumers now use the corrected immutable revision.
- **2026-10-11:** Redshank's migrated playback scope passes 261 CPU tests with
  seven explicitly ignored device/fixture checks. Exact presented-source
  accounting replaces capacity guesses and EOF timing heuristics; progressive
  decode, WSOLA flush, partial admission and annotation clocks remain covered.
  Runtime replacement and arena Node ID reuse cannot let a retired preview
  remove a node from the replacement graph. The migrated desktop passes three
  finite native processes and twelve reviewed nonblank captures, including
  authored-theme fresh-process reopening. See the
  [consumer receipt](receipts/audio_foundation_20261011/redshank-consumer/README.md).
- **2026-10-11:** Hocket's migrated independent workspace passes 166 CPU tests
  with one custody-specific ignore and twelve production-graph offline cases.
  The runtime uses current sampler resource/restart APIs, retires the silent
  input monitor and restores the model/export's linear gain contract. Actual
  input channels and liveness now gate Record for output-only fallback streams.
  Physical input resampling and sample-accurate calibration remain open.
  Five native processes qualify recorder/appearance regression with nineteen
  reviewed captures and two fresh appearance reopens at committed source
  `f5e0aed`; the [Hocket consumer receipt](../ports/hocket/design_docs/receipts/firewheel_20261011/README.md)
  preserves the exact source and device-free limits.
- **2026-10-11:** F4's first slice derives Set backing and preserves captures
  by owner/Card identity. Reorder/pitch edits retain PCM; changes to its clock
  and armed/active source switches are refused. Production storage tests cover
  source/settings reopen, while saved backing retains exact legacy pitches
  and timing. The full P7 migration, durable PCM, mixed material and one output
  clock without callback locks/recording allocations remain open.
- **2026-10-11:** The integrated family gate passes 1,454 CPU checks with nine
  explicit ignores (Woodshed 870/1, Hocket 166/1, Redshank 261/7, Ringdown
  157/0). Four independent locked metadata checks pass outside repository Cargo
  overrides. CI now includes Woodshed's desktop application tests and Hocket's
  device-free manual CLI tests. Both migrated consumers remain exact-pinned;
  Woodshed's older two-engine runtime is a separate follow-up. The first native
  Set replay is retained as failed: the locked Mac reports an occluded surface
  and zero presentations. Native Set flow and keeper-backed reopening remain
  open. See the [integration receipt](receipts/audio_foundation_20261011/integration/README.md).
- **2026-10-11 (maintainer correction):** No GitHub workflows or default pull
  request review step. Remove all root workflows and the fork's inherited
  workflow; retain ordinary local validation and platform packaging commands.
  Publish the qualified foundation directly to each repository's `main` after
  refreshing remote state and preserving concurrent work. Earlier Actions and
  draft-PR observations above are historical evidence, not continuing gates.
  Canonical plans and source already carry the execution frontier; PRs and
  issues would duplicate it and become stale as scope changes. During rapid
  prototyping, use focused local checks for concrete risks in the change.
  Existing suite commands remain available, rather than becoming mandatory
  gates for every interim commit. Broader conformance and automation belong
  to behavior intentionally stabilized for evaluation.
- **2026-10-11:** The parallel Woodshed stack lane published `8846b13` with
  coherent Mere/Genet/Dramatis/Netrender pins and Classic 0.11 across the UI
  workspaces. Its [acceptance record](../validation/stack-family_20261011/README.md)
  qualifies eight native runs and 38 reviewed captures: Woodshed Set,
  preview/rehearsal and appearance seed/reopen, plus Hocket and Redshank
  appearance seeds. The ports' appearance reopen attempts remain open after
  zero qualified presentations. Those source/binary receipts precede the
  following audit fixes and do not certify their native behavior.
- **2026-10-11:** Three bounded source corrections follow the audit:
  `a090e2f` retains both peers' append-only captured phrases/media while
  accepting a same-root Hocket branch, refusing conflicting immutable phrase
  IDs and malformed snapshots transactionally; `a3871e2` projects Looper
  relations from the host-admitted backing owner while preserving rehearsal's
  owner and the saved Song payload; `894cfc1` silences unwritten callback frames
  after one-shot backing stops. The handoff and one-shot regressions failed
  before their fixes. After merging the stack family, 8 handoff, 11 core
  overview, 11 view overview and 2 one-shot tests pass with locked dependencies
  under Rust 1.97.1. Changed-file formatting, whitespace and new doc links pass.
  No new native run, durable capture migration or one-clock runtime is claimed.
