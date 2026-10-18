# Woodshed complete-source audit — audio partition

Frozen revision: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`.
Snapshot: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.
Assigned manifest: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/audio-assigned.json`.

**Coverage is complete: 34/34 assigned files and 12,014/12,014 lines read, including all tests, manifests and the diagnostic example.** Every assigned SHA-256 and line count matched the assignment before reading; every hash was rechecked after the read. Exact contiguous read ranges, per-file hashes and status are in `audio-coverage.json`. Assigned source output was chunked and not truncated. An initial truncated documentation-index read was replaced by full, untruncated ranges 1–220 and 221–413.

This was a read-only source and architecture audit. No build, test, device/native run, network request, Git mutation or product/source/doc/lock/profile change was performed. Only the two authorized external audio audit artifacts were written. The primary checkout was inspected read-only only for the known dirty P5a preview work; its HEAD was the same frozen revision. Source-derived examples below are execution traces, not newly run tests.

Severity: P2 = correctness fix in the ordinary queue; P3 = bounded cleanup or verification weakness. No P0/P1 finding is asserted by this partition. These are partition-local findings for the parent to consolidate, not acceptance of unassigned source.

## Findings

| ID | Severity | Finding | Reachability |
| --- | --- | --- | --- |
| AUDIO-01 | P2 | Saved-bar record arm disappears at an earlier boundary | Current saved-song recorder UI and desktop route |
| AUDIO-02 | P2 | Live PCM admission can accept NaNs that WAV export rejects | Restored saved-song pitches; public runtime admission |
| AUDIO-03 | P2 | Shared onset sample clock double-counts carried samples | Woodshed live analyzer and opt-in Hocket engine detector |
| AUDIO-04 | P2 | Calibration displays a different match count from the estimator | Current Settings calibration flow |
| AUDIO-05 | P3 | Unused legacy execution engines retain independent semantics/tests | Public compatibility APIs; no current in-repo product caller found |
| AUDIO-06 | P3 | Runtime diagnostic exits successfully after observed device events | Documented audio_ddx runtime modes |

### AUDIO-01 — Preserve a saved-bar arm until playback reaches its target

**Trigger and effect.** Load a looping saved song with at least three bars. While playback is in bar index 0, select bar index 2 and press Record. The host accepts `arm_capture(2, false)`. At the next boundary playback advances only to index 1. `Processor::boundary` takes the Arm out of the pending slot, tests whether current index equals 2, and drops it when that test fails. It then clears the pending length. Capture never starts when playback later reaches index 2, and no refusal is returned. An armed control can return to Idle without a take or actionable reason. The same loss occurs when targeting the currently playing bar in a multi-bar song, because the next boundary enters another bar.

**Exact evidence.** [processor.rs:660–709](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/output_runtime/processor.rs:660) consumes all pending operations once; lines 685–704 start an Arm only if the cursor already equals its target. [processor.rs:815–835](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/output_runtime/processor.rs:815) advances the cursor before applying those operations. [output_runtime.rs:532–561](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/output_runtime.rs:532) accepts the arm and records its control-side reservation.

**Caller trace and authority.** [recorder.rs:229–239](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/looper/recorder.rs:229), 358–370 and 573–581 select any saved row and queue `RecordSavedBar(index)`; [sync.rs:516–520](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/sync.rs:516) executes it; [audio.rs:275–294](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/audio.rs:275) passes `seek=false`. This conflicts with the explicit host contract in [core/audio.rs:275–277](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-core/src/audio.rs:275): capture begins when playback next reaches that bar. The output processor should own execution of the accepted schedule. The competing implementation is the one-boundary operation drain, also reproduced by the old data-model helper in `song.rs:461–483`.

**Existing tests/gap.** Production graph tests cover seek=true in-block arming (`output_runtime/tests.rs:402–436`), one-bar capture and terminal one-shot cancellation. They do not cover seek=false with a requested bar beyond the first boundary. The pure Song tests likewise do not exercise a future-bar arm surviving intervening boundaries.

