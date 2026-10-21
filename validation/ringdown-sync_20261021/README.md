# Shared packs and Ringdown integration — 2026-10-21

The maintainer requested push/pull/merge during the Windows/Linux diagnostic
acceptance work. This packet records reconciliation with fetched Woodshed main
`196d50ecdd1c5664ed2fbbf21cdd6c24260a7362` in an isolated checkout.

## Preserved inputs

The original primary checkout was at `0452bcea1ff227924631ac190744e2bd9282fba5`,
with six local commits and 75 remote commits. Snapshot commit
`9207a03` preserves its 1,011 changed/untracked paths before reconciliation.
Primary status and all 1,011 path hashes were checked unchanged. No stash, reset,
force push or primary source overwrite was used.

## Integration

- Retain current upstream notation/editor, source-admission and audio fixes.
- Integrate shared pattern/pack crates, Hocket project retention, MIDI export,
  standards/private-adapter boundaries, and desktop pedal/diagnostic controls.
- Keep one pedal/clock output worker. Fold upstream frame completion observation,
  stale-worker acceptance clearing and requested-route deduplication into that
  owner. Remove the superseded standalone MIDI output loop. Port its host
  regressions to the retained writer; existing cancellation/selection/partial
  send cases continue through the same worker.
- Rebase the existing synthesized-key press admission patch onto Mere
  `7de937d0e708db24d9a325cbff3fbe5beb3a4d3c`, matching the current supplier's
  Cambium/Sprigging and AccessKit versions. The vendored host remains the single
  resolved host. Its upstream source hashes and narrow patch are recorded there.
- Preserve native chooser ownership; the existing rfd dependency now covers
  Windows and Linux, with its GTK backend on Linux.
- Root dependency resolution retains the upstream Git source identities. Hocket
  adds only the three local pack crates and midly to its lockfile. Ringdown's
  locked dependency resolution succeeds.

## Checks and limits

The merged Woodshed desktop and Hocket desktop compile with Rust 1.99.1.
The focused Woodshed desktop MIDI/diagnostic lane passes 43 cases, including
real retained controls, repeat suppression, route/status projection, single
writer cancellation/partial outcomes, worker exit and export close guards.
These are software checks. No guitar, native Windows/Linux dialog cohort, or
new hardware delivery is qualified by this integration. Historical native and
guitar packets retain their original source/binary hashes. The separate
Windows/Linux diagnostic acceptance remains pending SSH custody access.

Initial integration failures are retained: the old host snapshot's dependency
version was incompatible with current Cambium; duplicate historical notation
exports, an orphan doc comment and the moved routing helper needed reconciliation.
Successful logs are `integration-check-v4.log`, `hocket-check.log` and
`midi-final.log`. Dependency metadata checks do not establish runtime
behavior. Raw historical logs/vendor files retain their original whitespace;
`merge-source-whitespace.log` checks the reconciled desktop source only.

The clean integration worktree is retained at
`/Users/markik/Code/worktrees/woodshed-ringdown-sync` for continued acceptance.
The original dirty checkout remains recoverable separately. Shared build targets
are reused; no isolated Cargo home was created. Publication details are recorded
by the follow-up Git verification.
