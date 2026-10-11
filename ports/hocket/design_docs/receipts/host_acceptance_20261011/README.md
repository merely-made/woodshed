# Hocket host acceptance — 2026-10-11

Source is `9dc042f03d862ac71cf53d9a144f26e22d16d1a0`, based on coordinator
plan `95abb13` and current Tabard source `cd69da0`. The production desktop
binary SHA256 is
`b070337242d37e0b8576962353b9a41226ed37edb76b43bad0fd5c6ac2f402c5`.
`source-freeze.json` records all desktop source hashes and the committed
manifest/lock. Rust is 1.97.1. Mere `e95326dc`, Genet `7422e906`, Dramatis
`c1d62076`, Firewheel 0.10 and the existing Vello family remain pinned.

## Behavior and automated proof

An accepted native close or appearance Save-close now asks Mesquite to finish
the pending readback and write the interrupted outcome before the host exits.
Appearance cancellation keeps the scenario live. Failed GPU readback records
zero completed captures and an unsuccessful receipt. Hocket's busy reading
includes its existing active save/load/export state. Three unused direct
capture dependencies are removed; waveform leaves still use `paint_list_api`,
and the project worker still uses `pollster`.

`early-close-before.log` reproduces the pre-fix defect through the real Hocket
hooks: the mounted host exits while requested pixels remain outstanding. The
new regression passes after deferral. `host-tests.log` records 68 desktop
tests passing, with one keeper-specific custody test ignored, plus two manual
demo CPU tests. `manual-tests.log` repeats the manual checks after the final
empty-capture refusal. `build.log` records desktop and manual binaries building.
Locked full metadata resolves outside repository Cargo configuration, with
four workspace members and no renderer/audio repin.

The headless binary now requires explicit `--run` to open hardware, returns
failure for missing devices or engine operations, and refuses an empty or
incomplete capture. Success reports that capture completed and replay was
queued. Four actual CLI executions in `manual-cli.json` prove no-argument/help
requests and invalid arguments remain device-free. No real microphone demo
was run; audibility and calibrated musical timing remain unqualified.

## Native presentation and fresh-process proof

Five separate finite LaunchServices processes completed with `RESULT ok`, no
timeout, 178 successful presented frames and 19 original PNG captures. Every
capture was opened and visually reviewed; all are nonblank. The normal window
is 1180×700 logical pixels; the narrow workshop is 420×1000.

| Process | Capture count | Accepted observation |
|---|---:|---|
| `native-seed` | 9 | Warm missing-preference default; four modes; authored preview; Save/Apply; usable narrow workshop |
| `native-reopen` | 2 | Fresh process restores the saved blue appearance |
| `native-same-id` | 4 | Save keeps the installed blue snapshot; reopening and explicit Apply install the edited brown definition |
| `native-reopen-brown` | 2 | A second fresh process restores brown under the same saved identity |
| `native-recorder` | 2 | Retained input adds track five, arms track two, mutes track one, solos track two, changes BPM to 122, disables master clock and uses Stop |

The same-identity scenario's viewport pixel comparisons pass unchanged after
Save and changed after Apply. The saved library contains one authored
`theme:copy-1`; the saved application choice points to it in dark mode. All
recorder captures preserve the visible red record dot. The recorder scenario
clicks Record while audio is explicitly unavailable and asserts it stays
inactive; this is a device-free input/model proof, not a recording proof.

The isolated profile has no reachable resident identity. Its pending refusal
is visible and is not counted as successful custody/handoff. The existing
receipt-only audio status line remains clipped below the device controls in
these captures. This slice changes no layout or style source; its recorder
controls and workshop remain usable at the exercised sizes.

## Reproduction and limits

Use the independent `ports/hocket` workspace and its committed lock. Build
`hocket-genet` and `hocket-headless` with Rust 1.97.1. Run the scenarios in the
table through `mere/scripts/run_macos_scenario.py`, prefix `HOCKET`, one private
profile, a new output directory per process and a finite 45-second timeout.
Invoke that script with Python if the cached Git checkout lacks its executable
bit; the first direct-script attempt was refused before any product launched.

Set private `HOCKET_SETTINGS`, `HOCKET_APPEARANCE`, `HOCKET_THEME_LIBRARY`,
`XDG_DATA_HOME`, `XDG_CONFIG_HOME` and a short private `XDG_RUNTIME_DIR`.
`HOCKET_APPEARANCE_RECEIPT=1` requires a scenario and suppresses audio device
use only in this explicit acceptance lane. Ordinary desktop launches continue
to use real audio. `summary.json` records source, binary/scenario/capture hashes,
case/process outcomes, reviewed images and the evidence limits.

This receipt qualifies the current native appearance regression and recorder
input/state flow. It does not qualify microphone capture, audible playback,
calibration, keeper custody, project-file reopen, Windows/Linux native
presentation, updater installation or signed handoff. The close/map failure
paths are deterministic mounted-host proofs with synthetic readback, separate
from the real native pixel path. Nothing is published or released by this lane.
