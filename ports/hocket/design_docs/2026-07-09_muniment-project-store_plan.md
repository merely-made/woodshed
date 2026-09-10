# Muniment Project Store Plan

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
