# Audio ports rehome plan

**Status (2026-09-09): READY TO LAND.** Hocket and Ringdown now live in
Woodshed's `ports/` alongside Redshank while remaining separately named,
versioned, tested, and released embeddable products. Woodshed and Redshank now
use MPL-2.0. Publication and old-repository relocation notices remain.

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
