# Musical Projections Plan

**Status (2026-09-07): Circle-of-Fifths context, triadic Tonnetz, and minimum
pitch-motion comparison landed. S1 is partial; S2-S4 remain planned except
where explicitly recorded below.** Bounded comparison exports and
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
  same semantics (the rule `arrangement.rs` already states). Musical readings
  stay alongside the existing `StageGraphReading` policy. The proposed pitch-motion
  reading receives a stable anchor and complete candidate costs independently
  of visible edges; S2 records its layout contract. It is not contributed to
  `scenomise` in this plan.
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

### Minimum pitch motion (2026-09-07, landed)

The next slice makes comparison expose an exact one-to-one pitch-class
assignment. It holds shared tones and minimizes the sum of circular semitone
distances among the remaining tones, with deterministic tie-breaking. This is
an octave-free comparison, independent of the catalog's mean-nearest metric.
Nonempty equal-cardinality inputs have a score; different tone counts and
unresolved material have no scalar score in this slice. Existing exact shared
and exclusive tones remain visible, so omitted scoring does not hide differences.

The pure operation lives in `woodshedding::pitch_class_set`; keyed catalog
resolution stays in `woodshed-core::harmony`. The scene exposes pairwise motion
through current instance references, independent of visible/thresholded edges.
The context comparison shows total movement and each changed tone. Focus,
audition, and Add retain their existing semantics.

Done when known major/minor transformations report their exact movement,
assignment tests cover a case where greedy pairing fails, every target is used
once, reversal preserves total cost, and unequal/empty inputs are explicit;
a desktop scenario displays C-to-B and G-to-A movement while preserving Set
membership. A distance-based arrangement, unequal-voice policy, registered
voicings, and instrument-specific effort remain subsequent slices. In particular,
the older S2 focus-centered radial proposal must be reconciled with the user's
newer stable-position browsing decision before that layout is implemented.

**Validation:** all 349 library/example tests passed across woodshedding, core,
graph, and views. The independent exhaustive oracle checks all 48,400 pairs
of three-tone sets against all six assignments, including common-tone holding;
a separate fixture checks lexicographic ties. Scene tests prove motion works
without visible relations and rejects stale instance references. The desktop
build and `scenarios/p4e_pitch_motion.scn` passed. Presented captures under
`Code/testing/woodshed/pitch-motion-20260907/run01/` verify G-to-A two-semitone
motion, C-to-B one-semitone motion, unequal tone counts, and explicit Add.
The parent `receipt.json` records source/binary/artifact hashes. Existing
unused-import/dead-code warnings remain; physical audio was not tested.

The earlier S1 proposal below is historical scope, not a claim that thresholded
movement edges or its entire keyed relation family now exists. The scalar
score is total one-to-one motion, so its thresholds cannot inherit the older
mean-nearest metric's 1.5 value.

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

### S2. Nearby candidates and a pitch-motion reading (scoped 2026-09-07)

This replaces the older focus-centered radial proposal. Browsing focus must not
re-layout existing material. Work in two reviewable slices:

1. **Nearby candidates in the current reading.** Each reading supplies its
   candidate universe and reasons. Present a bounded, refreshable ranking beside
   the map, with exact held/moved tones and unavailable scores stated explicitly.
   Fifth distance, P/L/R depth, and summed pitch motion remain distinct columns;
   do not blend them into an unexplained relevance number. The first pitch-motion
   ranking can use the existing 24 major/minor triads, with the current nonempty
   equal-cardinality metric. Seventh chords and scales remain browseable in their
   applicable readings without inventing cross-cardinality distances.
2. **An explicitly selected Pitch motion reading.** Capture a layout anchor when
   entering the reading. Radius describes distance from that anchor, and stable
   catalog identity determines angular slots; zero-cost occurrences retain
   distinct targets. Focus changes comparisons and candidate ranking, while the
   anchor and existing coordinates remain fixed. An explicit recenter action may
   change the anchor. Label that distance is from the anchor, not every node pair.
   Unscored material occupies a labeled separate area, not an invented outer cost.

`StageGraphSnapshot::pitch_motion` already supplies values independently of
visible relations. Keep the complete candidate cost data independent of edge
thresholds; scalar totals cannot inherit the catalog metric's 1.5 threshold.
A product-owned reading model should collect candidate query, coordinates, and
relation explanations now dispatched separately in `stage_scene.rs`, `tonnetz.rs`,
and the view/host projection. It need not become a plugin framework.

**Ambient context boundary:** available catalog material, currently displayed
material, inspected focus, and authored Card occurrences are different states.
Shared/exclusive comparison emphasizes this wider material; it does not define
membership. Every suggestion carries a reason and its reading. Focus never
adds a Card or auditions it. Hear and Add remain explicit.

