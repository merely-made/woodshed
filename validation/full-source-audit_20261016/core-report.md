# Woodshed complete-source audit: core partition

Frozen published source: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`, at `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/source-d12201bf`.

**Complete:** every line of all 49 assigned files has been read, including tests/configuration/examples/binaries: **23,561 unique lines**. All 49 end-of-read SHA-256 values and line counts match the frozen assignment manifest. This report records **eight source-traced P2 findings**, **two P3 consolidation/cruft candidates**, and the separately owned dirty P5a timing work. No runtime reproduction is claimed.

| Reader | Assigned files | Unique assigned lines | Status |
|---|---:|---:|---|
| Root: persistence/history/settings/library seams | 7 | 6,303 | Complete |
| Projection/owner-bank reviewer | 8 | 6,280 | Complete |
| Music/realization reviewer | 15 | 6,348 | Complete |
| Media/scene/export reviewer | 19 | 4,630 | Complete |
| **Total** | **49** | **23,561** | **Complete; all hashes match** |

## Scope and evidence

The assignment is 49 files and 23,561 lines, including manifests, examples, binaries, and tests. `core-coverage.json` records the expected and observed SHA-256, line counts, reader, exact read ranges, and completion per assigned file. Source was read in full chunks; searches served only to trace callers and neighboring evidence. No builds, native runs, network requests, test executions, or source/Git changes were performed. Only the external `core-report.md` and `core-coverage.json` audit artifacts were written.

Paths and line numbers in findings refer to the frozen snapshot unless explicitly marked as supplier or dirty-primary observations. Existing tests are described from source; this audit does not claim that they were rerun. Supplemental neighboring files were read in the precise ranges listed in the ledger, rather than counted as complete source coverage.

## Findings

### CORE-1 — P2: malformed current history can be accepted as an empty legacy history

**Trigger and effect.** Reopen a saved session whose `practice_history` is a malformed current lineage object, for example `{"entries":[],"visits":"damaged","owners":[]}`. The current `Lineage` alternative fails its type check, then the untagged decoder tries `FlatLog`. That alternative ignores the lineage keys and defaults missing `events` to an empty vector. The result is a successful empty `PracticeHistory`. The original history can then be replaced by a cleanly serialized empty lineage during an ordinary save. This defeats the host's existing policy to retain and quarantine undecodable session data.

**Evidence and reachability.** `crates/woodshed-core/src/history.rs:390-405` defines a permissive defaulted legacy fallback; `history.rs:414-438` returns success and reconstructs an empty lineage. `crates/woodshed-core/src/storage.rs:524-526` admits it as a `PersistedSession`. The real desktop restore uses that decoder in `crates/woodshed-genet/src/session.rs:61-80` and refuses writes only on an actual error. `crates/woodshed-views/src/stage.rs:2476` installs the empty history and `stage.rs:2383-2400` captures it for persistence. The production commit path in `crates/woodshed-genet/src/capture_persistence.rs:525-528` serializes and saves that captured session.

**Authority owner and competitor.** The retained lineage is the history authority. The old-format compatibility decoder is acting as a competing interpretation of a damaged current-format object; the legacy format itself is a legitimate read-only migration.

**Tests and gap.** `history.rs:885-956` covers valid current roundtrip and valid legacy migration. It does not cover a damaged current lineage being rejected instead of decoded as an empty flat log. The desktop corruption tests exercise top-level invalid JSON or workspace admission, which this successful fallback bypasses.

**Smallest coherent direction.** Discriminate the current and legacy shapes before decoding: only enter flat migration when the explicit legacy `events` shape is present. Reject malformed recognized current fields. Preserve absent top-level history as the existing legacy default. Return the error through `decode_session` so the existing quarantine behavior retains the original bytes. Add one current-corruption/legacy-discrimination regression and a host reopen/quarantine regression.

**Dirty P5a status.** No dirty change to `history.rs` was present during the read-only primary-checkout check; the timing work does not address this.

### CORE-2 — P2: a typed but invalid history reference can panic before session quarantine

**Trigger and effect.** A well-typed retained lineage can contain an out-of-range visit, owner, or entry index. An example history is `{"entries":[],"visits":[],"owners":[{"identity":"practitioner","origin":null,"current":0,"creator":null,"pending_origin_parent":null,"owned_visits":[]}]}`. Its JSON types match `StemmaSnapshot`, so it selects `Lineage`, then restoration indexes an empty visit vector. Reopening the session panics instead of returning a recoverable admission failure. The same file causes the failure again on the next launch until repaired externally.

**Evidence and reachability.** `crates/woodshed-core/src/history.rs:414-419` calls the infallible supplier `Lineage::from_snapshot` directly from deserialization. The frozen `Cargo.lock:1305-1307` resolves Chartulary to Mere `6575075057e8909032475ae6c88118a19969eadc`. At that exact local supplier checkout, `crates/eidetic/chartulary/src/stemma/snapshot.rs:50,69-83,96-104` directly indexes `entry_ids`, `visit_ids`, and `owner_ids`; line 97 is the example's failing access. The supplier DTO definition is `crates/eidetic/chartulary/src/stemma/mod.rs:271-280,335-345`. Desktop `crates/woodshed-genet/src/session.rs:61-77` expects a `Result` from decoding but cannot quarantine a panic during that decode.

**Authority owner and competitor.** Chartulary should own structural snapshot reconstruction and invariants; Woodshed owns admission and retention of the product session. At present the trusted in-memory reconstruction API is also used as the durable-input admission boundary, with no check between them. There is no need for a second history store or model.

**Tests and gap.** The core history tests cover valid roundtrips, migration, provenance, and observation semantics, not invalid snapshot topology. No tests were executed for this audit; the index failure follows directly from the exact locked supplier source.

**Smallest coherent direction.** Add a fallible checked reconstruction seam in the supplier (bounds and graph consistency), then use it from Woodshed's deserializer and propagate an ordinary error to session quarantine. A local interim validator should be narrowly scoped and later replaced by the shared supplier seam, rather than duplicating a full reconstruction implementation. Cover invalid entry/parent/child/binding/owner references and ensure the original saved bytes stay retained. Consider cycle validation as part of the shared structural contract.

**Dirty P5a status.** The known timing changes do not touch this path.

### CORE-3 — P2: practice-based Related promotion has no normal UI producer

**Trigger and effect.** Stage a suggestion from the Related panel, rehearse that occurrence, then return to the originating material with history enabled. The ranking function cannot learn that the suggestion was practiced from this origin. Staging records the origin only on a `Staged` event, which is deliberately excluded from practice evidence. Both rehearsal-start and rehearsal-finish observations write `from_id: None`. The feature's positive tests construct a practice event with an origin that the ordinary product flow never emits.

**Evidence and reachability.** The consumer is `crates/woodshed-core/src/history.rs:246-250` and `crates/woodshed-core/src/lib.rs:910-937`: it requires both `is_practice()` and a matching `from_id`, then adds the "Previously practiced from here" evidence and boosts rank. The real panel calls this consumer at `crates/woodshed-views/src/stage/related.rs:641-646`. Its Stage action captures the origin at `related.rs:714-717`, and `crates/woodshed-views/src/stage.rs:2232-2251` stores it as `EngagementKind::Staged`. `stage.rs:2282-2288` and `stage.rs:2313-2315` emit the practice lifecycle events with no origin. Neighboring connected-material paths similarly use origins only for Previewed/Staged observations. This does not invalidate the separate observed-adjacency `transitions()` projection, which has different semantics and remains reachable.

**Authority owner and competitor.** Stated choice provenance belongs to Woodshed's observation/occurrence history. The Related ranking reads a schema that the product's observation writer does not maintain. This is a disconnected producer/consumer contract, rather than a second durable history authority.

**Tests and gap.** `crates/woodshed-core/src/lib.rs:2640-2673` injects `Rehearsed` plus `Some(from)` directly; `history.rs:469-519,843-882` tests the same synthetic possibility. These prove query behavior but not the normal Related → Stage → Rehearse path. Valid historical/imported practice events or direct API callers can still activate the query; the gap is the current ordinary UI producer. Existing provenance carries stable Set/Card IDs, so a narrow integration test can exercise the real actions without guessing from current catalog focus.

**Smallest coherent direction.** Preserve the explicit staging origin for the exact `(WorkingSetId, CardId)` occurrence and carry it into the appropriate event-time rehearsal observation, or define a query that joins the exact staging observation to subsequent observed practice for that same occurrence. Keep stated origin separate from walked adjacency and do not turn staging itself into practice. Count an actual practice boundary consistently to avoid counting both start and completion as two accepted choices. Test the real UI action path, repeated material in distinct occurrences, cross-Set local ID collisions, source editing, and reopen.

**Dirty P5a status.** The dirty timing work changes audition/refusal plumbing but does not repair history origin production.

### CORE-4 — P2: saved drawn Paths are retuned by the live exploration setup

**Trigger and effect.** Draw and save a Path while using Standard tuning, navigate the catalog to Drop D, then Hear the saved Card or start a new rehearsal run. Its low-string pitches now use Drop D even though the Card still names the saved Standard tuning. Switching to an instrument with fewer strings can also drop saved steps. Independently, moving the live neck start above a saved note hides that dot while Path audio still sounds it: the visual Path filter includes `fret_start`, while the audio filter does not. No explicit Set edit is required for either change.

**Evidence and reachability.** `crates/woodshed-core/src/lib.rs:1341-1348` saves positions, root, instrument, and tuning on the Card. The generic board is built from live `self.tuning()` at `lib.rs:1406`; the Path arm at `lib.rs:1450-1474` uses that board and live neck/string limits. The independently implemented audio Path arm at `lib.rs:1977-1987` again constructs a board from live tuning, with a different filter. The UI saves the Card through `crates/woodshed-views/src/stage.rs:2256-2259`; `stage.rs:1903-1908` returns live Stage outside a running rehearsal, and `stage.rs:2178-2193` routes expanded-Card Hear into this core resolver. Starting a runner captures current exploration (`crates/woodshed-views/src/stage/instances.rs:218-226`), not the Path's saved setup. An already-running rehearsal does snapshot its setup, so this finding does **not** claim that ordinary catalog browsing retunes a currently running owner.

**Authority owner and competitor.** The saved Card's Setting is the material/setup owner. Live Stage navigation competes with it in the Path-specific dot and audio resolvers. This is actual authority duplication: chord/scale Cards already have saved-setup resolution, while Path repeats the older live-Stage path.

**Tests and gap.** `lib.rs:2387-2437` checks saved Path order, named pitches, and note count only under the unchanged Stage setup. It does not change tuning, instrument, or neck visibility after saving. The running-owner snapshot tests establish the caveat above but do not give the Path its authored setup at the beginning of a new run.

**Smallest coherent direction.** Resolve a Path once from the Card's saved/inherited setup using the existing saved-setup seam, preserve exact visit order and repeats, and derive dots and audio from the same realization. Distinguish viewport clipping from playable instructions. Do not silently discard unresolvable positions; report unavailable setup while preserving the Card. Add a saved Path → tuning/instrument navigation → Hear/new Run/reopen regression and a dot/audio agreement test with a mid-neck viewport.

**Dirty P5a status.** The dirty `lib.rs` diff was read; timing return types change, but the Path arm still resolves from live Stage. This remains outside that timing consolidation.

### CORE-5 — P2: the saved-song silent toggle replaces the authored chord with Major

**Trigger and effect.** In the saved-song compatibility editor, make a bar Minor 7, Dominant 7, or another non-Major chord, then click `silent` twice. It becomes Major at the same root. The first click clears its exact authored pitch vector; the second synthesizes a new catalog voicing. This contradicts the method's promise to remember and restore the prior chord and can persist the changed musical material.

**Evidence and reachability.** `crates/woodshed-core/src/song.rs:128-136` clears `formula_name`, assigns `"Major"` on the next toggle, and calls `revoice`. `song.rs:90-95` clears exact pitches, root-frequency and display data when making the bar silent. The live control calls this method in `crates/woodshed-views/src/stage/looper.rs:130-134`; `looper.rs:171-179` mounts the saved-song editor. `crates/woodshed-genet/src/sync.rs:583-590` admits modified saved-song data to the audio backend. The compatibility contract expressly preserves arbitrary saved pitches/meter in `design_docs/2026-07-11_stage_set_tools_plan.md:1848-1860`.

**Authority owner and competitor.** The retained SongBar owns the exact formula/display/pitch payload. The toggle's hardcoded Major fallback becomes a competing source of material. SongDoc's continued existence is separately intentional compatibility work; that does not justify a reversible-looking silence control replacing its payload.

**Tests and gap.** `song.rs:296-311` actually cycles away from Major before toggling, but only verifies that toggling back produces a positive root frequency. The test's "restores a chord" condition accepts replacing the original chord. It does not compare formula, chord label, root frequency or exact pitch-vector bits before and after.

**Smallest coherent direction.** Represent silence independently of the authored payload, or retain the complete prior payload and restore it exactly. Preserve a compatible interpretation of older empty-formula silent bars. Test two toggles on both a non-Major catalog chord and a custom saved pitch vector, including persistence roundtrip. Do not require the broader Song-to-Set migration before fixing this local loss.

**Dirty P5a status.** Read-only primary inspection found this method unchanged; it is outside timing work.

### CORE-6 — P2: stale nonempty Solo marks fall back to sounding the complete chord

**Trigger and effect.** A chord/arpeggio Card has nonempty Solo marks, but those coordinates match none of the current resolved contacts. This can occur after choosing a different selected shape, or with retained/imported stale coordinates. The audio resolver computes an empty Solo selection and substitutes the full chord. The board still excludes every unmarked contact, so sound can contain the whole chord while the visible selection includes nothing. Capturing that arpeggio recipe can also succeed and retain this contradictory instruction.

**Evidence and reachability.** `crates/woodshed-core/src/lib.rs:2034-2057`, especially `2053-2054`, explicitly falls back to `card_voicing` after nonempty marks match no sounding dot. `crates/woodshed-views/src/stage/shapes.rs:11-28` changes shape without clearing marks; the selected shape writer at `crates/woodshed-core/src/card_shapes.rs:467-490` updates only the shape selectors/fingerprint/profile. The board's exclusion policy is `crates/woodshed-views/src/stage.rs:1942-1957`. `crates/woodshed-core/src/captured_arpeggio.rs:121-133` validates marked-contact membership only for scale patterns, so the chord-arpeggio path accepts the nonempty substituted chord at `134-147`. Retained Hear/Add call this preview in `crates/woodshed-views/src/stage/musical_context.rs:527-545`, then queue its sound (`647-672`) or append the instruction (`684-718`). Ordinary selected-Card Hear is also a real consumer, as traced in CORE-4.

**Authority owner and competitor.** Saved marks plus Solo mode own the effective sounding selection. The chord-specific fallback substitutes the unfiltered chord as a second interpretation. Scale (`scale_realization.rs:191-220`) and approach (`chord_approach.rs:265-285`) filtering instead preserve an empty filtered sequence. The deliberate "no marks means inert" behavior at `lib.rs:2035-2037` is a separate case and can remain.

**Tests and gap.** Chord mark tests use valid contacts (`lib.rs:2546-2577`, `card_shapes.rs:638-667`), as do captured-arpeggio tests (`captured_arpeggio.rs:319-339`). Scale-pattern tests reject unavailable retained marks (`captured_arpeggio.rs:412-440`), but chord recipes have no corresponding regression. Shape-selection tests do not assert a consistent sound/display outcome after marks become stale.

**Smallest coherent direction.** Keep an empty effective Solo result empty, or surface unavailable stale marks; never expand it to the unfiltered chord. Share resolved-contact filtering semantics across chord, scale and approach paths. Validate captured chord marks consistently with captured scale-pattern marks. Tests should cover an ordinary shape edit after Solo marking, direct stale retained coordinates, and retained Hear/Add admission, while separately preserving the no-marks inert rule.

**Dirty P5a status.** The inspected dirty primary `lib.rs` still contains this empty-Solo fallback. Timing result propagation alone does not address it; captured marked-contact membership remains scale-pattern-only in the inspected code.

### CORE-7 — P2: Pitch motion has two placement policies for the same sequential material

**Trigger and effect.** Enter Pitch motion with C Major as anchor/focus and disclose its arpeggio. The context arpeggio is placed at its chord's coordinates plus `(36,36)`, moving it off the exact zero-motion ring even though its pitch set is equal. Staging that same arpeggio sends it through a different placement path and into the Unscored shelf because its `arpeggio:` identity has no angular slot. The same supported musical subject therefore changes apparent scoredness merely by becoming an authored occurrence. Scale-pattern and chord-approach context offsets use the same independent mechanism and require the same policy review.

**Evidence and reachability.** `crates/woodshed-core/src/stage_scene.rs:796-813` selects the Pitch motion context and adds arpeggio disclosure. Context coordinates pass through `context_position`, whose sequential branches add Cartesian offsets before dispatching to the reading (`stage_scene.rs:1394-1436`). Authored occurrence coordinates instead call `pitch_motion_reading::subject_position` directly at `stage_scene.rs:607-618`. `crates/woodshed-core/src/pitch_motion_reading.rs:141-170` defines angles only for chord/scale formula IDs, and `189-220` sends absent angular slots to Unscored despite an available pitch-motion result. The underlying music math does support arpeggios: `crates/woodshed-core/src/harmony.rs:109-149,179-182,250-255` resolves the equivalent chord pitch set and minimum motion.

For the zero-ring example, `pitch_motion_reading.rs:85-86` fixes the center at `(-180,0)` and zero radius at `70`. C Major is at `(-180,-70)`; the context offset places its arpeggio at `(-144,-34)`, about `49.5` units from the center, inside that ring. The reading's public contract says every scored subject uses the exact summed distance (`pitch_motion_reading.rs:186-220`), and the mounted UI states that the inner ring is zero and truly Unscored material is on the right (`crates/woodshed-views/src/stage/pitch_motion.rs:34-47`).

**Authority owner and competitor.** The Pitch motion reading owns radial musical meaning. The generic scene adapter independently perturbs positions, and the reading independently declines angular identities for supported sequential subjects. Context and occurrence projections therefore disagree about one reading's semantics.

**Tests and gap.** Exact-radius/triad tests exist in `pitch_motion_reading.rs:251-295`. Scene arpeggio tests cover Tonnetz disclosure (`stage_scene.rs:2280-2305`) and staged identity/relationships (`2489-2535`), not scored placement before/after staging in Pitch motion.

**Smallest coherent direction.** Have the reading own every supported subject's placement for both catalog and occurrence nodes. Give sequential forms stable angular identities and separate selectable nodes by angle while retaining exact radius. Reserve Unscored for genuinely unavailable motion. Add equal-pitch-set arpeggio context/staged tests and equivalent pattern/approach tests; preserve durable occurrence separation without inventing radial distance.

**Dirty P5a status.** Primary `pitch_motion_reading.rs` and `stage_scene.rs` were byte-identical to the frozen snapshot at read-only comparison; timing work does not address this.

### CORE-8 — P2: explicit Context metadata bypasses the reading's edge-disclosure policy

**Trigger and effect.** Stage a C-major Card and choose Tonnetz, or use Circle of Fifths with background nodes and no focused background node. Core now gives context routes an explicit `StageRelationSource::Context`. The view's reading-specific visibility filter is still used only when a route has no source metadata, so the newly explicit Context routes take the generic visible path. Tonnetz can show non-P/L/R shared-tone and other context routes; Circle shows unfocused background-to-background tone routes rather than retaining the quiet fifth-chain landmarks until focus.

**Evidence and reachability.** Core assigns Context metadata in `crates/woodshed-core/src/stage_scene.rs:917-955` and the final-node pass (`1036-1037`, `1083-1084`, `1130-1131`, `1155-1156`, `1240-1241`), with `relation_source()` exposing it at `349-353`. The view's match at `crates/woodshed-views/src/stage.rs:646-738` has a specific Generated arm, then generic `Some(source)` handling with only `set_graph_relation_visible` at line 721. `context_relation_visible` is called solely in the `None` arm at 735. Its actual reading/focus policy is defined at `stage.rs:503-537`; the generic helper at `356-362` only checks the Next-family setting and explicit hidden members. The Set inventory intentionally excludes Context (`stage.rs:450-453`), so that inventory does not replace the bypassed context controls. Native `crates/woodshed-genet/src/leaves.rs:402-446` takes this swatch and paints it with those visibility decisions.

**Authority owner and competitor.** Core owns the complete available relation inventory; the Woodshed view owns disclosure policy. The consumer still classifies a context route by missing metadata while the producer now names its context identity explicitly. Two classifications of the same route disagree, causing a real policy bypass. This is not a reason to delete available core facts.

**Tests and gap.** Core relation-count and full availability tests (`stage_scene.rs:2228-2373`) correctly exercise inventory. The view Tonnetz test (`crates/woodshed-views/src/stage/context_tests.rs:23-66`) checks expansion/placement, not visible relation kinds. Design rules for quiet/focused Circle and highlighted Tonnetz families are in `design_docs/2026-09-04_musical_projections_plan.md:141-145,176-182`.

**Smallest coherent direction.** Handle `StageRelationSource::Context` explicitly and apply `context_relation_visible` there, combined with generic explicit member hiding if intended. Source absence should mean unavailable metadata, rather than act as the product's context classification. Add view-level assertions for a remote unfocused Circle tone route and for Tonnetz's allowed P/L/R context families, then retain the full underlying inventory.

**Dirty P5a status.** This finding is established against the frozen source. The producer `stage_scene.rs` remains byte-identical in the primary checkout; current dirty view code was not accepted or exhaustively re-audited. No claim of a P5a fix is made.

## Lower-priority consolidation and cruft candidates

### CORE-C1 — P3 candidate: keyed relation derivation is repeated in the builder and scene adapter

`crates/woodshed-core/src/stage_context.rs:504-574` derives shared-tone, fifth, and diatonic relations. `crates/woodshed-core/src/stage_scene.rs:959-1245` derives those facts again over the final displayed nodes and adds arpeggiation/pattern/approach facts. String prefixes and relation slugs are reinterpreted in that adapter. This is real repeated computation, although the intended visual-strength policy remains a product question.

A concrete difference appears in Circle with C staged and G/D as background. The builder's background G–D Fifth has `shared_tones:0` (`stage_context.rs:538-545`), and the adapter maps every builder relation's `shared_tones` to weight `max(1)/12` (`stage_scene.rs:944`). A Fifth involving a staged endpoint is instead supplied by the final-node pass at weight `1.0` (`1173-1191`), with deduplication preserving whichever producer supplied the route first. The builder's field documentation explicitly says `shared_tones` is meaningful only for its corresponding kind (`stage_context.rs:90-98`). A separate explicit background-strength policy may be wanted; the current result is an incidental consequence of a shared-tone formula applied to Fifth.

The two paths also differ in source identity: builder-derived `StageRelationSource::Context` keeps pre-remap catalog endpoints (`stage_scene.rs:950-954`) after displayed focus endpoints are changed to a Card (`918-927`), while the final-node pass derives the actual displayed node IDs in `context_source` (`1371-1384`). The Context variant's documented exact-displayed-node contract is at `stage_scene.rs:164-169`.

Smallest direction: candidate builders choose nodes; a single product-owned reading pass derives relations over the final displayed `StageNodeId`s with explicit per-kind weighting. Tonnetz already describes and uses that division (`tonnetz.rs:79-85,150-153`); the musical projections plan asks for a product reading model at `327-329`. Keep musical policy in Woodshed. Add cross-path identity/weight tests once the strength policy is decided. No new native failure is claimed, and the visual strength difference is intentionally kept a candidate rather than a confirmed behavioral defect.

### CORE-C2 — P3 candidate: `StageGraphSnapshot::reasons()` recomputes live Set facts and ignores its snapshot

`crates/woodshed-core/src/stage_scene.rs:509-526` takes `&self` but never reads it. It resolves Card IDs against a separately supplied Set and calls `relations_between()` again. Retaining a snapshot, editing the supplied Set, then calling `old_snapshot.reasons(...)` returns reasons for the edited material, ignoring captured source inventory, context, filtering and epoch qualification. This competes with the snapshot's exact `relation_sources` inventory and is misleading for a method documented as the fanned-edge detail query.

Repository-wide call tracing found only tests at `stage_scene.rs:1924,2515`; the production view uses `relation_detail()`/`relation_source()`. Thus this is public-API cruft and a contract hazard, not an established current product failure. Existing tests exercise the recomputation without changing the Set after snapshot construction. Remove/deprecate the method if compatibility permits and test the captured Generated source instead. If retained for external callers, rename/document it as a live catalog-pair query and move it off the snapshot type.

## Known dirty P5a work, kept separate

The frozen timing defect is already documented in `design_docs/2026-07-11_stage_set_tools_plan.md:1748-1798`: automatic dwell (`lib.rs:613-630`) and two ordered-preview implementations independently interpret the Card hold/BPM, with inconsistent upper/lower tempo handling and an infallible duration conversion for extreme imported Seconds. Treat that as the known P5a lane, not a newly discovered unowned task.

A read-only inspection of `/Users/markik/Code/repos/woodshed` found active dirty changes adding `crates/woodshed-core/src/card_timing.rs::resolve_card_timing`, fallible `card_dwell`, and delegation from the ordered arpeggio/scale paths. The primary checkout's observed HEAD was `1bc4f6ab347cc5c5fcc11ed937f1de2b75614835`; the dirty files are not the frozen published audit target. The core resolver and relevant diffs were read solely to separate this work from outstanding findings. No build, test, native, or acceptance status for those dirty changes is claimed.

## Deliberate authority and false-positive rebuttals

- `SessionStore` is a product persistence coordinator over `muniment::Backend`; it no longer duplicates a one-slot storage trait. Session/settings slots answer separate durable questions. Readback and per-slot error state are intentional, and a successful session save does not erase a failed settings outcome.
- `SealedBackend` composes persona sealing around the shared backend; Woodshed's seal context is product-owned. The explicit plaintext-adoption compatibility option is documented as a heuristic and is not an unconditional normal read.
- `PracticeHistory` delegates dated branching structure to Chartulary/Stemma, while engagement semantics and evidence policy remain Woodshed-owned. Stated origin and observed adjacency are intentionally different facts. The findings concern admission and disconnected production, not removing that distinction.
- `PersonalRelationSource` uses Chartulary's journal and exact assertion IDs. It does not duplicate generated catalog relation authority. It checks runtime generation, actor ownership, endpoints, and held lifecycle before mutation; multiple reasons remain separate assertions.
- `PersistedSession::capture` initializes optional host-owned attachments for the embedding to fill; the actual `UiState::to_persisted` overlays the retained banks/readings/relations. The root helper alone is not evidence of real-session data loss.
- Saved progression arrangements deliberately retain musical roles/tuning/alignment independently of optional portable scene export. Their own catalog/setup validation refuses silent retargeting. Retained snapshot copies and active/inactive working Sets are distinct lifecycle operations rather than two editable owners of one occurrence.
- `SongDoc` and its editor remain a declared P7 migration frontier: Cards cannot yet losslessly represent every retained bar's arbitrary pitch vector and individual meter (`Stage plan:1848-1860`). Removing or freezing that editor without resolving representation would lose capability. CORE-5 identifies a defect within this compatibility path, not an authorization to delete it.
- `LoopPlan` is a disposable Set-owned playback snapshot with stable Set/Card qualification and explicit clock/readiness refusals. Its transient segment list is not another saved Set. Legacy and canonical capture clocks/codec representations are explicit compatibility contracts, not interchangeable duplicated truth.
- The export examples are bounded fixture/proof producers, not live comparison authorities. Graph arrangement policy is intentionally local pending a real second consumer. `AudioBackend`'s permissive defaults deserve care for future hosts, but the current desktop overrides substantive operations and the browser only mounts its declared S0 demo; this audit did not establish a current production false-success defect from those defaults.
- Riff audition is explicitly unavailable and deferred in the Stage plan (`1123-1124`); empty sound alone is not a new defect claim. Path is reported separately because it claims to save playable instructions with an explicit setup and actively sounds them through a competing setup.
- The three graph surfaces answer different questions: Set occurrences, joined catalog/history Mere, and session inventory. The session overview separately qualifies the running Rehearsal owner and admitted Looper owner; loop-take nodes are manifest references/navigation subjects, not PCM or playback authorities. Complete projection/bank review found no basis for collapsing these into a second mutable owner.
- The pure owned theory model is an explicit product requirement. Concrete pitch-set comparison/minimum-motion math already delegates to `woodshedding::pitch_class_set`; root/formula identity, physical shape movement, acoustic note filtering and occurrence identity have different meanings. Bounded selected-shape inventories disclose their profile/limits and refuse fingerprint drift.
- `CapturedRecipe` is an explicit copied instruction with historical provenance and fresh-occurrence Add semantics; it is not a live alias of the source Set. Formula-level comparison also discloses its limits and uses the shared compiler for the retained relation reading. CORE-6 concerns an inconsistent selection/admission policy inside those otherwise deliberate boundaries.

## Unresolved questions and cross-partition follow-up

No assigned file or assigned line remains unread. These product/API decisions remain open and are not substituted for coverage:

- Define the effective outcome for a retained Path with unavailable setup/contacts: visible refusal versus an explicitly authored partial migration. Silent retuning/dropping is the concrete current defect in CORE-4.
- Decide how exact discovery-origin evidence is carried into a genuine practice boundary, without confusing stated choice with walked adjacency or counting both lifecycle start and completion as separate accepted choices (CORE-3).
- State the intended background/context strength policy before changing the duplicate relation derivations (CORE-C1); confirm any external compatibility promise for the unused public `reasons()` method before removal (CORE-C2).
- Clarify the catalog arpeggio Hear/Run/Stage realization contract. Current catalog Run/Step uses generated windows plus `ArpeggioRun`; generic catalog Hear uses formula preview, context Hear uses Card articulation, and Card staging retains direction/inversion but not `arpeggio_position_idx`. The plans leave portions of articulation/realization open, so this was withheld as a separate defect pending a product ruling.
- The eventual lossless saved-song-to-Set representation and malformed imported owner-bank identity policy remain broader migration/admission questions. The saved-song compatibility editor should not be removed merely because it is a second visible workflow.

The parent audit should reconcile findings that cross into the view/host partition, especially CORE-3, CORE-4, CORE-6 and CORE-8, using their exact frozen callers above. The eight confirmed findings are source-supported behaviors, not independent native, fresh-process, physical-audio or current-dirty acceptance claims. The report and coverage ledger are the only files written by this partition.
