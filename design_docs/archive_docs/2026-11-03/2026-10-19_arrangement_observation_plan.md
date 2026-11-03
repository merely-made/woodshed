# Arrangement authoring observations

**Date:** 2026-10-19. **Status:** implemented and qualified; this commit records the completed consolidation.

## Scope and ownership

Consolidate the existing arrangement editor's captured state checks while
preserving its typed authoring model, explicit Save, preview/cancel behavior,
per-working-Set ownership, input handling, validated scene intents and sealed
session persistence. The user authorized implementation and subagent review.
The subsequent user request authorizes publication of this consolidation.
Dependency changes remain outside the slice.

## Phases and done-conditions

- A1: replace JSON arrangement baselines and debug-string draft fingerprints
  with exact typed snapshots. Dirty detection includes incomplete input and
  structure; selection/focus alone remains neutral.
- A2: use one owner/Set/draft observation for gesture validation and preview
  reuse. Preserve full-Set coverage, viewport/workspace/persona guards, scene
  intent validation, explicit save and cancellation. Theme/proposed-span changes
  still invalidate presentation. Equal-value restoration retains the existing
  value-based semantics; this is not a new monotonic revision owner.
- A3: independently review the diff and run focused arrangement, host input,
  preservation and fresh-process persistence checks. Record actual process
  results and source hashes in testing. Native acceptance requires a separate
  observed run and is not inferred from existing receipts.

## Findings

2026-10-19: `stage/arrangement_editor.rs` compared the saved typed arrangement
through JSON, assembled a debug-string draft signature for dirty detection,
recaptured draft/whole-Set strings for gestures and combined them again for the
preview cache. `SavedProgressionArrangement` already supports exact equality.
The editor authors fields and typed span intents; routing them through UTF-8
byte edits would add an unnecessary representation. No shared supplier change
is justified by this consolidation. Relationship recipe authoring already uses
the shared typed recipe draft and remains outside this slice.

## Progress

- 2026-10-19: refreshed upstream, preserved the clean checkout, read the owning
  policy and arrangement paths; delegated independent guard review and
  qualification planning. Implemented typed draft baselines/snapshots and one
  captured arrangement observation. Focused checks and review are in progress.

Evidence home: `/Users/markik/Code/testing/woodshed-arrangement-guards-20261019/`.

- 2026-10-19: A1/A2 implemented. The exact typed saved baseline replaces JSON
  arrangement conversion, exact draft snapshots replace Debug fingerprints,
  and gestures/preview reuse share one owner/encoded-Set/draft observation per
  sync. Presentation ownership remains separate so Save/Reload preserves a valid
  selection. The existing value-based restoration contract is explicit.
- 2026-10-19: independent review found a selection-preservation regression in
  the first cache refactor. Fixed it and independently rechecked the source.
  Ten focused regressions pass, including distinct Card-4 voicing after Save
  and Reload, removed selection, different owner, incomplete text, structure,
  no-op mutable access and observed/unobserved intermediate changes. Full views
  suite passes 254; final host arrangement suite passes three; persistence/
  export suite passes five, including sealed reload and fresh CLI processes.
  Scoped library Clippy, formatting and diff checks also pass. Eight existing
  library warnings remain outside the changed arrangement files. A3 is complete
  within source/model, mounted host and persistence/CLI qualification; no fresh
  native GUI run is claimed.

## Design outcome

This is local state-check consolidation, not a third `mere-source-edit` text
consumer. The shared supplier stays unchanged: Woodshed's typed span/recipe
contracts already serve its domain edits. Dirty checks no longer serialize
arrangements or build formatted fingerprints. Full-Set capture deliberately
retains exact encoded-byte comparison. Snapshot capture clones structured state;
no measured CPU, allocation or retained-memory improvement is claimed. The
product implementation grows by 89 lines under the receipt's counting method;
clearer state ownership and fewer repeated captures are the demonstrated gains.

- 2026-10-19: publication is isolated from four unrelated local recorder commits.
  Copied only the five qualified paths onto fetched `origin/main` `8da953b`,
  repeated the views, mounted-host and persistence checks there, and preserved
  the original recorder history. Publication commands/results are recorded in
  `publication-checks.json` beside the original qualification evidence.