**Saturated browsing:** automatic retention fills the budget before fresh
candidates. Where offered by the reading, **Explore from here** already clears
disclosure at maximum breadth; Circle uses this coarse refresh, while Tonnetz
reports completion once all 24 triads are shown.
Separate discovery history/remembered coordinates from current membership, and
show nearby candidates even when the map is full. Refine the existing refresh
into an explicit Show nearby action that fills free slots or offers to replace
quiet, unpinned background at capacity;
protect authored, focused, and pinned material, preserve coordinates when material
returns, and expose breadth limits. Do not silently evict Cards to admit suggestions.
Multi-card comparison needs an explicit selection model; current Set cursor plus
context focus is only a pair. Scope that as a later extension, not an implicit
reinterpretation of every Card in the Set.

Done conditions: a full-budget exploration still offers an unseen neighbor;
repeated focus keeps coordinates and Set membership unchanged; unavailable
scores are distinct from zero; changing edge visibility cannot change ranking
or coordinates; the chosen anchor is visible; and a real pointer scenario can
inspect, hear, explicitly show, then add a candidate through existing boundaries.
The ranking slice can land before the new arrangement or any fingering work.

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

### S4. Resolve a selected shape, then compare neck movement (scoped 2026-09-07)

The prerequisite is one `woodshed-core` resolver consumed by `dots_for_card`,
`card_voicing`, and the effective-sound path. Today the board uses the live
instrument and all chord positions, while audition applies formula pitches near
MIDI 48. Neither reads `voicing_idx`. `ChordVoicing` has muted/played string
positions, pitches and intervals; it has no finger identities or barre contacts.

**First implementation slice: selected chord shapes.** Preserve
`Setting.voicing_idx == None` as its documented all-tone-map state. Add explicit
previous/next/clear shape controls; `Some(index)` opts into resolved-shape behavior.
An invalid index or impossible setup returns a typed unavailable reason and no
selected-shape sound, rather than silently substituting shape zero. The resolver
returns the exact setup, physical and relative positions, concert pitches, and
selection identity. An enumeration index alone must not be presented as a durable
shape identity: record/validate the string-position fingerprint and enumeration
profile before claiming the same saved shape across algorithm changes.

Resolution rules to implement and test together:

- A recognized explicit instrument resolves its named tuning with `find_for`,
  or a declared instrument-default policy when the tuning is absent. Unknown
  explicit values are unresolved. For legacy empty instrument fields, accept a
  unique named tuning or the matching supplied live tuning; empty instrument plus
  no tuning may inherit the live setup. Do not use first-match tuning lookup for
  ambiguous names. New authoring should store canonical instrument identity.
- Capo preserves the relative shape under the existing Setting contract. A C
  shape at capo 2 sounds D. Enumerate the untransposed setup and shape root, then
  add capo to physical frets and concert pitches; equivalently transpose BOTH
  tuning and requested root. Transposing only tuning would change the shape.
- Keep relative shape frets and physical display frets distinct. Pinned
  `FretWindow { start, span }` means inclusive physical `[start, start+span]`;
  absent windows inherit the live physical window. Convert to relative bounds
  before solving, reject below-capo/impossible windows, and honor instrument range.
- Start with the finder's root-bass policy. Fix root-bass validation to use lowest
  sounding MIDI pitch for re-entrant tunings, or return an explicit unsupported
  result there; string-list order is not bass order. Inversions require a recorded
  bass constraint in a subsequent slice.
- Apply existing Solo/Mute semantics through the same resolved setup. Sound masks
  do not automatically mean a finger contact was released. Scope comparisons as
  chosen-shape movement; do not silently change legacy mark-mode semantics.

Done when two selected shapes of one chord draw exactly their resolved physical
contacts and audition exactly their resolved concert pitches; tuning, capo,
window, invalid index and re-entrant bass fixtures pass; unselected Cards retain
the all-tone map; and actual shape-selection controls operate in Rehearsal.
Context catalog previews remain a separate formula-preview contract.

**Second slice: neck movement.** Compare two resolved shapes only under the same
instrument/tuning/capo setup. Report per-string held, moved, added and dropped
positions, total matched fret travel, span, and position shift separately. Added
or dropped strings do not receive an arbitrary fret-distance penalty. A same-chord
pair can have zero pitch-class cost and nonzero neck movement. Call this neck or
shape movement; the earlier `hand-moves` label overclaimed what the data can know.

**Later: fingering and phrase suggestions.** Finger/contact/barre identities,
held contacts, stretch, timing and user preferences must precede an ergonomic
score. Keep hard constraints separate from configurable cost components. Set
Card order and Looper `SongDoc` bars are currently separate authorities;
`Recipe::Song` is a source stamp, not a live event binding. Define the event-to-Card
realization contract before optimizing across bars. Then bounded candidate-layer
search can disclose candidate caps and modeled costs. It is optimal only within
that candidate set and model, not a universal claim about comfort.

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

The exact pitch-motion prerequisite is landed. Next, S2's nearby-candidate
ranking and S4's selected-shape resolver can proceed independently, with view
edits coordinated by component ownership. S2's anchored reading follows its
ranking contract; S4's neck comparison follows a verified resolver. Shape
selection is explicit opt-in through `voicing_idx`; legacy all-tone Cards keep
their existing behavior. S3 remains independent. General fingerings and
bar/line lookahead follow the contact and event-realization contracts, rather
than being implied by geometric neck cost. Each bounded slice uses its stated
done conditions before the dependent slice opens.

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
