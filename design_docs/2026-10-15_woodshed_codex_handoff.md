# Woodshed, Hocket and Firewheel handoff for Codex, 2026-10-15

**Status:** handoff, written for a Codex agent to execute in order. Claude
lanes designed and landed today's work. Mark moved implementation to Codex
and kept Claude for design. Every ruling below is Mark's and is settled; do
not re-decide one. If a task needs a decision that isn't written here, stop
and ask Mark with concrete options.

**2026-10-16 continuation:** the opened-link work is authorized and in progress
from root `aba2c7343`. Mere P2 is qualified, the former O1–O8 questions are
settled as L5–L12 (with independent reasons ruled as L13), and Mere's P3
Chartulary/shared Graphshell slice is published at `ee324e517`. The
[Stage plan](2026-07-11_stage_set_tools_plan.md) owns Woodshed's consumer work:
personal-only withdrawal, generated claims read-only, exact source identity,
and ordered before/after audition. The root lock alone advances to that supplier;
other dependency families and nested port locks retain their recorded versions.
The older snapshot and release receipts below remain historical evidence.

## 1. Where things stand

Woodshed `main` is `2f82f5e`, clean. Nothing is uncommitted or in flight in
the Woodshed/Hocket lane.

Four roots each have their own manifest, patch tables and lockfile:

| Root | Path |
|---|---|
| Woodshed | the repo root |
| Hocket | `ports/hocket` |
| Redshank | `ports/redshank` |
| Relationship recipe | `validation/relationship-recipe` |

All four are on Mere `39c3c955d`. Firewheel is the fork `mark-ik/firewheel`
at `2b03846` in the root, Hocket and Redshank. Nothing here needs the fork
changed.

What landed (Woodshed `main`, oldest first):

- `9b203aa`, `063f0d8`: the DSP/Graphshell/Hocket survey brief and Mark's six
  rulings, in `2026-10-14_dsp_graphshell_hocket_survey_brief.md`. Hocket's
  PROJECT_DESCRIPTION now names a coop over Stickleback, not Moothold/Murm.
- `6ca0ab6` (hocket-engine): `set_tempo` re-arms a pending capture. Both
  start paths pin exact capture-start frames, and the feed has an allocation
  guard.
- `1f1c0e7`: Looper takes appear as navigation-only Session Mere nodes and
  open into the Looper.
- `c11571b`, `7739e95`, `64b08e4`, `25acaa0`: `graphshell` dropped for
  `graphshell_endpoint::door`, and every root repinned.
- `f60e46e`, `2621430`, `4005c5a`: the Graphshell lift's P2 second-host
  receipt.
  - The overview, Related and the relationship editor drive pictograph's
    `CanvasCommand` through a `CameraHost`/`FitHost` adapter over Cambium's
    `GraphViewport`.
  - They take the whole wheel notch: `wheel_use` with pointer-anchored
    `wheel_zoom`, through `graph_canvas_with_wheel`.
  - `FaceAlignment` runs through a `FaceGeometry` adapter.
  - Camera-only changes now repaint, and 11 paint-currency assertions hold.
  - The receipt is `validation/canvas-p2_20261015/receipt.json`: RESULT ok,
    241 frames, 8 captures, scripted plain and Ctrl wheel notches.
- `2f82f5e`: the `weave` merge driver removed from `.gitattributes`.

Test counts at the last run:

| Root | Command | Passed |
|---|---|---|
| Woodshed | `cargo test -p woodshed-views -p woodshed-genet -p woodshed-core -p woodshed-audio -p woodshed-graph -p hocket-views` | 769 |
| Hocket | `cargo test -p hocket-genet -p hocket-engine` | 101 |
| Redshank | `cargo test --workspace` | 262 (7 ignored) |
| Redshank web | `cargo check -p redshank-web --target wasm32-unknown-unknown` | passes |

### Mark's rulings that bind this work

From the survey, 2026-10-14:

1. **Bar length:** Hocket's rule is canonical. Use quarter-note BPM, honour
   `beat_unit` and round per beat (`audio_primitives::frames_per_bar`). The
   slice that moves Woodshed to it re-qualifies the WAV gold and the sealed
   capture clocks for any meter where the two rules differ.
2. **Graphshell:** Woodshed-native graph surfaces first. Publishing to
   Graphshell or embedding its views waits for stable ids and revisions.
3. **Mere extraction:** coordinated through the Mere lift plan. That's done
   for P2.
4. **Pin:** repin forward to Mere `main`.
5. **Hocket carrier:** per-turn signed operations on a Stickleback domain.
   Local durability and the open host-acceptance defects come first.
