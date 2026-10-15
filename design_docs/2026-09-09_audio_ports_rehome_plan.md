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

**Single-output runtime status (2026-10-11): IMPLEMENTED AND QUALIFIED.** From refreshed
`48d4e5a`, the root now uses one Firewheel context and CPAL output stream for
click, preview and backing. At frozen source `4a4aba4`, 144 audio and 29 focused
desktop checks pass; three muted device processes each prove one output stream,
frame progress and zero observed CPAL status events. Five GUI seed/reopen and
stop/restart processes pass with 791 presentations and 16 reviewed full
captures; 17 checkpoints preserve unopened input and one output stream.
Physical capture and full P7 remain open. See the
[P7 implementation record](2026-07-11_stage_set_tools_plan.md#october-11-follow-up-single-output-runtime).

Follow [P7](2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set)
for Set-derived LoopPlan, stable capture segment identity, saved-song migration,
and companion persistence. Done when one output device/clock runs click,
preview, Set playback and loop voices without callback control mutexes or
recording allocations; the connected Hear/Add/rehearse/loop/edit/reopen flow
passes current-source automated and native acceptance. Preserve exploration
and deliberate practice as complementary flows.

The implemented runtime belongs to root `woodshed-audio`: one `OutputRuntime`,
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

Saved-song and Set admission now share guarded, fallible desktop backing
operations. Every rejected replacement,
including an edit to an already loaded saved song during capture, must retain
the previous accepted backing. Output unavailability must not look accepted.
The independent SongDoc/bar editor is still live. Lossless migration requires
exact pitch vectors, root frequency and display formula, per-bar tempo/meter,
and hold duration; current Cards and the global LoopPlan meter cannot express
all of those values. Freezing legacy authoring removes a product capability
and needs its own decision. Do not silently revoice or flatten saved material.
Set PCM now has exact sealed companion storage and a bounded ownership
manifest; 80 focused checks and four independent CPU round-trips pass, together
with four native processes, 160 presentations and eight reviewed captures in the
[P7 capture slice](2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set).
Blob publication precedes session replacement; post-commit readback failure
is unconfirmed rather than rolled back. Saved-song PCM, physical capture,
acoustic calibration and full P7 remain open.

### F5. Complete portable product seams

**Hocket recorder composition (2026-10-13): shared extraction and Woodshed surface adoption implemented; bounded native acceptance complete.**
The [host-refactor follow-through](../ports/hocket/design_docs/2026-07-08_genet_host_refactor_plan.md#embeddable-recorder-follow-through-2026-10-13)
owns the shared controller/surface and its original
[197-case/four-capture extraction receipt](../validation/hocket-recorder_20261013/receipt.json).
Shared model/project/controller/views live in root `crates/`; standalone
Hocket supplies native services. Woodshed now mounts the shared surface
through borrowed accepted-backing observations and guarded host controls.
Its ordered heterogeneous LoopPlan, Set/Card and saved-bar custody, single
OutputRuntime, compressed companions and WAV contract remain host-owned.
The separate [adoption receipt](../validation/hocket-looper_20261013/receipt.json)
records 591 distinct focused cases, four native processes/seven reviewed
captures, fresh compact reopening, exact sealed media retention and byte-identical
backing-plus-loop WAV. Shared DSP extraction remains later work.

Shared views require a coherent Cambium type graph. The adoption preserves
the concurrent Rust 1.99.0/MSRV 1.97.1 and Murm supplier move; its compiled
source uses Mere `0b9b7a9` and Genet `7a8c5b8`, recorded in the receipt.

**Mere application-door repin (2026-10-15).** Mark ruled on 2026-10-14 to
repin forward
([survey brief, Rulings §4](2026-10-14_dsp_graphshell_hocket_survey_brief.md)).

- **Revisions.** The root, `ports/hocket` and `validation/relationship-recipe`
  now pin:
  - Mere `90ce8f12d`;
  - Genet `ae3d9f5d`;
  - Murm `3d9e799f`;
  - Netrender `f4f9c0d8`, through the `www` source with Mere's restated
    `github.com/merely-made/netrender.git` bridge patch;
  - the vello fork `276b4664`.

  This follows Knot's `a27911a` cascade.
- **Custody imports.** Custody now comes from
  `graphshell_endpoint::door` (feature `door`). `graphshell` is no longer a
  dependency of any of the three roots, so the p2panda patch rows and the
  Retinue compatibility source keys, which existed only for its transport
  cone, are removed.
- **Member crates.** Direct Mere and Netrender pins in `woodshed-core`,
  `woodshed-graph`, `woodshed-genet` and `hocket-views` moved too. None of
  the three lockfiles has a package from two git sources.
- **CPU checks only (no native runs).**
  - The root workspace checks with `--all-targets`.
  - 586 root tests pass (6 ignored) across `woodshed-genet`,
    `woodshed-audio`, `woodshed-core`, `woodshed-graph` and `hocket-views`.
  - The Hocket workspace checks.
  - 101 `hocket-genet`/`hocket-engine` tests pass (1 ignored).
  - The recipe binary checks. Its test target still fails on a missing
    `tempfile` dev-dependency used by the included `storage.rs` tests; that
    gap predates this repin (since `8ec1578`).
- **Not covered.** `ports/redshank` keeps its own pin and is not part of
  this repin.
- **Historical families.** The `8568f42`/`2c7ef73` family cited in the Tabard
  and Hocket host-refactor plans is the family those slices qualified, not a
  current pin.


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
- **2026-10-11:** The separate `0032f35` follow-up closes the earlier Hocket
  and Redshank appearance-reopen presentation residuals, with four reviewed
  nonblank captures. Its [port receipt](../validation/port-reopen_20261011/receipt.json)
  preserves the failed attempts and exact source/binaries. The next root slice
  implements the single-output runtime, with 144 audio and 29 focused desktop
  checks and three muted one-stream device runs at `4a4aba4`. Source/binary
  hashes and scope are recorded in the [runtime receipt](../validation/output-runtime_20261011/receipt.json).
  The prepared GUI seed/reopen and metronome stop/restart runs still await the
  shared foreground handoff; full P7 and durable PCM remain open.
- **2026-10-11:** After Knot completes its native work and exits the acceptance
  apps, the root's five frozen GUI fixtures pass without further product-source
  edits. Set and retained preview/rehearsal fresh-process reopening, exact saved
  compatibility backing, bounded Hear/cancellation and click stop/restart are
  qualified with 791 presentations and 16 reviewed captures. One output stream
  and unopened input hold at all 17 checkpoints, and five coherent metronome
  clock samples advance strictly through both stop/restart transitions. The
  [native receipt](../validation/output-runtime_20261011/gui/receipt.json)
  preserves source/binary/scenario hashes, logs, presentation counts, capture
  hashes and native-control limits. The one-output runtime slice is qualified;
  physical capture, acoustic calibration, durable PCM and full P7 remain open.
- **2026-10-11:** Set/Card companion persistence is qualified at `a6288eb`,
  on the unchanged UI/output family. Exact f32/rate/frame/hash validation,
  artifact namespaces and no-clobber sealed companions preserve owner custody;
  verified blobs precede atomic session-reference replacement. Live/catalog
  quota is reserved before recording, and unavailable/deleted Card references
  remain explicitly clearable without admitted backing or output. Completion
  service saves current practice without a new dispatch or stale full-session
  snapshot. Eight reviewed native captures qualify reorder/reopen, clock
  refusal, Clear/reopen and missing-companion recovery with input unopened.
  The [capture receipt](../validation/set-captures_20261011/receipt.json) retains
  scoped revisions and limits. Existing loop-marker glyph fallback is recorded
  under P8; saved-song PCM, physical capture/acoustic calibration, WAV export,
  orphan collection and full P7 remain separate.
- **2026-10-12:** The next root slice implements admitted backing/completed-loop
  WAV export at `f5cc69d`: one exact cycle through the same Firewheel processor,
  mono float32 at the accepted device rate, explicit native destination and
  reviewed atomic replacement. Frozen capture/source/session custody and
  application-storage protection pass 77 distinct focused checks; the locked
  production build passes. macOS native choice uses an in-process AppKit panel
  after the initial script-backed chooser could not be reached by the native
  provider. Five final native processes/nine reviewed captures qualify actual
  Cancel/Save/Replace, exact reference-equal WAV and preserved sealed capture
  custody, with input unopened. Pipit/Wavicle current-source inspection confirms speech
  framing and lossless WavPack respectively; existing Hound writes WAV without
  another codec or a root-to-port dependency. Source, codec decision and limits
  remain in [P7's follow-through](2026-07-11_stage_set_tools_plan.md#october-12-follow-through-backing-and-completed-loop-wav-export)
  and the [export receipt](../validation/wav-export_20261012/receipt.json).

- **2026-10-12:** Root compressed capture storage uses repaired Wavicle `2c5a49f`,
  explicit codec/manifest versions and per-reference legacy migration. Exact float
  storage selects compression only when smaller; missing/future-codec references
  stay clearable, and verified companions precede the current session. One bounded
  encode-only worker preserves epoch/custody and caches failed publication work.
  The locked current-family build and 76 root/21 supplier cases pass; five
  synthetic child processes and five actual Keeper/profile processes retain exact
  PCM and WAV across migration/reopen, reducing the referenced fixture by 31.6%.
  The subsequent combined font-family source `751858f` passes 556 core/view/host
  cases and the production build. Two normal-output native processes qualify
  actual worker migration and fresh reopening with 54 presentations, three
  reviewed captures, unopened input, one output and exact PCM/full-WAV parity.
  Old raw blobs remain, so collection remains separate; startup timing, glyph
  coverage and physical capture stay open. Details remain in
  [P7's compressed-storage follow-through](2026-07-11_stage_set_tools_plan.md#october-12-follow-through-lossless-compressed-capture-storage)
  and its separate [combined native receipt](../validation/compressed-captures_font-family_20261012/receipt.json).
  Hocket's earlier codec pin remains independent: this root intake does not
  qualify Hocket's archive decoder or adopt the arithmetic repair there.

- **2026-10-13:** Woodshed Looper adoption is accepted and in progress from
  fetched `210235d` (including the concurrent Murm and Rust policy move). The
  shared recorder gets host-sequenced policy and a disposable view projection;
  Woodshed keeps its ordered heterogeneous LoopPlan, Set/Card identity,
  saved-song compatibility, single output and sealed capture/export authority.
  No shadow Hocket Session/History or independent parallel sampler is created.
  Input remains lazy on Record; no output meters are invented where the root
  runtime has no publication. A concurrent Rust-policy GitHub workflow is
  removed under the maintainer's explicit no-workflows instruction, while the
  toolchain/MSRV and supplier changes remain. Focused ownership/action checks,
  export/capture parity and fresh committed native mounting/reopen follow.

- **2026-10-13:** Shared recorder mounting and host-sequenced policy are wired
  into both Looper sources. Disposable observation carries admitted source
  metadata and completed PCM Arcs; accepted arm/start and active recording row
  stay distinct. Runtime controls emit frozen source/session/Card-or-bar
  requests and are checked again after dispatch-tail admission; two queued
  saved-bar clears retain different targets, and re-arm cannot toggle the first
  take. A stale Stop cannot cancel a later take on the same start segment.
  Draft config authoring is distinct from runtime acceptance. Source review
  closes the concrete adapter blockers; native and publication gates follow.

- **2026-10-13:** Woodshed shared-recorder adoption is accepted on compiled
  source `d2d4bc7` after source/ownership review and 591 distinct focused cases
  (repeated sub-process cases are counted once). Four finite native processes
  produced 128 presentations and seven inspected, nonblank captures: normal
  output wide/compact reopening, offline inherited-clock authoring and standalone
  Hocket controls. The reopened 96,000-frame WavPack take retains its exact
  Set/Card owner, clock, digest and decoded float bits; the 192,000-frame backing
  plus loop WAV is byte-identical to the earlier accepted gold. The current
  supplier's private synthetic Keeper is stopped and originals are unchanged.
  Quiet PCM gets a visible centerline without gain normalization; transport
  sizing and active mute/solo colors are corrected. The
  [adoption receipt](../validation/hocket-looper_20261013/receipt.json) preserves
  source/build identities, scoped checks, rejected attempts and native evidence.
  No workflows or PRs are added. Shared DSP, physical microphone/audibility,
  default startup and general Hocket project-worker reply correlation remain open.
