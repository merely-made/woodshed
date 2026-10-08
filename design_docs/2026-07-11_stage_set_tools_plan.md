# Stage, Set, Tools, Rehearsal, and Looper Plan

### Executable projection export proof (2026-09-04)

`crates/woodshed-core/examples/stage_projection_export.rs` constructs a real
three-card Set from the chord catalog, including two distinct occurrences of
C Major, and discloses typed fields plus the Stage source mapping. This is a
reproducible specimen, not an export of a user's saved practice library. The
Graphshell compiler and coordinated spatial/list authoring proof are owned and
tracked in `mere/design_docs/mere_docs/implementation_strategy/2026-08-15_projection_grammar_adoption_plan.md`.
The exporter and relevant Stage/arrangement source modules passed 18 focused
tests in an isolated source-path harness; this does not replace whole-product
or release validation. No Woodshed product dependency was added to Graphshell.
The consuming Graphshell wasm build and browser authoring/fresh-page reopen
scenarios passed. Their source-hashed receipt and four GPU frames are under
`mere/ports/graphshell/docs/receipts/`. The musical-projections plan's review
addendum identifies the next product questions and corrections before S1-S4.

**Status (reviewed 2026-10-05): in progress.** P1-P3 are partial. P4a and
P4b landed; P4c and P4d have bounded headed receipts; P4e is partial and P4f
is open. P5-P8 remain open. The clean-lock and CI repair is a release-baseline
gate, not completion of this product plan.

> **2026-08-10 — the gate is open.** The scenograph scene contract this plan
> gates on ("what remains is the freeze, not the proof") froze 2026-07-24 at
> 0.0.3: emphasis channels and a default pick added, intents stay
> protocol-side, `sceno::measure` deleted. Isometry and turnstone are
> re-resolved against it. Adoption can begin; the expansion map, with
> woodshed adoption as its L1 release gate, is mere's
> `design_docs/mere_docs/research/2026-08-10_scenograph_expansion_brief.md`.

## Cross-domain relational recipe contribution (2026-10-05)

**Status (2026-10-05): domain disclosure and retained reopening qualified;
visible shared-recipe host integration qualified at baseline and 420px, with fresh-process reopening.** The Knot coordination lane owns the shared
`scenomise::projection` compiler/editor seam and Knot adoption; Woodshed owns
its musical disclosure and adapter. Reuse `scenograph` authored definitions.
The shared implementation belongs in Mere's existing projection grammar plan,
not another Woodshed scene model. Existing physics and dynamics lanes retain
their owners and do not gate this bounded static recipe.

The first recipe uses authored order, occurrence labels and one explained
relationship, with duplicate occurrences of one material source. Woodshed's
`comparison_disclosure::disclose` resolves selected Cards through the existing
keyed catalog and exact pitch-set comparison. Identities include the working
Set owner; keyed sources retain tonic and articulation. Selection order does
not replace authored Set order. The `music.shared_pitch_classes` relationship
records selected occurrence endpoints, exact pitch classes, method/version and
an explanation; it asserts no fingering, register or harmonic function.
Unknown material, missing revision, invalid identity, absent endpoints and a
pair with no shared tones are explicit refusals. Reading does not edit Cards
or start playback.

The full cross-domain milestone is done when the shared adapter compiles this disclosure, renders the explained
relationship, retains an edited recipe and selection through Woodshed's session
storage, and rebinds the same recipe to compatible Knot disclosures. Required
semantic roles must be matched explicitly; numeric type alone does not make
tempo a replacement for authored order. Unsupported roles must explain refusal.
The existing `stage_projection_export` schema stays compatible; relationship
and semantic-role extensions use the coordinated shared wrapper.

**Earlier compatibility-only checkpoint (2026-10-05; superseded by the visible host integration below):** clean Woodshed main was `e778cb6`; its Mere dependency
is `8106c7c`. Current shared main `bd119a69d` still has the node/grid/scatter
compiler in Graphshell and no relationship-bearing dataset. The shared owner
provided the extracted API at `c79bb8c203b52817991e0d7ba1c4ce3c3a2aac34`,
published on Mere's `codex/relationship-recipes` branch. The standalone
`validation/relationship-recipe` instrument pins `scenograph` and `scenomise`
to that revision. Its two-process receipt uses Woodshed's actual `UiState`,
`PersistedSession`, `SessionStore` and desktop `FsBackend`, with explicit
isolated state/settings paths. The fixture is unsealed and accesses no personal
vault. The retained payload contains the typed shared snapshot and bounded
dataset; production session capture/restore preserves those opaque bytes.
The application does not compile or edit the payload in its visible host yet.
This wire boundary deliberately keeps the production host on its existing
dependency revision; it is not a full host adoption or repin claim.

Validation commands from the Woodshed root:

```sh
cargo test --locked --manifest-path validation/relationship-recipe/Cargo.toml --target-dir target
cargo run --locked --manifest-path validation/relationship-recipe/Cargo.toml --target-dir target -- seed /isolated/new-session.json
cargo run --locked --manifest-path validation/relationship-recipe/Cargo.toml --target-dir target -- reopen /isolated/new-session.json
```

Seed refuses an existing session file. Reopen runs in a separate process and
checks label/spacing edits, both selection identities, exact relationship
explanation, source Set preservation and inactive transport. The checked fixture
`scenarios/woodshed_relationships.json` is verified against its real owner-owned
generator (`relationship_disclosure_export`). Common compiler tests also check
duplicate sources/distinct occurrences, compatible rebinding between working
owners, missing semantic roles, absent relationship endpoints and stale provenance.
Knot owns the actual lexical-to-musical rebind proof. Native Woodshed recipe
controls, user-wallet qualification and current-host scene rendering are not
qualified by this instrument. Older binaries can ignore and drop the optional
reading payload if they resave the session.

Core 189, views 97, desktop session 16 and five shared-adoption checks pass;
existing exporter tests (10) and the Stage environment integration check pass.
Strict Clippy passes for the standalone instrument with `--no-deps`.
Whole-core strict Clippy remains blocked by pre-existing theory/core lints,
and whole-workspace formatting has pre-existing differences. The new files
are formatted and `git diff --check` passes. Runtime data and hash receipts live
outside the repository under `testing/woodshed/relationships-20261005/`.


### Visible Woodshed host integration (2026-10-05)

The review checkpoint refreshed clean main `c92e7c9` against origin/main and
verified the prior receipt's source hashes. The historical inspection above
predates shared publication: Mere main now contains the compiler/editor seam
at `c79bb8c2`, with qualification documentation at `b2f67356d`; Knot main
`5516606` contains the actual Mora-to-musical recipe rebind proof.

The production workspace now uniformly pins Mere `5011e2f9` and its matching
Genet `bd3e8861`; the browser host's wasm-bindgen pin follows Mere at 0.2.129.
No local source override, duplicated compiler or custom solver is introduced.
`woodshed-views/src/stage/relationship.rs` composes the shared draft and compiler
in an explicit Mere reading view. The retained reading is an artifact in the
session overview, historically related to its source working Set. Its detailed
graph is a projection of that artifact, not the session overview itself.

Choose actual Cards, bind their authored order and explained shared pitch-class
relationship, edit label/spacing, inspect occurrences, and explicitly rebind the
same authored recipe to another selected Set. Only compiled relationships are
shown. Shared scene positions feed the existing Cambium graph canvas; increasing
spacing preserves layout units rather than fitting the change away. The graph
can scroll horizontally at narrow widths, and the full occurrence roster remains
available. A source action is separate from reading selection and validates the
exact owner, occurrence and full ordered Card instructions. Cursor navigation
is excluded from content freshness. Tuning, fingering, timing, label, removal or
reorder invalidate a copied source action; another owner's runner stays bound.
Pending selection is scoped to its working owner, including identical numeric
Card IDs in different Sets.

The optional retained payload is bounded before decoding (2 MiB), versioned,
and checked by the shared compiler. Owner envelopes require unique nonzero
Card identities and an explicit selected pair. Older proof disclosures without
owner anchors remain inspectable and require explicit rebind for source actions.
Malformed, oversized or unsupported-version readings remain in session capture;
opening them cannot discard valid Sets or silently erase the payload. Recovery
or export of an invalid reading is a later UI slice. Session update wording does
not claim an acknowledged durable write: the existing backend reports write
failures through logging, and fresh-process acceptance supplies fixture evidence.

Automated checkpoint: core 189, core integration 1, desktop 63, graph 14,
views 106, theory 181 and four doctests pass (558 total); the standalone
compatibility instrument passes five tests. Locked desktop build, host-target
web check and metadata resolve pass without local source overrides. Views
Clippy reports four existing warnings and none in the new adapter. The actual
graph-node regression
exposed a 420px failure: a last-node scroll request advanced the vertical
workspace but left its horizontal ancestor offset at zero. The graph viewport
now retains definite authored dimensions and nonshrinking height. Diagnostic
wheel input established that the scrollport could reveal the node; the shared
owner traced the failure to Cambium Rootstock's vertical-only `scroll_into_view`.
The generic two-axis ancestor reveal fix is published in Mere `5011e2f9`, with
independent failing-before nested-scroll tests. Woodshed consumes that shared
fix and tests real last/middle graph-node clicks at 1100 and 420px. Final
automated gates and committed native qualification pass; no selector bypass
replaces the graph.

`relationship_recipe.scn` drives binding, spacing edit, explained selection,
exact source navigation, stale-source refusal and explicit owner rebind.
`relationship_recipe_reopen.scn` verifies retained edits/selection/evidence and
exact source navigation in a separate process. Final native acceptance uses
implementation commit `c55461cc97bb29e9e78ecb2bb310bb4bda5accdf`:
1280×900 baseline passes eight captures, 420×900 narrow replay passes eight,
and separate-process reopen passes two. All 18 captures were visually reviewed.
They show actual last/middle graph selections, readable relationship evidence,
visible stale-source refusal, explicit rebind preserving spacing, and the Mere
retained artifact's captured-from owner link. Reopen restores the recipe edits,
selection and evidence and navigates to the exact source Card without starting
transport. The first final narrow attempt produced zero captures because the
window was not presented; that failed attempt remains in the receipt, and the
unchanged binary and scenario assertions passed the foregrounded retry.

The tested binary SHA-256 is
`13124430aab205c970dde41dce82d85abc480631c6f9afcb8389bec70bcbf91f`.
Native fixture launchers use isolated unsealed desktop `FsBackend` profiles;
this does not qualify personal-wallet encryption, high zoom, browser execution,
other platforms, acoustic quality, signing/notarization or release delivery.
Implementation and this acceptance record are published together on Woodshed
main. Full revision, binary/scenario hashes, logs and failed/final attempts live
under `testing/woodshed/relationship-host-20261005/receipt.json`, with final
captures in `committed/{seed,reopen}/` and `committed-narrow-retry/seed/`.

### Exact tone relationship expansion (2026-10-05)

**Status (2026-10-05): landed; baseline/narrow native and fresh-process reopening qualified.** This bounded continuation adds exact pitch-class
set differences, equality and directed containment to the retained reading.
A chord/chord comparison answers what is shared and what each side contributes;
a chord/scale comparison can disclose exact inclusion across catalogs. These
are catalog facts, not registered voice leading, harmonic function, key inference,
playable fingering or a recommendation score.

Done-conditions: Cmaj7/Am7 discloses shared C/E/G and unique B/A without assigning
voices; a major scale contains its chord's tones with container-to-member
endpoints; disjoint pairs disclose differences without false overlap; equal
chord/arpeggio tones preserve distinct material and occurrence identities.
Version-1 retained readings keep their original evidence and source action.
Only explicit rebind creates a version-2 richer disclosure. The shared recipe,
compiler and layout remain unchanged; Woodshed owns the new musical facts.

Qualification includes source authority and freshness, shared compilation,
restoration, wide/narrow native inspection, and explicit return from the reading
to the source Card for Run/Pause practice. Implemented arithmetic and source
flows do not establish human musical usefulness. Catalog import/expansion,
all-pairs passage comparison, registered voice leading and reusable atmosphere
editing remain separate slices.

Automated gate: core 193, integration 1, desktop 64, graph 14, views 108,
theory 181 and four doctests pass (565 total). Locked desktop build passes.
The original five-test compatibility instrument remains a separate gate for
the legacy overlap-only disclosure. Native source and restoration scenarios
are `tone_relationships.scn` and `tone_relationships_reopen.scn`; acceptance
artifacts belong under `testing/woodshed/tones-20261005/`.

Committed implementation `a969153965b64bc63a26bf08010f118878454c95` passes the
native baseline (1280×900, four captures), separate-process reopen (two), and
420×900 flow (four). All ten captures were visually reviewed. Difference and
containment explanations remain readable; the scale graph occurrence is
selected, and the source action opens Card 3 for Run/Pause practice. Reopen
restores spacing 24, containment evidence and the exact selected source.
The final desktop regression additionally exercises both widths with actual
controls and checks that inspection preserves Set truth. Legacy compilation
passes five compatibility tests; Views Clippy passes with four existing warnings.

The native binary SHA-256 is
`ec9d2c906934593f98c98b6aa2301f068fb397d1470520755f5635fda00b8ea7`.
The initial baseline succeeded; a later duplicate launch hit existing capture
paths and is preserved in its log. An unnecessary foreground retry was stopped
before feature assertions. These harness attempts do not replace the successful
captures. External scenario copies only extend the initial settle interval to
permit foregrounding; their assertions remain unchanged. The receipt records
source/scenario/binary identities and exact frames. Retention uses isolated,
unsealed desktop `FsBackend` profiles. Personal-wallet encryption, acoustic
quality, high zoom, browser execution, other platforms and release delivery
remain unqualified. The implementation, added host regression and acceptance
record are published together on Woodshed main.

### Selected passage relationship reading (2026-10-05)

**Status (2026-10-05): landed; native flow and reopening qualified with a narrow graph visibility limit.** Compare each consecutive pair in the
selected authored sequence, with at most 64 occurrences and 63 pair comparisons.
Sparse selections retain original authored positions and explicitly describe a
selected sequence rather than adjacency across the complete Set. Endpoint labels
identify each explanation; duplicate materials keep distinct occurrence identities.
This is a linear passage reading, not an all-pairs search or a harmonic analysis.

Done-conditions: three selected Cards disclose both consecutive comparisons;
reversed UI selection cannot reverse authored order; sparse selections retain
positions and compare only their chosen sequence; over-budget selections refuse;
version-1/2 captured facts and source actions stay unchanged until explicit rebind;
version-3 source actions revalidate the complete passage disclosure. The recipe,
shared compiler and solver remain unchanged. Qualification includes wide/narrow
native inspection, exact source return for practice, and fresh-process retention.
Use `passage_relationships*.scn`; evidence belongs under
`testing/woodshed/passage-20261005/`. Human musical usefulness, all-pairs queries,
catalog expansion and atmosphere editing remain separate slices.

Automated gate passes 567 checks: core 194, integration 1, desktop 64, graph 14,
views 109, theory 181 and four doctests. The locked desktop build and five
standalone relationship compatibility tests pass. Views Clippy completes with
four existing warnings.

Native acceptance uses committed candidate `047791fa6819f6523cd78ace2aeac5401d2ef162`
and binary SHA-256
`3aa35b2b7f2d943def667174c1219b52137a5d7c0ef52f15223e04d211031105`.
At default zoom, baseline 1280x900 passes in 4123 frames/four captures;
fresh-process reopening passes in 4028 frames/two captures; the independent
420x900 replay passes in 4126 frames/four captures. All ten captures were
visually reviewed. The same retained reading exposes both chord differences
and chord/scale containment without rebinding between pairs. Exact source
return selects Card 3; Run/Pause and reopening preserve the intended inactive
runner, selected occurrence and spacing. External scenario copies extend only
the initial settle from 90 to 4000 frames to allow native foregrounding.

**Presentation limit:** the narrow graph capture clips its selected third node
horizontally. The graph click assertion and subsequent exact source return pass,
and the explanation text wraps legibly; this does not establish visible selected
node framing at 420px. Horizontal graph reveal remains open. Test profiles use
unsealed fixture sessions, not personal vaults. Acoustic quality, high zoom,
browser/other-platform behavior and release packaging are not qualified here.
The logs, scenario/capture hashes and publication record are retained in
`/Users/markik/Code/testing/woodshed/passage-20261005/receipt.json`.

### Relationship overview framing and rendering replay (2026-10-05)

**Status: landed; committed-source native acceptance qualified.** The
relationship overview fits the compiled scene to its pane width through the
same GraphCanvasSwatch geometry used for custom paint and native targets.
Compiled authored coordinates, recipe spacing, occurrence identity and musical
facts remain unchanged. This is an overview projection; it does not add saved
camera editing or physics. The host regression now requires the selected last
node's entire hit target to remain visible at both widths. The passage scenario
selects Card 1 before Card 3, so its assertion proves a selection transition
rather than merely confirming an already selected occurrence.

`large_fractional_zoom.scn` replays the historical 1500x1200/0.75-zoom Practice
failure and returns to Mere. An isolated preliminary replay on candidate
`047791f` rendered both surfaces; the old black capture is historical evidence,
not a reproduced current failure. macOS constrains the actual window to its
work area. Record requested and actual capture dimensions separately. Evidence
belongs under `/Users/markik/Code/testing/woodshed/framing-20261005/`.

