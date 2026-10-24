# Contextual navigation in Woodshed

Status: P1–P3 qualified on the portable Windows dependency graph, 2026-10-24.

## Scope

Prototype explicit contextual navigation on the existing Woodshed Stage and
Session Mere surfaces using Cambium's shared scope controls and navigation
model. A Card address includes its owning `WorkingSetId` and `CardId`; the same
musical material in another Set is a different occurrence. This slice covers
Session Mere, working Sets, and Card occurrences. Catalog exploration routing
is still open. It does not assign Stage, Rehearsal, the Looper, or the
arrangement editor to a made-up scope hierarchy.

Navigation stores locations, anchors, and view cameras only. It does not store
Set contents, practice history, editor drafts, audio transport, recording or
capture state. Editing a Set while another route is open must remain in that
Set when returning. A route is admitted only after its Set/exploration and any
Card/catalog anchor still exist; refusal leaves both the visible route and
Back/Forward cursor unchanged. Back/Forward move through temporal visits;
Up/Down move through the Session Mere → owner → occurrence hierarchy.

## Phases and done-conditions

### P1 — Native Woodshed controls and route resolution

- Show Cambium Back, Forward, Up, and Down controls and a scope trail in the
  Woodshed shell.
- Record the active owner and selected Card occurrence without cloning Set
  data. Keep each existing owner's presentation state on its current path.
- Route only to destinations that still resolve; preserve current history when
  a Set, exploration, Card, or catalog item has disappeared.
- Keep navigation independent from playback, transport, capture, and edits.

### P2 — Focused regressions and docs

- Cover two Sets containing the same musical material and prove return to the
  original owner/Card occurrence.
- Cover Back/Forward separately from Up/Down.
- Edit a Set while away and prove the edit remains when returning.
- Start existing playback/capture state and prove navigation does not change
  it.
- Update this plan and the canonical documentation index with the implemented
  surface and remaining limits.

### P3 — Persist route history with the session

- Save the bounded route, anchors, and cameras through Woodshed's existing
  `PersistedSession` backend; keep the payload opaque to the portable core.
- Restore only after Sets, Card identities, overview data, and workspace state
  load. Reject malformed snapshots and any history with stale destinations or
  anchors, clear the route, show a visible notice, and retain the rejected raw
  payload across unrelated saves until an accepted navigation replaces it.
- Do not serialize or restart playback, recording, capture, editor drafts, or
  practice ownership as part of navigation. Legacy sessions without route data
  continue to open with an empty route history.
- Cover save/reopen, malformed and stale payloads, and legacy sessions; retain
  Back/Forward owner-qualified returns after reopening.

## Findings (2026-10-23)

- `crates/woodshed-views/src/stage/overview.rs::open_overview_node` checks
  membership in the current Session Mere snapshot before routing, but does not
  retain the route that led to the overview.
- `crates/woodshed-views/src/stage/instances.rs::activate_working_set` swaps
  owner state and restores that Set's graph/context presentation. It also
  calls `leave_music_owner`, so scope navigation must account for that existing
  side effect instead of treating a route change as a playback command.
- `WorkingSets::get` resolves an active or inactive owner without changing the
  active owner. Card IDs identify an occurrence within that owner. A separate
  visual anchor avoids moving the runner cursor during navigation.
- `UiState` contains live audio, practice, capture, editor, and Set state.
  Scope history must not snapshot or restore those authorities.

## Progress

- 2026-10-23: Added Cambium controls and a location-only trail inside the
  existing workspace strip. Back/Forward validate and restore saved visits;
  Up/Down follow overview → Set → Card. Practice-panel routes retain their
  panel and graph camera. Set activation clears owner-local gestures without
  stopping its practice process. Added focused regressions and a native
  scenario fixture. The contextual navigation filter passed six tests; the
  independent practice-owner switch filter passed one test. The `woodshed-genet`
  native development binary builds successfully.
- 2026-10-23: The Windows native scenario passed with 175 presented frames and
  one nonblank capture. The first Card remained running under Set 1 while the
  visible controls traversed Card, Set and overview, including Back/Forward.
  Receipt: `../mere/support/family-promotion/contextual-navigation/woodshed-native-card-loop/scenario.done`.
  The capture was visually reviewed. Seven navigation regressions and four
  Windows path/configuration tests pass. The native run exposed and fixed a
  drive-prefix path-resolution bug by deferring metadata until the root is
  assembled, preserving canonical Windows path identity.
