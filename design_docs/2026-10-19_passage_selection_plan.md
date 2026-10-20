# Relationship passage selection construction

**Date:** 2026-10-19. **Status:** implemented and qualified locally; this commit records the completed consolidation. Publication is separate.

## Scope and ownership

Resolve one operation-local selected-material observation and reuse it when
constructing adjacent-pair relationships. Preserve the public disclosure and
projection APIs, exact JSON outputs, malformed-input failure precedence,
owner-qualified identities, and feature-disabled core compilation. This pass
changes only comparison construction and focused tests. Views, retained-reading
storage, source-return custody, shared dependencies and persisted formats stay
with their existing owners. Implementation and subagent orchestration are
user-authorized; publication is separate.

## Phases and done-conditions

- P0: freeze unchanged outputs and refusals before modifying core code. Use
  real catalog fixtures with 2, 3 and 64 selected Cards, repeated materials,
  sparse/reversed selections, rich and legacy pair policies, malformed tail
  material and multi-fault error precedence. Compare public projection wrappers
  as well as normal disclosed values. Control source and runners stay in testing.
- P1: validate and resolve the complete selection once, construct occurrence
  JSON once, then emit relationships for adjacent selected pairs. Preserve
  public wrappers and all selected-identity/material checks before publication.
  Keep core free of a new optional scene-schema dependency or retained cache.
- P2: require exact differential JSON and refusal equivalence, independent source
  review and meaningful core regressions. Prove one selection resolution, one
  occurrence construction and n−1 pair emissions with a narrow test-only seam.
  Run receiving relationship/view checks, core tests and feature-disabled check.
  Source-count reductions alone do not establish CPU or memory speedups.

## Findings

2026-10-19: `comparison_disclosure.rs:219` seeds a passage dataset from a full
selection disclosure, then repeats full disclosure and dataset construction for
each adjacent pair. A valid n-Card passage therefore resolves the full selection n times and builds
n complete occurrence datasets. Public pair wrappers accept a supplied
`SelectedMaterial`, so preserving caller-provided relationship metadata and
original evidence ordering is part of the compatibility baseline too.

The pattern may apply to other application operations, but different domain
validation and authority rules remain deliberate. This slice demonstrates the
mechanism locally before considering another consumer or shared API.

## Progress

- 2026-10-19: fetched and verified the clean checkout. Existing four recorder
  commits and local integration merge remain untouched. Frozen source is in
  `Code/testing/woodshed-passage-selection-20261019/baseline-source/`. Delegated
  external differential baseline capture and disjoint contract-test coverage;
  parent owns the construction refactor. No core source edits precede P0.

- 2026-10-19: P0 froze 38 exact control cases before the core edit, including
  caller-edited overlap metadata and malformed multi-fault cases. Both control
  and candidate use the locked Cargo-built dependencies and Rust 1.99.1. The
  external runner records compiler working directory to distinguish the parent
  directory's older default toolchain.
- 2026-10-19: P1 implemented a private resolved selection and separate dataset,
  overlap and rich-pair construction. Public wrappers retain their established
  metadata and error contracts. Passage validates all selected material once,
  builds occurrence JSON once and produces n−1 adjacent-pair outputs.
- 2026-10-19: exact control/golden comparison passes all 38 cases. All 21 focused
  comparison tests pass (13 existing, seven public-boundary regressions and one
  width-sweep construction-count test). The latter asserts [1 selection capture,
  1 dataset build, n−1 pair outputs] at widths 2, 3 and 64. Full core passes 306
  tests; receiving views pass 254. Feature-disabled core compilation, scoped
  library Clippy, formatting and diff checks pass. Independent review found no
  actionable regression in the inspected diff. P2 is complete within this
  bounded source/model and receiving-library qualification.

## Results and boundaries

For a valid 64-Card passage, complete selection resolution and occurrence-dataset
construction each fall from 64 calls to one; rich pair projections run 63 times.
Test-only per-thread counters cover widths 2, 3 and 64 and are absent from normal
builds. The pair counter measures rich-projection calls; source review separately
confirms one pitch comparison per adjacent pair. No timing, allocation or memory
speedup is inferred from those counts.

All existing APIs, output fields and failure precedence remain equivalent in
38 frozen control cases. The core's public wrappers still preserve supplied
relationship metadata and authored evidence order. Product code grows by 75
lines under the receipt's counting method; the gain is removal of repeated
whole-selection/dataset construction, not a total line-count reduction.

The unchanged control, exact golden, runners, source/log hashes, independent
review and qualification receipt are in
`Code/testing/woodshed-passage-selection-20261019/`. No source-return/UI routing,
retained byte storage, dependency, format or shared supplier change is included.
The broad reusable mechanism is operation-local resolution; domain checks and
operation boundaries remain explicit. This pass does not add a retained cache
or claim new native GUI acceptance.

- 2026-10-19: user authorized the scoped local commit. The six qualified paths
  are committed together; publication remains separate. Runtime sources match
  the qualification receipt; only commit-status wording changed afterward.