**Stack review:** inspected refreshed Mere `origin/main` at `289c9a98d` rather
than its stale local checkout. Graphshell's one-tree controls use shared typed
canvas commands for pan/zoom/fit, physics Play/Pause and Restore arrangement.
Its physics catalog separates arrangement generators, layout laws and overlays;
the dynamics grammar specifies portable terms and targets, with realization
gated by term class. These are the reference for subsequent Woodshed Mere
embedding: host-owned musical disclosure and actions feed reusable scene,
camera and dynamics capabilities. Keep sessions/resources/processes in the Mere
overview, with Set and fretboard readings as projections of those owners.
No physics adoption or dynamics-plan completion is claimed by this framing fix.

Qualification on candidate `6a4fdd5f9bd518a05064f194afbb89b516dd84bc` uses
binary SHA-256 `7e87fc0fae460f16af44ae75b247c8a3362fdd9d95c81a54c79622040b4516c0`.
The relevant gate passes 567 checks; the final viewport-cap edit also passes the
focused visibility regression and locked desktop build. Views Clippy completes
with four existing warnings. Native default-zoom passage runs pass at 1280x900
(1931 frames/four captures) and 420x900 (1934 frames/four captures). The narrow
capture shows all three nodes, the selected scale ring and full label. Exact
source return and Run/Pause pass. Fresh-process reopening passes in 1828 frames
with two reviewed captures. This supersedes the preceding narrow-node clipping
limit for this three-Card flow; dense 64-occurrence label readability is not
qualified.

The committed large fractional-zoom replay passes in 1828 frames/two captures.
Requested size is 1500x1200; macOS constrains the actual window to 1500x1140,
producing 3000x2280 Retina PNGs, matching the historical black capture dimensions.
Practice and Mere both render at 0.75 zoom. The historical failure is not
reproduced on the current build; its original cause remains unproven. This is
bounded current rendering evidence, not general size/zoom or acoustic acceptance.
All twelve accepted captures were reviewed. The initial passing reopen was
accidentally relaunched by a later CUA observation and refused duplicate
paintlist files; that diagnostic log is retained. A fresh isolated final reopen
run passed. No test processes remain running. Receipt and capture hashes:
`/Users/markik/Code/testing/woodshed/framing-20261005/receipt.json`.

## Product model

Woodshed is a practice app. Its organizing action is staging material for a
practice session, not browsing an explorer and not composing a song.

The product spine is:

`Catalogs -> Stage -> Set -> Rehearsal or Looper`

- **Catalogs** contain chords, scales, arpeggios, progressions, exercises, and
  set templates.
- **Stage** is the verb that adds configured material to the current Set.
- **Set** is the ordered, heterogeneous practice material for the session.
- **Rehearsal** plays through the Set as guided practice, including looping,
  metronome-synchronized articulation, and highlighted positions.
- **Looper** turns a clocked Set into a repeating backing form, captures a
  performance over it, supports replace and overdub, and exports the result.
- **Tools** are the fretboard, metronome, and tuner. They have standalone
  homes and appear contextually where useful.
- **Settings** is the canonical home for configuration. Contextual controls
  edit the same state rather than maintaining screen-local copies.

Catalog relations and practice history may be projected through the shared
projection engine inside Stage. Woodshed owns the musical facts and actions;
the engine owns selection, relationship filters, layout, and placed
representations. This is not an Explore section. Its job is to explain the
current material and answer a practical question: **what might I stage next?**

The Set itself may also be projected as a graph. In that projection each staged
Card occurrence is a numbered node, including repeated material, and Set order
is a typed `Next` edge. Selecting a node opens the same Card editor used by the
tray. Harmonic and historical edges may be layered onto this snapshot, but they
do not become a parallel material document or overwrite Set order.

This graph is the Set's domain workspace, within the wider session Mere rather
than the application's complete dataspace. Staging
material adds a Card occurrence to the Set and therefore a node to the graph.
The same occurrence may appear as a numbered glyph, a compact summary, or its
full editable Card. Expansion state belongs to the projection; Card edits land
on the one Set. The list/tray remains an alternate projection for dense and
accessible operation rather than a second workflow.

The Looper is deliberately smaller than a DAW. It does not introduce tracks,
arrangement sections, editing lanes, effects chains, or a song-authoring mode.

## Session Mere and focused working projections

**2026-09-30 maintainer direction:** the catalog/history swatch and the larger
Set graph expose narrower relationships while leaving the session's organizing
graph implicit. The Mere must account for retained artifacts, working views,
and real active processes, with navigable relations between them. A Set graph
and a fretboard are focused working projections inside that context. A useful
overview must first answer what is retained, what is open, and what is running.

This slice establishes that hierarchy using Woodshed's actual local state:
the working Set, an explicit retained Set snapshot library, the Looper form,
configured catalog explorations, practice history, and the existing shared-workbench views.
It does not fabricate multiple running sessions or import a foreign authority.
The catalog is reachable as a collection; the overview does not materialize its
entire contents beside every working artifact.

Artifact, view and process identities remain separate. Saving a Set snapshot
creates a fresh retained artifact without changing the working Set. Opening a
copy creates an independent working Set with fresh Card occurrence IDs; the
previous working Set stays available in Mere. The saved artifact remains unchanged.
An active rehearsal or Looper is disclosed from runtime facts; navigation does
not start or stop it, and restart does not automatically resume playback.
Derived snapshot lineage, view presentation and process consumption are typed
relations, with their meaning distinct from harmonic catalog relationships.

The shared `workbench` supplies tab/tree mechanics and Cambium's graph canvas
supplies the navigation projection. Graphshell's port informs the separation
of source bindings, occurrence identity, retained navigation and product-owned
actions; it is not imported as a second owner of Woodshed data. Canonical family
boundaries remain in `mere/design_docs/TERMINOLOGY.md` and
`mere/design_docs/2026-08-12_family_composition_thesis_brief.md`.

The permanent miniature catalog graph is removed from the ordinary Stage
suggestion panel. Suggestions remain available as explained actions, and their
catalog relationship view remains a deliberate focused projection. The session
Mere provides the broader graph and an accessible roster over the same subjects.

The selected Card inspector separates its compact parameter controls from
shape controls, descriptions and discovery actions. Descriptions have their own
vertical blocks; action rows have bounded usable buttons. Expanding an editor
does not put a paragraph into the same wrapping row as tempo and fret buttons.

Validation must distinguish portable model checks, production click/layout
checks, fresh-process persistence, and inspected presented captures. Native
Mere navigation, retained snapshot save/open, live activity, narrow Card layout,
and reopening are separate acceptance points; catalog graph density and audio
quality retain their previous qualifications.

The first session Mere checkpoint (`61d2ab3`) passed 528 checks: core 173, persistence integration 1,
desktop 56, graph 14, views 89, theory 181, documentation 4, and examples 10.
Production desktop checks cover 1100px and 420px layouts, pure overview
inspection/navigation, immutable snapshots, fresh working Card identities,
stale discovery cleanup, and an independent fresh-process persistence receipt.
Legacy four-panel workspace restoration preserves its saved presentation until
Mere is explicitly opened.

Native scenarios `session_mere.scn`, `session_mere_reopen.scn`, and
`session_mere_narrow.scn` make save/open, live rehearsal navigation, pause,
reopening, and 420px presentation repeatable. Run seed, then reopen, then
narrow: the reopen scenario deliberately verifies that the final Mere selection
from the seed survives restart. The narrow scenario ends in the Set view.
Wide Card descriptions use a bounded 760px explanation area; narrow canvas
labels abbreviate without changing full roster/inspector labels or identity.
The exact committed-revision capture receipt is kept outside the repository in
`/Users/markik/Code/testing/woodshed/mere-20260930/receipt.json`.
This slice does not establish acoustic quality, release packaging, or foreign
projection mounting; those retain their separate acceptance conditions.

### Independent working instances

**Status (2026-09-30): implemented; validation checkpoint based on `61d2ab3`.**
The maintainer approved several independently configured working Sets and
catalog explorations, related through Mere. Each Set retains its ordered Card
instructions, cursor and occurrence identities. The current product description
still describes the earlier single-Set frame; this authorized slice extends
that frame to one ordered Set per working instance. The maintainer-owned
`PROJECT_DESCRIPTION.md` is unchanged.

Phase 1 establishes stable working-instance identity and explicit creation,
duplication and switching. The active Set content remains in the existing
editor state; inactive owners are parked in a portable bank, without a second
mutable copy of the active content. Switching restores original occurrences;
duplication and opening a retained snapshot create new occurrences.

Phase 2 retains independent catalog selections, search and musical setup.
Exploration-specific tuning, fretboard and graph configuration travel with the
exploration, while appearance, device selection and transport preferences remain
application-owned. The existing workspace panes project the selected owner;
the overview must not pretend that each owner is a separate Workbench tile.
Transient melodic catalog previews stop on an explicit context switch; they
are not claimed as concurrently running explorer sessions.

Phase 3 binds the single rehearsal runner to the Set that starts it. Inspecting
or editing another Set must not replace the runner's Card, setup, clock, or
observation provenance. Starting rehearsal from another Set explicitly transfers
that one runner; restart restores owners without resuming activity.

Done-conditions: two divergent Sets and two divergent explorations survive
switching and fresh-process reopening; staging edits only the selected Set;
background rehearsal advances its original owner; Mere's view and process
relations expose those different bindings; invalid identities leave content
untouched; legacy single-Set sessions migrate without content loss. Validate
portable banks, production wide/narrow clicks, host runner clocks and
persistence separately from reviewed native captures.

**Findings (2026-09-30):** the former host dwell loop read `ui.set` directly,
so changing the editor owner would also have changed the runner. The host now
resolves `rehearsal_set` and its captured setup, with owner identity in the
instruction clock signature (`crates/woodshed-genet/src/drive.rs`). Card IDs
are local to a Set, so history provenance and occurrence suppression must also
qualify the owner (`crates/woodshed-core/src/history.rs`). Set-scene epochs
include owner scope, preventing identical local occurrences in different Sets
from accepting one another's retained scene events.

Saved snapshots remain immutable artifact copies. New snapshots record their
source working owner; legacy snapshots with unknown source do not gain an
invented owner relation. Opening now adds a working instance and leaves the
snapshot library unchanged; it does not create an extra snapshot every time.


**Validation (2026-09-30):** 550 automated checks pass: core 181,
core integration 1, desktop 62, graph 14, views 97, musical theory 181,
doc examples 4 and executable examples 10. Production desktop tests cover
wide and narrow instance controls, staging isolation, background owner advance,
explicit runner transfer, captured inherited setup, observation provenance,
fresh-process restoration and populated legacy migration. A focused host dwell
test verifies the clock, automatic cursor advance and emitted pitches while
another owner and tuning are visible. Appearance and transport preferences
remain shared; there is still one rehearsal runner and one selected projection
per workspace pane.

Native `working_instances.scn` and `working_instances_reopen.scn` pass at
1100x800/default zoom with reviewed captures of two divergent Sets, two catalog
explorations, the background process binding and fresh-process restoration.
The narrow replay exercises the instance graph and explicit staging targets.
Review also found and corrected a rename buffer keyed only by cursor: removing
a Card could rename the next Card, and identical local Card IDs in different
Sets could reuse the wrong buffer. Rename authority now includes Set and Card.
The rehearsal owner and background notice have separate rows above controls.
Narrow Mere uses a two-column graph with row spacing and distinct compact
instance names; the roster and inspector retain full names. Returning to the
running owner resolves its board, Hear action, shape controls and occurrence-bound
related discoveries through the same captured setup as automatic rehearsal.
Practice catalog audition continues to use its visible explorer. Recipe tiles
and search hits open a new named working Set; Clear preserves the selected
owner's Card identity allocator and stops only its own runner.
Logs, failed probes and capture hashes are retained under
`/Users/markik/Code/testing/woodshed/instances-20260930/receipt.json`.

**Rendering follow-up:** a 1500x1200 native Practice capture at 0.75 zoom is
entirely black despite finite layout bounds. The archived previous revision
`61d2ab3` reproduces the same failure in both Two pane and Full canvas, while
its 1100x800/default-zoom Practice capture renders. This evidence establishes a
pre-existing size/zoom rendering boundary, not its root cause; fractional render
scale and enlarged logical viewport still need isolation. Preserve those failed
captures separately from accepted default-zoom results. Audio stream underrun/
overrun messages also remain observed during native runs; automatic instruction
and pitch checks do not establish acoustic quality or release readiness.

## Connected exploration and deliberate practice

**Status (2026-09-30): in progress; first connected slice in parallel lanes.** This section
specifies connected implementation slices across P3-P6 and the musical
projections plan. It does not mark their open done-conditions complete or claim
new runtime evidence.

### Product intent and first complete flow

Deliberate drilling and lateral exploration across the elements of composition
are equally important. Both help assemble the Set and act on it. Exploration
can produce material worth practicing; rehearsal can expose a transition,
sound, or question that leads to another voicing, exercise, or variation. The
Set stays available for revision throughout both activities.

The ambient background in Woodshed's Mere makes relationships among its many
catalogs perceptible during that work. The motivating analogy is a linguistic
composition tool combining a dictionary, prosodic units and stress, grammar,
rhyme, and meaning: the creative capability emerges from traversing and
combining relationships and constraints. In Woodshed, musical catalogs,
context, instrument realization, touch, and timing provide those dimensions.
This analogy is design intent, not a claim of market uniqueness or a mandate
to add a linguistic subsystem.

The first complete flow is: stage Cmaj7 and Am7 on a selected instrument;
inspect their shared C/E/G and B-to-A difference; discover and audition the
Cmaj7 arpeggio; configure its articulation and timing; explicitly insert it
into the Set; rehearse and loop the passage; close and reopen; then explore
another realization or exercise with the Set and observations retained.

Acceptance must include a human review of whether the relationships help make
a musical choice and whether the resulting material is useful to drill.
Automated arithmetic, scenario completion, and captures establish their own
bounded evidence, not that creative or musical judgment.

### Shared subjects and musical context

Keep four identities distinct:

| Subject | Meaning and ownership |
| --- | --- |
| Catalog entry | A named formula, exercise, or recipe; catalog-owned identity. |
| Configured material | An entry with root/key and applicable settings; derived in musical context. |
| Playable realization | Sounding pitches, positions where known, articulation, and timing resolved from configured material. |
| Set occurrence | One authored Card with stable `CardId`; repeated material remains distinct. |

Extend keyed subject resolution beyond the current chord/scale boundary.
Progressions and exercises retain their recipe structure and preview the Cards
they will produce. Arpeggios retain their relationship to a chord formula and
their sequential articulation. Do not force every catalog kind into a new
`Material` variant or flatten every recipe into an unordered pitch collection.
Preserve source provenance when a recipe materializes into the Set.

Context includes explicit key/mode where supplied, the selected occurrence or
passage, effective instrument/tuning/capo, and applicable timing and touch.
Root alone does not establish key or harmonic function. Unknown catalog names,
unsupported relations, and unavailable realizations remain explicit.

### Relationship queries and realization

Add a portable query boundary in `woodshed-core` accepting the focus or passage,
musical context, requested relation families, optional constraints, and a
bounded discovery budget. Return stable candidate identities, all applicable
reasons, authority, relevant measurements with units, realization readiness,
and explicit actions. Disclose truncation and unsupported constraints. This is
a proposed contract, not a compile-ready API.

Initial relation coverage spans membership/containment, chord-arpeggio
realization, extensions/alterations/transpositions/modes, progression
membership and contextual roles, sounding-pitch and neck movement, exercise
patterns and targets, and personal exploration/rehearsal evidence. Derive
exercise links from actual recipe structure or authored targets; do not infer
an exercise's teaching purpose from its title.

Keep exact tone membership separate from contextual harmonic interpretations.
Allow competing readings with their assumptions. Preserve multiple reasons
between a pair. Ranking follows the selected question (share tones, explore
another catalog, reduce neck movement, revisit material); it does not erase
other relations or present one universal musical quality score.

Constraints eventually compose: retain specified tones, stay in a fret window,
preserve a selected shape, fit a duration, or satisfy a contextual role. Hard
constraints and preferences remain distinct. A bounded search states its limits
and returns an explanation when no compatible candidate exists.

Generalize the selected-shape resolver into a shared playable event realization
consumed by audition, fretboard, Rehearsal, and eventual Looper lowering. Events
carry sounding pitches, stable display-position identities where known,
articulation, and temporal position. Per-Card setup and explicit inheritance
must resolve consistently. Formula previews remain clearly identified where a
playable instrument realization is unavailable. Neck motion is not finger
assignment or physical comfort.

### Ambient Mere and explicit actions

Authored Set occurrences remain prominent; derived catalog context forms the
ambient background. Focus may be a Card, catalog subject, relation, or passage.
Inspect reveals details; Hear auditions the specified realization; Compare
explains differences; Stage previews and inserts material; Replace previews an
occurrence edit; Expand traverses a neighborhood; Pin retains a discovery.
Stage/Replace require an explicit action and preserve the appropriate occurrence
identity. Exploration never silently edits the Set.

Retain surviving positions and pinned discoveries as focus changes. Disclosure
uses relation filters and a visible density budget; a list offers equivalent
inspection, audition, and authoring with keyboard navigation. Respect the
selected reading's spatial meaning: pins do not permit arbitrary dragging when
coordinates encode musical distance. Persist authored material and explicitly
saved exploration choices; derive catalog relationships rather than storing a
second truth graph.

### Rehearsal observations and return to exploration

