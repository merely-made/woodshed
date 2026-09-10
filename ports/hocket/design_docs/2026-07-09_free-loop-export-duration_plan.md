# Free-Loop Export Duration Plan

## Goal

Let a performer explicitly export unequal free-capture loops for a selected
musical duration, without turning the session model into an arrangement
timeline.

## Design

- `OneCycle` remains the default and still rejects unequal audible loop
  lengths. It is the honest choice for a clocked session with a shared loop.
- `Bars(n)` is a host-local export setting. It repeats each audible loop until
  `n` bars at the session BPM and time signature are rendered.
- One shared `audio-primitives` calculation drives the click loop,
  bar-aligned capture, and export frame length. It includes the time-signature
  denominator, so a bar of 3/8 differs from a bar of 3/4 at the same BPM.
- Export duration travels with the Armillary worker command only. It is not
  written to the project bundle or represented in history because it does not
  change session truth.
- The header presents a segmented Cycle/Bars choice; Bars exposes a compact
  stepper initialized from the current session phrase-bar setting.

## Done Conditions

- A free session with unequal audible loops can export a selected bar duration.
- One-cycle export continues to reject unequal loop lengths.
- Exported frame counts match the session BPM and full time signature.
- Duration changes do not dirty, save, or synchronize a project.

## Progress

- 2026-07-09: **LANDED.** `hocket-engine` has typed duration policies and
  focused meter-aware rendering tests. The Genet header exposes Cycle/Bars
  controls and the project worker snapshots the selected policy for WAV export.
  The shared bar-frame helper also corrected the engine's old numerator-only
  clock calculation.
