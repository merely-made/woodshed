# Honest Local Session Plan

## Goal

Make the first-run session truthful and useful before adding persistence or
network collaboration. The app must show only work it can actually perform and
every session-structure gesture must remain visible to history.

## Scope

1. Start from an empty looper-pedal session rather than seeded silent media.
2. Add tracks through a model `Edit` so undo/redo and future sync can see the
   operation.
3. Make solo and stop affect the projected engine state.
4. Remove fake peers, hand-off, and inert history controls from the UI.
5. Replace stale Masonry-era repository documentation.

## Done Conditions

- Startup presents only empty, recordable tracks.
- `+ add track` appends a history-backed track that inherits the session mode.
- Solo mutes non-solo tracks in the engine projection; stop ends live voices.
- The UI does not advertise a hand-off, fake collaborator activity, or an
  action with no behavior.
- README and repository guidance describe the Genet host and current gaps.

## Progress

- 2026-07-09: Landed the scope above. `cargo test -p hocket-model` passed
  with 32 unit tests and 2 integration tests; `cargo check -p hocket-genet`
  passed. The repository-wide format check still proposes unrelated preexisting
  formatting changes outside this slice.

## Follow-On

The local-project bundle work moved to
[2026-07-09_muniment-project-store_plan.md](2026-07-09_muniment-project-store_plan.md).
It uses Muniment's backend seam rather than inventing a filesystem protocol in
Hocket.
