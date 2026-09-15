# Redshank GUI implementation plan

**Status (2026-09-13): ACTIVE.** Implements the endorsed Claude Design canvas
"Redshank Mockups" (project `c21e20cb-d4a7-46eb-99df-628b89869526`, two turns,
eighteen artboards) across the sovereign desktop, a new browser host, and the
form factors the canvas names. This plan is the implementation half of the
[listening and annotation port plan](2026-09-01_listening_annotation_port_plan.md)'s
"GUI evidence brief" and "GUI design done-conditions"; that plan remains the
product authority and this one owes it a closing receipt.

Rulings taken from Mark on 2026-09-13:

- scope is everything on the canvas, with what the stack cannot carry reported
  as blocked rather than faked;
- three extra lanes run beside the surface work: a self-drive scenario/capture
  lane, the family B transport rail for tablet, and a `redshank-web` host;
- the launch seed is wetland (`.t-redshank`); brand shell (`.t-dark`) is a scope
  class and a Settings choice;
- controls the design shows but the model lacks are backed by extending the
  model, not omitted and not rendered inert;
- subagents run in parallel lanes, never on the Fable model, and no two lanes
  edit the same file; builds, tests, and headed receipts run sequentially in
  the orchestrating session.

## The design, as rules

The canvas states its rules in prose; they are restated here so a lane can be
checked against them without opening the canvas.

1. **One dock, fixed height.** Identity row, seek track, transport-and-capture
   row. Recording, buffering, completed, and unavailable states swap the
   transport row's contents in place. Nothing grows.
2. **Ember is now/here.** `--t-tertiary` marks only the playhead and a frozen
   capture anchor.
3. **Recording is a word plus `--t-danger`**, never colour alone.
4. **Selection reads by ink weight** (Segment law): the selected tab or segment
   is `--t-text` on `--t-bg`; an active row carries a 2px inner ring.
5. **Two seeds, one markup.** `.t-redshank` (wetland greens) and `.t-dark`
   (brand shell) differ only by the `--t-*` role values on the scope class.
6. **Three widths, one dock.** Wide (≥ 900px): Queue and Notes side by side,
   tabs in the header. Narrow (600–900px): one work view, tabs collapse to a
   segment inside the view. Phone (< 600px): destinations move to a bottom bar
   under the dock; the work view scrolls, the dock does not.
7. **Tablet landscape takes family B**: the transport rail on the left carries
   identity, a vertical seek track, transport, and capture; tabs move to the
   top of the work area.
8. **Feed subscriptions are nodes**: artwork face, unplayed badge, and a
   chronological chain of episode nodes; notes hang off their episode as small
   gnodes. The Library roster, the Mere chain, the phone chain, the orrery, and
   the trail are five projections of that one structure.
9. **Capture must not depend on navigation**: the dock, the rail, the phone
   bar, and the media card all carry Text note and Voice note next to
   transport.

## Stack facts that shape the work

Verified against the pinned engines on 2026-09-13:

- Livery supports media queries (`min-width`/`max-width`), grid, custom
  properties, `color-mix`, keyframes, dashed borders, `box-shadow`,
  `transform`, `position: sticky`, `letter-spacing`, `text-transform`,
  `opacity`, `z-index`. The responsive rule is therefore plain CSS.
- Livery has **no `outline`** and **no `text-overflow: ellipsis`**. Active
  rings use `box-shadow: inset 0 0 0 2px` and truncation uses
  `overflow: hidden; white-space: nowrap`. This is recorded so a later engine
  slice can retire the substitution.
- Cambium ships `slider` (drag-to-set track with an absolute thumb), `select`,
  `textarea`, `on_pointer`, `on_key`, `on_hover`, and `GraphCanvasSwatch` over
  Sprigging. It has no Segment, Stepper, Toggle, or Readout control; the
  design-system specimens of those contracts exist only as HTML, so Redshank
  builds them as CSS on native buttons. If the shared library later grows the
  controls, Redshank adopts them and deletes its local copies.
- `tinct` derives a full `--t-*` role palette from primary/secondary/tertiary
  seeds and a mode. Both seeds are derived at build time into the sheet
  rather than hand-copied from the canvas, so the wetland and brand palettes
  stay one function of their seeds.
