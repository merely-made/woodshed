# Stage, Set, Tools and practice

Historical source: [Stage, Set, Tools, Rehearsal, and Looper Plan](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Boundaries and standing decisions
One Set owns authored Card occurrences; the session Overview and tools project it. Exploration never silently edits the Set. Preserve catalog entry, configured material, playable realization and occurrence identities separately. Source: original Product model and connected exploration, lines 483–855. The 2026-10-11 release ruling says: “no GitHub workflows and no default pull-request review step.” Source: original lines 1089–1092. Explicit release instruction remains necessary.

**Overview naming (Mark, 2026-11-04).** The workspace view over `woodshed-core` `session_overview` (`WorkspacePanel::Overview`) is labelled **Overview**, formerly "Mere". Lowercase mere is the data domain, not a panel. **Reservoir** is the name this view earns once it can browse other domains' meres (the [DSP/Graphshell/Hocket survey](2026-10-14_dsp_graphshell_hocket_survey_brief.md#three-readings-of-graphshell-in-woodshed)'s reading (c)); today it shows one domain.

## Open work and done conditions
- P1–P3: separate shell and section coordination, complete one-owner settings and explicit staging/tray interactions. Done when sections can change independently, host contains no product composition, settings and routes resolve to owned state and saved session reopening works (1393–1450).
- Connected practice: implement bounded cross-catalog relationships with explained reasons, explicit audition/stage/replace, shared realization for fretboard/audio/rehearsal, and durable event-time history. Done when chord→arpeggio→Set→rehearsal, chord/scale/progression, transition drill, exercise discovery, history-informed exploration and constrained passage paths meet original individual conditions (823–839), including duplicate material, tuning/capo, missing inputs, pause/edit, keyboard/list parity and reopen. Human musical usefulness, acoustic and native evidence are separate.
- P4: complete P4f explanations, keyed-instance relations, fretboard/catalog context and semantic zoom without changing Set authority. P5/P6 bounded accomplishments do not settle remaining full articulation, keyboard and listening acceptance. P7: complete Set-derived Looper, saved backing and PCM persistence, physical capture, drift and explicit source ownership. P8: narrow and high-scale clipping, accessible names and targets, touch/reduced motion, cross-platform presentation and release acceptance (1451–2435).
- Release: named revision, clean locked graph, platform archive/checksum, extracted launch, isolated save/reopen and known limits. 1.0 requires a persisted material→playable route→honest history→intelligible next-step flow, recovered after reopening (1081–1170).
- Catalog expansion and musical syntax: previewed recipes lower to matching explicit Cards; support realizations before advertising audition; validated packs have stable identity and provenance. Musical syntax remains a design problem; Knot owns linguistic records (1114–1157).

## Deferred and unresolved
Sampler full scope remains deferred. From approved prototype carry: distinguish explicit Record from Play and editing selection from moving playhead; per-bar tempo/meter/subdivision and multi-bar copy need current-source review. Mark chose a one-bar click count-in and sampler scope A (waveform, gain/reverse/normalize, replace/overdub; trim deferred); older no-track, dry and no-song-MIDI scope rulings are dated, requiring reconciliation with later design, not blanket permanent bans. Prototype sources: `archive_docs/2026-11-03/2026-05-16_song_mode_integration.md`, `archive_docs/2026-11-03/2026-05-19_song_timeline_layers_plan.md`. Cross-platform native, physical microphone, audibility, latency and human musical review remain open.


## Findings and progress

The successor initially added no fresh gates. H8 adds the bounded software
finding below; headed, audio and hardware acceptance remains separate.
Recheck open work against current source before implementation.

## Closure carry-forward check (2026-11-03)

The completed Mere-take, arrangement-observation, passage-selection and relationship-source-return records do not qualify physical microphone, audibility, audio quality or latency; arrangement and relationship consolidations add no fresh native GUI acceptance. New-take fallback placement is bounded label clearance, not arbitrary authored-overlap solving. Source: original Closure carry-forward.

