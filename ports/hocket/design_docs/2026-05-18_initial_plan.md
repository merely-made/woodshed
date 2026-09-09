# Initial Plan

Scaffold the workspace, then incrementally build a Deeler-inspired
collaborative loop recorder. Feature targets are ordered by
*unblocking* — each lights up capabilities the next depends on. No
calendar estimates; validation is stated as done conditions.

## Strategic decisions

These are commitments that shape every Feature Target below. Update
this section (not the FTs) when direction changes.

### Identity and scope

- **Hocket is a loop-first composition tool, not a DAW.** The
  passing-the-mic-in-a-circle workflow is the protection against
  feature drift. Every proposed feature must answer: *does this serve
  the passing-the-mic workflow?* — if yes, consider; if it serves
  traditional DAW workflows instead, defer or skip.
- **The arrange view is the canary for scope creep.** Adding it
  changes what Hocket *is*. Add it deliberately and only after the
  loop-recorder identity is solid, or you'll have a product that's
  neither thing well. (See DAW extension ladder below.)
- **Layered tracks with per-track playback mode.** A track is a stack
  of layers (each layer = one captured sample, append-only,
  individually muteable). How those layers play is controlled by a
  per-track `PlaybackMode`:
  - **`Sum`** (the *looper-pedal profile* default): all unmuted layers
    play simultaneously and sum at the track mixer.
  - **`SelectOne { active }`** (the *Deeler profile* default): exactly
    one layer is audible at a time; switching `active` is the
    variation-picking gesture. Other layers are dormant, not muted.
  Profiles are session-construction-time choices recorded as
  `Session::default_playback_mode`; per-track mode can change at
  runtime via `Edit::SetTrackPlaybackMode`. Faithful Deeler workflow
  requires `SelectOne`; the looper-pedal workflow uses `Sum`. Both
  ship in v1.
- **Default track count is 1 per collaborator** (looper profile,
  scalable). Deeler profile defaults to 10 tracks. Variable-length
  per track, with optional master clock at session level.
- **Asynchronous turn-taking, never realtime jamming.** Sequential
  hand-offs of session state via Moothold. Sub-100ms WAN-synced
  realtime transport is a separate research project, not v1.

### Audio engine