- The host frame hook exposes `logical_size` and a `capture` slot, and the
  winit host exports `read_frame`. A Redshank scenario lane mirrors
  `woodshed-genet/src/scenario.rs` over `genet-probe`.
- `cambium-genet-web-host` exists and `woodshed-web` is its consumer template.
  The browser has no cpal, no file system, and no microphone adapter in the
  stack yet, so the web host mounts the surfaces over an explicit
  unavailable-playback runtime and says so.
- IBM Plex Sans and Mono are the ruled UI families. They are not installed on
  the build machine; genet-livery registers fonts by bytes. The theme lane
  bundles the OFL faces if the host exposes registration, and otherwise falls
  back to `sans-serif` / `monospace` and records the gap here.

## Lanes

Each lane owns the files listed and nothing else. Shared vocabulary (state
types, commands, CSS class names) is fixed in the contract section below before
any lane starts, so lanes compose without editing each other's files.

### Lane V — vocabulary and skeleton (orchestrator, sequential)

Splits `surfaces/src/lib.rs` into modules with the state and command
vocabulary the design needs, plus the CSS class contract. Every other lane
fills bodies against it.

### Lane M — model, playback, session, desktop dispatch

Files: `model/src/lib.rs`, `playback/src/*.rs`, `desktop/src/session.rs`,
`desktop/src/main.rs` (dispatch and projection only).

- `ListenerSettings` gains: `playback_rate` (f32 percent as `u16`), `volume`
  (`u8`), `reaction_offset_ms`, `resume_after_capture`, `note_privacy`
  (`Private | Shareable`), `refresh_schedule` (`Manual | Hourly | Daily`),
  `auto_download`, `auto_reclaim`, `seed` (`Wetland | BrandShell`), `mode`
  (`Dark | Light | HcDark | HcLight`). All `#[serde(default)]` so existing
  stores load.
- `TimedTarget` gains `end_offset_ms: Option<u64>` for span annotations, and
  `Annotation` gains `privacy`. Both default so stored notes remain valid.
- `RedshankModel` gains `listening_sessions: Vec<ListeningSession>` (item,
  start/stop offsets, wall-clock start/end) recorded by the session on select,
  pause, end, and close. This backs the trail.
- Playback gains `SetRate(u16)` and `SetVolume(u8)`. Volume applies a
  Firewheel gain. Rate is applied where the backend can; where it cannot the
  snapshot reports the effective rate so the dock shows the truth.
- `FeedSubscription` unplayed count, episode count, and offline byte total are
  computed by the session projection, not stored.
- `desktop/src/session.rs::project` fills the new presentation facts the
  surfaces contract names (source kind, listened fraction, completed, note
  markers, feed groups, listening sessions, effective rate/volume).
- `desktop/src/main.rs` handles every new `CompactCommand` variant.

Done when the isolated workspace tests pass, a stored 2026-09-13 model loads
unchanged, and a span note round-trips through the store.

### Lane S1 — shell, dock, rail, phone bar, seek track, theme sheet

Files: `surfaces/src/shell.rs`, `surfaces/src/dock.rs`, `surfaces/src/rail.rs`,
`surfaces/src/theme.rs`, `surfaces/src/redshank.css`, and the font assets
under `surfaces/assets/`.

- Header with the Merely mark reduction and the five destinations as a
  bordered segment; narrow width collapses Settings to a gear glyph.
- Family A dock: identity row (artwork face or format tile, source and
  transport words in tracked mono microlabel, title, `pos / dur`), the seek
  track with buffered extent, listened extent, note ticks, span washes, and the
  ember playhead, then transport (−N, Play/Pause/Replay, +N, rate, volume) and
  capture (Text note with `N` kbd, Voice note with `R · hold` kbd). Recording
  swaps the row to `RECORDING · elapsed · anchor · Finish · Cancel`; buffering
  disables transport; unavailable shows the item-scoped message and a retry.
- Family B rail: same facts and commands, vertical seek track, selected by the
  host class `rs-rail`.