6. **Custody:** move Woodshed custody to a djinn-held reservoir mere with a
   Woodshed validator. This is Mere/djinn work.

From the stack, 2026-10-15:

- **Git:** land on `main` as work goes green. Fetch first, merge
  `origin/main`, and never rebase or force-push. Mark has authorized pushing.

## 2. Tasks, in order

### Task 1: join the repin round (spec §7.6)

**Status: done 2026-10-15 at Stage A (Claude, at Mark's direction).**
- **Pins:** all four roots and the member crates are on Mere R2 `b2c8dcc2`, with
  Genet `7ffe6620`, and Murm `M` `fb6360c1` through the `www` alias.
- **Renames:** the `edit-history` patch rows were renamed to `backstep`. The
  `graphshell_client::line`, `MereViewRequest` and `SessionStep` checks found
  no hits.
- **Locks:** each lock resolves one Mere source with no package from two git
  sources. The netrender bridge is still in use.
- **Remaining Murm split:** Redshank alone shows a second Murm source, and it
  comes from Genet. Genet's `netfetcher` pins `murm-wire` through the plain
  Murm URL at `80d99590`. Mere R2's own lock has the same entry, so it needs
  a Genet repin, not a Woodshed change.
- **Counts:** 769 root, 101 Hocket and 262 Redshank tests pass, with every
  `--all-targets` check, the `redshank-web` wasm32 check and the recipe
  binary check.
- **Stage B landed the same day.** Every Mere, Murm, Genet, Netrender and
  Knot row now reads `git = "https://github.com/merely-made/<repo>.git",
  branch = "main"`, with no `www` and no `rev`, per Mere's "Family source
  convention" (`mere/design_docs/2026-08-12_family_composition_thesis_brief.md`).
  - The same-repo bridge tables (netrender, and Mere in Redshank) are gone.
    Dramatis and the non-family forks keep their revisions.
  - Each committed `Cargo.lock` resolves one source per family repo, and the
    `murm-wire` split is gone.
  - **Repinning from now on** is `cargo update -p <any package from the repo>`
    at each root, then that root's checks, then commit the lock. It is no
    longer a manifest edit.

The round is defined in
`mere/design_docs/mere_docs/technical_architecture/2026-10-15_remote_mere_route_spec.md`
§7.

Murm is one line on `main`: `fb6360c12b`, which merges the C1 callback fix
and the in-process kind. That's `M`.

The Knot handoff (`knot-editor/design_docs/2026-10-15_knot_codex_handoff.md`,
task 1) produces Mere `R2`. Wait for it, or ask Mark whether to start from
current Mere `main` instead.

In each of the four roots, including `ports/redshank`, which the spec's §7.6
omits:

1. Every Mere row, through the `www.github.com/merely-made/mere.git` alias,
   moves to `R2`. Member crates pin Mere directly (woodshed-core,
   woodshed-graph, woodshed-genet, hocket-views), so sweep every
   `Cargo.toml`, not just the roots.
2. Every Murm row moves to the alias `https://www.github.com/merely-made/murm.git`
   at `M`. These were the plain-URL `mere-transport` and `mere-webrtc-carrier`
   rows at `3d9e799f`.
3. Restate Mere's `netrender.git` patch bridge in each root.
4. Run `rg 'graphshell_client::line'` and repoint any hit to `chirograph::line`;
   none were expected at `4005c5a`. Also check for `edit_history`, now
   `backstep`, and for `MereViewRequest` / `SessionStep` matches, which need a
   wildcard arm because both are `#[non_exhaustive]`.
5. Run `cargo update -w`, then confirm each lockfile resolves one Mere and one
   Murm source, with no package from two git sources.

**Checks:** the four test commands above, plus `cargo check --all-targets`
per root.

**Done when:**

- Woodshed is pushed;
- the counts match or exceed those above;
- the Mere lift plan's Progress section records the Woodshed revision (send
  the revision to whoever owns that plan if you can't push Mere).

### Task 2: move Woodshed's bar length to Hocket's rule (ruling 1)

**Files to change:**

- `crates/woodshed-audio/src/song.rs:124-126` (`Bar::duration_samples`)
- `sequencer.rs:49-52`
- `output_runtime/processor.rs:145-148` (truncated beat and measure)
- `output_runtime/processor.rs:565-579` (count-in lead)

All of them move to `audio_primitives::frames_per_bar` (`click.rs:43-58`).

**Done when:**

- the woodshed-audio tests pass, including `output_runtime` zero-alloc and
  `song_export` parity;
- the hocket-looper WAV gold and the sealed capture clocks are re-qualified
  for non-x/4 meters as *new* receipts (never edit old ones);
- you've checked whether persisted `Song` and `LoopCaptureManifest` clocks
  need a migration (if they do, stop and bring Mark options);
- the move is recorded in `2026-09-09_audio_ports_rehome_plan.md`.

### Task 3: shared pure-state DSP crate (after task 2)

It goes in root `crates/`, extending `audio-primitives` or sitting beside it.

**Contents:**

- a bar countdown/wrap detector (Hocket only);
- a take accumulator (Append, IndexedReplace, IndexedOverdubClamped;
  caller-preallocated);
- an `InputFeed` trait with no rtrb or Firewheel dependency;
- a `CompletedTake` DTO;
- a dev-only harness.

Each test binary declares its own counting `#[global_allocator]`.

**Stays product-local:** Firewheel graph and stream ownership, queues,
voices, count-in semantics and tick cadence.

**Gate:** both capture schedules unchanged.

### Task 4: finish the loop-take node checks

Still unverified:

- sealed playback opened from the Mere;
- the narrow layout;
- a native fresh-process reopen of the version-2 presentation payload;
- take nodes overlapping labels from a retained v1 arrangement.

Fix the overlap if it's a layout bug. If it's a migration question, bring
Mark options.

### Not for Codex yet (design, or waiting on another lane)

- **Hocket per-turn signed operations (ruling 5).** A design plan only. It is
  gated on Hocket local durability and on the open host-acceptance defects:
  project-worker reply correlation and native handoff qualification.
- **Saved-song bar graph nodes.** These need stable ids and PCM persistence
  (stage plan P7) first.
- **Woodshed custody to a reservoir mere (ruling 6).** Mere/djinn work, not
  yet scheduled; Mark schedules it.
- **Mere's link model:** the former O1–O8 questions are settled; the authorized
  P3 consumer continuation is recorded in the Stage plan above. Later fan
  retirement, all-family migration/history budgets and promotion keep their own gates.
- **Touch input on the host** (P2 touch is deferred), waiting on host touch
  input in Mere.
- **Firewheel and DSP open items from the survey:**
  - Hocket's capture starts on a 60 fps tick, and Firewheel's scheduled
    events are deferred;
  - neither product applies latency compensation;
  - the downmix differs (Hocket uses a mono `StreamReader`; Woodshed averages
    in the cpal callback);
  - there's no physical-device capture qualification;
  - the fork's own callback allocation guards are documented only (rehome
    plan, about lines 335-344) and can't be verified locally.

  Raise them with Mark before changing behaviour.

