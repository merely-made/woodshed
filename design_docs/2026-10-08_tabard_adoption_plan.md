# Shared Tabard authoring and native title bars

**Status — 2026-10-08: implementation and retained-host qualification complete;
native visual acceptance blocked.** Woodshed is the first application consumer
of the shared Tabard workshop. The actual native scenario reaches the redraw
loop but cannot present a frame on this macOS compositor. Its failure receipt
is preserved separately from passing retained-host and durable-library checks.

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
- Phase 4 remains open. The rebuilt Woodshed native fixture exits with
  `RESULT fail`: 147 redraws, zero presentations, zero captures, and the
  independent 10-second presentation stall deadline. Unlike the earlier
  microphone wait, this run reaches the scenario lane and writes its receipt.
  The native process exits zero, so the receipt must also be checked. The
  fresh-process reopen fixture was not run because the first scenario never
  advanced or saved its theme. No visual, window-resize or native accessibility
  success is claimed. Evidence and commands are in
  [the adoption receipts](../validation/tabard-adoption_20261008/README.md).

## Deferred work

Other consumers, including Turnstone, the Knot editor and Cleromancy, are
candidates for subsequent adoption. This change qualifies one application.
Platform-specific visual/native-accessibility acceptance requires a working
interactive compositor on each platform; no Windows or Linux headed result
is implied by macOS compilation or retained-host tests.
