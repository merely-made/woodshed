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

## Shared application appearance adoption, 2026-10-09

**Status: implemented and native macOS qualified, 2026-10-10.** The sovereign desktop
adds a separate application choice over the shared Tabard store and embeds
its existing workshop through Cambium's retained views and Genet host. The
portable listener state, command vocabulary, audio runtime and generation
store remain their existing authorities. An appearance click queues no
playback settings commands. Listener seed/mode preferences remain the exact
fallback selected by **Redshank default**, including Wetland's seven endorsed
dark role values; a new authored copy preserves those values as CSS authority.

The desktop wrapper owns appearance controls and the shared draft; it does
not put file stores or authoring state into the reusable surface model. The
hosted compact dock consumes supplied Tabard roles with exact legacy role
fallbacks and contributes no application theme picker. Authored CSS is last
in the cascade; omitted roles retain the selected legacy colors. Saved choice
publication follows a successful shared preference write, and saving the same
library identity leaves its applied snapshot unchanged until explicit Apply.

The native host routes editor fields through existing focused text slots and
consumes the optional shared native workshop adapter for preview registration,
control synchronization, export effects and focused-field discovery over the
existing RenderCore. Dirty workshop
close delegates to Tabard before Redshank's ordinary asynchronous listener
shutdown. Shared export guards protect the application choice, owned data
directory (including future generations), and local media identities. Neither
an unreadable choice nor a corrupt library is replaced by an empty store.

The native workspace CPU suite at the current Mere `7019f07d` pin passes 253
tests with no failures; seven
existing playback tests remain explicitly ignored for external fixtures or
audio hardware. Nine new appearance checks cover all four modes, authored and
custom roles, fresh state reopening, persistence refusal, same-identity applied
snapshots, dirty editor close, protected future data, controlled native editor
and feed input, shared scenario fixtures, and the real Save/Apply/Back path at
420 pixels. Original note input/save and listener assertions remain unchanged;
the embedded dock's actual computed paint verifies host roles and exact legacy
fallbacks. The completed fixed-renderer suite is `/tmp/redshank-tabard-header-final-gates.log`.

Native presentation acceptance passed. Current full metadata
resolves a single Mere `7019f07d36a9da8c1a3edbbcbabad606e9cb3278` and Genet
`7422e90613f9017e5bb790e3acb48f61776b2eda` identity with the shared native-helper
feature enabled. The production binary and browser/surface Wasm check both passed at this
family and fixed renderer; logs are
`/tmp/redshank-tabard-header-native-build.log` and
`/tmp/redshank-tabard-header-wasm-gate.log`. The standalone root now explicitly patches
`netrender-vello`, `vello_encoding` and `vello_shaders` to immutable
`491c376cf2b01fc11132cf8f86419dec114ae032`; dependency workspace patches are
not inherited. All three resolved renderer packages use that source. The matching native binary SHA256 is
`e7128e911484c2db69031d61172fa56e7247644cd50eaede28028bc7a6692dcd`. A scoped
fast-forward to Woodshed origin `fea7f0f` preserved the three overlapping
Redshank candidate manifest/lock files; its earlier graph semantics repin is
included in the current Mere family.
Native macOS acceptance uses the shared LaunchServices helper with isolated
listener, choice and theme-library paths. The six accepted processes contain
191 successful presentations and 22 nonblank captures, all reviewed. Earlier
wide four-mode authoring/reopening uses the fixed-renderer `11236fd4` family;
final 420 × 900 authoring/reopening and authored Dark/Garden fresh-process
receipts use `7019f07d`. The repaired workshop puts Undo/Redo/Save below its
title, with Back/Apply and all appearance controls visible. Exact authored
native pixels match the declared body and chrome roles. Saved copy/high-contrast
dark and authored custom choices reopen exactly; listener storage remains
empty and the authored library fixture bytes remain unchanged. No new GPU
reset occurred during these runs.

