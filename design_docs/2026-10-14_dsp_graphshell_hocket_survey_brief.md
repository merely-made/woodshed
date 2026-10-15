# Shared DSP, Graphshell and Hocket Integration Survey

**Status (2026-10-14): research brief with rulings.** The survey itself was
read-only. Mark ruled on all six decisions the same day (see
[Rulings](#rulings-mark-2026-10-14)), and three bounded follow-ups started
(see [Follow-ups](#follow-ups)).

## Origin

After the shared Hocket recorder landed in Woodshed's Looper
([adoption receipt](../validation/hocket-looper_20261013/receipt.json),
[rehome plan F5](2026-09-09_audio_ports_rehome_plan.md)), Mark asked on
2026-10-14 to look at the next layer and added: *"also... consider graphshell
in woodshed... integration with hocket..."*

## Method

Four Sonnet surveyors answered separate briefs (shared DSP; Graphshell today;
Hocket sharing over Mere; Woodshed as graph host). One skeptical Sonnet
verifier per report re-checked every cited claim against source and listed
omissions. The verifiers confirmed 66 claims and marked 20 overstated, 5 wrong
and 2 unverifiable; corrections are folded in below.

Sources read: Woodshed `main` at `4ddc394`; Mere local `110277520`. Woodshed
and Hocket pin Mere `0b9b7a9`, which is 69 commits behind that tree (32 of
them touch `ports/graphshell`). Woodshed-side claims are verified at the
current tree. Claims about Graphshell internals were read at Mere HEAD and
are **not pin-verified**.

## 1. Shared DSP

### What is shared today

[`audio-primitives`](../crates/audio-primitives/src/lib.rs) (lines 8-13: plain
kernels in, drivers stay in consumers) is the only shared DSP crate. Hocket,
Redshank (`Stretcher`, WSOLA) and, per a Cargo comment, strophe-engine consume
it by path. Both products call `click::click_sample`. Hocket hand-copies the
voicing constants (`hocket-engine/src/click.rs:19-46`, "matches
`Sound::click()`").

### Where the products diverge

- **Capture schedule (deliberate).**
  - Hocket polls bar wraps from its 60 fps host tick, reading the looping
    click sampler's playhead (`hocket-engine/src/lib.rs:467-487`, `703-759`).
  - It accepts up to about one tick of start overshoot without trimming. Its
    offline test asserts a start range, 9000..=10000
    (`offline_tests.rs:162-199`).
  - Woodshed's custom Firewheel processor advances one input sample per output
    frame with an exact `(bar_idx, sample_in_bar)` cursor
    (`woodshed-audio/src/output_runtime/processor.rs:751-808`).
  - The plans require both schedules to survive consolidation
    ([Hocket host-refactor plan](../ports/hocket/design_docs/2026-07-08_genet_host_refactor_plan.md),
    step 4).
- **Bar length (accidental).**
  - Hocket and `hocket-project` export use `audio_primitives::frames_per_bar`,
    which honours `beat_unit` and rounds per beat (`click.rs:43-58`).
  - Woodshed's `Bar::duration_samples` treats the denominator as informational
    and rounds the whole bar (`song.rs:124-126`). The processor then truncates
    beat and measure (`processor.rs:145-148`).
  - Ringdown's loop-file header is a third convention: tempo counts
    `beat_unit` notes and lengths round to 256-sample blocks
    (`ringdown/src/loopfile.rs:22-34`).
  - Results differ for non-x/4 meters and for fractional beat lengths.
- **Count-in.** Hocket's count-in is relative to arming (`lib.rs:520-548`).
  Woodshed's is a one-measure lead at play start (`processor.rs:565-579`).
- **Input downmix.** Hocket configures a mono `StreamReader`
  (`lib.rs:286-300`). Woodshed averages N channels in the cpal callback
  (`input.rs:733-738`).
- **Clocks.** There are three implementations: Hocket's sampler playhead,
  Woodshed's processor cursor, and Woodshed's separate allocating offline
  scheduler (`offline.rs:204-264`). The legacy `woodshed-audio::Looper` is
  exported but unused.

### Hazards found

- `Engine::set_tempo` (`lib.rs:928-952`) restarts the click playhead and
  changes `samples_per_bar`. A Waiting capture or pending layer keeps the
  phase and target it was armed with (`lib.rs:516-548`).
- Hocket's capture starts differ by path. The drained-input path feeds the
  whole overshooting batch (`lib.rs:726-741`). The wait-only path starts with
  an empty capture (`lib.rs:762-800`).

### Evidence gaps

- Woodshed has a zero-allocation processor test (`output_runtime/tests.rs:42`)
  and a block-size-invariant WAV/live test (`song_export/tests.rs:85-151`).
  That test runs at 1000 Hz and covers playback and export, not capture.
- The byte-identical WAV gold cited by
  [`wav-parity-final.json`](../validation/hocket-looper_20261013/wav-parity-final.json)
  lives outside the repo, under `Code/testing/`.
- Hocket has no allocation guard and no exact-frame schedule test.
- Neither product has physical-device capture qualification.

### Candidate shape

A pure-std layer in root `crates/`, either widening `audio-primitives` or a
sibling crate, beneath both adapters:

- named click voicing and two explicitly named bar-length functions;
- a bar-countdown/wrap detector (only Hocket needs it);
- a take accumulator with Append, IndexedReplace and IndexedOverdubClamped
  modes, caller-preallocated;
- an `InputFeed` trait with no rtrb or Firewheel dependency;
- a `CompletedTake` value that stops below custody;
- a dev-only harness. Each consuming test binary must still declare its own
  counting `#[global_allocator]`.

Firewheel graph and stream ownership, command queues, voice storage, count-in
semantics and tick cadence stay product-local. Mere has no audio crates, so
the code does not belong there. Widening `audio-primitives` also widens its
cross-repo path consumers.

## 2. Graphshell in Woodshed

### Today

- **Custody only.** Woodshed and Hocket use the `graphshell` crate solely for
  identity custody routed to djinn (`woodshed-genet` `persona.rs:18-19` and
  `storage.rs:22-24`; `hocket-genet` `identity.rs:23`).
  `validation/relationship-recipe` is a third consumer.
- **Full default cone.** All three declare `graphshell` with default features
  (root `Cargo.toml:~164`), which compiles the retained-content cone.
  Cleromancy instead narrows it with
  `default-features = false, features = ["native"]`.
- **Woodshed already hosts native graph surfaces** on Mere's shared crates
  (sceno/scenograph/scenomise/scenotime, pictograph, workbench, Cambium
  `graph_canvas`):
  - the chartulary catalog graph;
  - `stage_scene`;
  - the Session Mere (`session_overview`);
  - the relationship-recipe editor (`woodshed-views/src/stage/relationship.rs`).

  Graphshell's `projection_compile.rs` is only a re-export of `scenomise`,
  which Woodshed already uses.
- **The ownership rule** is in the
  [stage plan](2026-07-11_stage_set_tools_plan.md), lines 411-414: Graphshell
  informs the separation of source bindings and retained navigation, and "is
  not imported as a second owner of Woodshed data".

### Three readings of "Graphshell in Woodshed"

| Reading | Status |
|---|---|
| (a) Woodshed hosts shared graph libraries over its own data | Largely true already. |
| (b) Woodshed/Hocket publish projections that Graphshell consumes | Exists only as static host-dataset JSON exports plus a fixture co-op proof (two Graphshell browser surfaces passed admission, contribution and offline reopen; [musical projections plan](2026-09-04_musical_projections_plan.md), lines 690-706). A live `graphshell-endpoint` adapter needs stable ids and real revisions: `session_overview` stamps `Revision(0)` with positional `InstanceId`s (`session_overview.rs:603-688`). It also needs a hosting decision: an in-process admission-aware host, or a djinn `mere/{domain}` route with a Woodshed validator. |
| (c) Graphshell surfaces mounted in `woodshed-genet` | Only client-side models and views are feasible, such as `mere-view` (which needs pictograph `signals`, while Woodshed enables only `canvas`). Graphshell's canvas controls, readers, editors and Forme session are `web`-feature-gated but host-neutral modules inside the port; reuse needs a Mere-side extraction. Woodshed must never construct `GraphshellApp`/`MereHost` (second authority). |

### Further findings

- **Reservoir default.** The reservoir plan's default says apps can reach an
  identity's meres (`ambient/mere/{domain}`). That favours (c), Woodshed as a
  viewer of other domains. (b), Woodshed as a publisher, needs a reservoir
  domain.
- **Authority enforcement is mostly convention.**
  `PinnedProjectionAuthorityV1::SourceOwned` is a single-variant type, and a
  script blocks crate-to-port dependencies. The endpoint read and write
  boundaries are trait contracts, and `adjudicate_intent` is opt-in.
- **Duplicated fixture.** Graphshell vendors a copy of a Woodshed export
  (`web/fixtures/woodshed-stage.json`). A second cross-repo include breaks in
  worktrees.
- **Pin text disagrees.** Manifests pin Mere `0b9b7a9`. DOC_README, the
  [Tabard plan](2026-10-08_tabard_adoption_plan.md) (line 25) and the Hocket
  host-refactor plan (lines 470-472) cite the `8568f42`/`2c7ef73` family.

## 3. Hocket integration

### Inside Woodshed

- **The Looper's data belongs to `woodshed-core`, not `hocket-model`.** The
  mounted recorder is a projection over `LoopPlan`, `SongDoc` and
  `LoopCaptureManifest` (`woodshed-views/src/stage/looper/recorder.rs`).
  `hocket-model` arrives only transitively through `hocket-views` and
  `hocket-recorder`.
- **Takes.** Woodshed has one take per Card and no layers. Saved-song bars are
  positional (`RecorderTarget::SavedBar(usize)`) and have no persisted PCM.

### Standalone Hocket handoff

- **Format.** A handoff is a recipient-addressed, signed, unencrypted CBOR
  snapshot of the whole `ProjectBundle`. Media travels as float arrays of
  about 5 bytes per sample, with no compression.
- **No turn authorship.** History nodes carry no per-node author, and the
  signature covers the sender's whole bundle. `Edit::ArmTrack`/`Track.armed`
  is the nearest structure to a recording lock.
- **Hash mismatch.** `MediaRef` hashes rate plus decoded f32 samples. Mere's
  blob layers hash bytes. A sidecar would be needed.
- **Already available in Mere.** Mere has the Commons causal log with an
  encrypted profile, Gemot delegation, stickleback blobs and drops, `moot`
  coop lifecycle conformance, and the endpoint traits. The
  p2panda/stickleback/iroh/tokio cone is already in both lockfiles through
  `graphshell`, and Hocket already restates the fork patch rows.
- **Shared custody seam.** Hocket's `.hock` store and stickleback's store are
  both muniment backends.
- **Vocabulary drift.** Hocket's maintainer-owned `PROJECT_DESCRIPTION.md`
  names Moothold and Murm as the sync substrate. Mere `TERMINOLOGY.md` now
  reserves Moothold for multi-moot federation and says Murm carries
  conversation, not collaborative state. Mere's data-formats brief has the
  same drift.
- **Timeline limit.** A Graphshell timeline of turns has one numeric axis
  (scenomise `catalog.rs:184`), so per-track lanes need another family.

### Smallest slices

1. **Woodshed-native.**
   - Add take nodes to the Session Mere from the persisted manifest,
     qualified by `LoopSegmentId`.
   - Activation goes through `open_overview_node`
     (`stage/overview.rs:445-476`) and `open_loop_card`
     (`stage/looper.rs:236-240`). Setting `ui.loop_selected` alone falls back
     to the first row.
   - Emit the nodes outside the admitted-plan branch
     (`session_overview.rs:582-597`), and keep orphan takes visible.
   - The new enum variants touch the retained presentation payload, so they
     need forward-compatibility tests.
2. **Graphshell-facing.** A read-only Hocket turn-history host dataset, in the
   style of `woodshed-core/examples/*_export.rs`. It waits on a per-turn
   author mark in `hocket-model`.
3. **Carrier.** Either keep the snapshot envelope and add a transport, or move
   to per-`HistoryNode` signed operations on a stickleback domain with blobs
   carried separately. Hocket's rule of local durability before peer sync,
   and its open host-acceptance defects (project-worker reply correlation,
   native handoff qualification), come first.

## Rulings (Mark, 2026-10-14)

1. **Bar length: Hocket's rule is canonical.** Use quarter-note BPM, honour
   `beat_unit` and round per beat (`audio_primitives::frames_per_bar`).
   Woodshed's `Bar::duration_samples` and processor truncation move to it in a
   later slice. That slice must re-qualify the WAV gold and sealed capture
   clocks for any meter where the two rules differ. Ringdown's loop-file
   convention is reconciled separately.
2. **Graphshell: Woodshed-native first.** Woodshed keeps building its own
   graph surfaces over its own data, starting with take nodes. Publishing to
   Graphshell (b) and embedding Graphshell views (c) wait for stable ids and
   revisions.
3. **Mere extraction: coordinate with the Knot font-role adoption lane.** That
   lane's "Plan lifting Graphshell port code into the stack" session received
   Woodshed's second-consumer requirements on 2026-10-14:
   - client holders only;
   - wasm-clean model crates;
   - pictograph `signals`;
   - the vendored `woodshed-stage.json` fixture.
4. **Pin: repin forward to Mere main.** This is a coordinated
   Mere/Genet/Netrender/Murm move across root, `ports/hocket` and
   `ports/redshank`. It is sequenced after the `graphshell` narrowing lands,
   because both touch the same manifests.
5. **Hocket carrier: per-turn signed operations.** Each `HistoryNode` becomes
   one signed operation on a Stickleback domain, with blobs carried
   separately. A turn mark and per-node authorship enter `hocket-model`.
   Hocket's local-durability-first rule and its open host-acceptance defects
   still come first. Hocket's
   [PROJECT_DESCRIPTION](../ports/hocket/design_docs/PROJECT_DESCRIPTION.md)
   was updated with Mark's authorization to name a coop over Stickleback with
   Gemot write delegation instead of Moothold/Murm. Mere's data-formats brief
   still carries the old Moothold wording.
6. **Woodshed custody: move to a reservoir mere.** Practice data becomes a
   djinn-held reservoir domain with a Woodshed validator. That needs
   Mere/djinn work and a migration from persona-sealed muniment slots. Mere
   gets a Git dependency on a pure Woodshed domain crate, as djinn already
   has for Cleromancy.

## Decisions for the maintainer (as posed)

1. **Canonical bar length.** Quarter-note BPM honouring `beat_unit` (Hocket),
   ignoring the denominator (Woodshed), Ringdown's convention, or a recorded
   divergence.
2. **Graphshell direction.** Woodshed and Hocket as publishers Graphshell
   browses (b), or Woodshed as a viewer of other domains' meres (c).
3. **Mere extraction.** Whether Mere extracts Graphshell's canvas controls and
   editors into native-buildable crates now, with Woodshed as the second
   native consumer.
4. **Pin.** Repin Woodshed, Hocket and Redshank forward to Mere main, or hold
   at `0b9b7a9`.
5. **Hocket sync carrier and vocabulary.** Snapshot plus transport, or
   per-turn signed operations, and whether to update Hocket's
   `PROJECT_DESCRIPTION.md`.
6. **Woodshed take custody.** Whether practice data stays in persona-sealed
   muniment slots behind a derived projection, or becomes a djinn-held
   reservoir mere.

## Follow-ups

These were started 2026-10-14 as separate sessions; each records its result in
the owning plan:

- **Hocket capture-schedule guards.** Exact-frame tests for both start paths,
  the `set_tempo` hazard, and an allocation guard. This is the prerequisite
  for any DSP extraction.
- **Narrow `graphshell` to custody-only features** in Woodshed, Hocket and the
  validation instrument, and report the pin text disagreement.
- **Loop-take nodes in the Session Mere** (slice 1 above).