**Smallest coherent direction.** Retain a non-seeking Arm across boundaries until its target is reached, explicitly cancelled, superseded by an owner reset, or made unreachable by terminal playback. Keep its boundary reservation while retained. Do not repair this by forcing a seek: saved and Set capture schedules intentionally differ. Add a focused production-processor case for an intervening bar and an explicit one-shot unreachable result.

**Dirty P5a:** no relevant fix in the inspected audio diff.

### AUDIO-02 — Use one finite-PCM admission rule before publishing live resources

**Trigger and effect.** A restored saved-song bar can contain a finite positive pitch such as `1e38`, with a normal 120 BPM, four-beat clock. Host song admission validates the clock, then the audio preparation accepts any finite positive pitch. The chord renderer multiplies elapsed time, frequency, harmonic factors and TAU in f32; an intermediate overflows, sine produces NaN, and the final clamp preserves NaN. The prepared resource is published successfully, and the processor mixes it into device output. Exporting that same accepted song rejects it. This is a malformed persisted-input path, not a claim that ordinary catalog pitches overflow.

**Exact evidence.** [processor.rs:167–199](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/output_runtime/processor.rs:167) validates pitch count/finiteness/positivity but never validates the resulting samples. [chord_audio.rs:138–173](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/chord_audio.rs:138) performs the overflow-prone phase arithmetic and clamps the resulting PCM. [processor.rs:768–813](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/output_runtime/processor.rs:768) and 841 consume and output it. In contrast, [offline/song_export.rs:168–193](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/offline/song_export.rs:168) checks phase bounds and every captured PCM sample before rendering/writing.

**Caller trace and authority.** [stage.rs:2453](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage.rs:2453) restores `session.song` exactly; [sync.rs:584–601](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/sync.rs:584) admits it; [audio.rs:694–720](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-genet/src/audio.rs:694) checks tempo/meter/hold and copies stored pitch vectors through `to_song` (173–200). The owner of resource validity is control-side audio admission. Live preparation and export's independently written validator currently disagree about what that owner has accepted.

The same admission family has related seams: `PreparedBacking` checks capture rate/length but not PCM finiteness (processor.rs:159–166); `PreparedClick` checks only rendered sample zero (311–328), so later nonfinite samples can escape. Preview admission filters only invalid/nonpositive frequencies (output_runtime.rs:628–635), leaving the huge finite frequency case. These are one validation/consolidation issue rather than separate counts of the same defect.

**Existing tests/gap.** Export's `malformed_snapshots_and_preparation_bounds_refuse_before_touching_the_writer` explicitly tests `f32::MAX` chord input (`song_export/tests.rs:217–272`); live preparation has no matching refusal case. Ordinary chord finite/bounded tests only use musical frequencies. Raw preview tests cover nonfinite duration/strum, not renderer-produced nonfinite PCM.

**Smallest coherent direction.** Centralize preparation-time finite-resource admission in the shared production preparation path, then have export add only export-specific rules. Validate completed synthesized PCM before command publication; reject nonfinite imported PCM before it enters a live prepared resource. Keep checks off the audio callback. The previous accepted song/preview must survive rejection. This need not introduce a new global pitch policy or change valid musical PCM.

**Dirty P5a:** the local exact-Card wrapper changes duration/spacing and refusal reporting; its inner preparation path still has this finite-PCM gap.

### AUDIO-03 — Count each input sample once in the shared onset clock

**Trigger and effect.** Feed 128 samples and then another 128 samples into a default 256-sample detector. The first call saves the partial frame and increments `samples_consumed` by 128. The next call completes that same frame and adds the full 256 again, making the exported clock 384 after only 256 actual samples. Repeated splits accumulate drift, change debounce spacing in sample time, and bias inter-onset BPM estimates. The result depends on batch boundaries.

**Exact evidence.** [audio-primitives/onset.rs:174–205](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/audio-primitives/src/onset.rs:174), specifically 181–182 and 197–200. The inflated counter becomes the onset timestamp at 230–238 and drives `estimate_bpm` at 270–283.

