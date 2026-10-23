# Current audit frontier and B8 playback correction — 2026-10-23

The remaining audit frontier was rechecked against clean, fetched/pulled main
`3ce93675179098b7d1c81da20cb4f63caa75e6b5`. The selected B8 slice is implemented
and CPU/build qualified at `046b0bf4fe06a8e18dfe7b8dc8874813edfcbfbd`.
The [canonical plan](../../design_docs/2026-10-16_source_authority_audit_plan.md#p11-recheck-the-remaining-frontier-and-repair-playback-admissioncontrols-2026-10-23)
owns scope and ordering; [recheck.json](recheck.json) records the targeted source
reconciliation, and [receipt.json](receipt.json) records execution. This is not a
new exhaustive source audit or a transfer of historical native qualification.

## Why B8 was next

An ordinary cached audio file whose bytes no longer match its retained receipt
could trigger a nested mutable backend borrow during refusal. The worker caught
the panic and replaced a useful integrity diagnostic with generic recovery.
Recovery also rebuilt effective controls at defaults while retaining the old
published rate/volume/duck values. Preview output had a separate desired gain
despite sharing the actual output, so preview could ignore listener mute before
any episode opened output. Exact-end seek could remain Loading because the
backend waited for a first frame that could never arrive.

These ordinary playback paths took priority over B7's misleading availability
wording. Three new regressions reproduced the cache panic and backend/worker EOF
failures before any production fix; both captured baseline Cargo commands exited
101 with stable before/after source. The exact [test-only patch](baseline-tests.patch)
is retained for reproduction against the recorded base. Baseline failures are
negative controls and are not counted as positive qualification.

## Implemented behavior

- **R02 — shared gain:** one private output owner retains volume and capture
  duck before a runtime exists. Episode and voice-preview backends share it;
  the initial Firewheel gain node uses the effective setting directly. A voice
  preview keeps its independent unity-rate policy.
- **R03 — recovery:** control configuration survives outside the recoverable
  session. Recreated backends receive gain before output, and admitted loads
  reapply rate through the controller. Capture duck persists until the host
  explicitly sends Unduck; worker recreation cannot end a host-owned capture.
- **R06 — refusal:** cached admission finishes and releases its mutable borrow
  before error handling re-enters the backend. The integrity diagnosis and cache
  bytes survive; a later valid load remains possible.
- **R07 — terminal EOF:** EOF before a first frame is a terminal fact. Exact-end
  and empty admitted WAVs finish without opening a sink merely to finish.
- **R17 — admitted rate:** loaded changes and reloads use the existing playback
  controller. Source-less preferences wait for successful admission; a failed
  load does not publish them as accepted.

Only four playback source files changed. No supplier pin, manifest, lockfile,
saved format, original PCM, capture clock or shipping public API changed. The
`InspectControls` command exists only under `cfg(test)` and reads actual backend
rate, controller rate and shared effective gain.

## Checks and accounting

All positive gates have observed exit zero and stable captured source inputs.
**167 distinct unit/integration cases pass**, with **eight existing ignores**:

| Gate | Passing cases | Existing ignores | Evidence |
|---|---:|---:|---|
| Playback units and IPC integration | 39 + 3 | 7 | [Process](checks/playback-final/process.json), [log](checks/playback-final/command.log) |
| Desktop and model receiving | 92 + 18 + 15 | 1 | [Process](checks/receiving-final-r2/process.json), [log](checks/receiving-final-r2/command.log) |
| Locked desktop build | — | — | [Process](checks/desktop-build/process.json), [log](checks/desktop-build/command.log) |
| Changed-file formatting | — | — | [Process](checks/format-final/process.json) |

Exact commands and selected environment are in each process record. The three
baseline failures are in [cache](checks/baseline-cache/process.json) and
[EOF](checks/baseline-eof/process.json). An initial receiving command selected
the nonexistent package `redshank-genet`; its [exit-101 record](checks/receiving-final/process.json)
is retained and excluded. The corrected command selects `redshank-desktop`.
It was a package selection error; no product test ran in that attempt.

Seven new regressions cover actual production worker/controller/backend paths:
cache refusal/retry with byte retention; exact-end backend at 8/44.1/48 kHz;
worker resume and subsequent explicit seek to end; empty WAV completion; deferred,
clamped and repeated rate admission; actual backend/controller/gain after panic
and reload; shared episode/preview mute and duck in both directions. Recovery
assertions read reconstructed backend facts rather than echoing the retained
snapshot. A command barrier prevents a prior Ended snapshot from satisfying a
later seek assertion prematurely. The EOF tests assert absent sink/runtime.

Two independent reviewers read the final four-file patch and all seven tests;
neither found an actionable blocker. Tests/build ran on Rust/Cargo 1.99.1,
macOS 26.5.1 (25F80), arm64. The gates ran from source-dirty base `3ce93675`;
every captured compiled product input remained stable and matches the committed
qualified source byte-for-byte. Committing adds no repeated test count. The
[desktop artifact record](desktop-artifact.json) binds the successful build and
an immediately frozen local executable hash; no executable is bundled here.

This establishes scoped CPU behavior, real generated-WAV decoding, and desktop
compilation. It adds no new native GUI, microphone, physical sound, listening
gain/rate, second-host, MSRV or fresh-process native acceptance. The eight ignored
device/fixture cases remain unexecuted. Prior native receipts retain their own
source and observation boundaries.

## Corrected frontier

The [mutable ledger](../full-source-audit_20261016/current-finding-ledger.json)
retains all 97 rows, including aliases and observations; frozen audit reports
are unchanged. Source findings and qualification are recorded separately.

- **D1 is already qualified:** replace the stale singleton-store observation
  with its actual persona-store receipt and explicit native limits.
- **P5a's bounded assignment is complete:** the historical handoff was to the
  link-model coordinator, and the Stage plan records its qualification. The
  maintainer authorized taking over if needed; the completed reservation is
  retired. Generic Path/Walk Hear parity remains separate. No P5a code changed.
- **B7 V03 remains open; shared S4 is implemented:** Woodshed availability
  projection needs host facts; the existing public Recorder status fix is not
  duplicate work. Redshank R04's idle predicate has source replacement, with
  actual idle-native completion still unqualified.
- **Next product slices:** Redshank capture gesture/capacity and accepted
  duration (R05/R09), collision-safe annotation export identity (R08), then
  truthful Woodshed stream/listening projection (V03).
- **Other component work stays bounded:** Hocket staged-update/handoff,
  C1–C5, obsolete scenarios, packaging and scoring remain open. Q5's old cohort
  claim is superseded by the aligned lock and historical eleven-case receiving
  CPU result. Public compatibility removals remain decision-gated. Old private
  instrument findings moved into optional Ringdown research paths; they are not
  current Woodshed GUI RPC reach or authorization for hardware operations.

The recheck samples the named mechanisms and existing receipts; it does not
claim a second line-accounted read or new execution of every open finding.

## Verifying this bundle

Run `python3 validation/audit-playback_20261023/verify.py` from the repository.
It checks observed process exits, stable source inputs, log/evidence digests,
distinct test accounting, the negative controls, committed source bindings,
and the recheck's frozen-current hashes from the recorded Git revision.
It performs no native, network, custody or device action.

Raw logs/process records and the recorded gate wrapper retain absolute build and
trial paths as **local only** provenance. Verification reads bundled relative
paths and repository Git/source bytes; it never follows historical absolute
paths. Local executables, target caches and generated media are excluded. Raw
Cargo log terminal blank lines and original test-patch context spaces are
retained byte-for-byte and excluded from the source/document whitespace gate.

Source bindings are a snapshot of tracked product/configuration/assets, not a
claim that every bound file received new execution or line coverage.
