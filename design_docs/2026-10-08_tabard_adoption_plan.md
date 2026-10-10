# Shared Tabard authoring and native title bars

**Status — 2026-10-08: implementation, retained-host qualification and macOS
native visual acceptance complete.** Woodshed is the first application consumer
of the shared Tabard workshop. Four LaunchServices runs produce 14 nonblank
captures, including same-window resizing and fresh-process saved-theme reopen.
The earlier background-child launch failure is retained as historical evidence;
normal application launching presents successfully without a host bypass.

**Current reconciliation — 2026-10-10: qualified.** The broader application
rollout preserves this consumer and its six named defaults. Current shared
resolution/workshop APIs, persisted-before-activation settings, protected export
paths, and the repaired renderer pass the repeated 444-test CPU suite and real
native build. Four isolated LaunchServices lanes pass 242 presentations and 20
reviewed nonblank captures, including canonical modes, exact authored roles,
Save/Apply/Back, narrow specimens and fresh-process reopen. The October 8
receipts below remain specific to their original binary.

## Scope and ownership

The existing six named Woodshed themes remain selectable with their original
seed values and palette rules. The product owns those definitions and its
`stage_css` mapping. Tinct supplies derivation; Tabard supplies definition,
mode, registry and portable-library authority; `tabard-workshop` supplies the
same authoring state and surface as the standalone desktop workshop. Woodshed
adds a small adapter for product selection, library location, and native
lifecycle. It does not introduce another picker, palette engine, editor,
renderer, window-command queue or theme-file format.

The desktop title bar uses Cambium's ornament/title/action/caption slots and
the shared host's platform caption controls. Woodshed contributes its name
and mark. Native traffic-light spacing, drag regions and window commands remain
host concerns. The desktop root fills the native viewport so title-bar inset
variables retain their meaning; content padding belongs to the product root.
The browser demo stays on its existing surface and does not expose native
window actions or a simulated persistent authoring capability.

Authored definitions and the application selection are different durable
objects. The definition library is a separate file. The existing persona
settings store holds an optional shared theme ID and explicit preview mode;
legacy settings containing only the named `theme` field remain valid. Missing
definitions or attached modes produce visible fallback notices while keeping
the saved request recoverable. A damaged library disables authoring and shows
its error without replacing the user's file.

## Phases and done-conditions

### 1. Definitions and selection

Preserve all six original named palettes, add stable product IDs, and resolve
four canonical modes plus attached custom modes through Tabard. Register saved
user definitions in the Settings appearance list. Selection failures must not
mutate settings. Done when exact palette/rule compatibility, migration,
missing-definition fallback, custom-sheet resolution, and save/reopen tests
pass using the production dependency graph.

### 2. Shared editor embedding

Settings opens the current definition in the shared workshop. Editing a builtin
creates a protected authored copy. Previewing never changes application
appearance. Apply requires a saved, registered definition; Back preserves the
existing unsaved-work confirmation. Saving or discarding returns to Woodshed
without retaining a stale exit request for the next editor session. Native
close uses the same guard and distinguishes editor navigation from closing
the application. Done when mounted controls, typed fields, Save/Apply/Back,
failed save and close cancellation pass through the production retained host.

### 3. Native composition

Mount the shared title bar and use the existing custom-leaf registry for graph
preview and the shared scene producer for reader/application previews. All
previews use the native host's device and rendering path. Export destinations
come from the host chooser, while the workshop owns portable validation and
writes. Keep Woodshed's transports, MIDI, settings persistence and persona
gate in their existing hooks. Done when native builds, source identity checks,
wide/narrow layout, focus routing and lifecycle tests pass.

### 4. Native presentation acceptance

Run isolated scenarios with scratch practice/settings/theme paths. Capture
only frames that were actually presented; an unsuccessful redraw cannot
advance scenario steps or exhaust a capture's presented-frame allowance.
Fresh-window rendering, saved-theme reopen and resized layouts require actual
nonblank native captures before claiming visual acceptance. A compositor
refusal is recorded with its frame counts and independent stall deadline.
Tests of persistent settings do not substitute for these visual receipts.

## Findings — 2026-10-08

- Woodshed's clean main checkout was thirteen commits behind the fetched
  remote and was fast-forwarded before edits. Existing musical-context,
  captured-pattern and retained-recipe changes remain part of the baseline.
- Root production manifests need one published Mere revision and the matching
  Genet revision. Excluded Redshank, Hocket and Ringdown workspaces and the
  standalone relationship fixture have independent pins and remain outside
  this adoption. Qualification must inspect the resolved native closure.
- A previously reported fresh-window failure also occurs with an empty
  workshop library. Metal refuses drawable acquisition while the owned native
  window lacks the visible occlusion bit. The shared-host correction
  makes scenario advancement depend on successful presentation; native
  activation acceptance remains an independently measured boundary.
