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

Use independently reviewable branches and exact-revision integration. Root CI,
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

Done when all four workspace roots have committed locked-resolution and scoped
CPU-test CI jobs; Windows release triggers and version parsing agree with
product-qualified tags; Hocket uses Mesquite capture lifecycle with truthful
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

Follow [P7](2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set)
for Set-derived LoopPlan, stable capture segment identity, saved-song migration,
and companion persistence. Done when one output device/clock runs click,
preview, Set playback and loop voices without callback control mutexes or
recording allocations; the connected Hear/Add/rehearse/loop/edit/reopen flow
passes current-source automated and native acceptance. Preserve exploration
and deliberate practice as complementary flows.

### F5. Complete portable product seams

Done when Hocket project/media/store/export/handoff operations can be used
without its device runtime; Redshank shares portable transitions required by
its standalone and Turnstone hosts, explicitly admits host audio/microphone
capabilities, and completes its ruled extraction after existing gates. Shared
path crates need independent Git/package sourcing before extraction. Package
renaming, retiring updater transports, and promoting browser previews to full
products require explicit product decisions, not generic cleanup.

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
- **2026-10-11:** CI checks only the root workspace. Windows release still
  listens for `v*`. Hocket owns custom blocking readback and ignores capture
  errors. Redshank publishes full immutable model generations without pruning.
  Woodshed starts two output engines with callback mutexes/recording allocation;
  its independent SongDoc/bar editor remains the open P7 authority duplication.

### Progress

- **2026-10-11:** Refreshed remotes, created isolated integration worktree at
  `cd69da0`, and opened parallel foundation lanes. No stack repin is inferred
  from newer upstream revisions; native receipts remain revision-specific.
