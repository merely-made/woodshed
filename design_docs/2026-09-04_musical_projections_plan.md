# Musical Projections Plan

**Status (2026-09-06): Circle-of-Fifths context and triadic Tonnetz landed. S1-S4
remain planned except where explicitly recorded below.** Bounded comparison exports and
browser consumer proofs exist, as recorded below. The musical subsystem review
and isolated enumeration probe below are research, not implementation of these
slices. Luna/Terra are the requested implementation agents; each musical slice
still needs its reviewed done-conditions before opening.

Child of [2026-07-11_stage_set_tools_plan.md](2026-07-11_stage_set_tools_plan.md):
this is P4e item 6 (voice leading placed by motion cost) and the reasons half
of P4f (an edge explains itself in musical terms), plus the evidence layer the
Stage plan names but never wires. It does not reopen P4a-P4d and changes no
Set truth.

## Why

The Stage projection has ten arrangements and typed relations, and still does
not answer the questions a player brings to it. Choosing Circle over Snake
moves cards; it does not say which tones two chords keep, which fingers move
between them, or what has gone unpracticed. The grammar's next feature should
be chosen by those questions, not by another placement rule. Four questions,
and what the code has for each today:

| Question | State on 2026-09-04 |
|---|---|
| Which tones stay? | Not answerable. `woodshed-graph` relations are formula-level and key-agnostic by design: C Major and A Minor share two tones and the graph relates them by nothing. `stage_scene.rs` names keyed relations as its own undone slice. |
| Which fingers move? | Not answerable. `Setting::voicing_idx` is stored and never read. `card_voicing` in `woodshed-core/src/lib.rs` applies the formula at MIDI 48; no Card resolves to a `ChordVoicing`. |
| What connects these? | Landed (P4b). Typed parallel relations, fanned, with reasons on selection. |
| What have I neglected? | Half. `PracticeHistory` has time, last-seen, counts, and transitions. `StageSceneOptions::recency` exists and is emitted on the `heat` channel, but `set_graph_snapshot` passes defaults, so the scene never sees history. There is no struggle signal (tempo shortfall, restarts). |

## Boundaries

- **Keyed math lives in `woodshedding`** (decided 2026-09-04). Pure
  pitch-class set operations on concrete material: no I/O, no scene types.
  `woodshed-graph` stays formula-level; it is the catalog's relation family
  and its tests assert that. `stage_scene` derives occurrence relations from
  the new `woodshedding` API and emits them as its own relation kinds.
- **Arrangement stays product policy** until a second product proves the
  same semantics (the rule `arrangement.rs` already states). A voice-leading
  arrangement is added to `GraphArrangement`, not contributed to `scenomise`
  in this plan. Its input grows from `(count, edges, focus)` to carry per-edge
  motion cost; that is the one change to the arrangement seam.
- **The view explains, the scene carries.** Kept and moved tones ride the
  relation record and reach the UI through `StageRelationDetail`, the same
  path P4b built. No product vocabulary enters the `sceno` scene.
- **No struggle inference.** Slice 3 projects facts history already holds.
  A struggle signal is P5/P6 work, when Rehearsal owns a clock and can
  measure a miss.

## Slices

### Context around the Set (2026-09-06)

The user's current direction makes comparison an emphasis over a wider musical
graph. Shared tones, differing tones, and nearby or more distant material remain
available together. Background material is real catalog-derived material with a
stable identity; a repeated staged Card remains a separate occurrence.

Confirmed interaction decisions:

- Clicking background material focuses it and reveals its connections. Audition
  and **Add to Set** are separate, explicit actions.
- Focusing preserves existing positions and expands nearby. View-local focus,
  retained positions, and exploration never silently author Set membership.
- The arrangement determines the relevant background and its placement. A
  universal recommendation halo followed by a different geometric layout does
  not meet this requirement. Existing Grid/Snake/Circle names describe geometry;
  they do not establish musical meaning such as a circle of fifths.
- Relations between background nodes must be available, so the graph provides
  connected routes for exploration beyond the selected cards' intersection.
- Context visibility and breadth are user preferences. Dense remote detail may
  be reduced visually without misrepresenting membership or musical facts.

