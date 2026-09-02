# design_docs Index

Canonical first-reference document for project documentation. Read this
before any other doc in this directory.

## Project Reference Docs

- [PROJECT_DESCRIPTION.md](PROJECT_DESCRIPTION.md) — Product goals,
  major features, scope. Maintainer-owned (passed by Mark 2026-05-18).
- [DOC_POLICY.md](DOC_POLICY.md) — Documentation governance.

## Active Plans

- [2026-05-18_initial_plan.md](2026-05-18_initial_plan.md) — Initial
  scaffold, workspace skeleton, and feature-target ladder from
  click-track engine through P2P session hand-off.
- [2026-07-08_genet_host_refactor_plan.md](2026-07-08_genet_host_refactor_plan.md)
  — **LANDED.** UI rebuilt fresh on `xilem_serval` (the one-screen loop-recorder
  design, not a Masonry port) with `chisel` leaf waveforms/meters and the
  `hocket_engine` spine wired in. The Masonry app + the `mark-ik/xilem` fork
  are deleted family-wide (hocket + woodshed). Deferred follow-ups listed in
  the plan's Progress log.
- [2026-07-09_honest-local-session_plan.md](2026-07-09_honest-local-session_plan.md)
  - **LANDED.** Empty-session startup, history-backed track creation, real
  solo/stop behavior, and removal of collaboration/demo affordances that had
  no backing subsystem. Persistence and sync remain separately scoped.
- [2026-07-09_muniment-project-store_plan.md](2026-07-09_muniment-project-store_plan.md)
  - **LANDED.** Local project persistence over Muniment's generic backend seam,
  with a Genet-host Redb API. The manifest retains Hocket's session/history
  semantics and media keeps its existing `MediaRef` identity.
- [2026-07-09_project-controls_plan.md](2026-07-09_project-controls_plan.md)
  - **LANDED.** Native desktop open/save controls over the Muniment store, with
  Armillary moving Redb work off the Genet kernel thread.
- [2026-07-09_loop-export_plan.md](2026-07-09_loop-export_plan.md)
  - **LANDED.** Loop-first WAV export through the existing project worker, with
  explicit behavior for unequal free-capture loop lengths.
- [2026-07-09_free-loop-export-duration_plan.md](2026-07-09_free-loop-export-duration_plan.md)
  - **LANDED.** Explicit musical-bar export for unequal free-capture loops,
  held as host-local export intent rather than project state.
- [2026-07-09_audio-device-selection_plan.md](2026-07-09_audio-device-selection_plan.md)
  - **LANDED.** Per-launch local input/output selection over Firewheel CPAL,
  separate from project persistence and sync.
- [2026-07-10_signed-handoff-envelope_plan.md](2026-07-10_signed-handoff-envelope_plan.md)
  - **PARTIAL.** Signed, complete, recipient-addressed project hand-off bytes
  with durable-sender attestation; raw bytes are not encrypted.
  durable Windows host identity and engine branch acceptance are landed.
  Recipient exchange, carrier, review UI, user-facing acceptance, and branch
  merge remain separate work. Host-side gaps are carried forward in the
  hand-off UI plan below.
- [2026-07-18_handoff-ui_plan.md](2026-07-18_handoff-ui_plan.md)
  - **SCOPED.** The host-facing pass-the-mic gesture: a file carrier over the
  existing project worker, a copyable contact token with auto-addressed replies,
  incoming staging and review in the circle, and same-session or new-session
  acceptance. Cleartext carrier, stated honestly; encryption and a network
  carrier are later.
- [2026-08-08_persona_timeline_design.md](2026-08-08_persona_timeline_design.md)
  - **DESIGN SKETCH** (from Mark, no implementation planned). A persona as a
  lens over the session: click a participant, see their contributions laid out
  linearly with breaks where the turn passed, plus a combined loopline of
  everyone's turns in order. Grounds the concept in what the model records
  (authorship exists only in the signed hand-off envelope; `HistoryNode` and
  `Layer` carry no author, so the missing piece is a turn mark written at
  accept time from the verified envelope), names the owning layers (scenograph
  timeline scene, platen/forme arrangement, sprigging regions; hocket supplies
  the musical reading), and draws the doctrine line: provenance reading yes,
  editing from the timeline is the arrange-view canary.
- [2026-08-06_update_settings_persistence_plan.md](2026-08-06_update_settings_persistence_plan.md)
  - **LANDED.** Update policy becomes a stored device setting. `UpdateSettings`
  persists to `update-settings.json` through `UpdateSettingsProvider` (atomic
  tmp + rename) and is described at the `pelt/update` reference on Genet's
  settings contract; the app and `--update-now` load the same file, so they
  cannot disagree. Retires the interim `HOCKET_UPDATE_POLICY` env var, which is
  no longer read; `HOCKET_SETTINGS` overrides the file for isolated runs.
  Hocket's slice of Mere's cross-product
  [configuration ownership umbrella](../../mere/design_docs/mere_docs/implementation_strategy/2026-08-06_configuration_ownership_settings_projection_plan.md)
  (its C5).
