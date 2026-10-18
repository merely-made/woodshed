# Woodshed complete-source audit: theory partition

Frozen published revision: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`.

Snapshot root: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.

Assignment: `theory-assigned.json`; evidence receipt: `theory-coverage.json` in this audit directory.

**Assigned read is complete: 21 of 21 files, 11,180 of 11,180 lines, including every test, example, manifest, comment and catalog entry. All 21 SHA-256 values match the assignment.** The receipt gives exact inclusive read ranges and per-file hashes. Additional files were read only to trace callers, shared suppliers, admission, and the relevant plans; their coverage is explicitly partial unless the complete file was read.

No builds, tests, native processes, hardware operations, network requests, dependency changes, repository writes, or Git mutations were performed. The only audit writes are this report and its coverage JSON. A read-only `git diff --name-only` and bounded source reads in the primary checkout were used solely to distinguish the concurrent P5a work from the frozen published source. This report does not certify that local work.

## Assessment

The assigned partition does not contain a second persistent theory catalog, an alternate Set document, or an independent practice-history store competing with the current owners. The main consolidation concern is the dormant instrument adapter: it consumes the shared Ringdown client but bypasses the client's explicit write-receipt vocabulary, then reports cached requests as matched device state. The live musical product has an advertised pentatonic exercise that is actually a fixed, approximate fret pattern. Set identity admission also permits malformed persisted identities to become apparently usable graph and action identities.

There are four P2 findings below, four narrower P3 findings, and two explicitly bounded unresolved API questions. None is claimed as a P0/P1 incident. Instrument findings are reachable through the public crate and its shipped example, **not through a current Woodshed GUI instrument surface**: the source search found no product crate depending on `woodshed-instrument`, and the smart-instrument plan explicitly leaves W3 integration open.

All paths below are relative to the snapshot root unless an absolute primary-checkout path is supplied. Each listed line range is an exact inclusive snapshot range.

## Findings

### T1 — P2: an attempted instrument write becomes a confirmed match without a receipt

**Location:** `crates/woodshed-instrument/src/lib.rs:228-239,330-338`; `crates/woodshed-instrument/src/session.rs:60-75,128-156`.

**Trigger and effect:** Call `Session::sync_metronome` in Drive mode with a meter whose denominator differs from the guitar's, or with a request the device refuses or silently ignores. `set_metronome` sends only `bpm` and `num`, discards the returned JSON result, and returns `Ok(())` even when that result is `false`. The session then caches the entire supplied `Metronome`, including the denominator that was never sent. A second identical call returns `AlreadyMatched(woodshed)` without reading the guitar. For example, a guitar in 5/8 driven with the example's 4/4 target can remain in 4/8 while the next call says `AlreadyMatched(4/4)`. A physical knob change while Drive remains selected similarly leaves the cached result apparently matched indefinitely.

This is more than an imprecise name: `AlreadyMatched` is documented as “The instrument was already set to Woodshed's tempo,” and the enclosing type promises what the call actually did. The code proves only “the same request was previously sent.”

**Reachability:** `crates/woodshed-instrument/examples/metronome_link.rs:72-92` exercises both outcomes directly. W3 is not integrated into the app: `design_docs/2026-08-27_smart_instrument_plan.md:4-9,154-177`. This is a public library/example defect, not an observed desktop behavior.

**Authority owner and competing owner:** Ringdown owns protocol write/receipt semantics. Its `Sent` type deliberately retains uninterpreted replies and explains that even `true` does not prove application: `ports/ringdown/crates/ringdown-client/src/lib.rs:227-248`. Its typed `update_metronome` already exists at `:686-697`, with `ReadMetronome` named as the receipt. Woodshed rightly owns Follow/Drive policy and the choice to omit denominator control, but its raw `.call(...) -> ()` wrapper and cached `AlreadyMatched` outcome re-create a weaker protocol-success authority beside that shared seam. The lower client returns the JSON result as a value (`:439-490`), so a false result is not automatically a transport error.

**Tests and gap:** `lib.rs:349-403` meaningfully tests omission and wire order, but does not exercise refusal or receipt admission. `session.rs:292-303` only checks private cached field equality; it never calls the async synchronization path. The example's enum assertions are generated from these same optimistic outcomes.

**Smallest coherent direction:** Keep product authority in `MetronomeLink`. Use Ringdown's typed metronome write, preserve the `Sent` receipt, and distinguish sent, refused, unverified unchanged request, and observed matched state. Cache only the fields actually written. Reserve a matched-state claim for a readback comparison of those fields; represent the denominator as device-observed state, not a sent value. Add fake-transport checks for false/true-but-unapplied replies and a denominator differing from the requested one. Do not add a second protocol receipt model in Woodshed.

### T2 — P2: the hardware proof can print a mismatch and still report confirmation; failures also skip restoration

**Location:** `crates/woodshed-instrument/examples/metronome_link.rs:20-27,31-44,84-126`.

**Trigger and effect:** Run the shipped example against a device that returns a BPM different from the target while requests otherwise complete. Lines 102-109 only print `MISMATCH`; execution continues to `Ok(())` at line 126 and main prints “metronome link confirmed against hardware.” The check does not compare the supported numerator at all. Restoration is also only printed, not compared with the captured original value. Thus a machine-readable successful exit and final success statement can coexist with failed readback or failed restoration.

A separate error path is part of the same proof workflow: once the target has been sent, any `?` before line 119 (for example, a readback timeout at line 97) exits `exercise` before its restore step. `run` still releases the connection, but leaves the temporary tempo/numerator on the guitar. The top-level comment promises it is left as found.

**Reachability:** This is the explicit hardware qualification executable invoked by the file's `cargo run` instructions at lines 4-14. No hardware was contacted in this audit. Existing recorded successful runs are not disproved, but this executable cannot reject the negative controls described above.

**Authority owner and competing owner:** Actual device readback owns the proof. The example's exit status and final message currently claim that authority using a print-only comparison, while its cleanup scope protects only disconnection. The plan itself says not to trust firmware `true` and requires read-act-read evidence: `design_docs/2026-08-27_smart_instrument_plan.md:484-497`.

**Tests and gap:** The file has no testable proof-result/restore helper. Assertions at lines 86 and 92 check the adapter's own enum, not the guitar. The final restoration read at lines 120-124 is informational even if it disagrees.

**Smallest coherent direction:** Capture the original supported fields, run the experiment, then always attempt restoration after the first write, and always release the session. Preserve both the experiment error and any restore/release error. Make both target and restoration readback comparisons fallible and check BPM plus numerator; keep denominator preservation explicit. The example should return success only after those receipts agree. This can be tested with scripted responses without a device.

### T3 — P2: “Pentatonic Box Shift” uses its own approximate geometry instead of the theory and tuning authority

**Location:** `crates/woodshedding/src/exercise.rs:248-313,468-475,615-630`.

**Trigger and effect:** Select the built-in Pentatonic Box Shift on standard guitar at fret 5 and run it. The first string gets offsets 0/3, but every remaining string gets 0/2; the tuning pitches are never consulted. The first box therefore plays, among other notes, B4 at high-E fret 7 and F#4 at B-string fret 7. Its pitch classes are A, B, C, D, E, F#, G, whereas the catalog's A minor pentatonic is A, C, D, E, G (`scale.rs:239-250`). Even on standard guitar, the advertised box is not the claimed minor-pentatonic box. The second box simply translates that same approximation by three frets. Alternative and re-entrant tunings compound the disagreement.

**Reachability:** The exercise is in the normal catalog. `crates/woodshed-core/src/lib.rs:1188-1197` calls its generator for the rendered board, and `:2105-2120` calls the same generator then `Fretboard::pitch_at` to obtain its sounding note. `crates/woodshed-views/src/stage.rs:2905-2922` supplies Run; `crates/woodshed-genet/src/drive.rs:279-283` plays the generated current note. This is a product path, not an unused example.

**Authority owner and competing owner:** The owned scale formula and tuning-aware fretboard map already define pitch membership and realization (`scale.rs:66-74`, `fretboard.rs:175-185`). The exercise introduces another musical-truth implementation using only string count. Mechanical chromatic and spider patterns may intentionally ignore tuning pitches; a named pentatonic shape cannot safely do that while promising the scale's notes.

**Tests and gap:** The only dedicated check at `exercise.rs:615-630` asserts 24 steps and the presence of base frets 5 and 8. It certifies the current implementation's count and shift, not the musical meaning. The “all exercises generate” test at `:588-594` is similarly insufficient. The source comment itself calls the upper-string rule approximate at `:259`.

**Smallest coherent direction:** Either derive an explicitly defined box recipe from the canonical minor-pentatonic formula over the resolved tuning, using the existing position map, or honestly rename this as a mechanical translated fret drill. If the intent is two positions of the same scale, define the target scale/root independently of shape translation. Validate exact standard-guitar notes and one alternate/re-entrant tuning against scale membership, rather than only count. Do not create another hard-coded scale-tone table in the exercise module.

### T4 — P2: Set load admission accepts duplicate occurrence identities although actions and graphs treat identity as unique

**Location:** `crates/woodshedding/src/rehearsal.rs:344-354,402-420,507-524,529-535,565-600,606-627`.

**Trigger and effect:** Reopen a persisted Set containing two different Cards with the same nonzero `id`. Serde accepts it; `ensure_card_ids` repairs only zero IDs and keeps both duplicate IDs. `index_of`, `card`, `card_mut`, and `select_id` resolve the first occurrence, while `graph` emits both nodes under the same identity and may derive a `Next` self-edge. Selecting or editing the later occurrence by ID can therefore address the first Card instead. This violates the occurrence contract before any malformed graph/capture reference needs to be invented.

**Reachability:** `crates/woodshed-core/src/storage.rs:524-526` deserializes the session without a Set identity validation pass. `crates/woodshed-views/src/stage.rs:2417-2425` adopts the saved Set and calls `ensure_card_ids` at `:2516-2520`. There is no duplicate rejection in those read paths. The trigger requires malformed/imported persisted data; ordinary `push` and `duplicate` correctly mint different IDs. This is not an allegation that ordinary staging currently creates duplicates.

**Authority owner and competing owner:** The Set is the intended identity owner. Its mint and lookup APIs enforce/assume uniqueness, but derived `Deserialize` plus the load migration admit an incompatible second entry path. The code already recognizes the need for this invariant when copying: `copy_instructions_from` rejects duplicate source IDs at `rehearsal.rs:530-535`. Admission and copying therefore disagree about whether the same document is usable.

**Tests and gap:** Tests at `rehearsal.rs:772-808,819-869` cover fresh minting, removal/non-reuse, roundtrip and zero-ID migration. None covers duplicate nonzero persisted IDs or ambiguous graph references. The copy guard is not a substitute because rendering and actions can happen before a copy.

**Smallest coherent direction:** Put one fallible identity validation/migration operation at the Set boundary and call it for every loaded active, inactive, and retained Set. Permit the existing unassigned legacy migration, but refuse/quarantine ambiguous duplicate identities with a visible, owner-specific error. Avoid silently renumbering duplicate IDs because arrangement, loop and relation references cannot be disambiguated afterward. Reuse this validator in copying instead of maintaining two admission policies.

### T5 — P3: the instrument's typed decoder fabricates valid-looking measurements, and its resonance test never calls that decoder

**Location:** `crates/woodshed-instrument/src/lib.rs:117-131,241-261,442-462`.

**Trigger and effect:** A `GetAnalysis` row with four cells but nonnumeric entries, such as `["bad", null, "bad", {}]`, passes the row-shape check and becomes an `Ok(Resonance)` of zero values. A numeric-but-out-of-range metronome field also silently truncates through `as u16`/`as u8` (for example `num: 260` becomes 4). Both paths claim typed instrument facts different from the wire data instead of returning `InstrumentError::Shape`. This conflicts with the crate's explicit “Nothing is reported as fact until the instrument said it” contract at `lib.rs:9-14`.

**Reachability:** Public `Connection::read_resonances` and `read_metronome` are direct callers of these parsers. The instrument seam has no current GUI consumer; malformed-device behavior was not observed on hardware in this audit.

**Authority owner and competing owner:** The instrument reply owns the measurement. The adapter properly owns name/type conversion, but zero defaults and unchecked narrowing introduce guessed facts as if they were observed. This should be one strict conversion seam, not additional fallback policy in future product views.

**Tests and gap:** `the_captured_analysis_maps_to_resonances` at `:443-462` reads cells from its own literal JSON and asserts those literal values; it does not call the production mapping at `:248-261`. It would pass if production frequency/gain/Q indexes were swapped or if all measurements were replaced with zeros. Metronome tests exercise normal data and one missing field, not numeric bounds.

**Smallest coherent direction:** Extract the production row decoder into a pure function and call it from the connection method and fixture tests. Require each numeric cell, validate finite conversion and documented ranges where known, and use checked integer narrowing. Preserve unknown filter types as unknown values if that is intentional, but do not turn a missing/invalid type into filter 0. Add malformed rows and overlarge metronome fields as negative fixtures.

### T6 — P3: modal-family copy promises “same tones” while the related action preserves the tonic

**Location:** `crates/woodshed-graph/src/lib.rs:1165-1195`.

**Trigger and effect:** With C Major selected, inspect its Dorian mode-family relation. The rotation test correctly recognizes a formula relationship, but emits “Same tones, different centre.” The Related action selects Dorian while preserving the existing C root (`crates/woodshed-core/src/lib.rs:1053-1081`), yielding C Dorian, whose Eb and Bb differ from C Major's E and B. The reason is true of C Major and D Dorian, but the action does not carry that tonic change.

**Reachability:** Formula reasons flow through `StageState::related_material` at `crates/woodshed-core/src/lib.rs:809-867`; the normal Related pane displays `item.reason()` at `crates/woodshed-views/src/stage/related.rs:676-679` and selects the target at `:692-694`.

**Authority owner and competing owner:** The graph intentionally owns root-independent formula relationships; `woodshed-core::harmony` owns concrete keyed pitch sets. The rotation algorithm is not a duplicate to remove. The copy crosses their boundary by presenting formula-family membership as a statement about the current configured tones.

**Tests and gap:** `woodshed-graph/src/lib.rs:1696-1705` checks that Major/Dorian relate as modes. It does not assert the explanation against the root-preserving product action. The plans explicitly preserve the formula/keyed distinction (`design_docs/2026-09-04_musical_projections_plan.md:66-81`).

**Smallest coherent direction:** Keep the formula graph and change this explanation to accurately identify a rotated interval-pattern family. If a same-tones modal transformation is desired later, derive the required tonic and validate the concrete keyed result in the existing harmony layer before offering it as that action.

### T7 — P3: Quartal Tetrad violates the catalog's ascending compound-interval contract

**Location:** `crates/woodshedding/src/chord.rs:64-76,510-520`.

**Trigger and effect:** Apply Quartal Tetrad to C4 or preview it from the chord catalog. The final interval is `MINOR_THIRD`, so the result is C4, F4, Bb4, Eb4; the last note drops below the preceding two. Three stacked fourths require the final Eb in the next octave. Both the formula field and `apply_to` promise ascending pitches, and the entry's own comment calls the final third compound.

**Reachability:** `crates/woodshed-core/src/lib.rs:1872-1890` directly converts selected chord formulas into preview frequencies without repairing register. An unselected-shape Card also uses `apply_to` at `:1964-1972`. Fretboard pitch-class mapping ignores register, which explains why existing board/graph checks can appear fine despite the preview/register error.

**Authority owner and competing owner:** The canonical chord formula owns registered interval meaning. No external competing authority is needed; this is inconsistent data at that owner. Sorting at each consumer would hide the problem and still not produce the intended stacked-fourths register.

**Tests and gap:** The quartal test at `chord.rs:702-714` covers only Quartal Triad. The all-catalog test at `:756-766` merely checks that transposition succeeds; it does not check ascending MIDI order or the tetrad's expected intervals.

**Smallest coherent direction:** Express the final tone as a minor tenth in the canonical interval/formula vocabulary and add the explicit tetrad pitches plus an ascending-interval catalog invariant. Do not duplicate a corrected tetrad in preview or arpeggio code.

### T8 — P3: “Tenor (fifths)” is cataloged in fourths; the current test proves only that the name exists

**Location:** `crates/woodshedding/src/tuning.rs:1161-1172,1910-1917`.

**Trigger and effect:** Select Bass / Tenor (fifths). The entry's comment says CGDA and its name says fifths, but its actual open pitches are A1, D2, G2, C3: adjacent MIDI intervals are 5, 5, 5 semitones, which are fourths. A player selecting the advertised fifths tuning gets an ADGC board and sounding notes. This conclusion is from the source's own pitch/interval arithmetic; no external tuning standard is assumed.

**Reachability:** `Tuning::find_for` at `tuning.rs:203-209` returns this entry unchanged, and the ordinary tuning catalog supplies the product's resolved boards (`crates/woodshed-core/src/lib.rs:1090-1094,1406-1419`).

**Authority owner and competing owner:** The named catalog is authoritative; the label/comment and pitch data disagree. Tuning generators are complementary and should not be removed. The intended data correction remains a choice between the current tenor pitches and the advertised fifths identity.

**Tests and gap:** `bass_catalog_includes_extended_range_and_dropped` only asserts lookup success for this name. Other tests explicitly verify fifths/fourths for bowed instruments (`tuning.rs:1979-2011`), but this entry lacks an equivalent invariant.

**Smallest coherent direction:** Resolve whether the intended entry is the current ADGC tenor tuning or a CGDA/fifths entry, then correct its name/comment or pitches together at the catalog owner and add an exact interval fixture. Because Card settings persist tuning names, retain a migration/alias for any renamed identity rather than silently invalidating old Cards.

## Deliberate boundaries and false-positive rebuttals

- **Owned theory is intentional.** `CLAUDE.md:109-113` and `DOC_README.md:349-366` explicitly require the owned pure theory core. The absence of an external theory package is not siloing. The assigned `woodshedding` manifest has only serde at runtime and no I/O/UI/audio dependencies.
- **PracticeSet is a live template, not a second Set authority.** `practice.rs` owns reusable generators. `crates/woodshed-core/src/lib.rs:527-605` materializes them through `Set::from_cards`, and `crates/woodshed-views/src/stage/templates.rs:6-27` opens a new owning Set. `design_docs/2026-07-11_stage_set_tools_plan.md:1174-1184` explicitly retains this role. Removing it as obsolete would remove current Recipes behavior.
- **Catalog formulas, arpeggio identities and configured occurrences serve different subjects.** `woodshed-graph` derives its nodes from the theory catalog and gives chord/arpeggio separate identities without copying a second formula table (`lib.rs:690-778`). Configured roots/tunings and CardIds stay in the owning Set. The graph's OnceLock caches are derived values, not persistent competing documents.
- **The two pitch-distance algorithms are intentionally different.** Graph `voice_leading_distance` (`woodshed-graph/src/lib.rs:1062-1080`) is symmetric mean-nearest distance over formula pitch classes; `woodshedding::pitch_class_set::minimum_motion` (`:46-118`) is exact one-to-one assignment on equal nonempty concrete sets. The explicit design distinction is in `design_docs/2026-09-04_musical_projections_plan.md:220-240,511-519`. Combining them under one algorithm would change semantics, especially for different cardinalities. Naming/units can be clearer, but numerical disagreement alone is not duplicated authority.
- **Set graph projection does not author another document.** `Set::graph` derives node numbering and `Next` from current Cards; `with_relations` filters its result. No Set mutation or history write occurs. T4 concerns invalid identity admission, not the existence of this projection.
- **Generated inverse rows preserve one source assertion.** `RelationBuilder::add` and `add_directed` keep the same claim ID/provenance for inverse display (`woodshed-graph/src/lib.rs:891-942`). Independent progression reasons add progression/ordinal components (`:1219-1243`) and have dedicated tests. These are not accidental duplicate assertions.
- **Saved musical alignment belongs on Set.** The small `rehearsal/arrangement.rs` module contains musical integer coordinates, independent of scene geometry. `Set::copy_instructions_from` remaps both live references and missing references under fresh IDs. This is the intended scope described at `design_docs/2026-07-11_stage_set_tools_plan.md:3718-3744`, not a duplicate of Mere camera/layout persistence.
- **Named tuning duplication is often deliberate cultural/instrument identity.** Baritone names sharing pitches, bowed/mandolin equivalents and standard/transform APIs serve different queries. `DOC_README.md:407-413` explicitly says to retain both catalog and generators. T8 is an internal label/data contradiction, not a request to collapse named tunings.
- **Courses, diatonic dulcimer frets and microtonal support are known model limits.** `tuning.rs:1520-1524` discloses flattening and diatonic-fret refinement; `design_docs/2026-07-14_instruments_and_fretboard_rendering_plan.md:109-147` records the scope. They should not be reported as newly discovered hidden duplication. The source does not justify claiming physical cross-instrument qualification.
- **Voicing enumeration shares one validator and admits incomplete work explicitly.** Bounded/unbounded wrappers use the same search; `BoundedVoicingSearch.complete` avoids treating a capped prefix as the full picker inventory. Bass validation uses minimum sounding MIDI pitch for re-entrant tunings (`fretboard.rs:431-445`), with an explicit high-G ukulele regression (`:639-662`). The conservative result-limit incomplete flag is not a success-path bug.
- **Connection discovery already refuses the empty inventory.** `Connection::open` indexes `found[0]`, but the supplier `ringdown_ble::discover` returns `NotFound` when empty (`ports/ringdown/crates/ringdown-ble/src/lib.rs:205-210`). Do not flag that index as an unhandled ordinary no-device panic.
- **Woodshed's duplicate ReadConfig list is currently redundant, not an observed bypass.** Woodshed checks it at `woodshed-instrument/src/lib.rs:293-297`; Ringdown independently refuses the same method case-insensitively at `ringdown-client/src/lib.rs:459-474`. The shared transport guard still catches differently cased names. Consolidating protocol refusal metadata into the supplier is sensible housekeeping; the current redundant list alone is not evidence of an unsafe ordinary caller.

## Known concurrent P5a work — separate from frozen findings

The frozen source has the already identified triplicated Card timing policy: core dwell at `crates/woodshed-core/src/lib.rs:614-625`, arpeggio preview at `connected_catalog.rs:154-173`, and scale preview at `scale_realization.rs:233-252`. The snapshot plan explicitly tracks this at `design_docs/2026-07-11_stage_set_tools_plan.md:1748-1793`.

Read-only inspection of the dirty primary checkout found `/Users/markik/Code/repos/woodshed/crates/woodshed-core/src/card_timing.rs:1-116`, a fallible `resolve_card_timing` using `Duration::try_from_secs_f32`; primary `lib.rs:613-615` delegates dwell to it; primary `connected_catalog.rs:154-156` and `scale_realization.rs:233-235` delegate ordered articulation to the same result. This addresses the source-level duplication direction in the active local lane. Those files are outside the assigned theory files, and no finding T1–T8 is claimed fixed by that work. I did not run its tests, inspect all dependent host changes, or claim native/reopen acceptance. The primary source was active and remains mutable; its hashes at observation are recorded only as local context.

## Unresolved questions and scope limits

1. **PitchClass deserialization invariant:** `pitch.rs:11-25` promises 0..=11 but derives Deserialize directly on the private `u8`; serde can construct values 12..=255 without `PitchClass::new`. `pitch_class_set.rs:49-70,115-118` relies on a maximum 12-tone set and distance <= 11, so callers passing directly deserialized sets can violate the DP bound and subtraction invariant. Current product harmony resolution normalizes computed sounding classes through `PitchClass::new` (`crates/woodshed-core/src/harmony.rs:206-225`) before minimum motion, so I did **not** promote this to a demonstrated desktop DP crash. Decide whether the portable type should reject invalid wire values or normalize them as its constructor does; avoid leaving that decision to each consumer.
2. **Evidence basis is absent from generated claim identity:** `MaterialRelation::evidence_with_count` changes the provenance method/evidence basis but retains the same kind/source/target claim ID (`woodshed-graph/src/lib.rs:521-565`). Production has both WalkedPath (`crates/woodshed-core/src/mere.rs:112-137`) and StatedFromReason (`crates/woodshed-core/src/lib.rs:910-931`) producers. Their current display paths are separate; I did not establish a live combined export containing both, so this is not counted as a collision finding. Before combining the paths, decide whether the methods describe separate claims requiring distinct ID components, or different projections of one explicitly shared claim.

No physical-device assertions were refreshed. Prior plan receipts remain historical evidence at their recorded inputs. Complete assigned-file reading does not imply complete neighboring-file coverage; the parent audit owns whole-repository consolidation and independent verification. The report distinguishes source-proven execution paths from hardware outcomes that were not run.