- 2026-10-24: Began P3. Woodshed now has an app-owned JSON slot in
  `PersistedSession`; the shared storage core retains it without interpreting
  the route schema. View restore validates it after session models and the
  workspace are restored, and displays a notice when it is malformed or points
  to missing material. Rejected bytes survive unrelated saves until a new route
  is explicitly accepted. Added focused save/reopen, stale, corrupt, legacy,
  `MemoryBackend`, and encrypted fresh-filesystem-handle roundtrip regressions.
  Eight navigation tests, the independent owner-switch test, and both storage
  roundtrip tests pass. The native Woodshed binary builds.
- 2026-10-24: P3 passed a two-process Windows native run with a fresh, unlocked
  isolated Djinn receipt Keeper and encrypted app data. Process one saved two
  Sets containing the same Card occurrence IDs, navigated to owner 2 / Card 1,
  and exited normally. Process two reopened the same persona at that exact Card
  route with matching persisted music, route, workspace, overview presentation,
  and read-only Card context; playback remained stopped. Back reached the newer
  Overview bookmark at ordinal 8 / zoom 1.25, then the older bookmark at ordinal
  4 / zoom 1.00, then returned to owner 1 / Card 1. The sealed bytes matched the
  live model at each checkpoint. Attempt 04 completed with 212 seed frames and
  171 reopen frames; both processes exited normally. Attempt 05 repeated the
  pair with 212 seed frames and 172 reopen frames, plus one reviewed native
  capture of the Card-context strip. Receipts are retained under
  `../mere/support/family-promotion/contextual-navigation-publication/woodshed-native/attempt-04/`
  and the visual supplement under `.../attempt-05/`. Isolated Keeper data stays
  outside public receipts.
- 2026-10-24: The portable lock now resolves Mere `7632bbd32a` and Genet
  `2b5c01811e` as its branch-main suppliers, with the promoted Eidetic source
  `aa94fe22ee`. Locked metadata, 9 contextual-navigation tests, the owner-
  qualified Card view, opaque storage roundtrip, both native-frame tests, and
  all 174 Woodshed audio tests pass. The portable native binary passed a fresh
  two-process run against a new isolated unlocked Djinn Keeper: seed exited
  normally after 212 frames, and reopen exited normally after 171 frames. The
  exact owner 2 / Card 1 route, persisted music, Card context, and stopped
  playback reopened; Back restored the newer Overview at ordinal 8 / zoom
  1.25, the older Overview at ordinal 4 / zoom 1.00, then owner 1 / Card 1.
  The portable receipt and checkpoints are under
  `../mere/support/family-promotion/contextual-navigation-publication/woodshed-native/portable-attempt-01/`.

## Open limits

- Catalog item routing and additional surfaces remain open. The custody proof
  uses a synthetic unlocked persona rather than live user data. Native playback
  and processor-state qualification is separate from this route proof. The
  native playback receipt
  covers a looping first Card; the earlier mixed-Set attempt stopped at tone
  admission without recording its device rate. Offline reproduction showed
  that G2 at 44.1 kHz exceeds the integer-cycle PCM admission bound. Woodshed
  now preserves that fast path and uses a 4096-frame periodic table with
  fixed-point phase and linear interpolation only when no bounded integer loop
  meets the existing 1 ppm pitch limit. Focused runtime tests cover G2 at 44.1,
  48 and 96 kHz, pitch/waveform, block invariance, fixed device duration,
  stop/clear, and unchanged invalid/Nyquist refusal. A one-shot native scenario
  at `scenarios/stage_p6_card2_audio.scn` checks Card 2 and full-set completion,
  stopped runner state, and writes read-only snapshots with the actual device
  sample rate at Card 2 and after completion. The headed run passed at an
  actual 44.1 kHz: Card 2 was Playing at index 1/event 7, the full Set reached
  Complete at index 2, the runner remained stopped after another wait, and the
  two JSON observations agreed. The 136-frame session produced two reviewed
  captures at 2200x1500. This qualifies the native runner and processor state;
  physical audio quality and latency remain open. An earlier isolated audio
  attempt correctly reported a locked vault and did not save practice.

## Portable checkout build

The portable `Cargo.toml` and `Cargo.lock` pin the promoted shared suppliers.
No local receiving configuration is needed. Set
`CARGO_TARGET_DIR=C:/t/cargo-targets/woodshed` and run
`cargo --config 'build.build-dir="C:/t/cargo-targets/woodshed/build"' build --locked --offline -p woodshed-genet`.
The verified binary is `C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe`.
