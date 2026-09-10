# Real Waveforms and Meter Ballistics Plan

## Status

**LANDED 2026-07-11.**

## Goal

Replace plausible waveform stand-ins with projections of stored audio, and
give the live output meter frame-rate-independent attack, release, and peak
hold without putting host display policy into project state.

## Boundaries

- `audio-primitives` owns pure signed min/max peak extraction and configurable
  normalized meter ballistics. It depends on neither Firewheel nor Chisel.
- `hocket-engine::waveform` owns Hocket track/layer projection. Track views
  reuse export's source selection and repeating-loop renderer, so gain, mute,
  `Sum`/`SelectOne`, sample-rate validation, and free-loop repetition agree.
- `hocket-genet` owns projection caches, model-id-to-leaf keys, responsive
  Chisel composition, missing-media presentation, and host-local timing.
- Chisel owns retained vector leaves and registry lifecycle only. It does not
  learn audio samples or Hocket's model.

## Done Conditions

- Known signed samples and impulses produce exact min/max columns.
- Summed track peaks match real playback/export semantics.
- Per-layer rows use real media and remain visible while muted.
- Missing media produces an explicit unavailable state rather than fake peaks.
- Stable track/phrase keys survive display reordering, removed leaves are
  released, and an unchanged waveform signature produces zero repaints.
- Meter level and peak marker use configurable attack/release/hold timing.
- Summed and layer waveforms fill their available width.
- A headed Windows run can arm and capture a layer, display its real overview,
  and animate the output level/peak without an AccessKit startup panic.

## Sidequest Landed

Hocket now builds and installs its initial accessibility tree while the native
window is hidden, then reveals the window. Genet's `AccessKitBridge` docs now
state the Windows pre-show installation contract for other `xilem_serval` hosts.

## Remaining

- Offer raw-amplitude and visually normalized waveform display modes as a
  host-local setting; normalization must not alter stored media or mix gain.
- Persist or expose meter timing in a settings surface if users need profiles.
- Add peak-file pyramids when projects outgrow inexpensive in-memory overview
  generation.
- Promote the Hocket-owned audio leaf only after Woodshed or another app has a
  concrete Chisel waveform consumer.
- A zoomed editable waveform is a separate Path-B product feature.
