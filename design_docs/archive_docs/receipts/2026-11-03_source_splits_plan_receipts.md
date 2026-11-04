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

## 2026-11-04: H8 untouched full-workspace baseline

Baseline `cf2b09196adce374b65d401dfa7a2d0461e94b9c`. `cargo test --workspace --locked --no-fail-fast -- --test-threads=1`: 1705 passed, one failed, eight ignored. The existing overview case `stage::overview::tests::navigation_and_graph_inspection_do_not_start_or_pause_live_processes` fails at overview.rs:1690 with actual 2 versus expected 3. It is outside the selected originals. No selected Rust source changed during this run; the failure is compared after refactoring without relaxing its assertion. This broader root-workspace inventory includes the native/view packages that H6 did not qualify.

## 2026-11-04: H8 thirty-two originals qualified

| Original | Before | After |
|---|---:|---:|
| `crates/woodshed-genet/src/audio/input_admission/tests.rs` | 710 | 530 |
| `crates/woodshed-genet/src/audio/loop_projection.rs` | 921 | 323 |
| `crates/woodshed-genet/src/audio/practice_clock.rs` | 980 | 286 |
| `crates/woodshed-genet/src/drive.rs` | 744 | 371 |
| `crates/woodshed-genet/src/leaves.rs` | 749 | 598 |
| `crates/woodshed-genet/src/main.rs` | 1145 | 521 |
| `crates/woodshed-genet/src/midi.rs` | 634 | 560 |
| `crates/woodshed-genet/src/persona.rs` | 1093 | 449 |
| `crates/woodshed-genet/src/persona_preservation_tests.rs` | 904 | 571 |
| `crates/woodshed-genet/src/scenario/mixed_looper.rs` | 630 | 522 |
| `crates/woodshed-genet/src/scenario/physical_audio.rs` | 619 | 385 |
| `crates/woodshed-genet/src/scenario/practice_clock.rs` | 732 | 484 |
| `crates/woodshed-genet/src/session.rs` | 1667 | 212 |
| `crates/woodshed-genet/src/storage.rs` | 1497 | 514 |
| `crates/woodshed-genet/src/sync.rs` | 1562 | 411 |
| `crates/woodshed-genet/src/wav_export.rs` | 1233 | 499 |
| `crates/woodshed-views/src/appearance.rs` | 645 | 447 |
| `crates/woodshed-views/src/fretboard_leaf.rs` | 653 | 403 |
| `crates/woodshed-views/src/stage/canvas_camera.rs` | 652 | 412 |
| `crates/woodshed-views/src/stage/context.rs` | 908 | 467 |
| `crates/woodshed-views/src/stage/contextual_navigation.rs` | 998 | 567 |
| `crates/woodshed-views/src/stage/looper.rs` | 693 | 524 |
| `crates/woodshed-views/src/stage/looper/recorder.rs` | 1120 | 471 |
| `crates/woodshed-views/src/stage/music_editor.rs` | 1710 | 414 |
| `crates/woodshed-views/src/stage/musical_context.rs` | 753 | 302 |
| `crates/woodshed-views/src/stage/practice_clock.rs` | 770 | 516 |
| `crates/woodshed-views/src/stage/rehearsal.rs` | 639 | 474 |
| `crates/woodshed-views/src/stage/related.rs` | 1532 | 507 |
| `crates/woodshed-views/src/stage/relationship.rs` | 1372 | 467 |
| `crates/woodshed-views/src/stage/settings.rs` | 1199 | 474 |
| `crates/woodshed-views/src/theme.rs` | 755 | 246 |
| `crates/woodshed-views/src/workspace.rs` | 810 | 425 |

All 65 extracted Rust files fit, with a maximum across original/new files of
598 lines (`crates/woodshed-genet/src/leaves.rs`). Exactly 32 exceptions
retire; 37 remain outside this pass. Root workspace only; Hocket, Redshank
and Ringdown remain independent and unselected.

`cargo test --workspace --locked --no-fail-fast -- --test-threads=1`:
post 1705 passed, one failed, eight ignored, matching the untouched
baseline and complete emitted test-name/outcome inventory after fixture
regrouping. The sole failure is the unchanged overview navigation/process
case, still actual 2 versus expected 3. It is carried forward in the
[Stage plan](../../2026-11-03_stage_set_tools_plan.md#overview-process-count-assertion-2026-11-04).
No assertion, ignore or production behavior is relaxed to turn this broader
suite green. All moved-surface cases keep their prior outcomes.

`cargo doc -p woodshed-views -p woodshed-genet --locked --no-deps` and
`prek run --all-files` pass. After the full test run, unused private root
aliases were removed or gated by test; the final all-target Clippy gate
recompiles those production and test imports. Existing unrelated warnings
remain, including unresolved rustdoc names. The actual commit repeats
configured ceiling/Clippy hooks against the final snapshot.

The staged audit matches 506 old/new function/constant bodies, all
signatures and type fields; six differences are Rustfmt-only brace/semicolon
changes. Literal/fixture bytes and generic bounds remain matched. Session
subprocess workers stay in their original lexical test module through three
cohesive include files, retaining every original `--exact` entry name.
Public facade paths, cfg variants, DTO layouts, PCM/frame values, callback
resources, source ownership and persistence/publication boundaries remain.

No Cargo manifest/lock, CI, tool release, hook configuration, CLAUDE or
doctrine edit. Stable approved target, two jobs; no isolated build area,
Cargo home or worktree. No new native-headed, physical audio/MIDI, macOS,
Linux or other-platform qualification. Compact owner/aggregate receipts
remain within H8's shared 100 MB cap. The aggregate records the immutable
published source commit.