- Phone bar: five destinations under the dock, selected by the phone media
  query.
- `theme.rs` derives both seeds through `tinct` and emits the scope classes;
  the sheet consumes only `--t-*`, `--font-*`, `--space-*`, and `--text-*`
  tokens. Mode classes `.t-light`, `.t-hc-light`, `.t-hc-dark` derive the same
  way.

Done when the dock's laid-out height is identical across Empty, Playing,
Paused, Buffering, Recording, Completed, and Unavailable fixtures in a headless
layout test, and the Segment law holds in the DOM.

### Lane S2 — Listen, Library, Notes, Settings tab bodies

Files: `surfaces/src/tabs/listen.rs`, `tabs/library.rs`, `tabs/notes.rs`,
`tabs/settings.rs`.

- Listen: Up next rows (face, title, listened bar, source badge, overflow
  menu, active ring) beside Notes rows (type/anchor column, body or voice
  bars, Open at, overflow); narrow width shows one with a segment.
- Library: subscription roster rows (face with unplayed badge, counts,
  refresh-failed with retry), Local audio row, episode list grouped under the
  selected feed with one primary action (Play / Playing / Resume / Replay),
  BUFFERING/CLOUD/OFFLINE badges, quiet Subscribe field and `Ctrl O` hint.
- Notes: episode header with `this episode / all notes` segment and Export
  W3C, timeline strip with point ticks, a count chip per cluster (notes within
  ten seconds), span washes; cluster list (collapsible), span card (Play span,
  Edit, Share), representation card, Add text note at end.
- Settings: four sections (Playback, Capture, Storage, Feeds · Appearance)
  using Stepper, Segment, Toggle, Readout built as CSS on native buttons, every
  row emitting `UpdateSettings`.

Done when every artboard fact in 1d, 1f, 1h, 1i, 1k, 2a, 2b has a DOM
counterpart in a headless test and every control emits a real command.

### Lane S3 — Mere tab and the feed-node projections

Files: `surfaces/src/scene/mod.rs`, `scene/chain.rs`, `scene/orrery.rs`,
`scene/trail.rs`.

- Chain (1g wide, 2c phone): feed face at the head, episodes as 20px squares
  linked left-to-right (wide) or top-to-bottom (phone), listened/in
  progress/now/dashed-next states, note gnodes hanging below, a context card
  summoned beside the selected episode with prev/next/notes/representation
  and Open / Pin / Add to queue.
- Orrery (2f): feed at the centre, episodes on a ring clockwise by date,
  newest at twelve, notes as satellites of the playing episode, a selected
  connections card.
- Trail (2g): listening sessions newest first, sectioned by day with minutes,
  each card washed from start to stop with note dots on the wash.
- A `scene` segment selects among them; the Mere tab hosts them at wide width
  and the Library feed page hosts them on phone.

Wide-width edges use `GraphCanvasSwatch` where it fits; otherwise absolutely
positioned DOM boxes with CSS lines. Done when each projection renders the
same fixture feed and the selected episode's card lists the same connections.

### Lane P — scenario and capture lane

Files: `desktop/src/scenario.rs`, `desktop/Cargo.toml` (probe and image
deps), `scenarios/*.scn` under the port, `Code/testing/woodshed/redshank-run-scenario.ps1`.

- `REDSHANK_SCENARIO`, `REDSHANK_CAPTURE_DIR`, `REDSHANK_WIDTH`,
  `REDSHANK_HEIGHT` over `genet-probe`, mirroring the Woodshed lane; captures
  are in-process readbacks of the presented frame.
- Fixture scenarios seeded through `REDSHANK_DATA_DIR`: local file playing with
  two text and one voice note; remote buffering then playable; unavailable
  cloud placeholder; completed item; active recording; ten clustered notes
  plus one span; empty state; long names at 200% and 400% zoom. Each captured
  at 960, 720, and 412 logical widths, and 820×560 for the rail.

Done when `redshank-run-scenario.ps1` produces `scenario.done` with `RESULT ok`
and the PNG set for each fixture.

### Lane W — redshank-web

