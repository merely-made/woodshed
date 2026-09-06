# Musical Projections Plan

**Status (2026-09-04): plan only; nothing implemented.** Written while another
session re-homes the Cambium dependencies from genet.git to mere.git and adds
the Stage export example; implementation waits for that commit and a clean
build. Implementation runs on haiku/sonnet subagents, one slice at a time,
each slice reviewed before the next opens.

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

### S1. Keyed occurrence relations

`woodshedding` gains a `harmony` module (name open; `pitch_class_set` if
`harmony` reads as a product word) with:

- `PitchClassSet`: a set of 0..12 built from `(root: PitchClass, intervals)`,
  so a scale or chord formula at a root becomes a concrete set. Reuse the
  transposition `Interval` already provides.
- `shared(a, b) -> PitchClassSet`, `moved(a, b) -> Vec<(from, to)>` as a
  minimum-motion assignment of the differing tones, and
  `motion_cost(a, b) -> f32` matching the semantics of
  `voice_leading_distance` in `woodshed-graph` so the two layers never
  disagree on a number. Move that function's circular-distance core into
  `woodshedding` and have `woodshed-graph` call it, rather than keep two.

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
same two formulas at roots a tritone apart yield neither; adding a C Major
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