## 3. Traps

- **Native runs.**
  - Set `WOODSHED_SCENARIO`, `WOODSHED_CAPTURE_DIR`, `WOODSHED_STATE` and
    `WOODSHED_PRESENTATION_TIMEOUT_SECS=30`. `WOODSHED_STATE` is mandatory;
    without it the real session is overwritten.
  - macOS has no `timeout`: background the run and poll
    `<capture dir>/scenario.done`.
  - A locked screen gives 0 presented frames. Check with
    `ioreg -n Root -d1 -a | grep -A1 CGSSessionScreenIsLocked`.
  - Foreground runs are serialized with the Knot and Cleromancy lanes.
  - Captures live outside the repo under
    `/Users/markik/Code/testing/woodshed/<name>-<date>`, and so does the
    2026-10-13 WAV gold.
- **Scenario wheel:** `wheel … on <selector>` hits the centre of the match.
  `.related-graph-col` misses the canvas, so target `.graph-canvas-swatch`.
- **Paint checks:** `FaceAlignment` measures a freshly painted leaf. Leaf
  repaint keys in `woodshed-genet/src/leaves.rs` must include the viewport,
  so assert `*-paint-current`.
- **Pre-existing failure:** `validation/relationship-recipe`'s test target
  fails on a missing `tempfile` dev-dependency (since `8ec1578`). The binary
  checks fine. Fix it if it's in your way; it's a one-line dev-dependency.
- **Formatting:** the repo isn't rustfmt-clean. Running rustfmt on
  `scenario.rs` or `leaves.rs` reformats hundreds of unrelated lines, so
  format only what you change.
- **Shared checkout:** several sessions use Mark's Woodshed checkout. Work in
  your own clone or worktree, commit only your paths, and avoid
  `--autostash`.
- **Merging:** the `weave` merge driver is gone. User git uses
  `zdiff3` and `rerere`. Still read merged files, because earlier merges
  dropped imports and duplicated blocks.
