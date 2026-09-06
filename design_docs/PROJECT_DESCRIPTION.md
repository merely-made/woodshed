# Project Description

- Hocket is a cross-platform loop recorder with turn-based collaboration.

## What it is

It is
inspired by [Deeler](https://new.tapeop.com/interviews/47/menomena), the Max/MSP patch the band Menomena built and used
to write their albums, and extends that workflow with peer-to-peer
session sync, content-addressed media, and nondestructive history.

The core unit of work is a **phrase**: a short recording (four
measures by default, but per-track variable-length) made against an
optional click track. A session has a set number of **mono tracks**
(four by default, collaborator-scaled). Each track is a **stack of
layers** — every capture appends a new layer to its track, and layers
sum at playback (the looper-pedal / overdub model). Layers are
individually muteable and gain-adjustable; a **mix-down** gesture
consolidates several layers into a single new phrase when desired.
Songs emerge from the layered material: mute layers in or out, export
per-track or as a mix.

The collaboration model is **asynchronous sequential overdubbing**: one
member captures a layer on a track, hands the session off to the next
member, who responds by capturing a new layer on their own track in
light of what they heard.
I'd like this to be the digital equivalent of passing
a mic around in a circle and building loops turn by turn!

## Goals

- **Async-first collaboration** for distributed bands. The workflow is
  pass-the-mic, not synchronous-jam. Real-time presence is at most a
  side-channel awareness feature.
- **Hand-on-instrument flow.** Borrowed from Deeler's design north
  star: "minimize the interactions with the computer and try to
  preserve the continuity of the groove." Every UI decision tests
  against whether it keeps the musician's hands on their instrument.
- **Nondestructive everything.** Layers are immutable takes. Edits
  are nodes in an append-only history graph. Branches and suggestions
  reconcile via CRDT-shaped merges of *session structure*, not of
  audio buffers (which are content-addressed and travel as blobs).
- **Peer-to-peer by default.** Session state and media sync via
  Moothold/Murm — no centralized server, no account required.
- **Native and web from day one** *(web is a gated proof, not a free
  capability — see Tech Stack)*. Xilem + xilem_web for UI;
  [Firewheel](https://github.com/BillyDM/Firewheel) for the audio
  graph, with cpal on native and Web Audio / AudioWorklet on the web
  via Firewheel's web backend.

## Non-Goals

- Linear arrangement-first DAW workflow (Ableton/Bitwig/Logic). The
  arrangement is downstream of the loop library, not the primary
  surface.
- Mixer-first workflow with deep bus/send/insert structure. Hocket is
  a phrase sampler; mixing happens later, elsewhere, by exporting
  loops or arrangements.
- Real-time multiplayer jamming over the network as the primary composition model. Sequential
  overdubbing is the model because latency is not our friend.
- **Plugin hosting of any kind in v1** — no CLAP, no VST3, no LV2, no
  AU. Curated first-party Rust devices on Firewheel nodes, with
  schema-driven UI generated from parameter metadata. Drags
  DAW-shaped product gravity if we say yes; preserves loop-recorder
  identity if we say no. v1 says no.
- Notation rendering, advanced MIDI editing, complex automation lanes
  in v1 at least.

## Major Features

hocket should be configurable: track count, layer semantics (sum vs. select), 
bar length, count-in behavior, click preference, and other session-level settings.
The simple looper default for v1 is the looper-pedal profile: 
4 tracks, layered overdub, variable-length, optional master clock. 
A Deeler profile ships alongside as a named preset 
(10 tracks, 4 variation slots per track with select-one semantics, 
fixed bar length, click-driven) for users 
who want exactly that workflow (like myself).

### Session

- 4 mono tracks (configurable, collaborator-scaled)
- Each track is a stack of layers, append-only, individually muteable
- Variable-length loops with an *optional* session master clock
- Click track / metronome — first-class when the master clock is on
- Tempo and time signature stored at session level (informational
  when the master clock is off)

### Recording

- Big record button + count-in (count-in only when master clock is on)
- Per-track arm / disarm
- New captures append a new layer to the target track
- Input monitoring with latency-compensated playback
- "Consolidate to bar grid" gesture for aligning a recorded phrase
  to the master clock when desired (like Ableton's Looper / OP-1)

### Play / Arrange

- Mute / solo / gain-adjust layers in real time
- "Mix down" gesture: collapse N layers on a track into a single new
  phrase + replace them with one layer referencing it
- Export per-track and per-mix as WAV
- Sequence + arrangement views are out of scope for the initial loop
  recorder (see strict-cap doctrine in `design_docs/DOC_README.md`)

### Collaboration

- Session hand-off: push session state to a peer (recording-lock model
  — only one peer captures into a given track per turn)
- Branch / suggest / accept on the history graph
- Content-addressed media (BLAKE3); deduplicated across peers
- **Local-only transport for v1** — each peer's playback head is
  independent. Real-time WAN-synced transport is a separate research
  project, not v1.
- Per-region comments (later)

### Devices (first-party, no plugin hosting)

- gain, pan, meter (early)
- 3-band EQ, send reverb, delay (high-value/low-cost extension)
- Compressor / limiter, filters
- Schema-driven UI controls (knobs/faders/toggles) generated from
  device parameter metadata — no per-device hand-coded plugin GUIs

## Distribution Plan

1. itch.io / Gumroad (desktop: Windows, macOS, Linux)
2. Self-hosted web build (free, PWA-installable)
3. Google Play (Android — Xilem ships Android examples)
4. Apple App Store (iOS — via PWA initially, native later)

## Tech Stack

- **Language**: Rust
- **UI**: Cambium (formerly `xilem_serval`) hosted on Genet, with Chisel retained vector leaves
  for waveform and meter geometry
- **Audio engine**: [Firewheel](https://github.com/BillyDM/Firewheel)
  (BillyDM's modular audio graph engine, "wgpu but for audio"). Native
  + WASM backends. On crates.io as `firewheel-graph` 0.10.x.
- **Audio I/O**: cpal on native and Web Audio / AudioWorklet on web,
  both via Firewheel's bundled backends. Web is a *gated proof, not a
  free capability*: requires wasm threading/atomics, nightly +
  `build-std`, and COOP/COEP deploy headers.
- **Sample loading / disk streaming**: symphonium + creek
  (BillyDM-maintained crates that pair with Firewheel)
- **Realtime-safe primitives**: rtrb + basedrop
- **Persistence**: content-addressed sessions; postcard today, ciborium
  (CBOR) at FT8 to align with Moothold
- **P2P sync**: Moothold + Murm (sibling projects in the Merely
  family); content-addressed audio blobs travel separately from
  session structure

## Relationship to Woodshed and Mere

Hocket is the **pressure vessel for the reusable audio layer** across
the Merely family. Hard audio engineering happens in Hocket first;
when pieces stabilize, they get extracted to shared crates that
Woodshed and Mere consume independently. Planned shape:

- `audio-primitives` — waveform peaks, meter ballistics, onset / tap tempo,
  latency calibration, and other pure sample/timing kernels
- audio Chisel leaves — incubate in Hocket and promote only when Woodshed or
  another consumer needs the same retained geometry
- `audio-devices` — curated Firewheel node wrappers (gain/EQ/reverb/
  delay/compressor) + parameter schemas

Hocket itself keeps the product-specific bits — layered loop model,
turn-taking, hand-off protocol, branch/merge UX — which never leave
the Hocket crate.

Dependency direction is one-way: Hocket → shared audio crates;
Woodshed and Mere → shared audio crates. Hocket is not "Mere's audio
subsystem," and Woodshed is not "Hocket-lite." Different products,
overlapping audio primitives.

Extraction trigger: when first duplication appears between Hocket
and Woodshed (or Hocket and Mere). Don't pre-emptively create empty
crates.