Separate inspection, audition, and staging from rehearsal observations.
Correct `EngagementKind::is_practice` without rewriting historical staging into
completed practice. New observations retain run identity, Set occurrence,
catalog source, and event-time realization/context provenance sufficient to
explain what was presented; current settings must not reinterpret old events.
Legacy records lacking provenance stay qualified.

Record active elapsed time, presented events, loop/transition activity, and
completion according to explicit runner semantics. Starting playback does not
prove successful performance. Optional player annotations can describe a
practice intention, difficulty, or interesting result; audio assessment is a
separate evidence source if implemented later. Exploration ranking and practice
ranking use the relevant event classes while preserving both histories.

### Ownership and implementation slices

Pure musical operations stay in `woodshedding`; formula identities and catalog
relations stay in `woodshed-graph`; contextual queries, realization orchestration,
observations, and Set commands belong in `woodshed-core`; interaction belongs
in `woodshed-views`; device execution belongs in audio/host layers. Scenograph
projects Woodshed facts and routes typed actions to their owners. Move product
coordination out of the view owner as each touched slice requires, following P1.

The following slices develop exploration and drilling together. Each is open:

| Slice | Done-condition |
| --- | --- |
| Chord → arpeggio → Set → rehearsal | A contextual relation leads to audition and explicit insertion; display/audio agree; the mixed Set can run and save/reopen with source and occurrence identity intact. |
| Chord ↔ scale ↔ progression | Keyed containment and contextual explanations expose cross-catalog choices; recipe previews match staged Cards; the resulting mixed Set is rehearsable. |
| Transition → alternative → focused drill | Two occurrences can be compared, a realization explicitly chosen, and that transition looped; pitches, setup, highlights, timing, and retained identity agree. |
| Exercise discovery and construction | Links have actual structural or authored evidence; generated material is previewable and editable; the staged exercise preserves its declared target and can be drilled. |
| History-informed exploration | Interest and runner activity remain distinct; repeated material and edited realizations have event-time provenance; suggestions disclose evidence and survive reopen. |
| Passage constraints and lookahead | Multiple events accept locked choices and explicit constraints; alternatives preserve constraints, expose component costs and search limits, and can be staged and rehearsed. |

The first integrated gate includes duplicate material, custom tuning/capo,
missing catalog entries, unavailable shapes, pause/resume, edits during a run,
keyboard graph/list parity, and fresh-process reopening. Record deterministic
checks, native scenarios/captures, acoustic checks, and human review separately.
Later passage search does not block the first cross-catalog flow. P7 remains
responsible for Looper lowering and capture persistence.

### Parallel ownership and integration gate

- Catalog lane: contextual chord-to-arpeggio discovery, nonmutating previews,
  selected-setup preservation, and bounded ambient context.
- History lane: interest/practice classification and backward-compatible
  event-time provenance with qualified legacy observations.
- UI lane: occurrence-bound inspection, audition, explicit insertion, and
  rehearsal interaction using shared core contracts.
- Integration owner: shared module wiring, persistence/reopen checks, desktop
  scenario/capture validation, review of lane boundaries, and receipt updates.

Each lane owns separate files and supplies focused tests. Shared interfaces are
agreed before caller wiring; one integrated test gate follows lane completion.
The first slice is not accepted solely because each lane reports success.

### Verified findings and progress

- **2026-09-30:** inspected source at `5452419`. `woodshed-graph/src/lib.rs`
  exposes five catalog kinds and typed relations; `woodshed-core/src/mere.rs`
  joins catalog structure and history. `stage_context.rs` exposes Card/chord/scale
  node kinds; `harmony.rs` resolves keyed chord/scale references;
  `stage_candidates.rs::ranked_candidates` enumerates the 24 major/minor triads.
  These existing boundaries explain why broad cross-catalog discovery requires
  contextual adapters as well as additional relations.
- **2026-09-30:** `history.rs::EngagementKind::is_practice` excludes only
  `Previewed`, despite its comment distinguishing staging from practice.
  `Engagement` carries source and elapsed-time fields but no event-time
  occurrence/realization provenance. `card_shapes.rs` supplies selected chord
  shapes; `rehearsal.rs` supplies stable Card identity and recipe provenance.
- **2026-09-30:** maintainer endorsed the connected implementation direction
  and authorized documenting it. Implementation and runtime validation remain
  open; this update supplies acceptance targets and does not close P3-P7.
- **2026-09-30:** catalog, history, and UI lanes implemented a bounded first
  connection in `connected_catalog.rs`, `harmony.rs`, `stage_context.rs`,
  `stage_scene.rs`, `history.rs`, and `woodshed-views/src/stage/connected.rs`.
  Discovery resolves a selected chord occurrence to its sequential form;
  audition/insertion re-resolve the source at action time. Selected setup,
  recipe provenance, marks, timing, and new occurrence identity are preserved.
  The ambient context and foreground scene retain the typed relationship.
- **2026-09-30:** integration review corrected capoed inversion, inherited
  runner tempo, stale idle timestamps, unavailable repeated-occurrence binding,
  and false same-occurrence practice transitions. The host supplies an event
  clock; pause/edit observations retain authored instruction snapshots and
  consume active spans. Duration is observed runner time, not player success.
- **2026-09-30:** integrated automated gates pass: core 138, graph 14, views 65,
  export examples 10, and desktop tests 35 including a production-host wide/
  narrow click flow and two-process filesystem save/restore. The latter uses
  synthetic unsealed state; it does not prove persona switching or power-loss
  safety. Full commands, logs, and limitations are recorded in the testing
  workspace `testing/woodshed/connected-20260930/receipt.json`.
- **2026-09-30:** the initial locked-Mac attempt produced no captures. After
  unlock, `connected_practice.scn` passed with four presented captures; a fresh
  process running `connected_practice_reopen.scn` passed with two more. All six
  captures were inspected. The native run demonstrates discovery, audition
  dispatch, staging, runner advancement/pause, and restoration of three Cards,
  arpeggio shape, selected occurrence, and history through isolated filesystem
  state. It does not demonstrate acoustic correctness or persona switching.
  Receipts and captures live under `connected-20260930/unlocked/`.
  The ambient Stage capture at scenario zoom shows selected-card editor/graph
  overlap and clipped controls; Rehearsal controls are usable in inspected
  captures. Stage layout correction, acoustic/human review, keyboard graph/list
  acceptance, and full per-event highlights remain open.

- **2026-09-30:** follow-up layout correction moves the expanded selected Card
  into the graph inspection layout beside the canvas, wrapping below it when
  space is limited. Its natural height accommodates shape and discovery controls;
  selection and collapse retain the same occurrence. This supersedes P4d's
  in-node editor footprint without changing historical capture claims. Views
  65 and desktop 36 tests pass, including non-overlap and clickable discovery/
  staging; desktop build passes. The native follow-up produced zero frames
  after the Mac locked again, so presented-frame confirmation remains pending.

- **2026-09-30:** after unlock, the revised P4d layout scenario passes four
  presented captures at UI zoom 0.65. Expanded and resized captures show the
  selected editor beside the canvas, with activation, resize, and collapse
  passing. The preceding default-zoom fixture missed graph controls below the
  viewport; this receipt qualifies the recorded zoom, not all viewport/zoom
  combinations. Listening and keyboard acceptance remain open.

- **2026-09-30:** the next bounded slice adds rooted chord-to-scale formula
  containment in `connected_scales.rs`, explicit Explore/Hear/Stage actions,
  and progressive disclosure from four to 32 choices. Ranking prefers the same
  root, named Major/Minor scales, then fewer added tones and stable name/root
  ordering; this is a disclosure preference, not a suitability score. Actions
  revalidate the source occurrence and chosen keyed scale. A new Walk Card
  preserves authored setup, timing, and recipe while clearing chord shape and
  note marks. Chord-scale containment does not transpose from a capoed shape.
  This implements the chord-to-scale portion of the second slice; progression
  recipes, scale fingering, and general instrument realization remain open.
  General scale display still uses live tuning; formula audition uses written
  pitch classes and the existing short cascade, not synchronized event timing.
  Persisted per-Card setup is not evidence those realization gaps are closed.
- **2026-09-30:** scale integration passes 278 tests (core 144, integration 1,
  examples 10, graph 14, views 69, desktop 40), including wide/narrow production
  click dispatch, repeated occurrence identity, stale source/candidate rejection,
  and fresh-process chord/arpeggio/scale restoration with separate observations.
  Desktop build passes. Receipts live under `testing/woodshed/scales-20260930/`.
  Final native seed/reopen scenarios pass with three presented captures, all
  inspected: the scale stages, runs/pauses, and restores in a separate process.
  The earlier Stage layout scenario also passes at recorded zoom with four
  captures. Native formula audition dispatch is not acoustic correctness or
  successful player performance; keyboard and listening review remain open.
  Hidden discovery panels now omit their empty styled boxes.

- **2026-09-30, shared scale realization:** scale Cards now resolve their saved
  instrument, catalog tuning, capo, and physical fret window once for the
  displayed contacts and audition pitches. The written root is transposed by
  capo for concert sound. An ascending traversal selects one deterministic
  contact per distinct MIDI pitch; it does not prescribe a playable fingering.
  Solo resolves exact marked contacts; Mute retains the existing pitch-class
  semantics. Manual Walk uses quarter-note spacing, and timed Walk fits one
  traversal inside the authored dwell, including inherited runner BPM. Missing
  setup/formula and empty windows show an unavailable status and yield no live
  instrument fallback. This supersedes the general scale display/audio gaps
  recorded above. Synchronized note highlights, voice cancellation on pause,
  arbitrary persisted custom tunings, and fingering construction remain open.
  Production interaction and separate-process restoration checks establish a
  four-string high-G Ukulele Card, capo 2, physical frets 2–6 against a live
  six-string Guitar. A clicked string-index-1/fret-2 contact solos independently
  calculated D4 (approximately 293.665 Hz). Discovery and staged audition agree;
  an inherited 80 BPM bar occupies three seconds. Invalid stored tuning has a
  visible reason, no fret markers, and empty effective audio.
  Integrated validation passes 460 tests (151 core, one core integration,
  42 desktop, 14 graph, 71 views, 177 theory, and four theory doc examples);
  the desktop build passes. Native seed/reopen scenarios in
  `scenarios/scale_realization*.scn` exercise saved Ukulele geometry, written C
  Major/concert D Major, explicit staging, Run/Pause, and restored observations.
  The native fixture re-resolves the source shape after changing instrument;
  it does not treat a retained Guitar shape as a valid Ukulele shape.
  Final seed/reopen runs return `RESULT ok` with four presented-frame PNGs,
  all inspected. The saved setup, scale occurrence, and observations restore
  in the fresh native process. Receipts are recorded under
  `testing/woodshed/scales-20260930/realization-final-{seed,reopen}/`.
  Audio-device quality and keyboard acceptance remain separate; the initial
  native run logged stream underrun/overrun, so a capture pass is not an
  acoustic-quality receipt.

- **2026-09-30, executable scale degree-pair slice:** introduces two bounded
  structured recipes, Thirds and Fourths, over seven-note scale formulas. The
  pure degree grammar pairs 1–3/2–4 or 1–4/2–5 across octave boundaries. It uses
  absolute scale degrees: missing pitches in a physical window omit incomplete
  pairs rather than compressing the scale and changing the intervals. This is
  a first musical syntax construct, not a general-purpose parser.

  Explicit `ScalePattern` material stores the scale formula, written root, and
  recipe choice separately from the display label and provenance stamp. The
  previous Scale wire representation stays unchanged. Display and audition use
  the shared saved setup resolver. Sequential preview preserves pair order,
  downward transitions between pairs, and repeated notes; existing sustained
  one-shot audio and pause limitations still apply. Earlier app versions cannot
  be assumed to understand the new material variant.

  The source scale offers two explained contextual choices with separate
  inspection, Hear, and Stage actions. Inspection and audition retain the Set;
  staging creates a new occurrence after revalidating its source and available
  realization. Seven-note support and incomplete-pair behavior are disclosed.
  The catalog background also names keyed pattern realizations and typed
  degree-pattern relationships around the scale under the existing context
  budget. The authored Set, derived catalog context, and practice observations
  retain their separate identities.

  Integrated validation passes 480 tests (158 core, one core integration,
  46 desktop, 14 graph, 77 views, 180 theory, four doc examples) plus ten core
  example tests. Production click gates cover both recipes at 1100 and 420 px,
  independently calculated concert-pitch pairs, immutable inspection/audition,
  distinct repeated occurrences, invalid and removed sources, and separate
  measured pattern histories. Fresh-process tests compare full ordered audio
  tuples and saved recipe discriminants. Native seed/reopen scenarios pass
  with four presented captures, all inspected, including the ambient relation
  view, staged recipe, pause, and restored instruction/observations. Pattern
  satellites occupy a separate row above the source after visual review;
  overall dense catalog layout and human musical usefulness remain review
  work. Receipts live under `testing/woodshed/patterns-20260930/`.
  Native audio logs still report stream underrun/overrun; no acoustic quality
  acceptance or device-performance claim follows from these captures.

- **2026-09-30, chord-tone approach slice:** adds Below and Above chromatic
  approach recipes for an explicit adjacent pair of ordinary chord Cards. The
  next Card supplies target tones; the preceding Card supplies passage context,
  without inferring a key or a voice-leading route from its voices. Each pair
  visits a same-string semitone neighbor, then the target. Both occurrence IDs
  remain pinned and every action checks that the target still follows the source.
  Removal, reordering, an intervening duplicate, or a stale selected target shape
  produces an explanation instead of choosing another target.

  `ChordApproach` material stores the target formula, written root and direction.
  A standalone exercise retains the target's saved instrument, tuning, capo,
  physical window, timing and selected shape, clears marks, and uses Walk. With
  no selected shape, it chooses a deterministic contact for each target MIDI
  pitch that has an available partner. Incomplete physical pairs are omitted;
  unknown setup, formula, stale shape or no complete pairs fail closed. Ordered
  audition retains downward approaches and repeated visits. The previous Chord
  wire representation stays unchanged; older apps may not understand this variant.

  Inspect and Hear preserve the passage. **Append approach exercise** creates a
  separate occurrence at the end of the Set, preserving the passage order and
  cursor. Automatic interleaving and whole-progression construction remain later
  work. The ambient catalog exposes both choices around the target chord, with a
  typed target-approach relation and the existing context budget. Approach pitch
  classes include transient chromatic neighbors, so harmonic comparisons do not
  falsely present this recipe as only the target chord's tones. A free catalog
  recipe uses the current Stage setup and explains that basis explicitly.

  Integrated validation passes 501 tests (166 core, one core integration,
  51 desktop, 14 graph, 84 views, 181 theory, four doc examples) plus ten core
  example tests. Wide and narrow production click gates independently calculate
  the physical MIDI pairs, verify immutable exploration and append-only authoring,
  retain selected target shapes, preserve repeated visits for unselected targets,
  and measure exercise history separately from the original chords. Fresh-process
  tests restore the original pair plus both directions and compare the complete
  ordered audio tuple at native precision. The existing narrow fretboard scroll
  and note-hit regression also passes after hiding the empty approach panel when
  no adjacent pair is available.

  After unlock, the native seed and fresh-process reopen scenarios pass with
  four presented captures, all inspected. They exercise immutable audition,
  explicit append, the saved Ukulele/high-G/capo-2 realization with four strings,
  descending sequential pairs, Run/Pause and restored observations. Capture
  review found overlapping approach labels in the ambient graph; the follow-up
  uses compact chord-symbol/direction labels and staggered satellite spacing while
  preserving full recipe names in the inspector and authored Card. Overall
  dense-catalog layout remains review work. The scenario sources are
  `scenarios/chord_approaches.scn` and `scenarios/chord_approaches_reopen.scn`;
  logs, capture hashes and the exact-revision receipt are under
  `testing/woodshed/approaches-20260930/`. Audio stream underrun/overrun is still
  logged; these captures do not establish acoustic quality or human musical
  usefulness. Existing one-shot sustain and pause limitations remain.

**First-slice qualification:** functional discovery, staging, sequential-onset
preview, runner observation boundaries, and filesystem reopening are implemented
and automatically tested. The synth sustains prior tones through the cascade;
pausing stops runner advancement but does not cancel an already queued one-shot
preview. Full synchronized note events/highlights and audio pause semantics
remain under P5/P6. Provenance currently snapshots the authored Card; optional
run identity, effective inherited realization, and presented MIDI observations
remain unset unless a caller supplies them. Broad catalog queries, durable
exploration pins, exercise construction, and passage search remain later slices.
The complete first-flow acceptance and the wider P3-P7 done-conditions remain
open.

## Release baseline and 1.0 product proof

The release baseline is reached when a clean checkout resolves the committed
lock without local sibling patches, core and Windows-host CI pass, and a tag
produces a checksummed Windows ZIP. Repairing that baseline makes the alpha
credible; it does not close any product phase below.

