# Finite PCM admission and timing support

Date: 2026-10-18. Status: implemented and CPU qualified at `5dd01ae`; published through `8da953b` ([publication](publication.json)).
Baseline: published `b1c9dfd` (the nine previously reviewed commits).

B4 addresses AUDIO-02 (finite prepared backing), AUDIO-03 (onset sample clock),
and AUDIO-04 (calibration admitted support). Three cheaper Luna workers owned
disjoint source files. The coordinator reviewed integration, reused the shared
predicate in phrase preparation, and ran final process-bound qualification.

## Changes and authority

- Imported backing PCM rejects NaN and both infinities before publication.
  Completed rendered chord PCM, including cache hits, is checked before the next
  cache is published. Failed preparation retains accepted backing and cache.
  A poisoned-cache control exercises the completed-sample gate independently
  of frequency preflight. Rejected source PCM retains its exact bits.
- Saved chord, phrase and export preparation share one frequency overflow
  predicate, retaining each caller's duration precision. Imported, rendered
  chord and phrase buffers reuse `SampleBuffer::has_finite_samples`. All new
  scans occur during preparation, outside the audio callback. No renderer,
  resource cache or scheduler was duplicated.
- Completing an onset frame counts only newly consumed samples; a pending
  prefix has already been counted. Nonempty onset timestamps and total input
  counts agree for monolithic, irregular, empty and single-sample feeds. The
  Woodshed analyzer receiving test checks sample indices independently of
  callback wall-clock stamps.
- One calibration pairing result supplies both the estimate and admitted support
  count, including insufficient runs. Compatibility entry points delegate to it.
  The session consumes that result once; Settings already displays its count.
  Nearest-onset and upper-median rules remain unchanged. Zero support with a zero
  minimum returns no estimate. Real session tests feed synthetic audio and
  verify successful support and exclusion of a nearer negative onset.

## Qualification

The [receipt](receipt.json) binds all 11 changed source/test files, four unchanged
lockfiles and actual process exits/log digests to `5dd01ae`. Documentation was
being updated during qualification; process revision metadata alone is not a
clean-tree claim.

| Final library gate | Passed | Ignored |
|---|---:|---:|
| audio-primitives | 61 | 0 |
| woodshed-audio | 211 | 0 |
| Hocket engine (independent workspace) | 36 | 0 |
| Total distinct cases | 308 | 0 |

All three processes returned zero. No nested child invocations are in these
counts. Worker focused/repeated and failed development runs are excluded. The
audio suite includes existing allocation/deallocation guards, capture clock
controls and WAV/live parity across fractional clocks and chunk boundaries.
The export negative controls verify rejection before writer operations.

## Limits

No native apps, resident services, browser validation, microphone/device checks,
physical latency measurements or fresh-process native reopen were run. Calibration
remains approximate wall-clock/tap based; a corrected sample counter does not
establish measured round-trip latency. Existing nearest pairing can reuse an
onset; this batch does not change that policy. Original take PCM and playback
clocks are not migrated. No supplier or lockfile updates were made.

The seven reviewed B4 commits were published on 2026-10-19 under explicit user
authorization; the original process-bound receipt retains its dated local status. The broader audit remediation remains open.
