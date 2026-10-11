# Hocket standalone Firewheel 0.14 adoption

**Status (2026-10-11): offline consumer gate passed; native regression pending.** Migrate the device/runtime consumer from
published Firewheel 0.10 to the pushed standalone fork at exact revision
`2b03846d2e0dcc6ded2a85367bb8c5071ae99cdc`. The lane starts from integrated
Woodshed commit `2c21326`, including the portable `hocket-project` extraction
and Mesquite close/readback fixes. The coordinator owns root stack plans.

## Scope and ownership

Hocket retains its independent workspace, lockfile, release and application
identity. `hocket-model` remains session/history authority. `hocket-project`
continues to own portable media, project storage, export, handoff and waveform
projection without Firewheel or CPAL dependencies. This change belongs to
`hocket-engine`, its desktop audio integration and consumer validation.

The reusable provider is `mark-ik/firewheel` branch `owned/stream-foundation`,
based on upstream standalone 0.14. Its bounded stream-reader/writer nodes are
shared runtime mechanics. Hocket does not depend on Bevy ECS to record a loop.
The fork's final revision adds source-frame counters, explicit finish/drain,
concurrent EOF publication ordering and bounded resampler allocation checks.
Supported resampling rates and ratios belong to the provider; the consumer
must still qualify its own capture and playback actions.

Hocket preserves device enumeration and stable host-local ID selection. A
separately owned `CpalStream` carries the native backend and returns its
processor before context destruction. Graph construction and dynamic voice
creation use fallible node APIs, returning errors instead of treating a node
ID as unconditional success. Missing or malformed device IDs retain the
existing default-device fallback policy.

## Phases and done-conditions

### F3a — Consumer API and lifecycle

- Pin the independent workspace to the exact pushed fork revision and keep
  its transitive Firewheel crates on that same immutable source.
- Adopt native stream ownership, current enumeration and fallible graph APIs.
- Replace populated-at-construction sampler workarounds with resource and
  `start_or_restart` events on the same voice and click node.
- Remove the silent microphone-to-output monitor only after an actual offline
  graph proves the input-only reader receives signal and silent frames.
- Preserve the existing UI-tick bar-phase and count-in contract. This pass
  does not claim sample-accurate scheduling or latency calibration.
- Use a bounded input queue without source-time autocorrection. Report a live
  input overflow, abort the affected capture and expose the error in the host.
  Missing input cannot be retried or silently promoted as a complete phrase.

**Done when:** meaningful offline tests render the production graph, verify
capture/replay/control events and exercise reader finish, overflow and reset.
Neither no-device refusal nor synthetic output establishes physical audio.

### F3b — Independent workspace gate

- Run the independent locked Hocket workspace tests, including portable
  project/model tests and the device-free desktop suite.
- Build the native desktop and explicit `hocket-headless --run` demonstration.
- Preserve the manual demonstration's nonzero startup/execution failure
  semantics and its distinction between queued replay and audible acceptance.
- Record the exact consumer revision, fork revision, commands and outcomes.

**Done when:** locked tests/build pass at the final pin, with no local Cargo
patch, no provider path dependency and no duplicate Firewheel source family.

### F3c — Consumer native regression and physical gates

- Reserve the foreground lane before starting any owned native process.
- Replay the bounded recorder/Tabard scenarios from committed consumer source
  using the private device-free receipt profile; review captures and reopen
  the authored theme in a fresh process.
- Keep physical input/output selection, microphone permission, audible replay,
  count-in precision and round-trip calibration as explicit remaining gates
  unless those flows are actually exercised and observed.

**Done when:** the native receipt identifies committed source, binary and
scenario outcomes. A device-free receipt cannot certify audio devices. Engine
migration is separately reportable from release publication or main merge.

## Findings

- **2026-10-11:** The 0.10 consumer used populated sampler construction plus
  `SamplerState::mark_playing`, dynamic node replacement on restart and tempo
  change, and an inaudible input-to-output monitor to retain capture processing
  (`crates/hocket-engine/src/lib.rs`). Current 0.14 resource/play helpers and
  the restored input-only reader support removing those exact workarounds.
- **2026-10-11:** Project extraction is already present at the lane base. No
  project schema, media identity, history representation or persisted device
  selection change is required for this migration.
- **2026-10-11:** Existing capture/replay starts at the first UI tick observing
  a bar wrap and accepts a full drain batch. The existing half-bar wrap gate
  rejects small cross-thread playhead jitter. Offline assertions retain that
  tolerance rather than claiming sample-accurate boundaries.

- **2026-10-11:** An actual rendered restart test reproduced a gain mismatch:
  source `-0.5` at layer gain `0.5` emitted `-0.125`, while the documented
  multiplier and portable export imply `-0.25`. Both 0.10 and 0.14 square
  `Volume::Linear`; the consumer now converts amplitude gain to decibels and
  renders zero, fractional and unity gains, summed voices and the stop-key
  projection used for mute/solo. This is a behavior correction, not just an
  API rename.
- **2026-10-11:** Root consumer review found all-zero PCM for a very short
  `8000 -> 44100` streamed sample despite valid EOF counters at provider
  `2a0a342`. Final consumer/native qualification is held until the provider
  fixes resampler EOF delay and supplies a revised pushed exact revision.

## Progress

- **2026-10-11:** Created isolated branch/worktree from `2c21326`; inspected the
  new APIs and prepared production-graph offline tests while the provider's
  final EOF/allocation fixes were under review.
- **2026-10-11:** Received the pushed provider revision `2a0a342`; consumer pin
  and API changes are ready for the first scoped compile/test gate. Native
  execution has not started.

- **2026-10-11:** The first scoped runtime gate passed 23 tests after the
  reproduced gain correction. This includes ten actual offline graph tests;
  the wider independent workspace compile is underway at the provisional pin.
  No native product has been launched for this lane.

- **2026-10-11:** Received corrected pushed provider `2b03846` after the
  provider material-signal EOF matrix passed; repinned the consumer and began
  its final locked gate. Device enumeration/selection and capture/playback
  runtime errors are propagated to the existing host status.

- **2026-10-11:** At final provider `2b03846`, the independent locked workspace
  passes 165 tests with one keeper-specific ignore. Eleven tests execute the
  production graph offline. Scoped engine Clippy (`--no-deps --all-targets`,
  warnings denied), locked metadata and desktop/manual builds pass. An initial
  dependency-inclusive Clippy attempt stops at nine pre-existing model lints;
  those files are unchanged by this lane. Completed capture data is preserved
  if the continuously running reader overflows later.

- **2026-10-11:** Final profile review confirms `cpal_resample_inputs` is
  disabled, matching the prior 0.10 consumer. Mismatched input/output rates can
  cause CPAL's `fail_on_no_input=false` fallback to omit input. The engine/host
  now derive input availability from actual stream input channel count and
  liveness, refuse either capture mode for an output-only stream, and mark the
  Record switch unavailable. Enabling input resampling and qualifying physical
  mixed-rate device selection are deferred capabilities. An offline zero-input
  stream verifies refusal without promoting graph silence into a phrase.

- **2026-10-11:** The output-only input guard passes the final locked workspace
  gate: 166 tests, one keeper-specific ignore, twelve offline production-graph
  cases, scoped engine Clippy and desktop/manual builds. Native qualification
  will use the follow-up source commit with this availability check.