First-slice done-conditions: one explicitly musical arrangement defines a
bounded, explainable context; catalog candidates and repeated staged occurrences
are distinguishable through the scene and accessible UI; focus preserves the
Set and existing placement; audition and Add to Set act on the focused material;
and comparison reports shared and differing tones without reducing graph
membership to those tones. The wider arrangement catalog, fingering motion,
practice inference, and generative recommendations retain their separate gates.

**Implementation (2026-09-06):** `stage_scene.rs` maps each instance to either a
stable Card occurrence or a keyed catalog reference.
`StageState::neighborhood_snapshot` supplies a separate center-to-neighbor graph.
The existing ten `GraphArrangement` variants operate on count/edges/focus, without
keyed musical coordinates or a context query. Implementation was divided between
Terra (core identity/context/projection) and Luna (focus/actions/presentation),
with integration and validation owned by the parent task.

**First arrangement:** Circle of fifths is the first implemented musical
reading. The existing geometric layouts remain under Set. Major chords provide
landmarks around all twelve tonic positions; relative minor chords and keyed
scales occupy separate rings. The normal opening shows up to 16 context items,
focus expands to the user's 24-item default, and Show more can expand to the
36-item ceiling. Lower limits remain available in Stage settings. Existing
positions survive expansion; an explicit new reading releases old view pins.

The keyed identity and catalog resolution live in `woodshed-core::harmony`;
exact shared/exclusive pitch-set arithmetic lives in
`woodshedding::pitch_class_set`. This is the set-comparison foundation for S1,
not a voice assignment, ergonomic movement estimate, or completed S1-S4 feature.
Repeated Set occurrences retain Card IDs while context names a formula plus
tonic. Keyed and formula-level relations keep separate slugs.

**Presentation:** fixed tonic slots and separate rings preserve spatial bearings.
The graph uses 20-pixel hit targets, a square opening canvas, and a focus/action
panel beside the graph where space permits. A quiet major-chord fifth chain
remains visible; focusing material reveals its incident relations. Other derived
background relations become visible when their material is focused. The current
comparison covers the selected Set card and one focused context item; arbitrary
multi-card comparison remains open. Pitch-class labels currently use sharps.

Audition queues resolved pitches, so changing the board before the host drains
the request cannot change what is heard. Focus, positions, and retained context
are view state. Arrangement, context visibility, and breadth are saved settings.
At the context ceiling, **Explore from here** explicitly refreshes the disclosed
neighborhood around focus; fixed musical coordinates still preserve bearings.
Reducing breadth preserves the focused material. Compact labels stay adjacent
to their tonic slots instead of moving with the disclosed edges.

**Validation (2026-09-06):** 332 tests passed across `woodshedding` (169),
`woodshed-core` (89 plus 10 example tests), `woodshed-graph` (14), and
`woodshed-views` (50). The desktop build passed. Both commands ran from `C:/t`
with the Woodshed manifest, `--locked --offline`, and target directory
`C:/t/woodshed-context-target`, bypassing the ignored local Cargo overrides that
reference a missing worktree. Existing unused-import/dead-code warnings remain.

`scenarios/p4e_musical_context.scn` passed through actual pointer targets:
choose the arrangement, focus A minor without changing Set membership/cursor,
inspect C/E shared and G/A exclusive tones, explicitly Add, and hide context.
The final rendered receipt is
`Code/testing/woodshed/musical-context-20260906/run06/`; `receipt.json` in its
parent records the source commit and artifact hashes. Unit regressions cover
immutable audition pitches, stable expansion/hiding, stale callbacks, focused
material at reduced breadth, the complete major-key fifth chain through staged
C, and duplicate-free shared-tone routes. Physical audio output was not checked
in this graph receipt. Denser label/edge treatment, key-aware enharmonic spelling,
other musical arrangements, and multi-card comparison remain follow-up work.

### Tonnetz (2026-09-06)

Second musical reading: major/minor triads occupy triangles whose vertices name
their pitch classes. P/L/R transformations retain two tones, with separate slugs
and explanations for Parallel, Relative, and Leading-tone exchange. The same
keyed identities and Set occurrences pass through the shared context request;
the reading selects its candidate query, coordinates, and highlighted edges.

