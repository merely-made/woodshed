# Woodshed session Mere folds — 2026-10-16

This receipt records acceptance checks for explicit selection folds in Woodshed's session Mere
view. Select members through the graph or its source roster, choose **Fold
selected**, activate the visible `+N` member to expand, or use **Expand all
folds**. Undo/redo is bounded to the current process. A fresh process restores
fold records through the ordinary sealed session path.

The checkout was fetched before work and advanced seven commits from
`d12201b` to `e6f0950`. Implementation uses a separate worktree. The existing
Mere supplier at `657507505` already contains native v2 `FoldRecord`s. No
package version, source revision or nested-port lockfile changes: the root
lock adds only direct curation, hashing and serialization dependency edges.

Woodshed owns typed source IDs and curation commands. Its original Swatch lays
out and reconciles motion over all source members before fold projection.
A member stand-in retains its original position. The complete active fold set
maps each boundary endpoint once; internal cells disappear from presentation,
and directed family bundles carry source-cell counts. Their targets do not
reuse one source relation's evidence. Source graphs, catalog facts, ordered
Cards, timing, backing, practice lineage and live activity remain their owners'
authority.

Version 3 of the existing overview-presentation payload retains fold records.
Versions 1/2 still load. Unsupported nested fold schemas remain opaque, and
new folds are refused when their presentation could not be retained. Missing
members and overlapping folds cannot partially hide the source graph.

## Reproduction

Build the normal `woodshed-genet` binary with
`cargo build --locked -p woodshed-genet`. Run [`mere_folds.scn`](../../scenarios/mere_folds.scn), then
[`mere_folds_reopen.scn`](../../scenarios/mere_folds_reopen.scn), through Mere's
`scripts/run_macos_scenario.py`. Each run uses a new capture directory; both
use the same isolated `WOODSHED_STATE`, `WOODSHED_SETTINGS`, custody endpoint,
and binary. Set `WOODSHED_PRESENTATION_TIMEOUT_SECS=60`, width 1180 and height
900. The scenarios select UI zoom 0.8. Keep native processes serialized and
bounded; never omit the state override.

The acceptance run uses a fresh generated Djinn Keeper fixture, without a
wallet, and isolated application-data/config/cache paths. The helper and both
native processes must exit. Fixture identities, sealed profile blobs, native
binaries and build caches are excluded from this committed packet.

Read-only `fold-checkpoint-*` steps verify that live owner payloads and folds
match the normal sealed store. They record the complete source reading and
all source-member positions, alongside the projected boundary counts. The
external verification recomputes directed bundle counts from those source
cells and compares fingerprints across seed and reopen.

This gate covers the macOS session Mere graph. Related/catalog and relationship
editor graphs remain separate consumers. Descendant selection needs an explicit
source hierarchy. Physical audio, recovery, live screen-reader interaction,
global fold shortcuts and Windows/Linux headed behavior are outside this receipt.


## Final result

Final source `55d3615` includes native-arrangement main `43e40bb`, fetched and
merged after the initial seven-commit refresh. Its locked suite passes **671
cases**, zero failures, seven existing ignores, plus five fresh-process child
invocations. Core storage/export checks, views, native host and the new readback
regression are included. The normal locked desktop build passes.

| Native phase | Presented frames | Captures | Result |
| --- | ---: | ---: | --- |
| Two selection folds, undo/redo and confirmed retention | 68 | 5 | pass |
| Sealed fresh-process restore, summary activation, undo/redo, expand all | 47 | 3 | pass |

All eight 2360-by-1800 captures are nonblank and visually reviewed. At six
checkpoints the source reading, complete owner payload and all source-member
positions retain equal fingerprints. Two saved records reopen exactly. The
source remains eleven nodes and nine relations; two folds show nine nodes and
bundle eight boundary cells into five directed family routes. The final
checkpoint confirms all folds expanded and saved, with rehearsal and playback
still stopped.

Stemma's `owned_visits` is a HashSet and per-visit `bindings` a HashMap. Only
those unordered memberships are sorted for comparison; all event values,
references, timestamps, Card order and payload fields remain compared. The
regression rejects altered events, dates, membership and ordered Cards. No
product history serializer was changed. Earlier runs that compared accidental
set order are diagnostics, and a zero-presentation attempt is excluded.
The pre-merge two-process proof was repeated on the final combined source.

[Receipt](receipt.json), [source inputs](inputs.json), [verification](verification.json),
[CPU result](cpu-result.json), [seed launch](seed-config.json) and
[reopen launch](reopen-config.json) bind the commands and evidence.
`verify.py` recomputes source-commit hashes, bundle counts, checkpoint equality
and image integrity with Python/Pillow. `run_native.py` is the exact local run
driver, with qualification-host paths. Raw logs are copied byte-for-byte;
end-of-file blank lines are retained. Native binaries, sealed profiles,
identity material and caches remain outside the packet.
