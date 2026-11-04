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