The bounded patch contains each of the 24 triads once. Lattice vertices repeat
pitch classes at boundaries; transformation links crossing those boundaries are
explicitly explained as wraps. The pitch generators are fifths and major thirds,
with C major placed inside the patch so its three direct transformations share
literal visible edges. The implementation derives the pitch arithmetic rather
than importing a third-party visualization. Musical reference:
[P/L/R and Tonnetz explanation](https://tonnetz.liamrosenfeld.com/explain-music).

Done conditions: all 24 triangle pitch sets match catalog material; each P/L/R
operation is reversible and preserves two tones; breadth and prior disclosure
do not block discovery; focus preserves Set membership and existing positions;
the actual desktop displays triangles, notes, and inspectable chord targets;
comparison, Hear, Add, and context hiding work through the existing actions.
The fixed lattice supports pan/zoom; dragging an individual chord cannot detach
it from its pitches. Unsupported focus currently opens the C-major neighborhood;
scales and extended chords retain their fallback placement without triad cells.
Register-sensitive voice leading, ergonomic cost, seventh
chord extensions, and harmonic syntax remain separate work.

The native Sprigging leaf paints lattice strokes with the same viewport projection
as the graph; retained labels name pitch vertices and compact chord centers.
CSS-rotated line boxes failed the rendered gate and were replaced. Pitch labels
share the graph's retained label layer after a separate overlay failed to paint.

**Validation (2026-09-06):** 341 core, graph, views, woodshedding, and example
tests passed. Final view changes passed all 51 view tests; the strengthened
Tonnetz suite passed eight focused core tests, including all 24 triads' reversible
P/L/R operations and shared-tone counts. The desktop build passed with the
existing unused-import/dead-code warnings. `scenarios/p4e_tonnetz.scn` passed
actual pointer targets, C-major/E-minor comparison, explicit Add, and context
hiding. Presented captures in `Code/testing/woodshed/tonnetz-20260906/run03/`
confirm closed triangle cells, vertex pitches, compact chord labels, and focused
emphasis. `receipt.json` in its parent records the source commit and hashes.
Physical audio output was not checked. Enharmonic spelling remains sharp-based;
viewport framing and repeated-occurrence label density can be refined further.

### S1. Keyed occurrence relations

`woodshedding` gains a `harmony` module (name open; `pitch_class_set` if
`harmony` reads as a product word) with:

- `PitchClassSet`: a set of 0..12 built from `(root: PitchClass, intervals)`,
  so a scale or chord formula at a root becomes a concrete set. Reuse the
  transposition `Interval` already provides.
- `shared(a, b) -> PitchClassSet`, `moved(a, b) -> Vec<(from, to)>` as a
  proposed minimum-motion assignment of the differing tones, and a separately
  named assignment cost if that metric is adopted. The existing catalog
  `voice_leading_distance` is symmetric mean-nearest distance and is not an
  assignment. Preserve its behavior; shared circular-distance arithmetic does
  not make the two measures equivalent. Cardinality and unmatched-tone policy
  must be explicit before the assignment API is implemented.

`stage_scene` gains, for every pair of staged occurrences whose materials
resolve to a keyed set (Scale, Chord; Riff and Path do not):

- `woodshed:keeps-tones` with the kept tones on the record and weight from
  the kept count.
- `woodshed:moves-to` with the moved pairs and the motion cost, emitted only
  under a cost ceiling so the fan stays legible (the formula layer uses 1.5;
  keep that unless a fixture argues otherwise).
- `woodshed:diatonic-in` from a Chord occurrence to a Scale occurrence when the
  chord's set is a subset of the scale's set. This is `FitsInScale` made
  keyed: Am fits C Major, and does not fit E Major.

These are `RelationAuthority::Computed`, filterable through
`relation_kinds` like the rest, and explained through `relation_detail` with
tone names spelled from the scale's own root where one is present.

Done when: `Set` holding C Major (chord) and A Minor (chord) yields a
`keeps-tones` relation naming C and E and a `moves-to` naming G to A; the
same two formulas at roots a tritone apart yield no `keeps-tones`; a
`moves-to` result depends on the selected metric and threshold (the existing
mean-nearest rule admits C Major/F-sharp Minor); adding a C Major
scale card yields `diatonic-in` for both chords; the existing formula-level
relations still appear alongside, unchanged; `woodshed-graph` tests pass
against the moved distance function; and the Set tray's relation inventory
shows the new kinds with their tone names.

### S2. The voice-leading arrangement

`GraphArrangement::VoiceLeading`, placed by motion cost rather than order:

- Input: the pairwise `moves-to` costs from S1 (absent pairs are far).
- Placement: focused card at the origin; every other keyed card at a distance
  proportional to its motion cost from the focus, angle from a stable
  deterministic rule (Set order is fine). Unkeyed cards (Riff, Path) sit on
  an outer ring in Set order. Deterministic, no solver, same contract as the
  other ten.
- The relation filter defaults to `keeps-tones` and `moves-to` visible and the
  rest hidden when this arrangement is chosen, so the edges on screen are the
  ones the placement is about. The default is a preference, applied once on
  switch, not a lock.
- Edge label in the graph canvas: kept tones as a short run ("C E"), moved
  tones as "G→A". The detail panel keeps the full sentence.

Done when: selecting a different focus re-places the graph with that card at
the origin and nearer cards are lower-cost by the S1 number; the arrangement
persists as a setting like the other ten; a headed scenario under
`scenarios/` drives focus change and asserts distances through the snapshot;
and the receipt lands under `testing/woodshed/scenarios/`.

### S3. Practice evidence in the projection

Wire what exists:

- `set_graph_snapshot` fills `StageSceneOptions::recency` from
  `PracticeHistory::last_seen_ms` normalised over the Set, so the `heat`
  channel carries real values. The canvas already reads it.
- Two view filters over occurrences, applied as visibility and never as Set
  edits: **least practiced** (lowest `total_practiced_ms`, bottom N or a
  fraction) and **longest ago** (oldest `last_seen_ms`, including never).
- `PracticedBefore`/`PracticedAfter` from `PracticeHistory::transitions`
  join the projection as `Evidence` relations between occurrences whose
  catalog ids match a recorded transition. The kinds already exist in
  `woodshed-graph`; this emits them.

Done when: two cards with different recorded practice show different `heat`
in the snapshot; a never-practiced card survives the longest-ago filter and a
well-practiced one does not; a recorded transition appears as an evidence
edge with its traversal count in the detail; and the filters persist with the
other Stage settings.

### S4. Which fingers move

Prerequisite first, because nothing resolves a Card to a voicing today:

- `woodshed-core` resolves a Chord Card to a `ChordVoicing` through
  `Fretboard::find_chord_voicings` under the Card's tuning, capo, and fret
  window, selecting by `voicing_idx` and falling back to the first. This is
  the read side of a field that has only ever been written. `card_voicing`
  audition switches to the resolved voicing's pitches so the ear and the
  board agree; scales and paths are unchanged.

Then the diff:

- `woodshedding`: `voicing_movement(a, b) -> per-string move` where each
  string reports held, moved by N frets, added, or dropped. Fret span and
  lowest-fret change ride the summary.
- `stage_scene`: a `woodshed:hand-moves` relation between Chord occurrences
  that both resolve, weight inverse to total fret distance, the per-string
  detail on the record.
- Detail panel shows the per-string movement; the graph edge shows the total.

Done when: two staged voicings of the same chord differ by hand movement
and not by tones, and the projection says so (a `keeps-tones` of everything,
a `hand-moves` with a non-zero total); changing a Card's voicing index
changes the audition and the `hand-moves` number; and a Card with no
resolvable voicing produces no `hand-moves` and no error.

## Review before implementation (2026-09-04)

The Graphshell authoring proof now exercises a real Woodshed Set/catalog export,
editable placement and labels, exact occurrence selection across spatial/list
views, and browser-local recipe reopening. This supports the product direction
here; it does not implement S1-S4. Luna and Terra are the currently requested
subagent models. A Terra review against the current code found four items to
resolve before their affected slices open:

- **S1 needs an explicit metric.** `woodshed-graph/src/lib.rs` currently computes
  symmetric mean nearest-neighbour distance, which permits reuse of a target
  tone. That is different from a minimum one-to-one assignment. Define unmatched
  tones for different cardinalities and test C versus Cmaj7. The tritone
  done-condition above also needs correction: C Major versus F-sharp Minor has
  no shared tones, but its current mean-nearest distance is 4/3, below 1.5, so a
  `moves-to` edge is expected under that rule. Preserve the catalog metric or
  explicitly distinguish a new assignment metric; sharing a distance helper
  alone does not make their results equal.
- **S2 needs costs independent of visible edges.** A complete per-focus keyed
  cost table must drive radius, with an explicit unavailable value for unresolved
  material. Threshold-filtered `moves-to` edges cannot supply every distance.
  Specify radius scaling and overlap handling for zero-cost repeated cards, so
  changing an edge filter cannot move a card or make it unreachable.
- **S3 is catalog aggregate evidence.** `PracticeHistory` stores catalog-keyed
  subjects and transitions; production callers use `catalog_id_for_card`.
  Repeated occurrences share that history. Say so in heat and relation details,
  and avoid implying that every occurrence pair was traversed. Distinguish never
  practiced from an old timestamp. Exact occurrence evidence requires event-time
  Set/CardId provenance that current history does not contain.
- **S4 first needs one voicing resolver.** Define card tuning fallback, capo and
  concert-pitch semantics, fret limits, bass/inversion policy, and out-of-range
  index behavior. Both audition and the visual comparison should consume that
  resolver. Its proof should use two actually enumerated voicings on the same
  resolved board. Current `dots_for_card` also defers per-card tuning/capo, so
  changing only `card_voicing` would leave another visible disagreement.

## Order and sign-off

### Musical subsystem research (2026-09-06)

The maintainer asked to research musical meaning, inference, analysis,
generation, and comparison, including comfortable routes into the next bar or
line. This section records the current seams and research recommendations.
Comfort models and analysis runtimes remain choices for the next implementation slice.

#### What the current subsystems actually do

| Subsystem | Current owner and behavior | Missing contract |
| --- | --- | --- |
| Musical meaning | `woodshedding::{pitch,interval,chord,scale,progression,rehearsal}` preserves spelling, formulas, keyed progression roles, and Card material/setting/touch/timing. `woodshed-graph` derives formula-level relations. | One Card realization shared by display, audition, and comparison; contextual harmonic readings distinct from exact tone membership. |
| Inference and recommendation | `woodshed-core::related_material` resolves catalog neighbors; `related_material_with_history` promotes prior staged transitions. `RelatedSettings` filters/diversifies them. | Alternative interpretations with explicit context; separate interest, practice, performance, and player preference evidence. Existing rank scores are not probabilities. |
| Audio analysis | `woodshed-audio::input` has configurable monophonic pitch analysis; onset/tap-tempo lives beside it. The offline benchmark has normalized note events and a scorer. | A measured transcription adapter, observation-run provenance, alignment to expected events, and uncertainty-preserving catalog resolution. See the audio-material analysis plan. |
| Generation | `woodshedding` enumerates voicings, applies progression roles in a key, and generates named exercises; core generates arpeggio shapes. | Bounded candidate search across a phrase, constraints and objectives supplied by the player, stable candidate identity, and explicit acceptance into Set. |
| Comparison | Formula relations carry independent reasons/measurements. Stage keeps occurrence identity; the comparison export handles bounded C/Am membership. | Distinct tone, sounding-voice, hand-position, fingering, rhythm, and performance comparisons, each disclosing its input and units. |

The intelligence-stack names in the July analysis plan have moved: Vates and
Sibylla are compatibility re-exports of Mere's `esp::infer` and `esp::embed`.
Neither is a music-analysis implementation. Structured note observations and
music-specific interpretation stay in Woodshed's analysis/resolution contract;
ESP remains replaceable inference/embedding execution plumbing.

#### Five comparisons, five questions

1. **Tone membership:** which pitch classes are shared, added, or removed?
   Set arithmetic answers this exactly. Keep spelling/context for display;
   enharmonic equality does not erase the intended name.
2. **Sounding voices:** which actual pitches move, remain, enter, or leave?
   Preserve octave, repeated voices, bass, and optionally voice identity.
   Pitch-class sets cannot distinguish a doubled root or an octave displacement.
3. **Neck positions:** which strings/frets change? Current `ChordVoicing` can
   supply this geometric comparison, including mute/play changes. It cannot
   establish that a particular finger stays planted.
4. **Fingerings:** which finger contacts, barres, releases, stretches, and hand
   shifts occur? `ChordVoicing` has no finger assignments. Exercises carry a
   suggested `finger`, but that is not a general chord-contact model.
5. **Performance:** where did recorded events differ from intended events?
   This needs alignment, tempo/latency context, and confidence before naming a
   missed note or late attack. Sensor silence is not proof of player failure.

Harmonic function is another interpretation over a passage. “Shares C and E”
is an exact fact; “prepares a resolution” depends on key, phrase, and stylistic
context. Preserve multiple readings when the evidence admits them. Research
on harmonic analysis explicitly identifies segmentation, key, and treatment
of non-harmonic notes as interdependent sources of ambiguity:
[Micchi et al., 2020](https://transactions.ismir.net/articles/10.5334/tismir.45).

#### Comfortable next position, and lookahead over a line

Treat the next bar/line as a bounded sequence of events with candidate
realizations. A bar boundary alone is too coarse when notes sustain through a
change or the last note constrains the next hand position.

Separate **constraints** (required tones, bass/inversion, sustained contacts,
available instrument, pinned fingering, allowed span) from **preferences**
(position shifts, stretch, barres, open strings, timbre, texture, and practice
target). A failed constraint returns no compatible candidate with a reason;
it must not silently omit a chord tone. Preferences produce a cost breakdown,
not a universal comfort percentage.

For a first deterministic search, retain bounded candidate layers and use
dynamic programming over static shape costs plus transition costs. The result
is optimal only for those candidates, weights, and modeled state. A bounded
beam or candidate cap must be disclosed if introduced. Phrase lookahead and
locks should be settings. Costs which depend on held fingers or prior motion
need that history in the state; a pairwise fret difference alone cannot carry
it. The literature supplies direct precedent for separate static/transition
features, whole-sequence search, and learning preferences from examples:
[Radisavljevic and Driessen, 2004](https://www.mistic.ece.uvic.ca/publications/2004_icmc_pdl.pdf).
An earlier single-note study also uses string/fret/finger states, but targets
robots, so it is algorithm evidence rather than validation of human comfort:
[Itoh and Hayashida, 2004](https://doi.org/10.1541/ieejeiss.124.1396).

First product choices could be **least movement**, **keep this position**, and
**practice a shift**, with two or three alternatives and a reason beside each.
“Keep this finger planted” is available only after finger assignment exists.
Player feedback should compare concrete alternatives under the same context;
choosing a harder exercise is not evidence that the player finds it easier.
Measured fret distance in millimeters additionally needs scale length and
instrument geometry. A numeric fret delta alone is not physical distance.

#### Live enumeration probe and corrections

The isolated research workspace is
`Code/testing/woodshed/musical-meaning-20260906/`. Its `Cargo.lock`, source and
`receipt.json` reproduce enumeration through the real `woodshedding` path
dependency at Woodshed `42920669145cc5c64ef8a4ff086e403526a1abc4`.
`fretboard.rs` SHA-256:
`12e96e356bc56ec061dcc7d20478c65c1006eae475d304374b00e7171165dadb`.

- Standard guitar, 12 frets, windows starting 0 through 9 with span 3,
  root-position C-Am-F-G, deduplicated without candidate truncation:
  **152/145/44/150** candidates. The starting C is `x32010`.
- The illustrative transition sum is absolute fret change on each continuing
  string, 2 for a mute/play change, 0 for two muted states. It has no finger,
  duration, texture, or timbre model. Greedy costs **19**; dynamic programming
  costs **16**. The latter moves to three-string voicings. This is a search
  demonstration and an example of an incomplete objective, not a comfort win.
- Constraining every chord to five played strings yields **52/46/11/52**
  candidates. Greedy and lookahead both cost **20** for this phrase. Lookahead
  does not guarantee a strict improvement on every passage.
- The current mean-nearest pitch-class calculation gives **4/3** for C Major
  versus F-sharp Minor and **0.125** for C versus Cmaj7. Neither number is total
  voice displacement; the former falls below the plan's old 1.5 threshold.
- **Re-entrant bass defect:** high-G ukulele G Major candidate `0,2,x,2`
  sounds MIDI `67,62,71` (G4,D4,B4). Root-bass enumeration accepts it because
  `validate_voicing` takes the first played string, though D4 is the actual
  bass. Resolve bass by sounding pitch before advertising tuning-general
  inversion or comfortable-route recommendations. No product fix was made in
  this research pass.
- **Practice evidence mismatch:** `EngagementKind::is_practice` excludes only
  `Previewed`, despite its comment distinguishing staging from practice.
  `related_transition_count` uses this predicate, and the UI emits
  `PracticedAfter` with “Previously staged from here.” Preserve existing raw
  engagements; classify interest/practice/performance explicitly before
  deriving mastery, neglect, or difficulty scores.
- `find_chord_voicings_for_bass` enumerates the Cartesian product, requires all
  chord pitch classes and at least three played strings, and has no finger or
  barre feasibility pass. Arbitrary tuning support does not establish arbitrary
  instrument technique or scalable candidate generation.

The four existing Python benchmark tests pass in this session. No audio model,
physical fingering evaluation, human preference fit, or full Woodshed host test
was run. Synthetic note fixtures validate scoring, not transcription quality.

#### Recommended implementation order and done-conditions

1. **Realization and evidence corrections:** one resolver takes effective
   tuning, capo/concert-pitch convention, fret window, bass policy, and selected
   voicing. Audition and board agree on the same resolved pitches. Test the
   high-G counterexample, custom tuning, invalid selection, repeated Card
   identities, and staging versus completed-practice classification.
2. **Exact comparison:** ship keyed membership and explicit added/removed
   tones; keep the catalog's current metric named separately. Snapshot and
   audition agree, unknown material stays unknown, and graph filters never
   alter the candidate distance table or Set truth.
3. **Position-route generator:** bound a whole phrase, retain locked choices,
   expose texture and movement preferences, show component costs and alternate
   paths. A fixture must distinguish greedy from global search, another must
   tie them, and both must preserve declared musical constraints. Label results
   as position suggestions until finger feasibility is modeled and checked.
4. **Analysis and comparison:** follow R2-R4 in the audio-material analysis
   plan with a generated corpus plus independent held-out human recordings.
   Report transcription, alignment, and interpretation separately; then test
   whether recommendations help a player identify or practice a passage.

These recommendations extend the existing plans with explicit inputs,
constraints, and acceptance evidence for each subsystem.

### Bounded co-op proof (2026-09-04, verified in current working tree)

Mark separately authorized a two-peer practice-space proof after the Graphshell
authoring proof. `crates/woodshed-core/examples/musical_comparison_export.rs`
derives C Major/A Minor from real Set/catalog/Stage data and emits
`scenarios/woodshed_musical_comparison.json`. It reports shared C/E and the
singleton difference G to A; seven focused tests cover transposition, repeated
source identity and inapplicable inputs. The result is a contribution specimen,
not implementation of the general S1 metric or S2-S4.

The Commons/Gemot, Graphshell and two-peer persistence gates live in
`mere/design_docs/mere_docs/implementation_strategy/2026-08-15_projection_grammar_adoption_plan.md`.
The host persists the comparison only on explicit contribution. The example
does not write Set truth or a user's chronological practice history.
Both the independent-process HTTP receipt and two Graphshell browser surfaces
passed admission, contribution and offline disk reopening. This is a fixture
proof over the real shared-domain services, not a shipped Woodshed co-op mode;
the canonical Mere receipt records its identity, clock and transport limits.

### Musical slice sequence

S1 before S2 (S2 consumes S1's costs). S3 is independent and can run in
either gap. S4 last; its prerequisite touches audition and is the only slice
that changes something a player already hears. Each slice is one subagent
task with its done-conditions as the acceptance list, reviewed here before
the next opens. Items that need Mark's call as they arise: the module name in
S1, the cost ceiling if a fixture argues against 1.5, and whether S4's
audition change ships or stays behind a setting.

## Stop rules

Graphshell consumer receipt (2026-09-05): Mere's
`ports/graphshell/docs/receipts/practice_workspace_receipt.json` records a browser
workspace consuming the byte-identical `scenarios/woodshed_musical_comparison.json`
export. Relations, pitch-membership Compare, History, and validated save/reopen
share selection; Seiche dragging changes transient geometry. The embedded page
is a wrapper around the same Graphshell component, not native Woodshed integration.
It does not close the musical transformation, audition, or release slices above.

- If S1's keyed relations make the fan illegible on a ten-card Set, the fix
  is a filter default, not fewer relations. Availability belongs to the scene.
- If any slice wants to write a derived fact back into `Set`, stop. Relations
  are projections; the Set is the document.
- If S2 wants a solver, it belongs in `scenomise`, and that is a separate
  decision with Mark.