The older composable-surface Form B vision describes simultaneously resizable and persisted fretboard, tuner and metronome, tuner pausing other sound, shared exercise/progression clock, progression chord grid and custom exercises/progressions/tunings as 1.0 vision. Treat it as dated design intent requiring reconciliation against later Stage direction, not a new mandate or implementation claim. Source: `archive_docs/2026-11-03/2026-05-21_composable_instrument_surface_plan.md`.

## Specific Stage and responsive acceptance

P3 requires every catalog kind to stage valid Cards, previews exact multi-Card results before insertion, and tray selection/reorder/duplicate/remove/timing/touch/placement/clear/save/load without a separate Rehearsal or Looper material editor (original 1432–1449). P4 retains full theory-map catalog, keyed-instance relationships, fretboard Space and replacement of hand-rolled Related placement as checkable obligations (1451–1457). P8 retains the 420px toolbar overflow, 125% graph clipping, edge/text separation, recorded-loop glyph, compact filmstrip access and stale edit notice as named recheck candidates; verify current code before labeling each unresolved. Original 2390–2434.

## Additional prototype intent and review question

Retain recipe-derived Card provenance and individual edits without implicit resynchronization; spatial setting is independent of sequence time. The historical generic-resolver convergence was expressly declined; rich lens rendering and touch remain with their current owners. Original 2026-11-03 prototype carry. **Review question:** where the chosen Form B surface conflicts with the newer Stage direction, which concrete behavior should govern? Preserve the dated choice pending an explicit ruling. Sample slots/import were alternatives, not approved requirements.

## Coverage question for review

Which portions of P1–P4 and full P7 are already satisfied on current source, and which exact saved-backing/PCM and release acceptance criteria should the successor retain? Reconcile the original phase-specific criteria before choosing a narrower backlog.

## Capture custody and discovery contracts

P7 verifies immutable content-addressed PCM companions before atomically replacing session references. A failure before reference publication preserves the previous saved state and the live take; a readback failure after publication is reported as unconfirmed, without rollback. Unavailable references remain visible with explicit Clear. Set and Card identities survive reorder; captured-clock changes are refused until Clear, and replacement while capture is busy is refused. Existing clocks and PCM remain unchanged. P7b has bounded Seconds qualification; full saved-song PCM/migration, unsupported Manual/Reps/eventless cases and hardware acceptance are separate. Original lines 2111–2185, 2240–2253.

Relationship queries disclose bounded search and truncation, unsupported constraints, competing readings and all applicable reasons. Hard constraints remain distinct from preferences; an exercise's teaching purpose is not inferred from its title. Practice observations retain event-time realization and context, rather than interpreting old events through current settings. Original lines 731–813. These are accepted contracts and done conditions; their inclusion does not by itself establish current implementation.

### Overview process-count assertion, 2026-11-04

H8 source-split qualification reproduces the existing root-workspace failure
`stage::overview::tests::navigation_and_graph_inspection_do_not_start_or_pause_live_processes`
in `crates/woodshed-views/src/stage/overview.rs`: actual process count 2,
expected 3. That source is unselected and its assertion remains unchanged.
Resolve the intended process-count contract and reconcile the projection or
fixture in this owning feature lane; done when the existing case passes
without weakening the navigation/process-ownership contract. The source-split
[receipt](archive_docs/receipts/2026-11-03_source_splits_plan_receipts.md) records
the untouched baseline and post comparison. This is not a new device or
headed acceptance claim.

### Overview label, 2026-11-04

Progress: `WorkspacePanel::Overview` is labelled "Overview", formerly "Mere"
(`crates/woodshed-views/src/workspace.rs:51`); the `woodshed-genet` layout
selectors follow, as do the Overview title, its accessible graph name,
notices, the relationship editor's Back action, the Related graph arrangement
setting and the `scenarios/*.scn` panel clicks (scenario file names keep
their historical `mere_` prefixes). Saved workspace snapshots carry tile titles, and restore
requires the registered title, so a layout saved as "Mere" would have failed
closed and the desktop host would have refused writes for that session
(`crates/woodshed-genet/src/session.rs:66-69`). Restore now admits exactly
that former Overview title and rewrites it to the current label; any other
altered title still fails closed. The
[receipt](archive_docs/receipts/2026-11-03_stage_set_tools_plan_receipts.md)
records the checks. Native GUI and headed acceptance of the label are not
claimed.