- [2026-07-24_auto-update_plan.md](2026-07-24_auto-update_plan.md)
  - **H1-H4 (Windows) LANDED**; its interim `HOCKET_UPDATE_POLICY` env setting
  retired 2026-08-06 by the update-settings plan above. Configurable auto-update: a platform-neutral
  policy/status layer (four real policies, a status variant per real state,
  no placebo spinner) over an `UpdateTransport` seam, driven by an armillary
  worker off the UI thread. Default transport is **luggage** (mere's
  `cargo-packager-updater` fork); Velopack stays selectable as the A/B.
  Proven end to end on Windows: an installed 0.1.0 updated itself to 0.2.0
  through a signed feed, and a tampered artifact was refused. macOS and
  Linux legs remain, plus the in-app action buttons and OS install-trust
  signing.
- [RELEASING.md](RELEASING.md)
  - How to cut a Hocket release: `cargo-packager` + `luggage-manifest`, the
  signing key, per-host formats, and the two silent packaging traps that
  will otherwise ship a mislabelled binary.
- [2026-07-10_history-branches_plan.md](2026-07-10_history-branches_plan.md)
  - **LANDED.** Retained history branches, cross-branch checkout, and validated
  same-root graph integration; conflicting-edit reconciliation remains open.
- [2026-07-11_real_waveforms_meter_ballistics_plan.md](2026-07-11_real_waveforms_meter_ballistics_plan.md)
  - **LANDED.** Real cached summed/per-layer waveform projections through
  responsive Chisel leaves, plus shared configurable meter ballistics.
- [2026-07-14_open-project-format_plan.md](2026-07-14_open-project-format_plan.md)
  - **DOCTRINE + FULLY LANDED.** A `.hock` file must be openable and its material
  importable without Hocket — no lock-in. A `.hock` is a zip of `manifest.cbor`
  (CBOR structure) + `media/<hash>.wv` (lossless WavPack via wavicle) +
  `meta.json` (provenance), over a Muniment `ZipBackend` (the seam was kept, not
  dropped). Hardened after an adversarial review. FLAC was evaluated and rejected
  (it cannot hold `f32` audio both losslessly and importably); WavPack can, and a
  pure-Rust codec for it (wavicle) was built to do it.
- **WavPack codec — founded as wavicle, wired into Hocket.** The pure-Rust
  WavPack codec was founded 2026-07-15 as **wavicle**
  (github.com/merely-made/wavicle). Its plan lives in that repo's `design_docs/`; the
  pre-founding copy is archived at
  [archive_docs/2026-07-15/](archive_docs/2026-07-15/2026-07-15_wavpack_codec_plan.md).
  M6 landed 2026-07-18: `project_store` media is now `.wv` via
  `wavicle::encode_float`/`decode_stream`, with `MediaRef` still BLAKE3 over the
  decoded f32 samples. WAV is retained only for the offline mix export.

## Archive

- `archive_docs/` — retired plans and superseded notes (created on
  first archive).

## Working Principles for AI Assistants

These principles apply to AI-assisted work on this project. Update this
section whenever a durable working insight emerges from a session.

- **The model is framework-agnostic.** `hocket-model` does not depend
  on cpal, xilem, masonry, or any UI/audio framework. The audio
  engine, the UI, and the sync layer all consume it as a peer.
- **The UI rides Genet, not Masonry (from 2026-07-08; current host 2026-09-02).**
  The active host is `cambium-genet-winit-host`, with chisel leaves for waveform
  and meter drawing. Structure is native Cambium views plus tinct CSS. The
  Masonry application and fork were retired; the audio spine remains
  independent. See
  [2026-07-08_genet_host_refactor_plan.md](2026-07-08_genet_host_refactor_plan.md).
- **Async-first collaboration**, never real-time multiplayer jamming.
  The product's identity is sequential turn-taking over Moothold; that
  shape must not erode into "everyone plays together over the net" as
  a primary mode.
- **Hand-on-instrument flow.** Borrowed from Deeler's design north
  star. UI decisions are evaluated against whether they let the
  musician keep their hands on the instrument. Big buttons, minimal
  modes, count-in countdowns, no clicking-around-to-arm-a-track
  ceremonies.
- **Defaults, not limits.** The looper-pedal default is four tracks with
  layered overdub. Deeler is a named ten-track, SelectOne profile. Track count,
  phrase length, capture settings, and playback mode remain session settings;
  use `Vec` and runtime-known counts in the model.
- **"Pass the mic" is the north-star metaphor.** The collaboration
  model is the digital equivalent of passing a mic around in a circle
  and building loops turn by turn. UI and protocol decisions should
  honor that — sequential, polite, asynchronous. Latency is not our
  friend; we don't fight it, we route around it.
- **Strict cap doctrine — loop-first, not DAW.** Every proposed
  feature must answer: *does this serve the passing-the-mic
  workflow?* If yes, consider taking it. If it serves traditional
  DAW workflows instead, defer it or skip it. The arrange view is
  the single feature that could turn Hocket into something a person
  uses *instead of* a DAW — add it deliberately and only after the
  loop-recorder identity is solid. The failure mode to avoid: "loop
  recorder that grew DAW features and now is worse than both."