The [acceptance ledger](../ports/redshank/scenarios/validation/tabard_appearance_acceptance.json)
records exact source receipt and PNG hashes. Failed ambiguous-selector runs
and the rejected pre-repair narrow header remain preserved. Initial missing
glyph/control claims were retracted after decoded pixel-region comparisons
proved those paints present; no paint-loss repair is claimed. Original listener
and note-input assertions remain intact. This qualifies the tested macOS
application appearance lanes. Windows/Linux presentation, live screen reader/IME,
native export chooser interaction and external audio/playback fixtures remain
unexercised here.

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
- **2026-09-22, Lane T closed:** the network-authority condition met when
  woodshed `bf5923d` put the port's streaming, downloads and feeds on Mere's
  `fetch::Fetch` handle and Turnstone `77b7ece` handed the tile the shell's
  handle. All five Phase 7 conditions hold; the port plan's progress log
  carries the record. Microphone and a headed receipt stay open on the
  Turnstone side.
- **2026-09-29, persistence diagnostics pilot:** a bounded, opt-in
  `mere-apparatus` observation copy now follows real save dispatch, storage
  execution and desktop reply handling. Retry requests keep distinct references
  under the same save revision; coalesced dirty revisions have no invented
  action causes. The restored desktop gate passes 63/63 and the native
  settings-only scenario passes with explicit retention loss. The default
  receipt shape remains unchanged. The [qualification receipt](2026-09-29_redshank_persistence_diagnostics_receipt.md)
  records the failed early-durability control, native attachment failure
  control, commands, pins and hashes. This isolated diff awaits integration;
  exact frame correlation, human AT and stale/cancel playback remain open.

- **2026-10-06, historical bounded browser-input compatibility adoption.**
  Starting from the Turnstone-qualified Woodshed source
  `9e982b88bf57e41ef4fa846c66ffdd4f05beb91d`, the nested Redshank workspace's
  eight Mere aliases and the browser host's three aliases move together from
  `3d1cdacc90aa6a0736154d841223a3ae29e23aa5` to
  `db4ee31258b23c86572c429388c5d10bd4de9dc3`. That Mere revision retains the
  qualified baseline manifests, Genet `69a2383b`, and Knot `855cb75d`, and adds
  only shared Weld mouse/character dispatch and ordered Graft host events.
  Woodshed's other workspace and Hocket pins retain their existing sources.
  `cargo metadata --locked --format-version 1 --manifest-path
  ports/redshank/Cargo.toml` fetched the real immutable Git source and passed:
  all 16 app-facing Mere packages use `db4ee312`; all 24 Genet packages use
  `69a2383b`. The lock differs only in those 16 Mere source entries.
  Done-condition: Git-sourced consumer metadata retains one app-facing Mere
  type family and the existing Genet family; Turnstone's exact committed
  consumer passes native two-page input/find/zoom, permission and teardown
  scenarios before the temporary Weld host bridge is removed. The current
  primary checkout's transcript work is a separate lane and is not part of
  this compatibility source. No broad family upgrade is implied.
  Production adoption is superseded by the qualified current-stack pass below.

- **2026-10-06, focused browser font backend compatibility.** From `b613fc55`,
  the nested Redshank workspace's five Genet entries move together to
  `679d8314aab4ec9f57a903c79dde244c3c565c1e`; its eight Mere entries and the web
  host's three entries move to `edf175f9c0a8645318ac8925adf0af6956f61425`.
  Runtime source is unchanged. The deterministic nested lock changes 24 Genet
  and 16 Mere source records and replaces fontsan 0.7's `fontsan-woff2` provider
  with `wuff-capi,wuff`, preserving every existing registry version/checksum.
  Genet's three native codec tests pass; actual locked Git-sourced Turnstone
  resolver/build/native qualification is pending before main integration.
  Other Woodshed/Hocket pins retain their independent families, so this does
  not claim a whole-Woodshed-workspace codec or build qualification.

