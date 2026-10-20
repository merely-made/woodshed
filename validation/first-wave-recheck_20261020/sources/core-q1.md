# Woodshed A4 and Q1 current-source recheck

Date: 2026-10-20. Fetched baseline: `9f80bb5eabc4b513e057b22af6f80d83ecb8e9ac`.
The coordinator owns these seams and the integration receipt. P5a remains with
its existing owner.

Woodshed history decode recognizes any current snapshot key before attempting
the explicit legacy `events` shape (`crates/woodshed-core/src/history.rs:540–575`).
The guard at lines 403–530 checks reference bounds before supplier indexing,
unique entry/owner identities, parent/child and forward-binding consistency,
acyclic visit/owner ancestry, owner cursors/owned visits and entry visit counts.
Malformed current history therefore cannot fall back to an empty legacy value.
Current, legacy and retained-branch fixtures remain covered by the core gate.

The exact locked supplier is now Eidetic `738ef15956a29a125820b2d6c2280d39a6214f8e`.
Its `crates/chartulary/src/stemma/snapshot.rs:17–108` still offers trusted,
infallible `from_snapshot` and directly indexes serialized references. The
consumer guard remains necessary; its stale Mere-revision comment is corrected.
Moving the invariant into a fallible supplier API and adopting it remains a
concrete supplier follow-up. No supplier source, pin or lockfile changes here.

`decode_session` (`crates/woodshed-core/src/storage.rs:524–547`) admits active,
inactive and retained Sets through `Set::admit_card_ids`. The model preflights
duplicates and mint exhaustion before mutation, preserving legitimate zero-ID
migration (`crates/woodshedding/src/rehearsal.rs:533–567`). Existing regressions
cover every loaded bank, failed admission without mutation, zero migration and
near-exhaustion. Session-store and host tests cover quarantine and close refusal
without silently overwriting the original input.

Q1 process admission still flows from both CI/font entrypoints through
`validation/evidence_process.py`. It binds actual exit, run, command, timestamps
and exact captured log digest, then rejects failed or unclosed Cargo suites.
Fourteen controls exercise later suite failure, wrapper failure, malformed and
incomplete evidence, capture CLI failure/timeout/overwrite refusal, nested-child
accounting and the real CI-writer entrypoint. Seven historical link controls
exercise same-revision target and heading resolution, deletion, existing debt,
binary targets and symlinks. Their current runs do not qualify old product runs.

The remaining AUDIO-06 gap was concrete: `audio_ddx` runtime modes printed
nonzero device events and still returned `Ok(())`. All six modes now require
observed output progress and zero device events. Errors propagate through the
process result; raw/render close their stream before reading final counters.
Three device-free tests cover clean progress, nonzero events after progress and
no progress. Runtime events retain their documented control-side observation
scope. There is no physical-audio, new runtime-recovery or P5a scheduling claim.

An independent lane reviewed the A1/Q1 diff and found no actionable blocker.
Executable results and exact source/lock identities are owned by the aggregate
receipt, separate from these source observations.
