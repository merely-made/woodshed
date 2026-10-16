# Shared capture accumulator

Status (2026-10-16): task 3 implemented and CPU/offline qualified; native/device qualification remains separate.

## Feature target and done conditions

Extract allocation-free PCM writes into audio-primitives and consume them in
both adapters. Preserve raw append bits in Hocket and clamped indexed replace
and overdub in Woodshed. Both products retain scheduling, integrity and storage
ownership. Done when kernel tests, both engine suites, schedule/allocation guards
and Woodshed live/export parity pass with locked dependencies.

## Findings

2026-10-16: Hocket capture.rs owns a phrase buffer and appends samples; Woodshed
output_runtime/processor.rs writes preallocated per-bar PCM by cursor. Neither
requires a common scheduling algorithm. Hocket must reserve initialized storage
before feed; rearm after completion reserves again on the control side.

## Progress

2026-10-16: extracting only the accumulator portion of handoff task 3. The wrap
countdown, InputFeed trait, CompletedTake DTO and standalone dev harness remain
open. Existing product allocation guards remain independently owned.

2026-10-16: both adapters now use `audio_primitives::capture::TakeAccumulator`.
Hocket initializes phrase storage before feeding and reserves again when rearmed
after draining. Raw signed zero, out-of-range input and NaN payload bits survive
append. Indexed modes retain clamp order. No scheduler, persisted clock, PCM
migration, input-overflow policy or product completion API changed.

Validation: 55 audio-primitives + 158 woodshed-audio + 36 Hocket engine unit
+ 3 Hocket integration cases = 252 distinct passing tests. Both existing
allocation guards and exact schedule tests pass, including overshooting drained
input, empty wait-only start and tempo rearming. Woodshed live/export tests pass.
No new native, physical-input or latency qualification is claimed. Evidence is
in `validation/shared-capture_20261016/`; this receipt does not replace earlier
canonical-clock or native receipts.

2026-10-16 continuation: extract Hocket's shared phase countdown for capture and
queued layers, a minimal InputFeed implemented by drained slices and Woodshed's
queue consumer, and a generic CompletedTake storage/stamp envelope. Completion
stamps remain product-owned: Hocket retains its rate, Woodshed snapshots validity
and generation before returning deferred-destruction storage. The shared harness
is an integration-test binary with its own allocator, exercising the seams
together without devices or runtime dependencies. Done when this harness and
both actual adapter suites preserve schedule, generation, overflow and export
guards. No new timing, integrity admission or persistence policy is introduced.

2026-10-16 completion: all task 3 contents are connected. Hocket capture and
queued replay share BarCountdown; both engines consume InputFeed and
CompletedTake. The four-case shared integration harness has an independent
counting allocator and checks both allocations and frees, borrowed-feed capacity,
sample bits, resumed input gaps, indexed clipping, completion storage identity,
strict half-bar jitter rejection and tempo-restart baseline refresh. Existing
product guards still exercise the real adapters independently.

Final checks: 55 primitive unit + 4 shared harness + 158 Woodshed audio +
36 Hocket engine unit + 3 Hocket integration = 256 distinct passing cases.
Both `woodshed-genet --all-targets` and `hocket-genet --all-targets` locked offline
checks pass. No manifest, dependency pin or lockfile changes. Existing fractional
clock/live-WAV parity, legacy playback, input-loss invalidation and stale-owner
rejection tests pass. See `validation/shared-capture-seams_20261016/receipt.json`.
Earlier accumulator evidence remains unchanged. No persisted format changed and
no stored PCM was migrated. Native, reopened-profile and hardware evidence have
not been refreshed for this extraction.
