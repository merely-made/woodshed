# Project-worker request and session correlation

Status (2026-10-16): implemented and CPU/build qualified.

## Feature target and done conditions

Adopt Armillary request IDs and correlated replies for Save, Open, ExportMix,
WriteHandoff and ReadHandoff. A host-owned scope combines a replacement generation
with the session identity. Every terminal success and failure returns that scope
and request ID. Busy state comes from an admitted pending request, independently
of status text. Matching replies alone may change project association, saved head,
staged incoming handoff or status. Replacement/identity retirement invalidates old
work; a stale reply must not release newer work.

Prove worker round trips, failures, duplicate/unsolicited replies, same-session
replacement, old failure after new admission, handoff busy state, queue failure
and edits during save using device-free tests. Build/check the independent Hocket
host with locked dependencies. Native project/handoff acceptance remains separate;
this slice does not add signed turn operations or change file/envelope formats.

## Findings

2026-10-16: `project_io.rs` sends bare commands and updates. `state.rs` accepts
all updates unconditionally and infers busy state from Saving/Loading/Exporting
labels. WriteHandoff/ReadHandoff send no active status. The existing project-controls
plan explicitly calls for RequestIds/Correlated adoption on the next worker touch.
Recorder replacement is centralized in three host paths; identity switching must
also invalidate work addressed to the former persona.

## Progress

2026-10-16: source verified at Woodshed main ae2f34e. Reusing the clean isolated
worktree; no family pins, lockfiles, shared recorder model or DSP changes planned.

2026-10-16: all five command routes now pass through one pending-request
admission helper. The worker returns the Armillary request ID and host session
scope unchanged on every success/failure. Project replacement (including
same-session branch acceptance) and successful persona switching retire pending
work and any previously staged incoming handoff. The host drops unsolicited,
duplicate, foreign-scope and superseded replies before changing visible state.

The host preserves edits made while a save snapshot is in flight: a matching
completion acknowledges the submitted history head, so later edits remain dirty.
A stale reply cannot clear a newer pending request. Already-admitted file I/O can
still finish on the worker after retirement; correlation governs host admission,
not cancellation or rollback of file writes. No file/envelope bytes change.

Regression coverage includes a real actor/host Save/Open/WriteHandoff/ReadHandoff
round trip, all five actual worker failure routes with exact request/scope
checks, and delayed/duplicate/unsolicited replies, same-session acceptance,
wrong scope, staged incoming retirement, edit-during-save and disconnected-worker
admission. Final independent-workspace checks pass: 116 distinct cases (75 host, 36
engine unit, three engine integration, two manual-demo CPU), with one existing
isolated-Keeper test ignored. The 11 new regressions consist of ten host
correlation tests and one all-command worker-failure test. Locked offline
workspace all-target checks and the desktop build pass. No manifests, family
pins, lockfiles or persisted formats changed. Evidence:
`../../../validation/hocket-project-correlation_20261016/receipt.json`.

2026-10-16 native follow-through: bounded native Save/Open/WAV export and signed
self-addressed handoff review/discard/accept/adopt now pass, including exact fresh-
process project/history/media reopening. See
[the native acceptance plan](2026-10-16_native_project_handoff_plan.md) and its
receipt for the two-process/seven-capture boundary. This does not qualify
two-peer transport, signed per-turn operations or physical audio.
