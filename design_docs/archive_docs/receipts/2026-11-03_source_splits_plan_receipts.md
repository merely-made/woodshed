# Source splits: H6 Woodshed receipts

## 2026-11-03: S1 portable theory and capture history, twelve paths

Owner: house-style H6 / 01a19f86-cb71-7820-a6ac-aaec39d8bfd4.
Baseline: `56c3418bc270e661f094d3b3c593753b313087bc`. Compact summaries only, under H6's shared
100 MB cap. Frozen captures and prior receipts remain intact.

| Original source | Before | After |
|---|---:|---:|
| `crates/audio-primitives/src/alignment.rs` | 732 | 517 |
| `crates/audio-primitives/src/onset.rs` | 615 | 324 |
| `crates/hocket-model/src/history.rs` | 1290 | 466 |
| `crates/hocket-project/src/handoff.rs` | 823 | 416 |
| `crates/hocket-recorder/src/host_policy.rs` | 603 | 321 |
| `crates/woodshedding/src/chord.rs` | 768 | 548 |
| `crates/woodshedding/src/exercise.rs` | 648 | 487 |
| `crates/woodshedding/src/fretboard.rs` | 948 | 482 |
| `crates/woodshedding/src/notation.rs` | 874 | 401 |
| `crates/woodshedding/src/progression.rs` | 615 | 429 |
| `crates/woodshedding/src/rehearsal.rs` | 1075 | 497 |
| `crates/woodshedding/src/scale.rs` | 896 | 131 |

All new modules fit 600 physical lines. Twelve exact exceptions retire,
leaving 92 at this phase. Public facade paths, catalog values, arbitrary
string counts/tunings, history/session identities and ownership remain.

- Baseline: `cargo test --locked --no-fail-fast` selecting audio-primitives,
  woodshedding, hocket-model, hocket-project, hocket-recorder, hocket-views,
  woodshed-audio, woodshed-core, woodshed-graph, woodshed-instrument and
  woodshed-notation: 967 passed cases including doctests.
- Complete post suite selects the same eleven packages with
  `-- --test-threads=1`: 967 passed. Existing ignored cases remain unchanged.
- An initial parallel five-package attempt measured the existing 100-commit
  history scrub at 17.6869 ms against its unchanged <16 ms assertion.
  The isolated validation test and complete serial suite both pass. This
  records observed timing variance, not a changed threshold or broad new
  performance claim. The failed attempt is not hidden by the final pass.
- Literal review caught indentation changed inside an inline migration JSON
  fixture. Its original bytes were restored before qualification. Extraction
  now separates code dedenting from literal contents; final structural
  review compares 219 old/new function and data entities with identical
  operation/literal tokens apart from scope and rustfmt separators/blocks.
- `cargo doc --locked --no-deps` for the five changed packages: pass, with
  an existing unrelated Hocket-project documentation warning.
- `prek run --all-files`: ceiling and Clippy pass; actual commit hook repeats
  them. Source notices retained. No manifest, lock, dependency or policy change.
- Stable approved `C:/t/cargo-targets/woodshed`, two jobs. These software
  checks do not establish physical audio, device timing or nested-port
  acceptance. No isolated target, Cargo home or worktree was created.


## 2026-11-03: S2 and S3, twenty-three paths

Parent integration: `d2f990cf770a6fff2ccd321954010dd9d34c8b98`. S1's source
commit `297b162` was published through that clean merge with concurrent
Redshank note-layout work (`e729fda`). Its nested-port source, manifest and
lock changes belong to that owner; H6 does not qualify those ports.
The before column continues to use the original H6 baseline.