- Shared editor exit state was originally terminal. Embedded reuse requires
  canceling that state after a completed close, and failed Save-and-close must
  also retain an untouched imported draft whose library write did not succeed.
- The first Woodshed launch stalled before the first redraw in existing eager
  microphone initialization. A sample of the owned process isolated the wait
  to `InputEngineBuilder::build` and CoreAudio. Startup now retains its existing
  output engines and defers the one shared input stream until tuner enable,
  calibration start or recording arm. Failed tuner initialization retries only
  after an explicit disable/enable; calibration and recording requests can
  retry, and successful input recovery preserves output errors.
- Persona switching previously replaced all UI state, which would lose the
  global appearance library. The reset now preserves that library and its
  authoring capability while restoring the incoming persona's own choice.

## Progress

- Phases 1–3 are implemented and qualified against published Mere revision
  `7d2a5f3cfe97174058368073d2bae809fffbf902` and its matching Genet source.
  The native production closure has one identity for each shared stack crate,
  including Tinct, Illume, Tabard and the workshop, with no external local-path
  dependencies.
- The locked workspace suite passed 828 tests before the startup correction.
  After that correction, all 79 native tests passed, including five new
  device-free input-route regressions. Native build, scoped Clippy and the
  locked wasm web check pass; remaining Clippy statements are inherited.
- The earlier background-child attempt exited with
  `RESULT fail`: 147 redraws, zero presentations, zero captures, and the
  independent 10-second presentation stall deadline. Unlike the earlier
  microphone wait, this run reaches the scenario lane and writes its receipt.
  The native process exits zero, so the receipt must also be checked. The
  fresh-process reopen fixture was not run in that attempt because the first
  scenario never advanced or saved its theme. Evidence and commands are in
  [the adoption receipts](../validation/tabard-adoption_20261008/README.md).
- Phase 4 now passes on macOS 15.8.1, Intel, at device scale 2. The same production
  binary built from `2fa89ca61d75a9a03bf5e301eb658e696d74e44f` runs through a
  temporary application bundle and LaunchServices. Wide and narrow seed/reopen
  lanes all report `RESULT ok`: 132 presentations, 14 captures, zero blanks.
  Visual review confirms the shared title-bar actions, selection, Save/Apply/Back,
  reader glyphs, syntax spans and graph selection survive actual resizing.
  [Native visual receipts](../validation/tabard-adoption_20261008/visual/README.md)
  preserve all images, complete receipts, logs and binary/scenario hashes.

## Deferred work

Other consumers, including Turnstone, the Knot editor and Cleromancy, are
candidates for subsequent adoption. This change qualifies one application.
Windows/Linux headed acceptance, native OS decorations and live screen-reader
acceptance remain separate platform work. The captures cover the application's
rendered client area, not the operating system's traffic lights. Existing gold
selected navigation labels on pale Woodshed Settings surfaces remain a
nonblocking legibility follow-up; no formal contrast audit is claimed.

## Reconciliation progress — 2026-10-09

- Fetched Woodshed origin `e08bf4b9875a7efb02b1c17bf9fac56708712cee` into an
  isolated worktree, preserving the primary checkout. Root Woodshed and the
  excluded Redshank workspace retain separate release/lockfile boundaries.
- The source adapter now uses the shared definition-edit and saved-choice
  APIs. Its six legacy themes retain their exact `stage_css` output. Attached
  CSS uses the shared presentation distinction; a product role mapping consumes
  exported `--tabard-color-*` properties, with explicit defaults for omitted
  properties and existing typed instrument/graph paint. Exact authored rules
  enter the cascade last. Mounted color/reset regressions pass in the repeated CPU suite below.
- Remaining done-conditions are coherent published-source adoption, preserving
  application selection until a successful explicit save/Apply, protection of
  application export destinations, and new isolated native seed/reopen receipts.
  Existing private authored libraries must remain recoverable; sharing a library
  must not silently move or overwrite that data.

- The initial root dependency closure resolved one published Mere source
  `0669a9192ba98c686fdd37bbefe2021d460566cc` and one Genet source
  `15713014e2e23b887360471552f75f60684f5384`; excluded port pins are untouched.
  Selection requests persist through the existing persona SessionStore before
  activating the candidate settings and appearance snapshot. Failed writes
  retain the previous selection; saving the active definition alone retains
  its applied stylesheet until explicit Apply. The native export adapter
  protects its existing settings/session paths through the shared workshop.
- The private default library and explicit `WOODSHED_THEME_LIBRARY` override
  retain their existing locations. The browser demonstration has no application
  settings backend; its shared views do not claim durable appearance selection.
