# Loop Export Plan

## Goal

Export the current audible loop mix as a WAV without introducing an arrangement
timeline or silently inventing a duration for free captures.

## Design

- The engine renders from model state plus the host-local solo set. Track mute,
  layer mute, SelectOne, gain, and missing media retain their live semantics.
- Default export renders exactly one shared cycle and rejects unequal loop
  lengths. A later duration control can call the explicit-frame renderer.
- The mix is mono source material duplicated to stereo float WAV until pan or
  stereo capture exists.
- Export runs through the existing Armillary project worker and native save
  dialog, never on the Genet kernel thread.
- Clocked capture uses `Session::bars_per_phrase`, so phrase metadata now agrees
  with the recorded loop length.
- The clock, capture target, and export bar duration use the full session time
  signature through `audio-primitives`.

## Done Conditions

- Equal-length audible loops render to a stereo WAV.
- Gain, mute, solo, and SelectOne alter exported samples exactly as playback.
- Unequal loops fail with an actionable export error instead of truncating.
- Export is available from the host header and rejects capture-in-progress.

## Progress

- 2026-07-09: **LANDED.** The engine renders and writes a stereo float WAV;
  the Armillary worker has a focused export round-trip test. The host compiles
  with the native Export mix control and clocked capture now honors the session
  phrase-bar setting.