**Caller trace and authority.** Woodshed forwards unaligned CPAL mono batches at [input.rs:732–742](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/input.rs:732) to [woodshed-audio/onset.rs:189–198](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/onset.rs:189). Hocket's opt-in detector feeds whatever count was drained on a host tick at [hocket-engine/lib.rs:455–478](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/ports/hocket/crates/hocket-engine/src/lib.rs:455), then exposes `detected_bpm` at 434–440. The shared detector is the rightful sample-clock authority; its partial-input accounting and completed-frame accounting both claim the carried samples.

**Reachability limit.** Woodshed's current calibration pairs `DetectedOnset.at` wall-clock instants, so this sample counter error is not by itself evidence that its displayed latency changes. No in-repo desktop call of `OnsetHandle::current_bpm`, or Hocket host call enabling onset detection, was found. The wrong shared API output and the two actual feed adapters are established; a currently exposed desktop audio-tap-tempo feature is not claimed.

**Existing tests/gap.** `detector_handles_split_buffer_seam` (onset.rs:459–486) checks one seam and allows a whole frame of timestamp error; that tolerance conceals the duplicated prefix. Tests do not assert exact `samples_consumed == total fed`, repeated tiny chunks, or batch-invariant onset intervals.

**Smallest coherent direction.** Pick one meaning for the counter and account only newly consumed samples. If it includes partial input, completing a frame must add only the new suffix and timestamps must identify the correct frame end. Add exact total-count and many-chunk invariance cases in audio-primitives; both adapters then receive the fix without local clocks.

**Dirty P5a:** unchanged.

### AUDIO-04 — Report the estimator's admitted calibration pairs

**Trigger and effect.** At the standard six-click run, three hits occur 40 ms after their clicks and three occur 20 ms before theirs. The estimator ignores the three negative offsets and succeeds on three positive pairs, but `count_matches` includes all six. The UI displays a successful `40 ms · 6/6 hits` result although only three pairs support the measurement. With all six hits early, the failure branch can say `only 6/6 hits caught`.

**Exact evidence.** [audio-primitives/calibration.rs:45–78](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/audio-primitives/src/calibration.rs:45) filters negative offsets for the estimate; [calibration.rs:84–95](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/audio-primitives/src/calibration.rs:84) counts either sign. [woodshed-audio/calibration.rs:285–300](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/calibration.rs:285) combines those independent answers into Success/InsufficientPairs.

**Caller trace and authority.** Settings queues start, desktop audio.rs:846–913 runs/polls this session, then [settings.rs:117–131](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-views/src/stage/settings.rs:117) renders the count and exposes Accept. The estimator should own both the admitted estimate and its supporting pair count. The competing count helper has a broader pairing criterion.

**Existing tests/gap.** Primitive tests verify negative-offset rejection (149–158) and count-matches separately (160–170), thereby testing both contradictory rules independently. Session tests cover unavailable/rejected control paths but not mixed early/late pairing and truthful displayed counts.

**Smallest coherent direction.** Return the admitted pair count with the estimated latency, or expose one pairing result used by both functions. If the UI also wants any-window detection count, name that separately rather than using it as measurement support. Preserve the documented approximate control-time calibration policy; this finding does not request sample-accurate or physical calibration.

**Dirty P5a:** unchanged.

### AUDIO-05 — Retire or explicitly isolate unused legacy execution algorithms

**Concrete maintenance trigger/effect.** A consumer following the public `woodshed_audio::Looper`, `Song::advance` or `render_pattern` APIs executes independent capture/transport/mixing semantics, while tests of those helpers can be mistaken for coverage of the product's production processor. No current in-repo product call reaches those execution implementations. This is verified cruft/compatibility risk, not a claim that two output streams still run.

**Exact evidence and competing owners.**

- [looper.rs:60–181](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/looper.rs:60) owns its own record arms, mutable take, cursor, content flag and overdub; lib.rs:54 exports it. Its overdub uses a private clipper (195–205), whereas the current processor uses shared `TakeAccumulator` clamped indexed writes. The private clipper is even discontinuous at +/-1: values just above +1 map near +0.5. No product construction/use was found; all `Looper::new` hits are in its own tests.
- [song.rs:445–523](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/song.rs:445) retains a second pending/advance state machine. All `Song::advance` calls are its own tests; the only neighboring `on_bar_boundary` call is itself inside a loop-projection test. It differs from current one-shot processor cleanup, which cancels pending arms and finishes capture.
- [offline.rs:162–279](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/offline.rs:162) keeps `PatternRenderState`, trigger cloning and voice timing beside production `PreparedClick`/Processor. Only `render_pattern`/`export_wav`'s own module tests call them in the snapshot. Current backing WAV export correctly uses the production graph. This old public path also normalizes zero channels to one for allocation (45) but passes zero to its internal renderer, which divides by zero (207–209).

