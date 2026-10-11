# Muniment Project Store Plan

**Status (2026-10-11): original store landed; portable project extraction passes
its CPU gates; combined host acceptance follows integration.**

## Goal

Give Hocket one portable persistence path before adding save/open controls,
export, or peer transfer. Reuse Muniment's backend seam rather than create a
second filesystem protocol inside Hocket.

## Design

- `hocket-model::ProjectBundle` remains the versioned session/history manifest.
- `hocket-engine::ProjectStore<B>` stores that manifest at `hocket/manifest`
  and audio at `hocket/media/<MediaRef>` over a Muniment `Backend`.
- `MediaRef` remains Hocket's sample-rate-aware BLAKE3 identity. Muniment's
  generic `BlobStore` is intentionally not used directly because it hashes raw
  encoded bytes and would create a second identity for the same capture.
- Saving fails before writing when a manifest references unavailable media.
- Loading retains the session and reports missing media blobs. Those layers stay
  silent until their content arrives.

## Done Conditions

- Manifest schema version is explicit and rejects unknown versions.
- A generic store round-trips a manifest and captured audio through
  `MemoryBackend`.
- Missing media has distinct save and load behavior.
- Corrupt media is rejected rather than silently played.
- A future Genet host can choose Redb on desktop or OPFS in a browser without
  changing Hocket model or media semantics.

## Progress

- 2026-07-09: **LANDED.** Generic storage tests and Genet-host Redb save/open
  API pass. The local rail reports unavailable media after an open. Remaining
  host work is user-facing project selection and save/open controls.
- 2026-07-14: **Backend superseded.** The Muniment seam and this store's
  session/history/`MediaRef` semantics are unchanged, but the desktop `.hock`
  backend moved from redb to a new Muniment `ZipBackend`, and media now stores as
  WAV, so a `.hock` file is an openable zip. See
  [2026-07-14_open-project-format_plan.md](2026-07-14_open-project-format_plan.md).

## Portable project seam (2026-10-11)

The October 11 family audit found durable project operations coupled to the
audio driver. `hocket-project` now owns media, project storage, offline loop
export, signed handoff and waveform projection. `hocket-model` retains the authoritative schema,
history and sample-rate-aware MediaRef. The host imports those operations
directly rather than going through an audio engine; no forwarding modules
remain in the driver.

The extraction preserves the zip/CBOR/WavPack format and envelope bytes. Export
uses the shared pure-DSP `frames_per_bar` function already used by Hocket's
click driver, avoiding a reverse dependency on the engine. Existing codec,
missing/corrupt-media, signed-recipient/branch, export-duration and capture
pipeline tests move with the implementation.

Done when the project package passes its moved tests and relevant host tests,
its normal dependency tree contains neither CPAL nor Firewheel, and desktop
save/open/export paths still compile and pass their current-source gate.
Native lifecycle/capture work is independently owned by the host lane.

**Findings (2026-10-11):** all five extracted modules depend on the model,
host-supplied byte/identity seams, codec and pure DSP; none uses the device
runtime. The sole export dependency on the engine was its thin frame-duration
wrapper around `audio-primitives`.

**Progress (2026-10-11):** modules and their tests moved, host imports rewired,
engine-only normal dependencies reduced to model/DSP/Firewheel, and lockfile
updated without external source/version changes. The 30 project tests and 54
model/engine/capture/history tests pass. The normal project dependency tree
contains neither CPAL nor Firewheel. Combined desktop validation follows host
lane integration; native project save/open/export acceptance remains separate.
