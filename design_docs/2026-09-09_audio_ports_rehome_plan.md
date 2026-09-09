# Audio ports rehome plan

**Status (2026-09-09): IN PROGRESS.** The maintainer has ruled that Hocket and
Ringdown move into Woodshed's `ports/` alongside Redshank, while remaining
separately named, versioned, licensed, tested, embeddable products. The same
ruling changes Woodshed's repository license from MIT OR Apache-2.0 to
MPL-2.0.

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
- Woodshed root package metadata and license files state MPL-2.0;
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
  Woodshed root adopts the same license.

## Progress

- **2026-09-09:** Plan opened after the maintainer approved the rehome and the
  Woodshed MPL-2.0 conversion. Source-remote checkpoint pushes completed;
  history import, rewiring, validation, and relocation notices remain.