**Tests/gap.** Large self-contained test blocks preserve each private implementation. Production graph tests are distinct and do not call these helpers. Shared kernels already exist for capture writes, click synthesis and rounded beat clocks, so retaining the separate algorithms is not required to share those primitives.

**Smallest coherent direction.** Make a deliberate compatibility decision, then remove unreferenced execution helpers or isolate them behind an explicitly legacy API. Preserve `Song`/`Bar` as the live prepared-document projection where needed; do not delete those types wholesale. If public pattern export remains supported, route its execution through a device-free production pattern lane with a stated PCM/gain contract and parity checks. Do not adopt Hocket's product engine to achieve this cleanup.

**Compatibility uncertainty.** This repository cannot establish external callers of the public library. Root audio-primitives is marked publish=false; woodshed-audio is public API within this repository and may have external Git consumers. Removal must account for that, rather than assuming a repository search proves worldwide non-use.

**Dirty P5a:** does not remove these implementations.

### AUDIO-06 — Make the hardware diagnostic's terminal result state what passed

**Trigger/effect.** Run `audio_ddx song`, `chord`, `scale` or `idle` on a route that reports a recoverable output device event while continuing to process frames. `service` records the event without returning Err. The example prints `DDX complete`, prints the event count/last error, and returns Ok. A caller checking its exit status alone therefore gets success despite events. The raw/render modes assert zero errors and exit unsuccessfully under that same error-count condition.

**Exact evidence.** [output_runtime.rs:215–225](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/src/output_runtime.rs:215) records poll-status events but only fails for a stopped stream; [examples/audio_ddx.rs:129–144](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/crates/woodshed-audio/examples/audio_ddx.rs:129) asserts frame progress, then prints errors and returns Ok. The raw route asserts its error count at 213–217. [validation/song-cache_20261008/README.md:30–36](/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf/validation/song-cache_20261008/README.md:30) documents invoking the runtime example.

**Authority and current evidence limit.** Device health observation belongs to OutputRuntime. Qualification of a trial belongs to the diagnostic/receipt evaluator. No false historical acceptance is inferred: the recorded qualified receipts explicitly inspected event counts. The risk is that future automation treats process success as a zero-event qualification, unlike the raw diagnostic's existing behavior.

**Tests/gap and direction.** No source test pins the example's runtime terminal pass criterion. Give the diagnostic an explicit terminal classification and a consistent documented exit criterion, or a strict qualification mode that fails when its stated zero-event condition is unmet. Do not change ordinary product `service` to treat every recoverable notification as a fatal device failure.

**Dirty P5a:** unchanged.

## Known debt and deliberate boundaries — not additional duplication findings