Files: `web/Cargo.toml`, `web/src/lib.rs`, `web/www/index.html`,
workspace member registration.

- Mounts `redshank_surfaces::surface` over `cambium-genet-web-host` with the
  same sheet, an in-memory model seeded from a fixture, and an explicit
  unavailable playback runtime.
- Build and bindgen commands recorded in the crate doc.

Done when the wasm target builds and the page renders the Listen tab in
Chromium at phone and wide widths.

### Lane R — receipts (orchestrator, sequential)

Foreground workspace build, tests, strict Clippy, then the scenario captures,
examined frame by frame against the artboards. Closes this plan and writes the
receipt into the port plan's progress log.

## Contract

### State vocabulary (`redshank-surfaces`)

- `SurfaceTab`: `Listen | Library | Notes | Mere | Settings`.
- `Layout`: `Dock | Rail`, set by the host from width and aspect.
- `Scene`: `Chain | Orrery | Trail`.
- `SourceKind`: `Local | Cloud | Offline | HostBlob` for badges.
- `ItemRow`: id, title, feed title, face (artwork url or format tag), source
  kind, listened fraction, completed, cached bytes, note count.
- `FeedRow`: feed url, title, subtitle, face, episode count, unplayed count,
  offline bytes, last refreshed, failure text.
- `NoteMarker`: id, offset, optional end offset, kind.
- `ListeningSessionRow`: item id, title, start/stop offsets, day label, wall
  clock, note offsets, completed.
- `CompactPlayerState` gains: feed title, face, source kind, resumed-from,
  buffered fraction, markers, rate, volume, recording elapsed and anchor,
  completed.

### Commands added to `CompactCommand`

`Seek(u64)`, `Replay`, `SetRate(u16)`, `SetVolume(u8)`, `CancelVoiceNote`,
`SelectFeed(String)`, `SelectScene(Scene)`, `SetLayout(Layout)`,
`SelectTab(SurfaceTab)`, `PlaySpan { id }`, `ExportAnnotations`,
`RetryItem(ItemId)`, `PinItem(ItemId)`, `SaveSpanNote { anchor, end_offset_ms,
plain_text }`.

### CSS class contract

Prefix `rs-`. Root: `rs-app` plus scope (`t-redshank` | `t-dark`) plus layout
(`rs-dock-layout` | `rs-rail-layout`). Header `rs-header`, segment
`rs-segment` / `rs-segment-on`, work area `rs-work`, panel `rs-panel`, row
`rs-row` / `rs-row-active`, face `rs-face`, badge `rs-badge`, microlabel
`rs-micro`, dock `rs-dock`, seek `rs-seek` with `rs-seek-buffered`,
`rs-seek-played`, `rs-seek-tick`, `rs-seek-span`, `rs-seek-head`, transport
`rs-transport`, capture `rs-capture`, kbd `rs-kbd`, recording `rs-recording`,
rail `rs-rail`, phone bar `rs-phone-bar`, controls `rs-stepper`, `rs-toggle`,
`rs-readout`, scene `rs-scene`, node `rs-node` with state modifiers
`-listened`, `-progress`, `-now`, `-next`, gnode `rs-gnode`, context card
`rs-card`.

## Pass 2 lanes (2026-09-14)

Mark ruled on 2026-09-14 that all four follow-on lanes run. Lane F and Lane T
cross repository boundaries; each irrevocable step (a push, a pin bump) is
signed off separately.

### Lane F — Plex fonts through a Mere seam, and the pin bump

Files: `repos/mere/crates/cambium/cambium-rootstock` (an `Init.fonts`
field forwarded into the Livery text system, and an `Init.images` seam if the
image-source API has the same shape), then in a worktree of woodshed:
`ports/redshank/Cargo.toml`, `Cargo.lock`, every crate manifest that names a
Mere or Genet rev, `desktop/src/scenario.rs` (genet-probe became `taproot` in
the Genet gap), `surfaces/src/theme.rs` and `surfaces/assets/fonts/` (Plex
Sans and Mono under OFL, exposed as `FONTS`), and the desktop and web hosts'
`Init` construction. Verified first against the local Mere checkout through a
temporary `[patch]`, then pushed and pinned after sign-off.

