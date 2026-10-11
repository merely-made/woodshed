# Hocket standalone Firewheel consumer acceptance — 2026-10-11

Hocket adopts the owned standalone Firewheel 0.14 provider at immutable revision
`2b03846d2e0dcc6ded2a85367bb8c5071ae99cdc`. The accepted consumer source is
`f5e0aedc47de0674ab4e9556925da2e1a47e8798`, following the main migration commit
`59cbf31ad4ace8ab0d9222d688a73bdb534434b3`. This receipt records the independent
workspace gate and native recorder/appearance regression. Physical audio
qualification remains open in the [adoption plan](../../2026-10-11_firewheel_adoption_plan.md).

## Runtime and offline evidence

The engine owns the separate native `CpalStream`, adopts fallible graph APIs,
uses current sampler resource and restart events, and removes its old silent
input monitor and synthetic playing-state workaround. Twelve tests execute the
production graph with an offline processor: input-only signal and silence,
resource/restart/stop/click events, bar count-in and queued looping replay,
reader finish/drain/restart, overflow refusal and clean subsequent capture,
completed-capture retention after later overflow, pending-state reset, voice
limits, source-rate duration and output-only input refusal.

Actual rendered output reproduced a pre-existing layer-gain mismatch: sample
`-0.5` at gain `0.5` produced `-0.125` because Firewheel's linear volume applies
a squared slider taper. Hocket's documented multiplier and portable export
require `-0.25`. The consumer now converts positive amplitude gains to decibels
and handles zero as silence. Rendered zero/fractional/unity gain and voice-mix
assertions preserve the mute/solo projection. `engine-tests-first.log` retains
the failing reproduction from the provisional provider pin; acceptance uses
the final pin in `source-freeze.json` and `workspace-tests.log`.

The final independent locked workspace passes **166 tests**, with one
keeper-specific test explicitly ignored. Scoped engine Clippy passes with
`--no-deps --all-targets -- -D warnings`; the earlier dependency-inclusive
attempt is preserved because nine existing `hocket-model` lints stop that
broader command. Desktop and manual-demo builds pass. Four manual CLI probes
confirm help/no-argument success and invalid-argument refusal without opening
audio hardware. No physical `hocket-headless --run` demonstration is counted.

Locked metadata shows all eight Firewheel-family packages at the same exact
0.14 revision. The selected engine normal/build dependency tree contains no
`bevy_ecs`. Full Cargo metadata also lists optional dependencies, so its full
resolve graph is not treated as proof of the selected build. The consumer has
no local provider path override. Dependency details and warnings are retained
in `dependency-summary.json`, `engine-tree.log` and the associated logs.

## Native and persistence evidence

Five owned, finite LaunchServices processes on the Apple M4 (`Mac16,3`, arm64)
macOS 26.5.1 host completed with exit zero, no timeout and `RESULT ok`:

| Process | Scenario | Captures | Presented frames |
| --- | --- | ---: | ---: |
| `native-seed` | `tabard_appearance.scn` | 9 | 79 |
| `native-reopen` | `tabard_appearance_reopen.scn` | 2 | 13 |
| `native-same-id` | `tabard_appearance_same_id.scn` | 4 | 43 |
| `native-reopen-brown` | `tabard_appearance_reopen.scn` | 2 | 13 |
| `native-recorder` | `recorder_controls.scn` | 2 | 30 |

All **19 original PNG captures** were reviewed; none is blank. The warm default,
four canonical modes and authored blue/brown recorder controls remain readable.
Every recorder/appearance capture retains the semantic red Record dot. The
840×2000 physical-pixel narrow workshop keeps its editing and return/apply
controls usable. The two fresh processes restore the authored blue then brown
appearance from one `theme:copy-1` definition in dark mode.

Mesquite's retained viewport comparison confirms that Save preserves the
installed recorder snapshot and explicit Apply changes it. Independent original
PNG comparison of the recorder region below the title bar finds zero changed
pixels above a two-channel-unit tolerance after Save (maximum difference one),
then 3,078,997 changed pixels after Apply. The recorder fixture shows track
creation, arm/mute/solo, tempo 122 and master-clock controls. Its new
`input-available=false` assertion and inactive capture pass under the explicit
device-free receipt profile.

The accepted binary SHA256 is
`1c2a342559ac839d4561af92c341513054bbf254f0098da7eb28bd05cd0c720a`.
All five process receipts match that binary. Fifty source, manifest, lockfile
and scenario hashes were checked again before retaining this evidence.
`summary.json` records capture hashes, viewport comparison, process counts and
visual review. The total is 178 presented frames, 186 redraws and eight
unpresented redraws. Presentation counters are preserved as observed.

## Reproduction and remaining gates

From the independent `ports/hocket` workspace, with Rust 1.97.1 and an isolated
target directory, run:

```sh
CARGO_BUILD_JOBS=2 cargo +1.97.1 test --locked --workspace
CARGO_BUILD_JOBS=2 cargo +1.97.1 clippy --locked -p hocket-engine --no-deps --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo +1.97.1 build --locked -p hocket-genet -p hocket-headless
cargo +1.97.1 metadata --locked --format-version 1
cargo +1.97.1 tree --locked -p hocket-engine --edges normal,build
```

The native command provenance is retained in `run_native.py`. It uses Mere's
pinned `e95326d` `scripts/run_macos_scenario.py`, prefix `HOCKET`, a separate
output for each process and a 45-second timeout. All five runs share private
`HOCKET_SETTINGS`, `HOCKET_APPEARANCE`, `HOCKET_THEME_LIBRARY` and XDG
data/config/runtime directories. `HOCKET_APPEARANCE_RECEIPT=1` requires a
scenario and explicitly disables audio hardware. Rust 1.96 is the declared
package minimum; a minimum-version build is not claimed here.

Input resampling remains disabled, matching the prior consumer. Mismatched
input/output rates can cause CPAL's existing `fail_on_no_input=false` fallback
to omit input. Engine/host availability now follows actual stream input
channels and liveness, refuses either capture mode without input and marks the
Record switch unavailable. That guard is covered offline; enabling a new
mixed-rate input capability requires separate physical qualification.

Physical device selection, microphone permission, audible capture/replay,
device restart, count-in precision and round-trip latency calibration remain
open. Fresh reopen here qualifies application appearance persistence; native
project-file persistence and keeper custody remain separate gates. The isolated
profile visibly reports the unavailable identity service. Windows/Linux native
presentation, packaging and release publication are also outside this receipt.