- **Firewheel as the substrate** ([github.com/BillyDM/Firewheel](https://github.com/BillyDM/Firewheel)).
  BillyDM pivoted from Dropseed to Firewheel — a modular audio graph
  engine, deliberately *not* a DAW engine. Better fit for a loop
  recorder than Dropseed would have been. Active, 702 commits, dual
  MIT/Apache-2.0, "no mutexes" realtime constraints, ships native +
  WASM backends.
- **Firewheel is on crates.io.** `firewheel-graph` at 0.10.2 as of
  2026-03-17. Pin a crates.io version; only path-dep if we need
  unreleased fixes. *(Prior plan text said "not yet on crates.io" —
  that was wrong. Corrected here.)*
- **hocket-model is the authority; Firewheel is the runtime.**
  Session, tracks, layers, history, turn state, media refs all live
  in `hocket-model`. Firewheel just plays/captures/mixes the
  currently projected state. This matches Firewheel's "app/game state
  sends events into the audio graph" model far better than a
  transport-dominant DAW engine would.
- **The current `SequencerEngine` wrap is a placeholder.** Survives
  from FT1's click engine. **Replaced at FT3b-prime** (not FT3b) —
  pivot now, before FT3b hardens around the Woodshed engine boundary.
- **No plugin hosting for v1.** No CLAP, no VST3, no LV2, no AU, no
  plugin GUI hosting. Hocket v1 ships curated **first-party Rust
  devices** on Firewheel nodes, with **schema-driven UI** generated
  from parameter metadata. firewheel-extra's CLAP host is marked
  TODO in Firewheel's own design doc; CLAP isn't possible in WASM
  anyway; plugin hosting drags DAW-shaped product gravity Hocket
  shouldn't have. See `memory/project_hocket_no_clap_doctrine.md`.
- **Web is gated, not free.** `firewheel-web-audio` needs wasm
  threading/atomics, nightly + `build-std`, and COOP/COEP deploy
  headers. Real, but a proof gate — flagged for FT11.

### Collaboration model

- **Recording lock per track per turn.** CRDTs are wrong for audio
  data; they're right for *session structure* (track existence, turn
  ownership, mute/solo state, master clock if locked). Only one peer
  captures into a given track on a given turn.
- **Local-only transport for v1.** Each peer has its own playback
  head. Other peers' positions show as indicators, not as audible
  sync.
- **Branches map to Moothold IndexCommits.** Merge UI surfaces
  "Mark recorded A while Alice recorded B → keep both / pick one /
  layer both." Audio blobs are content-addressed and travel
  separately.

### Crate stack

Aligned with Moothold and Mere where applicable. Per Mark's
2026-05-18 update:

| Layer            | Crate                       | Notes |
|------------------|-----------------------------|-------|
| Audio engine     | firewheel (path-dep)        | The spine. Not on crates.io yet. |
| Audio I/O        | cpal (via firewheel)        | Native + Web Audio out of the box. |
| Sample loading   | symphonium                  | BillyDM's symphonia fork. |
| Disk streaming   | creek                       | For tracks larger than RAM. |
| Resampling       | fixed-resample / rubato     | fixed-resample is RT-safe; rubato more flexible. |
| Lock-free queues | rtrb                        | SPSC. |
| Memory mgmt      | basedrop                    | Deferred-drop on GC thread. |
| MIDI             | midir                       | Later — initial v1 is audio-only. |
| WAV I/O          | hound                       | For export. |
| UI               | xilem + masonry             | Linebender stack. |
| Rendering        | vello + parley              | Aligned with Mere. |
| P2P / sync       | iroh (via Moothold)         | Same as Moothold. |
| Crypto           | ed25519-dalek + blake3      | Same as Moothold. |
| Serialization    | ciborium (CBOR)             | Same as Moothold. **postcard survives until the FT8 migration.** |
| Web build        | wasm-bindgen + vite         | Like gpui-component's story-web pattern. |

**Explicitly not in stack:** nih-plug (writing plugins, not hosting),
JACK-specific (cpal handles PipeWire), symphonia directly (go through
symphonium), CLAP host crates (until firewheel-extra is ready).

### DAW-feature extension ladder

Ordered by ratio of value to scope-creep. **Strict cap: stay
loop-first.**

- *High value, low cost:* per-track gain envelope, send/return reverb,
  3-band EQ, individual-track + mix export
- *Medium:* loop quantization to grid (requires master clock),
  crossfade between layers, MIDI loop capture (requires synth)
- *High value, high cost — defer:* plugin hosting (CLAP via
  firewheel-extra), arrange view (this is the boundary), comping
- *Don't:* piano roll, plugin GUI hosting, VST3, video sync,
  surround/spatial

### Shared crates plan

Two shared crates live in the woodshed repo, consumed by both Woodshed
and Hocket (cross-repo path-deps). **Extract when there's something
to share.**

- **`audio-widgets`** — the UI layer (Masonry/Vello). As of 2026-05-19
  ships `waveform_view` + `compute_peaks` (extracted at FT5) and a
  `theme` module (spacing `SP_*`, type scale `TS_*`, `mono_family()`,
  base `Palette` + `ThemeMode`). `hocket-widgets` re-exports both.
  Product-specific colors (waveform fill, fretboard) stay per-host.
  Future: fader / knob / meter / transport-button.
- **`audio-primitives`** — the pure-DSP layer (zero deps, no engine/UI).
  As of 2026-05-20 ships `click` (synthesis), `onset` (`OnsetDetector`
  + `estimate_bpm`), `calibration` (`estimate_latency_from_pairs`).
  Drivers (cpal `Analyzer`, Firewheel graph, live calibration session)
  stay in the consuming crate. Future candidates: WAV I/O (`SampleBank`,
  Hocket FT8), MIDI clock sync.

The doctrine's third name, `audio-devices` (cpal/MIDI I/O), is not yet
extracted — Woodshed is cpal-direct and Hocket is on Firewheel, so the
device layer hasn't found a shared shape worth factoring out.

### Visual design conventions (apply during UI work)

- 4 px base spacing unit; only multiples allowed
- Typographic scale (1.125–1.2× ratio), smallest size for secondary
  info, monospace for changing numeric values
- 1 px low-contrast borders, single-step shadows for active/hovered
  states only, no gradients
- Tween *values* (faders, meters), never *widget boxes*
- Disabled / muted / hover / active palette defined before building
  components
- Theme primitive: `Palette { bg, surface, surface_2, text, text_dim,
  accent, accent_text, danger, success, ... }`, no raw `Color::rgb`
- Hit targets larger than visuals (4 px fader thumb → 24 px hit area)
- Double-click any meter/fader value text to edit numerically

## Plan

### Feature Target 0: Workspace scaffold

Establish the workspace and a buildable skeleton with no functional
behavior yet.

**Tasks:**
- Workspace `Cargo.toml` with five crates: `hocket`, `hocket-engine`,
  `hocket-model`, `hocket-widgets`, `hocket-xilem`
- Each crate has a stub `lib.rs` (or `main.rs` for the binary) that
  compiles
- Path-dep on sibling `../woodshed/crates/woodshed-audio` wired but
  not yet consumed
- Path-deps on local xilem checkout matching woodshed's pins
- LICENSE files, README, CLAUDE.md, design_docs/

**Validation:**
- `cargo build` succeeds workspace-wide on Mark's primary Windows
  laptop
- `cargo run -p hocket` prints a placeholder line and exits cleanly
- No `unsafe`, no warnings

### Feature Target 1: Click-track engine

The foundation of every Deeler workflow is the click. Get a metronome
running through cpal with sample-accurate scheduling and a configurable
tempo + time signature + bar count.

**Tasks:**
- `hocket-engine`: cpal output stream, voice mixer, click generator
  (reusing or wrapping `woodshed-audio`'s `Sound::Click` if direct
  reuse is clean; otherwise local)
- `Transport` type: tempo, time signature, bars per phrase, playing
  state
- Sample-accurate click scheduling — clicks land on bar/beat
  boundaries to within 1 sample at 48kHz
- Engine boundary: `Send`-able control handle. (Originally specified
  as lock-free rtrb command/event channels; deferred to Feature Target
  3 where audio-thread contention actually matters. See Findings.)

**Validation:**
- Audible click at 120 BPM, 4/4 — clicks loop forever ✅
  (maintainer-confirmed by ear 2026-05-18)
- Tempo change at runtime does not pause playback ✅ (proven by
  `woodshed-audio`'s existing `bpm_change_does_not_pause_playback`
  test; reused here by composition; confirmed by ear 2026-05-18 —
  smooth 120→90 BPM transition)
- Time signature change (3/4, 5/4, 7/8) sounds correct ✅
  (maintainer-confirmed 7/8 eighth-note clicks 2026-05-18; pattern
  restart on time-sig change is acceptable for v0)
- CPU stays under 5% on the Windows laptop — to be measured next
  session via Task Manager during a long-running demo (not blocking)

### Feature Target 2: Hocket-model (Tracks + Layers + History)

The nondestructive data substrate. `hocket-model` ships before the
recording surface so the engine has somewhere to put captured audio.

**Tasks (as landed):**
- `Phrase`: content-addressed media reference + length in bars +
  tempo + capture timestamp
- `Layer`: `phrase_id` + `gain` + `muted`. **No fixed slot count.**
  Variations as a separate concept don't exist; multiple takes are
  layers, distinguished at playback by per-track `PlaybackMode`.
- `PlaybackMode::{Sum, SelectOne { active }}` — per-track choice;
  Sum for looper-pedal profile, SelectOne for Deeler profile.
  Added 2026-05-18 as the load-bearing configurability axis.
- `Track`: `id` + name + color + `Vec<Layer>` + `playback_mode` +
  arm/mute state
- `Session`: `id` + tempo + time signature + `bars_per_phrase` +
  `default_playback_mode` + `Vec<Track>` + phrase pool
- Session profile constructors: `Session::new_default()` (looper, 4
  tracks, Sum) and `Session::new_deeler_profile()` (Deeler, 10 tracks,
  SelectOne)
- `Edit` enum (Genesis, SetBpm, SetTimeSignature, SetBarsPerPhrase,
  RenameTrack, SetTrackColor, ArmTrack, MuteTrack, AppendLayer,
  SetLayerGain, SetLayerMute, SetTrackPlaybackMode, SelectActiveLayer)
- History graph: `commit(Edit) -> NodeId`, `checkout(NodeId)`,
  parent pointers, append-only phrase pool
- Save / load via postcard
- Round-trip tests, 100-commit scrub timing, divergent-history
  shape test

**Validation:**
- Build a Session with 10 tracks, fill some PhraseSlots, save,
  reload — bit-identical ✅ (`ten_track_session_with_phrases_round_trips`)
- 100-commit history scrubs (`checkout` between any two nodes) in
  <16ms total ✅ (`one_hundred_commit_scrub_under_sixteen_ms` — well
  under budget; 100 round-trips of head→root→head completed in
  microseconds on the Windows laptop)
- A test exercising two divergent histories merging into a
  deterministic result ✅
  (`divergent_histories_union_deterministically` — three properties:
  no NodeId collisions across branches, BTreeMap union is
  order-independent, parent chains both reach the shared ancestor)
- `hocket-model` has zero deps on cpal, xilem, masonry, winit ✅
  (Cargo.toml: serde + postcard + uuid only)

### Feature Target 3: Phrase capture (record one variation)

Wire the engine to the model: arm a track, press record, capture the
audio of one phrase against the click, write it into a `PhraseSlot`,
play it back loop-aligned. Split into three sub-targets — the data
plane is logic that can be fully tested without a real audio device;
the cpal input stream + bar-phase sync is its own engineering problem;
latency calibration is a third concern.

#### Feature Target 3a — data plane ✅ (landed 2026-05-18)

Storage, hashing, capture-state machine, and `Edit::CapturePhrase`
commit, all in-memory and test-validatable.

**Tasks:**
- `hocket-engine::media`: `MediaStore` trait + `InMemoryStore` impl;
  BLAKE3-shaped content addressing (`hash_buffer`); idempotent `put`
- `hocket-engine::capture`: `Capture` state machine
  (`Idle → Recording → Complete`); `feed`/`feed_slice`/`take_completed`
  API; no cpal coupling
- Integration test: synthesized sine wave → Capture → MediaStore →
  Phrase → `Edit::CapturePhrase` → slot Filled, all via the
  public crate APIs

**Validation:**
- ✅ One captured phrase fills the target slot
  (`end_to_end_capture_one_phrase_fills_slot`)
- ✅ Two captures into different slots preserve both phrases
  (`capture_two_phrases_into_different_slots_preserves_both`) —
  matches the "slot A then slot B; both preserved" criterion
- ✅ Re-recording into the same slot is undoable to the previous
  phrase (`rerecord_into_same_slot_undoable_to_previous`)
- ✅ Same audio bytes always produce the same `MediaRef`; different
  sample rates yield different refs

#### Feature Target 3b-prime — Firewheel runtime proof ⭐ (next)

A focused proof of the new spine: build a minimal Firewheel graph
and validate it carries Hocket's needs before deeper integration.
Per Mark's 2026-05-18 directive: *pivot now, before FT3b hardens
around the current Woodshed/cpal engine boundary.*

**Tasks:**
- Pin `firewheel` / `firewheel-graph` from crates.io (latest stable
  on the 0.10.x line)
- Build a minimal Firewheel graph in `hocket-engine`:
  - Click node (port the click sound from `woodshed_audio::Sound::Click`
    to a Firewheel custom node — small, ~50 LOC)
  - Audio output node (Firewheel's default backend)
  - Audio input node + capture tap (feeds the existing
    `Capture` state machine from FT3a)
  - One sample-player node for replaying a captured phrase
  - One meter node (peak/RMS) on the output bus
- Add commands to `hocket-engine::Handle` for: start/stop transport,
  arm capture, play captured phrase
- Retire the `SequencerEngine` wrap from `hocket-engine`
- The demo binary scripts: 4s click, arm capture, capture 1 phrase
  off real input, play it back as a sample resource

**Validation:**
- Audible click via the Firewheel graph (maintainer-by-ear)
- Capture a phrase from real input → MediaRef stored in
  `InMemoryStore` → replay through a sample-player node (by-ear)
- Meter on output shows level activity during both click and replay
- `cargo test --workspace` stays green; `cargo build` clean on
  primary Windows laptop, then verified on iMac + Fedora 44 if time

**Out of scope for FT3b-prime (deferred to FT3b proper):**
- Bar-phase synchronization between input and output (FT3b)
- Count-in (FT3b)
- Layered-track playback (multiple sample players summing) — proof
  only needs one
- Latency calibration (FT3c)
- Web/WASM backend (FT11)

#### Feature Target 3b — cpal bar-phase + layered playback

With Firewheel proven as the substrate (FT3b-prime), expand the graph
to support bar-aligned capture against an optional master clock, plus
layered playback where multiple sample-player nodes on the same track
sum into the track's mixer.

**Tasks:**
- Variable-length loops with an *optional* session-level master
  clock — N=1 user works without locking to a click; the consolidate-
  to-bar-grid gesture is what aligns layers to the grid when desired
- Bar-phase sync between Firewheel's input and output streams (use
  Firewheel's musical clock for *local* scheduling, not as canonical
  session state)
- Count-in: configurable number of bars of click before arming
  transitions to recording (only meaningful when a master clock is set)
- Layered playback: multiple sample-player nodes per track summing
  into the track mixer; mute/gain per layer is a runtime parameter
  fed from `Layer` state
- Re-evaluate the rtrb lock-free upgrade now that input + output are
  both producing real-time pressure (deferred from FT1). Firewheel
  itself is "no mutexes" by design, so the Hocket-side boundary is
  the question — what crosses the audio thread?

**Validation:**
- Plug in an instrument, press record, capture a phrase, the new
  layer appears on the target track (by-ear)
- Phrase plays back loop-aligned to the click without drift (by-ear,
  when a master clock is set)
- Record a second layer on the same track; both layers play
  simultaneously (overdub) — mute one, the other survives (by-ear)

#### Feature Target 3c — latency calibration

Use `woodshed_audio::calibration` to measure input/output round-trip
latency at session start; offset captured buffer writes by the
measured delay so what the user hears as "beat 1" matches sample 0 of
the captured buffer.

**Tasks:**
- Wire `CalibrationSession` from woodshed-audio
- Apply the measured latency as a write-position offset on the
  Capture buffer
- Allow re-running calibration on demand

**Validation:**
- Latency calibration produces a stable estimate (variance < 1ms over
  10 runs)
- After calibration, a captured click-on-the-beat lands at sample 0
  (±1 sample) of the captured buffer

### Feature Target 4: Hocket-xilem app shell

Minimum Xilem app: window, tab/header, transport bar, click controls,
and a track-strip column. Track-strip rendering is **profile-aware**:
under the looper-pedal profile (default), each track strip shows its
layer stack; under the Deeler profile, each track strip shows up to
N (UI-conventional 4) variation slots with the currently-active
slot highlighted.

**Tasks:**
- `hocket` binary = the Xilem app (mirrors woodshed-xilem's pattern).
  *(Renamed 2026-05-18: the former `hocket-xilem` crate is now
  `hocket` — the binary users run — and the former scripted-demo
  `hocket` crate is now `hocket-headless`. `cargo run -p hocket`
  launches the app; `cargo run -p hocket-headless` runs the audio
  demo.)*
- Top-level state: handle to engine, handle to model, current
  selection, current session profile
- View hierarchy: Title row + Transport row + Track strip column + Click
  config row
- TrackStrip widget reads `track.playback_mode` to choose its sub-view:
  layer-stack for Sum, variation-slot grid for SelectOne
- `task()` view for engine event pump

**Validation:**
- App opens to a window in the looper-pedal profile by default
- Transport row plays / stops the click via the engine
- Tempo and time signature changes via UI propagate to engine
- Track strip renders 4 tracks × N layers in default profile and
  10 tracks × 4 slots when switched to Deeler profile
- No paint-thread blocking on engine messages

### Feature Target 5: WaveformWidget (the framework spike)

The Phase 0 critical spike — proves Vello/Masonry paint quality and
perf are good enough for the project's load-bearing widget. Renders a
Phrase's peak-file LOD inside each layer cell (or variation slot in
Deeler profile).

**Tasks:**
- Peak-file generator (min/max/RMS at multiple LOD tiers) in
  `hocket-widgets`
- `WaveformWidget`: Masonry custom widget, `paint(ctx, props, painter)`
  emits a Vello path over the peak data
- Wire into layer / slot rendering: filled cell shows its waveform
- Frame-time instrumentation

**Validation:**
- Stable 60fps with the densest profile lit up — Deeler profile
  (10 tracks × 4 slots = 40 waveforms) is the harder target; looper
  profile is bounded by how many layers the user has captured
- Polylines antialiased (qualitative — no staircasing at any zoom)
- Same paint path runs on macOS and Linux (Fedora/Wayland) without
  target-specific shims
- Per-frame paint time logged

### Feature Target 6: Layer / slot interaction

Click a layer/slot to focus it, double-click to record into the track,
right-click for the cell menu (mute, set gain, select-as-active in
Deeler mode, duplicate, etc.). All edits commit history nodes.

**Tasks:**
- Hit testing within `WaveformWidget` and the surrounding cell
- Per-cell transport: solo-preview this layer's contents
- Layer-level operations: mute toggle, gain adjust, select-active
  (SelectOne mode), duplicate, "make this the active variation"
- Undo / redo via history checkout

**Validation:**
- Cell interactions feel responsive (<50ms perceived latency)
- Undo restores the prior state exactly
- Operating one cell does not interrupt click playback or other
  cell playback

### Feature Target 7: Combinatorial-play view (Deeler-profile primary surface)

The primary surface **in the Deeler profile**: a grid showing which
variation plays per track. Clicking switches the active layer, the
engine schedules the change for the next bar boundary. Maps directly
onto `Edit::SelectActiveLayer` in `hocket-model`.

In the looper-pedal profile this view is hidden; that profile's
primary surface is the per-track layer stack (per FT4).

**Note on sections.** Deeler itself has no saved sections — variation
picking is live toggling only. Hocket deliberately extends past
Deeler with named sections (snapshots of "which layer is active per
track") for instant recall, with the strict-cap caveat that sections
don't drift into arrange-view territory (that's FT10).

**Tasks:**
- `CombinationGrid` widget: N tracks × N-slots (typically 10 × 4 in
  Deeler profile)
- Engine command: "for track N, switch active layer to index X at
  next bar boundary"
- Visual indication of currently-playing slot vs queued slot
- Save current combination as a named "section" (model type addition)

**Validation:**
- Switching variations on the fly produces sample-accurate transitions
  at bar boundaries — no clicks, no dropouts
- Up to 10 simultaneous tracks play in sync at 256-sample buffer
- Section save/load round-trips

### Feature Target 8: Local persistence

Save and load a complete Session including all captured audio,
selections, sections, and history.

**Tasks:**
- Session bundle on disk: project.bin + media/ subdir (one file per
  Phrase, content-hash named) + history.bin
- `rfd` file picker for new / open / save
- Autosave on every history commit

**Validation:**
- Save a session, close, reopen — bit-identical state
- Move a session bundle to another machine and open it — bit-identical
  state
- Autosave doesn't introduce audible glitches

### Feature Target 9: P2P hand-off (Moothold / Murm)

The collaboration moat. Push session state to a peer; the peer pulls
and continues.

**Tasks:**
- `hocket-sync` crate (new, added in this target): wraps Moothold for
  content-addressed blob storage and Murm for session-state sync
- Hand-off protocol: serialize the history graph to Moothold blobs,
  generate a hand-off token (an addressable history-node pointer),
  send the token via Murm. The transport-neutral signed snapshot envelope landed
  separately on 2026-07-10; it does not yet provide this carrier or a merge law.
- Pull side: receive token, fetch history nodes, fetch any missing
  media blobs, surface as "incoming hand-off" in the UI
- Accept: merge incoming history into the local session

**Validation:**
- Two instances of Hocket on the same LAN (one Windows, one Fedora)
  hand off a session both directions
- A Phrase recorded on one side appears on the other after hand-off
- Conflict case: both sides edit different tracks, then hand off
  simultaneously — merges deterministically with both edits preserved
- Hand-off works over the open internet between two non-LAN peers

### Feature Target 10: Arrangement

Sequence named sections into a song. Export rendered audio.

**Tasks:**
- `Arrangement` type in `hocket-model`: ordered list of (section,
  bar-count)
- `ArrangementWidget` in `hocket-widgets`: timeline-shaped, but
  bounded — one row per arrangement entry, not a continuous canvas
- Offline render: `woodshed-audio::offline::render_pattern`-shaped
  function for arrangements
- Export to WAV (later: FLAC via symphonia)

**Validation:**
- Build a 16-bar arrangement from 4 sections, render to WAV
- Rendered WAV matches realtime playback bit-identically (offline
  render is deterministic)
- Export sizes match expected duration

### Feature Target 11: Web target (xilem_web + AudioWorklet)

Hocket in a browser. Native and web build the same model and the
same `hocket-engine` API; the difference is the `AudioBackend` impl
and the widget render path (Masonry → xilem_web SVG).

**Tasks:**
- `trait AudioBackend` introduced retroactively in `hocket-engine`
  (defer to this target — premature abstraction earlier)
- cpal impl on native, AudioWorklet impl on web
- File backend impl for OPFS (web)
- `hocket-widgets` paint paths re-emitted as xilem_web SVG using
  the same `peniko`/`kurbo` geometry
- COOP/COEP deploy doc
- Self-hosted demo

**Validation:**
- Web build records a Phrase via AudioWorklet, plays back, scrubs
  combinations
- Session round-trips between native and web (same bundle format)
- Latency floor measured + documented per browser (Chrome / Firefox;
  Safari best-effort given WebGPU rollout)

### Feature Target 12: PWA + native installer + mobile

Distribution. PWA on web (offline install), itch.io/Gumroad desktop
installers, Android via cargo-apk.

**Tasks:**
- PWA manifest, service worker, OPFS offline path
- Signed Windows + macOS installers
- Android wrapper via cargo-apk; cpal/AAudio backend
- iOS deferred (PWA-on-Safari initially)

**Validation:**
- PWA installs and runs offline on Chrome
- Signed installers verified on Mark's hardware
- Android build runs on Mark's test device

---

## Findings

(populated as work proceeds)

### Session 2026-05-21 — undo / redo (FT6 nondestructive)

- **Model:** `History` gained `undo` / `redo` (built on `checkout` —
  walk to parent / to child) + `can_undo` / `can_redo`. Linear v0, so
  redo finds the single child of head. Every edit we commit
  (AppendLayer, SetLayer*, SelectActiveLayer, ArmTrack, SetBpm,
  SetMasterClock, SetCountInBars, SetTimeSignature) is now reversible.
  +1 round-trip test.
- **App reconcile:** after a history move the session can differ in
  tempo/clock, the layer set, mute/gain/active-variation, arm. So
  `rebuild_after_history` rebuilds *everything derived*:
  `recompute_all_peaks` (per-layer + combined waveforms from the store,
  to match the restored layer set), `resync_tempo` (re-render click +
  stop voices), then `reconcile_playback` (restart exactly the layers
  that `is_layer_audible` in the restored state). The media store is the
  monotonic content pool — undo never removes from it — so every
  restored layer's samples still resolve.
- **UI:** ↶ Undo / ↷ Redo buttons in the transport (labels show
  availability; no-op when unavailable). Exercises the whole
  "nondestructive everything" architecture — the payoff for routing all
  mutations through `History`.
- model 31 (+1) tests, engine 15+3 green; app builds.
- **Env note:** the shared xilem fork moved to `crates/xilem/` and was
  mid-edit (external-composite `window` plumbing) — briefly broke the
  app build (`masonry_winit`), unrelated to this work; resolved on
  re-build.

### Session 2026-05-21 — free / unclocked capture (variable-length loops)

- **Engine:** new `PendingCapture::FreeRecording(Vec<f32>)` accumulates
  *every* drained input sample with no target length; `start_free_capture`
  (immediate — no bar wait, no count-in) / `stop_free_capture`
  (→ `Complete`, retrieved by the same `take_bar_aligned_capture`).
  `CapturePhase::FreeRecording { samples_done }` for UI. The bar-aligned
  path is untouched.
- **App:** `record()` branches on the master clock — on: bar-aligned
  fixed-length + count-in (as before); **off: free capture, toggled**
  (first Record press starts, second stops; the loop is whatever length
  you played). Capture promotion now plays the new layer **immediately**
  (`play_layer`) when unclocked instead of waiting for a bar
  (`play_layer_at_next_bar`), and **guards empty buffers** (instant
  stop yields nothing → no zero-length layer).
- **Transport:** the Record button becomes **"■ Stop recording"** while
  a free capture runs; `capture_phase_text` shows elapsed seconds.
- This completes the looper's "variable-length / optional master clock"
  promise: clock off ⇒ no metronome, no count-in, free-length loops that
  start looping the moment you stop. Build clean (8.1s); engine 15+3,
  model 30+2 green. Runtime-validate the toggle + free-loop playback.

### Session 2026-05-21 — runtime tempo (click re-render)

- **Engine `set_tempo(bpm, beats_per_bar)`** re-renders the click loop
  and swaps it in by **rebuilding the click node** (remove + add) — the
  add-on-demand pattern the voices use, since post-hoc `SamplerNode`
  sample swaps are unreliable. Updates `bpm`/`beats_per_bar` so
  `samples_per_bar` (capture/replay alignment) tracks the new grid. The
  click playhead restarts at bar 0; new node starts at unity, so callers
  re-apply `set_click_enabled`.
- **App:** `nudge_bpm` (40–240) commits `SetBpm`; `nudge_beats` (1–16,
  the time-sig numerator) commits `SetTimeSignature`; both call
  `resync_tempo` (set_tempo + re-apply click mute). `switch_profile`
  now resyncs tempo too (covers the case where you'd changed BPM before
  switching). Makes the Deeler "fixed bar length" honest and gives the
  looper a real tempo control.
- **Settings UI:** BPM −/+ (±5) and beats/bar −/+ steppers (no longer
  dead controls — earlier they'd have been misleading).
- **Already-playing layers are fixed-length buffers at their captured
  tempo** — `set_tempo` doesn't time-stretch them. Rather than let them
  drift against the new grid, a tempo/bar-length change now **stops all
  playing loops** (`resync_tempo` calls `stop_all` first). Tempo is a
  set-before-recording gesture. Build clean; engine 15+3, model 30+2
  green.

### Session 2026-05-20 — master clock + count-in

- **Model:** `Session.master_clock_enabled: bool` + `count_in_bars: u8`
  (defaults: on, 1 bar), with `Edit::SetMasterClock` / `SetCountInBars`
  (apply/invert mirror `SetBarsPerPhrase`). So both are history-tracked
  + persist + travel on hand-off.
- **Engine:** `set_click_enabled(bool)` mutes/unmutes the click by
  patching the click **sampler's volume** via `sync_volume_event` — the
  same post-hoc-safe path `set_layer_gain` uses (a `play` toggle would
  hit the broken `Notify<bool>::patch`). The click keeps running on its
  grid but goes silent, so the bar-phase math that reads the click
  playhead is unaffected.
- **App:** `record()` now takes count-in from the session, gated by the
  master clock (`master_clock ? count_in_bars : 0`). `toggle_master_clock`
  commits the edit + mutes the engine click to match; `nudge_count_in`
  steps 0..=8 bars. `switch_profile` re-syncs the click to the new
  session's clock state.
- **Settings UI:** master-clock on/off toggle + count-in −/+ stepper,
  both reflected in the summary.
- **Interim semantics (noted):** master-clock-off mutes the click and
  skips count-in, but capture still snaps to the (now-silent) bar grid.
  Truly free / variable-length unclocked capture is a deeper engine path
  (separate from this toggle) — deferred. Runtime BPM change is likewise
  still unsupported (click loop is pre-rendered), so no BPM control was
  added.
- Build clean (0.6s); model 30+2, engine 15+3 tests green.

### Session 2026-05-20 — arm-state consolidation + per-layer waveforms

- **Arm state consolidated onto the model.** Dropped
  `AppState.armed_track: usize`; the armed track is now `Track.armed`
  (single source of truth). `arm(idx)` goes through `Edit::ArmTrack`
  (single-arm: unarm the stale one, arm the target), so arm state is
  history-tracked and survives undo/redo + hand-off. `armed_track()` is
  now a derived getter (`tracks.position(|t| t.armed)`); `record()` and
  the strips read it / `track.armed`. First track armed by default via
  `arm(0)` in `new()` + `switch_profile`.
- **Looper strip: compact = combined, expanded = per-layer.** Replaced
  the single per-track peak set with `layer_peaks: Vec<Vec<Vec<Peak>>>`
  (`[track][layer]`) + `combined_peaks: Vec<Vec<Peak>>`. At capture: the
  new layer's peaks append to `layer_peaks`, and `recompute_combined`
  re-sums *all* the track's layers from the content store → the compact
  waveform. Expanding the strip now renders each layer's own waveform
  (shorter `LAYER_WAVEFORM_H`) beside its mute/gain. The compact shape
  is literally the stacked overdub summed.
- Build clean (0.6s); hocket-model 30+2, hocket-engine 15+3 green.
  Audio/visual paths want a runtime eyeball.

### Session 2026-05-20 — app shell scaffold (surfaces + profile-aware strips)

- **Decomposed the flat `main.rs` prototype into a surface-based shell.**
  `main.rs` now owns `AppState`, the action helpers the views call, and
  the engine tick/capture glue; view composition lives in `src/view/`.
  Decision (Mark): Hocket-specific views live in the **app crate**, not
  `hocket-widgets` — they read `AppState`, so putting them in the
  widgets crate would be circular. `hocket-widgets` stays for
  data-parameterized widgets.
- **Surfaces:** a persistent `transport` bar (record/stop, status,
  output meter, surface nav) over one of `Tracks` / `Combination` /
  `Settings` (a `Surface` enum on `AppState`, dispatched via `OneOf3`).
- **Profile-aware track strips** — the heart, dispatched on
  `track.playback_mode` (`OneOf2`):
  - **Sum (looper):** compact strip (arm + summed waveform + expand
    toggle); expanding reveals per-layer **mute + gain± rows** (chosen
    layout: compact+expand, per Mark).
  - **SelectOne (Deeler):** a variation-slot row; the active slot is
    marked, clicking switches it.
- **Wired the future surfaces for real** (per Mark, not stubs):
  - **Combination grid** — tracks × variation slots; clicking a cell
    calls `select_variation`, which commits `Edit::SelectActiveLayer`,
    stops the previously-active voice, and **replays the chosen layer
    from the content store** at the next bar (`store.get(media) →
    play_layer_at_next_bar`). First use of store-replay outside the
    capture path.
  - **Settings** — profile switch (looper ↔ Deeler) rebuilds the
    session/history/store fresh; session summary readout.
- **New `AppState` action helpers** (the model+audio glue, kept out of
  the views): `record`, `stop_all`, `select_variation`,
  `toggle_layer_mute`, `nudge_layer_gain` (live via
  `engine.set_layer_gain`), `switch_profile`, `play_layer_from_store`,
  `arm`, `toggle_expand`. Layer edits all commit through `History`.
- **Deferred (flagged, not silently skipped):**
  - **Arm-state still UI-side** (`AppState.armed_track: usize`), not
    consolidated onto `Track.armed` + `Edit::ArmTrack`. Limited blast
    radius for the scaffold; consolidation is a clean follow-up so arm
    state survives hand-off/history.
  - **Master clock / count-in / click / track-count** settings have no
    backing model fields yet — Settings shows what exists; those land
    when the model grows the fields.
- Compiles clean (5.4s); hocket-engine 15+3, hocket-model 30+2 tests
  green. UI interactions (mute/gain/variation-switch audio, profile
  switch) want a runtime eyeball.

### Session 2026-05-20 — `xilem-components` (shared UI layer) + meter widget

- **New shared crate `xilem-components`** (woodshed repo), the
  domain-neutral UI layer. Decision (Mark): **two crates, split on
  "is this audio-domain?"** — `xilem-components` for what *all three*
  apps share **including Mere** (which isn't audio), `audio-widgets`
  for the audio-domain widgets that sit one layer above it. The test
  is the same domain-neutrality test as `audio-primitives` (pure DSP)
  vs the drivers. Surveyed four community toolkits first:
  `xilem_synth_widgets` (0.4.0 ✓ — knob/fader/scope, our exact pin),
  `xilem_extras` (xilem-main git ✗ — learn-from-only: trees/tables/
  modal), `xilem-material` (Material-You, theming reference),
  `xilem_baseview` (plugin-GUI path — future).
- **Seeded both layers this pass:**
  - `xilem-components`: a **generic `combobox`**, generalized off
    Woodshed's `AppState`-coupled original — the caller now passes
    `is_open` + a `set_open(state, Option<id>)` setter + `on_select`,
    so any host state shape works. Theme-independent (uses
    `text_button` defaults, no palette), so the crate stays free of
    the still-churning theme.
  - `audio-widgets`: a **`meter_view`** (read-only level bar, same
    canvas-closure idiom as `waveform_view`) + `db_to_norm` (linear-
    in-dB mapping). 10 tests green.
- **Hocket is a real consumer:** the app now draws **live stereo
  meter bars** from the shared `meter_view` (the text dB readout
  stays for precise values), and `hocket-widgets` re-exports
  `combobox` + `meter_view`. Full workspace builds (10.3s).
- **Borrow strategy:** chose to *reimplement* knob/fader/meter in our
  canvas idiom rather than vendor `xilem_synth_widgets` — avoids
  license-header/attribution overhead and 0.4.0-vs-our-checkout API
  drift; we own the code in our style. Did the read-only `meter`
  now; **knob/fader are interactive (pointer-drag) and need a study
  of xilem 0.4.0's gesture API before implementing** — deferred
  rather than guessed-and-shipped-broken.
- **Theme placement clarified (not yet acted on):** generic widgets
  that need colors (cards/buttons) want theme tokens, so `theme`
  ultimately belongs in `xilem-components`, not `audio-widgets` where
  it lives today. Migrating it now would collide with the live
  theme-system work in `audio_widgets::theme` (and woodshed's broken
  build), so it's **deferred until that settles**.
- **Cross-repo coupling note:** a *tri-consumer* crate (incl. Mere,
  a separate repo) living in woodshed's repo is the strongest signal
  yet that the shared layer (`audio-primitives` + `audio-widgets` +
  `xilem-components`) may want its own repo. Open decision.
- **woodshed-xilem rewire deferred:** extracting Woodshed's own
  combobox/widgets onto the shared crate is blocked on woodshed's
  pre-existing user-themes build break, so it wasn't touched; the
  shared crates were built + verified standalone instead.

### Session 2026-05-20 — `audio-primitives` extraction (shared pure DSP)

- **New shared crate `audio-primitives`** (woodshed repo, sibling to
  `audio-widgets`), the doctrine's second shared extraction. Pure std,
  **zero dependencies** — the rule: plain sample slices / timestamps in,
  plain data out; anything owning a stream/engine/`Mutex` handle is a
  *driver* and stays in the consuming crate. Mark picked the crate
  structure (new crate, not growing `audio-widgets`) and the first
  scope (click + onset + calibration) via the scoping question.
- **Direction note:** this extraction pulls *from Woodshed's mature
  DSP into* the shared layer, the reverse of the doctrine's "Hocket
  incubates → promotes." That's fine — the shared layer takes the best
  of either product; Woodshed had already incubated onset/calibration/
  click, so promoting them is the same move in the other direction.
- **Three modules extracted:**
  - `click` — `click_sample` (per-sample sine-burst+decay core) +
    `render_click_bar` (full-bar buffer). Killed a *literal*
    duplication: Hocket's `render_click_loop` was annotated "ported
    from `woodshed_audio::Sound::Click`." Now both call the shared
    synth. Woodshed's `Sound::Click` voice renders per-sample via
    `click_sample`; Hocket pre-renders a bar via `render_click_bar`.
  - `onset` — `OnsetDetector` (streaming energy-envelope transient
    detection) + `estimate_bpm` (median-interval tempo). The pure
    core only; Woodshed's `OnsetAnalyzer`/`OnsetHandle` (cpal
    `Analyzer`, `Mutex`-published, `Instant`-stamped) stayed put and
    now build on the shared detector.
  - `calibration` — `estimate_latency_from_pairs` + `count_matches` +
    `MATCH_WINDOW` (click↔onset pairing → median round-trip latency).
    Woodshed's `CalibrationSession` (drives the live run, owns the
    engine handles) stayed. **This is what Hocket FT3c needs** —
    latency calibration is now a shared primitive away.
- **Coupling fix:** Woodshed's `OnsetHandle` wrote `OnsetDetector`'s
  private `threshold_multiplier` field directly (same-module access).
  Once the detector moved out, added `set_threshold_multiplier` /
  `threshold_multiplier()` accessors and rewired the handle.
- **Back-compat:** Woodshed's `onset`/`calibration` modules `pub use`
  the moved items, so `woodshed_audio::{OnsetDetector, estimate_bpm,
  estimate_latency_from_pairs, MATCH_WINDOW}` resolve unchanged. Pure
  tests moved with the code.
- **Verified:** `audio-primitives` 24 tests, `woodshed-audio` 124
  tests, `hocket-engine` 15+3 tests all green; full Hocket workspace
  builds (22.8s). `woodshed-xilem` currently fails to compile, but on a
  *pre-existing, unrelated* WIP — Mark's in-flight user-themes feature
  (`Settings.user_themes`/`active_user_theme`, `set_user_theme`), not
  anything this extraction touched.
- **First Hocket consumer of the shared `onset` primitive.** Wired an
  `OnsetDetector` into `hocket-engine`: fed by the mic samples the
  engine already drains each `tick` (in `drain_and_advance_capture`),
  gated behind `set_onset_detection(bool)` (off by default, so the
  per-frame DSP only runs when a tap-tempo / calibration session needs
  it). Exposes `detected_bpm()` (audio tap-tempo via the shared
  `estimate_bpm`), `detected_onset_count()`, `reset_onsets()`. Proves
  the onset extraction is reusable across the Firewheel engine (the
  click reuse was already proven; onset/calibration weren't exercised
  by Hocket until now). No UI yet; substrate for FT3c + tap-tempo.
- **Where the extraction stopped, and why.** The remaining woodshed
  audio pieces don't (yet) justify extraction: `SampleBuffer` + its
  destructive ops (`apply_gain`/`normalize`/`reverse`) have *no* Hocket
  consumer — Hocket applies gain non-destructively via Firewheel's
  `Volume::Linear` at the node, not baked into the buffer — so promoting
  them would be extraction-ahead-of-need with no dedup. The WAV loader
  (`load_wav_to_buffer`) would drag `hound` into the deliberately
  zero-dep `audio-primitives`, and Hocket has no file-import path yet
  (capture is mic-only). `SampleBank` is woodshed-shaped (keyed by
  `Sound`/`SequencerPattern` ids; Hocket is content-addressed). Revisit
  WAV when Hocket grows file import (around FT8).

### Session 2026-05-19 — theme module (shared, via audio-widgets)

- **Theme primitives promoted to the shared `audio-widgets` crate**,
  not duplicated into a Hocket-local module. Mark's reminder ("we
  did an audio-widget shared crate with woodshed — a lot of the same
  work is in there") was the prompt; he confirmed the placement
  instinct ("good instinct! agreed!"). The spacing rhythm (`SP_*`,
  4px base), type scale (`TS_*`, ~1.2× modular), `mono_family()`, and
  a base `Palette` (surfaces / text hierarchy / Material-You triad /
  success-danger) are byte-identical across Woodshed and Hocket, so
  copying them would be genuine duplication. They now live in
  `audio-widgets::theme` and re-export through `hocket_widgets::theme`.
- **What stayed product-specific:** the waveform fill color (per
  `track.color`) and Woodshed's fretboard diagram colors are NOT in
  the shared palette — each host layers those on top and reads the
  shared base for everything else. The shared `Palette` deliberately
  drops Woodshed's `fret_*` / `*_dot` fields.
- **Woodshed was left untouched.** The extraction is additive: the
  shared base was lifted *from* Woodshed's proven dark/light values,
  but `woodshed-xilem/src/theme.rs` still carries its own (now-richer)
  Palette. Migrating Woodshed to compose the shared base is a later,
  optional cleanup — not forced by this pass.
- **One new dep on `audio-widgets`:** `serde` (workspace), because
  `ThemeMode` round-trips through a host's settings. Hocket has no
  settings persistence yet, so `Palette::dark()` is hardcoded at
  startup; a light toggle is a later settings pass (the app is already
  fully `palette`-driven).
- **Masonry default-property overlay needed.** Masonry's
  `default_property_set()` hardcodes near-white text + dark button
  surfaces (a dark-theme assumption), so a bare `label(...)` ignores
  the palette. `build_default_properties(&Palette)` overrides Label
  ContentColor + Button Background/Border/Corner, passed via
  `Xilem::new_simple(..).with_default_properties(..)`. Same pattern
  Woodshed uses. Set once at startup; a mid-session theme switch would
  need a property-set swap (deferred). Build clean (9.42s).

### Session 2026-05-18 — scaffold

- Per Mark's preference (confirmed 2026-05-18), Hocket lives in
  its own repo `repos/hocket/` rather than as a crate inside the
  Woodshed workspace.
- Crate name on crates.io: `hocket` is squatted by a dead 2016
  redirect (last release v0.1.1, "Moved to libhocket-sys"). Workspace
  uses `hocket-*` namespaced crates; the bare name is cosmetic since
  distribution is through itch.io/Gumroad, not `cargo install`. If we
  later want the bare name we can file a crates.io abandonment claim.
- Brand alignment: Hocket sits beside Mere under the Merely parent
  brand. The Greek root στροφή/στρόφος ("turn") still names Hocket on
  its own terms: the choral turn is the loop, the pass-the-session turn
  is the collaboration mechanic. It no longer echoes the parent, though.
  The umbrella was *Strophos* (same root) until 2026-07-09, when it
  became **Merely**. The family link is positioning now, not etymology.
- Initial deps via path: `../woodshed/crates/woodshed-audio` and the
  local xilem checkout (`../xilem/`). Matching woodshed's pins keeps
  the local xilem source-of-truth coherent across both projects.

### Session 2026-05-18 — Feature Target 1 click engine

- **Reuse strategy:** `hocket-engine` wraps `woodshed_audio::SequencerEngine`
  with a hocket-shaped `Transport` API. The click sound
  (`Sound::Click`), sample-accurate scheduling, and tempo-change
  continuity are *all* reused from woodshed-audio's tested
  implementation. The wrap is thin (~190 LOC including tests and
  doc comments).
- **`bars_per_phrase` lives on `Transport` from day one** even though
  it doesn't affect the click track; it's informational at
  Feature Target 1 and load-bearing from Feature Target 3 (phrase
  capture) onward. Putting it on the type up front avoids a transport
  shape change later.
- **Lock-free channels deferred to Feature Target 3.** The original
  Feature Target 1 spec called for `rtrb` command/event channels.
  Wrapping woodshed-audio's `Arc<Mutex>`-shaped `EngineHandle` mirrors
  woodshed-audio's existing pattern, which is documented as adequate
  for metronome / light sequencer use. Lock-free upgrade becomes
  load-bearing at Feature Target 3 (phrase capture writes audio
  buffers from the input-stream callback, which is where real-time
  contention starts to matter). Re-evaluate the engine boundary
  upgrade at that target rather than speculating now.
- **Time-signature continuity is out of scope for v0.**
  `Handle::set_time_signature` restarts the pattern (woodshed-audio's
  `set_pattern` resets sample/step counters). Continuous
  time-signature change is a future enhancement that would either
  require extending woodshed-audio's `EngineHandle` API or
  reimplementing the engine; not warranted yet.
- **Audible-output validation is by ear.** Headless unit tests cover
  the `Transport` shape and the pattern construction; the cpal stream
  + the actual click sound require the maintainer to listen. The
  demo binary (`cargo run -p hocket`) emits ~13 seconds of click
  with one tempo change and one time-signature change.

### Session 2026-05-18 — FT5 WaveformWidget (framework spike) ✅

- **FT5 landed + validated** ("did work!" 2026-05-18). The Vello/
  Masonry custom-widget proof the whole plan pointed at. Each track
  strip now shows a 240×40 waveform in the track's palette color;
  recording fills it with the captured audio's min/max envelope.
  Maintainer screenshot: 4 tracks recorded, all looping, distinct
  colors, smooth antialiased envelopes.
- **Implementation pattern** (simpler than a raw Masonry `Widget`
  impl): a widget is an Xilem `canvas(move |state, ctx, scene, size|
  {...})` view. Inside, wrap the `Scene` in a `Painter` and draw
  kurbo/peniko primitives. `painter.fill(&bezpath, color).draw()` /
  `painter.stroke(&path, &Stroke::new(w), color).draw()`. No Widget
  trait to implement. (Same pattern woodshed-xilem uses for
  fretboard / chord-diagram canvases.)
- **`hocket-widgets` API** (product-agnostic, extraction-ready for
  `audio-widgets`): `compute_peaks(&[f32], columns) -> Vec<(f32,f32)>`
  (single-LOD min/max peak file) + `waveform_view(peaks, wave_color,
  zero_line_color)` (filled envelope, faint zero-line). 4 unit tests
  on `compute_peaks`.
- **Spike confirms what the gpui-vs-Xilem decision hinged on:** Vello
  antialiases the filled waveform by default — the polyline-AA gap
  that ruled out gpui. The rendering stack is now proven for every
  downstream visual (piano roll, automation lanes, arrangement view
  reuse this exact `canvas` + `Painter` path).
- **Xilem gotchas hit + fixed:** `.width()/.height()` on `sized_box`
  need `use xilem::style::Style` + `Length` args (use `f64.px()` via
  `masonry::layout::AsUnit`). Per-track dynamic rows are
  `Vec<AnyFlexChild<State>>` via `.into_any_flex()` (distinct closure
  types need erasure).
- **Peaks cached at capture time** (not recomputed per frame):
  `AppState.track_peaks[track_idx] = compute_peaks(&samples, 256)`
  computed before `samples` moves into the engine.
- **Still showing only the most-recent layer's waveform per track**,
  with bare Masonry styling. Multi-layer waveform stacking,
  profile-aware strips, and the theme pass are refinements.

### Session 2026-05-18 — FT4.2 multi-track + model wiring

- **FT4.2 landed + validated** ("worked!" 2026-05-18). The full stack
  connects for the first time: **UI → model → engine.** `AppState`
  now owns the authoritative `Session` (looper profile, 4 tracks) +
  `History` + an `InMemoryStore`, alongside the `Engine`. Track strips
  render from `session.tracks` (●/○ armed marker + name + layer
  count); clicking a row arms that track. Record captures into the
  armed track; on completion the buffer is content-addressed
  (`store.put` → `MediaRef`), wrapped in `Phrase` + `Layer`, committed
  via `History::commit(Edit::AppendLayer)`, and queued in the engine
  with `play_layer_at_next_bar` using `LayerKey{track_id, layer_index}`.
  Multiple tracks loop simultaneously (Sum). "N loop(s) playing"
  counter + Stop-all button.
- **This is the architecture working as designed:** model is the
  authority (every capture is an undoable, content-addressed history
  commit), engine is the runtime that plays the projected state, UI
  translates intent into model edits + engine commands. No
  PlaybackMode opinion in the engine — the host decides what's
  audible.
- **Borrow-discipline note:** the tick handler reads meter/phase/
  capture within a single `&mut engine` borrow (returns a tuple), then
  handles capture-completion touching the disjoint fields
  (`store`, `session`, `history`, `engine`) in sequence.
  `history.commit(&mut session)` works because they're separate
  struct fields.
- **Still text-only UI** (bare Masonry labels + buttons). Track strips
  show layer *count*, not waveforms — that's FT5 (`WaveformWidget`).
  Profile-aware rendering (Deeler variation-slots vs looper
  layer-stack) is a refinement once the visual layer exists.

### Session 2026-05-18 — FT4.1 interactive record + tick refactor

- **FT4.1 landed + validated.** `hocket` app has a ● Record button
  (arms a bar-aligned capture, 1-bar count-in, 1-bar capture), a live
  capture-phase line (ready → count-in → recording → captured), a
  loop-status line, and a ■ Stop loop button. UI Record button
  click-tested by maintainer 2026-05-18: "confirmed" — loop fills,
  status flips to "playing (in phase with click)", audio loops. The
  full Deeler gesture (arm → count-in → capture → in-phase loop) is
  now driveable by hand with one track.
- **Key fix — `tick()` drives everything.** First UI attempt showed
  the capture stuck at "count-in 2 bars left" forever. Cause: the
  capture state machine was only advanced by `drain_input`, which the
  UI never called (only the headless demo's loop did). Fix:
  `Engine::tick()` now drains the reader internally (into a reused
  `input_scratch` buffer) and advances the bar-aligned capture, in
  addition to advancing queued layers + flushing Firewheel. `tick()`
  is the single "advance the engine" call; hosts no longer drain
  separately. Public `drain_input` removed; `recent_input() -> &[f32]`
  exposed for future VU metering. The headless demo's loop was
  restructured to one `tick()` at the top (before
  `take_bar_aligned_capture`) instead of a separate drain call.

### Session 2026-05-18 — FT4.0 app shell + crate rename

- **FT4.0 landed.** `hocket` (the app crate) opens a Xilem + Masonry
  window showing engine status + a live output meter. A `task_raw`
  background task drives `Engine::tick()` on a ~16 ms cadence — this
  is load-bearing (Firewheel needs `update()` regularly; it also
  advances the bar-aligned scheduling state machines). The `!Send`
  Firewheel engine lives directly in `AppState`; Xilem keeps state on
  the main thread so that's fine (same pattern woodshed-xilem uses for
  its `!Send` SequencerEngine).
- **Crate rename** (per Mark): the app is now the `hocket` binary
  (was `hocket-xilem`), and the scripted audio demo is now
  `hocket-headless` (was `hocket`). Directories + package names +
  bin names + workspace members all updated; README, CLAUDE.md, and
  this plan's FT4 task updated. `hocket-xilem` is gone from
  `workspace.dependencies` (the app + headless binaries are leaf
  members, not dep targets).
- No theme module yet — bare Masonry labels. Visual conventions
  (palette, 4 px spacing, type scale, mono font for the meter
  readout) land at FT4.1 or when the theme module arrives.
- **Awaiting maintainer visual validation**: window opens, status +
  live meter visible, click audible.

### Session 2026-05-18 — FT3b.2 bar-aligned capture + replay

Master clock plus bar-phase sync. Engine now has bar-aware
scheduling for both capture-arm and layer-start: capture waits for
the next click bar boundary plus `count_in_bars` of count-in before
recording N bars; layer playback can be queued to start on the next
bar boundary so captured loops play in phase with the click.

**New engine API:**

- `Engine::samples_per_bar() -> usize` — pure math from
  `(sample_rate, bpm, beats_per_bar)`. Engine carries `bpm` /
  `beats_per_bar` as fields, set at construction (defaults
  120 / 4). Runtime tempo changes are FT3b-future.
- `Engine::click_in_bar_phase() -> Option<usize>` — current click
  playhead position **within the current bar** (0 ≤ value <
  bar_samples). Wraps to 0 at each boundary.
- `Engine::samples_to_next_bar() -> Option<usize>` — `bar - in_bar`.
- `Engine::arm_bar_aligned_capture(bars, count_in_bars) -> Result<()>` —
  schedules a capture to start at the (count_in_bars+1)th bar
  boundary after arm. Engine owns the `Capture` state machine
  internally; host doesn't construct one.
- `Engine::take_bar_aligned_capture() -> Option<Vec<f32>>` — drains
  the completed buffer once `CapturePhase::Complete`.
- `Engine::pending_capture_progress() -> CapturePhase` —
  Idle / Waiting / Recording / Complete with progress info for UI.
- `Engine::play_layer_at_next_bar(key, samples, gain, looping)` —
  queues a `play_layer` to fire from the tick after the next bar
  boundary crosses.

**Bar-boundary detection: wrap-detect with half-bar threshold.**
`SamplerState::playhead_frames` is read across the audio-thread /
UI-thread boundary and can momentarily return a slightly stale
(smaller) value. A naive `cur < last` check fires on every such
jitter. The fix: require `last - cur > bar_samples / 2` — only a
real wrap from "near end" to "near start" produces a half-bar gap.

**Count-in semantics:** `count_in_bars` is the number of *full*
bars of click between "next bar boundary after arm" and "recording
starts." The brief partial-bar wait to the next boundary does not
count. Net effect: count-in is *at least* `count_in_bars` full bars
in every case (longer if armed mid-bar). Matches standard DAW feel.

**Demo flow (validated 2026-05-18):**

1. Click loops endlessly from t=0
2. t=4.0: `arm_bar_aligned_capture(1, 1)` — capture 1 bar, 1 bar
   count-in
3. t=6.0: bar boundary crosses, count-in complete, recording starts
4. t=8.0: recording complete (96000 samples = exactly 1 bar)
5. t=8.0: `play_layer_at_next_bar` queues replay
6. t=8.0+ε: next bar boundary, replay begins
7. Replay loops in phase with click (verified by ear)

**Tick-resolution precision (~15 ms):** the scheduler fires on the
next UI tick after the boundary, so layer/capture starts are up to
one tick late. At 48 kHz that's ~720 samples = ~15 ms. Audible as
a slight flam if compared to perfect sample alignment, but
musically acceptable for v0. Sample-accurate scheduling via
Firewheel's `scheduled_events` feature is a future enhancement.

**Outstanding for FT3b.2 to fully close:**

- ✅ Maintainer by-ear validation 2026-05-18: *"it seems perfectly
  timed to me."* Replay loop is in-phase with the click — no
  audible flam despite the ~15 ms tick-resolution scheduling. The
  ~15 ms imprecision is apparently below the perceptual threshold
  for this loop length / tempo, so `scheduled_events` sample-accurate
  scheduling is **not** urgent. Revisit only if tighter material
  (short percussive loops, faster tempi) exposes audible flam.
- Auto-cleanup of one-shot voices (poll `SamplerState::stopped()`)
- Session::master_clock_enabled model field — currently the engine
  click is unconditional. Wiring it through Session means the
  Deeler profile always has master_clock=true, and the looper
  profile can disable the click for variable-length-loop mode.
  Deferred until the UI surfaces a toggle.
- Tests for bar-phase math (unit-testable without audio device)

### Session 2026-05-18 — FT3b.1 layered playback (voice pool)

First substantive chunk of FT3b proper. Engine grew from "one
replay sampler" to "dynamic add-on-demand sampler nodes addressed
by `LayerKey`." `Engine::play_layer(key, samples, gain, looping)`
adds a fully-populated `SamplerNode` to the graph and wires it to
the meter; `stop_layer(key)` removes it. Looping captured audio
replays cleanly; maintainer-confirmed by ear 2026-05-18. Meter
shows real dB values during replay (was `-inf` in the failed
pre-allocated-pool design).

**Failed design first, working design second.** I initially built
a pre-allocated pool of N idle `SamplerNode`s wired to the meter
at construction, planning to "activate" them via post-hoc
`sync_*_event` calls when `play_layer` was called. **That
produced no audible output.** Symptom matched the `Notify<bool>`
bug we already knew about, but it persisted even with the manual
event workaround. Root cause traces to BillyDM's own TODO at the
top of `sampler.rs`: *"The logic in this has become incredibly
complex and error-prone. I plan on rewriting the sampler engine
using a state machine."* SamplerNode state transitions from
"empty / not playing" to "loaded / playing" via post-hoc events
don't take effect reliably in 0.10.0.

**Working pattern: add-on-demand + remove-on-stop.** Mirroring
the click sampler's proven approach. Each `play_layer` adds a new
SamplerNode fully populated (sample, volume, repeat_mode, play,
play_from) at `cx.add_node` time. The audio thread receives the
node already configured; no state machine transitions involved.
`stop_layer` calls `cx.remove_node` to free the slot. `set_layer_gain`
uses `sync_volume_event` (which uses `ParamData::Volume`, not
`Notify`, and works post-hoc).

**Standardized Firewheel-usage rules for Hocket** (codify in a
`hocket-engine` helper module after FT3b stabilizes):

- Construct sampler nodes fully populated at `add_node` time. The
  initial state is what reliably reaches the audio thread.
- For "change which sample plays," prefer `remove_node` + new
  `add_node` over mutating an existing sampler via events.
- For non-Notify params (Volume, RepeatMode, PlayFrom),
  post-hoc `sync_*_event` does work — those use `ParamData::any`
  or specific variants that match their patch path.
- For `Notify<T>` fields (play), the bundled `sync_*_event` is
  broken; if a manual event is unavoidable, construct it as
  `NodeEventType::Param { data: ParamData::any(Notify::new(value)), path }`.
- Voice cap `VOICE_POOL_SIZE = 32` is a soft ceiling for runaway
  hosts; Hocket's actual usage is bounded (Deeler maxes at 10
  active voices, looper bounded by user-captured layer count).

