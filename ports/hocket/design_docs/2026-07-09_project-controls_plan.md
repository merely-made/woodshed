# Project Controls Plan

## Goal

Turn the local project store into a usable desktop workflow without blocking the
Genet kernel or the audio engine.

## Design

- `rfd` provides native open/save dialogs until Genet exposes a shared desktop
  dialog API.
- An Armillary actor owns Redb project I/O. The host sends cloned save snapshots
  or open paths and drains typed updates on its own event loop.
- The live `AppState` keeps engine authority. An open result replaces model and
  media state only after the worker returns.
- The header shows the actual project file stem, save/open status, and dirty
  state. Saving and opening are rejected during recording or another project
  operation.

## Done Conditions

- Open and Save are visible, accessible controls.
- New projects prompt for a `.hocket` path; later saves reuse that path.
- Redb I/O never runs on the Genet kernel thread.
- A worker result updates the view and preserves missing-media behavior.
- The worker has a Redb save/open round-trip test.

## Progress

- 2026-07-09: **LANDED.** Native Open and Save controls use `rfd`; an
  Armillary worker round-trips a Redb project without blocking the host kernel.
  The header reflects the project path, save state, and unsaved history edits.
  Native-dialog visual smoke testing remains manual.
- 2026-07-11: Armillary now exposes `RequestId`/`RequestIds` and
  `Correlated<T>`, proven by Isometry's authority actor. The next project-worker
  touch should stamp Save/Open/Export commands and return that ID on every
  result, replacing the current action-string-only failure correlation. This is
  an adoption note, not a claim that Hocket has migrated yet.

- 2026-10-16: the Armillary adoption above is now implemented for Save/Open/
  Export and both handoff operations. Results carry request ID and host session
  scope, and pending work is independent of display status. See
  [the correlation plan](2026-10-16_project_worker_correlation_plan.md) for the
  host admission rules and separate native acceptance boundary.

2026-10-16 native follow-through: bounded native Save/Open/WAV export and signed
self-addressed handoff review/discard/accept/adopt now pass, including exact fresh-
process project/history/media reopening. See
[the native acceptance plan](2026-10-16_native_project_handoff_plan.md) and its
receipt for the two-process/seven-capture boundary. This does not qualify
two-peer transport, signed per-turn operations or physical audio.
