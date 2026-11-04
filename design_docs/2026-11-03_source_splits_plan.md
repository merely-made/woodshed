# Source splits: H6 Woodshed batch

**Status (2026-11-03):** landed; 35/35 paths qualified.

This is the second fifty-file pass under Mere's house-style plan, H6.
The eight earlier splits and all fifty H5 paths are excluded. Mark's
"Another batch?" continues the same qualification rule: an original counts
only after it and all new modules fit 600 physical lines, its exact ceiling
exception is retired, appropriate tests and local gates pass, and the change
is committed and pushed. Ordinary private implementation and fixture moves
need no new design ruling. Public API paths, owner authority, source data,
file formats, arithmetic, test assertions and feature boundaries stay intact.

The retained receipt cap is 100 MB across both repositories and Mere's
aggregate. Only compact summaries are retained; failed baseline findings are
recorded explicitly. Frozen receipts, imported patches and captured fixtures
remain unchanged. This pass supplies software/compiler evidence and does not
renew native-device, headed, radio, audio-quality or other-platform claims.

## Phases and done-conditions

- **S1. Portable theory and capture history (12 paths).** Separate existing
  theory catalog/query responsibilities and DSP/history/handoff regression
  groups. Keep arbitrary tunings and string counts, numerical operation
  order, frame units, session identity and immutable media semantics.
- **S2. Application models and projections (12 paths).** Separate StageState
  operations, history/storage fixtures, loop/practice/overview projections,
  graph adapters and notation helpers. Public facade types and functions
  retain their source paths. Product state stays in Woodshed; shared model
  helpers must not become an independent authority or a second transport.
- **S3. Runtime seams and recorder views (11 paths).** Separate existing
  input and output preparation, commands, processing and controlled fixtures;
  retain callback allocation rules, completion stamps, session/generation
  correlation, cancellation and native resource ownership. MIDI remains the
  single maintained adapter. Keep hocket-recorder device-free on construction.

Each phase is complete when its original files and children fit the ceiling,
its exceptions are removed, relevant baseline/post package tests agree,
feature and public docs checks cover the changed surfaces, and the full local
gate and actual commit hook pass. Publish each phase before counting it.
Root workspace checks do not qualify the independently released nested ports.
Nested port source, locks, products and validation remain under their owners.

## Findings

- 2026-11-03: the pre-existing diagnostics integration target imported the MIDI-only module even with MIDI disabled. Its crate now has the same `cfg(feature = "midi")` guard; the normal feature graph still runs all 967 cases.

- 2026-11-03: inline module extraction must preserve multiline/raw literal bytes independently of code indentation. The rehearsal migration JSON is a concrete witness; literal-token review catches value changes that parser-level behavior tests can miss.

- 2026-11-03: the clean checkout lists 104 exceptions. Thirty-five root
  workspace paths have separate existing implementation or regression groups.
  The long tuning catalog and monolithic stage-scene function are outside
  this pass. Shared theory and DSP stay free of I/O and device/UI dependencies.
- 2026-11-03: full source moves preserve existing module ownership. Child
  impls expose parent-private helpers only to their former owner; restricted
  visibility must account for the additional module level. Fixture include
  paths must still resolve to the original captured bytes.
- Evidence appends to
  [the receipt](archive_docs/receipts/2026-11-03_source_splits_plan_receipts.md).
  Mere's house-style receipt aggregates the two repositories' qualified paths.

## Selected original paths

- `crates/audio-primitives/src/alignment.rs`
- `crates/audio-primitives/src/onset.rs`
- `crates/hocket-model/src/history.rs`
- `crates/hocket-project/src/handoff.rs`
- `crates/hocket-recorder/src/host_policy.rs`
- `crates/hocket-recorder/src/lib.rs`
- `crates/hocket-views/src/leaves.rs`
- `crates/hocket-views/src/surface.rs`
- `crates/woodshed-audio/src/input.rs`
- `crates/woodshed-audio/src/offline/song_export/tests.rs`
- `crates/woodshed-audio/src/output_runtime.rs`
- `crates/woodshed-audio/src/output_runtime/processor.rs`
- `crates/woodshed-audio/src/output_runtime/tests.rs`
- `crates/woodshed-audio/src/song.rs`
- `crates/woodshed-core/src/chord_approach.rs`
- `crates/woodshed-core/src/comparison_disclosure.rs`
- `crates/woodshed-core/src/history.rs`
- `crates/woodshed-core/src/lib.rs`
- `crates/woodshed-core/src/loop_capture.rs`
- `crates/woodshed-core/src/loop_plan.rs`
- `crates/woodshed-core/src/practice_clock.rs`
- `crates/woodshed-core/src/session_overview.rs`
- `crates/woodshed-core/src/stage_context.rs`
- `crates/woodshed-core/src/storage.rs`
- `crates/woodshed-graph/src/lib.rs`
- `crates/woodshed-instrument/src/midi.rs`
- `crates/woodshed-instrument/src/midi/ble.rs`
- `crates/woodshed-notation/src/lib.rs`
- `crates/woodshedding/src/chord.rs`
- `crates/woodshedding/src/exercise.rs`
- `crates/woodshedding/src/fretboard.rs`
- `crates/woodshedding/src/notation.rs`
- `crates/woodshedding/src/progression.rs`
- `crates/woodshedding/src/rehearsal.rs`
- `crates/woodshedding/src/scale.rs`

## Progress

- 2026-11-03: selected thirty-five paths from clean main. Baseline package
  tests start before source edits; receipt cap is shared across H6.

- 2026-11-03: S1 landed: twelve originals/children fit, twelve exceptions retire, all 967 baseline/post software cases pass (post serial), public docs and full local/commit gates pass. The initial parallel timing miss and literal restoration are recorded in the receipt.

- 2026-11-03: S2 and S3 land together: twenty-three originals/children fit and their exact exceptions retire. The shared Cargo cache was busy during the first S2 attempts, so one complete final validation covers both phases. All 967 post cases, three no-default feature checks, public docs and full local/commit gates pass. Source tokens, signatures, type layouts and facade exports are reviewed.
