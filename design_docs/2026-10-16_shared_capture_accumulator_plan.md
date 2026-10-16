# Shared capture accumulator

Status (2026-10-16): accumulator slice implemented and bounded-tested; remaining task 3 seams open.

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
