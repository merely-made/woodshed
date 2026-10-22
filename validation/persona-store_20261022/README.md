# D1 persona-store cutover — 2026-10-22

D1 is implemented and qualified on committed Woodshed source
`8bf59b317aa653c36a09a5fa9f178ef8e408e7d6`. Woodshed is unreleased and has no
deployed data compatibility obligation. The accepted clean cutover replaces
development singleton stores with strict sealed stores for each cryptographic
owner. The [canonical plan](../../design_docs/2026-10-16_preservation_input_admission_plan.md#p7-d1-persona-store-cutover)
owns the decisions and scope; this directory holds their evidence. P5a retains
its existing owner and was not changed by D1.

## Result

`WOODSHED_DATA_DIR` selects the whole application root. The desktop refuses
obsolete `WOODSHED_STATE` and `WOODSHED_SETTINGS` configuration. Persona session,
settings and immutable PCM live below `personas/v1/<owner-token>` with a sealed
owner record. Device window geometry lives in `desktop.json`; authored theme
and font definitions remain shared. Current fixtures use the new root contract.

The host admits the complete incoming state and every retained PCM companion
before installing it. It then commits family-wide persona choice through the
same custody provider and verifies the cryptographic identity before and after
that request. Refusal retains outgoing work. Same-persona selection checks the
actual installed root and owner. Missing or locked custody exposes an empty,
nonwriting session with an explicit unsaved notice. Export protection uses the
frozen application root and library, including missing destinations and aliases.

No compatibility shim, legacy assignment UI, migration journal, backup policy or
downgrade subsystem was added. Existing arbitrary development files are not
automatically deleted.

## CPU and build evidence

The final default gates have **678 distinct passing parent/integration cases**:
328 `woodshed-core` unit cases, seven core integration cases, and 343
`woodshed-genet` cases. Five nested desktop subprocess invocations are reported
separately. Eight desktop cases are ignored by the default command: seven
existing cases and the new explicitly invoked native seed/readback helper.

- [Core gate](checks/test-r1.log):
  `cargo test --locked -p woodshed-core -p woodshed-genet`. The core inputs did
  not change in the subsequent host fixes. Its earlier desktop result is
  superseded by the final desktop gate and is not added to the total.
- [Final desktop gate](checks/test-r5.log):
  `cargo test --locked -p woodshed-genet`.
- [Final desktop build](checks/build-final.log):
  `cargo build --locked -p woodshed-genet`.
- The ignored `persona_store_receipt::seed_or_verify_native_persona_stores`
  helper was explicitly invoked against an isolated real custody keeper to
  [seed](readback/seed.log), [verify after switching/reopen](readback/verified.log),
  and [verify after restored failure input](readback/verified-after-failure.log).
  These repeated helper invocations are not added to the distinct test total.

The working toolchain was Rust/Cargo 1.99.1 on macOS 26.5.1 (25F80), arm64.
This work does not supply an independent MSRV or other-platform qualification.
The committed supplier pins remain Mere `a07a375e`, Personae/Dramatis `7d498c27`
and Muniment/Eidetic `34105d63`; there was no supplier repin. Cargo's only lock
change declares the existing `pollster 0.4.0` dependency for the host.

## Native evidence

The frozen final executable SHA-256 is
`7d656c9b370e2c10f6c88c6b6e250730049d52b6726d0b01774f51440432079c`.
All five product processes exit zero, report `RESULT ok`, and have successful
runner metadata without timeout. Each starts through a temporary macOS
application bundle using the production binary. Native capture review covered
all **12 final images**, independently reviewed by the preferences lane.

| Phase | Captures | Observed result |
|---|---:|---|
| [A → B → A](native/aba/scenario.done) | 5 | A has Ember, 87 bpm and one Card; B has Dusk, 143 bpm and two Cards. Returning to A restores its state. Selecting A again dismisses the picker and leaves the complete checkpoint equal. |
| [Fresh A process](native/reopen-a/scenario.done) | 1 | A's sealed Card, personal relation source and manifest reopen with Ember/87. |
| [Fresh B process](native/reopen-b/scenario.done) | 1 | B's distinct sealed Cards, source and manifest reopen with Dusk/143. |
| [Corrupt incoming settings](native/incoming-failure/scenario.done) | 3 | B is refused; Retry and Cancel retain A. Damaged B bytes remain unchanged and the public family status still names unlocked A. |
| [Locked custody](native/locked/scenario.done) | 2 | A fresh process displays zero Cards, Not saving and the corrected known-unsaved reason. The close guard is visible; all eight owner/session/settings/PCM files remain byte-identical. |

The locked phase's scenario completes at the visible close guard. After checking
its completed receipt, the coordinator clicked the observed native accessibility
button **Discard unsaved practice and close**. The
[observation record](native-close-observation.json) distinguishes that action
from the last captured frame; [process metadata](native/locked/process.json)
records the resulting zero exit without timeout.

Eight native checkpoint files have exact `live == stored` state. Full A-returned
and same-persona checkpoints match; the full pre-failure and post-cancel
checkpoints match. The keeper-backed [seed](readback/seed.json),
[first readback](readback/verified.json) and
[post-failure readback](readback/verified-after-failure.json) match exactly,
including both owners, full Cards, personal source IDs and empty journal payloads,
all settings, capture namespaces/clocks/digests and PCM bit digests. Each persona
has a different synthetic 2,000-frame mono take at 1,000 Hz. This is exact
storage/admission readback; the native scenarios do not play those PCM takes.

The custody fixture is an isolated actual Djinn keeper, with synthetic local
identities and an explicit local endpoint. The keeper binary SHA-256 is
`8ec18c48797c9f7c8aac80994340c0d05bfbd9c63d536659558340e8e8b0f63d`.
The Mere checkout revision observed before building the fixture was
`cfbdaa88a464613c929b2b172c1c2c22866b5a27`. This records fixture provenance,
not a new supplier qualification or a Woodshed dependency repin. Its
[successful build log](checks/keeper-build-r2.log) uses the explicit
`custody-receipt-host` feature. Public [custody replies](custody/) are bundled;
private vaults, passphrase material, store ciphertext, executables and build
outputs are excluded.

## Failures and qualification limits

Initial compilation failed because the host did not declare `pollster`; the
next check passed. A subsequently added regression used a private supplier
field and failed to compile; it was corrected to use the public text field.
Another regression failed on MusicXML's raw/canonical destination comparison;
the destination guard now normalizes the path itself. The corresponding failed
and superseded check/test logs are retained under `checks/` and are not counted
as final passes.

The pre-fix native probe at `88b6b61f` exposed a misleading possible-write notice
for the known nonwriting pending backend. Commit `8bf59b31` fixes it and adds a
regression while preserving uncertainty wording for actual unconfirmed sealed
writes. All five final native phases were rerun on that committed fix. Two
earlier locked probes are explicitly [excluded](excluded-probes/): one closed
before its receipt, and the other completed assertions but timed out waiting at
the guard. Neither is a final native pass.

This receipt qualifies the scoped local macOS persona transitions, native
refusal/retry/cancel/close behavior, exact retained-data readback and fresh product
processes. It does not qualify physical audio, native PCM playback/hydration,
second-host operation, native replacement of a seed under the same profile label,
or manual resize/reposition. Changed-seed isolation and device geometry are CPU
covered; fresh native launches reused persisted geometry. The existing Settings
caption about audio streams being open is inaccurate in disabled-audio receipt
mode and supplies no audio evidence. Hocket, Redshank and P5a are outside D1.

## Rechecking the bundle

Run `python3 validation/persona-store_20261022/verify.py` from the repository.
The standard-library verifier checks the source and evidence SHA-256 manifests,
all final process/scenario metadata, 12 PNG dimensions, eight exact checkpoints,
both complete readbacks, refusal/locked hash records and distinct CPU accounting.
It reads only repository-relative bundled files and performs no custody or native
I/O. Hash/dimension checks do not substitute for the recorded visual review.

`source-sha256.json` binds the committed crates, assets, vendor inputs, Cargo
configuration/lock and native runner; it is a snapshot binding, not a claim that
every listed source file received new coverage. Raw logs, scenarios and harness
sources retain original absolute trial/build paths as **local only** provenance.
Do not follow those paths to verify this portable bundle. The original fixture
root is `/Users/markik/Code/testing/woodshed-persona-store-20261022` (local only).

Source, documentation and verifier whitespace checks pass. Six raw Cargo logs
retain their original terminal blank lines; their whitespace diagnostics are
intentional evidence bytes and the files are hashed without modification.
