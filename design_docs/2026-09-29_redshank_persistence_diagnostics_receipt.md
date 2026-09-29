# Redshank persistence diagnostics receipt, 2026-09-29

The first real-worker Apparatus consumer is qualified at Redshank's existing
serial persistence owner. Dispatch, worker execution and desktop save-reply
handling remain separate observations. This receipt covers the isolated diff
from Woodshed `752c920e713fa511d6e5d73385c57522d7a03233`, awaiting integration.
The primary checkout's concurrent transcript work was not changed.

The contract follows Mere's current diagnostics design at
`mere/design_docs/mere_docs/implementation_strategy/2026-06-08_system_diagnostics_and_accessibility_plan.md`.
Redshank keeps Mere `ca2351b3fa21de1ca40a8553eed20fc503faf15c`, Genet
`19c206873ab08ae227217892d9e74d0df18b349a` and Netrender
`9607d16f1907f6c2085648ae96abcaa30d7c3d41`. The nested port lock changes only
three direct dependency edges; package versions and sources are unchanged.

## What the pilot records

`desktop/src/diagnostics.rs` owns a typed, redacted observation copy bounded by
count, accounted encoded bytes and age. It uses an `Arc<Mutex<_>>`; receipt
time is sampled inside that mutex. It introduces no application work queue.
The byte count comes from serializing the admitted typed payload. Apparatus
adds its envelope accounting; later JSON projection preserves that original
admission accounting rather than claiming an export-byte or allocator ceiling.

The real `Persistence::flush` records requested and accepted/rejected dispatch.
The storage thread records start and result around `JsonDirectoryStore::save`.
`Desktop::poll` handles the real reply and records `SaveReplyHandled`, with the
result and the resulting durable revision. A failed reply is not a durability
confirmation. Requests and execution occurrences have their own `RecordRef`s;
retries retain logical `OperationId("save:<revision>")` but get new request
references. Dirty revisions coalesced behind an in-flight save have no invented
operation or action cause. Payloads contain only phase, revision numbers and
fixed result categories, excluding paths, URLs, titles, transcript text,
fingerprints and raw errors.

`REDSHANK_DIAGNOSTICS=1` opts in. Retention settings are
`REDSHANK_DIAGNOSTIC_RECORDS` (default 256), `REDSHANK_DIAGNOSTIC_BYTES`
(262144), and `REDSHANK_DIAGNOSTIC_AGE_SECS` (300). Any zero disables retention
with explicit loss. Mesquite receives one bounded batch after completion using
its own independent cursor; another reader does not drain that cursor. Without
opt-in the existing scenario receipt has no diagnostics attachment.

## Automated gates

Workdir: `C:/Users/mark_/Code/worktrees/woodshed-diagnostics`. All successful
Cargo gates used the existing target and explicit profile settings:
`rustc 1.97.1 (8bab26f4f 2026-07-14)` and
`cargo 1.97.1 (c980f4866 2026-06-30)`.

```powershell
$env:CARGO_TARGET_DIR='C:/t/cargo-targets/woodshed'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
cargo test --locked --offline -j2 --manifest-path ports/redshank/Cargo.toml -p redshank-desktop diagnostics -- --test-threads=1
cargo test --locked --offline -j2 --manifest-path ports/redshank/Cargo.toml -p redshank-desktop -- --test-threads=1
cargo build --locked --offline -j2 --manifest-path ports/redshank/Cargo.toml -p redshank-desktop
cargo clippy --locked --offline -j2 --manifest-path ports/redshank/Cargo.toml -p redshank-desktop --all-targets -- -D warnings
```

The focused gate passed six selected tests: five new pilot tests and one
existing test. The restored full desktop gate passed **63/63**. The real IO
fixtures cover success before durable acknowledgement, failed IO plus retry of
the same revision, and later coalesced dirty revisions after acknowledgement.
Store tests cover independent readers and disabled-retention loss.
Strict desktop Clippy with all targets passed, as did scoped `rustfmt --check`
and `git diff --check`.

A deliberate temporary defect advanced `durable` at dispatch. Running the real
worker success test failed at the before-ack assertion, `1` versus expected `0`.
The defect was removed before the passing full gate and executable build.
The first cold compile found only fixture wiring errors; its failed log is
also retained. Logs under `C:/Users/mark_/Code/testing/redshank/diagnostics/`:
`focused.log`, `focused-restored.log`, `negative-early-durable.log`,
`desktop-restored.log`, `build.log`, and `clippy.log`.

## Native receipts

The empty-model `scenarios/apparatus_persistence.scn` issues three shipping
settings commands, without media, network, model downloads or DSP work:

```powershell
$env:REDSHANK_SCENARIO='C:/Users/mark_/Code/worktrees/woodshed-diagnostics/ports/redshank/scenarios/apparatus_persistence.scn'
$env:REDSHANK_CAPTURE_DIR='C:/Users/mark_/Code/testing/redshank/diagnostics/native'
$env:REDSHANK_DATA_DIR='C:/Users/mark_/Code/testing/redshank/diagnostics/native-data'
$env:REDSHANK_DIAGNOSTICS='1'
$env:REDSHANK_DIAGNOSTIC_RECORDS='2'
$env:REDSHANK_WIDTH='860'
$env:REDSHANK_HEIGHT='680'
& C:/t/cargo-targets/woodshed/debug/redshank-desktop.exe
```

`native/scenario.done` is **RESULT ok**, 22 lane frames, one nonblank
1720x1360 capture at the desktop's 2x scale. Three actual state generations
were persisted. The final batch retained two records, accounted bytes 649,
evicted 16, and reported the explicit sequence gap `[1,17)`. Sequence 17 is
successful execution; sequence 18 is the real desktop save reply with
`durable_revision=3` and cause sequence 17. `sampled_at_lane_frame=22` marks
the receipt observation point, without claiming causal pixel correlation.
The PNG was visually inspected as the empty light-mode Redshank view.

The same executable and fixture with diagnostics unset, using separate
`native-default` capture/data directories, also reported **RESULT ok** and
contained no diagnostics line. Setting the record limit to `invalid` in the
separate `native-invalid` run produced **RESULT fail**, with
`diagnostic attachment: invalid diagnostic retention setting`. Its original
failed receipt is preserved. Native process exit was zero in all three runs;
the scenario receipt's RESULT is the pass/fail authority.

SHA256 receipts:

- Executable: `DFB8DBB840AC6413FDABAAC43D14444CEF6EA1C3B05FE580A5FAE4D46DA5E580`.
- Native PNG: `CEEC18623DDEEAC63646A8EA64D06520E9164934E61E542C0111F6E5DB7D74E4`.
- Native scenario.done: `6FA2AA7C7B81BC6B31D2571490E213C2B6A811E7E20A4DEE0285A6361C354B32`.
- Per-file source hashes: `C:/Users/mark_/Code/testing/redshank/diagnostics/source-sha256.txt`.

## Open boundaries

This qualifies persistence observations and native receipt export. Exact
frame/pixel correlation, human assistive-technology testing, stale playback
replies and cancellation remain open. Storage cancellation and stale storage
rejection are not supplied by the current persistence owner. The observation
copy bounds its own retention; it does not claim to bound existing producer
queues or the model. The shared Woodshed target is retained for normal reuse;
no isolated Cargo home was created. The collision worktree remains owned by
the diagnostics integration until the root agent commits and integrates it.