Done when Redshank builds against Mere HEAD with no local patch, the headed
receipts render in Plex, and the scenario lane still passes its matrix.

### Lane K — pitch-preserving rate

Files: `crates/audio-primitives/src/stretch.rs` (a WSOLA time-stretch kernel,
pure std, with tests for ratio, continuity, and reset), `ports/redshank/
playback/src/backend.rs` and `output.rs` (the stage between decode and the
sink, source-time position mapping through the stretch, `SetRate` applied
live), `ports/redshank/Cargo.toml` (the root-crate path dependency, as Hocket
does).

Done when 0.8×, 1.0×, 1.2× and 1.5× play the fixture at the right wall-clock
duration within 2%, the reported position stays in source time, and the
snapshot reports the effective rate the listener chose.

### Lane D — in-repo design completions

Files: `surfaces/src/lib.rs` (new state: `listen_pane`, `expanded_clusters`,
`open_menu`), `tabs/*`, `scene/*` where menus apply, `redshank.css` /
`TABS_CSS` for the new rules, `model/src/lib.rs` (`pinned`,
`resume_completed_from`, a representation summary), `desktop/src/session.rs`
and `main.rs` (projection and dispatch), `web/src/lib.rs` (apply the new
commands).

- Overflow menus on queue, note, feed, and episode rows through Cambium's
  `detail_popover`, one open menu at a time.
- Cluster collapse on the Notes timeline; the phone Listen segment (Up next |
  Notes) as real state.
- "Resume completed items from" as a setting the selection policy honours.
- The representation card from a projected digest and retrieval date.
- Pin as a model concept the Mere overview shows.
- An artwork probe: whether Livery paints `background-image: url()` through
  the Cambium host; if not, the gap is recorded for a rootstock image seam.

Done when each control emits a real command, the headless tests cover them,
and the scenario captures show them.

### Lane T — Turnstone as second host (Phase 7)

Runs after Lane F lands, because Turnstone is on Mere HEAD. Files in
`repos/turnstone`: a contributed surface beside `knot_document_surface.rs`
that mounts `redshank_surfaces::compact_surface` under an episode page, a
handler for podcast enclosures, and the graph projection of one item,
progress, and note. Redshank is consumed as a git dependency on woodshed.git.

Done when the port plan's Phase 7 conditions hold: enclosure opened through
handler routing, the compact dock appears without a copied implementation,
one item, progress, and note reopen after restart, and the conformance tests
run against the same contract.

## Progress

- **2026-09-13:** Plan written after reading the canvas, the port plan's GUI
  brief, the live surfaces, and the pinned engine capabilities. Scope, lanes,
  seed default, and model extension ruled by Mark.
- **2026-09-14:** All lanes landed and integrated. Lane V split the surfaces
  crate; Lane M extended the model (ten settings fields, span targets, note
  privacy, bounded listening sessions, W3C export) and playback (a real
  Firewheel volume gain; rate recorded but honestly reported as 100% because
  Symphonia cannot time-stretch); Lane S1 derived both seeds through tinct
  (brand shell exact, wetland dark neutrals overridden to the endorsed values
  with drift recorded in `theme.rs`), built the dock, rail, phone bar, and the
  sheet, and proved the dock height identical across all seven transport
  states by headless layout at 960 and 412 wide; Lane S2 built the four tab
  bodies with Stepper/Segment/Toggle/Readout on native buttons; Lane S3 built
  chain, orrery, trail, and the connections card; Lane P added the
  `REDSHANK_SCENARIO` lane, seven fixtures, eight scenarios, and the matrix
  driver; Lane W added `redshank-web`, which builds for wasm32 and serves.
  Integration fixed what only rendered frames showed: `outline`-free rings,
  settings segments pushed off-screen by `margin-left: auto`, undefined tokens
  in the scene sheet, buttons centred by padding rather than flex, the
  emoji-prone play glyph, empty absolute spans collapsing in the notes strip,
  and the rail band narrowed to 700–900 px landscape so the 960×640 desktop
  artboard takes the dock. Workspace: 140 tests, strict Clippy clean. Receipts:
  `listen_playing` at 960×640, 720×640, 412×892, 820×560, plus `library_feed`,
  `notes_cluster`, `completed`, `unavailable`, `empty`, `longnames` all
  `RESULT ok` under `Code/testing/woodshed/redshank-scenarios/`.
