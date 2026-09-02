# Hocket onto the genet host + chisel leaves (Masonry retirement)

**Status (2026-07-08): LANDED.** Hocket's UI was rebuilt fresh on `xilem_serval`
(genet's third `xilem_core` backend) — not a port of the Masonry surfaces but
the new one-screen loop-recorder design — with the waveform + meter as `chisel`
leaves and the `hocket_engine` audio spine wired in unchanged. The Masonry app
and the `mark-ik/xilem` fork are deleted family-wide (hocket + woodshed). See
the Progress log for the slice-by-slice receipts; the design-forward track ran
S0 (scaffold) → S1 (UI) → S2 (model) → S5 (chisel leaves) → engine → S6 (cut).

Code samples are illustrative unless marked implementation-ready.

**Current-host note (2026-09-02):** Genet retired the Stylo and
`genet-layout` compatibility cone on 2026-08-21. Hocket now consumes the
shared `cambium-genet-winit-host` boundary: it retains the product's views,
Firewheel tick, project/update workers, chisel waveform and meter leaves, and
scenario capture hooks, while Genet owns the desktop lifecycle, Livery/Buckram
layout, paint, input, and Accessibility projection. The old direct
`IncrementalLayout`/`SurfaceHost` assembly and its `stylo_taffy` patch are
retired from this workspace.

---

## Why

The `mark-ik/xilem` fork (`xilem` / `masonry` / `masonry_winit`,
`woodshed-theme` branch) is the last non-genet UI stack in the Merely family.
Woodshed already proved the exit: it migrated its whole app onto `xilem_serval`
(the S0-S5 arc in
`woodshed/design_docs/2026-07-04_genet_host_cross_platform_plan.md`) and, at its
own S5 cut, deleted its Masonry app. But woodshed could not drop the fork,
because two Masonry-coupled shared crates live in its workspace and are still
consumed by **Hocket**:

- `audio-widgets` (`../woodshed/crates/audio-widgets`) — `meter` / `fader` /
  `knob` (+ `waveform`), Masonry `Widget`s with `paint()`.
- `xilem-components` (`../woodshed/crates/xilem-components`) — the domain-neutral
  combobox etc.

Because both are woodshed **workspace members**, their `xilem = { workspace = true }`
resolves against woodshed's `[workspace.dependencies]` even when Hocket builds
them, so the fork cannot leave woodshed while Hocket needs them. **Hocket is
the only live consumer.** Move Hocket off Masonry and both crates lose their
last consumer; then the fork retires from both workspaces. This plan is the
family-wide fork retirement, expressed as a Hocket refactor.

## What moves, and what does not

**Moves (the UI layer only):**

- `crates/hocket` (the Masonry app: `main.rs` + `view/{transport,tracks,combination,settings}.rs`).
- `crates/hocket-widgets` (re-exports + the custom-paint widgets).
- The theme (`hocket-widgets::theme`: `SP_*`, `TS_*`, `palette`, `mono_family`).

**Does not move (the whole audio spine + data):**

- `crates/hocket-engine` — the Firewheel audio graph, capture, the realtime
  thread. UI-agnostic.
- `crates/hocket-model` — pure session/track/layer data.
- `crates/hocket-headless` — the audio test-harness bin.
- `firewheel`, `cpal`, `rtrb`, `basedrop`, the sync/persistence layers.

This split is the whole reason the refactor is tractable and low-risk: **none of
the hard realtime work is touched.** `AppState` reads the model and drives the
engine through plain helper methods (`record`, `stop_all`, `arm`, `undo`,
`select_variation`, `toggle_layer_mute`, `nudge_layer_gain`, …); the views only
call those. Swapping the view engine leaves the audio behaviour a fixed
reference to compare against throughout.

## The surface today (what to map)

Read from the current app (2026-07-08):

- **App shell** (`view/mod.rs`): a persistent transport bar above one of three
  surfaces (`Tracks` / `Combination` / `Settings`), selected by `state.surface`;
  `flex_col((transport, surface))`.
- **Transport** (`transport.rs`): title label, surface-nav buttons (one per
  surface, active marked), status/meter/capture/loops readouts (mono labels), the
  **output meter** (two vertical `meter_view` bars), and controls
  (`Record`/`Stop all`/`Undo`/`Redo` `text_button`s wired to `AppState` methods).
- **Tracks** (`tracks.rs`): a strip per track, dispatched on `playback_mode`:
  - `Sum` (looper): arm `text_button` + summed **`waveform_view`** + expand
    toggle; when expanded, a per-layer row each with mute/gain buttons + the
    layer's own `waveform_view`.
  - `SelectOne` (Deeler): arm + a variation-slot button row (active slot marked).
- **Combination**: the Deeler combination grid (tracks x variation slots).
- **Settings**: session settings (profile, tempo).
- **Custom-paint widgets** (`audio-widgets` + `hocket-widgets`): `waveform`,
  `meter`, `fader`, `knob` — Masonry `Widget` + Xilem `View` pairs. The `knob`,
  for example, paints kurbo arcs + a circle in `paint()`, drives value on
  vertical-drag `on_pointer_event`, sizes 48px in `measure`, announces
  `Role::Slider` in `accessibility`.
- **Reusable structure** (`xilem-components`): the combobox.

## The mapping (Masonry/Xilem -> genet)

| Today (Masonry / Xilem) | Target (genet) |
| --- | --- |
| `flex_col` / `flex_row` / `sized_box` | `el("div", ..)` + CSS flexbox (as woodshed) |
| `label` / `text_button` | native `xilem-serval` `text` / `clickable`/`button` views |
| `OneOf3` / `OneOf2` (surface / strip switch) | `match` -> boxed `AnyView` (as woodshed's tab/lens switch) |
| combobox (`xilem-components`) | native `xilem-serval` `select` view (dissolves) |
| `waveform_view` / `meter_view` / `fader` / `knob` | **`chisel` Path-A leaves** + `xilem-serval` view wrappers |
| `hocket-widgets::theme` (`SP_*`, `palette`, `mono`) | tinct-derived CSS (as woodshed's `theme.rs`) |
| `AppState` + helper methods | **unchanged** (UI-agnostic; action closures call `st.method()`) |
| `hocket-engine` / `hocket-model` / Firewheel | **unchanged** |

The custom-paint widgets are all Path-A (vector shapes the paint vocabulary can
say: bars, arcs, poly-lines), so they stay resolution-independent and tile-cached
and never touch the Path-B texture route. Per the chisel design
(`genet/docs/2026-07-07_chisel_widget_leaf_design.md`), each becomes a `Leaf`
(measure / paint / event / accessibility / paint_dirty) plus a thin `xilem-serval`
view over `chisel_leaf(key)` that diffs typed props (e.g. the knob's `value`) into
the host-owned `LeafRegistry`. The Masonry `paint()` bodies port almost verbatim:
`painter.stroke(arc, ..)` -> `cx.emit(PaintCmd::DrawPath { .. })`, `on_pointer_event`
drag math -> `Leaf::event`, `Role::Slider` -> `Leaf::accessibility`.

## Target crate structure (mirroring woodshed)

```text
hocket-model     (exists)  pure data + UI-agnostic AppState/helpers   [maybe lift AppState here]
hocket-engine    (exists)  Firewheel graph, realtime thread           UNCHANGED
hocket-headless  (exists)  audio test harness                          UNCHANGED
hocket-views     (NEW)     genet views over AppState                 mirrors woodshed-views
hocket-genet    (NEW)     genet winit host (SurfaceHost, redraw)     mirrors woodshed-genet
hocket-widgets   (rewrite) chisel leaves (waveform/meter/fader/knob)  Masonry code deleted
hocket           (app)     the old Masonry bin                         DELETED at the parity cut
```

Decisions / recommendations:

- **The audio chisel leaves start Hocket-owned** (in `hocket-widgets`, rewritten
  on chisel). Woodshed-genet currently uses none of them (it themes via tinct and
  needs no meter/waveform), so there is no second consumer to design for. Extract
  to a shared `audio-chisel` crate only if one appears.
- **`xilem-components` dissolves**, it does not port: its combobox is exactly what
  native `xilem-serval` `select` already provides (woodshed uses it).
- **Two bins coexist during the migration** (`hocket` Masonry + `hocket-genet`),
  the same way woodshed ran `woodshed-xilem` alongside `woodshed-genet` until
  parity. Delete the Masonry bin only at the cut.
- **Reuse woodshed's genet build setup verbatim**: the `[workspace.dependencies]`
  genet/netrender/tinct git deps, the `[patch.crates-io]` stylo mirror pointed at
  `mark-ik/stylo` (`mark-ik/servo-media-features`), and the gitignored
  `.cargo/config.toml` local-genet `[patch]`. **Build hocket-genet from the
  hocket cwd** so the local patch applies (see
  `memory/reference_mere_cargo_cwd_local_genet` — this bit woodshed for ~75 min).

## Dependencies and gates

- **chisel live runner wiring (genet-side).** The chisel leaf layer is scaffolded
  and its render path is proven in tests, but the on-screen path is gated on
  wiring `LeafRegistry` ownership into `xilem-serval/runner.rs` (a concurrent
  rewrite) plus a headed smoke (chisel doc "Next" 1-2). **The structural migration
  does not need chisel** (it uses only native views, already shipped). So sequence
  the structure first (unblocked now) and the widget->chisel conversion after the
  runner wiring lands; until then, render placeholder widgets (a CSS bar for the
  meter, a static peaks poly-line for the waveform).
- **tinct** for theming (shared crate, already woodshed's).
- **genet / netrender git deps** must eventually be pushed for reproducible /
  CI builds; local development rides the local checkouts via the `[patch]`.

## Plan (slices; keep an app runnable throughout)

Organized by feature target + done-condition, not calendar. Each slice keeps
either the Masonry `hocket` (until S6) or `hocket-genet` runnable.

- **S0 - scaffold `hocket-genet`.** New winit + `SurfaceHost` bin (copy
  woodshed-genet's host skeleton: boot, rasterize, acquire, compose, present;
  incremental layout; key/pointer routing; optional CSD chrome). *Done:* a themed
  placeholder genet window opens via `cargo run -p hocket-genet` from the
  hocket cwd; `cargo run -p hocket` (Masonry) still works.
- **S1 - AppState reachable + tinct theme.** Confirm/lift `AppState` to a
  UI-agnostic home (hocket-model or a new hocket-core) if it carries any Masonry
  coupling (e.g. a peniko-typed `palette`). Port `hocket-widgets::theme` to tinct
  seeds -> CSS. *Done:* hocket-genet renders the transport bar's static content
  (title, status, nav) from `AppState`; clicking nav switches `state.surface`.
- **S2 - Transport surface.** Full transport as native views: nav, the readout
  labels, and controls (`Record`/`Stop`/`Undo`/`Redo`) wired to `AppState`
  methods. Meter = placeholder CSS bar. *Done:* record/stop/undo/redo drive the
  engine identically to the Masonry app; nav switches surfaces.
- **S3 - Tracks surface.** Sum + SelectOne strips (arm, expand, per-layer
  mute/gain, variation slots) as native views. Waveform = placeholder poly-line.
  *Done:* arming / expanding / variation-select drive `AppState`; the strip layout
  matches the Masonry app.
- **S4 - Combination + Settings.** The Deeler grid (CSS grid; an arrangement leaf
  only if the grid needs paint the vocabulary cannot say, which it likely does
  not) + settings. *Done:* all three surfaces at structural parity with Masonry.
- **S5 - chisel leaves** (gated on chisel's runner wiring). Convert
  `waveform` / `meter` / `fader` / `knob` to chisel Path-A leaves + view wrappers;
  swap out the placeholders. *Done:* real meters / waveforms / knobs render and
  interact (knob/fader drag) via chisel; a headed smoke shows them live and a
  `paint_dirty` test shows an unchanged leaf produces zero repaints.
- **S6 - parity cut (fork retirement).** Delete the Masonry `hocket` bin and
  `hocket-widgets`' Masonry code; drop `audio-widgets`, `xilem-components`, and
  the `xilem`/`masonry`/`masonry_winit` deps from Hocket's `Cargo.toml`. With no
  consumer left, retire `audio-widgets` + `xilem-components` in the woodshed repo
  and drop the fork from woodshed's `[workspace.dependencies]` too. *Done:*
  `grep -ri masonry` is clean across the hocket + woodshed workspaces; both build
  genet-only, single wgpu tree. **The `mark-ik/xilem` fork is retired.**

## Validation

- The audio engine + model are untouched, so audio output is a fixed reference:
  A/B the Masonry `hocket` and `hocket-genet` for identical audio behaviour at
  each slice.
- Each slice ends with a runnable app and (from S2) a driven receipt of the ported
  surface, the same headed-verify discipline woodshed used.
- Build from the hocket cwd; keep the stylo mirror in sync with genet.

## Findings (verify during execution)

- **`AppState` home + coupling.** It lives in the `hocket` app crate today
  (`crate::AppState`); its helpers look pure but `state.palette` may be a
  Masonry/peniko palette. Confirm and lift the UI-agnostic part to a core crate;
  re-derive `palette` from tinct.
- **Widget inventory + homes.** `waveform_view` / `meter_view` are re-exported by
  `hocket-widgets`; `meter` / `fader` / `knob` live in `audio-widgets`;
  `waveform` home to confirm. Enumerate the full custom-paint set before S5.
- **chisel runner-wiring status.** The S5 gate; coordinate with the genet-side
  chisel work (its "Next" 1-2). Structure slices (S0-S4) run ahead of it.
- **Combination grid** paint needs (native CSS grid vs. an arrangement leaf).

## Open questions

1. Audio chisel leaves Hocket-owned (`hocket-widgets`) vs. a shared
   `audio-chisel` crate. Recommend Hocket-owned first; extract on a second
   consumer.
2. Coexisting bins during migration acceptable (woodshed precedent says yes).
3. CSD chrome (own title bar, as woodshed) vs. OS decorations for hocket-genet.
4. Does any Hocket widget need Path B (a `vello::Scene` / shader) rather than
   Path A? Current set (bars, arcs, poly-lines) is all Path A.

## Progress

- 2026-07-08: **S6 done — the `mark-ik/xilem` fork is retired family-wide.** With
  the genet app at parity (UI + audio), the Masonry stack is deleted:
  - **hocket** (`ddf5d65`): rm `crates/hocket` (the Masonry bin) +
    `crates/hocket-widgets`; drop `xilem` / `masonry` / `masonry_winit` +
    the cross-repo `audio-widgets` / `xilem-components` path-deps. Kept
    `audio-primitives` (pure DSP, used by `hocket-engine`). Genet-only,
    single wgpu 29 tree; a pre-existing `hocket-headless` `CapturePhase` match
    was fixed forward.
  - **woodshed** (`35ac0c0`): rm `crates/audio-widgets` + `crates/xilem-components`
    (the fork's last consumers) + drop the fork deps. woodshed builds genet-only.
  Verified: `grep -ri masonry` clean (bar historical comments) and both
  workspaces build in one genet/wgpu-29 tree. **The plan's goal is met.**
  Deferred to a spin-out: the Deeler combination + settings surfaces,
  idle-tick throttling, and cross-platform validation (iMac / Fedora / Mint).
- 2026-07-08: **Audio engine wired — hocket-genet makes sound (parity with the
  Masonry app's core function).** `state.rs`'s `AppState` now owns a live
  `hocket_engine::Engine` + `InMemoryStore`, mirroring the Masonry app's glue:
  `toggle_record` arms a bar-aligned (or free) capture, `tick()` advances the
  engine ~60fps and promotes a completed capture into a real `AppendLayer` +
  looping playback, and arm / mute / tempo / click drive the engine
  (`set_click_enabled`, `set_tempo`, `stop/play_layer`). The host drives the tick
  from a winit `WaitUntil` timer via `runner.update(|s| s.tick())` (mutate +
  re-diff). `is_recording()` derives the record light + rail state from the real
  `CapturePhase` (no more manual flag). The output meters read the engine's
  `peak_db()` — a visible audio-flow signal. Demo layers stay silent placeholders
  (`MediaRef::ZERO`); real captures are audible. **Verified end to end:** Mark
  heard the metronome (engine + click + device confirmed by ear); pressing Record
  showed the count-in/recording phase (red light, rail "recording", red lane) and
  completed into a real captured layer (Guitar 3 → 4) that loops — the
  capture → store → AppendLayer → playback chain. This clears the S6 blocker:
  retiring the Masonry app no longer loses audio. Follow-ups: real per-layer peak
  data into the waveform leaves (needs `compute_peaks` lifted out of the
  Masonry-coupled `hocket-widgets`); input-monitor + count-in click polish;
  idle-tick throttling (currently a steady 60fps like the Masonry app).
- 2026-07-08: **S5 done — the waveforms + meters are chisel leaves.** The signature
  visual (each track's summed loop) is now a chisel Path-A leaf — a filled,
  mirrored amplitude envelope, resolution-independent and tile-cached — replacing
  the CSS-bar stand-in; the L/R output bars are chisel's built-in `Meter` leaf
  (teal fill, amber peak). `leaves.rs`: a `WaveformLeaf` (`Leaf`: measure / paint
  via `Path` + `fill_path` / paint_dirty), a key scheme (track index → wave key;
  a small meter namespace), a `reconcile(registry, &AppState)` that ensures/updates
  leaves from the session each frame (peaks lazily re-seeded only when a track's
  audible-layer signature or colour moves, so the retention gate holds), and a
  `LeafPaintSource` newtype forwarding `RenderedLeaves::get`. The host
  (`main.rs`) owns a `LeafRegistry<u64>` + `RenderedLeaves`; redraw reconciles,
  sizes leaves from `chisel_leaf_boxes()`, `render_into`s the dirty ones, and
  emits via `emit_paint_list_with_leaves`. The view places `<chisel-leaf key=…>`
  boxes. Per-layer mini-rows stay CSS bars (tiny; not worth a leaf each).
  Verified headed: the filled envelopes render in owner colours; committing a
  take (Guitar 3→4 layers) re-seeded its envelope (repaint-on-content-change);
  the meters render. **hocket is the second real chisel consumer** (after
  meerkat's grid/arrangement), exercising the leaf on-screen path end to end.
  These waveform follow-ups landed 2026-07-11; see the newer progress entry.
- 2026-07-11: **Real responsive waveforms + meter ballistics landed.** Shared
  `audio-primitives` now extracts signed min/max columns and advances
  configurable meter attack/release/peak hold. `hocket-engine::waveform`
  projects real stored layers and summed track mixes through the same source
  selection/repeating-loop semantics as export. The Genet host caches by
  content identity/signature, uses stable track/phrase-derived leaf keys,
  renders summed and per-layer responsive Chisel leaves, labels missing media,
  releases stale leaves, and feeds held peaks to Chisel meters. The old
  deterministic silhouettes and per-layer CSS bars are retired.
- 2026-07-08: **S2 done — the UI runs on the real model.** `state.rs` introduces
  `AppState`: a `hocket_model::Session` + its `History`; every data-bearing
  gesture commits a real `Edit` (`ArmTrack`, `AppendLayer`, `SetLayerMute`,
  `MuteTrack`, `SetBpm`, `SetMasterClock`), so undo/redo and the future sync
  layer see exactly what the UI did. The demo session is seeded through the same
  commits (renames, colours, layers), not hand-built structs. `view.rs` derives
  everything from the session: lane names/colours/stacks, the tracks chip, tempo
  (now the model's 120 default), meter, the record label (armed track's name),
  toggle states. Waveform stand-ins seed per layer from its `PhraseId`, so shapes
  are stable per take; the summed wave excludes muted layers. App-local (marked
  in `state.rs` with graduation notes): the live-capture flag (stopping commits
  an `AppendLayer` with placeholder media until the engine slice), the audible
  click, and solo (no model backing yet). Rail peers stay placeholder until sync.
  Verified by driving the running app (PostMessage to hocket's own hwnd —
  targeted client-coordinate clicks, cannot touch other windows): record stop
  appended L4 to Guitar; tempo stepped 120→124; arming Bass mid-capture stopped
  and committed the Guitar take and moved the arm (teal border followed); a layer
  tap dimmed L1 and recomputed the summed wave. Inert until their slices: solo
  audibility, stop button, add-track (no `AddTrack` edit in the model yet),
  hand-off.
- 2026-07-08: **Two of the S1 quirks were genet bugs — fixed upstream**
  (genet `dab0ee5`, regression tests included, 267-test suite green):
  - `:root` now matches the root element (`is_root` tested `parent().is_none()`,
    but the root's parent is the document *node*; it now tests parent-element
    semantics). Palette vars on `.app` are unchanged since `.app` *is* the root,
    but `:root` works from here on.
  - A single-side border shorthand (`border-bottom` etc.) no longer paints a
    phantom 3px `currentColor` border on the other edges — the paint path now
    zeroes a none/hidden side's width per CSS 2.1 §8.5.1, matching taffy.
  `theme.rs` dropped the transparent-border workaround for natural
  `border-<side>: 1px solid var(--line-soft)` dividers; render re-verified
  edge-clean against the fixed genet. The `min-height: 0` nested-flex note is
  standard CSS behaviour, not a bug — it stays.
- 2026-07-08: **S1 done — the approved loop-recorder UI renders in genet.**
  `view.rs` + `theme.rs` build the one-screen design from the 2026-07-08 concept:
  the pass-the-mic **rail** (the circle: You/Jonah/Mara/Eli + Hand off), the
  **loop table** (four owner-coloured lanes — Guitar/Bass/Drums/Keys — each an
  overdub stack of layer waveforms, muted layers dimmed, plus M/S/loop controls
  and an empty-state), and the **transport** (tempo/meter, click + master-clock
  toggles, the big Record, the output meter). Data is static this slice (S2 wires
  `AppState`); two interactions are live end-to-end (tap a lane dot to arm, tap
  Record to toggle capture). Waveforms are lightweight DOM bar stand-ins — S5
  swaps them for chisel leaves. Verified by client-area screenshot: faithful to
  the mockup, all four tracks + add-track on one screen, every edge clean.
  Three genet-CSS lessons banked (all worked around in `theme.rs`, none needed
  a host change):
  - **genet doesn't match `:root`.** Palette custom properties live on `.app`
    (the actual root element); on `:root` they never inherit. Inline `var()`
    (`--voice` per lane) and `.app`-scoped vars both cascade fine.
  - **Single-side border shorthands emit a phantom.** `border-bottom` /
    `border-right` / `border-top` paint a spurious 3px `currentColor` border on
    the *opposite* edge (a cream frame at the viewport rim here). The full
    four-side `border:` shorthand is clean, so dividers use
    `border: 1px solid transparent` + a `border-<side>-color` longhand. Worth a
    genet-layout paint fix later; the workaround is documented inline.
  - **Nested flex needs explicit `min-height: 0`** for the loop table to scroll
    internally instead of shoving the transport off-screen (`.body`/`.table`).
- 2026-07-08: **S0 done — `hocket-genet` scaffolded and building.** New winit +
  `SurfaceHost` bin (retained `IncrementalLayout` redraw + click dispatch),
  rendering a themed placeholder with a working counter (render + hit-test +
  dispatch + update + repaint proven). Built from the hocket cwd (9m cold,
  genet stack); the Masonry `hocket` bin still checks green, now riding the
  **git** xilem fork (`mark-ik/xilem@woodshed-theme`). Workspace wiring: genet /
  netrender / tinct git deps + a `[patch.crates-io]` stylo mirror to
  `mark-ik/stylo` (matching genet + woodshed). `.cargo/config.toml`
  (machine-local): dropped the old `paths = [woodshed, xilem-woodshed]` override
  (it would hijack genet's vendored `xilem_core` by name) for source-specific
  genet `[patch]` entries; the Masonry app rides git xilem instead of the
  worktree. **Both bins build.**
- 2026-07-08: **S5 unblocked.** The genet vector-widget / chisel-leaf path is now
  covered end to end, so S5 (converting `waveform`/`meter`/`fader`/`knob` to
  chisel leaves) no longer waits behind a runner-wiring gate; the real leaves can
  land when the slice arrives, and the S2-S4 placeholders are optional rather than
  necessary.
- 2026-07-08: Plan created. Grounded against the current Masonry app
  (`view/{mod,transport,tracks}.rs`, `audio-widgets/knob.rs`), the chisel design
  (`genet/docs/2026-07-07_chisel_widget_leaf_design.md`), and woodshed's shipped
  genet migration.