- **2026-10-07, coordinated current-stack adoption, qualified and published as `82271df`.** The nested
  Redshank workspace's eight Mere aliases and web host's three aliases move
  together to published Mere `57b4893db6909d5ed9c4ccae30216f0d8164201a`;
  its five Genet dependencies/IPC patch move together to published Genet
  `965b64e206a47d1c8808472de9aa461233638768`. The Woodshed root and Hocket
  retain their independently qualified pins. Product Rust sources are unchanged.
  The owned `woodshed-browser-input-compat` worktree starts from `fd25d4c`,
  which integrates published Woodshed `5b863e3`; primary transcript work stays
  with its existing owner. Done when real locked Git metadata proves one
  selected Mere/Genet family, desktop/playback/surface checks and focused tests
  pass, the browser host checks on wasm32, and the vendored default in-process
  IPC tests pass. Receipts live in
  `validation/redshank-current-stack_20261007/`, preserving raw bytes and exact
  tested source inputs. Publication follows the coordinated Knot publication;
  root owns Turnstone adoption and its combined native gates.
  The first real resolver runs exposed obsolete constraints: Errand's current
  feed parser is 0.4.0 rather than 0.3.4, and the shared web host pins
  wasm-bindgen 0.2.129 rather than 0.2.127. Those two rows now match their
  qualified supplier manifests. Both failed resolver logs are retained;
  successful Windows and wasm32 locked metadata each select 16 Mere and 24
  Genet packages from the exact new revisions, with one wgpu 30.0.1 and the
  default in-process IPC fork. Windows selects fontsan's Wuff/libz-sys backend;
  wasm32 selects no font sanitizer. The raw 148-input archive preserves the
  exact compiled inputs for this qualification. Native/browser presentation,
  bindgen output and whole-Woodshed qualification are separate gates.
  Windows desktop/playback/surface all-target checks pass, including the added
  test target. The focused tests pass: desktop 68, feed 2, playback 27 with
  seven existing fixture/device-dependent tests ignored, and surfaces 101.
  Three consumer IPC tests pass through Servo Media Player's public reexport,
  proving typed text/bytes and order, sender transfer, and queue drain followed
  by last-sender disconnection on the selected in-process backend. Cargo refuses
  dependency-owned IPC unit tests from this workspace because their dev
  dependencies belong to their source workspace; that diagnostic is retained
  separately and is not a passing gate. The final wasm32 web check passes.
  This is 201 passed tests and seven ignored, with native Cargo exit 0 for each
  qualified command. The final 149-input source archive includes the IPC test;
  the original 148-input archive preserves the earlier check/test inputs.
  The lock keeps 773 package records: eight registry version transitions are
  the required Gopher/wasm-bindgen family changes, Errand moves to 0.4.0, and
  Genet Livery adds two existing ICU dependency edges. Other normalized package
  records are identical. Receipt `qualification.json` and `artifact-manifest.json`
  distinguish successful commands from the retained failed diagnostics.

- **2026-10-07, coordinated browser-accessibility supplier identity adoption.**
  From published Woodshed `019fdf3858bb461db79a292ddcb238c707e1e08a`, the
  nested Redshank workspace's eight Mere aliases and web host's three aliases
  move together to `f1d169c755e082b5119c2485762fd28f4226d8fb`, after qualified
  Knot `14cd06e126c10df7f5506126b98af2f07ec87eb5` publication. Genet
  `965b64e206a47d1c8808472de9aa461233638768`, product Rust and the independent
  Woodshed/Hocket pins remain unchanged. The lock changes exactly 16 Mere
  source identities; all 773 normalized package records, registry versions,
  checksums and dependency edges stay identical. The excluded playback spike
  has no Mere edges, and its lock stays byte-identical.
  The actual-collision `woodshed-browser-a11y` worktree keeps the primary
  transcript lane's 16 guarded source/manifest/lock files byte-identical before
  integration. [Raw receipts](../validation/redshank-browser-a11y_20261007/README.md)
  bind 149 archived source inputs to BelowNormal, one-job Rust 1.97.1 commands.
  Windows and wasm32 locked metadata each select one Mere f1 family, one
  Genet965 family, wgpu 30.0.1 and the existing in-process IPC backend.
  Desktop/playback/surface all-target checks pass; the same consumer scope
  passes 201 tests with seven existing playback ignores. The separate IPC
  consumer rerun passes all three tests, and the wasm32 web check passes.
  This closes the nested consumer's coordinated pin adoption gate. Redshank
  does not select Mere's changed foreign-browser adapter, so these results
  establish source-family compatibility without qualifying browser subtree
  joining or actions. Native presentation/audio, browser bindgen/runtime,
  human accessibility and whole-Woodshed acceptance remain separate gates.
