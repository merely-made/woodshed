# Audio Device Selection Plan

## Goal

Let a performer choose the local input and output device without making
machine-specific hardware part of a shared Hocket project.

## Design

- `hocket-engine` enumerates Firewheel CPAL devices as stable string IDs plus
  display names. `AudioDeviceSelection` keeps those IDs host-local.
- The Genet transport has separate Input and Output dropdowns. `System default`
  remains an explicit option rather than a hidden fallback.
- Changing a device stops the old engine, builds a new engine against the
  selected IDs, restores the click/meter configuration, then re-projects every
  audible layer. Capture-in-progress blocks the change.
- Media retains its capture sample rate. Sampler speed converts to the new
  engine rate so a device change does not alter a stored loop's pitch or duration.
- Device IDs are not saved in a project bundle and do not enter history or sync.
  The catalog is a per-launch snapshot for now; preference persistence and
  hot-plug refresh are later host settings work.

## Done Conditions

- Input and output options come from CPAL at launch and include System default.
- Changing either selection rebuilds the audio engine and restores audible loops.
- Missing or malformed device IDs follow Firewheel's system-default fallback.
- Capture cannot be interrupted by a device change.
- Existing media plays at the same pitch and duration after an output-rate change.

## Progress

- 2026-07-09: **LANDED.** Firewheel device enumeration and selection now back
  Genet transport dropdowns. Focused tests cover selector mapping, malformed
  IDs following the system-default fallback, and source-rate playback speed.