- **Firewheel is the audio substrate.** Not Dropseed (back-burner),
  not a SequencerEngine wrap (placeholder from FT1). At FT3b-prime
  the engine moves to a Firewheel graph: loop tracks as sample-player
  nodes, mix bus, input capture node, optional click node. **On
  crates.io** at `firewheel-graph` 0.10.x — pin a version, only
  path-dep if unreleased fixes are needed.
- **hocket-model is the authority; Firewheel is the runtime.**
  Session truth lives in the model; the audio graph plays the
  *projected state*. Don't let runtime concerns leak back into the
  model (no transport-dominance, no graph topology bleed-through).
- **No plugin hosting for v1.** No CLAP, no VST3, no LV2, no AU, no
  plugin GUI hosting. Curated first-party Rust devices on Firewheel
  nodes with schema-driven UI generated from parameter metadata.
  Drags DAW-shaped product gravity if we say yes; preserves
  loop-recorder identity if we say no. v1 says no.
- **Layers are primary.** A track is an append-only stack of captured layers.
  Looper sessions sum unmuted layers; the Deeler profile selects one existing
  layer. Do not introduce a second variation-slot data structure.
- **Local-only transport for v1.** Each peer has its own playback
  head. Recording lock prevents two peers from capturing into the
  same track on the same turn; CRDTs apply to shared session structure
  (track existence, turn ownership, mute, locked BPM), never to audio
  data. Solo is local monitoring state.
- **No lock-in — the project file is a container, not a cage.** A `.hock`
  file must be openable, and the audio inside importable, without Hocket or a
  bespoke converter. The specialty-project-format norm (opaque, app-only) is a
  failure mode we reject as product identity, alongside plugin gravity and
  DAW scope creep. Long-term target is a zip container of standard-audio media
  plus documented structure. Realized 2026-07-14: a `.hock` is a zip of
  `manifest.cbor` (self-describing CBOR) + `media/<hash>.wav` (standard audio),
  over a Muniment `ZipBackend`. FLAC and a provenance entry are follow-on. See
  [2026-07-14_open-project-format_plan.md](2026-07-14_open-project-format_plan.md).

## Reference reading

Hand-picked technical references that inform Hocket's architecture:

**Realtime audio engineering:**
- Bencina, ["Real-time audio programming 101: time waits for nothing"](http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing)
- Bencina, ["Interfacing Real-Time Audio and File I/O"](http://www.rossbencina.com/code/interfacing-real-time-audio-and-file-io)
- Renn-Giles + Rowland, "Real-Time 101 Parts I & II" — ADC19 (YouTube)
- Doumler, "Using Locks in Real-Time Audio Processing, Safely" — ADC20
- Doumler, "What is Low Latency C++?" Parts 1 & 2 — CppNow 2023
- Doumler, "Thread synchronisation in real-time audio processing with RCU" — ADC
- Renn-Giles, "Real-Time Confessions in C++" — ADC23

**BillyDM blog (read in order for the Dropseed→Firewheel arc):**
- [DAW Frontend Development Struggles (2023-02)](https://billydm.github.io/blog/daw-frontend-development-struggles/)
- [Why I'm Taking a Break from Meadowlark (2023-04)](https://billydm.github.io/blog/why-im-taking-a-break-from-meadowlark/)
- [Clarifying Some Things (2023-05)](https://billydm.github.io/blog/clarifying-some-things/)
- [Rust vs C++ (2023-06)](https://billydm.github.io/blog/)
- [Accurate Timekeeping in a DAW (2022-11)](https://billydm.github.io/blog/) — directly relevant to optional-master-clock design

**Architecture references:**
- [Firewheel design doc](https://github.com/BillyDM/Firewheel/blob/main/DESIGN_DOC.md)
- [CLAP spec](https://github.com/free-audio/clap) — headers themselves are the spec
- **Share with Woodshed, don't fork.** Hocket consumes focused shared crates
  such as `audio-primitives` via path dependency. Keep application-level
  dependencies one-way and extract further only when both projects need them.
- **Merely family naming.** Crates use `hocket-*`. The bare `hocket` name is
  claimed on crates.io (0.0.1, 2026-07-14), so the umbrella name is ours to
  publish under when we want it.
- **The project was named Strophe until 2026-07-14.** A hocket splits one
  melodic line between voices, each sounding while the others rest — the
  pass-the-mic model in a word. The rename also freed us from the bare
  `strophe` crate name, squatted since 2016 by a dead redirect. Durable
  strings changed with it: the project extension is `.hock`, the bundle
  keys are `hocket/manifest` + `hocket/media/`, the hand-off salt is
  `hocket/handoff/v2/`, and the host identity lives under a `Hocket` data
  root. A one-time migration re-seals a pre-rename local identity so an
  existing fingerprint survives; no other legacy path is kept.