- Current-origin native acceptance extends the existing scenario to all four
  canonical modes and an exact authored role sheet, followed by saved-process
  reopen. These gates remain pending while the shared capture fix is qualified;
  the interrupted current-origin test build has no test result yet.

- Isolated native receipt commands require `WOODSHED_APPEARANCE_RECEIPT=1`
  alongside `WOODSHED_SCENARIO`. That explicit lane leaves the audio backend
  absent and MIDI device lists empty while mounting the production host, views,
  persona settings and shared workshop. The scenario asserts this device-free
  state; ordinary startup still initializes its existing music backends.

- The first current-source compile found two existing overview dynamics calls
  to the retired `PhysicsBoard::set_choice` API. The adapter now passes the
  same law/default channels through `PhysicsChoice::into_spec` and the shared
  `set_stage` binding API. Existing motion, drag and ownership assertions remain
  unchanged and require the resumed suite to pass.

- The locked production core/views/native suite passes 444 tests: 211 core,
  one core integration, 151 views and 81 native. This includes unchanged
  overview dynamics invariants, failed preference acceptance, active-identity
  Save/Apply snapshots, authored role/reset paint, native file protection and
  current scenario grammar. Log: `/tmp/woodshed-tabard-current-tests-staged-api.log`.
  These application behavior results use published Mere `0669a919` and Genet
  `15713014`; host qualification and native pixels must repeat after the shared
  asynchronous capture fix is adopted.

- The published asynchronous capture fix is now adopted coherently by the root
  workspace: full Cargo metadata resolves exactly one Mere source
  `b513994ba30a1226fb73e59ba13928350d8c979e` and one Genet source
  `7422e90613f9017e5bb790e3acb48f61776b2eda`. The root lock is regenerated;
  excluded port manifests retain their separate owner's pins. Repeated locked,
  offline application tests pass all 444 cases (211 core, one integration, 81
  native and 151 views). The successful final log is
  `/tmp/woodshed-tabard-published-capture-tests-resumed.log`; the interrupted
  preceding run is retained separately. The locked, offline native executable
  build also passes (log `/tmp/woodshed-tabard-published-capture-native-build.log`,
  SHA-256 `9b0b35348db15971c6522120bc456e62978155493602093fff07b6e18eefb27b`).
  New native seed/reopen visual acceptance remains pending while the shared
  renderer's GPU-hang investigation owns the headed lane. Woodshed has not
  launched under this pin; no earlier receipt qualifies this host.


## Renderer root closure — 2026-10-10

- The previous `b513994b` CPU suite and native build remain application behavior
  evidence. Their lock resolved registry `netrender-vello` 0.10.1: Cargo does
  not inherit Mere's workspace-root renderer patch into Woodshed. They do not
  qualify the repaired renderer or authorize current native visual acceptance.
- Woodshed now restates the published maintained renderer patch at its own
  workspace root. `netrender-vello`, `vello_encoding`, and `vello_shaders` use
  `491c376cf2b01fc11132cf8f86419dec114ae032` from `mark-ik/vello`. The legacy
  unrenamed `vello` compatibility tag remains a distinct dependency source.
  The maintained revision adds checked native shader compilation, initialized
  workgroup memory and fine command/segment validation. Shared offscreen GPU
  qualification is evidence for that renderer; Woodshed still needs its own
  coherent dependency closure, repeated CPU suite/build and native receipts.
- The matching Mere publication is adopted coherently in all four production
  manifests: `11236fd4f8da9b7e2ce3712b4f036fbf676b1c8d`, with unchanged Genet
  `7422e90613f9017e5bb790e3acb48f61776b2eda`. Full locked, offline metadata
  verifies one source identity per family and the actual native host closure
  reaches the maintained renderer triple. All 444 application tests pass again,
  and the production executable builds successfully with SHA-256
  `89abee996338444d84fe16df1114caea5f03fe0d14617bc6edecbc7bdb63e53f`.
- Four serialized LaunchServices lanes now pass: wide seed/reopen and narrow
  seed/reopen produce 242 presented frames, 20 captures and zero blanks. All
  PNGs were visually inspected. Canonical modes, exact authored role colors,
  shared Save/Apply/Back, reader/syntax/selected graph, same-window resizing and
  fresh-process saved-choice reopening render correctly. Startup occlusion
  recovered before scenario advancement; no new GPU reset occurred. The
  explicit device-free lane asserts unchanged practice/song/transport state.
  [Current native receipts](../validation/tabard-adoption_20261010/README.md)
  preserve source closure, CPU/build logs, complete receipts, hashes and images.
  Native OS decorations, live accessibility/IME and other headed platforms
  remain unqualified; this matrix does not claim audio/MIDI hardware coverage.
