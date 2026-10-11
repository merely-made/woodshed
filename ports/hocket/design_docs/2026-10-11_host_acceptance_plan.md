# Hocket host acceptance and manual audio demonstration

**Status (2026-10-11): landed on the isolated host lane.** Tightens the existing Mesquite scenario
integration after the accepted Tabard appearance lane. This is the Hocket host
slice of Woodshed's October 11 audio-family consolidation. Firewheel API
adoption, session model changes, updater selection and portable project-core
extraction remain separately owned.

## Scope and ownership

The current desktop already uses `mesquite::Lane<Product>` through its
after-frame hook. Mesquite owns scenario input, queued readback, capture
outcomes, receipt writing and finite presentation/readback failure. Hocket
owns recorder observations, project-worker quiescence and appearance close
policy. The earlier audio audit examined source before the Tabard integration;
its custom GPU capture removal is already implemented at this slice's base.
This work preserves the new appearance workflow and its committed receipts.

Hocket still declares direct `image`, `netrender` and `wgpu` member
dependencies although no application source uses them after capture moved to
Mesquite. Remove those member edges and regenerate only the relevant lockfile
dependency list. Retain `paint_list_api`: the waveform/meter leaves use its
`PaintCmd` type. Retain `pollster`: project storage uses it on the worker thread.
No renderer family repin or workspace flattening is needed.

The `hocket-headless` binary opens actual microphone/output devices and runs a
timed capture/replay demonstration. It must require explicit `--run`, explain
that hardware and listening are required, return failure when startup or any
engine operation fails, and return failure if capture never completes. Help
and unknown-argument handling must not open devices. A successful process
establishes completed capture and queued replay; a person must still assess
audibility, feedback, latency and musical timing. It is not an automated CI
hardware gate. Keep the established binary name because callers may use it;
correct its description, command and documentation rather than add a second
parallel harness.

## Phases and done-conditions

### H1 — Shared lifecycle boundaries

- Retain the existing Mesquite pump and product snapshot; extend recorder
  observations only where a deterministic acceptance scenario requires them.
- Include active project save/load/export work in the product's busy reading.
- Route an accepted native close or appearance Save-close through
  `Lane::request_close` while a lane remains unfinished. Keep the host visible
  until the requested pixels and interrupted outcome are written. Cancelling
  the appearance close must leave the lane running.
- Prove readback failure writes a failed receipt without counting a completed
  capture, and prove an early close preserves the queued capture while marking
  the run interrupted. These tests mount the real Hocket hooks and views; they
  use synthetic readback only to produce deterministic completion/error.
- Remove the unused capture-only direct member dependencies; retain all
  legitimate leaf and project worker edges.

**Done when:** the desktop suite exercises failed readback, pending capture,
early close and dirty-close behavior without opening audio or GPU devices,
and the independent locked workspace resolves outside local Cargo config.

### H2 — Explicit manual execution

- Replace the stale pre-Xilem-shell documentation with the current manual
  device demonstration contract.
- Fail startup, arm, tick, replay scheduling and incomplete capture.
- Stop the engine on completion or execution error after successful startup.
- Keep no-argument/help behavior device-free and reject unknown arguments.

**Done when:** focused tests prove argument admission and startup refusal,
the binary builds, and the README/port guide state the same limits. Actual
audible acceptance is recorded separately if a hardware run is performed.

### H3 — Native regression and fresh process

- Build the independent Hocket desktop with the pinned stack.
- Replay a finite Tabard seed and fresh-process reopen under one private
  settings/appearance/library profile and separate capture outputs.
- Replay deterministic recorder controls through real retained input in a
  device-free scenario profile, with explicit state assertions.
- Inspect original captures and report native presentation, pixel review,
  saved appearance reopening and audio hardware qualification separately.
- Record source revision, binary hash, launcher outcomes and incomplete runs.

**Done when:** the committed-source scenarios produce successful Mesquite
receipts and reviewed nonblank captures, and fresh-process reopening restores
the asserted saved appearance. Native hardware recording/playback is outside
these device-free proofs.

## Findings

- **2026-10-11:** `hocket-genet/src/scenario.rs` already uses Mesquite at base
  `95abb13` (Tabard source from `cd69da0`). The earlier bespoke GPU reader no
  longer exists. Three member dependencies remain unused.
- **2026-10-11:** the close hook in `hocket-genet/src/main.rs` exits directly;
  appearance Save-close also directly requests exit. Neither currently asks
  Mesquite to finish an interrupted scenario and its pending readback.
- **2026-10-11:** `AppState::is_project_io_active` describes save/load/export,
  but the scenario product reports only readback busy. Handoff write/read do
  not currently expose an active status, so this slice cannot qualify their
  worker quiescence. Their acceptance remains separate work.
- **2026-10-11:** the manual binary exits zero on missing audio hardware and
  logs arm/tick/scheduling failures without consistently failing execution.

## Progress

- **2026-10-11:** isolated lane created from the coordinator's plan commit;
  source ownership and preserved Tabard receipt verified. Focused lifecycle
  repro and manual execution checks are in progress.
- **2026-10-11:** H1/H2 implemented. The pre-fix mounted close reproduction
  fails because the host exits before its queued pixels complete; the same
  regression passes after close deferral. Desktop tests pass 68 checks with
  one keeper-specific test ignored; the explicit manual demo passes two CPU
  checks. The desktop/manual binaries build with Rust 1.97.1, and locked full
  metadata resolves outside repository Cargo configuration. The lockfile
  change removes exactly three unused member edges without repinning the
  renderer or audio family. Native committed-source seed/reopen and recorder
  control qualification is next; actual audio hardware remains unqualified.
- **2026-10-11:** H3 qualifies source `9dc042f` through five owned finite
  LaunchServices processes, 178 presented frames and 19 reviewed nonblank
  original captures. Tabard seed/four-mode/narrow and same-identity flows pass;
  two fresh processes restore the blue and then brown installed appearance.
  Retained recorder controls pass add/arm/mute/solo/tempo/clock/Stop assertions.
  The device-free Record refusal remains honest. The
  [exact receipt](receipts/host_acceptance_20261011/README.md) retains the
  pre-fix close reproduction, successful CPU/build logs, source/binary hashes,
  launcher outcomes, original captures and actual qualification limits.
