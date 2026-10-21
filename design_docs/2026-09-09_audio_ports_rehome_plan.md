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

**Audio-pack status (2026-10-20): AP0 ASSESSED; AP1/AP2 IMPLEMENTED.**
The [assessment results](#ap0-assessment-results-2026-10-19) select sequencer
extraction/reuse, midly for MIDI files and FunDSP as the preferred programmable
DSP supplier within Firewheel. Device-free probes establish embedding routes;
AP1 implements portable extraction, midly export and first sample-kit
admission/snapshots, with working bundled/directory/TOML/reopen demonstrations.
AP2 connects shared controls, retained edited selections and existing app
runtime/project paths. Its Mere span-binding increment is concrete; larger
composition surfaces remain planned. Build/file evidence is scoped below;
AP3 now has bounded macOS control/save/reopen evidence and owner-observed
guitar playback. Windows/Linux, performance latency and broader instrument
qualification remain open.

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

**Canonical bar-clock intake (2026-10-16): implemented and CPU-qualified.**
The approved target is Hocket's `audio_primitives::frames_per_bar`:
quarter-note BPM, denominator honoured, one rounding per beat, multiplied by
measure count. Source baseline is Woodshed `aba2c73`.

The arithmetic change also affects existing quarter-note Set captures:

| Rate | BPM | Meter | Measures | Existing frames | Canonical frames |
| --- | --- | --- | --- | --- | --- |
| 48,000 | 120 | 4/4 | 1 | 96,000 | 96,000 |
| 48,000 | 120 | 3/8 | 1 | 72,000 | 36,000 |
| 48,000 | 113.5 | 5/4 | 2 | 253,744 | 253,740 |
| 48,000 | 137 | 7/4 | 3 | 441,460 | 441,462 |

These were evaluated in Rust with the current Bar expression and the actual
shared click module, rather than by comparing decimal durations. Both
`capture_persistence::restore` and `audio::loop_projection::LoopCaptures::restore`
recalculate expected frame length using `Bar::duration_samples`; changing it
alone would refuse the differing old takes. `LoopCaptureRef` retains exact
frames and media digest but has no rounding-policy field. Stored `SongDoc`
contains musical settings, no sample-clock field or persisted PCM, so it can
adopt the new calculation without a stored-song schema migration. The earlier
96,000-frame sealed fixture and four-second WAV gold use the unchanged
120 BPM quarter-note clock; they do not qualify the fractional-clock case.

The initial assessment stopped at the handoff's Task 2 migration gate. Mark
then chose legacy clock preservation, recorded below. No original receipt or
fixture blob is rewritten; qualification writes new artifacts.

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
- **Redshank (same day).** `ports/redshank` and its `web` member moved to the
  same family with the same bridge. Redshank uses neither `graphshell` nor
  `mere-view`, so no code changed. Its workspace checks with `--all-targets`,
  `redshank-web` checks for `wasm32-unknown-unknown`, 262 tests pass
  (7 ignored), and the lock has no duplicate git sources. Its pre-existing
  unused p2panda/muniment patch rows are untouched.
- **Follow-up bump (later 2026-10-15).** All four roots moved to Mere
  `8357b00d`, which includes the Graphshell lift's P2/P3 (`e92572a4e`); the
  rest of the family is unchanged. CPU only:
  - all four roots check;
  - 761 root tests pass (6 ignored), now including `woodshed-views`;
  - 101 Hocket and 262 Redshank tests pass.

  Mere then made those helpers generic over a host camera (`a26ed1caf`), and
  all four roots moved to `ea530d41`. The three-surface adoption is recorded
  in the [stage plan](2026-07-11_stage_set_tools_plan.md).
- **Stage A repin round (later 2026-10-15).** All four roots and the member
  crates moved to Mere R2 `b2c8dcc2`, Genet `7ffe6620`, and Murm `fb6360c1`
  through the canonical `www` alias. The `edit-history` patch rows are now
  `backstep`. CPU checks and counts are unchanged (769/101/262). The handoff's
  Task 1 records the remaining Genet-sourced `murm-wire` split.
- **Stage B (same day).** All four roots name the family repos by
  `branch = "main"` through one canonical URL, and their committed locks are
  the tested revisions. Repins are now `cargo update -p`. The `murm-wire`
  split resolved, because Genet now names Murm by branch too.
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

**Clock preservation ruling (Mark, 2026-10-16).** Existing captures retain
their original whole-block frame count and legacy beat/count-in scheduling;
new captures use the canonical beat-rounded clock. Add an explicit capture
clock policy, default missing fields to legacy, and carry it through restored
backing and export. Do not resample, trim, pad or rewrite old PCM. Clear
returns a segment to canonical timing; recording refuses a loaded legacy-clock
segment until explicitly cleared, because a take may span every loaded segment.
Qualify old/new sealed references, exact fractional and x/8 frame boundaries,
count-in, zero-allocation and live/WAV parity with a new receipt.

**Implementation (2026-10-16).** `Bar` defaults to canonical timing, using the
shared beat/bar helpers; prepared backing uses those integer beats for measure
and count-in boundaries. Meter denominators also govern pattern playback and
offline rendering. Metronome phase accumulates integer frame numerators rather
than fractions so rounded beat boundaries do not drift by a sample; tempo
edits rescale that phase. Legacy playback retains the previous floating-point
operation order for whole-block lengths, truncated beats, count-in and chord
envelopes.

A missing capture `clock` field means legacy. New takes name `per-beat-v1` in
a V3 manifest; unknown policy strings survive deserialization and refuse
admission. Old-only catalogs retain V1/V2 as appropriate, and unchanged
references keep their existing codec, digest and frame count. The playback
projection carries the policy through recall, transport, waveform buffers and
WAV export. Clear switches the runtime segment to canonical; new empty bars
do not inherit legacy timing. New recording is refused before opening input
when a loaded legacy segment remains, with an explicit Clear message.

Ringdown's file format and Hocket's capture schedule remain separate and
unchanged. No shared frame-driven DSP crate is extracted in this clock slice.

**Clock-intake qualification (2026-10-16, source `9d27b06`).**
The new [receipt](../validation/canonical-clock_20261016/receipt.json) records
745 distinct passing suite cases (157 audio, 241 core, 164 host, 183 views),
plus one separately invoked accepted-fixture WAV probe; child runs and
repeated suites count once. The desktop build and all-target host check pass.
Exact-frame fractional/x/8 capture, zero-allocation renders, 48 kHz
heterogeneous live/WAV parity, metronome phase, sealed mixed-policy reopen,
and the sealed filesystem fresh-process sequence pass. Both legacy and
canonical renders of the accepted four-second fixture are byte-identical to
the existing WAV gold (`6c072633…`), and both original companion hashes still
match their historical receipt. This work leaves every archived profile file
byte-identical to its task-start value.

The sealed mixed-clock fixture preserves a 253,744-frame legacy 5/4 block
and a 105,110-frame canonical 5/4 block together, including original digest,
encrypted companion and exact decoded bits. Product Set captures still use
a quarter-note denominator; x/8 tests exercise the runtime, not a new meter
editor. No native foreground or physical-audio qualification is claimed.
This closes the bar-clock prerequisite; shared DSP extraction remains open.

### October 16 shared capture-state extraction

Handoff task 3 is implemented in root `audio-primitives::capture`: caller-owned
sample accumulation, InputFeed, host-stamped CompletedTake, and Hocket's
poll-driven BarCountdown. Hocket and Woodshed retain their different capture
schedules, input integrity admission and stream ownership. The shared dev-only
harness and existing adapter suites pass 256 cases; both desktop hosts pass
locked offline all-target checks. The [extraction plan](2026-10-16_shared_capture_accumulator_plan.md)
and `validation/shared-capture-seams_20261016/receipt.json` hold scoped evidence.
No persistent clocks or PCM changed; earlier native/export receipts retain their
original sources. Physical input, audibility and latency remain independently
unqualified by this work.

### October 16 Hocket project-worker correlation

The standalone host now stamps all five project worker operations with Armillary
request IDs and host session/replacement scope. Explicit admitted pending work
owns busy state, including handoff read/write. Correlation protects saved-file
association, live project replacement and staged incoming data from late replies.
The [port plan](../ports/hocket/design_docs/2026-10-16_project_worker_correlation_plan.md)
owns implementation and CPU evidence; native project/handoff qualification,
physical audio and signed turn operations remain separately gated.

## Shared data-defined audio packs (2026-10-19)

### Direction and ownership

Mark approved bundled modules and user-installed extensions expressed as
TOML/JSON plus assets. A pack can describe a drum kit, rhythm, instrument or
effect composition using the host's supported building blocks. Bundled and
installed packs use one admission path. Definitions may configure existing
devices and compose supported sample, oscillator, envelope, filter and routing
operations; a new DSP algorithm requires an implemented Rust supplier. The
initial vocabulary and file layout remain assessment outputs, not frozen APIs.

This section owns the shared work. Product plans link here and own their
consumer behavior. Shared portable code belongs in root `crates/`; runtime
adapters use the existing standalone Firewheel foundation. Preserve each
product's model, history, clock and independent workspace. Reuse existing
Cambium views and parameter metadata for controls. No new device stream,
transport master or port-to-port dependency follows from adding a pack.

Pattern generation and MIDI-file writing work without an audio device or
Firewheel. Auditioning projects those same musical events into the existing
audio runtime. MIDI export describes notes and timing; it does not carry a
sample kit's sound or make an arbitrary synthesis patch playable on a guitar.

### AP0 — Assess existing owners and embeddable Rust suppliers

**Status (2026-10-19): bounded assessment complete.** Source/dependency review,
isolated embedding probes and target gaps are recorded below. AP1 is next;
these observations do not qualify arbitrary patches or physical playback.

Done-conditions:

- Trace `woodshed-audio::sequencer::{SequencerPattern, Track, Step,
  Subdivision, TimeSignature}`, `Sound`, `SampleBank`, offline rendering and
  current processor consumers. Reuse that sequencer and its timing semantics.
  Identify required additions such as MIDI note mapping, velocity, duration
  and multi-bar patterns; do not create a second pattern model. If Hocket or
  Ringdown cannot consume its current crate without device dependencies,
  extract the existing portable owner with adapted callers in one slice.
- Produce a supplier decision in this section: embed, adapt, defer or reject,
  with exact revisions/features, license and asset requirements, dependency
  closure, supported targets, API fit, preparation cost and real-time behavior.
  Examine the existing sampler first; evaluate
  [midly](https://crates.io/crates/midly) for MIDI files,
  [RustySynth](https://crates.io/crates/rustysynth) for SoundFont audition,
  [FunDSP](https://crates.io/crates/fundsp) for synthesis/effect primitives and
  [Phosphor's pattern module](https://docs.rs/phosphor-core/latest/phosphor_core/pattern/)
  for reusable sequencing logic only where it fills a verified gap. Check
  Phosphor's containing engine dependencies before adopting it. This shortlist
  is research, not a dependency selection or claim of working integration.
- Inspect Isocosm's manifest/asset admission and retained authoring approach
  (`isocosm/crates/isometry-system/src/sys/generator.rs` and
  `isocosm/design_docs/2026-10-17_private_pack_authoring_plan.md`). Reuse proven
  mechanisms where ownership fits; establish audio-specific semantics here.
  Do not acquire a game-system dependency to reuse its packaging pattern.
- Record bounded embedding evidence separately from source inspection:
  device-free pattern-to-MIDI, candidate sound rendering through the existing
  graph, allocation/lifecycle findings and Windows/Linux/macOS support or
  explicit gaps. Browser support is separately qualified. Choose a smallest
  useful kit slice and list what still needs implementation before proceeding.

### AP1 — Admit portable definitions and assets

**Status (2026-10-19): shared file/API slice implemented, bounded demonstration complete.**
`audio-pattern` owns the moved existing
sequencer/sound modules and byte-oriented MIDI export. `audio-packs` owns shared
WAV loading, bounded TOML/JSON admission and retained exact-source/asset snapshots.
Woodshed's audio facade re-exports those owners; runtime preparation remains in
its existing processor. No application views or guitar writes are added here.

**Implemented slice:** extract the existing portable pattern/sound
owner and adapt its current callers together; retain host WAV loading and
production runtime preparation at their existing boundaries. Add midly export
and one bounded one-bar sample-kit definition using the existing sampler.
Support both bundled and externally installed kit directories through that
same path. A later FunDSP-backed synth/effect definition extends the operation
vocabulary after the first kit establishes identity, assets and admission.
Do not build a second sequencer, general patch editor or alternate graph host.

Done when a versioned TOML/JSON definition names stable pack/device identities,
required operation capabilities, parameter metadata, patterns and referenced
assets. One bundled kit and one externally installed kit use the same loader.
Validate paths, graph connections, numeric ranges and bounded resource needs;
report unsupported operations and missing assets before activation. Parse,
decode and prepare outside the audio callback, then admit a prepared replacement
through the existing runtime lifecycle. Failed edits retain the active pack.

Product saves bind the exact definition and asset versions they used. Resolve
asset custody, attribution, reopen, replacement and uninstall behavior before
claiming portable projects; a mutable installation path cannot be a session's
only identity. Assess existing media/storage owners before adding packaging
infrastructure. Richer node composition follows evidence from the first kit.

Implementation and evidence:

- `crates/audio-pattern` owns the moved `sequencer.rs` and `sound.rs`, retaining
  their existing wire representation/kernels. It adds checked authored-pattern
  admission and a midly format-0 byte export, with integral musical timing,
  channel/note/velocity mapping, note endings and an explicit final bar tick.
- `crates/audio-packs` owns the moved `SampleBank` and WAV decoder. Format 1
  admits sample voices, their MIDI assignments, parameter metadata and existing
  patterns. Bundled bytes, installed directories and retained snapshots share
  the same final admission/decoding path. The fixed sample-to-mono route uses
  the existing runtime; variable patch graphs remain later vocabulary work.
- Pack snapshots retain original TOML/JSON source and encoded assets under a
  versioned content digest. Captured packs do not read their installation again;
  replacement/uninstall cannot alter them. Future/corrupt snapshots, unsupported
  capabilities, invalid ranges, missing assets and unsafe references return
  errors before preparing a replacement. Source bounds and normalized finite
  sample PCM keep decoding/preparation bounded; no callback parsing or I/O.
- `assets/audio-packs/basic-drums` contains one original generated kick/snare/hat
  kit and a backbeat. `audio-pack` exports MIDI/snapshots without any device or
  Firewheel dependency. `woodshed-audio --example drum_pack` renders through the
  actual production processor with no device opened. Its facade re-exports the
  shared types, so existing callers retain one implementation.
- [AP1 receipt](../validation/audio-packs-ap1_20261019/README.md): macOS builds
  and fresh-process file workflows, equal bundled/directory MIDI and WAV bytes,
  edited user-kit snapshot reopening after its source directory moved, and
  equivalent TOML/JSON musical output. Shared libraries compile for Wasm on
  Rust 1.99.1 and natively with rustc 1.97.1 (Cargo 1.99.1). Cargo 1.97.1
  requests a workspace lock rewrite under `--locked`; that invocation is not
  qualified. The root lock adds only the two local
  packages and midly; existing external versions/edges and port locks remain.

This AP1 slice established portable pack saves and runtime consumption through
the shared file/API workflow. AP2 below adopts the snapshot into application
saves and controls. Native GUI, Windows/Linux execution, browser audio and
physical/guitar playback were not qualified by AP1. No test suites were added
or run in that slice.

### AP2 — Share authoring, audition and MIDI export

**Status (2026-10-20): implemented; compilation and file workflow evidence.** The shared controls project
the existing sequencer pattern retained with its exact pack snapshot. Woodshed
adopts it into the metronome/backing runtime and sealed practice save; Hocket
keeps a local editable kit draft in its project manifest and commits rendered
material through the existing recorder import/history path. Hocket handoff
ships normal rendered media, so a receiving peer needs no pack operation.
The kit draft itself is project metadata; contributed layers use the recorder
history path. The current native view has no exposed Undo control, so kit copy
must not promise one. Draft edits remain outside recorder history. Ringdown adds an offline branch to its existing
probe command using the same file workflow; it performs no instrument writes.

Shared Cambium buttons serve step/rest/accent, mute, level, pattern selection
and MIDI export. One-step hit moves use Scenomise's existing quantized span
operation and a revision-qualified music edit. This advances the Mere binding
seam without creating a graph editor or making scene layout the audio clock.
File loading, MIDI export and Hocket rendering run off dispatch/audio callbacks;
job completion checks the requesting session and frozen source/clock/target.

Implementation details and scope:

- `audio-packs::PackSelection` retains the edited current pattern and inactive
  pattern drafts beside the immutable snapshot. Switching patterns preserves
  edits. A skipped prepared-sample cache avoids decoding on ordinary controls;
  deserialized selections are admitted before product adoption. App tempo is
  projected explicitly into both audio and MIDI; sample gain/assets are audio
  data, while MIDI uses the kit's note/velocity mapping.
- `audio-pack-views` supplies the same Cambium buttons to Woodshed's Metronome
  page and Hocket's recorder view. Existing app transports remain the clock
  controls. Edits return checked replacements and cannot redirect stale views.
- Woodshed saves the selection in its existing sealed practice session. A
  session-qualified file worker loads kits/exports MIDI; the existing output
  processor admits sample steps for metronome, practice-program and backing
  playback. Backing uses its accepted bar clock; count-in retains its existing
  conventional click. Looper WAV export freezes the same selection into its
  existing worker and uses the same prepared backing rhythm. Closing waits for
  requested kit file work to finish.
- Hocket's existing correlated project actor loads kits, exports MIDI and
  renders a complete phrase at project tempo. A frozen session/source/clock/
  track check precedes the existing recorder import and history commit.
  Project manifests retain the local editable draft and exact assets, including
  dirty/save acknowledgements and reopening. Handoff omits the local draft and
  carries normal rendered media under existing session/history custody.
- Ringdown's existing probe returns from `--audio-pack` before discovery,
  reusing the portable file command. No pack/audio dependency enters its
  sans-io protocol core and no guitar file is written.
- The existing pure offline renderer moved into `audio-pattern::render`, with
  the old runtime API re-exported. Hocket renders exact frame counts through
  that same implementation; there is no new sequencer or runtime clock.

[AP2 receipt](../validation/audio-packs-ap2_20261020/README.md) records source,
locked compilation and an edited retained-selection file workflow, including
pattern switching and source-absent reopening. Native UI/project dialogs,
sealed-project/`.hock` fresh-process interaction, Windows/Linux execution,
browser audio and physical/guitar playback are not qualified by this slice.
No test suites were added or run. AP3 owns those acceptance boundaries.

Done when the reused sequencer drives audition and MIDI export from the same
admitted pattern, with explicit meter, tempo, accents/velocity, note endings
and loop boundaries. Any random generation has a retained seed or resolved
events so the exported rhythm agrees with the audition. Reopen preserves the
authored definition, pattern and assets. Shared Cambium controls serve the
product's existing state owner; define no separate editor-owned song document.

Woodshed uses packs for its existing Metronome, Rehearsal and Set-derived
backing. Hocket uses them to audition and create loop material under its
session/history and local transport. Specify whether a contribution retains
editable definitions or a rendered layer, including assets needed by a peer;
preserve handoff behavior when that peer lacks an optional operation.
Ringdown consumes portable authoring/export and instrument-specific validation;
its protocol core remains sans-io and does not require a local audio engine.

### AP3 — Qualify product and instrument paths

**2026-10-20: bounded macOS and guitar acceptance passed; target matrix open.**
[AP3 evidence](../validation/audio-packs-ap3_20261020/README.md) records actual
controls and native file dialogs in private synthetic profiles. Remove the
installed kit before fresh-process reopen; compare exact edited selections,
MIDI output and retained/rendered media. Keep Windows/Linux and timing claims
separate from this x86_64 macOS acceptance run.

Both actual native application paths pass, each across two processes with
native file dialogs and reviewed captures. Woodshed's sealed practice save
retains exact edited selection/assets and reproduces MIDI bytes without the
installation. Hocket's `.hock` reopens its kit drafts and ordinary rendered
media, reproducing both MIDI and nonzero stereo WAV bytes exactly. Bad kit
replacement preserves the accepted project. Selected owning-package tests:
701 passed, zero failed, two existing display tests ignored; thirteen new
regressions. All three lockfiles retain the AP2 hashes. Product changes remain
cross-platform Rust/shared controls; Mac-specific scripts are acceptance tools.

AP3 found two Hocket playback faults with slow native UI frames: input overflow
returned before servicing output graph changes, and phase-only pending-layer
observation could miss every bar wrap. Playback/stops now continue while input
loss still aborts incomplete capture and reports its error. Pending playback
also reads a deadline from Firewheel's existing DSP clock, invalidated on tempo
restart; capture's existing wrap policy is unchanged. This prevents indefinite
queuing and does not establish sample-accurate or low-latency performance.
Shared control labels/layout and Hocket panel bounds were repaired during
actual control trials. Hocket's kit notice now describes the exposed controls
without promising an Undo button.

**Guitar result (user-observed).** The owner approved one additive file,
`11-Ringdown_AP3.mid`, after reviewing the artifact. It was copied exactly,
all ten factory beat hashes were unchanged, and storage was safely ejected.
The owner reports: “It appears and plays repeatedly.” The candidate repeats
an actual Woodshed-app-authored bar five times, at PPQ 480 / 4/4 / channel 10,
using kick 36, snare 38 and hat 42. This proves custom-file discovery and
repeated physical playback on this guitar. Five identical bars cannot identify
the precise loop region or count-in semantics. After a power cycle with USB
OFF, the owner confirmed: “It still appears and plays after restarting.”
Reboot persistence is now qualified on this instrument. Complete voicing,
other firmware and Beats RPC/pedal control remain separate checks.
No factory beat or configuration was written by the assistant.

**Loop/count-in follow-up (2026-10-20): baseline physical trial complete.**
The owner-requested
[AP3 diagnostic](../validation/audio-packs-ap3_20261020/README.md#distinguishable-bar-loopcount-in-trial)
reuses shared patterns/export/render for five distinguishable bars. After
explicit approval, exact readback, unchanged prior file hashes and safe
ejection, the owner heard A–E once and B–E repeating. Stop/wait/start replayed
A once before the same B–E loop. Playback began directly with A; the owner
found no separate count-in setting. Thus this five-bar 4/4 file supplies its
own one-bar lead-in, followed by a four-bar repeat body. No extra generated
click count-in was reported. A post-copy extra directory entry remains
unidentified in the write receipt.

Ringdown's guitar-specific export profile should explicitly separate that
lead-in from the intended loop body, preserving generic shared MIDI export.
This is a planned consumer adaptation, not implemented export behavior.
Qualify other file lengths/meters and firmware/settings before generalizing;
heard bar order does not establish sample-accurate phase or latency.


Done when bundled/installable pack loading, editing, missing-asset refusal,
replacement and fresh-process reopening have scoped evidence on the target
desktops, and audible playback is qualified independently of file export.
Ringdown separately establishes the guitar's accepted file format, note map,
discovery rules and loop behavior, then offers a reviewable MIDI artifact before
an explicitly approved persistent-device trial. Existing USB file observations
do not prove custom playback. AP0–AP2 can progress with the guitar off.

### Publication and standards boundary (2026-10-20)

The [Ringdown provenance/terms and standards assessment](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#provenance-terms-and-standards-audit-2026-10-20)
owns the adapter audit, evidence frontier and unsent vendor outreach. Generic
pattern/audio/SMF work retains its own rights and assets. Documented MIDI
messages plus owner bindings can support a standards-based controller path;
broader file discovery/intro-loop coverage still needs device qualification.
Detailed app RPC uses custom methods, GATT identifiers, framing and compression.

A public-standard envelope or an offline execution branch does not establish
permission or artifact isolation. The subsequent
[build isolation](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#standards-based-midi-and-beats-build-isolation-2026-10-20)
uses the existing CLI with private features disabled by default. MIDI moved to
`woodshed-instrument`; `woodshed-audio` re-exports its one implementation.
The private session/example moved to `ringdown-ble::instrument`, removing even
optional private dependencies from the shared MIDI crate. Target graph and
host compiler evidence are scoped in the linked receipt. Subsequent
[split acceptance](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#split-acceptance-2026-10-20)
adds native OS MIDI capture, actual CLI/release exports and kit removal/reopen,
negative dependency controls and read-only live RPC status. Source archive and
other-platform release qualification remain separate.
The subsequent [BLE-MIDI peripheral integration](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20)
adds a connection variant on that same writer and reuses the desktop picker,
clock worker/status and CLI. One supplier covers all three native APIs; Mac
has one observed guitar trigger, with Windows/Linux target compilation and
native runtime qualification still separate. The private RPC exclusion holds.
Original synthetic kit assets remain the default; imported
packs and retained snapshots need appropriate asset redistribution rights.
Larger adaptive-audio work can proceed independently of a supported private
RPC adapter. Neither standards conformance nor physical playback is a legal
clearance. Publication review remains scoped to the real artifacts, terms,
provenance and distribution jurisdictions in the canonical adapter assessment.

### Findings

- **2026-10-19, source-inspected:** `crates/woodshed-audio/src/sequencer.rs`
  already has serializable multi-track steps, accents, subdivisions and meter;
  `sound.rs` separates serialized sample IDs from prepared PCM, and
  `offline.rs` renders patterns. These are reuse inputs, not a qualified pack
  loader or complete MIDI note model. Missing sample buffers currently render
  silence; pack admission must expose missing-asset failures to the player.
- **2026-10-19, upstream documentation inspected:** midly supplies MIDI
  parsing/writing; RustySynth supplies pure Rust SoundFont synthesis; FunDSP
  supplies composable DSP. Phosphor documents a pattern-to-MIDI module.
  No candidate was built, integrated, benchmarked or selected in this pass.
  This entry describes the initial planning pass; the subsequent AP0 probes
  and decisions below supersede its unbuilt/unselected status.

### AP0 assessment results (2026-10-19)

**Decision:** reuse the existing sequencer and sound/sample behavior, embed
midly for portable MIDI serialization, adapt FunDSP inside Firewheel for
programmable instruments/effects, retain RustySynth as an optional SoundFont
adapter, and reject a direct Phosphor-core dependency. These select integration
directions; product dependencies and adapters remain unimplemented.

[Receipt, probe sources and reproducible scratch setup](../validation/audio-packs-ap0_20261019/README.md)
retain exact supplier checksums, feature closures, source hashes and command
logs. They distinguish successful evidence from initial toolchain failures.

| Piece | Decision and fit | Dependencies, preparation and limits |
|---|---|---|
| Existing `SequencerPattern`, `Step`, `Track`, `Subdivision`, `TimeSignature` | **Adapt/extract existing owner.** Original modules compile in isolation. Preserve existing serialized data and notated-beat subdivisions; adapt current consumers in the extraction slice. | Current `woodshed-audio` unconditionally brings CPAL, midir, pitch detection and Firewheel. Its portable modules need Serde, existing DSP helpers and, for `SampleBank`, Hound. Keep file loading outside a pure pattern API; `audio-primitives` retains its dependency-free/I/O-free boundary. |
| Existing `Sound`, `SampleBank`, `PreparedClick`, Firewheel sampler | **Reuse/adapt for first kit.** Float32 WAV loading, rehydration, exact production-graph playback, stop and restart passed. Hocket already uses `SamplerNode`; add no audio backend. | Root prepares mixed step PCM off callback; eight overlapping step envelopes are bounded. It pre-mixes simultaneous tracks, so independent note-off/choke and velocity layers are future capabilities. Current samples are mono with linear interpolation; missing buffers silently produce no voice. Admission needs path/size/finite-value checks and explicit missing-asset errors. |
| [midly 0.5.3](https://crates.io/crates/midly/0.5.3), Unlicense | **Embed for MIDI import/export.** Existing patterns exported and parsed back at PPQ 480 for 4/4, 3/8, 7/8 and triplets. | `default-features=false`, `features=["std","strict"]`; zero normal transitive dependencies, no Rayon. Parse/write allocations belong on the control side. Probe note map, velocity and gate choices are illustrative, not a production or guitar profile. |
| [FunDSP 0.23.0](https://crates.io/crates/fundsp/0.23.0), MIT OR Apache-2.0 | **Adapt; preferred programmable DSP supplier.** Dynamic `AudioUnit` rendered within a disposable node on the owned Firewheel fork. Reuse its oscillators/envelopes/filters where existing Firewheel factories do not cover the operation. | `default-features=false`, `features=["std"]`; measured normal closure 34 packages including FunDSP, without CPAL, Symphonia or FFT-convolver. Prepare rate/buffers with `allocate` off callback. Block API accepts at most 64 frames; use bounded chunking or its block adapter. The probe used per-sample sine/gain, not arbitrary graphs. Keep its optional graph/sequencer under the device boundary; Firewheel remains the product runtime. |
| [RustySynth 1.3.7](https://crates.io/crates/rustysynth/1.3.7), MIT | **Adapt optionally for SoundFont instruments.** Synthetic SF2 rendered through the same Firewheel node interface, with note-on/off, voice pressure and all-notes-off. First WAV kit needs no SF2 dependency. | No normal dependencies beyond std. Parse SF2 and preallocate voices/effects off callback; caller supplies equal stereo buffers. Probe: 16 voices, effects off. Internal block size defaults to 64; arbitrary-offset MIDI timing needs qualification. Asset size/format validation and each SoundFont's provenance remain host concerns. |
| [Phosphor-core 0.3.97](https://crates.io/crates/phosphor-core/0.3.97), MIT | **Reject direct dependency for this slice.** Its position-derived events/pending-note-off design is useful reference material; our sequencer remains the musical owner. | Published core unconditionally depends on CPAL 0.15, audio-thread priority, Phosphor MIDI/plugin crates and channels. Its pattern file also refers to `transport::Transport::PPQ`. Taking the whole engine or copying a parallel pattern model adds ownership without filling the first kit's gap. Not compiled in AP0. |

**Runtime and portability evidence.** Original source imports plus midly run
on x86_64 macOS with Rust 1.99.0. Four candidate-node runs each render 65,536
finite/non-silent frames (FunDSP and RustySynth, 44.1/48 kHz). The actual
`OutputRuntime` renders 96,000 sample-kit frames; its sixteen onsets match MIDI
positions after accounting for the sample's one-frame initial zero. Measured
render calls observe zero allocating/reallocating/freeing operations on their
thread. Candidate preparation/destruction and graph servicing stay outside
that measured callback region. This is bounded evidence, not a complete
real-time or performance audit.

The isolated portable package with midly, FunDSP and RustySynth compiles for
`wasm32-unknown-unknown` on Rust 1.99.1 and checks natively on MSRV 1.97.1.
Windows/Linux execution, browser audio-host integration and physical playback
remain explicit gaps. The combined runtime probe deliberately links the actual
native `woodshed-audio` crate but opens no devices. No guitar was used.

**Existing-owner gaps.** `offline::render_pattern` is a historical allocating
voice mixer; native playback uses `output_runtime::PreparedClick`. Its sibling
`offline::song_export` uses the production runtime, so importing the entire
offline module is not a portable renderer extraction. Add one shared validated
musical-event projection for audition/export instead of elevating the old
mixer into another runtime. Current patterns have one-bar length and boolean
accents, without explicit MIDI note/channel, arbitrary velocity/gate or choke
groups. Add only the first kit's mapping requirements to the reused owner;
broader composition follows later. Hocket's current Phrase/MediaRef and
`hocket-project` already carry rendered loop layers: use that existing handoff
route first, with editable patch/pattern retention remaining an explicit follow-up.

**Isocosm reuse.** Its manifest-before-assets validation, canonical-root
containment, immutable authored revisions and conflict-aware explicit save
are useful mechanisms. Its loader reads text assets and its private authoring
tool retains game-specific schema/runtime dependencies; it is not a drop-in
binary audio asset loader or reusable audio editor. Reuse the pattern and
existing storage owners; do not import its game-system crate or private parser
fork merely for packaging.

**FunDSP maintenance.** Mark favors midly and FunDSP and accepts an owned fork
if needed. Registry release 0.23.0 is dated 2026-01-07; upstream `master` was
last pushed 2026-03-03 at `559584043aa3369e92d5aad12a5e3b003cfacfd5`, inspected
2026-10-19. The current release passed these probes without a patch. An owned
mirror/fork is a viable supply decision when integration starts; inactivity
alone has not established a technical fork requirement. Preserve the package's
MIT OR Apache-2.0 notices and record any future patch/revision separately.

### Mere and composition surfaces (2026-10-19 assessment)

Mere can project and edit disclosed musical and routing data through an audio
domain binding. Its scene nodes/relationships describe presentation and
interaction; Firewheel nodes schedule audio processing, while FunDSP units
implement signal processing within that runtime. Keep those contracts explicit
when mapping one view to another. A scene's geometry can be authored content
where the music binding defines time/pitch; generic layout movement does not
implicitly edit timing or reconnect DSP ports.

There is an existing seam: Woodshed's arrangement editor consumes
`scenomise::span_edit::{SpanEdit, SpanEditIntent}` for source/revision-qualified
move/resize proposals, then validates its own Set draft. Reuse that approach
for rhythm lanes or phrase spans. Other candidates are a typed patch-cable
view for pack routing, relationships between patterns/variations/instruments,
and explicit performance mappings from scene gestures to musical controls.

Scene dynamics may generate control-rate proposals when the player deliberately
enables that binding; the music owner validates/quantizes/timestamps them for a
bounded audio command path. Retain resolved events or a reproducible seed for
export. Audio-rate oscillators/modulation remain in DSP, and audio sample time
remains the scheduling authority; a GUI frame or force solver cannot set its
deadline. `scenograph::DynamicsSlot` is an opaque physics-spec carrier whose
binding host interprets it, not an existing audio-patch execution contract.

Shared references: `mere/crates/cambium/scenes/scenomise/src/span_edit.rs`,
`mere/crates/cambium/scenes/scenomise/src/template.rs`,
`mere/crates/cambium/scenes/scenograph/src/dynamics.rs`, and
`mere/design_docs/2026-08-12_family_composition_thesis_brief.md` (identity,
content, arrangement and dynamics ownership). These are source-inspected
opportunities, not newly qualified music surfaces. A generic graph patch
editor or generative-dynamics instrument does not block the first kit slice.

**AP2 increment (2026-10-20):** shared kit controls now call the existing
`SpanEdit::Move` operation for one-step musical hit proposals. The music owner
checks the current revision, source hit, grid bounds and destination before
adoption. Next, project those existing tracks/hits into a source-qualified
Scenomise rhythm/phrase scene and route captured gestures through the same
`PackEdit` boundary. Specify gate/duration semantics before enabling resize.
That scene remains a view of retained musical data; typed DSP routing and
deliberately bound dynamics follow as later vocabulary work.

### Progress

- **2026-10-19:** Shared direction and AP0–AP3 recorded at Mark's request.
  AP0 remains open. Hocket and Ringdown link to this owner; this is a planning
  change with no new schema, dependencies, code or hardware qualification.
- **2026-10-19 — AP0 bounded assessment complete.** Recorded supplier decisions,
  actual-source MIDI/render probes, callback allocation observations, Wasm and
  MSRV checks, and explicit platform/physical gaps. AP1's first sample-kit slice
  is now concrete. Mark's midly/FunDSP preference and Mere composition question
  are incorporated; no shipping implementation or owned supplier fork was added.
- **2026-10-19 — AP1 shared file/API implemented.** Extracted the sequencer and
  sound kernels, moved/reused sample loading, added midly export and bounded
  data-pack admission/snapshots. Demonstrated bundled and user-installed JSON
  and TOML kits, production-graph WAV rendering and source-absent snapshot
  reopening. Wasm/MSRV library checks pass. AP2 GUI/project adoption and Mere
  composition surfaces remain planned; no guitar or physical playback trial.
- **2026-10-20 — AP2 implemented.** Shared controls, retained edited selections,
  Woodshed session/runtime adoption, Hocket project-worker/material adoption
  and Ringdown offline export are connected. Pattern drafts survive switching;
  Hocket handoff uses normal rendered layers. Mere's existing span operation
  supplies the first musical edit binding. Compilation/file receipts are scoped
  separately from AP3 GUI, project interaction, cross-desktop and device work.

- **2026-10-20 — AP3 bounded acceptance.** Two native processes per app pass
  real controls/dialogs and installation-absent saves/reopens. Exact edited
  selections, inactive drafts, assets, MIDI and Hocket rendered-media/WAV bytes
  survive. Shared view and Hocket playback faults were fixed; 701 selected
  tests pass (two existing ignored). The owner confirms an explicitly approved
  custom MIDI file appears and plays repeatedly on the guitar. Cross-desktop,
  debug-host latency/capture, precise guitar loop behavior and remote Beats
  qualification remain open. No commit or publication occurred.

- **2026-10-20 — Distinguishable-bar guitar trial complete.** The owner confirms
  first authored bar once, bars 2–5 repeating, and intro replay after stop/start.
  Playback starts directly with the file; no separate count-in setting found.
  Record an explicit lead-in/body split for future guitar-specific export,
  with other lengths/meters and firmware still requiring qualification.

- **2026-10-20 — Adapter provenance/terms follow-up linked.** Ringdown owns the
  bounded standards/private-profile audit and unsent outreach. Keep generic
  authoring/audio and original assets independently releasable; demonstrate
  actual artifact isolation before calling a future controller standards-only.