**API added** in `hocket-engine`:

- `LayerKey { track_id: TrackId, layer_index: u16 }` — engine-side
  identifier for a model layer
- `Engine::play_layer(key, samples, gain, looping) -> Result<(), NoFreeVoices>`
- `Engine::stop_layer(key)`
- `Engine::set_layer_gain(key, gain)`
- `Engine::is_layer_assigned(key) -> bool`
- `Engine::voice_count() -> usize`
- `ModelTrackId` re-export so binaries can construct `LayerKey`
  without a separate `hocket-model` dep
- Old `play_replay` removed (subsumed by `play_layer`)

**Engine has no opinion about `PlaybackMode`.** That's deliberate.
The host (UI / demo) translates session state into a series of
`play_layer` / `stop_layer` calls based on what the model says
should be audible. For `Sum`-mode tracks, the host calls
`play_layer` for each unmuted layer. For `SelectOne`-mode tracks,
the host calls `play_layer` only for the active layer. Keeps the
model authoritative; engine is a dumb-pipe playback substrate.

**Outstanding for FT3b proper** (subsequent sub-targets):

- FT3b.2: optional master clock + bar-phase sync between input and
  output streams. Needed for count-in, quantize-to-grid, and
  layer-aligned looping.
- FT3b.3: count-in (depends on FT3b.2)
- Auto-cleanup of one-shot voices when their buffer naturally ends
  (poll `SamplerState::stopped()`; currently host must call
  `stop_layer` explicitly)