- **Open after this pass:** IBM Plex needs a font seam in `cambium-rootstock`
  (system faces until then); pitch-preserving rate needs a retiming stage;
  Pin has no model concept; the media-notification card (2e) is OS-level and
  outside Cambium; the Turnstone tile (1j) waits on Phase 7; cluster collapse,
  "resume completed from", and the representation digest card need the state
  fields Lane S2 listed; `recording.scn` needs a microphone to pass.
- **2026-09-14, pass 2:** Lanes F, K and D landed and merged (woodshed
  `3bf0e10`, `0c28fc3`, `882dc75`). The Cambium host font and image seam
  (`Init.fonts`, `Init.images`, `HostState::set_resources`, registration on
  every text-system construction) is on Mere main at `1009f02d`, swept into a
  concurrent session's receipt commit and followed by its Ahem-based test in
  `e5b6eac3`; Redshank pins that rev and Genet `7baa554c`, where `genet-probe`
  became `taproot`, and bundles IBM Plex Sans and Mono (OFL) as
  `theme::FONTS`, so receipts now shape in Plex (dotted-zero Plex Mono against
  the earlier slashed-zero fallback). Rate is real: a WSOLA kernel in
  `audio-primitives` with output-to-source position mapping, within 0.84% by
  frame count and 0.7% by device wall clock at 0.8x, 1.0x, 1.2x and 1.5x; the
  seam sound at 1.5x on speech is not yet human-confirmed. The completions
  landed as named state (menus, clusters, listen pane, resume-from, pin,
  representation card) and the scenario lane gained labels for them
  (`menu:`, `cluster:`, `pane:`, `pin:`, `resume:`), so
  `design_completions.scn` drives them by name. Two engine facts recorded from
  frames: Livery's grid track parser has no `minmax()` or `repeat()`, and
  `background-image: url()` paints nothing through the Cambium host because
  rootstock passed an empty image ledger, which `Init.images` now fills.
  Procedural: never `cargo fmt --all` on the port, because the
  `audio-primitives` path dependency makes cargo-fmt walk the whole Woodshed
  root workspace. Lane T (Turnstone as second host, with redshank-playback as
  Turnstone's one audio authority) is in progress.
- **2026-09-15, Lane T:** Turnstone hosts the compact dock as its third
  contributed surface. `redshank-surfaces` gained `surface_api`
  (`compact_descriptor`, `compact_stylesheet`, `compact_session`, and the
  `CompactDock` handle that pumps projection and commands, because
  `RunnerSurfaceSession` carries neither state nor a per-frame hook); Turnstone
  gained `redshank_host` (one Firewheel/CPAL runtime owned by the app, a model
  persisted under the session directory), `redshank_episode_surface` (the
  provider, ~60 lines), handler routing in `open_address` for enclosures and
  subscribed entries, and graph projection of item, progress and note with a
  two-App restart test; 525 Turnstone tests pass, 7 new, and the conformance
  test re-runs the dock's own assertions through Turnstone's registry. Phase 7
  conditions: routing, no copied implementation, restart, device authority,
  and same-contract tests are met; the network authority is not, because
  redshank-playback streams ranges itself over ureq/rustls rather than through
  `mere-fetch` (a host-blob source through Turnstone's download lane is the
  described alternative). Cargo does find packages inside the nested
  `ports/redshank` workspace of woodshed.git. Both pins that blocked a clean
  checkout moved the same day: knot-editor `cac6e82` aligns to Mere
  `1009f02d` (its six `Init` literals gain the seam's fields), woodshed
  `c31158b` is on GitHub, and Turnstone `07ba180` pins all three; a worktree
  with no local cargo config built from GitHub alone and passed 525 tests.
  Still open: microphone in Turnstone, a headed Turnstone receipt, and
  Turnstone's pre-existing strict-Clippy failures (140, none in this slice).