**2026-09-30 publishing assessment:** use a downloadable desktop alpha as the
initial candidate while audience choice remains open. The pushed scale slice
at `5f29760` has green [CI across all five jobs](https://github.com/merely-made/woodshed/actions/runs/36754594105):
clean lockfile, core tests, macOS/Linux builds, and Windows host check. Historical
manual [macOS packaging](https://github.com/merely-made/woodshed/actions/runs/34295644328)
and [Linux packaging](https://github.com/merely-made/woodshed/actions/runs/34296576133)
runs succeeded at older revisions; they do not validate a new candidate.
No Windows packaging run or [public release](https://github.com/merely-made/woodshed/releases)
is currently recorded. The macOS package is unsigned and architecture-specific.

The next release checkpoint is a named revision, clean locked build, platform
archive and checksum, extracted-app launch, isolated save/reopen, and a concise
known-limits document including audio, supported platforms, data storage and
upgrade behavior. Re-run packaging for that revision before claiming a current
candidate. Keep the artifact receipt with source/binary hashes and observations.
Publication requires an explicit release instruction; assessing or preparing an
alpha does not create a tag. Browser publication is a separate host-validation
slice because `woodshed-web` is still unshipped. The crate family is currently
git-first; a registry/library release needs its own dependency and publish check.

**Catalog expansion order:** source inventory currently includes 40 scale
formulas, 39 chord formulas, 12 progression recipes, seven exercise generators,
101 tunings across 15 instrument families, and 11 generated practice templates.
Arpeggios derive from chord formulas. Counts describe the built-in catalogs,
not independently rehearsable products or a runtime catalog-pack facility.

1. Add executable composition/practice recipes: diatonic thirds/fourths,
   chord-tone approaches, minor-key progressions, and transition drills. Each
   preview must lower to the same Cards as staging and have a useful playback
   and instrument realization. Current Riff audition is unavailable, so extend
   its realization before advertising new Riff generators as audible drills.
2. Introduce validated catalog packs with stable IDs, aliases, source/license
   provenance, explicit parameters, and migration from current name-based
   identities. Then expand community-curated material without recompiling every
   catalog change. Rhythm, articulation, and melodic sequences offer useful
   combinations beyond adding scale names. Microtonal catalogs require a pitch
   model beyond the current twelve-tone assumptions.
The linguistic composition direction belongs primarily to **Knot Editor**, per
Markik's 2026-09-30 clarification, and was handed off to
[the Knot task](codex://threads/01a0eb5b-546e-7fb2-a492-a873e8b12343).
It concerns pronunciation, stress, rhyme, grammar, meanings, and mora-like word
division. CMUdict and Open English WordNet remain possible resources for that
separate planning effort; neither is selected or integrated here. Woodshed's
catalog expansion remains musical material, playable recipes, and their typed
relationships. The shared principle is deliberate work combined with lateral
exploration; it does not make Woodshed the owner of Knot's lexical records.

**Musical syntax and ambient catalog context:** Markik clarified that Woodshed
should explore a comparable musical language syntax. Pitches, intervals, chord
and scale formulas, rhythm, articulation, and progression rules can form typed
vocabulary and compositional constraints. A phrase or recipe should be
inspectable, explainable, auditionable, and lower to explicit Set Cards where
its realization is supported. The concrete syntax and rule representation are
still design work; this is a product direction, not an implemented parser.

The ambient background is a central interaction surface for making large
catalogs useful. Around the current Card, phrase, or catalog subject, it should
expose relevant containment, compatible material, substitutions, tensions, and
possible transitions with reasons and bounded disclosure. Focus and authored
choices determine context; exploration can lead to explicit inspection,
audition, and staging. Ambient relationships and focused drills carry equal
product priority: they help assemble the Set and act on it. The next recipe
slice should therefore include contextual catalog relationships and explanations
alongside executable practice, rather than treating catalog expansion as an
inventory-only task.

Woodshed earns a 1.0 practice claim only when one persisted flow demonstrates
all four parts together:

1. choose or stage musical material;
2. turn it into a playable route on the selected instrument;
3. record honest practice history from the run;
4. present an intelligible next step grounded in theory or that history.

Done means the player can close and reopen the application and recover the Set,
route, history, and explanation. A graph screenshot, a hardware-only adapter,
or a package artifact proves only its own layer.

## Boundary decisions

### Keep the existing Set spine

`woodshedding::rehearsal::{Set, Card, Material, Setting, Touch, Timing}` is the
right portable core. A Card already means material plus instrument placement,
articulation, and dwell. Catalog choices and generated templates should stamp
Cards into one Set. Rehearsal and Looper should consume that Set instead of
maintaining independent musical documents.

`PracticeSet` remains useful as a catalog template or generator. It moves under
Stage and materializes Cards through the existing `set_from_practice` path. It
is not a top-level product section.

The current `Lens` name describes catalog selection, not a general projection
system. Rename it to `CatalogKind` once the views are decomposed. Fretboard
layout is a projection choice and belongs to the Fretboard tool/settings.

### Treat Card as a domain object with several projections

Card is the right unit because it is a finite practice instruction: play this
material, on this instrument and tuning, with this touch, for this long. It
must not imply one large visual card everywhere.

- Stage may show a detailed editable Card.
- The Set tray may show the same Card as a compact row or tile.
- Rehearsal may show it as a filmstrip frame and a full active surface.
- Looper may show it as a clocked segment.

Use ordinary `xilem_serval` elements for the Card's text, controls, focus,
reordering, and accessibility. Use Chisel for custom-painted projections inside
or beside it: fretboard geometry, interval diagrams, progression strips, and a
graph neighborhood. Chisel does not own Card state or product actions.

### Project catalog relations and practice history through a shared boundary

`woodshed-graph` already gives catalog objects stable IDs and relates
progressions to their chords and chords to scales they fit. Its stemma proof
records practice lineage against those same IDs. Extend that projection rather
than creating a recommendation-only store or importing Mere's graph kernel as
Woodshed truth. A Woodshed adapter should expose portable material and relation
snapshots to the shared projection engine (`scenograph`, named below).

Keep catalog formulas as stable nodes. Root, instrument, tuning, timing, and
touch belong to the staged Card or practice event; avoid multiplying the
catalog into a node for every possible configured realization.

Record typed engagement events: previewed, staged, rehearsed, completed,
looped, and recorded. Preview and staging are evidence of interest; completed
Rehearsal time is evidence of practice. Suggestions should identify which
evidence and relation produced them.

The compact **Related** swatch is the focused frontier of the Stage graph: a
small ranked set of useful neighbors, each stageable as a Card occurrence. It
can expand without changing identity into a deeper relationship view with the
selected material at the center, theory relations around it, and the player's
recent path or practice strength overlaid. Catalog search remains the direct
way to find a named item; the graph explains, extends, compares, and stages the
relationships around the current focus.

### Keep the Stage graph's authorities layered

The projected Stage graph composes four layers which must remain identifiable:

1. **Set layer:** staged Card occurrences and the `Next` edges derived from Set
   order. This is authored session material.
2. **Theory layer:** deterministic catalog and contextual musical relations
   such as contains interval, fits scale, fifth of, resolves to, shares tones,
   or voice-leads-to. Woodshed owns these facts and computations.
3. **Evidence layer:** observed practice transitions and engagement strength,
   with event kind and observation time retained.
4. **Suggestion layer:** derived or learned frontier nodes and edges, carrying
   producer, model/version where relevant, confidence, and an explanation.

Showing or hiding a relation family changes projection state. Staging a
frontier node, accepting a suggested relationship, drawing a Set edit, or
editing a Card changes an owning document through an explicit action. A filter
must never silently delete a relation, and a suggestion must never silently
become catalog or Set truth.

One material pair may carry several relations at once. Ranking chooses what to
surface first; it must not deduplicate `diatonic`, `shares tones`,
`voice-leading`, and `practiced after` into one winning reason. Selecting an
edge should expose every applicable reason and its authority.

### Project through the scenograph scene contract

The shared projection engine named above is the `scenograph` family (`sceno`
core, `scenomise` layout, `scenotime` runtime), the product-family projection
compiler and runtime founded in mere's
`2026-07-21_projection_engine_prior_art_brief`, where Woodshed is already
forcing-function #3. Woodshed consumes its scene contract and does not import
mere's graph kernel. `StageGraphSnapshot` is Woodshed's source adapter into that
contract, the analog of mere's `cartography` graph adapter: Woodshed owns the
musical facts and typed relations; `sceno` owns selection, placement,
footprints, representation, and gesture routing.

The engine's vocabulary maps onto the instrument. A fretboard is a `sceno`
frame, a coordinate space from (string, fret) to screen; a note is a projected
item with a point footprint; a fingering is a path footprint; a Tonnetz or
circle is a second fixed-layout frame; a voice-leading relation is a routed
edge. The P4e catalog is therefore `scenomise` layouts over Woodshed source
facts, not seven hand-built swatches, and the current `related_swatch` radial
placement is the first thing the contract retires.

Woodshed is the source-model sanity check for that contract. Every other
consumer (turnstone, hocket, isometry) authors or generates content before the
engine has anything to project; music theory hands Woodshed a dense,
deterministic, multi-relational fixture on day one, where one chord pair carries
diatonic, shared-tone, voice-leading, and practiced-after relations at once with
no authoring step. That makes it the natural stress test of the projection-graph
half of the contract (selection, multi-family edges, ranking without dedup),
complementary to isometry's proof of the scene half (footprints, placement,
representation). Its relations are static and deterministic, so it does not
exercise the late-arriving signal or streaming-uncertainty paths; it is the
clean first fixture, not the only proof.

Sequencing: Woodshed's typed relations (P4a identity, P4b relations) are
wall-side truth and proceed now, doubling as design pressure on `sceno`'s
source and channel model. Scene-contract consumption was gated on mere and
isometry proving and freezing the contract. **The proving half is done**
(verified 2026-07-24): mere consumes `sceno` through `cartography::scene_out`
and a persisted spiral score; isometry deleted `Overmap::layout` and its force
solver, emitting the same score/scene types for its overmap and tactical
board; a serialized coastal fixture exercises the geographic path; and
graphshell consumes `scenotime`'s snapshot/diff pair for remote replay. The
family also moved: `sceno`/`scenomise`/`scenotime` 0.0.2 now live in mere at
`crates/scenograph`, not a standalone repo.

**The freeze landed 2026-07-24**, published as `sceno` / `scenomise` /
`scenotime` / `scenograph` 0.0.3 on crates.io. The gate on scene-contract
consumption is lifted; Woodshed can adopt against a stated contract. The four
questions and their rulings, with what each means here:

- **Action intents stay out of `sceno`, permanently.** They live in the
  consuming protocol, bound to an instance id plus the epoch and revision it
  was observed at. Woodshed's gesture story is therefore its own to define,
  and it inherits no vocabulary it would have to agree with.
- **`measure` is deleted.** Hosts stamp the measured extent on
  `ScoreItem.footprint`, which is where `StageGraphSnapshot` should put a
  note's or fingering's size. There is no separate measurement map to fill.
- **Per-item emphasis channels landed** as an open `Vec<(String, f32)>` on
  `ProjectedItem`. Practice recency is exactly this shape: a per-note or
  per-card scalar the view shades by, carried inside the scene rather than
  read back out of Woodshed's store.
- **Picking landed in `scenotime`**, resolving a point to the topmost
  instance through the space chain. This is directly the fretboard case: a
  `Space` mapping (string, fret) to screen means a click resolves to a note
  instance without Woodshed writing hit-testing at all.

**The multi-reason requirement is satisfied with no contract change**, which
was the open worry. A chord pair carrying diatonic, shared-tone,
voice-leading, and practiced-after is **four `RoutedRelation`s**, not one
relation with four reasons. Relations deliberately did not get a channel map,
because mere had already ruled that multi-edge is truth and collapsing to one
line is an experience setting. Selecting an edge exposing every applicable
reason and its authority, the requirement stated above, is the fanned form
rendered without dedup.

Two
consequences for this plan. First, `scenomise::relax` (2026-07-23) is
dependency-free relaxation aimed squarely at swatch-scale surfaces, which is
exactly what `related_swatch` is, so the retirement named above has a landed
mechanism waiting for it.

Second, the arrangement question is **not** answered by the freeze, and that
is deliberate rather than an oversight. The shipped arrangements remain
`Spiral`, `Board`, and `Geographic`; a circle of fifths and an interval map
are fixed semantic layouts none of them covers. The freeze settled the
contract's *shape* questions, not the arrangement catalog, which is a growth
axis: `Arrangement` is a closed enum, so adding a variant is a routine break
at `0.0.x` and remains available whenever it is earned.

The recommendation is to start with `Placement::Coordinate` inside a
Woodshed-owned `Space`, which needs no upstream change at all and is the
usage the contract note already blesses (a fretboard *is* a `Space`; a
Tonnetz or circle is a second fixed-layout frame). Promote a shared
arrangement upstream only when a second consumer wants the same layout,
which is the same "decide when a consumer forces it" standard the family
applied to every question it just closed. Woodshed proving the fixed-layout
case locally is the evidence that would justify promoting it.

### Replace the Song model

`SongDoc`, `SongBar`, `Tab::Song`, and the public Song engine vocabulary are
retired. They overstate the product and duplicate the ordered timing already
owned by Set.

Replace them with two narrower models:

- `LoopPlan`: a derived, immutable-for-a-pass clocked rendering of a Set. Its
  segments carry stable Card IDs, tempo, meter, duration, click behavior, and
  resolved pitches.
- `LoopSession`: capture state and recorded layers associated with stable
  segment IDs, plus export settings. Musical material remains in the Set.

The audio layer may retain an internal segment sequencer while it is migrated,
but `AudioBackend` should expose loop-plan, transport, capture, clear, and
export operations. Recorded buffers must not be preserved by vector index.

Looper requires finite timing. `LoopPlan::from_set` returns readiness issues for
Cards that cannot be clocked. The UI links each issue back to the affected Card.
Defaults such as bars per Card, meter, count-in, and export format live in
Looper settings. They are never silent hardcoded repairs to unresolved Cards.

### Keep tools reusable and state-light

The Fretboard renders resolved material for an instrument and tuning. It does
not own material. Rehearsal can provide the active Card and active note;
Looper can provide the active segment; the standalone tool can use the current
catalog selection.

The Metronome owns one shared musical clock and exposes compact contextual
controls. Rehearsal and Looper follow that clock rather than creating private
tempo authorities. The Tuner remains a live-input tool and may be opened beside
practice without changing the Set.

## Plan

### P1. Separate the product views without changing behavior

**Current state: partial, still open.** Product modules exist, but
`woodshed-views/src/stage.rs` remains the large shared state and coordination
owner. The decomposition done-condition is not met.

Split the monolithic `woodshed-views/src/stage.rs` into an application shell,
Stage, Rehearsal, Looper, Tools, Settings, and shared controls. Keep coordination
in `woodshed-core`; keep desktop realization in `woodshed-genet`.

Done when each product section can be changed and tested without editing one
multi-thousand-line screen file, existing session loading still works, and the
desktop host contains no product composition.

### P2. Establish navigation and canonical settings

**Current state: partial, still open.** Nested routes and the `AppSettings`
envelope landed. Several sections still expose placeholders rather than real
owned settings, so the one-owner done-condition remains open.

Replace `Tab` with a nested route model:

- Stage
- Rehearsal
- Looper
- Tools: Fretboard, Metronome, Tuner
- Settings: General, Appearance, Instrument, Tuning, Stage, Fretboard,
  Metronome, Tuner, Rehearsal, Looper, Audio and MIDI, Accessibility

Introduce a serializable `AppSettings` in `woodshed-core`. Separate portable
preferences from host-local device selection and transient runtime state.
Instrument and tuning form one current musical context shared by catalogs,
Fretboard, Rehearsal, and Looper. Contextual controls address fields in
`AppSettings` or that shared context directly.

Done when every exposed configuration has one owner, every settings page has a
route, old Practice routes open Stage templates, old Song routes open Looper,
and missing host devices do not corrupt portable settings.

### P3. Make Stage an explicit workflow

**Current state: partial, still open.** Catalog staging, the Set tray, shared
Card editing, and collapse landed. Reusable user Set save/load and the final
tray composition remain open.

Build Stage around a catalog rail, a material workspace, and a persistent Set
tray. The catalog rail contains Scales, Chords, Arpeggios, Progressions,
Exercises, and Set Templates. Search filters or jumps into these catalogs.

The primary action is **Stage**. It adds the currently configured material as a
Card. Progressions, exercises, and templates may stage several Cards, with a
preview of what will be added. The Set tray supports selection, reorder,
duplicate, remove, timing, touch, instrument placement, clear, and save/load.

Done when every catalog kind can stage valid Cards, the exact staged result is
visible before leaving Stage, and neither Rehearsal nor Looper needs its own
material editor.

### P4. Make the Stage graph the relationship and composition surface

**Current state: in progress.** P4a/P4b are landed. P4c/P4d have receipts for
one shared snapshot, relation routing, selection, resizing, and one expanded
Card. P4e has selectable deterministic layouts but not the full theory-map
catalog. P4f remains open, as do keyed-instance relations, the fretboard
`Space`, and retirement of the hand-rolled Related placement.

P4 grows the landed Related swatch and the in-progress Set graph into one
reconfigurable projection over the current Set, catalog material, musical
context, and practice evidence. It does not replace `Set`, `Card`, or
`woodshed-graph`; it makes their relationships directly legible and editable.

#### P4a. Finish the derived Set graph baseline

Give every staged Card occurrence a stable `CardId` which survives reorder,
save/load, selection, Rehearsal, and Looper lowering. Staging the same material
twice creates two IDs. Duplicating a Card creates a new ID; editing or moving it
retains its ID. Migrate existing saved Sets once by assigning missing IDs on
load and persisting them on the next save.

Project the ordered Set into occurrence nodes addressed by `CardId`, with the
visible number derived from current Set order and `Next` edges derived between
adjacent occurrences. Keep the ordered `Vec<Card>` authoritative while the
product remains linear. Repeats and Set looping use the existing loop model.
Promote `Next` into authored flow only when an actual branching or alternate-
ending workflow requires it.

Replace the current all-or-nothing sequence-edge toggle with a serializable
relation-visibility set. Preserve node selection, graph/list cursor parity,
and graph focus through reorder or removal by stable identity rather than
vector index.

Done when duplicate material yields distinct stable occurrence nodes, reorder
changes numbering without changing identity, removal drops only incident
projected edges, a round-trip preserves the selected Card, and hiding `Next`
changes only the view.

**Landed 2026-07-24** (see Progress), with unit tests per done condition and a
headed receipt (`scenarios/p4a_occurrence_identity.scn`). `Card` identity is not
yet consumed by the Looper: preserving captures by occurrence rather than by bar
index is P7's work, and it is the reason this slice came first.

#### P4b. Preserve typed musical relationships

Split neighbor ranking from relation truth. Replace the flattened
`RelatedMaterial { reason, score }` boundary with typed material relations that
retain source, target, direction, kind, weight or distance, explanation, and
provenance. Several relations may connect one pair. Ranking operates over
these records and may choose a display order without deleting multiplicity.

Grow the deterministic vocabulary from the existing `Contains`,
`FitsInScale`, and `Realizes` relations toward:

- contains pitch class, interval, degree, or material;
- mode of, relative to, parallel to, and fifth of;
- diatonic in, borrowed from, extends, alters, or substitutes for;
- dominant of and resolves to;
- shared tones and symmetric voice-leading distance;
- used together or adjacent within a catalog progression;
- practiced before/after and engagement strength.

Keep root-independent formulas, contextual realizations, and instrument
placements distinct. `scale:Major` and `chord:Dominant 7` remain catalog
formula identities. A view may derive `C major` or `G7` from formula + tonic
without multiplying the durable catalog twelvefold. Concrete voicings and
string/fret positions remain Card setting and projection output.

Add first-class pitch-class and interval identities when the first interval or
circle projection needs them. Do not encode those relationships only in label
text. Progression continuations must carry their context: current key,
preceding material, and whether the reason is theoretical, historical, or
learned.

Done when an edge can explain all applicable relationships between two
materials, formula and keyed-instance identity are unambiguous, deterministic
relations are available without history or ML, and ranking no longer erases
relation kinds.

**Landed 2026-07-24** (see Progress), with unit tests per done condition and a
headed receipt (`scenarios/p4b_typed_relations.scn`). The keyed half is
deliberately not in it: `diatonic in`, `borrowed from`, `dominant of`,
`resolves to`, `relative to`, and `parallel to` are contextual relations that
exist only once a tonic is chosen, so they land with the keyed-instance layer
and first-class pitch-class identities rather than being faked over formulas.

#### P4c. Compose one Stage projection snapshot

**Current state: landed for the current Set/Related snapshot, with follow-ons
still owned by P4e/P4f.** Suggested-frontier and keyed-context behavior remain
open and must not be inferred from the scene-canvas receipt.

Expose a portable `StageGraphSnapshot` from Woodshed core or a narrow adapter,
Woodshed's source adapter into `sceno`'s scene contract (see the scenograph
boundary decision above). Its inputs are the Set occurrence graph, the focused
catalog/material identity,
the current tonic/instrument/tuning, typed theory relations, practice evidence,
and optional analysis signals. Its output carries stable node-instance IDs,
typed edges, relation authority, selection, and representation hints. It must
not depend on Genet, Chisel, wgpu, Burn, or Mere's kernel.

Projection settings include:

- focus and expansion depth;
- visible relation families;
- layout/preset;
- theory, history, and learned-suggestion layers;
- node label mode and edge explanation mode;
- optional pinned visual positions;
- level-of-detail thresholds for glyph, summary, and Card forms.

Keep these settings separate from musical truth. Durable user choices live in
`AppSettings::stage`; transient hover, animation, and temporary expansion stay
in view state. The compact Related swatch and expanded Stage graph consume the
same snapshot and actions.

Done when one snapshot drives both surfaces, relation filters do not rebuild or
mutate the Set/catalog, disabling history retains deterministic theory, and a
suggested frontier is visibly distinct from staged Card occurrences.

#### P4d. Make nodes expand into Cards without changing identity

**Current state: landed for one selected Card.** The headed receipt covers
resize, expansion, edit routing, and collapse. Broader multi-node semantic zoom
remains open.

Give each staged occurrence three representations of the same Card:

- **Glyph:** number or Roman numeral, suitable for dense maps.
- **Summary:** material name, function, key, and compact state.
- **Card:** the shared editor with material, setting, touch, timing, voicing,
  audition, and Stage actions.

Selecting a node synchronizes the Set cursor and the alternate list/tray.
Expanding it changes projection state and gives the Card an assigned region;
editing it updates the one Card. Shrinking it restores the compact
representation. Expansion may move neighboring nodes but must preserve graph
focus and camera. Respect reduced-motion settings.

Keep semantics and actions in ordinary Cambium/`xilem_serval` elements. The
painted graph layer may draw geometry underneath, but each visible node and
edge explanation needs an accessible semantic target. The current external
selected-Card editor is an acceptable bridge; P4d is complete only when the
expanded Card occupies the node's projected region rather than appearing as an
unrelated panel.

Done when a numbered node expands into its editable Card and collapses back
without losing identity, selection, edits, keyboard focus, or graph position;
the same operations remain available through the list projection.

#### P4e. Ship a small projection catalog

**Current state: partial.** Ten deterministic arrangements are selectable and
have a two-layout headed receipt. Circle-of-Fifths context and the triadic
Tonnetz have September 6 implementation and rendered receipts; the other
contextual theory maps remain open.

**2026-09-06 contextual arrangements:** Circle of fifths and Tonnetz are
tracked in [Musical Projections](2026-09-04_musical_projections_plan.md#context-around-the-set-2026-09-06).
An arrangement selects relevant background material as well as coordinates.
Clicking background material focuses and expands it while preserving placement;
audition and Add to Set remain separate actions. The existing geometric Circle
layout must remain distinguishable from the musical circle of fifths.

Avoid one universal force layout. Each view should state which relationships
and coordinate rules make it intelligible:

1. **Set sequence:** numbered staged occurrences, `Next` as the primary path,
   harmonic and evidence edges optional.
2. **Focused relationships:** the current material with progressively
   expandable typed neighbors. Arbitrary depth is a query capability; the view
   reveals it on demand rather than drawing the entire catalog.
3. **Circle of fifths:** contextual keys in a fixed cycle, expandable into
   scales, diatonic chords, relative modes, and borrowed material.
4. **Interval map:** pitch classes connected by selected intervals, with paths
   projected onto the current instrument.
5. **Scale family:** scales arranged by mode, contained degrees, or set
   difference.
6. **Voice leading:** chords placed by motion cost with shared and moving tones
   exposed on edges.
7. **Progression possibilities:** a directed frontier conditioned on key and
   preceding staged Cards, separating deterministic function, catalog usage,
   practice history, and learned suggestions.

The Set-sequence and focused-relationship views are the first two consumers.
The circle of fifths is the first full theory-map acceptance surface because it
forces contextual material identity, fixed semantic layout, nested expansion,
and Stage/fretboard synchronization without requiring ML. These layouts are
`scenomise` arrangements over `sceno` frames rather than Woodshed-owned
swatches. The shipped arrangements (`Spiral`, `Board`, `Geographic`) do not
cover a circle of fifths or an interval map, so this catalog is where Woodshed
either contributes a fixed-semantic-layout arrangement upstream or places
authored coordinates in its own frame (see the scenograph boundary decision).

Done when the same focused material can move between Set, relationship, and
circle projections without changing musical truth; projection choice and
relation filters persist as settings; and at least two layouts have headed
interaction receipts rather than static screenshots.

#### P4f. Join graph understanding to sound and practice

**Current state: open.** Current relation selection and history projection do
not yet satisfy the audible, playable, persisted progression flow below.

Selecting a node updates Stage and the Fretboard. Selecting an edge exposes its
reasons and offers an audition appropriate to the relation: shared tones,
before/after chords, or animated voice movement. Staging a frontier node uses
the ordinary Stage action and states where the new occurrence will enter the
Set. The initial behavior may append; insertion after the focused occurrence
must be an explicit action before it is offered.

P5 supplies the shared clock and event-position identity. P6 and P7 consume
the same `Next` traversal for Rehearsal and Looper. Practice events annotate
the evidence layer after their honest lifecycle boundaries rather than after a
mere hover or projection change.

Burn-backed producers may later supply embeddings, transition likelihood,
clusters, or personalized frontier ranking. Keep inference off the render
path. Every learned edge carries model/version, confidence, and generation;
turning the learned layer off leaves the deterministic engine intact. Accepting
a suggestion is an explicit Stage or relation action.

Done when a user can stage a short progression from the visible frontier,
expand its Cards to choose voicings, hear and see why each transition relates,
run the numbered Set through Rehearsal, and reopen the same Set and projection
settings after restart.

### P5. Unify the clock and visual articulation

**Current state: open.** Existing metronome and transport pieces do not yet
form the stable shared clock/event-position contract in this phase.

Create one core clock snapshot with beat, subdivision, Set cursor, Card-local
progress, and active sequence step. Map scale, arpeggio, chord, and exercise
sequences to stable display-position IDs so the Fretboard can highlight the
same event the audio backend articulates.

Done when changing tempo affects click, automatic Card dwell, audio preview,
and highlighted dots together; pause/resume does not drift; and manual Cards
remain manually advanceable.

### P6. Finish Rehearsal as the guided Set runner

**Current state: partial, still open.** A guided runner exists, but the mixed
Set, articulation, edit recovery, and keyboard acceptance conditions have not
all been re-receipted as one flow.

Rehearsal streams the current Set with previous/current/next context, a large
instrument view, transport, Card progress, and loop controls. It supports Set
looping and focused Card looping. Per-Card timing and touch remain editable via
the shared Set controls. Tool panels may open without leaving the rehearsal.

Done when a mixed Set can run from first Card to last, loop according to the
selected mode, articulate sequences in sync, recover predictably after edits,
and remain fully operable by keyboard.

### P7. Rebuild Song as Looper over Set

**Current state: open.** `SongDoc` and the current song-shaped audio lowering
still exist; `LoopPlan`/stable-segment capture and companion persistence have
not landed.

Add `LoopPlan` lowering and readiness validation. Migrate the current song
engine to a loop engine that plays Set-derived segments and preserves captures
by stable segment ID. Provide count-in, play/stop/rewind, replace, overdub,
clear, input level, and capture status.

Export the rendered backing plus captured loop to WAV through an explicit host
file-save seam. Keep raw captured audio available to the session while the app
is open; define companion-file persistence before claiming that recordings
survive restart.

Done when a clock-ready mixed Set loops without a parallel bar editor, capture
survives Set reordering where Card identity is retained, exported WAV duration
and tempo match the LoopPlan, and the UI contains no Song or DAW language.

### P8. Adaptive polish and release acceptance

**Current state: open.** Windows alpha behavior exists. Three-width product
acceptance, touch/reduced-motion coverage, and Mac/Linux receipts remain open.

Give each section explicit wide, medium, and narrow compositions. Wide layouts
may expose the Set tray and tool panels simultaneously. Narrow layouts use one
primary workspace with drawers or sheets for catalogs, Set, and tools. Adapt to
available width and input capabilities rather than naming device classes.

Add focus order, visible focus, accessible names and state, reduced-motion
behavior, touch targets, empty/loading/error states, and a compact command map.
Use design tokens for spacing, type, color, focus, motion, and control size.

Done when Stage, Rehearsal, Looper, Tools, and every Settings page work at the
three width bands; core flows work with mouse, keyboard, and touch-sized
controls; theme contrast is checked; Windows packaging passes; and Mac/Linux
receipts are recorded before those platforms are advertised.

## Migration and stop rules

- Make one bounded persisted-session migration, then delete the obsolete
  models. Do not maintain Stage/Practice or Looper/Song in parallel.
- Preserve a user's Set and settings. Best-effort import Song bars as staged
  chord Cards only if identity and timing can be represented honestly.
- Rehearsal owns guided visual articulation. Looper owns capture and export.
- Set owns ordered practice material. Fretboard owns its representation.
- Instrument and tuning are shared context. Audio/MIDI device IDs remain
  host-local.
- WAV is the first export target. Additional formats require a separate need.
- Multi-track arrangement, destructive waveform editing, plug-ins, and mixing
  are outside this plan.

## Findings

- The existing portable `Set` and `Card` model already carries material,
  setting, touch, timing, provenance, cursor, and loop mode.
- `Set::graph()` addresses occurrences by stable `CardId` and filters by a
  serializable relation set (P4a, 2026-07-24). The Stage scene now carries
  typed parallel relations, ten deterministic arrangements, direct canvas
  sizing, and a selected Card assigned to its node's projected region
  (P4b-P4e). Keyed-instance relations and multi-node semantic zoom remain.
- Card is a sound domain unit, but the current UI should not force one visual
  card treatment across Stage, Set tray, Rehearsal, and Looper.
- Chisel is a good fit for custom-painted material projections. Its semantic
  event/action seam is still a placeholder, so ordinary `xilem_serval` elements
  remain the right owner for Card interaction and accessibility.
- `woodshed-graph` projects scales, chords, arpeggios, progression and exercise
  identities, typed theory relations, and practice lineage into both the
  Related list and Chisel neighborhood. The flattened `{reason, score}` boundary
  is gone (P4b, 2026-07-24): a pair now carries every applicable relation, each
  with its own weight, measurement, and authority. What remains thin: keyed
  relations (diatonic in, borrowed from, dominant of, resolves to, relative and
  parallel) are deliberately absent, because they exist only under a chosen
  tonic and belong to the keyed-instance layer with first-class pitch-class and
  interval identities; the center-star snapshot rebuild is still there.
- Ranking crowding is a real effect, found by receipt: `Major` appears in nine
  catalog progressions that all score 96, so a six-row panel showed one relation
  family and no harmonic neighbour at all. Display now interleaves families,
  which deletes nothing a longer list would have kept, but it is a symptom worth
  remembering — a flat weight per relation kind ranks by family, not by
  usefulness to the player.
- The Related panel scrolls inside a row whose height the fretboard sets, so at
  1500x1200 roughly two rows are visible and the multiplicity line needs a
  scroll to reach. The relations are correct and observable; their presentation
  is not yet. Panel height belongs with P8's adaptive compositions.
- The current Cambium `GraphCanvasSwatch` carries uniform nodes and untyped
  `from/to` edges. It is enough for the Set-graph wiring proof. Directed and
  typed edge treatments, per-node regions, semantic zoom, and live Card slots
  belong at the shared projection boundary rather than as Woodshed-only swatch
  exceptions.
- Catalog formulas and contextual realizations are distinct. The catalog owns
  `Major` and `Dominant 7`; a projection derives `C major` and `G7` under the
  current tonic. A Card owns the concrete instrument setting and touch.
- Current `PracticeSet` values already lower to the same Cards, so Practice can
  become Set Templates without a new data model.
- Current `SongDoc` duplicates ordering, tempo, and duration. Progressions can
  currently bypass Set and become Song bars directly.
- Recorded loops are preserved by bar index during Song updates. Insert or
  reorder can attach a recording to the wrong musical material.
- Offline WAV export exists for sequencer patterns, but captured loop export is
  not exposed through the application backend.
- Settings currently combines theme/layout, device status, MIDI, and latency
  in one screen. Several durable settings still live as view-owned fields.
- Responsive width classes exist, but product sections do not yet have a
  coherent narrow-screen information architecture.

## Progress

- **2026-07-24, P4b landed — typed relations, and the end of the flattened
  reason:** `woodshed-graph`'s public boundary was `RelatedMaterial { reason,
  score }`, one winning string per neighbour, and the index deduplicated by
  target so the second and third ways two materials relate were discarded before
  anyone could see them. It is now `MaterialRelation { source, target, kind,
  weight, distance, shared_tones, explanation, authority }` with
  `RelatedNeighbor` carrying **every** relation for a pair, `relations_between`
  for the full list, and `RelationKind` naming a 14-member deterministic
  vocabulary with `inverse`/`is_symmetric` so one computation records both
  directions honestly. New derived relations, all root-independent: chord
  `Extends`/`ExtendedBy` (strict subset), `Alters` (same size, one tone moved),
  `SharesTones` and `VoiceLeadsTo` as *separate* records rather than one blended
  score, scale `ModeOf` (rotation of the same interval set) and `ScaleNeighbor`
  (one degree apart), and `UsedTogether` for chords the catalog itself puts
  adjacent inside a progression. Every record carries its `RelationAuthority`
  (Catalog / Computed / Evidence), and `MaterialRelation::evidence` is the only
  public constructor, so an observation cannot enter wearing catalog authority.
  Core's history ranking now *inserts* an evidence relation instead of
  overwriting the theory reason, which was the erasure the plan named. The
  Related row shows its primary reason plus an `also ...` line naming the other
  kinds. Verified: 14 graph tests (multiplicity survives ranking, authorities
  are attributable, modes relate as modes, deterministic relations need no
  history), 46 core tests, and `p4b_typed_relations.scn` asserting on the
  relation records themselves rather than on rendered text: `RESULT ok`, two
  captures. **Found by receipt**: the first run failed honestly. Every visible
  neighbour of `Major` carried exactly one relation, because nine catalog
  progressions all score 96 and filled the six-row panel; the multi-relation
  pairs sat at rank 10. Added a display-side family interleave (documented as a
  display policy that deletes nothing) and a test pinning it. Recorded but not
  fixed: the panel's own height leaves about two rows visible.

- **2026-07-26, the evidence layer adopted stemma.** Asked whether the clock work
  should generalize to mere first; the investigation inverted the question. **Mere
  already had the general thing and woodshed had built a second one**:
  `chartulary::stemma`, already a dependency through `woodshed-graph`, maintains
  per-subject `first_seen_at_ms` / `last_seen_at_ms` / `visit_count` and
  aggregates per-pair traversals with their own recency — the exact inputs a
  decayed strength model needs and exactly what the local log did not keep. Its
  `context: X` is a generic per-visit payload, and `StemmaSnapshot` is serde, so
  it rides woodshed's existing String-moving `Storage` seam with no new
  persistence lane. **Adopted rather than deferred** (Mark: unification is worth
  work up front, rather than growing two implementations of one idea) — and the
  deferral had been incoherent anyway, since the trigger I named was the strength
  model, which is the next task.
  `PracticeHistory` keeps its name, vocabulary, and every consumer; only the store
  beneath it moved. Woodshed still owns the musical judgment (`EngagementKind`,
  `is_practice`), the substrate owns the structure — the same line P4b drew.
  **Three mappings decided by the code, not by preference.** The engagement kind
  rides the visit `context`, not stemma's `TransitionKind`, whose variants answer
  "how did you get here" and map one-to-one onto a browsing trace (recorded on
  that type in chartulary, which had "generalize TransitionKind" as a declared
  open decision; the finding is that if it is ever generalized it wants a type
  parameter, preserving `Copy`/`Hash`/rkyv, not an open string tail). Datedness
  rides the context too, since `visit_entry` takes a `u64` and not an `Option`, so
  an undated engagement is `at_ms = 0` plus `dated: false` and no reader may read
  it as 1970. And **the stated reason is not the walked path**: a test failure
  caught that discarding `from_id` in favour of stemma's parent edge silently lost
  provenance — a first engagement has no parent yet can still name its source — so
  `from_id` lives in the context as the player's claim while the lineage keeps the
  path actually walked. Both are now queryable and distinct:
  `related_transition_count` (the stated reason, ranking a suggestion the player
  has taken before) and `traversals` (the lineage's own count plus its recency,
  which is what the strength model will decay).
  Gained for free: `engagement_count` maintained upstream, per-pair recency, and a
  class of bug deleted rather than fixed — the sequence counter that could collide
  on a legacy load no longer exists, because the lineage has no counter.
  One bounded migration: a session written as the flat log replays into the
  lineage on load and persists as a lineage from then on, with each engagement's
  kind, time, and span preserved exactly. Verified: 8 history tests including the
  flat-log replay and the stated-reason/walked-path separation, 53 core + 14 graph
  + 6 views + 167 woodshedding green, host builds, and both headed receipts re-run
  `RESULT ok`.
  **Next, on the substrate that now supports it**: the strength function
  (recency-decayed, kind-weighted, emitting both `PracticedBefore`/`PracticedAfter`
  plus subject-level strength) replacing core's `weight = 90 + count.min(10)`.
  Retention stays after that, and stemma does not solve it either — `delete_owner`
  collects ownerless branches, not old visits — so the Alembic/Athanor forgetting
  pass remains the answer, with `codicil` as the append-only home a growing
  lineage wants.

- **2026-07-26, the evidence layer gets a clock and a stopwatch:** The layer
  the plan defines as "observed practice transitions and engagement strength,
  with event kind and observation time retained" was failing its own spec:
  `PracticeEvent` carried only a `sequence` counter, with a comment admitting a
  host could enrich it "when history gains calendar views". Everything downstream
  wanted that field — strength is inherently time-weighted (twenty reps last year
  is not strength today), retention cannot evict what it cannot date, and the
  ranking fudge in core was `weight = 90 + count.min(10)` standing in for a
  model. So: `PracticeEvent.at_ms` (Unix epoch millis) and `practiced_ms` (the
  measured span, where an event has one), `record` now takes the time positionally
  so no call site can forget it silently and returns `&mut PracticeEvent` so a
  caller with a span attaches it (one door, optional enrichment, rather than a
  second timed variant to pick wrong). Reads that make the data mean something:
  `total_practiced_ms`, `last_seen_ms`, `has_times`.
  **The clock belongs to the host.** Core and views read none of their own — a
  browser host has a different clock and the portable core has no clock at all —
  so `UiState.now_ms` is refreshed once per frame by `woodshed-genet` and every
  engagement is dated from there. `None` means *unknown*, never epoch zero, or
  every legacy event would silently become maximally ancient.
  **Elapsed practice is now measured, not counted.** `record_rehearsal_cursor`
  opens a span when a card becomes active; `complete_rehearsal_cursor` closes it,
  and completing one card opens the next one's, so spans neither overlap nor
  restart from the run's beginning. A `checked_sub` means a system-time change
  mid-session yields no measurement rather than a wrapped one. This is the
  difference the plan already asserted and could not previously honour: preview
  and staging are evidence of *interest*, completed practice time is evidence of
  practice.
  **Found while writing the legacy test**: `next_sequence` is serialized but
  defaults to 0, so a session missing it would have minted duplicate sequences
  over its own events. Same shape as the `CardId` mint, fixed the same way (the
  mint continues past the highest stored event, not merely past the counter).
  Verified: 5 new `history` tests (supplied time retained, measured practice
  outweighing a pile of previews, spans accumulating, the legacy-session
  migration and its collision), 4 new view tests (the real span, no clock, a
  backwards clock, consecutive cards), 50 core + 6 views + 167 woodshedding + 14
  graph green, desktop host builds. No wire migration code needed: both fields are
  `#[serde(default)]`, so old sessions load as undated and persist dated on the
  next save.
  **Still open, in dependency order** (the chain this slice unblocks): a strength
  model to replace the count fudge (recency-decayed, kind-weighted, emitting both
  `PracticedBefore`/`PracticedAfter` and a subject-level strength the
  practice-strength overlay needs), then retention — the real wall, since
  `events` grows forever inside the sealed session JSON rewritten on every save.
  Retention is where mere's **Alembic/Athanor** pattern is the answer rather than
  an analogy: raw events as the short layer, distilled per-subject and per-pair
  strength as the long layer, a forgetting pass evicting raw events without
  losing the strength they contributed, and `codicil` as the append-only home a
  growing event log actually wants.

- **2026-07-24, P4a receipt — woodshed drives itself:** Woodshed had no
  self-drive lane, so its receipts were SendKeys plus a desktop grab, which the
  harness notes warn loses the foreground race and can photograph the wrong
  window. It now consumes `genet-probe`: the generic half (scenario parsing, the
  verb loop, selector resolution, assertions) is the shared crate, and what
  landed here is only woodshed's half —
  [`crates/woodshed-genet/src/scenario.rs`](../crates/woodshed-genet/src/scenario.rs)
  implementing `Automatable`/`Driveable` (surfaces, a typed snapshot, semantic
  events diffed from real state transitions, named commands, pointer routing
  through the app's own hit-test path) plus an in-process capture: the frame's
  own rasterized view composed into a `COPY_SRC` target and read back, so a
  capture needs no compositor, no foreground, and no ffmpeg. `WOODSHED_STATE`
  points the session at a scratch profile, because an automated run would
  otherwise read and then overwrite the real practice session.
  `p4a_occurrence_identity.scn` stages one catalog material three times, so the
  three occurrences are label-identical and only identity can tell them apart:
  it selects the second through its DOM key, reorders it, and asserts the id
  held while the number moved 2 -> 3, then hides the relation family and asserts
  the edges went while the occurrences stayed. `RESULT ok`, four captures in
  `testing/woodshed/scenarios/p4a_occurrence_identity/`, run through
  `testing/woodshed/run-scenario.ps1`. **Found by looking at the frames**: the
  relation toggle sat beside the swatch and was overdrawn by node labels that
  paint past the swatch's box. Fixed by giving the controls their own row; the
  underlying overflow (a 520px swatch with labels wider than its node spacing)
  is layout work for P4d/P4e, recorded rather than papered over.

- **2026-07-24, P4a landed — occurrence identity:** Every staged Card carries a
  `CardId`, minted by the owning `Set` and never reused, so staging the same
  material twice yields two occurrences and duplicating mints a third while the
  original keeps its own. `Set` gained `from_cards`, `ensure_card_ids`,
  `index_of`, `id_at`, `cursor_id`, `select_id`, `card`/`card_mut`;
  `Set::graph()` addresses nodes and `Next` edges by id, with the visible number
  and serpentine slot derived from current order. The all-or-nothing edge toggle
  became `StageSettings::visible_set_relations`, a serializable set over
  `SetGraphEdgeKind` with `ALL`/`label`, so the harmonic, evidence, and
  suggestion families join it as members; `SetGraph::with_relations` filters the
  projection without touching Set truth, and the Settings page lists one entry
  per family. The Set-graph swatch is now keyed by `CardId` (selection, hover,
  DOM key `set-card-<id>`), and native focus is tracked honestly through
  `graph_canvas_swatch_with_focus` rather than painting a ring where the
  keyboard is not. One bounded load migration in `apply_persisted`: legacy Sets
  gain ids, the legacy boolean folds into the relation set, and both persist on
  the next save; the legacy key stops being written. Verified: 7 new
  `woodshedding` tests (167 total) covering distinct occurrences, reorder,
  removal without id reuse, relation hiding, round-trip identity, and legacy
  migration idempotence; a new `woodshed-core` storage test for the settings
  fold (45 total); `cargo check` green for `woodshed-views` and
  `woodshed-genet`. **Build blocker found and repointed**: genet's 2026-07-24
  sweep moved the family to cambium 0.3.1 / cambium-winit 0.3.0 / sprigging
  0.2.1, which silently stopped matching this workspace's 0.2.0 pins, so the
  local `[patch]` entries went unused and the host built against the published
  0.2.0 API without `graph_canvas_swatch` or `on_hover`, while looking green.
  Resolved by taking all three from **genet.git by branch**, not from crates.io
  (Mark, 2026-07-24: cambium-winit will never be published, and the
  consolidation rewired it onto crates inheriting genet's `publish = false`;
  hocket reached the same conclusion the same day off a clean Linux checkout).
  One source for the family is not a preference: `cambium-winit` path-deps
  cambium and sprigging inside the genet repo, so a git `cambium-winit` beside a
  registry `cambium` puts two copies of the same types in one graph, and the
  published cambium/sprigging carry the crates.io `paint_list_api` while the
  rest of the stack git-deps netrender's. Two dead `[patch.crates-io]` entries
  removed, and `tinct`'s redirect moved into the genet.git table where it can
  actually match: it had been keyed to its retired standalone repo, so the local
  checkout was silently unused. **The lesson generalizes**: every one of these
  announced itself only as a "patch was not used" warning, which is the one
  cargo message this workspace must never scroll past. No headed receipt yet: the changed
  surfaces (node selection, the relation toggle's label) want one before P4a is
  called finished.

- **2026-07-24, the gate opened:** Re-checked the scenograph sequencing clause
  against the family's actual state. Mere and isometry both proved the scene
  contract on 2026-07-22 (mere's `cartography::scene_out` + persisted spiral
  score with a headed receipt; isometry deleting `Overmap::layout` and emitting
  the same score/scene types), P5's geographic fixture landed the same day, and
  2026-07-23 added `scenomise::relax` plus graphshell's consumption of
  `scenotime` diffs. So the "wait for mere and isometry" half of the gate is
  satisfied and only the freeze remains; the boundary decision now names the
  specific open items rather than a general wait. Also recorded that the family
  moved into mere at `crates/scenograph` (0.0.2) in the 2026-07-23
  consolidation, and that Woodshed's fixed semantic layouts are not covered by
  the shipped `Spiral`/`Board`/`Geographic` arrangements, which is the first
  design question Woodshed owes the contract. Counterpart notes recorded on the
  mere side in the scene contract note and the prior-art brief. No code
  changed.

- **2026-07-22, scenograph reconciliation:** Named the shared projection engine
  as the `scenograph` family (`sceno`/`scenomise`/`scenotime`) founded in mere's
  projection-engine prior-art brief, where Woodshed is already forcing-function
  #3. Reframed `StageGraphSnapshot` as Woodshed's source adapter into `sceno`'s
  scene contract, the analog of mere's `cartography` graph adapter, and recorded
  the fretboard-as-frame, note-as-point, fingering-as-path mapping; the P4e
  catalog is `scenomise` layouts, not Woodshed swatches, and `related_swatch` is
  the first placement the contract retires. Recorded Woodshed's role as the
  source-model sanity check: a dense, deterministic, multi-relational fixture
  that needs no authoring, complementary to isometry's scene-side proof and not
  a replacement for the proof ladder. Sequencing unchanged on the truth side
  (P4a identity, P4b relations proceed now as contract pressure); scene-contract
  consumption waits for mere and isometry to prove and freeze it. No code
  changed.

- **2026-07-21, Stage graph projection plan:** Expanded P4 from a ranked Related
  neighborhood into the authored Stage-graph direction. The plan now separates
  Set, deterministic theory, practice evidence, and learned suggestions;
  requires stable Card-occurrence identity and typed multi-relations; defines
  node-to-Card semantic zoom; and names the Set, focused relationship, circle
  of fifths, interval, scale-family, voice-leading, and progression projections.
  No code was changed in this planning pass. The existing dirty-tree Set graph
  remains the P4a wiring baseline described below.

- **2026-07-21, authored Set graph slice:** Began evolving the Set tray from a
  card-only arrangement into a graph projection of the same Set. The portable
  Set now derives distinct numbered Card-occurrence nodes and typed `Next`
  edges; the view exposes sequence-edge visibility as a durable Stage setting.
  The graph remains a projection: staging, editing, Rehearsal, Looper, and
  persistence still address the one Set. Rich harmonic edge layers, alternative
  layouts, direct graph authoring, and stable Card identity across arbitrary
  branching remain follow-ons.

- **2026-07-11:** Reconciled the maintainer's product model with the live Set,
  PracticeSet, SongDoc, AudioBackend, capture engine, persistence, navigation,
  settings, and responsive view seams. Wrote the replacement plan and updated
  the project authority/index language.
- **2026-07-11, related-material slice:** Added a cached, explainable
  `woodshed-graph` neighbor query; mapped stable graph identities to core Stage
  selections; and added a responsive Related panel with Select and Stage
  actions. Renamed the existing `+ Rehearse` action to `Stage`. Verified 8
  graph tests and 35 core tests, `cargo check -p woodshed-views`,
  `cargo build -p woodshed-genet`, and a live Windows receipt at 1100x664.
  The receipt proved that staging a suggestion changes the catalog projection
  and increments the Set; the test Card was removed afterward. P4 is partial:
  practice-history ranking, arpeggio nodes, richer relations, and the Chisel
  graph view remain.
- **2026-07-11, engagement-history slice:** Added persisted `PracticeHistory`
  with typed Previewed, Staged, Rehearsed, Completed, Looped, and Recorded
  events over stable catalog IDs. Stage, Related-Stage, preview, rehearsal
  start, manual running steps, and automatic rehearsal advance now record at
  their honest boundaries. Related choices remember their origin; repeated
  Stage paths rise in the panel with a history explanation, while previews do
  not affect ranking. The panel also shows the four most recent engagements,
  and a running rehearsal records Completed when it advances past a Card.
  Verified 37 core tests, 8 graph tests, and checks for the views and desktop
  host. P4 remains partial: elapsed practice facts, history retention settings,
  arpeggio nodes, richer harmonic scoring, and the Chisel graph view remain.
- **2026-07-11, P1 decomposition slice:** Moved Settings with MIDI/calibration,
  Rehearsal, the current Song-to-Looper surface, Related/history, and Set
  Templates into owned view modules. `stage.rs` fell from 2,087 lines to about
  1,040 and now concentrates shared app state plus the catalog/fretboard Stage
  surface. This is behavior-preserving and keeps coordination in the existing
  `UiState`; P1 remains partial until shared shell controls and the remaining
  large `UiState` coordination are separated.
- **2026-07-11, harmonic-neighborhood slice:** Added stable arpeggio graph
  identities and scored direct relations plus shared-tone and symmetric
  voice-leading chord affinity. Core now projects the ranked neighborhood once
  for both the Related list and a Chisel graph glyph; selecting an arpeggio
  changes both surfaces to the same identity. Also continued the P1 shell split
  with owned Stage, Rehearsal, Looper, Tools, and Settings sections and Catalog
  and Templates Stage pages. Verified 38 core tests, 10 graph tests, checks for
  the views and desktop host, a desktop build, and a live Windows receipt at
  1100x664. P4 remains partial: graph-node selection, context-sensitive
  instrument/tuning filtering, history controls, dismissals, and progression
  or scale-to-scale affinity remain.
- **2026-07-11, Related controls slice:** Added persisted settings for history
  ranking and the Chisel neighborhood, per-identity Hide actions, and a Restore
  hidden control. Turning history ranking off preserves deterministic theory
  suggestions, and hidden identities are removed from both the list and graph.
  Verified 39 core tests, checks and a desktop build, plus a live Windows
  receipt covering both toggles and dismissal restoration. P4 still needs
  Chisel event routing before graph nodes can select material directly; it also
  needs instrument/tuning context and broader progression/scale affinity.
- **2026-07-11, Settings routes slice:** Replaced the mixed Settings surface
  with explicit General, Appearance, Instrument, Tuning, Stage, Fretboard,
  Metronome, Tuner, Rehearsal, Looper, Audio and MIDI, and Accessibility
  pages. Each live control projects the same state used contextually elsewhere;
  pages with missing backend/configuration support say so directly. Verified
  39 core tests and checks for views and the desktop host. P2 remains partial
  until the durable fields move under a canonical core `AppSettings` envelope
  and nested page selection is persisted.
- **2026-07-11, AppSettings slice:** Added a canonical core `AppSettings` with
  typed Appearance, Instrument, Tuning, Stage, Fretboard, Metronome, Tuner,
  Rehearsal, Looper, Audio/MIDI, and Accessibility subsections. Existing durable
  theme, tuning, Related, layout, and tempo fields now live under those Rust
  owners while Serde flattening preserves the legacy JSON keys. Empty
  subsections mark routes whose runtimes do not yet implement durable knobs.
  Verified 39 core tests, including old flat-session migration and flat-wire
  round-trip coverage. The nested Settings page is now a core enum and restores
  with the session. P2 remains partial until contextual controls bind directly
  to `AppSettings` and the currently empty subsections gain real runtime knobs.
- **2026-07-11, contextual settings binding:** `UiState` now owns one
  `AppSettings`; theme, fretboard layout, tuning, Related/history behavior,
  metronome tempo, and the active Settings page read and write it directly.
  Transient transport playback still lives in `TransportState`, with tempo
  mutation routed through `UiState` so the durable metronome setting stays in
  sync. `PersistedSession` snapshots the canonical model instead of rebuilding
  a parallel settings copy. Verified 39 core tests and checks for views and the
  desktop host. P2 now remains open only for real settings in the empty
  Instrument, Tuner, Rehearsal, Looper, Audio/MIDI, and Accessibility sections.
- **2026-07-11, Stage Set tray slice:** Added a Stage-owned Set tray showing
  ordered Card kind, label, touch, dwell, tuning, and recipe provenance. The
  selected Card can move, duplicate, or be removed; the tray also owns clear,
  Set looping, and the transition into Rehearsal. Set Templates now fills the
  tray without navigating away from Stage. Verified views/desktop checks, 39
  core tests, a desktop build, and a live 1100x664 receipt over a real 12-card
  Set; duplicate/remove changed 12 -> 13 -> 12 as expected. P3 remains partial:
  Card timing/touch/placement editing still lives in Rehearsal, and the tray is
  document-bottom rather than a sticky/collapsible bottom rail.
- **2026-07-11, shared Card editor slice:** Moved touch, dwell, tempo override,
  and fret-window mutations behind shared `UiState` actions and exposed the
  same selected-Card editor in both the Stage Set tray and Rehearsal. The Stage
  tray can now collapse without changing the durable Set. P3 remains partial
  on reusable user Set save/load and whether the tray should become a sticky
  bottom rail; it is currently a collapsible document-bottom surface.
- **2026-08-18, scene-contract founding (the L1 release gate):** woodshed took
  its first `sceno` dependency and `StageGraphSnapshot` now exists, at
  `crates/woodshed-core/src/stage_scene.rs`, projecting one Set into the frozen
  0.0.3 contract. 10 module tests, 68 core tests, workspace check and clippy
  green.

  What it emits: one `ProjectedItem` per staged occurrence in Set order, Set
  order as `woodshed:sequence` relations read straight off `Set::graph()`, and
  every catalog reason between each pair as its own `RoutedRelation`. Practice
  recency rides `ProjectedItem.channels` as `"heat"`, supplied by the caller
  because this crate owns no history.

  **Three findings, two of which changed the work.**

  First, **P4a and P4b were already done and the plan did not say so.**
  `CardId`, `SetGraphNode`, `SetGraphEdge`, `SetGraph`, and `Set::graph()` were
  all landed in `woodshedding::rehearsal`, and `woodshed-graph` already carried
  a full relation engine (`MaterialRelation`, 16 `RelationKind` variants
  including `VoiceLeadsTo`, `SharesTones`, `FitsInScale`, `PracticedAfter`,
  plus `relations_between` returning *every* reason for a pair). So the
  founding was genuinely adapter-only: no relation had to be derived, because
  the fan the contract wanted was already computed catalog-side. Whoever
  reads this plan next should treat P4a/P4b as landed and P4's remaining work
  as surface, not model.

  Second, **the source/instance separation carries the occurrence model for
  free.** A staged card is an *instance*; the material is the *source*. Staging
  one voicing four times interns one `SourceRef` and emits four items, which is
  exactly the "staging the same material twice yields two occurrences" rule
  P4a states, expressed in the contract rather than restated beside it. The
  consequence: `sceno` addresses items by index, so the `InstanceId` to
  `CardId` mapping rides on the snapshot beside the scene rather than inside
  it, which keeps the scene product-free.

  Third, and this one bounds the next slice: **the relations lifted here are
  formula-level, and so key-agnostic.** `woodshed-graph` relates formulas
  (`Major 7` extends `Major` whatever the tonic) and its own doc comment says
  the keyed relations this plan also wants — diatonic in, borrowed from,
  dominant of, resolves to, relative/parallel — are contextual and belong to a
  keyed-instance layer. Staged cards carry roots, so **the Stage projection is
  that keyed layer**, and deriving those relations is its own slice rather
  than something the adapter should have faked.

  Still open in P4: retiring `related_swatch`'s hand-rolled arrangement onto
  `scenomise::relax`, the keyed-instance relation layer, and the fretboard
  `Space` mapping (string, fret) to screen so picking resolves through
  `scenotime` rather than woodshed hit-testing.

- **2026-08-19, P4c scene canvas and headed receipt:** Woodshed now adapts the
  Stage scene into Cambium's existing graph canvas for both compact and
  expanded Set views. Epoch-qualified `InstanceId` and `RelationId` values
  reach native hit targets; parallel relations are independently routed and
  selectable; node motion is retained as view-local position state. The new
  `scenarios/p4c_scene_canvas.scn` stages Major and Major 7, proves one snapshot
  yields two nodes and several relation cells in both sizes, activates a real
  relation target, drags a real node through host pointer capture, and records
  four presented-frame PNGs. P4a, P4b, and P4c all return `RESULT ok` at
  1500x1200. Verified 74 core, 26 views, and 20 desktop-host tests.

  The first screenshots found two presentation defects. The Related mere tried
  to draw its whole induced graph inside a 300px swatch, and native relation
  buttons inherited visible control chrome. The swatch now discloses a bounded,
  weighted spanning neighborhood, summarizes parallel reasons in compact form,
  expands them into independent cells, and keeps every relation hit target
  transparent. A selected Stage relation also gets its own row so its caption
  cannot overdraw a node label.

- **2026-08-19, P4c relation inventory and drag fast path:** The expanded Set
  graph now lists every derivable relation with pair, kind, authority, weight,
  explanation, and a view-local Shown/Hidden choice. Individual withholding,
  Hide all, and Show all change the graph presentation without editing Set or
  scene truth; stable semantic relation keys survive dense scene epochs.

  Captured graph motion now skips persona work, audio/MIDI synchronization,
  serialization, and storage writes on pointer Down/Move, then runs the full
  host tail once on release. `scenarios/p4c_relation_inventory_fast_drag.scn`
  proves at least three view-only dispatches and exactly one full release sync,
  along with all/one/zero/all visible-relation states. Its first headed run
  exposed a target-publication race after Show all; a zero-position assertion
  now waits for and verifies the published graph before the drag. Seven
  consecutive 1500x1200 runs return `RESULT ok` with four presented-frame PNGs.
  Verified 75 core, 27 views, and 20 desktop-host tests.

- **2026-08-20, P4c presented-drag performance:** The frame-spaced
  `scenarios/p4c_drag_performance.scn` now delivers Down, eight Moves, and Up
  on separate presented frames and publishes app-authored phase timings in its
  sentinel. The first 27-frame debug receipt averaged 106,548 us, peaked at
  317,509 us, and found 54 redundant retained-root rebuilds in the frame hook.

  The drag path now skips unchanged viewport and static live-drive rebuilds,
  refreshes only the Set graph leaf, and uses Cambium's opt-in deferred Move
  rebuild: pointer capture holds the original native target while the custom
  leaf follows live view state, then Up reconciles targets and labels once.
  Woodshed also compiles the hot external layout, paint, and GPU submission
  packages at opt-level 2 in an otherwise ordinary debug profile.

  On the same requested 1500x1200 run, captured at 2464x1504 physical pixels,
  two warm receipts average 26,999 us and 24,999 us. The latter peaks at
  85,449 us; Woodshed's viewport, drive, and leaf phases total only 4,134 us
  across all 27 frames, and the frame hook performs zero retained-root
  rebuilds. That is an observed 4.3x average improvement, not a 60 fps claim:
  the remaining presentation spikes belong to the shared host/render path and
  stay visible as follow-up evidence rather than being hidden by an average.
  A later confirmation under 12 concurrent Cargo and 21 rustc processes varied
  from 39,421 us to 133,852 us average, so these debug-machine receipts establish
  the fast-path boundary and regression counters rather than a stable benchmark.

  Shared-host profiling then located the avoidable work. During pointer capture,
  `pointer_moved` still dispatched hover against the stale element underneath the
  dragged node. Several Move frames consequently applied six to eight attribute
  mutations and spent about 33 ms in incremental layout. Captured motion now
  keeps hover on the gesture target until release; a host input-routing test
  covers that contract.

  The final 27-frame headed receipt averages 13,690 us wall time and 13,158 us
  inside the shared host, down from the immediate pre-fix 17,911 us and 17,140 us
  receipts. Its only layout rebuild and 11 mutations occur at gesture setup.
  Subsequent Move frames apply zero mutations and present in roughly 11.3-13.0
  ms on the quiet debug machine. Retained rendering was already working: each
  moving frame dirties one tile, with zero tile invalidation or rebuild work on
  the retained-fragment path. The remaining steady CPU-side costs are chiefly
  Vello submission, scene emission, and accessibility synchronization. These
  measurements do not include GPU timestamps. The one-time Down frame still
  peaks near 59 ms and remains a separate optimization target.

- **2026-08-20, P4e selectable layouts and label-clear routing:** Cambium's
  graph canvas now places visible labels on the quieter axis derived from each
  node's incident relations. Horizontal lanes label above or below their nodes;
  vertical lanes label beside them. Each label occupies a bounded, clipped box
  inside the canvas, so long names cannot run through an edge or escape the
  viewport.

  Parallel endpoint relations now fan into pixel-stable 12 px lanes with a
  held interior segment instead of converging through one V-shaped midpoint.
  Relation identity, visibility, activation, and authored routes are unchanged.
  The generic Cambium geometry has focused tests for label placement and lane
  spacing.

  Woodshed exposes its existing ten-arrangement catalog directly on the
  expanded Set graph. The picker writes the durable Stage arrangement, releases
  manual positions pinned under the previous layout, and leaves Set order and
  relation truth alone. The graph subsection now fits its 520 px canvas rather
  than claiming the window width. `scenarios/p4e_stage_layouts.scn` proves the
  same two-node, four-relation graph in horizontal Snake and vertical Circle
  layouts, including the open ten-item picker and the corresponding label
  anchors. The final 1500x1200 headed run returns `RESULT ok` with three
  presented-frame PNGs.

  Pattern matching can recommend one of these deterministic arrangements
  later; it should not silently replace a user's chosen or pinned layout.

- **2026-08-20, P4d resizable canvas and Card-in-node expansion:** Cambium now
  provides a retained two-axis resize handle. The caller owns durable geometry;
  the handle owns only pointer capture and keyboard interaction, clamps through
  caller-supplied bounds, and emits live and final sizes. Its focused tests
  cover pointer motion, arrow/Home/End operation, and clamping. GraphCanvas also
  publishes a node region from the same viewport projection used by paint and
  native hit targets, with clamping that keeps a requested Card inside the
  canvas. A consumer can now declare that region as the node's rectangular
  footprint. Cambium clips incoming and outgoing relation routes to its
  perimeter in leaf pixels, then sends that same route to Sprigging paint and
  native relation targets. Compact graphs and each fanned route's interior stay
  unchanged.

  Woodshed stores the expanded Set canvas size in `AppSettings::stage` with
  360x240 and 960x720 bounds, a 520x260 default, and a reset on the Stage
  Settings page. The visible grip sits immediately beyond the canvas corner so
  the graph's deeper positioned layers cannot obscure its native hit target.
  The selected epoch-qualified occurrence expands into the existing editable
  Card inside its projected node region and collapses back to the same node.
  Nested overlay roots keep Card controls above graph geometry under the
  current retained stacking model.

  `scenarios/p4d_stage_node_cards.scn` proves a real captured resize from
  520x260 to 660x360, one selected Card region, retained selection identity,
  all four visible relations anchored at the Card perimeter, and collapse back
  to the compact node. The stable-host 1500x1200 run returns `RESULT ok` with
  four presented-frame PNGs. The P4e ten-layout receipt and the relation
  inventory receipt also remain green. The frame-spaced P4c performance receipt
  remains green with zero drag-time root rebuilds and one host layout rebuild;
  its 27 frames averaged 16,561 us and peaked at 76,513 us. The peak is the
  one-time layout setup, so this remains regression evidence rather than a
  stable frame-rate claim.

  **2026-08-21 current-stack receipt:** the isolated post-Stylo worktree ran
  the scenario against the active Buckram/Livery sources at 1500x1200 and
  returned `RESULT ok`. The four presented-frame PNGs in
  `C:\t\woodshed-p4d-final-20260821-0700` show the graph under the Card,
  the Card covering its assigned node region, preserved graph paint after the
  520x260 to 660x360 resize, and the compact graph after collapse. This closes
  the prior current-checkout paint and hit-test regression; it is not a claim
  about a separate retained-fragment compositor contract.


### Native Mere presentation and shared board adoption (2026-10-07)

**Status: landed; automated and committed-runtime native acceptance qualified.** Refreshed Woodshed main to
`06c2b13`, Mere main to `d041cc69b`, and Genet main to `965b64e20`. The primary
workspace now pins Mere `d041cc69b588b6f1dadd22308c2bc4059496cabd` together with
its coherent Genet dependency `d851a9db0cd1ff7837768250f21e9dff63455940`;
independently adopting newer Genet would create duplicate source identities.
Nested audio ports and the historical relationship-recipe proof keep their
independent pins. The current shared compiler takes explicit host card sizes;
Woodshed retains its existing 164 by 68 relationship footprint.

The session Mere embeds Pictograph's graph-free `PhysicsBoard` underneath
Cambium's native graph canvas. Woodshed discloses stable owner-qualified item
identities and analytic arrangement slots; the board supplies fixed-world
positions, CPU spring motion and permitted pointer/keyboard movement. Paint and
native targets consume the same positions. Reconciliation compares base slots,
roles and effective motion; simulated positions never feed back into base slots
on each frame. The host ticks only the visible Mere scene. Camera fit, pan,
zoom and arrangement restoration stay presentation actions.

The inspector separates foreground/background emphasis from shared arrangement
roles: free placement retains a drop, anchors return after release, and pins
refuse movement. Native buttons expose movement to keyboard activation as well
as pointer dragging. Motion starts disabled and reduced motion enabled;
reduced motion overrides the saved motion preference. Changing any of these
choices does not edit a Set, acquire catalog material or start a process.
Source opening still validates the current session snapshot before dispatch.
Circle, Tonnetz and captured relationship reading paths remain available.

A versioned host presentation payload retains camera, current item positions,
selection, foreground/background emphasis, arrangement roles and motion
preferences beside Woodshed's session. Structured IDs use tuple arrays rather
than JSON object keys. Payloads and entry counts are bounded; invalid data
resets presentation only, while unknown future versions remain opaque and
survive saving. Runtime positions become retained placements on save; dynamics
restarts from those placements rather than persisting solver internals.

The current stack review confirms G1, G7 and G9 landed in the dynamics grammar
plan, while G2 is still unmerged. Graphshell's canvas command and reader modules
remain web-gated. This adoption uses their portable lower board seam; it does
not claim a native Graphshell editor, arbitrary dynamics grammar editing,
shared scene import/export or layered rendering strata. Background currently
means explicit quiet presentation emphasis. These remain subsequent slices,
with shared extraction coordinated in Mere when a second native consumer needs
it. Shared references: `mere/design_docs/mere_docs/implementation_strategy/2026-10-02_dynamics_grammar_plan.md`
and `mere/ports/graphshell/src/canvas_controls.rs`.

Acceptance scenarios are `mere_presentation.scn` and
`mere_presentation_reopen.scn`. Evidence belongs under
`/Users/markik/Code/testing/woodshed/mere-20261007/`, using isolated unsealed
fixture sessions. Native wide/narrow capture review and fresh-process reopening
must be recorded before this slice is described as qualified.

Implementation candidate `364741ec2387064d60f5ecb3783ed05a59d9d597` passes the
final relevant gate: 579 checks (194 core, one backdrop integration, 64 desktop,
14 graph, 121 views, 181 theory, four doctests). Seventeen focused Mere tests
pass, including an integration regression for actual static displacement and
pin refusal. The adapter explicitly materializes a paused drop without stepping
neighboring physics: the shared board's kinematic position otherwise remains
unfolded until a tick. Locked desktop build passes; views Clippy completes with
four existing warnings. Changed-document links and diff whitespace pass.

The initial native launch was blocked by the locked Mac. After unlock, the
same committed implementation passes the isolated native matrix: 1280x900 seed
(1895 frames, three captures), 420x900 seed (1898 frames, three captures), wide
fresh-process reopen (1864 frames, seven captures), and narrow fresh-process
reopen (1870 frames, seven captures). All twenty final captures are reviewed.
The expanded reopen scenario is committed as `d50892c`; runtime manifests,
lockfile and crates remain unchanged from implementation `364741e` through that
acceptance revision. The source binary hash remains
`e806d89c4e8b83bf90141c5514391d4c9e29526d5f110695e83f807593e14649`.

Native scripted controls exercise camera changes, free placement, anchoring,
pinning, motion opt-in and reduced-motion override. Fresh processes retain the
camera, selected working-Set occurrence, background emphasis, pin and motion
preferences. The wide graph region is pixel-identical before/after anchored
return, changes after free placement, and stays identical after pinned movement;
the narrow graph also changes for free placement and stays identical for a pin.
Restore/Fit recovers full overview geometry from a deliberately cropped camera.
At 420px the page scrolls vertically, labels are abbreviated/culled, and full
names and placement explanations remain readable in the roster and inspector.

An automation observation inadvertently relaunched completed narrow test apps
and disturbed their profiles. Those attempts remain under `narrow-initial` and
`narrow-rerun`; the final `narrow` seed/reopen pair uses an isolated clean profile
and has one passing scenario per phase. Final evidence and image/scenario hashes
are in `/Users/markik/Code/testing/woodshed/mere-20261007/receipt.json`.
No product code fix was needed during native acceptance. This qualifies the
scripted native fixture flow at default UI zoom; personal encrypted vaults,
browser/other-platform behavior, high zoom, screen-reader walks and the physical
OS pointer-drag matrix are not established. Release packaging and delivery are
separate work.


### Shared ambient atmosphere beneath the session Mere (2026-10-07)

**Status: landed; automated and committed-runtime native capture/reopen qualification complete.**
The current Mere main review through `973a7fc17` finds no changes to the
native Cambium/canvas API since the primary `d041cc69b` pin. This slice uses
Pictograph's existing `AmbientSim` seam without a dependency repin. Orbits uses
64 seeded N-body particles; Cells uses a 32 by 24 seeded Game of Life grid.
Both are optional, clipped to the graph viewport, and painted beneath its
nodes and relationships. Their coordinates stay fixed to the viewport while
the graph camera pans and zooms. The foreground graph alone owns interaction
and accessibility targets.

The atmosphere selector offers Off, Orbits and Cells, plus New pattern.
Woodshed retains the selected kind and pattern seed as additive version 1
presentation fields. Reopening reconstructs the deterministic initial pattern;
it does not persist simulation internals. Legacy sessions default to Off,
and unknown future presentation payloads retain the existing opaque-preservation
behavior. Motion remains disabled by default, reduced motion freezes both
arrangement and atmosphere, and hidden Mere views do not advance simulations.
The pattern uses the same scene motion controls as the shared arrangement.

Pan and item movement controls now sit behind Show movement controls; Fit,
Restore and Zoom remain visible. The disclosure starts closed after reopening.
Atmosphere is contextual paint: it does not infer musical relationships,
modify Sets, start practice, or replace explicit foreground/background item
roles. Arbitrary authored strata, interactive backdrop composition, portable
scene recipes, and dynamics grammar editing remain open shared-stack work.

Acceptance scenarios are `mere_atmosphere.scn` and
`mere_atmosphere_reopen.scn`; the existing presentation scenarios expand the
folded movement controls before acting. Automated coverage includes exact
static paint/revision preservation, deterministic seeded reconstruction,
legacy and retained presentation, offscreen motion gating, unchanged owner
facts, viewport clipping/resizing, layer order, and graph-camera independence.
Evidence is recorded under
`/Users/markik/Code/testing/woodshed/atmosphere-20261007/`.

The October 7 projection-grammar handoff (`973a7fc17`) remains compatible
with this scope: portable anatomy gap proofs and field receipts wait for a
forcing consumer, and arbitrary dynamics grammar remains a separate owner
lane. The atmosphere adapter exercises an existing shared ambient API; it
adds no field-evidence claims or shared projection vocabulary. Reference:
`mere/design_docs/mere_docs/research/2026-10-07_projection_grammar_handoff.md`.

Native qualification uses runtime `019fdf3858bb461db79a292ddcb238c707e1e08a`,
whose implementation is unchanged through the scenario-only `aec3d4e` follow-up.
The wider gate passes 585 checks; 22 overview checks and the leaf-composition
check pass again after the visual refinement. The desktop builds successfully.
The selected atmosphere now has a visible indicator as well as its accessible
pressed state. The canonical reopen scenario allows twelve settling frames
after revealing the canvas.

Accepted runs at 1280 by 900 and 420 by 900 each produce four seed captures and
three fresh-process reopen captures: 14 PNGs reviewed. Wide seed/reopen use
1912/1870 frames; the repeated narrow fixture uses 1913/1899. Pixel comparisons
of the graph area are identical when motion is enabled under reduced motion,
and change once motion is allowed, at both widths. Both persisted fixture
sessions finish with Cells, seed 3, motion disabled and reduced motion enabled.
Owner graph inspection/background actions preserve the two staged Cards.
The 420-pixel controls wrap, and normal scrolling reveals the complete canvas;
compact graph labels retain the existing culling policy and full roster names.

The receipt, hashes, scenarios, logs and accepted capture manifest live in
`/Users/markik/Code/testing/woodshed/atmosphere-20261007/final/receipt.json`.
The first batched preview of a narrow reopen image appeared empty. Sequential
inspection of the saved PNG, independent pixel inspection, and a direct native
restore/scroll inspection all show an intact Cells graph. A repeated narrow
fixture is preserved; this observation does not demonstrate a runtime rendering
failure. Qualification uses isolated unsealed fixtures and covers the native
runtime and presentation session path. Release packaging/signing and a personal
encrypted-vault workflow remain separate gates.


### October 7 follow-up: authored musical context in the session Mere

**Status: landed; automated and committed-runtime native capture/reopen qualification complete.**

Keep nearby retains up to twelve keyed chords or scales as Woodshed session
assets. Each occurrence has its own stable identity, exact formula and tonic,
and the working Set it was explicitly kept alongside. Repeated Keep of the
same material for the same owner focuses the existing reference and preserves
its authored foreground/background choice. Different roots or Set owners stay
separate. New references start in the background; the existing presentation
controls govern emphasis, placement, camera and motion.

The overview now includes these references in its single session graph. Uses
catalog material links resolve their authority; Kept nearby for links describe
an authored association with the originating Set, without asserting derivation
or harmonic membership. Pairwise links report equal pitch classes, directed
containment, or shared pitch classes. The inspector lists exact sounding tones
and common-tone explanations. These facts do not infer harmonic function or
recommendation. Catalog formulas, kept occurrences and authored Card occurrences
remain distinct identities.

Keep is available for the selected Card and catalog focus in Mere, and for
focused chord/scale context in the Stage context inspector. Open in Stage,
Hear, Add to the named current Set, and Remove from nearby are explicit owner
actions. Open and Hear preserve Set instructions; Add creates a new occurrence
in the current working Set, preserving the originating owner. Remove deletes
the kept reference and its scene presentation without deleting catalog material
or Cards. The current slice deliberately excludes arpeggios, scale patterns and
chord approaches until their playable source provenance is retained explicitly.

The additive core session collection is separate from presentation JSON. Legacy
sessions default to an empty shelf. Imported structure is normalized to twelve
unique nonzero occurrence IDs and unique owner/material pairs, retaining the
first occurrences and preserving stale formulas. Allocation advances past all
imported IDs, including clipped entries. Unavailable references remain visible
and removable; Open/Hear/Add refuse them rather than substituting a current
catalog selection. Presentation restoration follows collection restoration, so
focus, roles and positions can resolve their retained asset identities.

The stack review refreshed Mere through `b10378404`: snapshot undo History has
landed for the Scenograph editor, and the dynamics grammar owner lanes continue.
The existing native graph and keyed-material APIs satisfy this shelf. Woodshed
keeps its coherent Mere `d041cc69b` / Genet `d851a9db` family; no repin or shared
contract change is required. Reusable authored scene recipes, a portable scene
editor, and musical syntax/phrase composition remain subsequent work.

Acceptance scenarios are `mere_musical_context.scn` and
`mere_musical_context_reopen.scn`. Qualification artifacts are recorded in
`/Users/markik/Code/testing/woodshed/musical-context-20261007/receipt.json`.

The gate passes 597 checks: core 201, core integration 1, desktop 65,
graph 14, views 131, theory 181 and theory doctests 4. The wider gate preceded
final core-only legacy/import regressions, which pass in the final core rerun;
views also pass after state normalization. The desktop builds successfully.
A separate source review finds no actionable owner-boundary regression.

Native qualification uses committed runtime `0c74407` at 1280 by 900 and
420 by 900, with four seed and two fresh-process reopen captures each:
twelve PNGs reviewed. Wide seed/reopen run 1912/1848 frames; narrow run
1915/1864. Keep and Hear preserve the initial two Cards. Open in Stage selects
C Major while preserving those Cards; explicit Add produces the third Card,
and removing a retained reference preserves all three. The final sessions keep
C Major and C Major 7, two originating Set associations, two background items,
one pinned reference, and Cells atmosphere. Reopening validates those identities,
containment and owner links, roles, and unchanged Card count.

The shelf description and buttons wrap without overlap at 420 pixels. Existing
compact graph labels use their culling policy; full material names remain in
the roster and inspector. The first narrow seed inspector capture revealed its
selected roster row rather than the inspector below it. Scenario-only commit
`37be94e` explicitly reveals the inspector before capture; the narrow fresh
reopen then shows the complete explanations and wrapped owner controls. This
requires no runtime change. The receipt distinguishes runtime and scenario
revision and preserves the exact scenario/log/capture hashes.

Qualification uses isolated unsealed fixture sessions. Personal encrypted-vault
operations, release packaging/signing, and browser-host qualification remain
separate gates.


### October 7 follow-up: captured arpeggio recipes in the Mere

**Status: landed; automated and wide/narrow native save/reopen qualification complete.**

The retained context shelf now distinguishes a catalog chord/scale reference
from a copied playable arpeggio instruction. Keep arpeggio recipe captures an
already authored chord Card with Arpeggiate touch. Its source working Set and
Card identity are historical provenance; the retained instruction has an
unassigned Card ID, and each explicit Add receives a new occurrence ID.
Different directions, inversions, shapes, marks or timing remain different
kept recipes. Keeping an exact instruction again preserves its authored scene
emphasis. The twelve-item collection bound includes recipes and references.

Capture resolves inherited instrument/tuning, neck window and tempo at the
moment of keeping. It preserves material, direction, inversion, selected shape
fingerprint/profile, capo, marks, hold and recipe stamp. Hear and Add validate
and resolve that saved instruction directly, without consulting the current
catalog focus or transient arpeggio discovery source. Original Card edits or
removal do not rewrite the captured copy. Invalid catalog, setup or shape
state refuses playback and addition while keeping the payload removable.

Explore chord formula is explicitly a separate navigation action. It selects
the formula in Stage without claiming to project or edit the saved playable
recipe. The Mere inspector shows the saved instruction and its historical
source. Recipe items participate in catalog-authority and originating-Set
relations, but are excluded from formula pitch-class comparison edges. A
capo-shifted shape or marked subset must not be presented as the ordinary
formula's complete sounding set. Scale patterns, chord approaches and keeping
unstaged discovery previews remain later consumers of this captured-instruction
contract.

The current Mere remote review through `356a832cf` confirms dynamics grammar
G3 (combinators and currencies) has landed, including weighted composition,
groups and schedules in Seiche/Pictograph. G2 channel/Meaning work remains
unmerged and G4 persistence proceeds alongside it. This domain slice uses
existing Card playback and host-owned session assets; it does not adopt the
portable DynamicsSpec or projection editor. Woodshed's coherent pinned family
remains unchanged. Those shared capabilities require their own scene adoption
and native qualification rather than an incidental dependency update.

Scenarios are `mere_arpeggio_recipe.scn` and
`mere_arpeggio_recipe_reopen.scn`; evidence belongs under
`/Users/markik/Code/testing/woodshed/arpeggio-recipe-20261007/`.


The wider gate passes 607 checks (core 207, integration 1, desktop 65, graph 14,
views 135, theory 181 and doctests 4). Capture tests cover inherited setup/tempo,
exact selected shape and fingerprint replay, marks, stale/malformed refusal,
instruction deduplication and save/reopen. View tests prove historical source
removal, unchanged live Stage and transient discovery selection during Hear/Add,
new active-owner Card identities and failure without fallback.

Formula chord marks previously read live-board tuning/window even when a Card
specified its own setup. The chord-only dot resolver now honors that saved setup,
which is necessary for immutable copied-recipe replay. The background-rehearsal
fixture now clears both instrument and tuning to express actual inheritance:
an explicit Guitar with absent tuning selects Guitar's default, while both
absent identities inherit Stage. The focused test and wider gate pass with the
background owner's captured sound preserved across foreground changes.


Native qualification uses committed runtime `e0c24ff2b847d8ac25da478a84bc9816ea4ade85`
at 1280 by 900 and 420 by 900. Four runs pass: wide and narrow seed each
1888 frames, and each fresh-process reopen 1864 frames. Eight PNGs were
visually reviewed, including the complete compact inspector and saved Orbits
scene. Saved instructions show Guitar / Standard, capo 0, frets 2–8, Down,
inversion 1, shape 1, no marked notes and 80 BPM. The recipe remains background
and pinned after reopening. The compact graph abbreviates labels under its
existing culling policy; the roster and inspector retain full names.

Each seed removes historical source Card 3 and changes the live Stage/tempo
before Hear and Add. Hear preserves the two remaining Cards; Add creates a
third Card from the saved recipe. Formula exploration preserves that Set. Each
fresh process restores those three Cards with source Card 3 absent, replays
the saved recipe, and explicitly adds a fourth occurrence. Persisted sessions
retain one copied instruction with unassigned ID, its originating Set link,
80 BPM and selected shape fingerprint.

The exact binary, scenario, log, capture and final-session hashes are recorded
in `/Users/markik/Code/testing/woodshed/arpeggio-recipe-20261007/receipt.json`.
External launch scenarios add a startup settle only; canonical assertions are
unchanged. Qualification uses isolated unsealed sessions and verifies preview
dispatch rather than acoustic quality. Personal encrypted-vault workflows,
release packaging/signing and browser execution remain separate gates.


### October 7 follow-up: captured scale-pattern recipes in the Mere

**Status: implemented; automated qualification complete, native qualification pending.**

The copied playable-instruction shelf extends from chord arpeggios to authored
scale patterns in thirds and fourths. Capture resolves inherited setup, fret
window and tempo, preserves pattern, touch, marks, hold and recipe stamp, and
keeps the authored source Card as historical provenance. The instruction's
Card ID remains unassigned; explicit Add creates a new occurrence in the active
Set. Exact instruction deduplication and the shared twelve-item bound remain.
The existing arpeggio wire payload stays compatible through the generalized
`CapturedRecipe` type and its `CapturedArpeggio` API alias.

Patterns resolve deterministic scale contacts in degree-pair order, not a
chord-shape inventory. Stale chord-shape selectors on pattern payloads refuse
owner actions. Saved setup, out-of-window marks and missing formulas are
validated before Hear or Add, with no live Stage fallback. Original Card
removal or discovery-source changes leave the retained instructions intact.
Explore scale formula navigates separately to the base scale. Recipes retain
catalog and originating-Set relations, without formula-level pitch comparisons
that would misrepresent their realized note sequence.

Scenarios are `mere_scale_pattern_recipe.scn` and
`mere_scale_pattern_recipe_reopen.scn`. Revision-specific evidence belongs in
`/Users/markik/Code/testing/woodshed/scale-pattern-recipe-20261007/`.


Fresh stack audit: Mere main `cd3ebf26dff71ed36a296503813bbceda577142e`
and Genet main `161b1a8984553f4c26f8931ea0490105abccba11`. G3 dynamics
composition and editor undo/session persistence are landed. C1 adds Canvas
empty-left-drag panning, right-drag selection and host-owned context requests,
plus Cambium command sets and Pandect command menus. Woodshed currently uses
Cambium graph_canvas and graph-free PhysicsBoard rather than Canvas pointer
routing, so C1 introduces no direct gesture change in this slice. G2 semantic
channels/Meaning and G4 portable DynamicsSpec remain open; E3/E4 wait for the
S1 host dataset envelope. No private grouping or dynamics format is added here.

Mere main's manifest still uniformly pins Genet `965b64e206a`, while Genet main
has newer Streams/Fetch/TextDecoder work. Both latest hashes must not be
substituted independently as an assumed coherent family. This domain slice
retains Woodshed's qualified Mere `d041cc69` / Genet `d851a9db` pins. A coordinated
scene adoption should qualify the complete dependency family, legacy persisted
presentation, host gestures and native reopening together. The source audit
is distinct from downstream adoption or native acceptance of those new APIs.


The wider gate passes 613 checks: core 210, integration 1, desktop 65,
graph 14, views 138, theory 181 and doctests 4. The desktop builds successfully.
New core/view checks cover thirds and fourths, ordered/repeated visits, Solo/Mute
contacts, inherited Drop-D/window/tempo with capo, serialized reopening, source
removal, active-owner Add identities, legacy arpeggio compatibility and refused
stale payloads that remain removable. Final source review found no actionable
owner-boundary regression. Native qualification follows the committed runtime.