- Tests for voice-pool semantics (need a way to construct Engine
  without a real audio device — possibly an `AudioBackend` trait
  spike, deferred from FT11)

### Session 2026-05-18 — Deeler audit + PlaybackMode refactor

After FT3b-prime closed and the reader rate anomaly was fixed, Mark
asked for a plan review against the Deeler reference. Research agent
read every publicly available Menomena / Deeler interview + the
Ramona Falls writing-process video pointer. Findings:

**Deeler's mechanical shape (from public sources):**

- 10 mono tracks × 4 phrases each (consistent across sources)
- **Select-one playback per track**, not sum. Pick A/B/C/D; others
  don't play. The summation is *across* tracks (drums + bass + sax),
  not *within* a track.
- 4-measure phrase length is the default, reconfigurable per session.
- One tempo per session, click always on.
- Re-record (overwrite slot), not multi-layer within a slot.
- No saved sections — variation switching is live toggling only.
- No internal arrangement view — Menomena exported loops and
  assembled songs in Pro Tools.
- Hand-off is social convention (pass the laptop), not a Deeler
  feature.

**The single substantive gap in our model.** The looper-pedal layer
model assumes summation. Deeler requires select-one. We needed a
per-track `PlaybackMode` to support both profiles.

**Refactor landed in this session:**