- **Input callback locking/allocation is real but explicitly retained by the output-runtime plan.** OnsetAnalyzer holds its control/UI mutex through detector feeding and snapshot replacement (`woodshed-audio/onset.rs:167–199`); the shared detector allocates median scratch each frame (`audio-primitives/onset.rs:306–319`), and a nonempty recent history allocates a replacement Vec on successive enabled callbacks. PitchAnalyzer locks shared state and lazily initializes McLeod thread-local state. Input downmix can grow its scratch buffer. The output processor's zero-alloc tests do not cover those paths. The stated Analyzer contract (input.rs:58–66) is stronger than this implementation, but Stage plan 3166–3178 and rehome plan 202–206 explicitly exclude input cleanup from the qualified output slice. A future cleanup should prepare rate-specific state off-thread, reuse scratch, and publish bounded observations; this audit does not reclassify the existing output-only receipt as false.
- **One root OutputRuntime versus a Hocket engine is deliberate.** They have different document identity, count-in, scheduler cadence and capture admission. The extraction shares actual pure seams (TakeAccumulator, InputFeed, CompletedTake and Hocket BarCountdown) without transferring product clocks/custody. Do not make Woodshed depend on the port engine.
- **Legacy and canonical Bar clocks are deliberate compatibility.** Existing PCM retains its old rounded block clock; new capture is refused until legacy captures are explicitly cleared. The source has focused mixed-clock and export parity cases. Removing `legacy_clock` arithmetic as apparent duplication would alter persisted material.
- **SongDoc versus the Set is an acknowledged open P7 product migration.** Existing saved authoring cannot be losslessly replaced by the currently supported Set material/clock representation. Rehome plan 211–226 and Stage plan 1848–1858 preserve it explicitly. The runtime `Song` is a prepared control projection, not an additional durable material owner. AUDIO-05 targets unused execution algorithms, not that compatibility document.
- **Reexports/wrappers are not private DSP copies.** The calibration/onset facades, SampleBuffer buffer-shaping methods, Sound click renderer and frames-per-beat use shared audio-primitives implementations. Similar unit tests around the wrappers test the adapter surface; they are not evidence of separate kernel authority.
- **Alignment and WSOLA have a different actual consumer.** They are shared primitives for Redshank; absence from Woodshed's practice output is not proof of dead family code. The assigned source advertises synthetic fingerprint qualification limits and preserves algorithm/version/bounds. No broader real-recording matching qualification follows from reading those tests.
- **Approximate acoustic calibration and unused stored compensation remain stated open behavior.** The current host stores latency_ms, but no recording timing compensation was found. This was already declared open in the handoff; it is not a new claim that capture is compensated.

## Frozen preview findings already addressed by dirty local P5a

Read-only comparison was made against primary `/Users/markik/Code/repos/woodshed`, HEAD `d12201bfe80cc94345c6308c46caaebf4a9fe42e`, with uncommitted P5a changes. Its observed file hashes and inspected portions/diff paths are recorded separately in the coverage receipt. These are **source mitigations, not independently tested/published/native-accepted fixes**.

The frozen generic preview applies a 100 ms minimum to chord/ordered pitches (output_runtime.rs:598–609), silently returns Ok during backing playback (618–620), and offers no operation result through the default-no-op AudioBackend preview methods (core/audio.rs:234–257). Desktop sync uses those void methods at 138–175. This prevents exact short Card timing and truthful dependent rehearsal admission.

The local diff adds `OutputRuntime::preview_card_pitches` (local output_runtime.rs:592–607), preserving exact positive duration, rejecting backing playback and subframe duration/spacing before cancellation. Local audio.rs:471–476 returns this operation's Result through `admit_card_preview`; local sync.rs:123–127, 144–162 consumes it and surfaces notices. A new local unit test checks 60 frames at 1 kHz and refusal preserving prior work. Therefore these exact-Card minimum/refusal issues are **not counted again among the six unfixed findings**. Generic note/phrase audition still retains legacy contracts; the local full P5a ownership/timing implementation is outside this partition and was not comprehensively reviewed.

The independent raw nonfinite duration/strum guard is already present in the frozen snapshot, with tests at output_runtime/tests.rs:45–65. It is not an outstanding finding and should not be credited as a new local fix.

## Coverage and unresolved questions

The every-line assigned read is complete; no assigned file remains partial or unread. Tests were read, not executed. Supplemental neighboring files were read only in the exact ranges listed in the coverage receipt and are not claimed as complete partitions.

Questions for consolidation:

1. Confirm saved compatibility capture should wait through intervening bars, as both the public host contract and current selected-row UI say; retain Set seek=true behavior separately.
2. Decide the public API compatibility policy for the three unused legacy execution surfaces before removal.
3. Choose one preparation-time finite-PCM admission result used by live and export, preserving valid PCM bytes and previous accepted material on refusal.
4. Keep independent input-callback and physical capture/device-clock qualification open; neither this read nor output-only allocator tests close them.
5. Recheck P5a once its owner finishes and publishes; a dirty source mitigation is not a final receiving receipt.

No other tasks were messaged. The parent owns deduplication, verification and the collective final assessment.