| Original source | Before | After |
|---|---:|---:|
| `crates/woodshed-core/src/chord_approach.rs` | 668 | 376 |
| `crates/woodshed-core/src/comparison_disclosure.rs` | 674 | 361 |
| `crates/woodshed-core/src/history.rs` | 1228 | 552 |
| `crates/woodshed-core/src/lib.rs` | 2832 | 553 |
| `crates/woodshed-core/src/loop_capture.rs` | 1182 | 499 |
| `crates/woodshed-core/src/loop_plan.rs` | 761 | 354 |
| `crates/woodshed-core/src/practice_clock.rs` | 1018 | 121 |
| `crates/woodshed-core/src/session_overview.rs` | 1787 | 584 |
| `crates/woodshed-core/src/stage_context.rs` | 850 | 578 |
| `crates/woodshed-core/src/storage.rs` | 1309 | 587 |
| `crates/woodshed-graph/src/lib.rs` | 1735 | 579 |
| `crates/woodshed-notation/src/lib.rs` | 1059 | 419 |
| `crates/hocket-recorder/src/lib.rs` | 1694 | 503 |
| `crates/hocket-views/src/leaves.rs` | 647 | 362 |
| `crates/hocket-views/src/surface.rs` | 698 | 461 |
| `crates/woodshed-audio/src/input.rs` | 1185 | 538 |
| `crates/woodshed-audio/src/offline/song_export/tests.rs` | 601 | 472 |
| `crates/woodshed-audio/src/output_runtime.rs` | 1104 | 474 |
| `crates/woodshed-audio/src/output_runtime/processor.rs` | 2238 | 354 |
| `crates/woodshed-audio/src/output_runtime/tests.rs` | 2284 | 408 |
| `crates/woodshed-audio/src/song.rs` | 936 | 599 |
| `crates/woodshed-instrument/src/midi.rs` | 907 | 533 |
| `crates/woodshed-instrument/src/midi/ble.rs` | 620 | 137 |

All originals and new modules fit 600 physical lines (whole Woodshed H6
maximum: 599). These twenty-three exact exceptions retire; the thirty-five
H6 paths leave 69 exceptions. StageState, graph and view public facades stay
at their existing paths. Runtime fields, callback ownership, completion and
generation handling, cancellation, control order, allocation assertions,
wire shapes and numerical/literal values stay intact.

- S2's initial no-run compile and the complete S2/S3 no-run compile pass.
- Full eleven-package post suite uses S1's package selection and
  `-- --test-threads=1`: 967 passed including doctests, matching baseline.
  It passes again after the test-feature repair below.
- `cargo check -p woodshed-notation --no-default-features --all-targets --locked`
  and the corresponding woodshed-audio command pass. The normal test graph
  includes Windows BLE; the audio check covers wired MIDI without BLE.
- Instrument's first no-default all-targets check fails in its existing
  diagnostics integration test: it imports `midi` while that module is off.
  The unchanged pre-H6 target has no feature guard. Adding the library's same
  `#![cfg(feature = "midi")]` to this test fixes the mismatch; the identical
  check then passes and the normal 967-case suite remains intact. This is a
  test-target repair, separate from the twenty-three counted source moves.
- `cargo doc --locked --no-deps` selecting hocket-recorder, hocket-views,
  woodshed-core, woodshed-graph, woodshed-notation, woodshed-audio and
  woodshed-instrument passes. Existing unrelated documentation warnings
  remain; no broad warning cleanup or suppression is introduced.
- `prek run --all-files` passes before and after that guard. The actual
  commit hook repeats ceiling and Clippy checks against the final snapshot.
- Move audit accounts for 659 old/new function/data entities. Operation and
  literal tokens match apart from module scopes and two rustfmt-only bodies.
  Function signatures and 26 type layouts match after scope/format changes;
  all 35 moved public facade items have their original root re-export.
- Shared Cargo-cache lock contention interrupted the earlier S2 attempts;
  they are not counted as passed gates. Final evidence is the completed
  combined run and the explicit feature-repair rerun.
- Stable approved target, two jobs, locked cached dependencies. Root
  manifests and lockfile are unchanged. No physical audio, BLE-device,
  headed or other-platform qualification is renewed. No isolated target,
  Cargo home or worktree was created.