- `hocket-model::track::PlaybackMode { Sum, SelectOne { active } }`,
  per-track. Default `Sum` for looper-pedal profile, `SelectOne` for
  Deeler.
- `Track::new_with_mode(...)` constructor; `Session::default_playback_mode`
  field; `Session::new_deeler_profile()` constructor (10 tracks,
  SelectOne).
- New history edits: `Edit::SetTrackPlaybackMode` and
  `Edit::SelectActiveLayer`. The latter is the variation-picking
  gesture in Deeler-profile UIs; no-op on Sum-mode tracks.
- `PlaybackMode::is_layer_audible(index, muted)` for engine to query
  during playback rendering (used by FT3b's layered playback work).
- 50 workspace tests pass (29 model unit + 2 model integration +
  15 engine unit + 3 engine integration + 1 doc-test-equivalent
  spread; net +8 tests for PlaybackMode coverage).

**Plan refactor in this session (FT-level):**

- Strategic decisions: "Layered tracks, not variation slots" expanded
  into "Layered tracks with per-track playback mode" — both profiles
  ship in v1.
- FT2: task list rewritten to reflect what actually landed (Layer,
  PlaybackMode, all 13 Edit variants).
- FT4: track-strip widget rendering is now profile-aware (layer-stack
  for Sum, variation-slot grid for SelectOne); default is looper, not
  Deeler.
- FT5: 60fps target framed as "densest profile" — Deeler's 10×4=40
  is the harder target, looper bounded by user's layer count.
- FT6: renamed from "PhraseSlot interaction" to "Layer / slot
  interaction" — types updated.
- FT7: reframed as Deeler-profile primary surface, not Hocket's
  primary surface (looper's primary is the layer stack from FT4).
  Sections explicitly noted as a deliberate extension past Deeler,
  bounded by the strict-cap against arrange-view drift.

**Faithful-Deeler-replication test (informal):** Construct
`Session::new_deeler_profile()`. Capture phrase A into track 0 slot 0;
B into slot 1; C into slot 2; D into slot 3 (each as separate
`Edit::AppendLayer` against the same track). Apply
`Edit::SelectActiveLayer { track_id, from: None, to: Some(0) }` →
slot 0 is the audible variation. Cycle to Some(1), Some(2), Some(3),
None. That's the Deeler gesture. Engine implementation of the
playback path that respects this lands at FT3b proper.

### Session 2026-05-18 — late-day Firewheel pivot + corrections

After the mid-day directional update, Mark verified `cargo test
--workspace` passes and delivered a sharper directive: **pivot to
Firewheel now**, before FT3b hardens around the current Woodshed/cpal
engine boundary. Recorded here:

- **Firewheel is on crates.io.** Prior plan/memory said "not yet" —
  that was wrong; `firewheel-graph` at 0.10.2 as of 2026-03-17.
  Corrected throughout. Pin a crates.io version; only path-dep if
  unreleased fixes become necessary.
- **hocket-model is the authority; Firewheel is the runtime.** This
  was the key conceptual clarification — Firewheel's "app/game state
  sends events into the audio graph" model is much better-suited to
  Hocket than a transport-dominant DAW engine. The audio graph plays
  the *projected state*; the canonical session lives in
  `hocket-model`.
- **FT3b-prime inserted ahead of FT3b.** A focused proof: click +
  I/O + capture-into-MediaRef + replay-as-sample-resource + one meter.
  After it works, retire the SequencerEngine wrap from Hocket core
  (Woodshed keeps its own SequencerEngine).
- **No CLAP for Hocket v1.** Stronger than the prior framing: no
  plugin hosting at all in v1. First-party Rust devices on Firewheel
  nodes with schema-driven UI. CLAP isn't WASM-possible, drags
  DAW-shaped product gravity, and the firewheel-extra host is marked
  TODO. See `memory/project_hocket_no_clap_doctrine.md`.
- **Web is gated, not free.** firewheel-web-audio needs wasm
  threading/atomics + nightly + build-std + COOP/COEP. Proof gate,
  not a "for free" capability. Flagged on FT11.
- **Donor map** (Mark's read after auditing Woodshed + Mere):
  - *Woodshed: take now.* `calibration` (`estimate_latency_from_pairs`
    + `CalibrationSession`), `onset` (`OnsetDetector`, RMS, BPM
    estimation), `offline::export_wav`, the `woodshed-xilem` widget
    pattern (`fretboard_view` / `chord_diagram_view` as template for
    `WaveformWidget`), and Song Mode's bar-boundary/pending-change
    semantics (not the Song model itself).
  - *Woodshed: do not take* as Hocket's core: `InputEngine`,
    `SongEngine`, `Looper`. The Woodshed `Looper` is single-bar with
    overdub semantics — wrong shape for layered durable sample
    resources in a graph.
  - *Mere: take seams later, not code now.* `mere-transport` is the
    likely p2p substrate (versioned ALPN, MemoryTransport,
    IrohTransport, BLAKE3 blob storage). `eidetic` patterns for
    project persistence (typed payloads, bundle manifests, OPFS/native
    abstraction). `mere-masonry` as future embedding reference only
    (currently non-compiling).
  - *Mere: do not pull in* `mere-host-runtime`, host chrome, renderer
    registry, or Moothold proper for Hocket v1. Moothold is mostly
    placeholder/docs today.
  - *Graph viz for history DAG / merge / audio routing:* use
    `cartography` + `graph-canvas` from Mere, not the host chrome.
- **Shared crate trigger:** extract `audio-widgets` (or
  `audio-primitives`) only when first duplication appears — first
  candidate is `WaveformWidget` + meter at FT5, seeded from
  Woodshed's `calibration` / `onset` / meter primitives.

### Session 2026-05-18 — mid-day directional update

After FT3a landed, Mark delivered a substantial directional update.
Captured here so the rationale survives the session:

- **Audio engine substrate: Firewheel, not Dropseed.** BillyDM has
  pivoted from Dropseed (DAW engine) to Firewheel (modular audio
  graph engine, "wgpu but for audio"). Firewheel is deliberately
  *not* a DAW engine — its state-sync model is game-engine-shaped, no
  transport-overrides-user-events invariant — which is a better fit
  for a loop recorder than Dropseed would have been. Active, 702
  commits, dual MIT/Apache-2.0, ships native + WASM backends,
  "no mutexes" realtime constraints. Not on crates.io yet → path-dep.
- **Data model: layered, not variation-slot.** Drop the A/B/C/D
  variation framing entirely. A track is a stack of *layers*
  (looper-pedal model). Multiple takes are muteable layers, not
  alternative variations. Variable-length per track with optional
  session master clock. "Consolidate to bar grid" and "mix down N
  layers" are user gestures.
- **Local-only transport for v1.** Each peer's playback head is
  independent. Sub-100ms WAN-synced realtime transport is a separate
  research project — not v1.
- **Recording lock + structural CRDT.** CRDTs don't fit the audio
  data itself; they do fit the *session structure* (track existence,
  turn ownership, mute/solo, locked BPM). Branches map to Moothold
  IndexCommits.
- **Serialization: CBOR via ciborium**, to align with Moothold.
  Postcard from FT2 stays in place until FT8 makes the coordinated
  migration. No churn for churn's sake.
- **Audio-widgets** becomes a shared crate plan — extract when
  there's something to share (first candidate: `WaveformWidget` at
  FT5).
- **Strict cap doctrine** added to `DOC_README.md` working
  principles: every feature must answer "does this serve the
  passing-the-mic workflow?" The arrange view is the scope-creep
  canary. Promoted into the plan's new Strategic Decisions section
  along with the DAW extension ladder, visual design conventions,
  and full crate stack.
- **FT1 SequencerEngine wrap is now a labeled placeholder.** Code
  comments in `hocket-engine/src/lib.rs` cite the FT3b swap to
  Firewheel. No code change today — swap happens when Firewheel's
  capabilities (variable-length loop playback, cpal input, WASM
  backend) actually need to come in.
- **Outstanding for next session:** layered-model migration in
  `hocket-model` (replaces PhraseSlot / variations_per_track /
  Track.slots[A,B,C,D] with `Track.layers: Vec<Layer>`,
  `Edit::CapturePhrase` becomes `Edit::AppendLayer`, etc).
  Coordinated change — touches model + engine + integration tests
  together. Awaiting maintainer's OK on scope before executing.

---

## Progress

(session log — appended as work proceeds)

### 2026-05-18

- Repo scaffold drafted.
- Five-crate workspace created with stubs.
- Initial plan committed (this doc).
- `cargo build --workspace` green; `cargo run -p hocket` runs the
  Feature Target 0 placeholder.
- **PROJECT_DESCRIPTION pass by maintainer**: configurability framing
  (defaults not limits), "pass the mic around in a circle" north-star
  metaphor adopted. Derivative docs (README, CLAUDE.md, DOC_README)
  updated to match.
- **Feature Targets 1 + 2 landed in a single session.** Workspace
  builds clean; 23 model tests + 2 engine tests pass.
- **Feature Target 1 (click track engine) — landed.**
  - `hocket-engine` implements `Transport` + `Engine` + `Handle`
    wrapping `woodshed_audio::SequencerEngine`.
  - 2 unit tests pass (`transport_default`, `pattern round-trip`).
  - `cargo run -p hocket` drives a scripted click demo:
    4s at 120 BPM 4/4 → continuous tempo change to 90 BPM → 4s →
    pattern restart with time signature 7/8 → 4s → stop. Demo
    completes cleanly end-to-end.
  - **By-ear validation confirmed by maintainer** (2026-05-18):
    "heard the click! it was correct, a faster tempo then a slower
    one. sounded right to me for 120 and 90 bpm, then the 7/8 eighth
    note clicks."
  - **Outstanding for Mark**: CPU measurement during a long demo run
    (deferred to next session — not blocking Feature Target 2 work).
- **Feature Target 2 (hocket-model) — landed.**
  - Seven modules in `crates/hocket-model/src/`: `ids`, `phrase`,
    `track`, `session`, `history`, `persistence`, `lib`. Each under
    300 LOC.
  - All four validation criteria pass (see Plan section above).
  - 23 tests green (21 inline + 2 integration).
  - **Serialization format: postcard.** Chosen over rkyv (zero-copy
    not warranted yet), bincode (less compact, less portable),
    serde_json (HashMap key-order is non-deterministic; would have
    needed BTreeMap conversion anyway). postcard + BTreeMap gives
    deterministic byte output for content-addressing later.
  - **History now retains branches.** Committing after checkout adds a sibling
    without deleting existing descendants; checkout crosses siblings through
    their common ancestor and same-root node sets can be integrated. Full
    conflicting-edit reconciliation remains Feature Target 9 work.
  - **Phrase pool is monotonic / append-only.** Inverting a
    `CapturePhrase` restores the slot pointer but leaves the phrase
    in `session.phrases`. This keeps redo trivially correct and
    avoids "phantom edits" referencing missing pool entries during
    future branch merges.
  - **All IDs are UUID v4** for global uniqueness across peers
    (CRDT-ready). `MediaRef` is `[u8; 32]` for BLAKE3-shaped digests
    that the engine computes; the model just stores bytes.
  - **TimeSignature is defined locally**, mirroring
    `woodshed_audio::TimeSignature`'s shape. The engine converts at
    the boundary. Keeps hocket-model framework-agnostic.
  - **Configurability honored from day one**: `Vec<Track>` and
    `Vec<PhraseSlot>` with explicit `variations_per_track` /
    `bars_per_phrase` on `Session`, not const-generic arrays. Widening
    the defaults later is a session-config change.
  - **Outstanding**: nothing blocking. Feature Target 3 (phrase
    capture: wire engine to model) is the next vertical slice.
- **Mid-day directional update** (see corresponding Findings entry):
  Firewheel replaces Dropseed as the planned engine substrate;
  layered tracks replace A/B/C/D variation slots; local-only transport
  for v1; recording-lock + structural-CRDT collab model; CBOR over
  postcard (deferred to FT8); strict-cap doctrine added to working
  principles; visual design conventions captured.
- **Late-day Firewheel pivot directive received and absorbed.** See
  Findings entry for full corrections + donor map. Key moves:
  Firewheel is on crates.io (prior claim was wrong); no CLAP for v1
  at all (stronger than prior framing); FT3b-prime inserted as the
  focused Firewheel proof; hocket-model is the authority, Firewheel
  is the runtime.
- **Layered-model migration in hocket-model — landed.**
  - `PhraseSlot` → `Layer { phrase_id, gain, muted }`; `Track.slots`
    → `Track.layers: Vec<Layer>`; `Session.variations_per_track`
    removed; track count default 10 → 4.
  - `Edit::CapturePhrase` + `Edit::ClearSlot` → `Edit::AppendLayer`
    + `Edit::SetLayerGain` + `Edit::SetLayerMute`. Append-only
    semantics: no remove-layer in v0; "remove from playback" is
    `SetLayerMute(.., to: true)`. Mix-down (future) is the
    operation that actually collapses layers.
  - All apply/invert paths updated; phrase pool remains monotonic
    (inverting AppendLayer pops the layer but leaves the phrase in
    `session.phrases`).
  - All tests rewritten for the new shape. **41 tests green
    workspace-wide** (22 model unit + 2 model integration + 14
    engine unit + 3 engine integration; plus zero doc-test
    regressions across the workspace).
  - **Outstanding**: FT3b-prime (Firewheel runtime proof) is the
    next concrete code work. Add `firewheel` 0.10.x as a workspace
    dep; build a minimal graph (click node + I/O + capture tap +
    one sample-player + one meter); retire the SequencerEngine wrap.
- **FT3b-prime, step 1 — Firewheel substrate alive.**
  - `firewheel = { version = "0.10", features = ["cpal", "symphonium"] }`
    added to workspace; `woodshed-audio` dropped from hocket-engine.
  - `hocket-engine/src/lib.rs` rewritten on `FirewheelContext`. The
    `SequencerEngine` wrap is gone.
  - Minimal graph for the substrate proof: `graph_in → graph_out`
    mono-to-stereo passthrough.
  - Engine surface: `new() / tick() / sample_rate() / stop()` plus
    `Drop`.
  - Demo binary runs the passthrough for 8 seconds at a 15 ms tick
    cadence.
  - 39 workspace tests green (no Transport/Subdivision tests anymore;
    those types removed with the SequencerEngine wrap).
  - **By-ear validation confirmed by maintainer 2026-05-18.**
    Mic-to-speakers passthrough audible through the Firewheel graph.
    Firewheel is the right substrate for Hocket; the spine swap
    holds.
- **FT3b-prime, steps 2–5 — full graph implementation landed.**
  - Click sampler (`SamplerNode` + `RepeatMode::RepeatEndlessly`),
    pre-rendered click loop in new `hocket-engine::click` module
    (one bar at 120 BPM 4/4, accented downbeat, 50 ms decay envelope).
  - Capture tap via `StreamReaderNode` + `StreamReaderState`,
    exposed through `Engine::drain_input(&mut Vec<f32>) -> usize`.
  - Replay sampler (`SamplerNode` + `RepeatMode::PlayOnce`),
    triggered via `Engine::play_replay(Vec<f32>)`.
  - Peak meter (`PeakMeterStereoNode { enabled: true }`), exposed
    via `Engine::peak_db() -> [f32; 2]`.
  - Graph wiring: `graph_in → reader (mono tap)`,
    `click_sampler → meter → graph_out`,
    `replay_sampler → meter → graph_out`.
  - Features enabled on `firewheel`: `peak_meter_node`, `stream_nodes`
    (in addition to existing `cpal` + `symphonium`).
  - Workspace builds clean. All 42 tests green
    (15 engine unit incl. 3 new click tests + 3 engine integration +
    22 model unit + 2 model integration).
  - Demo binary scripts a 10-second flow: 4 s click → 3 s capture →
    3 s replay → done. Periodic peak-meter readout every 250 ms.
  - **Open issues for FT3b proper to investigate:**
    1. **Reader rate anomaly.** `StreamReaderState::available_frames()`
       reports ~10× more frames than realtime should produce; the
       3 s capture window collected ~1,425,600 samples (≈30 s at
       48 kHz). Workaround in the demo: truncate the captured Vec
       to the wall-clock-expected sample count before replay.
       Likely root causes to check: `ResamplingChannelConfig` defaults,
       cpal input/output sample-rate mismatch + resampling factor,
       or `read_interleaved` zero-padding on underflow.
    2. **Meter shows `-inf` dB throughout the demo.** Either the
       silence-mask optimization is treating sampler output as silent,
       or audio isn't actually reaching the meter inputs, or the
       0.4-amplitude click is somehow below the meter's threshold.
       Needs investigation — could be a wiring issue or could be a
       Firewheel-side optimization quirk. Resolves to one of:
       wire samplers directly to graph_out (skipping the meter),
       add a `sync_enabled_event` push for the meter, or accept that
       the meter only registers above a higher threshold.
  - **By-ear validation confirmed by maintainer 2026-05-18.**
    Click audible (loop continuous), capture works (mic samples
    flow into `Capture` then into `play_replay`), replay audible
    (the captured 3 s plays back during 7–10 s), meter shows real
    dB values (-8.2 dB on click hits, -inf between, matching the
    50 ms click envelope at the 250 ms meter print cadence). Silent
    monitor confirmed — Mark does not hear his live voice. FT3b-prime
    is **closed**.
- **Two Firewheel 0.10.0 quirks discovered and worked around.** Both
  worth recording for FT3b proper:
  1. **`Notify<T>` post-hoc `sync_*_event` is broken.** `Notify::diff`
     emits a `ParamData::any(Notify<T>::clone)` event only when
     the *counter* differs. But `SamplerNode::sync_play_event`
     produces `ParamData::Bool(true)` — wrong variant —
     and `Notify<bool>::patch` silently rejects it on `downcast_ref`.
     The audio thread never sees the change. **Workaround used:**
     (a) for the click sampler, pass the fully-populated state
     (sample + play=`Notify::new(true)` + repeat_mode +
     play_from=BEGINNING) to `cx.add_node` so the initial state
     carries the change. (b) For the replay sampler where the
     sample arrives at runtime, construct the play event manually
     with `NodeEventType::Param { data: ParamData::any(Notify::new(true)), path: ParamPath::Single(2) }`.

     **Upstream handling: managed locally, no issue filed (2026-05-18).**
     Verified upstream on main branch: bug is still present,
     `sync_play_event` now uses `ParamData::CustomBytes` with a
     manual byte-packed `Notify` ID + value (clearly an attempted
     optimization), and BillyDM has marked it with his own TODO:
     "This is not how `Patch` for `Notify<bool>` is implemented."
     He's aware. Won't pester; we keep our workaround documented
     in code and pull upstream fixes when they land. Recheck on
     each Firewheel release. **Don't fork** — see "Pressure vessel"
     memory; we extract value up to shared crates, not absorb
     upstream infrastructure.
  2. **Firewheel prunes input processing when no path reaches output.**
     A pure-sink reader fed only by `graph_in` doesn't actually
     receive samples — the audio thread skips processing input
     because nothing downstream depends on it. **Workaround used:**
     wire `graph_in → VolumeNode(Volume::SILENT) → graph_out` as a
     silent monitor path. The audio is inaudible, but the input is
     processed, and the reader gets samples in parallel. When live
     input monitoring becomes a real feature in FT3b proper, the
     monitor node becomes the gain control for it.
- **Open issues remaining for FT3b proper to investigate:**
  1. ~~**Reader rate anomaly**~~ ✅ **Resolved 2026-05-18.** Root cause:
     `ResamplingChannelConfig` defaults
     `underflow_autocorrect_percent_threshold: Some(25.0)` and
     `overflow_autocorrect_percent_threshold: Some(75.0)`. These knobs
     are designed for realtime consumers (stream-to-stream piping)
     and inject zero frames on underflow / discard on overflow to
     maintain `latency_seconds` (default 150 ms). Our reader is a
     non-realtime UI-thread drainer. Per fixed_resample docs: "if the
     consumer end is being used in a non-realtime context, then this
     should be set to None." Fix: construct the channel config with
     both autocorrect knobs set to `None` when calling
     `StreamReaderState::start_stream`. Capture now matches wall-clock
     exactly (3 s capture → 144,000 samples at 48 kHz). Demo binary
     reverted to using the `Capture` state machine end-to-end as FT3a
     intended.
  2. **Meter readings depend on click/print phase alignment.** Not a
     bug — the silence-mask optimization writes 0 on silent buffers
     and the meter print samples those buffers stochastically against
     the click rhythm. Future UI will animate via `PeakMeterSmoother`
     anyway, which handles the ballistics cleanly.
  - **API delta worth noting**: Firewheel 0.10.0 on crates.io uses
    `cx.start_stream(CpalConfig{..})` and `cx.stop_stream()` as
    methods on the context, *not* the `CpalStream::new(&mut cx, ..)`
    pattern from main-branch examples. The crate has been refactored
    between 0.10.0 and main. Pin 0.10.x; do not copy main-branch
    example snippets verbatim.
- **Outstanding for FT3b-prime "done":** click sampler (pre-rendered
  click loop via SamplerNode + RepeatMode::Loop), capture tap via
  `stream::reader` → `Capture`, replay sampler for captured buffer,
  peak meter on output bus. Each is a focused graph-node addition;
  each needs API discovery against Firewheel 0.10.0 specifically
  (docs.rs type indexes lack method-level detail, so iteration is
  via cargo errors + reading firewheel-nodes source).
- **Feature Target 3a (capture data plane) — landed.**
  - Two new modules in `hocket-engine`: `media` (MediaStore trait +
    BLAKE3 InMemoryStore, ~140 LOC incl. tests) and `capture` (Capture
    state machine, ~220 LOC incl. tests).
  - One new dep: `blake3` for content hashing. Engine-side only;
    `hocket-model` is unchanged.
  - **FT3 split into 3a/3b/3c.** FT3 as originally written was too
    large for one session (capture + cpal input + bar-phase sync +
    latency calibration + playback). Splitting acknowledges that the
    *data plane* (storage + hashing + state machine + commit) is
    logic that's fully test-validatable, while the *audible* half
    (cpal input, bar-phase sync, playback) is a separate engineering
    problem with its own risks. Plan updated to reflect.
  - **No Looper reuse from woodshed-audio for FT3a.** Woodshed's
    `Looper` is single-bar and has private buffer storage; for FT3a
    we don't need bar alignment (that's FT3b) and we do need access
    to the captured samples for hashing. A plain Vec-backed state
    machine is the right shape. FT3b will revisit whether the Looper
    becomes useful for the bar-phase question.
  - **Capture is not yet on the engine's `Handle`.** It's exposed as a
    standalone public type because FT3a has no audio thread driving
    it — tests feed synthetic samples synchronously. The cross-thread
    integration (and the lock-free question) lands with FT3b's cpal
    input stream.
  - All 17 hocket-engine tests pass (12 unit + 3 integration + 2
    transport carry-overs).
